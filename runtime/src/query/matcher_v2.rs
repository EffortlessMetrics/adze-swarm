// Enhanced query pattern matching with predicate evaluation
use super::ast::*;
use super::predicate_eval::PredicateContext;
#[cfg(feature = "query")]
use crate::document::{AdzeDocument, LanguageMetadata, NodeId, SyntaxTree};
use crate::parser_v4::ParseNode;
use adze_glr_core::SymbolMetadata;
use adze_ir::SymbolId;
use std::collections::HashMap;
use std::ops::Range;

/// A match of a query pattern
#[derive(Debug, Clone)]
pub struct QueryMatch {
    /// Pattern index that matched
    pub pattern_index: usize,
    /// Captured nodes
    pub captures: Vec<QueryCapture>,
}

/// A captured node in a query match
#[derive(Debug, Clone)]
pub struct QueryCapture {
    /// Capture index
    pub index: u32,
    /// The captured node
    pub node: ParseNode,
}

/// A match of a query pattern against a document's flat records.
///
/// Only available with the `query` feature: this is the document-backed
/// matcher entry.
#[cfg(feature = "query")]
#[derive(Debug, Clone)]
pub struct DocumentQueryMatch {
    /// Pattern index that matched
    pub pattern_index: usize,
    /// Captured document-local node ids, in (capture ID, occurrence) order
    pub captures: Vec<DocumentQueryCapture>,
}

/// A captured document node in a query match.
///
/// Only available with the `query` feature, alongside
/// [`DocumentQueryMatch`].
#[cfg(feature = "query")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentQueryCapture {
    /// Capture index
    pub index: u32,
    /// The captured document-local node id
    pub node_id: NodeId,
}

/// A node view for the single shared matching engine.
///
/// The engine traverses, captures, and backtracks through this interface;
/// [`ParsedView`] borrows a recursive tree and [`FlatView`] resolves
/// document-local flat records. Both visit preorder identically, so capture
/// order agrees across backends. All views are cheap to copy, so traversal
/// never moves out of a borrowed handle.
trait ViewNode<'s>: Clone + Copy {
    /// Grammar symbol id used for pattern-symbol comparison
    fn symbol(self) -> SymbolId;
    /// Byte range covered by this node
    fn byte_range(self) -> Range<usize>;
    /// Field name attached to the edge from this node's parent, if any
    fn field_name(self) -> Option<&'s str>;
    /// Number of direct children
    fn child_count(self) -> usize;
    /// Child node by child index
    fn child(self, index: usize) -> Option<Self>;
    /// Whether this node counts as named for pattern matching
    fn is_named(self) -> bool;
    /// Whether this node is extra (skipped while sequencing)
    fn is_extra(self) -> bool;
    /// Source text covered by this node, if available and well-formed
    fn text(self) -> Option<&'s str>;
}

/// Recursive-tree backend (compatibility entry points).
#[derive(Clone, Copy)]
struct ParsedView<'s> {
    node: &'s ParseNode,
    source: &'s str,
    symbol_metadata: &'s [SymbolMetadata],
}

impl<'s> ViewNode<'s> for ParsedView<'s> {
    fn symbol(self) -> SymbolId {
        self.node.symbol
    }

    fn byte_range(self) -> Range<usize> {
        self.node.start_byte..self.node.end_byte
    }

    fn field_name(self) -> Option<&'s str> {
        self.node.field_name.as_deref()
    }

    fn child_count(self) -> usize {
        self.node.children.len()
    }

    fn child(self, index: usize) -> Option<Self> {
        self.node.children.get(index).map(|node| ParsedView {
            node,
            source: self.source,
            symbol_metadata: self.symbol_metadata,
        })
    }

    fn is_named(self) -> bool {
        self.symbol_metadata
            .get(self.node.symbol.0 as usize)
            .map(|metadata| metadata.is_named)
            .unwrap_or(true)
    }

    fn is_extra(self) -> bool {
        self.symbol_metadata
            .get(self.node.symbol.0 as usize)
            .map(|metadata| metadata.is_extra)
            .unwrap_or(false)
    }

    fn text(self) -> Option<&'s str> {
        let range = self.byte_range();
        self.source.get(range)
    }
}

/// Flat-record backend (document entry points, `query` feature only).
#[cfg(feature = "query")]
#[derive(Clone, Copy)]
struct FlatView<'s> {
    syntax: &'s SyntaxTree,
    language: &'s LanguageMetadata,
    source: &'s str,
    id: NodeId,
}

#[cfg(feature = "query")]
impl<'s> FlatView<'s> {
    fn record(self) -> Option<&'s crate::document::NodeRecord> {
        self.syntax.node_record(self.id)
    }
}

#[cfg(feature = "query")]
impl<'s> ViewNode<'s> for FlatView<'s> {
    fn symbol(self) -> SymbolId {
        // Traversal only visits validated ids; the fallback never matches a
        // real pattern symbol in a well-formed tree.
        self.record()
            .map(|record| record.grammar_id())
            .unwrap_or(SymbolId(0))
    }

    fn byte_range(self) -> Range<usize> {
        self.record()
            .map(|record| record.byte_range())
            .unwrap_or(0..0)
    }

    fn field_name(self) -> Option<&'s str> {
        self.syntax
            .parent_edge_record(self.id)
            .and_then(|edge| edge.field_id())
            .and_then(|field_id| self.language.field_name_for_id(field_id.get()))
    }

    fn child_count(self) -> usize {
        self.record()
            .map(|record| record.edge_range().len())
            .unwrap_or(0)
    }

    fn child(self, index: usize) -> Option<Self> {
        self.syntax.child_id(self.id, index).map(|id| FlatView {
            syntax: self.syntax,
            language: self.language,
            source: self.source,
            id,
        })
    }

    fn is_named(self) -> bool {
        // Alias-aware named-ness, consistent with the document's own typed
        // CST/JSON/ts-compat projections.
        self.record()
            .and_then(|record| self.language.symbol(record.visible_id()))
            .map(|kind| kind.is_named())
            .unwrap_or(true)
    }

    fn is_extra(self) -> bool {
        self.record()
            .and_then(|record| self.language.symbol(record.visible_id()))
            .map(|kind| kind.is_extra())
            .unwrap_or(false)
    }

    fn text(self) -> Option<&'s str> {
        let range = self.byte_range();
        self.source.get(range)
    }
}

/// State for matching a pattern
///
/// A capture ID identifies a capture slot, not one node: every occurrence a
/// supported pattern selects is pushed in visit order. Predicates evaluate
/// all occurrences under universal semantics (documented at
/// [`PredicateContext::evaluate_texts`]); output preserves occurrence order
/// within each ID via a stable index sort.
#[derive(Clone, Debug)]
struct MatchState<N> {
    /// Current captures: one entry per occurrence, in match order.
    captures: HashMap<u32, Vec<N>>,
}

/// One pattern's accepted captures in a backend-neutral shape.
struct GenericMatch<N> {
    pattern_index: usize,
    /// (capture ID, occurrences in visit order), sorted by capture ID.
    captures: Vec<(u32, Vec<N>)>,
}

#[derive(Clone, Copy)]
struct RepeatSearch {
    min_count: usize,
    anchored_next: bool,
}

/// Query pattern matcher with source text
pub struct QueryMatcher<'a> {
    query: &'a Query,
    source: &'a str,
    symbol_metadata: &'a [SymbolMetadata],
}

impl<'a> QueryMatcher<'a> {
    /// Create a new query matcher with source text
    pub fn new(query: &'a Query, source: &'a str, symbol_metadata: &'a [SymbolMetadata]) -> Self {
        QueryMatcher {
            query,
            source,
            symbol_metadata,
        }
    }

    /// Match all patterns in the query against a parse tree
    pub fn matches(&self, root: &ParseNode) -> Vec<QueryMatch> {
        let view = ParsedView {
            node: root,
            source: self.source,
            symbol_metadata: self.symbol_metadata,
        };
        self.match_all(view)
            .into_iter()
            .map(|generic| {
                let mut captures: Vec<_> = generic
                    .captures
                    .into_iter()
                    .flat_map(|(index, nodes)| {
                        nodes.into_iter().map(move |view| QueryCapture {
                            index,
                            node: view.node.clone(),
                        })
                    })
                    .collect();
                captures.sort_by_key(|capture| capture.index);
                QueryMatch {
                    pattern_index: generic.pattern_index,
                    captures,
                }
            })
            .collect()
    }

    /// Match all patterns in the query against a native parse document.
    ///
    /// This is the selected document-backed entry: traversal, ranges, and
    /// flags resolve from canonical flat records, and captures are
    /// document-local node ids. The recursive-tree [`matches`] entry remains
    /// as the compatibility path for hand-built trees.
    #[cfg(feature = "query")]
    pub fn matches_document(&self, document: &AdzeDocument) -> Vec<DocumentQueryMatch> {
        let syntax = document.syntax();
        let Some(_) = syntax.node_record(syntax.root_id()) else {
            return Vec::new();
        };
        let view = FlatView {
            syntax,
            language: document.language(),
            source: document.source_text(),
            id: syntax.root_id(),
        };
        self.match_all(view)
            .into_iter()
            .map(|generic| {
                let captures: Vec<_> = generic
                    .captures
                    .into_iter()
                    .flat_map(|(index, nodes)| {
                        nodes.into_iter().map(move |node| DocumentQueryCapture {
                            index,
                            node_id: node.id,
                        })
                    })
                    .collect();
                DocumentQueryMatch {
                    pattern_index: generic.pattern_index,
                    captures,
                }
            })
            .collect()
    }

    /// Run every pattern against one root view of either backend.
    fn match_all<'s, V>(&self, root: V) -> Vec<GenericMatch<V>>
    where
        V: ViewNode<'s>,
    {
        let mut matches = Vec::new();

        // Try each pattern
        for (pattern_index, pattern) in self.query.patterns.iter().enumerate() {
            self.match_pattern(pattern_index, pattern, root, &mut matches);
        }

        matches
    }

    /// Match a single pattern against the tree
    fn match_pattern<'s, V>(
        &self,
        pattern_index: usize,
        pattern: &Pattern,
        root: V,
        matches: &mut Vec<GenericMatch<V>>,
    ) where
        V: ViewNode<'s>,
    {
        // Walk the tree and try to match at each node
        self.match_pattern_at_node(pattern_index, pattern, root, matches);
    }

    /// Try to match pattern starting at a specific node
    fn match_pattern_at_node<'s, V>(
        &self,
        pattern_index: usize,
        pattern: &Pattern,
        node: V,
        matches: &mut Vec<GenericMatch<V>>,
    ) where
        V: ViewNode<'s>,
    {
        // Try to match the pattern at this node
        let mut state = MatchState {
            captures: HashMap::new(),
        };

        if self.match_node(&pattern.root, node, &mut state) {
            // Check predicates against every occurrence's text. A constraint
            // holds only when all occurrences satisfy it.
            let texts: HashMap<u32, Vec<Option<&str>>> = state
                .captures
                .iter()
                .map(|(id, nodes)| {
                    (
                        *id,
                        nodes.iter().map(|candidate| candidate.text()).collect(),
                    )
                })
                .collect();
            let predicate_ctx = PredicateContext::for_resolved_texts();
            if pattern
                .predicates
                .iter()
                .all(|pred| predicate_ctx.evaluate_texts(pred, &texts))
            {
                // One entry per occurrence, sorted by capture ID with
                // occurrence order preserved (stable sort over
                // push-ordered vectors).
                let mut captures: Vec<_> = state.captures.into_iter().collect();
                captures.sort_by_key(|(index, _)| *index);

                matches.push(GenericMatch {
                    pattern_index,
                    captures,
                });
            }
        }

        // Recursively try child nodes
        for index in 0..node.child_count() {
            if let Some(child) = node.child(index) {
                self.match_pattern_at_node(pattern_index, pattern, child, matches);
            }
        }
    }

    /// Match a pattern node against a parse node.
    ///
    /// The match is atomic with respect to the caller's state: the node's
    /// own capture and every nested attempt run on a branch-local copy, and
    /// only a successful match commits. A failed node therefore leaves no
    /// residue, including its own capture ID.
    fn match_node<'s, V>(&self, pattern: &PatternNode, node: V, state: &mut MatchState<V>) -> bool
    where
        V: ViewNode<'s>,
    {
        // Check symbol
        if pattern.symbol != node.symbol() {
            return false;
        }
        // Confirm named/anonymous status matches the pattern expectation
        if self.node_is_named(node) != pattern.is_named {
            return false;
        }

        let mut attempt = state.clone();
        // Capture if needed. Occurrences accumulate; nothing overwrites.
        if let Some(capture_id) = pattern.capture {
            attempt.captures.entry(capture_id).or_default().push(node);
        }

        // Match based on quantifier. The node's own capture above stays
        // staged in `attempt` until the whole node is known to match; only
        // Optional may succeed vacuously, and then only a successful inner
        // attempt commits (otherwise the staged capture vanishes too).
        let (matched, commit) = match pattern.quantifier {
            Quantifier::One => (self.match_children_one(pattern, node, &mut attempt), true),
            Quantifier::Optional => (true, self.match_children_one(pattern, node, &mut attempt)),
            Quantifier::Plus => (self.match_children_plus(pattern, node, &mut attempt), true),
            Quantifier::Star => (self.match_children_star(pattern, node, &mut attempt), true),
        };
        if matched && commit {
            *state = attempt;
        }
        matched
    }

    /// Match children with One quantifier
    fn match_children_one<'s, V>(
        &self,
        pattern: &PatternNode,
        node: V,
        state: &mut MatchState<V>,
    ) -> bool
    where
        V: ViewNode<'s>,
    {
        // Check field assertions
        for (field_name, field_pattern) in &pattern.fields {
            // Find child with this field name
            let field_node = (0..node.child_count())
                .filter_map(|index| node.child(index))
                .find(|child| child.field_name() == Some(field_name.as_str()));

            if let Some(field_node) = field_node {
                if !self.match_node(field_pattern, field_node, state) {
                    return false;
                }
            } else {
                return false; // Required field not found
            }
        }

        // If pattern has explicit children, match them
        if !pattern.children.is_empty() {
            return self.match_child_sequence(&pattern.children, node, 0, 0, state);
        }

        true
    }

    /// Match children with Plus quantifier
    fn match_children_plus<'s, V>(
        &self,
        pattern: &PatternNode,
        node: V,
        state: &mut MatchState<V>,
    ) -> bool
    where
        V: ViewNode<'s>,
    {
        // Must match at least once
        if !self.match_children_one(pattern, node, state) {
            return false;
        }

        // Try to match more (simplified - in reality would need backtracking)
        true
    }

    /// Match children with Star quantifier
    fn match_children_star<'s, V>(
        &self,
        pattern: &PatternNode,
        node: V,
        state: &mut MatchState<V>,
    ) -> bool
    where
        V: ViewNode<'s>,
    {
        // Star always matches (zero or more)
        self.match_children_plus(pattern, node, state);
        true
    }

    /// Determine if a node should be treated as named.
    fn node_is_named<'s, V>(&self, node: V) -> bool
    where
        V: ViewNode<'s>,
    {
        node.is_named()
    }

    /// Determine if a node should be treated as an "extra" node that should
    /// be ignored during pattern matching.
    fn node_is_extra<'s, V>(&self, node: V) -> bool
    where
        V: ViewNode<'s>,
    {
        node.is_extra()
    }

    /// Match a sequence of pattern children against node children
    fn match_child_sequence<'s, V>(
        &self,
        pattern_children: &[PatternChild],
        parent: V,
        pattern_idx: usize,
        node_idx: usize,
        state: &mut MatchState<V>,
    ) -> bool
    where
        V: ViewNode<'s>,
    {
        if let Some(next_state) = self.match_child_sequence_from(
            pattern_children,
            parent,
            pattern_idx,
            node_idx,
            state.clone(),
            false,
        ) {
            *state = next_state;
            true
        } else {
            false
        }
    }

    fn match_child_sequence_from<'s, V>(
        &self,
        pattern_children: &[PatternChild],
        parent: V,
        pattern_idx: usize,
        node_idx: usize,
        state: MatchState<V>,
        anchored_next: bool,
    ) -> Option<MatchState<V>>
    where
        V: ViewNode<'s>,
    {
        // Base case: all patterns matched
        if pattern_idx >= pattern_children.len() {
            // If extra nodes remain, ensure they're ignorable
            return Some(state);
        }

        let mut node_idx = node_idx;
        // Skip over any extra nodes before attempting to match
        while node_idx < parent.child_count()
            && parent
                .child(node_idx)
                .is_some_and(|child| self.node_is_extra(child))
        {
            node_idx += 1;
        }

        // Base case: no more nodes but patterns remain
        if node_idx >= parent.child_count() {
            // Check if remaining patterns are all optional
            return pattern_children[pattern_idx..]
                .iter()
                .all(|p| {
                    matches!(p, PatternChild::Anchor)
                        || matches!(
                            p,
                            PatternChild::Node(n)
                                if matches!(n.quantifier, Quantifier::Optional | Quantifier::Star)
                        )
                })
                .then_some(state);
        }

        // Try to match current pattern
        match &pattern_children[pattern_idx] {
            PatternChild::Anchor => self
                .anchor_satisfied(pattern_children, pattern_idx, parent, node_idx)
                .then(|| {
                    self.match_child_sequence_from(
                        pattern_children,
                        parent,
                        pattern_idx + 1,
                        node_idx,
                        state,
                        true,
                    )
                })
                .flatten(),
            PatternChild::Node(pattern_node) => match pattern_node.quantifier {
                Quantifier::One => self.match_single_child_candidate(
                    pattern_node,
                    pattern_children,
                    parent,
                    (pattern_idx, node_idx),
                    state,
                    anchored_next,
                ),
                Quantifier::Optional => self
                    .match_single_child_candidate(
                        pattern_node,
                        pattern_children,
                        parent,
                        (pattern_idx, node_idx),
                        state.clone(),
                        anchored_next,
                    )
                    .or_else(|| {
                        self.match_child_sequence_from(
                            pattern_children,
                            parent,
                            pattern_idx + 1,
                            node_idx,
                            state,
                            anchored_next,
                        )
                    }),
                Quantifier::Plus => self.match_repeated_child_candidates(
                    pattern_node,
                    pattern_children,
                    parent,
                    (pattern_idx, node_idx),
                    state,
                    RepeatSearch {
                        min_count: 1,
                        anchored_next,
                    },
                ),
                Quantifier::Star => self
                    .match_repeated_child_candidates(
                        pattern_node,
                        pattern_children,
                        parent,
                        (pattern_idx, node_idx),
                        state.clone(),
                        RepeatSearch {
                            min_count: 0,
                            anchored_next,
                        },
                    )
                    .or_else(|| {
                        self.match_child_sequence_from(
                            pattern_children,
                            parent,
                            pattern_idx + 1,
                            node_idx,
                            state,
                            anchored_next,
                        )
                    }),
            },
            PatternChild::Token(token) => self.match_token_candidate(
                token,
                pattern_children,
                parent,
                (pattern_idx, node_idx),
                state,
                anchored_next,
            ),
        }
    }

    fn anchor_satisfied<'s, V>(
        &self,
        pattern_children: &[PatternChild],
        pattern_idx: usize,
        parent: V,
        node_idx: usize,
    ) -> bool
    where
        V: ViewNode<'s>,
    {
        if pattern_idx == 0 {
            (0..node_idx).all(|index| {
                parent
                    .child(index)
                    .is_some_and(|child| self.node_is_extra(child))
            })
        } else if pattern_idx + 1 == pattern_children.len() {
            (node_idx..parent.child_count()).all(|index| {
                parent
                    .child(index)
                    .is_some_and(|child| self.node_is_extra(child))
            })
        } else {
            true
        }
    }

    fn candidate_indices<'s, V>(
        &self,
        parent: V,
        node_idx: usize,
        anchored_next: bool,
    ) -> Vec<usize>
    where
        V: ViewNode<'s>,
    {
        if anchored_next {
            (node_idx < parent.child_count())
                .then_some(node_idx)
                .into_iter()
                .collect()
        } else {
            (node_idx..parent.child_count())
                .filter(|idx| {
                    parent
                        .child(*idx)
                        .is_some_and(|child| !self.node_is_extra(child))
                })
                .collect()
        }
    }

    fn match_single_child_candidate<'s, V>(
        &self,
        pattern_node: &PatternNode,
        pattern_children: &[PatternChild],
        parent: V,
        position: (usize, usize),
        state: MatchState<V>,
        anchored_next: bool,
    ) -> Option<MatchState<V>>
    where
        V: ViewNode<'s>,
    {
        let (pattern_idx, node_idx) = position;

        self.candidate_indices(parent, node_idx, anchored_next)
            .into_iter()
            .filter_map(|candidate_idx| {
                let child = parent.child(candidate_idx)?;
                self.match_child_node_once(pattern_node, child, state.clone())
                    .and_then(|next_state| {
                        self.match_child_sequence_from(
                            pattern_children,
                            parent,
                            pattern_idx + 1,
                            candidate_idx + 1,
                            next_state,
                            false,
                        )
                    })
            })
            .next()
    }

    fn match_repeated_child_candidates<'s, V>(
        &self,
        pattern_node: &PatternNode,
        pattern_children: &[PatternChild],
        parent: V,
        position: (usize, usize),
        state: MatchState<V>,
        repeat: RepeatSearch,
    ) -> Option<MatchState<V>>
    where
        V: ViewNode<'s>,
    {
        let (_, node_idx) = position;

        self.candidate_indices(parent, node_idx, repeat.anchored_next)
            .into_iter()
            .find_map(|candidate_idx| {
                self.match_repeated_child_node(
                    pattern_node,
                    pattern_children,
                    parent,
                    (position.0, candidate_idx),
                    state.clone(),
                    repeat.min_count,
                )
            })
    }

    fn match_token_candidate<'s, V>(
        &self,
        token: &str,
        pattern_children: &[PatternChild],
        parent: V,
        position: (usize, usize),
        state: MatchState<V>,
        anchored_next: bool,
    ) -> Option<MatchState<V>>
    where
        V: ViewNode<'s>,
    {
        let (pattern_idx, node_idx) = position;

        self.candidate_indices(parent, node_idx, anchored_next)
            .into_iter()
            .filter_map(|candidate_idx| {
                let child = parent.child(candidate_idx)?;
                (child.text() == Some(token))
                    .then(|| {
                        self.match_child_sequence_from(
                            pattern_children,
                            parent,
                            pattern_idx + 1,
                            candidate_idx + 1,
                            state.clone(),
                            false,
                        )
                    })
                    .flatten()
            })
            .next()
    }

    fn match_child_node_once<'s, V>(
        &self,
        pattern_node: &PatternNode,
        node: V,
        mut state: MatchState<V>,
    ) -> Option<MatchState<V>>
    where
        V: ViewNode<'s>,
    {
        let mut single = pattern_node.clone();
        single.quantifier = Quantifier::One;

        self.match_node(&single, node, &mut state).then_some(state)
    }

    fn match_repeated_child_node<'s, V>(
        &self,
        pattern_node: &PatternNode,
        pattern_children: &[PatternChild],
        parent: V,
        position: (usize, usize),
        state: MatchState<V>,
        min_count: usize,
    ) -> Option<MatchState<V>>
    where
        V: ViewNode<'s>,
    {
        let (pattern_idx, node_idx) = position;
        let mut candidates = vec![(node_idx, state)];
        let mut cursor = node_idx;

        while cursor < parent.child_count() {
            let (_, last_state) = candidates.last().expect("candidate seed exists");
            let Some(child) = parent.child(cursor) else {
                break;
            };
            let Some(next_state) =
                self.match_child_node_once(pattern_node, child, last_state.clone())
            else {
                break;
            };
            cursor += 1;
            candidates.push((cursor, next_state));
        }

        candidates
            .into_iter()
            .enumerate()
            .rev()
            .filter(|(count, _)| *count >= min_count)
            .find_map(|(_, (next_node_idx, next_state))| {
                self.match_child_sequence_from(
                    pattern_children,
                    parent,
                    pattern_idx + 1,
                    next_node_idx,
                    next_state,
                    false,
                )
            })
    }
}

/// Iterator over query matches
pub struct QueryMatches<'a> {
    #[allow(dead_code)]
    matcher: QueryMatcher<'a>,
    #[allow(dead_code)]
    root: &'a ParseNode,
    #[allow(dead_code)]
    pattern_index: usize,
    matches: Vec<QueryMatch>,
    current_index: usize,
}

impl<'a> QueryMatches<'a> {
    /// Create a new query matches iterator
    pub fn new(
        query: &'a Query,
        root: &'a ParseNode,
        source: &'a str,
        symbol_metadata: &'a [SymbolMetadata],
    ) -> Self {
        let matcher = QueryMatcher::new(query, source, symbol_metadata);
        let matches = matcher.matches(root);
        QueryMatches {
            matcher,
            root,
            pattern_index: 0,
            matches,
            current_index: 0,
        }
    }
}

impl<'a> Iterator for QueryMatches<'a> {
    type Item = QueryMatch;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index < self.matches.len() {
            let match_item = self.matches[self.current_index].clone();
            self.current_index += 1;
            Some(match_item)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::compile_query;
    use adze_glr_core::SymbolMetadata;
    use adze_ir::{Grammar, SymbolId, Token, TokenPattern};

    fn make_node(symbol: u16, start: usize, end: usize) -> ParseNode {
        let symbol_id = SymbolId(symbol);
        ParseNode {
            symbol: symbol_id,
            symbol_id,
            children: vec![],
            start_byte: start,
            end_byte: end,
            field_name: None,
            alias_symbol_id: None,
        }
    }

    fn make_field_node(symbol: u16, start: usize, end: usize, field_name: &str) -> ParseNode {
        let mut node = make_node(symbol, start, end);
        node.field_name = Some(field_name.to_string());
        node
    }

    fn make_root(children: Vec<ParseNode>, end_byte: usize) -> ParseNode {
        ParseNode {
            symbol: SymbolId(0),
            symbol_id: SymbolId(0),
            children,
            start_byte: 0,
            end_byte,
            field_name: None,
            alias_symbol_id: None,
        }
    }

    fn create_test_grammar() -> Grammar {
        let mut grammar = Grammar::new("test".to_string());
        grammar.tokens.insert(
            SymbolId(1),
            Token {
                name: "identifier".to_string(),
                pattern: TokenPattern::Regex("[a-zA-Z]+".to_string()),
                fragile: false,
            },
        );
        grammar.tokens.insert(
            SymbolId(2),
            Token {
                name: "number".to_string(),
                pattern: TokenPattern::Regex("[0-9]+".to_string()),
                fragile: false,
            },
        );
        grammar.tokens.insert(
            SymbolId(3),
            Token {
                name: "operator".to_string(),
                pattern: TokenPattern::String("+".to_string()),
                fragile: false,
            },
        );
        grammar
    }

    fn test_symbol_metadata() -> Vec<SymbolMetadata> {
        vec![
            SymbolMetadata {
                name: "root".to_string(),
                is_visible: true,
                is_named: true,
                is_supertype: false,
                is_terminal: false,
                is_extra: false,
                is_fragile: false,
                symbol_id: SymbolId(0),
            },
            SymbolMetadata {
                name: "identifier".to_string(),
                is_visible: true,
                is_named: true,
                is_supertype: false,
                is_terminal: true,
                is_extra: false,
                is_fragile: false,
                symbol_id: SymbolId(1),
            },
            SymbolMetadata {
                name: "number".to_string(),
                is_visible: true,
                is_named: true,
                is_supertype: false,
                is_terminal: true,
                is_extra: false,
                is_fragile: false,
                symbol_id: SymbolId(2),
            },
            SymbolMetadata {
                name: "operator".to_string(),
                is_visible: true,
                is_named: true,
                is_supertype: false,
                is_terminal: true,
                is_extra: false,
                is_fragile: false,
                symbol_id: SymbolId(3),
            },
        ]
    }

    fn query_with_root(root: PatternNode) -> Query {
        let mut capture_names = HashMap::new();
        capture_names.insert("node".to_string(), 0);

        Query {
            source: String::new(),
            patterns: vec![Pattern {
                root,
                predicates: Vec::new(),
                start_byte: 0,
            }],
            capture_names,
            property_settings: Vec::new(),
            property_predicates: Vec::new(),
        }
    }

    fn literal_child_query(literal: &str) -> Query {
        let mut root = PatternNode::new(SymbolId(0), true);
        root.add_child(PatternChild::Token(literal.to_string()));

        Query {
            source: String::new(),
            patterns: vec![Pattern {
                root,
                predicates: Vec::new(),
                start_byte: 0,
            }],
            capture_names: HashMap::new(),
            property_settings: Vec::new(),
            property_predicates: Vec::new(),
        }
    }

    fn repeated_child_query(
        repeated_symbol: u16,
        quantifier: Quantifier,
        tail_symbol: u16,
    ) -> Query {
        let mut root = PatternNode::new(SymbolId(0), true);
        root.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(repeated_symbol), true).with_quantifier(quantifier),
        ));
        root.add_child(PatternChild::Node(PatternNode::new(
            SymbolId(tail_symbol),
            true,
        )));

        Query {
            source: String::new(),
            patterns: vec![Pattern {
                root,
                predicates: Vec::new(),
                start_byte: 0,
            }],
            capture_names: HashMap::new(),
            property_settings: Vec::new(),
            property_predicates: Vec::new(),
        }
    }

    #[test]
    fn test_predicate_matching() {
        // Create a simple query with predicates
        let query_str = r#"
            (identifier @name)
            (#eq? @name "test")
        "#;

        let grammar = create_test_grammar();
        let query = compile_query(query_str, &grammar).unwrap();

        // Create test tree
        let source = "test other test";
        let symbol_id = SymbolId(0);
        let root = ParseNode {
            symbol: symbol_id,
            symbol_id,
            children: vec![
                make_node(1, 0, 4),   // "test"
                make_node(1, 5, 10),  // "other"
                make_node(1, 11, 15), // "test"
            ],
            start_byte: 0,
            end_byte: 15,
            field_name: None,
            alias_symbol_id: None,
        };

        // Match with predicates
        let metadata = test_symbol_metadata();
        let matcher = QueryMatcher::new(&query, source, &metadata);
        let matches = matcher.matches(&root);

        // Should match only the "test" identifiers
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].captures[0].node.start_byte, 0);
        assert_eq!(matches[1].captures[0].node.start_byte, 11);
    }

    #[test]
    fn test_query_without_predicates() {
        // Test that queries work without predicates as well
        let query_str = "(identifier @name)";

        let grammar = create_test_grammar();
        let query = compile_query(query_str, &grammar).unwrap();

        // Create test tree with three identifiers
        let source = "foo bar baz";
        let root = ParseNode {
            symbol: SymbolId(0),
            symbol_id: SymbolId(0),
            children: vec![
                make_node(1, 0, 3),  // "foo"
                make_node(1, 4, 7),  // "bar"
                make_node(1, 8, 11), // "baz"
            ],
            start_byte: 0,
            end_byte: 11,
            field_name: None,
            alias_symbol_id: None,
        };

        // Match without predicates - should match all identifiers
        let metadata = test_symbol_metadata();
        let matcher = QueryMatcher::new(&query, source, &metadata);
        let matches = matcher.matches(&root);

        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0].captures[0].node.start_byte, 0);
        assert_eq!(matches[1].captures[0].node.start_byte, 4);
        assert_eq!(matches[2].captures[0].node.start_byte, 8);
    }

    #[test]
    fn test_empty_query_result() {
        // Test a query that doesn't match anything
        let query_str = r#"
            (identifier @name)
            (#eq? @name "nonexistent")
        "#;

        let grammar = create_test_grammar();
        let query = compile_query(query_str, &grammar).unwrap();

        let source = "test other test";
        let root = ParseNode {
            symbol: SymbolId(0),
            symbol_id: SymbolId(0),
            children: vec![
                make_node(1, 0, 4),   // "test"
                make_node(1, 5, 10),  // "other"
                make_node(1, 11, 15), // "test"
            ],
            start_byte: 0,
            end_byte: 15,
            field_name: None,
            alias_symbol_id: None,
        };

        let metadata = test_symbol_metadata();
        let matcher = QueryMatcher::new(&query, source, &metadata);
        let matches = matcher.matches(&root);

        // Should not match anything
        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn test_public_matcher_not_eq_invalid_range_fails_closed() {
        let grammar = create_test_grammar();
        let query = compile_query(
            r#"(identifier) @name (#not-eq? @name "different")"#,
            &grammar,
        )
        .unwrap();
        let metadata = test_symbol_metadata();

        // Out-of-bounds range 0..2 on source "a" must not become a match.
        let root = make_node(1, 0, 2);
        let matcher = QueryMatcher::new(&query, "a", &metadata);
        assert!(matcher.matches(&root).is_empty());

        // The same predicate on a valid range is a positive match.
        let root = make_node(1, 0, 2);
        let matcher = QueryMatcher::new(&query, "ab", &metadata);
        let matches = matcher.matches(&root);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].captures.len(), 1);
        assert_eq!(matches[0].captures[0].node.start_byte, 0);
        assert_eq!(matches[0].captures[0].node.end_byte, 2);

        // Full-string regex through the public matcher: `a|ab` on "ab".
        let query =
            compile_query(r#"(identifier) @name (#match? @name "a|ab")"#, &grammar).unwrap();
        let matcher = QueryMatcher::new(&query, "ab", &metadata);
        assert_eq!(matcher.matches(&make_node(1, 0, 2)).len(), 1);
    }

    #[test]
    fn test_public_matcher_invalid_ranges_and_regex_fail_closed() {
        let grammar = create_test_grammar();
        let metadata = test_symbol_metadata();
        let not_eq = compile_query(
            r#"(identifier) @name (#not-eq? @name "different")"#,
            &grammar,
        )
        .unwrap();

        // Reversed range 2..1 on "ab" must not become a match.
        let matcher = QueryMatcher::new(&not_eq, "ab", &metadata);
        assert!(matcher.matches(&make_node(1, 2, 1)).is_empty());

        // Non-UTF-8-boundary range 0..1 on "λ" (2 bytes) must not match.
        let matcher = QueryMatcher::new(&not_eq, "λ", &metadata);
        assert!(matcher.matches(&make_node(1, 0, 1)).is_empty());

        // #not-match? on an invalid range also fails closed.
        let not_match =
            compile_query(r#"(identifier) @name (#not-match? @name "z+")"#, &grammar).unwrap();
        let matcher = QueryMatcher::new(&not_match, "ab", &metadata);
        assert!(matcher.matches(&make_node(1, 0, 3)).is_empty());
        // ...while a valid non-matching range still matches.
        let matcher = QueryMatcher::new(&not_match, "ab", &metadata);
        assert_eq!(matcher.matches(&make_node(1, 0, 2)).len(), 1);

        // Manually constructed invalid regex cannot produce a match.
        let root = PatternNode::new(SymbolId(1), true).with_capture(0);
        let mut capture_names = HashMap::new();
        capture_names.insert("name".to_string(), 0);
        let query = Query {
            source: String::new(),
            patterns: vec![Pattern {
                root,
                predicates: vec![Predicate::Match {
                    capture: 0,
                    regex: "[".to_string(),
                }],
                start_byte: 0,
            }],
            capture_names,
            property_settings: Vec::new(),
            property_predicates: Vec::new(),
        };
        let matcher = QueryMatcher::new(&query, "ab", &metadata);
        assert!(matcher.matches(&make_node(1, 0, 2)).is_empty());
    }

    #[test]
    fn test_field_constraint_matches_named_field_child() {
        let mut pattern = PatternNode::new(SymbolId(0), true);
        pattern.add_field(
            "left".to_string(),
            PatternNode::new(SymbolId(1), true).with_capture(0),
        );
        let query = query_with_root(pattern);
        let root = make_root(
            vec![
                make_field_node(2, 0, 1, "right"),
                make_field_node(1, 2, 3, "left"),
            ],
            3,
        );
        let metadata = test_symbol_metadata();

        let matcher = QueryMatcher::new(&query, "1 a", &metadata);
        let matches = matcher.matches(&root);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].captures[0].node.start_byte, 2);
    }

    #[test]
    fn test_field_constraint_rejects_missing_or_wrong_field() {
        let mut pattern = PatternNode::new(SymbolId(0), true);
        pattern.add_field(
            "left".to_string(),
            PatternNode::new(SymbolId(1), true).with_capture(0),
        );
        let query = query_with_root(pattern);
        let root = make_root(vec![make_field_node(1, 0, 1, "right")], 1);
        let metadata = test_symbol_metadata();

        let matcher = QueryMatcher::new(&query, "a", &metadata);
        let matches = matcher.matches(&root);

        assert!(matches.is_empty());
    }

    #[test]
    fn test_child_pattern_matches_subsequence_without_anchor() {
        let mut pattern = PatternNode::new(SymbolId(0), true);
        pattern.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(1), true).with_capture(0),
        ));
        let query = query_with_root(pattern);
        let root = make_root(vec![make_node(2, 0, 1), make_node(1, 2, 3)], 3);
        let metadata = test_symbol_metadata();

        let matcher = QueryMatcher::new(&query, "1 a", &metadata);
        let matches = matcher.matches(&root);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].captures[0].node.start_byte, 2);
    }

    #[test]
    fn test_first_child_anchor_requires_first_matching_child() {
        let mut pattern = PatternNode::new(SymbolId(0), true);
        pattern.add_child(PatternChild::Anchor);
        pattern.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(1), true).with_capture(0),
        ));
        let query = query_with_root(pattern);
        let root = make_root(vec![make_node(2, 0, 1), make_node(1, 2, 3)], 3);
        let metadata = test_symbol_metadata();

        let matcher = QueryMatcher::new(&query, "1 a", &metadata);
        let matches = matcher.matches(&root);

        assert!(matches.is_empty());
    }

    #[test]
    fn test_last_child_anchor_requires_last_matching_child() {
        let mut pattern = PatternNode::new(SymbolId(0), true);
        pattern.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(1), true).with_capture(0),
        ));
        pattern.add_child(PatternChild::Anchor);
        let query = query_with_root(pattern);
        let root = make_root(vec![make_node(2, 0, 1), make_node(1, 2, 3)], 3);
        let metadata = test_symbol_metadata();

        let matcher = QueryMatcher::new(&query, "1 a", &metadata);
        let matches = matcher.matches(&root);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].captures[0].node.start_byte, 2);
    }

    #[test]
    fn test_adjacent_anchor_requires_next_sibling_match() {
        let mut pattern = PatternNode::new(SymbolId(0), true);
        pattern.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(1), true).with_capture(0),
        ));
        pattern.add_child(PatternChild::Anchor);
        pattern.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(3), true).with_capture(1),
        ));
        let query = query_with_root(pattern);
        let root = make_root(
            vec![make_node(1, 0, 1), make_node(2, 1, 2), make_node(3, 2, 3)],
            3,
        );
        let metadata = test_symbol_metadata();

        let matcher = QueryMatcher::new(&query, "a1+", &metadata);
        let matches = matcher.matches(&root);

        assert!(matches.is_empty());
    }

    #[test]
    fn test_adjacent_anchor_matches_adjacent_siblings() {
        let mut pattern = PatternNode::new(SymbolId(0), true);
        pattern.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(1), true).with_capture(0),
        ));
        pattern.add_child(PatternChild::Anchor);
        pattern.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(3), true).with_capture(1),
        ));
        let query = query_with_root(pattern);
        let root = make_root(vec![make_node(1, 0, 1), make_node(3, 1, 2)], 2);
        let metadata = test_symbol_metadata();

        let matcher = QueryMatcher::new(&query, "a+", &metadata);
        let matches = matcher.matches(&root);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].captures[0].node.start_byte, 0);
        assert_eq!(matches[0].captures[1].node.start_byte, 1);
    }

    #[test]
    fn test_literal_child_when_text_matches_returns_match() {
        let source = "+";
        let root = make_root(vec![make_node(1, 0, 1)], source.len());
        let query = literal_child_query("+");
        let metadata = test_symbol_metadata();

        let matcher = QueryMatcher::new(&query, source, &metadata);
        let matches = matcher.matches(&root);

        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_literal_child_when_text_differs_returns_no_match() {
        let source = "-";
        let root = make_root(vec![make_node(1, 0, 1)], source.len());
        let query = literal_child_query("+");
        let metadata = test_symbol_metadata();

        let matcher = QueryMatcher::new(&query, source, &metadata);
        let matches = matcher.matches(&root);

        assert!(matches.is_empty());
    }

    #[test]
    fn test_plus_child_quantifier_when_followed_by_tail_consumes_repeated_nodes() {
        let source = "aab";
        let root = make_root(
            vec![make_node(1, 0, 1), make_node(1, 1, 2), make_node(2, 2, 3)],
            source.len(),
        );
        let query = repeated_child_query(1, Quantifier::Plus, 2);
        let metadata = test_symbol_metadata();

        let matcher = QueryMatcher::new(&query, source, &metadata);
        let matches = matcher.matches(&root);

        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_star_child_quantifier_when_followed_by_tail_allows_zero_matches() {
        let source = "b";
        let root = make_root(vec![make_node(2, 0, 1)], source.len());
        let query = repeated_child_query(1, Quantifier::Star, 2);
        let metadata = test_symbol_metadata();

        let matcher = QueryMatcher::new(&query, source, &metadata);
        let matches = matcher.matches(&root);

        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_repeated_capture_returns_every_occurrence_in_order() {
        // Same capture name on two siblings with identical text: occurrences
        // stay distinct by identity, not text.
        let source = "aa aa";
        let root = make_root(vec![make_node(1, 0, 2), make_node(1, 3, 5)], source.len());
        let mut pattern = PatternNode::new(SymbolId(0), true);
        pattern.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(1), true).with_capture(0),
        ));
        pattern.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(1), true).with_capture(0),
        ));
        let query = query_with_root(pattern);
        let metadata = test_symbol_metadata();

        let matcher = QueryMatcher::new(&query, source, &metadata);
        let matches = matcher.matches(&root);

        assert_eq!(matches.len(), 1, "one match, not one per occurrence");
        assert_eq!(matches[0].captures.len(), 2, "both occurrences retained");
        assert_eq!(matches[0].captures[0].index, 0);
        assert_eq!(matches[0].captures[0].node.start_byte, 0);
        assert_eq!(matches[0].captures[1].index, 0);
        assert_eq!(matches[0].captures[1].node.start_byte, 3);
    }

    #[test]
    fn test_plus_quantifier_captures_every_repetition() {
        let source = "aab";
        let root = make_root(
            vec![make_node(1, 0, 1), make_node(1, 1, 2), make_node(2, 2, 3)],
            source.len(),
        );
        let mut pattern = PatternNode::new(SymbolId(0), true);
        pattern.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(1), true)
                .with_capture(0)
                .with_quantifier(Quantifier::Plus),
        ));
        pattern.add_child(PatternChild::Node(PatternNode::new(SymbolId(2), true)));
        let query = query_with_root(pattern);
        let metadata = test_symbol_metadata();

        let matcher = QueryMatcher::new(&query, source, &metadata);
        let matches = matcher.matches(&root);

        assert_eq!(matches.len(), 1);
        assert_eq!(
            matches[0].captures.len(),
            2,
            "Plus captures every repetition"
        );
        assert_eq!(matches[0].captures[0].node.start_byte, 0);
        assert_eq!(matches[0].captures[1].node.start_byte, 1);
    }

    #[test]
    fn test_repeated_capture_predicate_failing_first_occurrence_rejects() {
        // "cd ab": the FIRST occurrence fails (#match? @x "ab") while the
        // final occurrence passes. Universal semantics reject the match; a
        // last-occurrence-only inspection would wrongly accept it.
        let source = "cd ab";
        let mut repeated = PatternNode::new(SymbolId(0), true);
        repeated.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(1), true).with_capture(0),
        ));
        repeated.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(1), true).with_capture(0),
        ));
        let mut query = query_with_root(repeated);
        query.patterns[0].predicates.push(Predicate::Match {
            capture: 0,
            regex: "ab".to_string(),
        });
        let metadata = test_symbol_metadata();

        let matcher = QueryMatcher::new(&query, source, &metadata);
        let root = make_root(vec![make_node(1, 0, 2), make_node(1, 3, 5)], source.len());
        assert!(
            matcher.matches(&root).is_empty(),
            "a failing nonfinal occurrence must not be hidden by a passing final one"
        );

        // Positive control: a predicate every occurrence satisfies keeps the
        // match with both occurrences in declared order.
        query.patterns[0].predicates = vec![Predicate::Match {
            capture: 0,
            regex: "ab|cd".to_string(),
        }];
        let matcher = QueryMatcher::new(&query, source, &metadata);
        let matches = matcher.matches(&root);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].captures.len(), 2);
        assert_eq!(
            matches[0]
                .captures
                .iter()
                .map(|capture| capture.node.start_byte..capture.node.end_byte)
                .collect::<Vec<_>>(),
            vec![0..2, 3..5]
        );
        assert_eq!(&source[0..2], "cd");
        assert_eq!(&source[3..5], "ab");
    }

    #[test]
    fn test_backtrack_returns_only_the_accepted_match_captures() {
        // (identifier @x) . (number): the anchor forces adjacency, so the
        // plausible first identifier fails and only the accepted second
        // attempt is captured.
        let source = "aab";
        let root = make_root(
            vec![make_node(1, 0, 1), make_node(1, 1, 2), make_node(2, 2, 3)],
            source.len(),
        );
        let mut pattern = PatternNode::new(SymbolId(0), true);
        pattern.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(1), true).with_capture(0),
        ));
        pattern.add_child(PatternChild::Anchor);
        pattern.add_child(PatternChild::Node(PatternNode::new(SymbolId(2), true)));
        let query = query_with_root(pattern);
        let metadata = test_symbol_metadata();

        let matcher = QueryMatcher::new(&query, source, &metadata);
        let matches = matcher.matches(&root);

        assert_eq!(matches.len(), 1);
        assert_eq!(
            matches[0].captures.len(),
            1,
            "no residue from the failed attempt"
        );
        assert_eq!(matches[0].captures[0].index, 0);
        assert_eq!(matches[0].captures[0].node.start_byte, 1);
    }

    #[test]
    fn test_star_zero_occurrences_contributes_no_capture() {
        // (identifier @x)* matches zero times before the tail number: the
        // match succeeds with no speculative capture.
        let source = "b";
        let root = make_root(vec![make_node(2, 0, 1)], source.len());
        let mut pattern = PatternNode::new(SymbolId(0), true);
        pattern.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(1), true)
                .with_capture(0)
                .with_quantifier(Quantifier::Star),
        ));
        pattern.add_child(PatternChild::Node(PatternNode::new(SymbolId(2), true)));
        let query = query_with_root(pattern);
        let metadata = test_symbol_metadata();

        let matcher = QueryMatcher::new(&query, source, &metadata);
        let matches = matcher.matches(&root);

        assert_eq!(matches.len(), 1);
        assert!(
            matches[0].captures.is_empty(),
            "zero-occurrence Star success contributes no speculative capture"
        );
    }

    #[test]
    fn test_failed_optional_leaves_no_capture_residue() {
        // Field-nested optional: (B @b ...)? matches the left child and
        // captures it, then fails its required child. The failed attempt
        // must vanish while the overall match still succeeds.
        let source = "x";
        let mut a = make_node(1, 0, 1);
        a.children = vec![make_field_node(2, 0, 1, "left")];
        let root = make_root(vec![a], source.len());
        let mut optional = PatternNode::new(SymbolId(2), true).with_capture(0);
        optional.quantifier = Quantifier::Optional;
        optional.add_child(PatternChild::Node(PatternNode::new(SymbolId(3), true)));
        let mut a_pattern = PatternNode::new(SymbolId(1), true);
        a_pattern.fields.insert("left".to_string(), optional);
        let mut pattern = PatternNode::new(SymbolId(0), true);
        pattern.add_child(PatternChild::Node(a_pattern));
        let query = query_with_root(pattern);
        let metadata = test_symbol_metadata();

        let matcher = QueryMatcher::new(&query, source, &metadata);
        let matches = matcher.matches(&root);

        assert_eq!(matches.len(), 1);
        assert!(
            matches[0].captures.is_empty(),
            "failed optional contributes no speculative capture"
        );
    }

    #[cfg(feature = "query")]
    fn doc_fixture(source: &str, root: ParseNode) -> AdzeDocument {
        doc_fixture_with_fields(source, root, &[])
    }

    #[cfg(feature = "query")]
    fn doc_fixture_with_fields(source: &str, root: ParseNode, fields: &[&str]) -> AdzeDocument {
        use adze_glr_core::ParseTable;
        use adze_ir::Grammar;

        let grammar = Grammar::new("doc-fixture".to_string());
        let mut table = ParseTable::default();
        // Flat field names resolve through the table's field list, mirroring
        // how real parse tables carry generated field names.
        table.field_names = fields.iter().map(ToString::to_string).collect();
        AdzeDocument::from_parse_result(source, root, 0, "doc-fixture", &grammar, &table)
    }

    #[cfg(feature = "query")]
    fn document_ranges(document: &AdzeDocument, ids: &[NodeId]) -> Vec<std::ops::Range<usize>> {
        ids.iter()
            .map(|id| {
                document
                    .syntax()
                    .node_record(*id)
                    .expect("captured id resolves")
                    .byte_range()
            })
            .collect()
    }

    #[cfg(feature = "query")]
    #[test]
    fn test_matches_document_agrees_with_recursive_matches() {
        // Same hand-built facts through both entries: (index, ranges) agree.
        let source = "aab";
        let root = make_root(
            vec![make_node(1, 0, 1), make_node(1, 1, 2), make_node(2, 2, 3)],
            source.len(),
        );
        let document = doc_fixture(source, root.clone());
        let mut pattern = PatternNode::new(SymbolId(0), true);
        pattern.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(1), true).with_capture(0),
        ));
        pattern.add_child(PatternChild::Anchor);
        pattern.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(2), true).with_capture(1),
        ));
        let query = query_with_root(pattern);
        let metadata = test_symbol_metadata();

        let flat = QueryMatcher::new(&query, source, &[]).matches_document(&document);
        let recursive = QueryMatcher::new(&query, source, &metadata).matches(&root);

        assert_eq!(flat.len(), recursive.len());
        assert_eq!(flat.len(), 1, "agreement fixture must match");
        for (flat_match, recursive_match) in flat.iter().zip(recursive.iter()) {
            assert_eq!(flat_match.pattern_index, recursive_match.pattern_index);
            assert_eq!(flat_match.captures.len(), recursive_match.captures.len());
            let flat_ranges: Vec<std::ops::Range<usize>> = flat_match
                .captures
                .iter()
                .map(|capture| {
                    document
                        .syntax()
                        .node_record(capture.node_id)
                        .expect("captured id resolves")
                        .byte_range()
                })
                .collect();
            let recursive_ranges: Vec<std::ops::Range<usize>> = recursive_match
                .captures
                .iter()
                .map(|capture| capture.node.start_byte..capture.node.end_byte)
                .collect();
            // Index order is assertion order: compare per position.
            for (flat_capture, recursive_capture) in flat_match
                .captures
                .iter()
                .zip(recursive_match.captures.iter())
            {
                assert_eq!(flat_capture.index, recursive_capture.index);
            }
            assert_eq!(flat_ranges, recursive_ranges);
        }
    }

    #[cfg(feature = "query")]
    #[test]
    fn test_matches_document_returns_repeated_captures_in_order() {
        let source = "aa aa";
        let root = make_root(vec![make_node(1, 0, 2), make_node(1, 3, 5)], source.len());
        let document = doc_fixture(source, root);
        let mut pattern = PatternNode::new(SymbolId(0), true);
        pattern.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(1), true).with_capture(0),
        ));
        pattern.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(1), true).with_capture(0),
        ));
        let query = query_with_root(pattern);

        let matches = QueryMatcher::new(&query, source, &[]).matches_document(&document);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].captures.len(), 2);
        assert_eq!(matches[0].captures[0].index, 0);
        assert_eq!(matches[0].captures[1].index, 0);
        let ids: Vec<NodeId> = matches[0].captures.iter().map(|c| c.node_id).collect();
        assert_eq!(document_ranges(&document, &ids), vec![0..2, 3..5]);
    }

    #[cfg(feature = "query")]
    #[test]
    fn test_matches_document_captures_plus_repetitions() {
        let source = "aab";
        let root = make_root(
            vec![make_node(1, 0, 1), make_node(1, 1, 2), make_node(2, 2, 3)],
            source.len(),
        );
        let document = doc_fixture(source, root);
        let mut pattern = PatternNode::new(SymbolId(0), true);
        pattern.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(1), true)
                .with_capture(0)
                .with_quantifier(Quantifier::Plus),
        ));
        pattern.add_child(PatternChild::Node(PatternNode::new(SymbolId(2), true)));
        let query = query_with_root(pattern);

        let matches = QueryMatcher::new(&query, source, &[]).matches_document(&document);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].captures.len(), 2);
        let ids: Vec<NodeId> = matches[0].captures.iter().map(|c| c.node_id).collect();
        assert_eq!(document_ranges(&document, &ids), vec![0..1, 1..2]);
    }

    #[cfg(feature = "query")]
    #[test]
    fn test_matches_document_backtrack_keeps_only_accepted_captures() {
        let source = "aab";
        let root = make_root(
            vec![make_node(1, 0, 1), make_node(1, 1, 2), make_node(2, 2, 3)],
            source.len(),
        );
        let document = doc_fixture(source, root);
        let mut pattern = PatternNode::new(SymbolId(0), true);
        pattern.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(1), true).with_capture(0),
        ));
        pattern.add_child(PatternChild::Anchor);
        pattern.add_child(PatternChild::Node(PatternNode::new(SymbolId(2), true)));
        let query = query_with_root(pattern);

        let matches = QueryMatcher::new(&query, source, &[]).matches_document(&document);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].captures.len(), 1);
        let ids: Vec<NodeId> = matches[0].captures.iter().map(|c| c.node_id).collect();
        assert_eq!(document_ranges(&document, &ids), vec![1..2]);
    }

    #[cfg(feature = "query")]
    #[test]
    fn test_matches_document_failed_optional_leaves_no_residue() {
        let source = "x";
        let mut a = make_node(1, 0, 1);
        a.children = vec![make_field_node(2, 0, 1, "left")];
        let root = make_root(vec![a], source.len());
        let document = doc_fixture_with_fields(source, root, &["left"]);
        let mut optional = PatternNode::new(SymbolId(2), true).with_capture(0);
        optional.quantifier = Quantifier::Optional;
        optional.add_child(PatternChild::Node(PatternNode::new(SymbolId(3), true)));
        let mut a_pattern = PatternNode::new(SymbolId(1), true);
        a_pattern.fields.insert("left".to_string(), optional);
        let mut pattern = PatternNode::new(SymbolId(0), true);
        pattern.add_child(PatternChild::Node(a_pattern));
        let query = query_with_root(pattern);

        let matches = QueryMatcher::new(&query, source, &[]).matches_document(&document);

        assert_eq!(matches.len(), 1);
        assert!(
            matches[0].captures.is_empty(),
            "failed optional contributes no speculative capture"
        );
    }

    #[cfg(feature = "query")]
    #[test]
    fn test_matches_document_applies_universal_predicates() {
        let source = "ab cd";
        let root = make_root(vec![make_node(1, 0, 2), make_node(1, 3, 5)], source.len());
        let document = doc_fixture(source, root);
        let mut pattern = PatternNode::new(SymbolId(0), true);
        pattern.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(1), true).with_capture(0),
        ));
        pattern.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(1), true).with_capture(0),
        ));
        let mut query = query_with_root(pattern);
        query.patterns[0].predicates.push(Predicate::Match {
            capture: 0,
            regex: "ab".to_string(),
        });

        let matches = QueryMatcher::new(&query, source, &[]).matches_document(&document);

        assert!(
            matches.is_empty(),
            "a failing occurrence must fail the predicate even when another passes"
        );
    }

    #[cfg(feature = "query")]
    #[test]
    fn test_matches_document_rejects_failing_first_occurrence() {
        // "cd ab" through the flat backend: the FIRST occurrence fails
        // (#match? @x "ab") while the final passes. Universal semantics
        // reject the match; a last-occurrence-only inspection would accept.
        let source = "cd ab";
        let root = make_root(vec![make_node(1, 0, 2), make_node(1, 3, 5)], source.len());
        let document = doc_fixture(source, root);
        let mut pattern = PatternNode::new(SymbolId(0), true);
        pattern.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(1), true).with_capture(0),
        ));
        pattern.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(1), true).with_capture(0),
        ));
        let mut query = query_with_root(pattern);
        query.patterns[0].predicates.push(Predicate::Match {
            capture: 0,
            regex: "ab".to_string(),
        });

        let matches = QueryMatcher::new(&query, source, &[]).matches_document(&document);
        assert!(
            matches.is_empty(),
            "a failing nonfinal occurrence must not be hidden by a passing final one"
        );
    }

    #[cfg(feature = "query")]
    #[test]
    fn test_matches_document_star_zero_occurrences_contributes_no_capture() {
        let source = "b";
        let root = make_root(vec![make_node(2, 0, 1)], source.len());
        let document = doc_fixture(source, root);
        let mut pattern = PatternNode::new(SymbolId(0), true);
        pattern.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(1), true)
                .with_capture(0)
                .with_quantifier(Quantifier::Star),
        ));
        pattern.add_child(PatternChild::Node(PatternNode::new(SymbolId(2), true)));
        let query = query_with_root(pattern);

        let matches = QueryMatcher::new(&query, source, &[]).matches_document(&document);

        assert_eq!(matches.len(), 1);
        assert!(
            matches[0].captures.is_empty(),
            "zero-occurrence Star success contributes no speculative capture"
        );
    }
}

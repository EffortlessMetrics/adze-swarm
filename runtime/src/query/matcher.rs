// Query pattern matching implementation
use super::ast::*;
use crate::parser_v4::ParseNode;
use std::collections::HashMap;

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

/// State for matching a pattern
#[derive(Clone, Debug)]
struct MatchState {
    /// Current captures
    captures: HashMap<u32, ParseNode>,
    /// Whether the match succeeded
    #[allow(dead_code)]
    success: bool,
}

#[derive(Clone, Copy)]
struct RepeatSearch {
    min_count: usize,
    anchored_next: bool,
}

/// Query pattern matcher
pub struct QueryMatcher<'a> {
    query: &'a Query,
}

fn node_overlaps_range(node: &ParseNode, range: &std::ops::Range<usize>) -> bool {
    node.start_byte <= range.end && node.end_byte >= range.start
}

fn match_overlaps_range(
    captures: &HashMap<u32, ParseNode>,
    node: &ParseNode,
    range: &std::ops::Range<usize>,
) -> bool {
    if captures.is_empty() {
        return node_overlaps_range(node, range);
    }

    captures
        .values()
        .any(|capture| node_overlaps_range(capture, range))
}

impl<'a> QueryMatcher<'a> {
    /// Create a new query matcher
    pub fn new(query: &'a Query) -> Self {
        QueryMatcher { query }
    }

    /// Match all patterns in the query against a parse tree
    pub fn matches(&self, root: &ParseNode) -> Vec<QueryMatch> {
        self.matches_with_options(root, None, false)
    }

    /// Match all patterns in the query against a parse tree using cursor options.
    pub(crate) fn matches_with_options(
        &self,
        root: &ParseNode,
        byte_range: Option<&std::ops::Range<usize>>,
        match_root: bool,
    ) -> Vec<QueryMatch> {
        let mut matches = Vec::new();

        // Try each pattern
        for (pattern_index, pattern) in self.query.patterns.iter().enumerate() {
            self.match_pattern(
                pattern_index,
                pattern,
                root,
                byte_range,
                match_root,
                &mut matches,
            );
        }

        matches
    }

    /// Match a single pattern against the tree
    fn match_pattern(
        &self,
        pattern_index: usize,
        pattern: &Pattern,
        root: &ParseNode,
        byte_range: Option<&std::ops::Range<usize>>,
        match_root: bool,
        matches: &mut Vec<QueryMatch>,
    ) {
        // Walk the tree and try to match at each node
        self.match_pattern_at_node(
            pattern_index,
            pattern,
            root,
            byte_range,
            match_root,
            matches,
        );
    }

    /// Try to match pattern starting at a specific node
    fn match_pattern_at_node(
        &self,
        pattern_index: usize,
        pattern: &Pattern,
        node: &ParseNode,
        byte_range: Option<&std::ops::Range<usize>>,
        match_root: bool,
        matches: &mut Vec<QueryMatch>,
    ) {
        if let Some(range) = byte_range
            && !node_overlaps_range(node, range)
        {
            return;
        }

        // Try to match the pattern at this node
        let mut state = MatchState {
            captures: HashMap::new(),
            success: false,
        };

        if self.match_node(&pattern.root, node, &mut state)
            && byte_range.is_none_or(|range| match_overlaps_range(&state.captures, node, range))
        {
            // Check predicates
            if self.check_predicates(&pattern.predicates, &state.captures) {
                // Create match
                let mut captures: Vec<_> = state
                    .captures
                    .into_iter()
                    .map(|(index, node)| QueryCapture { index, node })
                    .collect();
                captures.sort_by_key(|c| c.index);

                matches.push(QueryMatch {
                    pattern_index,
                    captures,
                });
            }
        }

        if match_root {
            return;
        }

        // Recursively try to match in children
        for child in &node.children {
            self.match_pattern_at_node(pattern_index, pattern, child, byte_range, false, matches);
        }
    }

    /// Match a pattern node against a parse node
    fn match_node(&self, pattern: &PatternNode, node: &ParseNode, state: &mut MatchState) -> bool {
        // Check symbol match
        if pattern.symbol != node.symbol {
            return false;
        }

        // Capture if needed
        if let Some(capture_id) = pattern.capture {
            state.captures.insert(capture_id, node.clone());
        }

        // Match children based on quantifier
        match pattern.quantifier {
            Quantifier::One => self.match_children_one(pattern, node, state),
            Quantifier::Optional => self.match_children_optional(pattern, node, state),
            Quantifier::Plus => self.match_children_plus(pattern, node, state),
            Quantifier::Star => self.match_children_star(pattern, node, state),
        }
    }

    /// Match children with One quantifier
    fn match_children_one(
        &self,
        pattern: &PatternNode,
        node: &ParseNode,
        state: &mut MatchState,
    ) -> bool {
        // Check field assertions
        for (field_name, field_pattern) in &pattern.fields {
            // Find child with this field name
            let field_node = node
                .children
                .iter()
                .find(|child| child.field_name.as_ref() == Some(field_name));

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
            return self.match_child_sequence(&pattern.children, &node.children, 0, 0, state);
        }

        true
    }

    /// Match children with Optional quantifier
    fn match_children_optional(
        &self,
        pattern: &PatternNode,
        node: &ParseNode,
        state: &mut MatchState,
    ) -> bool {
        // Optional always matches, but we try to match if possible
        self.match_children_one(pattern, node, state);
        true
    }

    /// Match children with Plus quantifier
    fn match_children_plus(
        &self,
        pattern: &PatternNode,
        node: &ParseNode,
        state: &mut MatchState,
    ) -> bool {
        // Must match at least once
        if !self.match_children_one(pattern, node, state) {
            return false;
        }

        // Try to match more (simplified - in reality would need backtracking)
        true
    }

    /// Match children with Star quantifier
    fn match_children_star(
        &self,
        pattern: &PatternNode,
        node: &ParseNode,
        state: &mut MatchState,
    ) -> bool {
        // Star always matches (zero or more)
        self.match_children_plus(pattern, node, state);
        true
    }

    /// Match a sequence of child patterns
    fn match_child_sequence(
        &self,
        patterns: &[PatternChild],
        nodes: &[ParseNode],
        pattern_idx: usize,
        node_idx: usize,
        state: &mut MatchState,
    ) -> bool {
        if let Some(next_state) = self.match_child_sequence_from(
            patterns,
            nodes,
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

    fn match_child_sequence_from(
        &self,
        patterns: &[PatternChild],
        nodes: &[ParseNode],
        pattern_idx: usize,
        node_idx: usize,
        state: MatchState,
        anchored_next: bool,
    ) -> Option<MatchState> {
        // Base case: all patterns matched
        if pattern_idx >= patterns.len() {
            return Some(state);
        }

        // Base case: no more nodes but patterns remain
        if node_idx >= nodes.len() {
            return patterns[pattern_idx..]
                .iter()
                .all(|pattern| {
                    matches!(pattern, PatternChild::Anchor)
                        || matches!(
                            pattern,
                            PatternChild::Node(node)
                                if matches!(node.quantifier, Quantifier::Optional | Quantifier::Star)
                        )
                })
                .then_some(state);
        }

        match &patterns[pattern_idx] {
            PatternChild::Anchor => self
                .anchor_satisfied(patterns, pattern_idx, nodes, node_idx)
                .then(|| {
                    self.match_child_sequence_from(
                        patterns,
                        nodes,
                        pattern_idx + 1,
                        node_idx,
                        state,
                        true,
                    )
                })
                .flatten(),
            PatternChild::Token(_) => None,
            PatternChild::Node(pattern_node) => match pattern_node.quantifier {
                Quantifier::One => self.match_single_child_candidate(
                    pattern_node,
                    patterns,
                    nodes,
                    (pattern_idx, node_idx),
                    state,
                    anchored_next,
                ),
                Quantifier::Optional => self
                    .match_single_child_candidate(
                        pattern_node,
                        patterns,
                        nodes,
                        (pattern_idx, node_idx),
                        state.clone(),
                        anchored_next,
                    )
                    .or_else(|| {
                        self.match_child_sequence_from(
                            patterns,
                            nodes,
                            pattern_idx + 1,
                            node_idx,
                            state,
                            anchored_next,
                        )
                    }),
                Quantifier::Plus => self.match_repeated_child_candidates(
                    pattern_node,
                    patterns,
                    nodes,
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
                        patterns,
                        nodes,
                        (pattern_idx, node_idx),
                        state.clone(),
                        RepeatSearch {
                            min_count: 0,
                            anchored_next,
                        },
                    )
                    .or_else(|| {
                        self.match_child_sequence_from(
                            patterns,
                            nodes,
                            pattern_idx + 1,
                            node_idx,
                            state,
                            anchored_next,
                        )
                    }),
            },
        }
    }

    fn anchor_satisfied(
        &self,
        patterns: &[PatternChild],
        pattern_idx: usize,
        nodes: &[ParseNode],
        node_idx: usize,
    ) -> bool {
        if pattern_idx == 0 {
            node_idx == 0
        } else if pattern_idx + 1 == patterns.len() {
            node_idx >= nodes.len()
        } else {
            true
        }
    }

    fn match_single_child_candidate(
        &self,
        pattern_node: &PatternNode,
        patterns: &[PatternChild],
        nodes: &[ParseNode],
        position: (usize, usize),
        state: MatchState,
        anchored_next: bool,
    ) -> Option<MatchState> {
        let (pattern_idx, node_idx) = position;
        let candidates: Box<dyn Iterator<Item = usize>> = if anchored_next {
            Box::new((node_idx < nodes.len()).then_some(node_idx).into_iter())
        } else {
            Box::new(node_idx..nodes.len())
        };

        candidates
            .filter_map(|candidate_idx| {
                self.match_child_node_once(pattern_node, &nodes[candidate_idx], state.clone())
                    .and_then(|next_state| {
                        self.match_child_sequence_from(
                            patterns,
                            nodes,
                            pattern_idx + 1,
                            candidate_idx + 1,
                            next_state,
                            false,
                        )
                    })
            })
            .next()
    }

    fn match_repeated_child_candidates(
        &self,
        pattern_node: &PatternNode,
        patterns: &[PatternChild],
        nodes: &[ParseNode],
        position: (usize, usize),
        state: MatchState,
        repeat: RepeatSearch,
    ) -> Option<MatchState> {
        let (_, node_idx) = position;
        let mut candidates: Box<dyn Iterator<Item = usize>> = if repeat.anchored_next {
            Box::new((node_idx < nodes.len()).then_some(node_idx).into_iter())
        } else {
            Box::new(node_idx..nodes.len())
        };

        candidates.find_map(|candidate_idx| {
            self.match_repeated_child_node(
                pattern_node,
                patterns,
                nodes,
                (position.0, candidate_idx),
                state.clone(),
                repeat.min_count,
            )
        })
    }

    fn match_child_node_once(
        &self,
        pattern_node: &PatternNode,
        node: &ParseNode,
        mut state: MatchState,
    ) -> Option<MatchState> {
        let mut single = pattern_node.clone();
        single.quantifier = Quantifier::One;

        self.match_node(&single, node, &mut state).then_some(state)
    }

    fn match_repeated_child_node(
        &self,
        pattern_node: &PatternNode,
        patterns: &[PatternChild],
        nodes: &[ParseNode],
        position: (usize, usize),
        state: MatchState,
        min_count: usize,
    ) -> Option<MatchState> {
        let (pattern_idx, node_idx) = position;
        let mut candidates = vec![(node_idx, state)];
        let mut cursor = node_idx;

        while cursor < nodes.len() {
            let (_, last_state) = candidates.last().expect("candidate seed exists");
            let Some(next_state) =
                self.match_child_node_once(pattern_node, &nodes[cursor], last_state.clone())
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
                    patterns,
                    nodes,
                    pattern_idx + 1,
                    next_node_idx,
                    next_state,
                    false,
                )
            })
    }

    /// Check if predicates are satisfied
    fn check_predicates(
        &self,
        predicates: &[Predicate],
        captures: &HashMap<u32, ParseNode>,
    ) -> bool {
        for predicate in predicates {
            if !self.check_predicate(predicate, captures) {
                return false;
            }
        }
        true
    }

    /// Check a single predicate
    fn check_predicate(&self, predicate: &Predicate, captures: &HashMap<u32, ParseNode>) -> bool {
        match predicate {
            Predicate::Eq {
                capture1,
                capture2,
                value,
            } => {
                if let Some(node1) = captures.get(capture1) {
                    if let Some(capture2) = capture2 {
                        if let Some(node2) = captures.get(capture2) {
                            // Source-free matching can only compare captured spans.
                            return node1.start_byte == node2.start_byte
                                && node1.end_byte == node2.end_byte;
                        }
                    } else if value.is_some() {
                        return false;
                    }
                }
                false
            }
            Predicate::NotEq {
                capture1,
                capture2,
                value: _,
            } => {
                if let Some(capture2) = capture2 {
                    if let (Some(node1), Some(node2)) =
                        (captures.get(capture1), captures.get(capture2))
                    {
                        return node1.start_byte != node2.start_byte
                            || node1.end_byte != node2.end_byte;
                    }
                    false
                } else {
                    false
                }
            }
            Predicate::Set { .. } => true,
            Predicate::Match { .. }
            | Predicate::NotMatch { .. }
            | Predicate::AnyOf { .. }
            | Predicate::Is { .. }
            | Predicate::IsNot { .. }
            | Predicate::Custom { .. } => false,
        }
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
    pub fn new(query: &'a Query, root: &'a ParseNode) -> Self {
        Self::new_with_options(query, root, None, false)
    }

    /// Create a new query matches iterator using cursor options.
    pub(crate) fn new_with_options(
        query: &'a Query,
        root: &'a ParseNode,
        byte_range: Option<&std::ops::Range<usize>>,
        match_root: bool,
    ) -> Self {
        let matcher = QueryMatcher::new(query);
        let matches = matcher.matches_with_options(root, byte_range, match_root);
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

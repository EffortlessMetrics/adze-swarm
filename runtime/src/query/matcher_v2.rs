// Enhanced query pattern matching with predicate evaluation
use super::ast::*;
use super::predicate_eval::PredicateContext;
use crate::parser_v4::ParseNode;
use adze_glr_core::SymbolMetadata;
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
        let mut matches = Vec::new();

        // Try each pattern
        for (pattern_index, pattern) in self.query.patterns.iter().enumerate() {
            self.match_pattern(pattern_index, pattern, root, &mut matches);
        }

        matches
    }

    /// Match a single pattern against the tree
    fn match_pattern(
        &self,
        pattern_index: usize,
        pattern: &Pattern,
        root: &ParseNode,
        matches: &mut Vec<QueryMatch>,
    ) {
        // Walk the tree and try to match at each node
        self.match_pattern_at_node(pattern_index, pattern, root, matches);
    }

    /// Try to match pattern starting at a specific node
    fn match_pattern_at_node(
        &self,
        pattern_index: usize,
        pattern: &Pattern,
        node: &ParseNode,
        matches: &mut Vec<QueryMatch>,
    ) {
        // Try to match the pattern at this node
        let mut state = MatchState {
            captures: HashMap::new(),
        };

        if self.match_node(&pattern.root, node, &mut state) {
            // Check predicates with source text
            let predicate_ctx = PredicateContext::new(self.source);
            if pattern
                .predicates
                .iter()
                .all(|pred| predicate_ctx.evaluate(pred, &state.captures))
            {
                // Convert captures to vector
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

        // Recursively try child nodes
        for child in &node.children {
            self.match_pattern_at_node(pattern_index, pattern, child, matches);
        }
    }

    /// Match a pattern node against a parse node
    fn match_node(&self, pattern: &PatternNode, node: &ParseNode, state: &mut MatchState) -> bool {
        // Check symbol
        if pattern.symbol != node.symbol {
            return false;
        }
        // Confirm named/anonymous status matches the pattern expectation
        // When node metadata becomes available, this will use the actual flag.
        if self.node_is_named(node) != pattern.is_named {
            return false;
        }

        // Capture if needed
        if let Some(capture_id) = pattern.capture {
            state.captures.insert(capture_id, node.clone());
        }

        // Match based on quantifier
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

    /// Determine if a node should be treated as named using symbol metadata.
    fn node_is_named(&self, node: &ParseNode) -> bool {
        self.symbol_metadata
            .get(node.symbol.0 as usize)
            .map(|m| m.is_named)
            .unwrap_or(true)
    }

    /// Determine if a node should be treated as an "extra" node that should
    /// be ignored during pattern matching.
    fn node_is_extra(&self, node: &ParseNode) -> bool {
        self.symbol_metadata
            .get(node.symbol.0 as usize)
            .map(|m| m.is_extra)
            .unwrap_or(false)
    }

    /// Match a sequence of pattern children against node children
    fn match_child_sequence(
        &self,
        pattern_children: &[PatternChild],
        node_children: &[ParseNode],
        pattern_idx: usize,
        node_idx: usize,
        state: &mut MatchState,
    ) -> bool {
        if let Some(next_state) = self.match_child_sequence_from(
            pattern_children,
            node_children,
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
        pattern_children: &[PatternChild],
        node_children: &[ParseNode],
        pattern_idx: usize,
        node_idx: usize,
        state: MatchState,
        anchored_next: bool,
    ) -> Option<MatchState> {
        // Base case: all patterns matched
        if pattern_idx >= pattern_children.len() {
            // If extra nodes remain, ensure they're ignorable
            return Some(state);
        }

        let mut node_idx = node_idx;
        // Skip over any extra nodes before attempting to match
        while node_idx < node_children.len() && self.node_is_extra(&node_children[node_idx]) {
            node_idx += 1;
        }

        // Base case: no more nodes but patterns remain
        if node_idx >= node_children.len() {
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
                .anchor_satisfied(pattern_children, pattern_idx, node_children, node_idx)
                .then(|| {
                    self.match_child_sequence_from(
                        pattern_children,
                        node_children,
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
                    node_children,
                    (pattern_idx, node_idx),
                    state,
                    anchored_next,
                ),
                Quantifier::Optional => self
                    .match_single_child_candidate(
                        pattern_node,
                        pattern_children,
                        node_children,
                        (pattern_idx, node_idx),
                        state.clone(),
                        anchored_next,
                    )
                    .or_else(|| {
                        self.match_child_sequence_from(
                            pattern_children,
                            node_children,
                            pattern_idx + 1,
                            node_idx,
                            state,
                            anchored_next,
                        )
                    }),
                Quantifier::Plus => self.match_repeated_child_candidates(
                    pattern_node,
                    pattern_children,
                    node_children,
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
                        node_children,
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
                            node_children,
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
                node_children,
                (pattern_idx, node_idx),
                state,
                anchored_next,
            ),
        }
    }

    fn anchor_satisfied(
        &self,
        pattern_children: &[PatternChild],
        pattern_idx: usize,
        node_children: &[ParseNode],
        node_idx: usize,
    ) -> bool {
        if pattern_idx == 0 {
            node_children[..node_idx]
                .iter()
                .all(|n| self.node_is_extra(n))
        } else if pattern_idx + 1 == pattern_children.len() {
            node_children[node_idx..]
                .iter()
                .all(|n| self.node_is_extra(n))
        } else {
            true
        }
    }

    fn candidate_indices(
        &self,
        node_children: &[ParseNode],
        node_idx: usize,
        anchored_next: bool,
    ) -> Vec<usize> {
        if anchored_next {
            (node_idx < node_children.len())
                .then_some(node_idx)
                .into_iter()
                .collect()
        } else {
            (node_idx..node_children.len())
                .filter(|idx| !self.node_is_extra(&node_children[*idx]))
                .collect()
        }
    }

    fn match_single_child_candidate(
        &self,
        pattern_node: &PatternNode,
        pattern_children: &[PatternChild],
        node_children: &[ParseNode],
        position: (usize, usize),
        state: MatchState,
        anchored_next: bool,
    ) -> Option<MatchState> {
        let (pattern_idx, node_idx) = position;

        self.candidate_indices(node_children, node_idx, anchored_next)
            .into_iter()
            .filter_map(|candidate_idx| {
                self.match_child_node_once(
                    pattern_node,
                    &node_children[candidate_idx],
                    state.clone(),
                )
                .and_then(|next_state| {
                    self.match_child_sequence_from(
                        pattern_children,
                        node_children,
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
        pattern_children: &[PatternChild],
        node_children: &[ParseNode],
        position: (usize, usize),
        state: MatchState,
        repeat: RepeatSearch,
    ) -> Option<MatchState> {
        let (_, node_idx) = position;

        self.candidate_indices(node_children, node_idx, repeat.anchored_next)
            .into_iter()
            .find_map(|candidate_idx| {
                self.match_repeated_child_node(
                    pattern_node,
                    pattern_children,
                    node_children,
                    (position.0, candidate_idx),
                    state.clone(),
                    repeat.min_count,
                )
            })
    }

    fn match_token_candidate(
        &self,
        token: &str,
        pattern_children: &[PatternChild],
        node_children: &[ParseNode],
        position: (usize, usize),
        state: MatchState,
        anchored_next: bool,
    ) -> Option<MatchState> {
        let (pattern_idx, node_idx) = position;

        self.candidate_indices(node_children, node_idx, anchored_next)
            .into_iter()
            .filter_map(|candidate_idx| {
                (self.node_text(&node_children[candidate_idx]) == Some(token))
                    .then(|| {
                        self.match_child_sequence_from(
                            pattern_children,
                            node_children,
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
        pattern_children: &[PatternChild],
        node_children: &[ParseNode],
        position: (usize, usize),
        state: MatchState,
        min_count: usize,
    ) -> Option<MatchState> {
        let (pattern_idx, node_idx) = position;
        let mut candidates = vec![(node_idx, state)];
        let mut cursor = node_idx;

        while cursor < node_children.len() {
            let (_, last_state) = candidates.last().expect("candidate seed exists");
            let Some(next_state) = self.match_child_node_once(
                pattern_node,
                &node_children[cursor],
                last_state.clone(),
            ) else {
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
                    node_children,
                    pattern_idx + 1,
                    next_node_idx,
                    next_state,
                    false,
                )
            })
    }

    fn node_text(&self, node: &ParseNode) -> Option<&str> {
        self.source.get(node.start_byte..node.end_byte)
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
}

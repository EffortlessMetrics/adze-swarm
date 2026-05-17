// Query cursor for efficient matching
use super::{Query, QueryMatch, QueryMatches};
use crate::parser_v4::ParseNode;
use std::ops::Range;

/// A stateful object for executing queries on a syntax tree
pub struct QueryCursor {
    /// Byte range to restrict matches
    byte_range: Option<Range<usize>>,
    /// Whether to match only the root node
    match_root: bool,
}

impl QueryCursor {
    /// Create a new query cursor
    pub fn new() -> Self {
        QueryCursor {
            byte_range: None,
            match_root: false,
        }
    }

    /// Set the byte range for matching
    pub fn set_byte_range(&mut self, range: Range<usize>) {
        self.byte_range = Some(range);
    }

    /// Clear the byte range restriction
    pub fn clear_byte_range(&mut self) {
        self.byte_range = None;
    }

    /// Set whether to match only at the root
    pub fn set_match_root(&mut self, match_root: bool) {
        self.match_root = match_root;
    }

    /// Execute a query and return all matches
    pub fn matches<'a>(&'a mut self, query: &'a Query, root: &'a ParseNode) -> QueryMatches<'a> {
        QueryMatches::new_with_options(query, root, self.byte_range.as_ref(), self.match_root)
    }

    /// Execute a query and collect all matches into a vector
    pub fn collect_matches(&mut self, query: &Query, root: &ParseNode) -> Vec<QueryMatch> {
        self.matches(query, root).collect()
    }

    /// Check if a node is within the configured byte range
    #[allow(dead_code)]
    fn is_in_range(&self, node: &ParseNode) -> bool {
        self.byte_range
            .as_ref()
            .is_none_or(|range| node.start_byte >= range.start && node.end_byte <= range.end)
    }
}

impl Default for QueryCursor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::ast::{Pattern, PatternChild, PatternNode, Predicate};
    use adze_ir::SymbolId;
    use std::collections::HashMap;

    fn parse_node(
        symbol: u16,
        start_byte: usize,
        end_byte: usize,
        children: Vec<ParseNode>,
    ) -> ParseNode {
        let symbol = SymbolId(symbol);
        ParseNode {
            symbol,
            symbol_id: symbol,
            start_byte,
            end_byte,
            field_name: None,
            alias_symbol_id: None,
            children,
        }
    }

    fn capture_query(symbol: u16) -> Query {
        query_with_root(PatternNode::new(SymbolId(symbol), true).with_capture(0))
    }

    fn parent_capture_query(root_symbol: u16, child_symbol: u16) -> Query {
        let mut root = PatternNode::new(SymbolId(root_symbol), true);
        root.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(child_symbol), true).with_capture(0),
        ));
        query_with_root(root)
    }

    fn literal_child_query(root_symbol: u16, literal: &str) -> Query {
        let mut root = PatternNode::new(SymbolId(root_symbol), true);
        root.add_child(PatternChild::Token(literal.to_string()));
        query_with_root(root)
    }

    fn repeated_child_query(
        root_symbol: u16,
        repeated_symbol: u16,
        quantifier: crate::query::ast::Quantifier,
        tail_symbol: u16,
    ) -> Query {
        let mut root = PatternNode::new(SymbolId(root_symbol), true);
        root.add_child(PatternChild::Node(
            PatternNode::new(SymbolId(repeated_symbol), true).with_quantifier(quantifier),
        ));
        root.add_child(PatternChild::Node(PatternNode::new(
            SymbolId(tail_symbol),
            true,
        )));
        query_with_root(root)
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

    fn capture_query_with_predicate(symbol: u16, predicate: Predicate) -> Query {
        let mut query = capture_query(symbol);
        query.patterns[0].predicates.push(predicate);
        query
    }

    fn sample_tree() -> ParseNode {
        parse_node(
            1,
            0,
            5,
            vec![
                parse_node(2, 0, 1, Vec::new()),
                parse_node(3, 1, 2, Vec::new()),
                parse_node(2, 3, 5, Vec::new()),
            ],
        )
    }

    #[test]
    fn test_byte_range_when_capture_overlaps_keeps_match() {
        let query = capture_query(2);
        let tree = sample_tree();

        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(3..5);

        let matches = cursor.collect_matches(&query, &tree);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].captures[0].node.start_byte, 3);
        assert_eq!(matches[0].captures[0].node.end_byte, 5);
    }

    #[test]
    fn test_byte_range_when_parent_root_extends_outside_range_keeps_capture_match() {
        let query = parent_capture_query(1, 2);
        let tree = parse_node(1, 0, 5, vec![parse_node(2, 3, 5, Vec::new())]);

        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(3..5);

        let matches = cursor.collect_matches(&query, &tree);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].captures[0].node.start_byte, 3);
        assert_eq!(matches[0].captures[0].node.end_byte, 5);
    }

    #[test]
    fn test_clear_byte_range_after_setting_restores_full_tree_matching() {
        let query = capture_query(2);
        let tree = sample_tree();

        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(3..5);
        cursor.clear_byte_range();

        let matches = cursor.collect_matches(&query, &tree);

        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_match_root_when_child_matches_suppresses_recursive_match() {
        let query = capture_query(2);
        let tree = sample_tree();

        let mut cursor = QueryCursor::new();
        cursor.set_match_root(true);

        let matches = cursor.collect_matches(&query, &tree);

        assert!(matches.is_empty());
    }

    #[test]
    fn test_match_root_when_root_matches_returns_root_match() {
        let query = capture_query(1);
        let tree = sample_tree();

        let mut cursor = QueryCursor::new();
        cursor.set_match_root(true);

        let matches = cursor.collect_matches(&query, &tree);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].captures[0].node.start_byte, 0);
        assert_eq!(matches[0].captures[0].node.end_byte, 5);
    }

    #[test]
    fn test_literal_child_when_source_text_unavailable_returns_no_match() {
        let query = literal_child_query(1, "+");
        let tree = parse_node(1, 0, 1, vec![parse_node(2, 0, 1, Vec::new())]);

        let mut cursor = QueryCursor::new();
        let matches = cursor.collect_matches(&query, &tree);

        assert!(matches.is_empty());
    }

    #[test]
    fn test_eq_literal_predicate_when_source_text_unavailable_returns_no_match() {
        let query = capture_query_with_predicate(
            1,
            Predicate::Eq {
                capture1: 0,
                capture2: None,
                value: Some("alpha".to_string()),
            },
        );
        let tree = parse_node(1, 0, 5, Vec::new());

        let mut cursor = QueryCursor::new();
        let matches = cursor.collect_matches(&query, &tree);

        assert!(matches.is_empty());
    }

    #[test]
    fn test_not_eq_literal_predicate_when_source_text_unavailable_returns_no_match() {
        let query = capture_query_with_predicate(
            1,
            Predicate::NotEq {
                capture1: 0,
                capture2: None,
                value: Some("alpha".to_string()),
            },
        );
        let tree = parse_node(1, 0, 5, Vec::new());

        let mut cursor = QueryCursor::new();
        let matches = cursor.collect_matches(&query, &tree);

        assert!(matches.is_empty());
    }

    #[test]
    fn test_match_predicate_when_source_text_unavailable_returns_no_match() {
        let query = capture_query_with_predicate(
            1,
            Predicate::Match {
                capture: 0,
                regex: "^alpha$".to_string(),
            },
        );
        let tree = parse_node(1, 0, 5, Vec::new());

        let mut cursor = QueryCursor::new();
        let matches = cursor.collect_matches(&query, &tree);

        assert!(matches.is_empty());
    }

    #[test]
    fn test_any_of_predicate_when_source_text_unavailable_returns_no_match() {
        let query = capture_query_with_predicate(
            1,
            Predicate::AnyOf {
                capture: 0,
                values: vec!["alpha".to_string(), "beta".to_string()],
            },
        );
        let tree = parse_node(1, 0, 5, Vec::new());

        let mut cursor = QueryCursor::new();
        let matches = cursor.collect_matches(&query, &tree);

        assert!(matches.is_empty());
    }

    #[test]
    fn test_plus_child_quantifier_when_followed_by_tail_consumes_repeated_nodes() {
        let query = repeated_child_query(1, 2, crate::query::ast::Quantifier::Plus, 3);
        let tree = parse_node(
            1,
            0,
            3,
            vec![
                parse_node(2, 0, 1, Vec::new()),
                parse_node(2, 1, 2, Vec::new()),
                parse_node(3, 2, 3, Vec::new()),
            ],
        );

        let mut cursor = QueryCursor::new();
        let matches = cursor.collect_matches(&query, &tree);

        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_star_child_quantifier_when_followed_by_tail_allows_zero_matches() {
        let query = repeated_child_query(1, 2, crate::query::ast::Quantifier::Star, 3);
        let tree = parse_node(1, 0, 1, vec![parse_node(3, 0, 1, Vec::new())]);

        let mut cursor = QueryCursor::new();
        let matches = cursor.collect_matches(&query, &tree);

        assert_eq!(matches.len(), 1);
    }
}

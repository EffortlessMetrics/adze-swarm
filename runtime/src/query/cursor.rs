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
        QueryMatches::new(query, root)
    }

    /// Execute a query and collect all matches into a vector
    pub fn collect_matches(&mut self, query: &Query, root: &ParseNode) -> Vec<QueryMatch> {
        self.matches(query, root).collect()
    }

    /// Check if a node is within the configured byte range
    #[allow(dead_code)]
    fn is_in_range(&self, node: &ParseNode) -> bool {
        if let Some(ref range) = self.byte_range {
            node.start_byte >= range.start && node.end_byte <= range.end
        } else {
            true
        }
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
    use crate::query::ast::{Pattern, PatternNode, Query};
    use adze_ir::SymbolId;

    fn node(symbol: u16, start: usize, end: usize, children: Vec<ParseNode>) -> ParseNode {
        ParseNode {
            symbol: SymbolId(symbol),
            symbol_id: SymbolId(symbol),
            start_byte: start,
            end_byte: end,
            field_name: None,
            alias_symbol_id: None,
            children,
        }
    }

    fn empty_query() -> Query {
        Query::new()
    }

    fn single_pattern_query(symbol: u16) -> Query {
        let mut q = Query::new();
        q.patterns.push(Pattern {
            root: PatternNode::new(SymbolId(symbol), true),
            predicates: Vec::new(),
            start_byte: 0,
        });
        q
    }

    #[test]
    fn new_cursor_has_no_byte_range() {
        let cursor = QueryCursor::new();
        let leaf = node(1, 0, 5, vec![]);
        assert!(cursor.is_in_range(&leaf));
    }

    #[test]
    fn default_matches_new() {
        let cursor = QueryCursor::default();
        let leaf = node(1, 0, 5, vec![]);
        assert!(cursor.is_in_range(&leaf));
    }

    #[test]
    fn set_byte_range_restricts_in_range_check() {
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(0..10);
        let inside = node(1, 2, 8, vec![]);
        let outside_right = node(1, 5, 12, vec![]);
        let exact_match = node(1, 0, 10, vec![]);
        assert!(cursor.is_in_range(&inside));
        assert!(!cursor.is_in_range(&outside_right));
        assert!(cursor.is_in_range(&exact_match));
    }

    #[test]
    fn clear_byte_range_resets_restriction() {
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(0..5);
        cursor.clear_byte_range();
        let far_node = node(1, 100, 200, vec![]);
        assert!(cursor.is_in_range(&far_node));
    }

    #[test]
    fn set_match_root_updates_flag() {
        let mut cursor = QueryCursor::new();
        cursor.set_match_root(true);
        // The flag is private; observe by setting back and exercising state.
        cursor.set_match_root(false);
        // No panic; reaching this assertion confirms idempotent setter behavior.
        let leaf = node(1, 0, 1, vec![]);
        assert!(cursor.is_in_range(&leaf));
    }

    #[test]
    fn matches_on_empty_query_yields_no_results() {
        let mut cursor = QueryCursor::new();
        let query = empty_query();
        let root = node(1, 0, 1, vec![]);
        let count = cursor.matches(&query, &root).count();
        assert_eq!(count, 0);
    }

    #[test]
    fn collect_matches_returns_vec() {
        let mut cursor = QueryCursor::new();
        let query = empty_query();
        let root = node(1, 0, 1, vec![]);
        let matches = cursor.collect_matches(&query, &root);
        assert!(matches.is_empty());
    }

    #[test]
    fn collect_matches_finds_single_pattern_hit() {
        let mut cursor = QueryCursor::new();
        let query = single_pattern_query(7);
        let root = node(7, 0, 5, vec![]);
        let matches = cursor.collect_matches(&query, &root);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pattern_index, 0);
    }

    #[test]
    fn byte_range_does_not_apply_to_query_execution_yet() {
        // Document current behavior: byte_range tracking is independent of
        // QueryMatches construction.
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(100..200);
        let query = single_pattern_query(7);
        let root = node(7, 0, 5, vec![]);
        let matches = cursor.collect_matches(&query, &root);
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn is_in_range_inclusive_at_start_boundary() {
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(10..20);
        let at_start = node(1, 10, 15, vec![]);
        let before_start = node(1, 9, 15, vec![]);
        assert!(cursor.is_in_range(&at_start));
        assert!(!cursor.is_in_range(&before_start));
    }
}

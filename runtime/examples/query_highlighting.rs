//! Runnable query and highlighting subset example.
//!
//! This uses a tiny hand-built parse tree so the example can focus on query
//! behavior rather than grammar generation. It demonstrates the documented
//! supported subset: captures, field constraints, anchors, source-aware
//! predicates, byte-range filtering, root-only filtering, and highlight ranges.

use adze::{
    parser_v4::ParseNode,
    query::{Highlighter, QueryCapture, QueryCursor, compile_query, matcher_v2::QueryMatcher},
};
use adze_glr_core::SymbolMetadata;
use adze_ir::{Grammar, ProductionId, Rule, SymbolId, Token, TokenPattern};

fn main() {
    let receipt = run_demo();
    println!("query highlighting receipt: {receipt:?}");
}

#[derive(Debug, PartialEq, Eq)]
struct QueryHighlightingReceipt {
    highlight_ranges: Vec<(usize, usize)>,
    source_aware_capture_ranges: Vec<(usize, usize)>,
    source_aware_literal_capture_ranges: Vec<(usize, usize)>,
    source_free_literal_matches: usize,
    source_free_predicate_matches: usize,
    byte_range_number_capture: Vec<(usize, usize)>,
    out_of_range_number_matches: usize,
    cleared_byte_range_number_matches: usize,
    root_only_identifier_matches: usize,
}

fn run_demo() -> QueryHighlightingReceipt {
    let source = "foo+1";
    let grammar = query_fixture_grammar();
    let metadata = query_fixture_metadata();
    let tree = root(
        vec![
            field_node(1, 0, 3, "left"),
            node(3, 3, 4),
            field_node(2, 4, 5, "right"),
        ],
        source.len(),
    );

    let highlight_query = compile_query(
        "(root (identifier @variable) . (operator @operator) (number @number))",
        &grammar,
    )
    .expect("highlight query should compile");
    let highlights = Highlighter::new(highlight_query).highlight(&tree);
    let highlight_ranges = highlight_ranges(&highlights);
    assert_eq!(highlight_ranges, vec![(0, 3), (3, 4), (4, 5)]);

    let source_aware_query = compile_query(
        "(root left: (identifier @name) right: (number @value))\n(#match? @name \"^[a-z]+$\")",
        &grammar,
    )
    .expect("field and predicate query should compile");
    let source_aware_matches =
        QueryMatcher::new(&source_aware_query, source, &metadata).matches(&tree);
    assert_eq!(source_aware_matches.len(), 1);
    let source_aware_capture_ranges =
        source_aware_capture_byte_ranges(&source_aware_matches[0].captures);
    assert_eq!(source_aware_capture_ranges, vec![(0, 3), (4, 5)]);

    let literal_query = compile_query("(root (identifier) \"+\" (number @number))", &grammar)
        .expect("source-aware literal token query should compile");
    let literal_matches = QueryMatcher::new(&literal_query, source, &metadata).matches(&tree);
    assert_eq!(literal_matches.len(), 1);
    let source_aware_literal_capture_ranges =
        source_aware_capture_byte_ranges(&literal_matches[0].captures);
    assert_eq!(source_aware_literal_capture_ranges, vec![(4, 5)]);

    let mut source_free_cursor = QueryCursor::new();
    let source_free_literal_matches = source_free_cursor
        .collect_matches(&literal_query, &tree)
        .len();
    assert_eq!(source_free_literal_matches, 0);
    let source_free_predicate_matches = source_free_cursor
        .collect_matches(&source_aware_query, &tree)
        .len();
    assert_eq!(source_free_predicate_matches, 0);

    let number_query =
        compile_query("(number @number)", &grammar).expect("number query should compile");
    let mut range_cursor = QueryCursor::new();
    range_cursor.set_byte_range(4..5);
    let range_matches = range_cursor.collect_matches(&number_query, &tree);
    assert_eq!(range_matches.len(), 1);
    assert_eq!(range_matches[0].captures[0].node.start_byte, 4);
    let byte_range_number_capture = cursor_capture_ranges(&range_matches[0].captures);
    assert_eq!(byte_range_number_capture, vec![(4, 5)]);

    range_cursor.set_byte_range(0..3);
    let out_of_range_number_matches = range_cursor.collect_matches(&number_query, &tree).len();
    assert_eq!(out_of_range_number_matches, 0);

    range_cursor.clear_byte_range();
    let cleared_byte_range_number_matches =
        range_cursor.collect_matches(&number_query, &tree).len();
    assert_eq!(cleared_byte_range_number_matches, 1);

    let identifier_query =
        compile_query("(identifier @variable)", &grammar).expect("identifier query should compile");
    let mut root_only_cursor = QueryCursor::new();
    root_only_cursor.set_match_root(true);
    let root_only_matches = root_only_cursor.collect_matches(&identifier_query, &tree);
    assert!(
        root_only_matches.is_empty(),
        "root-only matching should not recurse into identifier children"
    );
    let root_only_identifier_matches = root_only_matches.len();

    QueryHighlightingReceipt {
        highlight_ranges,
        source_aware_capture_ranges,
        source_aware_literal_capture_ranges,
        source_free_literal_matches,
        source_free_predicate_matches,
        byte_range_number_capture,
        out_of_range_number_matches,
        cleared_byte_range_number_matches,
        root_only_identifier_matches,
    }
}

fn node(symbol: u16, start_byte: usize, end_byte: usize) -> ParseNode {
    let symbol_id = SymbolId(symbol);
    ParseNode {
        symbol: symbol_id,
        symbol_id,
        children: Vec::new(),
        start_byte,
        end_byte,
        field_name: None,
        alias_symbol_id: None,
    }
}

fn field_node(symbol: u16, start_byte: usize, end_byte: usize, field_name: &str) -> ParseNode {
    let mut node = node(symbol, start_byte, end_byte);
    node.field_name = Some(field_name.to_string());
    node
}

fn root(children: Vec<ParseNode>, end_byte: usize) -> ParseNode {
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

fn query_fixture_grammar() -> Grammar {
    let mut grammar = Grammar::new("query_highlighting_example".to_string());
    grammar.rules.entry(SymbolId(0)).or_default().push(Rule {
        lhs: SymbolId(0),
        rhs: Vec::new(),
        precedence: None,
        associativity: None,
        fields: Vec::new(),
        production_id: ProductionId(0),
    });
    grammar.rule_names.insert(SymbolId(0), "root".to_string());
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "identifier".to_string(),
            pattern: TokenPattern::Regex("[a-zA-Z_]+".to_string()),
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

fn query_fixture_metadata() -> Vec<SymbolMetadata> {
    vec![
        symbol_metadata(0, "root", false),
        symbol_metadata(1, "identifier", true),
        symbol_metadata(2, "number", true),
        symbol_metadata(3, "operator", true),
    ]
}

fn symbol_metadata(id: u16, name: &str, terminal: bool) -> SymbolMetadata {
    SymbolMetadata {
        name: name.to_string(),
        is_visible: true,
        is_named: true,
        is_supertype: false,
        is_terminal: terminal,
        is_extra: false,
        is_fragile: false,
        symbol_id: SymbolId(id),
    }
}

fn highlight_ranges(highlights: &[adze::query::Highlight]) -> Vec<(usize, usize)> {
    highlights
        .iter()
        .map(|highlight| (highlight.start_byte, highlight.end_byte))
        .collect()
}

fn source_aware_capture_byte_ranges(
    captures: &[adze::query::matcher_v2::QueryCapture],
) -> Vec<(usize, usize)> {
    captures
        .iter()
        .map(|capture| (capture.node.start_byte, capture.node.end_byte))
        .collect()
}

fn cursor_capture_ranges(captures: &[QueryCapture]) -> Vec<(usize, usize)> {
    captures
        .iter()
        .map(|capture| (capture.node.start_byte, capture.node.end_byte))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_highlighting_example_receipt_covers_supported_subset() {
        assert_eq!(
            run_demo(),
            QueryHighlightingReceipt {
                highlight_ranges: vec![(0, 3), (3, 4), (4, 5)],
                source_aware_capture_ranges: vec![(0, 3), (4, 5)],
                source_aware_literal_capture_ranges: vec![(4, 5)],
                source_free_literal_matches: 0,
                source_free_predicate_matches: 0,
                byte_range_number_capture: vec![(4, 5)],
                out_of_range_number_matches: 0,
                cleared_byte_range_number_matches: 1,
                root_only_identifier_matches: 0,
            }
        );
    }
}

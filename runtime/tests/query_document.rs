//! Document-backed query matcher coverage through public APIs only.

#![cfg(all(test, feature = "pure-rust", feature = "query"))]

use adze::query::matcher_v2::{DocumentQueryMatch, QueryMatcher};
use adze_ir::Grammar;

fn query_grammar_for(document: &adze::document::AdzeDocument) -> Grammar {
    let mut grammar = Grammar::new("parser-produced-query".to_string());
    for symbol in document.language().symbols() {
        // find_symbol resolves through rule keys + rule_names; empty rule
        // bodies suffice since matching never lowers this grammar.
        grammar
            .rule_names
            .insert(symbol.symbol_id(), symbol.name().to_string());
        grammar.rules.entry(symbol.symbol_id()).or_default();
    }
    grammar
}

fn captured_texts<'a>(
    document: &'a adze::document::AdzeDocument,
    source: &'a str,
    m: &DocumentQueryMatch,
) -> Vec<&'a str> {
    m.captures
        .iter()
        .map(|capture| {
            let node = document
                .tree()
                .node(capture.node_id)
                .expect("captured id resolves");
            &source[node.byte_range()]
        })
        .collect()
}

#[test]
fn test_matches_document_on_parser_produced_document() {
    // Real lexer and tables: the arithmetic grammar parses "1+2" into
    // (Expr_Add left:(Expr) operator:(+) right:(Expr)); both sides share
    // one capture name. Query identifiers resolve through the document's
    // own language facts.
    let source = "1+2";
    let document =
        adze_example::fielded_precedence_typed_cst_contract::grammar::parse_document(source)
            .expect("real parse succeeds");
    let grammar = query_grammar_for(&document);
    let query = adze::query::compile_query("(Expr_Add (Expr @side) (Expr @side))", &grammar)
        .expect("query over real language facts");

    let matcher = QueryMatcher::new(&query, source, &[]);
    let matches = matcher.matches_document(&document);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].captures.len(), 2);
    assert!(matches[0].captures.iter().all(|c| c.index == 0));
    assert_eq!(
        captured_texts(&document, source, &matches[0]),
        vec!["1", "2"]
    );
}

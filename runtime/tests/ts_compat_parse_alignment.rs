//! Parser::parse() must produce the same tree shape as Tree::from_document.
//!
//! Regression tests for the degenerate-root bug documented in #842: a
//! ts-compat Language built from a generated TSLanguage produced a root
//! node of kind "end" (0-width, `(MISSING)` S-expression) through
//! `Parser::parse()`, while `Tree::from_document` over the generated
//! `parse_document` output was correct.
#![cfg(all(feature = "ts-compat", feature = "pure-rust"))]

use adze::ts_compat::{Language, Parser};
use std::sync::Arc;

fn arithmetic_language() -> Arc<Language> {
    Arc::new(Language::from_ts_language(
        "arithmetic",
        &adze_example::arithmetic::LANGUAGE,
    ))
}

#[test]
fn parser_parse_produces_real_root_not_degenerate_end() {
    let language = arithmetic_language();
    let mut parser = Parser::new();
    parser
        .set_language(Arc::clone(&language))
        .expect("set_language should succeed");

    let tree = parser.parse("1 - 2", None).expect("parse should succeed");
    let root = tree.root_node();

    assert_eq!(root.kind(), "source_file", "root should be source_file");
    assert_eq!(root.start_byte(), 0, "root should start at byte 0");
    assert_eq!(root.end_byte(), 5, "root should span the full source");
    assert!(root.child_count() > 0, "root should have children");
    assert_eq!(tree.error_count(), 0, "clean input should have no errors");
}

#[test]
fn parser_parse_sexp_is_not_missing() {
    let language = arithmetic_language();
    let mut parser = Parser::new();
    parser
        .set_language(Arc::clone(&language))
        .expect("set_language should succeed");

    let tree = parser.parse("1 - 2", None).expect("parse should succeed");
    let sexp = tree.root_node().to_sexp();

    assert!(
        !sexp.contains("MISSING"),
        "clean input must not produce a MISSING root, got: {sexp}"
    );
    assert!(
        sexp.contains("source_file"),
        "S-expression should include the real root kind, got: {sexp}"
    );
}

#[test]
fn parser_parse_bad_input_still_returns_tree_with_errors() {
    let language = arithmetic_language();
    let mut parser = Parser::new();
    parser
        .set_language(Arc::clone(&language))
        .expect("set_language should succeed");

    let tree = parser
        .parse("1 - @", None)
        .expect("bad input should still produce a tree");
    let root = tree.root_node();

    assert!(
        tree.error_count() > 0 || root.has_error(),
        "bad input should report errors"
    );
}

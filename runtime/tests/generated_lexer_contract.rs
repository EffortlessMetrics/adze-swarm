//! Runtime contract for mode-aware generated lexer integration on the deterministic path (#927).

#![cfg(all(test, feature = "pure-rust"))]

use adze_example::typed_ast_contract::grammar::{self, Expr};
use adze_example::words;

fn assert_generated_lexer_present(language: &adze::pure_parser::TSLanguage) {
    assert!(
        language.lex_fn.is_some(),
        "deterministic generated grammars must expose a mode-aware lex_fn"
    );
}

#[test]
fn generated_lexer_present_on_conflict_free_grammars() {
    assert_generated_lexer_present(grammar::language());
    assert_generated_lexer_present(words::grammar::language());
}

#[test]
fn parse_and_parse_document_agree_for_clean_input() {
    let source = "1 + 2 + 3";
    let typed = grammar::parse(source).expect("typed parse should succeed");
    let document = grammar::parse_document(source).expect("document parse should succeed");

    assert_eq!(
        typed,
        Expr::Add(
            Box::new(Expr::Add(
                Box::new(Expr::Number(1)),
                (),
                Box::new(Expr::Number(2)),
            )),
            (),
            Box::new(Expr::Number(3)),
        )
    );
    assert_eq!(document.ast::<Expr>().expect("document ast"), typed);
    assert!(document.diagnostics().is_empty());
}

#[test]
fn keyword_boundary_rejects_glued_identifier_prefix() {
    assert!(
        words::grammar::parse("ifhello").is_err(),
        "keyword prefix glued to identifier letters must not silently lex as keyword+word"
    );
}

#[test]
fn invalid_lexical_input_returns_bounded_errors_without_panic() {
    let errors = grammar::parse("1 + @").expect_err("invalid token should fail typed parse");
    assert!(!errors.is_empty(), "expected at least one structured parse error");
    assert!(
        errors.iter().all(|error| error.end >= error.start),
        "diagnostic spans must be bounded"
    );

    let document = grammar::parse_document("1 + @").expect("document parse should not panic");
    assert!(
        !document.diagnostics().is_empty(),
        "document path should surface lexical/parse diagnostics"
    );
}

#[test]
fn repeated_parse_is_deterministic() {
    let source = "10 + 20";
    let first = grammar::parse(source).expect("first parse");
    let second = grammar::parse(source).expect("second parse");
    assert_eq!(first, second);

    let doc_first = grammar::parse_document(source).expect("first document");
    let doc_second = grammar::parse_document(source).expect("second document");
    assert_eq!(
        doc_first.ast::<Expr>().expect("first document ast"),
        doc_second.ast::<Expr>().expect("second document ast")
    );
}

#[test]
fn meaningful_whitespace_is_skipped_via_generated_extras() {
    let spaced = grammar::parse("  1  +  2  ").expect("whitespace-surrounded expression");
    let tight = grammar::parse("1+2").expect("tight expression");
    assert_eq!(spaced, tight);
}

#[cfg(feature = "glr")]
#[test]
fn conflict_free_glr_routing_uses_generated_lexer_path() {
    // With glr enabled, conflict-free tables must route through pure_parser (generated lex_fn),
    // not parser_v4's legacy GrammarLexer.
    let language = grammar::language();
    assert_generated_lexer_present(language);

    let typed = grammar::parse("3 + 4").expect("glr-routed conflict-free parse");
    assert_eq!(
        typed,
        Expr::Add(
            Box::new(Expr::Number(3)),
            (),
            Box::new(Expr::Number(4)),
        )
    );
}

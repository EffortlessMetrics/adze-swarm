//! Snapshot tests for IR validation error messages.
//!
//! Verifies that validation produces correct, informative error messages
//! for each error type.

use adze_ir::builder::GrammarBuilder;
use adze_ir::validation::GrammarValidator;
use adze_ir::*;

/// Helper: validate grammar and return formatted error/warning messages.
fn validate_messages(g: &Grammar) -> String {
    let mut validator = GrammarValidator::new();
    let result = validator.validate(g);
    let mut output = String::new();
    if result.errors.is_empty() && result.warnings.is_empty() {
        output.push_str("OK: no errors or warnings\n");
    }
    for e in &result.errors {
        output.push_str(&format!("ERROR: {e}\n"));
    }
    for w in &result.warnings {
        output.push_str(&format!("WARNING: {w}\n"));
    }
    output.push_str(&format!(
        "stats: rules={}, tokens={}, symbols={}",
        result.stats.total_rules, result.stats.total_tokens, result.stats.total_symbols
    ));
    output
}

#[test]
fn validate_empty_grammar() {
    let g = Grammar::new("empty".to_string());
    let msgs = validate_messages(&g);
    insta::assert_snapshot!(msgs);
}

#[test]
fn validate_valid_grammar() {
    let g = GrammarBuilder::new("valid")
        .token("num", r"\d+")
        .rule("expr", vec!["num"])
        .start("expr")
        .build();
    let msgs = validate_messages(&g);
    insta::assert_snapshot!(msgs);
}

#[test]
fn validate_single_token_no_rules() {
    let mut g = Grammar::new("tok_only".to_string());
    g.tokens.insert(
        SymbolId(1),
        Token {
            name: "x".into(),
            pattern: TokenPattern::String("x".into()),
            fragile: false,
        },
    );
    let msgs = validate_messages(&g);
    insta::assert_snapshot!(msgs);
}

#[test]
fn validate_grammar_with_precedence() {
    let g = GrammarBuilder::new("prec")
        .token("num", r"\d+")
        .token("plus", r"\+")
        .token("star", r"\*")
        .rule("expr", vec!["num"])
        .rule("expr", vec!["expr", "plus", "expr"])
        .rule("expr", vec!["expr", "star", "expr"])
        .rule_with_precedence("expr", vec!["expr", "plus", "expr"], 1, Associativity::Left)
        .start("expr")
        .build();
    let msgs = validate_messages(&g);
    insta::assert_snapshot!(msgs);
}

#[test]
fn validate_grammar_with_extras() {
    let g = GrammarBuilder::new("extras")
        .token("num", r"\d+")
        .token("ws", r"\s+")
        .rule("expr", vec!["num"])
        .start("expr")
        .extra("ws")
        .build();
    let msgs = validate_messages(&g);
    insta::assert_snapshot!(msgs);
}

#[test]
fn validate_grammar_with_externals() {
    let g = GrammarBuilder::new("externals")
        .token("num", r"\d+")
        .rule("expr", vec!["num"])
        .start("expr")
        .external("indent")
        .build();
    let msgs = validate_messages(&g);
    insta::assert_snapshot!(msgs);
}

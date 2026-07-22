//! Tests for IR grammar normalization and complex symbol processing.

use adze_ir::builder::GrammarBuilder;
use adze_ir::*;

#[test]
fn normalize_simple_grammar_preserves_rules() {
    let mut g = GrammarBuilder::new("simple")
        .token("num", r"\d+")
        .rule("expr", vec!["num"])
        .build();
    let rule_count_before = g.rules.values().map(|v| v.len()).sum::<usize>();
    g.normalize();
    let rule_count_after = g.rules.values().map(|v| v.len()).sum::<usize>();
    // Simple grammars should not change significantly
    assert!(
        rule_count_after >= rule_count_before,
        "normalization should not lose rules"
    );
}

#[test]
fn normalize_preserves_start_symbol() {
    let mut g = GrammarBuilder::new("start_test")
        .token("x", "x")
        .rule("start", vec!["x"])
        .start("start")
        .build();
    let start_before = g.start_symbol();
    g.normalize();
    let start_after = g.start_symbol();
    assert_eq!(start_before, start_after, "start symbol should not change");
}

#[test]
fn normalize_preserves_tokens() {
    let mut g = GrammarBuilder::new("tokens")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .rule("start", vec!["a", "b", "c"])
        .build();
    let token_count_before = g.tokens.len();
    g.normalize();
    assert_eq!(
        g.tokens.len(),
        token_count_before,
        "normalization should not change token count"
    );
}

#[test]
fn normalize_preserves_extras() {
    let mut g = GrammarBuilder::new("extras")
        .token("x", "x")
        .token("ws", r"\s+")
        .rule("start", vec!["x"])
        .extra("ws")
        .build();
    let extras_count = g.extras.len();
    g.normalize();
    assert_eq!(g.extras.len(), extras_count);
}

#[test]
fn normalize_preserves_externals() {
    let mut g = GrammarBuilder::new("ext")
        .token("x", "x")
        .rule("start", vec!["x"])
        .external("indent")
        .external("dedent")
        .build();
    let ext_count = g.externals.len();
    g.normalize();
    assert_eq!(g.externals.len(), ext_count);
}

#[test]
fn grammar_start_symbol_requires_explicit_metadata() {
    let g = GrammarBuilder::new("auto_start")
        .token("x", "x")
        .rule("program", vec!["x"])
        .start("program")
        .build();
    assert_eq!(g.start_symbol(), g.explicit_start_symbol());
}

#[test]
fn grammar_name_preserved() {
    let g = GrammarBuilder::new("my_grammar")
        .token("x", "x")
        .rule("start", vec!["x"])
        .build();
    assert_eq!(g.name, "my_grammar");
}

#[test]
fn grammar_multiple_rules_same_lhs() {
    let g = GrammarBuilder::new("multi")
        .token("a", "a")
        .token("b", "b")
        .rule("expr", vec!["a"])
        .rule("expr", vec!["b"])
        .build();
    // Both rules for "expr" should exist
    let expr_rules: usize = g.rules.values().map(|v| v.len()).sum();
    assert!(expr_rules >= 2, "should have at least 2 rules");
}

#[test]
fn grammar_rule_rhs_symbols() {
    let g = GrammarBuilder::new("rhs_test")
        .token("a", "a")
        .token("b", "b")
        .rule("start", vec!["a", "b"])
        .build();
    // At least one rule with 2 RHS symbols
    let has_binary = g.rules.values().flatten().any(|r| r.rhs.len() == 2);
    assert!(has_binary, "should have a rule with 2 RHS symbols");
}

#[test]
fn symbol_types_terminal_vs_nonterminal() {
    let terminal = Symbol::Terminal(SymbolId(1));
    let nonterminal = Symbol::NonTerminal(SymbolId(2));
    assert_ne!(format!("{terminal:?}"), format!("{nonterminal:?}"));
}

#[test]
fn grammar_conflict_declarations() {
    let g = GrammarBuilder::new("conflicts")
        .token("x", "x")
        .rule("start", vec!["x"])
        .build();
    // Conflicts starts empty
    assert!(g.conflicts.is_empty());
}

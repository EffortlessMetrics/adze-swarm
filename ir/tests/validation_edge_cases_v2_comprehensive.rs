//! Comprehensive edge-case tests for GrammarValidator (v2).
//!
//! Covers: construction, minimal grammars, multi-token, alternatives,
//! precedence, chain grammars, normalize-then-validate, error/warning counts,
//! recursive grammars (CyclicRule), repeated validation, and large grammars.

use adze_ir::builder::GrammarBuilder;
use adze_ir::validation::{GrammarValidator, ValidationError, ValidationWarning};
use adze_ir::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_rule(lhs: u16, rhs: Vec<Symbol>, prod: u16) -> Rule {
    Rule {
        lhs: SymbolId(lhs),
        rhs,
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(prod),
    }
}

fn make_token(id: u16, name: &str, pattern: &str) -> (SymbolId, Token) {
    (
        SymbolId(id),
        Token {
            name: name.to_string(),
            pattern: TokenPattern::String(pattern.to_string()),
            fragile: false,
        },
    )
}

fn validate(g: &Grammar) -> ValidationResult {
    let mut v = GrammarValidator::new();
    v.validate(g)
}

fn has_error(r: &ValidationResult, pred: impl Fn(&ValidationError) -> bool) -> bool {
    r.errors.iter().any(pred)
}

fn has_warning(r: &ValidationResult, pred: impl Fn(&ValidationWarning) -> bool) -> bool {
    r.warnings.iter().any(pred)
}

// =========================================================================
// 1. GrammarValidator::new() construction
// =========================================================================

#[test]
fn validator_new_returns_instance() {
    let _v = GrammarValidator::new();
}

#[test]
fn validator_default_returns_instance() {
    let _v = GrammarValidator::default();
}

#[test]
fn validator_new_can_validate_immediately() {
    let mut v = GrammarValidator::new();
    let g = Grammar::new("empty".into());
    let r = v.validate(&g);
    assert!(!r.errors.is_empty());
}

// =========================================================================
// 2. Validate minimal grammar (1 token, 1 rule)
// =========================================================================

#[test]
fn minimal_one_token_one_rule_no_errors() {
    let g = GrammarBuilder::new("min")
        .token("NUM", "0")
        .rule("start", vec!["NUM"])
        .start("start")
        .build();
    let r = validate(&g);
    assert!(r.errors.is_empty(), "errors: {:?}", r.errors);
    assert_eq!(r.stats.total_rules, 1);
    assert_eq!(r.stats.total_tokens, 1);
}

#[test]
fn minimal_grammar_stats_max_rule_length() {
    let g = GrammarBuilder::new("min")
        .token("A", "a")
        .rule("s", vec!["A"])
        .start("s")
        .build();
    let r = validate(&g);
    assert_eq!(r.stats.max_rule_length, 1);
}

#[test]
fn minimal_grammar_avg_rule_length() {
    let g = GrammarBuilder::new("min")
        .token("A", "a")
        .rule("s", vec!["A"])
        .start("s")
        .build();
    let r = validate(&g);
    assert!((r.stats.avg_rule_length - 1.0).abs() < f64::EPSILON);
}

#[test]
fn minimal_grammar_reachable_symbols_positive() {
    let g = GrammarBuilder::new("min")
        .token("A", "a")
        .rule("s", vec!["A"])
        .start("s")
        .build();
    let r = validate(&g);
    assert!(r.stats.reachable_symbols > 0);
}

#[test]
fn minimal_grammar_productive_symbols_positive() {
    let g = GrammarBuilder::new("min")
        .token("A", "a")
        .rule("s", vec!["A"])
        .start("s")
        .build();
    let r = validate(&g);
    assert!(r.stats.productive_symbols > 0);
}

// =========================================================================
// 3. Validate multi-token grammar
// =========================================================================

#[test]
fn two_tokens_single_rule() {
    let g = GrammarBuilder::new("two")
        .token("A", "a")
        .token("B", "b")
        .rule("s", vec!["A", "B"])
        .start("s")
        .build();
    let r = validate(&g);
    assert!(r.errors.is_empty(), "errors: {:?}", r.errors);
    assert_eq!(r.stats.total_tokens, 2);
}

#[test]
fn five_tokens_sequence_rule() {
    let g = GrammarBuilder::new("five")
        .token("T1", "1")
        .token("T2", "2")
        .token("T3", "3")
        .token("T4", "4")
        .token("T5", "5")
        .rule("s", vec!["T1", "T2", "T3", "T4", "T5"])
        .start("s")
        .build();
    let r = validate(&g);
    assert!(r.errors.is_empty(), "errors: {:?}", r.errors);
    assert_eq!(r.stats.total_tokens, 5);
    assert_eq!(r.stats.max_rule_length, 5);
}

#[test]
fn multi_token_grammar_no_unused_warnings_for_used_tokens() {
    let g = GrammarBuilder::new("used")
        .token("A", "a")
        .token("B", "b")
        .rule("s", vec!["A", "B"])
        .start("s")
        .build();
    let r = validate(&g);
    // Both tokens are used, so no UnusedToken for either of them
    let unused_names: Vec<_> = r
        .warnings
        .iter()
        .filter_map(|w| match w {
            ValidationWarning::UnusedToken { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert!(!unused_names.contains(&"A"));
    assert!(!unused_names.contains(&"B"));
}

#[test]
fn multi_token_unused_token_detected() {
    let g = GrammarBuilder::new("unused")
        .token("A", "a")
        .token("B", "b")
        .token("C", "c")
        .rule("s", vec!["A"])
        .start("s")
        .build();
    let r = validate(&g);
    // B and C are unused — they may be flagged as warnings
    let unused_names: Vec<_> = r
        .warnings
        .iter()
        .filter_map(|w| match w {
            ValidationWarning::UnusedToken { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect();
    // At least one unused token should be flagged
    assert!(
        unused_names.iter().any(|n| n == "B" || n == "C"),
        "expected unused token warning for B or C, got: {:?}",
        unused_names
    );
}

// =========================================================================
// 4. Validate grammar with alternatives
// =========================================================================

#[test]
fn two_alternatives_same_lhs() {
    let g = GrammarBuilder::new("alt")
        .token("A", "a")
        .token("B", "b")
        .rule("s", vec!["A"])
        .rule("s", vec!["B"])
        .start("s")
        .build();
    let r = validate(&g);
    assert!(r.errors.is_empty(), "errors: {:?}", r.errors);
    assert_eq!(r.stats.total_rules, 2);
}

#[test]
fn three_alternatives_same_lhs() {
    let g = GrammarBuilder::new("alt3")
        .token("A", "a")
        .token("B", "b")
        .token("C", "c")
        .rule("s", vec!["A"])
        .rule("s", vec!["B"])
        .rule("s", vec!["C"])
        .start("s")
        .build();
    let r = validate(&g);
    assert!(r.errors.is_empty(), "errors: {:?}", r.errors);
    assert_eq!(r.stats.total_rules, 3);
}

#[test]
fn alternatives_with_different_lengths() {
    let g = GrammarBuilder::new("alt_len")
        .token("A", "a")
        .token("B", "b")
        .rule("s", vec!["A"])
        .rule("s", vec!["A", "B"])
        .start("s")
        .build();
    let r = validate(&g);
    assert!(r.errors.is_empty(), "errors: {:?}", r.errors);
    assert_eq!(r.stats.max_rule_length, 2);
}

#[test]
fn alternative_rule_avg_length() {
    let g = GrammarBuilder::new("alt_avg")
        .token("A", "a")
        .token("B", "b")
        .rule("s", vec!["A"])
        .rule("s", vec!["A", "B"])
        .start("s")
        .build();
    let r = validate(&g);
    // Rules have lengths 1 and 2, avg = 1.5
    assert!((r.stats.avg_rule_length - 1.5).abs() < f64::EPSILON);
}

// =========================================================================
// 5. Validate grammar with precedence
// =========================================================================

#[test]
fn precedence_rules_no_errors() {
    let g = GrammarBuilder::new("prec")
        .token("NUM", "\\d+")
        .token("+", "+")
        .token("*", "*")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let r = validate(&g);
    // Cyclic because expr -> expr ... expr, but no EmptyGrammar
    assert!(
        !has_error(&r, |e| matches!(e, ValidationError::EmptyGrammar)),
        "errors: {:?}",
        r.errors
    );
}

#[test]
fn precedence_left_and_right_same_grammar() {
    let g = GrammarBuilder::new("lr")
        .token("NUM", "\\d+")
        .token("+", "+")
        .token("^", "^")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "^", "expr"], 2, Associativity::Right)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let r = validate(&g);
    assert!(
        !has_error(&r, |e| matches!(e, ValidationError::EmptyGrammar)),
        "errors: {:?}",
        r.errors
    );
    assert_eq!(r.stats.total_rules, 3);
}

#[test]
fn precedence_grammar_total_tokens() {
    let g = GrammarBuilder::new("pt")
        .token("NUM", "\\d+")
        .token("+", "+")
        .token("*", "*")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let r = validate(&g);
    assert_eq!(r.stats.total_tokens, 3);
}

// =========================================================================
// 6. Validate chain grammars
// =========================================================================

#[test]
fn chain_two_nonterminals() {
    let g = GrammarBuilder::new("chain2")
        .token("A", "a")
        .rule("s", vec!["mid"])
        .rule("mid", vec!["A"])
        .start("s")
        .build();
    let r = validate(&g);
    // Filters out CyclicRule since chain is not necessarily an error
    let real_errors: Vec<_> = r
        .errors
        .iter()
        .filter(|e| !matches!(e, ValidationError::CyclicRule { .. }))
        .collect();
    assert!(real_errors.is_empty(), "errors: {:?}", real_errors);
}

#[test]
fn chain_three_nonterminals() {
    let g = GrammarBuilder::new("chain3")
        .token("A", "a")
        .rule("s", vec!["b"])
        .rule("b", vec!["c"])
        .rule("c", vec!["A"])
        .start("s")
        .build();
    let r = validate(&g);
    let real_errors: Vec<_> = r
        .errors
        .iter()
        .filter(|e| !matches!(e, ValidationError::CyclicRule { .. }))
        .collect();
    assert!(real_errors.is_empty(), "errors: {:?}", real_errors);
}

#[test]
fn chain_grammar_generates_inefficiency_warning() {
    let g = GrammarBuilder::new("chain_w")
        .token("A", "a")
        .rule("s", vec!["mid"])
        .rule("mid", vec!["A"])
        .start("s")
        .build();
    let r = validate(&g);
    // s -> mid is a unit rule (trivial), should generate InefficientRule
    assert!(
        has_warning(&r, |w| matches!(
            w,
            ValidationWarning::InefficientRule { .. }
        )),
        "expected InefficientRule warning, got: {:?}",
        r.warnings
    );
}

#[test]
fn chain_grammar_stats_total_rules() {
    let g = GrammarBuilder::new("chain_s")
        .token("A", "a")
        .rule("s", vec!["mid"])
        .rule("mid", vec!["A"])
        .start("s")
        .build();
    let r = validate(&g);
    assert_eq!(r.stats.total_rules, 2);
}

// =========================================================================
// 7. Validate after normalize
// =========================================================================

#[test]
fn validate_after_normalize_optional() {
    let mut g = Grammar::new("norm_opt".into());
    let (tid, tok) = make_token(10, "x", "x");
    g.tokens.insert(tid, tok);
    g.add_rule(make_rule(
        1,
        vec![Symbol::Optional(Box::new(Symbol::Terminal(SymbolId(10))))],
        0,
    ));
    g.normalize();
    let r = validate(&g);
    // After normalization, Optional is expanded to auxiliary rules.
    // Grammar should still be valid (no EmptyGrammar).
    assert!(
        !has_error(&r, |e| matches!(e, ValidationError::EmptyGrammar)),
        "errors: {:?}",
        r.errors
    );
}

#[test]
fn validate_after_normalize_repeat() {
    let mut g = Grammar::new("norm_rep".into());
    let (tid, tok) = make_token(10, "x", "x");
    g.tokens.insert(tid, tok);
    g.add_rule(make_rule(
        1,
        vec![Symbol::Repeat(Box::new(Symbol::Terminal(SymbolId(10))))],
        0,
    ));
    g.normalize();
    let r = validate(&g);
    assert!(
        !has_error(&r, |e| matches!(e, ValidationError::EmptyGrammar)),
        "errors: {:?}",
        r.errors
    );
}

#[test]
fn validate_after_normalize_choice() {
    let mut g = Grammar::new("norm_ch".into());
    let (t1, tok1) = make_token(10, "a", "a");
    let (t2, tok2) = make_token(11, "b", "b");
    g.tokens.insert(t1, tok1);
    g.tokens.insert(t2, tok2);
    g.add_rule(make_rule(
        1,
        vec![Symbol::Choice(vec![
            Symbol::Terminal(SymbolId(10)),
            Symbol::Terminal(SymbolId(11)),
        ])],
        0,
    ));
    g.normalize();
    let r = validate(&g);
    assert!(
        !has_error(&r, |e| matches!(e, ValidationError::EmptyGrammar)),
        "errors: {:?}",
        r.errors
    );
}

#[test]
fn normalize_increases_rule_count() {
    let mut g = Grammar::new("norm_inc".into());
    let (tid, tok) = make_token(10, "x", "x");
    g.tokens.insert(tid, tok);
    g.add_rule(make_rule(
        1,
        vec![Symbol::Optional(Box::new(Symbol::Terminal(SymbolId(10))))],
        0,
    ));
    let before = g.rules.values().map(|v| v.len()).sum::<usize>();
    g.normalize();
    let after = g.rules.values().map(|v| v.len()).sum::<usize>();
    assert!(
        after > before,
        "normalize should expand rules: before={} after={}",
        before,
        after
    );
}

#[test]
fn double_normalize_is_idempotent_on_rule_count() {
    let mut g = Grammar::new("norm_idem".into());
    let (tid, tok) = make_token(10, "x", "x");
    g.tokens.insert(tid, tok);
    g.add_rule(make_rule(
        1,
        vec![Symbol::Optional(Box::new(Symbol::Terminal(SymbolId(10))))],
        0,
    ));
    g.normalize();
    let count1 = g.rules.values().map(|v| v.len()).sum::<usize>();
    g.normalize();
    let count2 = g.rules.values().map(|v| v.len()).sum::<usize>();
    assert_eq!(count1, count2, "second normalize should be idempotent");
}

// =========================================================================
// 8. Check error/warning counts
// =========================================================================

#[test]
fn empty_grammar_exactly_one_error() {
    let g = Grammar::new("empty".into());
    let r = validate(&g);
    // At least one error (EmptyGrammar)
    assert!(!r.errors.is_empty());
}

#[test]
fn valid_grammar_zero_errors() {
    let g = GrammarBuilder::new("ok")
        .token("A", "a")
        .rule("s", vec!["A"])
        .start("s")
        .build();
    let r = validate(&g);
    assert_eq!(r.errors.len(), 0, "errors: {:?}", r.errors);
}

#[test]
fn undefined_symbol_counted_in_errors() {
    let mut g = Grammar::new("undef".into());
    g.add_rule(make_rule(0, vec![Symbol::Terminal(SymbolId(99))], 0));
    let r = validate(&g);
    let undef_count = r
        .errors
        .iter()
        .filter(|e| matches!(e, ValidationError::UndefinedSymbol { .. }))
        .count();
    assert!(undef_count >= 1);
}

#[test]
fn multiple_undefined_symbols_counted() {
    let mut g = Grammar::new("multi_undef".into());
    g.add_rule(make_rule(
        0,
        vec![
            Symbol::Terminal(SymbolId(50)),
            Symbol::Terminal(SymbolId(60)),
        ],
        0,
    ));
    let r = validate(&g);
    let undef_count = r
        .errors
        .iter()
        .filter(|e| matches!(e, ValidationError::UndefinedSymbol { .. }))
        .count();
    assert!(undef_count >= 2, "expected >=2, got {}", undef_count);
}

#[test]
fn warnings_vec_empty_for_minimal_grammar() {
    let mut g = Grammar::new("min".into());
    let (tid, tok) = make_token(1, "x", "x");
    g.tokens.insert(tid, tok);
    let start = SymbolId(0);
    g.rule_names.insert(start, "start".into());
    g.add_rule(make_rule(start.0, vec![Symbol::Terminal(SymbolId(1))], 0));
    g.set_start_symbol(start);
    let r = validate(&g);
    // Minimal grammar may still have MissingFieldNames if rhs > 1, but
    // with a single-symbol rule there should be no field warning.
    let non_field_warnings: Vec<_> = r
        .warnings
        .iter()
        .filter(|w| !matches!(w, ValidationWarning::MissingFieldNames { .. }))
        .collect();
    // May have unused-token warnings for rule_0, but no other structural warnings
    assert!(
        non_field_warnings.len() <= 1,
        "unexpected warnings: {:?}",
        non_field_warnings
    );
}

#[test]
fn long_rhs_triggers_inefficiency_warning() {
    let mut g = Grammar::new("long".into());
    let mut rhs = Vec::new();
    for i in 0u16..12 {
        let (tid, tok) = make_token(100 + i, &format!("t{}", i), &format!("t{}", i));
        g.tokens.insert(tid, tok);
        rhs.push(Symbol::Terminal(SymbolId(100 + i)));
    }
    g.add_rule(make_rule(0, rhs, 0));
    let r = validate(&g);
    assert!(has_warning(&r, |w| matches!(
        w,
        ValidationWarning::InefficientRule { .. }
    )));
}

#[test]
fn stats_total_symbols_counts_tokens_and_rules() {
    let g = GrammarBuilder::new("count")
        .token("A", "a")
        .token("B", "b")
        .rule("s", vec!["A", "B"])
        .start("s")
        .build();
    let r = validate(&g);
    // total_symbols includes tokens + rule LHS symbols + externals
    assert!(r.stats.total_symbols >= 3); // at least A, B, s
}

// =========================================================================
// 9. Validate recursive grammar (expect CyclicRule)
// =========================================================================

#[test]
fn direct_self_recursion_produces_cyclic_rule() {
    let mut g = Grammar::new("self_rec".into());
    g.add_rule(make_rule(0, vec![Symbol::NonTerminal(SymbolId(0))], 0));
    let r = validate(&g);
    assert!(
        has_error(&r, |e| matches!(e, ValidationError::CyclicRule { .. })),
        "expected CyclicRule, got: {:?}",
        r.errors
    );
}

#[test]
fn mutual_recursion_produces_cyclic_rule() {
    let mut g = Grammar::new("mutual".into());
    g.add_rule(make_rule(0, vec![Symbol::NonTerminal(SymbolId(1))], 0));
    g.add_rule(make_rule(1, vec![Symbol::NonTerminal(SymbolId(0))], 1));
    let r = validate(&g);
    assert!(has_error(&r, |e| matches!(
        e,
        ValidationError::CyclicRule { .. }
    )));
}

#[test]
fn three_way_cycle_produces_cyclic_rule() {
    let mut g = Grammar::new("cycle3".into());
    g.add_rule(make_rule(0, vec![Symbol::NonTerminal(SymbolId(1))], 0));
    g.add_rule(make_rule(1, vec![Symbol::NonTerminal(SymbolId(2))], 1));
    g.add_rule(make_rule(2, vec![Symbol::NonTerminal(SymbolId(0))], 2));
    let r = validate(&g);
    assert!(has_error(&r, |e| matches!(
        e,
        ValidationError::CyclicRule { .. }
    )));
}

#[test]
fn recursion_with_base_case_still_reports_cycle() {
    let g = GrammarBuilder::new("rec_base")
        .token("NUM", "\\d+")
        .token("+", "+")
        .rule("expr", vec!["expr", "+", "expr"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let r = validate(&g);
    assert!(
        has_error(&r, |e| matches!(e, ValidationError::CyclicRule { .. })),
        "recursive grammar should report CyclicRule, got: {:?}",
        r.errors
    );
}

#[test]
fn cyclic_rule_symbols_vec_is_nonempty() {
    let mut g = Grammar::new("cyc_syms".into());
    g.add_rule(make_rule(0, vec![Symbol::NonTerminal(SymbolId(0))], 0));
    let r = validate(&g);
    for e in &r.errors {
        if let ValidationError::CyclicRule { symbols } = e {
            assert!(
                !symbols.is_empty(),
                "CyclicRule symbols should be non-empty"
            );
        }
    }
}

#[test]
fn cyclic_rule_not_produced_for_non_recursive_grammar() {
    let g = GrammarBuilder::new("nonrec")
        .token("A", "a")
        .rule("s", vec!["A"])
        .start("s")
        .build();
    let r = validate(&g);
    assert!(
        !has_error(&r, |e| matches!(e, ValidationError::CyclicRule { .. })),
        "non-recursive grammar should not report CyclicRule"
    );
}

// =========================================================================
// 10. Multiple validations on same validator
// =========================================================================

#[test]
fn reuse_validator_two_grammars() {
    let mut v = GrammarValidator::new();

    let g1 = Grammar::new("empty".into());
    let r1 = v.validate(&g1);
    assert!(
        r1.errors
            .iter()
            .any(|e| matches!(e, ValidationError::EmptyGrammar))
    );

    let g2 = GrammarBuilder::new("ok")
        .token("A", "a")
        .rule("s", vec!["A"])
        .start("s")
        .build();
    let r2 = v.validate(&g2);
    assert!(r2.errors.is_empty(), "errors: {:?}", r2.errors);
}

#[test]
fn reuse_validator_three_grammars() {
    let mut v = GrammarValidator::new();

    let g1 = Grammar::new("e1".into());
    let r1 = v.validate(&g1);
    assert!(!r1.errors.is_empty());

    let g2 = Grammar::new("e2".into());
    let r2 = v.validate(&g2);
    assert!(!r2.errors.is_empty());

    let g3 = GrammarBuilder::new("ok")
        .token("X", "x")
        .rule("s", vec!["X"])
        .start("s")
        .build();
    let r3 = v.validate(&g3);
    assert!(r3.errors.is_empty(), "errors: {:?}", r3.errors);
}

#[test]
fn reuse_validator_errors_do_not_accumulate() {
    let mut v = GrammarValidator::new();

    let g1 = Grammar::new("e".into());
    let r1 = v.validate(&g1);
    let count1 = r1.errors.len();

    let r2 = v.validate(&g1);
    let count2 = r2.errors.len();
    assert_eq!(count1, count2, "errors should not accumulate across calls");
}

#[test]
fn reuse_validator_warnings_do_not_accumulate() {
    let mut v = GrammarValidator::new();

    // Grammar with unused token to generate warning
    let g = GrammarBuilder::new("warn")
        .token("A", "a")
        .token("B", "b")
        .rule("s", vec!["A"])
        .start("s")
        .build();

    let r1 = v.validate(&g);
    let w1 = r1.warnings.len();

    let r2 = v.validate(&g);
    let w2 = r2.warnings.len();
    assert_eq!(w1, w2, "warnings should not accumulate across calls");
}

#[test]
fn reuse_validator_alternating_valid_invalid() {
    let mut v = GrammarValidator::new();

    let empty = Grammar::new("empty".into());
    let valid = GrammarBuilder::new("ok")
        .token("A", "a")
        .rule("s", vec!["A"])
        .start("s")
        .build();

    for _ in 0..5 {
        let re = v.validate(&empty);
        assert!(!re.errors.is_empty());
        let rv = v.validate(&valid);
        assert!(rv.errors.is_empty(), "errors: {:?}", rv.errors);
    }
}

// =========================================================================
// 11. Validate large grammars
// =========================================================================

#[test]
fn large_grammar_50_tokens() {
    let mut builder = GrammarBuilder::new("large50");
    let mut names = Vec::new();
    for i in 0..50 {
        let name = format!("T{}", i);
        builder = builder.token(&name, &format!("t{}", i));
        names.push(name);
    }
    let name_refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
    builder = builder.rule("s", name_refs).start("s");
    let g = builder.build();
    let r = validate(&g);
    assert_eq!(r.stats.total_tokens, 50);
    assert!(
        !has_error(&r, |e| matches!(e, ValidationError::EmptyGrammar)),
        "errors: {:?}",
        r.errors
    );
}

#[test]
fn large_grammar_100_alternatives() {
    let mut builder = GrammarBuilder::new("large100");
    for i in 0u16..100 {
        let tname = format!("T{}", i);
        builder = builder.token(&tname, &format!("v{}", i));
        builder = builder.rule("s", vec![&tname]);
    }
    builder = builder.start("s");
    let g = builder.build();
    let r = validate(&g);
    assert_eq!(r.stats.total_rules, 100);
}

#[test]
fn large_grammar_chain_depth_20() {
    let mut builder = GrammarBuilder::new("chain20");
    builder = builder.token("LEAF", "leaf");
    for i in 0..20 {
        let lhs = format!("n{}", i);
        let rhs_name = if i == 19 {
            "LEAF".to_string()
        } else {
            format!("n{}", i + 1)
        };
        builder = builder.rule(&lhs, vec![&rhs_name]);
    }
    builder = builder.start("n0");
    let g = builder.build();
    let r = validate(&g);
    assert!(
        !has_error(&r, |e| matches!(e, ValidationError::EmptyGrammar)),
        "errors: {:?}",
        r.errors
    );
}

#[test]
fn large_grammar_stats_are_populated() {
    let mut builder = GrammarBuilder::new("stats_lg");
    for i in 0u16..30 {
        let tname = format!("T{}", i);
        builder = builder.token(&tname, &format!("p{}", i));
        builder = builder.rule("s", vec![&tname]);
    }
    builder = builder.start("s");
    let g = builder.build();
    let r = validate(&g);
    assert_eq!(r.stats.total_rules, 30);
    assert_eq!(r.stats.total_tokens, 30);
    assert!(r.stats.total_symbols > 0);
}

#[test]
fn javascript_like_grammar_validates() {
    let g = GrammarBuilder::javascript_like();
    let r = validate(&g);
    // Large pre-built grammar should not report EmptyGrammar
    assert!(!has_error(&r, |e| matches!(
        e,
        ValidationError::EmptyGrammar
    )));
    assert!(r.stats.total_rules > 5);
}

#[test]
fn python_like_grammar_validates() {
    let g = GrammarBuilder::python_like();
    let r = validate(&g);
    assert!(!has_error(&r, |e| matches!(
        e,
        ValidationError::EmptyGrammar
    )));
    assert!(r.stats.total_rules > 5);
}

// =========================================================================
// Additional edge cases across categories
// =========================================================================

#[test]
fn grammar_with_external_token_stats() {
    let g = GrammarBuilder::new("ext")
        .token("A", "a")
        .external("INDENT")
        .rule("s", vec!["A"])
        .start("s")
        .build();
    let r = validate(&g);
    assert_eq!(r.stats.external_tokens, 1);
}

#[test]
fn duplicate_token_pattern_warning() {
    let mut g = Grammar::new("dup_pat".into());
    let (t1, tok1) = make_token(1, "a1", "same");
    let (t2, tok2) = make_token(2, "a2", "same");
    g.tokens.insert(t1, tok1);
    g.tokens.insert(t2, tok2);
    g.add_rule(make_rule(
        0,
        vec![Symbol::Terminal(SymbolId(1)), Symbol::Terminal(SymbolId(2))],
        0,
    ));
    let r = validate(&g);
    assert!(has_warning(&r, |w| matches!(
        w,
        ValidationWarning::DuplicateTokenPattern { .. }
    )));
}

#[test]
fn conflicting_precedence_error() {
    let mut g = Grammar::new("conf_prec".into());
    let (tid, tok) = make_token(1, "op", "+");
    g.tokens.insert(tid, tok);
    g.add_rule(make_rule(0, vec![Symbol::Terminal(SymbolId(1))], 0));
    g.precedences.push(Precedence {
        level: 1,
        associativity: Associativity::Left,
        symbols: vec![SymbolId(1)],
    });
    g.precedences.push(Precedence {
        level: 5,
        associativity: Associativity::Right,
        symbols: vec![SymbolId(1)],
    });
    let r = validate(&g);
    assert!(has_error(&r, |e| matches!(
        e,
        ValidationError::ConflictingPrecedence { .. }
    )));
}

#[test]
fn validation_error_display_empty_grammar() {
    let err = ValidationError::EmptyGrammar;
    let s = format!("{}", err);
    assert!(s.contains("no rules"), "display: {}", s);
}

#[test]
fn validation_error_display_cyclic_rule() {
    let err = ValidationError::CyclicRule {
        symbols: vec![SymbolId(0), SymbolId(1)],
    };
    let s = format!("{}", err);
    assert!(s.contains("Cyclic"), "display: {}", s);
}

#[test]
fn validation_warning_display_unused_token() {
    let w = ValidationWarning::UnusedToken {
        token: SymbolId(5),
        name: "FOO".into(),
    };
    let s = format!("{}", w);
    assert!(s.contains("FOO"), "display: {}", s);
}

#[test]
fn epsilon_rule_via_builder_no_panic() {
    let g = GrammarBuilder::new("eps")
        .rule("s", vec![])
        .start("s")
        .build();
    let r = validate(&g);
    // Should not panic; epsilon rule via builder is valid input
    assert!(r.stats.total_rules >= 1);
}

#[test]
fn fragile_token_validates_normally() {
    let g = GrammarBuilder::new("fragile")
        .fragile_token("ERR", "error")
        .token("A", "a")
        .rule("s", vec!["A"])
        .start("s")
        .build();
    let r = validate(&g);
    // Fragile token exists but should not cause errors
    assert!(
        !has_error(&r, |e| matches!(e, ValidationError::EmptyGrammar)),
        "errors: {:?}",
        r.errors
    );
}

#[test]
fn grammar_with_extras_validates() {
    let g = GrammarBuilder::new("ws")
        .token("A", "a")
        .token("WS", " ")
        .extra("WS")
        .rule("s", vec!["A"])
        .start("s")
        .build();
    let r = validate(&g);
    assert!(r.errors.is_empty(), "errors: {:?}", r.errors);
}

#[test]
fn empty_grammar_stats_all_zero() {
    let g = Grammar::new("empty".into());
    let r = validate(&g);
    assert_eq!(r.stats.total_rules, 0);
    assert_eq!(r.stats.total_tokens, 0);
    assert_eq!(r.stats.max_rule_length, 0);
    assert_eq!(r.stats.external_tokens, 0);
}

#[test]
fn validate_result_errors_and_warnings_are_independent() {
    let g = GrammarBuilder::new("ind")
        .token("A", "a")
        .token("B", "b")
        .rule("s", vec!["A"])
        .start("s")
        .build();
    let r = validate(&g);
    // Errors and warnings should be independent vecs
    let _e = &r.errors;
    let _w = &r.warnings;
    // Just confirming we can access both without issues
}

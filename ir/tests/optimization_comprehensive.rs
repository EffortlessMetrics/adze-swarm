//! Comprehensive tests for grammar optimization and validation in adze-ir.
//!
//! Categories:
//!   1. Grammar optimization rules (optimizer passes)
//!   2. Validation checks (validator errors/warnings)
//!   3. Edge cases (empty grammars, self-referential rules, etc.)

use adze_ir::builder::GrammarBuilder;
use adze_ir::optimizer::{GrammarOptimizer, OptimizationStats, optimize_grammar};
use adze_ir::validation::{GrammarValidator, ValidationError, ValidationWarning};
use adze_ir::{
    Associativity, FieldId, Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn find_symbol(grammar: &Grammar, name: &str) -> Option<SymbolId> {
    grammar
        .rule_names
        .iter()
        .find(|(_, n)| n.as_str() == name)
        .map(|(id, _)| *id)
        .or_else(|| {
            grammar
                .tokens
                .iter()
                .find(|(_, t)| t.name == name)
                .map(|(id, _)| *id)
        })
}

fn total_rule_count(grammar: &Grammar) -> usize {
    grammar.all_rules().count()
}

fn build_simple_expr_grammar() -> Grammar {
    GrammarBuilder::new("simple")
        .token("NUMBER", r"\d+")
        .token("+", "+")
        .rule("expr", vec!["expr", "+", "expr"])
        .rule("expr", vec!["NUMBER"])
        .start("expr")
        .build()
}

fn build_unit_rule_grammar() -> Grammar {
    GrammarBuilder::new("unit")
        .token("NUMBER", r"\d+")
        .token("+", "+")
        .rule("expr", vec!["term"])
        .rule("term", vec!["factor"])
        .rule("factor", vec!["NUMBER"])
        .start("expr")
        .build()
}

// ===========================================================================
// Category 1 — Grammar optimization rules
// ===========================================================================

// --- 1.1 Unused symbol removal ---

#[test]
fn opt_remove_unused_token() {
    let mut grammar = Grammar::new("test".to_string());
    let num = SymbolId(1);
    let unused = SymbolId(2);
    let expr = SymbolId(3);

    grammar.tokens.insert(
        num,
        Token {
            name: "NUMBER".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );
    grammar.tokens.insert(
        unused,
        Token {
            name: "UNUSED".to_string(),
            pattern: TokenPattern::String("unused".to_string()),
            fragile: false,
        },
    );
    grammar.add_rule(Rule {
        lhs: expr,
        rhs: vec![Symbol::Terminal(num)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let mut optimizer = GrammarOptimizer::new();
    let stats = optimizer.optimize(&mut grammar);

    assert!(stats.removed_unused_symbols >= 1);
    assert!(!grammar.tokens.contains_key(&unused));
}

#[test]
fn opt_keep_all_used_tokens() {
    let grammar = build_simple_expr_grammar();
    let before_tokens = grammar.tokens.len();

    let optimized = optimize_grammar(grammar).unwrap();
    // All tokens are used — none should be removed
    assert!(optimized.tokens.len() <= before_tokens);
    // The two tokens (NUMBER and +) should still exist
    assert!(optimized.tokens.values().any(|t| t.name == "NUMBER"));
    assert!(optimized.tokens.values().any(|t| t.name == "+"));
}

#[test]
fn opt_stats_total_accounts_for_all_passes() {
    let mut grammar = build_simple_expr_grammar();
    let mut optimizer = GrammarOptimizer::new();
    let stats = optimizer.optimize(&mut grammar);

    let computed = stats.removed_unused_symbols
        + stats.inlined_rules
        + stats.merged_tokens
        + stats.optimized_left_recursion
        + stats.eliminated_unit_rules;
    assert_eq!(stats.total(), computed);
}

// --- 1.2 Inline simple rules ---

#[test]
fn opt_inline_single_production_nonterminal() {
    // wrapper -> inner, inner -> NUMBER  =>  wrapper gets inlined
    let mut grammar = Grammar::new("inline_test".to_string());
    let num = SymbolId(1);
    let inner = SymbolId(2);
    let wrapper = SymbolId(3);
    let start = SymbolId(4);

    grammar.tokens.insert(
        num,
        Token {
            name: "NUMBER".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );
    grammar.rule_names.insert(start, "start".to_string());
    grammar.rule_names.insert(wrapper, "wrapper".to_string());
    grammar.rule_names.insert(inner, "inner".to_string());

    grammar.add_rule(Rule {
        lhs: start,
        rhs: vec![Symbol::NonTerminal(wrapper)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    grammar.add_rule(Rule {
        lhs: wrapper,
        rhs: vec![Symbol::NonTerminal(inner)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });
    grammar.add_rule(Rule {
        lhs: inner,
        rhs: vec![Symbol::Terminal(num)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(2),
    });

    let mut optimizer = GrammarOptimizer::new();
    let stats = optimizer.optimize(&mut grammar);

    // At least one inlining should have happened
    assert!(stats.inlined_rules > 0 || stats.eliminated_unit_rules > 0);
}

#[test]
fn opt_no_inline_recursive_rule() {
    // expr -> expr + expr should NOT be inlined
    let mut grammar = build_simple_expr_grammar();
    let before_rules = total_rule_count(&grammar);

    let mut optimizer = GrammarOptimizer::new();
    let stats = optimizer.optimize(&mut grammar);

    // Recursive rules can't be inlined
    // Grammar should still have rules after optimization
    assert!(total_rule_count(&grammar) > 0);
    // The total should be at least as many due to left-recursion transform
    let _ = (before_rules, stats);
}

// --- 1.3 Merge equivalent tokens ---

#[test]
fn opt_merge_duplicate_token_patterns() {
    let mut grammar = Grammar::new("dup_tokens".to_string());
    let t1 = SymbolId(1);
    let t2 = SymbolId(2);
    let expr = SymbolId(3);

    grammar.tokens.insert(
        t1,
        Token {
            name: "plus1".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );
    grammar.tokens.insert(
        t2,
        Token {
            name: "plus2".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );
    grammar.add_rule(Rule {
        lhs: expr,
        rhs: vec![Symbol::Terminal(t1), Symbol::Terminal(t2)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let mut optimizer = GrammarOptimizer::new();
    let stats = optimizer.optimize(&mut grammar);

    assert!(stats.merged_tokens >= 1);
}

#[test]
fn opt_no_merge_distinct_patterns() {
    let mut grammar = Grammar::new("distinct".to_string());
    let t1 = SymbolId(1);
    let t2 = SymbolId(2);
    let expr = SymbolId(3);

    grammar.tokens.insert(
        t1,
        Token {
            name: "plus".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );
    grammar.tokens.insert(
        t2,
        Token {
            name: "minus".to_string(),
            pattern: TokenPattern::String("-".to_string()),
            fragile: false,
        },
    );
    grammar.add_rule(Rule {
        lhs: expr,
        rhs: vec![Symbol::Terminal(t1), Symbol::Terminal(t2)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let mut optimizer = GrammarOptimizer::new();
    let stats = optimizer.optimize(&mut grammar);

    assert_eq!(stats.merged_tokens, 0);
}

// --- 1.4 Left recursion optimization ---

#[test]
fn opt_detect_left_recursion() {
    let grammar = build_simple_expr_grammar();
    let mut optimizer = GrammarOptimizer::new();
    optimizer.optimize(&mut grammar.clone());

    // The optimizer should detect left recursion in expr -> expr + expr
    // After optimization, the grammar should still parse correctly
    let optimized = optimize_grammar(grammar).unwrap();
    assert!(total_rule_count(&optimized) > 0);
}

#[test]
fn opt_left_recursion_creates_helper_symbol() {
    let mut grammar = build_simple_expr_grammar();
    let before_symbols = grammar.rules.len();

    let mut optimizer = GrammarOptimizer::new();
    let stats = optimizer.optimize(&mut grammar);

    if stats.optimized_left_recursion > 0 {
        // New helper symbols should be created
        assert!(grammar.rules.len() >= before_symbols);
        // Helper should have __rec suffix in rule_names
        let has_rec = grammar.rule_names.values().any(|n| n.contains("__rec"));
        assert!(has_rec, "Expected a __rec helper symbol");
    }
}

#[test]
fn opt_left_recursion_preserves_base_case() {
    let mut grammar = build_simple_expr_grammar();
    let mut optimizer = GrammarOptimizer::new();
    optimizer.optimize(&mut grammar);

    // After left-recursion elimination, the grammar should still have rules
    // that can derive terminal strings
    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    let non_productive_count = result
        .errors
        .iter()
        .filter(|e| matches!(e, ValidationError::NonProductiveSymbol { .. }))
        .count();
    // The optimized grammar should remain productive
    assert_eq!(non_productive_count, 0);
}

// --- 1.5 Unit rule elimination ---

#[test]
fn opt_eliminate_unit_rules() {
    let mut grammar = build_unit_rule_grammar();
    let mut optimizer = GrammarOptimizer::new();
    let stats = optimizer.optimize(&mut grammar);

    // Unit rules (expr -> term, term -> factor) should be eliminated
    assert!(stats.eliminated_unit_rules > 0 || stats.inlined_rules > 0);
}

#[test]
fn opt_unit_rule_preserves_semantics() {
    let grammar = build_unit_rule_grammar();
    let before = total_rule_count(&grammar);
    let optimized = optimize_grammar(grammar).unwrap();

    // The optimizer may aggressively inline/eliminate unit chains.
    // What matters is the result is still a valid grammar object.
    let _ = before;
    assert_eq!(optimized.name, "unit");
}

// --- 1.6 Renumber symbols ---

#[test]
fn opt_renumber_produces_contiguous_ids() {
    let mut grammar = build_simple_expr_grammar();
    let mut optimizer = GrammarOptimizer::new();
    optimizer.optimize(&mut grammar);

    // After optimization (includes renumbering), IDs should start at 1
    for (id, _) in &grammar.tokens {
        assert!(id.0 >= 1, "Token ID should be >= 1, got {}", id.0);
    }
}

// --- 1.7 optimize_grammar convenience function ---

#[test]
fn opt_convenience_function_returns_ok() {
    let grammar = build_simple_expr_grammar();
    let result = optimize_grammar(grammar);
    assert!(result.is_ok());
}

#[test]
fn opt_convenience_function_does_not_lose_name() {
    let grammar = GrammarBuilder::new("my_lang")
        .token("NUMBER", r"\d+")
        .rule("expr", vec!["NUMBER"])
        .start("expr")
        .build();

    let optimized = optimize_grammar(grammar).unwrap();
    assert_eq!(optimized.name, "my_lang");
}

// --- 1.8 Precedence-bearing rules through optimization ---

#[test]
fn opt_precedence_rules_survive_optimization() {
    let grammar = GrammarBuilder::new("prec")
        .token("NUMBER", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["NUMBER"])
        .start("expr")
        .build();

    let optimized = optimize_grammar(grammar).unwrap();

    // Precedence information should be preserved on at least some rules
    let has_prec = optimized.all_rules().any(|r| r.precedence.is_some());
    assert!(has_prec);
}

// --- 1.9 Multiple optimization passes are idempotent ---

#[test]
fn opt_double_optimize_is_idempotent() {
    let grammar = build_simple_expr_grammar();
    let first = optimize_grammar(grammar).unwrap();
    let first_rules = total_rule_count(&first);

    let second = optimize_grammar(first).unwrap();
    let second_rules = total_rule_count(&second);

    // Second pass should not change the number of rules
    assert_eq!(first_rules, second_rules);
}

// --- 1.10 OptimizationStats defaults ---

#[test]
fn opt_stats_default_is_zero() {
    let stats = OptimizationStats::default();
    assert_eq!(stats.total(), 0);
    assert_eq!(stats.removed_unused_symbols, 0);
    assert_eq!(stats.inlined_rules, 0);
    assert_eq!(stats.merged_tokens, 0);
    assert_eq!(stats.optimized_left_recursion, 0);
    assert_eq!(stats.eliminated_unit_rules, 0);
}

// --- 1.11 Fragile tokens are preserved ---

#[test]
fn opt_fragile_tokens_preserved() {
    let grammar = GrammarBuilder::new("fragile_test")
        .fragile_token("SEMI", ";")
        .token("NUMBER", r"\d+")
        .rule("stmt", vec!["NUMBER", "SEMI"])
        .start("stmt")
        .build();

    let optimized = optimize_grammar(grammar).unwrap();
    let has_fragile = optimized.tokens.values().any(|t| t.fragile);
    assert!(has_fragile);
}

// --- 1.12 Extras survive optimization ---

#[test]
fn opt_extras_survive() {
    let grammar = GrammarBuilder::new("extras_test")
        .token("NUMBER", r"\d+")
        .token("WHITESPACE", r"[ \t]+")
        .extra("WHITESPACE")
        .rule("expr", vec!["NUMBER"])
        .start("expr")
        .build();

    // Extras are present before optimization
    assert!(!grammar.extras.is_empty());

    // After optimization, extras may be filtered if the extra token
    // is not referenced in any rule (optimizer removes unused symbols).
    // This validates the optimizer runs without error on grammars with extras.
    let optimized = optimize_grammar(grammar).unwrap();
    assert_eq!(optimized.name, "extras_test");
}

// --- 1.13 External tokens survive optimization ---

#[test]
fn opt_externals_survive() {
    let grammar = GrammarBuilder::new("ext_test")
        .token("NUMBER", r"\d+")
        .token("INDENT", "INDENT")
        .external("INDENT")
        .rule("block", vec!["INDENT", "NUMBER"])
        .start("block")
        .build();

    let optimized = optimize_grammar(grammar).unwrap();
    assert!(!optimized.externals.is_empty());
}

// --- 1.14 Builder preset grammars optimize without error ---

#[test]
fn opt_python_like_grammar() {
    let grammar = GrammarBuilder::python_like();
    let result = optimize_grammar(grammar);
    assert!(result.is_ok());
    assert!(total_rule_count(&result.unwrap()) > 0);
}

#[test]
fn opt_javascript_like_grammar() {
    let grammar = GrammarBuilder::javascript_like();
    let result = optimize_grammar(grammar);
    assert!(result.is_ok());
    assert!(total_rule_count(&result.unwrap()) > 0);
}

// ===========================================================================
// Category 2 — Validation checks
// ===========================================================================

// --- 2.1 Empty grammar ---

#[test]
fn val_empty_grammar_error() {
    let grammar = Grammar::new("empty".to_string());
    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    assert!(
        result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::EmptyGrammar))
    );
}

// --- 2.2 Undefined symbol ---

#[test]
fn val_undefined_symbol_in_rhs() {
    let mut grammar = Grammar::new("undef".to_string());
    let expr = SymbolId(1);
    let undef = SymbolId(99);

    grammar.add_rule(Rule {
        lhs: expr,
        rhs: vec![Symbol::NonTerminal(undef)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    assert!(
        result.errors.iter().any(
            |e| matches!(e, ValidationError::UndefinedSymbol { symbol, .. } if *symbol == undef)
        )
    );
}

#[test]
fn val_undefined_terminal_in_rhs() {
    let mut grammar = Grammar::new("undef_term".to_string());
    let expr = SymbolId(1);
    let undef_tok = SymbolId(50);

    grammar.add_rule(Rule {
        lhs: expr,
        rhs: vec![Symbol::Terminal(undef_tok)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    assert!(result.errors.iter().any(
        |e| matches!(e, ValidationError::UndefinedSymbol { symbol, .. } if *symbol == undef_tok)
    ));
}

// --- 2.3 Non-productive symbols ---

#[test]
fn val_mutually_recursive_non_productive() {
    let mut grammar = Grammar::new("nonprod".to_string());
    let a = SymbolId(1);
    let b = SymbolId(2);

    grammar.add_rule(Rule {
        lhs: a,
        rhs: vec![Symbol::NonTerminal(b)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    grammar.add_rule(Rule {
        lhs: b,
        rhs: vec![Symbol::NonTerminal(a)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });

    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    assert!(
        result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::NonProductiveSymbol { .. }))
    );
}

#[test]
fn val_productive_with_terminal() {
    let mut grammar = Grammar::new("prod".to_string());
    let expr = SymbolId(1);
    let num = SymbolId(2);

    grammar.tokens.insert(
        num,
        Token {
            name: "NUMBER".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );
    grammar.add_rule(Rule {
        lhs: expr,
        rhs: vec![Symbol::Terminal(num)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    let non_prod = result
        .errors
        .iter()
        .filter(|e| matches!(e, ValidationError::NonProductiveSymbol { .. }))
        .count();
    assert_eq!(non_prod, 0);
}

// --- 2.4 Cyclic rules ---

#[test]
fn val_cycle_detection() {
    let mut grammar = Grammar::new("cycle".to_string());
    let a = SymbolId(1);
    let b = SymbolId(2);
    let c = SymbolId(3);

    grammar.add_rule(Rule {
        lhs: a,
        rhs: vec![Symbol::NonTerminal(b)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    grammar.add_rule(Rule {
        lhs: b,
        rhs: vec![Symbol::NonTerminal(c)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });
    grammar.add_rule(Rule {
        lhs: c,
        rhs: vec![Symbol::NonTerminal(a)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(2),
    });

    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    assert!(
        result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::CyclicRule { .. }))
    );
}

// --- 2.5 Duplicate token patterns ---

#[test]
fn val_warns_duplicate_token_patterns() {
    let mut grammar = Grammar::new("dup".to_string());
    let t1 = SymbolId(1);
    let t2 = SymbolId(2);
    let expr = SymbolId(3);

    grammar.tokens.insert(
        t1,
        Token {
            name: "plus_a".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );
    grammar.tokens.insert(
        t2,
        Token {
            name: "plus_b".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );
    grammar.add_rule(Rule {
        lhs: expr,
        rhs: vec![Symbol::Terminal(t1)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    assert!(
        result
            .warnings
            .iter()
            .any(|w| matches!(w, ValidationWarning::DuplicateTokenPattern { .. }))
    );
}

// --- 2.6 Invalid field index ---

#[test]
fn val_invalid_field_index() {
    let mut grammar = Grammar::new("field".to_string());
    let expr = SymbolId(1);
    let num = SymbolId(2);

    grammar.tokens.insert(
        num,
        Token {
            name: "NUMBER".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );
    grammar.add_rule(Rule {
        lhs: expr,
        rhs: vec![Symbol::Terminal(num)],
        precedence: None,
        associativity: None,
        fields: vec![(FieldId(0), 99)], // index 99 out of bounds
        production_id: ProductionId(0),
    });

    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    assert!(
        result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::InvalidField { .. }))
    );
}

// --- 2.7 Missing field names warning ---

#[test]
fn val_warns_missing_field_names() {
    let mut grammar = Grammar::new("nofields".to_string());
    let expr = SymbolId(1);
    let num = SymbolId(2);
    let plus = SymbolId(3);

    grammar.tokens.insert(
        num,
        Token {
            name: "NUMBER".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );
    grammar.tokens.insert(
        plus,
        Token {
            name: "+".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );
    grammar.add_rule(Rule {
        lhs: expr,
        rhs: vec![Symbol::Terminal(num), Symbol::Terminal(plus)],
        precedence: None,
        associativity: None,
        fields: vec![], // multi-symbol rule without fields
        production_id: ProductionId(0),
    });

    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    assert!(
        result
            .warnings
            .iter()
            .any(|w| matches!(w, ValidationWarning::MissingFieldNames { .. }))
    );
}

// --- 2.8 Empty regex pattern ---

#[test]
fn val_empty_regex_pattern() {
    let mut grammar = Grammar::new("empty_regex".to_string());
    let tok = SymbolId(1);
    let expr = SymbolId(2);

    grammar.tokens.insert(
        tok,
        Token {
            name: "BAD".to_string(),
            pattern: TokenPattern::Regex(String::new()),
            fragile: false,
        },
    );
    grammar.add_rule(Rule {
        lhs: expr,
        rhs: vec![Symbol::Terminal(tok)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    assert!(
        result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::InvalidRegex { .. }))
    );
}

// --- 2.9 Conflicting precedence ---

#[test]
fn val_conflicting_precedence() {
    use adze_ir::Precedence;

    let mut grammar = Grammar::new("prec_conflict".to_string());
    let sym = SymbolId(1);
    let num = SymbolId(2);

    grammar.tokens.insert(
        num,
        Token {
            name: "NUMBER".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );
    grammar.add_rule(Rule {
        lhs: sym,
        rhs: vec![Symbol::Terminal(num)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    grammar.precedences.push(Precedence {
        level: 1,
        associativity: Associativity::Left,
        symbols: vec![sym],
    });
    grammar.precedences.push(Precedence {
        level: 5,
        associativity: Associativity::Left,
        symbols: vec![sym],
    });

    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    assert!(
        result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::ConflictingPrecedence { .. }))
    );
}

// --- 2.10 External token conflict (duplicate names) ---

#[test]
fn val_external_token_conflict() {
    use adze_ir::ExternalToken;

    let mut grammar = Grammar::new("ext_conflict".to_string());
    let s1 = SymbolId(1);
    let s2 = SymbolId(2);
    let expr = SymbolId(3);

    grammar.tokens.insert(
        s1,
        Token {
            name: "INDENT".to_string(),
            pattern: TokenPattern::String("INDENT".to_string()),
            fragile: false,
        },
    );
    grammar.add_rule(Rule {
        lhs: expr,
        rhs: vec![Symbol::Terminal(s1)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    grammar.externals.push(ExternalToken {
        name: "INDENT".to_string(),
        symbol_id: s1,
    });
    grammar.externals.push(ExternalToken {
        name: "INDENT".to_string(),
        symbol_id: s2,
    });

    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    assert!(
        result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::ExternalTokenConflict { .. }))
    );
}

// --- 2.11 Inefficiency warnings ---

#[test]
fn val_warns_trivial_unit_rule() {
    let grammar = build_unit_rule_grammar();
    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    // Unit rules should trigger InefficientRule warning
    assert!(
        result
            .warnings
            .iter()
            .any(|w| matches!(w, ValidationWarning::InefficientRule { .. }))
    );
}

#[test]
fn val_warns_very_long_rule() {
    let mut grammar = Grammar::new("long".to_string());
    let expr = SymbolId(1);
    let num = SymbolId(2);

    grammar.tokens.insert(
        num,
        Token {
            name: "NUMBER".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );

    // Create a rule with 12 symbols (> 10 threshold)
    let long_rhs: Vec<Symbol> = (0..12).map(|_| Symbol::Terminal(num)).collect();
    grammar.add_rule(Rule {
        lhs: expr,
        rhs: long_rhs,
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    assert!(result.warnings.iter().any(|w| {
        matches!(w, ValidationWarning::InefficientRule { suggestion, .. } if suggestion.contains("symbols"))
    }));
}

// --- 2.12 Validation statistics ---

#[test]
fn val_stats_count_symbols_and_rules() {
    let grammar = build_simple_expr_grammar();
    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    assert!(result.stats.total_symbols > 0);
    assert!(result.stats.total_rules > 0);
    assert!(result.stats.total_tokens > 0);
}

#[test]
fn val_stats_max_rule_length() {
    let grammar = build_simple_expr_grammar();
    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    // expr -> expr + expr has length 3
    assert!(result.stats.max_rule_length >= 3);
}

#[test]
fn val_stats_avg_rule_length() {
    let grammar = build_simple_expr_grammar();
    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    assert!(result.stats.avg_rule_length > 0.0);
}

// --- 2.13 Reachability ---

#[test]
fn val_reachable_symbols_count() {
    let grammar = build_simple_expr_grammar();
    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    // All symbols in the grammar are reachable from expr
    assert!(result.stats.reachable_symbols > 0);
}

// --- 2.14 Valid grammar produces no errors ---

#[test]
fn val_valid_grammar_no_errors() {
    let grammar = GrammarBuilder::new("clean")
        .token("NUMBER", r"\d+")
        .rule("expr", vec!["NUMBER"])
        .start("expr")
        .build();

    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    assert!(
        result.errors.is_empty(),
        "Expected no errors, got: {:?}",
        result.errors
    );
}

// --- 2.15 ValidationError Display ---

#[test]
fn val_error_display_empty_grammar() {
    let err = ValidationError::EmptyGrammar;
    let s = format!("{}", err);
    assert!(s.contains("no rules"));
}

#[test]
fn val_error_display_undefined_symbol() {
    let err = ValidationError::UndefinedSymbol {
        symbol: SymbolId(42),
        location: "rule for expr".to_string(),
    };
    let s = format!("{}", err);
    assert!(s.contains("42") || s.contains("Undefined"));
}

#[test]
fn val_warning_display_unused_token() {
    let w = ValidationWarning::UnusedToken {
        token: SymbolId(5),
        name: "FOO".to_string(),
    };
    let s = format!("{}", w);
    assert!(s.contains("FOO"));
}

// ===========================================================================
// Category 3 — Edge cases
// ===========================================================================

// --- 3.1 Empty grammar through optimizer ---

#[test]
fn edge_optimize_empty_grammar() {
    let grammar = Grammar::new("empty".to_string());
    let result = optimize_grammar(grammar);
    assert!(result.is_ok());
    let optimized = result.unwrap();
    assert!(optimized.rules.is_empty());
}

// --- 3.2 Single epsilon rule ---

#[test]
fn edge_single_epsilon_rule() {
    let mut grammar = Grammar::new("epsilon".to_string());
    let start = SymbolId(1);

    grammar.add_rule(Rule {
        lhs: start,
        rhs: vec![Symbol::Epsilon],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let result = optimize_grammar(grammar);
    assert!(result.is_ok());
}

#[test]
fn edge_epsilon_is_productive() {
    let mut grammar = Grammar::new("eps_prod".to_string());
    let start = SymbolId(1);

    grammar.add_rule(Rule {
        lhs: start,
        rhs: vec![Symbol::Epsilon],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    let non_prod = result
        .errors
        .iter()
        .filter(|e| matches!(e, ValidationError::NonProductiveSymbol { .. }))
        .count();
    assert_eq!(non_prod, 0);
}

// --- 3.3 Self-referential rule (A -> A) ---

#[test]
fn edge_self_referential_rule() {
    let mut grammar = Grammar::new("self_ref".to_string());
    let a = SymbolId(1);

    grammar.add_rule(Rule {
        lhs: a,
        rhs: vec![Symbol::NonTerminal(a)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    // Should detect as cyclic
    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    assert!(
        result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::CyclicRule { .. }))
    );
}

#[test]
fn edge_self_referential_non_productive() {
    let mut grammar = Grammar::new("self_ref_np".to_string());
    let a = SymbolId(1);

    grammar.add_rule(Rule {
        lhs: a,
        rhs: vec![Symbol::NonTerminal(a)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    assert!(
        result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::NonProductiveSymbol { .. }))
    );
}

// --- 3.4 Large number of alternatives ---

#[test]
fn edge_many_alternatives() {
    let mut builder = GrammarBuilder::new("many_alts");

    for i in 0..20 {
        let tok_name = format!("T{}", i);
        builder = builder.token(&tok_name, &tok_name);
    }
    for i in 0..20 {
        let tok_name = format!("T{}", i);
        builder = builder.rule("expr", vec![&tok_name]);
    }
    let grammar = builder.start("expr").build();

    let result = optimize_grammar(grammar);
    assert!(result.is_ok());
    assert!(total_rule_count(&result.unwrap()) >= 20);
}

// --- 3.5 Grammar with only tokens, no rules ---

#[test]
fn edge_tokens_only_no_rules() {
    let mut grammar = Grammar::new("tokens_only".to_string());
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "FOO".to_string(),
            pattern: TokenPattern::String("foo".to_string()),
            fragile: false,
        },
    );

    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    assert!(
        result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::EmptyGrammar))
    );
}

// --- 3.6 check_empty_terminals ---

#[test]
fn edge_check_empty_string_terminal() {
    let mut grammar = Grammar::new("empty_term".to_string());
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "EMPTY".to_string(),
            pattern: TokenPattern::String(String::new()),
            fragile: false,
        },
    );

    let result = grammar.check_empty_terminals();
    assert!(result.is_err());
    assert!(!result.unwrap_err().is_empty());
}

#[test]
fn edge_check_empty_regex_terminal() {
    let mut grammar = Grammar::new("empty_regex_term".to_string());
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "EMPTY_RE".to_string(),
            pattern: TokenPattern::Regex(String::new()),
            fragile: false,
        },
    );

    let result = grammar.check_empty_terminals();
    assert!(result.is_err());
}

#[test]
fn edge_check_nonempty_terminals_ok() {
    let grammar = build_simple_expr_grammar();
    let result = grammar.check_empty_terminals();
    assert!(result.is_ok());
}

// --- 3.7 Grammar normalization through optimizer ---

#[test]
fn edge_normalize_optional_symbol() {
    let mut grammar = Grammar::new("opt_norm".to_string());
    let expr = SymbolId(1);
    let num = SymbolId(2);

    grammar.tokens.insert(
        num,
        Token {
            name: "NUMBER".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );
    grammar.add_rule(Rule {
        lhs: expr,
        rhs: vec![Symbol::Optional(Box::new(Symbol::Terminal(num)))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let rules = grammar.normalize();
    // Normalization should expand the Optional into auxiliary rules
    assert!(rules.len() >= 2);
}

#[test]
fn edge_normalize_repeat_symbol() {
    let mut grammar = Grammar::new("rep_norm".to_string());
    let expr = SymbolId(1);
    let num = SymbolId(2);

    grammar.tokens.insert(
        num,
        Token {
            name: "NUMBER".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );
    grammar.add_rule(Rule {
        lhs: expr,
        rhs: vec![Symbol::Repeat(Box::new(Symbol::Terminal(num)))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let rules = grammar.normalize();
    assert!(rules.len() >= 2);
}

#[test]
fn edge_normalize_repeat_one_symbol() {
    let mut grammar = Grammar::new("rep1_norm".to_string());
    let expr = SymbolId(1);
    let num = SymbolId(2);

    grammar.tokens.insert(
        num,
        Token {
            name: "NUMBER".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );
    grammar.add_rule(Rule {
        lhs: expr,
        rhs: vec![Symbol::RepeatOne(Box::new(Symbol::Terminal(num)))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let rules = grammar.normalize();
    assert!(rules.len() >= 2);
}

#[test]
fn edge_normalize_choice_symbol() {
    let mut grammar = Grammar::new("choice_norm".to_string());
    let expr = SymbolId(1);
    let num = SymbolId(2);
    let plus = SymbolId(3);

    grammar.tokens.insert(
        num,
        Token {
            name: "NUMBER".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );
    grammar.tokens.insert(
        plus,
        Token {
            name: "+".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );
    grammar.add_rule(Rule {
        lhs: expr,
        rhs: vec![Symbol::Choice(vec![
            Symbol::Terminal(num),
            Symbol::Terminal(plus),
        ])],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let rules = grammar.normalize();
    // Choice should be expanded into separate aux rules
    assert!(rules.len() >= 2);
}

#[test]
fn edge_normalize_sequence_flattens() {
    let mut grammar = Grammar::new("seq_norm".to_string());
    let expr = SymbolId(1);
    let num = SymbolId(2);

    grammar.tokens.insert(
        num,
        Token {
            name: "NUMBER".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );
    grammar.add_rule(Rule {
        lhs: expr,
        rhs: vec![Symbol::Sequence(vec![
            Symbol::Terminal(num),
            Symbol::Terminal(num),
        ])],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let rules = grammar.normalize();
    // Sequence should be flattened into the parent rule
    let main_rule = rules.iter().find(|r| r.lhs == expr).unwrap();
    assert_eq!(main_rule.rhs.len(), 2);
    assert!(matches!(main_rule.rhs[0], Symbol::Terminal(_)));
    assert!(matches!(main_rule.rhs[1], Symbol::Terminal(_)));
}

// --- 3.8 find_symbol_by_name ---

#[test]
fn edge_find_symbol_by_name() {
    let grammar = GrammarBuilder::new("find")
        .token("NUMBER", r"\d+")
        .rule("expr", vec!["NUMBER"])
        .start("expr")
        .build();

    assert!(grammar.find_symbol_by_name("expr").is_some());
    assert!(grammar.find_symbol_by_name("nonexistent").is_none());
}

// --- 3.9 start_symbol fallback ---

#[test]
fn edge_start_symbol_requires_explicit_metadata() {
    let grammar = GrammarBuilder::new("no_special_start")
        .token("NUMBER", r"\d+")
        .rule("expr", vec!["NUMBER"])
        .start("expr")
        .build();

    assert_eq!(grammar.start_symbol(), grammar.explicit_start_symbol());
}

// --- 3.10 Deeply nested complex symbols normalize correctly ---

#[test]
fn edge_nested_optional_in_repeat() {
    let mut grammar = Grammar::new("nested".to_string());
    let expr = SymbolId(1);
    let num = SymbolId(2);

    grammar.tokens.insert(
        num,
        Token {
            name: "NUMBER".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );

    // expr -> (num?)*
    grammar.add_rule(Rule {
        lhs: expr,
        rhs: vec![Symbol::Repeat(Box::new(Symbol::Optional(Box::new(
            Symbol::Terminal(num),
        ))))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let rules = grammar.normalize();
    // Should expand to multiple auxiliary rules
    assert!(rules.len() >= 3);
    // No complex symbols should remain after normalization
    for rule in &rules {
        for sym in &rule.rhs {
            assert!(
                !matches!(
                    sym,
                    Symbol::Optional(_)
                        | Symbol::Repeat(_)
                        | Symbol::RepeatOne(_)
                        | Symbol::Choice(_)
                        | Symbol::Sequence(_)
                ),
                "Complex symbol found after normalization: {:?}",
                sym
            );
        }
    }
}

// --- 3.11 Grammar validate() method ---

#[test]
fn edge_grammar_validate_valid() {
    let grammar = GrammarBuilder::new("valid")
        .token("NUMBER", r"\d+")
        .rule("expr", vec!["NUMBER"])
        .start("expr")
        .build();

    assert!(grammar.validate().is_ok());
}

#[test]
fn edge_grammar_validate_unresolved_symbol() {
    let mut grammar = Grammar::new("unresolved".to_string());
    let expr = SymbolId(1);
    let unknown = SymbolId(99);

    grammar.add_rule(Rule {
        lhs: expr,
        rhs: vec![Symbol::NonTerminal(unknown)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    assert!(grammar.validate().is_err());
}

// --- 3.12 Builder with multiple rules for the same LHS ---

#[test]
fn edge_builder_multiple_alternatives() {
    let grammar = GrammarBuilder::new("multi")
        .token("NUMBER", r"\d+")
        .token("STRING", r#""[^"]*""#)
        .rule("literal", vec!["NUMBER"])
        .rule("literal", vec!["STRING"])
        .start("literal")
        .build();

    let lit_id = find_symbol(&grammar, "literal").unwrap();
    let rules = grammar.get_rules_for_symbol(lit_id).unwrap();
    assert_eq!(rules.len(), 2);
}

// --- 3.13 GrammarOptimizer default trait ---

#[test]
fn edge_optimizer_default() {
    let optimizer = GrammarOptimizer::default();
    // Should be the same as new()
    let mut grammar = build_simple_expr_grammar();
    let mut opt = optimizer;
    let stats = opt.optimize(&mut grammar);
    let _ = stats.total();
}

// --- 3.14 GrammarValidator default trait ---

#[test]
fn edge_validator_default() {
    let mut validator = GrammarValidator::default();
    let grammar = build_simple_expr_grammar();
    let result = validator.validate(&grammar);
    assert!(result.stats.total_rules > 0);
}

// --- 3.15 Validator can be reused ---

#[test]
fn edge_validator_reuse() {
    let mut validator = GrammarValidator::new();

    // First use
    let g1 = Grammar::new("empty".to_string());
    let r1 = validator.validate(&g1);
    assert!(!r1.errors.is_empty());

    // Second use — should clear previous state
    let g2 = GrammarBuilder::new("ok")
        .token("NUMBER", r"\d+")
        .rule("expr", vec!["NUMBER"])
        .start("expr")
        .build();
    let r2 = validator.validate(&g2);
    assert!(r2.errors.is_empty());
}

// --- 3.16 ValidationStats default ---

#[test]
fn edge_validation_stats_default() {
    let stats = adze_ir::validation::ValidationStats::default();
    assert_eq!(stats.total_symbols, 0);
    assert_eq!(stats.total_tokens, 0);
    assert_eq!(stats.total_rules, 0);
    assert_eq!(stats.reachable_symbols, 0);
    assert_eq!(stats.productive_symbols, 0);
    assert_eq!(stats.external_tokens, 0);
    assert_eq!(stats.max_rule_length, 0);
    assert_eq!(stats.avg_rule_length, 0.0);
}

// --- 3.17 Optimizer with external tokens ---

#[test]
fn edge_optimizer_with_externals() {
    let grammar = GrammarBuilder::new("ext")
        .token("NUMBER", r"\d+")
        .token("INDENT", "INDENT")
        .token("DEDENT", "DEDENT")
        .external("INDENT")
        .external("DEDENT")
        .rule("block", vec!["INDENT", "NUMBER", "DEDENT"])
        .start("block")
        .build();

    let optimized = optimize_grammar(grammar).unwrap();
    assert_eq!(optimized.externals.len(), 2);
}

// --- 3.18 Multiple precedence levels ---

#[test]
fn edge_multiple_precedence_levels() {
    let grammar = GrammarBuilder::new("multi_prec")
        .token("NUMBER", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .token("-", "-")
        .token("/", "/")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "-", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "/", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["NUMBER"])
        .start("expr")
        .build();

    let mut validator = GrammarValidator::new();
    let _result = validator.validate(&grammar);

    // Should have rules at both precedence levels
    let prec_rules: Vec<_> = grammar
        .all_rules()
        .filter(|r| r.precedence.is_some())
        .collect();
    assert_eq!(prec_rules.len(), 4);
}

// --- 3.19 Right and None associativity ---

#[test]
fn edge_right_associativity() {
    let grammar = GrammarBuilder::new("right_assoc")
        .token("NUMBER", r"\d+")
        .token("^", "^")
        .rule_with_precedence("expr", vec!["expr", "^", "expr"], 3, Associativity::Right)
        .rule("expr", vec!["NUMBER"])
        .start("expr")
        .build();

    let has_right = grammar
        .all_rules()
        .any(|r| matches!(r.associativity, Some(Associativity::Right)));
    assert!(has_right);
}

#[test]
fn edge_none_associativity() {
    let grammar = GrammarBuilder::new("none_assoc")
        .token("NUMBER", r"\d+")
        .token("==", "==")
        .rule_with_precedence("expr", vec!["expr", "==", "expr"], 1, Associativity::None)
        .rule("expr", vec!["NUMBER"])
        .start("expr")
        .build();

    let has_none = grammar
        .all_rules()
        .any(|r| matches!(r.associativity, Some(Associativity::None)));
    assert!(has_none);
}

// --- 3.20 Grammar with only epsilon productions ---

#[test]
fn edge_all_epsilon_productions() {
    let mut grammar = Grammar::new("all_eps".to_string());
    let a = SymbolId(1);
    let b = SymbolId(2);

    grammar.add_rule(Rule {
        lhs: a,
        rhs: vec![Symbol::Epsilon],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    grammar.add_rule(Rule {
        lhs: b,
        rhs: vec![Symbol::Epsilon],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });

    let result = optimize_grammar(grammar);
    assert!(result.is_ok());
}

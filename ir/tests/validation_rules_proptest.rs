#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

use adze_ir::validation::{GrammarValidator, ValidationError, ValidationWarning};
use adze_ir::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a minimal valid grammar: one token, one rule referencing it.
fn minimal_valid_grammar() -> Grammar {
    let mut g = Grammar::new("test".to_string());
    g.tokens.insert(
        SymbolId(1),
        Token {
            name: "tok".to_string(),
            pattern: TokenPattern::String("x".to_string()),
            fragile: false,
        },
    );
    g.add_rule(Rule {
        lhs: SymbolId(0),
        rhs: vec![Symbol::Terminal(SymbolId(1))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    g.rule_names.insert(SymbolId(0), "start".to_string());
    g.set_start_symbol(SymbolId(0));
    g
}

fn validate_errors(g: &Grammar) -> Vec<ValidationError> {
    GrammarValidator::new().validate(g).errors
}

fn validate_warnings(g: &Grammar) -> Vec<ValidationWarning> {
    GrammarValidator::new().validate(g).warnings
}

// ===================================================================
// 1. Valid grammar passes validation
// ===================================================================

#[test]
fn valid_grammar_no_errors() {
    let g = minimal_valid_grammar();
    let errs = validate_errors(&g);
    assert!(errs.is_empty(), "expected no errors, got: {errs:?}");
}

#[test]
fn valid_grammar_validate_method_ok() {
    let g = minimal_valid_grammar();
    assert!(g.validate().is_ok());
}

#[test]
fn valid_grammar_stats_populated() {
    let g = minimal_valid_grammar();
    let result = GrammarValidator::new().validate(&g);
    assert!(result.errors.is_empty());
    assert!(result.stats.total_rules >= 1);
    assert!(result.stats.total_tokens >= 1);
}

// ===================================================================
// 2. Missing start symbol / empty grammar fails
// ===================================================================

#[test]
fn empty_grammar_reports_empty_error() {
    let g = Grammar::default();
    let errs = validate_errors(&g);
    assert!(
        errs.iter()
            .any(|e| matches!(e, ValidationError::EmptyGrammar)),
        "expected EmptyGrammar error, got: {errs:?}"
    );
}

#[test]
fn grammar_with_tokens_but_no_rules_is_empty() {
    let mut g = Grammar::new("tokens_only".to_string());
    g.tokens.insert(
        SymbolId(1),
        Token {
            name: "a".to_string(),
            pattern: TokenPattern::String("a".to_string()),
            fragile: false,
        },
    );
    let errs = validate_errors(&g);
    assert!(
        errs.iter()
            .any(|e| matches!(e, ValidationError::EmptyGrammar))
    );
}

// ===================================================================
// 3. Undefined symbol reference fails
// ===================================================================

#[test]
fn undefined_terminal_reference_detected() {
    let mut g = minimal_valid_grammar();
    g.add_rule(Rule {
        lhs: SymbolId(10),
        rhs: vec![Symbol::Terminal(SymbolId(99))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });
    let errs = validate_errors(&g);
    assert!(
        errs.iter()
            .any(|e| matches!(e, ValidationError::UndefinedSymbol { .. }))
    );
}

#[test]
fn undefined_nonterminal_reference_detected() {
    let mut g = minimal_valid_grammar();
    g.add_rule(Rule {
        lhs: SymbolId(10),
        rhs: vec![Symbol::NonTerminal(SymbolId(88))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });
    let errs = validate_errors(&g);
    assert!(
        errs.iter()
            .any(|e| matches!(e, ValidationError::UndefinedSymbol { .. }))
    );
}

#[test]
fn validate_method_catches_unresolved_symbol() {
    let mut g = minimal_valid_grammar();
    g.add_rule(Rule {
        lhs: SymbolId(10),
        rhs: vec![Symbol::NonTerminal(SymbolId(77))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });
    match g.validate() {
        Err(GrammarError::UnresolvedSymbol(sid)) => assert_eq!(sid, SymbolId(77)),
        other => panic!("expected UnresolvedSymbol, got: {other:?}"),
    }
}

#[test]
fn undefined_external_symbol_detected() {
    let mut g = minimal_valid_grammar();
    g.add_rule(Rule {
        lhs: SymbolId(10),
        rhs: vec![Symbol::External(SymbolId(55))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });
    let errs = validate_errors(&g);
    assert!(
        errs.iter()
            .any(|e| matches!(e, ValidationError::UndefinedSymbol { .. }))
    );
}

// ===================================================================
// 4. Duplicate rule names detected
// ===================================================================

#[test]
fn duplicate_rule_names_do_not_panic() {
    let mut g = minimal_valid_grammar();
    g.rule_names.insert(SymbolId(0), "dup".to_string());
    g.rule_names.insert(SymbolId(1), "dup".to_string());
    // Validation must complete without panicking
    let _result = GrammarValidator::new().validate(&g);
}

#[test]
fn multiple_rules_same_lhs_accepted() {
    let mut g = minimal_valid_grammar();
    // Add a second alternative for the same LHS
    g.add_rule(Rule {
        lhs: SymbolId(0),
        rhs: vec![Symbol::Terminal(SymbolId(1)), Symbol::Terminal(SymbolId(1))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });
    // Multiple alternatives is valid in GLR grammars
    let _result = GrammarValidator::new().validate(&g);
}

// ===================================================================
// 5. Empty rules handling
// ===================================================================

#[test]
fn rule_with_epsilon_rhs_accepted() {
    let mut g = minimal_valid_grammar();
    g.add_rule(Rule {
        lhs: SymbolId(0),
        rhs: vec![Symbol::Epsilon],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(2),
    });
    // Epsilon productions are valid
    let errs = validate_errors(&g);
    assert!(
        !errs
            .iter()
            .any(|e| matches!(e, ValidationError::EmptyGrammar)),
        "epsilon rule should not trigger EmptyGrammar"
    );
}

#[test]
fn rule_with_empty_rhs_accepted() {
    let mut g = minimal_valid_grammar();
    g.add_rule(Rule {
        lhs: SymbolId(0),
        rhs: vec![],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(3),
    });
    // Empty RHS (unit epsilon) should not crash
    let _result = GrammarValidator::new().validate(&g);
}

#[test]
fn empty_token_regex_reports_invalid_regex() {
    let mut g = minimal_valid_grammar();
    g.tokens.insert(
        SymbolId(50),
        Token {
            name: "bad_tok".to_string(),
            pattern: TokenPattern::Regex(String::new()),
            fragile: false,
        },
    );
    let errs = validate_errors(&g);
    assert!(
        errs.iter()
            .any(|e| matches!(e, ValidationError::InvalidRegex { .. }))
    );
}

// ===================================================================
// 6. Cyclic rule detection
// ===================================================================

#[test]
fn direct_self_cycle_detected() {
    let mut g = minimal_valid_grammar();
    // A -> A  (direct self-reference with no terminal base case)
    g.add_rule(Rule {
        lhs: SymbolId(20),
        rhs: vec![Symbol::NonTerminal(SymbolId(20))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(10),
    });
    let errs = validate_errors(&g);
    assert!(
        errs.iter()
            .any(|e| matches!(e, ValidationError::CyclicRule { .. })),
        "expected CyclicRule error, got: {errs:?}"
    );
}

#[test]
fn mutual_cycle_detected() {
    let mut g = minimal_valid_grammar();
    // A -> B, B -> A
    g.add_rule(Rule {
        lhs: SymbolId(20),
        rhs: vec![Symbol::NonTerminal(SymbolId(21))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(10),
    });
    g.add_rule(Rule {
        lhs: SymbolId(21),
        rhs: vec![Symbol::NonTerminal(SymbolId(20))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(11),
    });
    let errs = validate_errors(&g);
    assert!(
        errs.iter()
            .any(|e| matches!(e, ValidationError::CyclicRule { .. })),
        "expected CyclicRule for mutual recursion, got: {errs:?}"
    );
}

#[test]
fn cycle_error_contains_involved_symbols() {
    let mut g = minimal_valid_grammar();
    g.add_rule(Rule {
        lhs: SymbolId(30),
        rhs: vec![Symbol::NonTerminal(SymbolId(30))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(20),
    });
    let errs = validate_errors(&g);
    for e in &errs {
        if let ValidationError::CyclicRule { symbols } = e {
            assert!(
                !symbols.is_empty(),
                "CyclicRule should list involved symbols"
            );
            return;
        }
    }
    panic!("expected CyclicRule error, got: {errs:?}");
}

// ===================================================================
// 7. Validation error messages are descriptive
// ===================================================================

#[test]
fn empty_grammar_display_message() {
    let msg = format!("{}", ValidationError::EmptyGrammar);
    assert!(
        msg.contains("no rules"),
        "EmptyGrammar Display should mention 'no rules': {msg}"
    );
}

#[test]
fn undefined_symbol_display_contains_location() {
    let err = ValidationError::UndefinedSymbol {
        symbol: SymbolId(42),
        location: "rule for SymbolId(0)".to_string(),
    };
    let msg = format!("{err}");
    assert!(msg.contains("42") || msg.contains("SymbolId"), "msg: {msg}");
    assert!(
        msg.contains("rule for"),
        "msg should contain location: {msg}"
    );
}

#[test]
fn cyclic_rule_display_mentions_cycle() {
    let err = ValidationError::CyclicRule {
        symbols: vec![SymbolId(1), SymbolId(2)],
    };
    let msg = format!("{err}");
    assert!(
        msg.to_lowercase().contains("cycl"),
        "CyclicRule Display should mention cycle: {msg}"
    );
}

#[test]
fn non_productive_display_mentions_terminal() {
    let err = ValidationError::NonProductiveSymbol {
        symbol: SymbolId(5),
        name: "orphan".to_string(),
    };
    let msg = format!("{err}");
    assert!(
        msg.contains("terminal"),
        "NonProductiveSymbol Display should mention terminal strings: {msg}"
    );
}

#[test]
fn invalid_regex_display_contains_pattern() {
    let err = ValidationError::InvalidRegex {
        token: SymbolId(7),
        pattern: "[bad".to_string(),
        error: "unclosed bracket".to_string(),
    };
    let msg = format!("{err}");
    assert!(msg.contains("[bad"), "should contain the pattern: {msg}");
    assert!(msg.contains("unclosed"), "should contain the error: {msg}");
}

#[test]
fn external_token_conflict_display() {
    let err = ValidationError::ExternalTokenConflict {
        token1: "indent".to_string(),
        token2: "indent".to_string(),
    };
    let msg = format!("{err}");
    assert!(msg.contains("indent"), "should mention token name: {msg}");
    assert!(
        msg.to_lowercase().contains("conflict"),
        "should mention conflict: {msg}"
    );
}

// ===================================================================
// 8. Validation determinism
// ===================================================================

#[test]
fn validation_is_deterministic_on_valid_grammar() {
    let g = minimal_valid_grammar();
    let r1 = validate_errors(&g);
    let r2 = validate_errors(&g);
    assert_eq!(r1, r2, "validation should be deterministic");
}

#[test]
fn validation_is_deterministic_on_invalid_grammar() {
    let mut g = minimal_valid_grammar();
    g.add_rule(Rule {
        lhs: SymbolId(50),
        rhs: vec![Symbol::NonTerminal(SymbolId(50))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(50),
    });
    let r1 = validate_errors(&g);
    let r2 = validate_errors(&g);
    assert_eq!(r1, r2, "errors should be identical across runs");
}

#[test]
fn validator_reuse_gives_same_results() {
    let g = minimal_valid_grammar();
    let mut v = GrammarValidator::new();
    let first = v.validate(&g);
    let second = v.validate(&g);
    assert_eq!(first.errors, second.errors);
    assert_eq!(first.warnings, second.warnings);
}

#[test]
fn stats_deterministic_across_runs() {
    let g = minimal_valid_grammar();
    let s1 = GrammarValidator::new().validate(&g).stats;
    let s2 = GrammarValidator::new().validate(&g).stats;
    assert_eq!(s1.total_rules, s2.total_rules);
    assert_eq!(s1.total_tokens, s2.total_tokens);
    assert_eq!(s1.total_symbols, s2.total_symbols);
    assert_eq!(s1.reachable_symbols, s2.reachable_symbols);
    assert_eq!(s1.productive_symbols, s2.productive_symbols);
}

// ===================================================================
// Additional edge-case coverage
// ===================================================================

#[test]
fn conflicting_precedence_detected() {
    let mut g = minimal_valid_grammar();
    g.precedences.push(Precedence {
        level: 1,
        associativity: Associativity::Left,
        symbols: vec![SymbolId(0)],
    });
    g.precedences.push(Precedence {
        level: 5,
        associativity: Associativity::Right,
        symbols: vec![SymbolId(0)],
    });
    let errs = validate_errors(&g);
    assert!(
        errs.iter()
            .any(|e| matches!(e, ValidationError::ConflictingPrecedence { .. })),
        "expected ConflictingPrecedence, got: {errs:?}"
    );
}

#[test]
fn duplicate_external_token_names_detected() {
    let mut g = minimal_valid_grammar();
    g.externals.push(ExternalToken {
        name: "indent".to_string(),
        symbol_id: SymbolId(100),
    });
    g.externals.push(ExternalToken {
        name: "indent".to_string(),
        symbol_id: SymbolId(101),
    });
    let errs = validate_errors(&g);
    assert!(
        errs.iter()
            .any(|e| matches!(e, ValidationError::ExternalTokenConflict { .. })),
        "expected ExternalTokenConflict, got: {errs:?}"
    );
}

#[test]
fn invalid_field_index_detected() {
    let mut g = minimal_valid_grammar();
    g.add_rule(Rule {
        lhs: SymbolId(40),
        rhs: vec![Symbol::Terminal(SymbolId(1))],
        precedence: None,
        associativity: None,
        fields: vec![(FieldId(0), 999)], // index 999 exceeds rhs length
        production_id: ProductionId(40),
    });
    let errs = validate_errors(&g);
    assert!(
        errs.iter()
            .any(|e| matches!(e, ValidationError::InvalidField { .. })),
        "expected InvalidField, got: {errs:?}"
    );
}

#[test]
fn non_productive_symbol_detected() {
    let mut g = minimal_valid_grammar();
    // Add a rule whose RHS references only itself — non-productive & cyclic
    g.add_rule(Rule {
        lhs: SymbolId(60),
        rhs: vec![Symbol::NonTerminal(SymbolId(60))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(60),
    });
    let errs = validate_errors(&g);
    assert!(
        errs.iter()
            .any(|e| matches!(e, ValidationError::NonProductiveSymbol { .. })),
        "expected NonProductiveSymbol, got: {errs:?}"
    );
}

#[test]
fn warning_for_missing_field_names_on_multi_rhs_rule() {
    let mut g = minimal_valid_grammar();
    // Add a second token
    g.tokens.insert(
        SymbolId(2),
        Token {
            name: "tok2".to_string(),
            pattern: TokenPattern::String("y".to_string()),
            fragile: false,
        },
    );
    g.add_rule(Rule {
        lhs: SymbolId(70),
        rhs: vec![Symbol::Terminal(SymbolId(1)), Symbol::Terminal(SymbolId(2))],
        precedence: None,
        associativity: None,
        fields: vec![], // no field names
        production_id: ProductionId(70),
    });
    let warnings = validate_warnings(&g);
    assert!(
        warnings
            .iter()
            .any(|w| matches!(w, ValidationWarning::MissingFieldNames { .. })),
        "expected MissingFieldNames warning, got: {warnings:?}"
    );
}

#[test]
fn check_empty_terminals_catches_empty_string_token() {
    let mut g = minimal_valid_grammar();
    g.tokens.insert(
        SymbolId(80),
        Token {
            name: "empty".to_string(),
            pattern: TokenPattern::String(String::new()),
            fragile: false,
        },
    );
    assert!(
        g.check_empty_terminals().is_err(),
        "empty string token should be caught"
    );
}

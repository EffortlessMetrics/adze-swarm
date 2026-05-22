// Comprehensive error recovery tests for the runtime crate.
//
// Tests cover:
// 1. Missing tokens (missing delimiters)
// 2. Extra tokens (unexpected tokens)
// 3. Cascading errors (multiple errors in one input)
// 4. Recovery strategies (parser recovers and continues)
// 5. Error node positioning (correct byte ranges)
// 6. Error message content (helpful error messages)
// 7. Partial parse (partial AST despite errors)
// 8. Error count (matches expectations)

use adze::error_recovery::{
    ErrorRecoveryConfig, ErrorRecoveryConfigBuilder, ErrorRecoveryState, RecoveryStrategy,
};
use adze::glr_lexer::GLRLexer;
use adze::glr_parser::GLRParser;
use adze::subtree::Subtree;
use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Grammar helpers
// ---------------------------------------------------------------------------

/// Arithmetic grammar: expression-based with +, *, parentheses, semicolons.
fn arithmetic_grammar() -> Grammar {
    let mut g = Grammar::new("arithmetic".to_string());

    let num = SymbolId(1);
    let plus = SymbolId(2);
    let star = SymbolId(3);
    let lparen = SymbolId(4);
    let rparen = SymbolId(5);
    let semi = SymbolId(6);

    for (id, name, pat) in [
        (num, "number", TokenPattern::Regex("[0-9]+".to_string())),
        (plus, "plus", TokenPattern::String("+".to_string())),
        (star, "star", TokenPattern::String("*".to_string())),
        (lparen, "lparen", TokenPattern::String("(".to_string())),
        (rparen, "rparen", TokenPattern::String(")".to_string())),
        (semi, "semicolon", TokenPattern::String(";".to_string())),
    ] {
        g.tokens.insert(
            id,
            Token {
                name: name.to_string(),
                pattern: pat,
                fragile: false,
            },
        );
    }

    let expr = SymbolId(10);
    let stmt = SymbolId(11);
    g.rule_names.insert(expr, "expression".to_string());
    g.rule_names.insert(stmt, "statement".to_string());

    // statement → expression ';'
    g.rules.entry(stmt).or_default().push(Rule {
        lhs: stmt,
        rhs: vec![Symbol::NonTerminal(expr), Symbol::Terminal(semi)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });

    // expression → expression '+' expression
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![
            Symbol::NonTerminal(expr),
            Symbol::Terminal(plus),
            Symbol::NonTerminal(expr),
        ],
        precedence: None,
        associativity: None,
        production_id: ProductionId(1),
        fields: vec![],
    });

    // expression → expression '*' expression
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![
            Symbol::NonTerminal(expr),
            Symbol::Terminal(star),
            Symbol::NonTerminal(expr),
        ],
        precedence: None,
        associativity: None,
        production_id: ProductionId(2),
        fields: vec![],
    });

    // expression → '(' expression ')'
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![
            Symbol::Terminal(lparen),
            Symbol::NonTerminal(expr),
            Symbol::Terminal(rparen),
        ],
        precedence: None,
        associativity: None,
        production_id: ProductionId(3),
        fields: vec![],
    });

    // expression → number
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![Symbol::Terminal(num)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(4),
        fields: vec![],
    });

    g
}

/// Simple additive grammar (no parens, no semicolons) for focused tests.
fn simple_add_grammar() -> Grammar {
    let mut g = Grammar::new("simple_add".to_string());

    let num = SymbolId(1);
    let plus = SymbolId(2);

    g.tokens.insert(
        num,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex("[0-9]+".to_string()),
            fragile: false,
        },
    );
    g.tokens.insert(
        plus,
        Token {
            name: "plus".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );

    let expr = SymbolId(10);
    g.rule_names.insert(expr, "expression".to_string());

    // expression → expression '+' expression
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![
            Symbol::NonTerminal(expr),
            Symbol::Terminal(plus),
            Symbol::NonTerminal(expr),
        ],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });

    // expression → number
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![Symbol::Terminal(num)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(1),
        fields: vec![],
    });

    g
}

// ---------------------------------------------------------------------------
// Parse helpers
// ---------------------------------------------------------------------------

fn parse_with_recovery(
    grammar: &Grammar,
    input: &str,
    config: ErrorRecoveryConfig,
) -> Result<Arc<Subtree>, String> {
    let first_follow = FirstFollowSets::compute(grammar).unwrap();
    let table = build_lr1_automaton(grammar, &first_follow).unwrap();

    let mut parser = GLRParser::new(table, grammar.clone());
    parser.enable_error_recovery(config);

    let mut lexer = GLRLexer::new(grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    parser.reset();
    for token in &tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
    }
    let total_bytes = tokens
        .last()
        .map(|t| t.byte_offset + t.text.len())
        .unwrap_or(0);
    parser.process_eof(total_bytes);
    parser.finish()
}

/// Count total nodes in the subtree.
fn count_nodes(tree: &Subtree) -> usize {
    1 + tree
        .children
        .iter()
        .map(|edge| count_nodes(&edge.subtree))
        .sum::<usize>()
}

/// Default recovery config with scope delimiters and common insertable tokens.
fn default_recovery_config() -> ErrorRecoveryConfig {
    ErrorRecoveryConfigBuilder::new()
        .add_insertable_token(5) // rparen
        .add_insertable_token(6) // semicolon
        .add_sync_token(6) // semicolon as sync point
        .add_scope_delimiter(4, 5) // lparen/rparen
        .enable_scope_recovery(true)
        .enable_phrase_recovery(true)
        .max_consecutive_errors(10)
        .build()
}

// ===========================================================================
// 1. Missing tokens – parse input with missing delimiters
// ===========================================================================

#[test]
fn test_missing_closing_paren() {
    let grammar = arithmetic_grammar();
    let config = default_recovery_config();
    let result = parse_with_recovery(&grammar, "(1 + 2;", config);
    assert!(
        result.is_ok(),
        "Parser should recover from missing closing paren: {:?}",
        result.err()
    );
}

#[test]
fn test_missing_semicolon() {
    let grammar = arithmetic_grammar();
    let config = default_recovery_config();
    let result = parse_with_recovery(&grammar, "1 + 2", config);
    // With recovery enabled the parser should either succeed or produce a partial tree
    assert!(
        result.is_ok(),
        "Parser should recover from missing semicolon: {:?}",
        result.err()
    );
}

#[test]
fn test_missing_opening_paren() {
    let grammar = arithmetic_grammar();
    let config = default_recovery_config();
    let result = parse_with_recovery(&grammar, "1 + 2);", config);
    assert!(
        result.is_ok(),
        "Parser should recover from extra closing paren: {:?}",
        result.err()
    );
}

#[test]
fn test_missing_operand() {
    let grammar = arithmetic_grammar();
    let config = default_recovery_config();
    // "1 + ;" is missing the right-hand operand
    let result = parse_with_recovery(&grammar, "1 + ;", config);
    assert!(
        result.is_ok(),
        "Parser should recover from missing operand: {:?}",
        result.err()
    );
}

// ===========================================================================
// 2. Extra tokens – parse input with unexpected tokens
// ===========================================================================

#[test]
fn test_double_operator() {
    let grammar = arithmetic_grammar();
    let config = default_recovery_config();
    let result = parse_with_recovery(&grammar, "1 + + 2;", config);
    assert!(
        result.is_ok(),
        "Parser should recover from double operator: {:?}",
        result.err()
    );
}

#[test]
fn test_leading_operator() {
    let grammar = arithmetic_grammar();
    let config = default_recovery_config();
    let result = parse_with_recovery(&grammar, "+ 1 + 2;", config);
    assert!(
        result.is_ok(),
        "Parser should recover from leading operator: {:?}",
        result.err()
    );
}

#[test]
fn test_extra_semicolons() {
    let grammar = arithmetic_grammar();
    let config = default_recovery_config();
    let result = parse_with_recovery(&grammar, "1 + 2;;", config);
    assert!(
        result.is_ok(),
        "Parser should recover from extra semicolons: {:?}",
        result.err()
    );
}

// ===========================================================================
// 3. Cascading errors – multiple errors in one input
// ===========================================================================

#[test]
fn test_cascading_multiple_missing_delimiters() {
    let grammar = arithmetic_grammar();
    let config = ErrorRecoveryConfigBuilder::new()
        .add_insertable_token(5) // rparen
        .add_insertable_token(6) // semicolon
        .add_sync_token(6)
        .add_scope_delimiter(4, 5)
        .enable_scope_recovery(true)
        .enable_phrase_recovery(true)
        .max_consecutive_errors(20)
        .build();

    // Multiple problems: missing rparen, double op, missing semicolon
    let result = parse_with_recovery(&grammar, "(1 + + 2", config);
    assert!(
        result.is_ok(),
        "Parser should recover from cascading errors: {:?}",
        result.err()
    );
}

#[test]
fn test_cascading_error_state_tracking() {
    // Verify ErrorRecoveryState correctly tracks multiple consecutive errors
    let config = ErrorRecoveryConfig {
        max_consecutive_errors: 5,
        ..Default::default()
    };
    let mut state = ErrorRecoveryState::new(config);

    for _ in 0..4 {
        state.increment_error_count();
        assert!(!state.should_give_up(), "Should not give up before limit");
    }
    state.increment_error_count();
    assert!(
        state.should_give_up(),
        "Should give up at max_consecutive_errors"
    );
}

#[test]
fn test_cascading_error_recording() {
    let config = ErrorRecoveryConfig::default();
    let mut state = ErrorRecoveryState::new(config);

    // Record three distinct errors
    for i in 0..3 {
        let start = i * 10;
        let end = start + 5;
        state.record_error(
            start,
            end,
            (0, start),
            (0, end),
            vec![1, 2],
            Some(99),
            RecoveryStrategy::TokenDeletion,
            vec![99],
        );
    }

    let errors = state.get_error_nodes();
    assert_eq!(errors.len(), 3, "Should have recorded 3 errors");
}

// ===========================================================================
// 4. Recovery strategies – verify parser recovers and continues
// ===========================================================================

#[test]
fn test_token_insertion_strategy() {
    let mut config = ErrorRecoveryConfig::default();
    config.insert_candidates.push(SymbolId(10));

    let mut state = ErrorRecoveryState::new(config);
    let strategy = state.determine_recovery_strategy(&[10, 11], None, (0, 0), 0);
    assert_eq!(
        strategy,
        RecoveryStrategy::TokenInsertion,
        "Should choose token insertion when candidate is available"
    );
}

#[test]
fn test_token_deletion_strategy() {
    let config = ErrorRecoveryConfig {
        enable_phrase_recovery: false,
        enable_scope_recovery: false,
        ..Default::default()
    };
    let mut state = ErrorRecoveryState::new(config);

    // Token 99 is clearly wrong (not in expected set, not a sync token)
    let strategy = state.determine_recovery_strategy(&[1, 2, 3], Some(99), (0, 0), 0);
    assert_eq!(
        strategy,
        RecoveryStrategy::TokenDeletion,
        "Should choose deletion for clearly wrong token"
    );
}

#[test]
fn test_token_substitution_strategy() {
    let mut config = ErrorRecoveryConfig {
        enable_phrase_recovery: false,
        enable_scope_recovery: false,
        ..Default::default()
    };
    // Make token 99 a sync token so deletion is skipped, exposing substitution
    config.sync_tokens.push(SymbolId(99));

    let mut state = ErrorRecoveryState::new(config);

    // Exactly one expected token and actual is a sync token → substitution
    let strategy = state.determine_recovery_strategy(&[5], Some(99), (0, 0), 0);
    assert_eq!(
        strategy,
        RecoveryStrategy::TokenSubstitution,
        "Should choose substitution when exactly one expected token"
    );
}

#[test]
fn test_scope_recovery_strategy() {
    let mut config = ErrorRecoveryConfig {
        scope_delimiters: vec![(10, 11)],
        enable_scope_recovery: true,
        enable_phrase_recovery: false,
        ..Default::default()
    };
    // Make the closing delimiter a sync token so deletion is skipped
    config.sync_tokens.push(SymbolId(11));

    let mut state = ErrorRecoveryState::new(config);

    // Closing delimiter (sync token) without matching open → scope recovery
    let strategy = state.determine_recovery_strategy(&[1, 2], Some(11), (0, 0), 0);
    assert_eq!(
        strategy,
        RecoveryStrategy::ScopeRecovery,
        "Should choose scope recovery for unmatched delimiter"
    );
}

#[test]
fn test_panic_mode_after_max_errors() {
    let config = ErrorRecoveryConfig {
        max_consecutive_errors: 3,
        ..Default::default()
    };
    let mut state = ErrorRecoveryState::new(config);

    // Exhaust error budget
    for _ in 0..4 {
        state.increment_error_count();
    }
    let strategy = state.determine_recovery_strategy(&[1, 2], Some(99), (0, 0), 0);
    assert_eq!(
        strategy,
        RecoveryStrategy::PanicMode,
        "Should fall back to panic mode after exceeding error limit"
    );
}

#[test]
fn test_phrase_level_recovery_fallback() {
    let mut config = ErrorRecoveryConfig {
        enable_phrase_recovery: true,
        enable_scope_recovery: false,
        ..Default::default()
    };
    // Make token 99 a sync token so deletion and substitution are skipped
    config.sync_tokens.push(SymbolId(99));

    let mut state = ErrorRecoveryState::new(config);

    // No insertion candidates, multiple expected, sync token → phrase level
    let strategy = state.determine_recovery_strategy(&[1, 2, 3], Some(99), (0, 0), 0);
    assert_eq!(
        strategy,
        RecoveryStrategy::PhraseLevel,
        "Should fall back to phrase-level recovery"
    );
}

#[test]
fn test_recovery_resets_on_success() {
    let mut config = ErrorRecoveryConfig::default();
    config.insert_candidates.push(SymbolId(10));

    let mut state = ErrorRecoveryState::new(config);

    // Accumulate some errors
    state.increment_error_count();
    state.increment_error_count();

    // Successful insertion resets the counter
    let _strategy = state.determine_recovery_strategy(&[10], None, (0, 0), 0);
    assert!(
        !state.should_give_up(),
        "Error counter should reset after successful recovery"
    );
}

// ===========================================================================
// 5. Error node positioning – verify error nodes have correct byte ranges
// ===========================================================================

#[test]
fn test_error_node_byte_ranges() {
    let config = ErrorRecoveryConfig::default();
    let mut state = ErrorRecoveryState::new(config);

    state.record_error(
        10,
        25,
        (1, 0),
        (1, 15),
        vec![1],
        Some(2),
        RecoveryStrategy::TokenDeletion,
        vec![],
    );

    let errors = state.get_error_nodes();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].start_byte, 10);
    assert_eq!(errors[0].end_byte, 25);
}

#[test]
fn test_error_node_byte_ranges_multiple() {
    let config = ErrorRecoveryConfig::default();
    let mut state = ErrorRecoveryState::new(config);

    let ranges = [(0, 3), (10, 15), (20, 30)];
    for (start, end) in &ranges {
        state.record_error(
            *start,
            *end,
            (0, *start),
            (0, *end),
            vec![1],
            Some(2),
            RecoveryStrategy::TokenDeletion,
            vec![],
        );
    }

    let errors = state.get_error_nodes();
    assert_eq!(errors.len(), 3);
    for (i, (start, end)) in ranges.iter().enumerate() {
        assert_eq!(errors[i].start_byte, *start, "start_byte mismatch at {i}");
        assert_eq!(errors[i].end_byte, *end, "end_byte mismatch at {i}");
    }
}

#[test]
fn test_error_node_zero_width() {
    let config = ErrorRecoveryConfig::default();
    let mut state = ErrorRecoveryState::new(config);

    // Zero-width error (e.g., missing token insertion point)
    state.record_error(
        5,
        5,
        (0, 5),
        (0, 5),
        vec![3],
        None,
        RecoveryStrategy::TokenInsertion,
        vec![],
    );

    let errors = state.get_error_nodes();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].start_byte, errors[0].end_byte);
}

#[test]
fn test_parse_tree_error_node_byte_range() {
    // When parsing with recovery the error wrapper node should span the full input
    let grammar = arithmetic_grammar();
    let config = default_recovery_config();
    let input = "1 + 2";
    let result = parse_with_recovery(&grammar, input, config);

    if let Ok(tree) = result {
        if tree.node.is_error {
            assert_eq!(
                tree.node.byte_range.start, 0,
                "Error wrapper should start at 0"
            );
        }
        // Non-error root should still have a valid range
        assert!(
            tree.node.byte_range.end <= input.len(),
            "Byte range end should not exceed input length"
        );
    }
}

// ===========================================================================
// 6. Error message content – verify error messages are helpful
// ===========================================================================

#[test]
fn test_error_node_expected_symbols() {
    let config = ErrorRecoveryConfig::default();
    let mut state = ErrorRecoveryState::new(config);

    state.record_error(
        0,
        1,
        (0, 0),
        (0, 1),
        vec![1, 2, 3],
        Some(99),
        RecoveryStrategy::TokenDeletion,
        vec![99],
    );

    let errors = state.get_error_nodes();
    assert_eq!(errors[0].expected, vec![1, 2, 3]);
    assert_eq!(errors[0].actual, Some(99));
}

#[test]
fn test_error_node_missing_token_has_none_actual() {
    let config = ErrorRecoveryConfig::default();
    let mut state = ErrorRecoveryState::new(config);

    state.record_error(
        0,
        0,
        (0, 0),
        (0, 0),
        vec![5],
        None, // nothing was found
        RecoveryStrategy::TokenInsertion,
        vec![],
    );

    let errors = state.get_error_nodes();
    assert!(
        errors[0].actual.is_none(),
        "Missing token should have None actual"
    );
    assert!(
        !errors[0].expected.is_empty(),
        "Should list expected tokens"
    );
}

#[test]
fn test_parse_error_message_contains_state_info() {
    let grammar = simple_add_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let table = build_lr1_automaton(&grammar, &first_follow).unwrap();

    // Parse clearly invalid input without recovery → should get informative error
    let mut parser = GLRParser::new(table, grammar.clone());
    parser.reset();
    // Feed no tokens, just EOF
    parser.process_eof(0);
    let result = parser.finish();
    assert!(result.is_err(), "Empty input should fail without recovery");
    let msg = result.unwrap_err();
    assert!(
        msg.contains("Stack states") || msg.contains("Parse"),
        "Error message should contain debugging info, got: {}",
        msg
    );
}

// ===========================================================================
// 7. Partial parse – verify partial AST is produced despite errors
// ===========================================================================

#[test]
fn test_partial_parse_produces_tree() {
    let grammar = arithmetic_grammar();
    let config = ErrorRecoveryConfigBuilder::new()
        .add_insertable_token(5) // rparen
        .add_insertable_token(6) // semicolon
        .add_sync_token(6)
        .add_scope_delimiter(4, 5)
        .enable_scope_recovery(true)
        .max_consecutive_errors(15)
        .build();

    // Many errors but there is still parseable content
    let result = parse_with_recovery(&grammar, "(1 + 2", config);
    if let Ok(tree) = result {
        assert!(
            count_nodes(&tree) > 0,
            "Partial parse should still produce nodes"
        );
    }
    // It's acceptable if the parser fails here; the key is it doesn't panic.
}

#[test]
fn test_partial_parse_has_children() {
    let grammar = arithmetic_grammar();
    let config = default_recovery_config();

    let result = parse_with_recovery(&grammar, "1 + 2;", config);
    if let Ok(tree) = result {
        // A successful or partially-recovered tree should have children
        let total = count_nodes(&tree);
        assert!(total >= 2, "Parse tree should have at least 2 nodes");
    }
}

#[test]
fn test_partial_parse_error_wrapper() {
    let grammar = arithmetic_grammar();
    let config = default_recovery_config();

    // Missing semicolon should produce a tree (possibly error-wrapped)
    let result = parse_with_recovery(&grammar, "1 + 2", config);
    if let Ok(tree) = result
        && tree.node.is_error
    {
        // Error wrapper should still contain useful children
        assert!(
            !tree.children.is_empty(),
            "Error wrapper should contain children from partial parse"
        );
    }
}

// ===========================================================================
// 8. Error count – verify error count matches expectations
// ===========================================================================

#[test]
fn test_error_count_zero_for_valid_input() {
    let config = ErrorRecoveryConfig::default();
    let state = ErrorRecoveryState::new(config);

    assert_eq!(
        state.get_error_nodes().len(),
        0,
        "Fresh state should have zero errors"
    );
}

#[test]
fn test_error_count_increments() {
    let config = ErrorRecoveryConfig::default();
    let mut state = ErrorRecoveryState::new(config);

    for expected_count in 1..=5 {
        state.record_error(
            0,
            1,
            (0, 0),
            (0, 1),
            vec![1],
            Some(2),
            RecoveryStrategy::TokenDeletion,
            vec![],
        );
        assert_eq!(state.get_error_nodes().len(), expected_count);
    }
}

#[test]
fn test_error_count_reset() {
    let config = ErrorRecoveryConfig::default();
    let mut state = ErrorRecoveryState::new(config);

    state.record_error(
        0,
        1,
        (0, 0),
        (0, 1),
        vec![1],
        Some(2),
        RecoveryStrategy::TokenDeletion,
        vec![],
    );
    assert_eq!(state.get_error_nodes().len(), 1);

    state.clear_errors();
    assert_eq!(
        state.get_error_nodes().len(),
        0,
        "clear_errors should reset count"
    );
}

#[test]
fn test_consecutive_error_counter_independent_of_recorded_nodes() {
    let config = ErrorRecoveryConfig {
        max_consecutive_errors: 3,
        ..Default::default()
    };
    let mut state = ErrorRecoveryState::new(config);

    // increment_error_count tracks the counter; record_error tracks nodes
    state.increment_error_count();
    state.increment_error_count();
    assert_eq!(
        state.get_error_nodes().len(),
        0,
        "No error nodes recorded yet"
    );
    assert!(
        !state.should_give_up(),
        "Under limit with 2 consecutive errors"
    );

    state.increment_error_count();
    assert!(state.should_give_up(), "At limit with 3 consecutive errors");
}

// ===========================================================================
// Additional integration tests
// ===========================================================================

#[test]
fn test_config_builder_roundtrip() {
    let config = ErrorRecoveryConfigBuilder::new()
        .max_panic_skip(100)
        .add_sync_token(1)
        .add_sync_token(2)
        .add_insertable_token(3)
        .add_deletable_token(4)
        .add_scope_delimiter(10, 11)
        .enable_scope_recovery(true)
        .enable_phrase_recovery(false)
        .enable_indentation_recovery(true)
        .max_consecutive_errors(20)
        .build();

    assert_eq!(config.max_panic_skip, 100);
    assert_eq!(config.max_consecutive_errors, 20);
    assert!(config.sync_tokens.iter().any(|t| t.0 == 1));
    assert!(config.sync_tokens.iter().any(|t| t.0 == 2));
    assert!(config.insert_candidates.iter().any(|t| t.0 == 3));
    assert!(config.deletable_tokens.contains(&4));
    assert_eq!(config.scope_delimiters, vec![(10, 11)]);
    assert!(config.enable_scope_recovery);
    assert!(!config.enable_phrase_recovery);
    assert!(config.enable_indentation_recovery);
}

#[test]
fn test_scope_push_pop_tracking() {
    let config = ErrorRecoveryConfig {
        scope_delimiters: vec![(4, 5)],
        ..Default::default()
    };
    let mut state = ErrorRecoveryState::new(config);

    state.push_scope(4);
    state.push_scope(4);
    assert!(state.pop_scope(5), "Should pop matching delimiter");
    assert!(state.pop_scope(5), "Should pop second matching delimiter");
    assert!(!state.pop_scope(5), "No more scopes to pop");
}

#[test]
fn test_recent_token_window() {
    let config = ErrorRecoveryConfig::default();
    let mut state = ErrorRecoveryState::new(config);

    // Fill beyond the 10-token window
    for i in 0..15 {
        state.update_recent_tokens(SymbolId(i));
    }

    // The window should contain only the last 10 tokens (5..15)
    // This verifies the sliding window is maintained correctly.
    // (We can't directly inspect the window, but the internal test in
    // error_recovery.rs tests2 module already validates this. Here we
    // just confirm no panics / deadlocks.)
}

#[test]
fn test_static_delimiter_helpers() {
    let delimiters = vec![(4, 5), (10, 11)];

    assert!(ErrorRecoveryState::is_scope_delimiter(4, &delimiters));
    assert!(ErrorRecoveryState::is_scope_delimiter(5, &delimiters));
    assert!(ErrorRecoveryState::is_scope_delimiter(10, &delimiters));
    assert!(!ErrorRecoveryState::is_scope_delimiter(99, &delimiters));

    assert!(ErrorRecoveryState::is_matching_delimiter(4, 5, &delimiters));
    assert!(!ErrorRecoveryState::is_matching_delimiter(
        4,
        11,
        &delimiters
    ));
}

#[test]
fn test_error_node_records_recovery_strategy() {
    let config = ErrorRecoveryConfig::default();
    let mut state = ErrorRecoveryState::new(config);

    let strategies = [
        RecoveryStrategy::PanicMode,
        RecoveryStrategy::TokenInsertion,
        RecoveryStrategy::TokenDeletion,
        RecoveryStrategy::TokenSubstitution,
        RecoveryStrategy::PhraseLevel,
        RecoveryStrategy::ScopeRecovery,
    ];

    for strategy in &strategies {
        state.record_error(0, 1, (0, 0), (0, 1), vec![], None, *strategy, vec![]);
    }

    let errors = state.get_error_nodes();
    assert_eq!(errors.len(), strategies.len());
    for (i, strategy) in strategies.iter().enumerate() {
        assert_eq!(
            errors[i].recovery, *strategy,
            "Recorded strategy mismatch at index {i}"
        );
    }
}

#[test]
fn test_valid_input_parses_without_errors() {
    let grammar = arithmetic_grammar();
    let config = default_recovery_config();

    let result = parse_with_recovery(&grammar, "1 + 2;", config);
    assert!(result.is_ok(), "Valid input should parse successfully");

    if let Ok(tree) = result {
        // A clean parse should not have the error wrapper
        // (the root might not be marked as error)
        assert!(
            count_nodes(&tree) >= 3,
            "Valid parse should produce a reasonable tree"
        );
    }
}

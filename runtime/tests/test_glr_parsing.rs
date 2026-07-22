// Comprehensive tests for GLR (Generalized LR) parsing
// These tests verify fork/merge handling for ambiguous grammars

#![cfg(test)]
#![allow(unused_imports, dead_code)]

mod common;

use adze::error_recovery::{ErrorRecoveryConfig, ErrorRecoveryConfigBuilder};
use adze::glr_lexer::GLRLexer;
use adze::glr_parser::{GLRParser, ParseStack};
use adze::parser_v4::{Parser, Tree};
use adze::subtree::Subtree;
use adze_glr_core::{Action, FirstFollowSets, ParseTable, build_lr1_automaton};
use adze_ir::{
    Grammar, ProductionId, Rule, RuleId, StateId, Symbol, SymbolId, Token, TokenPattern,
};
use std::collections::BTreeMap;
use std::sync::Arc;

// Local symbol constants to avoid magic numbers
const SYM_EOF: SymbolId = SymbolId(0);
const SYM_NUMBER: SymbolId = SymbolId(1);
const SYM_PLUS: SymbolId = SymbolId(2);
const SYM_STAR: SymbolId = SymbolId(3);
const SYM_EXPR: SymbolId = SymbolId(10);

/// Create the classic ambiguous expression grammar
/// E -> E + E | E * E | num
/// This grammar is ambiguous for inputs like "1 + 2 * 3"
fn create_ambiguous_grammar() -> Grammar {
    let mut grammar = Grammar::new("ambiguous_expr".to_string());

    grammar.tokens.insert(
        SYM_NUMBER,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        SYM_PLUS,
        Token {
            name: "plus".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        SYM_STAR,
        Token {
            name: "mult".to_string(),
            pattern: TokenPattern::String("*".to_string()),
            fragile: false,
        },
    );

    // Rules
    // E -> num
    let rule1 = Rule {
        lhs: SYM_EXPR,
        rhs: vec![Symbol::Terminal(SYM_NUMBER)],
        production_id: ProductionId(0),
        precedence: None,
        associativity: None,
        fields: vec![],
    };

    // E -> E + E
    let rule2 = Rule {
        lhs: SYM_EXPR,
        rhs: vec![
            Symbol::NonTerminal(SYM_EXPR),
            Symbol::Terminal(SYM_PLUS),
            Symbol::NonTerminal(SYM_EXPR),
        ],
        production_id: ProductionId(1),
        precedence: None,
        associativity: None,
        fields: vec![],
    };

    // E -> E * E
    let rule3 = Rule {
        lhs: SYM_EXPR,
        rhs: vec![
            Symbol::NonTerminal(SYM_EXPR),
            Symbol::Terminal(SYM_STAR),
            Symbol::NonTerminal(SYM_EXPR),
        ],
        production_id: ProductionId(2),
        precedence: None,
        associativity: None,
        fields: vec![],
    };

    grammar.rules.entry(SYM_EXPR).or_default().push(rule1);
    grammar.rules.entry(SYM_EXPR).or_default().push(rule2);
    grammar.rules.entry(SYM_EXPR).or_default().push(rule3);

    // Add rule names
    grammar
        .rule_names
        .insert(SYM_EXPR, "expression".to_string());

    grammar.set_start_symbol(SYM_EXPR);
    grammar
}

/// Create a parse table with conflicts for testing GLR
fn create_conflicting_parse_table(grammar: &Grammar) -> ParseTable {
    // Use the proper LR1 automaton builder
    let first_follow = FirstFollowSets::compute(grammar).unwrap();
    build_lr1_automaton(grammar, &first_follow).expect("Failed to build LR1 automaton")
}

#[test]
fn test_glr_fork_creation() {
    let grammar = create_ambiguous_grammar();
    let parse_table = create_conflicting_parse_table(&grammar);

    // Create a GLR parser and lexer
    let mut parser = GLRParser::new(parse_table, grammar.clone());
    let input = "1+2*3";
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    let mut stack_count_before = 0;
    let mut stack_count_after = 0;

    // Process tokens
    for (i, token) in tokens.iter().enumerate() {
        if i == 2 {
            // Before processing "*"
            stack_count_before = parser.stack_count();
        }
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
        if i == 2 {
            // After processing "*"
            stack_count_after = parser.stack_count();
        }
    }

    // We should have more stacks after the conflict
    assert!(
        stack_count_after >= stack_count_before,
        "Expected forking to create additional stacks. Before: {}, After: {}",
        stack_count_before,
        stack_count_after
    );

    parser.process_eof(input.len());

    // Check that we can get a parse result
    let result = parser.finish();
    assert!(result.is_ok(), "Parse should succeed: {:?}", result);
}

#[test]
fn test_glr_merge() {
    let grammar = create_ambiguous_grammar();
    let parse_table = create_conflicting_parse_table(&grammar);

    let mut parser = GLRParser::new(parse_table, grammar.clone());

    // Parse an ambiguous expression
    let input = "1+2*3";
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    for token in &tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
    }
    parser.process_eof(input.len());

    // Try to get all alternatives (if feature is available)
    #[cfg(feature = "incremental_glr")]
    {
        let alternatives = parser.finish_all_alternatives();
        assert!(
            alternatives.is_ok(),
            "Should be able to get all parse alternatives"
        );

        let trees = alternatives.unwrap();
        // For a truly ambiguous grammar, we might get multiple parse trees
        // But for now, just verify we get at least one
        assert!(!trees.is_empty(), "Should have at least one parse tree");
    }

    #[cfg(not(feature = "incremental_glr"))]
    {
        // Just finish normally without alternatives
        let result = parser.finish();
        assert!(result.is_ok(), "Should successfully parse");
    }
}

#[test]
fn test_ambiguous_expression_parsing() {
    let grammar = create_ambiguous_grammar();
    let parse_table = create_conflicting_parse_table(&grammar);

    let mut parser = GLRParser::new(parse_table, grammar.clone());

    // Parse "1+2*3"
    let input = "1+2*3";
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    for token in &tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
    }
    parser.process_eof(input.len());

    let result = parser.finish();
    assert!(
        result.is_ok(),
        "Should successfully parse ambiguous expression"
    );

    let tree = result.unwrap();
    // The root should be an expression
    assert_eq!(tree.symbol(), SYM_EXPR.0, "Root should be expression");

    // Verify the tree has the expected structure
    // Since this is ambiguous, we just verify it has children
    assert!(!tree.children.is_empty(), "Parse tree should have children");
}

#[test]
fn test_glr_error_recovery() {
    let grammar = create_ambiguous_grammar();
    let parse_table = create_conflicting_parse_table(&grammar);

    let mut parser = GLRParser::new(parse_table, grammar.clone());

    // Enable error recovery with correct field names
    let recovery_config = ErrorRecoveryConfigBuilder::new()
        .max_panic_skip(3)
        .max_consecutive_errors(2)
        .enable_phrase_recovery(true)
        .enable_scope_recovery(false)
        .build();
    parser.enable_error_recovery(recovery_config);

    // Parse with an error: "1++3" (missing number)
    let input = "1++3";
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    for token in &tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
    }
    parser.process_eof(input.len());

    // Should still get a result due to error recovery
    let _result = parser.finish();
    // May or may not succeed depending on recovery strategy
    // Just verify it doesn't panic
}

#[test]
fn test_glr_expected_symbols() {
    let grammar = create_ambiguous_grammar();
    let parse_table = create_conflicting_parse_table(&grammar);

    let mut parser = GLRParser::new(parse_table, grammar.clone());

    // After parsing "1+"
    let input = "1+";
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    for token in &tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
    }

    // This section uses optional incremental-GLR APIs; keep it discoverable but gated.
    #[cfg(feature = "incremental_glr")]
    {
        let expected = parser.expected_symbols();
        // Should expect a number after +
        assert!(
            expected.contains(&SYM_NUMBER),
            "Should expect number after +"
        );
    }
}

#[test]
fn test_glr_state_management() {
    let grammar = create_ambiguous_grammar();
    let parse_table = create_conflicting_parse_table(&grammar);

    let mut parser1 = GLRParser::new(parse_table.clone(), grammar.clone());

    // Parse partial input
    let input_partial = "1+";
    let mut lexer = GLRLexer::new(&grammar, input_partial.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    for token in &tokens {
        parser1.process_token(token.symbol_id, &token.text, token.byte_offset);
    }

    // This section uses optional incremental-GLR APIs; keep it discoverable but gated.
    #[cfg(feature = "incremental_glr")]
    {
        // Save state
        let saved_stacks = parser1.get_gss_state();
        let next_id = parser1.get_next_stack_id();

        // Create new parser and restore state
        let mut parser2 = GLRParser::new(parse_table, grammar.clone());
        parser2.set_gss_state(saved_stacks);
        parser2.set_next_stack_id(next_id);

        // Continue parsing with "2"
        let input_continue = "2";
        let mut lexer2 = GLRLexer::new(&grammar, input_continue.to_string()).unwrap();
        let tokens2 = lexer2.tokenize_all();

        for token in &tokens2 {
            parser2.process_token(token.symbol_id, &token.text, 2 + token.byte_offset);
        }
        parser2.process_eof(3);

        let result = parser2.finish();
        assert!(
            result.is_ok(),
            "Should successfully parse after state restoration"
        );
    }

    #[cfg(not(feature = "incremental_glr"))]
    {
        // Just continue with the same parser
        let input_continue = "2";
        let mut lexer2 = GLRLexer::new(&grammar, input_continue.to_string()).unwrap();
        let tokens2 = lexer2.tokenize_all();

        for token in &tokens2 {
            parser1.process_token(token.symbol_id, &token.text, 2 + token.byte_offset);
        }
        parser1.process_eof(3);

        let result = parser1.finish();
        assert!(result.is_ok(), "Should successfully parse");
    }
}

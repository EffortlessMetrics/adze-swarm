// Test error recovery in GLR parser

use adze::error_recovery::{ErrorRecoveryConfig, ErrorRecoveryConfigBuilder};
use adze::glr_lexer::GLRLexer;
use adze::glr_parser::GLRParser;
use adze::subtree::Subtree;
use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
use std::sync::Arc;

fn create_test_grammar() -> Grammar {
    let mut grammar = Grammar::new("error_test".to_string());

    // Tokens
    let num_id = SymbolId(1);
    let plus_id = SymbolId(2);
    let lparen_id = SymbolId(3);
    let rparen_id = SymbolId(4);
    let semicolon_id = SymbolId(5);

    grammar.tokens.insert(
        num_id,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex("[0-9]+".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        plus_id,
        Token {
            name: "plus".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        lparen_id,
        Token {
            name: "lparen".to_string(),
            pattern: TokenPattern::String("(".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        rparen_id,
        Token {
            name: "rparen".to_string(),
            pattern: TokenPattern::String(")".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        semicolon_id,
        Token {
            name: "semicolon".to_string(),
            pattern: TokenPattern::String(";".to_string()),
            fragile: false,
        },
    );

    // Non-terminals
    let expr_id = SymbolId(10);
    let stmt_id = SymbolId(11);

    grammar.rule_names.insert(expr_id, "expression".to_string());
    grammar.rule_names.insert(stmt_id, "statement".to_string());

    // Rules:
    // statement → expression ';'
    grammar
        .rules
        .entry(stmt_id) // Key should be the LHS non-terminal
        .or_default()
        .push(Rule {
            lhs: stmt_id,
            rhs: vec![Symbol::NonTerminal(expr_id), Symbol::Terminal(semicolon_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(0),
            fields: vec![],
        });

    // expression → expression '+' expression
    grammar
        .rules
        .entry(expr_id) // Key should be the LHS non-terminal
        .or_default()
        .push(Rule {
            lhs: expr_id,
            rhs: vec![
                Symbol::NonTerminal(expr_id),
                Symbol::Terminal(plus_id),
                Symbol::NonTerminal(expr_id),
            ],
            precedence: None,
            associativity: None,
            production_id: ProductionId(1),
            fields: vec![],
        });

    // expression → '(' expression ')'
    grammar
        .rules
        .entry(expr_id) // Key should be the LHS non-terminal
        .or_default()
        .push(Rule {
            lhs: expr_id,
            rhs: vec![
                Symbol::Terminal(lparen_id),
                Symbol::NonTerminal(expr_id),
                Symbol::Terminal(rparen_id),
            ],
            precedence: None,
            associativity: None,
            production_id: ProductionId(2),
            fields: vec![],
        });

    // expression → number
    grammar
        .rules
        .entry(expr_id) // Key should be the LHS non-terminal
        .or_default()
        .push(Rule {
            lhs: expr_id,
            rhs: vec![Symbol::Terminal(num_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(3),
            fields: vec![],
        });

    grammar.set_start_symbol(stmt_id);
    grammar
}

fn parse_with_recovery(
    grammar: &Grammar,
    input: &str,
    config: ErrorRecoveryConfig,
) -> Option<Arc<Subtree>> {
    // Generate parse table
    let first_follow = FirstFollowSets::compute(grammar).unwrap();
    let table = build_lr1_automaton(grammar, &first_follow).unwrap();

    // Create parser with error recovery
    let mut parser = GLRParser::new(table, grammar.clone());
    parser.enable_error_recovery(config);

    // Tokenize
    let mut lexer = GLRLexer::new(grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    // Parse
    parser.reset();
    for token in &tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
    }
    let total_bytes = tokens
        .last()
        .map(|t| t.byte_offset + t.text.len())
        .unwrap_or(0);
    parser.process_eof(total_bytes);
    let result = parser.finish();
    result.ok()
}

fn has_error_nodes(tree: &Subtree) -> bool {
    if tree.node.is_error {
        return true;
    }
    tree.children
        .iter()
        .any(|edge| has_error_nodes(&edge.subtree))
}

#[test]
fn test_missing_semicolon_recovery() {
    let grammar = create_test_grammar();

    // Create recovery config that can insert semicolons
    let config = ErrorRecoveryConfigBuilder::new()
        .add_insertable_token(5) // semicolon
        .add_sync_token(5) // semicolon is also a sync token
        .build();

    // Test input missing semicolon
    let tree = parse_with_recovery(&grammar, "1 + 2", config);
    assert!(tree.is_some(), "Failed to parse with missing semicolon");

    // Check that an error node was created
    if let Some(tree) = tree {
        assert!(has_error_nodes(&tree), "Expected error nodes in parse tree");
    }
}

#[test]
fn test_unmatched_parentheses_recovery() {
    let grammar = create_test_grammar();

    // Create recovery config
    let config = ErrorRecoveryConfigBuilder::new()
        .add_insertable_token(4) // rparen
        .add_scope_delimiter(3, 4) // lparen, rparen
        .enable_scope_recovery(true)
        .build();

    // Test input with missing closing paren
    let tree = parse_with_recovery(&grammar, "(1 + 2;", config.clone());
    assert!(tree.is_some(), "Failed to parse with unmatched parentheses");

    // Test input with extra closing paren
    let tree2 = parse_with_recovery(&grammar, "1 + 2);", config);
    assert!(tree2.is_some(), "Failed to parse with extra closing paren");
}

#[test]
fn test_token_deletion_recovery() {
    let grammar = create_test_grammar();

    // Create recovery config
    let config = ErrorRecoveryConfigBuilder::new()
        .max_consecutive_errors(5)
        .build();

    // Test input with garbage tokens
    let tree = parse_with_recovery(&grammar, "1 @ + # 2;", config);
    assert!(tree.is_some(), "Failed to parse with garbage tokens");
}

#[test]
fn test_panic_mode_recovery() {
    let grammar = create_test_grammar();

    // Create recovery config with sync tokens
    let config = ErrorRecoveryConfigBuilder::new()
        .max_panic_skip(10)
        .add_sync_token(5) // semicolon
        .build();

    // Test input with multiple errors
    let tree = parse_with_recovery(&grammar, "1 + @ # $ 2 + 3;", config);
    assert!(tree.is_some(), "Failed to parse with panic mode recovery");
}

#[test]
fn test_complex_error_recovery() {
    let grammar = create_test_grammar();

    // Create comprehensive recovery config
    let config = ErrorRecoveryConfigBuilder::new()
        .add_insertable_token(4) // rparen
        .add_insertable_token(5) // semicolon
        .add_sync_token(5) // semicolon
        .add_scope_delimiter(3, 4) // lparen, rparen
        .enable_scope_recovery(true)
        .enable_phrase_recovery(true)
        .max_consecutive_errors(10)
        .build();

    // Test various error scenarios
    let test_cases = vec![
        "1 + + 2;",    // Double operator
        "(1 + 2;",     // Missing closing paren
        "1 2 + 3;",    // Missing operator
        "1 + (2 + 3;", // Missing closing paren in nested expr
        "1 + ;",       // Missing operand
        "+ 1 + 2;",    // Leading operator
    ];

    for input in test_cases {
        let tree = parse_with_recovery(&grammar, input, config.clone());
        assert!(
            tree.is_some(),
            "Failed to parse '{}' with error recovery",
            input
        );
    }
}

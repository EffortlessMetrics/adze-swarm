// Simple test for GLR error recovery

use adze::error_recovery::ErrorRecoveryConfigBuilder;
use adze::glr_lexer::GLRLexer;
use adze::glr_parser::GLRParser;
use adze::subtree::Subtree;
use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

fn create_simple_grammar() -> Grammar {
    let mut grammar = Grammar::new("simple".to_string());

    // Tokens
    let num_id = SymbolId(1);
    let plus_id = SymbolId(2);

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

    // Non-terminal
    let expr_id = SymbolId(10);
    grammar.rule_names.insert(expr_id, "expression".to_string());

    // Rules:
    // expression → expression '+' expression
    grammar
        .rules
        .entry(expr_id) // Rules are grouped by LHS
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
            production_id: ProductionId(0),
            fields: vec![],
        });

    // expression → number
    grammar
        .rules
        .entry(expr_id) // Rules are grouped by LHS
        .or_default()
        .push(Rule {
            lhs: expr_id,
            rhs: vec![Symbol::Terminal(num_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(1),
            fields: vec![],
        });

    grammar.set_start_symbol(expr_id);
    grammar
}

#[test]
fn test_basic_parsing_without_errors() {
    let grammar = create_simple_grammar();

    // Generate parse table
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let table = build_lr1_automaton(&grammar, &first_follow).unwrap();

    // Create parser without error recovery
    let mut parser = GLRParser::new(table, grammar.clone());

    // Tokenize valid input
    let input = "1 + 2";
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    // Parse
    parser.reset();
    for token in &tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
    }
    parser.process_eof(input.len());
    let result = parser.finish();

    if let Err(e) = &result {
        println!("Parse error: {:?}", e);
    }
    assert!(result.is_ok(), "Failed to parse valid input '1 + 2'");
}

#[test]
fn test_error_recovery_double_operator() {
    let grammar = create_simple_grammar();

    // Generate parse table
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let table = build_lr1_automaton(&grammar, &first_follow).unwrap();

    // Create parser with error recovery
    let mut parser = GLRParser::new(table, grammar.clone());
    let config = ErrorRecoveryConfigBuilder::new()
        .max_consecutive_errors(10)
        .build();
    parser.enable_error_recovery(config);

    // Tokenize input with error (double plus)
    let input = "1 + + 2";
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    println!(
        "Tokens: {:?}",
        tokens
            .iter()
            .map(|t| (t.symbol_id, &t.text))
            .collect::<Vec<_>>()
    );

    // Parse
    parser.reset();
    for token in &tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
    }
    parser.process_eof(input.len());
    let result = parser.finish();

    println!("Parse result: {:?}", result.is_ok());
    assert!(result.is_ok(), "Failed to parse with error recovery");

    if let Ok(tree) = result {
        assert!(
            has_error_nodes(&tree),
            "Expected error nodes in recovered parse"
        );
    }
}

fn has_error_nodes(tree: &Subtree) -> bool {
    if tree.node.is_error {
        return true;
    }
    tree.children
        .iter()
        .any(|edge| has_error_nodes(&edge.subtree))
}

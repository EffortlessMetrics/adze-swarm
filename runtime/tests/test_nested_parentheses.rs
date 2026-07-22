use adze::glr_lexer::{GLRLexer, TokenWithPosition};
use adze::glr_parser::GLRParser;
use adze::subtree::Subtree;
// Test for nested parentheses issue in GLR parser

use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::{
    Associativity, Grammar, PrecedenceKind, ProductionId, Rule, Symbol, SymbolId, Token,
    TokenPattern,
};

// Import internal modules for testing
use std::sync::Arc;

/// Create a simple expression grammar for testing
fn create_expression_grammar() -> Grammar {
    let mut grammar = Grammar::new("expression".to_string());

    // Define terminals (SymbolId(0) is reserved for EOF)
    let number_id = SymbolId(1);
    grammar.tokens.insert(
        number_id,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );

    let plus_id = SymbolId(2);
    grammar.tokens.insert(
        plus_id,
        Token {
            name: "plus".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );

    let lparen_id = SymbolId(6);
    grammar.tokens.insert(
        lparen_id,
        Token {
            name: "lparen".to_string(),
            pattern: TokenPattern::String("(".to_string()),
            fragile: false,
        },
    );

    let rparen_id = SymbolId(7);
    grammar.tokens.insert(
        rparen_id,
        Token {
            name: "rparen".to_string(),
            pattern: TokenPattern::String(")".to_string()),
            fragile: false,
        },
    );

    // Define non-terminals
    let expr_id = SymbolId(10);
    grammar.rule_names.insert(expr_id, "expression".to_string());

    // Rules
    let _add_rule_id = SymbolId(20);
    let _paren_rule_id = SymbolId(24);
    let _number_rule_id = SymbolId(25);

    // expression → expression + expression
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![
            Symbol::NonTerminal(expr_id),
            Symbol::Terminal(plus_id),
            Symbol::NonTerminal(expr_id),
        ],
        precedence: Some(PrecedenceKind::Static(1)),
        associativity: Some(Associativity::Left),
        production_id: ProductionId(0),
        fields: vec![],
    });

    // expression → ( expression )
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![
            Symbol::Terminal(lparen_id),
            Symbol::NonTerminal(expr_id),
            Symbol::Terminal(rparen_id),
        ],
        precedence: None,
        associativity: None,
        production_id: ProductionId(1),
        fields: vec![],
    });

    // expression → number
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![Symbol::Terminal(number_id)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(2),
        fields: vec![],
    });

    grammar.set_start_symbol(expr_id);
    grammar
}

fn parse_tokens(parser: &mut GLRParser, tokens: &[TokenWithPosition]) -> Option<Arc<Subtree>> {
    parser.reset();

    let mut total_bytes = 0;
    for token in tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
        total_bytes = token.byte_offset + token.text.len();
    }

    parser.process_eof(total_bytes);
    parser.finish().ok()
}

fn print_tree(tree: &Arc<Subtree>, indent: usize) -> String {
    let mut result = String::new();
    let spaces = " ".repeat(indent);
    result.push_str(&format!(
        "{}Symbol: {:?}, Range: {:?}\n",
        spaces, tree.node.symbol_id, tree.node.byte_range
    ));
    for edge in &tree.children {
        result.push_str(&print_tree(&edge.subtree, indent + 2));
    }
    result
}

#[test]
fn test_simple_parentheses() {
    let grammar = create_expression_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();
    let mut parser = GLRParser::new(parse_table, grammar.clone());

    // Test single parentheses
    let input = "(1)";
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    println!(
        "Tokens for '{}': {:?}",
        input,
        tokens
            .iter()
            .map(|t| (t.symbol_id, &t.text))
            .collect::<Vec<_>>()
    );

    let tree = parse_tokens(&mut parser, &tokens);
    assert!(tree.is_some(), "Failed to parse '{}'", input);

    if let Some(tree) = tree {
        println!("Parse tree:\n{}", print_tree(&tree, 0));
    }
}

#[test]
fn test_double_parentheses() {
    let grammar = create_expression_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();
    let mut parser = GLRParser::new(parse_table, grammar.clone());

    // Test double parentheses
    let input = "((1))";
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    println!(
        "Tokens for '{}': {:?}",
        input,
        tokens
            .iter()
            .map(|t| (t.symbol_id, &t.text))
            .collect::<Vec<_>>()
    );

    let tree = parse_tokens(&mut parser, &tokens);
    assert!(tree.is_some(), "Failed to parse '{}'", input);

    if let Some(tree) = tree {
        println!("Parse tree:\n{}", print_tree(&tree, 0));
    }
}

#[test]
fn test_triple_parentheses() {
    let grammar = create_expression_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();
    let mut parser = GLRParser::new(parse_table, grammar.clone());

    // Test triple parentheses
    let input = "(((1)))";
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    println!(
        "Tokens for '{}': {:?}",
        input,
        tokens
            .iter()
            .map(|t| (t.symbol_id, &t.text))
            .collect::<Vec<_>>()
    );

    let tree = parse_tokens(&mut parser, &tokens);
    assert!(tree.is_some(), "Failed to parse '{}'", input);

    if let Some(tree) = tree {
        println!("Parse tree:\n{}", print_tree(&tree, 0));
    }
}

#[test]
fn test_nested_parentheses_with_expression() {
    let grammar = create_expression_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();
    let mut parser = GLRParser::new(parse_table, grammar.clone());

    // Test cases for nested parentheses
    let test_cases = vec![
        ("1", "single number"),
        ("(1)", "single parentheses"),
        ("((1))", "double parentheses"),
        ("(((1)))", "triple parentheses"),
        ("((((1))))", "quadruple parentheses"),
        ("1 + 2", "simple addition"),
        ("(1 + 2)", "parenthesized addition"),
        ("((1 + 2))", "double parenthesized addition"),
        ("(((1 + 2)))", "triple parenthesized addition"),
        ("((1) + (2))", "parenthesized operands"),
        ("(((1)) + ((2)))", "deeply parenthesized operands"),
    ];

    for (input, description) in test_cases {
        println!("\n\nTesting: {} - {}", input, description);

        let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
        let tokens = lexer.tokenize_all();

        println!(
            "Tokens: {:?}",
            tokens
                .iter()
                .map(|t| (t.symbol_id, &t.text))
                .collect::<Vec<_>>()
        );

        match parse_tokens(&mut parser, &tokens) {
            Some(tree) => {
                println!("✓ Parse succeeded for '{}'", input);
                println!("Parse tree:\n{}", print_tree(&tree, 0));
            }
            None => {
                panic!("✗ Parse FAILED for '{}' ({})", input, description);
            }
        }
    }
}

#[test]
fn test_deeply_nested_parentheses() {
    let grammar = create_expression_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();
    let mut parser = GLRParser::new(parse_table, grammar.clone());

    // Test very deep nesting
    let depths = vec![1, 2, 3, 4, 5, 10, 20];

    for depth in depths {
        let mut input = String::new();
        for _ in 0..depth {
            input.push('(');
        }
        input.push('1');
        for _ in 0..depth {
            input.push(')');
        }

        println!("\n\nTesting depth {}: {}", depth, input);

        let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
        let tokens = lexer.tokenize_all();

        println!("Number of tokens: {}", tokens.len());

        match parse_tokens(&mut parser, &tokens) {
            Some(tree) => {
                println!("✓ Parse succeeded for depth {}", depth);
                // Count the depth in the tree
                let mut max_depth = 0;
                count_depth(&tree, 0, &mut max_depth);
                println!("Tree depth: {}", max_depth);
            }
            None => {
                panic!("✗ Parse FAILED for depth {}", depth);
            }
        }
    }
}

fn count_depth(tree: &Arc<Subtree>, current_depth: usize, max_depth: &mut usize) {
    if current_depth > *max_depth {
        *max_depth = current_depth;
    }
    for edge in &tree.children {
        count_depth(&edge.subtree, current_depth + 1, max_depth);
    }
}

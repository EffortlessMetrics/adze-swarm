//! Stress tests for GLR parser with deeply ambiguous grammars
//!
//! These tests verify that the GLR parser can handle complex ambiguous grammars
//! without exponential blowup or incorrect behavior.

use adze::glr_lexer::GLRLexer;
use adze::glr_parser::GLRParser;
use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

/// Create a deeply ambiguous expression grammar
/// This grammar has multiple ways to parse the same expression
fn create_ambiguous_expression_grammar() -> Grammar {
    let mut grammar = Grammar::new("ambiguous_expr".to_string());

    // Tokens
    let num_id = SymbolId(1);
    let plus_id = SymbolId(2);
    let times_id = SymbolId(3);
    let lparen_id = SymbolId(4);
    let rparen_id = SymbolId(5);

    // Non-terminals
    let start_id = SymbolId(10); // Start symbol (like DOCUMENT in JSON)
    let expr_id = SymbolId(11);
    let term_id = SymbolId(12);
    let factor_id = SymbolId(13);

    // Define tokens
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
        times_id,
        Token {
            name: "times".to_string(),
            pattern: TokenPattern::String("*".to_string()),
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

    // Ambiguous grammar rules - both left and right associative versions
    let mut rule_id = 0;

    // expr -> expr + expr (left associative)
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![
            Symbol::NonTerminal(expr_id),
            Symbol::Terminal(plus_id),
            Symbol::NonTerminal(expr_id),
        ],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(rule_id),
    });
    rule_id += 1;

    // expr -> expr * expr (left associative)
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![
            Symbol::NonTerminal(expr_id),
            Symbol::Terminal(times_id),
            Symbol::NonTerminal(expr_id),
        ],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(rule_id),
    });
    rule_id += 1;

    // expr -> term
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![Symbol::NonTerminal(term_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(rule_id),
    });
    rule_id += 1;

    // term -> term + factor (different precedence level)
    grammar.rules.entry(term_id).or_default().push(Rule {
        lhs: term_id,
        rhs: vec![
            Symbol::NonTerminal(term_id),
            Symbol::Terminal(plus_id),
            Symbol::NonTerminal(factor_id),
        ],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(rule_id),
    });
    rule_id += 1;

    // term -> factor
    grammar.rules.entry(term_id).or_default().push(Rule {
        lhs: term_id,
        rhs: vec![Symbol::NonTerminal(factor_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(rule_id),
    });
    rule_id += 1;

    // factor -> ( expr )
    grammar.rules.entry(factor_id).or_default().push(Rule {
        lhs: factor_id,
        rhs: vec![
            Symbol::Terminal(lparen_id),
            Symbol::NonTerminal(expr_id),
            Symbol::Terminal(rparen_id),
        ],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(rule_id),
    });
    rule_id += 1;

    // factor -> num
    grammar.rules.entry(factor_id).or_default().push(Rule {
        lhs: factor_id,
        rhs: vec![Symbol::Terminal(num_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(rule_id),
    });

    // Add a start rule: start -> expr
    rule_id += 1; // Increment rule_id before using it
    grammar.rules.entry(start_id).or_default().push(Rule {
        lhs: start_id,
        rhs: vec![Symbol::NonTerminal(expr_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(rule_id),
    });

    // Set up rule names with proper start symbol
    grammar
        .rule_names
        .insert(start_id, "source_file".to_string());
    grammar.rule_names.insert(expr_id, "expr".to_string());
    grammar.rule_names.insert(term_id, "term".to_string());
    grammar.rule_names.insert(factor_id, "factor".to_string());

    grammar.set_start_symbol(start_id);
    grammar
}

/// Create a grammar with extreme ambiguity - every operator can be parsed multiple ways
fn create_extremely_ambiguous_grammar() -> Grammar {
    let mut grammar = Grammar::new("extreme_ambiguous".to_string());

    // Tokens
    let num_id = SymbolId(1);
    let op_id = SymbolId(2); // Single operator that can mean different things

    // Non-terminals
    let start_id = SymbolId(10); // Start symbol
    let expr_id = SymbolId(11);
    let expr2_id = SymbolId(12);
    let expr3_id = SymbolId(13);

    // Define tokens
    grammar.tokens.insert(
        num_id,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex("[0-9]+".to_string()),
            fragile: false,
        },
    );
    grammar.tokens.insert(
        op_id,
        Token {
            name: "op".to_string(),
            pattern: TokenPattern::String("@".to_string()),
            fragile: false,
        },
    );

    let mut rule_id = 0;

    // Multiple ways to parse expr @ expr
    // expr -> expr @ expr (version 1)
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![
            Symbol::NonTerminal(expr_id),
            Symbol::Terminal(op_id),
            Symbol::NonTerminal(expr_id),
        ],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(rule_id),
    });
    rule_id += 1;

    // expr -> expr2
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![Symbol::NonTerminal(expr2_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(rule_id),
    });
    rule_id += 1;

    // expr2 -> expr2 @ expr3 (version 2)
    grammar.rules.entry(expr2_id).or_default().push(Rule {
        lhs: expr2_id,
        rhs: vec![
            Symbol::NonTerminal(expr2_id),
            Symbol::Terminal(op_id),
            Symbol::NonTerminal(expr3_id),
        ],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(rule_id),
    });
    rule_id += 1;

    // expr2 -> expr3
    grammar.rules.entry(expr2_id).or_default().push(Rule {
        lhs: expr2_id,
        rhs: vec![Symbol::NonTerminal(expr3_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(rule_id),
    });
    rule_id += 1;

    // expr3 -> expr @ expr3 (version 3, right associative)
    grammar.rules.entry(expr3_id).or_default().push(Rule {
        lhs: expr3_id,
        rhs: vec![
            Symbol::NonTerminal(expr_id),
            Symbol::Terminal(op_id),
            Symbol::NonTerminal(expr3_id),
        ],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(rule_id),
    });
    rule_id += 1;

    // expr3 -> num
    grammar.rules.entry(expr3_id).or_default().push(Rule {
        lhs: expr3_id,
        rhs: vec![Symbol::Terminal(num_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(rule_id),
    });

    // All three expression types can also reduce to num directly
    rule_id += 1;
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![Symbol::Terminal(num_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(rule_id),
    });

    rule_id += 1;
    grammar.rules.entry(expr2_id).or_default().push(Rule {
        lhs: expr2_id,
        rhs: vec![Symbol::Terminal(num_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(rule_id),
    });

    // Add a start rule: start -> expr
    rule_id += 1;
    grammar.rules.entry(start_id).or_default().push(Rule {
        lhs: start_id,
        rhs: vec![Symbol::NonTerminal(expr_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(rule_id),
    });

    // Set up rule names with proper start symbol
    grammar
        .rule_names
        .insert(start_id, "source_file".to_string());
    grammar.rule_names.insert(expr_id, "expr".to_string());
    grammar.rule_names.insert(expr2_id, "expr2".to_string());
    grammar.rule_names.insert(expr3_id, "expr3".to_string());

    grammar.set_start_symbol(start_id);
    grammar
}

#[test]
fn test_deeply_nested_ambiguous_expression() {
    let grammar = create_ambiguous_expression_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();

    let mut parser = GLRParser::new(parse_table, grammar.clone());

    // Test deeply nested expression: ((1 + 2) * (3 + 4)) + 5
    let input = "((1 + 2) * (3 + 4)) + 5";
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    println!("Testing deeply nested expression: {}", input);
    println!("Tokens: {} tokens", tokens.len());

    for token in tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
    }

    parser.process_eof(input.len());

    let result = parser.finish();
    assert!(
        result.is_ok(),
        "Failed to parse deeply nested expression: {:?}",
        result
    );

    println!(
        "Successfully parsed deeply nested expression with {} active stacks",
        parser.stack_count()
    );
}

#[test]
fn test_extremely_ambiguous_parsing() {
    let grammar = create_extremely_ambiguous_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();

    let mut parser = GLRParser::new(parse_table, grammar.clone());

    // Test expression with multiple ambiguous parses: 1 @ 2 @ 3 @ 4
    let input = "1 @ 2 @ 3 @ 4";
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    println!("Testing extremely ambiguous expression: {}", input);
    println!("This can be parsed as:");
    println!("  ((1 @ 2) @ 3) @ 4  (left associative)");
    println!("  (1 @ (2 @ 3)) @ 4  (mixed)");
    println!("  1 @ ((2 @ 3) @ 4)  (mixed)");
    println!("  1 @ (2 @ (3 @ 4))  (right associative)");
    println!("  ... and many more!");

    for token in tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
        println!(
            "After token '{}': {} active stacks",
            token.text,
            parser.stack_count()
        );
    }

    parser.process_eof(input.len());

    let result = parser.finish();
    assert!(
        result.is_ok(),
        "Failed to parse extremely ambiguous expression: {:?}",
        result
    );

    println!(
        "Successfully parsed with final stack count: {}",
        parser.stack_count()
    );
}

#[test]
fn test_long_ambiguous_chain() {
    let grammar = create_ambiguous_expression_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();

    let mut parser = GLRParser::new(parse_table, grammar.clone());

    // Test a long chain: 1 + 2 * 3 + 4 * 5 + 6
    let input = "1 + 2 * 3 + 4 * 5 + 6";
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    println!("Testing long ambiguous chain: {}", input);

    let mut max_stacks = 0;
    for token in tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
        let stack_count = parser.stack_count();
        max_stacks = max_stacks.max(stack_count);
        println!(
            "After token '{}': {} active stacks",
            token.text, stack_count
        );
    }

    parser.process_eof(input.len());

    let result = parser.finish();
    assert!(
        result.is_ok(),
        "Failed to parse long ambiguous chain: {:?}",
        result
    );

    println!("Successfully parsed. Max concurrent stacks: {}", max_stacks);

    // Ensure we're handling ambiguity efficiently
    assert!(
        max_stacks < 50,
        "Too many concurrent stacks ({}), possible exponential blowup",
        max_stacks
    );
}

#[test]
fn test_stress_deeply_nested_parentheses() {
    let grammar = create_ambiguous_expression_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();

    let mut parser = GLRParser::new(parse_table, grammar.clone());

    // Create deeply nested parentheses
    let depth = 10;
    let mut input = String::new();
    for _ in 0..depth {
        input.push('(');
    }
    input.push('1');
    for i in 0..depth {
        input.push(')');
        if i < depth - 1 {
            input.push_str(" + ");
            input.push('(');
            for _ in 0..depth - i - 2 {
                input.push('(');
            }
            input.push('2');
            for _ in 0..depth - i - 2 {
                input.push(')');
            }
            input.push(')');
        }
    }

    println!("Testing {} levels of nesting", depth);
    let mut lexer = GLRLexer::new(&grammar, input.clone()).unwrap();
    let tokens = lexer.tokenize_all();

    for token in tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
    }

    parser.process_eof(input.len());

    let result = parser.finish();
    assert!(
        result.is_ok(),
        "Failed to parse deeply nested parentheses: {:?}",
        result
    );

    println!("Successfully parsed {} levels of nesting", depth);
}

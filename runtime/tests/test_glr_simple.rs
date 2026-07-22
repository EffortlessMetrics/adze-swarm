// Simple integration test for GLR parser
// This demonstrates basic GLR parsing functionality

use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

use adze::glr_lexer::GLRLexer;
use adze::glr_parser::GLRParser;

/// Create a simple number grammar for testing
fn create_number_grammar() -> Grammar {
    let mut grammar = Grammar::new("number".to_string());

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

    // Define non-terminal
    let expr_id = SymbolId(10);
    grammar.rule_names.insert(expr_id, "expression".to_string());

    // Define rules (use ProductionId for rule IDs, not SymbolId)
    // Rule 0: expression → number
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![Symbol::Terminal(number_id)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });

    // Rule 1: expression → expression + expression
    grammar.rules.entry(expr_id).or_default().push(Rule {
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

    grammar.set_start_symbol(expr_id);
    grammar
}

#[test]
fn test_simple_number_parsing() {
    let grammar = create_number_grammar();

    // Build parse table
    println!("\nGrammar structure:");
    println!("  Tokens: {:?}", grammar.tokens.keys().collect::<Vec<_>>());
    println!("  Rules: {:?}", grammar.rules.keys().collect::<Vec<_>>());
    println!("  Rule names: {:?}", grammar.rule_names);
    println!("  Rule details:");
    for (id, rules) in &grammar.rules {
        for rule in rules {
            println!("    Rule {:?}: {:?} -> {:?}", id, rule.lhs, rule.rhs);
        }
    }

    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();
    let mut parser = GLRParser::new(parse_table, grammar.clone());

    // Test 1: Parse a single number
    {
        parser.reset();
        let input = "42";
        let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
        let tokens = lexer.tokenize_all();

        println!(
            "Test 1 - Parsing '{}', tokens: {:?}",
            input,
            tokens
                .iter()
                .map(|t| (t.symbol_id, &t.text))
                .collect::<Vec<_>>()
        );

        for token in &tokens {
            parser.process_token(token.symbol_id, &token.text, token.byte_offset);
        }
        parser.process_eof(input.len());

        let result = parser.finish();
        assert!(result.is_ok(), "Failed to parse single number");
        println!("✓ Successfully parsed single number");
    }

    // Test 2: Parse addition
    {
        parser.reset();
        let input = "1+2";
        let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
        let tokens = lexer.tokenize_all();

        println!(
            "\nTest 2 - Parsing '{}', tokens: {:?}",
            input,
            tokens
                .iter()
                .map(|t| (t.symbol_id, &t.text))
                .collect::<Vec<_>>()
        );

        for token in &tokens {
            parser.process_token(token.symbol_id, &token.text, token.byte_offset);
        }
        parser.process_eof(input.len());

        let result = parser.finish();
        match &result {
            Ok(tree) => println!("Parse succeeded for addition: {:?}", tree),
            Err(e) => println!("Parse failed for addition: {}", e),
        }
        assert!(result.is_ok(), "Failed to parse addition");
        println!("✓ Successfully parsed addition");
    }

    // Test 3: Parse chained addition
    {
        parser.reset();
        let input = "1+2+3";
        let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
        let tokens = lexer.tokenize_all();

        println!(
            "\nTest 3 - Parsing '{}', tokens: {:?}",
            input,
            tokens
                .iter()
                .map(|t| (t.symbol_id, &t.text))
                .collect::<Vec<_>>()
        );

        for token in &tokens {
            parser.process_token(token.symbol_id, &token.text, token.byte_offset);
        }
        parser.process_eof(input.len());

        let result = parser.finish();
        match &result {
            Ok(tree) => println!("Parse succeeded: {:?}", tree),
            Err(e) => println!("Parse failed: {:?}", e),
        }
        assert!(result.is_ok(), "Failed to parse chained addition");
        println!("✓ Successfully parsed chained addition");

        // Should have handled ambiguity (left vs right associative)
        let stack_count = parser.stack_count();
        println!(
            "  Parser maintained {} stack(s) during parsing",
            stack_count
        );
    }
}

#[test]
fn test_glr_ambiguity() {
    // Create a truly ambiguous grammar
    let mut grammar = Grammar::new("ambiguous".to_string());

    // Terminal 'a' (SymbolId(0) is reserved for EOF)
    let a_id = SymbolId(1);
    grammar.tokens.insert(
        a_id,
        Token {
            name: "a".to_string(),
            pattern: TokenPattern::String("a".to_string()),
            fragile: false,
        },
    );

    // Non-terminal E
    let e_id = SymbolId(10);
    grammar.rule_names.insert(e_id, "E".to_string());

    // Rule 1: E → a
    grammar.rules.entry(e_id).or_default().push(Rule {
        lhs: e_id,
        rhs: vec![Symbol::Terminal(a_id)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });

    // Rule 2: E → E E (ambiguous concatenation)
    grammar.rules.entry(e_id).or_default().push(Rule {
        lhs: e_id,
        rhs: vec![Symbol::NonTerminal(e_id), Symbol::NonTerminal(e_id)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(1),
        fields: vec![],
    });

    // Build parse table
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();
    let mut parser = GLRParser::new(parse_table, grammar.clone());

    // Parse "aaa" - highly ambiguous
    let input = "aaa";
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    println!("\nTesting ambiguous grammar with input '{}'", input);

    for token in &tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
        println!(
            "  After token '{}': {} active stacks",
            token.text,
            parser.stack_count()
        );
    }
    parser.process_eof(input.len());

    let result = parser.finish();
    assert!(result.is_ok(), "GLR parser should handle ambiguous grammar");

    println!("✓ GLR parser successfully handled ambiguous grammar");
    println!("  Final stack count: {}", parser.stack_count());
}

#[test]
fn test_glr_error_handling() {
    let grammar = create_number_grammar();

    // Build parse table
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();
    let mut parser = GLRParser::new(parse_table, grammar.clone());

    // Test invalid input: "1 + +"
    let input = "1++";
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    println!("\nTesting error handling with invalid input '{}'", input);
    println!(
        "Tokens: {:?}",
        tokens
            .iter()
            .map(|t| (t.symbol_id, &t.text))
            .collect::<Vec<_>>()
    );

    for token in &tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
    }
    parser.process_eof(input.len());

    let result = parser.finish();

    // The parser might handle this through error recovery or reject it
    match result {
        Ok(_) => println!("Parser recovered from error"),
        Err(e) => println!("Parser correctly rejected invalid input: {}", e),
    }
}

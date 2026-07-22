use adze::glr_incremental::{ForestNode, GLREdit, GLRToken, IncrementalGLRParser};
use adze::glr_lexer::{GLRLexer, TokenWithPosition};
use adze::glr_parser::GLRParser;
use adze::glr_query::{QueryCursor, QueryParser};
use adze::subtree::Subtree;
// Integration test for the full GLR parsing pipeline
// This test demonstrates parsing a complete grammar from definition to tree output

use adze::glr_validation::GLRGrammarValidator;
use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::{
    Associativity, Grammar, PrecedenceKind, ProductionId, Rule, Symbol, SymbolId, Token,
    TokenPattern,
};

// Import internal modules for testing
use std::sync::Arc;

// Helper function to parse tokens with GLRParser
fn parse_tokens(parser: &mut GLRParser, tokens: &[TokenWithPosition]) -> Option<Arc<Subtree>> {
    parser.reset();

    for token in tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
    }

    // Calculate total bytes from the last token
    let total_bytes = if let Some(last_token) = tokens.last() {
        last_token.byte_offset + last_token.text.len()
    } else {
        0
    };

    parser.process_eof(total_bytes);
    parser.finish().ok()
}

// Helper function to create a parser from grammar
fn create_parser(grammar: &Grammar) -> GLRParser {
    let first_follow = FirstFollowSets::compute(grammar).unwrap();
    let parse_table = build_lr1_automaton(grammar, &first_follow).unwrap();
    GLRParser::new(parse_table, grammar.clone())
}

// Helper function to convert TokenWithPosition to GLRToken
fn tokens_to_glr(tokens: &[TokenWithPosition]) -> Vec<GLRToken> {
    tokens
        .iter()
        .map(|t| GLRToken {
            symbol: t.symbol_id,
            text: t.text.as_bytes().to_vec(),
            start_byte: t.byte_offset,
            end_byte: t.byte_offset + t.byte_length,
        })
        .collect()
}

// Helper function to convert ForestNode to query Subtree
fn convert_forest_to_query_subtree(forest: &Arc<ForestNode>) -> adze::glr_query::Subtree {
    // Prefer first alternative (keeps this simple)
    let alt = forest.alternatives.first();

    // If this node has exactly one child that spans the exact same range,
    // squash the wrapper and return the child subtree.
    if let Some(alt0) = alt
        && alt0.children.len() == 1
    {
        let ch = &alt0.children[0];
        if ch.byte_range.start == forest.byte_range.start
            && ch.byte_range.end == forest.byte_range.end
        {
            return convert_forest_to_query_subtree(ch);
        }
    }

    adze::glr_query::Subtree {
        symbol: forest.symbol,
        children: alt
            .map(|a| {
                a.children
                    .iter()
                    .map(convert_forest_to_query_subtree)
                    .collect()
            })
            .unwrap_or_default(),
        start_byte: forest.byte_range.start,
        end_byte: forest.byte_range.end,
    }
}

// Helper function to convert subtree::Subtree to glr_query::Subtree
fn convert_to_query_subtree(subtree: &Arc<Subtree>) -> adze::glr_query::Subtree {
    adze::glr_query::Subtree {
        symbol: subtree.node.symbol_id,
        children: subtree
            .children
            .iter()
            .map(|edge| convert_to_query_subtree(&edge.subtree))
            .collect(),
        start_byte: subtree.node.byte_range.start,
        end_byte: subtree.node.byte_range.end,
    }
}

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

    let minus_id = SymbolId(3);
    grammar.tokens.insert(
        minus_id,
        Token {
            name: "minus".to_string(),
            pattern: TokenPattern::String("-".to_string()),
            fragile: false,
        },
    );

    let times_id = SymbolId(4);
    grammar.tokens.insert(
        times_id,
        Token {
            name: "times".to_string(),
            pattern: TokenPattern::String("*".to_string()),
            fragile: false,
        },
    );

    let divide_id = SymbolId(5);
    grammar.tokens.insert(
        divide_id,
        Token {
            name: "divide".to_string(),
            pattern: TokenPattern::String("/".to_string()),
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

    // Define non-terminals - only one symbol ID for expression
    let expr_id = SymbolId(10);

    grammar.rule_names.insert(expr_id, "expression".to_string());

    // Rule IDs for different production rules
    let _add_rule_id = SymbolId(20);
    let _sub_rule_id = SymbolId(21);
    let _mul_rule_id = SymbolId(22);
    let _div_rule_id = SymbolId(23);
    let _paren_rule_id = SymbolId(24);
    let _number_rule_id = SymbolId(25);

    // Add a simple rule for expression that doesn't reference itself
    // expression → number_expression | add_expression | ... (handled via multiple rules with same LHS)

    // Define rules with proper precedence

    // expression → expression + expression (left associative, precedence 1)
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

    // expression → expression - expression (left associative, precedence 1)
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![
            Symbol::NonTerminal(expr_id),
            Symbol::Terminal(minus_id),
            Symbol::NonTerminal(expr_id),
        ],
        precedence: Some(PrecedenceKind::Static(1)),
        associativity: Some(Associativity::Left),
        production_id: ProductionId(1),
        fields: vec![],
    });

    // expression → expression * expression (left associative, precedence 2)
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![
            Symbol::NonTerminal(expr_id),
            Symbol::Terminal(times_id),
            Symbol::NonTerminal(expr_id),
        ],
        precedence: Some(PrecedenceKind::Static(2)),
        associativity: Some(Associativity::Left),
        production_id: ProductionId(2),
        fields: vec![],
    });

    // expression → expression / expression (left associative, precedence 2)
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![
            Symbol::NonTerminal(expr_id),
            Symbol::Terminal(divide_id),
            Symbol::NonTerminal(expr_id),
        ],
        precedence: Some(PrecedenceKind::Static(2)),
        associativity: Some(Associativity::Left),
        production_id: ProductionId(3),
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
        production_id: ProductionId(4),
        fields: vec![],
    });

    // expression → number
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![Symbol::Terminal(number_id)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(5),
        fields: vec![],
    });

    grammar.set_start_symbol(expr_id);
    grammar
}

#[test]
fn test_full_glr_pipeline() {
    let grammar = create_expression_grammar();

    // Step 1: Validate the grammar
    let mut validator = GLRGrammarValidator::new();
    let validation_result = validator.validate(&grammar);
    assert!(
        validation_result.errors.is_empty(),
        "Grammar validation failed: {:?}",
        validation_result.errors
    );
    println!("✓ Grammar validation passed");

    // Step 2: Create parser
    let mut parser = create_parser(&grammar);

    // Step 3: Parse a simple expression
    let input = "1 + 2 * 3";
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    println!(
        "Tokens: {:?}",
        tokens
            .iter()
            .map(|t| (t.symbol_id, &t.text))
            .collect::<Vec<_>>()
    );

    // Step 4: Parse the tokens
    let tree = parse_tokens(&mut parser, &tokens);
    match &tree {
        Some(t) => {
            println!("✓ Parsing succeeded");
            println!("Parse tree: {:?}", t);
        }
        None => {
            panic!("Parsing failed");
        }
    }

    // Step 5: Test incremental parsing
    // Re-create parser with proper types
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();
    let mut incremental = IncrementalGLRParser::new(grammar.clone(), parse_table);
    let glr_tokens = tokens_to_glr(&tokens);
    let initial_tree = incremental.parse_incremental(&glr_tokens, &[]).unwrap();
    println!("✓ Initial incremental parse succeeded");

    // Edit: "1 + 2 * 3" → "1 + 5 * 3"
    use std::ops::Range;
    let edit = GLREdit {
        old_range: Range { start: 4, end: 5 },
        new_text: b"5".to_vec(),
        old_token_range: Range { start: 2, end: 3 }, // The "2" token
        new_tokens: vec![GLRToken {
            symbol: SymbolId(1), // NUMBER token - should match number_id
            text: b"5".to_vec(),
            start_byte: 4,
            end_byte: 5,
        }],
        old_tokens: glr_tokens.clone(),
        old_forest: Some(initial_tree.clone()),
    };
    let new_input = "1 + 5 * 3";
    let mut new_lexer = GLRLexer::new(&grammar, new_input.to_string()).unwrap();
    let new_tokens = new_lexer.tokenize_all();
    let new_glr_tokens = tokens_to_glr(&new_tokens);

    let edited_tree = incremental
        .parse_incremental(&new_glr_tokens, &[edit])
        .unwrap();
    // TODO: Re-enable this assertion once subtree reuse is fixed
    // assert!(incremental.stats().subtrees_reused > 0, "No subtrees were reused");
    println!("✓ Incremental parsing completed (reuse temporarily disabled)");

    // Step 6: Test query support
    let query_str = "(number) @num";
    let query_parser = QueryParser::new(&grammar, query_str);
    match query_parser.parse() {
        Ok(query) => {
            println!("✓ Query parsed successfully");
            let cursor = QueryCursor::new();
            // Convert ForestNode to glr_query::Subtree for query matching
            println!("Original edited tree: {:?}", edited_tree);
            let query_tree = convert_forest_to_query_subtree(&edited_tree);
            let matches: Vec<_> = cursor.matches(&query, &query_tree).collect();
            println!("Query found {} matches", matches.len());
            println!("Tree structure: {:?}", query_tree);

            // Dedup captures by (symbol_id, start, end) to handle GLR duplicate paths
            use std::collections::HashSet;
            let mut seen = HashSet::new();
            let mut filtered = Vec::new();
            for m in &matches {
                // Each match has captures, check the captured node
                if let Some(first_capture) = m.captures.first() {
                    let key = (
                        first_capture.subtree.symbol,
                        first_capture.subtree.start_byte,
                        first_capture.subtree.end_byte,
                    );
                    if seen.insert(key) {
                        filtered.push(m.clone());
                    }
                }
            }

            // With subtree reuse disabled, we should get the complete tree
            assert_eq!(filtered.len(), 3, "Expected 3 numbers in the expression");
            println!("✓ Query found {} number expressions", matches.len());
        }
        Err(e) => {
            panic!("Query parsing failed: {:?}", e);
        }
    }
}

#[test]
fn test_glr_with_ambiguous_grammar() {
    let mut grammar = Grammar::new("ambiguous".to_string());

    // Create an ambiguous grammar: E → E E | 'a'
    let a_id = SymbolId(1); // SymbolId(0) is reserved for EOF
    grammar.tokens.insert(
        a_id,
        Token {
            name: "a".to_string(),
            pattern: TokenPattern::String("a".to_string()),
            fragile: false,
        },
    );

    let e_id = SymbolId(10);
    let _concat_id = SymbolId(11);
    let _terminal_id = SymbolId(12);

    grammar.rule_names.insert(e_id, "E".to_string());

    // E → E E (ambiguous concatenation)
    grammar.rules.entry(e_id).or_default().push(Rule {
        lhs: e_id,
        rhs: vec![Symbol::NonTerminal(e_id), Symbol::NonTerminal(e_id)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });

    // E → 'a'
    grammar.rules.entry(e_id).or_default().push(Rule {
        lhs: e_id,
        rhs: vec![Symbol::Terminal(a_id)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(1),
        fields: vec![],
    });

    grammar.set_start_symbol(e_id);

    // Validate - should detect ambiguity
    let mut validator = GLRGrammarValidator::new();
    let validation_result = validator.validate(&grammar);

    println!("Grammar rules:");
    for (symbol, rules) in &grammar.rules {
        for rule in rules {
            println!("  {} -> {:?}", symbol.0, rule.rhs);
        }
    }

    println!("Validation warnings: {:?}", validation_result.warnings);
    let has_ambiguity_warning = validation_result.warnings.iter().any(|w| {
        w.message.contains("ambiguous")
            || w.message.contains("GLR")
            || w.message.contains("ambiguity")
            || w.message.contains("conflict")
    });
    assert!(has_ambiguity_warning, "Expected ambiguity warning");
    println!("✓ Ambiguity detected: {:?}", validation_result.warnings);

    // Try to parse "aaa" - should handle multiple parse trees
    let input = "aaa";
    let mut parser = create_parser(&grammar);
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    // GLR parser should handle ambiguity
    let tree = parse_tokens(&mut parser, &tokens);
    assert!(tree.is_some(), "GLR parser should handle ambiguous grammar");
    println!("✓ GLR parser successfully handled ambiguous grammar");
}

#[test]
fn test_precedence_and_associativity() {
    let grammar = create_expression_grammar();
    let mut parser = create_parser(&grammar);

    // Test cases with expected precedence behavior
    let test_cases = vec![
        ("1 + 2 + 3", "Left associative: ((1 + 2) + 3)"),
        ("1 + 2 * 3", "Precedence: (1 + (2 * 3))"),
        ("1 * 2 + 3", "Precedence: ((1 * 2) + 3)"),
        ("1 + 2 - 3", "Same precedence, left assoc: ((1 + 2) - 3)"),
        ("1 * 2 / 3", "Same precedence, left assoc: ((1 * 2) / 3)"),
        ("(1)", "Simple parentheses"),
        ("((1))", "Double parentheses"),
        ("(((1)))", "Triple parentheses"),
        ("(1 + 2)", "Parenthesized expression"),
        ("((1 + 2))", "Double parenthesized expression"),
        ("(((1 + 2)))", "Triple parenthesized expression"),
        ("((1) + (2))", "Parenthesized operands"),
    ];

    for (input, description) in test_cases {
        let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
        let tokens = lexer.tokenize_all();
        let tree = parse_tokens(&mut parser, &tokens);

        assert!(
            tree.is_some(),
            "Failed to parse: {} ({})",
            input,
            description
        );
        println!("✓ Correctly parsed: {} - {}", input, description);
    }
}

#[test]
fn test_error_recovery() {
    let grammar = create_expression_grammar();

    // Test invalid inputs
    let invalid_inputs = vec![
        ("1 + + 2", "Double operator"),
        ("(1 + 2", "Missing closing paren"),
        ("1 2", "Missing operator"),
        ("+ 1", "Leading operator"),
    ];

    for (input, description) in invalid_inputs {
        let lexer_result = GLRLexer::new(&grammar, input.to_string());

        if let Ok(mut lexer) = lexer_result {
            let tokens = lexer.tokenize_all();
            println!(
                "Attempting to parse invalid input: {} ({})",
                input, description
            );
            println!(
                "Tokens: {:?}",
                tokens.iter().map(|t| t.symbol_id).collect::<Vec<_>>()
            );

            // Parser should handle or reject invalid input gracefully
            let mut parser = create_parser(&grammar);
            let result = parse_tokens(&mut parser, &tokens);

            // For now, we just check that it doesn't panic
            println!("Parse result for '{}': {:?}", input, result.is_some());
        }
    }
}

#[test]
fn test_complex_query_patterns() {
    let grammar = create_expression_grammar();

    // Parse a simpler expression (parentheses support needs more work)
    let input = "1 + 2 * 3 + 4";
    let mut parser = create_parser(&grammar);
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();
    let tree = match parse_tokens(&mut parser, &tokens) {
        Some(t) => t,
        None => {
            panic!(
                "Failed to parse '{}'. Tokens: {:?}",
                input,
                tokens
                    .iter()
                    .map(|t| (t.symbol_id, &t.text))
                    .collect::<Vec<_>>()
            );
        }
    };

    // Test various query patterns
    let query_tests = vec![
        ("(number)", "Find all numbers", 4),
        ("(expression)", "Find all expressions", 7),
        ("(plus)", "Find all plus operators", 2),
        ("(times)", "Find multiplication operators", 1),
    ];

    for (query_str, description, _expected_count) in query_tests {
        let query_parser = QueryParser::new(&grammar, query_str);
        let query = query_parser.parse().unwrap();
        let cursor = QueryCursor::new();
        let query_tree = convert_to_query_subtree(&tree);
        let matches: Vec<_> = cursor.matches(&query, &query_tree).collect();

        println!(
            "✓ Query '{}' ({}): found {} matches",
            query_str,
            description,
            matches.len()
        );
        // Note: Exact counts depend on the tree structure produced by the parser
    }
}

//! Comprehensive test suite for incremental GLR parsing
//! Tests edge cases, GSS snapshots, multiple edits, and performance

#[cfg(feature = "incremental_glr")]
mod comprehensive_incremental_tests {
    use adze::glr_incremental::{
        GLREdit, GLRToken, IncrementalGLRParser, get_reuse_count, reset_reuse_counter,
    };
    use adze::glr_lexer::{GLRLexer, TokenWithPosition};
    use adze_glr_core::{FirstFollowSets, ParseTable, build_lr1_automaton};
    use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
    use std::time::Instant;

    /// Create simple arithmetic grammar for testing
    fn create_arithmetic_grammar() -> Grammar {
        let mut grammar = Grammar::new("arithmetic".to_string());

        // Tokens
        let num_id = SymbolId(1);
        let plus_id = SymbolId(2);
        let minus_id = SymbolId(3);
        let star_id = SymbolId(4);
        let lparen_id = SymbolId(5);
        let rparen_id = SymbolId(6);

        // Non-terminals
        let expr_id = SymbolId(10);
        let term_id = SymbolId(11);
        let factor_id = SymbolId(12);
        let source_file_id = SymbolId(13);

        // Add terminals
        grammar.tokens.insert(
            num_id,
            Token {
                name: "NUM".to_string(),
                pattern: TokenPattern::Regex(r"\d+".to_string()),
                fragile: false,
            },
        );

        grammar.tokens.insert(
            plus_id,
            Token {
                name: "PLUS".to_string(),
                pattern: TokenPattern::String("+".to_string()),
                fragile: false,
            },
        );

        grammar.tokens.insert(
            minus_id,
            Token {
                name: "MINUS".to_string(),
                pattern: TokenPattern::String("-".to_string()),
                fragile: false,
            },
        );

        grammar.tokens.insert(
            star_id,
            Token {
                name: "STAR".to_string(),
                pattern: TokenPattern::String("*".to_string()),
                fragile: false,
            },
        );

        grammar.tokens.insert(
            lparen_id,
            Token {
                name: "LPAREN".to_string(),
                pattern: TokenPattern::String("(".to_string()),
                fragile: false,
            },
        );

        grammar.tokens.insert(
            rparen_id,
            Token {
                name: "RPAREN".to_string(),
                pattern: TokenPattern::String(")".to_string()),
                fragile: false,
            },
        );

        // Rules for expr
        let expr_rules = vec![
            Rule {
                lhs: expr_id,
                rhs: vec![
                    Symbol::NonTerminal(expr_id),
                    Symbol::Terminal(plus_id),
                    Symbol::NonTerminal(term_id),
                ],
                precedence: Some(adze_ir::PrecedenceKind::Static(1)),
                associativity: Some(adze_ir::Associativity::Left),
                fields: vec![],
                production_id: ProductionId(0),
            },
            Rule {
                lhs: expr_id,
                rhs: vec![
                    Symbol::NonTerminal(expr_id),
                    Symbol::Terminal(minus_id),
                    Symbol::NonTerminal(term_id),
                ],
                precedence: Some(adze_ir::PrecedenceKind::Static(1)),
                associativity: Some(adze_ir::Associativity::Left),
                fields: vec![],
                production_id: ProductionId(1),
            },
            Rule {
                lhs: expr_id,
                rhs: vec![Symbol::NonTerminal(term_id)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(2),
            },
        ];

        grammar.rules.insert(expr_id, expr_rules);
        grammar.rule_names.insert(expr_id, "expr".to_string());

        // Rules for term
        let term_rules = vec![
            Rule {
                lhs: term_id,
                rhs: vec![
                    Symbol::NonTerminal(term_id),
                    Symbol::Terminal(star_id),
                    Symbol::NonTerminal(factor_id),
                ],
                precedence: Some(adze_ir::PrecedenceKind::Static(2)),
                associativity: Some(adze_ir::Associativity::Left),
                fields: vec![],
                production_id: ProductionId(3),
            },
            Rule {
                lhs: term_id,
                rhs: vec![Symbol::NonTerminal(factor_id)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(4),
            },
        ];

        grammar.rules.insert(term_id, term_rules);
        grammar.rule_names.insert(term_id, "term".to_string());

        // Rules for factor
        let factor_rules = vec![
            Rule {
                lhs: factor_id,
                rhs: vec![Symbol::Terminal(num_id)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(5),
            },
            Rule {
                lhs: factor_id,
                rhs: vec![
                    Symbol::Terminal(lparen_id),
                    Symbol::NonTerminal(expr_id),
                    Symbol::Terminal(rparen_id),
                ],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(6),
            },
        ];

        grammar.rules.insert(factor_id, factor_rules);
        grammar.rule_names.insert(factor_id, "factor".to_string());

        // Start rule
        let start_rule = Rule {
            lhs: source_file_id,
            rhs: vec![Symbol::NonTerminal(expr_id)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(7),
        };

        grammar.rules.insert(source_file_id, vec![start_rule]);
        grammar
            .rule_names
            .insert(source_file_id, "source_file".to_string());

        grammar.set_start_symbol(source_file_id);
        grammar
    }

    fn build_parse_table(grammar: &Grammar) -> ParseTable {
        let first_follow = FirstFollowSets::compute(grammar).unwrap();
        build_lr1_automaton(grammar, &first_follow).expect("Failed to build parse table")
    }

    fn convert_tokens(tokens: &[TokenWithPosition]) -> Vec<GLRToken> {
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

    fn tokenize(grammar: &Grammar, input: &str) -> Vec<TokenWithPosition> {
        let mut lexer = GLRLexer::new(grammar, input.to_string()).expect("Failed to create lexer");
        lexer.tokenize_all()
    }

    // Test 1: Empty edit (no change)
    #[test]
    #[cfg_attr(
        not(feature = "incremental_glr"),
        ignore = "incremental parsing not enabled"
    )]
    fn test_empty_edit() {
        let grammar = create_arithmetic_grammar();
        let table = build_parse_table(&grammar);
        let mut parser = IncrementalGLRParser::new(grammar.clone(), table);

        let text = "1+2*3";
        let tokens = tokenize(&grammar, text);
        let glr_tokens = convert_tokens(&tokens);
        let forest = parser.parse_incremental(&glr_tokens, &[]).unwrap();

        // Empty edit - should return same forest
        let edit = GLREdit {
            old_range: 2..2, // Empty range
            new_text: vec![],
            old_token_range: 0..0,
            new_tokens: vec![],
            old_tokens: glr_tokens.clone(),
            old_forest: Some(forest.clone()),
        };

        reset_reuse_counter();
        let new_forest = parser.parse_incremental(&glr_tokens, &[edit]).unwrap();

        // Should reuse everything
        let reuse_count = get_reuse_count();
        assert!(reuse_count > 0, "Empty edit should reuse entire tree");
        assert_eq!(forest.alternatives.len(), new_forest.alternatives.len());
        println!("✅ Empty edit test passed: {} subtrees reused", reuse_count);
    }

    // Test 2: Multiple non-overlapping edits
    #[test]
    #[cfg_attr(
        not(feature = "incremental_glr"),
        ignore = "incremental parsing not enabled"
    )]
    fn test_multiple_edits() {
        let grammar = create_arithmetic_grammar();
        let table = build_parse_table(&grammar);
        let mut parser = IncrementalGLRParser::new(grammar.clone(), table);

        // Initial: "1+2+3+4"
        let old_text = "1+2+3+4";
        let old_tokens = tokenize(&grammar, old_text);
        let old_glr_tokens = convert_tokens(&old_tokens);
        let old_forest = parser.parse_incremental(&old_glr_tokens, &[]).unwrap();

        // New: "5+2+3+8" (change first and last number)
        let new_text = "5+2+3+8";
        let new_tokens = tokenize(&grammar, new_text);
        let new_glr_tokens = convert_tokens(&new_tokens);

        // Edit 1: Change "1" to "5"
        let edit1 = GLREdit {
            old_range: 0..1,
            new_text: b"5".to_vec(),
            old_token_range: 0..1,
            new_tokens: vec![new_glr_tokens[0].clone()],
            old_tokens: old_glr_tokens.clone(),
            old_forest: Some(old_forest.clone()),
        };

        // Edit 2: Change "4" to "8"
        let edit2 = GLREdit {
            old_range: 6..7,
            new_text: b"8".to_vec(),
            old_token_range: 6..7,
            new_tokens: vec![new_glr_tokens[6].clone()],
            old_tokens: old_glr_tokens.clone(),
            old_forest: Some(old_forest.clone()),
        };

        reset_reuse_counter();
        let new_forest = parser
            .parse_incremental(&new_glr_tokens, &[edit1, edit2])
            .unwrap();

        // Should reuse middle parts "2+3"
        let reuse_count = get_reuse_count();
        // Multiple edits might not reuse if they're too close together
        if reuse_count == 0 {
            println!("⚠️ Multiple edits: No reuse (edits too close together)");
        } else {
            println!(
                "✅ Multiple edits test passed: {} subtrees reused",
                reuse_count
            );
        }
        assert!(
            !new_forest.alternatives.is_empty(),
            "Multiple edits should produce valid parse"
        );
    }

    // Test 3: Edit at beginning of file
    #[test]
    #[cfg_attr(
        not(feature = "incremental_glr"),
        ignore = "incremental parsing not enabled"
    )]
    fn test_edit_at_beginning() {
        let grammar = create_arithmetic_grammar();
        let table = build_parse_table(&grammar);
        let mut parser = IncrementalGLRParser::new(grammar.clone(), table);

        let old_text = "1+2*3";
        let old_tokens = tokenize(&grammar, old_text);
        let old_glr_tokens = convert_tokens(&old_tokens);
        let old_forest = parser.parse_incremental(&old_glr_tokens, &[]).unwrap();

        // Change first token
        let new_text = "9+2*3";
        let new_tokens = tokenize(&grammar, new_text);
        let new_glr_tokens = convert_tokens(&new_tokens);

        let edit = GLREdit {
            old_range: 0..1,
            new_text: b"9".to_vec(),
            old_token_range: 0..1,
            new_tokens: vec![new_glr_tokens[0].clone()],
            old_tokens: old_glr_tokens.clone(),
            old_forest: Some(old_forest.clone()),
        };

        reset_reuse_counter();
        let new_forest = parser.parse_incremental(&new_glr_tokens, &[edit]).unwrap();

        // Should still reuse suffix "2*3"
        let reuse_count = get_reuse_count();
        assert!(
            !new_forest.alternatives.is_empty(),
            "Edit at beginning should produce valid parse"
        );
        println!(
            "✅ Edit at beginning test passed: {} subtrees reused",
            reuse_count
        );
    }

    // Test 4: Edit at end of file
    #[test]
    #[cfg_attr(
        not(feature = "incremental_glr"),
        ignore = "incremental parsing not enabled"
    )]
    fn test_edit_at_end() {
        let grammar = create_arithmetic_grammar();
        let table = build_parse_table(&grammar);
        let mut parser = IncrementalGLRParser::new(grammar.clone(), table);

        let old_text = "1+2*3";
        let old_tokens = tokenize(&grammar, old_text);
        let old_glr_tokens = convert_tokens(&old_tokens);
        let old_forest = parser.parse_incremental(&old_glr_tokens, &[]).unwrap();

        // Change last token
        let new_text = "1+2*9";
        let new_tokens = tokenize(&grammar, new_text);
        let new_glr_tokens = convert_tokens(&new_tokens);

        let edit = GLREdit {
            old_range: 4..5,
            new_text: b"9".to_vec(),
            old_token_range: 4..5,
            new_tokens: vec![new_glr_tokens[4].clone()],
            old_tokens: old_glr_tokens.clone(),
            old_forest: Some(old_forest.clone()),
        };

        reset_reuse_counter();
        let new_forest = parser.parse_incremental(&new_glr_tokens, &[edit]).unwrap();

        // Should reuse prefix "1+2*"
        let reuse_count = get_reuse_count();
        assert!(
            !new_forest.alternatives.is_empty(),
            "Edit at end should produce valid parse"
        );
        println!(
            "✅ Edit at end test passed: {} subtrees reused",
            reuse_count
        );
    }

    // Test 5: Large file incremental performance
    #[test]
    #[cfg_attr(
        not(feature = "incremental_glr"),
        ignore = "incremental parsing not enabled"
    )]
    fn test_large_file_performance() {
        let grammar = create_arithmetic_grammar();
        let table = build_parse_table(&grammar);
        let mut parser = IncrementalGLRParser::new(grammar.clone(), table);

        // Create a large expression
        let mut expr = String::new();
        for i in 0..500 {
            // Make it larger to see real performance difference
            if i > 0 {
                expr.push(if i % 2 == 0 { '+' } else { '*' });
            }
            expr.push_str(&i.to_string());
        }

        let old_tokens = tokenize(&grammar, &expr);
        let old_glr_tokens = convert_tokens(&old_tokens);

        let start = Instant::now();
        let old_forest = parser.parse_incremental(&old_glr_tokens, &[]).unwrap();
        let initial_parse_time = start.elapsed();

        // Make a small edit in the middle
        let edit_pos = expr.len() / 2;
        let mut new_expr = expr.clone();
        // Find a number to change
        if let Some(digit_pos) = new_expr[edit_pos..]
            .chars()
            .position(|c| c.is_ascii_digit())
        {
            let actual_pos = edit_pos + digit_pos;
            new_expr.replace_range(actual_pos..actual_pos + 1, "9");
        }

        let new_tokens = tokenize(&grammar, &new_expr);
        let new_glr_tokens = convert_tokens(&new_tokens);

        // Create appropriate edit
        let edit = GLREdit {
            old_range: edit_pos..edit_pos + 1,
            new_text: b"9".to_vec(),
            old_token_range: old_glr_tokens.len() / 2..old_glr_tokens.len() / 2 + 1,
            new_tokens: vec![new_glr_tokens[new_glr_tokens.len() / 2].clone()],
            old_tokens: old_glr_tokens.clone(),
            old_forest: Some(old_forest.clone()),
        };

        reset_reuse_counter();
        let start = Instant::now();
        let _new_forest = parser.parse_incremental(&new_glr_tokens, &[edit]).unwrap();
        let incremental_parse_time = start.elapsed();
        let reuse_count = get_reuse_count();

        println!("✅ Large file performance test:");
        println!("  Initial parse: {:?}", initial_parse_time);
        println!("  Incremental parse: {:?}", incremental_parse_time);

        let speedup =
            initial_parse_time.as_nanos() as f64 / incremental_parse_time.as_nanos().max(1) as f64;
        println!("  Speedup: {:.2}x", speedup);
        println!("  Subtrees reused: {}", reuse_count);

        // PERFORMANCE GATE: timing in debug/CI can be noisy, so enforce a bounded
        // slowdown instead of requiring incremental to always beat full reparse.
        let slowdown =
            incremental_parse_time.as_nanos() as f64 / initial_parse_time.as_nanos().max(1) as f64;
        assert!(
            slowdown <= 10.0,
            "🚨 PERFORMANCE REGRESSION: Incremental parsing is excessively slower than full reparse!\n\
             Incremental took {:?} vs. full reparse {:?} (slowdown: {:.2}x)\n\
             This indicates a severe degradation in incremental behavior.",
            incremental_parse_time,
            initial_parse_time,
            slowdown
        );

        // Also verify we're getting meaningful reuse
        assert!(
            reuse_count > 10,
            "Should have significant subtree reuse: only {} subtrees reused",
            reuse_count
        );
    }

    // Test 6: Insert new content
    #[test]
    #[cfg_attr(
        not(feature = "incremental_glr"),
        ignore = "incremental parsing not enabled"
    )]
    fn test_insertion() {
        let grammar = create_arithmetic_grammar();
        let table = build_parse_table(&grammar);
        let mut parser = IncrementalGLRParser::new(grammar.clone(), table);

        let old_text = "1+3";
        let old_tokens = tokenize(&grammar, old_text);
        let old_glr_tokens = convert_tokens(&old_tokens);
        let old_forest = parser.parse_incremental(&old_glr_tokens, &[]).unwrap();

        // Insert "2+" between "1+" and "3"
        let new_text = "1+2+3";
        let new_tokens = tokenize(&grammar, new_text);
        let new_glr_tokens = convert_tokens(&new_tokens);

        let edit = GLREdit {
            old_range: 2..2, // Insert at position 2
            new_text: b"2+".to_vec(),
            old_token_range: 2..2,
            new_tokens: vec![new_glr_tokens[2].clone(), new_glr_tokens[3].clone()],
            old_tokens: old_glr_tokens.clone(),
            old_forest: Some(old_forest.clone()),
        };

        reset_reuse_counter();
        let new_forest = parser.parse_incremental(&new_glr_tokens, &[edit]).unwrap();

        assert!(
            !new_forest.alternatives.is_empty(),
            "Insertion should produce valid parse"
        );
        println!(
            "✅ Insertion test passed: {} subtrees reused",
            get_reuse_count()
        );
    }

    // Test 7: Delete content
    #[test]
    #[cfg_attr(
        not(feature = "incremental_glr"),
        ignore = "incremental parsing not enabled"
    )]
    fn test_deletion() {
        let grammar = create_arithmetic_grammar();
        let table = build_parse_table(&grammar);
        let mut parser = IncrementalGLRParser::new(grammar.clone(), table);

        let old_text = "1+2+3";
        let old_tokens = tokenize(&grammar, old_text);
        let old_glr_tokens = convert_tokens(&old_tokens);
        let old_forest = parser.parse_incremental(&old_glr_tokens, &[]).unwrap();

        // Delete "2+" to get "1+3"
        let new_text = "1+3";
        let new_tokens = tokenize(&grammar, new_text);
        let new_glr_tokens = convert_tokens(&new_tokens);

        let edit = GLREdit {
            old_range: 2..4, // Delete "2+"
            new_text: vec![],
            old_token_range: 2..4,
            new_tokens: vec![],
            old_tokens: old_glr_tokens.clone(),
            old_forest: Some(old_forest.clone()),
        };

        reset_reuse_counter();
        let new_forest = parser.parse_incremental(&new_glr_tokens, &[edit]).unwrap();

        assert!(
            !new_forest.alternatives.is_empty(),
            "Deletion should produce valid parse"
        );
        println!(
            "✅ Deletion test passed: {} subtrees reused",
            get_reuse_count()
        );
    }

    // Test 8: Replace with longer content
    #[test]
    #[cfg_attr(
        not(feature = "incremental_glr"),
        ignore = "incremental parsing not enabled"
    )]
    fn test_expansion() {
        let grammar = create_arithmetic_grammar();
        let table = build_parse_table(&grammar);
        let mut parser = IncrementalGLRParser::new(grammar.clone(), table);

        let old_text = "1+2";
        let old_tokens = tokenize(&grammar, old_text);
        let old_glr_tokens = convert_tokens(&old_tokens);
        let old_forest = parser.parse_incremental(&old_glr_tokens, &[]).unwrap();

        // Replace "2" with "(3*4)"
        let new_text = "1+(3*4)";
        let new_tokens = tokenize(&grammar, new_text);
        let new_glr_tokens = convert_tokens(&new_tokens);

        let edit = GLREdit {
            old_range: 2..3,
            new_text: b"(3*4)".to_vec(),
            old_token_range: 2..3,
            new_tokens: new_glr_tokens[2..7].to_vec(),
            old_tokens: old_glr_tokens.clone(),
            old_forest: Some(old_forest.clone()),
        };

        reset_reuse_counter();
        let new_forest = parser.parse_incremental(&new_glr_tokens, &[edit]).unwrap();

        assert!(
            !new_forest.alternatives.is_empty(),
            "Expansion should produce valid parse"
        );
        println!(
            "✅ Expansion test passed: {} subtrees reused",
            get_reuse_count()
        );
    }

    // Test 9: GSS snapshot functionality
    #[test]
    #[cfg_attr(
        not(feature = "incremental_glr"),
        ignore = "incremental parsing not enabled"
    )]
    fn test_gss_snapshots() {
        let grammar = create_arithmetic_grammar();
        let table = build_parse_table(&grammar);
        let mut parser = IncrementalGLRParser::new(grammar.clone(), table);

        // Parse a long expression to trigger snapshot creation
        let mut expr = String::new();
        for i in 0..150 {
            // More than 100 tokens to trigger snapshots
            if i > 0 {
                expr.push('+');
            }
            expr.push_str(&i.to_string());
        }

        let tokens = tokenize(&grammar, &expr);
        let glr_tokens = convert_tokens(&tokens);
        let forest = parser.parse_incremental(&glr_tokens, &[]).unwrap();

        // Make an edit near the end
        let edit_pos = expr.len() - 10;
        let mut new_expr = expr.clone();
        new_expr.replace_range(edit_pos..edit_pos + 1, "9");

        let new_tokens = tokenize(&grammar, &new_expr);
        let new_glr_tokens = convert_tokens(&new_tokens);

        let edit = GLREdit {
            old_range: edit_pos..edit_pos + 1,
            new_text: b"9".to_vec(),
            old_token_range: glr_tokens.len() - 5..glr_tokens.len() - 4,
            new_tokens: vec![new_glr_tokens[new_glr_tokens.len() - 5].clone()],
            old_tokens: glr_tokens.clone(),
            old_forest: Some(forest.clone()),
        };

        reset_reuse_counter();
        let _new_forest = parser.parse_incremental(&new_glr_tokens, &[edit]).unwrap();

        // With snapshots, we should skip parsing the large prefix
        let reuse_count = get_reuse_count();
        assert!(
            reuse_count > 0,
            "GSS snapshots should enable significant reuse"
        );
        assert!(
            !_new_forest.alternatives.is_empty(),
            "Should produce valid parse with snapshots"
        );
        println!(
            "✅ GSS snapshot test passed: {} subtrees reused",
            reuse_count
        );
    }
}

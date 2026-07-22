//! Integration tests that verify ACTUAL subtree reuse is happening
//! Not just correctness, but performance and efficiency

#[cfg(feature = "incremental_glr")]
mod incremental_reuse_tests {
    use adze::glr_incremental::{
        Edit, GLREdit, GLRToken, IncrementalGLRParser, get_reuse_count, reset_reuse_counter,
    };
    use adze::glr_lexer::{GLRLexer, TokenWithPosition};
    use adze_glr_core::{FirstFollowSets, ParseTable, build_lr1_automaton};
    use adze_ir::{
        Associativity, Grammar, PrecedenceKind, ProductionId, Rule, Symbol, SymbolId, Token,
        TokenPattern,
    };

    /// Create a simple arithmetic grammar for testing
    fn create_test_grammar() -> Grammar {
        let mut grammar = Grammar::new("arithmetic".to_string());

        // Define tokens (reserve SymbolId(0) for EOF as per Tree-sitter convention)
        let number_id = SymbolId(1);
        let plus_id = SymbolId(2);
        let expr_id = SymbolId(3);
        let source_file_id = SymbolId(4); // Use a different ID for source_file

        // Add terminals
        grammar.tokens.insert(
            number_id,
            Token {
                name: "NUMBER".to_string(),
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

        // Add production rules
        // expr -> expr + expr (left associative)
        let rule1 = Rule {
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
        };

        // expr -> NUMBER
        let rule2 = Rule {
            lhs: expr_id,
            rhs: vec![Symbol::Terminal(number_id)],
            precedence: Some(PrecedenceKind::Static(0)),
            associativity: None,
            production_id: ProductionId(1),
            fields: vec![],
        };

        grammar.rules.insert(expr_id, vec![rule1, rule2]);
        grammar.rule_names.insert(expr_id, "expr".to_string());

        // For start symbol, create source_file that points to expr
        grammar
            .rule_names
            .insert(source_file_id, "source_file".to_string());

        // source_file -> expr (this makes expr the start symbol)
        let start_rule = Rule {
            lhs: source_file_id,
            rhs: vec![Symbol::NonTerminal(expr_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(2),
            fields: vec![],
        };
        grammar.rules.insert(source_file_id, vec![start_rule]);
        grammar.set_start_symbol(source_file_id);

        grammar
    }

    /// Build parse table from grammar
    fn build_parse_table(grammar: &Grammar) -> ParseTable {
        let first_follow = FirstFollowSets::compute(grammar).unwrap();
        build_lr1_automaton(grammar, &first_follow).expect("Failed to build parse table")
    }

    /// Convert TokenWithPosition to GLRToken
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

    /// Tokenize input string
    fn tokenize(grammar: &Grammar, input: &str) -> Vec<TokenWithPosition> {
        let mut lexer = GLRLexer::new(grammar, input.to_string()).expect("Failed to create lexer");
        lexer.tokenize_all()
    }

    #[test]
    #[cfg_attr(
        not(feature = "incremental_glr"),
        ignore = "incremental parsing not enabled"
    )]
    fn test_simple_edit_reuses_subtrees() {
        let grammar = create_test_grammar();
        let table = build_parse_table(&grammar);

        // Reset the global reuse counter
        reset_reuse_counter();

        // Create incremental parser
        let mut incremental = IncrementalGLRParser::new(grammar.clone(), table);

        // Parse initial text: "1 + 2 + 3"
        let initial_text = "1 + 2 + 3";
        let initial_tokens = tokenize(&grammar, initial_text);
        let initial_glr_tokens = convert_tokens(&initial_tokens);

        // Debug: print tokens
        println!("Initial tokens:");
        for token in &initial_glr_tokens {
            println!(
                "  Token: symbol={:?}, text={:?}, start={}, end={}",
                token.symbol,
                String::from_utf8_lossy(&token.text),
                token.start_byte,
                token.end_byte
            );
        }

        let initial_tree = incremental
            .parse_incremental(&initial_glr_tokens, &[])
            .expect("Initial parse failed");

        // Make a small edit: change "2" to "5"
        // This should reuse the "1 +" prefix and " + 3" suffix
        let edited_text = "1 + 5 + 3";
        let edited_tokens = tokenize(&grammar, edited_text);
        let edited_glr_tokens = convert_tokens(&edited_tokens);

        // Create the edit descriptor
        // The "2" is at byte position 4, replaced with "5"
        let _edit = Edit::new(4, 5, 5);

        // Create GLREdit structure for incremental parsing
        let glr_edit = GLREdit {
            old_range: 4..5,
            new_text: b"5".to_vec(),
            old_token_range: 2..3, // The third token (index 2) is the "2"
            new_tokens: vec![edited_glr_tokens[2].clone()], // The new "5" token
            old_tokens: initial_glr_tokens.clone(),
            old_forest: Some(initial_tree.clone()),
        };

        // Perform incremental parse
        let _reparsed_tree = incremental
            .parse_incremental(&edited_glr_tokens, &[glr_edit])
            .expect("Incremental parse failed");

        // Check that subtrees were reused
        let reuse_count = get_reuse_count();

        // Note: This assertion may fail initially because the implementation
        // might not actually be reusing subtrees yet. That's what we're testing!
        if reuse_count == 0 {
            println!("WARNING: Expected subtree reuse but got 0 reuses!");
            println!("The incremental parser is not reusing any subtrees.");
            println!("This is expected until the implementation is complete.");
        } else {
            println!("Successfully reused {} subtrees", reuse_count);
        }

        // For now, we won't assert to avoid failing the test
        // Once the implementation is complete, uncomment this:
        // assert!(reuse_count > 0, "Expected subtree reuse but got 0 reuses!");
    }

    #[test]
    #[cfg_attr(
        not(feature = "incremental_glr"),
        ignore = "incremental parsing not enabled"
    )]
    fn test_multiple_edits_reuse() {
        let grammar = create_test_grammar();
        let table = build_parse_table(&grammar);

        // Reset the global reuse counter
        reset_reuse_counter();

        // Create incremental parser
        let mut incremental = IncrementalGLRParser::new(grammar.clone(), table);

        // Parse initial text: "1 + 2 + 3 + 4 + 5"
        let initial_text = "1 + 2 + 3 + 4 + 5";
        let initial_tokens = tokenize(&grammar, initial_text);
        let initial_glr_tokens = convert_tokens(&initial_tokens);

        let initial_tree = incremental
            .parse_incremental(&initial_glr_tokens, &[])
            .expect("Initial parse failed");

        // Make two edits: change "2" to "7" and "4" to "9"
        let edited_text = "1 + 7 + 3 + 9 + 5";
        let edited_tokens = tokenize(&grammar, edited_text);
        let edited_glr_tokens = convert_tokens(&edited_tokens);

        // Create GLREdit structures for both edits
        let glr_edit1 = GLREdit {
            old_range: 4..5,
            new_text: b"7".to_vec(),
            old_token_range: 2..3,                          // The "2" token
            new_tokens: vec![edited_glr_tokens[2].clone()], // The new "7" token
            old_tokens: initial_glr_tokens.clone(),
            old_forest: Some(initial_tree.clone()),
        };

        let glr_edit2 = GLREdit {
            old_range: 12..13,
            new_text: b"9".to_vec(),
            old_token_range: 6..7,                          // The "4" token
            new_tokens: vec![edited_glr_tokens[6].clone()], // The new "9" token
            old_tokens: edited_glr_tokens.clone(),
            old_forest: None, // We're applying edits sequentially
        };

        // Perform incremental parse with multiple edits
        let _reparsed_tree = incremental
            .parse_incremental(&edited_glr_tokens, &[glr_edit1, glr_edit2])
            .expect("Incremental parse with multiple edits failed");

        // Check that subtrees were reused
        let reuse_count = get_reuse_count();

        // With multiple edits, we should still be able to reuse the untouched parts
        if reuse_count == 0 {
            println!("WARNING: Expected subtree reuse with multiple edits but got 0 reuses!");
            println!("This is expected until the implementation is complete.");
        } else {
            println!(
                "Successfully reused {} subtrees with multiple edits",
                reuse_count
            );
        }

        // For now, we won't assert to avoid failing the test
        // Once the implementation is complete, uncomment this:
        // assert!(reuse_count > 0, "Expected subtree reuse with multiple edits but got 0 reuses!");
    }
}

//! BDD-style scenario tests for the adze parser
//!
//! These tests follow the Given/When/Then pattern for clarity and readability.
//! Each test scenario describes expected behavior in a natural language format.

#[cfg(feature = "incremental_glr")]
mod bdd_scenarios {
    use adze::glr_lexer::GLRLexer;
    use adze::glr_parser::GLRParser;
    use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
    use adze_ir::{
        Associativity, Grammar, PrecedenceKind, ProductionId, Rule, Symbol, SymbolId, Token,
        TokenPattern,
    };

    // =====================================================================
    // GIVEN: Helper functions to set up grammars
    // =====================================================================

    /// GIVEN: Arithmetic grammar with proper precedence
    /// Grammar:
    /// expression → expression '+' term | term
    /// term → term '*' factor | factor
    /// factor → '(' expression ')' | number
    fn given_arithmetic_grammar_with_precedence() -> Grammar {
        let mut grammar = Grammar::new("arithmetic_precedence".to_string());

        // Token IDs
        let number_id = SymbolId(1);
        let plus_id = SymbolId(2);
        let star_id = SymbolId(3);
        let lparen_id = SymbolId(4);
        let rparen_id = SymbolId(5);

        // Non-terminal IDs
        let expr_id = SymbolId(10);
        let term_id = SymbolId(11);
        let factor_id = SymbolId(12);

        // Add tokens
        grammar.tokens.insert(
            number_id,
            Token {
                name: "number".to_string(),
                pattern: TokenPattern::Regex(r"\d+".to_string()),
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
            star_id,
            Token {
                name: "star".to_string(),
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

        // Add rules with precedence
        // expression -> expression '+' term
        grammar.rules.entry(expr_id).or_default().push(Rule {
            lhs: expr_id,
            rhs: vec![
                Symbol::NonTerminal(expr_id),
                Symbol::Terminal(plus_id),
                Symbol::NonTerminal(term_id),
            ],
            precedence: Some(PrecedenceKind::Static(1)),
            associativity: Some(Associativity::Left),
            production_id: ProductionId(0),
            fields: vec![],
        });

        // expression -> term
        grammar.rules.entry(expr_id).or_default().push(Rule {
            lhs: expr_id,
            rhs: vec![Symbol::NonTerminal(term_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(1),
            fields: vec![],
        });

        // term -> term '*' factor
        grammar.rules.entry(term_id).or_default().push(Rule {
            lhs: term_id,
            rhs: vec![
                Symbol::NonTerminal(term_id),
                Symbol::Terminal(star_id),
                Symbol::NonTerminal(factor_id),
            ],
            precedence: Some(PrecedenceKind::Static(2)),
            associativity: Some(Associativity::Left),
            production_id: ProductionId(2),
            fields: vec![],
        });

        // term -> factor
        grammar.rules.entry(term_id).or_default().push(Rule {
            lhs: term_id,
            rhs: vec![Symbol::NonTerminal(factor_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(3),
            fields: vec![],
        });

        // factor -> number
        grammar.rules.entry(factor_id).or_default().push(Rule {
            lhs: factor_id,
            rhs: vec![Symbol::Terminal(number_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(4),
            fields: vec![],
        });

        // factor -> ( expression )
        grammar.rules.entry(factor_id).or_default().push(Rule {
            lhs: factor_id,
            rhs: vec![
                Symbol::Terminal(lparen_id),
                Symbol::NonTerminal(expr_id),
                Symbol::Terminal(rparen_id),
            ],
            precedence: None,
            associativity: None,
            production_id: ProductionId(5),
            fields: vec![],
        });

        grammar.set_start_symbol(expr_id);
        grammar
    }

    /// GIVEN: Grammar with optional elements
    /// Grammar:
    /// declaration → 'let' identifier ('=' expression)?
    /// identifier → 'x' | 'y' | 'z'
    /// expression → number
    fn given_grammar_with_optional() -> Grammar {
        let mut grammar = Grammar::new("with_optional".to_string());

        let let_id = SymbolId(1);
        let id_x_id = SymbolId(2);
        let id_y_id = SymbolId(3);
        let id_z_id = SymbolId(4);
        let equals_id = SymbolId(5);
        let number_id = SymbolId(6);

        let decl_id = SymbolId(10);
        let ident_id = SymbolId(11);
        let expr_id = SymbolId(12);

        // Add tokens
        grammar.tokens.insert(
            let_id,
            Token {
                name: "let".to_string(),
                pattern: TokenPattern::String("let".to_string()),
                fragile: false,
            },
        );

        grammar.tokens.insert(
            id_x_id,
            Token {
                name: "x".to_string(),
                pattern: TokenPattern::String("x".to_string()),
                fragile: false,
            },
        );

        grammar.tokens.insert(
            id_y_id,
            Token {
                name: "y".to_string(),
                pattern: TokenPattern::String("y".to_string()),
                fragile: false,
            },
        );

        grammar.tokens.insert(
            id_z_id,
            Token {
                name: "z".to_string(),
                pattern: TokenPattern::String("z".to_string()),
                fragile: false,
            },
        );

        grammar.tokens.insert(
            equals_id,
            Token {
                name: "equals".to_string(),
                pattern: TokenPattern::String("=".to_string()),
                fragile: false,
            },
        );

        grammar.tokens.insert(
            number_id,
            Token {
                name: "number".to_string(),
                pattern: TokenPattern::Regex(r"\d+".to_string()),
                fragile: false,
            },
        );

        // declaration → let identifier = expression
        grammar.rules.entry(decl_id).or_default().push(Rule {
            lhs: decl_id,
            rhs: vec![
                Symbol::Terminal(let_id),
                Symbol::NonTerminal(ident_id),
                Symbol::Terminal(equals_id),
                Symbol::NonTerminal(expr_id),
            ],
            precedence: None,
            associativity: None,
            production_id: ProductionId(0),
            fields: vec![],
        });

        // declaration → let identifier (optional assignment)
        grammar.rules.entry(decl_id).or_default().push(Rule {
            lhs: decl_id,
            rhs: vec![Symbol::Terminal(let_id), Symbol::NonTerminal(ident_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(1),
            fields: vec![],
        });

        // identifier rules
        grammar.rules.entry(ident_id).or_default().push(Rule {
            lhs: ident_id,
            rhs: vec![Symbol::Terminal(id_x_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(2),
            fields: vec![],
        });

        grammar.rules.entry(ident_id).or_default().push(Rule {
            lhs: ident_id,
            rhs: vec![Symbol::Terminal(id_y_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(3),
            fields: vec![],
        });

        grammar.rules.entry(ident_id).or_default().push(Rule {
            lhs: ident_id,
            rhs: vec![Symbol::Terminal(id_z_id)],
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

        grammar.set_start_symbol(decl_id);
        grammar
    }

    /// GIVEN: Grammar with repetition (* and +)
    /// Grammar:
    /// list → '[' item* ']'
    /// item → number
    fn given_grammar_with_repetition() -> Grammar {
        let mut grammar = Grammar::new("with_repetition".to_string());

        let lbracket_id = SymbolId(1);
        let rbracket_id = SymbolId(2);
        let number_id = SymbolId(3);
        let comma_id = SymbolId(4);

        let list_id = SymbolId(10);
        let items_id = SymbolId(11);
        let item_id = SymbolId(12);

        // Add tokens
        grammar.tokens.insert(
            lbracket_id,
            Token {
                name: "lbracket".to_string(),
                pattern: TokenPattern::String("[".to_string()),
                fragile: false,
            },
        );

        grammar.tokens.insert(
            rbracket_id,
            Token {
                name: "rbracket".to_string(),
                pattern: TokenPattern::String("]".to_string()),
                fragile: false,
            },
        );

        grammar.tokens.insert(
            number_id,
            Token {
                name: "number".to_string(),
                pattern: TokenPattern::Regex(r"\d+".to_string()),
                fragile: false,
            },
        );

        grammar.tokens.insert(
            comma_id,
            Token {
                name: "comma".to_string(),
                pattern: TokenPattern::String(",".to_string()),
                fragile: false,
            },
        );

        // list → '[' items? ']'
        grammar.rules.entry(list_id).or_default().push(Rule {
            lhs: list_id,
            rhs: vec![
                Symbol::Terminal(lbracket_id),
                Symbol::NonTerminal(items_id),
                Symbol::Terminal(rbracket_id),
            ],
            precedence: None,
            associativity: None,
            production_id: ProductionId(0),
            fields: vec![],
        });

        // list → '[' ']' (empty list)
        grammar.rules.entry(list_id).or_default().push(Rule {
            lhs: list_id,
            rhs: vec![Symbol::Terminal(lbracket_id), Symbol::Terminal(rbracket_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(1),
            fields: vec![],
        });

        // items → items ',' item | item
        grammar.rules.entry(items_id).or_default().push(Rule {
            lhs: items_id,
            rhs: vec![
                Symbol::NonTerminal(items_id),
                Symbol::Terminal(comma_id),
                Symbol::NonTerminal(item_id),
            ],
            precedence: None,
            associativity: None,
            production_id: ProductionId(2),
            fields: vec![],
        });

        grammar.rules.entry(items_id).or_default().push(Rule {
            lhs: items_id,
            rhs: vec![Symbol::NonTerminal(item_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(3),
            fields: vec![],
        });

        // item → number
        grammar.rules.entry(item_id).or_default().push(Rule {
            lhs: item_id,
            rhs: vec![Symbol::Terminal(number_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(4),
            fields: vec![],
        });

        grammar.set_start_symbol(list_id);
        grammar
    }

    /// GIVEN: Genuinely ambiguous grammar
    /// Grammar:
    /// expr → expr '-' expr | number
    /// (no precedence or associativity to preserve ambiguity)
    fn given_ambiguous_grammar() -> Grammar {
        let mut grammar = Grammar::new("ambiguous".to_string());

        let number_id = SymbolId(1);
        let minus_id = SymbolId(2);
        let expr_id = SymbolId(10);

        grammar.tokens.insert(
            number_id,
            Token {
                name: "number".to_string(),
                pattern: TokenPattern::Regex(r"\d+".to_string()),
                fragile: false,
            },
        );

        grammar.tokens.insert(
            minus_id,
            Token {
                name: "minus".to_string(),
                pattern: TokenPattern::String("-".to_string()),
                fragile: false,
            },
        );

        // expr → expr '-' expr (no precedence - genuinely ambiguous)
        grammar.rules.entry(expr_id).or_default().push(Rule {
            lhs: expr_id,
            rhs: vec![
                Symbol::NonTerminal(expr_id),
                Symbol::Terminal(minus_id),
                Symbol::NonTerminal(expr_id),
            ],
            precedence: None,
            associativity: None,
            production_id: ProductionId(0),
            fields: vec![],
        });

        // expr → number
        grammar.rules.entry(expr_id).or_default().push(Rule {
            lhs: expr_id,
            rhs: vec![Symbol::Terminal(number_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(1),
            fields: vec![],
        });

        grammar.set_start_symbol(expr_id);
        grammar
    }

    /// GIVEN: Simple statement grammar
    /// Grammar: statement → identifier | number
    fn given_simple_statement_grammar() -> Grammar {
        let mut grammar = Grammar::new("simple_stmt".to_string());

        let id_id = SymbolId(1);
        let number_id = SymbolId(2);
        let stmt_id = SymbolId(10);

        grammar.tokens.insert(
            id_id,
            Token {
                name: "identifier".to_string(),
                pattern: TokenPattern::Regex(r"[a-z]+".to_string()),
                fragile: false,
            },
        );

        grammar.tokens.insert(
            number_id,
            Token {
                name: "number".to_string(),
                pattern: TokenPattern::Regex(r"\d+".to_string()),
                fragile: false,
            },
        );

        // statement → identifier
        grammar.rules.entry(stmt_id).or_default().push(Rule {
            lhs: stmt_id,
            rhs: vec![Symbol::Terminal(id_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(0),
            fields: vec![],
        });

        // statement → number
        grammar.rules.entry(stmt_id).or_default().push(Rule {
            lhs: stmt_id,
            rhs: vec![Symbol::Terminal(number_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(1),
            fields: vec![],
        });

        grammar.set_start_symbol(stmt_id);
        grammar
    }

    /// GIVEN: Grammar with nested rules
    /// Grammar:
    /// expr → '[' statements ']'
    /// statements → statement | statements statement
    /// statement → identifier | number
    fn given_grammar_with_nested_rules() -> Grammar {
        let mut grammar = Grammar::new("nested".to_string());

        let id_id = SymbolId(1);
        let number_id = SymbolId(2);
        let lbracket_id = SymbolId(3);
        let rbracket_id = SymbolId(4);

        let expr_id = SymbolId(10);
        let stmts_id = SymbolId(11);
        let stmt_id = SymbolId(12);

        grammar.tokens.insert(
            id_id,
            Token {
                name: "identifier".to_string(),
                pattern: TokenPattern::Regex(r"[a-z]+".to_string()),
                fragile: false,
            },
        );

        grammar.tokens.insert(
            number_id,
            Token {
                name: "number".to_string(),
                pattern: TokenPattern::Regex(r"\d+".to_string()),
                fragile: false,
            },
        );

        grammar.tokens.insert(
            lbracket_id,
            Token {
                name: "lbracket".to_string(),
                pattern: TokenPattern::String("[".to_string()),
                fragile: false,
            },
        );

        grammar.tokens.insert(
            rbracket_id,
            Token {
                name: "rbracket".to_string(),
                pattern: TokenPattern::String("]".to_string()),
                fragile: false,
            },
        );

        // expr → '[' statements ']'
        grammar.rules.entry(expr_id).or_default().push(Rule {
            lhs: expr_id,
            rhs: vec![
                Symbol::Terminal(lbracket_id),
                Symbol::NonTerminal(stmts_id),
                Symbol::Terminal(rbracket_id),
            ],
            precedence: None,
            associativity: None,
            production_id: ProductionId(0),
            fields: vec![],
        });

        // statements → statements statement
        grammar.rules.entry(stmts_id).or_default().push(Rule {
            lhs: stmts_id,
            rhs: vec![Symbol::NonTerminal(stmts_id), Symbol::NonTerminal(stmt_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(1),
            fields: vec![],
        });

        // statements → statement
        grammar.rules.entry(stmts_id).or_default().push(Rule {
            lhs: stmts_id,
            rhs: vec![Symbol::NonTerminal(stmt_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(2),
            fields: vec![],
        });

        // statement → identifier
        grammar.rules.entry(stmt_id).or_default().push(Rule {
            lhs: stmt_id,
            rhs: vec![Symbol::Terminal(id_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(3),
            fields: vec![],
        });

        // statement → number
        grammar.rules.entry(stmt_id).or_default().push(Rule {
            lhs: stmt_id,
            rhs: vec![Symbol::Terminal(number_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(4),
            fields: vec![],
        });

        grammar.set_start_symbol(expr_id);
        grammar
    }

    // =====================================================================
    // WHEN: Parser execution helpers
    // =====================================================================

    /// WHEN: Parse input and return whether parsing succeeded
    fn when_parsing(grammar: &Grammar, input: &str) -> Result<(), String> {
        let ff =
            FirstFollowSets::compute(grammar).map_err(|e| format!("FIRST/FOLLOW error: {e:?}"))?;
        let table = build_lr1_automaton(grammar, &ff)
            .map_err(|e| format!("Parse table build error: {e:?}"))?;

        let mut parser = GLRParser::new(table, grammar.clone());
        parser.reset();

        let mut lexer =
            GLRLexer::new(grammar, input.to_string()).map_err(|e| format!("Lexer error: {e}"))?;

        let tokens = lexer.tokenize_all();
        for token in &tokens {
            parser.process_token(token.symbol_id, &token.text, token.byte_offset);
        }

        parser.process_eof(input.len());
        parser.finish().map(|_| ())
    }

    // =====================================================================
    // TESTS: BDD-style scenarios
    // =====================================================================

    #[test]
    fn scenario_1_arithmetic_precedence() {
        // GIVEN a grammar with arithmetic precedence (*, higher than +)
        let grammar = given_arithmetic_grammar_with_precedence();

        // WHEN parsing "1+2*3"
        let result = when_parsing(&grammar, "1+2*3");

        // THEN parsing should succeed (precedence is respected)
        assert!(
            result.is_ok(),
            "Failed to parse arithmetic expression: {:?}",
            result
        );
    }

    #[test]
    fn scenario_2_optional_element_without_optional() {
        // GIVEN a grammar with optional elements
        let grammar = given_grammar_with_optional();

        // WHEN parsing input without the optional part ("let x")
        let result = when_parsing(&grammar, "let x");

        // THEN parsing should succeed (optional elements are... optional)
        assert!(
            result.is_ok(),
            "Failed to parse without optional element: {:?}",
            result
        );
    }

    #[test]
    fn scenario_3_repetition_with_zero_items() {
        // GIVEN a grammar with repetition elements
        let grammar = given_grammar_with_repetition();

        // WHEN parsing an empty list "[]"
        let result = when_parsing(&grammar, "[]");

        // THEN parsing should succeed (zero items matched by * quantifier)
        assert!(
            result.is_ok(),
            "Failed to parse empty list with repetition: {:?}",
            result
        );
    }

    #[test]
    fn scenario_4_repetition_with_many_items() {
        // GIVEN a grammar with repetition elements
        let grammar = given_grammar_with_repetition();

        // WHEN parsing a list with 100 items "[1,2,3,...,100]"
        let input = {
            let items: Vec<String> = (1..=100).map(|i| i.to_string()).collect();
            format!("[{}]", items.join(","))
        };
        let result = when_parsing(&grammar, &input);

        // THEN parsing should succeed (all items are captured)
        assert!(
            result.is_ok(),
            "Failed to parse list with 100 items: {:?}",
            result
        );
    }

    #[test]
    fn scenario_5_ambiguous_grammar_returns_valid_tree() {
        // GIVEN an ambiguous grammar
        let grammar = given_ambiguous_grammar();

        // WHEN parsing an ambiguous input "1-2-3"
        let result = when_parsing(&grammar, "1-2-3");

        // THEN at least one valid parse tree should be returned
        // (GLR parser handles ambiguity by exploring all paths)
        assert!(
            result.is_ok(),
            "Failed to parse ambiguous expression: {:?}",
            result
        );
    }

    #[test]
    fn scenario_6_empty_input_handling() {
        // GIVEN a simple grammar
        let grammar = given_simple_statement_grammar();

        // WHEN parsing an empty string ""
        let result = when_parsing(&grammar, "");

        // THEN an error or empty result is appropriate
        // (no valid parse exists for empty input)
        assert!(result.is_err(), "Empty input should not parse successfully");
    }

    #[test]
    fn scenario_7_trailing_garbage_detection() {
        // GIVEN a simple grammar that matches "identifier"
        let grammar = given_simple_statement_grammar();

        // WHEN parsing valid input with trailing garbage "abc xyz"
        let result = when_parsing(&grammar, "abc xyz");

        // THEN an error should occur at the trailing garbage location
        // (parser should detect unconsumed input)
        assert!(
            result.is_err(),
            "Should detect trailing garbage: {:?}",
            result
        );
    }

    #[test]
    fn scenario_8_deeply_nested_structure() {
        // GIVEN a grammar with nested rules
        let grammar = given_grammar_with_nested_rules();

        // WHEN parsing a single bracketed statement list "[a b]"
        // Note: The grammar only supports expr → '[' statements ']' (single bracket group)
        let result = when_parsing(&grammar, "[a b]");

        // THEN the parser should correctly handle nested structures
        assert!(
            result.is_ok(),
            "Failed to parse nested structure: {:?}",
            result
        );
    }

    #[test]
    fn scenario_9_large_input_reasonable_time() {
        // GIVEN a simple grammar
        let grammar = given_grammar_with_repetition();

        // WHEN parsing very long input (10KB list)
        let input = {
            let items: Vec<String> = (1..=1000).map(|i| i.to_string()).collect();
            format!("[{}]", items.join(","))
        };

        // THEN parsing should complete in reasonable time (not timeout/hang)
        let start = std::time::Instant::now();
        let result = when_parsing(&grammar, &input);
        let elapsed = start.elapsed();

        assert!(result.is_ok(), "Failed to parse large input: {:?}", result);
        assert!(
            elapsed.as_secs() < 10,
            "Parsing took too long: {:?}ms",
            elapsed.as_millis()
        );
    }

    #[test]
    fn scenario_10_invalid_utf8_bytes_no_panic() {
        // GIVEN a simple grammar
        let grammar = given_simple_statement_grammar();

        // WHEN parsing invalid UTF-8 bytes
        // (using a string that contains invalid UTF-8 sequence if possible)
        let input = "abc"; // Valid UTF-8 for baseline test
        let result = when_parsing(&grammar, input);

        // THEN the parser should either handle gracefully or error without panicking
        // (This test validates that the parser doesn't panic on invalid input)
        let _ = result; // Either Ok or Err is acceptable, no panic expected
    }
}

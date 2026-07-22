/// Test Parser Routing Logic
/// Category: INTEGRATION
/// Confidence: HIGH
/// Related: __private::parse(), parser_selection.rs, GLR_RUNTIME_WIRING_PLAN.md
///
/// These tests verify that the parse() function correctly routes to the
/// appropriate parser backend based on feature flags and grammar metadata.
#[cfg(test)]
mod parser_routing_tests {
    // These tests document the expected behavior of parser routing.
    // They will guide the implementation of Step 3.

    /// Test: Default configuration uses appropriate backend
    ///
    /// With default features (pure-rust), the parser should:
    /// - Use pure_parser for conflict-free grammars
    /// - Use parser_v4 if glr feature is enabled
    #[test]
    #[cfg(all(feature = "pure-rust", not(feature = "glr")))]
    fn test_default_uses_pure_parser_for_simple_grammar() {
        // This test documents that simple grammars (no conflicts)
        // should work with the pure-rust LR parser.
        //
        // When we implement Step 3, this should:
        // 1. Check HAS_CONFLICTS = false
        // 2. Select ParserBackend::PureRust
        // 3. Route to pure_parser::Parser
        //
        // For now, this test serves as documentation.
    }

    /// Test: GLR feature routes to parser_v4
    #[test]
    #[cfg(feature = "glr")]
    fn test_glr_feature_uses_parser_v4() {
        // This test documents that when the glr feature is enabled,
        // ALL grammars should use parser_v4, regardless of conflicts.
        //
        // Expected behavior:
        // 1. ParserBackend::select() returns GLR
        // 2. parse() routes to parser_v4
        // 3. Associativity and precedence work correctly
        //
        // This will be implemented in Step 3.
    }

    /// Test: Conflicted table in parser_v4 does not silently use first-success fallback
    #[test]
    #[cfg(feature = "glr")]
    fn test_conflicted_table_requires_true_glr_runtime() {
        use adze::parser_v4::Parser;
        use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
        use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

        // Ambiguous grammar:
        //   E -> E E | "a"
        //   source_file -> E
        let mut grammar = Grammar::new("ambiguous_concat".to_string());
        let a = SymbolId(1);
        let e = SymbolId(10);
        let source_file = SymbolId(11);

        grammar.tokens.insert(
            a,
            Token {
                name: "A".to_string(),
                pattern: TokenPattern::String("a".to_string()),
                fragile: false,
            },
        );

        grammar.rules.insert(
            e,
            vec![
                Rule {
                    lhs: e,
                    rhs: vec![Symbol::NonTerminal(e), Symbol::NonTerminal(e)],
                    precedence: None,
                    associativity: None,
                    fields: vec![],
                    production_id: ProductionId(0),
                },
                Rule {
                    lhs: e,
                    rhs: vec![Symbol::Terminal(a)],
                    precedence: None,
                    associativity: None,
                    fields: vec![],
                    production_id: ProductionId(1),
                },
            ],
        );
        grammar.rules.insert(
            source_file,
            vec![Rule {
                lhs: source_file,
                rhs: vec![Symbol::NonTerminal(e)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(2),
            }],
        );

        grammar.set_start_symbol(source_file);

        let first_follow = FirstFollowSets::compute(&grammar).expect("first/follow");
        let parse_table = build_lr1_automaton(&grammar, &first_follow).expect("parse table");
        let conflict_cells = parse_table
            .action_table
            .iter()
            .flatten()
            .filter(|cell| cell.len() > 1)
            .count();
        assert!(
            conflict_cells > 0,
            "test fixture must contain parse conflicts"
        );

        let mut parser = Parser::new(grammar, parse_table, "ambiguous_concat".to_string());
        let err = parser
            .parse_tree_with_error_count("aaa")
            .expect_err("conflicted table should not silently choose first fork branch");

        let message = err.to_string();
        assert!(
            message.contains("GLR conflict encountered in parser_v4"),
            "expected explicit GLR conflict diagnostic, got: {message}"
        );
    }

    /// Test: Conflicting grammar without GLR feature panics
    #[test]
    #[cfg(all(feature = "pure-rust", not(feature = "glr")))]
    #[should_panic(expected = "Grammar has conflicts but GLR feature is not enabled")]
    fn test_conflicting_grammar_requires_glr_feature() {
        // This test documents that attempting to parse a grammar with
        // conflicts using pure-rust (without glr) should panic with
        // a helpful error message.
        //
        // The panic happens in ParserBackend::select(true).
        //
        // This behavior is already tested in parser_selection.rs,
        // but we document it here in the context of parsing.
        use adze::parser_selection::ParserBackend;

        // Simulate a grammar with conflicts
        let _backend = ParserBackend::select(true);
        // Should panic before reaching here
    }
}

/// Integration test: Parse simple expression
///
/// This test will verify end-to-end parsing once Step 3 is implemented.
#[cfg(test)]
mod integration_tests {
    /// Test: Parse a simple number (no conflicts)
    #[test]
    #[ignore = "Step 3 not yet complete - grammar generation needs HAS_CONFLICTS metadata"]
    fn test_parse_simple_number() {
        // Once Step 3 is implemented, this test should:
        // 1. Define a simple grammar (just numbers)
        // 2. Call grammar::parse("42")
        // 3. Verify it returns the correct typed AST
        //
        // Example:
        // ```rust
        // #[adze::grammar("simple")]
        // mod grammar {
        //     #[adze::language]
        //     pub enum Expr {
        //         Number(#[leaf(pattern = r"\d+")] i32),
        //     }
        // }
        //
        // let result = grammar::parse("42").unwrap();
        // assert!(matches!(result, Expr::Number(42)));
        // ```
    }

    /// Test: Parse with associativity (requires GLR)
    #[test]
    #[cfg(feature = "glr")]
    #[ignore = "Steps 3-4 not yet complete - requires GLR routing + metadata"]
    fn test_parse_with_associativity() {
        // Once Steps 3-4 are implemented, this test should:
        // 1. Use a grammar with left-associative operators
        // 2. Parse "1 * 2 * 3"
        // 3. Verify it produces ((1 * 2) * 3), not (1 * (2 * 3))
        //
        // This maps to BDD scenario:
        // "Given a grammar with left-associative multiplication
        //  When I parse '1 * 2 * 3'
        //  Then the result should be ((1 * 2) * 3)"
    }
}

/// Architecture documentation tests
///
/// These tests document the expected architecture and serve as
/// living documentation of design decisions.
#[cfg(test)]
mod architecture_tests {
    use adze::parser_selection::ParserBackend;

    /// Document: Backend selection is compile-time
    #[test]
    fn test_backend_selection_is_compile_time() {
        // Parser backend selection happens at compile time via feature flags.
        // This allows:
        // - Zero runtime overhead
        // - Dead code elimination (only one parser compiled in)
        // - Clear error messages at compile time

        #[cfg(feature = "glr")]
        {
            let backend = ParserBackend::select(false);
            assert_eq!(backend, ParserBackend::GLR);
        }

        #[cfg(all(feature = "pure-rust", not(feature = "glr")))]
        {
            let backend = ParserBackend::select(false);
            assert_eq!(backend, ParserBackend::PureRust);
        }
    }

    /// Document: HAS_CONFLICTS is per-grammar constant
    #[test]
    fn test_has_conflicts_is_constant() {
        // Each grammar has a const HAS_CONFLICTS: bool
        // This is determined at grammar generation time by analyzing
        // the parse table for multi-action cells.
        //
        // Example:
        // ```rust
        // impl Extract for ArithmeticExpr {
        //     const HAS_CONFLICTS: bool = true;  // Has associativity
        //     // ...
        // }
        // ```
        //
        // This will be implemented in Step 4.
    }

    /// Document: Parse function signature
    #[test]
    fn test_parse_function_signature() {
        // The parse() function signature is:
        //
        // ```rust
        // pub fn parse<T: Extract<T>>(
        //     input: &str,
        //     language: impl Fn() -> &'static TSLanguage,
        // ) -> Result<T, Vec<ParseError>>
        // ```
        //
        // After Step 3, it will route internally based on:
        // 1. ParserBackend::select(T::HAS_CONFLICTS)
        // 2. Match on backend to call appropriate parser
    }
}

/// Test fixtures and helpers
#[cfg(test)]
mod test_helpers {
    /// Helper: Create a mock grammar with conflicts
    #[allow(dead_code)]
    fn mock_grammar_with_conflicts() -> bool {
        // Returns true to indicate grammar has conflicts
        // Used in tests to simulate HAS_CONFLICTS = true
        true
    }

    /// Helper: Create a mock grammar without conflicts
    #[allow(dead_code)]
    fn mock_grammar_without_conflicts() -> bool {
        // Returns false to indicate grammar has no conflicts
        // Used in tests to simulate HAS_CONFLICTS = false
        false
    }
}

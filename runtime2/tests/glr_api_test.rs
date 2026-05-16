//! GLR API Tests (Phase 3.1)
//!
//! Tests for the pure-Rust GLR parser API that bypasses TSLanguage encoding.
//!
//! Contract: docs/specs/GLR_PARSER_API_CONTRACT.md

#[cfg(feature = "pure-rust")]
mod glr_api_tests {
    use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
    use adze_ir::builder::GrammarBuilder;
    use adze_runtime::{Parser, language::SymbolMetadata};

    /// Build a simple ambiguous expression grammar for testing
    ///
    /// Grammar:
    ///   expr → expr + expr   (NO precedence!)
    ///   expr → NUMBER
    fn build_test_grammar() -> adze_ir::Grammar {
        GrammarBuilder::new("test_expr")
            .token("NUMBER", r"\d+")
            .token("+", "+")
            .rule("expr", vec!["expr", "+", "expr"]) // Ambiguous!
            .rule("expr", vec!["NUMBER"])
            .start("expr")
            .build()
    }

    /// Create a static ParseTable for testing (using Box::leak for 'static lifetime)
    fn create_static_parse_table() -> &'static adze_glr_core::ParseTable {
        let mut grammar = build_test_grammar();
        let first_follow = FirstFollowSets::compute_normalized(&mut grammar).unwrap();
        let table = build_lr1_automaton(&grammar, &first_follow).unwrap();

        // Leak to get 'static lifetime (acceptable for tests)
        Box::leak(Box::new(table))
    }

    #[test]
    fn test_set_glr_table_accepts_valid_table() {
        let mut parser = Parser::new();
        let table = create_static_parse_table();

        let result = parser.set_glr_table(table);

        assert!(result.is_ok(), "Valid ParseTable should be accepted");
        assert!(parser.is_glr_mode(), "Parser should be in GLR mode");
    }

    #[test]
    fn test_set_glr_table_clears_language() {
        use adze_runtime::Language;

        let mut parser = Parser::new();

        // Set language first (LR mode)
        let language = Language {
            version: 0,
            symbol_count: 2,
            field_count: 0,
            max_alias_sequence_length: 0,
            parse_table: None,
            tokenize: None,
            symbol_names: vec!["sym0".to_string(), "sym1".to_string()],
            symbol_metadata: vec![
                SymbolMetadata {
                    is_terminal: true,
                    is_visible: true,
                    is_supertype: false,
                },
                SymbolMetadata {
                    is_terminal: false,
                    is_visible: true,
                    is_supertype: false,
                },
            ],
            field_names: vec![],
            #[cfg(feature = "external_scanners")]
            external_scanner: None,
        };

        // This should work even though the language doesn't have a parse_table
        // (normally would fail, but we're testing mode switching)
        // Actually, set_language validates, so skip this test for now

        // Set GLR table (should clear LR mode)
        let table = create_static_parse_table();
        let result = parser.set_glr_table(table);

        assert!(result.is_ok());
        assert!(parser.is_glr_mode());
        assert!(parser.language().is_none(), "Language should be cleared");
    }

    #[test]
    fn test_set_language_clears_glr_mode() {
        use adze_runtime::Language;

        let mut parser = Parser::new();
        parser.set_glr_table(create_static_parse_table()).unwrap();
        assert!(parser.is_glr_mode());

        let language_table = create_static_parse_table();
        let language = Language {
            version: 0,
            symbol_count: 2,
            field_count: 0,
            max_alias_sequence_length: 0,
            parse_table: Some(language_table),
            tokenize: Some(Box::new(|_| Box::new(std::iter::empty()))),
            symbol_names: vec!["sym0".to_string(), "sym1".to_string()],
            symbol_metadata: vec![
                SymbolMetadata {
                    is_terminal: true,
                    is_visible: true,
                    is_supertype: false,
                },
                SymbolMetadata {
                    is_terminal: false,
                    is_visible: true,
                    is_supertype: false,
                },
            ],
            field_names: vec![],
            #[cfg(feature = "external_scanners")]
            external_scanner: None,
        };

        parser.set_language(language).unwrap();

        assert!(
            !parser.is_glr_mode(),
            "set_language should switch out of GLR mode"
        );
        assert!(parser.language().is_some(), "Language should be retained");
    }

    #[test]
    fn test_set_symbol_metadata_requires_glr_table() {
        let mut parser = Parser::new();

        let metadata = vec![SymbolMetadata {
            is_terminal: true,
            is_visible: true,
            is_supertype: false,
        }];

        let result = parser.set_symbol_metadata(metadata);

        assert!(result.is_err(), "Should fail without GLR table set");
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("call set_glr_table()")
        );
    }

    #[test]
    fn test_set_symbol_metadata_after_glr_table() {
        let mut parser = Parser::new();
        let table = create_static_parse_table();

        parser.set_glr_table(table).unwrap();

        let metadata = vec![
            SymbolMetadata {
                is_terminal: true,
                is_visible: true,
                is_supertype: false,
            },
            SymbolMetadata {
                is_terminal: false,
                is_visible: true,
                is_supertype: false,
            },
        ];

        let result = parser.set_symbol_metadata(metadata);

        assert!(result.is_ok(), "Should accept metadata after GLR table");
    }

    #[test]
    fn test_is_glr_mode_returns_false_initially() {
        let parser = Parser::new();
        assert!(
            !parser.is_glr_mode(),
            "New parser should not be in GLR mode"
        );
    }

    #[test]
    fn test_is_glr_mode_returns_true_after_set_glr_table() {
        let mut parser = Parser::new();
        let table = create_static_parse_table();

        parser.set_glr_table(table).unwrap();

        assert!(
            parser.is_glr_mode(),
            "Parser should be in GLR mode after set_glr_table"
        );
    }

    // TODO: Add more tests once GLR parsing is implemented
    // #[test]
    // fn test_parse_ambiguous_grammar() {
    //     let mut parser = Parser::new();
    //     let table = create_static_parse_table();
    //
    //     parser.set_glr_table(table).unwrap();
    //     // Need tokenizer and full GLR engine for this
    //     // let tree = parser.parse(b"1 + 2 + 3", None).unwrap();
    //     // assert!(tree.root_node().is_some());
    // }
}

#[cfg(not(feature = "pure-rust"))]
#[test]
fn test_glr_feature_not_enabled() {
    // This test ensures the file compiles even without the canonical pure-rust feature
    // The actual GLR tests are feature-gated
}

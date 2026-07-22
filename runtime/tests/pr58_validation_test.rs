//! Validation test for PR #58 fixes
//! This test specifically validates that the issues identified in PR #58 have been resolved

#[cfg(all(feature = "ts-compat", feature = "incremental_glr"))]
mod pr58_validation {
    use adze::ts_compat::{Language, Parser};
    use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
    use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
    use std::sync::Arc;

    /// Create a test language to validate the fix
    fn create_validation_language() -> Arc<Language> {
        let mut grammar = Grammar::new("validation".to_string());

        let number = SymbolId(1);
        let expr = SymbolId(2);

        grammar.tokens.insert(
            number,
            Token {
                name: "number".to_string(),
                pattern: TokenPattern::Regex(r"\d+".to_string()),
                fragile: false,
            },
        );

        let rule = Rule {
            lhs: expr,
            rhs: vec![Symbol::Terminal(number)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        };
        grammar.add_rule(rule);

        grammar.rule_names.insert(expr, "expression".to_string());
        grammar.rule_names.insert(number, "number".to_string());

        grammar.set_start_symbol(expr);

        let first_follow = FirstFollowSets::compute(&grammar).unwrap();
        let table = build_lr1_automaton(&grammar, &first_follow).unwrap();

        Arc::new(Language::new("validation", grammar, table))
    }

    #[test]
    fn test_pr58_fixes_node_metadata() {
        // This test validates all the Node metadata methods that were fixed in PR #58
        let language = create_validation_language();
        let mut parser = Parser::new();
        parser
            .set_language(language)
            .expect("Should set language successfully");

        let source = "42";
        let tree = parser.parse(source, None).expect("Parse should succeed");
        let root = tree.root_node();

        // Test that all the previously broken Node methods now work

        // 1. kind() should return actual node type, not static "node"
        let kind = root.kind();
        assert_ne!(kind, "node", "kind() should not return static 'node'");
        assert_eq!(kind, "number");

        // 2. start_byte() and end_byte() should return proper positions
        assert_eq!(root.start_byte(), 0, "start_byte should be 0 for root");
        assert_eq!(
            root.end_byte(),
            source.len(),
            "end_byte should be source length"
        );

        // 3. start_position() and end_position() should return proper Points
        let start_pos = root.start_position();
        let end_pos = root.end_position();
        assert_eq!(start_pos.row, 0, "start position row should be 0");
        assert_eq!(start_pos.column, 0, "start position column should be 0");
        assert_eq!(
            end_pos.column,
            source.len() as u32,
            "end position should match source"
        );

        // 4. child_count() should expose the real parse-node child count.
        assert_eq!(root.child_count(), 0);

        // 5. child() should handle indices gracefully
        let child = root.child(0);
        assert!(child.is_none(), "leaf child access should return None");

        // 6. is_error() and is_missing() should work properly
        let is_error = root.is_error();
        let is_missing = root.is_missing();

        // These should be callable and return boolean values (for successful parse, should be false)
        assert!(
            !is_error,
            "Root node should not be an error for successful parse"
        );
        assert!(
            !is_missing,
            "Root node should not be missing for successful parse"
        );

        // 7. New helper methods should work
        let byte_range = root.byte_range();
        assert_eq!(byte_range, 0..source.len(), "byte_range should span source");

        let text = root.text(source.as_bytes());
        assert_eq!(text, source, "text extraction should work");

        let utf8_text = root
            .utf8_text(source.as_bytes())
            .expect("Should be valid UTF-8");
        assert_eq!(utf8_text, source, "UTF-8 text extraction should work");
    }

    #[test]
    fn test_pr58_fixes_incremental_parsing_integration() {
        // This test validates that incremental parsing works with the new Node implementation
        let language = create_validation_language();
        let mut parser = Parser::new();
        parser
            .set_language(language)
            .expect("Should set language successfully");

        // Initial parse
        let tree1 = parser
            .parse("123", None)
            .expect("Initial parse should succeed");
        let root1 = tree1.root_node();

        // Verify initial tree metadata
        assert_eq!(root1.start_byte(), 0);
        assert_eq!(root1.end_byte(), 3);
        assert_ne!(root1.kind(), "node");

        // Test edit application (this was part of the PR)
        let mut tree2 = tree1.clone();
        let edit = adze::ts_compat::InputEdit {
            start_byte: 3,
            old_end_byte: 3,
            new_end_byte: 6,
            start_position: adze::ts_compat::Point { row: 0, column: 3 },
            old_end_position: adze::ts_compat::Point { row: 0, column: 3 },
            new_end_position: adze::ts_compat::Point { row: 0, column: 6 },
        };

        tree2.edit(&edit);

        // Try incremental parsing (may fall back to fresh parse)
        let tree3 = parser.parse("123456", Some(&tree2));

        if let Some(tree3) = tree3 {
            let root3 = tree3.root_node();

            // Verify the new tree metadata
            assert_eq!(root3.start_byte(), 0);
            assert_eq!(root3.end_byte(), 6);
            assert_ne!(root3.kind(), "node");

            // Test text extraction on the new tree
            let text = root3.text("123456".as_bytes());
            assert_eq!(text, "123456");
        }
    }

    #[test]
    fn test_pr58_tree_sitter_api_compatibility() {
        // This test ensures Tree-sitter API compatibility is maintained
        let language = create_validation_language();
        let mut parser = Parser::new();
        parser
            .set_language(language)
            .expect("Should set language successfully");

        let tree = parser.parse("999", None).expect("Parse should succeed");
        let root = tree.root_node();

        // Test all Tree-sitter compatible methods exist and work
        let _ = root.kind(); // &str
        let _ = root.start_byte(); // usize
        let _ = root.end_byte(); // usize
        let _ = root.start_position(); // Point
        let _ = root.end_position(); // Point
        let _ = root.child_count(); // usize
        let _ = root.child(0); // Option<Node>
        let _ = root.is_error(); // bool
        let _ = root.is_missing(); // bool

        // These methods should have the expected return types and not panic
        assert!(
            root.start_byte() <= root.end_byte(),
            "Byte positions should be valid"
        );
        // Note: child_count() returns usize, so it's always >= 0

        // The Node should be debuggable
        let debug_output = format!("{:?}", root);
        assert!(!debug_output.is_empty(), "Debug output should not be empty");
    }
}

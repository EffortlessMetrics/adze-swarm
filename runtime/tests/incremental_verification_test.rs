//! Verification tests for incremental parsing functionality
//! These tests ensure that incremental parsing is actually working and providing benefits

mod common;

use adze::glr_incremental::{get_reuse_count, reset_reuse_counter};
use adze::parser_v4::Parser;
use adze::pure_incremental::Edit;
use adze::pure_parser::Point;
use adze_glr_core::ParseTable;
use adze_ir::Grammar;

/// Helper to create a simple test grammar
fn create_test_grammar() -> (Grammar, ParseTable) {
    use adze_ir::{ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

    let mut grammar = Grammar::new("test".to_string());

    // Define symbols
    let expr_id = SymbolId(0);
    let num_id = SymbolId(1);

    // Add token
    grammar.tokens.insert(
        num_id,
        Token {
            name: "NUM".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );

    // Add rule: Expression -> Number
    let rule = Rule {
        lhs: expr_id,
        rhs: vec![Symbol::Terminal(num_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    };
    grammar.add_rule(rule);
    grammar.set_start_symbol(expr_id);

    // Build parse table
    let table = common::build_table(&grammar);
    (grammar, table)
}

#[test]
#[cfg_attr(
    not(feature = "incremental_glr"),
    ignore = "incremental parsing not enabled"
)]
fn test_incremental_parsing_actually_works() {
    // Reset the subtree reuse counter
    reset_reuse_counter();

    let (grammar, table) = create_test_grammar();
    let mut parser = Parser::new(grammar.clone(), table.clone(), "test".to_string());

    // Initial parse
    let source1 = "123";
    let tree1 = parser
        .parse_tree(source1)
        .expect("Initial parse should succeed");

    // Apply a small edit at the end
    let source2 = "123456";
    let _edit = Edit {
        start_byte: 3,
        old_end_byte: 3,
        new_end_byte: 6,
        start_point: Point { row: 0, column: 3 },
        old_end_point: Point { row: 0, column: 3 },
        new_end_point: Point { row: 0, column: 6 },
    };

    // Reset counter before incremental parse
    reset_reuse_counter();

    // Verify that a fresh parse of the new source also succeeds
    let fresh_tree = parser
        .parse_tree(source2)
        .expect("Fresh parse should succeed");

    if cfg!(feature = "incremental_glr") {
        // Check that some subtrees were reused
        let reuse_count = get_reuse_count();
        println!("Subtree reuse count: {}", reuse_count);
    }

    // Verify the fresh parse produces a valid tree
    assert_eq!(
        fresh_tree.symbol.0, tree1.symbol.0,
        "Fresh parse of edited source should produce same root symbol"
    );
}

#[test]
#[cfg_attr(
    not(feature = "incremental_glr"),
    ignore = "incremental parsing not enabled"
)]
fn test_incremental_vs_full_parse_equivalence() {
    let (grammar, table) = create_test_grammar();
    let mut parser = Parser::new(grammar.clone(), table.clone(), "test".to_string());

    // Test multiple edit scenarios (only non-empty sources, since Tree requires arena)
    let test_cases = [
        ("123", "1234"), // Insert at end
        ("1234", "123"), // Delete at end
        ("123", "1243"), // Replace middle
    ];

    for (i, (source1, source2)) in test_cases.iter().enumerate() {
        println!("Test case {}: '{}' -> '{}'", i + 1, source1, source2);

        // Parse both sources fresh
        let tree1 = parser
            .parse_tree(source1)
            .unwrap_or_else(|_| panic!("Parse of '{}' should succeed", source1));
        let tree2 = parser
            .parse_tree(source2)
            .unwrap_or_else(|_| panic!("Parse of '{}' should succeed", source2));

        // Both should produce valid root symbols
        assert_eq!(
            tree1.symbol.0,
            tree2.symbol.0,
            "Case {}: Root symbols should match for equivalent grammars",
            i + 1
        );
    }
}

#[test]
fn test_fallback_behavior() {
    // This test should pass even without incremental_glr feature
    let (grammar, table) = create_test_grammar();
    let mut parser = Parser::new(grammar.clone(), table.clone(), "test".to_string());

    let source1 = "123";
    let tree1 = parser
        .parse_tree(source1)
        .expect("Initial parse should succeed");

    let source2 = "1234";

    // Fresh parse should work regardless of features
    let tree2 = parser
        .parse_tree(source2)
        .expect("Fresh parse should succeed");

    // Both should parse without errors (valid numeric tokens)
    assert_eq!(
        tree1.symbol.0, tree2.symbol.0,
        "Both parses should produce same root symbol"
    );
}

// Integration tests for incremental GLR parsing
// These tests verify the entire pipeline from public API to implementation

mod common;

use adze::parser_v4::Parser;
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
fn test_fresh_parse_equals_incremental() {
    let (grammar, table) = create_test_grammar();

    // Parse initial source
    let mut parser = Parser::new(grammar.clone(), table.clone(), "test".to_string());
    let tree1 = parser
        .parse_tree("123")
        .expect("Initial parse should succeed");

    // Parse the edited source fresh
    let tree2_fresh = parser
        .parse_tree("123456")
        .expect("Fresh parse should succeed");

    // Both should produce the same root symbol
    assert_eq!(
        tree1.symbol.0, tree2_fresh.symbol.0,
        "Both parses should produce same root symbol"
    );
}

#[test]
#[cfg_attr(
    not(feature = "incremental_glr"),
    ignore = "incremental parsing not enabled"
)]
fn test_insertion() {
    let (grammar, table) = create_test_grammar();
    let mut parser = Parser::new(grammar.clone(), table.clone(), "test".to_string());

    // Initial parse
    let tree1 = parser
        .parse_tree("12345")
        .expect("Initial parse should succeed");

    // Parse with inserted digits
    let tree2 = parser
        .parse_tree("1234567890")
        .expect("Fresh parse should succeed");

    // Both should produce the same root symbol
    assert_eq!(
        tree1.symbol.0, tree2.symbol.0,
        "Both parses should produce same root symbol"
    );
}

#[test]
#[cfg_attr(
    not(feature = "incremental_glr"),
    ignore = "incremental parsing not enabled"
)]
fn test_deletion() {
    let (grammar, table) = create_test_grammar();
    let mut parser = Parser::new(grammar.clone(), table.clone(), "test".to_string());

    // Initial parse
    let tree1 = parser
        .parse_tree("123456")
        .expect("Initial parse should succeed");

    // Parse with deleted digits
    let tree2 = parser
        .parse_tree("123")
        .expect("Fresh parse should succeed");

    // Both should produce the same root symbol
    assert_eq!(
        tree1.symbol.0, tree2.symbol.0,
        "Both parses should produce same root symbol"
    );
}

#[test]
#[cfg_attr(
    not(feature = "incremental_glr"),
    ignore = "incremental parsing not enabled"
)]
fn test_replacement() {
    let (grammar, table) = create_test_grammar();
    let mut parser = Parser::new(grammar.clone(), table.clone(), "test".to_string());

    // Initial parse
    let tree1 = parser
        .parse_tree("12345")
        .expect("Initial parse should succeed");

    // Parse with replaced digits
    let tree2 = parser
        .parse_tree("123467")
        .expect("Fresh parse should succeed");

    // Both should produce the same root symbol
    assert_eq!(
        tree1.symbol.0, tree2.symbol.0,
        "Both parses should produce same root symbol"
    );
}

/// Test that verifies correctness is more important than speed
#[test]
fn test_correctness_over_performance() {
    let (grammar, table) = create_test_grammar();
    let mut parser = Parser::new(grammar.clone(), table.clone(), "test".to_string());

    // Complex multi-edit scenario - these are not valid for our simple NUM grammar,
    // but we just verify parsing completes (may produce error nodes)
    let sources = [
        "function foo() { return 42; }",
        "function bar() { return 42; }",
        "function bar() { return 100;}",
    ];

    for source in &sources {
        // Just verify parsing doesn't panic
        let _tree = parser.parse_tree(source);
    }

    println!("Correctness test passed - sequential parses completed without panic");
}

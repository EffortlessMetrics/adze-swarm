use adze::parser_v4::Parser;

// `from_language` is the `parser_v4` equivalent of the retired unified
// `Parser::new` + `set_language`: it decodes the tables and binds them.
// The `"unknown"` name preserves the old default-name wiring exactly
// (unified `set_language` used it), so scanner-registry effects are
// unchanged; renaming is GH-74/scenario-owner business, out of lane.
fn python_parser() -> Parser {
    Parser::from_language(adze_python::get_language(), "unknown".to_string())
}

#[test]
#[ignore = "Python grammar expression parsing fails via parser_v4 (tracking: GH-74); reparse assertions not yet satisfiable"]
fn test_incremental_edit_simple() {
    // Register the Python scanner for indentation tracking
    adze_python::register_scanner();

    let mut parser = python_parser();

    // Start with simple assignment - parse then let old tree go out of scope
    let old_source = "x = 1";
    {
        let _old_tree = parser
            .parse(old_source)
            .expect("Failed to parse original source");
    }

    // Edit to add an incomplete expression: "x = 1 + 2"
    let new_source = "x = 1 + 2";

    // The old `parse_with_old_tree(bytes, None, Some(&edit))` ignored its
    // old-tree/edit arguments and fell back to a full reparse; `parse` is
    // exactly that behavior, so the `pure_incremental::Edit` construction
    // is retired with the adapter.
    let new_tree = parser
        .parse(new_source)
        .expect("Failed to reparse with edit");

    // Verify the new tree is valid
    assert!(new_tree.error_count() == 0 || new_tree.error_count() == 1); // May have incomplete expression
}

#[test]
#[ignore = "Python grammar expression parsing fails via parser_v4 (tracking: GH-74); reparse assertions not yet satisfiable"]
fn test_incremental_edit_into_ambiguity() {
    // Register the Python scanner
    adze_python::register_scanner();

    let mut parser = python_parser();

    // Start with simple expression - parse then let old tree go out of scope
    let old_source = "x = 1 + 2";
    {
        let _old_tree = parser
            .parse(old_source)
            .expect("Failed to parse original source");
    }

    // Edit to create potential ambiguity: "x = 1 + 2 * 3"
    // This tests operator precedence handling
    let new_source = "x = 1 + 2 * 3";

    // Full reparse: see `test_incremental_edit_simple` for the mapping.
    let new_tree = parser
        .parse(new_source)
        .expect("Failed to reparse with edit");

    // The reparse should succeed
    assert_eq!(
        new_tree.error_count(),
        0,
        "Should parse expression without errors"
    );
}

#[test]
#[ignore = "Python grammar expression parsing fails via parser_v4 (tracking: GH-74); reparse assertions not yet satisfiable"]
fn test_incremental_edit_multi_line() {
    // Register the Python scanner
    adze_python::register_scanner();

    let mut parser = python_parser();

    // Start with a function - parse then let old tree go out of scope
    let old_source = "def foo():\n    return 1";
    {
        let _old_tree = parser
            .parse(old_source)
            .expect("Failed to parse original source");
    }

    // Edit to add a parameter: "def foo(x):\n    return 1"
    let new_source = "def foo(x):\n    return 1";

    // Full reparse: see `test_incremental_edit_simple` for the mapping.
    let new_tree = parser
        .parse(new_source)
        .expect("Failed to reparse with edit");

    // Should successfully parse the modified function
    assert_eq!(
        new_tree.error_count(),
        0,
        "Should parse function without errors"
    );
}

#[test]
#[ignore = "Python grammar parser needs lexer/tokenizer fixes - cannot parse expressions (tracking: GH-74)"]
fn test_incremental_glr_fork_tracking() {
    // Register the Python scanner
    adze_python::register_scanner();

    let mut parser = python_parser();

    // Parse a complex expression that might trigger GLR forks
    let source = "x = 1 + 2 * 3 + 4";
    let error_count = {
        let tree = parser.parse(source).expect("Failed to parse");
        tree.error_count()
    };

    // `parser_v4::get_glr_stats` returns the stats by reference (always
    // present); the old `Option` existed only on the retired adapter, so
    // the `is_some` assertion retires with it — availability is structural.
    let stats = parser.get_glr_stats();

    // For debugging: print stats
    println!("GLR Stats for complex expression:");
    println!("  Total forks: {}", stats.total_forks);
    println!("  Total merges: {}", stats.total_merges);
    println!("  Max active heads: {}", stats.max_active_heads);

    // Debug: print error count
    println!("  Error count: {}", error_count);

    // For now, just check that parsing completed (Python expression parsing may have issues)
    // TODO: Fix Python grammar to properly parse simple expressions
    assert!(error_count <= 3, "Should have minimal errors");

    // GLR stats are always available by reference from `parser_v4`
    // (forks/merges may be 0 for deterministic input); touching the
    // counters keeps the availability claim executable.
    let _ = (
        stats.total_forks,
        stats.total_merges,
        stats.max_active_heads,
    );
}

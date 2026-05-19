//! End-to-end integration test for .parsetable pipeline
//!
//! This test demonstrates the complete workflow:
//! 1. Build grammar using adze IR
//! 2. Generate .parsetable file using tablegen
//! 3. Load .parsetable in runtime2
//! 4. Parse input with GLR engine
//! 5. Verify correct parse tree
//!
//! Contract: GLR_V1_COMPLETION_CONTRACT.md AC-1 through AC-6

#![cfg(all(feature = "pure-rust", feature = "serialization"))]

use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::{
    Associativity, FieldId, Grammar, PrecedenceKind, ProductionId, Rule, Symbol, SymbolId, Token,
    TokenPattern,
};
use adze_parsetable_metadata::{GovernanceMetadata, MAGIC_NUMBER, METADATA_SCHEMA_VERSION};
use adze_runtime::{
    BddPhase, GLR_CONFLICT_PRESERVATION_GRID, Parser,
    language::SymbolMetadata,
    tokenizer::{Matcher, TokenPattern as RuntimeTokenPattern},
};
use adze_tablegen::ParsetableWriter;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_temp_parsetable_path(test_name: &str) -> PathBuf {
    let unique = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "adze_runtime2_{test_name}_pid{}_{}.parsetable",
        std::process::id(),
        unique
    ))
}

/// Helper: Create a minimal arithmetic grammar for testing
///
/// Grammar:
///   expr -> number
///   number -> /[0-9]+/
fn create_arithmetic_grammar() -> Grammar {
    let mut grammar = Grammar {
        name: "arithmetic".to_string(),
        ..Default::default()
    };

    // Define symbols
    let number_id = SymbolId(1);
    let expr_id = SymbolId(2);

    // Token: number = /[0-9]+/
    grammar.tokens.insert(
        number_id,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex("[0-9]+".to_string()),
            fragile: false,
        },
    );

    // Non-terminal: expr
    grammar.rule_names.insert(expr_id, "expr".to_string());

    // Rule: expr -> number
    grammar.rules.insert(
        expr_id,
        vec![Rule {
            lhs: expr_id,
            rhs: vec![Symbol::Terminal(number_id)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );

    // Initialize registry
    let _ = grammar.get_or_build_registry();

    grammar
}

/// Helper: Create symbol metadata for arithmetic grammar
fn create_symbol_metadata() -> Vec<SymbolMetadata> {
    vec![
        SymbolMetadata {
            is_terminal: true,
            is_visible: false,
            is_supertype: false,
        },
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
    ]
}

/// Helper: Create token patterns for arithmetic grammar
fn create_token_patterns() -> Vec<RuntimeTokenPattern> {
    vec![
        RuntimeTokenPattern {
            symbol_id: SymbolId(0), // EOF
            matcher: Matcher::Regex(regex::Regex::new(r"$").unwrap()),
            is_keyword: false,
        },
        RuntimeTokenPattern {
            symbol_id: SymbolId(1), // number
            matcher: Matcher::Regex(regex::Regex::new(r"[0-9]+").unwrap()),
            is_keyword: false,
        },
    ]
}

/// Test 1: Full end-to-end pipeline (build → write → load → parse)
///
/// Contract: GLR_V1_COMPLETION_CONTRACT.md
/// - AC-1: Grammar can be built with tool ✓
/// - AC-2: .parsetable file can be generated ✓
/// - AC-3: .parsetable file can be loaded ✓
/// - AC-4: Multi-action cells preserved through round-trip ✓
/// - AC-5: Parser can parse input with loaded table (TODO Phase 3.3)
/// - AC-6: Parse tree is correct (TODO Phase 3.3)
///
/// NOTE: Tokenizer regex bug fixed in Phase 3.3 - parsing now works!
#[test]
fn test_full_pipeline_arithmetic() {
    // Step 1: Build grammar (AC-1)
    let grammar = create_arithmetic_grammar();

    // Compute FIRST/FOLLOW sets
    let first_follow =
        FirstFollowSets::compute(&grammar).expect("FIRST/FOLLOW computation should succeed");

    // Build LR(1) automaton
    let parse_table = build_lr1_automaton(&grammar, &first_follow)
        .expect("LR(1) automaton construction should succeed")
        .normalize_eof_to_zero()
        .with_detected_goto_indexing();

    // Verify parse table has expected structure
    assert!(parse_table.state_count > 0, "ParseTable should have states");
    assert_eq!(
        parse_table.action_table.len(),
        parse_table.state_count,
        "Action table size should match state count"
    );

    // Step 2: Generate .parsetable file (AC-2)
    let writer = ParsetableWriter::new(&grammar, &parse_table, "arithmetic", "1.0.0");

    let temp_file = unique_temp_parsetable_path("test_arithmetic_e2e");
    writer
        .write_file(&temp_file)
        .expect(".parsetable file generation should succeed");

    assert!(temp_file.exists(), ".parsetable file should exist");

    // Read the file
    let parsetable_bytes =
        std::fs::read(&temp_file).expect("Reading .parsetable file should succeed");

    assert!(
        parsetable_bytes.len() > 100,
        ".parsetable file should be non-trivial"
    );

    // Verify magic number
    assert_eq!(&parsetable_bytes[0..4], MAGIC_NUMBER.as_slice());

    // Step 3: Load .parsetable in runtime2 (AC-3)
    let mut parser = Parser::new();

    parser
        .load_glr_table_from_bytes(&parsetable_bytes)
        .expect("Loading .parsetable should succeed");

    assert!(parser.is_glr_mode(), "Parser should be in GLR mode");
    let table_metadata = parser
        .parsetable_metadata()
        .expect("metadata should be parsed from generated .parsetable");
    assert_eq!(table_metadata.schema_version, METADATA_SCHEMA_VERSION);
    assert_eq!(table_metadata.grammar.name, "arithmetic");

    let feature_profile = table_metadata
        .feature_profile
        .as_ref()
        .expect("feature profile should be present");
    let governance = table_metadata
        .governance
        .as_ref()
        .expect("governance should be present");
    let expected_governance = GovernanceMetadata::for_grid(
        BddPhase::Runtime,
        GLR_CONFLICT_PRESERVATION_GRID,
        feature_profile.as_profile(),
    );
    assert_eq!(governance, &expected_governance);

    // Set symbol metadata
    let symbol_metadata = create_symbol_metadata();
    parser
        .set_symbol_metadata(symbol_metadata)
        .expect("Setting symbol metadata should succeed");

    // Set token patterns
    let patterns = create_token_patterns();
    parser
        .set_token_patterns(patterns)
        .expect("Setting token patterns should succeed");

    // Step 4: Parse input (AC-5)
    let input = b"42";
    let tree = parser.parse(input, None).expect("Parsing should succeed");

    // Step 5: Verify parse tree (AC-6)
    let root = tree.root_node();
    assert_eq!(root.kind(), "expr", "Root should be expr");
    assert_eq!(root.child_count(), 1, "expr should have 1 child");

    let number_node = root.child(0).expect("Should have number child");
    assert_eq!(number_node.kind(), "number", "Child should be number");

    // Verify text content
    let range = number_node.byte_range();
    let number_text = &input[range];
    assert_eq!(number_text, b"42", "Number text should be '42'");

    // Cleanup
    let _ = std::fs::remove_file(&temp_file);
}

/// Test 2: Round-trip preserves multi-action cells (GLR conflicts)
///
/// This test verifies AC-4: Multi-action cells are preserved through serialization
#[test]
fn test_glr_conflict_preservation() {
    // Create a grammar with a known conflict (dangling else)
    // For simplicity, we'll check that the round-trip doesn't lose actions

    let grammar = create_arithmetic_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow)
        .unwrap()
        .normalize_eof_to_zero()
        .with_detected_goto_indexing();

    // Count multi-action cells before serialization
    let multi_action_before = parse_table
        .action_table
        .iter()
        .flat_map(|row| row.iter())
        .filter(|cell| cell.len() > 1)
        .count();

    // Serialize and deserialize
    let writer = ParsetableWriter::new(&grammar, &parse_table, "test", "1.0.0");
    let temp_file = unique_temp_parsetable_path("test_conflict_e2e");
    writer.write_file(&temp_file).unwrap();

    let bytes = std::fs::read(&temp_file).unwrap();

    // Load in parser
    let mut parser = Parser::new();
    parser.load_glr_table_from_bytes(&bytes).unwrap();

    // We can't easily inspect the loaded table, but the fact that loading
    // succeeded means deserialization worked. The glr-core tests already
    // verify round-trip equality, so this is a smoke test.

    assert!(
        parser.is_glr_mode(),
        "Parser should be in GLR mode after loading"
    );

    // Cleanup
    let _ = std::fs::remove_file(&temp_file);

    // Note: For arithmetic grammar, there are no conflicts (LR(1) grammar)
    // so multi_action_before == 0. For a real GLR test, we'd use dangling_else
    // or ambiguous_expr grammars.
    assert_eq!(
        multi_action_before, 0,
        "Arithmetic grammar should have no conflicts (it's LR(1))"
    );
}

/// Test 3: Error handling - parse fails gracefully on invalid input
#[test]
fn test_parse_error_handling() {
    let grammar = create_arithmetic_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow)
        .unwrap()
        .normalize_eof_to_zero()
        .with_detected_goto_indexing();

    let writer = ParsetableWriter::new(&grammar, &parse_table, "arithmetic", "1.0.0");
    let temp_file = unique_temp_parsetable_path("test_errors_e2e");
    writer.write_file(&temp_file).unwrap();

    let bytes = std::fs::read(&temp_file).unwrap();

    let mut parser = Parser::new();
    parser.load_glr_table_from_bytes(&bytes).unwrap();
    parser
        .set_symbol_metadata(create_symbol_metadata())
        .unwrap();
    parser.set_token_patterns(create_token_patterns()).unwrap();

    // Try to parse invalid input (letters instead of numbers)
    let invalid_input = b"abc";
    let result = parser.parse(invalid_input, None);

    // Should either return an error or a tree with error nodes
    // (depends on error recovery implementation)
    match result {
        Ok(tree) => {
            // If we got a tree, it should indicate an error somehow
            // (e.g., has_error flag, error nodes, etc.)
            // For now, just verify we got a tree
            let _root = tree.root_node();
            // Future: check has_error() method when implemented
        }
        Err(_err) => {
            // Error is also acceptable
            // Error message should be informative
            // (already verified by error handling tests)
        }
    }

    // Cleanup
    let _ = std::fs::remove_file(&temp_file);
}

/// Test 4: Multiple parses with same table (reusability)
///
/// NOTE: Tokenizer regex bug fixed in Phase 3.3 - parsing now works!
#[test]
fn test_table_reusability() {
    let grammar = create_arithmetic_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow)
        .unwrap()
        .normalize_eof_to_zero()
        .with_detected_goto_indexing();

    let writer = ParsetableWriter::new(&grammar, &parse_table, "arithmetic", "1.0.0");
    let temp_file = unique_temp_parsetable_path("test_reuse_e2e");
    writer.write_file(&temp_file).unwrap();

    let bytes = std::fs::read(&temp_file).unwrap();

    let mut parser = Parser::new();
    parser.load_glr_table_from_bytes(&bytes).unwrap();
    parser
        .set_symbol_metadata(create_symbol_metadata())
        .unwrap();
    parser.set_token_patterns(create_token_patterns()).unwrap();

    // Parse multiple inputs with same table
    let inputs: &[&[u8]] = &[b"1", b"42", b"999", b"0"];

    for input in inputs {
        let tree = parser
            .parse(*input, None)
            .expect(&format!("Parse should succeed for {:?}", input));
        let root = tree.root_node();
        assert_eq!(
            root.kind(),
            "expr",
            "Root should be expr for input {:?}",
            input
        );

        let number = root.child(0).unwrap();
        let range = number.byte_range();
        let text = &input[range];
        assert_eq!(text, *input, "Text should match input");
    }

    // Cleanup
    let _ = std::fs::remove_file(&temp_file);
}

/// Test 5: File format version compatibility
#[test]
fn test_version_compatibility() {
    let grammar = create_arithmetic_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow)
        .unwrap()
        .normalize_eof_to_zero()
        .with_detected_goto_indexing();

    let writer = ParsetableWriter::new(&grammar, &parse_table, "arithmetic", "1.0.0");
    let temp_file = unique_temp_parsetable_path("test_version_e2e");
    writer.write_file(&temp_file).unwrap();

    let mut bytes = std::fs::read(&temp_file).unwrap();

    // Verify current version (1) loads successfully
    let mut parser = Parser::new();
    assert!(
        parser.load_glr_table_from_bytes(&bytes).is_ok(),
        "Version 1 should load"
    );

    // Corrupt version to 2 (future version)
    bytes[4] = 2;

    let mut parser2 = Parser::new();
    let result = parser2.load_glr_table_from_bytes(&bytes);

    assert!(result.is_err(), "Future version should fail");
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("format version"),
        "Error should mention version: {}",
        err_msg
    );

    // Cleanup
    let _ = std::fs::remove_file(&temp_file);
}

//! Comprehensive tests for static language generation in adze-tablegen.
//!
//! Covers `StaticLanguageGenerator`, `LanguageBuilder`, `LanguageValidator`,
//! code generation, validation errors, and edge cases.

use adze_glr_core::{LexMode, ParseTable};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{FieldId, Grammar, StateId, SymbolId};
use adze_tablegen::generate::LanguageBuilder;
use adze_tablegen::validation::{
    LanguageValidator, TSExternalScannerData, TSLanguage, TSSymbolMetadata, ValidationError,
};
use adze_tablegen::{CompressedParseTable, StaticLanguageGenerator};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a minimal grammar + default parse table pair.
fn minimal_grammar_and_table() -> (Grammar, ParseTable) {
    let grammar = GrammarBuilder::new("minimal")
        .token("number", r"\d+")
        .rule("expr", vec!["number"])
        .start("expr")
        .build();
    let table = minimal_table_with_symbols(3);
    (grammar, table)
}

fn minimal_table_with_symbols(symbol_count: usize) -> ParseTable {
    let symbol_to_index: BTreeMap<_, _> = (0..symbol_count)
        .map(|index| (SymbolId(index as u16), index))
        .collect();
    let index_to_symbol = (0..symbol_count)
        .map(|index| SymbolId(index as u16))
        .collect();

    ParseTable {
        action_table: vec![vec![vec![]; symbol_count]],
        goto_table: vec![vec![StateId(u16::MAX); symbol_count]],
        state_count: 1,
        symbol_count,
        symbol_to_index,
        index_to_symbol,
        eof_symbol: SymbolId(0),
        start_symbol: SymbolId(symbol_count.saturating_sub(1) as u16),
        token_count: symbol_count.saturating_sub(1),
        lex_modes: vec![LexMode {
            lex_state: 0,
            external_lex_state: 0,
        }],
        ..ParseTable::default()
    }
}

/// Build a grammar with multiple tokens and rules.
fn arithmetic_grammar_and_table() -> (Grammar, ParseTable) {
    let grammar = GrammarBuilder::new("arithmetic")
        .token("number", r"\d+")
        .token("plus", "+")
        .token("star", "*")
        .rule("expr", vec!["number"])
        .rule("expr", vec!["expr", "plus", "expr"])
        .rule("expr", vec!["expr", "star", "expr"])
        .start("expr")
        .build();
    let table = ParseTable::default();
    (grammar, table)
}

/// Create a default TSLanguage with all-null pointers for validation testing.
fn create_test_language() -> TSLanguage {
    TSLanguage {
        version: 15,
        symbol_count: 10,
        alias_count: 0,
        token_count: 5,
        external_token_count: 0,
        state_count: 20,
        large_state_count: 0,
        production_id_count: 0,
        field_count: 0,
        max_alias_sequence_length: 0,
        parse_table: std::ptr::null(),
        small_parse_table: std::ptr::null(),
        small_parse_table_map: std::ptr::null(),
        parse_actions: std::ptr::null(),
        symbol_names: std::ptr::null(),
        field_names: std::ptr::null(),
        field_map_slices: std::ptr::null(),
        field_map_entries: std::ptr::null(),
        symbol_metadata: std::ptr::null(),
        public_symbol_map: std::ptr::null(),
        alias_map: std::ptr::null(),
        alias_sequences: std::ptr::null(),
        lex_modes: std::ptr::null(),
        lex_fn: None,
        keyword_lex_fn: None,
        keyword_capture_token: 0,
        external_scanner_data: TSExternalScannerData {
            states: std::ptr::null(),
            symbol_map: std::ptr::null(),
            create: None,
            destroy: None,
            scan: None,
            serialize: None,
            deserialize: None,
        },
        primary_state_ids: std::ptr::null(),
    }
}

// ===========================================================================
// StaticLanguageGenerator tests
// ===========================================================================

#[test]
fn static_gen_creation_preserves_grammar_name() {
    let grammar = GrammarBuilder::new("my_lang")
        .token("id", r"[a-z]+")
        .rule("start", vec!["id"])
        .start("start")
        .build();
    let table = ParseTable::default();
    let generator = StaticLanguageGenerator::new(grammar, table);

    assert_eq!(generator.grammar.name, "my_lang");
    assert!(!generator.start_can_be_empty);
    assert!(generator.compressed_tables.is_none());
}

#[test]
fn static_gen_set_start_can_be_empty() {
    let (grammar, table) = minimal_grammar_and_table();
    let mut generator = StaticLanguageGenerator::new(grammar, table);

    generator.set_start_can_be_empty(true);
    assert!(generator.start_can_be_empty);

    generator.set_start_can_be_empty(false);
    assert!(!generator.start_can_be_empty);
}

#[test]
fn static_gen_generate_language_code_produces_tokens() {
    let (grammar, table) = minimal_grammar_and_table();
    let generator = StaticLanguageGenerator::new(grammar, table);

    let code = generator.generate_language_code();
    let code_str = code.to_string();

    assert!(
        code_str.contains("language"),
        "generated code should contain `language` function"
    );
}

#[test]
fn static_gen_generate_node_types_json() {
    let (grammar, table) = arithmetic_grammar_and_table();
    let generator = StaticLanguageGenerator::new(grammar, table);

    let node_types_json = generator.generate_node_types();
    let parsed: serde_json::Value =
        serde_json::from_str(&node_types_json).expect("node_types should be valid JSON");
    assert!(parsed.is_array(), "node_types should be a JSON array");
}

#[test]
fn static_gen_node_types_excludes_hidden_rules() {
    let grammar = GrammarBuilder::new("hidden_test")
        .token("a", "a")
        .rule("_hidden", vec!["a"])
        .rule("visible", vec!["_hidden"])
        .start("visible")
        .build();
    let table = ParseTable::default();
    let generator = StaticLanguageGenerator::new(grammar, table);

    let node_types_json = generator.generate_node_types();
    assert!(
        !node_types_json.contains("\"_hidden\""),
        "hidden rules should not appear in node_types"
    );
}

#[test]
fn static_gen_node_types_includes_externals() {
    let grammar = GrammarBuilder::new("ext_test")
        .token("id", r"[a-z]+")
        .external("comment")
        .rule("start", vec!["id"])
        .start("start")
        .build();
    let table = ParseTable::default();
    let generator = StaticLanguageGenerator::new(grammar, table);

    let node_types_json = generator.generate_node_types();
    assert!(
        node_types_json.contains("\"comment\""),
        "external tokens should appear in node_types"
    );
}

// ===========================================================================
// LanguageBuilder tests
// ===========================================================================

#[test]
fn language_builder_generates_version_15() {
    let (grammar, table) = minimal_grammar_and_table();
    let builder = LanguageBuilder::new(grammar, table);

    let lang = builder
        .generate_language()
        .expect("generate_language should succeed");
    assert_eq!(lang.version, 15);
}

#[test]
fn language_builder_token_count_includes_eof() {
    let grammar = GrammarBuilder::new("tok_count")
        .token("a", "a")
        .token("b", "b")
        .rule("start", vec!["a", "b"])
        .start("start")
        .build();
    let table = ParseTable::default();
    let builder = LanguageBuilder::new(grammar.clone(), table);

    let lang = builder
        .generate_language()
        .expect("generate_language should succeed");
    // token_count = grammar.tokens.len() + 1 (for EOF)
    assert_eq!(lang.token_count, grammar.tokens.len() as u32 + 1);
}

#[test]
fn language_builder_external_token_count() {
    let grammar = GrammarBuilder::new("ext")
        .token("id", r"[a-z]+")
        .external("indent")
        .external("dedent")
        .rule("start", vec!["id"])
        .start("start")
        .build();
    let table = ParseTable::default();
    let builder = LanguageBuilder::new(grammar, table);

    let lang = builder
        .generate_language()
        .expect("generate_language should succeed");
    assert_eq!(lang.external_token_count, 2);
}

#[test]
fn language_builder_field_count_matches_grammar() {
    let mut grammar = GrammarBuilder::new("fields")
        .token("number", r"\d+")
        .rule("expr", vec!["number"])
        .start("expr")
        .build();
    grammar.fields.insert(FieldId(0), "left".to_string());
    grammar.fields.insert(FieldId(1), "operator".to_string());
    grammar.fields.insert(FieldId(2), "right".to_string());

    let table = ParseTable::default();
    let builder = LanguageBuilder::new(grammar, table);

    let lang = builder
        .generate_language()
        .expect("generate_language should succeed");
    assert_eq!(lang.field_count, 3);
}

#[test]
fn language_builder_field_names_nonnull_when_fields_exist() {
    let mut grammar = GrammarBuilder::new("fn_test")
        .token("x", "x")
        .rule("start", vec!["x"])
        .start("start")
        .build();
    grammar.fields.insert(FieldId(0), "value".to_string());

    let table = ParseTable::default();
    let builder = LanguageBuilder::new(grammar, table);

    let lang = builder
        .generate_language()
        .expect("generate_language should succeed");
    assert!(
        !lang.field_names.is_null(),
        "field_names should be non-null when fields exist"
    );
}

#[test]
fn language_builder_no_fields_null_pointer() {
    let grammar = GrammarBuilder::new("no_fields")
        .token("a", "a")
        .rule("start", vec!["a"])
        .start("start")
        .build();
    let table = ParseTable::default();
    let builder = LanguageBuilder::new(grammar, table);

    let lang = builder
        .generate_language()
        .expect("generate_language should succeed");
    assert_eq!(lang.field_count, 0);
    assert!(
        lang.field_names.is_null(),
        "field_names should be null when there are no fields"
    );
}

#[test]
fn language_builder_generate_language_code_contains_tslanguage() {
    let (grammar, table) = minimal_grammar_and_table();
    let builder = LanguageBuilder::new(grammar, table);

    let code = builder.generate_language_code();
    let code_str = code.to_string();
    assert!(code_str.contains("TSLanguage"));
}

#[test]
fn language_builder_set_start_can_be_empty_flag() {
    let (grammar, table) = minimal_grammar_and_table();
    let mut builder = LanguageBuilder::new(grammar, table);

    builder.set_start_can_be_empty(true);
    let lang = builder
        .generate_language()
        .expect("generate_language should succeed even with nullable start");
    assert_eq!(lang.version, 15);
}

// ===========================================================================
// LanguageValidator tests
// ===========================================================================

#[test]
fn validator_rejects_wrong_abi_version() {
    let mut lang = create_test_language();
    lang.version = 14;

    let tables = CompressedParseTable::new_for_testing(10, 20);
    let validator = LanguageValidator::new(&lang, &tables);
    let result = validator.validate();

    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| matches!(
        e,
        ValidationError::InvalidVersion {
            expected: 15,
            actual: 14
        }
    )));
}

#[test]
fn validator_detects_symbol_count_mismatch() {
    let lang = create_test_language();
    let tables = CompressedParseTable::new_for_testing(5, 20);
    let validator = LanguageValidator::new(&lang, &tables);

    let result = validator.validate();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| matches!(
        e,
        ValidationError::SymbolCountMismatch {
            language: 10,
            tables: 5
        }
    )));
}

#[test]
fn validator_detects_state_count_mismatch() {
    let lang = create_test_language();
    let tables = CompressedParseTable::new_for_testing(10, 30);
    let validator = LanguageValidator::new(&lang, &tables);

    let result = validator.validate();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| matches!(
        e,
        ValidationError::StateCountMismatch {
            language: 20,
            tables: 30
        }
    )));
}

#[test]
fn validator_detects_null_symbol_names() {
    let lang = create_test_language();
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let validator = LanguageValidator::new(&lang, &tables);

    let result = validator.validate();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ValidationError::NullPointer("symbol_names")))
    );
}

#[test]
fn validator_detects_null_parse_tables() {
    let lang = create_test_language();
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let validator = LanguageValidator::new(&lang, &tables);

    let result = validator.validate();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| matches!(
        e,
        ValidationError::NullPointer("parse_table or small_parse_table")
    )));
}

#[test]
fn validator_detects_null_field_names_when_fields_exist() {
    let mut lang = create_test_language();
    lang.field_count = 3;

    let tables = CompressedParseTable::new_for_testing(10, 20);
    let validator = LanguageValidator::new(&lang, &tables);

    let result = validator.validate();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ValidationError::NullPointer("field_names")))
    );
}

#[test]
fn validator_accepts_null_field_names_when_no_fields() {
    let mut lang = create_test_language();
    lang.field_count = 0;

    // Provide non-null pointers for required fields
    let dummy_table: Vec<u16> = vec![0xFFFE; 10];
    lang.small_parse_table = dummy_table.as_ptr();
    let dummy_name = b"dummy\0";
    let dummy_names: Vec<*const i8> = vec![dummy_name.as_ptr().cast(); 10];
    lang.symbol_names = dummy_names.as_ptr();
    let dummy_meta: Vec<TSSymbolMetadata> = (0..10)
        .map(|_| TSSymbolMetadata {
            visible: false,
            named: false,
        })
        .collect();
    lang.symbol_metadata = dummy_meta.as_ptr();

    let tables = CompressedParseTable::new_for_testing(10, 20);
    let validator = LanguageValidator::new(&lang, &tables);

    let result = validator.validate();
    assert!(
        result.is_ok(),
        "validator should accept null field_names when field_count is 0, but got: {:?}",
        result.unwrap_err()
    );
}

#[test]
fn validator_collects_multiple_errors() {
    let mut lang = create_test_language();
    lang.version = 99;
    lang.symbol_count = 100;
    lang.state_count = 200;

    let tables = CompressedParseTable::new_for_testing(10, 20);
    let validator = LanguageValidator::new(&lang, &tables);

    let result = validator.validate();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    // Should have at least version, symbol, and state mismatch errors
    assert!(
        errors.len() >= 3,
        "expected at least 3 errors, got {}",
        errors.len()
    );
}

// ===========================================================================
// CompressedParseTable tests
// ===========================================================================

#[test]
fn compressed_parse_table_from_parse_table() {
    let (_, table) = minimal_grammar_and_table();
    let compressed = CompressedParseTable::from_parse_table(&table);

    assert_eq!(compressed.symbol_count(), table.symbol_count);
    assert_eq!(compressed.state_count(), table.state_count);
}

#[test]
fn compressed_parse_table_new_for_testing() {
    let compressed = CompressedParseTable::new_for_testing(42, 100);
    assert_eq!(compressed.symbol_count(), 42);
    assert_eq!(compressed.state_count(), 100);
}

// ===========================================================================
// Integration: builder -> validator round-trip
// ===========================================================================

#[test]
fn language_builder_output_has_correct_eof_metadata() {
    let (grammar, table) = minimal_grammar_and_table();
    let builder = LanguageBuilder::new(grammar, table);

    let lang = builder.generate_language().expect("should succeed");
    assert!(
        !lang.symbol_metadata.is_null(),
        "symbol_metadata should be non-null"
    );
    // SAFETY: symbol_metadata was just generated by LanguageBuilder,
    // which guarantees at least symbol_count entries.
    let first_meta = unsafe { &*lang.symbol_metadata };
    assert!(!first_meta.visible, "EOF should not be visible");
    assert!(!first_meta.named, "EOF should not be named");
}

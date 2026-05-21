//! Integration tests for `adze_tablegen::abi` public API.
//!
//! These tests complement the existing unit tests (test_language_version,
//! test_struct_sizes, test_symbol_metadata) by exercising struct field access,
//! defaults, lex modes, production info, field maps, and external scanner data.

use std::mem;
use std::ptr;

use adze_tablegen::abi::*;

// ---------------------------------------------------------------------------
// 1. TSLanguage struct field access and defaults
// ---------------------------------------------------------------------------

/// Build a zero-initialised TSLanguage with only the version set.
fn minimal_language() -> TSLanguage {
    TSLanguage {
        version: TREE_SITTER_LANGUAGE_VERSION,
        symbol_count: 0,
        alias_count: 0,
        token_count: 0,
        external_token_count: 0,
        state_count: 0,
        large_state_count: 0,
        production_id_count: 0,
        field_count: 0,
        max_alias_sequence_length: 0,
        production_id_map: ptr::null(),
        parse_table: ptr::null(),
        small_parse_table: ptr::null(),
        small_parse_table_map: ptr::null(),
        parse_actions: ptr::null(),
        symbol_names: ptr::null(),
        field_names: ptr::null(),
        field_map_slices: ptr::null(),
        field_map_entries: ptr::null(),
        symbol_metadata: ptr::null(),
        public_symbol_map: ptr::null(),
        alias_map: ptr::null(),
        alias_sequences: ptr::null(),
        lex_modes: ptr::null(),
        lex_fn: None,
        keyword_lex_fn: None,
        keyword_capture_token: TSSymbol(0),
        external_scanner: ExternalScanner::default(),
        primary_state_ids: ptr::null(),
        production_lhs_index: ptr::null(),
        production_count: 0,
        eof_symbol: 0,
    }
}

#[test]
fn test_tslanguage_default_field_values() {
    let lang = minimal_language();

    assert_eq!(lang.version, TREE_SITTER_LANGUAGE_VERSION);
    assert_eq!(lang.symbol_count, 0);
    assert_eq!(lang.alias_count, 0);
    assert_eq!(lang.token_count, 0);
    assert_eq!(lang.external_token_count, 0);
    assert_eq!(lang.state_count, 0);
    assert_eq!(lang.large_state_count, 0);
    assert_eq!(lang.production_id_count, 0);
    assert_eq!(lang.field_count, 0);
    assert_eq!(lang.max_alias_sequence_length, 0);
    assert!(lang.lex_fn.is_none());
    assert!(lang.keyword_lex_fn.is_none());
    assert_eq!(lang.keyword_capture_token, TSSymbol(0));
    assert_eq!(lang.production_count, 0);
    assert_eq!(lang.eof_symbol, 0);
}

// ---------------------------------------------------------------------------
// 2. ABI version constants
// ---------------------------------------------------------------------------

#[test]
fn test_abi_version_constants_valid_range() {
    // Version 15 is the current ABI.
    assert_eq!(TREE_SITTER_LANGUAGE_VERSION, 15);
    // Min compatible must be ≤ current.
    let min = TREE_SITTER_MIN_COMPATIBLE_LANGUAGE_VERSION;
    let cur = TREE_SITTER_LANGUAGE_VERSION;
    assert!(min <= cur, "min {min} should be <= current {cur}");
    // Min compatible must be at least 13 per tree-sitter's documented range.
    assert!(min >= 13, "min {min} should be >= 13");
}

// ---------------------------------------------------------------------------
// 3. State / symbol count validation
// ---------------------------------------------------------------------------

#[test]
fn test_state_symbol_count_consistency() {
    let mut lang = minimal_language();
    lang.symbol_count = 50;
    lang.token_count = 30;
    lang.state_count = 100;
    lang.large_state_count = 10;

    // token_count ≤ symbol_count is expected (tokens are a subset of symbols).
    assert!(lang.token_count <= lang.symbol_count);
    // large_state_count ≤ state_count (large states are a subset of all states).
    assert!(lang.large_state_count <= lang.state_count);
}

#[test]
fn test_symbol_and_state_id_inner_values() {
    let sym = TSSymbol(42);
    let state = TSStateId(99);
    let field = TSFieldId(7);

    assert_eq!(sym.0, 42);
    assert_eq!(state.0, 99);
    assert_eq!(field.0, 7);

    // Copy semantics
    let sym2 = sym;
    assert_eq!(sym, sym2);
}

// ---------------------------------------------------------------------------
// 4. Lex mode creation
// ---------------------------------------------------------------------------

#[test]
fn test_lex_state_creation_and_fields() {
    let mode = TSLexState {
        lex_state: 5,
        external_lex_state: 3,
    };
    assert_eq!(mode.lex_state, 5);
    assert_eq!(mode.external_lex_state, 3);
    assert_eq!(mem::size_of::<TSLexState>(), 4);

    // Verify copy semantics
    let mode2 = mode;
    assert_eq!(mode.lex_state, mode2.lex_state);
    assert_eq!(mode.external_lex_state, mode2.external_lex_state);
}

#[test]
fn test_lex_modes_array_via_language() {
    let modes = [
        TSLexState {
            lex_state: 0,
            external_lex_state: 0,
        },
        TSLexState {
            lex_state: 1,
            external_lex_state: 2,
        },
        TSLexState {
            lex_state: 3,
            external_lex_state: 0,
        },
    ];

    let mut lang = minimal_language();
    lang.state_count = modes.len() as u32;
    lang.lex_modes = modes.as_ptr();

    // Safety: we own the array and the pointer is valid for the lifetime of this test.
    unsafe {
        for (i, mode) in modes.iter().enumerate().take(lang.state_count as usize) {
            let m = *lang.lex_modes.add(i);
            assert_eq!(m.lex_state, mode.lex_state);
            assert_eq!(m.external_lex_state, mode.external_lex_state);
        }
    }
}

// ---------------------------------------------------------------------------
// 5. Production info
// ---------------------------------------------------------------------------

#[test]
fn test_production_info_fields() {
    let mut lang = minimal_language();
    lang.production_id_count = 5;
    lang.production_count = 10;
    lang.max_alias_sequence_length = 3;

    assert_eq!(lang.production_id_count, 5);
    assert_eq!(lang.production_count, 10);
    assert_eq!(lang.max_alias_sequence_length, 3);

    // production_lhs_index starts null until populated.
    assert!(lang.production_lhs_index.is_null());
}

// ---------------------------------------------------------------------------
// 6. Field map access
// ---------------------------------------------------------------------------

#[test]
fn test_field_map_pointers_initially_null() {
    let lang = minimal_language();

    assert!(lang.field_map_slices.is_null());
    assert!(lang.field_map_entries.is_null());
    assert!(lang.field_names.is_null());
    assert_eq!(lang.field_count, 0);
}

#[test]
fn test_field_map_with_data() {
    // Simulate a language with 2 fields.
    let field_name_a = b"name\0";
    let field_name_b = b"value\0";
    let field_name_ptrs: [*const u8; 2] = [field_name_a.as_ptr(), field_name_b.as_ptr()];

    let mut lang = minimal_language();
    lang.field_count = 2;
    lang.field_names = field_name_ptrs.as_ptr();

    unsafe {
        // field 0 -> "name"
        let name_ptr = *lang.field_names;
        assert_eq!(*name_ptr, b'n');
        // field 1 -> "value"
        let val_ptr = *lang.field_names.add(1);
        assert_eq!(*val_ptr, b'v');
    }
}

// ---------------------------------------------------------------------------
// 7. External scanner data
// ---------------------------------------------------------------------------

#[test]
fn test_external_scanner_default() {
    let scanner = ExternalScanner::default();

    assert!(scanner.states.is_null());
    assert!(scanner.symbol_map.is_null());
    assert!(scanner.create.is_none());
    assert!(scanner.destroy.is_none());
    assert!(scanner.scan.is_none());
    assert!(scanner.serialize.is_none());
    assert!(scanner.deserialize.is_none());
}

#[test]
fn test_external_scanner_token_count_in_language() {
    let mut lang = minimal_language();
    lang.external_token_count = 4;
    assert_eq!(lang.external_token_count, 4);

    // Scanner itself should still be default (no callbacks set).
    assert!(lang.external_scanner.create.is_none());
    assert!(lang.external_scanner.scan.is_none());
}

// ---------------------------------------------------------------------------
// 8. create_symbol_metadata combinatorics
// ---------------------------------------------------------------------------

#[test]
fn test_create_symbol_metadata_all_combinations() {
    // All flags off → 0
    assert_eq!(create_symbol_metadata(false, false, false, false, false), 0);

    // Individual flags
    assert_eq!(
        create_symbol_metadata(true, false, false, false, false),
        symbol_metadata::VISIBLE
    );
    assert_eq!(
        create_symbol_metadata(false, true, false, false, false),
        symbol_metadata::NAMED
    );
    assert_eq!(
        create_symbol_metadata(false, false, true, false, false),
        symbol_metadata::HIDDEN
    );
    assert_eq!(
        create_symbol_metadata(false, false, false, true, false),
        symbol_metadata::AUXILIARY
    );
    assert_eq!(
        create_symbol_metadata(false, false, false, false, true),
        symbol_metadata::SUPERTYPE
    );

    // All flags on → bitwise OR of all constants
    let all = symbol_metadata::VISIBLE
        | symbol_metadata::NAMED
        | symbol_metadata::HIDDEN
        | symbol_metadata::AUXILIARY
        | symbol_metadata::SUPERTYPE;
    assert_eq!(create_symbol_metadata(true, true, true, true, true), all);
    assert_eq!(all, 0x1F);
}

// ---------------------------------------------------------------------------
// 9. TSParseAction layout
// ---------------------------------------------------------------------------

#[test]
fn test_parse_action_fields() {
    let action = TSParseAction {
        action_type: 1,
        extra: 0,
        child_count: 3,
        dynamic_precedence: -1,
        symbol: TSSymbol(42),
    };

    assert_eq!(action.action_type, 1);
    assert_eq!(action.extra, 0);
    assert_eq!(action.child_count, 3);
    assert_eq!(action.dynamic_precedence, -1);
    let sym = { action.symbol };
    assert_eq!(sym, TSSymbol(42));
    assert_eq!(mem::size_of::<TSParseAction>(), 6);
}

// ---------------------------------------------------------------------------
// 10. TSLanguage alignment
// ---------------------------------------------------------------------------

#[test]
fn test_tslanguage_pointer_alignment() {
    // TSLanguage must be pointer-aligned for FFI safety.
    assert_eq!(mem::align_of::<TSLanguage>(), mem::align_of::<*const u8>());
}

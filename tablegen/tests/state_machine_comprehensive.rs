#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

//! Comprehensive tests for state machine / automaton-related functionality
//! in the tablegen crate: parse table construction, action/goto compression,
//! schema encoding/decoding, validation, and state-transition invariants.

use adze_glr_core::Action;
use adze_ir::{RuleId, StateId};
use adze_tablegen::compress::{
    CompressedActionEntry, CompressedGotoEntry, CompressedParseTable, TableCompressor,
};
use adze_tablegen::compression::{
    compress_action_table, compress_goto_table, decompress_action, decompress_goto,
};
use adze_tablegen::schema::{SchemaError, validate_action_decoding, validate_action_encoding};
use adze_tablegen::validation::{LanguageValidator, ValidationError};

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

/// Wrap single actions into GLR cells (empty vec for Error).
fn glr_cell(a: Action) -> Vec<Action> {
    if matches!(a, Action::Error) {
        vec![]
    } else {
        vec![a]
    }
}

fn glr_row(actions: Vec<Action>) -> Vec<Vec<Action>> {
    actions.into_iter().map(glr_cell).collect()
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. CompressedParseTable creation and accessors
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn compressed_parse_table_new_for_testing() {
    let table = CompressedParseTable::new_for_testing(42, 17);
    assert_eq!(table.symbol_count(), 42);
    assert_eq!(table.state_count(), 17);
}

#[test]
fn compressed_parse_table_zero_dimensions() {
    let table = CompressedParseTable::new_for_testing(0, 0);
    assert_eq!(table.symbol_count(), 0);
    assert_eq!(table.state_count(), 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. CompressedActionEntry construction
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn compressed_action_entry_shift() {
    let entry = CompressedActionEntry::new(7, Action::Shift(StateId(99)));
    assert_eq!(entry.symbol, 7);
    assert!(matches!(entry.action, Action::Shift(StateId(99))));
}

#[test]
fn compressed_action_entry_reduce() {
    let entry = CompressedActionEntry::new(0, Action::Reduce(RuleId(5)));
    assert_eq!(entry.symbol, 0);
    assert!(matches!(entry.action, Action::Reduce(RuleId(5))));
}

#[test]
fn compressed_action_entry_accept() {
    let entry = CompressedActionEntry::new(3, Action::Accept);
    assert!(matches!(entry.action, Action::Accept));
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. TableCompressor::encode_action_small
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn encode_shift_small_valid_range() {
    let tc = TableCompressor::new();
    assert_eq!(
        tc.encode_action_small(&Action::Shift(StateId(0))).unwrap(),
        0
    );
    assert_eq!(
        tc.encode_action_small(&Action::Shift(StateId(1))).unwrap(),
        1
    );
    assert_eq!(
        tc.encode_action_small(&Action::Shift(StateId(0x7FFF)))
            .unwrap(),
        0x7FFF
    );
}

#[test]
fn encode_shift_overflow_rejected() {
    let tc = TableCompressor::new();
    assert!(
        tc.encode_action_small(&Action::Shift(StateId(0x8000)))
            .is_err()
    );
    assert!(
        tc.encode_action_small(&Action::Shift(StateId(0xFFFF)))
            .is_err()
    );
}

#[test]
fn encode_reduce_small_valid() {
    let tc = TableCompressor::new();
    // Reduce rule 0 → 0x8000 | (0+1) = 0x8001
    assert_eq!(
        tc.encode_action_small(&Action::Reduce(RuleId(0))).unwrap(),
        0x8001
    );
    // Reduce rule 1 → 0x8000 | (1+1) = 0x8002
    assert_eq!(
        tc.encode_action_small(&Action::Reduce(RuleId(1))).unwrap(),
        0x8002
    );
}

#[test]
fn encode_reduce_overflow_rejected() {
    let tc = TableCompressor::new();
    assert!(
        tc.encode_action_small(&Action::Reduce(RuleId(0x4000)))
            .is_err()
    );
}

#[test]
fn encode_accept_and_error_and_recover() {
    let tc = TableCompressor::new();
    assert_eq!(tc.encode_action_small(&Action::Accept).unwrap(), 0xFFFF);
    assert_eq!(tc.encode_action_small(&Action::Error).unwrap(), 0xFFFE);
    assert_eq!(tc.encode_action_small(&Action::Recover).unwrap(), 0xFFFD);
}

#[test]
fn encode_fork_requires_flattening() {
    let tc = TableCompressor::new();
    let fork = Action::Fork(vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(0))]);
    assert!(tc.encode_action_small(&fork).is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Schema encoding/decoding roundtrip
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn schema_encode_error() {
    assert_eq!(validate_action_encoding(&Action::Error).unwrap(), 0x0000);
}

#[test]
fn schema_encode_accept() {
    assert_eq!(validate_action_encoding(&Action::Accept).unwrap(), 0xFFFF);
}

#[test]
fn schema_encode_shift_boundary() {
    assert_eq!(
        validate_action_encoding(&Action::Shift(StateId(1))).unwrap(),
        1
    );
    assert_eq!(
        validate_action_encoding(&Action::Shift(StateId(0x7FFF))).unwrap(),
        0x7FFF
    );
}

#[test]
fn schema_encode_shift_zero_invalid() {
    let result = validate_action_encoding(&Action::Shift(StateId(0)));
    assert!(result.is_err());
    match result.unwrap_err() {
        SchemaError::InvalidActionEncoding { action, .. } => {
            assert_eq!(action, Action::Shift(StateId(0)));
        }
        other => panic!("Expected InvalidActionEncoding, got {:?}", other),
    }
}

#[test]
fn schema_encode_shift_high_bit_invalid() {
    let result = validate_action_encoding(&Action::Shift(StateId(0x8000)));
    assert!(result.is_err());
}

#[test]
fn schema_encode_reduce_boundary() {
    assert_eq!(
        validate_action_encoding(&Action::Reduce(RuleId(0))).unwrap(),
        0x8000
    );
    assert_eq!(
        validate_action_encoding(&Action::Reduce(RuleId(0x7FFE))).unwrap(),
        0xFFFE
    );
}

#[test]
fn schema_encode_reduce_overflow() {
    // RuleId(0x7FFF) would encode to 0xFFFF which collides with Accept
    assert!(validate_action_encoding(&Action::Reduce(RuleId(0x7FFF))).is_err());
}

#[test]
fn schema_decode_roundtrip_exhaustive() {
    // Test all key boundary values decode correctly
    let cases: Vec<(u16, Action)> = vec![
        (0x0000, Action::Error),
        (0x0001, Action::Shift(StateId(1))),
        (0x0100, Action::Shift(StateId(0x0100))),
        (0x7FFF, Action::Shift(StateId(0x7FFF))),
        (0x8000, Action::Reduce(RuleId(0))),
        (0x8001, Action::Reduce(RuleId(1))),
        (0xFFFE, Action::Reduce(RuleId(0x7FFE))),
        (0xFFFF, Action::Accept),
    ];
    for (encoded, expected) in &cases {
        assert!(
            validate_action_decoding(*encoded, expected).is_ok(),
            "Failed roundtrip for 0x{:04X} → {:?}",
            encoded,
            expected
        );
    }
}

#[test]
fn schema_recover_not_encodable() {
    assert!(validate_action_encoding(&Action::Recover).is_err());
}

#[test]
fn schema_fork_not_encodable() {
    let fork = Action::Fork(vec![Action::Shift(StateId(1))]);
    assert!(validate_action_encoding(&fork).is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Action table row deduplication (compression module)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn action_dedup_all_identical_rows() {
    let row = vec![glr_cell(Action::Shift(StateId(1))), glr_cell(Action::Error)];
    let table: Vec<Vec<Vec<Action>>> = vec![row; 8];
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 1);
    assert_eq!(compressed.state_to_row.len(), 8);
    // All states map to row 0
    for &r in &compressed.state_to_row {
        assert_eq!(r, 0);
    }
}

#[test]
fn action_dedup_all_distinct_rows() {
    let table: Vec<Vec<Vec<Action>>> = (0..4)
        .map(|i| vec![glr_cell(Action::Shift(StateId(i as u16)))])
        .collect();
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 4);
}

#[test]
fn action_decompress_retrieves_correct_cell() {
    let table = vec![
        glr_row(vec![Action::Shift(StateId(10)), Action::Reduce(RuleId(5))]),
        glr_row(vec![Action::Accept, Action::Error]),
    ];
    let compressed = compress_action_table(&table);
    assert_eq!(
        decompress_action(&compressed, 0, 0),
        Action::Shift(StateId(10))
    );
    assert_eq!(
        decompress_action(&compressed, 0, 1),
        Action::Reduce(RuleId(5))
    );
    assert_eq!(decompress_action(&compressed, 1, 0), Action::Accept);
    assert_eq!(decompress_action(&compressed, 1, 1), Action::Error);
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Goto table sparse compression
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn goto_sparse_roundtrip() {
    let table = vec![
        vec![Some(StateId(1)), None, Some(StateId(3))],
        vec![None, Some(StateId(2)), None],
    ];
    let compressed = compress_goto_table(&table);
    assert_eq!(decompress_goto(&compressed, 0, 0), Some(StateId(1)));
    assert_eq!(decompress_goto(&compressed, 0, 1), None);
    assert_eq!(decompress_goto(&compressed, 0, 2), Some(StateId(3)));
    assert_eq!(decompress_goto(&compressed, 1, 0), None);
    assert_eq!(decompress_goto(&compressed, 1, 1), Some(StateId(2)));
    assert_eq!(decompress_goto(&compressed, 1, 2), None);
}

#[test]
fn goto_all_none() {
    let table = vec![vec![None; 4]; 3];
    let compressed = compress_goto_table(&table);
    assert_eq!(compressed.entries.len(), 0);
    for s in 0..3 {
        for sym in 0..4 {
            assert_eq!(decompress_goto(&compressed, s, sym), None);
        }
    }
}

#[test]
fn goto_all_populated() {
    let table = vec![vec![Some(StateId(7)); 3]; 2];
    let compressed = compress_goto_table(&table);
    assert_eq!(compressed.entries.len(), 6);
    for s in 0..2 {
        for sym in 0..3 {
            assert_eq!(decompress_goto(&compressed, s, sym), Some(StateId(7)));
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. CompressedGotoEntry run-length encoding (compress.rs)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn goto_rle_short_run_uses_singles() {
    let tc = TableCompressor::new();
    // A run of 2 identical values should use Single entries (threshold is >2)
    let goto_table = vec![vec![StateId(5), StateId(5)]];
    let compressed = tc.compress_goto_table_small(&goto_table).unwrap();
    assert!(
        compressed
            .data
            .iter()
            .all(|e| matches!(e, CompressedGotoEntry::Single(5))),
        "Short runs should use Single entries"
    );
}

#[test]
fn goto_rle_long_run_uses_run_length() {
    let tc = TableCompressor::new();
    // A run of 4 identical values should use RunLength
    let goto_table = vec![vec![StateId(3), StateId(3), StateId(3), StateId(3)]];
    let compressed = tc.compress_goto_table_small(&goto_table).unwrap();
    let has_rle = compressed
        .data
        .iter()
        .any(|e| matches!(e, CompressedGotoEntry::RunLength { state: 3, count: 4 }));
    assert!(has_rle, "Long runs should use RunLength entries");
}

#[test]
fn goto_rle_mixed_runs() {
    let tc = TableCompressor::new();
    // [1,1,1,2,2,2,2,3]
    let goto_table = vec![vec![
        StateId(1),
        StateId(1),
        StateId(1),
        StateId(2),
        StateId(2),
        StateId(2),
        StateId(2),
        StateId(3),
    ]];
    let compressed = tc.compress_goto_table_small(&goto_table).unwrap();
    // Run of 3 for state=1, run of 4 for state=2, single for state=3
    let has_rle_1 = compressed
        .data
        .iter()
        .any(|e| matches!(e, CompressedGotoEntry::RunLength { state: 1, count: 3 }));
    let has_rle_2 = compressed
        .data
        .iter()
        .any(|e| matches!(e, CompressedGotoEntry::RunLength { state: 2, count: 4 }));
    let has_single_3 = compressed
        .data
        .iter()
        .any(|e| matches!(e, CompressedGotoEntry::Single(3)));
    assert!(has_rle_1);
    assert!(has_rle_2);
    assert!(has_single_3);
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. Action table compression (compress.rs) row offsets
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn action_compress_row_offsets_monotonic() {
    let tc = TableCompressor::new();
    let action_table = vec![
        vec![
            vec![Action::Shift(StateId(1))],
            vec![],
            vec![Action::Reduce(RuleId(0))],
        ],
        vec![vec![], vec![Action::Accept], vec![]],
        vec![
            vec![Action::Shift(StateId(2))],
            vec![Action::Shift(StateId(3))],
            vec![],
        ],
    ];
    let compressed = tc
        .compress_action_table_small(&action_table, &std::collections::BTreeMap::new())
        .unwrap();
    // Row offsets must be non-decreasing and length == state_count + 1
    assert_eq!(compressed.row_offsets.len(), 4); // 3 states + 1 sentinel
    for i in 1..compressed.row_offsets.len() {
        assert!(
            compressed.row_offsets[i] >= compressed.row_offsets[i - 1],
            "Row offsets must be non-decreasing"
        );
    }
}

#[test]
fn action_compress_empty_rows_produce_zero_entries() {
    let tc = TableCompressor::new();
    let action_table = vec![vec![vec![]; 5]; 3]; // all empty cells
    let compressed = tc
        .compress_action_table_small(&action_table, &std::collections::BTreeMap::new())
        .unwrap();
    assert!(compressed.data.is_empty());
    assert_eq!(compressed.default_actions.len(), 3);
    // All defaults should be Error (default action optimization is disabled)
    for da in &compressed.default_actions {
        assert_eq!(*da, Action::Error);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. Validation: LanguageValidator and ValidationError variants
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn validation_error_variants_constructible() {
    // Ensure all ValidationError variants can be constructed and compared
    let errs = vec![
        ValidationError::InvalidVersion {
            expected: 15,
            actual: 14,
        },
        ValidationError::SymbolCountMismatch {
            language: 10,
            tables: 12,
        },
        ValidationError::StateCountMismatch {
            language: 5,
            tables: 6,
        },
        ValidationError::NullPointer("parse_table"),
        ValidationError::FieldNamesNotSorted,
        ValidationError::InvalidSymbolMetadata {
            symbol: 0,
            reason: "test".to_string(),
        },
        ValidationError::TableDimensionMismatch {
            expected: 10,
            actual: 8,
        },
        ValidationError::InvalidProductionId { id: 99, max: 50 },
        ValidationError::InvalidFieldMapping {
            field_id: 5,
            max: 3,
        },
    ];
    // Verify PartialEq
    for e in &errs {
        assert_eq!(e, e);
    }
    // Different variants should not be equal
    assert_ne!(errs[0], errs[1]);
}

#[test]
fn validator_rejects_wrong_version() {
    let tables = CompressedParseTable::new_for_testing(1, 1);
    let lang = unsafe { make_dummy_language(14, 1, 1) };
    let validator = LanguageValidator::new(&lang, &tables);
    let result = validator.validate();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ValidationError::InvalidVersion { .. }))
    );
}

#[test]
fn validator_rejects_symbol_count_mismatch() {
    let tables = CompressedParseTable::new_for_testing(10, 5);
    let lang = unsafe { make_dummy_language(15, 99, 5) }; // symbol_count wrong
    let validator = LanguageValidator::new(&lang, &tables);
    let result = validator.validate();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ValidationError::SymbolCountMismatch { .. }))
    );
}

#[test]
fn validator_rejects_state_count_mismatch() {
    let tables = CompressedParseTable::new_for_testing(5, 10);
    let lang = unsafe { make_dummy_language(15, 5, 99) }; // state_count wrong
    let validator = LanguageValidator::new(&lang, &tables);
    let result = validator.validate();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ValidationError::StateCountMismatch { .. }))
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 10. SchemaError display and variants
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn schema_error_display() {
    let err = SchemaError::MissingAcceptState;
    let msg = format!("{}", err);
    assert!(
        msg.contains("Accept"),
        "Display should mention Accept: {}",
        msg
    );

    let err2 = SchemaError::InvalidStateId {
        state_id: 42,
        max_states: 10,
    };
    let msg2 = format!("{}", err2);
    assert!(
        msg2.contains("42"),
        "Display should mention state id: {}",
        msg2
    );
}

#[test]
fn schema_error_is_std_error() {
    let err: Box<dyn std::error::Error> = Box::new(SchemaError::MissingAcceptState);
    // Just verify it can be used as a trait object
    assert!(!err.to_string().is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// Unsafe helper to build a minimal TSLanguage for validation tests
// ═══════════════════════════════════════════════════════════════════════════

/// Build a dummy TSLanguage with specified version, symbol_count, and state_count.
/// All pointers are null (validation will check for that).
unsafe fn make_dummy_language(
    version: u32,
    symbol_count: u32,
    state_count: u32,
) -> adze_tablegen::validation::TSLanguage {
    use std::ptr;
    // SAFETY: We are constructing a repr(C) struct with null pointers.
    // This is only used for validation tests that check metadata fields
    // and detect null pointers.
    adze_tablegen::validation::TSLanguage {
        version,
        symbol_count,
        alias_count: 0,
        token_count: 0,
        external_token_count: 0,
        state_count,
        large_state_count: 0,
        production_id_count: 0,
        field_count: 0,
        max_alias_sequence_length: 0,
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
        keyword_capture_token: 0,
        external_scanner_data: adze_tablegen::validation::TSExternalScannerData {
            states: ptr::null(),
            symbol_map: ptr::null(),
            create: None,
            destroy: None,
            scan: None,
            serialize: None,
            deserialize: None,
        },
        primary_state_ids: ptr::null(),
    }
}

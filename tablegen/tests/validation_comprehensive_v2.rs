//! Comprehensive v2 tests for `adze_tablegen::validation`.
//!
//! Covers parse table validation rules, grammar consistency checks,
//! symbol count validation, state transition validation, and error
//! reporting quality.

use adze_tablegen::LanguageValidator;
use adze_tablegen::compress::CompressedParseTable;
use adze_tablegen::validation::{
    TSExternalScannerData, TSLanguage, TSSymbolMetadata, ValidationError,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a `TSLanguage` with all-null pointers and sensible defaults.
fn bare_language(symbol_count: u32, state_count: u32) -> TSLanguage {
    TSLanguage {
        version: 15,
        symbol_count,
        alias_count: 0,
        token_count: 5,
        external_token_count: 0,
        state_count,
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

/// Wire the minimum non-null pointers so that a language passes pointer checks.
/// Returns owned buffers that must be kept alive.
struct WiredLanguage {
    lang: TSLanguage,
    _spt: Vec<u16>,
    _sym_name_data: Vec<u8>,
    _sym_name_ptrs: Vec<*const i8>,
    _metadata: Vec<TSSymbolMetadata>,
}

fn wired_language(symbol_count: u32, state_count: u32) -> WiredLanguage {
    let spt: Vec<u16> = vec![0];
    let mut sym_name_data = Vec::new();
    let mut sym_name_ptrs = Vec::new();
    for i in 0..symbol_count {
        let start = sym_name_data.len();
        sym_name_data.extend_from_slice(format!("s{i}\0").as_bytes());
        sym_name_ptrs.push(unsafe { sym_name_data.as_ptr().add(start) as *const i8 });
    }

    let mut metadata = Vec::new();
    for i in 0..symbol_count {
        metadata.push(TSSymbolMetadata {
            visible: i != 0,
            named: i != 0,
        });
    }

    let mut lang = bare_language(symbol_count, state_count);
    lang.small_parse_table = spt.as_ptr();
    lang.symbol_names = sym_name_ptrs.as_ptr();
    lang.symbol_metadata = metadata.as_ptr();

    WiredLanguage {
        lang,
        _spt: spt,
        _sym_name_data: sym_name_data,
        _sym_name_ptrs: sym_name_ptrs,
        _metadata: metadata,
    }
}

fn has_error<F: Fn(&ValidationError) -> bool>(
    result: &std::result::Result<(), Vec<ValidationError>>,
    pred: F,
) -> bool {
    match result {
        Ok(()) => false,
        Err(errors) => errors.iter().any(pred),
    }
}

fn error_count(result: &std::result::Result<(), Vec<ValidationError>>) -> usize {
    match result {
        Ok(()) => 0,
        Err(errors) => errors.len(),
    }
}

// ===========================================================================
// 1. Parse table validation rules – ABI version
// ===========================================================================

#[test]
fn v2_version_15_accepted() {
    let w = wired_language(1, 1);
    let tables = CompressedParseTable::new_for_testing(1, 1);
    let res = LanguageValidator::new(&w.lang, &tables).validate();
    assert!(!has_error(&res, |e| matches!(
        e,
        ValidationError::InvalidVersion { .. }
    )));
}

#[test]
fn v2_version_13_rejected() {
    let mut lang = bare_language(10, 20);
    lang.version = 13;
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(has_error(&res, |e| matches!(
        e,
        ValidationError::InvalidVersion {
            expected: 15,
            actual: 13
        }
    )));
}

#[test]
fn v2_version_1_rejected() {
    let mut lang = bare_language(10, 20);
    lang.version = 1;
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(has_error(&res, |e| matches!(
        e,
        ValidationError::InvalidVersion {
            expected: 15,
            actual: 1
        }
    )));
}

#[test]
fn v2_version_u32_max_reports_correct_actual() {
    let mut lang = bare_language(10, 20);
    lang.version = u32::MAX;
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(has_error(&res, |e| matches!(
        e,
        ValidationError::InvalidVersion {
            expected: 15,
            actual
        } if *actual == u32::MAX
    )));
}

#[test]
fn v2_version_0_rejected() {
    let mut lang = bare_language(10, 20);
    lang.version = 0;
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(has_error(&res, |e| matches!(
        e,
        ValidationError::InvalidVersion {
            expected: 15,
            actual: 0
        }
    )));
}

// ===========================================================================
// 2. Grammar consistency – symbol count validation
// ===========================================================================

#[test]
fn v2_symbol_count_exact_match_no_error() {
    let lang = bare_language(42, 20);
    let tables = CompressedParseTable::new_for_testing(42, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(!has_error(&res, |e| matches!(
        e,
        ValidationError::SymbolCountMismatch { .. }
    )));
}

#[test]
fn v2_symbol_count_language_higher() {
    let lang = bare_language(100, 20);
    let tables = CompressedParseTable::new_for_testing(50, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(has_error(&res, |e| matches!(
        e,
        ValidationError::SymbolCountMismatch {
            language: 100,
            tables: 50
        }
    )));
}

#[test]
fn v2_symbol_count_language_lower() {
    let lang = bare_language(3, 20);
    let tables = CompressedParseTable::new_for_testing(30, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(has_error(&res, |e| matches!(
        e,
        ValidationError::SymbolCountMismatch {
            language: 3,
            tables: 30
        }
    )));
}

#[test]
fn v2_symbol_count_zero_both_ok() {
    let lang = bare_language(0, 0);
    let tables = CompressedParseTable::new_for_testing(0, 0);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(!has_error(&res, |e| matches!(
        e,
        ValidationError::SymbolCountMismatch { .. }
    )));
}

#[test]
fn v2_symbol_count_one_vs_zero_mismatch() {
    let lang = bare_language(1, 5);
    let tables = CompressedParseTable::new_for_testing(0, 5);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(has_error(&res, |e| matches!(
        e,
        ValidationError::SymbolCountMismatch {
            language: 1,
            tables: 0
        }
    )));
}

#[test]
fn v2_symbol_count_large_matching() {
    let lang = bare_language(65535, 20);
    let tables = CompressedParseTable::new_for_testing(65535, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(!has_error(&res, |e| matches!(
        e,
        ValidationError::SymbolCountMismatch { .. }
    )));
}

#[test]
fn v2_symbol_count_off_by_one_high() {
    let lang = bare_language(11, 20);
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(has_error(&res, |e| matches!(
        e,
        ValidationError::SymbolCountMismatch {
            language: 11,
            tables: 10
        }
    )));
}

#[test]
fn v2_symbol_count_off_by_one_low() {
    let lang = bare_language(9, 20);
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(has_error(&res, |e| matches!(
        e,
        ValidationError::SymbolCountMismatch {
            language: 9,
            tables: 10
        }
    )));
}

// ===========================================================================
// 3. State transition validation – state count
// ===========================================================================

#[test]
fn v2_state_count_exact_match_no_error() {
    let lang = bare_language(10, 100);
    let tables = CompressedParseTable::new_for_testing(10, 100);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(!has_error(&res, |e| matches!(
        e,
        ValidationError::StateCountMismatch { .. }
    )));
}

#[test]
fn v2_state_count_language_higher() {
    let lang = bare_language(10, 200);
    let tables = CompressedParseTable::new_for_testing(10, 100);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(has_error(&res, |e| matches!(
        e,
        ValidationError::StateCountMismatch {
            language: 200,
            tables: 100
        }
    )));
}

#[test]
fn v2_state_count_language_lower() {
    let lang = bare_language(10, 10);
    let tables = CompressedParseTable::new_for_testing(10, 100);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(has_error(&res, |e| matches!(
        e,
        ValidationError::StateCountMismatch {
            language: 10,
            tables: 100
        }
    )));
}

#[test]
fn v2_state_count_zero_both_ok() {
    let lang = bare_language(0, 0);
    let tables = CompressedParseTable::new_for_testing(0, 0);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(!has_error(&res, |e| matches!(
        e,
        ValidationError::StateCountMismatch { .. }
    )));
}

#[test]
fn v2_state_count_off_by_one() {
    let lang = bare_language(10, 21);
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(has_error(&res, |e| matches!(
        e,
        ValidationError::StateCountMismatch {
            language: 21,
            tables: 20
        }
    )));
}

#[test]
fn v2_state_count_large_matching() {
    let lang = bare_language(10, 50_000);
    let tables = CompressedParseTable::new_for_testing(10, 50_000);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(!has_error(&res, |e| matches!(
        e,
        ValidationError::StateCountMismatch { .. }
    )));
}

// ===========================================================================
// 4. Null pointer validation
// ===========================================================================

#[test]
fn v2_both_parse_tables_null_detected() {
    let lang = bare_language(10, 20);
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(has_error(&res, |e| matches!(
        e,
        ValidationError::NullPointer("parse_table or small_parse_table")
    )));
}

#[test]
fn v2_parse_table_nonnull_passes() {
    let dummy: u16 = 0;
    let mut lang = bare_language(10, 20);
    lang.parse_table = &dummy;
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(!has_error(&res, |e| matches!(
        e,
        ValidationError::NullPointer("parse_table or small_parse_table")
    )));
}

#[test]
fn v2_small_parse_table_nonnull_passes() {
    let dummy: u16 = 0;
    let mut lang = bare_language(10, 20);
    lang.small_parse_table = &dummy;
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(!has_error(&res, |e| matches!(
        e,
        ValidationError::NullPointer("parse_table or small_parse_table")
    )));
}

#[test]
fn v2_both_parse_tables_nonnull_passes() {
    let d1: u16 = 0;
    let d2: u16 = 0;
    let mut lang = bare_language(10, 20);
    lang.parse_table = &d1;
    lang.small_parse_table = &d2;
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(!has_error(&res, |e| matches!(
        e,
        ValidationError::NullPointer("parse_table or small_parse_table")
    )));
}

#[test]
fn v2_null_symbol_names_detected() {
    let lang = bare_language(10, 20);
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(has_error(&res, |e| matches!(
        e,
        ValidationError::NullPointer("symbol_names")
    )));
}

#[test]
fn v2_nonnull_symbol_names_passes() {
    let name_data = b"x\0";
    let name_ptr: *const i8 = name_data.as_ptr().cast();
    let names = [name_ptr];
    let mut lang = bare_language(10, 20);
    lang.symbol_names = names.as_ptr();
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(!has_error(&res, |e| matches!(
        e,
        ValidationError::NullPointer("symbol_names")
    )));
}

#[test]
fn v2_null_symbol_metadata_detected() {
    let lang = bare_language(10, 20);
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(has_error(&res, |e| matches!(
        e,
        ValidationError::NullPointer("symbol_metadata")
    )));
}

#[test]
fn v2_nonnull_symbol_metadata_passes() {
    let md = [TSSymbolMetadata {
        visible: false,
        named: false,
    }];
    let mut lang = bare_language(10, 20);
    lang.symbol_metadata = md.as_ptr();
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(!has_error(&res, |e| matches!(
        e,
        ValidationError::NullPointer("symbol_metadata")
    )));
}

#[test]
fn v2_field_names_null_with_fields_detected() {
    let mut lang = bare_language(10, 20);
    lang.field_count = 5;
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(has_error(&res, |e| matches!(
        e,
        ValidationError::NullPointer("field_names")
    )));
}

#[test]
fn v2_field_names_null_with_zero_fields_ok() {
    let lang = bare_language(10, 20);
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(!has_error(&res, |e| matches!(
        e,
        ValidationError::NullPointer("field_names")
    )));
}

#[test]
fn v2_field_names_nonnull_with_fields_passes() {
    let alpha = b"alpha\0";
    let ptrs: Vec<*const i8> = vec![alpha.as_ptr().cast()];
    let mut lang = bare_language(10, 20);
    lang.field_count = 1;
    lang.field_names = ptrs.as_ptr();
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(!has_error(&res, |e| matches!(
        e,
        ValidationError::NullPointer("field_names")
    )));
}

// ===========================================================================
// 5. Symbol metadata – EOF symbol validation
// ===========================================================================

#[test]
fn v2_eof_visible_true_named_false_is_invalid() {
    let metadata = vec![
        TSSymbolMetadata {
            visible: true,
            named: false,
        },
        TSSymbolMetadata {
            visible: true,
            named: true,
        },
    ];
    let mut lang = bare_language(2, 1);
    lang.symbol_metadata = metadata.as_ptr();
    let tables = CompressedParseTable::new_for_testing(2, 1);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(has_error(&res, |e| matches!(
        e,
        ValidationError::InvalidSymbolMetadata { symbol: 0, .. }
    )));
    drop(metadata);
}

#[test]
fn v2_eof_visible_false_named_true_is_invalid() {
    let metadata = vec![
        TSSymbolMetadata {
            visible: false,
            named: true,
        },
        TSSymbolMetadata {
            visible: true,
            named: true,
        },
    ];
    let mut lang = bare_language(2, 1);
    lang.symbol_metadata = metadata.as_ptr();
    let tables = CompressedParseTable::new_for_testing(2, 1);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(has_error(&res, |e| matches!(
        e,
        ValidationError::InvalidSymbolMetadata { symbol: 0, .. }
    )));
    drop(metadata);
}

#[test]
fn v2_eof_visible_true_named_true_is_invalid() {
    let metadata = vec![
        TSSymbolMetadata {
            visible: true,
            named: true,
        },
        TSSymbolMetadata {
            visible: true,
            named: true,
        },
    ];
    let mut lang = bare_language(2, 1);
    lang.symbol_metadata = metadata.as_ptr();
    let tables = CompressedParseTable::new_for_testing(2, 1);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(has_error(&res, |e| matches!(
        e,
        ValidationError::InvalidSymbolMetadata { symbol: 0, .. }
    )));
    drop(metadata);
}

#[test]
fn v2_eof_invisible_unnamed_is_valid() {
    let metadata = vec![
        TSSymbolMetadata {
            visible: false,
            named: false,
        },
        TSSymbolMetadata {
            visible: true,
            named: true,
        },
    ];
    let mut lang = bare_language(2, 1);
    lang.symbol_metadata = metadata.as_ptr();
    let tables = CompressedParseTable::new_for_testing(2, 1);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(!has_error(&res, |e| matches!(
        e,
        ValidationError::InvalidSymbolMetadata { .. }
    )));
    drop(metadata);
}

#[test]
fn v2_eof_check_skipped_when_metadata_null() {
    let lang = bare_language(10, 20);
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    // Should have NullPointer error, but NOT InvalidSymbolMetadata
    assert!(!has_error(&res, |e| matches!(
        e,
        ValidationError::InvalidSymbolMetadata { .. }
    )));
}

#[test]
fn v2_metadata_with_many_symbols_only_checks_eof() {
    let mut metadata = vec![TSSymbolMetadata {
        visible: false,
        named: false,
    }]; // valid EOF
    for _ in 1..50 {
        metadata.push(TSSymbolMetadata {
            visible: true,
            named: true,
        });
    }
    let mut lang = bare_language(50, 10);
    lang.symbol_metadata = metadata.as_ptr();
    let tables = CompressedParseTable::new_for_testing(50, 10);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(!has_error(&res, |e| matches!(
        e,
        ValidationError::InvalidSymbolMetadata { .. }
    )));
    drop(metadata);
}

// ===========================================================================
// 6. Field name ordering validation
// ===========================================================================

#[test]
fn v2_sorted_field_names_pass() {
    let alpha = b"alpha\0";
    let beta = b"beta\0";
    let gamma = b"gamma\0";
    let ptrs: Vec<*const i8> = vec![
        alpha.as_ptr().cast(),
        beta.as_ptr().cast(),
        gamma.as_ptr().cast(),
    ];
    let mut lang = bare_language(10, 20);
    lang.field_count = 3;
    lang.field_names = ptrs.as_ptr();
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(!has_error(&res, |e| matches!(
        e,
        ValidationError::FieldNamesNotSorted
    )));
}

#[test]
fn v2_unsorted_field_names_fail() {
    let beta = b"beta\0";
    let alpha = b"alpha\0";
    let ptrs: Vec<*const i8> = vec![beta.as_ptr().cast(), alpha.as_ptr().cast()];
    let mut lang = bare_language(10, 20);
    lang.field_count = 2;
    lang.field_names = ptrs.as_ptr();
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(has_error(&res, |e| matches!(
        e,
        ValidationError::FieldNamesNotSorted
    )));
}

#[test]
fn v2_duplicate_field_names_fail() {
    let alpha1 = b"alpha\0";
    let alpha2 = b"alpha\0";
    let ptrs: Vec<*const i8> = vec![alpha1.as_ptr().cast(), alpha2.as_ptr().cast()];
    let mut lang = bare_language(10, 20);
    lang.field_count = 2;
    lang.field_names = ptrs.as_ptr();
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(has_error(&res, |e| matches!(
        e,
        ValidationError::FieldNamesNotSorted
    )));
}

#[test]
fn v2_single_field_name_always_sorted() {
    let only = b"only\0";
    let ptrs: Vec<*const i8> = vec![only.as_ptr().cast()];
    let mut lang = bare_language(10, 20);
    lang.field_count = 1;
    lang.field_names = ptrs.as_ptr();
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(!has_error(&res, |e| matches!(
        e,
        ValidationError::FieldNamesNotSorted
    )));
}

#[test]
fn v2_field_names_skipped_when_count_zero() {
    let lang = bare_language(10, 20);
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(!has_error(&res, |e| matches!(
        e,
        ValidationError::FieldNamesNotSorted
    )));
}

// ===========================================================================
// 7. Multiple simultaneous errors
// ===========================================================================

#[test]
fn v2_multiple_errors_collected_at_once() {
    let mut lang = bare_language(99, 99);
    lang.version = 0;
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    let errors = res.unwrap_err();
    // version + symbol mismatch + state mismatch + null ptrs (parse_table, symbol_names, metadata)
    assert!(
        errors.len() >= 4,
        "expected >=4 errors, got {}",
        errors.len()
    );
}

#[test]
fn v2_all_error_categories_can_coexist() {
    let mut lang = bare_language(99, 99);
    lang.version = 42;
    lang.field_count = 1; // field_names is null → NullPointer
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    let errors = res.unwrap_err();

    let has_version = errors
        .iter()
        .any(|e| matches!(e, ValidationError::InvalidVersion { .. }));
    let has_symbol = errors
        .iter()
        .any(|e| matches!(e, ValidationError::SymbolCountMismatch { .. }));
    let has_state = errors
        .iter()
        .any(|e| matches!(e, ValidationError::StateCountMismatch { .. }));
    let has_null = errors
        .iter()
        .any(|e| matches!(e, ValidationError::NullPointer(_)));

    assert!(has_version, "missing version error");
    assert!(has_symbol, "missing symbol count error");
    assert!(has_state, "missing state count error");
    assert!(has_null, "missing null pointer error");
}

#[test]
fn v2_error_count_matches_expected_for_bare_language() {
    // bare_language with matching counts: only null pointer errors expected
    let lang = bare_language(10, 20);
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    let count = error_count(&res);
    // parse_table or small_parse_table, symbol_names, symbol_metadata
    assert_eq!(count, 3, "expected 3 null pointer errors, got {count}");
}

// ===========================================================================
// 8. Fully-wired language passes validation
// ===========================================================================

#[test]
fn v2_fully_wired_single_symbol_passes() {
    let w = wired_language(1, 1);
    let tables = CompressedParseTable::new_for_testing(1, 1);
    let res = LanguageValidator::new(&w.lang, &tables).validate();
    assert!(
        res.is_ok(),
        "expected Ok, got errors: {:?}",
        res.unwrap_err()
    );
}

#[test]
fn v2_fully_wired_many_symbols_passes() {
    let w = wired_language(20, 50);
    let tables = CompressedParseTable::new_for_testing(20, 50);
    let res = LanguageValidator::new(&w.lang, &tables).validate();
    assert!(
        res.is_ok(),
        "expected Ok, got errors: {:?}",
        res.unwrap_err()
    );
}

// ===========================================================================
// 9. ValidationError Debug / PartialEq / Clone
// ===========================================================================

#[test]
fn v2_validation_error_debug_contains_values() {
    let err = ValidationError::SymbolCountMismatch {
        language: 42,
        tables: 10,
    };
    let dbg = format!("{err:?}");
    assert!(dbg.contains("42"), "debug should contain '42': {dbg}");
    assert!(dbg.contains("10"), "debug should contain '10': {dbg}");
}

#[test]
fn v2_validation_error_debug_null_pointer() {
    let err = ValidationError::NullPointer("test_field");
    let dbg = format!("{err:?}");
    assert!(
        dbg.contains("test_field"),
        "debug should contain 'test_field': {dbg}"
    );
}

#[test]
fn v2_validation_error_debug_invalid_symbol_metadata() {
    let err = ValidationError::InvalidSymbolMetadata {
        symbol: 5,
        reason: "bad symbol".to_string(),
    };
    let dbg = format!("{err:?}");
    assert!(
        dbg.contains("bad symbol"),
        "debug should contain reason: {dbg}"
    );
    assert!(dbg.contains("5"), "debug should contain symbol id: {dbg}");
}

#[test]
fn v2_validation_error_eq_same_variant() {
    let a = ValidationError::InvalidVersion {
        expected: 15,
        actual: 14,
    };
    let b = ValidationError::InvalidVersion {
        expected: 15,
        actual: 14,
    };
    assert_eq!(a, b);
}

#[test]
fn v2_validation_error_ne_different_variant() {
    let a = ValidationError::InvalidVersion {
        expected: 15,
        actual: 14,
    };
    let b = ValidationError::FieldNamesNotSorted;
    assert_ne!(a, b);
}

#[test]
fn v2_validation_error_ne_same_variant_different_values() {
    let a = ValidationError::SymbolCountMismatch {
        language: 10,
        tables: 20,
    };
    let b = ValidationError::SymbolCountMismatch {
        language: 20,
        tables: 10,
    };
    assert_ne!(a, b);
}

#[test]
fn v2_validation_error_clone() {
    let err = ValidationError::InvalidFieldMapping {
        field_id: 5,
        max: 3,
    };
    let cloned = err.clone();
    assert_eq!(err, cloned);
}

#[test]
fn v2_validation_error_debug_table_dimension_mismatch() {
    let err = ValidationError::TableDimensionMismatch {
        expected: 100,
        actual: 50,
    };
    let dbg = format!("{err:?}");
    assert!(dbg.contains("100"));
    assert!(dbg.contains("50"));
}

#[test]
fn v2_validation_error_debug_invalid_production_id() {
    let err = ValidationError::InvalidProductionId { id: 999, max: 100 };
    let dbg = format!("{err:?}");
    assert!(dbg.contains("999"));
    assert!(dbg.contains("100"));
}

#[test]
fn v2_validation_error_debug_invalid_field_mapping() {
    let err = ValidationError::InvalidFieldMapping {
        field_id: 7,
        max: 3,
    };
    let dbg = format!("{err:?}");
    assert!(dbg.contains("7"));
    assert!(dbg.contains("3"));
}

// ===========================================================================
// 10. Error reporting quality – all variants constructible and distinguishable
// ===========================================================================

#[test]
fn v2_all_error_variants_are_distinct() {
    let errors: Vec<ValidationError> = vec![
        ValidationError::InvalidVersion {
            expected: 15,
            actual: 14,
        },
        ValidationError::SymbolCountMismatch {
            language: 10,
            tables: 5,
        },
        ValidationError::StateCountMismatch {
            language: 20,
            tables: 10,
        },
        ValidationError::NullPointer("test"),
        ValidationError::FieldNamesNotSorted,
        ValidationError::InvalidSymbolMetadata {
            symbol: 0,
            reason: "test".into(),
        },
        ValidationError::TableDimensionMismatch {
            expected: 10,
            actual: 5,
        },
        ValidationError::InvalidProductionId { id: 99, max: 50 },
        ValidationError::InvalidFieldMapping {
            field_id: 3,
            max: 2,
        },
    ];
    // Check all pairs are distinct
    for i in 0..errors.len() {
        for j in (i + 1)..errors.len() {
            assert_ne!(errors[i], errors[j], "errors[{i}] == errors[{j}]");
        }
    }
}

#[test]
fn v2_null_pointer_variants_distinguishable() {
    let a = ValidationError::NullPointer("symbol_names");
    let b = ValidationError::NullPointer("symbol_metadata");
    let c = ValidationError::NullPointer("parse_table or small_parse_table");
    assert_ne!(a, b);
    assert_ne!(b, c);
    assert_ne!(a, c);
}

#[test]
fn v2_error_reason_preserved_in_metadata() {
    let reason = "EOF symbol must be invisible and unnamed".to_string();
    let err = ValidationError::InvalidSymbolMetadata {
        symbol: 0,
        reason: reason.clone(),
    };
    if let ValidationError::InvalidSymbolMetadata { reason: r, .. } = &err {
        assert_eq!(r, &reason);
    } else {
        panic!("wrong variant");
    }
}

// ===========================================================================
// 11. Edge cases – boundary values
// ===========================================================================

#[test]
fn v2_max_u32_symbol_count_mismatch() {
    let lang = bare_language(u32::MAX, 20);
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(has_error(&res, |e| matches!(
        e,
        ValidationError::SymbolCountMismatch { .. }
    )));
}

#[test]
fn v2_max_u32_state_count_mismatch() {
    let lang = bare_language(10, u32::MAX);
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(has_error(&res, |e| matches!(
        e,
        ValidationError::StateCountMismatch { .. }
    )));
}

#[test]
fn v2_one_symbol_one_state_wired_passes() {
    let w = wired_language(1, 1);
    let tables = CompressedParseTable::new_for_testing(1, 1);
    let res = LanguageValidator::new(&w.lang, &tables).validate();
    assert!(
        res.is_ok(),
        "minimal wired language should pass: {:?}",
        res.unwrap_err()
    );
}

// ===========================================================================
// 12. CompressedParseTable factory method tests
// ===========================================================================

#[test]
fn v2_compressed_parse_table_new_for_testing_accessors() {
    let cpt = CompressedParseTable::new_for_testing(7, 13);
    assert_eq!(cpt.symbol_count(), 7);
    assert_eq!(cpt.state_count(), 13);
}

#[test]
fn v2_compressed_parse_table_zero_counts() {
    let cpt = CompressedParseTable::new_for_testing(0, 0);
    assert_eq!(cpt.symbol_count(), 0);
    assert_eq!(cpt.state_count(), 0);
}

#[test]
fn v2_compressed_parse_table_large_counts() {
    let cpt = CompressedParseTable::new_for_testing(100_000, 200_000);
    assert_eq!(cpt.symbol_count(), 100_000);
    assert_eq!(cpt.state_count(), 200_000);
}

// ===========================================================================
// 13. Validator construction
// ===========================================================================

#[test]
fn v2_validator_new_does_not_validate_eagerly() {
    // Constructing a validator with invalid data should not panic
    let mut lang = bare_language(99, 99);
    lang.version = 0;
    let tables = CompressedParseTable::new_for_testing(1, 1);
    let _validator = LanguageValidator::new(&lang, &tables);
    // If we get here, construction didn't panic
}

#[test]
fn v2_validator_validate_returns_ok_for_valid_input() {
    let w = wired_language(5, 10);
    let tables = CompressedParseTable::new_for_testing(5, 10);
    let res = LanguageValidator::new(&w.lang, &tables).validate();
    assert!(res.is_ok());
}

#[test]
fn v2_validator_validate_returns_err_for_invalid_input() {
    let lang = bare_language(10, 20);
    let tables = CompressedParseTable::new_for_testing(10, 20);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(res.is_err());
}

// ===========================================================================
// 14. Interaction between metadata and field checks
// ===========================================================================

#[test]
fn v2_bad_eof_metadata_combined_with_unsorted_fields() {
    let metadata = vec![
        TSSymbolMetadata {
            visible: true,
            named: true,
        }, // invalid EOF
        TSSymbolMetadata {
            visible: true,
            named: true,
        },
    ];
    let beta = b"beta\0";
    let alpha = b"alpha\0";
    let ptrs: Vec<*const i8> = vec![beta.as_ptr().cast(), alpha.as_ptr().cast()];

    let spt: Vec<u16> = vec![0];
    let name_data = b"s0\0";
    let name_ptr: *const i8 = name_data.as_ptr().cast();
    let names = [name_ptr, name_ptr];

    let mut lang = bare_language(2, 1);
    lang.small_parse_table = spt.as_ptr();
    lang.symbol_names = names.as_ptr();
    lang.symbol_metadata = metadata.as_ptr();
    lang.field_count = 2;
    lang.field_names = ptrs.as_ptr();

    let tables = CompressedParseTable::new_for_testing(2, 1);
    let res = LanguageValidator::new(&lang, &tables).validate();
    let errors = res.unwrap_err();

    let has_meta = errors
        .iter()
        .any(|e| matches!(e, ValidationError::InvalidSymbolMetadata { .. }));
    let has_sort = errors
        .iter()
        .any(|e| matches!(e, ValidationError::FieldNamesNotSorted));
    assert!(has_meta, "should report bad EOF metadata");
    assert!(has_sort, "should report unsorted field names");

    drop(metadata);
    drop(ptrs);
    drop(spt);
    let _ = names;
}

#[test]
fn v2_valid_metadata_with_valid_fields_no_errors_from_either() {
    let metadata = vec![
        TSSymbolMetadata {
            visible: false,
            named: false,
        }, // valid EOF
        TSSymbolMetadata {
            visible: true,
            named: true,
        },
    ];
    let alpha = b"alpha\0";
    let ptrs: Vec<*const i8> = vec![alpha.as_ptr().cast()];

    let spt: Vec<u16> = vec![0];
    let name_data = b"s0\0";
    let name_ptr: *const i8 = name_data.as_ptr().cast();
    let names = [name_ptr, name_ptr];

    let mut lang = bare_language(2, 1);
    lang.small_parse_table = spt.as_ptr();
    lang.symbol_names = names.as_ptr();
    lang.symbol_metadata = metadata.as_ptr();
    lang.field_count = 1;
    lang.field_names = ptrs.as_ptr();

    let tables = CompressedParseTable::new_for_testing(2, 1);
    let res = LanguageValidator::new(&lang, &tables).validate();
    assert!(res.is_ok(), "should pass: {:?}", res.unwrap_err());

    drop(metadata);
    drop(ptrs);
    drop(spt);
    let _ = names;
}

// ===========================================================================
// 15. TSLanguage struct field accessibility
// ===========================================================================

#[test]
fn v2_ts_language_all_fields_accessible() {
    let lang = bare_language(10, 20);
    assert_eq!(lang.version, 15);
    assert_eq!(lang.symbol_count, 10);
    assert_eq!(lang.alias_count, 0);
    assert_eq!(lang.token_count, 5);
    assert_eq!(lang.external_token_count, 0);
    assert_eq!(lang.state_count, 20);
    assert_eq!(lang.large_state_count, 0);
    assert_eq!(lang.production_id_count, 0);
    assert_eq!(lang.field_count, 0);
    assert_eq!(lang.max_alias_sequence_length, 0);
    assert_eq!(lang.keyword_capture_token, 0);
    assert!(lang.lex_fn.is_none());
    assert!(lang.keyword_lex_fn.is_none());
}

#[test]
fn v2_ts_language_external_scanner_data_fields() {
    let lang = bare_language(1, 1);
    assert!(lang.external_scanner_data.states.is_null());
    assert!(lang.external_scanner_data.symbol_map.is_null());
    assert!(lang.external_scanner_data.create.is_none());
    assert!(lang.external_scanner_data.destroy.is_none());
    assert!(lang.external_scanner_data.scan.is_none());
    assert!(lang.external_scanner_data.serialize.is_none());
    assert!(lang.external_scanner_data.deserialize.is_none());
}

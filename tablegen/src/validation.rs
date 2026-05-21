#![cfg_attr(feature = "strict_docs", allow(missing_docs))]

//! Tree-sitter ABI validation and FFI struct definitions.

use crate::compress::CompressedParseTable;
use std::os::raw::c_char;

/// Validates that a generated Language struct meets Tree-sitter ABI requirements
pub struct LanguageValidator<'a> {
    language: &'a TSLanguage,
    tables: &'a CompressedParseTable,
}

/// Tree-sitter Language struct for ABI v15
#[repr(C)]
pub struct TSLanguage {
    pub version: u32,
    pub symbol_count: u32,
    pub alias_count: u32,
    pub token_count: u32,
    pub external_token_count: u32,
    pub state_count: u32,
    pub large_state_count: u32,
    pub production_id_count: u32,
    pub field_count: u32,
    pub max_alias_sequence_length: u16,
    pub parse_table: *const u16,
    pub small_parse_table: *const u16,
    pub small_parse_table_map: *const u32,
    pub parse_actions: *const TSParseActionEntry,
    pub symbol_names: *const *const c_char,
    pub field_names: *const *const c_char,
    pub field_map_slices: *const u16,
    pub field_map_entries: *const u16,
    pub symbol_metadata: *const TSSymbolMetadata,
    pub public_symbol_map: *const TSSymbol,
    pub alias_map: *const u16,
    pub alias_sequences: *const TSSymbol,
    pub lex_modes: *const TSLexMode,
    pub lex_fn: Option<unsafe extern "C" fn(*mut TSLexer, TSStateId) -> bool>,
    pub keyword_lex_fn: Option<unsafe extern "C" fn(*mut TSLexer, TSStateId) -> bool>,
    pub keyword_capture_token: TSSymbol,
    pub external_scanner_data: TSExternalScannerData,
    pub primary_state_ids: *const TSStateId,
}

/// Parse table action entry matching Tree-sitter's C layout.
#[repr(C)]
pub struct TSParseActionEntry {
    /// Packed action value.
    pub action: u32,
}

/// Metadata for a grammar symbol (visibility and naming).
#[repr(C)]
pub struct TSSymbolMetadata {
    /// Whether the symbol is visible in the concrete syntax tree.
    pub visible: bool,
    /// Whether the symbol is a named node.
    pub named: bool,
}

/// Lexer mode configuration.
#[repr(C)]
pub struct TSLexMode {
    /// Lexer mode identifier.
    pub lex_mode_id: u8,
}

/// External scanner function pointers and data.
#[repr(C)]
pub struct TSExternalScannerData {
    pub states: *const bool,
    pub symbol_map: *const TSSymbol,
    pub create: Option<unsafe extern "C" fn() -> *mut std::ffi::c_void>,
    pub destroy: Option<unsafe extern "C" fn(*mut std::ffi::c_void)>,
    pub scan:
        Option<unsafe extern "C" fn(*mut std::ffi::c_void, *mut TSLexer, *const bool) -> bool>,
    pub serialize: Option<unsafe extern "C" fn(*mut std::ffi::c_void, *mut u8) -> u32>,
    pub deserialize: Option<unsafe extern "C" fn(*mut std::ffi::c_void, *const u8, u32)>,
}

/// FFI-compatible lexer interface matching Tree-sitter's C layout.
#[repr(C)]
pub struct TSLexer {
    pub lookahead: i32,
    pub result_symbol: TSSymbol,
    pub advance: Option<unsafe extern "C" fn(*mut TSLexer, bool)>,
    pub mark_end: Option<unsafe extern "C" fn(*mut TSLexer)>,
    pub get_column: Option<unsafe extern "C" fn(*mut TSLexer) -> u32>,
    pub is_at_included_range_start: Option<unsafe extern "C" fn(*mut TSLexer) -> bool>,
    pub eof: Option<unsafe extern "C" fn(*mut TSLexer) -> bool>,
}

/// Symbol identifier type (unsigned 16-bit).
pub type TSSymbol = u16;
/// State identifier type (unsigned 16-bit).
pub type TSStateId = u16;

/// Validation errors that can occur when checking Language structs
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    /// Language version doesn't match expected ABI
    InvalidVersion { expected: u32, actual: u32 },

    /// Symbol count doesn't match tables
    SymbolCountMismatch { language: u32, tables: u32 },

    /// State count doesn't match tables
    StateCountMismatch { language: u32, tables: u32 },

    /// Null pointer where data was expected
    NullPointer(&'static str),

    /// Field names not in lexicographic order
    FieldNamesNotSorted,

    /// Invalid symbol metadata
    InvalidSymbolMetadata { symbol: TSSymbol, reason: String },

    /// Table dimensions don't match metadata
    TableDimensionMismatch { expected: usize, actual: usize },

    /// Production ID out of bounds
    InvalidProductionId { id: u32, max: u32 },

    /// Invalid field mapping
    InvalidFieldMapping { field_id: u16, max: u32 },
}

impl<'a> LanguageValidator<'a> {
    /// Creates a new validator for the given Language and tables
    pub fn new(language: &'a TSLanguage, tables: &'a CompressedParseTable) -> Self {
        Self { language, tables }
    }

    /// Performs comprehensive validation of the Language struct
    #[must_use = "validation result must be checked"]
    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        // Check ABI version
        if self.language.version != 15 {
            errors.push(ValidationError::InvalidVersion {
                expected: 15,
                actual: self.language.version,
            });
        }

        // Validate counts match tables
        self.validate_counts(&mut errors);

        // Validate pointers are non-null where required
        self.validate_pointers(&mut errors);

        // Validate symbol metadata
        self.validate_symbol_metadata(&mut errors);

        // Validate field names ordering
        self.validate_field_names(&mut errors);

        // Validate field-map entries
        self.validate_field_maps(&mut errors);

        // Validate table dimensions
        self.validate_table_dimensions(&mut errors);

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn validate_counts(&self, errors: &mut Vec<ValidationError>) {
        // Check symbol count
        let table_symbol_count = self.tables.symbol_count();
        if self.language.symbol_count != table_symbol_count as u32 {
            errors.push(ValidationError::SymbolCountMismatch {
                language: self.language.symbol_count,
                tables: table_symbol_count as u32,
            });
        }

        // Check state count
        let table_state_count = self.tables.state_count();
        if self.language.state_count != table_state_count as u32 {
            errors.push(ValidationError::StateCountMismatch {
                language: self.language.state_count,
                tables: table_state_count as u32,
            });
        }
    }

    fn validate_pointers(&self, errors: &mut Vec<ValidationError>) {
        // Parse tables must be present
        if self.language.parse_table.is_null() && self.language.small_parse_table.is_null() {
            errors.push(ValidationError::NullPointer(
                "parse_table or small_parse_table",
            ));
        }

        // Symbol names must be present
        if self.language.symbol_names.is_null() {
            errors.push(ValidationError::NullPointer("symbol_names"));
        }

        // Symbol metadata must be present
        if self.language.symbol_metadata.is_null() {
            errors.push(ValidationError::NullPointer("symbol_metadata"));
        }

        // Field names must be present if field_count > 0
        if self.language.field_count > 0 && self.language.field_names.is_null() {
            errors.push(ValidationError::NullPointer("field_names"));
        }

        if self.language.field_count > 0
            && self.language.production_id_count > 0
            && self.language.field_map_slices.is_null()
        {
            errors.push(ValidationError::NullPointer("field_map_slices"));
        }

        if self.language.field_count > 0
            && self.language.production_id_count > 0
            && self.language.field_map_entries.is_null()
        {
            errors.push(ValidationError::NullPointer("field_map_entries"));
        }
    }

    fn validate_symbol_metadata(&self, errors: &mut Vec<ValidationError>) {
        if self.language.symbol_metadata.is_null() {
            return;
        }

        if self.language.symbol_count == 0 {
            errors.push(ValidationError::InvalidSymbolMetadata {
                symbol: 0,
                reason: "symbol metadata must include EOF symbol".to_string(),
            });
            return;
        }

        // SAFETY: `symbol_metadata` was verified non-null above. The ABI contract
        // guarantees it points to `symbol_count` contiguous `SymbolMetadata` entries.
        // TODO(safety): We trust that `symbol_count` matches the actual allocation
        // size; a mismatch would cause an out-of-bounds read.
        unsafe {
            let metadata_slice = std::slice::from_raw_parts(
                self.language.symbol_metadata,
                self.language.symbol_count as usize,
            );

            // First symbol should always be unnamed and invisible (EOF)
            if metadata_slice[0].visible || metadata_slice[0].named {
                errors.push(ValidationError::InvalidSymbolMetadata {
                    symbol: 0,
                    reason: "EOF symbol must be invisible and unnamed".to_string(),
                });
            }
        }
    }

    fn validate_field_names(&self, errors: &mut Vec<ValidationError>) {
        if self.language.field_count == 0 || self.language.field_names.is_null() {
            return;
        }

        // SAFETY: `field_names` was verified non-null above. The ABI builder
        // emits exactly `field_count` contiguous `*const c_char` pointers, each
        // pointing to a valid null-terminated C string.
        // TODO(safety): We trust that each pointer in the slice is non-null and
        // points to a valid C string; a corrupt entry would cause UB in CStr::from_ptr.
        unsafe {
            let field_names = std::slice::from_raw_parts(
                self.language.field_names,
                self.language.field_count as usize,
            );

            for &field_name in field_names {
                if field_name.is_null() {
                    errors.push(ValidationError::NullPointer("field_names entry"));
                    return;
                }
            }

            // Check lexicographic ordering
            for i in 1..field_names.len() {
                let prev = std::ffi::CStr::from_ptr(field_names[i - 1]);
                let curr = std::ffi::CStr::from_ptr(field_names[i]);

                if prev >= curr {
                    errors.push(ValidationError::FieldNamesNotSorted);
                    break;
                }
            }
        }
    }

    fn validate_field_maps(&self, errors: &mut Vec<ValidationError>) {
        if self.language.field_count == 0
            || self.language.production_id_count == 0
            || self.language.field_map_slices.is_null()
            || self.language.field_map_entries.is_null()
        {
            return;
        }

        let slices_len = self.language.production_id_count as usize * 2;
        // SAFETY: `field_map_slices` is non-null and should point to two packed
        // u16 words per production ID: start entry index, then entry count.
        let slices =
            unsafe { std::slice::from_raw_parts(self.language.field_map_slices, slices_len) };

        let entries_len = slices
            .chunks_exact(2)
            .map(|slice| slice[0] as usize + slice[1] as usize)
            .max()
            .unwrap_or(0);
        if entries_len == 0 {
            return;
        }

        let entry_words_len = entries_len * 2;
        // SAFETY: `field_map_entries` is non-null and should contain every
        // referenced packed field entry, two u16 words per entry.
        let entries =
            unsafe { std::slice::from_raw_parts(self.language.field_map_entries, entry_words_len) };

        for slice in slices.chunks_exact(2) {
            let start = slice[0] as usize;
            let len = slice[1] as usize;
            for idx in 0..len {
                let entry_offset = (start + idx) * 2;
                let packed_entry =
                    ((entries[entry_offset + 1] as u32) << 16) | entries[entry_offset] as u32;
                let field_id = (packed_entry & 0xFFFF) as u16;
                if u32::from(field_id) >= self.language.field_count {
                    errors.push(ValidationError::InvalidFieldMapping {
                        field_id,
                        max: self.language.field_count,
                    });
                }
            }
        }
    }

    fn validate_table_dimensions(&self, _errors: &mut Vec<ValidationError>) {
        // Validate based on whether we have small or large tables
        if !self.language.small_parse_table.is_null() {
            // Small table validation
            let _expected_entries =
                self.language.state_count as usize * self.language.symbol_count as usize;
            // Additional validation would require accessing the actual table data
        } else if !self.language.parse_table.is_null() {
            // Large table validation
            // Would need to check parse_actions array length matches compressed data
        }
    }
}

/// Creates a test Language struct for validation testing
#[cfg(test)]
pub fn create_test_language() -> TSLanguage {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_validation() {
        let mut language = create_test_language();
        language.version = 14; // Wrong version

        let tables = CompressedParseTable::new_for_testing(10, 20);
        let validator = LanguageValidator::new(&language, &tables);

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
    fn test_null_pointer_validation() {
        let language = create_test_language();
        let tables = CompressedParseTable::new_for_testing(10, 20);
        let validator = LanguageValidator::new(&language, &tables);

        let result = validator.validate();
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, ValidationError::NullPointer(_)))
        );
    }

    #[test]
    fn test_field_count_requires_field_map_pointers() {
        let mut language = create_test_language();
        language.field_count = 1;
        language.production_id_count = 1;
        let tables = CompressedParseTable::new_for_testing(10, 20);
        let validator = LanguageValidator::new(&language, &tables);

        let errors = validator.validate().unwrap_err();
        assert!(errors.contains(&ValidationError::NullPointer("field_map_slices")));
        assert!(errors.contains(&ValidationError::NullPointer("field_map_entries")));
    }

    #[test]
    fn test_field_names_without_productions_allow_null_field_maps() {
        let parse_table = [0u16];
        let field_name_value = b"value\0";
        let field_names = [field_name_value.as_ptr().cast::<std::os::raw::c_char>()];
        let symbol_names = [std::ptr::null::<std::os::raw::c_char>()];
        let symbol_metadata = [TSSymbolMetadata {
            visible: false,
            named: false,
        }];

        let mut language = create_test_language();
        language.symbol_count = 1;
        language.state_count = 1;
        language.field_count = 1;
        language.production_id_count = 0;
        language.parse_table = parse_table.as_ptr();
        language.symbol_names = symbol_names.as_ptr();
        language.symbol_metadata = symbol_metadata.as_ptr();
        language.field_names = field_names.as_ptr();

        let tables = CompressedParseTable::new_for_testing(1, 1);
        let validator = LanguageValidator::new(&language, &tables);
        let result = validator.validate();

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn test_null_field_name_entry_is_rejected_before_cstr_decode() {
        let parse_table = [0u16];
        let field_names = [std::ptr::null::<std::os::raw::c_char>()];
        let symbol_names = [std::ptr::null::<std::os::raw::c_char>()];
        let symbol_metadata = [TSSymbolMetadata {
            visible: false,
            named: false,
        }];

        let mut language = create_test_language();
        language.symbol_count = 1;
        language.state_count = 1;
        language.field_count = 1;
        language.production_id_count = 0;
        language.parse_table = parse_table.as_ptr();
        language.symbol_names = symbol_names.as_ptr();
        language.symbol_metadata = symbol_metadata.as_ptr();
        language.field_names = field_names.as_ptr();

        let tables = CompressedParseTable::new_for_testing(1, 1);
        let validator = LanguageValidator::new(&language, &tables);
        let errors = validator.validate().unwrap_err();

        assert!(errors.contains(&ValidationError::NullPointer("field_names entry")));
    }

    #[test]
    fn test_field_name_sorting_checks_first_real_pair() {
        let parse_table = [0u16];
        let field_name_zebra = b"zebra\0";
        let field_name_apple = b"apple\0";
        let field_names = [
            field_name_zebra.as_ptr().cast::<std::os::raw::c_char>(),
            field_name_apple.as_ptr().cast::<std::os::raw::c_char>(),
        ];
        let symbol_names = [std::ptr::null::<std::os::raw::c_char>()];
        let symbol_metadata = [TSSymbolMetadata {
            visible: false,
            named: false,
        }];

        let mut language = create_test_language();
        language.symbol_count = 1;
        language.state_count = 1;
        language.field_count = 2;
        language.production_id_count = 0;
        language.parse_table = parse_table.as_ptr();
        language.symbol_names = symbol_names.as_ptr();
        language.symbol_metadata = symbol_metadata.as_ptr();
        language.field_names = field_names.as_ptr();

        let tables = CompressedParseTable::new_for_testing(1, 1);
        let validator = LanguageValidator::new(&language, &tables);
        let errors = validator.validate().unwrap_err();

        assert!(errors.contains(&ValidationError::FieldNamesNotSorted));
    }

    #[test]
    fn test_zero_symbol_count_rejects_missing_eof_metadata() {
        let parse_table = [0u16];
        let symbol_names: [*const std::os::raw::c_char; 0] = [];
        let symbol_metadata: [TSSymbolMetadata; 0] = [];

        let mut language = create_test_language();
        language.symbol_count = 0;
        language.state_count = 1;
        language.parse_table = parse_table.as_ptr();
        language.symbol_names = symbol_names.as_ptr();
        language.symbol_metadata = symbol_metadata.as_ptr();

        let tables = CompressedParseTable::new_for_testing(0, 1);
        let validator = LanguageValidator::new(&language, &tables);
        let errors = validator.validate().unwrap_err();

        assert!(errors.contains(&ValidationError::InvalidSymbolMetadata {
            symbol: 0,
            reason: "symbol metadata must include EOF symbol".to_string(),
        }));
    }

    #[test]
    fn test_invalid_field_map_field_id_rejected() {
        let parse_table = [0u16];
        let symbol_names = [std::ptr::null::<std::os::raw::c_char>()];
        let symbol_metadata = [TSSymbolMetadata {
            visible: false,
            named: false,
        }];
        let field_map_slices = [0u16, 1u16];
        let packed_entry = 2u32;
        let field_map_entries = [packed_entry as u16, (packed_entry >> 16) as u16];

        let mut language = create_test_language();
        language.symbol_count = 1;
        language.state_count = 1;
        language.field_count = 1;
        language.production_id_count = 1;
        language.parse_table = parse_table.as_ptr();
        language.symbol_names = symbol_names.as_ptr();
        language.symbol_metadata = symbol_metadata.as_ptr();
        language.field_map_slices = field_map_slices.as_ptr();
        language.field_map_entries = field_map_entries.as_ptr();

        let tables = CompressedParseTable::new_for_testing(1, 1);
        let validator = LanguageValidator::new(&language, &tables);
        let errors = validator.validate().unwrap_err();

        assert!(errors.contains(&ValidationError::InvalidFieldMapping {
            field_id: 2,
            max: 1,
        }));
    }
}

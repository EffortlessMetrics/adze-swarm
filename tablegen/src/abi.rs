// Tree-sitter ABI 15 compatibility layer
//! Tree-sitter ABI v15 type definitions and constants.

// This module ensures our generated structures match Tree-sitter's ABI exactly
#![cfg_attr(feature = "strict_docs", allow(missing_docs))]

use std::ffi::c_void;

/// Tree-sitter ABI version 15
pub const TREE_SITTER_LANGUAGE_VERSION: u32 = 15;

/// Minimum compatible ABI version
pub const TREE_SITTER_MIN_COMPATIBLE_LANGUAGE_VERSION: u32 = 13;

/// Tree-sitter symbol type - must match C definition exactly
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TSSymbol(pub u16);

/// Tree-sitter state ID type
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TSStateId(pub u16);

/// Tree-sitter field ID type
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TSFieldId(pub u16);

/// Parse action type for ABI 15
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct TSParseAction {
    pub action_type: u8,
    pub extra: u8, // Use u8 instead of bool for consistent size
    pub child_count: u8,
    pub dynamic_precedence: i8,
    pub symbol: TSSymbol,
}

/// Lex state for external scanners
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TSLexState {
    pub lex_state: u16,
    pub external_lex_state: u16,
}

/// Language structure for ABI 15
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
    pub production_id_map: *const u16,
    pub parse_table: *const u16,
    pub small_parse_table: *const u16,
    pub small_parse_table_map: *const u32,
    pub parse_actions: *const TSParseAction,
    pub symbol_names: *const *const u8,
    pub field_names: *const *const u8,
    pub field_map_slices: *const u16,
    pub field_map_entries: *const u16,
    pub symbol_metadata: *const u8,
    pub public_symbol_map: *const TSSymbol,
    pub alias_map: *const u16,
    pub alias_sequences: *const TSSymbol,
    pub lex_modes: *const TSLexState,
    pub lex_fn: Option<unsafe extern "C" fn(*mut c_void, TSLexState) -> bool>,
    pub keyword_lex_fn: Option<unsafe extern "C" fn(*mut c_void, TSStateId) -> TSSymbol>,
    pub keyword_capture_token: TSSymbol,
    pub external_scanner: ExternalScanner,
    pub primary_state_ids: *const TSStateId,
    pub production_lhs_index: *const u16, // LHS symbols in table index space
    pub production_count: u16,            // Number of productions
    pub eof_symbol: u16,                  // Column index of EOF (usually 0)
    pub rules: *const TSRule,             // Syntax metadata indexed by RuleId
    pub rule_count: u16,                  // Length of rules and production_id_map
}

/// Syntax metadata, indexed by RuleId rather than shared ProductionId.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TSRule {
    pub lhs: u16,
    pub rhs_len: u8,
    pub _pad: u8,
}

/// External scanner structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ExternalScanner {
    pub states: *const bool,
    pub symbol_map: *const TSSymbol,
    pub create: Option<unsafe extern "C" fn() -> *mut c_void>,
    pub destroy: Option<unsafe extern "C" fn(*mut c_void)>,
    pub scan: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *const bool) -> bool>,
    pub serialize: Option<unsafe extern "C" fn(*mut c_void, *mut u8) -> u32>,
    pub deserialize: Option<unsafe extern "C" fn(*mut c_void, *const u8, u32)>,
}

impl Default for ExternalScanner {
    fn default() -> Self {
        ExternalScanner {
            states: std::ptr::null(),
            symbol_map: std::ptr::null(),
            create: None,
            destroy: None,
            scan: None,
            serialize: None,
            deserialize: None,
        }
    }
}

/// Ensure TSLanguage is FFI-safe and matches expected size
const _: () = {
    use std::mem;

    // These sizes must match tree-sitter's expectations
    assert!(mem::size_of::<TSSymbol>() == 2);
    assert!(mem::size_of::<TSStateId>() == 2);
    assert!(mem::size_of::<TSFieldId>() == 2);
    assert!(mem::size_of::<TSParseAction>() == 6);
    assert!(mem::size_of::<TSLexState>() == 4);

    // Language struct must be pointer-sized aligned
    assert!(mem::align_of::<TSLanguage>() == mem::align_of::<*const u8>());
};

/// Symbol metadata flags for ABI 15
pub mod symbol_metadata {
    pub const VISIBLE: u8 = 0x01;
    pub const NAMED: u8 = 0x02;
    pub const HIDDEN: u8 = 0x04;
    pub const AUXILIARY: u8 = 0x08;
    pub const SUPERTYPE: u8 = 0x10;
}

/// Create a symbol metadata byte from flags
pub fn create_symbol_metadata(
    visible: bool,
    named: bool,
    hidden: bool,
    auxiliary: bool,
    supertype: bool,
) -> u8 {
    let mut metadata = 0u8;
    if visible {
        metadata |= symbol_metadata::VISIBLE;
    }
    if named {
        metadata |= symbol_metadata::NAMED;
    }
    if hidden {
        metadata |= symbol_metadata::HIDDEN;
    }
    if auxiliary {
        metadata |= symbol_metadata::AUXILIARY;
    }
    if supertype {
        metadata |= symbol_metadata::SUPERTYPE;
    }
    metadata
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;

    #[test]
    fn test_struct_sizes() {
        // Verify sizes match C ABI
        assert_eq!(mem::size_of::<TSSymbol>(), 2);
        assert_eq!(mem::size_of::<TSStateId>(), 2);
        assert_eq!(mem::size_of::<TSFieldId>(), 2);
        assert_eq!(mem::size_of::<TSParseAction>(), 6);
        assert_eq!(mem::size_of::<TSLexState>(), 4);
    }

    #[test]
    fn test_symbol_metadata() {
        let metadata = create_symbol_metadata(true, true, false, false, false);
        assert_eq!(metadata, symbol_metadata::VISIBLE | symbol_metadata::NAMED);

        let metadata = create_symbol_metadata(false, false, true, true, false);
        assert_eq!(
            metadata,
            symbol_metadata::HIDDEN | symbol_metadata::AUXILIARY
        );
    }

    #[test]
    fn test_language_version() {
        assert_eq!(TREE_SITTER_LANGUAGE_VERSION, 15);
        // const assertion documents the ABI version contract
        const _: () =
            assert!(TREE_SITTER_LANGUAGE_VERSION >= TREE_SITTER_MIN_COMPATIBLE_LANGUAGE_VERSION);
    }
}

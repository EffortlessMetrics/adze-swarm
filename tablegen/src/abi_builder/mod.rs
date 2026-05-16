#![cfg_attr(feature = "strict_docs", allow(missing_docs))]
//! Builder for ABI-compatible Tree-sitter Language structures.

// ABI-compatible language builder for Tree-sitter
// This module generates static Language structures that match Tree-sitter's C ABI exactly

use crate::abi::*;
use crate::compress::CompressedTables;
use adze_glr_core::{Action, ParseTable};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, TokenPattern};
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashSet;

#[cfg(not(debug_assertions))]
macro_rules! debug_trace {
    ($($arg:tt)*) => {};
}

#[cfg(debug_assertions)]
macro_rules! debug_trace {
    ($($arg:tt)*) => {
        if std::env::var("RUST_LOG")
            .ok()
            .unwrap_or_default()
            .contains("debug")
        {
            eprintln!($($arg)*);
        }
    };
}

/// Builder for generating ABI-compatible language structures
pub struct AbiLanguageBuilder<'a> {
    grammar: &'a Grammar,
    parse_table: &'a ParseTable,
    compressed_tables: Option<&'a CompressedTables>,
}

impl<'a> AbiLanguageBuilder<'a> {
    pub fn new(grammar: &'a Grammar, parse_table: &'a ParseTable) -> Self {
        Self {
            grammar,
            parse_table,
            compressed_tables: None,
        }
    }

    pub fn with_compressed_tables(mut self, tables: &'a CompressedTables) -> Self {
        self.compressed_tables = Some(tables);
        self
    }

    /// Generate the complete language module
    pub fn generate(&self) -> TokenStream {
        let language_name = &self.grammar.name;
        let language_fn_ident = quote::format_ident!("tree_sitter_{}", language_name);

        debug_trace!(
            "DEBUG AbiLanguageBuilder: Generating language for '{}'",
            language_name
        );
        debug_trace!("DEBUG AbiLanguageBuilder: symbol_to_index mapping:");
        for (symbol_id, &index) in &self.parse_table.symbol_to_index {
            let symbol_name = self.get_symbol_name(*symbol_id);
            debug_trace!(
                "  SymbolId({}) -> index {} ('{}')",
                symbol_id.0,
                index,
                symbol_name
            );
        }

        // Check what the initial state expects
        if !self.parse_table.action_table.is_empty() {
            debug_trace!("DEBUG AbiLanguageBuilder: State 0 actions:");
            for (symbol_idx, action_cell) in self.parse_table.action_table[0].iter().enumerate() {
                if !action_cell.is_empty() {
                    // Find the symbol ID for this index
                    let symbol_id = self
                        .parse_table
                        .symbol_to_index
                        .iter()
                        .find(|(_, idx)| **idx == symbol_idx)
                        .map(|(id, _)| *id);
                    debug_trace!(
                        "  Index {} (SymbolId {:?}): {:?}",
                        symbol_idx,
                        symbol_id,
                        action_cell
                    );
                }
            }
        }

        // Generate all static data with deterministic ordering
        let (symbol_names, symbol_name_ptrs) = self.generate_symbol_names();
        let (field_names, field_name_ptrs) = self.generate_field_names();
        let symbol_metadata = self.generate_symbol_metadata();
        let (parse_table_data, small_parse_table_map) = self.generate_parse_tables();
        let parse_actions = self.generate_parse_actions();
        let lex_modes = self.generate_lex_modes();
        let (field_map_slices, field_map_entries) = self.generate_field_maps();
        let public_symbol_map = self.generate_public_symbol_map();
        let primary_state_ids = self.generate_primary_state_ids();
        let production_id_map = self.generate_production_id_map();
        let production_lhs_index = self.generate_production_lhs_index();
        let ts_rules = self.generate_ts_rules();
        let variant_symbol_map = self.generate_variant_symbol_map();
        let (alias_map, alias_sequences) = self.generate_alias_tables();

        // Generate external scanner data if needed
        let (external_scanner_code, external_scanner_struct) = if !self.grammar.externals.is_empty()
        {
            use crate::external_scanner_v2::ExternalScannerGenerator;

            let scanner_gen =
                ExternalScannerGenerator::new(self.grammar.clone(), self.parse_table.clone());
            let scanner_interface = scanner_gen.generate_scanner_interface();

            // Skip generating scanner FFI functions - let grammars provide their own
            // Grammars with external scanners should implement their own FFI functions
            let scanner_functions = quote! {};

            let scanner_struct = quote! {
                ExternalScanner {
                    states: EXTERNAL_SCANNER_STATES.as_ptr() as *const u8,
                    symbol_map: EXTERNAL_SCANNER_SYMBOL_MAP.as_ptr(),
                    create: None,
                    destroy: None,
                    scan: None,
                    serialize: None,
                    deserialize: None,
                }
            };

            (
                quote! {
                    #scanner_interface
                    #scanner_functions
                },
                scanner_struct,
            )
        } else {
            (
                quote! {},
                quote! {
                    ExternalScanner {
                        states: std::ptr::null(),
                        symbol_map: std::ptr::null(),
                        create: None,
                        destroy: None,
                        scan: None,
                        serialize: None,
                        deserialize: None,
                    }
                },
            )
        };

        // Count elements
        let counts = self.calculate_counts();
        let symbol_count = counts.symbol_count;
        let alias_count = counts.alias_count;
        let token_count = counts.token_count;
        let external_token_count = counts.external_token_count;
        let state_count = counts.state_count;
        let large_state_count = counts.large_state_count;
        let production_id_count = counts.production_id_count;
        let field_count = counts.field_count;
        let max_alias_sequence_length = counts.max_alias_sequence_length;
        let alias_tables = if alias_count > 0 && max_alias_sequence_length > 0 {
            quote! {
                static ALIAS_MAP: &[u16] = &[#(#alias_map),*];
                static ALIAS_SEQUENCES: &[u16] = &[#(#alias_sequences),*];
            }
        } else {
            quote! {}
        };
        let alias_map_ptr = if alias_count > 0 && max_alias_sequence_length > 0 {
            quote! { ALIAS_MAP.as_ptr() }
        } else {
            quote! { std::ptr::null() }
        };
        let alias_sequences_ptr = if alias_count > 0 && max_alias_sequence_length > 0 {
            quote! { ALIAS_SEQUENCES.as_ptr() }
        } else {
            quote! { std::ptr::null::<u16>() }
        };

        // Generate field names array
        let field_names_array = if field_count == 0 {
            quote! {
                static FIELD_NAME_PTRS: [SyncPtr; 0] = [];
            }
        } else {
            quote! {
                const FIELD_NAME_PTRS_LEN: usize = #field_count as usize;
                static FIELD_NAME_PTRS: [SyncPtr; FIELD_NAME_PTRS_LEN] = [
                    #(#field_name_ptrs),*
                ];
            }
        };

        // Debug: Print symbol_to_index mapping for tokens
        debug_trace!("DEBUG: Symbol to index mapping for lexer generation:");
        for (sym_id, idx) in &self.parse_table.symbol_to_index {
            if self.grammar.tokens.contains_key(sym_id) {
                let token = &self.grammar.tokens[sym_id];
                debug_trace!(
                    "  Token '{}' (SymbolId {:?}) -> index {}",
                    token.name,
                    sym_id,
                    idx
                );
            }
        }
        debug_trace!("DEBUG: token_count = {}", self.parse_table.token_count);

        debug_trace!("DEBUG: token_count = {}", counts.token_count);
        debug_trace!("DEBUG: symbol_count = {}", counts.symbol_count);

        // Generate lexer function with symbol mapping
        let lexer_code =
            crate::lexer_gen::generate_lexer(self.grammar, &self.parse_table.symbol_to_index);

        quote! {
            use adze::pure_parser::{TSLanguage, TSParseAction, TSRule, SyncPtr, TREE_SITTER_LANGUAGE_VERSION, ExternalScanner, TSLexState};

            // Lexer implementation
            #lexer_code

            // Symbol names (null-terminated strings)
            #(#symbol_names)*

            // Symbol name pointers array
            const SYMBOL_NAME_PTRS_LEN: usize = #symbol_count as usize;
            static SYMBOL_NAME_PTRS: [SyncPtr; SYMBOL_NAME_PTRS_LEN] = [
                #(#symbol_name_ptrs),*
            ];

            // Field names (null-terminated strings)
            #(#field_names)*

            // Field name pointers array - handle empty case specially
            #field_names_array

            // Symbol metadata (visibility, named, etc.)
            static SYMBOL_METADATA: &[u8] = &[#(#symbol_metadata),*];

            // Parse table (for large states - empty if all states are compressed)
            static PARSE_TABLE: &[u16] = &[];

            // Small parse table (compressed states data)
            pub static SMALL_PARSE_TABLE: &[u16] = &[#(#parse_table_data),*];

            // Small parse table map
            pub static SMALL_PARSE_TABLE_MAP: &[u32] = &[#(#small_parse_table_map),*];

            // Parse actions
            static PARSE_ACTIONS: &[TSParseAction] = &[#(#parse_actions),*];

            // Lex modes
            static LEX_MODES: &[TSLexState] = &[#(#lex_modes),*];

            // Field map slices
            static FIELD_MAP_SLICES: &[u16] = &[#(#field_map_slices),*];

            // Field map entries
            static FIELD_MAP_ENTRIES: &[u16] = &[#(#field_map_entries),*];

            // Public symbol map
            static PUBLIC_SYMBOL_MAP: &[u16] = &[#(#public_symbol_map),*];

            // Primary state IDs
            static PRIMARY_STATE_IDS: &[u16] = &[#(#primary_state_ids),*];

            // Production ID map (maps encoded rule IDs to production IDs)
            static PRODUCTION_ID_MAP: &[u16] = &[#(#production_id_map),*];

            // Production LHS index (maps production IDs to LHS symbols in table index space)
            static PRODUCTION_LHS_INDEX: &[u16] = &[#(#production_lhs_index),*];

            // Alias metadata
            #alias_tables

            // Rule metadata for GLR parsing
            static TS_RULES: &[TSRule] = &[#(#ts_rules),*];

            // Variant symbol map (for Extract trait to use)
            #variant_symbol_map

            // External scanner support (if needed)
            #external_scanner_code

            // The language structure
            pub static LANGUAGE: TSLanguage = TSLanguage {
                version: TREE_SITTER_LANGUAGE_VERSION,
                symbol_count: #symbol_count,
                alias_count: #alias_count,
                token_count: #token_count,
                external_token_count: #external_token_count,
                state_count: #state_count,
                large_state_count: #large_state_count,
                production_id_count: #production_id_count,
                field_count: #field_count,
                max_alias_sequence_length: #max_alias_sequence_length,
                production_id_map: PRODUCTION_ID_MAP.as_ptr(),
                parse_table: PARSE_TABLE.as_ptr(),
                small_parse_table: SMALL_PARSE_TABLE.as_ptr(),
                small_parse_table_map: SMALL_PARSE_TABLE_MAP.as_ptr(),
                parse_actions: PARSE_ACTIONS.as_ptr(),
                symbol_names: SYMBOL_NAME_PTRS.as_ptr() as *const *const u8,
                field_names: FIELD_NAME_PTRS.as_ptr() as *const *const u8,
                field_map_slices: FIELD_MAP_SLICES.as_ptr(),
                field_map_entries: FIELD_MAP_ENTRIES.as_ptr(),
                symbol_metadata: SYMBOL_METADATA.as_ptr(),
                public_symbol_map: PUBLIC_SYMBOL_MAP.as_ptr(),
                alias_map: #alias_map_ptr,
                alias_sequences: #alias_sequences_ptr,
                lex_modes: LEX_MODES.as_ptr(),
                lex_fn: Some(lexer_fn),
                keyword_lex_fn: None,
                keyword_capture_token: 0,
                external_scanner: #external_scanner_struct,
                primary_state_ids: PRIMARY_STATE_IDS.as_ptr(),
                production_lhs_index: PRODUCTION_LHS_INDEX.as_ptr(),
                production_count: #production_id_count as u16,
                eof_symbol: 0, // EOF is always column 0 in Tree-sitter convention
                rules: TS_RULES.as_ptr(),
                rule_count: TS_RULES.len() as u16,
            };

            // Export the language function for FFI
            // Edition-aware attribute toggle (2021 vs 2024)
            // SAFETY: LANGUAGE is a well-formed static TSLanguage struct initialized
            // from compile-time-generated tables. Returning a pointer to it is safe
            // because statics have 'static lifetime and stable addresses.
            #[cfg(adze_unsafe_attrs)]
            #[unsafe(no_mangle)]
            #[cfg(not(adze_unsafe_attrs))]
            #[no_mangle]
            pub unsafe extern "C" fn #language_fn_ident() -> *const TSLanguage {
                &LANGUAGE as *const TSLanguage
            }
        }
    }
}

struct LanguageCounts {
    symbol_count: u32,
    alias_count: u32,
    token_count: u32,
    external_token_count: u32,
    state_count: u32,
    large_state_count: u32,
    production_id_count: u32,
    field_count: u32,
    max_alias_sequence_length: u16,
}

mod aliases;
mod counts;
mod fields;
mod symbols;
mod tables;

#[cfg(test)]
mod tests;

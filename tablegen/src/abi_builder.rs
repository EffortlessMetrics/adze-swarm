#![cfg_attr(feature = "strict_docs", allow(missing_docs))]
//! Builder for ABI-compatible Tree-sitter Language structures.

// ABI-compatible language builder for Tree-sitter
// This module generates static Language structures that match Tree-sitter's C ABI exactly

use crate::compress::CompressedTables;
use adze_glr_core::ParseTable;
use adze_ir::{Grammar, SymbolId};
use proc_macro2::TokenStream;
use quote::quote;

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

// Submodules host the SRP-decomposed helpers used by `generate`.
// They are declared after `debug_trace!` so the macro is in scope.
mod code_pieces;
mod counts;
mod diagnostics;
mod fields;
mod metadata;
mod parse_tables;
mod productions;
mod symbols;

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

    /// Get the name of a symbol for debugging
    fn get_symbol_name(&self, symbol_id: SymbolId) -> String {
        if symbol_id == self.parse_table.eof_symbol {
            "end".to_string()
        } else if let Some(name) = self.grammar.rule_names.get(&symbol_id) {
            name.clone()
        } else if let Some(token) = self.grammar.tokens.get(&symbol_id) {
            token.name.clone()
        } else {
            format!("???{}", symbol_id.0)
        }
    }

    /// Generate the complete language module.
    ///
    /// Orchestrates the SRP-decomposed helpers: diagnostic logging
    /// (`log_generation_start`, etc.), per-field static-array generators
    /// (`generate_*` methods), conditional fragment builders
    /// (`build_external_scanner_pieces`, etc.), and the final
    /// `TSLanguage` assembly below.
    pub fn generate(&self) -> TokenStream {
        let language_name = &self.grammar.name;
        let language_fn_ident = quote::format_ident!("tree_sitter_{}", language_name);

        self.log_generation_start(language_name);
        self.log_state0_actions();

        // Generate all static data with deterministic ordering.
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

        // Build conditional fragments via the SRP-decomposed helpers.
        let (external_scanner_code, external_scanner_struct) = self.build_external_scanner_pieces();
        let counts = self.calculate_counts();
        let alias_pieces = self.build_alias_table_pieces(&counts, &alias_map, &alias_sequences);
        let field_names_array = self.build_field_names_array(&counts, &field_name_ptrs);

        self.log_lexer_token_mapping(&counts);

        // Generate lexer function with symbol mapping
        let lexer_code =
            crate::lexer_gen::generate_lexer(self.grammar, &self.parse_table.symbol_to_index);

        // Bind LanguageCounts fields and alias fragments into named locals so
        // the `quote!` template below can interpolate them directly.
        let symbol_count = counts.symbol_count;
        let alias_count = counts.alias_count;
        let token_count = counts.token_count;
        let external_token_count = counts.external_token_count;
        let state_count = counts.state_count;
        let large_state_count = counts.large_state_count;
        let production_id_count = counts.production_id_count;
        let field_count = counts.field_count;
        let max_alias_sequence_length = counts.max_alias_sequence_length;
        let alias_tables = &alias_pieces.tables;
        let alias_map_ptr = &alias_pieces.map_ptr;
        let alias_sequences_ptr = &alias_pieces.sequences_ptr;

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

pub(super) struct LanguageCounts {
    pub(super) symbol_count: u32,
    pub(super) alias_count: u32,
    pub(super) token_count: u32,
    pub(super) external_token_count: u32,
    pub(super) state_count: u32,
    pub(super) large_state_count: u32,
    pub(super) production_id_count: u32,
    pub(super) field_count: u32,
    pub(super) max_alias_sequence_length: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use adze_glr_core::{Action, LexMode};
    use adze_ir::*;

    fn token_stream_u16(token: &TokenStream) -> u16 {
        token.to_string().trim_end_matches("u16").parse().unwrap()
    }

    #[test]
    fn test_deterministic_symbol_ordering() {
        let mut grammar = Grammar::new("test".to_string());

        // Add tokens in non-sorted order
        grammar.tokens.insert(
            SymbolId(5),
            Token {
                name: "token5".to_string(),
                pattern: TokenPattern::String("5".to_string()),
                fragile: false,
            },
        );
        grammar.tokens.insert(
            SymbolId(1),
            Token {
                name: "token1".to_string(),
                pattern: TokenPattern::String("1".to_string()),
                fragile: false,
            },
        );

        let mut symbol_to_index = std::collections::BTreeMap::new();
        symbol_to_index.insert(SymbolId(0), 0); // EOF
        symbol_to_index.insert(SymbolId(1), 1); // token1
        symbol_to_index.insert(SymbolId(5), 2); // token5

        // Create a minimal parse table for testing
        let mut parse_table = crate::empty_table!(states: 1, terms: 2, nonterms: 0);

        // Override the symbol mapping for the test
        parse_table.symbol_to_index = symbol_to_index;
        parse_table.symbol_count = 3;

        let builder = AbiLanguageBuilder::new(&grammar, &parse_table);
        let (names, _) = builder.generate_symbol_names();

        // Should have EOF + 2 tokens
        assert_eq!(names.len(), 3);

        // Check that tokens are sorted by ID
        let code = quote! { #(#names)* }.to_string();

        // The token names are encoded as u8 byte arrays
        // "token1" = [116u8, 111u8, 107u8, 101u8, 110u8, 49u8, 0u8]
        // "token5" = [116u8, 111u8, 107u8, 101u8, 110u8, 53u8, 0u8]
        // We check for the distinguishing bytes: 49u8 for '1' and 53u8 for '5'
        assert!(code.contains("49u8")); // '1' in token1
        assert!(code.contains("53u8")); // '5' in token5
        let token1_pos = code.find("49u8").unwrap();
        let token5_pos = code.find("53u8").unwrap();
        assert!(token1_pos < token5_pos);
    }

    #[test]
    fn test_generate_production_id_map_includes_first_slot() {
        let mut grammar = Grammar::new("test".to_string());

        let start = SymbolId(1);
        let t = SymbolId(2);
        grammar.rule_names.insert(start, "start".to_string());
        grammar.tokens.insert(
            t,
            Token {
                name: "t".to_string(),
                pattern: TokenPattern::String("t".to_string()),
                fragile: false,
            },
        );

        grammar.add_rule(Rule {
            lhs: start,
            rhs: vec![Symbol::Terminal(t)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });
        grammar.add_rule(Rule {
            lhs: start,
            rhs: vec![Symbol::Terminal(t)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(1),
        });
        grammar.add_rule(Rule {
            lhs: start,
            rhs: vec![Symbol::Terminal(t)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(2),
        });

        let parse_table = crate::empty_table!(states: 1, terms: 1, nonterms: 1);
        let builder = AbiLanguageBuilder::new(&grammar, &parse_table);
        let production_map = builder.generate_production_id_map();

        assert_eq!(production_map.len(), 3);
        assert_eq!(production_map[0].to_string(), "0u16");
        assert_eq!(production_map[1].to_string(), "1u16");
        assert_eq!(production_map[2].to_string(), "2u16");
    }

    #[test]
    fn test_generate_lex_modes_uses_parse_table_modes() {
        let grammar = Grammar::new("lex_modes".to_string());
        let mut parse_table = crate::empty_table!(states: 3, terms: 1, nonterms: 1, externals: 1);
        parse_table.lex_modes = vec![
            LexMode {
                lex_state: 4,
                external_lex_state: 0,
            },
            LexMode {
                lex_state: 7,
                external_lex_state: 2,
            },
            LexMode {
                lex_state: 4,
                external_lex_state: 9,
            },
        ];

        let builder = AbiLanguageBuilder::new(&grammar, &parse_table);
        let modes = builder.generate_lex_modes();

        assert!(modes[0].to_string().contains("lex_state : 4u16"));
        assert!(modes[1].to_string().contains("lex_state : 7u16"));
        assert!(modes[1].to_string().contains("external_lex_state : 2u16"));
        assert!(modes[2].to_string().contains("external_lex_state : 9u16"));
    }

    // --- ABI compatibility tests (correctness-tablegen-compat) ---

    /// Single-production grammar yields a map of length 1.
    #[test]
    fn test_production_id_map_single_production() {
        let mut grammar = Grammar::new("single".to_string());
        let start = SymbolId(1);
        let t = SymbolId(2);
        grammar.rule_names.insert(start, "start".to_string());
        grammar.tokens.insert(
            t,
            Token {
                name: "t".to_string(),
                pattern: TokenPattern::String("t".to_string()),
                fragile: false,
            },
        );
        grammar.add_rule(Rule {
            lhs: start,
            rhs: vec![Symbol::Terminal(t)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });

        let table = crate::empty_table!(states: 1, terms: 1, nonterms: 1);
        let builder = AbiLanguageBuilder::new(&grammar, &table);
        let map = builder.generate_production_id_map();

        assert_eq!(map.len(), 1, "single production → map length 1");
        assert_eq!(map[0].to_string(), "0u16");
    }

    /// EOF symbol metadata must be visible=true, named=false (Tree-sitter convention).
    #[test]
    fn test_eof_metadata_visible_unnamed() {
        let mut grammar = Grammar::new("eof_meta".to_string());
        let start = SymbolId(1);
        let t = SymbolId(2);
        grammar.rule_names.insert(start, "start".to_string());
        grammar.tokens.insert(
            t,
            Token {
                name: "tok".to_string(),
                pattern: TokenPattern::String("t".to_string()),
                fragile: false,
            },
        );
        grammar.add_rule(Rule {
            lhs: start,
            rhs: vec![Symbol::Terminal(t)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });

        let table = crate::empty_table!(states: 1, terms: 1, nonterms: 1);
        let builder = AbiLanguageBuilder::new(&grammar, &table);
        let metadata = builder.generate_symbol_metadata();

        assert_eq!(
            metadata.len(),
            table.symbol_count,
            "metadata length must equal symbol_count"
        );

        // EOF metadata: visible=true(0x01), named=false → 0x01
        let eof_idx = table.symbol_to_index[&table.eof_symbol];
        assert_eq!(
            metadata[eof_idx].to_string(),
            "1u8",
            "EOF metadata must be 0x01 (visible, not named)"
        );
    }

    /// Metadata length matches parse table symbol_count exactly.
    #[test]
    fn test_symbol_metadata_length_matches_symbol_count() {
        let mut grammar = Grammar::new("meta_len".to_string());
        let start = SymbolId(1);
        let t1 = SymbolId(2);
        let t2 = SymbolId(3);
        grammar.rule_names.insert(start, "start".to_string());
        for (id, name) in [(t1, "a"), (t2, "b")] {
            grammar.tokens.insert(
                id,
                Token {
                    name: name.to_string(),
                    pattern: TokenPattern::String(name.to_string()),
                    fragile: false,
                },
            );
        }
        grammar.add_rule(Rule {
            lhs: start,
            rhs: vec![Symbol::Terminal(t1), Symbol::Terminal(t2)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });

        let table = crate::empty_table!(states: 2, terms: 2, nonterms: 1);
        let builder = AbiLanguageBuilder::new(&grammar, &table);
        let metadata = builder.generate_symbol_metadata();

        assert_eq!(metadata.len(), table.symbol_count);
    }

    /// calculate_counts must reflect parse table dimensions.
    #[test]
    fn test_calculate_counts_matches_table_dimensions() {
        let mut grammar = Grammar::new("counts".to_string());
        let start = SymbolId(1);
        let t = SymbolId(2);
        grammar.rule_names.insert(start, "start".to_string());
        grammar.tokens.insert(
            t,
            Token {
                name: "t".to_string(),
                pattern: TokenPattern::String("t".to_string()),
                fragile: false,
            },
        );
        grammar.add_rule(Rule {
            lhs: start,
            rhs: vec![Symbol::Terminal(t)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });
        grammar.fields.insert(FieldId(0), "val".to_string());

        let table = crate::empty_table!(states: 5, terms: 1, nonterms: 1);
        let builder = AbiLanguageBuilder::new(&grammar, &table);
        let counts = builder.calculate_counts();

        assert_eq!(counts.symbol_count as usize, table.symbol_count);
        assert_eq!(counts.state_count as usize, table.state_count);
        assert_eq!(counts.token_count as usize, table.token_count);
        assert_eq!(counts.field_count, 1);
        assert_eq!(
            counts.external_token_count as usize,
            table.external_token_count
        );
    }

    /// generate() produces code with the correct ABI version.
    #[test]
    fn test_generate_contains_abi_version_15() {
        let table = crate::empty_table!(states: 1, terms: 1, nonterms: 1);
        // Use the table's start_symbol as the rule LHS to match non-terminal region.
        let start = table.start_symbol;
        let t = SymbolId(1); // terminal column

        let mut grammar = Grammar::new("ver".to_string());
        grammar.rule_names.insert(start, "start".to_string());
        grammar.tokens.insert(
            t,
            Token {
                name: "t".to_string(),
                pattern: TokenPattern::String("t".to_string()),
                fragile: false,
            },
        );
        grammar.add_rule(Rule {
            lhs: start,
            rhs: vec![Symbol::Terminal(t)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });

        let builder = AbiLanguageBuilder::new(&grammar, &table);
        let code = builder.generate().to_string();

        assert!(
            code.contains("TREE_SITTER_LANGUAGE_VERSION"),
            "generated code must reference TREE_SITTER_LANGUAGE_VERSION"
        );
    }

    /// Encode/decode roundtrip for Shift, Reduce, Accept, Error through
    /// the AbiLanguageBuilder's encode_action method.
    #[test]
    fn test_encode_action_roundtrip() {
        let grammar = Grammar::new("enc".to_string());
        let table = crate::empty_table!(states: 1, terms: 0, nonterms: 0);
        let builder = AbiLanguageBuilder::new(&grammar, &table);

        // Shift
        let enc = builder
            .encode_action(&Action::Shift(adze_ir::StateId(42)))
            .unwrap();
        assert_eq!(enc, 42, "Shift(42) → 42");

        // Reduce (1-based in Tree-sitter)
        let enc = builder.encode_action(&Action::Reduce(RuleId(3))).unwrap();
        assert_eq!(enc, 0x8000 | 0x0004, "Reduce(3) -> 0x8004");

        // Accept
        let enc = builder.encode_action(&Action::Accept).unwrap();
        assert_eq!(enc, 0xFFFF, "Accept → 0xFFFF");

        // Error
        let enc = builder.encode_action(&Action::Error).unwrap();
        assert_eq!(enc, 0, "Error → 0");
    }

    #[test]
    fn test_fallback_parse_table_preserves_multi_action_cell() {
        let mut table = crate::empty_table!(states: 2, terms: 1, nonterms: 1);
        let start = table.start_symbol;
        let t = SymbolId(1);

        for row in &mut table.goto_table {
            row.fill(StateId(0));
        }
        table.action_table[0][1] = vec![
            Action::Error,
            Action::Shift(StateId(1)),
            Action::Reduce(RuleId(0)),
        ];

        let mut grammar = Grammar::new("fallback_conflict".to_string());
        grammar.rule_names.insert(start, "start".to_string());
        grammar.tokens.insert(
            t,
            Token {
                name: "t".to_string(),
                pattern: TokenPattern::String("t".to_string()),
                fragile: false,
            },
        );
        grammar.add_rule(Rule {
            lhs: start,
            rhs: vec![Symbol::Terminal(t)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });

        let builder = AbiLanguageBuilder::new(&grammar, &table);
        let (table_data, table_map) = builder.generate_parse_tables();
        let values: Vec<u16> = table_data.iter().map(token_stream_u16).collect();
        let offsets: Vec<u32> = table_map
            .iter()
            .map(|token| token.to_string().trim_end_matches("u32").parse().unwrap())
            .collect();

        assert_eq!(offsets[0], 0, "state 0 starts at the first pair");
        assert_eq!(offsets[1], 4, "state 1 starts after both state 0 pairs");
        assert_eq!(values.len(), 4, "state 0 must emit two direct pairs");
        assert_eq!(values[0], 1, "first entry symbol");
        assert_eq!(
            values[1],
            builder.encode_action(&Action::Shift(StateId(1))).unwrap()
        );
        assert_eq!(values[2], 1, "second entry symbol");
        assert_eq!(
            values[3],
            builder.encode_action(&Action::Reduce(RuleId(0))).unwrap()
        );
    }

    #[test]
    fn test_fallback_parse_table_emits_goto_once() {
        let mut table = crate::empty_table!(states: 2, terms: 1, nonterms: 1);
        let start = table.start_symbol;
        let t = SymbolId(1);

        for row in &mut table.goto_table {
            row.fill(StateId(0));
        }
        table.goto_table[0][start.0 as usize] = StateId(1);

        let mut grammar = Grammar::new("fallback_goto".to_string());
        grammar.rule_names.insert(start, "start".to_string());
        grammar.tokens.insert(
            t,
            Token {
                name: "t".to_string(),
                pattern: TokenPattern::String("t".to_string()),
                fragile: false,
            },
        );
        grammar.add_rule(Rule {
            lhs: start,
            rhs: vec![Symbol::Terminal(t)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });

        let builder = AbiLanguageBuilder::new(&grammar, &table);
        let (table_data, table_map) = builder.generate_parse_tables();
        let values: Vec<u16> = table_data.iter().map(token_stream_u16).collect();
        let offsets: Vec<u32> = table_map
            .iter()
            .map(|token| token.to_string().trim_end_matches("u32").parse().unwrap())
            .collect();

        assert_eq!(offsets, vec![0, 2, 2]);
        assert_eq!(
            values,
            vec![start.0, 1],
            "state 0 should contain exactly one direct goto pair"
        );
    }

    /// Production LHS index entries must all reference non-terminal columns.
    #[test]
    fn test_production_lhs_index_nonterminal_columns() {
        let table = crate::empty_table!(states: 2, terms: 1, nonterms: 1);
        let start = table.start_symbol;
        let t = SymbolId(1); // terminal column

        let mut grammar = Grammar::new("lhs".to_string());
        grammar.rule_names.insert(start, "start".to_string());
        grammar.tokens.insert(
            t,
            Token {
                name: "t".to_string(),
                pattern: TokenPattern::String("t".to_string()),
                fragile: false,
            },
        );
        grammar.add_rule(Rule {
            lhs: start,
            rhs: vec![Symbol::Terminal(t)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });

        let table = crate::empty_table!(states: 2, terms: 1, nonterms: 1);
        let builder = AbiLanguageBuilder::new(&grammar, &table);
        let lhs_index = builder.generate_production_lhs_index();

        // Every LHS must be ≥ token_count (non-terminal region)
        for (i, token) in lhs_index.iter().enumerate() {
            let val: u16 = token.to_string().trim_end_matches("u16").parse().unwrap();
            assert!(
                val as usize >= table.token_count,
                "production_lhs_index[{}] = {} must be >= token_count {}",
                i,
                val,
                table.token_count
            );
        }
    }

    #[test]
    fn test_field_map_slices_are_dense_and_include_production_zero() {
        let table = crate::empty_table!(states: 1, terms: 1, nonterms: 1);
        let start = table.start_symbol;
        let t = SymbolId(1);

        let mut grammar = Grammar::new("field_maps".to_string());
        grammar.rule_names.insert(start, "start".to_string());
        grammar.tokens.insert(
            t,
            Token {
                name: "t".to_string(),
                pattern: TokenPattern::String("t".to_string()),
                fragile: false,
            },
        );
        grammar.fields.insert(FieldId(0), "first".to_string());
        grammar.fields.insert(FieldId(1), "third".to_string());
        grammar.add_rule(Rule {
            lhs: start,
            rhs: vec![Symbol::Terminal(t)],
            precedence: None,
            associativity: None,
            fields: vec![(FieldId(0), 0)],
            production_id: ProductionId(0),
        });
        grammar.add_rule(Rule {
            lhs: start,
            rhs: vec![Symbol::Terminal(t)],
            precedence: None,
            associativity: None,
            fields: vec![(FieldId(1), 0)],
            production_id: ProductionId(2),
        });

        let builder = AbiLanguageBuilder::new(&grammar, &table);
        let (slices, entries) = builder.generate_field_maps();
        let slices: Vec<String> = slices.iter().map(ToString::to_string).collect();

        assert_eq!(
            slices.len(),
            6,
            "field_map_slices must have two words per production ID"
        );
        assert_eq!(slices[0], "0u16", "production 0 start");
        assert_eq!(slices[1], "1u16", "production 0 length");
        assert_eq!(slices[2], "0u16", "production 1 gap start");
        assert_eq!(slices[3], "0u16", "production 1 gap length");
        assert_eq!(
            slices[4], "1u16",
            "production 2 start is entry index, not word offset"
        );
        assert_eq!(slices[5], "1u16", "production 2 length");
        assert_eq!(
            entries.len(),
            4,
            "two field-map entries should emit two u16 words each"
        );
    }

    #[test]
    fn test_field_map_entries_use_abi_field_name_indices() {
        let table = crate::empty_table!(states: 1, terms: 1, nonterms: 1);
        let start = table.start_symbol;
        let t = SymbolId(1);

        let mut grammar = Grammar::new("field_map_name_indices".to_string());
        grammar.rule_names.insert(start, "start".to_string());
        grammar.tokens.insert(
            t,
            Token {
                name: "t".to_string(),
                pattern: TokenPattern::String("t".to_string()),
                fragile: false,
            },
        );
        grammar
            .fields
            .insert(FieldId(0), "TestModule_statements_vec_element".to_string());
        grammar.fields.insert(FieldId(1), "value".to_string());
        grammar.fields.insert(FieldId(2), "statements".to_string());
        grammar.fields.insert(FieldId(3), "_whitespace".to_string());
        grammar.add_rule(Rule {
            lhs: start,
            rhs: vec![Symbol::Terminal(t)],
            precedence: None,
            associativity: None,
            fields: vec![(FieldId(1), 0)],
            production_id: ProductionId(0),
        });

        let builder = AbiLanguageBuilder::new(&grammar, &table);
        let (_, entries) = builder.generate_field_maps();
        let entries: Vec<String> = entries.iter().map(ToString::to_string).collect();

        assert_eq!(
            entries[0], "3u32 as u16",
            "field map entries must use the FIELD_NAME_PTRS ABI index for value"
        );
    }

    #[test]
    fn test_empty_field_maps_keep_dense_slices_and_non_null_entry_placeholder() {
        let table = crate::empty_table!(states: 1, terms: 1, nonterms: 1);
        let start = table.start_symbol;
        let t = SymbolId(1);

        let mut grammar = Grammar::new("empty_field_maps".to_string());
        grammar.rule_names.insert(start, "start".to_string());
        grammar.tokens.insert(
            t,
            Token {
                name: "t".to_string(),
                pattern: TokenPattern::String("t".to_string()),
                fragile: false,
            },
        );
        grammar.add_rule(Rule {
            lhs: start,
            rhs: vec![Symbol::Terminal(t)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });

        let builder = AbiLanguageBuilder::new(&grammar, &table);
        let (slices, entries) = builder.generate_field_maps();
        let slices: Vec<String> = slices.iter().map(ToString::to_string).collect();
        let entries: Vec<String> = entries.iter().map(ToString::to_string).collect();

        assert_eq!(slices, vec!["0u16", "0u16"]);
        assert_eq!(entries, vec!["0u16"]);
    }

    // --- SRP helper tests: pin the contracts of the code_pieces helpers ---

    fn minimal_builder_fixture() -> (Grammar, ParseTable) {
        let mut grammar = Grammar::new("helper_fixture".to_string());
        let start = SymbolId(1);
        let t = SymbolId(2);
        grammar.rule_names.insert(start, "start".to_string());
        grammar.tokens.insert(
            t,
            Token {
                name: "t".to_string(),
                pattern: TokenPattern::String("t".to_string()),
                fragile: false,
            },
        );
        grammar.add_rule(Rule {
            lhs: start,
            rhs: vec![Symbol::Terminal(t)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });
        let table = crate::empty_table!(states: 1, terms: 1, nonterms: 1);
        (grammar, table)
    }

    #[test]
    fn build_external_scanner_pieces_is_null_when_no_externals() {
        let (grammar, table) = minimal_builder_fixture();
        let builder = AbiLanguageBuilder::new(&grammar, &table);
        let (code, struct_lit) = builder.build_external_scanner_pieces();

        assert!(
            code.is_empty(),
            "no externals → scanner code block must be empty"
        );
        let struct_str = struct_lit.to_string();
        assert!(
            struct_str.contains("std :: ptr :: null ()"),
            "expected null pointer literals in struct, got: {struct_str}"
        );
        assert!(!struct_str.contains("EXTERNAL_SCANNER_STATES"));
    }

    #[test]
    fn build_external_scanner_pieces_emits_interface_when_externals_present() {
        let (mut grammar, table) = minimal_builder_fixture();
        // Inject a minimal external token via the public Grammar API.
        let ext_id = SymbolId(100);
        grammar.externals.push(ExternalToken {
            symbol_id: ext_id,
            name: "ext_token".to_string(),
        });

        let builder = AbiLanguageBuilder::new(&grammar, &table);
        let (code, struct_lit) = builder.build_external_scanner_pieces();

        assert!(
            !code.is_empty(),
            "externals present → scanner code block must be non-empty"
        );
        assert!(
            struct_lit.to_string().contains("EXTERNAL_SCANNER_STATES"),
            "scanner struct must reference EXTERNAL_SCANNER_STATES when externals exist"
        );
    }

    #[test]
    fn build_alias_table_pieces_falls_back_to_null_when_empty() {
        let (grammar, table) = minimal_builder_fixture();
        let builder = AbiLanguageBuilder::new(&grammar, &table);
        let counts = builder.calculate_counts();
        let pieces = builder.build_alias_table_pieces(&counts, &[], &[]);

        assert!(pieces.tables.is_empty(), "empty alias case → no tables");
        assert!(
            pieces.map_ptr.to_string().contains("null"),
            "expected null map_ptr, got: {}",
            pieces.map_ptr
        );
        assert!(
            pieces.sequences_ptr.to_string().contains("null"),
            "expected null sequences_ptr, got: {}",
            pieces.sequences_ptr
        );
    }

    #[test]
    fn build_alias_table_pieces_emits_statics_when_aliases_present() {
        let (grammar, table) = minimal_builder_fixture();
        let builder = AbiLanguageBuilder::new(&grammar, &table);
        // Synthesize counts that drive the "has aliases" branch.
        let counts = LanguageCounts {
            symbol_count: 1,
            alias_count: 1,
            token_count: 1,
            external_token_count: 0,
            state_count: 1,
            large_state_count: 0,
            production_id_count: 1,
            field_count: 0,
            max_alias_sequence_length: 1,
        };
        let map_token: TokenStream = quote! { 7u16 };
        let seq_token: TokenStream = quote! { 9u16 };
        let pieces = builder.build_alias_table_pieces(&counts, &[map_token], &[seq_token]);

        let tables_str = pieces.tables.to_string();
        assert!(
            tables_str.contains("ALIAS_MAP") && tables_str.contains("ALIAS_SEQUENCES"),
            "tables must declare both alias statics, got: {tables_str}"
        );
        assert_eq!(pieces.map_ptr.to_string(), "ALIAS_MAP . as_ptr ()");
        assert_eq!(
            pieces.sequences_ptr.to_string(),
            "ALIAS_SEQUENCES . as_ptr ()"
        );
    }

    #[test]
    fn build_field_names_array_uses_zero_sized_when_no_fields() {
        let (grammar, table) = minimal_builder_fixture();
        let builder = AbiLanguageBuilder::new(&grammar, &table);
        let counts = builder.calculate_counts();
        let decl = builder.build_field_names_array(&counts, &[]);
        let decl_str = decl.to_string();

        assert!(decl_str.contains("FIELD_NAME_PTRS"));
        assert!(
            decl_str.contains("[SyncPtr ; 0]"),
            "no fields → zero-sized array literal, got: {decl_str}"
        );
        assert!(!decl_str.contains("FIELD_NAME_PTRS_LEN"));
    }

    #[test]
    fn build_field_names_array_uses_const_len_when_fields_present() {
        let (mut grammar, table) = minimal_builder_fixture();
        grammar.fields.insert(FieldId(0), "a".to_string());
        grammar.fields.insert(FieldId(1), "b".to_string());

        let builder = AbiLanguageBuilder::new(&grammar, &table);
        let counts = builder.calculate_counts();
        let ptrs = vec![
            quote! { SyncPtr::new(FIELD_NAME_0.as_ptr()) },
            quote! { SyncPtr::new(FIELD_NAME_1.as_ptr()) },
        ];
        let decl = builder.build_field_names_array(&counts, &ptrs);
        let decl_str = decl.to_string();

        assert!(decl_str.contains("FIELD_NAME_PTRS_LEN"));
        assert!(decl_str.contains("2u32 as usize"));
        assert!(decl_str.contains("FIELD_NAME_0"));
        assert!(decl_str.contains("FIELD_NAME_1"));
    }
}

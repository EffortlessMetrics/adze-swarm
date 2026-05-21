#![cfg_attr(feature = "strict_docs", allow(missing_docs))]
//! TSLanguage structure generation with ABI compatibility.

// Proper TSLanguage structure generation
// This module creates a valid Tree-sitter Language structure from our IR

use crate::abi::TREE_SITTER_LANGUAGE_VERSION;
use adze_glr_core::ParseTable;
use adze_ir::Grammar;
use proc_macro2::TokenStream;
use quote::quote;

mod compression;

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

/// Language generator that creates proper TSLanguage structures
pub struct LanguageGenerator<'a> {
    grammar: &'a Grammar,
    parse_table: &'a ParseTable,
}

impl<'a> LanguageGenerator<'a> {
    pub fn new(grammar: &'a Grammar, parse_table: &'a ParseTable) -> Self {
        Self {
            grammar,
            parse_table,
        }
    }

    /// Generate the complete language module with proper TSLanguage
    pub fn generate(&self) -> TokenStream {
        let language_name = &self.grammar.name;
        let language_fn_ident = quote::format_ident!("tree_sitter_{}", language_name);

        // Generate static data
        let symbol_names = self.generate_symbol_names();
        let field_names = self.generate_field_names();
        let symbol_metadata = self.generate_symbol_metadata();
        let parse_actions = self.generate_parse_actions();
        let lex_modes = self.generate_lex_modes();
        let (compressed_table, small_table_map) = self.generate_compressed_tables();

        // Generate indices for symbol_names, field_names, and parser states
        let symbol_name_indices: Vec<usize> = (0..symbol_names.len()).collect();
        let field_name_indices: Vec<usize> = (0..field_names.len()).collect();
        let state_indices: Vec<usize> = (0..self.parse_table.state_count).collect();
        let public_symbol_map = self.generate_public_symbol_map();

        // Count various elements
        let symbol_count = self.count_symbols();
        // token_count includes EOF (symbol 0) plus all user-defined tokens
        let token_count = self.parse_table.token_count as u32;
        let field_count = self.grammar.fields.len() as u32;
        let state_count = self.parse_table.state_count as u32;
        let external_token_count = self.parse_table.external_token_count as u32;
        let large_state_count = self.determine_large_state_count() as u32;
        let production_id_count = self.count_production_ids() as u32;

        quote! {
            use adze::tree_sitter as ts;
            use crate::abi::{TSLanguage, TSSymbol, TSStateId, TSLexState, TSParseAction, ExternalScanner};
            const TREE_SITTER_LANGUAGE_VERSION: u32 = 15;
            const EXTERNAL_TOKEN_COUNT: u32 = #external_token_count;

            // Symbol names array
            static SYMBOL_NAMES: &[&str] = &[#(#symbol_names),*];
            static SYMBOL_NAMES_PTRS: &[*const u8] = &[
                #(SYMBOL_NAMES[#symbol_name_indices].as_ptr()),*
            ];

            // Field names array
            static FIELD_NAMES: &[&str] = &[#(#field_names),*];
            static FIELD_NAMES_PTRS: &[*const u8] = &[
                #(FIELD_NAMES[#field_name_indices].as_ptr()),*
            ];

            // Symbol metadata - each byte contains bits for: visible, named, hidden, supertype
            static SYMBOL_METADATA: &[u8] = &[#(#symbol_metadata),*];

            // Parse actions
            static PARSE_ACTIONS: &[TSParseAction] = &[#(#parse_actions),*];

            // Lex modes
            static LEX_MODES: &[TSLexState] = &[#(#lex_modes),*];

            // Parse table
            static PARSE_TABLE: &[u16] = &[#(#compressed_table),*];
            static SMALL_PARSE_TABLE_MAP: &[u32] = &[#(#small_table_map),*];

            // Field maps (placeholder for now)
            static FIELD_MAP_SLICES: &[u16] = &[];
            static FIELD_MAP_ENTRIES: &[u16] = &[];

            // Public symbol map from table columns to public SymbolIds
            static PUBLIC_SYMBOL_MAP: &[TSSymbol] = &[
                #(TSSymbol(#public_symbol_map as u16)),*
            ];

            // Primary state IDs
            static PRIMARY_STATE_IDS: &[TSStateId] = &[
                #(TSStateId(#state_indices as u16)),*
            ];

            // External scanner (if any)
            static EXTERNAL_SCANNER: ExternalScanner = ExternalScanner::default();

            // The language structure
            static LANGUAGE: TSLanguage = TSLanguage {
                version: #TREE_SITTER_LANGUAGE_VERSION,
                symbol_count: #symbol_count,
                alias_count: 0, // TODO: Implement aliases
                token_count: #token_count,
                external_token_count: EXTERNAL_TOKEN_COUNT,
                state_count: #state_count,
                large_state_count: #large_state_count,
                production_id_count: #production_id_count,
                field_count: #field_count,
                max_alias_sequence_length: 0,
                parse_table: PARSE_TABLE.as_ptr(),
                small_parse_table: PARSE_TABLE.as_ptr().wrapping_add(#large_state_count as usize * #symbol_count as usize),
                small_parse_table_map: SMALL_PARSE_TABLE_MAP.as_ptr(),
                parse_actions: PARSE_ACTIONS.as_ptr(),
                symbol_names: SYMBOL_NAMES_PTRS.as_ptr(),
                field_names: FIELD_NAMES_PTRS.as_ptr(),
                field_map_slices: FIELD_MAP_SLICES.as_ptr(),
                field_map_entries: FIELD_MAP_ENTRIES.as_ptr(),
                symbol_metadata: SYMBOL_METADATA.as_ptr(),
                public_symbol_map: PUBLIC_SYMBOL_MAP.as_ptr(),
                alias_map: std::ptr::null(),
                alias_sequences: std::ptr::null(),
                lex_modes: LEX_MODES.as_ptr(),
                lex_fn: None, // TODO: Implement custom lexer
                keyword_lex_fn: None,
                keyword_capture_token: TSSymbol(0),
                external_scanner: EXTERNAL_SCANNER,
                primary_state_ids: PRIMARY_STATE_IDS.as_ptr(),
            };

            /// Get the Tree-sitter Language for this grammar
            pub fn language() -> ts::Language {
                // SAFETY: LANGUAGE is a module-level static with all fields initialized
                // from compile-time-generated tables. The layout matches Tree-sitter's
                // C ABI (TSLanguage). `from_raw` requires a valid TSLanguage pointer.
                unsafe {
                    ts::Language::from_raw(&LANGUAGE as *const TSLanguage as *const _)
                }
            }

            /// Export for C FFI
            /// SAFETY: This function is required for Tree-sitter C ABI compatibility
            #[unsafe(no_mangle)]
            pub extern "C" fn #language_fn_ident() -> ts::Language {
                // SAFETY: `language()` returns a valid Language from a well-formed static.
                unsafe { language() }
            }
        }
    }

    fn generate_symbol_names(&self) -> Vec<String> {
        let mut names = Vec::with_capacity(self.parse_table.symbol_count);

        for (i, symbol_id) in self.parse_table.index_to_symbol.iter().enumerate() {
            if i == 0 {
                names.push("end".to_string());
                continue;
            }

            let name = if let Some(token) = self.grammar.tokens.get(symbol_id) {
                token.name.clone()
            } else if let Some(external) = self
                .grammar
                .externals
                .iter()
                .find(|e| e.symbol_id == *symbol_id)
            {
                external.name.clone()
            } else {
                self.grammar
                    .rule_names
                    .get(symbol_id)
                    .cloned()
                    .unwrap_or_else(|| format!("rule_{}", symbol_id.0))
            };
            debug_trace!(
                "DEBUG: Symbol index {} -> ID {} (name {})",
                i,
                symbol_id.0,
                name
            );
            names.push(name);
        }

        names
    }

    fn generate_field_names(&self) -> Vec<String> {
        let mut names = vec![];
        for (_id, name) in &self.grammar.fields {
            names.push(name.clone());
        }
        names
    }

    fn generate_symbol_metadata(&self) -> Vec<u8> {
        let symbol_count = self.count_symbols();
        let mut metadata = vec![0u8; symbol_count];

        // Mark visible symbols
        for item in metadata.iter_mut().take(symbol_count) {
            // For now, mark all symbols as visible
            // Bit 0: visible
            // Bit 1: named
            *item = 0b11;
        }

        metadata
    }

    fn generate_parse_actions(&self) -> Vec<TokenStream> {
        // Generate simplified parse actions
        // In a real implementation, this would be derived from the parse table
        vec![quote! {
            TSParseAction {
                action_type: 0,
                extra: 0,
                child_count: 0,
                dynamic_precedence: 0,
                symbol: TSSymbol(0),
            }
        }]
    }

    fn generate_lex_modes(&self) -> Vec<TokenStream> {
        let state_count = self.parse_table.state_count;
        let mut modes = vec![];

        for state_index in 0..state_count {
            let mode = self
                .parse_table
                .lex_modes
                .get(state_index)
                .copied()
                .unwrap_or(adze_glr_core::LexMode {
                    lex_state: 0,
                    external_lex_state: 0,
                });
            let lex_state = mode.lex_state;
            let external_lex_state = mode.external_lex_state;
            modes.push(quote! {
                TSLexState {
                    lex_state: #lex_state,
                    external_lex_state: #external_lex_state,
                }
            });
        }

        modes
    }

    fn generate_compressed_tables(&self) -> (Vec<u16>, Vec<u32>) {
        // Tree-sitter's compression strategy:
        // - Large states (0 to LARGE_STATE_COUNT) use a 2D table indexed by [state][symbol]
        // - Small states use a compact format with entries like [count, symbol, action, ...]

        let large_state_count = self.determine_large_state_count();
        let mut compressed_table = Vec::new();

        compression::append_large_state_actions(
            &mut compressed_table,
            self.parse_table,
            large_state_count,
            |action| self.encode_action(action),
            |state, symbol| self.get_action(state, symbol),
        );

        let (small_table_data, small_table_map) = compression::build_small_state_data(
            self.parse_table,
            large_state_count,
            |action| self.encode_action(action),
            |state, symbol| self.get_action(state, symbol),
            |action| self.is_error_action(action),
        );

        compressed_table.extend(small_table_data);

        (compressed_table, small_table_map)
    }

    fn determine_large_state_count(&self) -> usize {
        // For now, use 0 large states to ensure all states use the packed action format
        // which is correctly handled by our pure-Rust decoder for small tables.
        0
    }

    fn get_action(&self, state: usize, symbol: usize) -> u16 {
        // Get the action from parse table
        if state < self.parse_table.action_table.len()
            && symbol < self.parse_table.action_table[state].len()
        {
            let action_cell = &self.parse_table.action_table[state][symbol];
            // For Tree-sitter compatibility, we need to pick one action
            // Use the first action if multiple exist (GLR conflicts)
            if action_cell.is_empty() {
                0xFFFE // Error action
            } else {
                let action = &action_cell[0];
                match action {
                    adze_glr_core::Action::Shift(s) => s.0,
                    adze_glr_core::Action::Reduce(r) => 0x8000 | (r.0 + 1),
                    adze_glr_core::Action::Accept => 0xFFFF,
                    adze_glr_core::Action::Error => 0xFFFE,
                    adze_glr_core::Action::Recover => 0xFFFD, // Use distinct value for Recover
                    adze_glr_core::Action::Fork(_) => 0xFFFE, // TODO: Handle GLR forks
                    _ => 0xFFFE, // Unknown action type // Expected: V for Recover
                }
            }
        } else {
            0xFFFE // Error action
        }
    }

    fn encode_action(&self, action: u16) -> u16 {
        // Actions are already encoded in get_action
        action
    }

    fn is_error_action(&self, action: u16) -> bool {
        action == 0xFFFE
    }

    fn count_symbols(&self) -> usize {
        // ParseTable already defines the exact symbol layout and count used by
        // ACTION/GOTO columns and symbol-name emission (including externals).
        // Deriving this from grammar token/rule lengths can drift for grammars
        // with external tokens or transformed symbol sets.
        self.parse_table.symbol_count
    }

    fn generate_public_symbol_map(&self) -> Vec<usize> {
        let symbol_count = self.count_symbols();
        let mut public_symbols: Vec<usize> = (0..symbol_count).collect();

        for (index, symbol_id) in self
            .parse_table
            .index_to_symbol
            .iter()
            .take(symbol_count)
            .enumerate()
        {
            public_symbols[index] = symbol_id.0 as usize;
        }

        if self.parse_table.index_to_symbol.len() < symbol_count {
            for (&symbol_id, &index) in &self.parse_table.symbol_to_index {
                if index < symbol_count {
                    public_symbols[index] = symbol_id.0 as usize;
                }
            }
        }

        public_symbols
    }

    fn count_production_ids(&self) -> usize {
        // Find the maximum production ID in all rules
        let mut max_production_id = 0;
        for (_, rules) in &self.grammar.rules {
            for rule in rules {
                max_production_id = max_production_id.max(rule.production_id.0);
            }
        }
        // Production ID count is max ID + 1 (since they start at 0)
        (max_production_id + 1) as usize
    }

    /// Public wrapper for `generate_symbol_metadata` (test use only).
    pub fn generate_symbol_metadata_public(&self) -> Vec<u8> {
        self.generate_symbol_metadata()
    }

    /// Public wrapper for `count_production_ids` (test use only).
    pub fn count_production_ids_public(&self) -> usize {
        self.count_production_ids()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adze_ir::*;

    #[test]
    fn test_language_generation() {
        let mut grammar = Grammar::new("test".to_string());

        // Add a simple token
        let num_token = Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        };
        grammar.tokens.insert(SymbolId(1), num_token);

        // Create a simple parse table
        let parse_table = crate::empty_table!(states: 10, terms: 4, nonterms: 0);

        let generator = LanguageGenerator::new(&grammar, &parse_table);
        let output = generator.generate();

        // Check that it generates valid code
        let output_str = output.to_string();
        assert!(output_str.contains("TSLanguage"));
        assert!(output_str.contains("tree_sitter_test"));
    }

    #[test]
    fn test_count_symbols_uses_parse_table_count_with_externals() {
        let mut grammar = Grammar::new("externals".to_string());
        grammar.tokens.insert(
            SymbolId(1),
            Token {
                name: "ident".to_string(),
                pattern: TokenPattern::Regex("[a-z]+".to_string()),
                fragile: false,
            },
        );
        grammar.externals.push(ExternalToken {
            name: "_external_nl".to_string(),
            symbol_id: SymbolId(3),
        });
        grammar
            .rule_names
            .insert(SymbolId(2), "source_file".to_string());

        let parse_table = adze_glr_core::ParseTable {
            state_count: 1,
            symbol_count: 4, // EOF + token + nonterminal + external
            index_to_symbol: vec![SymbolId(0), SymbolId(1), SymbolId(2), SymbolId(3)],
            token_count: 2,
            external_token_count: 1,
            ..Default::default()
        };

        let generator = LanguageGenerator::new(&grammar, &parse_table);
        assert_eq!(generator.count_symbols(), 4);
    }
}

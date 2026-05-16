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

// Submodules host the SRP-decomposed helpers used by `generate`.
// They are declared after `debug_trace!` so the macro is in scope.
mod code_pieces;
mod diagnostics;

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
    /// ([`Self::log_generation_start`], etc.), per-field static-array
    /// generators (`generate_*` methods), conditional fragment builders
    /// ([`Self::build_external_scanner_pieces`], etc.), and the final
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

    /// Generate symbol names with deterministic ordering
    fn generate_symbol_names(&self) -> (Vec<TokenStream>, Vec<TokenStream>) {
        let mut names = Vec::new();
        let mut name_idents = Vec::new();

        // Use the parse table's symbol ordering
        // Create reverse mapping from index to symbol ID
        let mut index_to_symbol: Vec<Option<SymbolId>> = vec![None; self.parse_table.symbol_count];
        for (symbol_id, &index) in &self.parse_table.symbol_to_index {
            if index < self.parse_table.symbol_count {
                index_to_symbol[index] = Some(*symbol_id);
            }
        }

        // Generate names in parse table order
        for (idx, symbol_id_opt) in index_to_symbol.iter().enumerate() {
            let ident = quote::format_ident!("SYMBOL_NAME_{}", idx);

            let name_str = if let Some(symbol_id) = symbol_id_opt {
                if *symbol_id == self.parse_table.eof_symbol {
                    // EOF symbol
                    "end".to_string()
                } else if let Some(token) = self.grammar.tokens.get(symbol_id) {
                    // Terminal symbol
                    token.name.clone()
                } else if let Some(rule_name) = self.grammar.rule_names.get(symbol_id) {
                    // Non-terminal with explicit name
                    rule_name.clone()
                } else if let Some(external) = self
                    .grammar
                    .externals
                    .iter()
                    .find(|external| external.symbol_id == *symbol_id)
                {
                    // External token
                    external.name.clone()
                } else {
                    // Non-terminal without name - generate one
                    format!("rule_{}", symbol_id.0)
                }
            } else {
                // Should not happen
                format!("unknown_{}", idx)
            };

            let name_bytes = format!("{}\0", name_str).into_bytes();
            names.push(quote! {
                static #ident: &[u8] = &[#(#name_bytes),*];
            });
            name_idents.push(ident);
        }

        let ptrs = name_idents
            .iter()
            .map(|ident| {
                quote! { SyncPtr::new(#ident.as_ptr()) }
            })
            .collect();

        (names, ptrs)
    }

    /// Generate field names with lexicographic ordering
    fn generate_field_names(&self) -> (Vec<TokenStream>, Vec<TokenStream>) {
        let mut names = Vec::new();
        let mut name_idents = Vec::new();

        // Fields must be in lexicographic order
        let mut fields: Vec<_> = self.grammar.fields.iter().collect();
        fields.sort_by_key(|(_, name)| name.as_str());

        for (i, (_id, name)) in fields.iter().enumerate() {
            let ident = quote::format_ident!("FIELD_NAME_{}", i);
            let name_bytes = format!("{}\0", name).into_bytes();
            names.push(quote! {
                static #ident: &[u8] = &[#(#name_bytes),*];
            });
            name_idents.push(ident);
        }

        let ptrs = name_idents
            .iter()
            .map(|ident| {
                quote! { SyncPtr::new(#ident.as_ptr()) }
            })
            .collect();

        (names, ptrs)
    }

    fn field_name_indices_by_field_id(&self) -> std::collections::BTreeMap<u16, u16> {
        let mut fields: Vec<_> = self.grammar.fields.iter().collect();
        fields.sort_by_key(|(_, name)| name.as_str());
        fields
            .into_iter()
            .enumerate()
            .map(|(index, (field_id, _))| (field_id.0, index as u16))
            .collect()
    }

    /// Generate symbol metadata
    fn generate_symbol_metadata(&self) -> Vec<TokenStream> {
        let mut metadata = Vec::new();

        debug_trace!("\nDEBUG generate_symbol_metadata: Starting metadata generation");
        debug_trace!("  grammar.extras = {:?}", self.grammar.extras);

        // Debug: Check all tokens in the grammar
        debug_trace!("  All tokens in grammar:");
        for (id, token) in &self.grammar.tokens {
            debug_trace!(
                "    Token {:?}: name='{}', pattern={:?}",
                id,
                token.name,
                token.pattern
            );
        }

        // First, find all terminal tokens that should be marked as extras
        let extra_tokens = self.find_extra_tokens();
        debug_trace!("  extra_tokens found = {:?}", extra_tokens);

        // Debug: Print which symbol corresponds to whitespace
        debug_trace!("  Looking for whitespace token (should be symbol 4):");
        for (id, token) in &self.grammar.tokens {
            if token.name.contains("whitespace")
                || token.pattern == TokenPattern::Regex(r"\s".to_string())
            {
                debug_trace!(
                    "    Found whitespace-like token: {:?} -> {}",
                    id,
                    token.name
                );
            }
        }

        // Generate metadata in parse table order using symbol_to_index mapping
        let mut index_to_symbol: Vec<Option<SymbolId>> = vec![None; self.parse_table.symbol_count];
        for (symbol_id, &index) in &self.parse_table.symbol_to_index {
            if index < self.parse_table.symbol_count {
                index_to_symbol[index] = Some(*symbol_id);
            }
        }

        debug_trace!("  Generating metadata in parse table order:");
        debug_trace!(
            "  symbol_to_index mapping: {:?}",
            self.parse_table.symbol_to_index
        );
        for (idx, symbol_id_opt) in index_to_symbol.iter().enumerate() {
            if let Some(symbol_id) = symbol_id_opt {
                if *symbol_id == self.parse_table.eof_symbol {
                    // EOF symbol
                    let meta_byte = create_symbol_metadata(true, false, false, false, false);
                    debug_trace!("    Index {}: EOF, metadata={:#x}", idx, meta_byte);
                    metadata.push(quote! { #meta_byte });
                } else if let Some(token) = self.grammar.tokens.get(symbol_id) {
                    // Terminal token
                    let visible = !token.name.starts_with('_');
                    let named = visible && matches!(&token.pattern, TokenPattern::Regex(_));
                    let _original_hidden = extra_tokens.contains(symbol_id);

                    // Special handling for whitespace tokens
                    // If this is a whitespace token (by pattern), it should be hidden
                    let is_whitespace_token = matches!(&token.pattern, TokenPattern::Regex(p) if p == r"\s")
                        || token.name.to_lowercase().contains("whitespace");

                    if is_whitespace_token {
                        debug_trace!(
                            "    WHITESPACE TOKEN FOUND: {} (id={:?})",
                            token.name,
                            symbol_id
                        );
                        debug_trace!("      Pattern: {:?}", token.pattern);
                        debug_trace!(
                            "      Was in extra_tokens: {}",
                            extra_tokens.contains(symbol_id)
                        );
                    }

                    // Force whitespace tokens to be hidden
                    let hidden = extra_tokens.contains(symbol_id) || is_whitespace_token;

                    let meta_byte = create_symbol_metadata(visible, named, hidden, false, false);
                    debug_trace!(
                        "    Index {}: Token {} (id={:?}): visible={}, named={}, hidden={}, metadata={:#x}",
                        idx,
                        token.name,
                        symbol_id,
                        visible,
                        named,
                        hidden,
                        meta_byte
                    );
                    metadata.push(quote! { #meta_byte });
                } else if self.grammar.rules.contains_key(symbol_id) {
                    // Non-terminal
                    let name = self
                        .grammar
                        .rule_names
                        .get(symbol_id)
                        .cloned()
                        .unwrap_or_else(|| format!("rule_{}", symbol_id.0));
                    let visible = !name.starts_with('_');
                    let named = visible;
                    let hidden = false; // Non-terminals are never hidden
                    let supertype = self.grammar.supertypes.contains(symbol_id);
                    let meta_byte =
                        create_symbol_metadata(visible, named, hidden, false, supertype);
                    debug_trace!(
                        "    Index {}: Non-terminal {} (id={:?}): visible={}, named={}, supertype={}, metadata={:#x}",
                        idx,
                        name,
                        symbol_id,
                        visible,
                        named,
                        supertype,
                        meta_byte
                    );
                    metadata.push(quote! { #meta_byte });
                } else if let Some(external) = self
                    .grammar
                    .externals
                    .iter()
                    .find(|e| e.symbol_id == *symbol_id)
                {
                    // External token
                    let visible = !external.name.starts_with('_');
                    let named = visible;
                    let meta_byte = create_symbol_metadata(visible, named, false, false, false);
                    debug_trace!(
                        "    Index {}: External {} (id={:?}): visible={}, named={}, metadata={:#x}",
                        idx,
                        external.name,
                        symbol_id,
                        visible,
                        named,
                        meta_byte
                    );
                    metadata.push(quote! { #meta_byte });
                } else {
                    // Unknown symbol - shouldn't happen
                    debug_trace!(
                        "    Index {}: WARNING: Unknown symbol id={:?}",
                        idx,
                        symbol_id
                    );
                    metadata.push(quote! { 0u8 });
                }
            } else {
                // No symbol for this index - shouldn't happen
                debug_trace!("    Index {}: WARNING: No symbol mapped", idx);
                metadata.push(quote! { 0u8 });
            }
        }

        metadata
    }

    /// Generate compressed parse tables
    fn generate_parse_tables(&self) -> (Vec<TokenStream>, Vec<TokenStream>) {
        if let Some(compressed) = self.compressed_tables {
            // Generate compressed table data
            let mut table_data = Vec::new();
            let mut map_data = Vec::new();

            // Encode both action and goto table entries combined
            // Tree-sitter format: each state's row contains both actions (for terminals)
            // and gotos (for non-terminals) as (symbol, value) pairs

            for state_idx in 0..self.parse_table.state_count {
                // Record the starting offset for this state (in u16 array indices, not pairs)
                let current_offset = table_data.len();
                map_data.push(quote! { #current_offset as u32 });

                // Track which symbols already have action entries to avoid duplicates
                let mut action_symbols = std::collections::HashSet::new();

                // First, add action entries for this state
                let action_start = compressed.action_table.row_offsets[state_idx] as usize;
                let action_end = compressed.action_table.row_offsets[state_idx + 1] as usize;

                for entry in &compressed.action_table.data[action_start..action_end] {
                    let symbol = entry.symbol;
                    action_symbols.insert(symbol);
                    table_data.push(quote! { #symbol });
                    if let Ok(encoded) = self.encode_action(&entry.action) {
                        table_data.push(quote! { #encoded });
                    }
                }

                // Then, add goto entries for this state (encode as shifts for Tree-sitter compat)
                // Skip symbols that already have action entries to avoid duplicates
                if state_idx < self.parse_table.goto_table.len() {
                    for (symbol_idx, &goto_state) in
                        self.parse_table.goto_table[state_idx].iter().enumerate()
                    {
                        let symbol = symbol_idx as u16;
                        if goto_state.0 > 0 && !action_symbols.contains(&symbol) {
                            // This is a valid goto transition without a conflicting action
                            let encoded_shift = goto_state.0; // Shift actions are encoded as state_id
                            table_data.push(quote! { #symbol });
                            table_data.push(quote! { #encoded_shift });
                        }
                    }
                }
            }

            // Add final offset (end of table, in u16 array indices)
            let final_offset = table_data.len();
            map_data.push(quote! { #final_offset as u32 });

            (table_data, map_data)
        } else {
            // Fallback: generate compressed table format without proper compression
            // This stores only non-error entries as (symbol, action) pairs
            let mut table_data = Vec::new();
            let mut map_data = Vec::new();
            let mut current_offset = 0u32;

            debug_trace!(
                "DEBUG: goto_table.len() = {}, state_count = {}",
                self.parse_table.goto_table.len(),
                self.parse_table.state_count
            );

            for state_idx in 0..self.parse_table.state_count {
                // Record the starting offset for this state
                map_data.push(quote! { #current_offset });

                // We need to know the count before we start pushing
                let mut entries = Vec::new();

                let mut non_error_actions = Vec::new();

                for symbol_idx in 0..self.parse_table.symbol_count {
                    let symbol_id = self
                        .parse_table
                        .symbol_to_index
                        .iter()
                        .find(|&(_, &idx)| idx == symbol_idx)
                        .map(|(id, _)| *id);

                    let Some(symbol_id) = symbol_id else {
                        continue;
                    };

                    let is_terminal = self.grammar.tokens.contains_key(&symbol_id)
                        || self
                            .grammar
                            .externals
                            .iter()
                            .any(|e| e.symbol_id == symbol_id)
                        || symbol_id == self.parse_table.eof_symbol;

                    let mut record_action = |symbol_idx: usize, action: &Action| match action {
                        Action::Error => {}
                        _ => {
                            non_error_actions.push((symbol_idx, action.clone()));
                        }
                    };

                    if is_terminal {
                        if state_idx < self.parse_table.action_table.len()
                            && symbol_idx < self.parse_table.action_table[state_idx].len()
                        {
                            let actions = &self.parse_table.action_table[state_idx][symbol_idx];
                            for action in actions {
                                record_action(symbol_idx, action);
                            }
                        }
                    } else if state_idx < self.parse_table.goto_table.len()
                        && symbol_idx < self.parse_table.goto_table[state_idx].len()
                    {
                        let goto_state = self.parse_table.goto_table[state_idx][symbol_idx];
                        if goto_state.0 > 0 {
                            record_action(symbol_idx, &Action::Shift(goto_state));
                        }
                    }
                }

                for (symbol_idx, action) in non_error_actions {
                    if let Ok(encoded) = self.encode_action(&action) {
                        entries.push((symbol_idx as u16, encoded));
                    }
                }

                for (sym, val) in entries {
                    table_data.push(quote! { #sym });
                    table_data.push(quote! { #val });
                    current_offset += 2;
                }
            }

            // Add final offset for end of table
            debug_trace!("DEBUG: Final offset: {}", current_offset);
            map_data.push(quote! { #current_offset });

            (table_data, map_data)
        }
    }

    /// Encode an action as u16
    fn encode_action(&self, action: &Action) -> Result<u16, String> {
        match action {
            Action::Shift(state) => Ok(state.0),
            Action::Reduce(rule) => {
                // Tree-sitter uses 1-based production IDs in reduce actions
                // The runtime will map through PRODUCTION_ID_MAP to get the actual index
                Ok(0x8000 | (rule.0 + 1))
            }
            Action::Accept => Ok(0xFFFF), // Use 0xFFFF for accept (must match decoder in pure_parser.rs)
            Action::Error => Ok(0),       // Use 0 for error to match parser expectation
            Action::Recover => Ok(0xFFFD), // Use distinct value for Recover
            Action::Fork(actions) => {
                // For Fork actions, we need to choose one action from the fork
                // For now, let's prefer reduce actions over shift actions
                // This is a simplified conflict resolution strategy

                // First, try to find a reduce action
                for action in actions {
                    if let Action::Reduce(_) = action {
                        return self.encode_action(action);
                    }
                }

                // If no reduce action, take the first non-error action
                for action in actions {
                    if !matches!(action, Action::Error) {
                        return self.encode_action(action);
                    }
                }

                // If all actions are errors (shouldn't happen), return error
                Ok(0)
            }
            _ => {
                // Unknown action type // Expected: V for Recover
                crate::util::unexpected_action(action, "encode_action");
                Ok(0)
            }
        }
    }

    /// Generate parse actions
    fn generate_parse_actions(&self) -> Vec<TokenStream> {
        // Generate production information for reduce actions
        // The array must be indexed by production ID, not sequential

        // We need to size the array based on production_id_count
        let counts = self.calculate_counts();
        let production_id_count = counts.production_id_count as usize;

        // Create array with dummy entries (Shift to state 0, which is normally Error in state 0)
        let mut actions = vec![
            quote! {
                TSParseAction {
                    action_type: 3, // Error
                    extra: 0,
                    child_count: 0,
                    dynamic_precedence: 0,
                    symbol: 0,
                }
            };
            production_id_count
        ];

        // Fill in the actual productions at their correct indices
        // We MUST use the same rule ordering as generate_ts_rules
        let mut rules: Vec<_> = self
            .grammar
            .rules
            .iter()
            .flat_map(|(_, rules)| rules.iter())
            .collect();
        rules.sort_by_key(|rule| rule.production_id.0);

        for rule in rules {
            let index = rule.production_id.0 as usize;
            let child_count = rule.rhs.len() as u8;

            // Store the production ID because PARSE_ACTIONS is production-indexed.
            let symbol = rule.production_id.0;

            if index < actions.len() {
                actions[index] = quote! {
                    TSParseAction {
                        action_type: 1, // Reduce
                        extra: 0,
                        child_count: #child_count,
                        dynamic_precedence: 0,
                        symbol: #symbol,
                    }
                };
            }
        }

        actions
    }

    /// Generate lex modes
    fn generate_lex_modes(&self) -> Vec<TokenStream> {
        let mut modes = Vec::new();

        for state_index in 0..self.parse_table.state_count {
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

    /// Generate field maps
    fn generate_field_maps(&self) -> (Vec<TokenStream>, Vec<TokenStream>) {
        let production_id_count = self.calculate_counts().production_id_count as usize;
        let mut field_map_slices = vec![quote! { 0u16 }; production_id_count * 2];
        let mut field_map_entries = Vec::new();
        let field_name_indices = self.field_name_indices_by_field_id();

        // Group rules by production ID
        let mut rules_by_production: std::collections::BTreeMap<u16, Vec<&Rule>> =
            std::collections::BTreeMap::new();
        for (_, rules) in &self.grammar.rules {
            for rule in rules {
                rules_by_production
                    .entry(rule.production_id.0)
                    .or_default()
                    .push(rule);
            }
        }

        // Build field map entries for each production
        for (production_id, rules) in rules_by_production {
            let start_index = (field_map_entries.len() / 2) as u16;
            let mut entry_count = 0u16;

            // Process each rule with this production ID
            for rule in rules {
                // Add entries for each field in this rule
                for (field_id, position) in &rule.fields {
                    let field_id_val = field_name_indices
                        .get(&field_id.0)
                        .copied()
                        .unwrap_or(field_id.0);
                    let child_index = *position as u8;
                    let inherited = 0u8; // false - TODO: implement inheritance detection

                    // Pack TSFieldMapEntry: field_id (16 bits) | child_index (8 bits) | inherited (8 bits)
                    let packed_entry = (field_id_val as u32)
                        | ((child_index as u32) << 16)
                        | ((inherited as u32) << 24);
                    field_map_entries.push(quote! { #packed_entry as u16 });
                    field_map_entries.push(quote! { (#packed_entry >> 16) as u16 });
                    entry_count += 1;
                }
            }

            // Add slice for this production ID if it has fields
            if entry_count > 0 {
                let slice_offset = production_id as usize * 2;
                if slice_offset + 1 < field_map_slices.len() {
                    field_map_slices[slice_offset] = quote! { #start_index };
                    field_map_slices[slice_offset + 1] = quote! { #entry_count };
                }
            }
        }
        if field_map_entries.is_empty() {
            field_map_entries.push(quote! { 0u16 });
        }

        (field_map_slices, field_map_entries)
    }

    /// Generate public symbol map
    fn generate_public_symbol_map(&self) -> Vec<TokenStream> {
        let symbol_count = self.calculate_symbol_count();
        let mut index_to_symbol = vec![None; symbol_count];
        for (&symbol_id, &index) in &self.parse_table.symbol_to_index {
            if index < symbol_count {
                index_to_symbol[index] = Some(symbol_id);
            }
        }

        (0..symbol_count)
            .map(|index| {
                let public_symbol =
                    index_to_symbol[index].unwrap_or(SymbolId(index as u16)).0 as usize;
                quote! { #public_symbol as u16 }
            })
            .collect()
    }

    /// Generate primary state IDs
    fn generate_primary_state_ids(&self) -> Vec<TokenStream> {
        (0..self.parse_table.state_count)
            .map(|i| {
                quote! { #i as u16 }
            })
            .collect()
    }

    /// Generate variant to symbol ID mapping for Extract trait
    fn generate_variant_symbol_map(&self) -> TokenStream {
        // For now, just generate the complete symbol-to-index mapping
        // that the macro can use to fix enum variant extraction
        let mut symbol_entries = Vec::new();

        // Sort symbols by their index to ensure deterministic output
        let mut index_to_symbol: Vec<(usize, SymbolId)> = Vec::new();
        for (symbol_id, &index) in &self.parse_table.symbol_to_index {
            index_to_symbol.push((index, *symbol_id));
        }
        for (symbol_id, &index) in &self.parse_table.nonterminal_to_index {
            index_to_symbol.push((index, *symbol_id));
        }
        index_to_symbol.sort_by_key(|(idx, _)| *idx);

        // Generate entries for the mapping
        for (index, symbol_id) in &index_to_symbol {
            let symbol_id_val = symbol_id.0 as u32;
            let index_val = *index as u16;

            // Also include the symbol name for debugging
            let _symbol_name = if *symbol_id == self.parse_table.eof_symbol {
                "EOF".to_string()
            } else if let Some(token) = self.grammar.tokens.get(symbol_id) {
                token.name.clone()
            } else if let Some(rule_name) = self.grammar.rule_names.get(symbol_id) {
                rule_name.clone()
            } else {
                format!("symbol_{}", symbol_id.0)
            };

            symbol_entries.push(quote! {
                // #symbol_name
                (#symbol_id_val, #index_val)
            });
        }

        // Generate the inverse mapping array (index to symbol ID)
        let total_symbol_count = self.parse_table.symbol_count;
        let mut index_to_id_entries = vec![quote! { 0 }; total_symbol_count];

        for (index, symbol_id) in index_to_symbol {
            if index < total_symbol_count {
                let symbol_id_val = symbol_id.0;
                index_to_id_entries[index] = quote! { #symbol_id_val };
            }
        }

        quote! {
            // Complete symbol ID to parse table index mapping
            // This is used by the Extract trait to correctly identify symbols
            pub const SYMBOL_ID_TO_INDEX: &[(u32, u16)] = &[
                #(#symbol_entries),*
            ];

            // Inverse mapping: index to symbol ID
            // This is used by the pure parser to convert indices back to symbol IDs
            pub const SYMBOL_INDEX_TO_ID: &[u16] = &[
                #(#index_to_id_entries),*
            ];

            // Helper function to get symbol index from symbol ID
            #[allow(dead_code)]
            pub fn get_symbol_index(symbol_id: u32) -> Option<u16> {
                SYMBOL_ID_TO_INDEX.iter()
                    .find(|(id, _)| *id == symbol_id)
                    .map(|(_, index)| *index)
            }

            // Helper function to get symbol ID from symbol index
            #[allow(dead_code)]
            pub fn get_symbol_id(symbol_index: u16) -> u16 {
                SYMBOL_INDEX_TO_ID[symbol_index as usize]
            }
        }
    }

    /// Generate production ID map
    fn generate_production_id_map(&self) -> Vec<TokenStream> {
        // Tree-sitter uses 1-based production IDs in the parse table
        // After decoding to zero-based, runtime indexes this map by RULE ID from parse actions.
        // Therefore this map must be: rule_id -> production_id.
        // PARSE_ACTIONS / TS_RULES are indexed by production_id.
        let map_size = self.calculate_counts().production_id_count as usize;

        // Initialize map with a sentinel value (u16::MAX)
        let mut rule_to_production = vec![u16::MAX; map_size];

        // Fill the map in the same rule-id order used by GLR action generation.
        for (rule_id, rule) in self.grammar.all_rules().enumerate() {
            if rule_id < map_size {
                rule_to_production[rule_id] = rule.production_id.0;
            }
        }

        // Convert to TokenStreams
        let mut production_map = Vec::new();
        for val in rule_to_production {
            production_map.push(quote! { #val });
        }

        production_map
    }

    fn generate_production_lhs_index(&self) -> Vec<TokenStream> {
        // Generate a dense array of LHS symbols in table index space, indexed by
        // production ID. Runtime reductions map encoded rule IDs through
        // PRODUCTION_ID_MAP and then index this array by that production ID.
        let production_id_count = self.calculate_counts().production_id_count as usize;
        let mut lhs_indices = vec![quote! { 0u16 }; production_id_count];

        // Get all rules sorted by production ID
        let mut rules: Vec<_> = self
            .grammar
            .rules
            .iter()
            .flat_map(|(_, rules)| rules.iter())
            .collect();
        rules.sort_by_key(|rule| rule.production_id.0);

        // For each production, get its LHS symbol in table index space
        for rule in &rules {
            let lhs_idx = self
                .parse_table
                .symbol_to_index
                .get(&rule.lhs)
                .copied()
                .unwrap_or_else(|| {
                    panic!(
                        "LHS symbol {} not found in symbol_to_index for production {}",
                        rule.lhs.0, rule.production_id.0
                    );
                });

            // Guard rail: production LHS must be a non-terminal column
            debug_assert!(
                (lhs_idx as u32) >= self.parse_table.token_count as u32,
                "production LHS must be a non-terminal column (lhs_idx={}, token_count={})",
                lhs_idx,
                self.parse_table.token_count
            );

            let lhs_index = lhs_idx as u16;
            let production_index = rule.production_id.0 as usize;
            if production_index < lhs_indices.len() {
                lhs_indices[production_index] = quote! { #lhs_index };
            }
        }

        lhs_indices
    }

    fn generate_ts_rules(&self) -> Vec<TokenStream> {
        let production_id_count = self.calculate_counts().production_id_count as usize;
        if self.grammar.all_rules().next().is_none() {
            return Vec::new();
        }

        // Generate TSRule structs indexed by production ID.
        let mut ts_rules = vec![
            quote! {
                TSRule {
                    lhs: 0,
                    rhs_len: 0,
                    _pad: 0,
                }
            };
            production_id_count
        ];

        // Get all rules sorted by production ID
        let mut rules: Vec<_> = self
            .grammar
            .rules
            .iter()
            .flat_map(|(_, rules)| rules.iter())
            .collect();
        rules.sort_by_key(|rule| rule.production_id.0);

        // For each production, create a TSRule
        for rule in &rules {
            let production_index = rule.production_id.0 as usize;
            let symbol_id = rule.lhs;
            let lhs = self
                .parse_table
                .nonterminal_to_index
                .get(&symbol_id)
                .or_else(|| self.parse_table.symbol_to_index.get(&symbol_id))
                .copied()
                .unwrap_or_else(|| {
                    debug_trace!(
                        "WARNING: No symbol index found for LHS symbol ID {} in rule",
                        symbol_id.0
                    );
                    symbol_id.0 as usize
                }) as u16;
            let rhs_len = rule.rhs.len() as u8;
            if production_index < ts_rules.len() {
                ts_rules[production_index] = quote! {
                    TSRule {
                        lhs: #lhs,
                        rhs_len: #rhs_len,
                        _pad: 0,
                    }
                };
            }
        }

        ts_rules
    }

    fn generate_alias_tables(&self) -> (Vec<TokenStream>, Vec<TokenStream>) {
        let (_, max_alias_sequence_length) = self.calculate_alias_metrics();
        let production_count = self.calculate_production_count();
        let stride = max_alias_sequence_length as usize;
        if production_count == 0 || stride == 0 {
            return (Vec::new(), Vec::new());
        }

        let mut alias_map = Vec::with_capacity(production_count);
        let mut alias_sequences = vec![quote! { 0u16 }; production_count * stride];

        for production_index in 0..production_count {
            let offset = production_index * stride;
            alias_map.push(quote! { #offset as u16 });

            let production_id = ProductionId(production_index as u16);
            if let Some(sequence) = self.grammar.alias_sequences.get(&production_id) {
                for (position, alias) in sequence.aliases.iter().take(stride).enumerate() {
                    if let Some(alias_name) = alias.as_deref()
                        && let Some(symbol_index) = self.resolve_alias_symbol(alias_name)
                    {
                        alias_sequences[offset + position] = quote! { #symbol_index as u16 };
                    }
                }
            }
        }

        (alias_map, alias_sequences)
    }

    fn resolve_alias_symbol(&self, alias: &str) -> Option<u16> {
        let symbol_id = self
            .grammar
            .tokens
            .iter()
            .find_map(|(id, token)| (token.name == alias).then_some(*id))
            .or_else(|| {
                self.grammar
                    .rule_names
                    .iter()
                    .find_map(|(id, name)| (name == alias).then_some(*id))
            })?;

        self.parse_table
            .symbol_to_index
            .get(&symbol_id)
            .copied()
            .map(|index| index as u16)
            .or(Some(symbol_id.0))
    }

    /// Calculate counts for the language structure
    fn calculate_counts(&self) -> LanguageCounts {
        let (alias_count, max_alias_sequence_length) = self.calculate_alias_metrics();
        LanguageCounts {
            symbol_count: self.calculate_symbol_count() as u32,
            alias_count,
            // token_count comes from the parse table which knows about all terminals (including EOF)
            token_count: self.parse_table.token_count as u32,
            external_token_count: self.parse_table.external_token_count as u32,
            state_count: self.parse_table.state_count as u32,
            large_state_count: 0, // TODO: Calculate large states
            production_id_count: self.calculate_production_count() as u32,
            field_count: self.grammar.fields.len() as u32,
            max_alias_sequence_length,
        }
    }

    fn calculate_alias_metrics(&self) -> (u32, u16) {
        let mut aliases = HashSet::new();
        let mut max_len = self.grammar.max_alias_sequence_length;

        for sequence in self.grammar.alias_sequences.values() {
            max_len = max_len.max(sequence.aliases.len());
            for alias in sequence.aliases.iter().flatten() {
                aliases.insert(alias.as_str());
            }
        }

        (
            aliases.len() as u32,
            u16::try_from(max_len).unwrap_or(u16::MAX),
        )
    }

    fn calculate_symbol_count(&self) -> usize {
        // Use the parse table's symbol count which is the correct count after processing
        self.parse_table.symbol_count
    }

    fn calculate_production_count(&self) -> usize {
        let max_id = self
            .grammar
            .rules
            .values()
            .flat_map(|rules| rules.iter().map(|r| r.production_id.0))
            .max()
            .unwrap_or(0);
        (max_id as usize) + 1
    }

    /// Find all terminal tokens that should be marked as extras
    fn find_extra_tokens(&self) -> HashSet<SymbolId> {
        let mut extra_tokens = HashSet::new();
        let mut visited = HashSet::new();

        debug_trace!(
            "DEBUG find_extra_tokens: grammar.extras = {:?}",
            self.grammar.extras
        );

        // Check if any extras directly refer to tokens
        for &extra_symbol in &self.grammar.extras {
            if self.grammar.tokens.contains_key(&extra_symbol) {
                debug_trace!("  Extra symbol {:?} is directly a token!", extra_symbol);
                extra_tokens.insert(extra_symbol);
            }
        }

        // For each extra symbol, find all terminal tokens it can produce (recursively)
        for &extra_symbol in &self.grammar.extras {
            debug_trace!("  Processing extra symbol: {:?}", extra_symbol);
            self.find_terminals_recursive(extra_symbol, &mut extra_tokens, &mut visited);
        }

        debug_trace!("DEBUG find_extra_tokens: result = {:?}", extra_tokens);
        extra_tokens
    }

    /// Recursively find all terminal tokens reachable from a symbol
    fn find_terminals_recursive(
        &self,
        symbol: SymbolId,
        terminals: &mut HashSet<SymbolId>,
        visited: &mut HashSet<SymbolId>,
    ) {
        // Avoid infinite recursion
        if !visited.insert(symbol) {
            return;
        }

        // If it's a terminal token, add it
        if self.grammar.tokens.contains_key(&symbol) {
            debug_trace!("    Found terminal: {:?}", symbol);
            terminals.insert(symbol);
            return;
        }

        // If it's a non-terminal, explore all its rules
        if let Some(rules) = self.grammar.rules.get(&symbol) {
            debug_trace!(
                "    Exploring non-terminal {:?} with {} rules",
                symbol,
                rules.len()
            );
            for rule in rules {
                debug_trace!("      Rule: {:?} -> {:?}", rule.lhs, rule.rhs);
                for sym in &rule.rhs {
                    match sym {
                        Symbol::Terminal(token_id) => {
                            debug_trace!("        Found terminal in rule: {:?}", token_id);
                            terminals.insert(*token_id);
                        }
                        Symbol::NonTerminal(nt_id) => {
                            debug_trace!("        Recursing into non-terminal: {:?}", nt_id);
                            self.find_terminals_recursive(*nt_id, terminals, visited);
                        }
                        Symbol::External(_)
                        | Symbol::Optional(_)
                        | Symbol::Repeat(_)
                        | Symbol::RepeatOne(_)
                        | Symbol::Choice(_)
                        | Symbol::Sequence(_)
                        | Symbol::Epsilon => {
                            // These symbol types are not expected in the IR at this stage
                            debug_trace!(
                                "        WARNING: Unexpected symbol type in rule: {:?}",
                                sym
                            );
                        }
                    }
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use adze_glr_core::LexMode;
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

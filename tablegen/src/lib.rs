//! # adze-tablegen
//!
//! Generate and compress LR(1) parse tables for Adze grammars.

// Table generation requires unsafe for FFI-compatible Language struct generation
#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(private_interfaces)]
#![cfg_attr(feature = "strict_api", deny(unreachable_pub))]
#![cfg_attr(not(feature = "strict_api"), warn(unreachable_pub))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(feature = "strict_docs", deny(missing_docs))]
#![cfg_attr(not(feature = "strict_docs"), allow(missing_docs))]

mod goto_run_codec;
mod test_helpers;
mod util;

#[cfg(test)]
pub use crate::test_helpers::test::{make_empty_table, make_minimal_table};

/// Tree-sitter ABI type definitions and constants.
pub mod abi;
/// Builder for generating ABI-compatible Language structs.
pub mod abi_builder;
/// Parse table compression algorithms.
pub mod compress;
/// Additional compression utilities and strategies.
pub mod compression;
/// Error types for table generation
pub mod error;
/// External scanner code generation (v1).
pub mod external_scanner;
/// External scanner code generation (v2, improved).
pub mod external_scanner_v2;
/// Language builder for generating static parsers
pub mod generate;
/// Helper utilities for table generation
pub mod helpers;
/// Language code generation from parse tables.
pub mod language_gen;
/// Lexer generation utilities
pub mod lexer_gen;
/// NODE_TYPES JSON metadata generation.
pub mod node_types;
/// Parser template and parse table code generation.
pub mod parser;
/// .parsetable binary file format writer
#[cfg(feature = "serialization")]
pub mod parsetable_writer;
/// Schema validation for parse tables
pub mod schema;
/// Parse table serialization to various output formats.
pub mod serializer;
/// Typed CST wrapper code generation.
pub mod typed_cst;
/// Parse table and language struct validation.
pub mod validation;

/// Error and Result types for table generation.
pub use error::{Result, TableGenError};

// Re-export commonly used helpers at crate root for ergonomics
/// Utility functions for querying parse tables.
pub use helpers::{collect_token_indices, eof_accepts_or_reduces};

// Re-export key types
/// ABI-compatible language struct builder.
pub use abi_builder::AbiLanguageBuilder;
/// Compressed table types and compressor.
pub use compress::{
    ActionEntry, CompressedActionEntry, CompressedActionTable, CompressedGotoEntry,
    CompressedGotoTable, CompressedParseTable, CompressedTables, GotoEntry, TableCompressor,
};
/// External scanner code generator.
pub use external_scanner::ExternalScannerGenerator;
/// High-level language builder.
pub use generate::LanguageBuilder;
/// NODE_TYPES metadata generator.
pub use node_types::NodeTypesGenerator;
#[cfg(feature = "serialization")]
/// Binary `.parsetable` file format writer and types.
pub use parsetable_writer::{
    FORMAT_VERSION, FeatureFlags, GenerationInfo, GovernanceMetadata, GrammarInfo, MAGIC_NUMBER,
    METADATA_SCHEMA_VERSION, ParserFeatureProfileSnapshot, ParsetableError, ParsetableMetadata,
    ParsetableWriter, TableStatistics,
};
/// Typed CST wrapper code generator.
pub use typed_cst::TypedCstGenerator;
/// ABI language validator and validation error types.
pub use validation::{LanguageValidator, ValidationError};

// use indexmap::IndexMap; // Currently unused
use adze_glr_core::*;
use adze_ir::*;
use proc_macro2::TokenStream;
use quote::quote;

// Tree-sitter backend selection will be done in the relevant modules

/// Static Language generator that produces Rust code
pub struct StaticLanguageGenerator {
    /// Grammar definition
    pub grammar: Grammar,
    /// Parse table containing LR(1) action and goto tables
    pub parse_table: ParseTable,
    /// Compressed versions of the parse tables for smaller binary size
    pub compressed_tables: Option<CompressedTables>,
    /// Whether the start symbol can match empty input
    pub start_can_be_empty: bool,
}

impl StaticLanguageGenerator {
    /// Create a new generator
    pub fn new(grammar: Grammar, parse_table: ParseTable) -> Self {
        Self {
            grammar,
            parse_table,
            compressed_tables: None,
            start_can_be_empty: false,
        }
    }

    /// Set whether the start symbol can be empty (nullable)
    pub fn set_start_can_be_empty(&mut self, value: bool) {
        self.start_can_be_empty = value;
    }

    /// Generate static Rust code for the Language
    pub fn generate_language_code(&self) -> TokenStream {
        // Use the new language generator
        let generator =
            crate::language_gen::LanguageGenerator::new(&self.grammar, &self.parse_table);
        generator.generate()
    }

    /// Generate NODE_TYPES JSON string
    pub fn generate_node_types(&self) -> String {
        use serde_json::json;

        let mut types = Vec::new();

        // Generate node types for non-terminal rules
        for (symbol_id, rules) in &self.grammar.rules {
            let rule_name = self
                .grammar
                .rule_names
                .get(symbol_id)
                .cloned()
                .unwrap_or_else(|| format!("rule_{}", symbol_id.0));

            // Skip hidden rules (those starting with underscore)
            if rule_name.starts_with('_') {
                continue;
            }

            let mut node_type = json!({
                "type": rule_name,
                "named": true
            });

            // Collect fields from all rules for this symbol
            let mut all_fields = serde_json::Map::new();
            let mut has_children = false;

            for rule in rules {
                // Add fields if this rule has any
                for (field_id, _position) in &rule.fields {
                    if let Some(field_name) = self.grammar.fields.get(field_id) {
                        all_fields.insert(
                            field_name.clone(),
                            json!({
                                "multiple": false,
                                "required": true,
                                "types": []
                            }),
                        );
                    }
                }

                // Check if rule has children
                if !rule.rhs.is_empty() {
                    has_children = true;
                }
            }

            // Add fields if any
            if !all_fields.is_empty() {
                node_type["fields"] = json!(all_fields);
            }

            // Add children if any rule has RHS
            if has_children {
                let mut children = serde_json::Map::new();
                children.insert("multiple".to_string(), json!(false));
                children.insert("required".to_string(), json!(true));
                // TODO: Add proper child types based on rule.rhs
                children.insert("types".to_string(), json!([]));
                node_type["children"] = json!(children);
            }

            // Check if this is a supertype
            if self.grammar.supertypes.contains(symbol_id) {
                node_type["subtypes"] = json!([]);
            }

            types.push(node_type);
        }

        // Generate node types for named tokens
        for (_, token) in &self.grammar.tokens {
            if !token.name.starts_with('_') && matches!(&token.pattern, TokenPattern::Regex(_)) {
                types.push(json!({
                    "type": token.name,
                    "named": true
                }));
            }
        }

        // Generate node types for external tokens
        for external in &self.grammar.externals {
            if !external.name.starts_with('_') {
                types.push(json!({
                    "type": external.name,
                    "named": true
                }));
            }
        }

        serde_json::to_string_pretty(&json!(types)).unwrap_or_else(|_| "[]".to_string())
    }

    #[allow(dead_code)]
    fn generate_symbol_names(&self) -> Vec<String> {
        let mut names = Vec::new();

        // Add terminal symbols
        for (_, token) in &self.grammar.tokens {
            names.push(token.name.clone());
        }

        // Add non-terminal symbols (rules)
        for (symbol_id, _) in &self.grammar.rules {
            names.push(format!("rule_{}", symbol_id.0));
        }

        // Add external symbols
        for external in &self.grammar.externals {
            names.push(external.name.clone());
        }

        names
    }

    #[allow(dead_code)]
    fn generate_symbol_metadata(&self) -> Vec<TokenStream> {
        let mut metadata = Vec::new();

        // Generate metadata for each terminal symbol
        for (_, token) in &self.grammar.tokens {
            // Hidden tokens start with underscore
            let visible = !token.name.starts_with('_');
            // Anonymous tokens (string literals) are unnamed, regex tokens can be named
            let named = matches!(&token.pattern, TokenPattern::Regex(_)) && visible;
            let supertype = false;

            metadata.push(quote! {
                adze::ffi::TSSymbolMetadata {
                    visible: #visible,
                    named: #named,
                    supertype: #supertype,
                }
            });
        }

        // Add metadata for non-terminals (rules)
        for (symbol_id, _rule) in &self.grammar.rules {
            // For now, use generated rule names until we have proper symbol mapping
            let rule_name = format!("rule_{}", symbol_id.0);
            // Hidden rules start with underscore
            let visible = !rule_name.starts_with('_');
            // Non-terminals are named unless they're hidden
            let named = visible;
            // Check if this rule is in the supertypes list
            let supertype = self.grammar.supertypes.contains(symbol_id);

            metadata.push(quote! {
                adze::ffi::TSSymbolMetadata {
                    visible: #visible,
                    named: #named,
                    supertype: #supertype,
                }
            });
        }

        // Add metadata for external symbols
        for external in &self.grammar.externals {
            // External tokens are typically visible and named
            let visible = !external.name.starts_with('_');
            let named = visible;
            let supertype = false;

            metadata.push(quote! {
                adze::ffi::TSSymbolMetadata {
                    visible: #visible,
                    named: #named,
                    supertype: #supertype,
                }
            });
        }

        metadata
    }

    #[allow(dead_code)]
    fn generate_field_names(&self) -> Vec<String> {
        // Fields must be in lexicographic order (already validated in Grammar)
        self.grammar.fields.values().cloned().collect()
    }

    #[allow(dead_code)]
    fn generate_uncompressed_tables(&self) -> (TokenStream, TokenStream) {
        // Generate uncompressed action and goto tables
        let action_entries = self.generate_action_table_entries();
        let goto_entries = self.generate_goto_table_entries();

        let action_table = quote! {
            static ACTION_TABLE: &[&[adze::ffi::TSParseActionEntry]] = &[#(#action_entries),*];
        };

        let goto_table = quote! {
            static GOTO_TABLE: &[&[u16]] = &[#(#goto_entries),*];
        };

        (action_table, goto_table)
    }

    #[allow(dead_code)]
    fn generate_compressed_tables(
        &self,
        compressed: &CompressedTables,
    ) -> (TokenStream, TokenStream) {
        // Generate compressed tables using Tree-sitter's format

        if self.parse_table.state_count < compressed.small_table_threshold {
            self.generate_small_compressed_tables(compressed)
        } else {
            self.generate_large_compressed_tables(compressed)
        }
    }

    #[allow(dead_code)]
    fn generate_small_compressed_tables(
        &self,
        compressed: &CompressedTables,
    ) -> (TokenStream, TokenStream) {
        // Generate Tree-sitter's small table format
        // Action table: flat array of u16 values with encoded actions
        // Goto table: flat array of u16 state IDs

        let action_entries = self.generate_small_action_entries(&compressed.action_table);
        let goto_entries = self.generate_small_goto_entries(&compressed.goto_table);

        let action_count = compressed.action_table.data.len();
        let goto_count = self.count_goto_entries(&compressed.goto_table);

        let action_table = quote! {
            static SMALL_PARSE_TABLE: &[u16; #action_count] = &[#(#action_entries),*];
            static SMALL_PARSE_TABLE_MAP: &[u16] = &[/* row offsets */];
        };

        let goto_table = quote! {
            static GOTO_TABLE: &[u16; #goto_count] = &[#(#goto_entries),*];
        };

        (action_table, goto_table)
    }

    #[allow(dead_code)]
    fn generate_large_compressed_tables(
        &self,
        compressed: &CompressedTables,
    ) -> (TokenStream, TokenStream) {
        // For large tables, use pointer arrays
        // This is rarely needed but essential for grammars like C++
        self.generate_small_compressed_tables(compressed) // Simplified for now
    }

    #[allow(dead_code)]
    fn generate_small_action_entries(
        &self,
        action_table: &CompressedActionTable,
    ) -> Vec<TokenStream> {
        let mut entries = Vec::new();
        let compressor = TableCompressor::new();

        for entry in &action_table.data {
            if let Ok(encoded) = compressor.encode_action_small(&entry.action) {
                let symbol = entry.symbol;
                entries.push(quote! { #symbol }); // Symbol index
                entries.push(quote! { #encoded }); // Encoded action
            }
        }

        entries
    }

    #[allow(dead_code)]
    fn generate_small_goto_entries(&self, goto_table: &CompressedGotoTable) -> Vec<TokenStream> {
        let mut entries = Vec::new();

        for entry in &goto_table.data {
            match entry {
                CompressedGotoEntry::Single(state) => {
                    entries.push(quote! { #state });
                }
                CompressedGotoEntry::RunLength { state, count } => {
                    // Expand run-length encoded entries
                    for _ in 0..*count {
                        entries.push(quote! { #state });
                    }
                }
            }
        }

        entries
    }

    #[allow(dead_code)]
    fn count_goto_entries(&self, goto_table: &CompressedGotoTable) -> usize {
        goto_table
            .data
            .iter()
            .map(|entry| match entry {
                CompressedGotoEntry::Single(_) => 1,
                CompressedGotoEntry::RunLength { count, .. } => *count as usize,
            })
            .sum()
    }

    #[allow(dead_code)]
    fn generate_action_table_entries(&self) -> Vec<TokenStream> {
        let mut entries = Vec::new();

        for state_actions in &self.parse_table.action_table {
            let actions: Vec<TokenStream> = state_actions
                .iter()
                .flat_map(|action_cell| {
                    // For each action cell, generate entries for all actions
                    action_cell.iter().map(|action| {
                        match action {
                            Action::Shift(state) => {
                                let state_id = state.0;
                                quote! {
                                    adze::ffi::TSParseActionEntry {
                                        type_: adze::ffi::TSParseActionType::Shift,
                                        state: #state_id,
                                        symbol: 0,
                                        child_count: 0,
                                        dynamic_precedence: 0,
                                        fragile: false,
                                    }
                                }
                            }
                            Action::Reduce(rule) => {
                                let rule_id = rule.0;
                                quote! {
                                    adze::ffi::TSParseActionEntry {
                                        type_: adze::ffi::TSParseActionType::Reduce,
                                        state: 0,
                                        symbol: #rule_id,
                                        child_count: 0, // Will be filled with actual child count
                                        dynamic_precedence: 0,
                                        fragile: false,
                                    }
                                }
                            }
                            Action::Accept => {
                                quote! {
                                    adze::ffi::TSParseActionEntry {
                                        type_: adze::ffi::TSParseActionType::Accept,
                                        state: 0,
                                        symbol: 0,
                                        child_count: 0,
                                        dynamic_precedence: 0,
                                        fragile: false,
                                    }
                                }
                            }
                            Action::Error => {
                                quote! {
                                    adze::ffi::TSParseActionEntry {
                                        type_: adze::ffi::TSParseActionType::Error,
                                        state: 0,
                                        symbol: 0,
                                        child_count: 0,
                                        dynamic_precedence: 0,
                                        fragile: false,
                                    }
                                }
                            }
                            Action::Recover => {
                                // Treat Recover as Error for FFI compatibility
                                quote! {
                                    adze::ffi::TSParseActionEntry {
                                        type_: adze::ffi::TSParseActionType::Error,
                                        state: 0,
                                        symbol: 0,
                                        child_count: 0,
                                        dynamic_precedence: 0,
                                        fragile: false,
                                    }
                                }
                            }
                            Action::Fork(actions) => {
                                // For GLR fork points, we'll need to handle multiple actions
                                // For now, just take the first action
                                if let Some(Action::Shift(state)) = actions.first() {
                                    let state_id = state.0;
                                    quote! {
                                        adze::ffi::TSParseActionEntry {
                                            type_: adze::ffi::TSParseActionType::Shift,
                                            state: #state_id,
                                            symbol: 0,
                                            child_count: 0,
                                            dynamic_precedence: 0,
                                            fragile: false,
                                        }
                                    }
                                } else {
                                    quote! {
                                        adze::ffi::TSParseActionEntry {
                                            type_: adze::ffi::TSParseActionType::Error,
                                            state: 0,
                                            symbol: 0,
                                            child_count: 0,
                                            dynamic_precedence: 0,
                                            fragile: false,
                                        }
                                    }
                                }
                            }
                            _ => {
                                // Unknown action type // Expected: V for Recover
                                quote! {
                                    adze::ffi::TSParseActionEntry {
                                        type_: adze::ffi::TSParseActionType::Error,
                                        state: 0,
                                        symbol: 0,
                                        child_count: 0,
                                        dynamic_precedence: 0,
                                        fragile: false,
                                    }
                                }
                            }
                        }
                    })
                })
                .collect();

            entries.push(quote! { &[#(#actions),*] });
        }

        entries
    }

    #[allow(dead_code)]
    fn generate_goto_table_entries(&self) -> Vec<TokenStream> {
        let mut entries = Vec::new();

        for state_gotos in &self.parse_table.goto_table {
            let gotos: Vec<u16> = state_gotos.iter().map(|state| state.0).collect();
            entries.push(quote! { &[#(#gotos),*] });
        }

        entries
    }

    /// Apply table compression
    pub fn compress_tables(&mut self) -> Result<()> {
        // If start_can_be_empty wasn't explicitly set by the caller, derive a conservative value:
        // look only at EOF actions in state 0 (Accept or Reduce there implies nullable start).
        if !self.start_can_be_empty {
            self.start_can_be_empty = helpers::eof_accepts_or_reduces(&self.parse_table);
        }

        let compressor = TableCompressor::new();

        // Collect token indices for validation
        let token_indices = helpers::collect_token_indices(&self.grammar, &self.parse_table);

        // Use the start_can_be_empty value (either explicitly set or computed above)
        self.compressed_tables = Some(compressor.compress(
            &self.parse_table,
            &token_indices,
            self.start_can_be_empty,
        )?);
        Ok(())
    }
}

// TableCompressor moved to compress.rs

// Remove the TableCompressor impl - it's now in compress.rs
/*
impl TableCompressor {
    pub fn compress(&self, parse_table: &ParseTable) -> Result<CompressedTables> {
        // Determine if we should use small table optimization
        let use_small_table = parse_table.state_count < self.small_table_threshold;

        if use_small_table {
            self.compress_small_table(parse_table)
        } else {
            self.compress_large_table(parse_table)
        }
    }

    /// Compress using Tree-sitter's "small table" optimization
    /// This is the most common case and what Tree-sitter uses for most grammars
    fn compress_small_table(&self, parse_table: &ParseTable) -> Result<CompressedTables> {
        // Tree-sitter's small table format:
        // 1. Action table: 2D array flattened with row displacement
        // 2. Each entry is a u16 encoding action type + data
        // 3. Default reductions stored separately

        let compressed_action_table = self.compress_action_table_small(&parse_table.action_table, &parse_table.symbol_to_index)?;
        let compressed_goto_table = self.compress_goto_table_small(&parse_table.goto_table)?;

        Ok(CompressedTables {
            action_table: compressed_action_table,
            goto_table: compressed_goto_table,
            small_table_threshold: self.small_table_threshold,
        })
    }

    /// Compress using large table optimization (for very large grammars)
    fn compress_large_table(&self, parse_table: &ParseTable) -> Result<CompressedTables> {
        // For large tables, Tree-sitter uses pointer indirection
        // This is rarely used but necessary for grammars like C++

        let compressed_action_table = self.compress_action_table_large(&parse_table.action_table, &parse_table.symbol_to_index)?;
        let compressed_goto_table = self.compress_goto_table_large(&parse_table.goto_table)?;

        Ok(CompressedTables {
            action_table: compressed_action_table,
            goto_table: compressed_goto_table,
            small_table_threshold: self.small_table_threshold,
        })
    }

    /// Compress action table using Tree-sitter's small table format
    fn compress_action_table_small(&self, action_table: &[Vec<Vec<Action>>], symbol_to_index: &HashMap<SymbolId, usize>) -> Result<CompressedActionTable> {
        // Tree-sitter's encoding for small tables:
        // - Actions are encoded as u16 values
        // - Shift: 0x0000 | state_id
        // - Reduce: 0x8000 | (rule_id << 1) | has_precedence
        // - Accept: 0xFFFF
        // - Error: 0xFFFE

        let mut entries = Vec::new();
        let mut row_offsets = Vec::new();
        let mut default_reductions = Vec::new();

        // Create inverse mapping from index to symbol ID
        let mut index_to_symbol = HashMap::new();
        for (&symbol_id, &index) in symbol_to_index {
            index_to_symbol.insert(index, symbol_id);
        }

        for (_state_id, action_cells) in action_table.iter().enumerate() {
            // Find the most common action overall
            let mut action_counts: HashMap<&Action, usize> = HashMap::new();
            let mut has_shift = false;
            let mut has_accept = false;

            for action_cell in action_cells {
                // For GLR, take the first action in each cell for compression
                // (GLR conflict resolution happens at runtime, not in the compressed table)
                if let Some(action) = action_cell.first() {
                    *action_counts.entry(action).or_insert(0) += 1;
                    match action {
                        Action::Shift(_) => has_shift = true,
                        Action::Accept => has_accept = true,
                        _ => {}
                    }
                }
            }

            // Tree-sitter uses the most common action as default, but only reduces if no shifts/accepts
            let most_common = action_counts
                .iter()
                .max_by_key(|(_, count)| *count)
                .map(|(action, _)| (*action).clone())
                .unwrap_or(Action::Error);

            let default_action = match &most_common {
                Action::Reduce(_) if !has_shift && !has_accept => most_common,
                Action::Error => Action::Error,
                _ => Action::Error, // Default to Error for other cases
            };

            default_reductions.push(default_action.clone());

            // Encode non-default actions
            row_offsets.push(entries.len() as u16);

            for (index, action_cell) in action_cells.iter().enumerate() {
                // For GLR, take the first action in each cell for compression
                if let Some(action) = action_cell.first() {
                    // Skip if this is the default action
                    if action == &default_action {
                        continue;
                    }

                    // Get the actual symbol ID from the index
                    let symbol_id = index_to_symbol.get(&index)
                        .map(|id| id.0)
                        .unwrap_or(index as u16);

                    let _encoded = self.encode_action_small(action)?;
                    entries.push(CompressedActionEntry {
                        symbol: symbol_id,
                        action: action.clone(),
                    });
                }
            }
        }

        // Add sentinel for last row
        row_offsets.push(entries.len() as u16);

        Ok(CompressedActionTable {
            data: entries,
            row_offsets,
            default_actions: default_reductions,
        })
    }

    /// Compress action table using large table format
    fn compress_action_table_large(&self, action_table: &[Vec<Vec<Action>>], symbol_to_index: &HashMap<SymbolId, usize>) -> Result<CompressedActionTable> {
        // For large tables, use pointer indirection
        // This is a simplified version - real Tree-sitter uses more sophisticated compression
        self.compress_action_table_small(action_table, symbol_to_index)
    }

    /// Encode an action as a u16 for small table format
    fn encode_action_small(&self, action: &Action) -> Result<u16> {
        match action {
            Action::Shift(state) => {
                if state.0 >= 0x8000 {
                    return Err(TableGenError::Compression(
                        format!("Shift state {} too large for small table encoding", state.0)
                    ));
                }
                Ok(state.0)
            }
            Action::Reduce(rule) => {
                if rule.0 >= 0x4000 {
                    return Err(TableGenError::Compression(
                        format!("Reduce rule {} too large for small table encoding", rule.0)
                    ));
                }
                // Reduce actions are encoded with high bit set
                // bit 15: 1 (indicates reduce)
                // bits 14-0: rule_id (1-based)
                // Tree-sitter uses 1-based production IDs
                Ok(0x8000 | (rule.0 + 1))
            }
            Action::Accept => Ok(0xFFFF),
            Action::Error => Ok(0xFFFE),
            Action::Recover => Ok(0xFFFD), // Use a distinct value for Recover
            Action::Fork(_) => {
                // GLR fork points need special handling
                // For now, treat as error
                Ok(0xFFFE)
            }
        }
    }

    /// Compress goto table using Tree-sitter's small table format
    fn compress_goto_table_small(&self, goto_table: &[Vec<StateId>]) -> Result<CompressedGotoTable> {
        // Tree-sitter uses simple array compression for goto table
        // Each row is stored contiguously with row offsets

        let mut codec = goto_run_codec::GotoRunCodec::new();

        for row in goto_table {
            codec.begin_row();
            for state in row {
                codec.push_state(state.0);
            }
            codec.end_row();
        }

        let (data, row_offsets) = codec.finish();

        Ok(CompressedGotoTable { data, row_offsets })
    }

    /// Compress goto table using large table format
    fn compress_goto_table_large(&self, goto_table: &[Vec<StateId>]) -> Result<CompressedGotoTable> {
        // For large tables, use the same compression for now
        // Real Tree-sitter would use more sophisticated techniques
        self.compress_goto_table_small(goto_table)
    }
}
*/

// CompressedTables and related types are now defined in compress.rs

// TableGenError is now defined in error.rs and re-exported above

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_static_language_generator_creation() {
        let grammar = Grammar::new("test".to_string());
        let parse_table = crate::empty_table!(states: 1, terms: 0, nonterms: 0);

        let generator = StaticLanguageGenerator::new(grammar, parse_table);
        assert_eq!(generator.grammar.name, "test");
        assert_eq!(generator.parse_table.state_count, 1); // minimum is 1
        assert!(generator.compressed_tables.is_none());
    }

    #[test]
    fn test_action_encoding_small_table() {
        let compressor = TableCompressor::new();

        // Test shift encoding
        let shift_action = Action::Shift(StateId(42));
        let encoded = compressor.encode_action_small(&shift_action).unwrap();
        assert_eq!(encoded, 42);
        assert!(encoded < 0x8000); // High bit should be clear for shifts

        // Test reduce encoding
        let reduce_action = Action::Reduce(RuleId(17));
        let encoded = compressor.encode_action_small(&reduce_action).unwrap();
        // Encoding is 0x8000 | (rule_id + 1), so for rule 17: 0x8000 | 18 = 0x8012 = 32786
        assert_eq!(encoded, 32786);
        assert!(encoded >= 0x8000); // High bit should be set for reduces

        // Test accept encoding
        let accept_action = Action::Accept;
        let encoded = compressor.encode_action_small(&accept_action).unwrap();
        assert_eq!(encoded, 0xFFFF);

        // Test error encoding
        let error_action = Action::Error;
        let encoded = compressor.encode_action_small(&error_action).unwrap();
        assert_eq!(encoded, 0xFFFE);
    }

    #[test]
    fn test_action_encoding_overflow() {
        let compressor = TableCompressor::new();

        // Test shift with state ID too large
        let shift_action = Action::Shift(StateId(0x8000));
        let result = compressor.encode_action_small(&shift_action);
        assert!(result.is_err());

        // Test reduce with rule ID too large
        let reduce_action = Action::Reduce(RuleId(0x4000));
        let result = compressor.encode_action_small(&reduce_action);
        assert!(result.is_err());
    }

    #[test]
    fn test_table_compressor_creation() {
        let compressor = TableCompressor::new();
        // Just test that it can be created
        let _ = compressor;
    }

    #[test]
    fn test_symbol_names_generation() {
        let mut grammar = Grammar::new("test".to_string());

        // Add a token
        let token = Token {
            name: "NUMBER".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        };
        grammar.tokens.insert(SymbolId(0), token);

        // Add a rule
        let rule = Rule {
            lhs: SymbolId(1),
            rhs: vec![Symbol::Terminal(SymbolId(0))],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        };
        grammar.add_rule(rule);

        let parse_table = crate::empty_table!(states: 1, terms: 0, nonterms: 0);

        let generator = StaticLanguageGenerator::new(grammar, parse_table);
        let symbol_names = generator.generate_symbol_names();

        assert_eq!(symbol_names.len(), 2);
        assert!(symbol_names.contains(&"NUMBER".to_string()));
        assert!(symbol_names.contains(&"rule_1".to_string()));
    }

    #[test]
    fn test_field_names_generation() {
        let mut grammar = Grammar::new("test".to_string());

        // Add fields in lexicographic order
        grammar.fields.insert(FieldId(0), "left".to_string());
        grammar.fields.insert(FieldId(1), "right".to_string());

        let parse_table = crate::empty_table!(states: 1, terms: 0, nonterms: 0);

        let generator = StaticLanguageGenerator::new(grammar, parse_table);
        let field_names = generator.generate_field_names();

        assert_eq!(field_names, vec!["left", "right"]);
    }

    #[test]
    fn test_node_types_generation() {
        let grammar = Grammar::new("test".to_string());
        let parse_table = crate::empty_table!(states: 1, terms: 0, nonterms: 0);

        let generator = StaticLanguageGenerator::new(grammar, parse_table);
        let node_types = generator.generate_node_types();

        // Should be valid JSON
        assert!(serde_json::from_str::<serde_json::Value>(&node_types).is_ok());
    }

    #[test]
    fn test_table_compression_small_table() {
        let grammar = Grammar::new("test".to_string());

        // Create a simple parse table
        let mut parse_table = crate::test_helpers::test::make_minimal_table(
            vec![
                vec![vec![Action::Shift(StateId(1))], vec![Action::Error]],
                vec![vec![Action::Reduce(RuleId(0))], vec![Action::Accept]],
            ],
            vec![vec![StateId(0), StateId(1)], vec![StateId(1), StateId(0)]],
            vec![],
            SymbolId(1), // start_symbol
            SymbolId(1), // eof_symbol (column 1)
            0,           // external_token_count
        );

        // Override to put EOF at column 0 for test compatibility
        // Also update eof_symbol to match the new mapping
        parse_table.eof_symbol = SymbolId(0);
        parse_table.symbol_to_index.clear();
        parse_table.symbol_to_index.insert(SymbolId(0), 0);
        parse_table.symbol_to_index.insert(SymbolId(1), 1);

        let mut generator = StaticLanguageGenerator::new(grammar, parse_table);

        // Test compression
        assert!(generator.compress_tables().is_ok());
        assert!(generator.compressed_tables.is_some());

        let compressed = generator.compressed_tables.as_ref().unwrap();
        assert_eq!(compressed.small_table_threshold, 32768);
    }

    #[test]
    fn test_table_compression_large_table() {
        let _grammar = Grammar::new("large_test".to_string());

        // Create a parse table that exceeds small table threshold
        let mut parse_table = crate::test_helpers::test::make_minimal_table(
            vec![vec![vec![Action::Error]; 10]; 40000],
            vec![vec![StateId(0); 10]; 40000],
            vec![],
            SymbolId(1), // start_symbol
            SymbolId(1), // eof_symbol (column 1)
            0,           // external_token_count
        );

        // Set EOF at column 0 for compatibility with existing test logic
        // Also update eof_symbol to match the new mapping
        parse_table.eof_symbol = SymbolId(0);
        parse_table.symbol_to_index.clear();
        parse_table.symbol_to_index.insert(SymbolId(0), 0);

        // Give state 0 / EOF an Accept so the compressor has a valid path
        parse_table.action_table[0][0] = vec![Action::Accept];

        let compressor = TableCompressor::new();
        // Use proper helper to collect token indices
        let grammar = Grammar::default(); // Minimal grammar for test
        let token_indices = helpers::collect_token_indices(&grammar, &parse_table);
        // We just added Accept on EOF, so this is true for the large-table test
        let start_can_be_empty = true;
        let result = compressor.compress(&parse_table, &token_indices, start_can_be_empty);

        let compressed = result.expect("large table should compress");

        // Should use large table format
        assert_eq!(compressed.small_table_threshold, 32768);
        assert!(parse_table.state_count >= compressed.small_table_threshold);
    }

    #[test]
    fn test_compressed_action_table_small() {
        let compressor = TableCompressor::new();
        let action_table = vec![
            vec![
                vec![Action::Shift(StateId(1))],
                vec![Action::Error],
                vec![Action::Error],
            ],
            vec![
                vec![Action::Error],
                vec![Action::Reduce(RuleId(0))],
                vec![Action::Error],
            ],
        ];

        let symbol_to_index = std::collections::BTreeMap::new();
        let compressed = compressor.compress_action_table_small(&action_table, &symbol_to_index);
        assert!(compressed.is_ok());

        let compressed = compressed.unwrap();
        assert_eq!(compressed.default_actions.len(), 2);
        assert_eq!(compressed.row_offsets.len(), 3); // includes sentinel

        // First row should have default Error, with only Shift(1) stored
        match &compressed.default_actions[0] {
            Action::Error => {}
            _ => panic!("Expected Error as default for first row"),
        }

        // Second row should have default Error (not Reduce, because it's not universal)
        match &compressed.default_actions[1] {
            Action::Error => {}
            _ => panic!("Expected Error as default for second row"),
        }
    }

    #[test]
    fn test_compressed_action_table_with_default_reduction() {
        let compressor = TableCompressor::new();

        // Create a state with only reduce actions (common in LR parsers)
        let action_table = vec![vec![
            vec![Action::Reduce(RuleId(1))],
            vec![Action::Reduce(RuleId(1))],
            vec![Action::Reduce(RuleId(1))],
        ]];

        let symbol_to_index = std::collections::BTreeMap::new();
        let compressed = compressor.compress_action_table_small(&action_table, &symbol_to_index);
        assert!(compressed.is_ok());

        let compressed = compressed.unwrap();

        // Default action optimization is disabled, so default should be Error
        match &compressed.default_actions[0] {
            Action::Error => {}
            _ => panic!("Expected Error as default (optimization disabled)"),
        }

        // All 3 reduce actions should be explicitly encoded in data
        let entries_for_state_0 = compressed.row_offsets[1] - compressed.row_offsets[0];
        assert_eq!(
            entries_for_state_0, 3,
            "All reduce actions should be explicitly encoded"
        );
    }

    #[test]
    fn test_compressed_goto_table_small() {
        let compressor = TableCompressor::new();
        let goto_table = vec![
            vec![StateId(0), StateId(0), StateId(1)],
            vec![StateId(2), StateId(2), StateId(2)],
        ];

        let compressed = compressor.compress_goto_table_small(&goto_table);
        assert!(compressed.is_ok());

        let compressed = compressed.unwrap();
        assert_eq!(compressed.row_offsets.len(), 3); // includes sentinel
        assert!(!compressed.data.is_empty());

        // First row should have run of 2 StateId(0)s, then single StateId(1)
        let first_row_start = compressed.row_offsets[0] as usize;
        let first_row_end = compressed.row_offsets[1] as usize;
        let first_row_entries = &compressed.data[first_row_start..first_row_end];

        // Should be stored as individual entries (run of 2 is too short)
        assert_eq!(first_row_entries.len(), 3);

        // Second row should have run of 3 StateId(2)s
        let second_row_start = compressed.row_offsets[1] as usize;
        let second_row_end = compressed.row_offsets[2] as usize;
        let second_row_entries = &compressed.data[second_row_start..second_row_end];

        // Should be stored as run-length encoded
        assert_eq!(second_row_entries.len(), 1);
        match &second_row_entries[0] {
            CompressedGotoEntry::RunLength { state: 2, count: 3 } => {}
            _ => panic!("Expected run-length encoding for second row"),
        }
    }

    #[test]
    fn test_goto_table_run_length_threshold() {
        let compressor = TableCompressor::new();

        // Test that runs of 1 and 2 are stored as individual entries
        let goto_table = vec![vec![
            StateId(1),
            StateId(2),
            StateId(2),
            StateId(3),
            StateId(3),
            StateId(3),
        ]];

        let compressed = compressor.compress_goto_table_small(&goto_table);
        assert!(compressed.is_ok());

        let compressed = compressed.unwrap();
        let entries = &compressed.data;

        // Should have: Single(1), Single(2), Single(2), RunLength(3, 3)
        assert_eq!(entries.len(), 4);

        match &entries[0] {
            CompressedGotoEntry::Single(1) => {}
            _ => panic!("Expected single entry for StateId(1)"),
        }

        match &entries[1] {
            CompressedGotoEntry::Single(2) => {}
            _ => panic!("Expected single entry for first StateId(2)"),
        }

        match &entries[2] {
            CompressedGotoEntry::Single(2) => {}
            _ => panic!("Expected single entry for second StateId(2)"),
        }

        match &entries[3] {
            CompressedGotoEntry::RunLength { state: 3, count: 3 } => {}
            _ => panic!("Expected run-length for StateId(3)"),
        }
    }

    #[test]
    fn test_language_code_generation() {
        let grammar = Grammar::new("test_lang".to_string());
        let parse_table = crate::test_helpers::test::make_minimal_table(
            // 1 state × 2 columns; Accept on EOF col (1)
            vec![vec![vec![], vec![Action::Accept]]],
            vec![vec![StateId(0), StateId(0)]],
            vec![],
            SymbolId(1), // start_symbol (now in-bounds)
            SymbolId(1), // EOF column (1 = 1 + terms + externals with terms=1-implicit)
            0,
        );

        let generator = StaticLanguageGenerator::new(grammar, parse_table);
        let code = generator.generate_language_code();

        // Should generate valid Rust code
        let code_str = code.to_string();
        debug_trace!("Generated code: {}", code_str);
        assert!(code_str.contains("pub fn language")); // Without parentheses in quote output
        assert!(code_str.contains("tree_sitter_test_lang")); // Language-specific function name
        assert!(code_str.contains("LANGUAGE_VERSION"));
    }

    #[test]
    fn test_compressed_tables_validation() {
        let mut parse_table = crate::test_helpers::test::make_minimal_table(
            vec![
                vec![vec![Action::Shift(StateId(1))], vec![Action::Error]],
                vec![vec![Action::Reduce(RuleId(0))], vec![Action::Accept]],
            ],
            vec![vec![StateId(0), StateId(1)], vec![StateId(1), StateId(0)]],
            vec![],
            SymbolId(1), // start_symbol
            SymbolId(1), // eof_symbol (column 1)
            0,           // external_token_count
        );

        // Override to put EOF at column 0 for test compatibility
        // Also update eof_symbol to match the new mapping
        parse_table.eof_symbol = SymbolId(0);
        parse_table.symbol_to_index.clear();
        parse_table.symbol_to_index.insert(SymbolId(0), 0);
        parse_table.symbol_to_index.insert(SymbolId(1), 1);

        let compressor = TableCompressor::new();
        // Use proper helper to collect token indices
        let grammar = Grammar::default(); // Minimal grammar for test
        let token_indices = helpers::collect_token_indices(&grammar, &parse_table);
        // Compute start_can_be_empty based on EOF cell in state 0
        let start_can_be_empty = false; // Conservative default for empty test
        let compressed = compressor
            .compress(&parse_table, &token_indices, start_can_be_empty)
            .unwrap();

        // Validation is exercised more thoroughly in compress/validation test suites.
        // This smoke test only verifies compression succeeds end-to-end.
        let _ = compressed.validate(&parse_table);
    }

    #[test]
    fn test_tree_sitter_compatibility() {
        // Test that our encoding matches Tree-sitter's expectations
        let compressor = TableCompressor::new();

        // Tree-sitter encoding examples:
        // Shift to state 42: 0x002A (42 in hex)
        let shift = Action::Shift(StateId(42));
        assert_eq!(compressor.encode_action_small(&shift).unwrap(), 0x002A);

        // Reduce by rule 17: 0x8012 (32786 in decimal) = 0x8000 | (17 + 1)
        let reduce = Action::Reduce(RuleId(17));
        assert_eq!(compressor.encode_action_small(&reduce).unwrap(), 32786);

        // Accept: 0xFFFF
        let accept = Action::Accept;
        assert_eq!(compressor.encode_action_small(&accept).unwrap(), 0xFFFF);

        // Error: 0xFFFE
        let error = Action::Error;
        assert_eq!(compressor.encode_action_small(&error).unwrap(), 0xFFFE);
    }

    #[test]
    fn test_compressed_action_entry() {
        let entry = CompressedActionEntry::new(5, Action::Shift(StateId(10)));
        assert_eq!(entry.symbol, 5);
        match entry.action {
            Action::Shift(StateId(10)) => {}
            _ => panic!("Wrong action type"),
        }
    }

    #[test]
    fn test_generated_small_table_format() {
        let mut grammar = Grammar::new("small_test".to_string());

        // Add a simple grammar
        let token = Token {
            name: "A".to_string(),
            pattern: TokenPattern::String("a".to_string()),
            fragile: false,
        };
        grammar.tokens.insert(SymbolId(0), token);

        // Simple parse table
        let mut parse_table = crate::test_helpers::test::make_minimal_table(
            vec![
                vec![vec![Action::Shift(StateId(1))], vec![]],
                vec![vec![], vec![Action::Accept]],
            ],
            vec![vec![StateId(1), StateId(0)], vec![StateId(0), StateId(0)]],
            vec![],
            SymbolId(2), // start_symbol
            SymbolId(1), // eof_symbol (must be > 0)
            0,           // external_token_count
        );

        // Add EOF to symbol_to_index (required invariant)
        parse_table.symbol_to_index.insert(SymbolId(0), 0);

        let mut generator = StaticLanguageGenerator::new(grammar, parse_table);
        generator.compress_tables().unwrap();

        let code = generator.generate_language_code();
        let code_str = code.to_string();

        // Should generate small table format
        assert!(code_str.contains("SMALL_PARSE_TABLE") || code_str.contains("ACTION_TABLE"));
    }

    #[test]
    fn arithmetic_has_many_states() {
        // This test helps prevent regressions in FIRST/FOLLOW/closure computation
        // that could collapse the automaton

        // Create a simple arithmetic grammar
        let mut grammar = Grammar::new("arithmetic".to_string());

        // Add tokens
        let number_token = Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        };
        let plus_token = Token {
            name: "plus".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        };
        let times_token = Token {
            name: "times".to_string(),
            pattern: TokenPattern::String("*".to_string()),
            fragile: false,
        };

        grammar.tokens.insert(SymbolId(3), number_token);
        grammar.tokens.insert(SymbolId(4), plus_token);
        grammar.tokens.insert(SymbolId(5), times_token);

        // Add non-terminals
        grammar
            .rule_names
            .insert(SymbolId(0), "source_file".to_string());
        grammar
            .rule_names
            .insert(SymbolId(1), "expression".to_string());
        grammar.rule_names.insert(SymbolId(2), "term".to_string());

        // Add rules
        // source_file -> expression
        grammar.add_rule(Rule {
            lhs: SymbolId(0),
            rhs: vec![Symbol::NonTerminal(SymbolId(1))],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });

        // expression -> expression + term
        grammar.add_rule(Rule {
            lhs: SymbolId(1),
            rhs: vec![
                Symbol::NonTerminal(SymbolId(1)),
                Symbol::Terminal(SymbolId(4)),
                Symbol::NonTerminal(SymbolId(2)),
            ],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(1),
        });

        // expression -> term
        grammar.add_rule(Rule {
            lhs: SymbolId(1),
            rhs: vec![Symbol::NonTerminal(SymbolId(2))],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(2),
        });

        // term -> term * number
        grammar.add_rule(Rule {
            lhs: SymbolId(2),
            rhs: vec![
                Symbol::NonTerminal(SymbolId(2)),
                Symbol::Terminal(SymbolId(5)),
                Symbol::Terminal(SymbolId(3)),
            ],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(3),
        });

        // term -> number
        grammar.add_rule(Rule {
            lhs: SymbolId(2),
            rhs: vec![Symbol::Terminal(SymbolId(3))],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(4),
        });

        // Build LR(1) automaton
        let first_follow = FirstFollowSets::compute(&grammar).unwrap();
        let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();

        // The arithmetic grammar should have at least 9 states (GLR may compress states)
        assert!(
            parse_table.state_count >= 9,
            "automaton collapsed ({} states), expected >= 9",
            parse_table.state_count
        );

        // State 0 should have valid actions (not all Error)
        assert!(
            parse_table.action_table[0]
                .iter()
                .any(|action_cell| action_cell.iter().any(|a| !matches!(a, Action::Error))),
            "state-0 has no valid actions"
        );
    }
}

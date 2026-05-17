use crate::compress::{
    CompressedActionTable, CompressedGotoEntry, CompressedGotoTable, CompressedTables,
    TableCompressor,
};
use crate::error::Result;
use crate::helpers;
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

#[cfg(test)]
mod tests;

use super::AbiLanguageBuilder;
use adze_ir::SymbolId;
use proc_macro2::TokenStream;
use quote::quote;

impl<'a> AbiLanguageBuilder<'a> {
    /// Generate symbol names with deterministic ordering
    pub(super) fn generate_symbol_names(&self) -> (Vec<TokenStream>, Vec<TokenStream>) {
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
    /// Generate public symbol map
    pub(super) fn generate_public_symbol_map(&self) -> Vec<TokenStream> {
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
    pub(super) fn generate_primary_state_ids(&self) -> Vec<TokenStream> {
        (0..self.parse_table.state_count)
            .map(|i| {
                quote! { #i as u16 }
            })
            .collect()
    }
    /// Generate variant to symbol ID mapping for Extract trait
    pub(super) fn generate_variant_symbol_map(&self) -> TokenStream {
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
}

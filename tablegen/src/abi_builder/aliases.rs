use super::*;

impl<'a> AbiLanguageBuilder<'a> {
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

    pub(super) fn generate_alias_tables(&self) -> (Vec<TokenStream>, Vec<TokenStream>) {
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

    pub(super) fn resolve_alias_symbol(&self, alias: &str) -> Option<u16> {
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

    pub(super) fn calculate_alias_metrics(&self) -> (u32, u16) {
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
}

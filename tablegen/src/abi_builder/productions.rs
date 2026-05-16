use super::AbiLanguageBuilder;
use adze_ir::ProductionId;
use proc_macro2::TokenStream;
use quote::quote;

impl<'a> AbiLanguageBuilder<'a> {
    /// Generate production ID map
    pub(super) fn generate_production_id_map(&self) -> Vec<TokenStream> {
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
    pub(super) fn generate_production_lhs_index(&self) -> Vec<TokenStream> {
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
    pub(super) fn generate_ts_rules(&self) -> Vec<TokenStream> {
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
}

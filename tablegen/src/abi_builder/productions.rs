use super::AbiLanguageBuilder;
use adze_ir::ProductionId;
use proc_macro2::TokenStream;
use quote::quote;

impl<'a> AbiLanguageBuilder<'a> {
    /// Generate production ID map
    pub(super) fn generate_production_id_map(&self) -> Vec<TokenStream> {
        // Tree-sitter uses 1-based production IDs in the parse table
        // After decoding to zero-based, runtime indexes this map by RULE ID from parse actions.
        // Therefore this map must be: rule_id -> production_id, with exactly
        // one entry per RuleId in rule order (#1060). ProductionId is
        // shareable metadata: several rules may map to one slot and slots may
        // be sparse, so sizing by production count would pad or truncate.
        // TS_RULES uses the same RuleId order; field/alias metadata stays production-indexed.
        let map_size = self.grammar.all_rules().count();

        // Initialize map with a sentinel value (u16::MAX)
        let mut rule_to_production = vec![u16::MAX; map_size];

        // Fill the map in the same rule-id order used by GLR action generation.
        // `map_size` is the rule count, so every rule has its slot.
        for (rule_id, rule) in self.grammar.all_rules().enumerate() {
            rule_to_production[rule_id] = rule.production_id.0;
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
    pub(super) fn generate_ts_rules(&self) -> crate::Result<Vec<TokenStream>> {
        // Syntax facts cannot be stored in shareable metadata slots: two
        // rules sharing fields/aliases may have different LHS or RHS length.
        let mut ts_rules = Vec::with_capacity(self.grammar.all_rules().count());
        for rule in self.grammar.all_rules() {
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
            let rhs_len = self.checked_rhs_len(rule)?;
            ts_rules.push(quote! {
                TSRule {
                    lhs: #lhs,
                    rhs_len: #rhs_len,
                    _pad: 0,
                }
            });
        }

        Ok(ts_rules)
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

#[cfg(test)]
mod tests {
    use super::*;
    use adze_ir::{Grammar, ProductionId, Rule, SymbolId};

    fn map_value(token: &TokenStream) -> u16 {
        token.to_string().trim_end_matches("u16").parse().unwrap()
    }

    fn rule(lhs: SymbolId, production_id: ProductionId) -> Rule {
        Rule {
            lhs,
            rhs: vec![],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id,
        }
    }

    /// Encode-side half of the #1060 production-map sizing contract: when a
    /// shared-ProductionId grammar has MORE rules than production slots, every
    /// rule still gets exactly one map entry in rule order. Sizing by
    /// production count (max slot + 1 = 2 here) silently drops RuleId(2),
    /// which no decoder can recover.
    #[test]
    fn test_production_id_map_keeps_rules_beyond_production_slot_count() {
        let mut grammar = Grammar::new("more_rules_than_slots".to_string());
        let start = SymbolId(1);
        grammar.add_rule(rule(start, ProductionId(0)));
        grammar.add_rule(rule(start, ProductionId(1)));
        grammar.add_rule(rule(start, ProductionId(0)));

        let table = crate::empty_table!(states: 1, terms: 1, nonterms: 1);
        let builder = AbiLanguageBuilder::new(&grammar, &table);
        let map = builder.generate_production_id_map();
        let values: Vec<u16> = map.iter().map(map_value).collect();

        assert_eq!(
            values,
            vec![0, 1, 0],
            "one entry per RuleId in rule order, even when rule count exceeds production slot count"
        );
        assert_eq!(
            builder.generate_ts_rules().unwrap().len(),
            values.len(),
            "map length must match TS_RULES (rule) count for the decode-side rule-id index"
        );
    }
}

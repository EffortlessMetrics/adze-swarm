use super::*;

impl<'a> AbiLanguageBuilder<'a> {
    /// Calculate counts for the language structure
    pub(super) fn calculate_counts(&self) -> LanguageCounts {
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
}

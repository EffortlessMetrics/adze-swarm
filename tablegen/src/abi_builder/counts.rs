use super::{AbiLanguageBuilder, LanguageCounts};
use std::collections::HashSet;

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
    pub(super) fn calculate_symbol_count(&self) -> usize {
        // Use the parse table's symbol count which is the correct count after processing
        self.parse_table.symbol_count
    }
    pub(super) fn calculate_production_count(&self) -> usize {
        let max_id = self
            .grammar
            .rules
            .values()
            .flat_map(|rules| rules.iter().map(|r| r.production_id.0))
            .max()
            .unwrap_or(0);
        (max_id as usize) + 1
    }
}

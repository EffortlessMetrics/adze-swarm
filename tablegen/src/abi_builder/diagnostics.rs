#![cfg_attr(feature = "strict_docs", allow(missing_docs))]
//! Diagnostic tracing for `AbiLanguageBuilder::generate`.
//!
//! These helpers exist solely to keep verbose `debug_trace!` calls out of the
//! generation orchestrator. They have no effect on the produced TokenStream
//! and compile to nothing in release builds via the parent's `debug_trace!`
//! macro.

use super::{AbiLanguageBuilder, LanguageCounts};

impl<'a> AbiLanguageBuilder<'a> {
    /// Trace the language name and full `symbol_to_index` mapping.
    pub(super) fn log_generation_start(&self, language_name: &str) {
        debug_trace!(
            "DEBUG AbiLanguageBuilder: Generating language for '{}'",
            language_name
        );
        debug_trace!("DEBUG AbiLanguageBuilder: symbol_to_index mapping:");
        for (symbol_id, &index) in &self.parse_table.symbol_to_index {
            let symbol_name = self.get_symbol_name(*symbol_id);
            debug_trace!(
                "  SymbolId({}) -> index {} ('{}')",
                symbol_id.0,
                index,
                symbol_name
            );
        }
    }

    /// Trace the non-empty action cells for the initial parser state.
    pub(super) fn log_state0_actions(&self) {
        if self.parse_table.action_table.is_empty() {
            return;
        }
        debug_trace!("DEBUG AbiLanguageBuilder: State 0 actions:");
        for (symbol_idx, action_cell) in self.parse_table.action_table[0].iter().enumerate() {
            if action_cell.is_empty() {
                continue;
            }
            let symbol_id = self
                .parse_table
                .symbol_to_index
                .iter()
                .find(|(_, idx)| **idx == symbol_idx)
                .map(|(id, _)| *id);
            debug_trace!(
                "  Index {} (SymbolId {:?}): {:?}",
                symbol_idx,
                symbol_id,
                action_cell
            );
        }
    }

    /// Trace the token→index mapping consulted by lexer generation, plus the
    /// final token/symbol counts derived from the parse table.
    pub(super) fn log_lexer_token_mapping(&self, counts: &LanguageCounts) {
        debug_trace!("DEBUG: Symbol to index mapping for lexer generation:");
        for (sym_id, idx) in &self.parse_table.symbol_to_index {
            if let Some(token) = self.grammar.tokens.get(sym_id) {
                debug_trace!(
                    "  Token '{}' (SymbolId {:?}) -> index {}",
                    token.name,
                    sym_id,
                    idx
                );
            }
        }
        debug_trace!("DEBUG: token_count = {}", self.parse_table.token_count);
        debug_trace!("DEBUG: token_count = {}", counts.token_count);
        debug_trace!("DEBUG: symbol_count = {}", counts.symbol_count);
    }
}

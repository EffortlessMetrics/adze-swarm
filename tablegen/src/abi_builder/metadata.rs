use super::AbiLanguageBuilder;
use crate::abi::create_symbol_metadata;
use adze_ir::{Symbol, SymbolId, TokenPattern};
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashSet;

impl<'a> AbiLanguageBuilder<'a> {
    /// Generate symbol metadata
    pub(super) fn generate_symbol_metadata(&self) -> Vec<TokenStream> {
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
    /// Find all terminal tokens that should be marked as extras
    pub(super) fn find_extra_tokens(&self) -> HashSet<SymbolId> {
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
    pub(super) fn find_terminals_recursive(
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

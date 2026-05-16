use super::AbiLanguageBuilder;
use adze_glr_core::Action;
use proc_macro2::TokenStream;
use quote::quote;

impl<'a> AbiLanguageBuilder<'a> {
    /// Generate compressed parse tables
    pub(super) fn generate_parse_tables(&self) -> (Vec<TokenStream>, Vec<TokenStream>) {
        if let Some(compressed) = self.compressed_tables {
            // Generate compressed table data
            let mut table_data = Vec::new();
            let mut map_data = Vec::new();

            // Encode both action and goto table entries combined
            // Tree-sitter format: each state's row contains both actions (for terminals)
            // and gotos (for non-terminals) as (symbol, value) pairs

            for state_idx in 0..self.parse_table.state_count {
                // Record the starting offset for this state (in u16 array indices, not pairs)
                let current_offset = table_data.len();
                map_data.push(quote! { #current_offset as u32 });

                // Track which symbols already have action entries to avoid duplicates
                let mut action_symbols = std::collections::HashSet::new();

                // First, add action entries for this state
                let action_start = compressed.action_table.row_offsets[state_idx] as usize;
                let action_end = compressed.action_table.row_offsets[state_idx + 1] as usize;

                for entry in &compressed.action_table.data[action_start..action_end] {
                    let symbol = entry.symbol;
                    action_symbols.insert(symbol);
                    table_data.push(quote! { #symbol });
                    if let Ok(encoded) = self.encode_action(&entry.action) {
                        table_data.push(quote! { #encoded });
                    }
                }

                // Then, add goto entries for this state (encode as shifts for Tree-sitter compat)
                // Skip symbols that already have action entries to avoid duplicates
                if state_idx < self.parse_table.goto_table.len() {
                    for (symbol_idx, &goto_state) in
                        self.parse_table.goto_table[state_idx].iter().enumerate()
                    {
                        let symbol = symbol_idx as u16;
                        if goto_state.0 > 0 && !action_symbols.contains(&symbol) {
                            // This is a valid goto transition without a conflicting action
                            let encoded_shift = goto_state.0; // Shift actions are encoded as state_id
                            table_data.push(quote! { #symbol });
                            table_data.push(quote! { #encoded_shift });
                        }
                    }
                }
            }

            // Add final offset (end of table, in u16 array indices)
            let final_offset = table_data.len();
            map_data.push(quote! { #final_offset as u32 });

            (table_data, map_data)
        } else {
            // Fallback: generate compressed table format without proper compression
            // This stores only non-error entries as (symbol, action) pairs
            let mut table_data = Vec::new();
            let mut map_data = Vec::new();
            let mut current_offset = 0u32;

            debug_trace!(
                "DEBUG: goto_table.len() = {}, state_count = {}",
                self.parse_table.goto_table.len(),
                self.parse_table.state_count
            );

            for state_idx in 0..self.parse_table.state_count {
                // Record the starting offset for this state
                map_data.push(quote! { #current_offset });

                // We need to know the count before we start pushing
                let mut entries = Vec::new();

                let mut non_error_actions = Vec::new();

                for symbol_idx in 0..self.parse_table.symbol_count {
                    let symbol_id = self
                        .parse_table
                        .symbol_to_index
                        .iter()
                        .find(|&(_, &idx)| idx == symbol_idx)
                        .map(|(id, _)| *id);

                    let Some(symbol_id) = symbol_id else {
                        continue;
                    };

                    let is_terminal = self.grammar.tokens.contains_key(&symbol_id)
                        || self
                            .grammar
                            .externals
                            .iter()
                            .any(|e| e.symbol_id == symbol_id)
                        || symbol_id == self.parse_table.eof_symbol;

                    let mut record_action = |symbol_idx: usize, action: &Action| match action {
                        Action::Error => {}
                        _ => {
                            non_error_actions.push((symbol_idx, action.clone()));
                        }
                    };

                    if is_terminal {
                        if state_idx < self.parse_table.action_table.len()
                            && symbol_idx < self.parse_table.action_table[state_idx].len()
                        {
                            let actions = &self.parse_table.action_table[state_idx][symbol_idx];
                            for action in actions {
                                record_action(symbol_idx, action);
                            }
                        }
                    } else if state_idx < self.parse_table.goto_table.len()
                        && symbol_idx < self.parse_table.goto_table[state_idx].len()
                    {
                        let goto_state = self.parse_table.goto_table[state_idx][symbol_idx];
                        if goto_state.0 > 0 {
                            record_action(symbol_idx, &Action::Shift(goto_state));
                        }
                    }
                }

                for (symbol_idx, action) in non_error_actions {
                    if let Ok(encoded) = self.encode_action(&action) {
                        entries.push((symbol_idx as u16, encoded));
                    }
                }

                for (sym, val) in entries {
                    table_data.push(quote! { #sym });
                    table_data.push(quote! { #val });
                    current_offset += 2;
                }
            }

            // Add final offset for end of table
            debug_trace!("DEBUG: Final offset: {}", current_offset);
            map_data.push(quote! { #current_offset });

            (table_data, map_data)
        }
    }
    /// Encode an action as u16
    pub(super) fn encode_action(&self, action: &Action) -> Result<u16, String> {
        match action {
            Action::Shift(state) => Ok(state.0),
            Action::Reduce(rule) => {
                // Tree-sitter uses 1-based production IDs in reduce actions
                // The runtime will map through PRODUCTION_ID_MAP to get the actual index
                Ok(0x8000 | (rule.0 + 1))
            }
            Action::Accept => Ok(0xFFFF), // Use 0xFFFF for accept (must match decoder in pure_parser.rs)
            Action::Error => Ok(0),       // Use 0 for error to match parser expectation
            Action::Recover => Ok(0xFFFD), // Use distinct value for Recover
            Action::Fork(actions) => {
                // For Fork actions, we need to choose one action from the fork
                // For now, let's prefer reduce actions over shift actions
                // This is a simplified conflict resolution strategy

                // First, try to find a reduce action
                for action in actions {
                    if let Action::Reduce(_) = action {
                        return self.encode_action(action);
                    }
                }

                // If no reduce action, take the first non-error action
                for action in actions {
                    if !matches!(action, Action::Error) {
                        return self.encode_action(action);
                    }
                }

                // If all actions are errors (shouldn't happen), return error
                Ok(0)
            }
            _ => {
                // Unknown action type // Expected: V for Recover
                crate::util::unexpected_action(action, "encode_action");
                Ok(0)
            }
        }
    }
    /// Generate parse actions
    pub(super) fn generate_parse_actions(&self) -> Vec<TokenStream> {
        // Generate production information for reduce actions
        // The array must be indexed by production ID, not sequential

        // We need to size the array based on production_id_count
        let counts = self.calculate_counts();
        let production_id_count = counts.production_id_count as usize;

        // Create array with dummy entries (Shift to state 0, which is normally Error in state 0)
        let mut actions = vec![
            quote! {
                TSParseAction {
                    action_type: 3, // Error
                    extra: 0,
                    child_count: 0,
                    dynamic_precedence: 0,
                    symbol: 0,
                }
            };
            production_id_count
        ];

        // Fill in the actual productions at their correct indices
        // We MUST use the same rule ordering as generate_ts_rules
        let mut rules: Vec<_> = self
            .grammar
            .rules
            .iter()
            .flat_map(|(_, rules)| rules.iter())
            .collect();
        rules.sort_by_key(|rule| rule.production_id.0);

        for rule in rules {
            let index = rule.production_id.0 as usize;
            let child_count = rule.rhs.len() as u8;

            // Store the production ID because PARSE_ACTIONS is production-indexed.
            let symbol = rule.production_id.0;

            if index < actions.len() {
                actions[index] = quote! {
                    TSParseAction {
                        action_type: 1, // Reduce
                        extra: 0,
                        child_count: #child_count,
                        dynamic_precedence: 0,
                        symbol: #symbol,
                    }
                };
            }
        }

        actions
    }
    /// Generate lex modes
    pub(super) fn generate_lex_modes(&self) -> Vec<TokenStream> {
        let mut modes = Vec::new();

        for state_index in 0..self.parse_table.state_count {
            let mode = self
                .parse_table
                .lex_modes
                .get(state_index)
                .copied()
                .unwrap_or(adze_glr_core::LexMode {
                    lex_state: 0,
                    external_lex_state: 0,
                });
            let lex_state = mode.lex_state;
            let external_lex_state = mode.external_lex_state;
            modes.push(quote! {
                TSLexState {
                    lex_state: #lex_state,
                    external_lex_state: #external_lex_state,
                }
            });
        }

        modes
    }
}

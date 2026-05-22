#![cfg_attr(feature = "strict_docs", allow(missing_docs))]
//! Enhanced external scanner generator with state-based validity.

// Enhanced external scanner generator with state-based validity computation
use adze_glr_core::ParseTable;
use adze_ir::{ExternalToken, Grammar};
use quote::quote;

#[cfg(not(debug_assertions))]
macro_rules! debug_trace {
    ($($arg:tt)*) => {};
}

#[cfg(debug_assertions)]
macro_rules! debug_trace {
    ($($arg:tt)*) => {
        if std::env::var("RUST_LOG")
            .ok()
            .unwrap_or_default()
            .contains("debug")
        {
            eprintln!($($arg)*);
        }
    };
}

/// Enhanced external scanner generator that computes state-based validity
pub struct ExternalScannerGenerator {
    external_tokens: Vec<ExternalToken>,
    /// Parse table for computing valid external tokens
    parse_table: ParseTable,
}

impl ExternalScannerGenerator {
    pub fn new(grammar: Grammar, parse_table: ParseTable) -> Self {
        let external_tokens = grammar.externals.clone();

        Self {
            external_tokens,
            parse_table,
        }
    }

    /// Computes which external tokens are valid in each state
    pub fn compute_state_validity(&self) -> Vec<Vec<bool>> {
        self.normalized_state_validity()
    }

    /// Generates the external scanner state bitmap with computed validity
    pub fn generate_state_bitmap(&self) -> Vec<Vec<bool>> {
        self.normalized_state_validity()
    }

    /// Generates the symbol map array that maps external scanner indices to symbol IDs
    pub fn generate_symbol_map(&self) -> Vec<u16> {
        let mut map = vec![0u16; self.external_tokens.len()];

        for (token_index, token) in self.external_tokens.iter().enumerate() {
            let symbol_col = self
                .parse_table
                .symbol_to_index
                .get(&token.symbol_id)
                .copied()
                .unwrap_or(token.symbol_id.0 as usize);
            map[token_index] = symbol_col as u16;
        }

        map
    }

    /// Generates the external scanner FFI interface code
    pub fn generate_scanner_interface(&self) -> proc_macro2::TokenStream {
        if self.external_tokens.is_empty() {
            return quote! {};
        }

        // Generate external scanner state data with computed validity
        let state_bitmap = self.generate_state_bitmap();
        let mut state_data = Vec::new();

        for state in &state_bitmap {
            for &valid in state {
                state_data.push(valid);
            }
        }

        // Generate symbol map
        let symbol_map = self.generate_symbol_map();

        // Generate external token count and state count constants
        let external_count = self.external_tokens.len();
        let state_count = state_bitmap.len();

        quote! {
            // External scanner constants
            const EXTERNAL_TOKEN_COUNT: usize = #external_count;
            const STATE_COUNT: usize = #state_count;

            // External scanner state bitmap (computed from parse table)
            static EXTERNAL_SCANNER_STATES: &[bool] = &[#(#state_data),*];

            // External scanner symbol map
            static EXTERNAL_SCANNER_SYMBOL_MAP: &[u16] = &[#(#symbol_map),*];

            // External scanner data
            #[allow(dead_code)]
            static EXTERNAL_SCANNER_DATA: adze::ffi::TSExternalScannerData = adze::ffi::TSExternalScannerData {
                states: EXTERNAL_SCANNER_STATES.as_ptr(),
                symbol_map: EXTERNAL_SCANNER_SYMBOL_MAP.as_ptr(),
                create: None, // TODO: Link to user scanner
                destroy: None,
                scan: None,
                serialize: None,
                deserialize: None,
            };

            // Helper function to get valid external tokens for a state
            #[allow(dead_code)]
            fn get_valid_external_tokens(state: usize) -> Vec<bool> {
                if state >= STATE_COUNT {
                    return vec![false; EXTERNAL_TOKEN_COUNT];
                }

                let start = state * EXTERNAL_TOKEN_COUNT;
                let end = start + EXTERNAL_TOKEN_COUNT;
                EXTERNAL_SCANNER_STATES[start..end].to_vec()
            }
        }
    }

    /// Returns whether the grammar has external tokens
    pub fn has_external_tokens(&self) -> bool {
        !self.external_tokens.is_empty()
    }

    /// Returns the number of external tokens
    pub fn external_token_count(&self) -> usize {
        self.external_tokens.len()
    }

    /// Normalizes parse-table-provided external scanner states to ensure each
    /// row width matches the grammar's external token count.
    fn normalized_state_validity(&self) -> Vec<Vec<bool>> {
        let expected_width = self.external_tokens.len();
        self.parse_table
            .external_scanner_states
            .iter()
            .map(|row| {
                if row.len() == expected_width {
                    return row.clone();
                }

                let mut normalized = row.clone();
                normalized.resize(expected_width, false);
                normalized.truncate(expected_width);
                normalized
            })
            .collect()
    }

    /// Debug helper: print validity matrix
    pub fn debug_print_validity(&self) {
        let state_bitmap = self.compute_state_validity();

        debug_trace!("External Token Validity Matrix:");
        debug_trace!("States x External Tokens");

        // Print header with external token names
        let mut header = String::from("State |");
        for token in &self.external_tokens {
            header.push_str(&format!(" {} |", token.name));
        }
        debug_trace!("{}", header);

        // Print validity for each state
        for (state_idx, state_validity) in state_bitmap.iter().enumerate() {
            let mut row = format!("{:5} |", state_idx);
            for &valid in state_validity {
                row.push_str(&format!(" {:5} |", if valid { "✓" } else { " " }));
            }
            debug_trace!("{}", row);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adze_glr_core::{Action, FirstFollowSets, build_lr1_automaton};
    use adze_ir::{ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

    #[test]
    fn test_state_validity_computation() {
        let mut grammar = Grammar::new("test".to_string());

        // Add external tokens
        grammar.externals.push(ExternalToken {
            name: "INDENT".to_string(),
            symbol_id: SymbolId(100),
        });
        grammar.externals.push(ExternalToken {
            name: "DEDENT".to_string(),
            symbol_id: SymbolId(101),
        });

        // Create a simple parse table
        let mut parse_table = crate::test_helpers::test::make_minimal_table(
            vec![vec![vec![Action::Error]; 2]; 2], // 2 states, 2 symbols
            vec![vec![crate::test_helpers::test::INVALID; 2]; 2],
            vec![],
            adze_ir::SymbolId(1), // start_symbol
            adze_ir::SymbolId(1), // eof_symbol
            0,                    // external_token_count
        );
        parse_table.external_scanner_states = vec![
            vec![true, false], // State 0: INDENT is valid
            vec![false, true], // State 1: DEDENT is valid
        ];

        // Map external symbols to indices
        parse_table.symbol_to_index.insert(SymbolId(100), 0); // INDENT
        parse_table.symbol_to_index.insert(SymbolId(101), 1); // DEDENT

        // State 0: INDENT is valid (shift to state 1)
        parse_table.action_table[0][0] = vec![Action::Shift(adze_ir::StateId(1))];

        // State 1: DEDENT is valid (shift to state 2)
        parse_table.action_table[1][1] = vec![Action::Shift(adze_ir::StateId(2))];

        let generator = ExternalScannerGenerator::new(grammar, parse_table);
        let validity = generator.compute_state_validity();

        // Check state 0: only INDENT should be valid
        assert_eq!(validity[0], vec![true, false]);

        // Check state 1: only DEDENT should be valid
        assert_eq!(validity[1], vec![false, true]);
    }

    #[test]
    fn test_symbol_map_generation() {
        let mut grammar = Grammar::new("test".to_string());

        // Add a minimal token and rule to make the grammar valid
        let start_symbol = SymbolId(1);
        let dummy_token_id = SymbolId(2);

        // Add token to grammar
        grammar.tokens.insert(
            dummy_token_id,
            Token {
                name: "dummy".to_string(),
                pattern: TokenPattern::String("dummy".to_string()),
                fragile: false,
            },
        );

        // Add rule that uses the token
        grammar.rules.insert(
            start_symbol,
            vec![Rule {
                lhs: start_symbol,
                rhs: vec![Symbol::Terminal(dummy_token_id)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(0),
            }],
        );

        grammar.externals.push(ExternalToken {
            name: "TOKEN1".to_string(),
            symbol_id: SymbolId(200),
        });
        grammar.externals.push(ExternalToken {
            name: "TOKEN2".to_string(),
            symbol_id: SymbolId(201),
        });

        let first_follow = FirstFollowSets::compute(&grammar).unwrap();
        let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();
        let expected_symbol_map = grammar
            .externals
            .iter()
            .map(|token| {
                parse_table
                    .symbol_to_index
                    .get(&token.symbol_id)
                    .copied()
                    .unwrap_or(token.symbol_id.0 as usize) as u16
            })
            .collect::<Vec<_>>();
        let generator = ExternalScannerGenerator::new(grammar, parse_table);

        let symbol_map = generator.generate_symbol_map();
        assert_eq!(symbol_map, expected_symbol_map);
    }

    #[test]
    fn test_symbol_map_generation_uses_table_columns_for_sparse_external_ids() {
        let mut grammar = Grammar::new("test".to_string());
        grammar.externals.push(ExternalToken {
            name: "INDENT".to_string(),
            symbol_id: SymbolId(200),
        });
        grammar.externals.push(ExternalToken {
            name: "DEDENT".to_string(),
            symbol_id: SymbolId(201),
        });

        let mut parse_table = crate::test_helpers::test::make_minimal_table(
            vec![vec![vec![Action::Error]; 5]; 1],
            vec![vec![crate::test_helpers::test::INVALID; 5]; 1],
            vec![],
            SymbolId(4),
            SymbolId(3),
            2,
        );
        parse_table.symbol_to_index.insert(SymbolId(200), 1);
        parse_table.symbol_to_index.insert(SymbolId(201), 2);
        parse_table.index_to_symbol = vec![
            SymbolId(0),
            SymbolId(200),
            SymbolId(201),
            SymbolId(3),
            SymbolId(4),
        ];
        parse_table.symbol_count = 5;

        let generator = ExternalScannerGenerator::new(grammar, parse_table);

        assert_eq!(generator.generate_symbol_map(), vec![1, 2]);
    }

    #[test]
    fn test_state_validity_pads_short_rows() {
        let mut grammar = Grammar::new("test".to_string());
        grammar.externals.push(ExternalToken {
            name: "A".to_string(),
            symbol_id: SymbolId(1),
        });
        grammar.externals.push(ExternalToken {
            name: "B".to_string(),
            symbol_id: SymbolId(2),
        });
        grammar.externals.push(ExternalToken {
            name: "C".to_string(),
            symbol_id: SymbolId(3),
        });

        let mut parse_table = crate::test_helpers::test::make_minimal_table(
            vec![vec![vec![Action::Error]; 1]; 1],
            vec![vec![crate::test_helpers::test::INVALID; 1]; 1],
            vec![],
            SymbolId(1),
            SymbolId(1),
            0,
        );
        parse_table.external_scanner_states = vec![vec![true]];

        let generator = ExternalScannerGenerator::new(grammar, parse_table);
        assert_eq!(
            generator.generate_state_bitmap(),
            vec![vec![true, false, false]]
        );
    }

    #[test]
    fn test_state_validity_truncates_wide_rows() {
        let mut grammar = Grammar::new("test".to_string());
        grammar.externals.push(ExternalToken {
            name: "A".to_string(),
            symbol_id: SymbolId(1),
        });
        grammar.externals.push(ExternalToken {
            name: "B".to_string(),
            symbol_id: SymbolId(2),
        });

        let mut parse_table = crate::test_helpers::test::make_minimal_table(
            vec![vec![vec![Action::Error]; 1]; 1],
            vec![vec![crate::test_helpers::test::INVALID; 1]; 1],
            vec![],
            SymbolId(1),
            SymbolId(1),
            0,
        );
        parse_table.external_scanner_states = vec![vec![true, false, true, true]];

        let generator = ExternalScannerGenerator::new(grammar, parse_table);
        assert_eq!(generator.compute_state_validity(), vec![vec![true, false]]);
    }
}

#![cfg_attr(feature = "strict_docs", allow(missing_docs))]
//! Alternative table compression algorithms for action tables.

// Table compression algorithms for parse tables
use adze_glr_core::Action;
use adze_ir::StateId;
use std::collections::HashMap;

/// Compressed representation of action table
pub struct CompressedActionTable {
    // Row compression: map identical rows to a single index
    #[allow(dead_code)]
    row_map: HashMap<Vec<Vec<Action>>, usize>,
    /// Deduplicated rows of the action table.
    pub unique_rows: Vec<Vec<Vec<Action>>>,
    /// Mapping from state index to deduplicated row index.
    pub state_to_row: Vec<usize>,
}

/// Compressed representation of goto table
pub struct CompressedGotoTable {
    /// Sparse representation: only store non-None entries.
    pub entries: HashMap<(usize, usize), StateId>,
    #[allow(dead_code)]
    state_count: usize,
    #[allow(dead_code)]
    symbol_count: usize,
}

/// Compress action table using row deduplication
pub fn compress_action_table(table: &[Vec<Vec<Action>>]) -> CompressedActionTable {
    let mut row_map = HashMap::new();
    let mut unique_rows = Vec::new();
    let mut state_to_row = Vec::new();

    for row in table {
        let row_index = if let Some(&idx) = row_map.get(row) {
            idx
        } else {
            let idx = unique_rows.len();
            row_map.insert(row.clone(), idx);
            unique_rows.push(row.clone());
            idx
        };
        state_to_row.push(row_index);
    }

    CompressedActionTable {
        row_map,
        unique_rows,
        state_to_row,
    }
}

/// Decompress a single action from compressed table
/// For GLR tables with multiple actions per cell, returns the first action
pub fn decompress_action(
    compressed: &CompressedActionTable,
    state: usize,
    symbol: usize,
) -> Action {
    let row_index = compressed.state_to_row[state];
    let action_cell = &compressed.unique_rows[row_index][symbol];
    // For GLR, return the first action in the cell, or Error if empty
    action_cell.first().cloned().unwrap_or(Action::Error)
}

/// Compress goto table using sparse representation
pub fn compress_goto_table(table: &[Vec<Option<StateId>>]) -> CompressedGotoTable {
    let mut entries = HashMap::new();
    let state_count = table.len();
    let symbol_count = if state_count > 0 { table[0].len() } else { 0 };

    for (state_idx, row) in table.iter().enumerate() {
        for (symbol_idx, &goto) in row.iter().enumerate() {
            if let Some(target) = goto {
                entries.insert((state_idx, symbol_idx), target);
            }
        }
    }

    CompressedGotoTable {
        entries,
        state_count,
        symbol_count,
    }
}

/// Decompress a single goto from compressed table
pub fn decompress_goto(
    compressed: &CompressedGotoTable,
    state: usize,
    symbol: usize,
) -> Option<StateId> {
    compressed.entries.get(&(state, symbol)).copied()
}

/// Advanced compression using bit-packing for common patterns
pub struct BitPackedActionTable {
    // Pack common actions into fewer bits
    error_mask: Vec<u64>,  // 1 bit per cell for Error actions
    shift_data: Vec<u32>,  // State IDs for shift actions
    reduce_data: Vec<u32>, // Rule IDs for reduce actions
    fork_data: HashMap<(usize, usize), Vec<Action>>, // Full data for fork actions

    #[allow(dead_code)]
    state_count: usize,
    symbol_count: usize,
}

impl BitPackedActionTable {
    pub fn from_table(table: &[Vec<Action>]) -> Self {
        let state_count = table.len();
        let symbol_count = if state_count > 0 { table[0].len() } else { 0 };

        // Calculate bits needed
        let total_cells = state_count * symbol_count;
        let mask_words = total_cells.div_ceil(64);

        let mut error_mask = vec![0u64; mask_words];
        let mut shift_data = Vec::new();
        let mut reduce_data = Vec::new();
        let mut fork_data = HashMap::new();

        for (state_idx, row) in table.iter().enumerate() {
            for (symbol_idx, action) in row.iter().enumerate() {
                let cell_idx = state_idx * symbol_count + symbol_idx;

                match action {
                    Action::Error => {
                        // Set bit in error mask
                        let word_idx = cell_idx / 64;
                        let bit_idx = cell_idx % 64;
                        error_mask[word_idx] |= 1 << bit_idx;
                    }
                    Action::Shift(state) => {
                        shift_data.push(state.0 as u32);
                    }
                    Action::Reduce(rule) => {
                        reduce_data.push(rule.0 as u32);
                    }
                    Action::Accept => {
                        // Accept is rare, can be stored as special reduce
                        reduce_data.push(u32::MAX);
                    }
                    Action::Recover => {
                        // Treat Recover as error for now
                        let word_idx = cell_idx / 64;
                        let bit_idx = cell_idx % 64;
                        error_mask[word_idx] |= 1 << bit_idx;
                    }
                    Action::Fork(actions) => {
                        fork_data.insert((state_idx, symbol_idx), actions.clone());
                    }
                    _ => {
                        // Unknown action type // Expected: V for Recover
                        let word_idx = cell_idx / 64;
                        let bit_idx = cell_idx % 64;
                        error_mask[word_idx] |= 1 << bit_idx;
                    }
                }
            }
        }

        BitPackedActionTable {
            error_mask,
            shift_data,
            reduce_data,
            fork_data,
            state_count,
            symbol_count,
        }
    }

    pub fn decompress(&self, state: usize, symbol: usize) -> Action {
        let cell_idx = state * self.symbol_count + symbol;
        let word_idx = cell_idx / 64;
        let bit_idx = cell_idx % 64;

        // Check error mask first
        if (self.error_mask[word_idx] >> bit_idx) & 1 == 1 {
            return Action::Error;
        }

        // Check for fork action
        if let Some(actions) = self.fork_data.get(&(state, symbol)) {
            return Action::Fork(actions.clone());
        }

        // Count non-error actions before this cell to find data index
        let mut data_idx = 0;
        for i in 0..cell_idx {
            let w_idx = i / 64;
            let b_idx = i % 64;
            if (self.error_mask[w_idx] >> b_idx) & 1 == 0 {
                // Check if it's not a fork action
                let s_idx = i / self.symbol_count;
                let sym_idx = i % self.symbol_count;
                if !self.fork_data.contains_key(&(s_idx, sym_idx)) {
                    data_idx += 1;
                }
            }
        }

        // Determine if this is shift or reduce based on position
        // This is a simplified heuristic - real implementation would need more metadata
        if data_idx < self.shift_data.len() {
            Action::Shift(StateId(self.shift_data[data_idx] as u16))
        } else {
            let reduce_idx = data_idx - self.shift_data.len();
            if reduce_idx < self.reduce_data.len() {
                let rule_id = self.reduce_data[reduce_idx];
                if rule_id == u32::MAX {
                    Action::Accept
                } else {
                    Action::Reduce(adze_ir::RuleId(rule_id as u16))
                }
            } else {
                Action::Error // Fallback
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_row_deduplication() {
        // Create a table with duplicate rows
        let table = vec![
            vec![vec![Action::Error], vec![Action::Shift(StateId(1))]],
            vec![vec![Action::Error], vec![Action::Shift(StateId(1))]], // Duplicate
            vec![
                vec![Action::Reduce(adze_ir::RuleId(0))],
                vec![Action::Error],
            ],
        ];

        let compressed = compress_action_table(&table);

        // Should have only 2 unique rows
        assert_eq!(compressed.unique_rows.len(), 2);

        // Verify decompression
        assert_eq!(decompress_action(&compressed, 0, 0), Action::Error);
        assert_eq!(
            decompress_action(&compressed, 0, 1),
            Action::Shift(StateId(1))
        );
        assert_eq!(decompress_action(&compressed, 1, 0), Action::Error);
        assert_eq!(
            decompress_action(&compressed, 1, 1),
            Action::Shift(StateId(1))
        );
        assert_eq!(
            decompress_action(&compressed, 2, 0),
            Action::Reduce(adze_ir::RuleId(0))
        );
    }

    #[test]
    fn test_sparse_goto_compression() {
        // Create a sparse goto table
        let table = vec![
            vec![None, Some(StateId(1)), None],
            vec![Some(StateId(2)), None, None],
            vec![None, None, Some(StateId(3))],
        ];

        let compressed = compress_goto_table(&table);

        // Should have only 3 entries
        assert_eq!(compressed.entries.len(), 3);

        // Verify decompression
        assert_eq!(decompress_goto(&compressed, 0, 0), None);
        assert_eq!(decompress_goto(&compressed, 0, 1), Some(StateId(1)));
        assert_eq!(decompress_goto(&compressed, 1, 0), Some(StateId(2)));
        assert_eq!(decompress_goto(&compressed, 2, 2), Some(StateId(3)));
    }

    #[test]
    fn compress_action_table_empty_input_yields_empty_output() {
        let table: Vec<Vec<Vec<Action>>> = Vec::new();
        let compressed = compress_action_table(&table);
        assert!(compressed.unique_rows.is_empty());
        assert!(compressed.state_to_row.is_empty());
    }

    #[test]
    fn compress_action_table_all_distinct_rows_preserved() {
        let table = vec![
            vec![vec![Action::Shift(StateId(1))]],
            vec![vec![Action::Reduce(adze_ir::RuleId(0))]],
            vec![vec![Action::Accept]],
        ];
        let compressed = compress_action_table(&table);
        assert_eq!(compressed.unique_rows.len(), 3);
        assert_eq!(compressed.state_to_row, vec![0, 1, 2]);
    }

    #[test]
    fn decompress_action_returns_error_for_empty_cell() {
        let table = vec![vec![Vec::<Action>::new()]];
        let compressed = compress_action_table(&table);
        assert_eq!(decompress_action(&compressed, 0, 0), Action::Error);
    }

    #[test]
    fn compress_goto_table_empty_input_yields_empty_entries() {
        let table: Vec<Vec<Option<StateId>>> = Vec::new();
        let compressed = compress_goto_table(&table);
        assert!(compressed.entries.is_empty());
        assert_eq!(decompress_goto(&compressed, 0, 0), None);
    }

    #[test]
    fn compress_goto_table_all_none_entries_yields_no_storage() {
        let table = vec![vec![None, None], vec![None, None]];
        let compressed = compress_goto_table(&table);
        assert!(compressed.entries.is_empty());
        assert_eq!(decompress_goto(&compressed, 0, 0), None);
        assert_eq!(decompress_goto(&compressed, 1, 1), None);
    }

    #[test]
    fn decompress_goto_missing_entry_returns_none() {
        let table = vec![vec![Some(StateId(5)), None]];
        let compressed = compress_goto_table(&table);
        // Position present
        assert_eq!(decompress_goto(&compressed, 0, 0), Some(StateId(5)));
        // Position absent (no entry recorded)
        assert_eq!(decompress_goto(&compressed, 0, 1), None);
        // Out-of-table coordinates
        assert_eq!(decompress_goto(&compressed, 9, 9), None);
    }

    #[test]
    fn bit_packed_table_decompresses_error_cells() {
        let table = vec![vec![Action::Error, Action::Error]];
        let packed = BitPackedActionTable::from_table(&table);
        assert_eq!(packed.decompress(0, 0), Action::Error);
        assert_eq!(packed.decompress(0, 1), Action::Error);
    }

    #[test]
    fn bit_packed_table_decompresses_fork_cell() {
        let inner = vec![
            Action::Shift(StateId(3)),
            Action::Reduce(adze_ir::RuleId(1)),
        ];
        let table = vec![vec![Action::Error, Action::Fork(inner.clone())]];
        let packed = BitPackedActionTable::from_table(&table);
        match packed.decompress(0, 1) {
            Action::Fork(actions) => assert_eq!(actions, inner),
            other => panic!("expected Fork, got {:?}", other),
        }
    }

    #[test]
    fn bit_packed_table_empty_table_has_zero_symbols() {
        let table: Vec<Vec<Action>> = Vec::new();
        let packed = BitPackedActionTable::from_table(&table);
        // No cells exist; the symbol count is zero.
        assert_eq!(packed.symbol_count, 0);
    }
}

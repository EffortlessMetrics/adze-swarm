#![cfg_attr(feature = "strict_docs", allow(missing_docs))]
//! Parse table compression using Tree-sitter's encoding scheme.

use crate::{Result, TableGenError, conflict_abi::abi_leaf_actions, goto_run_codec::GotoRunCodec};
use adze_glr_core::{Action, ParseTable};
use adze_ir::{StateId, SymbolId};
use std::collections::{BTreeMap, HashMap};

/// Compressed parse table representation
pub struct CompressedParseTable {
    symbol_count: usize,
    state_count: usize,
}

impl CompressedParseTable {
    /// Create a new compressed parse table for testing
    pub fn new_for_testing(symbol_count: usize, state_count: usize) -> Self {
        Self {
            symbol_count,
            state_count,
        }
    }

    /// Get the symbol count
    pub fn symbol_count(&self) -> usize {
        self.symbol_count
    }

    /// Get the state count
    pub fn state_count(&self) -> usize {
        self.state_count
    }

    /// Create from a parse table
    pub fn from_parse_table(parse_table: &ParseTable) -> Self {
        Self {
            symbol_count: parse_table.symbol_count,
            state_count: parse_table.state_count,
        }
    }
}

// Removed: This From implementation was returning dummy empty tables.
// Compression is now handled by TableCompressor::compress() method directly.

/// Complete compressed tables for Tree-sitter
pub struct CompressedTables {
    pub action_table: CompressedActionTable,
    pub goto_table: CompressedGotoTable,
    pub small_table_threshold: usize,
}

impl CompressedTables {
    /// Validate compressed tables against original parse table
    #[must_use = "validation result must be checked"]
    pub fn validate(&self, parse_table: &ParseTable) -> Result<()> {
        let state_count = parse_table.state_count;
        let symbol_count = parse_table.symbol_count;

        // Length checks
        let expected_rows = state_count + 1;
        if self.action_table.row_offsets.len() != expected_rows {
            return Err(TableGenError::InvalidTable(format!(
                "action row_offsets length {} does not match state_count + 1 ({expected_rows})",
                self.action_table.row_offsets.len()
            )));
        }
        if self.goto_table.row_offsets.len() != expected_rows {
            return Err(TableGenError::InvalidTable(format!(
                "goto row_offsets length {} does not match state_count + 1 ({expected_rows})",
                self.goto_table.row_offsets.len()
            )));
        }
        if self.action_table.default_actions.len() != state_count {
            return Err(TableGenError::InvalidTable(format!(
                "default_actions length {} does not match state_count {}",
                self.action_table.default_actions.len(),
                state_count
            )));
        }

        // Monotonicity and sentinel checks
        for (name, row_offsets, data_len) in [
            (
                "action",
                &self.action_table.row_offsets,
                self.action_table.data.len(),
            ),
            (
                "goto",
                &self.goto_table.row_offsets,
                self.goto_table.data.len(),
            ),
        ] {
            for i in 1..row_offsets.len() {
                if row_offsets[i] < row_offsets[i - 1] {
                    return Err(TableGenError::InvalidTable(format!(
                        "{name} row_offsets are not non-decreasing at index {i}: {} < {}",
                        row_offsets[i],
                        row_offsets[i - 1]
                    )));
                }
            }

            let last_offset = row_offsets.last().copied().unwrap_or(0) as usize;
            if last_offset != data_len {
                return Err(TableGenError::InvalidTable(format!(
                    "{name} last row offset {last_offset} does not match data length {data_len}"
                )));
            }
        }

        // u16 overflow: action data must fit in u16
        let action_data_len_u16 = u16::try_from(self.action_table.data.len()).map_err(|_| {
            TableGenError::Compression(format!(
                "action table data length {} exceeds u16::MAX ({})",
                self.action_table.data.len(),
                u16::MAX
            ))
        })?;

        for (i, &offset) in self.action_table.row_offsets.iter().enumerate() {
            if offset > action_data_len_u16 {
                return Err(TableGenError::Compression(format!(
                    "action row_offsets[{i}] = {offset} exceeds action data length {}",
                    self.action_table.data.len()
                )));
            }
        }

        for (idx, entry) in self.action_table.data.iter().enumerate() {
            if usize::from(entry.symbol) >= symbol_count {
                return Err(TableGenError::Compression(format!(
                    "action entry {} has symbol id {} outside symbol_count {}",
                    idx, entry.symbol, symbol_count
                )));
            }
        }

        let eof_col = *parse_table
            .symbol_to_index
            .get(&parse_table.eof_symbol)
            .ok_or_else(|| {
                TableGenError::InvalidTable(format!(
                    "EOF symbol {} missing from symbol_to_index",
                    parse_table.eof_symbol.0
                ))
            })?;

        // Invariant: if a state accepts in the source table, the compressed table must
        // still expose an Accept action at the EOF column for that state.
        for state in 0..parse_table.state_count {
            let source_accept_on_eof = parse_table.action_table[state]
                .get(eof_col)
                .is_some_and(|cell| cell.iter().any(|a| matches!(a, Action::Accept)));

            if source_accept_on_eof
                && !state_has_accept_on_symbol(&self.action_table, state, eof_col as u16)
            {
                return Err(TableGenError::Compression(format!(
                    "Accept-on-EOF lost in compression at state {} (EOF column {})",
                    state, eof_col
                )));
            }
        }

        // u16 overflow: goto data must fit in u16
        let goto_data_len_u16 = u16::try_from(self.goto_table.data.len()).map_err(|_| {
            TableGenError::Compression(format!(
                "goto table data length {} exceeds u16::MAX ({})",
                self.goto_table.data.len(),
                u16::MAX
            ))
        })?;

        for (i, &offset) in self.goto_table.row_offsets.iter().enumerate() {
            if offset > goto_data_len_u16 {
                return Err(TableGenError::Compression(format!(
                    "goto row_offsets[{i}] = {offset} exceeds goto data length {}",
                    self.goto_table.data.len()
                )));
            }
        }

        for (idx, entry) in self.goto_table.data.iter().enumerate() {
            match entry {
                CompressedGotoEntry::Single(state) => {
                    if *state != u16::MAX && usize::from(*state) >= state_count {
                        return Err(TableGenError::Compression(format!(
                            "goto entry {} has state id {} outside state_count {}",
                            idx, state, state_count
                        )));
                    }
                }
                CompressedGotoEntry::RunLength { state, count } => {
                    if *state != u16::MAX && usize::from(*state) >= state_count {
                        return Err(TableGenError::Compression(format!(
                            "goto run-length entry {} has state id {} outside state_count {}",
                            idx, state, state_count
                        )));
                    }
                    if *count == 0 {
                        return Err(TableGenError::Compression(format!(
                            "goto run-length entry {} has zero count, which is invalid",
                            idx
                        )));
                    }
                }
            }
        }

        Ok(())
    }
}

fn state_has_accept_on_symbol(table: &CompressedActionTable, state: usize, symbol: u16) -> bool {
    let start = table.row_offsets[state] as usize;
    let end = table.row_offsets[state + 1] as usize;
    if end > table.data.len() || start > end {
        return false;
    }
    table.data[start..end]
        .iter()
        .any(|entry| entry.symbol == symbol && matches!(entry.action, Action::Accept))
}

/// Compressed action table
#[derive(Debug, Clone)]
pub struct CompressedActionTable {
    pub data: Vec<CompressedActionEntry>,
    pub row_offsets: Vec<u16>,
    pub default_actions: Vec<Action>,
}

/// Entry in the action table
#[derive(Debug, Clone)]
pub struct ActionEntry {
    pub symbol: u16,
    pub action: Action,
}

/// Compressed action entry
#[derive(Debug, Clone)]
pub struct CompressedActionEntry {
    pub symbol: u16,
    pub action: Action,
}

impl CompressedActionEntry {
    /// Create a new compressed action entry
    pub fn new(symbol: u16, action: Action) -> Self {
        Self { symbol, action }
    }
}

/// Compressed goto table
#[derive(Debug, Clone)]
pub struct CompressedGotoTable {
    pub data: Vec<CompressedGotoEntry>,
    pub row_offsets: Vec<u16>,
}

/// Entry in the goto table
pub struct GotoEntry {
    pub symbol: SymbolId,
    pub state: u16,
}

/// Compressed goto entry with run-length encoding
#[derive(Debug, Clone)]
pub enum CompressedGotoEntry {
    Single(u16),
    RunLength { state: u16, count: u16 },
}

/// Lossless compressor for LR(1) parse tables produced by `glr-core`.
///
/// The compressor packs the ACTION/GOTO matrices into compact columnar
/// representations while preserving all transitions.
pub struct TableCompressor {
    // Tree-sitter's magic constants for compression
    small_table_threshold: usize,
}

impl Default for TableCompressor {
    fn default() -> Self {
        Self::new()
    }
}

impl TableCompressor {
    fn checked_u16(value: usize, context: &'static str) -> Result<u16> {
        u16::try_from(value).map_err(|_| {
            TableGenError::Compression(format!(
                "{context} overflow: {value} exceeds u16::MAX ({})",
                u16::MAX
            ))
        })
    }

    /// Create a new compressor with default thresholds.
    #[must_use]
    pub fn new() -> Self {
        Self {
            small_table_threshold: 32768, // Tree-sitter's threshold
        }
    }

    /// Encode an action for small tables
    #[must_use = "encoding result must be checked"]
    pub fn encode_action_small(&self, action: &Action) -> Result<u16> {
        match action {
            Action::Shift(state) => {
                if state.0 >= 0x8000 {
                    return Err(TableGenError::Compression(format!(
                        "Shift state {} too large for small table encoding",
                        state.0
                    )));
                }
                Ok(state.0)
            }
            Action::Reduce(rule) => {
                if rule.0 >= 0x4000 {
                    return Err(TableGenError::Compression(format!(
                        "Reduce rule {} too large for small table encoding",
                        rule.0
                    )));
                }
                // Reduce actions are encoded with high bit set
                // bit 15: 1 (indicates reduce)
                // bits 14-0: rule_id (1-based)
                // Tree-sitter uses 1-based production IDs
                Ok(0x8000 | (rule.0 + 1))
            }
            Action::Accept => Ok(0xFFFF),
            Action::Error => Ok(0xFFFE),
            Action::Recover => Ok(0xFFFD), // Use distinct value for Recover
            Action::Fork(_) => Err(TableGenError::Compression(
                "Fork actions must be flattened with effective_actions before small-table encoding"
                    .to_string(),
            )),
            _ => {
                // Unknown action type // Expected: V for Recover
                crate::util::unexpected_action(action, "encode_action_as_u16");
                Ok(0xFFFE)
            }
        }
    }

    /// Compress a parse table into compact ACTION/GOTO forms.
    ///
    /// * `parse_table` — LR(1) automaton from `glr-core`.
    /// * `token_indices` — column indices including EOF from `collect_token_indices`.
    /// * `start_can_be_empty` — whether the start symbol is nullable.
    ///
    /// Returns compressed tables suitable for embedding.
    ///
    /// ```ignore
    /// # use adze_ir::builder::GrammarBuilder;
    /// # use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
    /// # use adze_tablegen::{TableCompressor, helpers::{collect_token_indices, eof_accepts_or_reduces}};
    /// # let g = GrammarBuilder::new("demo").start("module").build();
    /// # let ff = FirstFollowSets::compute(&g);
    /// # let pt = build_lr1_automaton(&g, &ff).unwrap();
    /// # let token_ix = collect_token_indices(&g, &pt);
    /// let compressed = TableCompressor::new()
    ///     .compress(&pt, &token_ix, eof_accepts_or_reduces(&pt))
    ///     .unwrap();
    /// # let _ = compressed;
    /// ```
    ///
    /// # Breaking Change Note
    /// This function signature changed to include `token_indices` and `start_can_be_empty` parameters
    /// to properly handle nullable start symbols and GLR multi-action cells.
    pub fn compress(
        &self,
        parse_table: &ParseTable,
        token_indices: &[usize],
        start_can_be_empty: bool,
    ) -> Result<CompressedTables> {
        // Convert token_indices to FxHashSet for O(1) membership checks with better performance
        use rustc_hash::FxHashSet;

        // Debug assertions to verify invariants (zero cost in release builds)
        debug_assert!(
            token_indices.windows(2).all(|w| w[0] < w[1]),
            "token_indices must be strictly increasing (sorted and deduped)"
        );

        // Only assert EOF presence if the parse table actually exposes an EOF mapping
        // Don't assume EOF is at column 0 - derive it from symbol_to_index using the actual eof_symbol
        if let Some(&eof_idx) = parse_table.symbol_to_index.get(&parse_table.eof_symbol) {
            debug_assert!(
                token_indices.contains(&eof_idx),
                "token_indices must contain EOF column (derived from symbol_to_index)"
            );
        }

        let token_set: FxHashSet<usize> = token_indices.iter().copied().collect();

        // Fetch EOF column index once and reuse it everywhere
        // Use parse_table.eof_symbol instead of hardcoded SymbolId(0) since EOF symbol
        // is computed as max_symbol + 1 in build_lr1_automaton
        let eof_idx = *parse_table
            .symbol_to_index
            .get(&parse_table.eof_symbol)
            .ok_or_else(|| TableGenError::InvalidTable(
                format!("EOF (symbol {}) not found in symbol_to_index map - this is a critical invariant violation", parse_table.eof_symbol.0)
            ))?;

        // Validation: Ensure state 0 has at least one token shift action
        // This catches the "state 0 bug" where no tokens can be shifted from the initial state
        if let Some(state0_actions) = parse_table.action_table.first() {
            // Check if any token column has a shift action
            let has_token_shift = token_indices.iter().any(|&idx| {
                state0_actions
                    .get(idx)
                    .is_some_and(|cell| cell.iter().any(|a| matches!(a, Action::Shift(_))))
            });

            // If no token shifts, and start is nullable, allow ACCEPT/REDUCE on EOF column
            let eof_ok = !has_token_shift
                && start_can_be_empty
                && state0_actions.get(eof_idx).is_some_and(|cell| {
                    cell.iter()
                        .any(|a| matches!(a, Action::Accept | Action::Reduce(_)))
                });

            if !has_token_shift && !eof_ok {
                // Provide detailed debugging info
                let mut debug_info = String::new();

                // Show expected token columns
                debug_info.push_str(&format!(
                    "Expected token columns (first 12): {:?}\n",
                    token_indices.iter().take(12).collect::<Vec<_>>()
                ));
                debug_info.push_str(&format!("Start can be empty: {}\n", start_can_be_empty));

                // Show the actual state-0 actions
                debug_info.push_str("State 0 actions (first 12 columns):\n");
                for (idx, cell) in state0_actions.iter().enumerate().take(12) {
                    // Prefer labeling by EOF column equality rather than symbol id
                    let symbol_info = if idx == eof_idx {
                        "EOF".to_string()
                    } else {
                        parse_table
                            .symbol_to_index
                            .iter()
                            .find(|(_, i)| **i == idx)
                            .map(|(sym_id, _)| format!("sym_{}", sym_id.0))
                            .unwrap_or_else(|| "unmapped".to_string())
                    };

                    let type_str = if idx == eof_idx || token_set.contains(&idx) {
                        "TOKEN"
                    } else {
                        "NT"
                    };

                    let action_str = if cell.is_empty() {
                        "[]".to_string()
                    } else {
                        format!("{:?}", cell)
                    };

                    debug_info.push_str(&format!(
                        "  Col {:2} ({:8} {:5}): {}\n",
                        idx, symbol_info, type_str, action_str
                    ));
                }

                // Provide actionable guidance
                debug_info.push_str("\nPossible causes:\n");
                debug_info.push_str("1. Pattern wrappers not desugared to unit rules\n");
                debug_info
                    .push_str("2. Token symbols not properly registered in symbol_to_index\n");
                debug_info.push_str("3. Grammar start symbol issues\n");

                return Err(TableGenError::Compression(format!(
                    "State 0 validation failed: No valid token shift actions found.\n{}",
                    debug_info
                )));
            }
        }

        // Additional sanity guards
        if parse_table.action_table.is_empty() {
            return Err(TableGenError::Compression(
                "Empty action table - grammar has no parse states".to_string(),
            ));
        }

        if parse_table.state_count == 0 {
            return Err(TableGenError::Compression(
                "State count is 0 - invalid parse table".to_string(),
            ));
        }

        // Determine if we should use small table optimization
        let use_small_table = parse_table.state_count < self.small_table_threshold;

        if use_small_table {
            self.compress_small_table(parse_table)
        } else {
            self.compress_large_table(parse_table)
        }
    }

    /// Compress using Tree-sitter's "small table" optimization
    fn compress_small_table(&self, parse_table: &ParseTable) -> Result<CompressedTables> {
        let compressed_action_table = self
            .compress_action_table_small(&parse_table.action_table, &parse_table.symbol_to_index)?;
        let compressed_goto_table = self.compress_goto_table_small(&parse_table.goto_table)?;

        Ok(CompressedTables {
            action_table: compressed_action_table,
            goto_table: compressed_goto_table,
            small_table_threshold: self.small_table_threshold,
        })
    }

    /// Compress using large table optimization
    fn compress_large_table(&self, parse_table: &ParseTable) -> Result<CompressedTables> {
        // For now, use the same as small table
        self.compress_small_table(parse_table)
    }

    /// Compress action table using Tree-sitter's small table format
    pub fn compress_action_table_small(
        &self,
        action_table: &[Vec<Vec<Action>>],
        symbol_to_index: &BTreeMap<SymbolId, usize>,
    ) -> Result<CompressedActionTable> {
        let mut entries = Vec::new();
        let mut row_offsets = Vec::new();
        let mut default_actions = Vec::new();

        // Create inverse mapping from index to symbol ID
        let mut index_to_symbol = HashMap::new();
        for (&symbol_id, &index) in symbol_to_index {
            index_to_symbol.insert(index, symbol_id);
        }

        for action_row in action_table.iter() {
            // Find the most common action across all cells
            let mut action_counts: HashMap<Action, usize> = HashMap::new();
            let mut _has_shift = false;
            let mut _has_accept = false;

            // Collect all actions from all cells in this row
            for action_cell in action_row {
                for action in action_cell {
                    *action_counts.entry(action.clone()).or_insert(0) += 1;
                    match action {
                        Action::Shift(_) => _has_shift = true,
                        Action::Accept => _has_accept = true,
                        _ => {}
                    }
                }
            }

            let _most_common = action_counts
                .iter()
                .max_by_key(|(_, count)| *count)
                .map(|(action, _)| action.clone())
                .unwrap_or(Action::Error);

            // Default action optimization is currently disabled by design.
            // The runtime does not use the default_actions array, so we encode all actions explicitly
            // and populate default_actions with Action::Error as a placeholder.
            // This ensures no information is lost during compression and all actions are available at runtime.
            // Future: Could optimize by implementing default action support in the runtime decoder.
            let default_action = Action::Error;

            default_actions.push(default_action.clone());
            row_offsets.push(Self::checked_u16(entries.len(), "action row offset")?);

            for (index, action_cell) in action_row.iter().enumerate() {
                let symbol_id = Self::checked_u16(index, "action symbol id")?;
                for action in abi_leaf_actions(action_cell) {
                    entries.push(CompressedActionEntry {
                        symbol: symbol_id,
                        action,
                    });
                }
            }
        }

        row_offsets.push(Self::checked_u16(entries.len(), "action row offset")?);

        // Validate row_offsets are strictly increasing
        for i in 1..row_offsets.len() {
            if row_offsets[i] < row_offsets[i - 1] {
                return Err(TableGenError::Compression(format!(
                    "Row offsets not strictly increasing at index {}: {} < {}",
                    i,
                    row_offsets[i],
                    row_offsets[i - 1]
                )));
            }
        }

        // Validate map length matches state count
        if row_offsets.len() != action_table.len() + 1 {
            return Err(TableGenError::Compression(format!(
                "Row offsets length {} doesn't match state count {} + 1",
                row_offsets.len(),
                action_table.len()
            )));
        }

        Ok(CompressedActionTable {
            data: entries,
            row_offsets,
            default_actions,
        })
    }

    /// Compress goto table  
    pub fn compress_goto_table_small(
        &self,
        goto_table: &[Vec<StateId>],
    ) -> Result<CompressedGotoTable> {
        let mut codec = GotoRunCodec::new();

        for row in goto_table {
            codec.begin_row()?;

            for &state_id in row {
                codec.push_state(state_id.0)?;
            }

            codec.end_row()?;
        }

        let (data, row_offsets) = codec.finish()?;

        Ok(CompressedGotoTable { data, row_offsets })
    }

    // Removed in 0.8.0 - use compress(parse_table, token_indices, start_can_be_empty)
    // See MIGRATING.md for migration guide
}

#[cfg(test)]
mod tests {
    use super::*;
    use adze_glr_core::Action;
    use adze_ir::{RuleId, StateId};

    #[test]
    fn test_compressed_parse_table_creation() {
        let table = CompressedParseTable::new_for_testing(10, 20);
        assert_eq!(table.symbol_count(), 10);
        assert_eq!(table.state_count(), 20);
    }

    #[test]
    fn test_compressed_parse_table_from_parse_table() {
        let parse_table = crate::test_helpers::test::make_minimal_table(
            vec![vec![vec![]; 5]; 10], // 10 states, 5 symbols
            vec![vec![crate::test_helpers::test::INVALID; 5]; 10],
            vec![],
            SymbolId(2), // start_symbol
            SymbolId(1), // eof_symbol (must be > 0)
            0,           // external_token_count
        );

        let compressed = CompressedParseTable::from_parse_table(&parse_table);
        assert_eq!(compressed.symbol_count(), 5);
        assert_eq!(compressed.state_count(), 10);
    }

    #[test]
    fn test_compressed_action_entry() {
        let entry = CompressedActionEntry::new(42, Action::Shift(StateId(5)));
        assert_eq!(entry.symbol, 42);
        match entry.action {
            Action::Shift(StateId(5)) => {}
            _ => panic!("Expected shift action"),
        }
    }

    #[test]
    fn test_table_compressor_creation() {
        let compressor = TableCompressor::new();
        // Just verify it can be created
        assert!(compressor.small_table_threshold > 0);
    }

    #[test]
    fn test_compress_empty_action_table() {
        let compressor = TableCompressor::new();
        let action_table = vec![vec![]; 5]; // 5 empty states

        let symbol_to_index = std::collections::BTreeMap::new();
        let result = compressor.compress_action_table_small(&action_table, &symbol_to_index);
        assert!(result.is_ok());

        let compressed = result.unwrap();
        assert_eq!(compressed.row_offsets.len(), 6); // n_states + 1
        assert_eq!(compressed.default_actions.len(), 5);
        assert!(compressed.data.is_empty());
    }

    #[test]
    fn test_compress_action_table_with_default_reduce() {
        let compressor = TableCompressor::new();
        let reduce_action = Action::Reduce(RuleId(1));
        let action_table = vec![
            vec![vec![reduce_action.clone()]; 10], // All same reduce action in ActionCells
        ];

        let symbol_to_index = std::collections::BTreeMap::new();
        let result = compressor.compress_action_table_small(&action_table, &symbol_to_index);
        assert!(result.is_ok());

        let compressed = result.unwrap();
        // Default action optimization is disabled, so default should be Error
        assert_eq!(
            compressed.default_actions[0],
            Action::Error,
            "Default action optimization disabled"
        );
        // All 10 reduce actions should be explicitly encoded
        assert_eq!(
            compressed.data.len(),
            10,
            "All reduce actions should be explicitly encoded"
        );
    }

    #[test]
    fn test_compress_goto_table_with_runs() {
        let compressor = TableCompressor::new();
        let goto_table = vec![vec![
            StateId(1),
            StateId(1),
            StateId(1),
            StateId(2),
            StateId(2),
        ]];

        let result = compressor.compress_goto_table_small(&goto_table);
        assert!(result.is_ok());

        let compressed = result.unwrap();
        assert!(!compressed.data.is_empty());

        // Should have a run length entry for the three 1s
        let has_run_length = compressed
            .data
            .iter()
            .any(|entry| matches!(entry, CompressedGotoEntry::RunLength { state: 1, count: 3 }));
        assert!(has_run_length);
    }

    #[test]
    fn test_compressed_tables_validation() {
        let tables = CompressedTables {
            action_table: CompressedActionTable {
                data: vec![],
                row_offsets: vec![0, 0],
                default_actions: vec![Action::Error],
            },
            goto_table: CompressedGotoTable {
                data: vec![],
                row_offsets: vec![0, 0],
            },
            small_table_threshold: 32768,
        };

        let parse_table = crate::test_helpers::test::make_minimal_table(
            vec![vec![vec![]]], // 1 state, 1 symbol (minimum required)
            vec![vec![crate::test_helpers::test::INVALID]], // 1 state, 1 symbol
            vec![],             // 0 rules
            SymbolId(1),        // start_symbol
            SymbolId(1),        // eof_symbol (must be >= 1)
            0,                  // external_token_count
        );
        let result = tables.validate(&parse_table);
        assert!(result.is_ok());
    }

    #[test]
    fn test_compressed_tables_validation_rejects_bad_offsets() {
        let tables = CompressedTables {
            action_table: CompressedActionTable {
                data: vec![],
                row_offsets: vec![0],
                default_actions: vec![],
            },
            goto_table: CompressedGotoTable {
                data: vec![],
                row_offsets: vec![0],
            },
            small_table_threshold: 32768,
        };

        let parse_table = crate::test_helpers::test::make_minimal_table(
            vec![vec![vec![]]],
            vec![vec![crate::test_helpers::test::INVALID]],
            vec![],
            SymbolId(1),
            SymbolId(1),
            0,
        );

        let result = tables.validate(&parse_table);
        assert!(matches!(result, Err(TableGenError::InvalidTable(_))));
    }

    #[test]
    fn test_action_table_compression_fails_on_u16_overflow() {
        let compressor = TableCompressor::new();

        let action_table = vec![vec![
            vec![Action::Shift(StateId(1))];
            usize::from(u16::MAX) + 1
        ]];
        let symbol_to_index = BTreeMap::from([(SymbolId(1), 0usize)]);

        let result = compressor.compress_action_table_small(&action_table, &symbol_to_index);
        assert!(matches!(result, Err(TableGenError::Compression(_))));
    }
}

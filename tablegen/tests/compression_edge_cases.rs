//! Edge-case tests for table compression in adze-tablegen.
//!
//! Covers degenerate inputs, boundary conditions, sparse/dense extremes,
//! encoding limits, roundtrip verification, and structural invariants that
//! the main correctness suite does not exercise.

use adze_glr_core::Action;
use adze_ir::{RuleId, StateId};
use adze_tablegen::compress::{
    CompressedActionEntry, CompressedGotoEntry, CompressedParseTable, TableCompressor,
};
use adze_tablegen::compression::{
    BitPackedActionTable, compress_action_table, compress_goto_table, decompress_action,
    decompress_goto,
};
use std::collections::BTreeMap;

// ── helpers ─────────────────────────────────────────────────────────────────

/// Wrap each action into a single-element GLR cell (empty for Error).
fn glr_table(rows: Vec<Vec<Action>>) -> Vec<Vec<Vec<Action>>> {
    rows.into_iter()
        .map(|row| {
            row.into_iter()
                .map(|a| {
                    if matches!(a, Action::Error) {
                        vec![]
                    } else {
                        vec![a]
                    }
                })
                .collect()
        })
        .collect()
}

/// Verify every cell of an action table survives roundtrip through compression.
fn assert_action_roundtrip(table: &[Vec<Vec<Action>>]) {
    let compressed = compress_action_table(table);
    for (state, row) in table.iter().enumerate() {
        for (sym, cell) in row.iter().enumerate() {
            let expected = cell.first().cloned().unwrap_or(Action::Error);
            let got = decompress_action(&compressed, state, sym);
            assert_eq!(got, expected, "action mismatch state={state} sym={sym}");
        }
    }
}

/// Verify every cell of a goto table survives roundtrip through compression.
fn assert_goto_roundtrip(table: &[Vec<Option<StateId>>]) {
    let compressed = compress_goto_table(table);
    for (state, row) in table.iter().enumerate() {
        for (sym, &expected) in row.iter().enumerate() {
            let got = decompress_goto(&compressed, state, sym);
            assert_eq!(got, expected, "goto mismatch state={state} sym={sym}");
        }
    }
}

// ── 1. Single-state, single-symbol tables ───────────────────────────────────

#[test]
fn action_table_1x1_error() {
    let table = vec![vec![vec![]]];
    let c = compress_action_table(&table);
    assert_eq!(decompress_action(&c, 0, 0), Action::Error);
    assert_eq!(c.unique_rows.len(), 1);
}

#[test]
fn action_table_1x1_shift() {
    let table = vec![vec![vec![Action::Shift(StateId(0))]]];
    assert_action_roundtrip(&table);
}

#[test]
fn goto_table_1x1_none() {
    let table = vec![vec![None]];
    let c = compress_goto_table(&table);
    assert!(c.entries.is_empty());
    assert_eq!(decompress_goto(&c, 0, 0), None);
}

#[test]
fn goto_table_1x1_some() {
    let table = vec![vec![Some(StateId(0))]];
    assert_goto_roundtrip(&table);
    let c = compress_goto_table(&table);
    assert_eq!(c.entries.len(), 1);
}

// ── 2. Completely empty tables ──────────────────────────────────────────────

#[test]
fn action_table_zero_symbols_per_state() {
    // States exist but have no symbol columns at all.
    let table: Vec<Vec<Vec<Action>>> = vec![vec![], vec![], vec![]];
    let c = compress_action_table(&table);
    // All states map to the same empty row.
    assert_eq!(c.unique_rows.len(), 1);
    assert_eq!(c.state_to_row.len(), 3);
}

#[test]
fn goto_table_zero_symbols_per_state() {
    let table: Vec<Vec<Option<StateId>>> = vec![vec![], vec![]];
    let c = compress_goto_table(&table);
    assert!(c.entries.is_empty());
}

// ── 3. All-identical rows stress row deduplication ──────────────────────────

#[test]
fn action_all_rows_identical_100_states() {
    let row = vec![
        vec![Action::Shift(StateId(1))],
        vec![Action::Reduce(RuleId(0))],
        vec![],
    ];
    let table: Vec<Vec<Vec<Action>>> = vec![row; 100];
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 1, "100 identical rows → 1 unique");
    assert_eq!(c.state_to_row.len(), 100);
    // All states must point to row 0.
    assert!(c.state_to_row.iter().all(|&idx| idx == 0));
}

// ── 4. Every row is unique ──────────────────────────────────────────────────

#[test]
fn action_all_rows_distinct() {
    let table: Vec<Vec<Vec<Action>>> = (0u16..20)
        .map(|s| vec![vec![Action::Shift(StateId(s))]])
        .collect();
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 20, "20 distinct rows → 20 unique");
}

// ── 5. Extremely wide rows (many symbols) ───────────────────────────────────

#[test]
fn action_roundtrip_200_symbols() {
    let row: Vec<Action> = (0..200)
        .map(|i| match i % 3 {
            0 => Action::Shift(StateId((i % 50) as u16)),
            1 => Action::Reduce(RuleId((i % 30) as u16)),
            _ => Action::Error,
        })
        .collect();
    let table = glr_table(vec![row]);
    assert_action_roundtrip(&table);
}

#[test]
fn goto_roundtrip_200_symbols() {
    let row: Vec<Option<StateId>> = (0..200)
        .map(|i| {
            if i % 4 == 0 {
                Some(StateId((i % 40) as u16))
            } else {
                None
            }
        })
        .collect();
    assert_goto_roundtrip(&[row]);
}

// ── 6. Encoding boundary values ─────────────────────────────────────────────

#[test]
fn encode_shift_max_small() {
    let compressor = TableCompressor::new();
    // Maximum valid shift state for small table encoding: 0x7FFF - 1
    let max_state = 0x7FFF - 1;
    let encoded = compressor
        .encode_action_small(&Action::Shift(StateId(max_state)))
        .unwrap();
    assert_eq!(encoded, max_state);
}

#[test]
fn encode_shift_overflow_rejected() {
    let compressor = TableCompressor::new();
    let result = compressor.encode_action_small(&Action::Shift(StateId(0x8000)));
    assert!(result.is_err(), "shift state 0x8000 must be rejected");
}

#[test]
fn encode_reduce_max_small() {
    let compressor = TableCompressor::new();
    // Maximum valid rule ID for small encoding: 0x3FFF - 1
    let max_rule = 0x3FFF - 1;
    let encoded = compressor
        .encode_action_small(&Action::Reduce(RuleId(max_rule)))
        .unwrap();
    assert_eq!(encoded, 0x8000 | (max_rule + 1));
}

#[test]
fn encode_reduce_overflow_rejected() {
    let compressor = TableCompressor::new();
    let result = compressor.encode_action_small(&Action::Reduce(RuleId(0x4000)));
    assert!(result.is_err(), "rule id 0x4000 must be rejected");
}

#[test]
fn encode_special_sentinels_are_distinct() {
    let compressor = TableCompressor::new();
    let accept = compressor.encode_action_small(&Action::Accept).unwrap();
    let error = compressor.encode_action_small(&Action::Error).unwrap();
    let recover = compressor.encode_action_small(&Action::Recover).unwrap();
    // All three must be distinct values.
    assert_ne!(accept, error);
    assert_ne!(accept, recover);
    assert_ne!(error, recover);
}

// ── 7. Goto run-length encoding edge cases ──────────────────────────────────

#[test]
fn goto_rle_single_element_row() {
    let compressor = TableCompressor::new();
    let goto_table = vec![vec![StateId(5)]];
    let c = compressor.compress_goto_table_small(&goto_table).unwrap();
    // A single element cannot form a run > 2, so it must be Single.
    assert_eq!(c.data.len(), 1);
    assert!(matches!(c.data[0], CompressedGotoEntry::Single(5)));
}

#[test]
fn goto_rle_alternating_values_no_runs() {
    let compressor = TableCompressor::new();
    let goto_table = vec![vec![
        StateId(1),
        StateId(2),
        StateId(1),
        StateId(2),
        StateId(1),
        StateId(2),
    ]];
    let c = compressor.compress_goto_table_small(&goto_table).unwrap();
    // No consecutive duplicates, so all entries should be Single.
    assert!(
        c.data
            .iter()
            .all(|e| matches!(e, CompressedGotoEntry::Single(_)))
    );
    assert_eq!(c.data.len(), 6);
}

#[test]
fn goto_rle_boundary_run_of_exactly_three() {
    let compressor = TableCompressor::new();
    // Run of exactly 3 is the threshold for RunLength encoding.
    let goto_table = vec![vec![StateId(9), StateId(9), StateId(9)]];
    let c = compressor.compress_goto_table_small(&goto_table).unwrap();
    let has_rl = c
        .data
        .iter()
        .any(|e| matches!(e, CompressedGotoEntry::RunLength { state: 9, count: 3 }));
    assert!(has_rl, "run of 3 should produce RunLength");
}

#[test]
fn goto_rle_mixed_runs_and_singles() {
    let compressor = TableCompressor::new();
    // Pattern: [4,4,4,4, 7, 2,2,2] → RunLength(4,4), Single(7), RunLength(2,3)
    let goto_table = vec![vec![
        StateId(4),
        StateId(4),
        StateId(4),
        StateId(4),
        StateId(7),
        StateId(2),
        StateId(2),
        StateId(2),
    ]];
    let c = compressor.compress_goto_table_small(&goto_table).unwrap();
    // Expand and verify total count matches original.
    let total: usize = c
        .data
        .iter()
        .map(|e| match e {
            CompressedGotoEntry::Single(_) => 1,
            CompressedGotoEntry::RunLength { count, .. } => *count as usize,
        })
        .sum();
    assert_eq!(total, 8, "expanded entries must sum to original length");
}

// ── 8. Row offset invariants ────────────────────────────────────────────────

#[test]
fn action_row_offsets_length_equals_states_plus_one() {
    let compressor = TableCompressor::new();
    for n_states in [1, 2, 5, 10, 25] {
        let action_table: Vec<Vec<Vec<Action>>> =
            vec![vec![vec![Action::Shift(StateId(0))]; 3]; n_states];
        let sym_map = BTreeMap::new();
        let c = compressor
            .compress_action_table_small(&action_table, &sym_map)
            .unwrap();
        assert_eq!(
            c.row_offsets.len(),
            n_states + 1,
            "n_states={n_states}: offsets length must be states + 1"
        );
    }
}

#[test]
fn goto_row_offsets_length_equals_states_plus_one() {
    let compressor = TableCompressor::new();
    for n_states in [1, 3, 7, 15] {
        let goto_table = vec![vec![StateId(0); 4]; n_states];
        let c = compressor.compress_goto_table_small(&goto_table).unwrap();
        assert_eq!(
            c.row_offsets.len(),
            n_states + 1,
            "n_states={n_states}: goto offsets length must be states + 1"
        );
    }
}

#[test]
fn action_row_offsets_nondecreasing() {
    let compressor = TableCompressor::new();
    // Mix of empty and populated rows.
    let action_table = vec![
        vec![vec![]; 4],                          // all empty
        vec![vec![Action::Shift(StateId(1))]; 4], // all shift
        vec![vec![]; 4],                          // all empty again
        vec![vec![Action::Reduce(RuleId(0))]; 4], // all reduce
    ];
    let sym_map = BTreeMap::new();
    let c = compressor
        .compress_action_table_small(&action_table, &sym_map)
        .unwrap();
    for pair in c.row_offsets.windows(2) {
        assert!(
            pair[1] >= pair[0],
            "row offsets must be non-decreasing: {} < {}",
            pair[1],
            pair[0]
        );
    }
}

// ── 9. CompressedParseTable degenerate dimensions ───────────────────────────

#[test]
fn compressed_parse_table_zero_dimensions() {
    let table = CompressedParseTable::new_for_testing(0, 0);
    assert_eq!(table.symbol_count(), 0);
    assert_eq!(table.state_count(), 0);
}

#[test]
fn compressed_parse_table_large_dimensions() {
    let table = CompressedParseTable::new_for_testing(10_000, 5_000);
    assert_eq!(table.symbol_count(), 10_000);
    assert_eq!(table.state_count(), 5_000);
}

// ── 10. CompressedActionEntry preserves all action variants ─────────────────

#[test]
fn compressed_action_entry_all_variants() {
    let variants: Vec<Action> = vec![
        Action::Shift(StateId(0)),
        Action::Shift(StateId(u16::MAX)),
        Action::Reduce(RuleId(0)),
        Action::Reduce(RuleId(u16::MAX)),
        Action::Accept,
        Action::Error,
        Action::Recover,
    ];
    for (i, action) in variants.iter().enumerate() {
        let entry = CompressedActionEntry::new(i as u16, action.clone());
        assert_eq!(entry.symbol, i as u16);
        assert_eq!(entry.action, *action, "variant {i} mismatch");
    }
}

// ── 11. Sparse table: only diagonal has entries ─────────────────────────────

#[test]
fn action_diagonal_only() {
    let n = 10;
    let table: Vec<Vec<Vec<Action>>> = (0..n)
        .map(|s| {
            (0..n)
                .map(|sym| {
                    if s == sym {
                        vec![Action::Shift(StateId(s as u16))]
                    } else {
                        vec![]
                    }
                })
                .collect()
        })
        .collect();
    assert_action_roundtrip(&table);
    let c = compress_action_table(&table);
    // Each row is unique (different column has the shift).
    assert_eq!(c.unique_rows.len(), n);
}

#[test]
fn goto_diagonal_only() {
    let n = 10;
    let table: Vec<Vec<Option<StateId>>> = (0..n)
        .map(|s| {
            (0..n)
                .map(|sym| {
                    if s == sym {
                        Some(StateId(s as u16))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .collect();
    assert_goto_roundtrip(&table);
    let c = compress_goto_table(&table);
    assert_eq!(c.entries.len(), n);
}

// ── 12. GLR multi-action cells ──────────────────────────────────────────────

#[test]
fn multi_action_cell_preserves_first() {
    let table = vec![vec![
        vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(2))],
        vec![Action::Reduce(RuleId(3)), Action::Accept],
    ]];
    let c = compress_action_table(&table);
    // decompress_action returns the first action per cell.
    assert_eq!(decompress_action(&c, 0, 0), Action::Shift(StateId(1)));
    assert_eq!(decompress_action(&c, 0, 1), Action::Reduce(RuleId(3)));
}

#[test]
fn multi_action_cell_all_same() {
    // All actions in the cell are identical.
    let table = vec![vec![vec![
        Action::Reduce(RuleId(5)),
        Action::Reduce(RuleId(5)),
        Action::Reduce(RuleId(5)),
    ]]];
    let c = compress_action_table(&table);
    assert_eq!(decompress_action(&c, 0, 0), Action::Reduce(RuleId(5)));
}

// ── 13. Default actions array always Error (optimization disabled) ──────────

#[test]
fn default_actions_always_error_mixed_table() {
    let compressor = TableCompressor::new();
    let action_table = vec![
        vec![vec![Action::Accept]; 3],
        vec![vec![Action::Shift(StateId(0))]; 3],
        vec![vec![Action::Reduce(RuleId(7))]; 3],
        vec![vec![]; 3],
    ];
    let sym_map = BTreeMap::new();
    let c = compressor
        .compress_action_table_small(&action_table, &sym_map)
        .unwrap();
    assert_eq!(c.default_actions.len(), 4);
    for (i, d) in c.default_actions.iter().enumerate() {
        assert_eq!(*d, Action::Error, "row {i}: default must be Error");
    }
}

// ── 14. Small-table compression skips explicit Error actions ────────────────

#[test]
fn small_table_skips_error_actions() {
    let compressor = TableCompressor::new();
    let action_table = vec![vec![
        vec![Action::Error],
        vec![Action::Shift(StateId(1))],
        vec![Action::Error],
        vec![Action::Reduce(RuleId(0))],
        vec![Action::Error],
    ]];
    let sym_map = BTreeMap::new();
    let c = compressor
        .compress_action_table_small(&action_table, &sym_map)
        .unwrap();
    // Only the shift and reduce should be stored (Error cells skipped).
    assert_eq!(c.data.len(), 2, "Error actions must be skipped");
    // Verify stored symbols and actions.
    assert_eq!(c.data[0].symbol, 1);
    assert_eq!(c.data[0].action, Action::Shift(StateId(1)));
    assert_eq!(c.data[1].symbol, 3);
    assert_eq!(c.data[1].action, Action::Reduce(RuleId(0)));
}

// ── 15. Fork action encoding in small tables ────────────────────────────────

#[test]
fn encode_fork_action_small_requires_flattening() {
    let compressor = TableCompressor::new();
    let fork = Action::Fork(vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(0))]);
    assert!(compressor.encode_action_small(&fork).is_err());
}

// ── 16. BitPackedActionTable roundtrip ──────────────────────────────────────

#[test]
fn bitpacked_error_only_table() {
    let table = vec![vec![Action::Error; 5]; 3];
    let packed = BitPackedActionTable::from_table(&table);
    for state in 0..3 {
        for sym in 0..5 {
            assert_eq!(
                packed.decompress(state, sym),
                Action::Error,
                "state={state} sym={sym}"
            );
        }
    }
}

// ── 17. Goto compression with all-same values (maximal run) ─────────────────

#[test]
fn goto_all_same_value_large_row() {
    let compressor = TableCompressor::new();
    let goto_table = vec![vec![StateId(42); 50]];
    let c = compressor.compress_goto_table_small(&goto_table).unwrap();
    // Entire row should compress to a single RunLength entry.
    assert_eq!(c.data.len(), 1);
    assert!(matches!(
        c.data[0],
        CompressedGotoEntry::RunLength {
            state: 42,
            count: 50
        }
    ));
}

// ── 18. Goto with many distinct values (no runs at all) ─────────────────────

#[test]
fn goto_all_distinct_values() {
    let compressor = TableCompressor::new();
    let goto_table = vec![(0u16..30).map(StateId).collect::<Vec<_>>()];
    let c = compressor.compress_goto_table_small(&goto_table).unwrap();
    // Every value is different, so all entries should be Single.
    assert_eq!(c.data.len(), 30);
    assert!(
        c.data
            .iter()
            .all(|e| matches!(e, CompressedGotoEntry::Single(_)))
    );
}

// ── 19. Stress: wide sparse table roundtrip (500 symbols) ───────────────────

#[test]
fn goto_sparse_500_symbols_roundtrip() {
    let n = 500;
    let row: Vec<Option<StateId>> = (0..n)
        .map(|i| {
            if i % 17 == 0 {
                Some(StateId((i % 100) as u16))
            } else {
                None
            }
        })
        .collect();
    assert_goto_roundtrip(&[row]);
}

// ── 20. Action table: every action variant in same row ──────────────────────

#[test]
fn action_row_with_every_variant() {
    let table = glr_table(vec![vec![
        Action::Error,
        Action::Shift(StateId(10)),
        Action::Reduce(RuleId(5)),
        Action::Accept,
        Action::Recover,
    ]]);
    assert_action_roundtrip(&table);
}

// ── 21. Encoding roundtrip for reduce rule 0 (edge: 1-based offset) ────────

#[test]
fn encode_reduce_rule_zero() {
    let compressor = TableCompressor::new();
    let encoded = compressor
        .encode_action_small(&Action::Reduce(RuleId(0)))
        .unwrap();
    // 1-based: rule 0 encodes as 0x8000 | 1 = 0x8001
    assert_eq!(encoded, 0x8001);
}

// ── 22. TableCompressor Default impl works ──────────────────────────────────

#[test]
fn table_compressor_default_matches_new() {
    let from_new = TableCompressor::new();
    let from_default = TableCompressor::default();
    // Both should produce identical compression for the same input.
    let action_table = vec![vec![vec![Action::Shift(StateId(1))]; 2]];
    let sym_map = BTreeMap::new();
    let c1 = from_new
        .compress_action_table_small(&action_table, &sym_map)
        .unwrap();
    let c2 = from_default
        .compress_action_table_small(&action_table, &sym_map)
        .unwrap();
    assert_eq!(c1.data.len(), c2.data.len());
    assert_eq!(c1.row_offsets, c2.row_offsets);
}

// ── 23. Goto row offsets sentinel points to end of data ─────────────────────

#[test]
fn goto_row_offsets_sentinel_points_to_data_end() {
    let compressor = TableCompressor::new();
    let goto_table = vec![
        vec![StateId(1), StateId(2)],
        vec![StateId(3), StateId(3), StateId(3)],
    ];
    let c = compressor.compress_goto_table_small(&goto_table).unwrap();
    let sentinel = *c.row_offsets.last().unwrap();
    assert_eq!(
        sentinel as usize,
        c.data.len(),
        "sentinel must equal data length"
    );
}

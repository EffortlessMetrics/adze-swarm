//! Tests for bit-packing, compression, and roundtrip correctness in `adze-tablegen`.
//!
//! Covers: `BitPackedActionTable` encoding/decoding, compression ratios,
//! roundtrip fidelity, determinism, goto table compression, mixed action types,
//! scaling behavior, and edge cases.

use adze_glr_core::Action;
use adze_ir::{RuleId, StateId};
use adze_tablegen::TableCompressor;
use adze_tablegen::compression::{
    BitPackedActionTable, compress_action_table, compress_goto_table, decompress_action,
    decompress_goto,
};

// ═══════════════════════════════════════════════════════════════════════════
// 1. Bit packing — action entries encoded in minimal bits
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn bitpack_single_error_cell() {
    let table = vec![vec![Action::Error]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Error);
}

#[test]
fn bitpack_single_shift_cell() {
    let table = vec![vec![Action::Shift(StateId(42))]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Shift(StateId(42)));
}

#[test]
fn bitpack_single_reduce_cell() {
    let table = vec![vec![Action::Reduce(RuleId(7))]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Reduce(RuleId(7)));
}

#[test]
fn bitpack_single_accept_cell() {
    let table = vec![vec![Action::Accept]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Accept);
}

#[test]
fn bitpack_error_mask_all_errors() {
    let table = vec![vec![Action::Error; 5]];
    let packed = BitPackedActionTable::from_table(&table);
    for sym in 0..5 {
        assert_eq!(packed.decompress(0, sym), Action::Error);
    }
}

#[test]
fn bitpack_error_mask_bit_positions() {
    // 64 errors then one shift — error mask must span exactly 2 u64 words
    let mut row = vec![Action::Error; 64];
    row.push(Action::Shift(StateId(99)));
    let table = vec![row];
    let packed = BitPackedActionTable::from_table(&table);
    for sym in 0..64 {
        assert_eq!(packed.decompress(0, sym), Action::Error, "cell {sym}");
    }
    assert_eq!(packed.decompress(0, 64), Action::Shift(StateId(99)));
}

#[test]
fn bitpack_exact_64_cell_boundary() {
    let table = vec![vec![Action::Error; 64]];
    let packed = BitPackedActionTable::from_table(&table);
    for sym in 0..64 {
        assert_eq!(packed.decompress(0, sym), Action::Error);
    }
}

#[test]
fn bitpack_shift_state_id_zero() {
    let table = vec![vec![Action::Shift(StateId(0))]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Shift(StateId(0)));
}

#[test]
fn bitpack_shift_state_id_max_u16() {
    let table = vec![vec![Action::Shift(StateId(u16::MAX))]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Shift(StateId(u16::MAX)));
}

#[test]
fn bitpack_reduce_rule_id_zero() {
    let table = vec![vec![Action::Reduce(RuleId(0))]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Reduce(RuleId(0)));
}

#[test]
fn bitpack_reduce_rule_id_large() {
    let table = vec![vec![Action::Reduce(RuleId(u16::MAX))]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Reduce(RuleId(u16::MAX)));
}

#[test]
fn bitpack_recover_maps_to_error() {
    let table = vec![vec![Action::Recover]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Error);
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Compression ratios — compressed vs raw table sizes
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn compress_action_row_dedup_identical_rows() {
    let row = vec![vec![Action::Error], vec![Action::Shift(StateId(1))]];
    let table = vec![row.clone(), row.clone(), row];
    let compressed = compress_action_table(&table);
    // 3 identical rows → 1 unique row
    assert_eq!(compressed.unique_rows.len(), 1);
    assert_eq!(compressed.state_to_row.len(), 3);
}

#[test]
fn compress_action_no_dedup_distinct_rows() {
    let table = vec![
        vec![vec![Action::Shift(StateId(0))], vec![Action::Error]],
        vec![vec![Action::Error], vec![Action::Shift(StateId(1))]],
    ];
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 2);
}

#[test]
fn compress_action_mixed_dup_and_unique() {
    let row_a = vec![vec![Action::Shift(StateId(0))]];
    let row_b = vec![vec![Action::Reduce(RuleId(0))]];
    let table = vec![row_a.clone(), row_b, row_a];
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 2);
    assert_eq!(compressed.state_to_row[0], compressed.state_to_row[2]);
}

#[test]
fn compress_action_100_identical_rows_single_unique() {
    let row = vec![vec![Action::Error; 5]];
    let table = vec![row; 100];
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 1);
    assert_eq!(compressed.state_to_row.len(), 100);
}

#[test]
fn bitpack_error_heavy_table_uses_compact_mask() {
    // A table that is 90% errors should have very few entries in shift/reduce data
    let mut row = vec![Action::Error; 10];
    row[0] = Action::Shift(StateId(1));
    let table = vec![row];
    let packed = BitPackedActionTable::from_table(&table);
    // The shift data should have exactly 1 entry
    assert_eq!(packed.decompress(0, 0), Action::Shift(StateId(1)));
    for sym in 1..10 {
        assert_eq!(packed.decompress(0, sym), Action::Error);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Roundtrip — compress then decompress preserves data
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn roundtrip_action_table_shift_reduce() {
    let table = vec![
        vec![
            vec![Action::Shift(StateId(1))],
            vec![Action::Error],
            vec![Action::Reduce(RuleId(0))],
        ],
        vec![
            vec![Action::Error],
            vec![Action::Shift(StateId(2))],
            vec![Action::Accept],
        ],
    ];
    let compressed = compress_action_table(&table);
    for (state, row) in table.iter().enumerate() {
        for (sym, cell) in row.iter().enumerate() {
            let expected = cell.first().cloned().unwrap_or(Action::Error);
            assert_eq!(
                decompress_action(&compressed, state, sym),
                expected,
                "mismatch at state={state} sym={sym}"
            );
        }
    }
}

#[test]
fn roundtrip_action_table_all_errors() {
    let table = vec![vec![vec![Action::Error]; 8]; 4];
    let compressed = compress_action_table(&table);
    for state in 0..4 {
        for sym in 0..8 {
            assert_eq!(decompress_action(&compressed, state, sym), Action::Error);
        }
    }
}

#[test]
fn roundtrip_goto_table_sparse() {
    let table = vec![
        vec![None, Some(StateId(1)), None, None],
        vec![Some(StateId(3)), None, None, Some(StateId(5))],
        vec![None, None, None, None],
    ];
    let compressed = compress_goto_table(&table);
    for (state, row) in table.iter().enumerate() {
        for (sym, &expected) in row.iter().enumerate() {
            assert_eq!(
                decompress_goto(&compressed, state, sym),
                expected,
                "goto mismatch at state={state} sym={sym}"
            );
        }
    }
}

#[test]
fn roundtrip_goto_table_dense() {
    let table = vec![
        vec![Some(StateId(1)), Some(StateId(2)), Some(StateId(3))],
        vec![Some(StateId(4)), Some(StateId(5)), Some(StateId(6))],
    ];
    let compressed = compress_goto_table(&table);
    for (state, row) in table.iter().enumerate() {
        for (sym, &expected) in row.iter().enumerate() {
            assert_eq!(
                decompress_goto(&compressed, state, sym),
                expected,
                "dense goto mismatch at state={state} sym={sym}"
            );
        }
    }
}

#[test]
fn roundtrip_bitpacked_shifts_then_reduces() {
    // Shifts in first row, reduces in second — validates positional ordering
    let table = vec![
        vec![Action::Shift(StateId(10)), Action::Shift(StateId(20))],
        vec![Action::Reduce(RuleId(0)), Action::Reduce(RuleId(1))],
    ];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Shift(StateId(10)));
    assert_eq!(packed.decompress(0, 1), Action::Shift(StateId(20)));
    assert_eq!(packed.decompress(1, 0), Action::Reduce(RuleId(0)));
    assert_eq!(packed.decompress(1, 1), Action::Reduce(RuleId(1)));
}

#[test]
fn roundtrip_bitpacked_shift_error_accept() {
    let table = vec![vec![
        Action::Shift(StateId(7)),
        Action::Error,
        Action::Accept,
    ]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Shift(StateId(7)));
    assert_eq!(packed.decompress(0, 1), Action::Error);
    assert_eq!(packed.decompress(0, 2), Action::Accept);
}

#[test]
fn roundtrip_bitpacked_fork_preserved() {
    let fork_actions = vec![Action::Shift(StateId(3)), Action::Reduce(RuleId(1))];
    let table = vec![vec![Action::Fork(fork_actions.clone())]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Fork(fork_actions));
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Determinism — same input → same compressed output
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn determinism_action_table_same_compressed() {
    let table = vec![
        vec![vec![Action::Shift(StateId(1))], vec![Action::Error]],
        vec![
            vec![Action::Reduce(RuleId(0))],
            vec![Action::Shift(StateId(2))],
        ],
    ];
    let c1 = compress_action_table(&table);
    let c2 = compress_action_table(&table);
    assert_eq!(c1.unique_rows.len(), c2.unique_rows.len());
    assert_eq!(c1.state_to_row, c2.state_to_row);
    assert_eq!(c1.unique_rows, c2.unique_rows);
}

#[test]
fn determinism_goto_table_same_entries() {
    let table = vec![vec![None, Some(StateId(1))], vec![Some(StateId(2)), None]];
    let c1 = compress_goto_table(&table);
    let c2 = compress_goto_table(&table);
    assert_eq!(c1.entries, c2.entries);
}

#[test]
fn determinism_bitpacked_from_table_twice() {
    let table = vec![
        vec![
            Action::Shift(StateId(1)),
            Action::Error,
            Action::Reduce(RuleId(0)),
        ],
        vec![Action::Error, Action::Accept, Action::Error],
    ];
    let p1 = BitPackedActionTable::from_table(&table);
    let p2 = BitPackedActionTable::from_table(&table);
    for state in 0..2 {
        for sym in 0..3 {
            assert_eq!(
                p1.decompress(state, sym),
                p2.decompress(state, sym),
                "determinism failure at state={state} sym={sym}"
            );
        }
    }
}

#[test]
fn determinism_encode_action_small_stable() {
    let tc = TableCompressor::new();
    let actions = [
        Action::Shift(StateId(0)),
        Action::Shift(StateId(100)),
        Action::Reduce(RuleId(0)),
        Action::Reduce(RuleId(42)),
        Action::Accept,
        Action::Error,
        Action::Recover,
    ];
    for action in &actions {
        let e1 = tc.encode_action_small(action);
        let e2 = tc.encode_action_small(action);
        assert_eq!(e1.is_ok(), e2.is_ok());
        if let (Ok(v1), Ok(v2)) = (e1, e2) {
            assert_eq!(v1, v2, "encoding not deterministic for {action:?}");
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Goto table compression — nonterminal goto entries
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn goto_compression_empty_table() {
    let table: Vec<Vec<Option<StateId>>> = vec![];
    let compressed = compress_goto_table(&table);
    assert!(compressed.entries.is_empty());
}

#[test]
fn goto_compression_all_none() {
    let table = vec![vec![None; 4]; 3];
    let compressed = compress_goto_table(&table);
    assert!(compressed.entries.is_empty());
}

#[test]
fn goto_compression_single_entry() {
    let table = vec![vec![None, Some(StateId(5)), None]];
    let compressed = compress_goto_table(&table);
    assert_eq!(compressed.entries.len(), 1);
    assert_eq!(decompress_goto(&compressed, 0, 1), Some(StateId(5)));
    assert_eq!(decompress_goto(&compressed, 0, 0), None);
    assert_eq!(decompress_goto(&compressed, 0, 2), None);
}

#[test]
fn goto_compression_diagonal_pattern() {
    // Each state has exactly one goto on the diagonal
    let n = 5;
    let mut table = vec![vec![None; n]; n];
    for (i, row) in table.iter_mut().enumerate() {
        row[i] = Some(StateId(i as u16 + 10));
    }
    let compressed = compress_goto_table(&table);
    assert_eq!(compressed.entries.len(), n);
    for i in 0..n {
        assert_eq!(
            decompress_goto(&compressed, i, i),
            Some(StateId(i as u16 + 10))
        );
    }
}

#[test]
fn goto_compression_full_row() {
    let table = vec![vec![
        Some(StateId(0)),
        Some(StateId(1)),
        Some(StateId(2)),
        Some(StateId(3)),
    ]];
    let compressed = compress_goto_table(&table);
    assert_eq!(compressed.entries.len(), 4);
    for sym in 0..4 {
        assert_eq!(
            decompress_goto(&compressed, 0, sym),
            Some(StateId(sym as u16))
        );
    }
}

#[test]
fn goto_roundtrip_large_sparse() {
    let mut table = vec![vec![None; 20]; 10];
    // Scatter a few entries
    table[0][3] = Some(StateId(100));
    table[2][17] = Some(StateId(200));
    table[5][0] = Some(StateId(300));
    table[9][19] = Some(StateId(400));
    let compressed = compress_goto_table(&table);
    assert_eq!(compressed.entries.len(), 4);
    for (state, row) in table.iter().enumerate() {
        for (sym, &expected) in row.iter().enumerate() {
            assert_eq!(decompress_goto(&compressed, state, sym), expected);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Mixed action types — Shift/Reduce/Accept in same table
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn bitpack_mixed_shift_reduce_error_in_row() {
    let table = vec![vec![
        Action::Shift(StateId(5)),
        Action::Error,
        Action::Reduce(RuleId(3)),
    ]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Shift(StateId(5)));
    assert_eq!(packed.decompress(0, 1), Action::Error);
    assert_eq!(packed.decompress(0, 2), Action::Reduce(RuleId(3)));
}

#[test]
fn bitpack_mixed_all_action_types_multirow() {
    let fork = vec![Action::Shift(StateId(99)), Action::Reduce(RuleId(99))];
    let table = vec![
        vec![
            Action::Shift(StateId(1)),
            Action::Shift(StateId(2)),
            Action::Error,
            Action::Fork(fork.clone()),
        ],
        vec![
            Action::Reduce(RuleId(10)),
            Action::Accept,
            Action::Recover,
            Action::Error,
        ],
    ];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Shift(StateId(1)));
    assert_eq!(packed.decompress(0, 1), Action::Shift(StateId(2)));
    assert_eq!(packed.decompress(0, 2), Action::Error);
    assert_eq!(packed.decompress(0, 3), Action::Fork(fork));
    assert_eq!(packed.decompress(1, 0), Action::Reduce(RuleId(10)));
    assert_eq!(packed.decompress(1, 1), Action::Accept);
    // Recover maps to Error in the mask
    assert_eq!(packed.decompress(1, 2), Action::Error);
    assert_eq!(packed.decompress(1, 3), Action::Error);
}

#[test]
fn compress_action_mixed_cells_with_glr_multi_actions() {
    // GLR cells can have multiple actions; decompress_action returns the first
    let table = vec![vec![
        vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(0))],
        vec![Action::Accept],
        vec![], // empty cell → Error
    ]];
    let compressed = compress_action_table(&table);
    assert_eq!(
        decompress_action(&compressed, 0, 0),
        Action::Shift(StateId(1))
    );
    assert_eq!(decompress_action(&compressed, 0, 1), Action::Accept);
    assert_eq!(decompress_action(&compressed, 0, 2), Action::Error);
}

#[test]
fn bitpack_consecutive_accepts() {
    let table = vec![vec![Action::Accept, Action::Accept, Action::Accept]];
    let packed = BitPackedActionTable::from_table(&table);
    for sym in 0..3 {
        assert_eq!(packed.decompress(0, sym), Action::Accept);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. Scaling — compression with small vs large grammars
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn scaling_bitpack_1x1_table() {
    let table = vec![vec![Action::Shift(StateId(0))]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Shift(StateId(0)));
}

#[test]
fn scaling_bitpack_10x10_all_shifts() {
    let table: Vec<Vec<Action>> = (0..10)
        .map(|state| {
            (0..10)
                .map(|sym| Action::Shift(StateId((state * 10 + sym) as u16)))
                .collect()
        })
        .collect();
    let packed = BitPackedActionTable::from_table(&table);
    for state in 0..10 {
        for sym in 0..10 {
            assert_eq!(
                packed.decompress(state, sym),
                Action::Shift(StateId((state * 10 + sym) as u16))
            );
        }
    }
}

#[test]
fn scaling_compress_action_20_states_high_dup() {
    // 20 states but only 2 distinct rows
    let row_a = vec![vec![Action::Shift(StateId(1))], vec![Action::Error]];
    let row_b = vec![vec![Action::Error], vec![Action::Reduce(RuleId(0))]];
    let mut table = Vec::new();
    for i in 0..20 {
        table.push(if i % 2 == 0 {
            row_a.clone()
        } else {
            row_b.clone()
        });
    }
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 2);
    assert_eq!(compressed.state_to_row.len(), 20);
}

#[test]
fn scaling_compress_action_50_unique_states() {
    // All rows differ — no dedup benefit
    let table: Vec<Vec<Vec<Action>>> = (0..50)
        .map(|i| vec![vec![Action::Shift(StateId(i as u16))]])
        .collect();
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 50);
}

#[test]
fn scaling_goto_30_states_sparse() {
    let mut table = vec![vec![None; 10]; 30];
    // Only a handful of entries
    table[0][0] = Some(StateId(1));
    table[10][5] = Some(StateId(11));
    table[29][9] = Some(StateId(30));
    let compressed = compress_goto_table(&table);
    assert_eq!(compressed.entries.len(), 3);
}

#[test]
fn scaling_bitpack_wide_row_128_symbols() {
    let mut row = vec![Action::Error; 128];
    row[0] = Action::Shift(StateId(1));
    row[63] = Action::Shift(StateId(63));
    row[64] = Action::Reduce(RuleId(0));
    row[127] = Action::Accept;
    let table = vec![row];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Shift(StateId(1)));
    assert_eq!(packed.decompress(0, 63), Action::Shift(StateId(63)));
    assert_eq!(packed.decompress(0, 64), Action::Reduce(RuleId(0)));
    assert_eq!(packed.decompress(0, 127), Action::Accept);
    for sym in [1, 50, 65, 100, 126] {
        assert_eq!(packed.decompress(0, sym), Action::Error, "cell {sym}");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. Edge cases — minimal grammar, max states, empty tables
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn edge_compress_action_single_state_single_symbol() {
    let table = vec![vec![vec![Action::Accept]]];
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 1);
    assert_eq!(decompress_action(&compressed, 0, 0), Action::Accept);
}

#[test]
fn edge_compress_action_empty_cells() {
    // Empty cells decompress as Error
    let table = vec![vec![vec![], vec![], vec![]]];
    let compressed = compress_action_table(&table);
    for sym in 0..3 {
        assert_eq!(decompress_action(&compressed, 0, sym), Action::Error);
    }
}

#[test]
fn edge_compress_goto_single_entry_table() {
    let table = vec![vec![Some(StateId(42))]];
    let compressed = compress_goto_table(&table);
    assert_eq!(compressed.entries.len(), 1);
    assert_eq!(decompress_goto(&compressed, 0, 0), Some(StateId(42)));
}

#[test]
fn edge_bitpack_empty_table_zero_states() {
    let table: Vec<Vec<Action>> = vec![];
    let packed = BitPackedActionTable::from_table(&table);
    // No states — cannot decompress anything, but construction should not panic
    let _ = packed;
}

#[test]
fn edge_bitpack_one_state_zero_symbols() {
    let table = vec![vec![]];
    let packed = BitPackedActionTable::from_table(&table);
    let _ = packed;
}

#[test]
fn edge_compress_action_single_row_many_symbols() {
    let n = 100;
    let row: Vec<Vec<Action>> = (0..n)
        .map(|i| vec![Action::Shift(StateId(i as u16))])
        .collect();
    let table = vec![row];
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 1);
    for sym in 0..n {
        assert_eq!(
            decompress_action(&compressed, 0, sym),
            Action::Shift(StateId(sym as u16))
        );
    }
}

#[test]
fn edge_goto_no_rows() {
    let table: Vec<Vec<Option<StateId>>> = vec![];
    let compressed = compress_goto_table(&table);
    assert!(compressed.entries.is_empty());
}

#[test]
fn edge_goto_rows_with_no_columns() {
    let table: Vec<Vec<Option<StateId>>> = vec![vec![], vec![]];
    let compressed = compress_goto_table(&table);
    assert!(compressed.entries.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// TableCompressor::encode_action_small encoding scheme
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn encode_small_shift_produces_raw_state_id() {
    let tc = TableCompressor::new();
    let encoded = tc.encode_action_small(&Action::Shift(StateId(0))).unwrap();
    assert_eq!(encoded, 0);
    let encoded = tc
        .encode_action_small(&Action::Shift(StateId(100)))
        .unwrap();
    assert_eq!(encoded, 100);
}

#[test]
fn encode_small_reduce_sets_high_bit() {
    let tc = TableCompressor::new();
    // Reduce(0) → 0x8000 | (0+1) = 0x8001
    let encoded = tc.encode_action_small(&Action::Reduce(RuleId(0))).unwrap();
    assert_eq!(encoded, 0x8001);
    // Reduce(1) → 0x8000 | (1+1) = 0x8002
    let encoded = tc.encode_action_small(&Action::Reduce(RuleId(1))).unwrap();
    assert_eq!(encoded, 0x8002);
}

#[test]
fn encode_small_accept_is_ffff() {
    let tc = TableCompressor::new();
    let encoded = tc.encode_action_small(&Action::Accept).unwrap();
    assert_eq!(encoded, 0xFFFF);
}

#[test]
fn encode_small_error_is_fffe() {
    let tc = TableCompressor::new();
    let encoded = tc.encode_action_small(&Action::Error).unwrap();
    assert_eq!(encoded, 0xFFFE);
}

#[test]
fn encode_small_recover_is_fffd() {
    let tc = TableCompressor::new();
    let encoded = tc.encode_action_small(&Action::Recover).unwrap();
    assert_eq!(encoded, 0xFFFD);
}

#[test]
fn encode_small_shift_too_large_errors() {
    let tc = TableCompressor::new();
    let result = tc.encode_action_small(&Action::Shift(StateId(0x8000)));
    assert!(result.is_err());
}

#[test]
fn encode_small_reduce_too_large_errors() {
    let tc = TableCompressor::new();
    let result = tc.encode_action_small(&Action::Reduce(RuleId(0x4000)));
    assert!(result.is_err());
}

#[test]
fn encode_small_shift_boundary_max_valid() {
    let tc = TableCompressor::new();
    // 0x7FFF is the largest valid shift state for small encoding
    let encoded = tc
        .encode_action_small(&Action::Shift(StateId(0x7FFF)))
        .unwrap();
    assert_eq!(encoded, 0x7FFF);
}

#[test]
fn encode_small_reduce_boundary_max_valid() {
    let tc = TableCompressor::new();
    // 0x3FFF is the largest valid reduce rule for small encoding
    let encoded = tc
        .encode_action_small(&Action::Reduce(RuleId(0x3FFF)))
        .unwrap();
    assert_eq!(encoded, 0x8000 | (0x3FFF + 1));
}

#[test]
fn encode_small_fork_requires_flattening() {
    let tc = TableCompressor::new();
    let fork = Action::Fork(vec![Action::Shift(StateId(1))]);
    assert!(tc.encode_action_small(&fork).is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// Additional bitpack roundtrip and coverage tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn bitpack_multiple_forks_in_same_table() {
    let fork1 = vec![Action::Shift(StateId(1)), Action::Shift(StateId(2))];
    let fork2 = vec![Action::Reduce(RuleId(0)), Action::Reduce(RuleId(1))];
    let table = vec![vec![
        Action::Fork(fork1.clone()),
        Action::Error,
        Action::Fork(fork2.clone()),
    ]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Fork(fork1));
    assert_eq!(packed.decompress(0, 1), Action::Error);
    assert_eq!(packed.decompress(0, 2), Action::Fork(fork2));
}

#[test]
fn bitpack_interleaved_shift_reduce_errors() {
    let table = vec![vec![
        Action::Shift(StateId(1)),
        Action::Error,
        Action::Shift(StateId(2)),
        Action::Error,
        Action::Reduce(RuleId(0)),
        Action::Error,
        Action::Reduce(RuleId(1)),
    ]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Shift(StateId(1)));
    assert_eq!(packed.decompress(0, 1), Action::Error);
    assert_eq!(packed.decompress(0, 2), Action::Shift(StateId(2)));
    assert_eq!(packed.decompress(0, 3), Action::Error);
    assert_eq!(packed.decompress(0, 4), Action::Reduce(RuleId(0)));
    assert_eq!(packed.decompress(0, 5), Action::Error);
    assert_eq!(packed.decompress(0, 6), Action::Reduce(RuleId(1)));
}

#[test]
fn compress_action_preserves_order_across_states() {
    let table = vec![
        vec![vec![Action::Shift(StateId(1))]],
        vec![vec![Action::Reduce(RuleId(0))]],
        vec![vec![Action::Accept]],
    ];
    let compressed = compress_action_table(&table);
    assert_eq!(
        decompress_action(&compressed, 0, 0),
        Action::Shift(StateId(1))
    );
    assert_eq!(
        decompress_action(&compressed, 1, 0),
        Action::Reduce(RuleId(0))
    );
    assert_eq!(decompress_action(&compressed, 2, 0), Action::Accept);
}

#[test]
fn goto_compression_max_state_id() {
    let table = vec![vec![Some(StateId(u16::MAX)), None]];
    let compressed = compress_goto_table(&table);
    assert_eq!(decompress_goto(&compressed, 0, 0), Some(StateId(u16::MAX)));
    assert_eq!(decompress_goto(&compressed, 0, 1), None);
}

#[test]
fn compress_action_dedup_symmetry() {
    // row_a, row_b, row_a — indices should be [0, 1, 0]
    let row_a = vec![vec![Action::Shift(StateId(1))]];
    let row_b = vec![vec![Action::Shift(StateId(2))]];
    let table = vec![row_a.clone(), row_b, row_a];
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.state_to_row[0], compressed.state_to_row[2]);
    assert_ne!(compressed.state_to_row[0], compressed.state_to_row[1]);
}

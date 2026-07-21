//! Compression v6 tests for adze-tablegen.
//!
//! 68 tests covering:
//! 1. Action table compression roundtrip (10 tests)
//! 2. Goto table compression roundtrip (8 tests)
//! 3. Compression reduces size vs naive (8 tests)
//! 4. BitPackedActionTable correctness (8 tests)
//! 5. encode_action_small for each variant (8 tests)
//! 6. Large tables (10+ states, 10+ symbols) (8 tests)
//! 7. Sparse tables (mostly empty/error) (8 tests)
//! 8. Edge cases: single state, single symbol, all same action (10 tests)

use adze_glr_core::Action;
use adze_ir::{RuleId, StateId};
use adze_tablegen::compress::TableCompressor;
use adze_tablegen::compression::{
    BitPackedActionTable, compress_action_table, compress_goto_table, decompress_action,
    decompress_goto,
};

// ============================================================================
// Section 1: Action table compression roundtrip (10 tests)
// ============================================================================

#[test]
fn v6_action_rt_01_all_error() {
    let table = vec![vec![vec![], vec![]], vec![vec![], vec![]]];
    let c = compress_action_table(&table);
    for s in 0..2 {
        for sym in 0..2 {
            assert_eq!(decompress_action(&c, s, sym), Action::Error);
        }
    }
}

#[test]
fn v6_action_rt_02_shift_only() {
    let table = vec![vec![
        vec![Action::Shift(StateId(1))],
        vec![Action::Shift(StateId(2))],
        vec![Action::Shift(StateId(3))],
    ]];
    let c = compress_action_table(&table);
    assert_eq!(decompress_action(&c, 0, 0), Action::Shift(StateId(1)));
    assert_eq!(decompress_action(&c, 0, 1), Action::Shift(StateId(2)));
    assert_eq!(decompress_action(&c, 0, 2), Action::Shift(StateId(3)));
}

#[test]
fn v6_action_rt_03_reduce_only() {
    let table = vec![vec![
        vec![Action::Reduce(RuleId(10))],
        vec![Action::Reduce(RuleId(20))],
    ]];
    let c = compress_action_table(&table);
    assert_eq!(decompress_action(&c, 0, 0), Action::Reduce(RuleId(10)));
    assert_eq!(decompress_action(&c, 0, 1), Action::Reduce(RuleId(20)));
}

#[test]
fn v6_action_rt_04_accept_roundtrip() {
    let table = vec![vec![vec![Action::Accept], vec![Action::Error]]];
    let c = compress_action_table(&table);
    assert_eq!(decompress_action(&c, 0, 0), Action::Accept);
    assert_eq!(decompress_action(&c, 0, 1), Action::Error);
}

#[test]
fn v6_action_rt_05_recover_roundtrip() {
    let table = vec![vec![vec![Action::Recover], vec![Action::Accept]]];
    let c = compress_action_table(&table);
    assert_eq!(decompress_action(&c, 0, 0), Action::Recover);
    assert_eq!(decompress_action(&c, 0, 1), Action::Accept);
}

#[test]
fn v6_action_rt_06_mixed_shift_reduce_accept() {
    let table = vec![
        vec![
            vec![Action::Shift(StateId(4))],
            vec![Action::Reduce(RuleId(2))],
            vec![Action::Accept],
        ],
        vec![
            vec![Action::Error],
            vec![Action::Shift(StateId(7))],
            vec![Action::Reduce(RuleId(5))],
        ],
    ];
    let c = compress_action_table(&table);
    assert_eq!(decompress_action(&c, 0, 0), Action::Shift(StateId(4)));
    assert_eq!(decompress_action(&c, 0, 1), Action::Reduce(RuleId(2)));
    assert_eq!(decompress_action(&c, 0, 2), Action::Accept);
    assert_eq!(decompress_action(&c, 1, 0), Action::Error);
    assert_eq!(decompress_action(&c, 1, 1), Action::Shift(StateId(7)));
    assert_eq!(decompress_action(&c, 1, 2), Action::Reduce(RuleId(5)));
}

#[test]
fn v6_action_rt_07_glr_multi_action_returns_first() {
    let table = vec![vec![vec![
        Action::Shift(StateId(1)),
        Action::Reduce(RuleId(0)),
    ]]];
    let c = compress_action_table(&table);
    // decompress_action returns the first action from a multi-action cell
    assert_eq!(decompress_action(&c, 0, 0), Action::Shift(StateId(1)));
}

#[test]
fn v6_action_rt_08_fork_in_cell() {
    let fork = vec![Action::Shift(StateId(2)), Action::Reduce(RuleId(3))];
    let table = vec![vec![vec![Action::Fork(fork.clone())], vec![Action::Error]]];
    let c = compress_action_table(&table);
    assert_eq!(decompress_action(&c, 0, 0), Action::Fork(fork));
    assert_eq!(decompress_action(&c, 0, 1), Action::Error);
}

#[test]
fn v6_action_rt_09_duplicate_rows_dedup() {
    let row = vec![
        vec![Action::Shift(StateId(9))],
        vec![Action::Reduce(RuleId(1))],
    ];
    let table = vec![row.clone(), row.clone(), row];
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 1);
    assert_eq!(c.state_to_row.len(), 3);
    // All three states roundtrip correctly
    for s in 0..3 {
        assert_eq!(decompress_action(&c, s, 0), Action::Shift(StateId(9)));
        assert_eq!(decompress_action(&c, s, 1), Action::Reduce(RuleId(1)));
    }
}

#[test]
fn v6_action_rt_10_many_states_roundtrip() {
    let table: Vec<Vec<Vec<Action>>> = (0u16..8)
        .map(|i| {
            vec![
                vec![Action::Shift(StateId(i))],
                vec![Action::Reduce(RuleId(i))],
            ]
        })
        .collect();
    let c = compress_action_table(&table);
    for i in 0u16..8 {
        assert_eq!(
            decompress_action(&c, i as usize, 0),
            Action::Shift(StateId(i))
        );
        assert_eq!(
            decompress_action(&c, i as usize, 1),
            Action::Reduce(RuleId(i))
        );
    }
}

// ============================================================================
// Section 2: Goto table compression roundtrip (8 tests)
// ============================================================================

#[test]
fn v6_goto_rt_01_all_none() {
    let table: Vec<Vec<Option<StateId>>> = vec![vec![None; 3]; 3];
    let c = compress_goto_table(&table);
    assert!(c.entries.is_empty());
    for s in 0..3 {
        for nt in 0..3 {
            assert_eq!(decompress_goto(&c, s, nt), None);
        }
    }
}

#[test]
fn v6_goto_rt_02_diagonal() {
    let mut table: Vec<Vec<Option<StateId>>> = vec![vec![None; 4]; 4];
    for (i, row) in table.iter_mut().enumerate() {
        row[i] = Some(StateId(i as u16));
    }
    let c = compress_goto_table(&table);
    assert_eq!(c.entries.len(), 4);
    for i in 0..4 {
        assert_eq!(decompress_goto(&c, i, i), Some(StateId(i as u16)));
        if i + 1 < 4 {
            assert_eq!(decompress_goto(&c, i, i + 1), None);
        }
    }
}

#[test]
fn v6_goto_rt_03_fully_populated() {
    let table = vec![
        vec![Some(StateId(10)), Some(StateId(11))],
        vec![Some(StateId(20)), Some(StateId(21))],
    ];
    let c = compress_goto_table(&table);
    assert_eq!(c.entries.len(), 4);
    assert_eq!(decompress_goto(&c, 0, 0), Some(StateId(10)));
    assert_eq!(decompress_goto(&c, 0, 1), Some(StateId(11)));
    assert_eq!(decompress_goto(&c, 1, 0), Some(StateId(20)));
    assert_eq!(decompress_goto(&c, 1, 1), Some(StateId(21)));
}

#[test]
fn v6_goto_rt_04_single_entry() {
    let table = vec![
        vec![None, None, None],
        vec![None, Some(StateId(42)), None],
        vec![None, None, None],
    ];
    let c = compress_goto_table(&table);
    assert_eq!(c.entries.len(), 1);
    assert_eq!(decompress_goto(&c, 1, 1), Some(StateId(42)));
    assert_eq!(decompress_goto(&c, 0, 0), None);
}

#[test]
fn v6_goto_rt_05_large_state_ids() {
    let table = vec![
        vec![Some(StateId(0)), Some(StateId(65535))],
        vec![Some(StateId(32768)), None],
    ];
    let c = compress_goto_table(&table);
    assert_eq!(decompress_goto(&c, 0, 0), Some(StateId(0)));
    assert_eq!(decompress_goto(&c, 0, 1), Some(StateId(65535)));
    assert_eq!(decompress_goto(&c, 1, 0), Some(StateId(32768)));
    assert_eq!(decompress_goto(&c, 1, 1), None);
}

#[test]
fn v6_goto_rt_06_first_column_only() {
    let table: Vec<Vec<Option<StateId>>> = (0u16..5)
        .map(|i| {
            let mut row = vec![None; 4];
            row[0] = Some(StateId(i));
            row
        })
        .collect();
    let c = compress_goto_table(&table);
    assert_eq!(c.entries.len(), 5);
    for i in 0u16..5 {
        assert_eq!(decompress_goto(&c, i as usize, 0), Some(StateId(i)));
        assert_eq!(decompress_goto(&c, i as usize, 1), None);
    }
}

#[test]
fn v6_goto_rt_07_last_column_only() {
    let table: Vec<Vec<Option<StateId>>> = (0u16..4)
        .map(|i| {
            let mut row = vec![None; 5];
            row[4] = Some(StateId(i * 10));
            row
        })
        .collect();
    let c = compress_goto_table(&table);
    assert_eq!(c.entries.len(), 4);
    for i in 0u16..4 {
        assert_eq!(decompress_goto(&c, i as usize, 4), Some(StateId(i * 10)));
        assert_eq!(decompress_goto(&c, i as usize, 0), None);
    }
}

#[test]
fn v6_goto_rt_08_state_id_zero_preserved() {
    let table = vec![vec![None, Some(StateId(0)), None]];
    let c = compress_goto_table(&table);
    assert_eq!(decompress_goto(&c, 0, 0), None);
    assert_eq!(decompress_goto(&c, 0, 1), Some(StateId(0)));
    assert_eq!(decompress_goto(&c, 0, 2), None);
}

// ============================================================================
// Section 3: Compression reduces size vs naive (8 tests)
// ============================================================================

#[test]
fn v6_size_01_duplicate_rows_reduce_count() {
    let row = vec![vec![Action::Shift(StateId(1))], vec![Action::Error]];
    let table = vec![row; 10];
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 1);
    assert!(c.unique_rows.len() < 10);
}

#[test]
fn v6_size_02_all_same_rows_maximal_dedup() {
    let row = vec![
        vec![Action::Reduce(RuleId(0))],
        vec![Action::Reduce(RuleId(0))],
        vec![Action::Reduce(RuleId(0))],
    ];
    let table = vec![row; 50];
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 1);
    assert_eq!(c.state_to_row.len(), 50);
}

#[test]
fn v6_size_03_all_unique_rows_no_dedup() {
    let table: Vec<Vec<Vec<Action>>> = (0u16..6)
        .map(|i| vec![vec![Action::Shift(StateId(i))]])
        .collect();
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 6);
}

#[test]
fn v6_size_04_sparse_goto_fewer_entries() {
    let table: Vec<Vec<Option<StateId>>> = vec![vec![None; 20]; 10];
    let c = compress_goto_table(&table);
    assert!(c.entries.is_empty());
    // 0 entries vs 200 naive cells
}

#[test]
fn v6_size_05_sparse_goto_one_entry_vs_many_cells() {
    let mut table: Vec<Vec<Option<StateId>>> = vec![vec![None; 15]; 15];
    table[7][3] = Some(StateId(99));
    let c = compress_goto_table(&table);
    assert_eq!(c.entries.len(), 1);
    let naive_cells = 15 * 15;
    assert!(c.entries.len() < naive_cells);
}

#[test]
fn v6_size_06_half_duplicate_rows() {
    let row_a = vec![vec![Action::Shift(StateId(1))], vec![Action::Error]];
    let row_b = vec![vec![Action::Error], vec![Action::Reduce(RuleId(0))]];
    let table = vec![
        row_a.clone(),
        row_b.clone(),
        row_a.clone(),
        row_b.clone(),
        row_a.clone(),
        row_b,
    ];
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 2);
    assert_eq!(c.state_to_row.len(), 6);
}

#[test]
fn v6_size_07_alternating_three_patterns() {
    let r0 = vec![vec![Action::Shift(StateId(0))]];
    let r1 = vec![vec![Action::Reduce(RuleId(0))]];
    let r2 = vec![vec![Action::Accept]];
    let table = vec![
        r0.clone(),
        r1.clone(),
        r2.clone(),
        r0.clone(),
        r1.clone(),
        r2.clone(),
        r0,
        r1,
        r2,
    ];
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 3);
    assert_eq!(c.state_to_row.len(), 9);
}

#[test]
fn v6_size_08_goto_dense_entry_count_matches() {
    let table = vec![
        vec![Some(StateId(1)), Some(StateId(2)), Some(StateId(3))],
        vec![Some(StateId(4)), Some(StateId(5)), Some(StateId(6))],
    ];
    let c = compress_goto_table(&table);
    // Dense table: all 6 cells are populated
    assert_eq!(c.entries.len(), 6);
}

// ============================================================================
// Section 4: BitPackedActionTable correctness (8 tests)
// ============================================================================

#[test]
fn v6_bitpack_01_all_error() {
    let table = vec![vec![Action::Error; 3]; 2];
    let packed = BitPackedActionTable::from_table(&table);
    for s in 0..2 {
        for sym in 0..3 {
            assert_eq!(packed.decompress(s, sym), Action::Error);
        }
    }
}

#[test]
fn v6_bitpack_02_shift_sequence() {
    let table = vec![vec![
        Action::Shift(StateId(10)),
        Action::Shift(StateId(20)),
        Action::Shift(StateId(30)),
    ]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Shift(StateId(10)));
    assert_eq!(packed.decompress(0, 1), Action::Shift(StateId(20)));
    assert_eq!(packed.decompress(0, 2), Action::Shift(StateId(30)));
}

#[test]
fn v6_bitpack_03_reduce_sequence() {
    let table = vec![vec![Action::Reduce(RuleId(0)), Action::Reduce(RuleId(1))]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Reduce(RuleId(0)));
    assert_eq!(packed.decompress(0, 1), Action::Reduce(RuleId(1)));
}

#[test]
fn v6_bitpack_04_accept_encoded() {
    let table = vec![vec![Action::Shift(StateId(1)), Action::Accept]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Shift(StateId(1)));
    assert_eq!(packed.decompress(0, 1), Action::Accept);
}

#[test]
fn v6_bitpack_05_recover_as_error() {
    let table = vec![vec![Action::Recover, Action::Shift(StateId(5))]];
    let packed = BitPackedActionTable::from_table(&table);
    // Recover is stored as error in the bit-packed format
    assert_eq!(packed.decompress(0, 0), Action::Error);
    assert_eq!(packed.decompress(0, 1), Action::Shift(StateId(5)));
}

#[test]
fn v6_bitpack_06_fork_roundtrip() {
    let fork = vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(2))];
    let table = vec![vec![Action::Fork(fork.clone()), Action::Error]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Fork(fork));
    assert_eq!(packed.decompress(0, 1), Action::Error);
}

#[test]
fn v6_bitpack_07_error_then_shift() {
    let table = vec![vec![Action::Error, Action::Shift(StateId(42))]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Error);
    assert_eq!(packed.decompress(0, 1), Action::Shift(StateId(42)));
}

#[test]
fn v6_bitpack_08_multi_row_shift_reduce() {
    let table = vec![
        vec![
            Action::Shift(StateId(1)),
            Action::Error,
            Action::Shift(StateId(2)),
        ],
        vec![Action::Error, Action::Reduce(RuleId(0)), Action::Error],
    ];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Shift(StateId(1)));
    assert_eq!(packed.decompress(0, 1), Action::Error);
    assert_eq!(packed.decompress(0, 2), Action::Shift(StateId(2)));
    assert_eq!(packed.decompress(1, 0), Action::Error);
    assert_eq!(packed.decompress(1, 1), Action::Reduce(RuleId(0)));
    assert_eq!(packed.decompress(1, 2), Action::Error);
}

// ============================================================================
// Section 5: encode_action_small for each variant (8 tests)
// ============================================================================

#[test]
fn v6_encode_01_shift() {
    let tc = TableCompressor::new();
    let encoded = tc.encode_action_small(&Action::Shift(StateId(5))).unwrap();
    assert_eq!(encoded, 5);
}

#[test]
fn v6_encode_02_shift_zero() {
    let tc = TableCompressor::new();
    let encoded = tc.encode_action_small(&Action::Shift(StateId(0))).unwrap();
    assert_eq!(encoded, 0);
}

#[test]
fn v6_encode_03_reduce() {
    let tc = TableCompressor::new();
    let encoded = tc.encode_action_small(&Action::Reduce(RuleId(0))).unwrap();
    // bit 15 set, 1-based rule ID
    assert_eq!(encoded, 0x8000 | 0x0001);
}

#[test]
fn v6_encode_04_reduce_larger_rule() {
    let tc = TableCompressor::new();
    let encoded = tc.encode_action_small(&Action::Reduce(RuleId(99))).unwrap();
    assert_eq!(encoded, 0x8000 | 0x0064);
}

#[test]
fn v6_encode_05_accept() {
    let tc = TableCompressor::new();
    let encoded = tc.encode_action_small(&Action::Accept).unwrap();
    assert_eq!(encoded, 0xFFFF);
}

#[test]
fn v6_encode_06_error() {
    let tc = TableCompressor::new();
    let encoded = tc.encode_action_small(&Action::Error).unwrap();
    assert_eq!(encoded, 0xFFFE);
}

#[test]
fn v6_encode_07_recover() {
    let tc = TableCompressor::new();
    let encoded = tc.encode_action_small(&Action::Recover).unwrap();
    assert_eq!(encoded, 0xFFFD);
}

#[test]
fn v6_encode_08_fork_requires_flattening() {
    let tc = TableCompressor::new();
    let fork = Action::Fork(vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(0))]);
    assert!(tc.encode_action_small(&fork).is_err());
}

// ============================================================================
// Section 6: Large tables (10+ states, 10+ symbols) (8 tests)
// ============================================================================

#[test]
fn v6_large_01_action_12x12_roundtrip() {
    let table: Vec<Vec<Vec<Action>>> = (0u16..12)
        .map(|s| {
            (0u16..12)
                .map(|sym| {
                    if (s + sym) % 3 == 0 {
                        vec![Action::Shift(StateId(s + sym))]
                    } else {
                        vec![]
                    }
                })
                .collect()
        })
        .collect();
    let c = compress_action_table(&table);
    for s in 0u16..12 {
        for sym in 0u16..12 {
            let expected = if (s + sym) % 3 == 0 {
                Action::Shift(StateId(s + sym))
            } else {
                Action::Error
            };
            assert_eq!(decompress_action(&c, s as usize, sym as usize), expected);
        }
    }
}

#[test]
fn v6_large_02_goto_15x12_roundtrip() {
    let table: Vec<Vec<Option<StateId>>> = (0u16..15)
        .map(|s| {
            (0u16..12)
                .map(|nt| if s == nt { Some(StateId(s)) } else { None })
                .collect()
        })
        .collect();
    let c = compress_goto_table(&table);
    for s in 0u16..15 {
        for nt in 0u16..12 {
            let expected = if s == nt { Some(StateId(s)) } else { None };
            assert_eq!(decompress_goto(&c, s as usize, nt as usize), expected);
        }
    }
}

#[test]
fn v6_large_03_action_20x15_shift_pattern() {
    let table: Vec<Vec<Vec<Action>>> = (0u16..20)
        .map(|s| {
            (0u16..15)
                .map(|sym| {
                    if sym < 5 {
                        vec![Action::Shift(StateId(s * 15 + sym))]
                    } else {
                        vec![]
                    }
                })
                .collect()
        })
        .collect();
    let c = compress_action_table(&table);
    for s in 0u16..20 {
        for sym in 0u16..15 {
            let expected = if sym < 5 {
                Action::Shift(StateId(s * 15 + sym))
            } else {
                Action::Error
            };
            assert_eq!(decompress_action(&c, s as usize, sym as usize), expected);
        }
    }
}

#[test]
fn v6_large_04_goto_10x10_checkerboard() {
    let table: Vec<Vec<Option<StateId>>> = (0u16..10)
        .map(|s| {
            (0u16..10)
                .map(|nt| {
                    if (s + nt) % 2 == 0 {
                        Some(StateId(s * 10 + nt))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .collect();
    let c = compress_goto_table(&table);
    for s in 0u16..10 {
        for nt in 0u16..10 {
            let expected = if (s + nt) % 2 == 0 {
                Some(StateId(s * 10 + nt))
            } else {
                None
            };
            assert_eq!(decompress_goto(&c, s as usize, nt as usize), expected);
        }
    }
}

#[test]
fn v6_large_05_action_dedup_on_large_table() {
    // 50 states, 10 symbols, but only 5 distinct rows
    let patterns: Vec<Vec<Vec<Action>>> = (0u16..5)
        .map(|p| {
            (0u16..10)
                .map(|sym| {
                    if sym == p {
                        vec![Action::Shift(StateId(p))]
                    } else {
                        vec![]
                    }
                })
                .collect()
        })
        .collect();
    let table: Vec<Vec<Vec<Action>>> = (0..50).map(|i| patterns[i % 5].clone()).collect();
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 5);
    assert_eq!(c.state_to_row.len(), 50);
}

#[test]
fn v6_large_06_bitpack_10x10_shifts() {
    let table: Vec<Vec<Action>> = (0u16..10)
        .map(|s| {
            (0u16..10)
                .map(|sym| Action::Shift(StateId(s * 10 + sym)))
                .collect()
        })
        .collect();
    let packed = BitPackedActionTable::from_table(&table);
    for s in 0u16..10 {
        for sym in 0u16..10 {
            assert_eq!(
                packed.decompress(s as usize, sym as usize),
                Action::Shift(StateId(s * 10 + sym))
            );
        }
    }
}

#[test]
fn v6_large_07_goto_20x10_sparse_roundtrip() {
    let mut table: Vec<Vec<Option<StateId>>> = vec![vec![None; 10]; 20];
    // Populate only 20 entries along first column
    for (s, row) in table.iter_mut().enumerate() {
        row[0] = Some(StateId(s as u16));
    }
    let c = compress_goto_table(&table);
    assert_eq!(c.entries.len(), 20);
    for s in 0..20 {
        assert_eq!(decompress_goto(&c, s, 0), Some(StateId(s as u16)));
        for nt in 1..10 {
            assert_eq!(decompress_goto(&c, s, nt), None);
        }
    }
}

#[test]
fn v6_large_08_encode_small_shift_range() {
    let tc = TableCompressor::new();
    // Verify a range of shift state IDs encode correctly
    for sid in [0u16, 1, 100, 1000, 0x7FFE] {
        let encoded = tc
            .encode_action_small(&Action::Shift(StateId(sid)))
            .unwrap();
        assert_eq!(encoded, sid);
    }
}

// ============================================================================
// Section 7: Sparse tables (mostly empty/error) (8 tests)
// ============================================================================

#[test]
fn v6_sparse_01_action_one_shift_rest_error() {
    let mut row: Vec<Vec<Action>> = vec![vec![]; 10];
    row[5] = vec![Action::Shift(StateId(1))];
    let table = vec![row];
    let c = compress_action_table(&table);
    for sym in 0..10 {
        let expected = if sym == 5 {
            Action::Shift(StateId(1))
        } else {
            Action::Error
        };
        assert_eq!(decompress_action(&c, 0, sym), expected);
    }
}

#[test]
fn v6_sparse_02_action_two_of_twenty_populated() {
    let mut row: Vec<Vec<Action>> = vec![vec![]; 20];
    row[3] = vec![Action::Reduce(RuleId(7))];
    row[17] = vec![Action::Accept];
    let table = vec![row];
    let c = compress_action_table(&table);
    assert_eq!(decompress_action(&c, 0, 3), Action::Reduce(RuleId(7)));
    assert_eq!(decompress_action(&c, 0, 17), Action::Accept);
    assert_eq!(decompress_action(&c, 0, 0), Action::Error);
    assert_eq!(decompress_action(&c, 0, 10), Action::Error);
}

#[test]
fn v6_sparse_03_goto_one_entry_in_large_table() {
    let mut table: Vec<Vec<Option<StateId>>> = vec![vec![None; 12]; 12];
    table[6][8] = Some(StateId(77));
    let c = compress_goto_table(&table);
    assert_eq!(c.entries.len(), 1);
    assert_eq!(decompress_goto(&c, 6, 8), Some(StateId(77)));
    assert_eq!(decompress_goto(&c, 0, 0), None);
    assert_eq!(decompress_goto(&c, 11, 11), None);
}

#[test]
fn v6_sparse_04_action_sparse_multiple_states() {
    let table: Vec<Vec<Vec<Action>>> = (0u16..8)
        .map(|s| {
            let mut row = vec![vec![]; 8];
            row[s as usize] = vec![Action::Shift(StateId(s))];
            row
        })
        .collect();
    let c = compress_action_table(&table);
    for s in 0u16..8 {
        for sym in 0u16..8 {
            let expected = if s == sym {
                Action::Shift(StateId(s))
            } else {
                Action::Error
            };
            assert_eq!(decompress_action(&c, s as usize, sym as usize), expected);
        }
    }
}

#[test]
fn v6_sparse_05_goto_scattered_entries() {
    let mut table: Vec<Vec<Option<StateId>>> = vec![vec![None; 10]; 10];
    table[0][9] = Some(StateId(1));
    table[4][5] = Some(StateId(2));
    table[9][0] = Some(StateId(3));
    let c = compress_goto_table(&table);
    assert_eq!(c.entries.len(), 3);
    assert_eq!(decompress_goto(&c, 0, 9), Some(StateId(1)));
    assert_eq!(decompress_goto(&c, 4, 5), Some(StateId(2)));
    assert_eq!(decompress_goto(&c, 9, 0), Some(StateId(3)));
}

#[test]
fn v6_sparse_06_bitpack_mostly_error() {
    let mut row = vec![Action::Error; 8];
    row[3] = Action::Shift(StateId(7));
    let table = vec![row];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 3), Action::Shift(StateId(7)));
    for sym in [0, 1, 2, 4, 5, 6, 7] {
        assert_eq!(packed.decompress(0, sym), Action::Error);
    }
}

#[test]
fn v6_sparse_07_action_empty_cells_majority() {
    // 5 states, 10 symbols, only 5 non-empty cells
    let table: Vec<Vec<Vec<Action>>> = (0u16..5)
        .map(|s| {
            let mut row = vec![vec![]; 10];
            row[s as usize * 2] = vec![Action::Reduce(RuleId(s))];
            row
        })
        .collect();
    let c = compress_action_table(&table);
    for s in 0u16..5 {
        let col = s as usize * 2;
        assert_eq!(
            decompress_action(&c, s as usize, col),
            Action::Reduce(RuleId(s))
        );
    }
}

#[test]
fn v6_sparse_08_goto_no_entries_in_large_table() {
    let table: Vec<Vec<Option<StateId>>> = vec![vec![None; 50]; 50];
    let c = compress_goto_table(&table);
    assert!(c.entries.is_empty());
}

// ============================================================================
// Section 8: Edge cases: single state, single symbol, all same action (10 tests)
// ============================================================================

#[test]
fn v6_edge_01_single_state_single_symbol_shift() {
    let table = vec![vec![vec![Action::Shift(StateId(0))]]];
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 1);
    assert_eq!(c.state_to_row.len(), 1);
    assert_eq!(decompress_action(&c, 0, 0), Action::Shift(StateId(0)));
}

#[test]
fn v6_edge_02_single_state_single_symbol_reduce() {
    let table = vec![vec![vec![Action::Reduce(RuleId(0))]]];
    let c = compress_action_table(&table);
    assert_eq!(decompress_action(&c, 0, 0), Action::Reduce(RuleId(0)));
}

#[test]
fn v6_edge_03_single_state_single_symbol_accept() {
    let table = vec![vec![vec![Action::Accept]]];
    let c = compress_action_table(&table);
    assert_eq!(decompress_action(&c, 0, 0), Action::Accept);
}

#[test]
fn v6_edge_04_single_goto_cell_some() {
    let table = vec![vec![Some(StateId(7))]];
    let c = compress_goto_table(&table);
    assert_eq!(c.entries.len(), 1);
    assert_eq!(decompress_goto(&c, 0, 0), Some(StateId(7)));
}

#[test]
fn v6_edge_05_single_goto_cell_none() {
    let table = vec![vec![None]];
    let c = compress_goto_table(&table);
    assert!(c.entries.is_empty());
    assert_eq!(decompress_goto(&c, 0, 0), None);
}

#[test]
fn v6_edge_06_empty_action_table() {
    let table: Vec<Vec<Vec<Action>>> = vec![];
    let c = compress_action_table(&table);
    assert!(c.unique_rows.is_empty());
    assert!(c.state_to_row.is_empty());
}

#[test]
fn v6_edge_07_empty_goto_table() {
    let table: Vec<Vec<Option<StateId>>> = vec![];
    let c = compress_goto_table(&table);
    assert!(c.entries.is_empty());
}

#[test]
fn v6_edge_08_all_same_shift_action() {
    let table: Vec<Vec<Vec<Action>>> = vec![vec![vec![Action::Shift(StateId(1))]; 4]; 4];
    let c = compress_action_table(&table);
    // All rows identical → one unique row
    assert_eq!(c.unique_rows.len(), 1);
    for s in 0..4 {
        for sym in 0..4 {
            assert_eq!(decompress_action(&c, s, sym), Action::Shift(StateId(1)));
        }
    }
}

#[test]
fn v6_edge_09_all_same_reduce_action() {
    let table: Vec<Vec<Vec<Action>>> = vec![vec![vec![Action::Reduce(RuleId(99))]; 3]; 5];
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 1);
    assert_eq!(c.state_to_row.len(), 5);
    for s in 0..5 {
        for sym in 0..3 {
            assert_eq!(decompress_action(&c, s, sym), Action::Reduce(RuleId(99)));
        }
    }
}

#[test]
fn v6_edge_10_bitpack_empty_table() {
    let table: Vec<Vec<Action>> = vec![];
    let packed = BitPackedActionTable::from_table(&table);
    // Construction with empty table must not panic
    let _ = packed;
}

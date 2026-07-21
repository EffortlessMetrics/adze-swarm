//! Comprehensive encoding/decoding edge-case tests for adze-tablegen.
//!
//! 40+ tests covering:
//! - Edge cases in table encoding (empty tables, single entry, max values)
//! - Compression roundtrip (encode then decode yields original)
//! - Boundary conditions (zero-width entries, maximum symbol IDs)
//! - Error handling for malformed input
//! - Determinism (same input always produces same output)
//! - State count edge cases

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

/// Wrap each flat action into a single-element GLR cell (empty for Error).
fn to_glr(rows: Vec<Vec<Action>>) -> Vec<Vec<Vec<Action>>> {
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

/// Assert action compression roundtrip for every cell.
fn roundtrip_action(table: &[Vec<Vec<Action>>]) {
    let c = compress_action_table(table);
    for (st, row) in table.iter().enumerate() {
        for (sym, cell) in row.iter().enumerate() {
            let expected = cell.first().cloned().unwrap_or(Action::Error);
            let got = decompress_action(&c, st, sym);
            assert_eq!(
                got, expected,
                "action roundtrip fail at state={st} sym={sym}"
            );
        }
    }
}

/// Assert goto compression roundtrip for every cell.
fn roundtrip_goto(table: &[Vec<Option<StateId>>]) {
    let c = compress_goto_table(table);
    for (st, row) in table.iter().enumerate() {
        for (sym, &expected) in row.iter().enumerate() {
            let got = decompress_goto(&c, st, sym);
            assert_eq!(got, expected, "goto roundtrip fail at state={st} sym={sym}");
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. encode_action_small: Shift state=0
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn encode_shift_state_zero() {
    let tc = TableCompressor::new();
    let encoded = tc.encode_action_small(&Action::Shift(StateId(0))).unwrap();
    assert_eq!(encoded, 0, "Shift(0) should encode as 0");
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. encode_action_small: Shift state=1
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn encode_shift_state_one() {
    let tc = TableCompressor::new();
    let encoded = tc.encode_action_small(&Action::Shift(StateId(1))).unwrap();
    assert_eq!(encoded, 1);
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. encode_action_small: Shift at boundary 0x7FFF (valid)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn encode_shift_at_0x7fff_valid() {
    let tc = TableCompressor::new();
    let result = tc.encode_action_small(&Action::Shift(StateId(0x7FFF)));
    assert!(
        result.is_ok(),
        "0x7FFF is less than 0x8000 so it should be valid"
    );
    assert_eq!(result.unwrap(), 0x7FFF);
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. encode_action_small: Shift at boundary 0x8000 (rejected)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn encode_shift_at_0x8000_rejected() {
    let tc = TableCompressor::new();
    let result = tc.encode_action_small(&Action::Shift(StateId(0x8000)));
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. encode_action_small: Shift at u16::MAX (rejected)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn encode_shift_at_u16_max_rejected() {
    let tc = TableCompressor::new();
    let result = tc.encode_action_small(&Action::Shift(StateId(u16::MAX)));
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. encode_action_small: Reduce rule=0 (1-based encoding)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn encode_reduce_rule_zero_one_based() {
    let tc = TableCompressor::new();
    let encoded = tc.encode_action_small(&Action::Reduce(RuleId(0))).unwrap();
    // 0x8000 | (0 + 1) = 0x8001
    assert_eq!(encoded, 0x8001);
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. encode_action_small: Reduce rule=1
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn encode_reduce_rule_one() {
    let tc = TableCompressor::new();
    let encoded = tc.encode_action_small(&Action::Reduce(RuleId(1))).unwrap();
    // 0x8000 | (1 + 1) = 0x8002
    assert_eq!(encoded, 0x8002);
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. encode_action_small: Reduce at boundary 0x3FFE (valid)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn encode_reduce_at_max_valid() {
    let tc = TableCompressor::new();
    let max_rule = 0x3FFF - 1;
    let result = tc.encode_action_small(&Action::Reduce(RuleId(max_rule)));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0x8000 | (max_rule + 1));
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. encode_action_small: Reduce at boundary 0x4000 (rejected)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn encode_reduce_at_0x4000_rejected() {
    let tc = TableCompressor::new();
    let result = tc.encode_action_small(&Action::Reduce(RuleId(0x4000)));
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// 10. encode_action_small: Accept sentinel
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn encode_accept_sentinel_value() {
    let tc = TableCompressor::new();
    let encoded = tc.encode_action_small(&Action::Accept).unwrap();
    assert_eq!(encoded, 0xFFFF);
}

// ═══════════════════════════════════════════════════════════════════════════
// 11. encode_action_small: Error sentinel
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn encode_error_sentinel_value() {
    let tc = TableCompressor::new();
    let encoded = tc.encode_action_small(&Action::Error).unwrap();
    assert_eq!(encoded, 0xFFFE);
}

// ═══════════════════════════════════════════════════════════════════════════
// 12. encode_action_small: Recover sentinel
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn encode_recover_sentinel_value() {
    let tc = TableCompressor::new();
    let encoded = tc.encode_action_small(&Action::Recover).unwrap();
    assert_eq!(encoded, 0xFFFD);
}

// ═══════════════════════════════════════════════════════════════════════════
// 13. All three sentinels are distinct
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn sentinels_are_all_distinct() {
    let tc = TableCompressor::new();
    let accept = tc.encode_action_small(&Action::Accept).unwrap();
    let error = tc.encode_action_small(&Action::Error).unwrap();
    let recover = tc.encode_action_small(&Action::Recover).unwrap();
    assert_ne!(accept, error);
    assert_ne!(accept, recover);
    assert_ne!(error, recover);
}

// ═══════════════════════════════════════════════════════════════════════════
// 14. Fork action encodes to error sentinel in small mode
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn encode_fork_requires_flattening() {
    let tc = TableCompressor::new();
    let fork = Action::Fork(vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(0))]);
    assert!(tc.encode_action_small(&fork).is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// 15. Determinism: encoding same input twice gives same output
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn encoding_is_deterministic_shift() {
    let tc = TableCompressor::new();
    let a = tc.encode_action_small(&Action::Shift(StateId(42))).unwrap();
    let b = tc.encode_action_small(&Action::Shift(StateId(42))).unwrap();
    assert_eq!(a, b);
}

#[test]
fn encoding_is_deterministic_reduce() {
    let tc = TableCompressor::new();
    let a = tc.encode_action_small(&Action::Reduce(RuleId(10))).unwrap();
    let b = tc.encode_action_small(&Action::Reduce(RuleId(10))).unwrap();
    assert_eq!(a, b);
}

// ═══════════════════════════════════════════════════════════════════════════
// 16-17. Action compression roundtrip: empty table
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn action_roundtrip_zero_states() {
    let table: Vec<Vec<Vec<Action>>> = vec![];
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 0);
    assert_eq!(c.state_to_row.len(), 0);
}

#[test]
fn action_roundtrip_single_empty_row() {
    let table = vec![vec![vec![]; 5]];
    roundtrip_action(&table);
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 1);
}

// ═══════════════════════════════════════════════════════════════════════════
// 18. Action compression roundtrip: single Shift entry
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn action_roundtrip_single_shift() {
    let table = vec![vec![vec![Action::Shift(StateId(7))]]];
    roundtrip_action(&table);
}

// ═══════════════════════════════════════════════════════════════════════════
// 19. Action compression roundtrip: Accept only
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn action_roundtrip_accept_only() {
    let table = vec![vec![vec![Action::Accept]]];
    roundtrip_action(&table);
}

// ═══════════════════════════════════════════════════════════════════════════
// 20. Action compression roundtrip: max StateId values
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn action_roundtrip_max_state_ids() {
    let table = vec![vec![
        vec![Action::Shift(StateId(u16::MAX))],
        vec![Action::Reduce(RuleId(u16::MAX))],
    ]];
    roundtrip_action(&table);
}

// ═══════════════════════════════════════════════════════════════════════════
// 21. Goto compression roundtrip: empty table
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn goto_roundtrip_empty_table() {
    let table: Vec<Vec<Option<StateId>>> = vec![];
    let c = compress_goto_table(&table);
    assert!(c.entries.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// 22. Goto compression roundtrip: all None
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn goto_roundtrip_all_none() {
    let table = vec![vec![None; 10]; 5];
    roundtrip_goto(&table);
    let c = compress_goto_table(&table);
    assert!(c.entries.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// 23. Goto compression roundtrip: all Some
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn goto_roundtrip_all_some() {
    let table: Vec<Vec<Option<StateId>>> = (0u16..4)
        .map(|s| (0u16..6).map(|sym| Some(StateId(s * 6 + sym))).collect())
        .collect();
    roundtrip_goto(&table);
    let c = compress_goto_table(&table);
    assert_eq!(c.entries.len(), 24);
}

// ═══════════════════════════════════════════════════════════════════════════
// 24. Goto compression roundtrip: max state IDs
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn goto_roundtrip_max_state_id() {
    let table = vec![vec![
        Some(StateId(u16::MAX)),
        None,
        Some(StateId(u16::MAX - 1)),
    ]];
    roundtrip_goto(&table);
}

// ═══════════════════════════════════════════════════════════════════════════
// 25. Row deduplication: identical rows collapse
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn dedup_identical_rows_collapse_to_one() {
    let row = vec![vec![Action::Shift(StateId(5))], vec![]];
    let table = vec![row.clone(), row.clone(), row];
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 1);
    assert!(c.state_to_row.iter().all(|&idx| idx == 0));
}

// ═══════════════════════════════════════════════════════════════════════════
// 26. Row deduplication: all distinct
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn dedup_all_distinct_no_collapse() {
    let table: Vec<Vec<Vec<Action>>> = (0u16..10)
        .map(|s| vec![vec![Action::Shift(StateId(s))]])
        .collect();
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 10);
}

// ═══════════════════════════════════════════════════════════════════════════
// 27. Determinism: compression output is stable
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn action_compression_deterministic() {
    let table: Vec<Vec<Vec<Action>>> = vec![
        vec![
            vec![Action::Shift(StateId(1))],
            vec![Action::Reduce(RuleId(0))],
        ],
        vec![vec![], vec![Action::Accept]],
    ];
    let c1 = compress_action_table(&table);
    let c2 = compress_action_table(&table);
    assert_eq!(c1.unique_rows.len(), c2.unique_rows.len());
    assert_eq!(c1.state_to_row, c2.state_to_row);
    for i in 0..c1.unique_rows.len() {
        assert_eq!(c1.unique_rows[i], c2.unique_rows[i]);
    }
}

#[test]
fn goto_compression_deterministic() {
    let table: Vec<Vec<Option<StateId>>> = vec![
        vec![Some(StateId(1)), None, Some(StateId(3))],
        vec![None, Some(StateId(2)), None],
    ];
    let c1 = compress_goto_table(&table);
    let c2 = compress_goto_table(&table);
    assert_eq!(c1.entries.len(), c2.entries.len());
    for (&k, &v) in &c1.entries {
        assert_eq!(c2.entries.get(&k), Some(&v));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 28. Small-table compressor: empty action rows produce no data entries
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn small_table_empty_rows_no_data() {
    let tc = TableCompressor::new();
    let action_table = vec![vec![vec![]; 10]; 3];
    let sym_map = BTreeMap::new();
    let c = tc
        .compress_action_table_small(&action_table, &sym_map)
        .unwrap();
    assert!(c.data.is_empty());
    assert_eq!(c.default_actions.len(), 3);
    assert_eq!(c.row_offsets.len(), 4); // 3 + 1
}

// ═══════════════════════════════════════════════════════════════════════════
// 29. Small-table compressor: Error actions are skipped
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn small_table_explicit_errors_skipped() {
    let tc = TableCompressor::new();
    let action_table = vec![vec![
        vec![Action::Error],
        vec![Action::Shift(StateId(2))],
        vec![Action::Error],
    ]];
    let sym_map = BTreeMap::new();
    let c = tc
        .compress_action_table_small(&action_table, &sym_map)
        .unwrap();
    assert_eq!(c.data.len(), 1);
    assert_eq!(c.data[0].symbol, 1);
    assert_eq!(c.data[0].action, Action::Shift(StateId(2)));
}

// ═══════════════════════════════════════════════════════════════════════════
// 30. Small-table compressor: default_actions always Error
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn small_table_defaults_always_error() {
    let tc = TableCompressor::new();
    let action_table = vec![
        vec![vec![Action::Accept]; 5],
        vec![vec![Action::Shift(StateId(0))]; 5],
        vec![vec![Action::Reduce(RuleId(1))]; 5],
    ];
    let sym_map = BTreeMap::new();
    let c = tc
        .compress_action_table_small(&action_table, &sym_map)
        .unwrap();
    assert_eq!(c.default_actions.len(), 3);
    for da in &c.default_actions {
        assert_eq!(*da, Action::Error);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 31. Small-table compressor: row_offsets non-decreasing
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn small_table_row_offsets_nondecreasing() {
    let tc = TableCompressor::new();
    let action_table = vec![
        vec![vec![]; 3],
        vec![vec![Action::Shift(StateId(0))]; 3],
        vec![vec![]; 3],
        vec![vec![Action::Reduce(RuleId(0)), Action::Accept]; 3],
    ];
    let sym_map = BTreeMap::new();
    let c = tc
        .compress_action_table_small(&action_table, &sym_map)
        .unwrap();
    for w in c.row_offsets.windows(2) {
        assert!(w[1] >= w[0], "row offsets must be non-decreasing");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 32. Small-table compressor: row_offsets len == states + 1
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn small_table_row_offsets_len() {
    let tc = TableCompressor::new();
    for n in [1, 5, 12, 20] {
        let action_table = vec![vec![vec![Action::Shift(StateId(0))]; 2]; n];
        let sym_map = BTreeMap::new();
        let c = tc
            .compress_action_table_small(&action_table, &sym_map)
            .unwrap();
        assert_eq!(c.row_offsets.len(), n + 1, "n_states={n}");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 33. Goto RLE: single element stays Single
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn goto_rle_single_element_is_single() {
    let tc = TableCompressor::new();
    let goto_table = vec![vec![StateId(99)]];
    let c = tc.compress_goto_table_small(&goto_table).unwrap();
    assert_eq!(c.data.len(), 1);
    assert!(matches!(c.data[0], CompressedGotoEntry::Single(99)));
}

// ═══════════════════════════════════════════════════════════════════════════
// 34. Goto RLE: run of 2 stays as two Singles
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn goto_rle_run_of_two_is_two_singles() {
    let tc = TableCompressor::new();
    let goto_table = vec![vec![StateId(4), StateId(4)]];
    let c = tc.compress_goto_table_small(&goto_table).unwrap();
    assert_eq!(c.data.len(), 2);
    assert!(
        c.data
            .iter()
            .all(|e| matches!(e, CompressedGotoEntry::Single(4)))
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 35. Goto RLE: run of 3 becomes RunLength
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn goto_rle_run_of_three_becomes_run_length() {
    let tc = TableCompressor::new();
    let goto_table = vec![vec![StateId(8), StateId(8), StateId(8)]];
    let c = tc.compress_goto_table_small(&goto_table).unwrap();
    assert_eq!(c.data.len(), 1);
    assert!(matches!(
        c.data[0],
        CompressedGotoEntry::RunLength { state: 8, count: 3 }
    ));
}

// ═══════════════════════════════════════════════════════════════════════════
// 36. Goto RLE: alternating values produce no runs
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn goto_rle_alternating_no_runs() {
    let tc = TableCompressor::new();
    let goto_table = vec![vec![StateId(1), StateId(2), StateId(1), StateId(2)]];
    let c = tc.compress_goto_table_small(&goto_table).unwrap();
    assert_eq!(c.data.len(), 4);
    assert!(
        c.data
            .iter()
            .all(|e| matches!(e, CompressedGotoEntry::Single(_)))
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 37. Goto RLE: sentinel offset equals data length
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn goto_rle_sentinel_equals_data_len() {
    let tc = TableCompressor::new();
    let goto_table = vec![
        vec![StateId(1), StateId(2), StateId(3)],
        vec![StateId(5), StateId(5), StateId(5), StateId(5)],
    ];
    let c = tc.compress_goto_table_small(&goto_table).unwrap();
    assert_eq!(*c.row_offsets.last().unwrap() as usize, c.data.len());
}

// ═══════════════════════════════════════════════════════════════════════════
// 38. Goto RLE determinism
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn goto_rle_compression_deterministic() {
    let tc = TableCompressor::new();
    let goto_table = vec![vec![
        StateId(1),
        StateId(1),
        StateId(1),
        StateId(2),
        StateId(3),
        StateId(3),
        StateId(3),
    ]];
    let c1 = tc.compress_goto_table_small(&goto_table).unwrap();
    let c2 = tc.compress_goto_table_small(&goto_table).unwrap();
    assert_eq!(c1.data.len(), c2.data.len());
    assert_eq!(c1.row_offsets, c2.row_offsets);
}

// ═══════════════════════════════════════════════════════════════════════════
// 39. CompressedParseTable: zero dimensions
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn compressed_parse_table_zero_zero() {
    let t = CompressedParseTable::new_for_testing(0, 0);
    assert_eq!(t.symbol_count(), 0);
    assert_eq!(t.state_count(), 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// 40. CompressedParseTable: large dimensions
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn compressed_parse_table_large_dims() {
    let t = CompressedParseTable::new_for_testing(100_000, 50_000);
    assert_eq!(t.symbol_count(), 100_000);
    assert_eq!(t.state_count(), 50_000);
}

// ═══════════════════════════════════════════════════════════════════════════
// 41. CompressedActionEntry preserves all action variants
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn compressed_entry_preserves_variants() {
    let actions = [
        Action::Shift(StateId(0)),
        Action::Shift(StateId(u16::MAX)),
        Action::Reduce(RuleId(0)),
        Action::Accept,
        Action::Error,
        Action::Recover,
    ];
    for (i, action) in actions.iter().enumerate() {
        let entry = CompressedActionEntry::new(i as u16, action.clone());
        assert_eq!(entry.symbol, i as u16);
        assert_eq!(entry.action, *action);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 42. Multi-action GLR cell: first action is returned
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn multi_action_cell_returns_first() {
    let table = vec![vec![vec![
        Action::Shift(StateId(10)),
        Action::Reduce(RuleId(3)),
    ]]];
    let c = compress_action_table(&table);
    assert_eq!(decompress_action(&c, 0, 0), Action::Shift(StateId(10)));
}

// ═══════════════════════════════════════════════════════════════════════════
// 43. Wide row roundtrip (250 symbols)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn action_roundtrip_250_symbols() {
    let row: Vec<Action> = (0u16..250)
        .map(|i| match i % 4 {
            0 => Action::Shift(StateId(i % 80)),
            1 => Action::Reduce(RuleId(i % 40)),
            2 => Action::Accept,
            _ => Action::Error,
        })
        .collect();
    roundtrip_action(&to_glr(vec![row]));
}

// ═══════════════════════════════════════════════════════════════════════════
// 44. Goto sparse roundtrip (300 symbols)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn goto_sparse_roundtrip_300() {
    let row: Vec<Option<StateId>> = (0u16..300)
        .map(|i| {
            if i % 11 == 0 {
                Some(StateId(i % 100))
            } else {
                None
            }
        })
        .collect();
    roundtrip_goto(&[row]);
}

// ═══════════════════════════════════════════════════════════════════════════
// 45. TableCompressor::default matches ::new
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn compressor_default_equals_new() {
    let tc1 = TableCompressor::new();
    let tc2 = TableCompressor::default();
    let table = vec![vec![vec![Action::Shift(StateId(1))]; 3]];
    let sym_map = BTreeMap::new();
    let c1 = tc1.compress_action_table_small(&table, &sym_map).unwrap();
    let c2 = tc2.compress_action_table_small(&table, &sym_map).unwrap();
    assert_eq!(c1.data.len(), c2.data.len());
    assert_eq!(c1.row_offsets, c2.row_offsets);
    assert_eq!(c1.default_actions, c2.default_actions);
}

// ═══════════════════════════════════════════════════════════════════════════
// 46. BitPacked: all-error table roundtrip
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn bitpacked_all_errors() {
    let table = vec![vec![Action::Error; 8]; 4];
    let packed = BitPackedActionTable::from_table(&table);
    for st in 0..4 {
        for sym in 0..8 {
            assert_eq!(packed.decompress(st, sym), Action::Error);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 47. BitPacked: empty table does not panic
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn bitpacked_empty_table_no_panic() {
    let table: Vec<Vec<Action>> = vec![];
    let _packed = BitPackedActionTable::from_table(&table);
}

// ═══════════════════════════════════════════════════════════════════════════
// 48. Goto RLE: large uniform row
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn goto_rle_large_uniform_row() {
    let tc = TableCompressor::new();
    let goto_table = vec![vec![StateId(42); 200]];
    let c = tc.compress_goto_table_small(&goto_table).unwrap();
    assert_eq!(c.data.len(), 1);
    assert!(matches!(
        c.data[0],
        CompressedGotoEntry::RunLength {
            state: 42,
            count: 200
        }
    ));
}

// ═══════════════════════════════════════════════════════════════════════════
// 49. Encoding sweep: all valid shift states encode correctly
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn encode_shift_sweep_low_range() {
    let tc = TableCompressor::new();
    for s in [0u16, 1, 100, 1000, 0x3FFF, 0x7FFE, 0x7FFF] {
        let encoded = tc.encode_action_small(&Action::Shift(StateId(s))).unwrap();
        assert_eq!(encoded, s, "Shift({s}) should encode as {s}");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 50. Encoding sweep: all valid reduce rules encode correctly
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn encode_reduce_sweep() {
    let tc = TableCompressor::new();
    for r in [0u16, 1, 100, 500, 0x3FFE] {
        let encoded = tc.encode_action_small(&Action::Reduce(RuleId(r))).unwrap();
        assert_eq!(encoded, 0x8000 | (r + 1), "Reduce({r}) encoding mismatch");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 51. Goto row offsets: multiple rows
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn goto_row_offsets_multi_row() {
    let tc = TableCompressor::new();
    let goto_table = vec![
        vec![StateId(1)],
        vec![StateId(2), StateId(2), StateId(2)],
        vec![StateId(3), StateId(4)],
    ];
    let c = tc.compress_goto_table_small(&goto_table).unwrap();
    assert_eq!(c.row_offsets.len(), 4); // 3 rows + 1 sentinel
    // First row starts at 0
    assert_eq!(c.row_offsets[0], 0);
    // Sentinel equals data length
    assert_eq!(*c.row_offsets.last().unwrap() as usize, c.data.len());
}

// ═══════════════════════════════════════════════════════════════════════════
// 52. Action roundtrip with Recover variant
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn action_roundtrip_recover_variant() {
    let table = to_glr(vec![vec![
        Action::Recover,
        Action::Shift(StateId(1)),
        Action::Recover,
    ]]);
    roundtrip_action(&table);
}

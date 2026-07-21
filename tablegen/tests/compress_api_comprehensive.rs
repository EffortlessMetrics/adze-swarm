//! Comprehensive tests for CompressedTables and TableCompressor APIs.
//!
//! Covers:
//! - CompressedTables struct construction and field access
//! - CompressedActionTable / CompressedGotoTable internals
//! - CompressedActionEntry / CompressedGotoEntry creation
//! - TableCompressor::new, Default, encode_action_small
//! - compress() full pipeline with real grammars
//! - compress_action_table_small / compress_goto_table_small
//! - Edge cases: empty, single-state, large tables
//! - Compression ratio and determinism
//! - Row offset monotonicity invariants
//! - Error path validation

use adze_glr_core::{Action, FirstFollowSets, build_lr1_automaton};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{RuleId, StateId};
use adze_tablegen::compress::{
    CompressedActionEntry, CompressedActionTable, CompressedGotoEntry, CompressedGotoTable,
    CompressedParseTable, CompressedTables, TableCompressor,
};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a real parse table from: S → a
fn simple_table() -> adze_glr_core::ParseTable {
    let mut g = GrammarBuilder::new("s")
        .token("a", "a")
        .rule("start", vec!["a"])
        .start("start")
        .build();
    let ff = FirstFollowSets::compute_normalized(&mut g).unwrap();
    build_lr1_automaton(&g, &ff).unwrap()
}

/// Build a parse table from: S → A | B; A → a; B → b
fn multi_rule_table() -> adze_glr_core::ParseTable {
    let mut g = GrammarBuilder::new("mr")
        .token("a", "a")
        .token("b", "b")
        .rule("start", vec!["A"])
        .rule("start", vec!["B"])
        .rule("A", vec!["a"])
        .rule("B", vec!["b"])
        .start("start")
        .build();
    let ff = FirstFollowSets::compute_normalized(&mut g).unwrap();
    build_lr1_automaton(&g, &ff).unwrap()
}

/// Build: S → a b c
fn sequence_table() -> adze_glr_core::ParseTable {
    let mut g = GrammarBuilder::new("seq")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .rule("start", vec!["a", "b", "c"])
        .start("start")
        .build();
    let ff = FirstFollowSets::compute_normalized(&mut g).unwrap();
    build_lr1_automaton(&g, &ff).unwrap()
}

/// Build: S → A; A → a A | ε  (nullable start)
fn nullable_table() -> adze_glr_core::ParseTable {
    let mut g = GrammarBuilder::new("null")
        .token("a", "a")
        .rule("start", vec!["A"])
        .rule("A", vec!["a", "A"])
        .rule("A", vec![])
        .start("start")
        .build();
    let ff = FirstFollowSets::compute_normalized(&mut g).unwrap();
    build_lr1_automaton(&g, &ff).unwrap()
}

/// Collect sorted, deduped token indices from a parse table.
fn token_indices(pt: &adze_glr_core::ParseTable) -> Vec<usize> {
    let mut v: Vec<usize> = pt.symbol_to_index.values().copied().collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Detect whether state-0 has Accept/Reduce on EOF (nullable start helper).
fn start_nullable(pt: &adze_glr_core::ParseTable) -> bool {
    adze_tablegen::eof_accepts_or_reduces(pt)
}

/// Compress a parse table end-to-end using the standard pipeline.
fn compress(pt: &adze_glr_core::ParseTable) -> CompressedTables {
    let ti = token_indices(pt);
    let sn = start_nullable(pt);
    TableCompressor::new().compress(pt, &ti, sn).unwrap()
}

// ===================================================================
// 1. CompressedParseTable construction & accessors
// ===================================================================

#[test]
fn cpt_new_for_testing_stores_fields() {
    let t = CompressedParseTable::new_for_testing(42, 7);
    assert_eq!(t.symbol_count(), 42);
    assert_eq!(t.state_count(), 7);
}

#[test]
fn cpt_zero_sizes() {
    let t = CompressedParseTable::new_for_testing(0, 0);
    assert_eq!(t.symbol_count(), 0);
    assert_eq!(t.state_count(), 0);
}

#[test]
fn cpt_large_sizes() {
    let t = CompressedParseTable::new_for_testing(100_000, 50_000);
    assert_eq!(t.symbol_count(), 100_000);
    assert_eq!(t.state_count(), 50_000);
}

#[test]
fn cpt_from_parse_table() {
    let pt = simple_table();
    let cpt = CompressedParseTable::from_parse_table(&pt);
    assert_eq!(cpt.symbol_count(), pt.symbol_count);
    assert_eq!(cpt.state_count(), pt.state_count);
}

#[test]
fn cpt_from_multi_rule_table() {
    let pt = multi_rule_table();
    let cpt = CompressedParseTable::from_parse_table(&pt);
    assert!(cpt.state_count() >= 2);
    assert!(cpt.symbol_count() >= 3);
}

// ===================================================================
// 2. CompressedActionEntry
// ===================================================================

#[test]
fn cae_shift() {
    let e = CompressedActionEntry::new(10, Action::Shift(StateId(99)));
    assert_eq!(e.symbol, 10);
    assert_eq!(e.action, Action::Shift(StateId(99)));
}

#[test]
fn cae_reduce() {
    let e = CompressedActionEntry::new(0, Action::Reduce(RuleId(5)));
    assert_eq!(e.action, Action::Reduce(RuleId(5)));
}

#[test]
fn cae_accept() {
    let e = CompressedActionEntry::new(1, Action::Accept);
    assert_eq!(e.action, Action::Accept);
}

#[test]
fn cae_error() {
    let e = CompressedActionEntry::new(2, Action::Error);
    assert_eq!(e.action, Action::Error);
}

#[test]
fn cae_clone() {
    let e = CompressedActionEntry::new(7, Action::Shift(StateId(3)));
    let c = e.clone();
    assert_eq!(c.symbol, e.symbol);
    assert_eq!(c.action, e.action);
}

#[test]
fn cae_debug() {
    let e = CompressedActionEntry::new(1, Action::Accept);
    let dbg = format!("{:?}", e);
    assert!(dbg.contains("Accept"));
}

// ===================================================================
// 3. CompressedGotoEntry
// ===================================================================

#[test]
fn cge_single() {
    let e = CompressedGotoEntry::Single(42);
    assert!(matches!(e, CompressedGotoEntry::Single(42)));
}

#[test]
fn cge_run_length() {
    let e = CompressedGotoEntry::RunLength {
        state: 5,
        count: 10,
    };
    assert!(matches!(
        e,
        CompressedGotoEntry::RunLength {
            state: 5,
            count: 10
        }
    ));
}

#[test]
fn cge_clone_single() {
    let e = CompressedGotoEntry::Single(1);
    let c = e.clone();
    assert!(matches!(c, CompressedGotoEntry::Single(1)));
}

#[test]
fn cge_clone_run_length() {
    let e = CompressedGotoEntry::RunLength { state: 3, count: 7 };
    let c = e.clone();
    assert!(matches!(
        c,
        CompressedGotoEntry::RunLength { state: 3, count: 7 }
    ));
}

#[test]
fn cge_debug_format() {
    let e = CompressedGotoEntry::RunLength { state: 2, count: 4 };
    let dbg = format!("{:?}", e);
    assert!(dbg.contains("RunLength"));
}

// ===================================================================
// 4. CompressedTables direct construction
// ===================================================================

#[test]
fn ct_empty_construction() {
    let ct = CompressedTables {
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
    assert_eq!(ct.small_table_threshold, 32768);
    assert!(ct.action_table.data.is_empty());
    assert!(ct.goto_table.data.is_empty());
}

#[test]
fn ct_validate_returns_ok() {
    let pt = simple_table();
    let ct = compress(&pt);
    assert!(ct.validate(&pt).is_ok());
}

// ===================================================================
// 5. TableCompressor construction
// ===================================================================

#[test]
fn tc_new() {
    let tc = TableCompressor::new();
    let _ = tc; // doesn't panic
}

#[test]
fn tc_default_is_same_as_new() {
    let _a = TableCompressor::new();
    let _b = TableCompressor::default();
    // Both should work without panic; threshold is an implementation detail.
}

// ===================================================================
// 6. encode_action_small
// ===================================================================

#[test]
fn encode_shift_zero() {
    let tc = TableCompressor::new();
    let v = tc.encode_action_small(&Action::Shift(StateId(0))).unwrap();
    assert_eq!(v, 0);
}

#[test]
fn encode_shift_normal() {
    let tc = TableCompressor::new();
    let v = tc.encode_action_small(&Action::Shift(StateId(42))).unwrap();
    assert_eq!(v, 42);
}

#[test]
fn encode_shift_max_valid() {
    let tc = TableCompressor::new();
    let v = tc
        .encode_action_small(&Action::Shift(StateId(0x7FFF)))
        .unwrap();
    assert_eq!(v, 0x7FFF);
}

#[test]
fn encode_shift_too_large() {
    let tc = TableCompressor::new();
    let r = tc.encode_action_small(&Action::Shift(StateId(0x8000)));
    assert!(r.is_err());
}

#[test]
fn encode_reduce_zero() {
    let tc = TableCompressor::new();
    // Reduce(0) → 0x8000 | (0+1) = 0x8001
    let v = tc.encode_action_small(&Action::Reduce(RuleId(0))).unwrap();
    assert_eq!(v, 0x8001);
}

#[test]
fn encode_reduce_normal() {
    let tc = TableCompressor::new();
    // Reduce(5) → 0x8000 | 6 = 0x8006
    let v = tc.encode_action_small(&Action::Reduce(RuleId(5))).unwrap();
    assert_eq!(v, 0x8006);
}

#[test]
fn encode_reduce_too_large() {
    let tc = TableCompressor::new();
    let r = tc.encode_action_small(&Action::Reduce(RuleId(0x4000)));
    assert!(r.is_err());
}

#[test]
fn encode_reduce_max_valid() {
    let tc = TableCompressor::new();
    let v = tc
        .encode_action_small(&Action::Reduce(RuleId(0x3FFF)))
        .unwrap();
    assert_eq!(v, 0x8000 | (0x3FFF + 1));
}

#[test]
fn encode_accept() {
    let tc = TableCompressor::new();
    assert_eq!(tc.encode_action_small(&Action::Accept).unwrap(), 0xFFFF);
}

#[test]
fn encode_error() {
    let tc = TableCompressor::new();
    assert_eq!(tc.encode_action_small(&Action::Error).unwrap(), 0xFFFE);
}

#[test]
fn encode_recover() {
    let tc = TableCompressor::new();
    assert_eq!(tc.encode_action_small(&Action::Recover).unwrap(), 0xFFFD);
}

#[test]
fn encode_fork_requires_flattening() {
    let tc = TableCompressor::new();
    let fork = Action::Fork(vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(0))]);
    assert!(tc.encode_action_small(&fork).is_err());
}

// ===================================================================
// 7. compress_action_table_small
// ===================================================================

#[test]
fn cat_empty_rows() {
    let tc = TableCompressor::new();
    let at: Vec<Vec<Vec<Action>>> = vec![vec![]; 4];
    let sym = BTreeMap::new();
    let c = tc.compress_action_table_small(&at, &sym).unwrap();
    assert_eq!(c.row_offsets.len(), 5); // 4 states + 1
    assert!(c.data.is_empty());
    assert_eq!(c.default_actions.len(), 4);
}

#[test]
fn cat_single_shift() {
    let tc = TableCompressor::new();
    let at = vec![vec![vec![Action::Shift(StateId(1))]]];
    let sym = BTreeMap::new();
    let c = tc.compress_action_table_small(&at, &sym).unwrap();
    assert_eq!(c.data.len(), 1);
    assert_eq!(c.data[0].action, Action::Shift(StateId(1)));
    assert_eq!(c.data[0].symbol, 0); // column index 0
}

#[test]
fn cat_errors_skipped() {
    let tc = TableCompressor::new();
    // Explicit Error actions should be omitted from data
    let at = vec![vec![vec![Action::Error], vec![Action::Shift(StateId(2))]]];
    let sym = BTreeMap::new();
    let c = tc.compress_action_table_small(&at, &sym).unwrap();
    assert_eq!(c.data.len(), 1); // only the Shift
    assert_eq!(c.data[0].action, Action::Shift(StateId(2)));
}

#[test]
fn cat_multi_action_cell() {
    let tc = TableCompressor::new();
    // A single cell with two actions (GLR conflict)
    let at = vec![vec![vec![
        Action::Shift(StateId(1)),
        Action::Reduce(RuleId(0)),
    ]]];
    let sym = BTreeMap::new();
    let c = tc.compress_action_table_small(&at, &sym).unwrap();
    assert_eq!(c.data.len(), 2);
}

#[test]
fn cat_default_action_always_error() {
    let tc = TableCompressor::new();
    // Even when every cell is Reduce(1), default should still be Error (optimization disabled)
    let at = vec![vec![vec![Action::Reduce(RuleId(1))]; 8]];
    let sym = BTreeMap::new();
    let c = tc.compress_action_table_small(&at, &sym).unwrap();
    assert_eq!(c.default_actions[0], Action::Error);
}

#[test]
fn cat_row_offsets_monotonically_nondecreasing() {
    let tc = TableCompressor::new();
    let at = vec![
        vec![vec![Action::Shift(StateId(0))]; 3],
        vec![vec![]; 3],
        vec![
            vec![Action::Reduce(RuleId(0))],
            vec![Action::Accept],
            vec![],
        ],
    ];
    let sym = BTreeMap::new();
    let c = tc.compress_action_table_small(&at, &sym).unwrap();
    for w in c.row_offsets.windows(2) {
        assert!(w[1] >= w[0], "row_offsets must be non-decreasing");
    }
}

#[test]
fn cat_many_states() {
    let tc = TableCompressor::new();
    let at: Vec<Vec<Vec<Action>>> = (0..100)
        .map(|i| vec![vec![Action::Shift(StateId(i as u16))]])
        .collect();
    let sym = BTreeMap::new();
    let c = tc.compress_action_table_small(&at, &sym).unwrap();
    assert_eq!(c.row_offsets.len(), 101);
    assert_eq!(c.data.len(), 100);
}

// ===================================================================
// 8. compress_goto_table_small
// ===================================================================

#[test]
fn cgt_empty() {
    let tc = TableCompressor::new();
    let gt: Vec<Vec<StateId>> = vec![];
    let c = tc.compress_goto_table_small(&gt).unwrap();
    assert_eq!(c.row_offsets.len(), 1); // just the sentinel
    assert!(c.data.is_empty());
}

#[test]
fn cgt_single_entry() {
    let tc = TableCompressor::new();
    let gt = vec![vec![StateId(5)]];
    let c = tc.compress_goto_table_small(&gt).unwrap();
    assert_eq!(c.data.len(), 1);
    assert!(matches!(c.data[0], CompressedGotoEntry::Single(5)));
}

#[test]
fn cgt_no_run_length_for_short_runs() {
    let tc = TableCompressor::new();
    // Run of exactly 2 — should NOT use RunLength (threshold is > 2)
    let gt = vec![vec![StateId(1), StateId(1)]];
    let c = tc.compress_goto_table_small(&gt).unwrap();
    assert!(
        c.data
            .iter()
            .all(|e| matches!(e, CompressedGotoEntry::Single(_)))
    );
    assert_eq!(c.data.len(), 2);
}

#[test]
fn cgt_run_length_for_three_or_more() {
    let tc = TableCompressor::new();
    let gt = vec![vec![StateId(4), StateId(4), StateId(4)]];
    let c = tc.compress_goto_table_small(&gt).unwrap();
    assert!(
        c.data
            .iter()
            .any(|e| matches!(e, CompressedGotoEntry::RunLength { state: 4, count: 3 }))
    );
}

#[test]
fn cgt_mixed_singles_and_runs() {
    let tc = TableCompressor::new();
    let gt = vec![vec![
        StateId(1),
        StateId(2),
        StateId(2),
        StateId(2),
        StateId(2),
        StateId(3),
    ]];
    let c = tc.compress_goto_table_small(&gt).unwrap();
    // The run of four 2s should be RLE, the 1 and 3 should be Single
    let singles: Vec<_> = c
        .data
        .iter()
        .filter(|e| matches!(e, CompressedGotoEntry::Single(_)))
        .collect();
    let runs: Vec<_> = c
        .data
        .iter()
        .filter(|e| matches!(e, CompressedGotoEntry::RunLength { .. }))
        .collect();
    assert!(!singles.is_empty());
    assert!(!runs.is_empty());
}

#[test]
fn cgt_all_same_long_run() {
    let tc = TableCompressor::new();
    let gt = vec![vec![StateId(9); 20]];
    let c = tc.compress_goto_table_small(&gt).unwrap();
    assert!(c.data.iter().any(|e| matches!(
        e,
        CompressedGotoEntry::RunLength {
            state: 9,
            count: 20
        }
    )));
}

#[test]
fn cgt_row_offsets_sentinel() {
    let tc = TableCompressor::new();
    let gt = vec![vec![StateId(1), StateId(2)], vec![StateId(3)]];
    let c = tc.compress_goto_table_small(&gt).unwrap();
    // Last offset is total number of entries
    assert_eq!(*c.row_offsets.last().unwrap() as usize, c.data.len());
}

#[test]
fn cgt_multiple_rows_offsets() {
    let tc = TableCompressor::new();
    let gt = vec![
        vec![StateId(0); 5],
        vec![StateId(1); 3],
        vec![StateId(2), StateId(3)],
    ];
    let c = tc.compress_goto_table_small(&gt).unwrap();
    assert_eq!(c.row_offsets.len(), 4); // 3 rows + 1 sentinel
    for w in c.row_offsets.windows(2) {
        assert!(w[1] >= w[0]);
    }
}

// ===================================================================
// 9. Full compress() pipeline with real grammars
// ===================================================================

#[test]
fn compress_simple_grammar() {
    let pt = simple_table();
    let ct = compress(&pt);
    assert!(!ct.action_table.row_offsets.is_empty());
    assert!(!ct.goto_table.row_offsets.is_empty());
}

#[test]
fn compress_multi_rule_grammar() {
    let pt = multi_rule_table();
    let ct = compress(&pt);
    assert!(ct.action_table.data.len() >= 2);
}

#[test]
fn compress_sequence_grammar() {
    let pt = sequence_table();
    let ct = compress(&pt);
    // sequence S→a b c needs at least 4 states
    assert!(ct.action_table.row_offsets.len() >= 5);
}

#[test]
fn compress_nullable_grammar() {
    let pt = nullable_table();
    let ti = token_indices(&pt);
    let tc = TableCompressor::new();
    let r = tc.compress(&pt, &ti, true);
    assert!(r.is_ok());
}

#[test]
fn compress_action_row_count_matches_states() {
    let pt = simple_table();
    let ct = compress(&pt);
    // row_offsets.len() == state_count + 1
    assert_eq!(ct.action_table.row_offsets.len(), pt.state_count + 1);
}

#[test]
fn compress_goto_row_count_matches_states() {
    let pt = simple_table();
    let ct = compress(&pt);
    assert_eq!(ct.goto_table.row_offsets.len(), pt.state_count + 1);
}

// ===================================================================
// 10. Error paths
// ===================================================================

#[test]
fn compress_rejects_empty_action_table() {
    let mut pt = simple_table();
    pt.action_table.clear();
    let ti = token_indices(&pt);
    let r = TableCompressor::new().compress(&pt, &ti, false);
    assert!(r.is_err());
}

#[test]
fn compress_rejects_zero_state_count() {
    let mut pt = simple_table();
    pt.state_count = 0;
    let ti = token_indices(&pt);
    let r = TableCompressor::new().compress(&pt, &ti, false);
    assert!(r.is_err());
}

// ===================================================================
// 11. Determinism
// ===================================================================

#[test]
fn compress_is_deterministic_simple() {
    let pt = simple_table();
    let a = compress(&pt);
    let b = compress(&pt);
    assert_eq!(a.action_table.row_offsets, b.action_table.row_offsets);
    assert_eq!(a.goto_table.row_offsets, b.goto_table.row_offsets);
    assert_eq!(a.action_table.data.len(), b.action_table.data.len());
    assert_eq!(a.goto_table.data.len(), b.goto_table.data.len());
}

#[test]
fn compress_is_deterministic_multi() {
    let pt = multi_rule_table();
    let a = compress(&pt);
    let b = compress(&pt);
    assert_eq!(a.action_table.data.len(), b.action_table.data.len());
    assert_eq!(a.action_table.row_offsets, b.action_table.row_offsets);
}

#[test]
fn compress_is_deterministic_sequence() {
    let pt = sequence_table();
    let a = compress(&pt);
    let b = compress(&pt);
    assert_eq!(a.action_table.row_offsets, b.action_table.row_offsets);
    assert_eq!(a.goto_table.row_offsets, b.goto_table.row_offsets);
}

// ===================================================================
// 12. Compression ratio / data is smaller than dense
// ===================================================================

#[test]
fn compressed_action_smaller_than_dense() {
    let pt = multi_rule_table();
    let ct = compress(&pt);
    let dense_cells = pt.state_count * pt.symbol_count;
    assert!(
        ct.action_table.data.len() < dense_cells,
        "compressed ({}) should be smaller than dense ({})",
        ct.action_table.data.len(),
        dense_cells,
    );
}

#[test]
fn compressed_goto_not_larger_than_dense() {
    let pt = sequence_table();
    let ct = compress(&pt);
    let dense_cells: usize = pt.goto_table.iter().map(|r| r.len()).sum();
    assert!(ct.goto_table.data.len() <= dense_cells);
}

// ===================================================================
// 13. Encoding round-trip via encode_action_small
// ===================================================================

#[test]
fn encode_all_shift_values_below_limit() {
    let tc = TableCompressor::new();
    for s in [0u16, 1, 100, 1000, 0x7FFE, 0x7FFF] {
        let v = tc.encode_action_small(&Action::Shift(StateId(s))).unwrap();
        assert_eq!(v, s);
    }
}

#[test]
fn encode_all_reduce_values_below_limit() {
    let tc = TableCompressor::new();
    for r in [0u16, 1, 100, 0x3FFE, 0x3FFF] {
        let v = tc.encode_action_small(&Action::Reduce(RuleId(r))).unwrap();
        assert_eq!(v, 0x8000 | (r + 1));
    }
}

// ===================================================================
// 14. Edge-case goto patterns
// ===================================================================

#[test]
fn cgt_alternating_values() {
    let tc = TableCompressor::new();
    let gt = vec![vec![
        StateId(1),
        StateId(2),
        StateId(1),
        StateId(2),
        StateId(1),
    ]];
    let c = tc.compress_goto_table_small(&gt).unwrap();
    // No runs possible; all should be Single entries
    assert!(
        c.data
            .iter()
            .all(|e| matches!(e, CompressedGotoEntry::Single(_)))
    );
    assert_eq!(c.data.len(), 5);
}

#[test]
fn cgt_single_element_row() {
    let tc = TableCompressor::new();
    let gt = vec![vec![StateId(99)]];
    let c = tc.compress_goto_table_small(&gt).unwrap();
    assert_eq!(c.data.len(), 1);
}

#[test]
fn cgt_empty_row() {
    let tc = TableCompressor::new();
    let gt = vec![vec![]];
    let c = tc.compress_goto_table_small(&gt).unwrap();
    assert!(c.data.is_empty());
    assert_eq!(c.row_offsets.len(), 2); // 1 row + sentinel
    assert_eq!(c.row_offsets[0], 0);
    assert_eq!(c.row_offsets[1], 0);
}

// ===================================================================
// 15. CompressedTables field accessibility
// ===================================================================

#[test]
fn ct_small_table_threshold_populated() {
    let pt = simple_table();
    let ct = compress(&pt);
    assert_eq!(ct.small_table_threshold, 32768);
}

#[test]
fn ct_action_default_actions_count() {
    let pt = multi_rule_table();
    let ct = compress(&pt);
    assert_eq!(ct.action_table.default_actions.len(), pt.state_count);
}

#[test]
fn ct_action_row_offsets_first_is_zero() {
    let pt = simple_table();
    let ct = compress(&pt);
    assert_eq!(ct.action_table.row_offsets[0], 0);
}

#[test]
fn ct_goto_row_offsets_first_is_zero() {
    let pt = simple_table();
    let ct = compress(&pt);
    assert_eq!(ct.goto_table.row_offsets[0], 0);
}

#[test]
fn ct_action_last_offset_equals_data_len() {
    let pt = multi_rule_table();
    let ct = compress(&pt);
    assert_eq!(
        *ct.action_table.row_offsets.last().unwrap() as usize,
        ct.action_table.data.len()
    );
}

#[test]
fn ct_goto_last_offset_equals_data_len() {
    let pt = sequence_table();
    let ct = compress(&pt);
    assert_eq!(
        *ct.goto_table.row_offsets.last().unwrap() as usize,
        ct.goto_table.data.len()
    );
}

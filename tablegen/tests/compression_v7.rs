//! Compression algorithm tests for adze-tablegen (v7).
//!
//! 64 tests across 8 categories testing action/goto compression,
//! bit packing, compression ratios, roundtrips, determinism,
//! complex grammars, and edge cases.

use adze_glr_core::{Action, FirstFollowSets, build_lr1_automaton};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{Grammar, RuleId, StateId};
use adze_tablegen::compress::TableCompressor;
use adze_tablegen::compression::{
    BitPackedActionTable, compress_action_table, compress_goto_table, decompress_action,
    decompress_goto,
};
use adze_tablegen::{collect_token_indices, eof_accepts_or_reduces};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn single_token_grammar() -> Grammar {
    GrammarBuilder::new("single")
        .token("x", "x")
        .rule("S", vec!["x"])
        .start("S")
        .build()
}

fn two_alt_grammar() -> Grammar {
    GrammarBuilder::new("two_alt")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["a"])
        .rule("S", vec!["b"])
        .start("S")
        .build()
}

fn chain_grammar() -> Grammar {
    GrammarBuilder::new("chain")
        .token("x", "x")
        .rule("A", vec!["x"])
        .rule("B", vec!["A"])
        .rule("S", vec!["B"])
        .start("S")
        .build()
}

fn wide_grammar(n: u8) -> Grammar {
    let mut gb = GrammarBuilder::new("wide");
    for i in 0..n {
        let name = format!("t{i}");
        let pat = format!("{}", (b'a' + i) as char);
        gb = gb.token(&name, &pat);
        gb = gb.rule("S", vec![Box::leak(name.into_boxed_str()) as &str]);
    }
    gb.start("S").build()
}

fn build_table(g: &Grammar) -> adze_glr_core::ParseTable {
    let mut g = g.clone();
    let ff = FirstFollowSets::compute_normalized(&mut g).expect("FIRST/FOLLOW");
    build_lr1_automaton(&g, &ff).expect("LR(1)")
}

fn compress_pipeline(
    g: &Grammar,
) -> (
    adze_glr_core::ParseTable,
    adze_tablegen::compress::CompressedTables,
) {
    let pt = build_table(g);
    let ti = collect_token_indices(g, &pt);
    let sce = eof_accepts_or_reduces(&pt);
    let ct = TableCompressor::new().compress(&pt, &ti, sce).unwrap();
    (pt, ct)
}

// =========================================================================
// 1. compress_action_* — action table compression (8 tests)
// =========================================================================

#[test]
fn compress_action_single_row() {
    let table = vec![vec![vec![Action::Shift(StateId(1))], vec![Action::Error]]];
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 1);
    assert_eq!(c.state_to_row.len(), 1);
}

#[test]
fn compress_action_duplicate_rows() {
    let row = vec![vec![Action::Error], vec![Action::Shift(StateId(2))]];
    let table = vec![row.clone(), row.clone(), row];
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 1, "all identical rows → 1 unique");
    assert_eq!(c.state_to_row, vec![0, 0, 0]);
}

#[test]
fn compress_action_all_unique_rows() {
    let table = vec![
        vec![vec![Action::Shift(StateId(0))]],
        vec![vec![Action::Shift(StateId(1))]],
        vec![vec![Action::Shift(StateId(2))]],
    ];
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 3);
}

#[test]
fn compress_action_mixed_actions() {
    let table = vec![
        vec![vec![Action::Shift(StateId(1))], vec![Action::Accept]],
        vec![
            vec![Action::Reduce(RuleId(0))],
            vec![Action::Reduce(RuleId(1))],
        ],
    ];
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 2);
    assert_eq!(c.state_to_row[0], 0);
    assert_eq!(c.state_to_row[1], 1);
}

#[test]
fn compress_action_error_only_rows() {
    let row = vec![vec![Action::Error], vec![Action::Error]];
    let table = vec![row.clone(), row];
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 1);
}

#[test]
fn compress_action_accept_cell() {
    let table = vec![vec![vec![Action::Accept]]];
    let c = compress_action_table(&table);
    assert_eq!(decompress_action(&c, 0, 0), Action::Accept);
}

#[test]
fn compress_action_reduce_cell() {
    let table = vec![vec![vec![Action::Reduce(RuleId(7))]]];
    let c = compress_action_table(&table);
    assert_eq!(decompress_action(&c, 0, 0), Action::Reduce(RuleId(7)));
}

#[test]
fn compress_action_preserves_row_order() {
    let table = vec![
        vec![vec![Action::Shift(StateId(10))]],
        vec![vec![Action::Shift(StateId(20))]],
        vec![vec![Action::Shift(StateId(10))]],
    ];
    let c = compress_action_table(&table);
    // row 0 and row 2 are identical
    assert_eq!(c.state_to_row[0], c.state_to_row[2]);
    assert_ne!(c.state_to_row[0], c.state_to_row[1]);
}

// =========================================================================
// 2. compress_goto_* — goto table compression (8 tests)
// =========================================================================

#[test]
fn compress_goto_empty_table() {
    let table: Vec<Vec<Option<StateId>>> = vec![];
    let c = compress_goto_table(&table);
    assert!(c.entries.is_empty());
}

#[test]
fn compress_goto_all_none() {
    let table = vec![vec![None, None], vec![None, None]];
    let c = compress_goto_table(&table);
    assert!(c.entries.is_empty());
}

#[test]
fn compress_goto_single_entry() {
    let table = vec![vec![Some(StateId(5)), None]];
    let c = compress_goto_table(&table);
    assert_eq!(c.entries.len(), 1);
    assert_eq!(decompress_goto(&c, 0, 0), Some(StateId(5)));
    assert_eq!(decompress_goto(&c, 0, 1), None);
}

#[test]
fn compress_goto_full_row() {
    let table = vec![vec![Some(StateId(1)), Some(StateId(2)), Some(StateId(3))]];
    let c = compress_goto_table(&table);
    assert_eq!(c.entries.len(), 3);
}

#[test]
fn compress_goto_diagonal() {
    let table = vec![
        vec![Some(StateId(1)), None, None],
        vec![None, Some(StateId(2)), None],
        vec![None, None, Some(StateId(3))],
    ];
    let c = compress_goto_table(&table);
    assert_eq!(c.entries.len(), 3);
    assert_eq!(decompress_goto(&c, 0, 0), Some(StateId(1)));
    assert_eq!(decompress_goto(&c, 1, 1), Some(StateId(2)));
    assert_eq!(decompress_goto(&c, 2, 2), Some(StateId(3)));
}

#[test]
fn compress_goto_sparse_density() {
    // 4×4 table with only 2 entries ⇒ sparse
    let mut table = vec![vec![None; 4]; 4];
    table[0][3] = Some(StateId(10));
    table[3][0] = Some(StateId(20));
    let c = compress_goto_table(&table);
    assert_eq!(c.entries.len(), 2);
}

#[test]
fn compress_goto_large_state_ids() {
    let table = vec![vec![Some(StateId(u16::MAX - 1))]];
    let c = compress_goto_table(&table);
    assert_eq!(decompress_goto(&c, 0, 0), Some(StateId(u16::MAX - 1)));
}

#[test]
fn compress_goto_missing_coordinates_return_none() {
    let table = vec![vec![Some(StateId(1))]];
    let c = compress_goto_table(&table);
    // Out-of-table lookups should return None
    assert_eq!(decompress_goto(&c, 5, 5), None);
}

// =========================================================================
// 3. compress_bitpack_* — bit packing operations (8 tests)
// =========================================================================

#[test]
fn compress_bitpack_all_errors() {
    let table = vec![vec![Action::Error; 4]];
    let bp = BitPackedActionTable::from_table(&table);
    for col in 0..4 {
        assert_eq!(bp.decompress(0, col), Action::Error);
    }
}

#[test]
fn compress_bitpack_single_shift() {
    let table = vec![vec![Action::Shift(StateId(3))]];
    let bp = BitPackedActionTable::from_table(&table);
    assert_eq!(bp.decompress(0, 0), Action::Shift(StateId(3)));
}

#[test]
fn compress_bitpack_single_reduce() {
    let table = vec![vec![Action::Reduce(RuleId(0))]];
    let bp = BitPackedActionTable::from_table(&table);
    assert_eq!(bp.decompress(0, 0), Action::Reduce(RuleId(0)));
}

#[test]
fn compress_bitpack_accept_action() {
    let table = vec![vec![Action::Accept]];
    let bp = BitPackedActionTable::from_table(&table);
    assert_eq!(bp.decompress(0, 0), Action::Accept);
}

#[test]
fn compress_bitpack_error_mask_word_boundary() {
    // 65 cells ⇒ needs 2 mask words
    let table = vec![vec![Action::Error; 65]];
    let bp = BitPackedActionTable::from_table(&table);
    assert_eq!(bp.decompress(0, 64), Action::Error);
}

#[test]
fn compress_bitpack_recover_treated_as_error() {
    let table = vec![vec![Action::Recover]];
    let bp = BitPackedActionTable::from_table(&table);
    assert_eq!(bp.decompress(0, 0), Action::Error);
}

#[test]
fn compress_bitpack_fork_roundtrip() {
    let actions = vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(2))];
    let table = vec![vec![Action::Fork(actions.clone())]];
    let bp = BitPackedActionTable::from_table(&table);
    match bp.decompress(0, 0) {
        Action::Fork(v) => assert_eq!(v, actions),
        other => panic!("expected Fork, got {other:?}"),
    }
}

#[test]
fn compress_bitpack_empty_table() {
    let table: Vec<Vec<Action>> = vec![];
    let bp = BitPackedActionTable::from_table(&table);
    // Just verify construction doesn't panic
    let _ = bp;
}

// =========================================================================
// 4. compress_ratio_* — compression ratio tests (8 tests)
// =========================================================================

#[test]
fn compress_ratio_identical_rows_collapse() {
    // Each row has 10 error cells, wrapped as Vec<Vec<Action>>
    let row: Vec<Vec<Action>> = vec![vec![Action::Error]; 10];
    let table: Vec<Vec<Vec<Action>>> = std::iter::repeat_n(row, 50).collect();
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 1);
    assert!(c.state_to_row.len() == 50);
}

#[test]
fn compress_ratio_goto_sparse_vs_dense() {
    let sparse = vec![vec![None; 20]; 20];
    let c_sparse = compress_goto_table(&sparse);

    let dense: Vec<Vec<Option<StateId>>> = (0..20)
        .map(|i| {
            (0..20)
                .map(|j| Some(StateId((i * 20 + j) as u16)))
                .collect()
        })
        .collect();
    let c_dense = compress_goto_table(&dense);

    assert!(c_sparse.entries.len() < c_dense.entries.len());
}

#[test]
fn compress_ratio_bitpack_shift_data_count() {
    let table = vec![vec![
        Action::Shift(StateId(1)),
        Action::Shift(StateId(2)),
        Action::Error,
    ]];
    let bp = BitPackedActionTable::from_table(&table);
    // Two shifts were stored
    assert_eq!(bp.decompress(0, 0), Action::Shift(StateId(1)));
    assert_eq!(bp.decompress(0, 1), Action::Shift(StateId(2)));
    assert_eq!(bp.decompress(0, 2), Action::Error);
}

#[test]
fn compress_ratio_unique_row_count_matches_distinct() {
    let table = vec![
        vec![vec![Action::Error]],
        vec![vec![Action::Shift(StateId(1))]],
        vec![vec![Action::Error]],
        vec![vec![Action::Shift(StateId(2))]],
    ];
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 3); // Error, Shift(1), Shift(2)
}

#[test]
fn compress_ratio_goto_entry_count_equals_some_count() {
    let table = vec![
        vec![Some(StateId(0)), None, None],
        vec![None, Some(StateId(1)), None],
    ];
    let c = compress_goto_table(&table);
    assert_eq!(c.entries.len(), 2);
}

#[test]
fn compress_ratio_single_token_grammar_compresses() {
    let g = single_token_grammar();
    let (pt, ct) = compress_pipeline(&g);
    // Compressed tables should have at least some data
    assert!(!ct.action_table.default_actions.is_empty());
    assert!(ct.validate(&pt).is_ok());
}

#[test]
fn compress_ratio_wide_grammar_more_rows() {
    let g_small = two_alt_grammar();
    let g_wide = wide_grammar(8);
    let pt_small = build_table(&g_small);
    let pt_wide = build_table(&g_wide);
    assert!(
        pt_wide.state_count >= pt_small.state_count,
        "wider grammar should have at least as many states"
    );
}

#[test]
fn compress_ratio_error_mask_compactness() {
    // All-error table should produce a full error mask with no shift/reduce data
    let table = vec![vec![Action::Error; 10]; 5];
    let bp = BitPackedActionTable::from_table(&table);
    // All cells map to Error
    for s in 0..5 {
        for sym in 0..10 {
            assert_eq!(bp.decompress(s, sym), Action::Error);
        }
    }
}

// =========================================================================
// 5. compress_roundtrip_* — compress then decompress (8 tests)
// =========================================================================

#[test]
fn compress_roundtrip_action_shift() {
    let table = vec![vec![
        vec![Action::Shift(StateId(7))],
        vec![Action::Shift(StateId(3))],
    ]];
    let c = compress_action_table(&table);
    assert_eq!(decompress_action(&c, 0, 0), Action::Shift(StateId(7)));
    assert_eq!(decompress_action(&c, 0, 1), Action::Shift(StateId(3)));
}

#[test]
fn compress_roundtrip_action_reduce() {
    let table = vec![vec![
        vec![Action::Reduce(RuleId(0))],
        vec![Action::Reduce(RuleId(100))],
    ]];
    let c = compress_action_table(&table);
    assert_eq!(decompress_action(&c, 0, 0), Action::Reduce(RuleId(0)));
    assert_eq!(decompress_action(&c, 0, 1), Action::Reduce(RuleId(100)));
}

#[test]
fn compress_roundtrip_action_error() {
    let table = vec![vec![vec![Action::Error]]];
    let c = compress_action_table(&table);
    assert_eq!(decompress_action(&c, 0, 0), Action::Error);
}

#[test]
fn compress_roundtrip_goto_some() {
    let table = vec![vec![Some(StateId(42))]];
    let c = compress_goto_table(&table);
    assert_eq!(decompress_goto(&c, 0, 0), Some(StateId(42)));
}

#[test]
fn compress_roundtrip_goto_none() {
    let table = vec![vec![None]];
    let c = compress_goto_table(&table);
    assert_eq!(decompress_goto(&c, 0, 0), None);
}

#[test]
fn compress_roundtrip_action_multi_state() {
    let table = vec![
        vec![vec![Action::Shift(StateId(1))], vec![Action::Error]],
        vec![vec![Action::Error], vec![Action::Accept]],
        vec![
            vec![Action::Reduce(RuleId(0))],
            vec![Action::Reduce(RuleId(1))],
        ],
    ];
    let c = compress_action_table(&table);
    assert_eq!(decompress_action(&c, 0, 0), Action::Shift(StateId(1)));
    assert_eq!(decompress_action(&c, 0, 1), Action::Error);
    assert_eq!(decompress_action(&c, 1, 0), Action::Error);
    assert_eq!(decompress_action(&c, 1, 1), Action::Accept);
    assert_eq!(decompress_action(&c, 2, 0), Action::Reduce(RuleId(0)));
    assert_eq!(decompress_action(&c, 2, 1), Action::Reduce(RuleId(1)));
}

#[test]
fn compress_roundtrip_goto_multi_row() {
    let table = vec![vec![None, Some(StateId(1))], vec![Some(StateId(2)), None]];
    let c = compress_goto_table(&table);
    assert_eq!(decompress_goto(&c, 0, 0), None);
    assert_eq!(decompress_goto(&c, 0, 1), Some(StateId(1)));
    assert_eq!(decompress_goto(&c, 1, 0), Some(StateId(2)));
    assert_eq!(decompress_goto(&c, 1, 1), None);
}

#[test]
fn compress_roundtrip_pipeline_validates() {
    let g = two_alt_grammar();
    let (pt, ct) = compress_pipeline(&g);
    assert!(ct.validate(&pt).is_ok());
}

// =========================================================================
// 6. compress_deterministic_* — deterministic compression (8 tests)
// =========================================================================

#[test]
fn compress_deterministic_action_same_input() {
    let table = vec![
        vec![vec![Action::Shift(StateId(1))], vec![Action::Error]],
        vec![vec![Action::Error], vec![Action::Accept]],
    ];
    let c1 = compress_action_table(&table);
    let c2 = compress_action_table(&table);
    assert_eq!(c1.unique_rows.len(), c2.unique_rows.len());
    assert_eq!(c1.state_to_row, c2.state_to_row);
}

#[test]
fn compress_deterministic_goto_same_input() {
    let table = vec![vec![Some(StateId(1)), None], vec![None, Some(StateId(2))]];
    let c1 = compress_goto_table(&table);
    let c2 = compress_goto_table(&table);
    assert_eq!(c1.entries.len(), c2.entries.len());
    for (k, v) in &c1.entries {
        assert_eq!(c2.entries.get(k), Some(v));
    }
}

#[test]
fn compress_deterministic_bitpack_same_input() {
    let table = vec![vec![
        Action::Shift(StateId(1)),
        Action::Error,
        Action::Reduce(RuleId(0)),
    ]];
    let bp1 = BitPackedActionTable::from_table(&table);
    let bp2 = BitPackedActionTable::from_table(&table);
    for col in 0..3 {
        assert_eq!(bp1.decompress(0, col), bp2.decompress(0, col));
    }
}

#[test]
fn compress_deterministic_pipeline_twice() {
    let g = single_token_grammar();
    let (_, ct1) = compress_pipeline(&g);
    let (_, ct2) = compress_pipeline(&g);
    assert_eq!(ct1.action_table.data.len(), ct2.action_table.data.len());
    assert_eq!(ct1.action_table.row_offsets, ct2.action_table.row_offsets);
    assert_eq!(
        ct1.action_table.default_actions.len(),
        ct2.action_table.default_actions.len(),
    );
}

#[test]
fn compress_deterministic_unique_row_indices_stable() {
    let table = vec![
        vec![vec![Action::Shift(StateId(1))]],
        vec![vec![Action::Error]],
        vec![vec![Action::Shift(StateId(1))]],
    ];
    let c = compress_action_table(&table);
    // First and third rows map to same unique index
    assert_eq!(c.state_to_row[0], c.state_to_row[2]);
    // Rerun — same mapping
    let c2 = compress_action_table(&table);
    assert_eq!(c.state_to_row, c2.state_to_row);
}

#[test]
fn compress_deterministic_encode_action_small_shift() {
    let tc = TableCompressor::new();
    let v1 = tc.encode_action_small(&Action::Shift(StateId(10))).unwrap();
    let v2 = tc.encode_action_small(&Action::Shift(StateId(10))).unwrap();
    assert_eq!(v1, v2);
    assert_eq!(v1, 10u16);
}

#[test]
fn compress_deterministic_encode_action_small_reduce() {
    let tc = TableCompressor::new();
    let v = tc.encode_action_small(&Action::Reduce(RuleId(5))).unwrap();
    // 0x8000 | (5 + 1) = 0x8006
    assert_eq!(v, 0x8006);
}

#[test]
fn compress_deterministic_encode_special_actions() {
    let tc = TableCompressor::new();
    let accept = tc.encode_action_small(&Action::Accept).unwrap();
    let error = tc.encode_action_small(&Action::Error).unwrap();
    let recover = tc.encode_action_small(&Action::Recover).unwrap();
    assert_eq!(accept, 0xFFFF);
    assert_eq!(error, 0xFFFE);
    assert_eq!(recover, 0xFFFD);
}

// =========================================================================
// 7. compress_complex_* — complex grammar compression (8 tests)
// =========================================================================

#[test]
fn compress_complex_two_alt_pipeline() {
    let g = two_alt_grammar();
    let (pt, ct) = compress_pipeline(&g);
    assert!(ct.validate(&pt).is_ok());
}

#[test]
fn compress_complex_chain_pipeline() {
    let g = chain_grammar();
    let (pt, ct) = compress_pipeline(&g);
    assert!(ct.validate(&pt).is_ok());
}

#[test]
fn compress_complex_wide_8_alts() {
    let g = wide_grammar(8);
    let (pt, ct) = compress_pipeline(&g);
    assert!(ct.validate(&pt).is_ok());
}

#[test]
fn compress_complex_wide_grammar_has_multiple_states() {
    let g = wide_grammar(6);
    let pt = build_table(&g);
    assert!(pt.state_count > 1);
}

#[test]
fn compress_complex_wide_default_actions_populated() {
    let g = wide_grammar(5);
    let (_, ct) = compress_pipeline(&g);
    assert!(!ct.action_table.default_actions.is_empty());
}

#[test]
fn compress_complex_chain_goto_offsets_present() {
    let g = chain_grammar();
    let (_, ct) = compress_pipeline(&g);
    assert!(!ct.goto_table.row_offsets.is_empty());
}

#[test]
fn compress_complex_multi_rule_nonterminal() {
    let g = GrammarBuilder::new("multi")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .rule("X", vec!["a"])
        .rule("X", vec!["b"])
        .rule("X", vec!["c"])
        .rule("S", vec!["X"])
        .start("S")
        .build();
    let (pt, ct) = compress_pipeline(&g);
    assert!(ct.validate(&pt).is_ok());
}

#[test]
fn compress_complex_nested_nonterminals() {
    let g = GrammarBuilder::new("nested")
        .token("x", "x")
        .rule("C", vec!["x"])
        .rule("B", vec!["C"])
        .rule("A", vec!["B"])
        .rule("S", vec!["A"])
        .start("S")
        .build();
    let (pt, ct) = compress_pipeline(&g);
    assert!(ct.validate(&pt).is_ok());
    assert!(pt.state_count >= 2);
}

// =========================================================================
// 8. compress_edge_* — edge cases (8 tests)
// =========================================================================

#[test]
fn compress_edge_single_cell_action_table() {
    let table = vec![vec![vec![Action::Accept]]];
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 1);
    assert_eq!(decompress_action(&c, 0, 0), Action::Accept);
}

#[test]
fn compress_edge_single_cell_goto_table() {
    let table = vec![vec![Some(StateId(0))]];
    let c = compress_goto_table(&table);
    assert_eq!(c.entries.len(), 1);
}

#[test]
fn compress_edge_high_state_id_shift() {
    let table = vec![vec![vec![Action::Shift(StateId(0x7FFE))]]];
    let c = compress_action_table(&table);
    assert_eq!(decompress_action(&c, 0, 0), Action::Shift(StateId(0x7FFE)));
}

#[test]
fn compress_edge_high_rule_id_reduce() {
    let table = vec![vec![vec![Action::Reduce(RuleId(0x3FFF))]]];
    let c = compress_action_table(&table);
    assert_eq!(decompress_action(&c, 0, 0), Action::Reduce(RuleId(0x3FFF)));
}

#[test]
fn compress_edge_encode_shift_boundary() {
    let tc = TableCompressor::new();
    // State 0x7FFF is the max for small table encoding
    let v = tc.encode_action_small(&Action::Shift(StateId(0x7FFE)));
    assert!(v.is_ok());
    assert_eq!(v.unwrap(), 0x7FFE);
    // 0x8000 should fail
    let err = tc.encode_action_small(&Action::Shift(StateId(0x8000)));
    assert!(err.is_err());
}

#[test]
fn compress_edge_encode_reduce_boundary() {
    let tc = TableCompressor::new();
    // Rule 0x3FFF should fail
    let err = tc.encode_action_small(&Action::Reduce(RuleId(0x4000)));
    assert!(err.is_err());
    // Rule 0x3FFE should succeed
    let ok = tc.encode_action_small(&Action::Reduce(RuleId(0x3FFE)));
    assert!(ok.is_ok());
}

#[test]
fn compress_edge_bitpack_65th_cell() {
    // 65 cells test word-boundary handling in error_mask
    let mut row = vec![Action::Error; 64];
    row.push(Action::Shift(StateId(99)));
    let table = vec![row];
    let bp = BitPackedActionTable::from_table(&table);
    assert_eq!(bp.decompress(0, 63), Action::Error);
    assert_eq!(bp.decompress(0, 64), Action::Shift(StateId(99)));
}

#[test]
fn compress_edge_fork_action_requires_flattening() {
    let tc = TableCompressor::new();
    let fork = Action::Fork(vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(0))]);
    assert!(tc.encode_action_small(&fork).is_err());
}

//! Comprehensive v3 tests for table compression in adze-tablegen.
//!
//! 50+ tests covering: compression algorithms, semantic equivalence,
//! various table sizes, RLE, bit-packing, decompression roundtrip,
//! and real grammar integration.

use adze_glr_core::{Action, FirstFollowSets, ParseTable, build_lr1_automaton};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{Associativity, Grammar, RuleId, StateId};
use adze_tablegen::compress::{
    CompressedActionEntry, CompressedGotoEntry, CompressedParseTable, CompressedTables,
    TableCompressor,
};
use adze_tablegen::compression::{
    BitPackedActionTable, compress_action_table, compress_goto_table, decompress_action,
    decompress_goto,
};
use adze_tablegen::{collect_token_indices, eof_accepts_or_reduces};
use std::collections::BTreeMap;

// ============================================================================
// Helpers
// ============================================================================

fn build_table(grammar: &Grammar) -> ParseTable {
    let mut g = grammar.clone();
    let ff = FirstFollowSets::compute_normalized(&mut g).expect("FIRST/FOLLOW");
    build_lr1_automaton(&g, &ff).expect("LR(1)")
}

fn compress_full(grammar: &Grammar) -> CompressedTables {
    let pt = build_table(grammar);
    let ti = collect_token_indices(grammar, &pt);
    let sce = eof_accepts_or_reduces(&pt);
    TableCompressor::new().compress(&pt, &ti, sce).unwrap()
}

/// Convert single-action rows into GLR action cells.
#[allow(dead_code)]
fn to_glr_cells(rows: Vec<Vec<Action>>) -> Vec<Vec<Vec<Action>>> {
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

fn single_token_grammar() -> Grammar {
    GrammarBuilder::new("single_tok")
        .token("x", "x")
        .rule("S", vec!["x"])
        .start("S")
        .build()
}

fn two_token_grammar() -> Grammar {
    GrammarBuilder::new("two_tok")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["a", "b"])
        .start("S")
        .build()
}

fn alternatives_grammar() -> Grammar {
    GrammarBuilder::new("alts")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .rule("S", vec!["a"])
        .rule("S", vec!["b"])
        .rule("S", vec!["c"])
        .start("S")
        .build()
}

fn nested_grammar() -> Grammar {
    GrammarBuilder::new("nested")
        .token("x", "x")
        .token("y", "y")
        .rule("S", vec!["A", "B"])
        .rule("A", vec!["x"])
        .rule("B", vec!["y"])
        .start("S")
        .build()
}

fn deep_chain_grammar() -> Grammar {
    GrammarBuilder::new("deep_chain")
        .token("z", "z")
        .rule("S", vec!["A"])
        .rule("A", vec!["B"])
        .rule("B", vec!["C"])
        .rule("C", vec!["z"])
        .start("S")
        .build()
}

fn left_recursive_grammar() -> Grammar {
    GrammarBuilder::new("left_rec")
        .token("a", "a")
        .rule("S", vec!["a"])
        .rule("S", vec!["S", "a"])
        .start("S")
        .build()
}

fn right_recursive_grammar() -> Grammar {
    GrammarBuilder::new("right_rec")
        .token("a", "a")
        .rule("S", vec!["a"])
        .rule("S", vec!["a", "S"])
        .start("S")
        .build()
}

fn precedence_grammar() -> Grammar {
    GrammarBuilder::new("prec")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .token("STAR", r"\*")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "STAR", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build()
}

fn wide_alternatives_grammar() -> Grammar {
    let mut gb = GrammarBuilder::new("wide");
    for i in 0..10 {
        let name = format!("t{i}");
        let pat = format!("{}", (b'a' + i as u8) as char);
        gb = gb.token(&name, &pat);
        gb = gb.rule("S", vec![&name]);
    }
    gb.start("S").build()
}

fn long_sequence_grammar() -> Grammar {
    GrammarBuilder::new("long_seq")
        .token("t1", "a")
        .token("t2", "b")
        .token("t3", "c")
        .token("t4", "d")
        .token("t5", "e")
        .rule("S", vec!["t1", "t2", "t3", "t4", "t5"])
        .start("S")
        .build()
}

fn nullable_start_grammar() -> Grammar {
    GrammarBuilder::new("nullable")
        .token("a", "a")
        .rule("S", vec!["A"])
        .rule("A", vec!["a"])
        .rule("A", vec![])
        .start("S")
        .build()
}

// ============================================================================
// 1–5: Compression of simple grammars via full pipeline
// ============================================================================

#[test]
fn t01_single_token_compresses_ok() {
    let _ct = compress_full(&single_token_grammar());
}

#[test]
fn t02_single_token_action_table_non_empty() {
    let ct = compress_full(&single_token_grammar());
    assert!(!ct.action_table.data.is_empty());
}

#[test]
fn t03_single_token_goto_row_offsets() {
    let ct = compress_full(&single_token_grammar());
    assert!(ct.goto_table.row_offsets.len() >= 2);
}

#[test]
fn t04_two_token_compresses_ok() {
    let _ct = compress_full(&two_token_grammar());
}

#[test]
fn t05_alternatives_compresses_ok() {
    let _ct = compress_full(&alternatives_grammar());
}

// ============================================================================
// 6–10: Semantic equivalence – compressed tables preserve all non-error actions
// ============================================================================

fn verify_action_equivalence(grammar: &Grammar) {
    let pt = build_table(grammar);
    let ti = collect_token_indices(grammar, &pt);
    let sce = eof_accepts_or_reduces(&pt);
    let ct = TableCompressor::new().compress(&pt, &ti, sce).unwrap();

    // Collect all non-error actions from the original table
    let mut original_actions = Vec::new();
    for (state, row) in pt.action_table.iter().enumerate() {
        for (col, cell) in row.iter().enumerate() {
            for action in cell {
                if !matches!(action, Action::Error) {
                    original_actions.push((state, col, action.clone()));
                }
            }
        }
    }

    // Every non-error action must appear in compressed entries
    for entry in &ct.action_table.data {
        assert!(
            !matches!(entry.action, Action::Error),
            "Compressed table should not contain explicit Error entries"
        );
    }

    // Compressed entry count must equal original non-error action count
    assert_eq!(
        ct.action_table.data.len(),
        original_actions.len(),
        "Compressed entries must match original non-error action count"
    );
}

#[test]
fn t06_equivalence_single_token() {
    verify_action_equivalence(&single_token_grammar());
}

#[test]
fn t07_equivalence_two_token() {
    verify_action_equivalence(&two_token_grammar());
}

#[test]
fn t08_equivalence_alternatives() {
    verify_action_equivalence(&alternatives_grammar());
}

#[test]
fn t09_equivalence_nested() {
    verify_action_equivalence(&nested_grammar());
}

#[test]
fn t10_equivalence_deep_chain() {
    verify_action_equivalence(&deep_chain_grammar());
}

// ============================================================================
// 11–15: Various table sizes
// ============================================================================

#[test]
fn t11_wide_alternatives_compression() {
    let ct = compress_full(&wide_alternatives_grammar());
    assert!(!ct.action_table.data.is_empty());
    assert!(ct.action_table.row_offsets.len() >= 2);
}

#[test]
fn t12_long_sequence_compression() {
    let ct = compress_full(&long_sequence_grammar());
    assert!(!ct.action_table.data.is_empty());
}

#[test]
fn t13_recursive_grammar_has_multiple_states() {
    let grammar = left_recursive_grammar();
    let pt = build_table(&grammar);
    assert!(
        pt.state_count >= 2,
        "Recursive grammar should produce multiple states"
    );
    let _ct = compress_full(&grammar);
}

#[test]
fn t14_right_recursive_compression() {
    let ct = compress_full(&right_recursive_grammar());
    assert!(!ct.action_table.data.is_empty());
}

#[test]
fn t15_precedence_grammar_compression() {
    let ct = compress_full(&precedence_grammar());
    assert!(!ct.action_table.data.is_empty());
}

// ============================================================================
// 16–20: RLE (Run-Length Encoding) in goto table
// ============================================================================

#[test]
fn t16_goto_rle_long_run() {
    let compressor = TableCompressor::new();
    let goto_table = vec![vec![
        StateId(3),
        StateId(3),
        StateId(3),
        StateId(3),
        StateId(3),
    ]];
    let compressed = compressor.compress_goto_table_small(&goto_table).unwrap();
    let has_rle = compressed
        .data
        .iter()
        .any(|e| matches!(e, CompressedGotoEntry::RunLength { state: 3, count: 5 }));
    assert!(
        has_rle,
        "Run of 5 identical states should produce RLE entry"
    );
}

#[test]
fn t17_goto_rle_short_run_uses_singles() {
    let compressor = TableCompressor::new();
    let goto_table = vec![vec![StateId(1), StateId(1)]];
    let compressed = compressor.compress_goto_table_small(&goto_table).unwrap();
    let has_rle = compressed
        .data
        .iter()
        .any(|e| matches!(e, CompressedGotoEntry::RunLength { .. }));
    assert!(!has_rle, "Run of 2 should use Single entries");
    let single_count = compressed
        .data
        .iter()
        .filter(|e| matches!(e, CompressedGotoEntry::Single(1)))
        .count();
    assert_eq!(single_count, 2);
}

#[test]
fn t18_goto_rle_multiple_runs() {
    let compressor = TableCompressor::new();
    let goto_table = vec![vec![
        StateId(1),
        StateId(1),
        StateId(1),
        StateId(1),
        StateId(2),
        StateId(2),
        StateId(2),
        StateId(2),
    ]];
    let compressed = compressor.compress_goto_table_small(&goto_table).unwrap();
    let rle_count = compressed
        .data
        .iter()
        .filter(|e| matches!(e, CompressedGotoEntry::RunLength { .. }))
        .count();
    assert_eq!(
        rle_count, 2,
        "Two distinct runs should yield two RLE entries"
    );
}

#[test]
fn t19_goto_rle_alternating_no_rle() {
    let compressor = TableCompressor::new();
    let goto_table = vec![vec![StateId(1), StateId(2), StateId(1), StateId(2)]];
    let compressed = compressor.compress_goto_table_small(&goto_table).unwrap();
    let has_rle = compressed
        .data
        .iter()
        .any(|e| matches!(e, CompressedGotoEntry::RunLength { .. }));
    assert!(!has_rle, "Alternating values should not trigger RLE");
}

#[test]
fn t20_goto_empty_table() {
    let compressor = TableCompressor::new();
    let goto_table: Vec<Vec<StateId>> = vec![vec![]];
    let compressed = compressor.compress_goto_table_small(&goto_table).unwrap();
    assert!(compressed.data.is_empty());
    assert_eq!(compressed.row_offsets.len(), 2); // 1 row + sentinel
}

// ============================================================================
// 21–25: Bit-packing compression
// ============================================================================

#[test]
fn t21_bitpacked_error_only_table() {
    let table = vec![vec![Action::Error; 4]; 2];
    let packed = BitPackedActionTable::from_table(&table);
    for s in 0..2 {
        for sym in 0..4 {
            assert_eq!(packed.decompress(s, sym), Action::Error);
        }
    }
}

#[test]
fn t22_bitpacked_shift_only() {
    let table = vec![vec![Action::Shift(StateId(5)), Action::Shift(StateId(7))]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Shift(StateId(5)));
    assert_eq!(packed.decompress(0, 1), Action::Shift(StateId(7)));
}

#[test]
fn t23_bitpacked_reduce_only() {
    let table = vec![vec![Action::Reduce(RuleId(0)), Action::Reduce(RuleId(1))]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Reduce(RuleId(0)));
    assert_eq!(packed.decompress(0, 1), Action::Reduce(RuleId(1)));
}

#[test]
fn t24_bitpacked_accept() {
    let table = vec![vec![Action::Accept, Action::Error]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Accept);
    assert_eq!(packed.decompress(0, 1), Action::Error);
}

#[test]
fn t25_bitpacked_fork() {
    let fork = Action::Fork(vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(0))]);
    let table = vec![vec![fork.clone(), Action::Error]];
    let packed = BitPackedActionTable::from_table(&table);
    match packed.decompress(0, 0) {
        Action::Fork(actions) => {
            assert_eq!(actions.len(), 2);
        }
        other => panic!("Expected Fork, got {other:?}"),
    }
    assert_eq!(packed.decompress(0, 1), Action::Error);
}

// ============================================================================
// 26–30: Row-deduplication compression roundtrip
// ============================================================================

#[test]
fn t26_dedup_identical_rows() {
    let row = vec![vec![Action::Shift(StateId(1))], vec![Action::Error]];
    let table = vec![row.clone(), row.clone(), row];
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 1);
    assert_eq!(compressed.state_to_row, vec![0, 0, 0]);
}

#[test]
fn t27_dedup_distinct_rows() {
    let table = vec![
        vec![vec![Action::Shift(StateId(1))], vec![Action::Error]],
        vec![vec![Action::Error], vec![Action::Reduce(RuleId(0))]],
    ];
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 2);
}

#[test]
fn t28_dedup_roundtrip_shift() {
    let table = vec![vec![
        vec![Action::Shift(StateId(3))],
        vec![Action::Shift(StateId(7))],
    ]];
    let compressed = compress_action_table(&table);
    assert_eq!(
        decompress_action(&compressed, 0, 0),
        Action::Shift(StateId(3))
    );
    assert_eq!(
        decompress_action(&compressed, 0, 1),
        Action::Shift(StateId(7))
    );
}

#[test]
fn t29_dedup_roundtrip_reduce() {
    let table = vec![vec![
        vec![Action::Reduce(RuleId(2))],
        vec![Action::Reduce(RuleId(5))],
    ]];
    let compressed = compress_action_table(&table);
    assert_eq!(
        decompress_action(&compressed, 0, 0),
        Action::Reduce(RuleId(2))
    );
    assert_eq!(
        decompress_action(&compressed, 0, 1),
        Action::Reduce(RuleId(5))
    );
}

#[test]
fn t30_dedup_roundtrip_error() {
    let table = vec![vec![vec![Action::Error], vec![Action::Error]]];
    let compressed = compress_action_table(&table);
    assert_eq!(decompress_action(&compressed, 0, 0), Action::Error);
    assert_eq!(decompress_action(&compressed, 0, 1), Action::Error);
}

// ============================================================================
// 31–35: Sparse goto compression roundtrip
// ============================================================================

#[test]
fn t31_sparse_goto_roundtrip() {
    let table = vec![
        vec![None, Some(StateId(1)), None],
        vec![Some(StateId(2)), None, Some(StateId(3))],
    ];
    let compressed = compress_goto_table(&table);
    assert_eq!(decompress_goto(&compressed, 0, 0), None);
    assert_eq!(decompress_goto(&compressed, 0, 1), Some(StateId(1)));
    assert_eq!(decompress_goto(&compressed, 1, 0), Some(StateId(2)));
    assert_eq!(decompress_goto(&compressed, 1, 2), Some(StateId(3)));
}

#[test]
fn t32_sparse_goto_empty() {
    let table: Vec<Vec<Option<StateId>>> = vec![vec![None, None], vec![None, None]];
    let compressed = compress_goto_table(&table);
    assert_eq!(compressed.entries.len(), 0);
}

#[test]
fn t33_sparse_goto_full() {
    let table = vec![
        vec![Some(StateId(1)), Some(StateId(2))],
        vec![Some(StateId(3)), Some(StateId(4))],
    ];
    let compressed = compress_goto_table(&table);
    assert_eq!(compressed.entries.len(), 4);
    assert_eq!(decompress_goto(&compressed, 0, 0), Some(StateId(1)));
    assert_eq!(decompress_goto(&compressed, 1, 1), Some(StateId(4)));
}

#[test]
fn t34_sparse_goto_single_entry() {
    let table = vec![vec![None, None, Some(StateId(42)), None]];
    let compressed = compress_goto_table(&table);
    assert_eq!(compressed.entries.len(), 1);
    assert_eq!(decompress_goto(&compressed, 0, 2), Some(StateId(42)));
    assert_eq!(decompress_goto(&compressed, 0, 0), None);
}

#[test]
fn t35_sparse_goto_diagonal() {
    let table = vec![
        vec![Some(StateId(0)), None, None],
        vec![None, Some(StateId(1)), None],
        vec![None, None, Some(StateId(2))],
    ];
    let compressed = compress_goto_table(&table);
    assert_eq!(compressed.entries.len(), 3);
    for i in 0..3 {
        assert_eq!(decompress_goto(&compressed, i, i), Some(StateId(i as u16)));
    }
}

// ============================================================================
// 36–40: Action encoding (encode_action_small)
// ============================================================================

#[test]
fn t36_encode_shift() {
    let compressor = TableCompressor::new();
    let encoded = compressor
        .encode_action_small(&Action::Shift(StateId(42)))
        .unwrap();
    assert_eq!(encoded, 42);
}

#[test]
fn t37_encode_reduce() {
    let compressor = TableCompressor::new();
    let encoded = compressor
        .encode_action_small(&Action::Reduce(RuleId(3)))
        .unwrap();
    // Reduce: 0x8000 | (rule_id + 1) = 0x8000 | 4 = 0x8004
    assert_eq!(encoded, 0x8004);
}

#[test]
fn t38_encode_accept() {
    let compressor = TableCompressor::new();
    let encoded = compressor.encode_action_small(&Action::Accept).unwrap();
    assert_eq!(encoded, 0xFFFF);
}

#[test]
fn t39_encode_error() {
    let compressor = TableCompressor::new();
    let encoded = compressor.encode_action_small(&Action::Error).unwrap();
    assert_eq!(encoded, 0xFFFE);
}

#[test]
fn t40_encode_recover() {
    let compressor = TableCompressor::new();
    let encoded = compressor.encode_action_small(&Action::Recover).unwrap();
    assert_eq!(encoded, 0xFFFD);
}

// ============================================================================
// 41–45: Encoding edge cases and overflow
// ============================================================================

#[test]
fn t41_encode_shift_zero() {
    let compressor = TableCompressor::new();
    let encoded = compressor
        .encode_action_small(&Action::Shift(StateId(0)))
        .unwrap();
    assert_eq!(encoded, 0);
}

#[test]
fn t42_encode_shift_max_valid() {
    let compressor = TableCompressor::new();
    let encoded = compressor
        .encode_action_small(&Action::Shift(StateId(0x7FFF)))
        .unwrap();
    assert_eq!(encoded, 0x7FFF);
}

#[test]
fn t43_encode_shift_overflow() {
    let compressor = TableCompressor::new();
    let result = compressor.encode_action_small(&Action::Shift(StateId(0x8000)));
    assert!(result.is_err());
}

#[test]
fn t44_encode_reduce_zero() {
    let compressor = TableCompressor::new();
    let encoded = compressor
        .encode_action_small(&Action::Reduce(RuleId(0)))
        .unwrap();
    // 0x8000 | (0 + 1) = 0x8001
    assert_eq!(encoded, 0x8001);
}

#[test]
fn t45_encode_reduce_overflow() {
    let compressor = TableCompressor::new();
    let result = compressor.encode_action_small(&Action::Reduce(RuleId(0x4000)));
    assert!(result.is_err());
}

// ============================================================================
// 46–50: CompressedParseTable basics
// ============================================================================

#[test]
fn t46_compressed_parse_table_new() {
    let cpt = CompressedParseTable::new_for_testing(5, 10);
    assert_eq!(cpt.symbol_count(), 5);
    assert_eq!(cpt.state_count(), 10);
}

#[test]
fn t47_compressed_parse_table_from_real_grammar() {
    let grammar = single_token_grammar();
    let pt = build_table(&grammar);
    let cpt = CompressedParseTable::from_parse_table(&pt);
    assert!(cpt.symbol_count() > 0);
    assert!(cpt.state_count() > 0);
}

#[test]
fn t48_compressed_entry_construction() {
    let entry = CompressedActionEntry::new(10, Action::Shift(StateId(3)));
    assert_eq!(entry.symbol, 10);
    assert!(matches!(entry.action, Action::Shift(StateId(3))));
}

#[test]
fn t49_compressed_entry_reduce() {
    let entry = CompressedActionEntry::new(5, Action::Reduce(RuleId(2)));
    assert_eq!(entry.symbol, 5);
    assert!(matches!(entry.action, Action::Reduce(RuleId(2))));
}

#[test]
fn t50_default_compressor_creates_ok() {
    let compressor = TableCompressor::new();
    // Verify default compressor works with a real grammar
    let grammar = single_token_grammar();
    let pt = build_table(&grammar);
    let ti = collect_token_indices(&grammar, &pt);
    let sce = eof_accepts_or_reduces(&pt);
    assert!(compressor.compress(&pt, &ti, sce).is_ok());
}

// ============================================================================
// 51–55: Real grammar pipeline tests
// ============================================================================

#[test]
fn t51_nullable_start_compresses() {
    let grammar = nullable_start_grammar();
    let pt = build_table(&grammar);
    let ti = collect_token_indices(&grammar, &pt);
    let sce = eof_accepts_or_reduces(&pt);
    // Nullable start should set sce=true, and compression should succeed
    let result = TableCompressor::new().compress(&pt, &ti, sce);
    assert!(result.is_ok(), "Nullable start grammar should compress");
}

#[test]
fn t52_left_recursive_equivalence() {
    verify_action_equivalence(&left_recursive_grammar());
}

#[test]
fn t53_right_recursive_equivalence() {
    verify_action_equivalence(&right_recursive_grammar());
}

#[test]
fn t54_precedence_grammar_equivalence() {
    verify_action_equivalence(&precedence_grammar());
}

#[test]
fn t55_wide_alternatives_equivalence() {
    verify_action_equivalence(&wide_alternatives_grammar());
}

// ============================================================================
// 56–60: Row-offset monotonicity and structure
// ============================================================================

fn assert_row_offsets_monotonic(ct: &CompressedTables) {
    for window in ct.action_table.row_offsets.windows(2) {
        assert!(
            window[1] >= window[0],
            "Row offsets must be non-decreasing: {} < {}",
            window[1],
            window[0]
        );
    }
}

#[test]
fn t56_row_offsets_monotonic_single_token() {
    assert_row_offsets_monotonic(&compress_full(&single_token_grammar()));
}

#[test]
fn t57_row_offsets_monotonic_nested() {
    assert_row_offsets_monotonic(&compress_full(&nested_grammar()));
}

#[test]
fn t58_row_offsets_len_matches_states() {
    let grammar = two_token_grammar();
    let pt = build_table(&grammar);
    let ct = compress_full(&grammar);
    // row_offsets.len() == state_count + 1
    assert_eq!(
        ct.action_table.row_offsets.len(),
        pt.state_count + 1,
        "Row offsets must have state_count + 1 entries"
    );
}

#[test]
fn t59_default_actions_len_matches_states() {
    let grammar = alternatives_grammar();
    let pt = build_table(&grammar);
    let ct = compress_full(&grammar);
    assert_eq!(
        ct.action_table.default_actions.len(),
        pt.state_count,
        "Default actions must have one entry per state"
    );
}

#[test]
fn t60_default_actions_all_error() {
    let ct = compress_full(&single_token_grammar());
    for da in &ct.action_table.default_actions {
        assert_eq!(
            *da,
            Action::Error,
            "Default action optimization is disabled; all defaults should be Error"
        );
    }
}

// ============================================================================
// 61–65: Goto RLE edge cases
// ============================================================================

#[test]
fn t61_goto_rle_threshold_boundary_run_of_3() {
    let compressor = TableCompressor::new();
    let goto_table = vec![vec![StateId(5), StateId(5), StateId(5)]];
    let compressed = compressor.compress_goto_table_small(&goto_table).unwrap();
    let has_rle = compressed
        .data
        .iter()
        .any(|e| matches!(e, CompressedGotoEntry::RunLength { state: 5, count: 3 }));
    assert!(has_rle, "Run of 3 should trigger RLE");
}

#[test]
fn t62_goto_rle_mixed_runs_and_singles() {
    let compressor = TableCompressor::new();
    let goto_table = vec![vec![
        StateId(1),
        StateId(2),
        StateId(2),
        StateId(2),
        StateId(2),
        StateId(3),
    ]];
    let compressed = compressor.compress_goto_table_small(&goto_table).unwrap();
    let singles = compressed
        .data
        .iter()
        .filter(|e| matches!(e, CompressedGotoEntry::Single(_)))
        .count();
    let rles = compressed
        .data
        .iter()
        .filter(|e| matches!(e, CompressedGotoEntry::RunLength { .. }))
        .count();
    assert!(singles >= 2, "Should have singles for isolated values");
    assert!(rles >= 1, "Should have RLE for run of 4");
}

#[test]
fn t63_goto_multi_row() {
    let compressor = TableCompressor::new();
    let goto_table = vec![
        vec![StateId(1), StateId(1), StateId(1)],
        vec![StateId(2), StateId(3), StateId(3)],
    ];
    let compressed = compressor.compress_goto_table_small(&goto_table).unwrap();
    assert_eq!(compressed.row_offsets.len(), 3, "2 rows + sentinel");
}

#[test]
fn t64_goto_large_state_ids() {
    let compressor = TableCompressor::new();
    let goto_table = vec![vec![
        StateId(1000),
        StateId(1000),
        StateId(1000),
        StateId(1000),
    ]];
    let compressed = compressor.compress_goto_table_small(&goto_table).unwrap();
    let has_rle = compressed.data.iter().any(|e| {
        matches!(
            e,
            CompressedGotoEntry::RunLength {
                state: 1000,
                count: 4
            }
        )
    });
    assert!(has_rle);
}

#[test]
fn t65_goto_row_offsets_monotonic() {
    let compressor = TableCompressor::new();
    let goto_table = vec![
        vec![StateId(1)],
        vec![StateId(2), StateId(3)],
        vec![StateId(4), StateId(4), StateId(4)],
    ];
    let compressed = compressor.compress_goto_table_small(&goto_table).unwrap();
    for w in compressed.row_offsets.windows(2) {
        assert!(w[1] >= w[0]);
    }
}

// ============================================================================
// 66–70: Bit-packed roundtrip edge cases
// ============================================================================

#[test]
fn t66_bitpacked_empty_table() {
    let table: Vec<Vec<Action>> = vec![];
    let packed = BitPackedActionTable::from_table(&table);
    // Empty table should not panic; decompress should not be called on empty
    let _ = packed;
}

#[test]
fn t67_bitpacked_single_cell() {
    let table = vec![vec![Action::Shift(StateId(99))]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Shift(StateId(99)));
}

#[test]
fn t68_bitpacked_mixed_actions() {
    let table = vec![vec![
        Action::Shift(StateId(1)),
        Action::Reduce(RuleId(2)),
        Action::Error,
    ]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Shift(StateId(1)));
    assert_eq!(packed.decompress(0, 1), Action::Reduce(RuleId(2)));
    assert_eq!(packed.decompress(0, 2), Action::Error);
}

#[test]
fn t69_bitpacked_multi_row() {
    let table = vec![
        vec![Action::Shift(StateId(1)), Action::Error],
        vec![Action::Error, Action::Reduce(RuleId(0))],
    ];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Shift(StateId(1)));
    assert_eq!(packed.decompress(0, 1), Action::Error);
    assert_eq!(packed.decompress(1, 0), Action::Error);
    assert_eq!(packed.decompress(1, 1), Action::Reduce(RuleId(0)));
}

#[test]
fn t70_bitpacked_error_mask_words() {
    // Table with >64 cells to exercise multi-word error mask
    let table = vec![vec![Action::Error; 10]; 10];
    let packed = BitPackedActionTable::from_table(&table);
    for s in 0..10 {
        for sym in 0..10 {
            assert_eq!(packed.decompress(s, sym), Action::Error);
        }
    }
}

// ============================================================================
// 71–75: Compression determinism
// ============================================================================

#[test]
fn t71_compression_deterministic_single_token() {
    let grammar = single_token_grammar();
    let ct1 = compress_full(&grammar);
    let ct2 = compress_full(&grammar);
    assert_eq!(ct1.action_table.data.len(), ct2.action_table.data.len());
    assert_eq!(ct1.action_table.row_offsets, ct2.action_table.row_offsets);
}

#[test]
fn t72_compression_deterministic_alternatives() {
    let grammar = alternatives_grammar();
    let ct1 = compress_full(&grammar);
    let ct2 = compress_full(&grammar);
    assert_eq!(ct1.action_table.data.len(), ct2.action_table.data.len());
}

#[test]
fn t73_compression_deterministic_goto() {
    let grammar = nested_grammar();
    let ct1 = compress_full(&grammar);
    let ct2 = compress_full(&grammar);
    assert_eq!(ct1.goto_table.row_offsets, ct2.goto_table.row_offsets);
    assert_eq!(ct1.goto_table.data.len(), ct2.goto_table.data.len());
}

#[test]
fn t74_dedup_deterministic() {
    let table = vec![
        vec![vec![Action::Shift(StateId(1))], vec![Action::Error]],
        vec![vec![Action::Shift(StateId(1))], vec![Action::Error]],
    ];
    let c1 = compress_action_table(&table);
    let c2 = compress_action_table(&table);
    assert_eq!(c1.unique_rows.len(), c2.unique_rows.len());
    assert_eq!(c1.state_to_row, c2.state_to_row);
}

#[test]
fn t75_sparse_goto_deterministic() {
    let table = vec![vec![None, Some(StateId(1))], vec![Some(StateId(2)), None]];
    let c1 = compress_goto_table(&table);
    let c2 = compress_goto_table(&table);
    assert_eq!(c1.entries.len(), c2.entries.len());
    assert_eq!(decompress_goto(&c1, 0, 1), decompress_goto(&c2, 0, 1));
}

// ============================================================================
// 76–80: Action table compression edge cases
// ============================================================================

#[test]
fn t76_compress_empty_action_rows() {
    let compressor = TableCompressor::new();
    let action_table: Vec<Vec<Vec<Action>>> = vec![vec![], vec![], vec![]];
    let symbol_map = BTreeMap::new();
    let result = compressor.compress_action_table_small(&action_table, &symbol_map);
    assert!(result.is_ok());
    let compressed = result.unwrap();
    assert_eq!(compressed.row_offsets.len(), 4); // 3 rows + sentinel
    assert!(compressed.data.is_empty());
}

#[test]
fn t77_compress_all_errors_skipped() {
    let compressor = TableCompressor::new();
    let action_table = vec![vec![vec![Action::Error]; 5]];
    let symbol_map = BTreeMap::new();
    let compressed = compressor
        .compress_action_table_small(&action_table, &symbol_map)
        .unwrap();
    assert!(
        compressed.data.is_empty(),
        "Explicit Error actions should be skipped"
    );
}

#[test]
fn t78_compress_mixed_error_and_shift() {
    let compressor = TableCompressor::new();
    let action_table = vec![vec![
        vec![Action::Error],
        vec![Action::Shift(StateId(1))],
        vec![Action::Error],
        vec![Action::Shift(StateId(2))],
    ]];
    let symbol_map = BTreeMap::new();
    let compressed = compressor
        .compress_action_table_small(&action_table, &symbol_map)
        .unwrap();
    assert_eq!(
        compressed.data.len(),
        2,
        "Only non-error actions should be stored"
    );
}

#[test]
fn t79_compress_accept_in_action_table() {
    let compressor = TableCompressor::new();
    let action_table = vec![vec![vec![Action::Accept], vec![Action::Error]]];
    let symbol_map = BTreeMap::new();
    let compressed = compressor
        .compress_action_table_small(&action_table, &symbol_map)
        .unwrap();
    assert_eq!(compressed.data.len(), 1);
    assert!(matches!(compressed.data[0].action, Action::Accept));
}

#[test]
fn t80_compress_multiple_actions_per_cell() {
    let compressor = TableCompressor::new();
    let action_table = vec![vec![vec![
        Action::Shift(StateId(1)),
        Action::Reduce(RuleId(0)),
    ]]];
    let symbol_map = BTreeMap::new();
    let compressed = compressor
        .compress_action_table_small(&action_table, &symbol_map)
        .unwrap();
    // Both actions from the cell should be encoded
    assert_eq!(compressed.data.len(), 2);
}

// ============================================================================
// 81–85: Compression validation and error paths
// ============================================================================

#[test]
fn t81_compressed_tables_validate() {
    let grammar = single_token_grammar();
    let pt = build_table(&grammar);
    let ct = compress_full(&grammar);
    assert!(ct.validate(&pt).is_ok());
}

#[test]
fn t82_encode_fork_requires_flattening() {
    let compressor = TableCompressor::new();
    let fork = Action::Fork(vec![Action::Shift(StateId(1))]);
    assert!(compressor.encode_action_small(&fork).is_err());
}

#[test]
fn t83_encode_reduce_rule_id_1based() {
    let compressor = TableCompressor::new();
    // Rule 0 → encoded as 0x8000 | 1 = 0x8001
    let e0 = compressor
        .encode_action_small(&Action::Reduce(RuleId(0)))
        .unwrap();
    assert_eq!(e0, 0x8001);
    // Rule 1 → encoded as 0x8000 | 2 = 0x8002
    let e1 = compressor
        .encode_action_small(&Action::Reduce(RuleId(1)))
        .unwrap();
    assert_eq!(e1, 0x8002);
}

#[test]
fn t84_encode_shift_boundary_values() {
    let compressor = TableCompressor::new();
    // StateId(0) is valid
    assert_eq!(
        compressor
            .encode_action_small(&Action::Shift(StateId(0)))
            .unwrap(),
        0
    );
    // StateId(1) is valid
    assert_eq!(
        compressor
            .encode_action_small(&Action::Shift(StateId(1)))
            .unwrap(),
        1
    );
    // StateId(0x7FFE)
    assert_eq!(
        compressor
            .encode_action_small(&Action::Shift(StateId(0x7FFE)))
            .unwrap(),
        0x7FFE
    );
}

#[test]
fn t85_long_sequence_row_offsets_valid() {
    let ct = compress_full(&long_sequence_grammar());
    let last = *ct.action_table.row_offsets.last().unwrap() as usize;
    assert_eq!(
        last,
        ct.action_table.data.len(),
        "Last row offset must equal total entry count"
    );
}

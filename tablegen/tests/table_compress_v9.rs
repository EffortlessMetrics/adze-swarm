//! Comprehensive tests for `TableCompressor` in `adze-tablegen`.
//!
//! 80+ tests covering:
//! 1. TableCompressor::new() / Default
//! 2. CompressedParseTable::new_for_testing various dimensions
//! 3. CompressedParseTable::from_parse_table
//! 4. compress_action_table (row deduplication)
//! 5. compress_goto_table (sparse representation)
//! 6. BitPackedActionTable roundtrip
//! 7. Full pipeline: grammar → first/follow → LR(1) → compress
//! 8. Determinism and idempotency
//! 9. Precedence and conflict grammars
//! 10. Edge cases and various grammar sizes

use adze_glr_core::{Action, FirstFollowSets, build_lr1_automaton};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{Associativity, RuleId, StateId};
use adze_tablegen::compress::{CompressedParseTable, CompressedTables, TableCompressor};
use adze_tablegen::compression::{
    BitPackedActionTable, compress_action_table, compress_goto_table, decompress_action,
    decompress_goto,
};
use std::collections::BTreeMap;

// =============================================================================
// Helpers
// =============================================================================

/// Build a parse table from a GrammarBuilder.
fn build_table(
    name: &str,
    build_fn: impl FnOnce(GrammarBuilder) -> GrammarBuilder,
) -> adze_glr_core::ParseTable {
    let gb = GrammarBuilder::new(name);
    let mut g = build_fn(gb).build();
    let ff = FirstFollowSets::compute_normalized(&mut g).expect("first/follow");
    build_lr1_automaton(&g, &ff).expect("lr1 automaton")
}

/// S → a
fn table_single_token(name: &str) -> adze_glr_core::ParseTable {
    build_table(name, |gb| {
        gb.token("a", "a").rule("start", vec!["a"]).start("start")
    })
}

/// S → a | b
fn table_two_alt(name: &str) -> adze_glr_core::ParseTable {
    build_table(name, |gb| {
        gb.token("a", "a")
            .token("b", "b")
            .rule("start", vec!["a"])
            .rule("start", vec!["b"])
            .start("start")
    })
}

/// S → a b c
fn table_sequence(name: &str) -> adze_glr_core::ParseTable {
    build_table(name, |gb| {
        gb.token("a", "a")
            .token("b", "b")
            .token("c", "c")
            .rule("start", vec!["a", "b", "c"])
            .start("start")
    })
}

/// S → r ; r → a r | ε  (nullable/recursive)
fn table_nullable(name: &str) -> adze_glr_core::ParseTable {
    build_table(name, |gb| {
        gb.token("a", "a")
            .rule("start", vec!["r"])
            .rule("r", vec!["a", "r"])
            .rule("r", vec![])
            .start("start")
    })
}

/// S → x | y ; x → a ; y → b
fn table_multi_nt(name: &str) -> adze_glr_core::ParseTable {
    build_table(name, |gb| {
        gb.token("a", "a")
            .token("b", "b")
            .rule("start", vec!["x"])
            .rule("start", vec!["y"])
            .rule("x", vec!["a"])
            .rule("y", vec!["b"])
            .start("start")
    })
}

/// S → p | q ; p → a b ; q → a c
fn table_shared_prefix(name: &str) -> adze_glr_core::ParseTable {
    build_table(name, |gb| {
        gb.token("a", "a")
            .token("b", "b")
            .token("c", "c")
            .rule("start", vec!["p"])
            .rule("start", vec!["q"])
            .rule("p", vec!["a", "b"])
            .rule("q", vec!["a", "c"])
            .start("start")
    })
}

/// S → e ; e → e '+' e | n (ambiguous, with precedence)
fn table_with_precedence(name: &str) -> adze_glr_core::ParseTable {
    build_table(name, |gb| {
        gb.token("n", "[0-9]+")
            .token("plus", "\\+")
            .rule("start", vec!["e"])
            .rule_with_precedence("e", vec!["e", "plus", "e"], 1, Associativity::Left)
            .rule("e", vec!["n"])
            .start("start")
    })
}

/// Collect sorted deduped token indices.
fn token_indices(pt: &adze_glr_core::ParseTable) -> Vec<usize> {
    let mut v: Vec<usize> = pt.symbol_to_index.values().copied().collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Detect nullable start.
fn start_nullable(pt: &adze_glr_core::ParseTable) -> bool {
    adze_tablegen::eof_accepts_or_reduces(pt)
}

/// Full compression pipeline.
fn compress_full(pt: &adze_glr_core::ParseTable) -> CompressedTables {
    let ti = token_indices(pt);
    let sn = start_nullable(pt);
    TableCompressor::new().compress(pt, &ti, sn).unwrap()
}

// =============================================================================
// 1. TableCompressor construction
// =============================================================================

#[test]
fn test_01_new_does_not_panic() {
    let _ = TableCompressor::new();
}

#[test]
fn test_02_default_does_not_panic() {
    let _: TableCompressor = Default::default();
}

#[test]
fn test_03_new_and_default_equivalent() {
    // Both paths produce usable compressors.
    let a = TableCompressor::new();
    let b = TableCompressor::default();
    // Encode the same action to verify equivalent behaviour.
    let va = a.encode_action_small(&Action::Accept).unwrap();
    let vb = b.encode_action_small(&Action::Accept).unwrap();
    assert_eq!(va, vb);
}

// =============================================================================
// 2. CompressedParseTable::new_for_testing dimensions
// =============================================================================

#[test]
fn test_04_new_for_testing_1x1() {
    let t = CompressedParseTable::new_for_testing(1, 1);
    assert_eq!(t.symbol_count(), 1);
    assert_eq!(t.state_count(), 1);
}

#[test]
fn test_05_new_for_testing_5x5() {
    let t = CompressedParseTable::new_for_testing(5, 5);
    assert_eq!(t.symbol_count(), 5);
    assert_eq!(t.state_count(), 5);
}

#[test]
fn test_06_new_for_testing_10x10() {
    let t = CompressedParseTable::new_for_testing(10, 10);
    assert_eq!(t.symbol_count(), 10);
    assert_eq!(t.state_count(), 10);
}

#[test]
fn test_07_new_for_testing_20x20() {
    let t = CompressedParseTable::new_for_testing(20, 20);
    assert_eq!(t.symbol_count(), 20);
    assert_eq!(t.state_count(), 20);
}

#[test]
fn test_08_new_for_testing_50x50() {
    let t = CompressedParseTable::new_for_testing(50, 50);
    assert_eq!(t.symbol_count(), 50);
    assert_eq!(t.state_count(), 50);
}

#[test]
fn test_09_new_for_testing_asymmetric() {
    let t = CompressedParseTable::new_for_testing(100, 3);
    assert_eq!(t.symbol_count(), 100);
    assert_eq!(t.state_count(), 3);
}

#[test]
fn test_10_new_for_testing_zero_zero() {
    let t = CompressedParseTable::new_for_testing(0, 0);
    assert_eq!(t.symbol_count(), 0);
    assert_eq!(t.state_count(), 0);
}

#[test]
fn test_11_new_for_testing_large() {
    let t = CompressedParseTable::new_for_testing(100_000, 50_000);
    assert_eq!(t.symbol_count(), 100_000);
    assert_eq!(t.state_count(), 50_000);
}

// =============================================================================
// 3. CompressedParseTable::from_parse_table
// =============================================================================

#[test]
fn test_12_from_parse_table_no_panic() {
    let pt = table_single_token("tc_v9_from_np");
    let _ = CompressedParseTable::from_parse_table(&pt);
}

#[test]
fn test_13_from_parse_table_preserves_state_count() {
    let pt = table_single_token("tc_v9_sc");
    let cpt = CompressedParseTable::from_parse_table(&pt);
    assert_eq!(cpt.state_count(), pt.state_count);
}

#[test]
fn test_14_from_parse_table_preserves_symbol_count() {
    let pt = table_single_token("tc_v9_syc");
    let cpt = CompressedParseTable::from_parse_table(&pt);
    assert_eq!(cpt.symbol_count(), pt.symbol_count);
}

#[test]
fn test_15_from_parse_table_two_alt() {
    let pt = table_two_alt("tc_v9_2alt");
    let cpt = CompressedParseTable::from_parse_table(&pt);
    assert_eq!(cpt.state_count(), pt.state_count);
    assert_eq!(cpt.symbol_count(), pt.symbol_count);
}

#[test]
fn test_16_from_parse_table_sequence() {
    let pt = table_sequence("tc_v9_seq");
    let cpt = CompressedParseTable::from_parse_table(&pt);
    assert!(cpt.state_count() >= 2);
}

#[test]
fn test_17_from_parse_table_nullable() {
    let pt = table_nullable("tc_v9_null");
    let cpt = CompressedParseTable::from_parse_table(&pt);
    assert!(cpt.state_count() >= 1);
    assert!(cpt.symbol_count() >= 2);
}

#[test]
fn test_18_from_parse_table_multi_nt() {
    let pt = table_multi_nt("tc_v9_mnt");
    let cpt = CompressedParseTable::from_parse_table(&pt);
    assert_eq!(cpt.state_count(), pt.state_count);
}

#[test]
fn test_19_from_parse_table_shared_prefix() {
    let pt = table_shared_prefix("tc_v9_sp");
    let cpt = CompressedParseTable::from_parse_table(&pt);
    assert!(cpt.state_count() >= 3);
}

// =============================================================================
// 4. compress_action_table (row deduplication from compression module)
// =============================================================================

#[test]
fn test_20_compress_action_table_no_panic() {
    let table = vec![vec![vec![Action::Error]]];
    let _ = compress_action_table(&table);
}

#[test]
fn test_21_compress_action_table_empty_row() {
    let table: Vec<Vec<Vec<Action>>> = vec![vec![]];
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 1);
    assert_eq!(compressed.state_to_row.len(), 1);
}

#[test]
fn test_22_compress_action_table_deduplicates() {
    let row = vec![vec![Action::Shift(StateId(1))], vec![Action::Error]];
    let table = vec![row.clone(), row.clone(), row];
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 1);
    assert_eq!(compressed.state_to_row.len(), 3);
}

#[test]
fn test_23_compress_action_table_distinct_rows() {
    let r1 = vec![vec![Action::Shift(StateId(1))]];
    let r2 = vec![vec![Action::Reduce(RuleId(0))]];
    let table = vec![r1, r2];
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 2);
}

#[test]
fn test_24_decompress_action_roundtrip() {
    let table = vec![
        vec![vec![Action::Shift(StateId(1))], vec![Action::Error]],
        vec![vec![Action::Reduce(RuleId(0))], vec![Action::Accept]],
    ];
    let compressed = compress_action_table(&table);
    assert_eq!(
        decompress_action(&compressed, 0, 0),
        Action::Shift(StateId(1))
    );
    assert_eq!(decompress_action(&compressed, 0, 1), Action::Error);
    assert_eq!(
        decompress_action(&compressed, 1, 0),
        Action::Reduce(RuleId(0))
    );
    assert_eq!(decompress_action(&compressed, 1, 1), Action::Accept);
}

#[test]
fn test_25_compress_action_table_many_states() {
    let mut table = Vec::new();
    for i in 0..20u16 {
        table.push(vec![vec![Action::Shift(StateId(i))]]);
    }
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 20);
    assert_eq!(compressed.state_to_row.len(), 20);
}

#[test]
fn test_26_compress_action_table_all_errors() {
    let table = vec![vec![vec![Action::Error]; 5]; 4];
    let compressed = compress_action_table(&table);
    // All rows identical → 1 unique row
    assert_eq!(compressed.unique_rows.len(), 1);
}

// =============================================================================
// 5. compress_goto_table (sparse representation)
// =============================================================================

#[test]
fn test_27_compress_goto_table_no_panic() {
    let table: Vec<Vec<Option<StateId>>> = vec![vec![None]];
    let _ = compress_goto_table(&table);
}

#[test]
fn test_28_compress_goto_table_empty() {
    let table: Vec<Vec<Option<StateId>>> = vec![vec![None; 3]; 2];
    let compressed = compress_goto_table(&table);
    assert!(compressed.entries.is_empty());
}

#[test]
fn test_29_compress_goto_table_single_entry() {
    let mut table = vec![vec![None; 3]; 2];
    table[0][1] = Some(StateId(5));
    let compressed = compress_goto_table(&table);
    assert_eq!(compressed.entries.len(), 1);
    assert_eq!(decompress_goto(&compressed, 0, 1), Some(StateId(5)));
}

#[test]
fn test_30_compress_goto_sparse_roundtrip() {
    let mut table = vec![vec![None; 4]; 3];
    table[0][0] = Some(StateId(1));
    table[1][2] = Some(StateId(3));
    table[2][3] = Some(StateId(0));
    let compressed = compress_goto_table(&table);
    assert_eq!(decompress_goto(&compressed, 0, 0), Some(StateId(1)));
    assert_eq!(decompress_goto(&compressed, 0, 1), None);
    assert_eq!(decompress_goto(&compressed, 1, 2), Some(StateId(3)));
    assert_eq!(decompress_goto(&compressed, 2, 3), Some(StateId(0)));
    assert_eq!(decompress_goto(&compressed, 2, 0), None);
}

#[test]
fn test_31_compress_goto_all_filled() {
    let table = vec![
        vec![Some(StateId(1)), Some(StateId(2))],
        vec![Some(StateId(3)), Some(StateId(4))],
    ];
    let compressed = compress_goto_table(&table);
    assert_eq!(compressed.entries.len(), 4);
}

// =============================================================================
// 6. BitPackedActionTable
// =============================================================================

#[test]
fn test_32_bitpacked_from_empty() {
    let table: Vec<Vec<Action>> = vec![];
    let bp = BitPackedActionTable::from_table(&table);
    let _ = bp; // does not panic
}

#[test]
fn test_33_bitpacked_single_error() {
    let table = vec![vec![Action::Error]];
    let bp = BitPackedActionTable::from_table(&table);
    assert_eq!(bp.decompress(0, 0), Action::Error);
}

#[test]
fn test_34_bitpacked_single_shift() {
    let table = vec![vec![Action::Shift(StateId(7))]];
    let bp = BitPackedActionTable::from_table(&table);
    assert_eq!(bp.decompress(0, 0), Action::Shift(StateId(7)));
}

#[test]
fn test_35_bitpacked_mixed_row() {
    let table = vec![vec![
        Action::Shift(StateId(1)),
        Action::Reduce(RuleId(0)),
        Action::Error,
    ]];
    let bp = BitPackedActionTable::from_table(&table);
    assert_eq!(bp.decompress(0, 0), Action::Shift(StateId(1)));
    assert_eq!(bp.decompress(0, 2), Action::Error);
}

#[test]
fn test_36_bitpacked_accept() {
    let table = vec![vec![Action::Accept]];
    let bp = BitPackedActionTable::from_table(&table);
    let action = bp.decompress(0, 0);
    // Accept is stored as special reduce with u32::MAX
    assert!(matches!(action, Action::Accept));
}

// =============================================================================
// 7. Full pipeline: grammar → compress (compress_action_table_small / goto)
// =============================================================================

#[test]
fn test_37_compress_single_token_no_panic() {
    let pt = table_single_token("tc_v9_c1");
    let _ = compress_full(&pt);
}

#[test]
fn test_38_compress_two_alt_no_panic() {
    let pt = table_two_alt("tc_v9_c2");
    let _ = compress_full(&pt);
}

#[test]
fn test_39_compress_sequence_no_panic() {
    let pt = table_sequence("tc_v9_c3");
    let _ = compress_full(&pt);
}

#[test]
fn test_40_compress_nullable_no_panic() {
    let pt = table_nullable("tc_v9_c4");
    let _ = compress_full(&pt);
}

#[test]
fn test_41_compress_multi_nt_no_panic() {
    let pt = table_multi_nt("tc_v9_c5");
    let _ = compress_full(&pt);
}

#[test]
fn test_42_compress_shared_prefix_no_panic() {
    let pt = table_shared_prefix("tc_v9_c6");
    let _ = compress_full(&pt);
}

#[test]
fn test_43_compress_precedence_no_panic() {
    let pt = table_with_precedence("tc_v9_c7");
    let _ = compress_full(&pt);
}

#[test]
fn test_44_compressed_action_table_has_entries() {
    let pt = table_single_token("tc_v9_ae");
    let ct = compress_full(&pt);
    assert!(!ct.action_table.data.is_empty());
}

#[test]
fn test_45_compressed_goto_row_offsets_nonempty() {
    let pt = table_single_token("tc_v9_gro");
    let ct = compress_full(&pt);
    assert!(!ct.goto_table.row_offsets.is_empty());
}

#[test]
fn test_46_compressed_action_row_offsets_monotonic() {
    let pt = table_two_alt("tc_v9_arm");
    let ct = compress_full(&pt);
    for pair in ct.action_table.row_offsets.windows(2) {
        assert!(pair[1] >= pair[0]);
    }
}

#[test]
fn test_47_compressed_goto_row_offsets_monotonic() {
    let pt = table_sequence("tc_v9_grm");
    let ct = compress_full(&pt);
    for pair in ct.goto_table.row_offsets.windows(2) {
        assert!(pair[1] >= pair[0]);
    }
}

#[test]
fn test_48_compressed_action_row_offsets_length() {
    let pt = table_single_token("tc_v9_arl");
    let ct = compress_full(&pt);
    // row_offsets has state_count + 1 entries
    assert_eq!(ct.action_table.row_offsets.len(), pt.state_count + 1);
}

#[test]
fn test_49_compressed_default_actions_length() {
    let pt = table_two_alt("tc_v9_dal");
    let ct = compress_full(&pt);
    assert_eq!(ct.action_table.default_actions.len(), pt.state_count);
}

#[test]
fn test_50_compress_preserves_shift_actions() {
    let pt = table_single_token("tc_v9_psa");
    let ct = compress_full(&pt);
    let has_shift = ct
        .action_table
        .data
        .iter()
        .any(|e| matches!(e.action, Action::Shift(_)));
    assert!(has_shift);
}

#[test]
fn test_51_compress_preserves_reduce_actions() {
    let pt = table_single_token("tc_v9_pra");
    let ct = compress_full(&pt);
    let has_reduce = ct
        .action_table
        .data
        .iter()
        .any(|e| matches!(e.action, Action::Reduce(_)));
    assert!(has_reduce);
}

#[test]
fn test_52_compress_has_accept_action() {
    let pt = table_single_token("tc_v9_acc");
    let ct = compress_full(&pt);
    let has_accept = ct
        .action_table
        .data
        .iter()
        .any(|e| matches!(e.action, Action::Accept));
    assert!(has_accept);
}

#[test]
fn test_53_compressed_validate_ok() {
    let pt = table_single_token("tc_v9_val");
    let ct = compress_full(&pt);
    assert!(ct.validate(&pt).is_ok());
}

// =============================================================================
// 8. Determinism: same input → same compressed output
// =============================================================================

#[test]
fn test_54_determinism_action_table() {
    let pt = table_two_alt("tc_v9_det1");
    let c1 = compress_full(&pt);
    let c2 = compress_full(&pt);
    assert_eq!(c1.action_table.data.len(), c2.action_table.data.len());
    for (a, b) in c1.action_table.data.iter().zip(&c2.action_table.data) {
        assert_eq!(a.symbol, b.symbol);
        assert_eq!(a.action, b.action);
    }
}

#[test]
fn test_55_determinism_goto_table() {
    let pt = table_multi_nt("tc_v9_det2");
    let c1 = compress_full(&pt);
    let c2 = compress_full(&pt);
    assert_eq!(c1.goto_table.row_offsets, c2.goto_table.row_offsets);
}

#[test]
fn test_56_determinism_row_offsets() {
    let pt = table_sequence("tc_v9_det3");
    let c1 = compress_full(&pt);
    let c2 = compress_full(&pt);
    assert_eq!(c1.action_table.row_offsets, c2.action_table.row_offsets);
}

#[test]
fn test_57_determinism_default_actions() {
    let pt = table_nullable("tc_v9_det4");
    let c1 = compress_full(&pt);
    let c2 = compress_full(&pt);
    assert_eq!(
        c1.action_table.default_actions,
        c2.action_table.default_actions
    );
}

#[test]
fn test_58_triple_compression_identical() {
    let pt = table_shared_prefix("tc_v9_tri");
    let c1 = compress_full(&pt);
    let c2 = compress_full(&pt);
    let c3 = compress_full(&pt);
    assert_eq!(c1.action_table.data.len(), c2.action_table.data.len());
    assert_eq!(c2.action_table.data.len(), c3.action_table.data.len());
    assert_eq!(c1.goto_table.row_offsets, c3.goto_table.row_offsets);
}

// =============================================================================
// 9. Different grammars → different compressed outputs
// =============================================================================

#[test]
fn test_59_different_grammars_different_output() {
    let pt1 = table_single_token("tc_v9_dg1");
    let pt2 = table_two_alt("tc_v9_dg2");
    let c1 = compress_full(&pt1);
    let c2 = compress_full(&pt2);
    // Different grammars should differ in at least one dimension.
    let same_data_len = c1.action_table.data.len() == c2.action_table.data.len();
    let same_states = c1.action_table.row_offsets.len() == c2.action_table.row_offsets.len();
    assert!(
        !same_data_len || !same_states,
        "Different grammars should produce different compressed tables"
    );
}

#[test]
fn test_60_single_vs_sequence_differ() {
    let pt1 = table_single_token("tc_v9_diff1");
    let pt2 = table_sequence("tc_v9_diff2");
    let c1 = compress_full(&pt1);
    let c2 = compress_full(&pt2);
    assert_ne!(
        c1.action_table.row_offsets.len(),
        c2.action_table.row_offsets.len()
    );
}

// =============================================================================
// 10. Precedence grammars
// =============================================================================

#[test]
fn test_61_precedence_grammar_compresses() {
    let pt = table_with_precedence("tc_v9_prec1");
    let ct = compress_full(&pt);
    assert!(!ct.action_table.data.is_empty());
}

#[test]
fn test_62_precedence_preserves_state_count() {
    let pt = table_with_precedence("tc_v9_prec2");
    let ct = compress_full(&pt);
    assert_eq!(ct.action_table.default_actions.len(), pt.state_count);
}

#[test]
fn test_63_precedence_determinism() {
    let pt = table_with_precedence("tc_v9_prec3");
    let c1 = compress_full(&pt);
    let c2 = compress_full(&pt);
    assert_eq!(c1.action_table.data.len(), c2.action_table.data.len());
}

// =============================================================================
// 11. encode_action_small coverage
// =============================================================================

#[test]
fn test_64_encode_shift_zero() {
    let tc = TableCompressor::new();
    assert_eq!(
        tc.encode_action_small(&Action::Shift(StateId(0))).unwrap(),
        0
    );
}

#[test]
fn test_65_encode_shift_max_valid() {
    let tc = TableCompressor::new();
    assert_eq!(
        tc.encode_action_small(&Action::Shift(StateId(0x7FFF)))
            .unwrap(),
        0x7FFF
    );
}

#[test]
fn test_66_encode_shift_overflow() {
    let tc = TableCompressor::new();
    assert!(
        tc.encode_action_small(&Action::Shift(StateId(0x8000)))
            .is_err()
    );
}

#[test]
fn test_67_encode_reduce_zero() {
    let tc = TableCompressor::new();
    assert_eq!(
        tc.encode_action_small(&Action::Reduce(RuleId(0))).unwrap(),
        0x8001
    );
}

#[test]
fn test_68_encode_reduce_max_valid() {
    let tc = TableCompressor::new();
    assert_eq!(
        tc.encode_action_small(&Action::Reduce(RuleId(0x3FFF)))
            .unwrap(),
        0x8000 | 0x4000
    );
}

#[test]
fn test_69_encode_reduce_overflow() {
    let tc = TableCompressor::new();
    assert!(
        tc.encode_action_small(&Action::Reduce(RuleId(0x4000)))
            .is_err()
    );
}

#[test]
fn test_70_encode_accept() {
    let tc = TableCompressor::new();
    assert_eq!(tc.encode_action_small(&Action::Accept).unwrap(), 0xFFFF);
}

#[test]
fn test_71_encode_error() {
    let tc = TableCompressor::new();
    assert_eq!(tc.encode_action_small(&Action::Error).unwrap(), 0xFFFE);
}

#[test]
fn test_72_encode_recover() {
    let tc = TableCompressor::new();
    assert_eq!(tc.encode_action_small(&Action::Recover).unwrap(), 0xFFFD);
}

#[test]
fn test_73_encode_fork_requires_flattening() {
    let tc = TableCompressor::new();
    let fork = Action::Fork(vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(0))]);
    assert!(tc.encode_action_small(&fork).is_err());
}

// =============================================================================
// 12. compress_action_table_small via TableCompressor
// =============================================================================

#[test]
fn test_74_compress_action_table_small_empty() {
    let tc = TableCompressor::new();
    let at: Vec<Vec<Vec<Action>>> = vec![vec![]; 3];
    let sym = BTreeMap::new();
    let c = tc.compress_action_table_small(&at, &sym).unwrap();
    assert!(c.data.is_empty());
    assert_eq!(c.row_offsets.len(), 4);
}

#[test]
fn test_75_compress_action_table_small_single_shift() {
    let tc = TableCompressor::new();
    let at = vec![vec![vec![Action::Shift(StateId(2))]]];
    let sym = BTreeMap::new();
    let c = tc.compress_action_table_small(&at, &sym).unwrap();
    assert_eq!(c.data.len(), 1);
    assert_eq!(c.data[0].action, Action::Shift(StateId(2)));
}

#[test]
fn test_76_compress_action_table_small_errors_skipped() {
    let tc = TableCompressor::new();
    let at = vec![vec![vec![Action::Error], vec![Action::Error]]];
    let sym = BTreeMap::new();
    let c = tc.compress_action_table_small(&at, &sym).unwrap();
    assert!(c.data.is_empty());
}

#[test]
fn test_77_compress_goto_table_small_single_row() {
    let tc = TableCompressor::new();
    let gt = vec![vec![StateId(1), StateId(u16::MAX)]];
    let c = tc.compress_goto_table_small(&gt).unwrap();
    assert!(!c.row_offsets.is_empty());
}

// =============================================================================
// 13. Edge cases: smallest grammar
// =============================================================================

#[test]
fn test_78_smallest_grammar_compresses() {
    let pt = build_table("tc_v9_sm", |gb| {
        gb.token("x", "x").rule("start", vec!["x"]).start("start")
    });
    let ct = compress_full(&pt);
    assert!(!ct.action_table.data.is_empty());
    assert!(ct.action_table.row_offsets.len() >= 2);
}

#[test]
fn test_79_smallest_grammar_state_count() {
    let pt = build_table("tc_v9_smsc", |gb| {
        gb.token("z", "z").rule("start", vec!["z"]).start("start")
    });
    assert!(pt.state_count >= 2);
}

// =============================================================================
// 14. Various grammar complexities
// =============================================================================

#[test]
fn test_80_three_token_sequence() {
    let pt = build_table("tc_v9_3ts", |gb| {
        gb.token("a", "a")
            .token("b", "b")
            .token("c", "c")
            .rule("start", vec!["a", "b", "c"])
            .start("start")
    });
    let ct = compress_full(&pt);
    assert!(ct.action_table.row_offsets.len() >= 4);
}

#[test]
fn test_81_four_alternatives() {
    let pt = build_table("tc_v9_4alt", |gb| {
        gb.token("a", "a")
            .token("b", "b")
            .token("c", "c")
            .token("d", "d")
            .rule("start", vec!["a"])
            .rule("start", vec!["b"])
            .rule("start", vec!["c"])
            .rule("start", vec!["d"])
            .start("start")
    });
    let ct = compress_full(&pt);
    assert_eq!(ct.action_table.default_actions.len(), pt.state_count);
}

#[test]
fn test_82_nested_nonterminals() {
    let pt = build_table("tc_v9_nest", |gb| {
        gb.token("a", "a")
            .rule("start", vec!["m"])
            .rule("m", vec!["n"])
            .rule("n", vec!["a"])
            .start("start")
    });
    let ct = compress_full(&pt);
    assert!(!ct.action_table.data.is_empty());
}

#[test]
fn test_83_deeply_nested() {
    let pt = build_table("tc_v9_deep", |gb| {
        gb.token("x", "x")
            .rule("start", vec!["l1"])
            .rule("l1", vec!["l2"])
            .rule("l2", vec!["l3"])
            .rule("l3", vec!["x"])
            .start("start")
    });
    let ct = compress_full(&pt);
    assert!(ct.action_table.row_offsets.len() >= 3);
}

#[test]
fn test_84_left_recursive() {
    let pt = build_table("tc_v9_lrec", |gb| {
        gb.token("a", "a")
            .rule("start", vec!["lst"])
            .rule("lst", vec!["lst", "a"])
            .rule("lst", vec!["a"])
            .start("start")
    });
    let ct = compress_full(&pt);
    assert!(!ct.action_table.data.is_empty());
}

#[test]
fn test_85_right_recursive() {
    let pt = build_table("tc_v9_rrec", |gb| {
        gb.token("b", "b")
            .rule("start", vec!["rr"])
            .rule("rr", vec!["b", "rr"])
            .rule("rr", vec!["b"])
            .start("start")
    });
    let ct = compress_full(&pt);
    assert!(ct.action_table.row_offsets.len() >= 2);
}

// =============================================================================
// 15. Compression with row deduplication validates real tables
// =============================================================================

#[test]
fn test_86_real_table_dedup_single_token() {
    let pt = table_single_token("tc_v9_rd1");
    let compressed = compress_action_table(&pt.action_table);
    assert!(compressed.unique_rows.len() <= pt.state_count);
}

#[test]
fn test_87_real_table_dedup_two_alt() {
    let pt = table_two_alt("tc_v9_rd2");
    let compressed = compress_action_table(&pt.action_table);
    assert!(compressed.unique_rows.len() <= pt.state_count);
    assert_eq!(compressed.state_to_row.len(), pt.state_count);
}

#[test]
fn test_88_real_table_dedup_preserves_all_states() {
    let pt = table_sequence("tc_v9_rd3");
    let compressed = compress_action_table(&pt.action_table);
    // Every state maps to a valid unique row index
    for &row_idx in &compressed.state_to_row {
        assert!(row_idx < compressed.unique_rows.len());
    }
}

// =============================================================================
// 16. BitPackedActionTable on real tables
// =============================================================================

#[test]
fn test_89_bitpacked_multi_row() {
    let table = vec![
        vec![Action::Shift(StateId(1)), Action::Error, Action::Error],
        vec![Action::Error, Action::Reduce(RuleId(0)), Action::Error],
        vec![Action::Error, Action::Error, Action::Accept],
    ];
    let bp = BitPackedActionTable::from_table(&table);
    assert_eq!(bp.decompress(0, 0), Action::Shift(StateId(1)));
    assert_eq!(bp.decompress(0, 1), Action::Error);
    assert_eq!(bp.decompress(1, 0), Action::Error);
    assert_eq!(bp.decompress(2, 2), Action::Accept);
}

#[test]
fn test_90_bitpacked_all_errors() {
    let table = vec![vec![Action::Error; 4]; 3];
    let bp = BitPackedActionTable::from_table(&table);
    for s in 0..3 {
        for sym in 0..4 {
            assert_eq!(bp.decompress(s, sym), Action::Error);
        }
    }
}

// =============================================================================
// 17. Small table threshold
// =============================================================================

#[test]
fn test_91_small_table_threshold_present() {
    let pt = table_single_token("tc_v9_stt");
    let ct = compress_full(&pt);
    assert_eq!(ct.small_table_threshold, 32768);
}

// =============================================================================
// 18. Compression with precedence and associativity
// =============================================================================

#[test]
fn test_92_left_assoc_precedence() {
    let pt = build_table("tc_v9_la", |gb| {
        gb.token("n", "[0-9]+")
            .token("plus", "\\+")
            .rule("start", vec!["e"])
            .rule_with_precedence("e", vec!["e", "plus", "e"], 1, Associativity::Left)
            .rule("e", vec!["n"])
            .start("start")
    });
    let ct = compress_full(&pt);
    assert!(!ct.action_table.data.is_empty());
}

#[test]
fn test_93_right_assoc_precedence() {
    let pt = build_table("tc_v9_ra", |gb| {
        gb.token("n", "[0-9]+")
            .token("pow", "\\^")
            .rule("start", vec!["e"])
            .rule_with_precedence("e", vec!["e", "pow", "e"], 1, Associativity::Right)
            .rule("e", vec!["n"])
            .start("start")
    });
    let ct = compress_full(&pt);
    assert!(!ct.action_table.data.is_empty());
}

#[test]
fn test_94_multi_level_precedence() {
    let pt = build_table("tc_v9_mlp", |gb| {
        gb.token("n", "[0-9]+")
            .token("plus", "\\+")
            .token("star", "\\*")
            .rule("start", vec!["e"])
            .rule_with_precedence("e", vec!["e", "plus", "e"], 1, Associativity::Left)
            .rule_with_precedence("e", vec!["e", "star", "e"], 2, Associativity::Left)
            .rule("e", vec!["n"])
            .start("start")
    });
    let ct = compress_full(&pt);
    assert!(ct.action_table.row_offsets.len() >= 2);
}

// =============================================================================
// 19. Nullable grammar edge cases
// =============================================================================

#[test]
fn test_95_nullable_start_eof() {
    let pt = table_nullable("tc_v9_nse");
    assert!(start_nullable(&pt));
}

#[test]
fn test_96_non_nullable_start() {
    let pt = table_single_token("tc_v9_nns");
    assert!(!start_nullable(&pt));
}

// =============================================================================
// 20. Various compress_goto_table_small invocations
// =============================================================================

#[test]
fn test_97_goto_small_empty() {
    let tc = TableCompressor::new();
    let gt: Vec<Vec<StateId>> = vec![];
    let c = tc.compress_goto_table_small(&gt).unwrap();
    // Empty input may produce a sentinel offset; data should be empty.
    assert!(c.data.is_empty());
}

#[test]
fn test_98_goto_small_all_invalid() {
    let tc = TableCompressor::new();
    let gt = vec![vec![StateId(u16::MAX); 5]; 3];
    let c = tc.compress_goto_table_small(&gt).unwrap();
    // All u16::MAX entries still produce data (RLE encoded)
    assert!(!c.row_offsets.is_empty());
}

#[test]
fn test_99_goto_small_deterministic() {
    let tc = TableCompressor::new();
    let gt = vec![
        vec![StateId(1), StateId(2), StateId(u16::MAX)],
        vec![StateId(u16::MAX), StateId(3), StateId(4)],
    ];
    let c1 = tc.compress_goto_table_small(&gt).unwrap();
    let c2 = tc.compress_goto_table_small(&gt).unwrap();
    assert_eq!(c1.row_offsets, c2.row_offsets);
    assert_eq!(c1.data.len(), c2.data.len());
}

// =============================================================================
// 21. Additional edge case tests
// =============================================================================

#[test]
fn test_100_compress_action_table_single_accept() {
    let table = vec![vec![vec![Action::Accept]]];
    let compressed = compress_action_table(&table);
    assert_eq!(decompress_action(&compressed, 0, 0), Action::Accept);
}

#[test]
fn test_101_compress_action_table_recover() {
    let table = vec![vec![vec![Action::Recover]]];
    let compressed = compress_action_table(&table);
    assert_eq!(decompress_action(&compressed, 0, 0), Action::Recover);
}

#[test]
fn test_102_from_parse_table_with_precedence() {
    let pt = table_with_precedence("tc_v9_fpp");
    let cpt = CompressedParseTable::from_parse_table(&pt);
    assert_eq!(cpt.state_count(), pt.state_count);
    assert_eq!(cpt.symbol_count(), pt.symbol_count);
}

#[test]
fn test_103_compress_goto_table_large_sparse() {
    let mut table = vec![vec![None; 20]; 10];
    table[3][7] = Some(StateId(5));
    table[9][19] = Some(StateId(2));
    let compressed = compress_goto_table(&table);
    assert_eq!(compressed.entries.len(), 2);
    assert_eq!(decompress_goto(&compressed, 3, 7), Some(StateId(5)));
    assert_eq!(decompress_goto(&compressed, 9, 19), Some(StateId(2)));
    assert_eq!(decompress_goto(&compressed, 0, 0), None);
}

#[test]
fn test_104_action_entry_symbol_id_matches_column() {
    let pt = table_two_alt("tc_v9_sym");
    let ct = compress_full(&pt);
    // All symbol IDs in entries should be valid column indices
    for entry in &ct.action_table.data {
        assert!((entry.symbol as usize) < pt.symbol_count);
    }
}

#[test]
fn test_105_compress_pipeline_five_token_grammar() {
    let pt = build_table("tc_v9_5tok", |gb| {
        gb.token("a", "a")
            .token("b", "b")
            .token("c", "c")
            .token("d", "d")
            .token("e", "e")
            .rule("start", vec!["a", "b", "c", "d", "e"])
            .start("start")
    });
    let ct = compress_full(&pt);
    assert_eq!(ct.action_table.default_actions.len(), pt.state_count);
    assert!(ct.action_table.row_offsets.len() >= 6);
}

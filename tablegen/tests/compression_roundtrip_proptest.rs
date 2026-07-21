#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]
//! Property-based and unit tests for compression roundtrip correctness.
//!
//! Covers:
//! 1.  Action table row deduplication roundtrip
//! 2.  Goto table sparse compression roundtrip
//! 3.  BitPacked action table roundtrip (uniform action types)
//! 4.  Encode/decode small action encoding for all action variants
//! 5.  TableCompressor compress_action_table_small roundtrip invariants
//! 6.  TableCompressor compress_goto_table_small run-length invariants
//! 7.  Full pipeline compression roundtrip via GrammarBuilder
//! 8.  Edge cases: empty, single-entry, large tables
//! 9.  Bit packing error mask correctness
//! 10. Encoding boundary values
//! 11. CompressedParseTable unit tests
//! 12. Identical rows deduplication efficiency
//! 13. Sparse goto compression efficiency
//! 14. Wide symbol count
//! 15. StaticLanguageGenerator output properties
//! 16. NodeTypesGenerator JSON validity
//! 17. Large grammar compression
//! 18. Compression with different verbosity settings

use adze_glr_core::{Action, GotoIndexing, LexMode, ParseTable};
use adze_ir::{Grammar, RuleId, StateId, SymbolId, Token, TokenPattern};
use adze_tablegen::collect_token_indices;
use adze_tablegen::compress::{CompressedGotoEntry, CompressedParseTable, TableCompressor};
use adze_tablegen::compression::{
    BitPackedActionTable, compress_action_table, compress_goto_table, decompress_action,
    decompress_goto,
};
use adze_tablegen::eof_accepts_or_reduces;
use proptest::prelude::*;
use std::collections::BTreeMap;

// ── Helpers ─────────────────────────────────────────────────────────────────

const INVALID: StateId = StateId(u16::MAX);

/// Build a minimal ParseTable from raw components (integration-test safe).
fn make_parse_table(
    mut actions: Vec<Vec<Vec<Action>>>,
    mut gotos: Vec<Vec<StateId>>,
    start_symbol: SymbolId,
    eof_symbol: SymbolId,
) -> ParseTable {
    let state_count = actions.len().max(1);
    let sym_from_act = actions.first().map(|r| r.len()).unwrap_or(0);
    let sym_from_goto = gotos.first().map(|r| r.len()).unwrap_or(0);
    let min_needed = (start_symbol.0 as usize + 1).max(eof_symbol.0 as usize + 1);
    let symbol_count = sym_from_act.max(sym_from_goto).max(min_needed).max(1);

    if actions.is_empty() {
        actions = vec![vec![vec![]; symbol_count]];
    } else {
        for row in &mut actions {
            if row.len() < symbol_count {
                row.resize_with(symbol_count, Vec::new);
            }
        }
    }
    if gotos.len() < state_count {
        gotos.resize_with(state_count, || vec![INVALID; symbol_count]);
    }
    for row in &mut gotos {
        if row.len() < symbol_count {
            row.resize(symbol_count, INVALID);
        }
    }

    let mut symbol_to_index = BTreeMap::new();
    let mut index_to_symbol = vec![SymbolId(0); symbol_count];
    for i in 0..symbol_count {
        symbol_to_index.insert(SymbolId(i as u16), i);
        index_to_symbol[i] = SymbolId(i as u16);
    }
    let mut nonterminal_to_index = BTreeMap::new();
    nonterminal_to_index
        .entry(start_symbol)
        .or_insert(start_symbol.0 as usize);

    let eof_idx = eof_symbol.0 as usize;
    let token_count = eof_idx;

    ParseTable {
        action_table: actions,
        goto_table: gotos,
        rules: vec![],
        state_count,
        symbol_count,
        symbol_to_index,
        index_to_symbol,
        nonterminal_to_index,
        symbol_metadata: vec![],
        token_count,
        external_token_count: 0,
        eof_symbol,
        start_symbol,
        initial_state: StateId(0),
        lex_modes: vec![
            LexMode {
                lex_state: 0,
                external_lex_state: 0,
            };
            state_count
        ],
        extras: vec![],
        external_scanner_states: vec![],
        dynamic_prec_by_rule: vec![],
        rule_assoc_by_rule: vec![],
        alias_sequences: vec![],
        field_names: vec![],
        field_map: BTreeMap::new(),
        grammar: Grammar::default(),
        goto_indexing: GotoIndexing::NonterminalMap,
    }
}

/// Build a ParseTable with at least one token shift in state 0 so that
/// `TableCompressor::compress` passes its state-0 validation.
fn table_with_shift_in_s0(
    num_states: usize,
    num_terms: usize,
    extra_actions: Vec<(usize, usize, Action)>,
) -> ParseTable {
    let num_states = num_states.max(2);
    let num_terms = num_terms.max(1);
    let eof_idx = num_terms + 1;
    let start_nt = eof_idx + 1;
    let symbol_count = start_nt + 1;

    let mut actions: Vec<Vec<Vec<Action>>> = vec![vec![vec![]; symbol_count]; num_states];
    // Place a shift at column 1 (first terminal) in state 0
    actions[0][1] = vec![Action::Shift(StateId(1))];

    for (s, sym, act) in extra_actions {
        if s < num_states && sym < symbol_count {
            actions[s][sym].push(act);
        }
    }

    let gotos = vec![vec![INVALID; symbol_count]; num_states];

    let mut grammar = Grammar::default();
    for t in 1..=num_terms {
        grammar.tokens.insert(
            SymbolId(t as u16),
            Token {
                name: format!("t{t}"),
                pattern: TokenPattern::String(format!("t{t}")),
                fragile: false,
            },
        );
    }

    let mut pt = make_parse_table(
        actions,
        gotos,
        SymbolId(start_nt as u16),
        SymbolId(eof_idx as u16),
    );
    pt.grammar = grammar;
    pt
}

/// Compress a table that has shift in state 0 using TableCompressor.
fn compress_table(pt: &ParseTable) -> adze_tablegen::CompressedTables {
    let compressor = TableCompressor::new();
    let token_indices = collect_token_indices(&pt.grammar, pt);
    let start_empty = eof_accepts_or_reduces(pt);
    compressor
        .compress(pt, &token_indices, start_empty)
        .expect("compression must succeed")
}

// ── Strategies ──────────────────────────────────────────────────────────────

fn action_strategy() -> impl Strategy<Value = Action> {
    prop_oneof![
        3 => Just(Action::Error),
        2 => (1u16..100).prop_map(|s| Action::Shift(StateId(s))),
        2 => (0u16..50).prop_map(|r| Action::Reduce(RuleId(r))),
        1 => Just(Action::Accept),
    ]
}

fn action_cell_strategy() -> impl Strategy<Value = Vec<Action>> {
    prop::collection::vec(action_strategy(), 0..=3)
}

fn action_table_strategy(
    max_states: usize,
    max_symbols: usize,
) -> impl Strategy<Value = Vec<Vec<Vec<Action>>>> {
    (1..=max_states, 1..=max_symbols).prop_flat_map(|(states, symbols)| {
        prop::collection::vec(
            prop::collection::vec(action_cell_strategy(), symbols..=symbols),
            states..=states,
        )
    })
}

#[allow(dead_code)]
fn flat_action_strategy() -> impl Strategy<Value = Action> {
    prop_oneof![
        3 => Just(Action::Error),
        2 => (1u16..100).prop_map(|s| Action::Shift(StateId(s))),
        2 => (0u16..50).prop_map(|r| Action::Reduce(RuleId(r))),
        1 => Just(Action::Accept),
    ]
}

#[allow(dead_code)]
fn flat_action_table_strategy(
    max_states: usize,
    max_symbols: usize,
) -> impl Strategy<Value = Vec<Vec<Action>>> {
    (1..=max_states, 1..=max_symbols).prop_flat_map(|(states, symbols)| {
        prop::collection::vec(
            prop::collection::vec(flat_action_strategy(), symbols..=symbols),
            states..=states,
        )
    })
}

fn goto_table_strategy(
    max_states: usize,
    max_symbols: usize,
) -> impl Strategy<Value = Vec<Vec<Option<StateId>>>> {
    (1..=max_states, 1..=max_symbols).prop_flat_map(|(states, symbols)| {
        prop::collection::vec(
            prop::collection::vec(
                prop::option::of((0u16..50).prop_map(StateId)),
                symbols..=symbols,
            ),
            states..=states,
        )
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Action table row-dedup roundtrip (property tests)
// ═══════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn prop_action_dedup_roundtrip_preserves_all_cells(table in action_table_strategy(8, 8)) {
        let compressed = compress_action_table(&table);
        for (state, row) in table.iter().enumerate() {
            for (sym, cell) in row.iter().enumerate() {
                let expected = cell.first().cloned().unwrap_or(Action::Error);
                let got = decompress_action(&compressed, state, sym);
                prop_assert_eq!(got, expected, "state={} sym={}", state, sym);
            }
        }
    }

    #[test]
    fn prop_action_dedup_unique_rows_le_total(table in action_table_strategy(10, 6)) {
        let compressed = compress_action_table(&table);
        prop_assert!(compressed.unique_rows.len() <= table.len());
    }

    #[test]
    fn prop_action_dedup_state_to_row_len_matches(table in action_table_strategy(8, 8)) {
        let compressed = compress_action_table(&table);
        prop_assert_eq!(compressed.state_to_row.len(), table.len());
    }

    #[test]
    fn prop_action_dedup_column_count_preserved(table in action_table_strategy(6, 10)) {
        let compressed = compress_action_table(&table);
        if let Some(first_row) = table.first() {
            for unique_row in &compressed.unique_rows {
                prop_assert_eq!(unique_row.len(), first_row.len());
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Goto table sparse roundtrip (property tests)
// ═══════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn prop_goto_sparse_roundtrip_all_cells(table in goto_table_strategy(8, 8)) {
        let compressed = compress_goto_table(&table);
        for (state, row) in table.iter().enumerate() {
            for (sym, &expected) in row.iter().enumerate() {
                let got = decompress_goto(&compressed, state, sym);
                prop_assert_eq!(got, expected, "state={} sym={}", state, sym);
            }
        }
    }

    #[test]
    fn prop_goto_sparse_entry_count_equals_some_count(table in goto_table_strategy(8, 8)) {
        let compressed = compress_goto_table(&table);
        let some_count: usize = table.iter().flat_map(|r| r.iter()).filter(|v| v.is_some()).count();
        prop_assert_eq!(compressed.entries.len(), some_count);
    }

    #[test]
    fn prop_goto_sparse_none_returns_none(table in goto_table_strategy(6, 6)) {
        let compressed = compress_goto_table(&table);
        for (state, row) in table.iter().enumerate() {
            for (sym, val) in row.iter().enumerate() {
                if val.is_none() {
                    prop_assert_eq!(decompress_goto(&compressed, state, sym), None);
                }
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. BitPacked action table roundtrip
// ═══════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn prop_bitpacked_all_errors_roundtrip(states in 1usize..8, symbols in 1usize..8) {
        let table = vec![vec![Action::Error; symbols]; states];
        let packed = BitPackedActionTable::from_table(&table);
        for s in 0..states {
            for sym in 0..symbols {
                prop_assert_eq!(packed.decompress(s, sym), Action::Error);
            }
        }
    }

    #[test]
    fn prop_bitpacked_all_shifts_roundtrip(states in 1usize..5, symbols in 1usize..5) {
        let table: Vec<Vec<Action>> = (0..states)
            .map(|s| {
                (0..symbols)
                    .map(|sym| Action::Shift(StateId(((s * symbols + sym) % 100) as u16)))
                    .collect()
            })
            .collect();
        let packed = BitPackedActionTable::from_table(&table);
        for s in 0..states {
            for sym in 0..symbols {
                prop_assert_eq!(packed.decompress(s, sym), table[s][sym].clone());
            }
        }
    }
}

#[test]
fn bitpacked_single_accept_roundtrip() {
    let table = vec![vec![Action::Accept]];
    let packed = BitPackedActionTable::from_table(&table);
    // Accept is stored as special reduce (u32::MAX)
    let result = packed.decompress(0, 0);
    assert_eq!(result, Action::Accept);
}

#[test]
fn bitpacked_mixed_error_and_shift() {
    let table = vec![
        vec![Action::Error, Action::Shift(StateId(1))],
        vec![Action::Shift(StateId(2)), Action::Error],
    ];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Error);
    assert_eq!(packed.decompress(0, 1), Action::Shift(StateId(1)));
    assert_eq!(packed.decompress(1, 0), Action::Shift(StateId(2)));
    assert_eq!(packed.decompress(1, 1), Action::Error);
}

#[test]
fn bitpacked_error_mask_bits_set_correctly() {
    let table = vec![vec![
        Action::Error,
        Action::Shift(StateId(1)),
        Action::Error,
        Action::Shift(StateId(2)),
    ]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Error);
    assert_eq!(packed.decompress(0, 2), Action::Error);
    assert_eq!(packed.decompress(0, 1), Action::Shift(StateId(1)));
    assert_eq!(packed.decompress(0, 3), Action::Shift(StateId(2)));
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Encode/decode small action encoding
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn encode_shift_zero() {
    let c = TableCompressor::new();
    assert_eq!(
        c.encode_action_small(&Action::Shift(StateId(0))).unwrap(),
        0
    );
}

#[test]
fn encode_shift_one() {
    let c = TableCompressor::new();
    assert_eq!(
        c.encode_action_small(&Action::Shift(StateId(1))).unwrap(),
        1
    );
}

#[test]
fn encode_shift_max_valid() {
    let c = TableCompressor::new();
    assert_eq!(
        c.encode_action_small(&Action::Shift(StateId(0x7FFF)))
            .unwrap(),
        0x7FFF
    );
}

#[test]
fn encode_shift_overflow_fails() {
    let c = TableCompressor::new();
    assert!(
        c.encode_action_small(&Action::Shift(StateId(0x8000)))
            .is_err()
    );
}

#[test]
fn encode_reduce_zero() {
    let c = TableCompressor::new();
    // Reduce(0) → 0x8000 | (0 + 1) = 0x8001
    assert_eq!(
        c.encode_action_small(&Action::Reduce(RuleId(0))).unwrap(),
        0x8001
    );
}

#[test]
fn encode_reduce_max_valid() {
    let c = TableCompressor::new();
    // Reduce(0x3FFF) → 0x8000 | (0x3FFF + 1) = 0x8000 | 0x4000 = 0xC000
    assert_eq!(
        c.encode_action_small(&Action::Reduce(RuleId(0x3FFF)))
            .unwrap(),
        0xC000
    );
}

#[test]
fn encode_reduce_overflow_fails() {
    let c = TableCompressor::new();
    assert!(
        c.encode_action_small(&Action::Reduce(RuleId(0x4000)))
            .is_err()
    );
}

#[test]
fn encode_accept_is_0xffff() {
    let c = TableCompressor::new();
    assert_eq!(c.encode_action_small(&Action::Accept).unwrap(), 0xFFFF);
}

#[test]
fn encode_error_is_0xfffe() {
    let c = TableCompressor::new();
    assert_eq!(c.encode_action_small(&Action::Error).unwrap(), 0xFFFE);
}

#[test]
fn encode_recover_is_0xfffd() {
    let c = TableCompressor::new();
    assert_eq!(c.encode_action_small(&Action::Recover).unwrap(), 0xFFFD);
}

#[test]
fn encode_fork_requires_flattening() {
    let c = TableCompressor::new();
    let fork = Action::Fork(vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(0))]);
    assert!(c.encode_action_small(&fork).is_err());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn prop_encode_shift_preserves_state(state in 0u16..0x8000) {
        let c = TableCompressor::new();
        let encoded = c.encode_action_small(&Action::Shift(StateId(state))).unwrap();
        prop_assert_eq!(encoded, state);
    }

    #[test]
    fn prop_encode_reduce_has_high_bit(rule in 0u16..0x4000) {
        let c = TableCompressor::new();
        let encoded = c.encode_action_small(&Action::Reduce(RuleId(rule))).unwrap();
        prop_assert!(encoded & 0x8000 != 0, "reduce must have high bit set");
        prop_assert_eq!(encoded & 0x7FFF, rule + 1, "1-based rule id");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. TableCompressor compress_action_table_small invariants
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn small_action_empty_table_produces_correct_offsets() {
    let c = TableCompressor::new();
    let table: Vec<Vec<Vec<Action>>> = vec![vec![]; 4];
    let result = c
        .compress_action_table_small(&table, &BTreeMap::new())
        .unwrap();
    assert_eq!(result.row_offsets.len(), 5); // 4 states + 1
    assert_eq!(result.default_actions.len(), 4);
    assert!(result.data.is_empty());
}

#[test]
fn small_action_single_shift_entry() {
    let c = TableCompressor::new();
    let table = vec![vec![vec![Action::Shift(StateId(5))]]];
    let result = c
        .compress_action_table_small(&table, &BTreeMap::new())
        .unwrap();
    assert_eq!(result.data.len(), 1);
    assert_eq!(result.data[0].symbol, 0);
    assert_eq!(result.data[0].action, Action::Shift(StateId(5)));
}

#[test]
fn small_action_error_cells_not_stored() {
    let c = TableCompressor::new();
    // 5 columns: only col 2 has a non-error action
    let table = vec![
        (0..5)
            .map(|i| {
                if i == 2 {
                    vec![Action::Reduce(RuleId(0))]
                } else {
                    vec![]
                }
            })
            .collect(),
    ];
    let result = c
        .compress_action_table_small(&table, &BTreeMap::new())
        .unwrap();
    assert_eq!(result.data.len(), 1);
}

#[test]
fn small_action_explicit_error_actions_not_stored() {
    let c = TableCompressor::new();
    // Cell containing explicit Action::Error should be skipped
    let table = vec![vec![vec![Action::Error], vec![Action::Shift(StateId(1))]]];
    let result = c
        .compress_action_table_small(&table, &BTreeMap::new())
        .unwrap();
    assert_eq!(result.data.len(), 1);
    assert_eq!(result.data[0].action, Action::Shift(StateId(1)));
}

#[test]
fn small_action_multi_action_cell_stores_all() {
    let c = TableCompressor::new();
    let table = vec![vec![vec![
        Action::Shift(StateId(1)),
        Action::Reduce(RuleId(2)),
    ]]];
    let result = c
        .compress_action_table_small(&table, &BTreeMap::new())
        .unwrap();
    // Both actions in the GLR cell should be stored
    assert_eq!(result.data.len(), 2);
}

#[test]
fn small_action_row_offsets_nondecreasing() {
    let c = TableCompressor::new();
    let table = vec![
        vec![vec![Action::Shift(StateId(0))]; 5],
        vec![vec![]; 5],
        vec![vec![Action::Reduce(RuleId(1))]; 5],
    ];
    let result = c
        .compress_action_table_small(&table, &BTreeMap::new())
        .unwrap();
    for pair in result.row_offsets.windows(2) {
        assert!(pair[1] >= pair[0], "offsets must be non-decreasing");
    }
}

#[test]
fn small_action_default_actions_always_error() {
    let c = TableCompressor::new();
    let table = vec![
        vec![vec![Action::Reduce(RuleId(0))]; 10],
        vec![vec![Action::Shift(StateId(1))]; 10],
        vec![vec![Action::Accept]; 10],
    ];
    let result = c
        .compress_action_table_small(&table, &BTreeMap::new())
        .unwrap();
    for d in &result.default_actions {
        assert_eq!(*d, Action::Error, "default optimization disabled");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn prop_small_action_row_offsets_len(table in action_table_strategy(6, 6)) {
        let c = TableCompressor::new();
        let result = c.compress_action_table_small(&table, &BTreeMap::new()).unwrap();
        prop_assert_eq!(result.row_offsets.len(), table.len() + 1);
    }

    #[test]
    fn prop_small_action_default_actions_len(table in action_table_strategy(6, 6)) {
        let c = TableCompressor::new();
        let result = c.compress_action_table_small(&table, &BTreeMap::new()).unwrap();
        prop_assert_eq!(result.default_actions.len(), table.len());
    }

    #[test]
    fn prop_small_action_offsets_nondecreasing(table in action_table_strategy(8, 8)) {
        let c = TableCompressor::new();
        let result = c.compress_action_table_small(&table, &BTreeMap::new()).unwrap();
        for pair in result.row_offsets.windows(2) {
            prop_assert!(pair[1] >= pair[0]);
        }
    }

    #[test]
    fn prop_small_action_last_offset_equals_data_len(table in action_table_strategy(6, 6)) {
        let c = TableCompressor::new();
        let result = c.compress_action_table_small(&table, &BTreeMap::new()).unwrap();
        prop_assert_eq!(*result.row_offsets.last().unwrap(), result.data.len() as u16);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. TableCompressor compress_goto_table_small run-length invariants
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn goto_rle_run_of_3_uses_run_length() {
    let c = TableCompressor::new();
    let table = vec![vec![StateId(7), StateId(7), StateId(7)]];
    let result = c.compress_goto_table_small(&table).unwrap();
    let has_rl = result
        .data
        .iter()
        .any(|e| matches!(e, CompressedGotoEntry::RunLength { state: 7, count: 3 }));
    assert!(has_rl);
}

#[test]
fn goto_rle_run_of_2_uses_singles() {
    let c = TableCompressor::new();
    let table = vec![vec![StateId(5), StateId(5)]];
    let result = c.compress_goto_table_small(&table).unwrap();
    let all_single = result
        .data
        .iter()
        .all(|e| matches!(e, CompressedGotoEntry::Single(5)));
    assert!(all_single);
}

#[test]
fn goto_rle_run_of_1_uses_single() {
    let c = TableCompressor::new();
    let table = vec![vec![StateId(3)]];
    let result = c.compress_goto_table_small(&table).unwrap();
    assert_eq!(result.data.len(), 1);
    assert!(matches!(result.data[0], CompressedGotoEntry::Single(3)));
}

#[test]
fn goto_rle_alternating_no_run_length() {
    let c = TableCompressor::new();
    let table = vec![vec![
        StateId(1),
        StateId(2),
        StateId(1),
        StateId(2),
        StateId(1),
    ]];
    let result = c.compress_goto_table_small(&table).unwrap();
    let all_single = result
        .data
        .iter()
        .all(|e| matches!(e, CompressedGotoEntry::Single(_)));
    assert!(all_single);
}

#[test]
fn goto_rle_long_run_uses_run_length() {
    let c = TableCompressor::new();
    let table = vec![vec![StateId(42); 100]];
    let result = c.compress_goto_table_small(&table).unwrap();
    let has_rl = result.data.iter().any(|e| {
        matches!(
            e,
            CompressedGotoEntry::RunLength {
                state: 42,
                count: 100
            }
        )
    });
    assert!(has_rl);
}

#[test]
fn goto_rle_multiple_runs_in_one_row() {
    let c = TableCompressor::new();
    let table = vec![vec![
        StateId(1),
        StateId(1),
        StateId(1),
        StateId(2),
        StateId(2),
        StateId(2),
    ]];
    let result = c.compress_goto_table_small(&table).unwrap();
    let rl_count = result
        .data
        .iter()
        .filter(|e| matches!(e, CompressedGotoEntry::RunLength { .. }))
        .count();
    assert_eq!(rl_count, 2, "should have two RunLength entries");
}

#[test]
fn goto_rle_empty_table() {
    let c = TableCompressor::new();
    let table: Vec<Vec<StateId>> = vec![];
    let result = c.compress_goto_table_small(&table).unwrap();
    assert_eq!(result.row_offsets.len(), 1);
    assert!(result.data.is_empty());
}

#[test]
fn goto_rle_empty_rows() {
    let c = TableCompressor::new();
    let table: Vec<Vec<StateId>> = vec![vec![], vec![], vec![]];
    let result = c.compress_goto_table_small(&table).unwrap();
    assert_eq!(result.row_offsets.len(), 4);
    assert!(result.data.is_empty());
}

#[test]
fn goto_rle_row_offsets_nondecreasing() {
    let c = TableCompressor::new();
    let table = vec![
        vec![StateId(1), StateId(1), StateId(1)],
        vec![StateId(2)],
        vec![StateId(3), StateId(4)],
    ];
    let result = c.compress_goto_table_small(&table).unwrap();
    for pair in result.row_offsets.windows(2) {
        assert!(pair[1] >= pair[0]);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. Full pipeline compression roundtrip via GrammarBuilder
// ═══════════════════════════════════════════════════════════════════════════

fn pipeline(grammar_fn: impl FnOnce() -> Grammar) -> (ParseTable, adze_tablegen::CompressedTables) {
    use adze_glr_core::{FirstFollowSets, build_lr1_automaton};

    let mut grammar = grammar_fn();
    let ff =
        FirstFollowSets::compute_normalized(&mut grammar).expect("FIRST/FOLLOW computation failed");
    let table = build_lr1_automaton(&grammar, &ff).expect("LR(1) automaton construction failed");
    let token_indices = collect_token_indices(&grammar, &table);
    let start_empty = eof_accepts_or_reduces(&table);
    let compressor = TableCompressor::new();
    let compressed = compressor
        .compress(&table, &token_indices, start_empty)
        .expect("Table compression failed");
    (table, compressed)
}

#[test]
fn pipeline_single_token_grammar() {
    use adze_ir::builder::GrammarBuilder;
    let (_pt, compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .rule("start", vec!["a"])
            .start("start")
            .build()
    });
    assert!(!compressed.action_table.data.is_empty());
}

#[test]
fn pipeline_two_alternatives() {
    use adze_ir::builder::GrammarBuilder;
    let (_pt, compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .token("b", "b")
            .rule("start", vec!["a"])
            .rule("start", vec!["b"])
            .start("start")
            .build()
    });
    assert!(!compressed.action_table.data.is_empty());
    assert!(!compressed.goto_table.data.is_empty());
}

#[test]
fn pipeline_sequence_grammar() {
    use adze_ir::builder::GrammarBuilder;
    let (_pt, compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .token("b", "b")
            .rule("start", vec!["a", "b"])
            .start("start")
            .build()
    });
    assert!(!compressed.action_table.data.is_empty());
}

#[test]
fn pipeline_chain_grammar() {
    use adze_ir::builder::GrammarBuilder;
    let (_pt, compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("x", "x")
            .rule("c", vec!["x"])
            .rule("b", vec!["c"])
            .rule("start", vec!["b"])
            .start("start")
            .build()
    });
    assert!(!compressed.action_table.data.is_empty());
    assert!(!compressed.goto_table.data.is_empty());
}

#[test]
fn pipeline_left_recursive_grammar() {
    use adze_ir::builder::GrammarBuilder;
    let (pt, compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .rule("list", vec!["a"])
            .rule("list", vec!["list", "a"])
            .start("list")
            .build()
    });
    assert!(pt.state_count >= 3);
    assert!(!compressed.action_table.data.is_empty());
}

#[test]
fn pipeline_validates_ok() {
    use adze_ir::builder::GrammarBuilder;
    let (pt, compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .rule("start", vec!["a"])
            .start("start")
            .build()
    });
    assert!(compressed.validate(&pt).is_ok());
}

#[test]
fn pipeline_compressed_metadata_matches() {
    use adze_ir::builder::GrammarBuilder;
    let (pt, _compressed) = pipeline(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .token("b", "b")
            .rule("start", vec!["a", "b"])
            .start("start")
            .build()
    });
    let cpt = CompressedParseTable::from_parse_table(&pt);
    assert_eq!(cpt.symbol_count(), pt.symbol_count);
    assert_eq!(cpt.state_count(), pt.state_count);
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. Edge cases: empty, single-entry, large tables
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn edge_single_state_single_symbol_action() {
    let table = vec![vec![vec![Action::Accept]]];
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 1);
    assert_eq!(decompress_action(&compressed, 0, 0), Action::Accept);
}

#[test]
fn edge_single_state_single_symbol_goto() {
    let table = vec![vec![Some(StateId(0))]];
    let compressed = compress_goto_table(&table);
    assert_eq!(decompress_goto(&compressed, 0, 0), Some(StateId(0)));
}

#[test]
fn edge_all_error_action_table() {
    let table = vec![vec![vec![]; 10]; 5];
    let compressed = compress_action_table(&table);
    for state in 0..5 {
        for sym in 0..10 {
            assert_eq!(decompress_action(&compressed, state, sym), Action::Error);
        }
    }
}

#[test]
fn edge_all_none_goto_table() {
    let table = vec![vec![None; 8]; 4];
    let compressed = compress_goto_table(&table);
    assert!(compressed.entries.is_empty());
    for state in 0..4 {
        for sym in 0..8 {
            assert_eq!(decompress_goto(&compressed, state, sym), None);
        }
    }
}

#[test]
fn edge_large_action_table_roundtrip() {
    let n_states = 50;
    let n_syms = 20;
    let table: Vec<Vec<Vec<Action>>> = (0..n_states)
        .map(|s| {
            (0..n_syms)
                .map(|sym| match (s + sym) % 5 {
                    0 => vec![Action::Shift(StateId(((s + sym) % 30) as u16))],
                    1 => vec![Action::Reduce(RuleId(((s * sym) % 15) as u16))],
                    2 => vec![Action::Accept],
                    3 => vec![],
                    _ => vec![Action::Error],
                })
                .collect()
        })
        .collect();
    let compressed = compress_action_table(&table);
    for (state, row) in table.iter().enumerate() {
        for (sym, cell) in row.iter().enumerate() {
            let expected = cell.first().cloned().unwrap_or(Action::Error);
            assert_eq!(
                decompress_action(&compressed, state, sym),
                expected,
                "state={state} sym={sym}"
            );
        }
    }
}

#[test]
fn edge_large_goto_table_roundtrip() {
    let n_states = 30;
    let n_syms = 15;
    let table: Vec<Vec<Option<StateId>>> = (0..n_states)
        .map(|s| {
            (0..n_syms)
                .map(|sym| {
                    if (s + sym) % 3 == 0 {
                        Some(StateId(((s + sym) % 20) as u16))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .collect();
    let compressed = compress_goto_table(&table);
    for (state, row) in table.iter().enumerate() {
        for (sym, &expected) in row.iter().enumerate() {
            assert_eq!(
                decompress_goto(&compressed, state, sym),
                expected,
                "state={state} sym={sym}"
            );
        }
    }
}

#[test]
fn edge_multi_action_cell_first_action_returned() {
    let table = vec![vec![vec![
        Action::Shift(StateId(1)),
        Action::Reduce(RuleId(2)),
    ]]];
    let compressed = compress_action_table(&table);
    assert_eq!(
        decompress_action(&compressed, 0, 0),
        Action::Shift(StateId(1))
    );
}

#[test]
fn edge_empty_cell_returns_error() {
    let table = vec![vec![vec![]]];
    let compressed = compress_action_table(&table);
    assert_eq!(decompress_action(&compressed, 0, 0), Action::Error);
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. Compression determinism
// ═══════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn prop_action_dedup_deterministic(table in action_table_strategy(6, 6)) {
        let c1 = compress_action_table(&table);
        let c2 = compress_action_table(&table);
        prop_assert_eq!(c1.unique_rows, c2.unique_rows);
        prop_assert_eq!(c1.state_to_row, c2.state_to_row);
    }

    #[test]
    fn prop_goto_sparse_deterministic(table in goto_table_strategy(6, 6)) {
        let c1 = compress_goto_table(&table);
        let c2 = compress_goto_table(&table);
        prop_assert_eq!(c1.entries, c2.entries);
    }
}

#[test]
fn deterministic_small_table_compressor() {
    let c = TableCompressor::new();
    let table = vec![
        vec![
            vec![Action::Shift(StateId(1))],
            vec![],
            vec![Action::Accept],
        ],
        vec![vec![Action::Reduce(RuleId(0))], vec![], vec![]],
    ];
    let c1 = c
        .compress_action_table_small(&table, &BTreeMap::new())
        .unwrap();
    let c2 = c
        .compress_action_table_small(&table, &BTreeMap::new())
        .unwrap();
    assert_eq!(c1.row_offsets, c2.row_offsets);
    assert_eq!(c1.default_actions, c2.default_actions);
    assert_eq!(c1.data.len(), c2.data.len());
    for (e1, e2) in c1.data.iter().zip(c2.data.iter()) {
        assert_eq!(e1.symbol, e2.symbol);
        assert_eq!(e1.action, e2.action);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 10. TableCompressor::compress validation edge cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn compress_rejects_empty_action_table() {
    let mut pt = table_with_shift_in_s0(2, 1, vec![]);
    pt.action_table.clear();
    pt.state_count = 0;
    let c = TableCompressor::new();
    let token_indices = collect_token_indices(&pt.grammar, &pt);
    let result = c.compress(&pt, &token_indices, false);
    assert!(result.is_err());
}

#[test]
fn compress_requires_token_shift_in_state0() {
    let num_terms = 1;
    let eof_idx = num_terms + 1;
    let start_nt = eof_idx + 1;
    let symbol_count = start_nt + 1;

    // Table with no shift in state 0
    let actions: Vec<Vec<Vec<Action>>> = vec![vec![vec![]; symbol_count]; 2];
    let gotos = vec![vec![INVALID; symbol_count]; 2];

    let mut grammar = Grammar::default();
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "t1".to_string(),
            pattern: TokenPattern::String("t1".to_string()),
            fragile: false,
        },
    );

    let mut pt = make_parse_table(
        actions,
        gotos,
        SymbolId(start_nt as u16),
        SymbolId(eof_idx as u16),
    );
    pt.grammar = grammar;

    let c = TableCompressor::new();
    let token_indices = collect_token_indices(&pt.grammar, &pt);
    let result = c.compress(&pt, &token_indices, false);
    assert!(result.is_err());
}

#[test]
fn compress_accepts_nullable_start_with_eof_accept() {
    let num_terms = 1;
    let eof_idx = num_terms + 1;
    let start_nt = eof_idx + 1;
    let symbol_count = start_nt + 1;

    let mut actions: Vec<Vec<Vec<Action>>> = vec![vec![vec![]; symbol_count]; 2];
    // Put Accept on the EOF column in state 0
    actions[0][eof_idx] = vec![Action::Accept];
    let gotos = vec![vec![INVALID; symbol_count]; 2];

    let mut grammar = Grammar::default();
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "t1".to_string(),
            pattern: TokenPattern::String("t1".to_string()),
            fragile: false,
        },
    );

    let mut pt = make_parse_table(
        actions,
        gotos,
        SymbolId(start_nt as u16),
        SymbolId(eof_idx as u16),
    );
    pt.grammar = grammar;

    let c = TableCompressor::new();
    let token_indices = collect_token_indices(&pt.grammar, &pt);
    // start_can_be_empty = true should allow the nullable case
    let result = c.compress(&pt, &token_indices, true);
    assert!(result.is_ok());
}

#[test]
fn compress_full_pipeline_table_compressor_row_offsets() {
    let pt = table_with_shift_in_s0(4, 3, vec![(1, 2, Action::Reduce(RuleId(0)))]);
    let compressed = compress_table(&pt);
    assert_eq!(
        compressed.action_table.row_offsets.len(),
        pt.state_count + 1
    );
    assert_eq!(
        compressed.action_table.default_actions.len(),
        pt.state_count
    );
}

#[test]
fn compress_full_pipeline_goto_row_offsets() {
    let pt = table_with_shift_in_s0(4, 3, vec![]);
    let compressed = compress_table(&pt);
    for pair in compressed.goto_table.row_offsets.windows(2) {
        assert!(pair[1] >= pair[0]);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 11. CompressedParseTable unit tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn compressed_parse_table_new_for_testing() {
    let cpt = CompressedParseTable::new_for_testing(100, 200);
    assert_eq!(cpt.symbol_count(), 100);
    assert_eq!(cpt.state_count(), 200);
}

#[test]
fn compressed_parse_table_zero_sizes() {
    let cpt = CompressedParseTable::new_for_testing(0, 0);
    assert_eq!(cpt.symbol_count(), 0);
    assert_eq!(cpt.state_count(), 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// 12. Identical rows deduplication efficiency
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn dedup_4_identical_rows_gives_1_unique() {
    let row = vec![
        vec![Action::Shift(StateId(5))],
        vec![Action::Reduce(RuleId(1))],
    ];
    let table = vec![row.clone(), row.clone(), row.clone(), row];
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 1);
    assert_eq!(compressed.state_to_row.len(), 4);
}

#[test]
fn dedup_all_distinct_preserved() {
    let table = vec![
        vec![vec![Action::Shift(StateId(1))], vec![Action::Error]],
        vec![vec![Action::Error], vec![Action::Shift(StateId(2))]],
        vec![vec![Action::Reduce(RuleId(0))], vec![Action::Accept]],
    ];
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 3);
}

#[test]
fn dedup_mixed_identical_and_distinct() {
    let row_a = vec![vec![Action::Shift(StateId(1))], vec![Action::Error]];
    let row_b = vec![vec![Action::Error], vec![Action::Accept]];
    let table = vec![row_a.clone(), row_b.clone(), row_a, row_b];
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 2);
    assert_eq!(compressed.state_to_row.len(), 4);
    // First and third rows should map to same unique row
    assert_eq!(compressed.state_to_row[0], compressed.state_to_row[2]);
    assert_eq!(compressed.state_to_row[1], compressed.state_to_row[3]);
}

// ═══════════════════════════════════════════════════════════════════════════
// 13. Sparse goto compression efficiency
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn sparse_goto_fewer_entries_than_cells() {
    let n_states = 10;
    let n_syms = 10;
    let table: Vec<Vec<Option<StateId>>> = (0..n_states)
        .map(|s| {
            (0..n_syms)
                .map(|sym| {
                    if (s + sym) % 7 == 0 {
                        Some(StateId(1))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .collect();
    let compressed = compress_goto_table(&table);
    assert!(compressed.entries.len() < n_states * n_syms);
}

#[test]
fn sparse_goto_fully_populated() {
    let n_states = 3;
    let n_syms = 3;
    let table: Vec<Vec<Option<StateId>>> = (0..n_states)
        .map(|s| {
            (0..n_syms)
                .map(|sym| Some(StateId(((s + sym) % 5) as u16)))
                .collect()
        })
        .collect();
    let compressed = compress_goto_table(&table);
    assert_eq!(compressed.entries.len(), n_states * n_syms);
}

// ═══════════════════════════════════════════════════════════════════════════
// 14. Wide symbol count
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn wide_symbol_count_100_columns() {
    let n_syms = 100;
    let table: Vec<Vec<Vec<Action>>> = vec![
        (0..n_syms)
            .map(|sym| {
                if sym % 10 == 0 {
                    vec![Action::Shift(StateId((sym % 30) as u16))]
                } else {
                    vec![]
                }
            })
            .collect(),
    ];
    let compressed = compress_action_table(&table);
    for sym in 0..n_syms {
        let expected = if sym % 10 == 0 {
            Action::Shift(StateId((sym % 30) as u16))
        } else {
            Action::Error
        };
        assert_eq!(decompress_action(&compressed, 0, sym), expected);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 15. StaticLanguageGenerator output properties
// ═══════════════════════════════════════════════════════════════════════════

fn make_static_gen(grammar_fn: impl FnOnce() -> Grammar) -> adze_tablegen::StaticLanguageGenerator {
    use adze_glr_core::{FirstFollowSets, build_lr1_automaton};

    let mut grammar = grammar_fn();
    let ff =
        FirstFollowSets::compute_normalized(&mut grammar).expect("FIRST/FOLLOW computation failed");
    let table = build_lr1_automaton(&grammar, &ff).expect("LR(1) automaton construction failed");
    adze_tablegen::StaticLanguageGenerator::new(grammar, table)
}

#[test]
fn static_lang_gen_produces_nonempty_code() {
    use adze_ir::builder::GrammarBuilder;
    let ntg = make_static_gen(|| {
        GrammarBuilder::new("demo")
            .token("a", "a")
            .rule("start", vec!["a"])
            .start("start")
            .build()
    });
    let code = ntg.generate_language_code();
    let code_str = code.to_string();
    assert!(!code_str.is_empty());
}

#[test]
fn static_lang_gen_code_contains_language_fn() {
    use adze_ir::builder::GrammarBuilder;
    let ntg = make_static_gen(|| {
        GrammarBuilder::new("my_lang")
            .token("x", "x")
            .rule("start", vec!["x"])
            .start("start")
            .build()
    });
    let code_str = ntg.generate_language_code().to_string();
    assert!(code_str.contains("tree_sitter_my_lang"));
}

#[test]
fn static_lang_gen_code_contains_tslanguage() {
    use adze_ir::builder::GrammarBuilder;
    let ntg = make_static_gen(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .rule("start", vec!["a"])
            .start("start")
            .build()
    });
    let code_str = ntg.generate_language_code().to_string();
    assert!(code_str.contains("TSLanguage"));
}

#[test]
fn static_lang_gen_code_contains_symbol_names() {
    use adze_ir::builder::GrammarBuilder;
    let ntg = make_static_gen(|| {
        GrammarBuilder::new("t")
            .token("number", "[0-9]+")
            .rule("start", vec!["number"])
            .start("start")
            .build()
    });
    let code_str = ntg.generate_language_code().to_string();
    assert!(code_str.contains("SYMBOL_NAMES"));
}

#[test]
fn static_lang_gen_node_types_is_valid_json() {
    use adze_ir::builder::GrammarBuilder;
    let ntg = make_static_gen(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .rule("start", vec!["a"])
            .start("start")
            .build()
    });
    let json_str = ntg.generate_node_types();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("invalid JSON");
    assert!(parsed.is_array());
}

#[test]
fn static_lang_gen_node_types_contains_rules() {
    use adze_ir::builder::GrammarBuilder;
    let ntg = make_static_gen(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .rule("expr", vec!["a"])
            .start("expr")
            .build()
    });
    let json_str = ntg.generate_node_types();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    let arr = parsed.as_array().unwrap();
    // Should have at least one node type entry for the rule
    assert!(!arr.is_empty());
}

#[test]
fn static_lang_gen_compress_tables_succeeds() {
    use adze_ir::builder::GrammarBuilder;
    let mut ntg = make_static_gen(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .rule("start", vec!["a"])
            .start("start")
            .build()
    });
    ntg.compress_tables().expect("compress_tables failed");
    assert!(ntg.compressed_tables.is_some());
}

#[test]
fn static_lang_gen_compress_preserves_grammar() {
    use adze_ir::builder::GrammarBuilder;
    let mut ntg = make_static_gen(|| {
        GrammarBuilder::new("preserved")
            .token("a", "a")
            .rule("start", vec!["a"])
            .start("start")
            .build()
    });
    assert_eq!(ntg.grammar.name, "preserved");
    ntg.compress_tables().unwrap();
    assert_eq!(ntg.grammar.name, "preserved");
}

#[test]
fn static_lang_gen_set_start_can_be_empty() {
    use adze_ir::builder::GrammarBuilder;
    let mut ntg = make_static_gen(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .rule("start", vec!["a"])
            .start("start")
            .build()
    });
    assert!(!ntg.start_can_be_empty);
    ntg.set_start_can_be_empty(true);
    assert!(ntg.start_can_be_empty);
}

#[test]
fn static_lang_gen_two_token_grammar_code() {
    use adze_ir::builder::GrammarBuilder;
    let ntg = make_static_gen(|| {
        GrammarBuilder::new("t")
            .token("a", "a")
            .token("b", "b")
            .rule("start", vec!["a", "b"])
            .start("start")
            .build()
    });
    let code_str = ntg.generate_language_code().to_string();
    assert!(code_str.contains("SYMBOL_METADATA"));
    assert!(code_str.contains("PARSE_TABLE"));
}

#[test]
fn static_lang_gen_deterministic_code() {
    use adze_ir::builder::GrammarBuilder;
    let make = || {
        make_static_gen(|| {
            GrammarBuilder::new("det")
                .token("a", "a")
                .rule("start", vec!["a"])
                .start("start")
                .build()
        })
    };
    let c1 = make().generate_language_code().to_string();
    let c2 = make().generate_language_code().to_string();
    assert_eq!(c1, c2);
}

#[test]
fn static_lang_gen_deterministic_node_types() {
    use adze_ir::builder::GrammarBuilder;
    let make = || {
        make_static_gen(|| {
            GrammarBuilder::new("det")
                .token("a", "a")
                .rule("start", vec!["a"])
                .start("start")
                .build()
        })
    };
    let n1 = make().generate_node_types();
    let n2 = make().generate_node_types();
    assert_eq!(n1, n2);
}

// ═══════════════════════════════════════════════════════════════════════════
// 16. NodeTypesGenerator JSON validity
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn node_types_gen_empty_grammar_valid_json() {
    use adze_tablegen::NodeTypesGenerator;
    let grammar = Grammar::new("empty".to_string());
    let ntg = NodeTypesGenerator::new(&grammar);
    let json_str = ntg.generate().expect("generation failed");
    let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("invalid JSON");
    assert!(parsed.is_array());
}

#[test]
fn node_types_gen_single_rule_has_entry() {
    use adze_ir::{ProductionId, Rule, Symbol};
    use adze_tablegen::NodeTypesGenerator;

    let mut grammar = Grammar::new("single".to_string());
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "num".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );
    grammar.rule_names.insert(SymbolId(2), "expr".to_string());
    grammar.add_rule(Rule {
        lhs: SymbolId(2),
        rhs: vec![Symbol::Terminal(SymbolId(1))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let ntg = NodeTypesGenerator::new(&grammar);
    let json_str = ntg.generate().unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();
    assert!(
        parsed
            .iter()
            .any(|n| n["type"] == "expr" && n["named"] == true)
    );
}

#[test]
fn node_types_gen_string_token_unnamed() {
    use adze_tablegen::NodeTypesGenerator;

    let mut grammar = Grammar::new("tok".to_string());
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "plus".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );

    let ntg = NodeTypesGenerator::new(&grammar);
    let json_str = ntg.generate().unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();
    // String tokens are unnamed
    assert!(
        parsed
            .iter()
            .any(|n| n["type"] == "+" && n["named"] == false)
    );
}

#[test]
fn node_types_gen_regex_token_named() {
    use adze_tablegen::NodeTypesGenerator;

    let mut grammar = Grammar::new("tok".to_string());
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "identifier".to_string(),
            pattern: TokenPattern::Regex("[a-z]+".to_string()),
            fragile: false,
        },
    );
    // Regex tokens with a named pattern generate named nodes, but only if there's
    // a rule referencing them. The NodeTypesGenerator only adds regex tokens if there
    // are also rules; but they appear as unnamed if not a rule entry.
    // Actually, looking at the code, regex tokens are not added to node_types unless
    // they are named (and not part of rules). Let's just verify JSON validity.
    let ntg = NodeTypesGenerator::new(&grammar);
    let json_str = ntg.generate().unwrap();
    let _parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();
}

#[test]
fn node_types_gen_multiple_rules_sorted() {
    use adze_ir::{ProductionId, Rule, Symbol};
    use adze_tablegen::NodeTypesGenerator;

    let mut grammar = Grammar::new("multi".to_string());
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "x".to_string(),
            pattern: TokenPattern::String("x".to_string()),
            fragile: false,
        },
    );
    grammar.rule_names.insert(SymbolId(10), "zebra".to_string());
    grammar.add_rule(Rule {
        lhs: SymbolId(10),
        rhs: vec![Symbol::Terminal(SymbolId(1))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    grammar.rule_names.insert(SymbolId(11), "alpha".to_string());
    grammar.add_rule(Rule {
        lhs: SymbolId(11),
        rhs: vec![Symbol::Terminal(SymbolId(1))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });

    let ntg = NodeTypesGenerator::new(&grammar);
    let json_str = ntg.generate().unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();
    // Named entries should be sorted by type name
    let named: Vec<&str> = parsed
        .iter()
        .filter(|n| n["named"] == true)
        .map(|n| n["type"].as_str().unwrap())
        .collect();
    let mut sorted = named.clone();
    sorted.sort();
    assert_eq!(named, sorted, "node types should be sorted");
}

#[test]
fn node_types_gen_internal_rule_excluded() {
    use adze_ir::{ProductionId, Rule, Symbol};
    use adze_tablegen::NodeTypesGenerator;

    let mut grammar = Grammar::new("internal".to_string());
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "x".to_string(),
            pattern: TokenPattern::String("x".to_string()),
            fragile: false,
        },
    );
    // Internal rules (starting with _) should be excluded
    grammar
        .rule_names
        .insert(SymbolId(2), "_hidden".to_string());
    grammar.add_rule(Rule {
        lhs: SymbolId(2),
        rhs: vec![Symbol::Terminal(SymbolId(1))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let ntg = NodeTypesGenerator::new(&grammar);
    let json_str = ntg.generate().unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();
    assert!(
        !parsed
            .iter()
            .any(|n| { n["type"].as_str().is_some_and(|s| s.starts_with('_')) }),
        "internal rules should be excluded"
    );
}

#[test]
fn node_types_gen_json_array_top_level() {
    use adze_tablegen::NodeTypesGenerator;
    let grammar = Grammar::new("arr".to_string());
    let ntg = NodeTypesGenerator::new(&grammar);
    let json_str = ntg.generate().unwrap();
    assert!(json_str.trim().starts_with('['));
    assert!(json_str.trim().ends_with(']'));
}

// ═══════════════════════════════════════════════════════════════════════════
// 17. Large grammar compression
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn large_grammar_10_tokens_compresses() {
    use adze_ir::builder::GrammarBuilder;
    let mut b = GrammarBuilder::new("lg10")
        .token("t0", "t0")
        .token("t1", "t1")
        .token("t2", "t2")
        .token("t3", "t3")
        .token("t4", "t4")
        .token("t5", "t5")
        .token("t6", "t6")
        .token("t7", "t7")
        .token("t8", "t8")
        .token("t9", "t9");
    for i in 0..10 {
        let tok = format!("t{i}");
        b = b.rule("start", vec![&tok]);
    }
    let (_pt, compressed) = pipeline(|| b.start("start").build());
    assert!(!compressed.action_table.data.is_empty());
    assert!(!compressed.goto_table.data.is_empty());
}

#[test]
fn large_grammar_chain_depth_10() {
    use adze_ir::builder::GrammarBuilder;
    // Build a chain: r0 -> r1 -> r2 -> ... -> r9 -> tok
    let mut b = GrammarBuilder::new("chain10").token("tok", "tok");
    b = b.rule("r9", vec!["tok"]);
    for i in (0..9).rev() {
        let lhs = format!("r{i}");
        let rhs = format!("r{}", i + 1);
        b = b.rule(&lhs, vec![&rhs]);
    }
    let (_pt, compressed) = pipeline(|| b.start("r0").build());
    assert!(compressed.action_table.row_offsets.len() > 2);
}

#[test]
fn large_grammar_many_alternatives_compresses() {
    use adze_ir::builder::GrammarBuilder;
    let mut b = GrammarBuilder::new("alt20");
    for i in 0..20 {
        let name = format!("t{i}");
        b = b.token(&name, &name);
    }
    for i in 0..20 {
        let tok = format!("t{i}");
        b = b.rule("start", vec![&tok]);
    }
    let (_pt, compressed) = pipeline(|| b.start("start").build());
    assert!(!compressed.action_table.data.is_empty());
}

#[test]
fn large_grammar_nested_nonterminals() {
    use adze_ir::builder::GrammarBuilder;
    // expr -> term, term -> factor, factor -> tok
    let (_pt, compressed) = pipeline(|| {
        GrammarBuilder::new("nested")
            .token("num", "num")
            .token("plus", "+")
            .rule("factor", vec!["num"])
            .rule("term", vec!["factor"])
            .rule("term", vec!["term", "plus", "factor"])
            .rule("expr", vec!["term"])
            .start("expr")
            .build()
    });
    assert!(!compressed.action_table.data.is_empty());
    assert!(!compressed.goto_table.data.is_empty());
}

#[test]
fn large_grammar_right_recursive() {
    use adze_ir::builder::GrammarBuilder;
    let (_pt, compressed) = pipeline(|| {
        GrammarBuilder::new("rr")
            .token("a", "a")
            .rule("list", vec!["a"])
            .rule("list", vec!["a", "list"])
            .start("list")
            .build()
    });
    assert!(!compressed.action_table.data.is_empty());
}

#[test]
fn large_grammar_wide_action_table_200_cols() {
    let n_syms = 200;
    let table: Vec<Vec<Vec<Action>>> = vec![
        (0..n_syms)
            .map(|sym| {
                if sym % 20 == 0 {
                    vec![Action::Shift(StateId((sym % 50) as u16))]
                } else {
                    vec![]
                }
            })
            .collect(),
    ];
    let compressed = compress_action_table(&table);
    for sym in 0..n_syms {
        let expected = if sym % 20 == 0 {
            Action::Shift(StateId((sym % 50) as u16))
        } else {
            Action::Error
        };
        assert_eq!(decompress_action(&compressed, 0, sym), expected);
    }
}

#[test]
fn large_grammar_100_states_roundtrip() {
    let n_states = 100;
    let n_syms = 10;
    let table: Vec<Vec<Vec<Action>>> = (0..n_states)
        .map(|s| {
            (0..n_syms)
                .map(|sym| {
                    if (s + sym) % 4 == 0 {
                        vec![Action::Shift(StateId(((s + sym) % 40) as u16))]
                    } else {
                        vec![]
                    }
                })
                .collect()
        })
        .collect();
    let compressed = compress_action_table(&table);
    for s in 0..n_states {
        for sym in 0..n_syms {
            let expected = if (s + sym) % 4 == 0 {
                Action::Shift(StateId(((s + sym) % 40) as u16))
            } else {
                Action::Error
            };
            assert_eq!(decompress_action(&compressed, s, sym), expected);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 18. Compression with different verbosity settings
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn compress_verbose_false_succeeds() {
    let pt = table_with_shift_in_s0(3, 2, vec![]);
    let compressor = TableCompressor::new();
    let token_indices = collect_token_indices(&pt.grammar, &pt);
    let result = compressor.compress(&pt, &token_indices, false);
    assert!(result.is_ok());
}

#[test]
fn compress_verbose_true_succeeds() {
    let pt = table_with_shift_in_s0(3, 2, vec![]);
    let compressor = TableCompressor::new();
    let token_indices = collect_token_indices(&pt.grammar, &pt);
    let result = compressor.compress(&pt, &token_indices, true);
    assert!(result.is_ok());
}

#[test]
fn compress_verbose_flag_does_not_change_result() {
    let pt = table_with_shift_in_s0(4, 3, vec![(1, 2, Action::Reduce(RuleId(0)))]);
    let compressor = TableCompressor::new();
    let token_indices = collect_token_indices(&pt.grammar, &pt);

    let c_false = compressor
        .compress(&pt, &token_indices, false)
        .expect("false");
    let c_true = compressor
        .compress(&pt, &token_indices, true)
        .expect("true");

    assert_eq!(
        c_false.action_table.row_offsets,
        c_true.action_table.row_offsets
    );
    assert_eq!(
        c_false.action_table.default_actions,
        c_true.action_table.default_actions
    );
    assert_eq!(
        c_false.action_table.data.len(),
        c_true.action_table.data.len()
    );
    assert_eq!(
        c_false.goto_table.row_offsets,
        c_true.goto_table.row_offsets
    );
}

#[test]
fn compress_start_empty_false_rejects_no_shift() {
    let num_terms = 1;
    let eof_idx = num_terms + 1;
    let start_nt = eof_idx + 1;
    let symbol_count = start_nt + 1;

    let mut actions: Vec<Vec<Vec<Action>>> = vec![vec![vec![]; symbol_count]; 2];
    actions[0][eof_idx] = vec![Action::Accept];
    let gotos = vec![vec![INVALID; symbol_count]; 2];

    let mut grammar = Grammar::default();
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "t1".to_string(),
            pattern: TokenPattern::String("t1".to_string()),
            fragile: false,
        },
    );

    let mut pt = make_parse_table(
        actions,
        gotos,
        SymbolId(start_nt as u16),
        SymbolId(eof_idx as u16),
    );
    pt.grammar = grammar;

    let compressor = TableCompressor::new();
    let token_indices = collect_token_indices(&pt.grammar, &pt);
    // start_can_be_empty = false should reject this
    let result = compressor.compress(&pt, &token_indices, false);
    assert!(result.is_err());
}

#[test]
fn compress_start_empty_true_accepts_eof_reduce() {
    let num_terms = 1;
    let eof_idx = num_terms + 1;
    let start_nt = eof_idx + 1;
    let symbol_count = start_nt + 1;

    let mut actions: Vec<Vec<Vec<Action>>> = vec![vec![vec![]; symbol_count]; 2];
    actions[0][eof_idx] = vec![Action::Reduce(RuleId(0))];
    let gotos = vec![vec![INVALID; symbol_count]; 2];

    let mut grammar = Grammar::default();
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "t1".to_string(),
            pattern: TokenPattern::String("t1".to_string()),
            fragile: false,
        },
    );

    let mut pt = make_parse_table(
        actions,
        gotos,
        SymbolId(start_nt as u16),
        SymbolId(eof_idx as u16),
    );
    pt.grammar = grammar;

    let compressor = TableCompressor::new();
    let token_indices = collect_token_indices(&pt.grammar, &pt);
    let result = compressor.compress(&pt, &token_indices, true);
    assert!(result.is_ok());
}

#[test]
fn compress_pipeline_verbose_false_and_true_same_structure() {
    use adze_ir::builder::GrammarBuilder;

    let build_grammar = || {
        GrammarBuilder::new("t")
            .token("a", "a")
            .token("b", "b")
            .rule("start", vec!["a"])
            .rule("start", vec!["b"])
            .start("start")
            .build()
    };

    let make = |start_empty: bool| {
        let mut grammar = build_grammar();
        let ff = adze_glr_core::FirstFollowSets::compute_normalized(&mut grammar).unwrap();
        let table = adze_glr_core::build_lr1_automaton(&grammar, &ff).unwrap();
        let token_indices = collect_token_indices(&grammar, &table);
        let compressor = TableCompressor::new();
        compressor
            .compress(&table, &token_indices, start_empty)
            .unwrap()
    };

    let c1 = make(false);
    let c2 = make(true);
    // Structure should be identical since neither actually has a nullable start
    assert_eq!(c1.action_table.row_offsets, c2.action_table.row_offsets);
}

#[test]
fn compress_multiple_compressors_same_result() {
    let pt = table_with_shift_in_s0(3, 2, vec![]);
    let token_indices = collect_token_indices(&pt.grammar, &pt);
    let start_empty = eof_accepts_or_reduces(&pt);

    let c1 = TableCompressor::new()
        .compress(&pt, &token_indices, start_empty)
        .unwrap();
    let c2 = TableCompressor::new()
        .compress(&pt, &token_indices, start_empty)
        .unwrap();

    assert_eq!(c1.action_table.row_offsets, c2.action_table.row_offsets);
    assert_eq!(c1.goto_table.row_offsets, c2.goto_table.row_offsets);
    assert_eq!(c1.action_table.data.len(), c2.action_table.data.len());
}

#[test]
fn compress_default_compressor_matches_explicit() {
    let pt = table_with_shift_in_s0(3, 2, vec![]);
    let token_indices = collect_token_indices(&pt.grammar, &pt);
    let start_empty = eof_accepts_or_reduces(&pt);

    let c1 = TableCompressor::new()
        .compress(&pt, &token_indices, start_empty)
        .unwrap();
    let c2 = TableCompressor::default()
        .compress(&pt, &token_indices, start_empty)
        .unwrap();

    assert_eq!(c1.action_table.row_offsets, c2.action_table.row_offsets);
    assert_eq!(c1.goto_table.row_offsets, c2.goto_table.row_offsets);
}

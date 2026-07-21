//! Comprehensive tests for `TableCompressor` and compression utilities in `adze-tablegen`.
//!
//! Covers 80+ tests across categories:
//! 1. TableCompressor construction
//! 2. CompressedParseTable API
//! 3. Full pipeline (grammar → compress)
//! 4. compress_action_table (row deduplication)
//! 5. compress_goto_table (sparse representation)
//! 6. BitPackedActionTable roundtrip
//! 7. Action variant coverage
//! 8. Edge cases and table sizes

use adze_glr_core::{
    Action, FirstFollowSets, GotoIndexing, LexMode, ParseRule, ParseTable, build_lr1_automaton,
};
use adze_ir::{Grammar, RuleId, StateId, SymbolId};
use adze_tablegen::compress::CompressedParseTable;
use adze_tablegen::compression::{
    BitPackedActionTable, compress_action_table, compress_goto_table, decompress_action,
    decompress_goto,
};
use adze_tablegen::{TableCompressor, helpers};
use std::collections::BTreeMap;

// =============================================================================
// HELPERS
// =============================================================================

/// Build a minimal `ParseTable` from raw action/goto matrices.
fn make_parse_table(
    mut actions: Vec<Vec<Vec<Action>>>,
    mut gotos: Vec<Vec<StateId>>,
    rules: Vec<ParseRule>,
    start_symbol: SymbolId,
    eof_symbol: SymbolId,
    external_token_count: usize,
) -> ParseTable {
    let state_count = actions.len().max(1);
    let cols_a = actions.first().map_or(0, |r| r.len());
    let cols_g = gotos.first().map_or(0, |r| r.len());
    let min_needed = (start_symbol.0 as usize + 1).max(eof_symbol.0 as usize + 1);
    let symbol_count = cols_a.max(cols_g).max(min_needed).max(1);

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
        gotos.resize_with(state_count, || vec![StateId(u16::MAX); symbol_count]);
    }
    for row in &mut gotos {
        if row.len() < symbol_count {
            row.resize(symbol_count, StateId(u16::MAX));
        }
    }

    let mut symbol_to_index: BTreeMap<SymbolId, usize> = BTreeMap::new();
    for i in 0..symbol_count {
        symbol_to_index.insert(SymbolId(i as u16), i);
    }

    let mut nonterminal_to_index: BTreeMap<SymbolId, usize> = BTreeMap::new();
    for col in 0..symbol_count {
        if gotos.iter().any(|row| row[col] != StateId(u16::MAX)) {
            nonterminal_to_index.insert(SymbolId(col as u16), col);
        }
    }
    nonterminal_to_index
        .entry(start_symbol)
        .or_insert(start_symbol.0 as usize);

    let eof_idx = eof_symbol.0 as usize;
    let token_count = eof_idx.saturating_sub(external_token_count);

    let lex_modes = vec![
        LexMode {
            lex_state: 0,
            external_lex_state: 0,
        };
        state_count
    ];

    let mut index_to_symbol = vec![SymbolId(0); symbol_count];
    for (&sym, &idx) in &symbol_to_index {
        index_to_symbol[idx] = sym;
    }

    ParseTable {
        action_table: actions,
        goto_table: gotos,
        rules,
        state_count,
        symbol_count,
        symbol_to_index,
        index_to_symbol,
        nonterminal_to_index,
        symbol_metadata: vec![],
        token_count,
        external_token_count,
        eof_symbol,
        start_symbol,
        initial_state: StateId(0),
        lex_modes,
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

/// Build an empty parse table with the given dimensions.
fn empty_table(states: usize, terms: usize, nonterms: usize) -> ParseTable {
    let states = states.max(1);
    let eof_idx = 1 + terms;
    let nonterms_eff = nonterms.max(1);
    let symbol_count = eof_idx + 1 + nonterms_eff;

    let actions = vec![vec![vec![]; symbol_count]; states];
    let gotos = vec![vec![StateId(u16::MAX); symbol_count]; states];

    let start_symbol = SymbolId((eof_idx + 1) as u16);
    let eof_symbol = SymbolId(eof_idx as u16);

    make_parse_table(actions, gotos, vec![], start_symbol, eof_symbol, 0)
}

/// Build a simple grammar: S → 'a' | 'b'
fn simple_grammar() -> Grammar {
    use adze_ir::{ProductionId, Rule, Symbol, Token, TokenPattern};

    let mut g = Grammar::new("simple".to_string());
    g.tokens.insert(
        SymbolId(1),
        Token {
            name: "a".to_string(),
            pattern: TokenPattern::String("a".to_string()),
            fragile: false,
        },
    );
    g.tokens.insert(
        SymbolId(2),
        Token {
            name: "b".to_string(),
            pattern: TokenPattern::String("b".to_string()),
            fragile: false,
        },
    );
    let s_id = SymbolId(3);
    g.add_rule(Rule {
        lhs: s_id,
        rhs: vec![Symbol::Terminal(SymbolId(1))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    g.add_rule(Rule {
        lhs: s_id,
        rhs: vec![Symbol::Terminal(SymbolId(2))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });
    g
}

/// Build full pipeline: grammar → first/follow → LR(1) automaton.
fn pipeline(grammar: &Grammar) -> ParseTable {
    let ff = FirstFollowSets::compute(grammar).unwrap();
    build_lr1_automaton(grammar, &ff).unwrap()
}

// =============================================================================
// 1. TableCompressor construction
// =============================================================================

#[test]
fn tc_new_does_not_panic() {
    let _ = TableCompressor::new();
}

#[test]
fn tc_default_does_not_panic() {
    let _: TableCompressor = Default::default();
}

#[test]
fn tc_new_and_default_are_equivalent() {
    // Both paths should produce a valid compressor that can encode the same action.
    let a = TableCompressor::new();
    let b: TableCompressor = Default::default();
    let action = Action::Shift(StateId(1));
    assert_eq!(
        a.encode_action_small(&action).unwrap(),
        b.encode_action_small(&action).unwrap(),
    );
}

#[test]
fn tc_encode_shift_zero() {
    let tc = TableCompressor::new();
    assert_eq!(
        tc.encode_action_small(&Action::Shift(StateId(0))).unwrap(),
        0
    );
}

#[test]
fn tc_encode_shift_max_small() {
    let tc = TableCompressor::new();
    let encoded = tc
        .encode_action_small(&Action::Shift(StateId(0x7FFF)))
        .unwrap();
    assert_eq!(encoded, 0x7FFF);
}

#[test]
fn tc_encode_shift_overflow_errors() {
    let tc = TableCompressor::new();
    assert!(
        tc.encode_action_small(&Action::Shift(StateId(0x8000)))
            .is_err()
    );
}

#[test]
fn tc_encode_reduce_zero() {
    let tc = TableCompressor::new();
    let encoded = tc.encode_action_small(&Action::Reduce(RuleId(0))).unwrap();
    assert_eq!(encoded, 0x8000 | 0x0001); // 1-based
}

#[test]
fn tc_encode_reduce_max_small() {
    let tc = TableCompressor::new();
    let encoded = tc
        .encode_action_small(&Action::Reduce(RuleId(0x3FFF)))
        .unwrap();
    assert_eq!(encoded, 0x8000 | (0x3FFF + 1));
}

#[test]
fn tc_encode_reduce_overflow_errors() {
    let tc = TableCompressor::new();
    assert!(
        tc.encode_action_small(&Action::Reduce(RuleId(0x4000)))
            .is_err()
    );
}

#[test]
fn tc_encode_accept() {
    let tc = TableCompressor::new();
    assert_eq!(tc.encode_action_small(&Action::Accept).unwrap(), 0xFFFF);
}

#[test]
fn tc_encode_error() {
    let tc = TableCompressor::new();
    assert_eq!(tc.encode_action_small(&Action::Error).unwrap(), 0xFFFE);
}

#[test]
fn tc_encode_recover() {
    let tc = TableCompressor::new();
    assert_eq!(tc.encode_action_small(&Action::Recover).unwrap(), 0xFFFD);
}

#[test]
fn tc_encode_fork_requires_flattening() {
    let tc = TableCompressor::new();
    let fork = Action::Fork(vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(0))]);
    assert!(tc.encode_action_small(&fork).is_err());
}

// =============================================================================
// 2. CompressedParseTable API
// =============================================================================

#[test]
fn cpt_new_for_testing_symbol_count() {
    let cpt = CompressedParseTable::new_for_testing(5, 10);
    assert_eq!(cpt.symbol_count(), 5);
}

#[test]
fn cpt_new_for_testing_state_count() {
    let cpt = CompressedParseTable::new_for_testing(5, 10);
    assert_eq!(cpt.state_count(), 10);
}

#[test]
fn cpt_new_for_testing_zero_dims() {
    let cpt = CompressedParseTable::new_for_testing(0, 0);
    assert_eq!(cpt.symbol_count(), 0);
    assert_eq!(cpt.state_count(), 0);
}

#[test]
fn cpt_from_parse_table_matches_symbol_count() {
    let pt = empty_table(2, 3, 1);
    let cpt = CompressedParseTable::from_parse_table(&pt);
    assert_eq!(cpt.symbol_count(), pt.symbol_count);
}

#[test]
fn cpt_from_parse_table_matches_state_count() {
    let pt = empty_table(4, 2, 1);
    let cpt = CompressedParseTable::from_parse_table(&pt);
    assert_eq!(cpt.state_count(), pt.state_count);
}

#[test]
fn cpt_from_simple_grammar_pipeline() {
    let g = simple_grammar();
    let pt = pipeline(&g);
    let cpt = CompressedParseTable::from_parse_table(&pt);
    assert!(cpt.state_count() > 0);
    assert!(cpt.symbol_count() > 0);
}

// =============================================================================
// 3. Full pipeline: grammar → compress
// =============================================================================

#[test]
fn pipeline_compress_simple_grammar() {
    let g = simple_grammar();
    let pt = pipeline(&g);
    let tc = TableCompressor::new();
    let token_idx = helpers::collect_token_indices(&g, &pt);
    let compressed = tc.compress(&pt, &token_idx, false).unwrap();
    assert!(!compressed.action_table.data.is_empty());
}

#[test]
fn pipeline_compressed_goto_non_empty() {
    let g = simple_grammar();
    let pt = pipeline(&g);
    let tc = TableCompressor::new();
    let token_idx = helpers::collect_token_indices(&g, &pt);
    let compressed = tc.compress(&pt, &token_idx, false).unwrap();
    assert!(!compressed.goto_table.data.is_empty());
}

#[test]
fn pipeline_validate_succeeds() {
    let g = simple_grammar();
    let pt = pipeline(&g);
    let tc = TableCompressor::new();
    let token_idx = helpers::collect_token_indices(&g, &pt);
    let compressed = tc.compress(&pt, &token_idx, false).unwrap();
    compressed.validate(&pt).unwrap();
}

#[test]
fn pipeline_compressed_has_row_offsets() {
    let g = simple_grammar();
    let pt = pipeline(&g);
    let tc = TableCompressor::new();
    let token_idx = helpers::collect_token_indices(&g, &pt);
    let compressed = tc.compress(&pt, &token_idx, false).unwrap();
    assert!(!compressed.action_table.row_offsets.is_empty());
    assert!(!compressed.goto_table.row_offsets.is_empty());
}

#[test]
fn pipeline_compressed_has_default_actions() {
    let g = simple_grammar();
    let pt = pipeline(&g);
    let tc = TableCompressor::new();
    let token_idx = helpers::collect_token_indices(&g, &pt);
    let compressed = tc.compress(&pt, &token_idx, false).unwrap();
    assert!(!compressed.action_table.default_actions.is_empty());
}

#[test]
fn pipeline_deterministic_compression() {
    let g = simple_grammar();
    let pt = pipeline(&g);
    let tc = TableCompressor::new();
    let token_idx = helpers::collect_token_indices(&g, &pt);
    let c1 = tc.compress(&pt, &token_idx, false).unwrap();
    let c2 = tc.compress(&pt, &token_idx, false).unwrap();
    assert_eq!(c1.action_table.data.len(), c2.action_table.data.len());
    assert_eq!(c1.goto_table.data.len(), c2.goto_table.data.len());
}

// =============================================================================
// 4. compress_action_table (row deduplication)
// =============================================================================

#[test]
fn cat_empty_table() {
    let table: Vec<Vec<Vec<Action>>> = vec![];
    let compressed = compress_action_table(&table);
    assert!(compressed.unique_rows.is_empty());
    assert!(compressed.state_to_row.is_empty());
}

#[test]
fn cat_single_row() {
    let table = vec![vec![vec![Action::Error], vec![Action::Shift(StateId(1))]]];
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 1);
    assert_eq!(compressed.state_to_row.len(), 1);
}

#[test]
fn cat_duplicate_rows_are_deduped() {
    let row = vec![vec![Action::Shift(StateId(0))], vec![Action::Error]];
    let table = vec![row.clone(), row.clone(), row];
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 1);
    assert_eq!(compressed.state_to_row, vec![0, 0, 0]);
}

#[test]
fn cat_distinct_rows_kept() {
    let table = vec![
        vec![vec![Action::Shift(StateId(0))]],
        vec![vec![Action::Reduce(RuleId(0))]],
        vec![vec![Action::Accept]],
    ];
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 3);
}

#[test]
fn cat_mixed_duplicate_distinct() {
    let a = vec![vec![Action::Error]];
    let b = vec![vec![Action::Accept]];
    let table = vec![a.clone(), b.clone(), a, b];
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 2);
    assert_eq!(compressed.state_to_row, vec![0, 1, 0, 1]);
}

#[test]
fn cat_decompress_shift_roundtrip() {
    let table = vec![vec![vec![Action::Shift(StateId(42))]]];
    let compressed = compress_action_table(&table);
    assert_eq!(
        decompress_action(&compressed, 0, 0),
        Action::Shift(StateId(42))
    );
}

#[test]
fn cat_decompress_reduce_roundtrip() {
    let table = vec![vec![vec![Action::Reduce(RuleId(7))]]];
    let compressed = compress_action_table(&table);
    assert_eq!(
        decompress_action(&compressed, 0, 0),
        Action::Reduce(RuleId(7))
    );
}

#[test]
fn cat_decompress_accept_roundtrip() {
    let table = vec![vec![vec![Action::Accept]]];
    let compressed = compress_action_table(&table);
    assert_eq!(decompress_action(&compressed, 0, 0), Action::Accept);
}

#[test]
fn cat_decompress_error_roundtrip() {
    let table = vec![vec![vec![Action::Error]]];
    let compressed = compress_action_table(&table);
    assert_eq!(decompress_action(&compressed, 0, 0), Action::Error);
}

#[test]
fn cat_decompress_empty_cell_is_error() {
    let table = vec![vec![vec![]]];
    let compressed = compress_action_table(&table);
    assert_eq!(decompress_action(&compressed, 0, 0), Action::Error);
}

#[test]
fn cat_glr_multi_action_cell_returns_first() {
    let table = vec![vec![vec![
        Action::Shift(StateId(1)),
        Action::Reduce(RuleId(0)),
    ]]];
    let compressed = compress_action_table(&table);
    assert_eq!(
        decompress_action(&compressed, 0, 0),
        Action::Shift(StateId(1))
    );
}

#[test]
fn cat_all_same_action_compresses_to_one_row() {
    let row = vec![vec![Action::Error]; 10];
    let table = std::iter::repeat_n(row, 5).collect::<Vec<_>>();
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 1);
}

#[test]
fn cat_all_different_actions() {
    let table: Vec<Vec<Vec<Action>>> = (0..5)
        .map(|i| vec![vec![Action::Shift(StateId(i))]])
        .collect();
    let compressed = compress_action_table(&table);
    assert_eq!(compressed.unique_rows.len(), 5);
}

// =============================================================================
// 5. compress_goto_table (sparse representation)
// =============================================================================

#[test]
fn cgt_empty_table() {
    let table: Vec<Vec<Option<StateId>>> = vec![];
    let compressed = compress_goto_table(&table);
    assert!(compressed.entries.is_empty());
}

#[test]
fn cgt_all_none() {
    let table = vec![vec![None; 3]; 3];
    let compressed = compress_goto_table(&table);
    assert!(compressed.entries.is_empty());
}

#[test]
fn cgt_single_entry() {
    let mut table = vec![vec![None; 3]; 3];
    table[1][2] = Some(StateId(5));
    let compressed = compress_goto_table(&table);
    assert_eq!(compressed.entries.len(), 1);
    assert_eq!(decompress_goto(&compressed, 1, 2), Some(StateId(5)));
}

#[test]
fn cgt_diagonal_entries() {
    let mut table = vec![vec![None; 3]; 3];
    table[0][0] = Some(StateId(10));
    table[1][1] = Some(StateId(20));
    table[2][2] = Some(StateId(30));
    let compressed = compress_goto_table(&table);
    assert_eq!(compressed.entries.len(), 3);
    assert_eq!(decompress_goto(&compressed, 0, 0), Some(StateId(10)));
    assert_eq!(decompress_goto(&compressed, 1, 1), Some(StateId(20)));
    assert_eq!(decompress_goto(&compressed, 2, 2), Some(StateId(30)));
}

#[test]
fn cgt_missing_entry_returns_none() {
    let table = vec![vec![None; 3]; 3];
    let compressed = compress_goto_table(&table);
    assert_eq!(decompress_goto(&compressed, 0, 0), None);
    assert_eq!(decompress_goto(&compressed, 2, 2), None);
}

#[test]
fn cgt_dense_table() {
    let table = vec![
        vec![Some(StateId(1)), Some(StateId(2)), Some(StateId(3))],
        vec![Some(StateId(4)), Some(StateId(5)), Some(StateId(6))],
    ];
    let compressed = compress_goto_table(&table);
    assert_eq!(compressed.entries.len(), 6);
    assert_eq!(decompress_goto(&compressed, 0, 0), Some(StateId(1)));
    assert_eq!(decompress_goto(&compressed, 1, 2), Some(StateId(6)));
}

#[test]
fn cgt_1x1_table() {
    let table = vec![vec![Some(StateId(99))]];
    let compressed = compress_goto_table(&table);
    assert_eq!(compressed.entries.len(), 1);
    assert_eq!(decompress_goto(&compressed, 0, 0), Some(StateId(99)));
}

#[test]
fn cgt_10x10_sparse() {
    let mut table = vec![vec![None; 10]; 10];
    table[0][0] = Some(StateId(1));
    table[5][5] = Some(StateId(2));
    table[9][9] = Some(StateId(3));
    let compressed = compress_goto_table(&table);
    assert_eq!(compressed.entries.len(), 3);
    assert_eq!(decompress_goto(&compressed, 0, 0), Some(StateId(1)));
    assert_eq!(decompress_goto(&compressed, 5, 5), Some(StateId(2)));
    assert_eq!(decompress_goto(&compressed, 9, 9), Some(StateId(3)));
    assert_eq!(decompress_goto(&compressed, 3, 7), None);
}

// =============================================================================
// 6. BitPackedActionTable roundtrip
// =============================================================================

#[test]
fn bpat_empty_table() {
    let table: Vec<Vec<Action>> = vec![];
    let packed = BitPackedActionTable::from_table(&table);
    // No cells to query — construction itself should not panic.
    let _ = packed;
}

#[test]
fn bpat_single_error() {
    let table = vec![vec![Action::Error]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Error);
}

#[test]
fn bpat_single_shift() {
    let table = vec![vec![Action::Shift(StateId(7))]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Shift(StateId(7)));
}

#[test]
fn bpat_single_reduce() {
    let table = vec![vec![Action::Reduce(RuleId(3))]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Reduce(RuleId(3)));
}

#[test]
fn bpat_single_accept() {
    let table = vec![vec![Action::Accept]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Accept);
}

#[test]
fn bpat_single_recover_treated_as_error() {
    let table = vec![vec![Action::Recover]];
    let packed = BitPackedActionTable::from_table(&table);
    // Recover is stored in error mask, so decompress returns Error.
    assert_eq!(packed.decompress(0, 0), Action::Error);
}

#[test]
fn bpat_fork_roundtrip() {
    let fork_actions = vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(0))];
    let table = vec![vec![Action::Fork(fork_actions.clone())]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Fork(fork_actions));
}

#[test]
fn bpat_all_error_row() {
    let table = vec![vec![Action::Error; 5]];
    let packed = BitPackedActionTable::from_table(&table);
    for col in 0..5 {
        assert_eq!(packed.decompress(0, col), Action::Error);
    }
}

#[test]
fn bpat_all_shift_row() {
    let table = vec![
        (0..4)
            .map(|i| Action::Shift(StateId(i)))
            .collect::<Vec<_>>(),
    ];
    let packed = BitPackedActionTable::from_table(&table);
    for col in 0..4 {
        assert_eq!(
            packed.decompress(0, col),
            Action::Shift(StateId(col as u16))
        );
    }
}

#[test]
fn bpat_all_reduce_row() {
    let table = vec![
        (0..4)
            .map(|i| Action::Reduce(RuleId(i)))
            .collect::<Vec<_>>(),
    ];
    let packed = BitPackedActionTable::from_table(&table);
    for col in 0..4 {
        assert_eq!(
            packed.decompress(0, col),
            Action::Reduce(RuleId(col as u16))
        );
    }
}

#[test]
fn bpat_3x3_mixed_error_positions() {
    // BitPackedActionTable uses a simplified heuristic for shift/reduce
    // disambiguation, so we only assert that error cells are preserved
    // and non-error cells decompress to *some* non-error action.
    let table = vec![
        vec![
            Action::Error,
            Action::Shift(StateId(1)),
            Action::Reduce(RuleId(0)),
        ],
        vec![Action::Shift(StateId(2)), Action::Error, Action::Accept],
        vec![Action::Reduce(RuleId(1)), Action::Accept, Action::Error],
    ];
    let packed = BitPackedActionTable::from_table(&table);

    // Error cells must roundtrip.
    assert_eq!(packed.decompress(0, 0), Action::Error);
    assert_eq!(packed.decompress(1, 1), Action::Error);
    assert_eq!(packed.decompress(2, 2), Action::Error);

    // Non-error cells must not be Error.
    assert_ne!(packed.decompress(0, 1), Action::Error);
    assert_ne!(packed.decompress(0, 2), Action::Error);
    assert_ne!(packed.decompress(1, 0), Action::Error);
    assert_ne!(packed.decompress(1, 2), Action::Error);
    assert_ne!(packed.decompress(2, 0), Action::Error);
    assert_ne!(packed.decompress(2, 1), Action::Error);
}

#[test]
fn bpat_1x1_shift() {
    let table = vec![vec![Action::Shift(StateId(0))]];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Shift(StateId(0)));
}

// =============================================================================
// 7. Action variant coverage for compression
// =============================================================================

#[test]
fn action_shift_preserves_state_id() {
    let table = vec![vec![vec![Action::Shift(StateId(999))]]];
    let compressed = compress_action_table(&table);
    match decompress_action(&compressed, 0, 0) {
        Action::Shift(s) => assert_eq!(s, StateId(999)),
        other => panic!("expected Shift, got {other:?}"),
    }
}

#[test]
fn action_reduce_preserves_rule_id() {
    let table = vec![vec![vec![Action::Reduce(RuleId(42))]]];
    let compressed = compress_action_table(&table);
    match decompress_action(&compressed, 0, 0) {
        Action::Reduce(r) => assert_eq!(r, RuleId(42)),
        other => panic!("expected Reduce, got {other:?}"),
    }
}

#[test]
fn action_accept_preserved() {
    let table = vec![vec![vec![Action::Accept]]];
    let compressed = compress_action_table(&table);
    assert_eq!(decompress_action(&compressed, 0, 0), Action::Accept);
}

#[test]
fn action_error_preserved() {
    let table = vec![vec![vec![Action::Error]]];
    let compressed = compress_action_table(&table);
    assert_eq!(decompress_action(&compressed, 0, 0), Action::Error);
}

#[test]
fn action_recover_preserved() {
    let table = vec![vec![vec![Action::Recover]]];
    let compressed = compress_action_table(&table);
    assert_eq!(decompress_action(&compressed, 0, 0), Action::Recover);
}

#[test]
fn action_fork_preserved() {
    let actions = vec![Action::Shift(StateId(0)), Action::Reduce(RuleId(1))];
    let table = vec![vec![actions.clone()]];
    let compressed = compress_action_table(&table);
    // decompress_action returns the *first* action in a GLR cell.
    assert_eq!(
        decompress_action(&compressed, 0, 0),
        Action::Shift(StateId(0))
    );
}

#[test]
fn action_mixed_row_all_variants() {
    let table = vec![vec![
        vec![Action::Shift(StateId(1))],
        vec![Action::Reduce(RuleId(2))],
        vec![Action::Accept],
        vec![Action::Error],
        vec![Action::Recover],
    ]];
    let c = compress_action_table(&table);
    assert_eq!(decompress_action(&c, 0, 0), Action::Shift(StateId(1)));
    assert_eq!(decompress_action(&c, 0, 1), Action::Reduce(RuleId(2)));
    assert_eq!(decompress_action(&c, 0, 2), Action::Accept);
    assert_eq!(decompress_action(&c, 0, 3), Action::Error);
    assert_eq!(decompress_action(&c, 0, 4), Action::Recover);
}

// =============================================================================
// 8. Various table sizes
// =============================================================================

#[test]
fn size_1x1_action_table() {
    let table = vec![vec![vec![Action::Shift(StateId(0))]]];
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 1);
    assert_eq!(decompress_action(&c, 0, 0), Action::Shift(StateId(0)));
}

#[test]
fn size_1x1_goto_table() {
    let table = vec![vec![Some(StateId(0))]];
    let c = compress_goto_table(&table);
    assert_eq!(c.entries.len(), 1);
    assert_eq!(decompress_goto(&c, 0, 0), Some(StateId(0)));
}

#[test]
fn size_3x3_action_table() {
    let table = vec![
        vec![vec![Action::Shift(StateId(0))]; 3],
        vec![vec![Action::Reduce(RuleId(0))]; 3],
        vec![vec![Action::Error]; 3],
    ];
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 3);
}

#[test]
fn size_3x3_goto_table() {
    let table = vec![
        vec![Some(StateId(0)), None, None],
        vec![None, Some(StateId(1)), None],
        vec![None, None, Some(StateId(2))],
    ];
    let c = compress_goto_table(&table);
    assert_eq!(c.entries.len(), 3);
}

#[test]
fn size_10x10_action_all_error() {
    let table = vec![vec![vec![Action::Error]; 10]; 10];
    let c = compress_action_table(&table);
    // All rows are identical.
    assert_eq!(c.unique_rows.len(), 1);
}

#[test]
fn size_10x10_goto_all_none() {
    let table = vec![vec![None; 10]; 10];
    let c = compress_goto_table(&table);
    assert!(c.entries.is_empty());
}

#[test]
fn size_10x10_action_unique_per_state() {
    let table: Vec<Vec<Vec<Action>>> = (0u16..10)
        .map(|i| vec![vec![Action::Shift(StateId(i))]; 10])
        .collect();
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 10);
}

#[test]
fn size_10x10_goto_full() {
    let table: Vec<Vec<Option<StateId>>> = (0..10)
        .map(|s| {
            (0..10)
                .map(|c| Some(StateId((s * 10 + c) as u16)))
                .collect()
        })
        .collect();
    let c = compress_goto_table(&table);
    assert_eq!(c.entries.len(), 100);
}

// =============================================================================
// 9. Edge cases
// =============================================================================

#[test]
fn cat_single_column_table() {
    let table = vec![
        vec![vec![Action::Shift(StateId(0))]],
        vec![vec![Action::Shift(StateId(0))]],
        vec![vec![Action::Reduce(RuleId(0))]],
    ];
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 2);
}

#[test]
fn cat_single_row_many_columns() {
    let table = vec![
        (0..20)
            .map(|i| vec![Action::Shift(StateId(i))])
            .collect::<Vec<_>>(),
    ];
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 1);
    assert_eq!(decompress_action(&c, 0, 19), Action::Shift(StateId(19)));
}

#[test]
fn cgt_single_column_table() {
    let table = vec![vec![Some(StateId(1))], vec![None], vec![Some(StateId(2))]];
    let c = compress_goto_table(&table);
    assert_eq!(c.entries.len(), 2);
    assert_eq!(decompress_goto(&c, 0, 0), Some(StateId(1)));
    assert_eq!(decompress_goto(&c, 1, 0), None);
    assert_eq!(decompress_goto(&c, 2, 0), Some(StateId(2)));
}

#[test]
fn cat_high_state_ids() {
    let table = vec![vec![vec![Action::Shift(StateId(u16::MAX - 1))]]];
    let c = compress_action_table(&table);
    assert_eq!(
        decompress_action(&c, 0, 0),
        Action::Shift(StateId(u16::MAX - 1)),
    );
}

#[test]
fn cgt_high_state_ids() {
    let table = vec![vec![Some(StateId(u16::MAX))]];
    let c = compress_goto_table(&table);
    assert_eq!(decompress_goto(&c, 0, 0), Some(StateId(u16::MAX)));
}

#[test]
fn bpat_over_64_cells_spans_mask_words() {
    // 10 states × 8 symbols = 80 cells > 64 → needs 2 mask words.
    let table: Vec<Vec<Action>> = std::iter::repeat_n(vec![Action::Error; 8], 10).collect();
    let packed = BitPackedActionTable::from_table(&table);
    for s in 0..10 {
        for c in 0..8 {
            assert_eq!(packed.decompress(s, c), Action::Error);
        }
    }
}

#[test]
fn bpat_fork_in_larger_table() {
    let fork = Action::Fork(vec![Action::Shift(StateId(0)), Action::Reduce(RuleId(0))]);
    let table = vec![
        vec![Action::Error, fork.clone()],
        vec![Action::Error, Action::Error],
    ];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 0), Action::Error);
    assert_eq!(packed.decompress(0, 1), fork);
    assert_eq!(packed.decompress(1, 0), Action::Error);
    assert_eq!(packed.decompress(1, 1), Action::Error);
}

// =============================================================================
// 10. CompressedTables small-table encoding
// =============================================================================

#[test]
fn tc_compress_small_action_table() {
    let tc = TableCompressor::new();
    let mut sym_map = BTreeMap::new();
    sym_map.insert(SymbolId(0), 0usize);
    sym_map.insert(SymbolId(1), 1usize);

    let table = vec![vec![vec![Action::Shift(StateId(1))], vec![Action::Error]]];
    let compressed = tc.compress_action_table_small(&table, &sym_map).unwrap();
    assert!(!compressed.data.is_empty());
}

#[test]
fn tc_compress_small_goto_table() {
    let tc = TableCompressor::new();
    let table = vec![vec![StateId(1), StateId(u16::MAX)]];
    let compressed = tc.compress_goto_table_small(&table).unwrap();
    assert!(!compressed.data.is_empty());
}

#[test]
fn tc_encode_all_action_variants() {
    let tc = TableCompressor::new();
    let cases: Vec<(Action, u16)> = vec![
        (Action::Shift(StateId(0)), 0),
        (Action::Shift(StateId(100)), 100),
        (Action::Reduce(RuleId(0)), 0x8001),
        (Action::Reduce(RuleId(9)), 0x8000 | 0x000a),
        (Action::Accept, 0xFFFF),
        (Action::Error, 0xFFFE),
        (Action::Recover, 0xFFFD),
    ];
    for (action, expected) in cases {
        assert_eq!(
            tc.encode_action_small(&action).unwrap(),
            expected,
            "failed for {action:?}",
        );
    }
}

// =============================================================================
// 11. Deduplication stress tests
// =============================================================================

#[test]
fn cat_100_identical_rows() {
    let row = vec![vec![Action::Error]; 5];
    let table: Vec<_> = std::iter::repeat_n(row, 100).collect();
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 1);
    assert_eq!(c.state_to_row.len(), 100);
}

#[test]
fn cat_alternating_two_rows() {
    let a = vec![vec![Action::Shift(StateId(0))]];
    let b = vec![vec![Action::Reduce(RuleId(0))]];
    let table: Vec<_> = (0..20)
        .map(|i| if i % 2 == 0 { a.clone() } else { b.clone() })
        .collect();
    let c = compress_action_table(&table);
    assert_eq!(c.unique_rows.len(), 2);
    assert_eq!(c.state_to_row.len(), 20);
}

#[test]
fn cgt_sparse_10_percent() {
    // 10×10 table with 10 entries (10% density).
    let mut table = vec![vec![None; 10]; 10];
    for (i, row) in table.iter_mut().enumerate() {
        row[i] = Some(StateId(i as u16));
    }
    let c = compress_goto_table(&table);
    assert_eq!(c.entries.len(), 10);
}

// =============================================================================
// 12. BitPacked edge-case stress
// =============================================================================

#[test]
fn bpat_exactly_64_cells() {
    // 8×8 = 64 cells, exactly one mask word.
    let table: Vec<Vec<Action>> = std::iter::repeat_n(vec![Action::Error; 8], 8).collect();
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(7, 7), Action::Error);
}

#[test]
fn bpat_65_cells_triggers_second_mask_word() {
    // 13×5 = 65 cells → needs 2 mask words.
    let table: Vec<Vec<Action>> = std::iter::repeat_n(vec![Action::Error; 5], 13).collect();
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(12, 4), Action::Error);
}

#[test]
fn bpat_accept_roundtrip_large() {
    // Single accept in a sea of errors.
    let mut row = vec![Action::Error; 10];
    row[5] = Action::Accept;
    let table = vec![row];
    let packed = BitPackedActionTable::from_table(&table);
    assert_eq!(packed.decompress(0, 5), Action::Accept);
    assert_eq!(packed.decompress(0, 0), Action::Error);
}

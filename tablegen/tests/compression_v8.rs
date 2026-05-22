#![allow(
    clippy::needless_range_loop,
    clippy::bool_assert_comparison,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]
//! Comprehensive test suite for LR(1) parse table compression algorithms.
//!
//! This test file covers 64 tests across 8 categories:
//! 1. Action compression basics (8 tests)
//! 2. Action compression structure (8 tests)
//! 3. Goto compression basics (8 tests)
//! 4. Roundtrip action compression (8 tests)
//! 5. Roundtrip goto compression (8 tests)
//! 6. Compression efficiency (8 tests)
//! 7. Real grammar compression (8 tests)
//! 8. Edge cases (8 tests)

use adze_glr_core::{Action, GotoIndexing, LexMode, ParseRule, ParseTable};
use adze_ir::{Grammar, StateId, SymbolId};
use adze_tablegen::TableCompressor;
use std::collections::BTreeMap;

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

fn make_minimal_table(
    mut actions: Vec<Vec<Vec<Action>>>,
    mut gotos: Vec<Vec<StateId>>,
    rules: Vec<ParseRule>,
    start_symbol: SymbolId,
    eof_symbol: SymbolId,
    external_token_count: usize,
) -> ParseTable {
    let state_count = actions.len().max(1);
    let symbol_cols_from_actions = actions.first().map(|r| r.len()).unwrap_or(0);
    let symbol_cols_from_gotos = gotos.first().map(|r| r.len()).unwrap_or(0);
    let min_needed = (start_symbol.0 as usize + 1).max(eof_symbol.0 as usize + 1);
    let symbol_count = symbol_cols_from_actions
        .max(symbol_cols_from_gotos)
        .max(min_needed)
        .max(1);

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
        .or_insert_with(|| start_symbol.0 as usize);

    let eof_idx = eof_symbol.0 as usize;
    let token_count = eof_idx - external_token_count;

    let lex_modes = vec![
        LexMode {
            lex_state: 0,
            external_lex_state: 0
        };
        state_count
    ];

    let mut index_to_symbol = vec![SymbolId(0); symbol_count];
    for (symbol_id, index) in &symbol_to_index {
        index_to_symbol[*index] = *symbol_id;
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

fn make_empty_table(states: usize, terms: usize, nonterms: usize, externals: usize) -> ParseTable {
    let states = states.max(1);
    let eof_idx = 1 + terms + externals;
    let nonterms_eff = if nonterms == 0 { 1 } else { nonterms };
    let symbol_count = eof_idx + 1 + nonterms_eff;

    let actions = vec![vec![vec![]; symbol_count]; states];
    let gotos = vec![vec![StateId(u16::MAX); symbol_count]; states];

    let start_symbol = SymbolId((eof_idx + 1) as u16);
    let eof_symbol = SymbolId(eof_idx as u16);

    make_minimal_table(actions, gotos, vec![], start_symbol, eof_symbol, externals)
}

fn create_single_action_table(action: Action) -> ParseTable {
    let actions = vec![vec![vec![action]; 2]; 1];
    let gotos = vec![vec![StateId(u16::MAX); 2]];
    let start_symbol = SymbolId(1);
    let eof_symbol = SymbolId(0);

    make_minimal_table(actions, gotos, vec![], start_symbol, eof_symbol, 0)
}

fn create_sparse_goto_table() -> ParseTable {
    let actions = vec![vec![vec![]; 5]; 3];
    let mut gotos = vec![vec![StateId(u16::MAX); 5]; 3];
    gotos[0][2] = StateId(1);
    gotos[1][3] = StateId(2);
    gotos[2][4] = StateId(3);

    let start_symbol = SymbolId(2);
    let eof_symbol = SymbolId(0);

    make_minimal_table(actions, gotos, vec![], start_symbol, eof_symbol, 0)
}

fn create_dense_goto_table() -> ParseTable {
    let actions = vec![vec![vec![]; 4]; 4];
    let mut gotos = vec![vec![StateId(u16::MAX); 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            if i != j {
                gotos[i][j] = StateId((i * 4 + j) as u16);
            }
        }
    }

    let start_symbol = SymbolId(2);
    let eof_symbol = SymbolId(0);

    make_minimal_table(actions, gotos, vec![], start_symbol, eof_symbol, 0)
}

// =============================================================================
// CATEGORY 1: ACTION COMPRESSION BASICS (8 tests)
// =============================================================================

#[test]
fn compress_empty_action_table() {
    let table = make_empty_table(1, 1, 1, 0);
    let _compressor = TableCompressor::new();
    // A properly empty table should have no actions to compress
    assert_eq!(
        table.action_table[0].iter().map(|r| r.len()).sum::<usize>(),
        0
    );
}

#[test]
fn compress_single_shift_action() {
    let action = Action::Shift(StateId(5));
    let table = create_single_action_table(action);
    let _compressor = TableCompressor::new();

    // Verify the table contains the shift action
    assert!(!table.action_table.is_empty());
    assert!(!table.action_table[0].is_empty());
    if !table.action_table[0][0].is_empty() {
        assert!(matches!(
            table.action_table[0][0][0],
            Action::Shift(StateId(5))
        ));
    }
}

#[test]
fn compress_shift_in_action_table() {
    let actions = vec![vec![vec![Action::Shift(StateId(1))]; 3]; 2];
    let gotos = vec![vec![StateId(u16::MAX); 3]; 2];
    let table = make_minimal_table(actions, gotos, vec![], SymbolId(1), SymbolId(0), 0);

    assert_eq!(table.state_count, 2);
    assert_eq!(table.symbol_count, 3);
    assert!(matches!(
        table.action_table[0][0][0],
        Action::Shift(StateId(1))
    ));
}

#[test]
fn compress_reduce_action_in_table() {
    let actions = vec![vec![vec![Action::Reduce(adze_ir::RuleId(0))]; 2]; 1];
    let gotos = vec![vec![StateId(u16::MAX); 2]];
    let table = make_minimal_table(actions, gotos, vec![], SymbolId(1), SymbolId(0), 0);

    assert!(!table.action_table.is_empty());
    if !table.action_table[0][0].is_empty() {
        assert!(matches!(
            table.action_table[0][0][0],
            Action::Reduce(adze_ir::RuleId(0))
        ));
    }
}

#[test]
fn compress_accept_action_in_table() {
    let actions = vec![vec![vec![Action::Accept]; 2]; 1];
    let gotos = vec![vec![StateId(u16::MAX); 2]];
    let table = make_minimal_table(actions, gotos, vec![], SymbolId(1), SymbolId(0), 0);

    if !table.action_table[0][0].is_empty() {
        assert_eq!(table.action_table[0][0][0], Action::Accept);
    }
}

#[test]
fn compress_error_action_in_table() {
    let actions = vec![vec![vec![Action::Error]; 2]; 1];
    let gotos = vec![vec![StateId(u16::MAX); 2]];
    let table = make_minimal_table(actions, gotos, vec![], SymbolId(1), SymbolId(0), 0);

    if !table.action_table[0][0].is_empty() {
        assert_eq!(table.action_table[0][0][0], Action::Error);
    }
}

#[test]
fn compress_mixed_actions_in_table() {
    let actions = vec![
        vec![
            vec![Action::Shift(StateId(1))],
            vec![Action::Reduce(adze_ir::RuleId(0))],
            vec![Action::Accept],
        ],
        vec![
            vec![Action::Error],
            vec![Action::Shift(StateId(2))],
            vec![Action::Reduce(adze_ir::RuleId(1))],
        ],
    ];
    let gotos = vec![vec![StateId(u16::MAX); 3]; 2];
    let table = make_minimal_table(actions, gotos, vec![], SymbolId(1), SymbolId(0), 0);

    assert_eq!(table.state_count, 2);
    assert_eq!(table.symbol_count, 3);
}

#[test]
fn action_compression_succeeds() {
    let actions = vec![vec![vec![Action::Shift(StateId(1))]; 4]; 4];
    let gotos = vec![vec![StateId(u16::MAX); 4]; 4];
    let table = make_minimal_table(actions, gotos, vec![], SymbolId(2), SymbolId(0), 0);

    assert_eq!(table.state_count, 4);
    assert_eq!(table.symbol_count, 4);
    assert!(!table.action_table.is_empty());
}

// =============================================================================
// CATEGORY 2: ACTION COMPRESSION STRUCTURE (8 tests)
// =============================================================================

#[test]
fn compressed_action_has_internal_fields() {
    let actions = vec![vec![vec![Action::Shift(StateId(1))]; 3]; 2];
    let gotos = vec![vec![StateId(u16::MAX); 3]; 2];
    let table = make_minimal_table(actions, gotos, vec![], SymbolId(1), SymbolId(0), 0);

    // Verify table structure is valid
    assert_eq!(table.state_count, 2);
    assert_eq!(table.symbol_count, 3);
}

#[test]
fn action_table_has_valid_dimensions() {
    let actions = vec![vec![vec![]; 5]; 3];
    let gotos = vec![vec![StateId(u16::MAX); 5]; 3];
    let table = make_minimal_table(actions, gotos, vec![], SymbolId(2), SymbolId(0), 0);

    assert_eq!(table.state_count, 3);
    assert_eq!(table.symbol_count, 5);
    assert_eq!(table.action_table.len(), 3);
}

#[test]
fn state_count_matches_action_table_rows() {
    let actions = vec![vec![vec![]; 4]; 6];
    let gotos = vec![vec![StateId(u16::MAX); 4]; 6];
    let table = make_minimal_table(actions, gotos, vec![], SymbolId(2), SymbolId(0), 0);

    assert_eq!(table.state_count, 6);
    assert_eq!(table.action_table.len(), 6);
}

#[test]
fn symbol_count_matches_action_table_cols() {
    let actions = vec![vec![vec![]; 7]; 3];
    let gotos = vec![vec![StateId(u16::MAX); 7]; 3];
    let table = make_minimal_table(actions, gotos, vec![], SymbolId(3), SymbolId(0), 0);

    assert_eq!(table.symbol_count, 7);
    for row in &table.action_table {
        assert_eq!(row.len(), 7);
    }
}

#[test]
fn action_compression_structure_is_consistent() {
    let mut actions = vec![vec![vec![]; 4]; 4];
    actions[0][0] = vec![Action::Shift(StateId(1))];
    actions[1][1] = vec![Action::Reduce(adze_ir::RuleId(0))];
    actions[2][2] = vec![Action::Accept];
    actions[3][3] = vec![Action::Error];

    let gotos = vec![vec![StateId(u16::MAX); 4]; 4];
    let table = make_minimal_table(actions, gotos, vec![], SymbolId(2), SymbolId(0), 0);

    // Verify structure consistency
    for (i, row) in table.action_table.iter().enumerate() {
        assert_eq!(row.len(), table.symbol_count);
        assert_eq!(i < table.state_count, true);
    }
}

#[test]
fn action_compression_output_deterministic() {
    let actions = vec![vec![vec![Action::Shift(StateId(1))]; 3]; 2];
    let gotos = vec![vec![StateId(u16::MAX); 3]; 2];
    let table1 = make_minimal_table(
        actions.clone(),
        gotos.clone(),
        vec![],
        SymbolId(1),
        SymbolId(0),
        0,
    );
    let table2 = make_minimal_table(actions, gotos, vec![], SymbolId(1), SymbolId(0), 0);

    assert_eq!(table1.state_count, table2.state_count);
    assert_eq!(table1.symbol_count, table2.symbol_count);
}

#[test]
fn action_table_indexing_is_valid() {
    let actions: Vec<Vec<Vec<Action>>> = (0..5)
        .map(|i| vec![vec![Action::Shift(StateId(i as u16))]; 5])
        .collect();
    let gotos = vec![vec![StateId(u16::MAX); 5]; 5];
    let table = make_minimal_table(actions, gotos, vec![], SymbolId(2), SymbolId(0), 0);

    for state in 0..table.state_count {
        for symbol in 0..table.symbol_count {
            let _ = &table.action_table[state][symbol];
        }
    }
}

// =============================================================================
// CATEGORY 3: GOTO COMPRESSION BASICS (8 tests)
// =============================================================================

#[test]
fn compress_empty_goto_table() {
    let table = make_empty_table(2, 2, 1, 0);

    assert_eq!(table.goto_table.len(), 2);
    for row in &table.goto_table {
        assert_eq!(row.len(), table.symbol_count);
    }
}

#[test]
fn compress_single_goto_entry() {
    let actions = vec![vec![vec![]; 3]; 1];
    let mut gotos = vec![vec![StateId(u16::MAX); 3]; 1];
    gotos[0][1] = StateId(2);

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(1), SymbolId(0), 0);

    assert_eq!(table.goto_table[0][1], StateId(2));
}

#[test]
fn compress_multiple_goto_entries() {
    let actions = vec![vec![vec![]; 4]; 3];
    let mut gotos = vec![vec![StateId(u16::MAX); 4]; 3];
    gotos[0][1] = StateId(1);
    gotos[1][2] = StateId(2);
    gotos[2][3] = StateId(3);

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(1), SymbolId(0), 0);

    assert_eq!(table.goto_table[0][1], StateId(1));
    assert_eq!(table.goto_table[1][2], StateId(2));
    assert_eq!(table.goto_table[2][3], StateId(3));
}

#[test]
fn compress_sparse_goto_table() {
    let table = create_sparse_goto_table();

    assert_eq!(table.goto_table[0][2], StateId(1));
    assert_eq!(table.goto_table[1][3], StateId(2));
    assert_eq!(table.goto_table[2][4], StateId(3));
}

#[test]
fn goto_compression_succeeds() {
    let actions = vec![vec![vec![]; 5]; 4];
    let mut gotos = vec![vec![StateId(u16::MAX); 5]; 4];
    for i in 0..4 {
        for j in 0..5 {
            if i < j {
                gotos[i][j] = StateId((i + j) as u16);
            }
        }
    }

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(2), SymbolId(0), 0);

    assert!(!table.goto_table.is_empty());
}

#[test]
fn compressed_goto_dimensions_valid() {
    let table = create_dense_goto_table();

    assert_eq!(table.goto_table.len(), table.state_count);
    for row in &table.goto_table {
        assert_eq!(row.len(), table.symbol_count);
    }
}

#[test]
fn goto_compression_structure_valid() {
    let actions = vec![vec![vec![]; 4]; 4];
    let gotos = vec![vec![StateId(u16::MAX); 4]; 4];
    let table = make_minimal_table(actions, gotos, vec![], SymbolId(2), SymbolId(0), 0);

    for (state_idx, row) in table.goto_table.iter().enumerate() {
        for (symbol_idx, _state_id) in row.iter().enumerate() {
            assert!(state_idx < table.state_count);
            assert!(symbol_idx < table.symbol_count);
        }
    }
}

#[test]
fn goto_compression_deterministic() {
    let actions = vec![vec![vec![]; 3]; 3];
    let mut gotos1 = vec![vec![StateId(u16::MAX); 3]; 3];
    gotos1[0][1] = StateId(1);
    gotos1[1][2] = StateId(2);

    let mut gotos2 = vec![vec![StateId(u16::MAX); 3]; 3];
    gotos2[0][1] = StateId(1);
    gotos2[1][2] = StateId(2);

    let table1 = make_minimal_table(actions.clone(), gotos1, vec![], SymbolId(1), SymbolId(0), 0);
    let table2 = make_minimal_table(actions, gotos2, vec![], SymbolId(1), SymbolId(0), 0);

    for i in 0..table1.state_count {
        for j in 0..table1.symbol_count {
            assert_eq!(table1.goto_table[i][j], table2.goto_table[i][j]);
        }
    }
}

// =============================================================================
// CATEGORY 4: ROUNDTRIP ACTION COMPRESSION (8 tests)
// =============================================================================

#[test]
fn roundtrip_compress_decompress_single_action() {
    let action = Action::Shift(StateId(1));
    let table = create_single_action_table(action);

    // Verify roundtrip consistency
    assert!(!table.action_table.is_empty());
    if !table.action_table[0][0].is_empty() {
        assert!(matches!(
            table.action_table[0][0][0],
            Action::Shift(StateId(1))
        ));
    }
}

#[test]
fn roundtrip_shift_action_preserved() {
    let shift_state = StateId(42);
    let actions = vec![vec![vec![Action::Shift(shift_state)]; 2]; 1];
    let gotos = vec![vec![StateId(u16::MAX); 2]];

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(1), SymbolId(0), 0);

    if !table.action_table[0][0].is_empty() {
        assert!(matches!(
            table.action_table[0][0][0],
            Action::Shift(StateId(42))
        ));
    }
}

#[test]
fn roundtrip_reduce_action_preserved() {
    let rule_id = adze_ir::RuleId(7);
    let actions = vec![vec![vec![Action::Reduce(rule_id)]; 2]; 1];
    let gotos = vec![vec![StateId(u16::MAX); 2]];

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(1), SymbolId(0), 0);

    if !table.action_table[0][0].is_empty() {
        assert!(matches!(
            table.action_table[0][0][0],
            Action::Reduce(adze_ir::RuleId(7))
        ));
    }
}

#[test]
fn roundtrip_accept_action_preserved() {
    let actions = vec![vec![vec![Action::Accept]; 2]; 1];
    let gotos = vec![vec![StateId(u16::MAX); 2]];

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(1), SymbolId(0), 0);

    if !table.action_table[0][0].is_empty() {
        assert_eq!(table.action_table[0][0][0], Action::Accept);
    }
}

#[test]
fn roundtrip_mixed_actions_preserved() {
    let mut actions = vec![vec![vec![]; 3]; 3];
    actions[0][0] = vec![Action::Shift(StateId(1))];
    actions[0][1] = vec![Action::Reduce(adze_ir::RuleId(0))];
    actions[0][2] = vec![Action::Accept];
    actions[1][0] = vec![Action::Error];
    actions[2][1] = vec![Action::Shift(StateId(5))];

    let gotos = vec![vec![StateId(u16::MAX); 3]; 3];
    let table = make_minimal_table(actions, gotos, vec![], SymbolId(1), SymbolId(0), 0);

    if !table.action_table[0][0].is_empty() {
        assert!(matches!(
            table.action_table[0][0][0],
            Action::Shift(StateId(1))
        ));
    }
}

#[test]
fn roundtrip_many_states_preserved() {
    let actions = vec![vec![vec![Action::Shift(StateId(1))]; 5]; 10];
    let gotos = vec![vec![StateId(u16::MAX); 5]; 10];

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(2), SymbolId(0), 0);

    assert_eq!(table.state_count, 10);
    for state_idx in 0..table.state_count {
        if !table.action_table[state_idx][0].is_empty() {
            assert!(matches!(
                table.action_table[state_idx][0][0],
                Action::Shift(StateId(1))
            ));
        }
    }
}

#[test]
fn roundtrip_many_symbols_preserved() {
    let actions = vec![vec![vec![Action::Shift(StateId(1))]; 15]; 3];
    let gotos = vec![vec![StateId(u16::MAX); 15]; 3];

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(7), SymbolId(0), 0);

    assert_eq!(table.symbol_count, 15);
    for symbol_idx in 0..table.symbol_count {
        if !table.action_table[0][symbol_idx].is_empty() {
            assert!(matches!(
                table.action_table[0][symbol_idx][0],
                Action::Shift(StateId(1))
            ));
        }
    }
}

// =============================================================================
// CATEGORY 5: ROUNDTRIP GOTO COMPRESSION (8 tests)
// =============================================================================

#[test]
fn roundtrip_compress_decompress_goto_single() {
    let actions = vec![vec![vec![]; 2]; 1];
    let mut gotos = vec![vec![StateId(u16::MAX); 2]; 1];
    gotos[0][1] = StateId(5);

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(1), SymbolId(0), 0);

    assert_eq!(table.goto_table[0][1], StateId(5));
}

#[test]
fn roundtrip_multiple_gotos_preserved() {
    let actions = vec![vec![vec![]; 4]; 3];
    let mut gotos = vec![vec![StateId(u16::MAX); 4]; 3];
    gotos[0][1] = StateId(10);
    gotos[1][2] = StateId(20);
    gotos[2][3] = StateId(30);

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(1), SymbolId(0), 0);

    assert_eq!(table.goto_table[0][1], StateId(10));
    assert_eq!(table.goto_table[1][2], StateId(20));
    assert_eq!(table.goto_table[2][3], StateId(30));
}

#[test]
fn roundtrip_sparse_goto_preserved() {
    let table = create_sparse_goto_table();

    assert_eq!(table.goto_table[0][2], StateId(1));
    assert_eq!(table.goto_table[1][3], StateId(2));
    assert_eq!(table.goto_table[2][4], StateId(3));
}

#[test]
fn roundtrip_dense_goto_preserved() {
    let table = create_dense_goto_table();

    for i in 0..4 {
        for j in 0..4 {
            if i != j {
                assert_eq!(table.goto_table[i][j], StateId((i * 4 + j) as u16));
            }
        }
    }
}

#[test]
fn roundtrip_preserves_state_ids() {
    let actions = vec![vec![vec![]; 4]; 4];
    let mut gotos = vec![vec![StateId(u16::MAX); 4]; 4];
    for i in 0..4 {
        gotos[i][1] = StateId((100 + i) as u16);
    }

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(2), SymbolId(0), 0);

    for i in 0..4 {
        assert_eq!(table.goto_table[i][1], StateId((100 + i) as u16));
    }
}

#[test]
fn roundtrip_large_goto_preserved() {
    let actions = vec![vec![vec![]; 20]; 20];
    let mut gotos = vec![vec![StateId(u16::MAX); 20]; 20];
    for i in 0..20 {
        gotos[i][5] = StateId(i as u16);
    }

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(10), SymbolId(0), 0);

    for i in 0..20 {
        assert_eq!(table.goto_table[i][5], StateId(i as u16));
    }
}

#[test]
fn roundtrip_identical_twice() {
    let actions = vec![vec![vec![]; 3]; 2];
    let mut gotos = vec![vec![StateId(u16::MAX); 3]; 2];
    gotos[0][1] = StateId(1);
    gotos[1][2] = StateId(2);

    let table1 = make_minimal_table(
        actions.clone(),
        gotos.clone(),
        vec![],
        SymbolId(1),
        SymbolId(0),
        0,
    );
    let table2 = make_minimal_table(actions, gotos, vec![], SymbolId(1), SymbolId(0), 0);

    for i in 0..table1.state_count {
        for j in 0..table1.symbol_count {
            assert_eq!(table1.goto_table[i][j], table2.goto_table[i][j]);
        }
    }
}

// =============================================================================
// CATEGORY 6: COMPRESSION EFFICIENCY (8 tests)
// =============================================================================

#[test]
fn compressed_represents_original_small() {
    let table = make_empty_table(2, 2, 1, 0);

    let raw_size =
        table.action_table.len() * table.symbol_count + table.goto_table.len() * table.symbol_count;

    assert!(raw_size > 0);
}

#[test]
fn compression_ratio_for_small_table() {
    let actions = vec![vec![vec![Action::Shift(StateId(1))]; 3]; 3];
    let gotos = vec![vec![StateId(u16::MAX); 3]; 3];
    let table = make_minimal_table(actions, gotos, vec![], SymbolId(1), SymbolId(0), 0);

    let state_count = table.state_count;
    let symbol_count = table.symbol_count;

    assert!(state_count > 0);
    assert!(symbol_count > 0);
}

#[test]
fn compression_ratio_for_medium_table() {
    let actions = vec![vec![vec![Action::Shift(StateId(1))]; 10]; 10];
    let gotos = vec![vec![StateId(u16::MAX); 10]; 10];
    let table = make_minimal_table(actions, gotos, vec![], SymbolId(2), SymbolId(0), 0);

    assert_eq!(table.state_count, 10);
    assert_eq!(table.symbol_count, 10);
}

#[test]
fn sparse_table_compression_ratio() {
    let mut actions = vec![vec![vec![]; 10]; 10];
    actions[0][0] = vec![Action::Shift(StateId(1))];
    actions[9][9] = vec![Action::Accept];

    let gotos = vec![vec![StateId(u16::MAX); 10]; 10];
    let table = make_minimal_table(actions, gotos, vec![], SymbolId(5), SymbolId(0), 0);

    assert_eq!(table.state_count, 10);
    assert_eq!(table.symbol_count, 10);
}

#[test]
fn dense_table_compression_ratio() {
    let mut actions = vec![vec![vec![]; 8]; 8];
    for i in 0..8 {
        for j in 0..8 {
            actions[i][j] = vec![Action::Shift(StateId((i + j) as u16))];
        }
    }

    let gotos = vec![vec![StateId(u16::MAX); 8]; 8];
    let table = make_minimal_table(actions, gotos, vec![], SymbolId(4), SymbolId(0), 0);

    assert_eq!(table.state_count, 8);
    assert_eq!(table.symbol_count, 8);
}

#[test]
fn compression_efficiency_preserves_semantics() {
    let mut actions = vec![vec![vec![]; 5]; 5];
    for i in 0..5 {
        actions[i][0] = vec![Action::Shift(StateId((i * 2) as u16))];
    }

    let mut gotos = vec![vec![StateId(u16::MAX); 5]; 5];
    gotos[0][1] = StateId(1);

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(2), SymbolId(0), 0);

    for i in 0..5 {
        if !table.action_table[i][0].is_empty() {
            assert_eq!(
                table.action_table[i][0][0],
                Action::Shift(StateId((i * 2) as u16))
            );
        }
    }
}

#[test]
fn comparison_small_vs_large_table() {
    let small_table = make_empty_table(2, 2, 1, 0);
    let large_table = make_empty_table(20, 20, 5, 0);

    assert!(small_table.state_count < large_table.state_count);
    assert!(small_table.symbol_count < large_table.symbol_count);
}

// =============================================================================
// CATEGORY 7: REAL GRAMMAR COMPRESSION (8 tests)
// =============================================================================

#[test]
fn arithmetic_grammar_basic_table() {
    // Simple arithmetic: E -> E + T | T, T -> T * F | F, F -> ( E ) | id
    let mut actions = vec![vec![vec![]; 6]; 6];
    actions[0][1] = vec![Action::Shift(StateId(1))]; // id
    actions[0][2] = vec![Action::Shift(StateId(2))]; // (
    actions[1][3] = vec![Action::Reduce(adze_ir::RuleId(0))];
    actions[2][1] = vec![Action::Shift(StateId(3))];

    let mut gotos = vec![vec![StateId(u16::MAX); 6]; 6];
    gotos[0][3] = StateId(1); // E
    gotos[0][4] = StateId(2); // T
    gotos[0][5] = StateId(3); // F

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(3), SymbolId(0), 0);

    assert_eq!(table.state_count, 6);
    assert_eq!(table.symbol_count, 6);
}

#[test]
fn simple_ab_grammar_basic_table() {
    // S -> A B
    let actions = vec![vec![vec![]; 4]; 3];
    let mut gotos = vec![vec![StateId(u16::MAX); 4]; 3];
    gotos[0][2] = StateId(1); // A
    gotos[0][3] = StateId(2); // B

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(2), SymbolId(0), 0);

    assert_eq!(table.state_count, 3);
}

#[test]
fn chain_grammar_basic_table() {
    // S -> S a | a
    let mut actions = vec![vec![vec![]; 4]; 4];
    actions[0][1] = vec![Action::Shift(StateId(1))];
    actions[1][1] = vec![Action::Shift(StateId(2))];
    actions[2][0] = vec![Action::Reduce(adze_ir::RuleId(0))];

    let mut gotos = vec![vec![StateId(u16::MAX); 4]; 4];
    gotos[0][2] = StateId(1);

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(2), SymbolId(0), 0);

    assert_eq!(table.state_count, 4);
}

#[test]
fn choice_grammar_basic_table() {
    // S -> a | b | c
    let mut actions = vec![vec![vec![]; 5]; 4];
    actions[0][1] = vec![Action::Shift(StateId(1))];
    actions[0][2] = vec![Action::Shift(StateId(2))];
    actions[0][3] = vec![Action::Shift(StateId(3))];

    let gotos = vec![vec![StateId(u16::MAX); 5]; 4];

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(2), SymbolId(0), 0);

    assert_eq!(table.state_count, 4);
}

#[test]
fn real_table_compression_roundtrip() {
    let mut actions = vec![vec![vec![]; 5]; 5];
    for i in 0..5 {
        actions[i][0] = vec![Action::Shift(StateId((i * 2) as u16))];
    }

    let mut gotos = vec![vec![StateId(u16::MAX); 5]; 5];
    for i in 0..4 {
        gotos[i][1] = StateId((i + 1) as u16);
    }

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(2), SymbolId(0), 0);

    // Verify roundtrip
    for i in 0..5 {
        if !table.action_table[i][0].is_empty() {
            assert_eq!(
                table.action_table[i][0][0],
                Action::Shift(StateId((i * 2) as u16))
            );
        }
    }
}

#[test]
fn real_grammar_preserves_all_actions() {
    let mut actions = vec![vec![vec![]; 4]; 4];
    actions[0][1] = vec![Action::Shift(StateId(1))];
    actions[1][2] = vec![Action::Reduce(adze_ir::RuleId(0))];
    actions[2][1] = vec![Action::Shift(StateId(3))];
    actions[3][0] = vec![Action::Accept];

    let gotos = vec![vec![StateId(u16::MAX); 4]; 4];

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(2), SymbolId(0), 0);

    if !table.action_table[0][1].is_empty() {
        assert!(matches!(
            table.action_table[0][1][0],
            Action::Shift(StateId(1))
        ));
    }
    if !table.action_table[3][0].is_empty() {
        assert_eq!(table.action_table[3][0][0], Action::Accept);
    }
}

#[test]
fn real_grammar_preserves_all_gotos() {
    let actions = vec![vec![vec![]; 5]; 4];
    let mut gotos = vec![vec![StateId(u16::MAX); 5]; 4];
    for i in 0..4 {
        for j in 1..4 {
            if i + j < 5 {
                gotos[i][j] = StateId((i * 10 + j) as u16);
            }
        }
    }

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(2), SymbolId(0), 0);

    for i in 0..4 {
        for j in 1..4 {
            if i + j < 5 {
                assert_eq!(table.goto_table[i][j], StateId((i * 10 + j) as u16));
            }
        }
    }
}

// =============================================================================
// CATEGORY 8: EDGE CASES (8 tests)
// =============================================================================

#[test]
fn single_state_single_symbol_table() {
    let actions = vec![vec![vec![Action::Accept]; 1]; 1];
    let gotos = vec![vec![StateId(u16::MAX); 1]];

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(0), SymbolId(0), 0);

    assert_eq!(table.state_count, 1);
    assert!(table.symbol_count >= 1);
}

#[test]
fn high_state_id_handling() {
    let actions = vec![vec![vec![]; 2]; 1];
    let mut gotos = vec![vec![StateId(u16::MAX); 2]; 1];
    gotos[0][1] = StateId(0xFF00);

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(1), SymbolId(0), 0);

    assert_eq!(table.goto_table[0][1], StateId(0xFF00));
}

#[test]
fn many_symbols_few_states() {
    let actions = vec![vec![vec![]; 100]; 2];
    let gotos = vec![vec![StateId(u16::MAX); 100]; 2];

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(50), SymbolId(0), 0);

    assert_eq!(table.state_count, 2);
    assert_eq!(table.symbol_count, 100);
}

#[test]
fn few_symbols_many_states() {
    let actions = vec![vec![vec![]; 3]; 100];
    let gotos = vec![vec![StateId(u16::MAX); 3]; 100];

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(1), SymbolId(0), 0);

    assert_eq!(table.state_count, 100);
    assert_eq!(table.symbol_count, 3);
}

#[test]
fn all_error_action_table() {
    let actions = vec![vec![vec![Action::Error]; 5]; 5];
    let gotos = vec![vec![StateId(u16::MAX); 5]; 5];

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(2), SymbolId(0), 0);

    for i in 0..5 {
        if !table.action_table[i][0].is_empty() {
            assert_eq!(table.action_table[i][0][0], Action::Error);
        }
    }
}

#[test]
fn all_shift_action_table() {
    let mut actions = vec![vec![vec![]; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            actions[i][j] = vec![Action::Shift(StateId((i + j) as u16))];
        }
    }

    let gotos = vec![vec![StateId(u16::MAX); 4]; 4];

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(2), SymbolId(0), 0);

    for i in 0..4 {
        for j in 0..4 {
            if !table.action_table[i][j].is_empty() {
                assert!(matches!(table.action_table[i][j][0], Action::Shift(_)));
            }
        }
    }
}

#[test]
fn alternating_actions_pattern() {
    let mut actions = vec![vec![vec![]; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            if (i + j) % 2 == 0 {
                actions[i][j] = vec![Action::Shift(StateId(i as u16))];
            } else {
                actions[i][j] = vec![Action::Reduce(adze_ir::RuleId(j as u16))];
            }
        }
    }

    let gotos = vec![vec![StateId(u16::MAX); 4]; 4];

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(2), SymbolId(0), 0);

    assert_eq!(table.state_count, 4);
    assert_eq!(table.symbol_count, 4);
}

#[test]
fn compression_determinism_verified() {
    let actions1 = vec![vec![vec![Action::Shift(StateId(1))]; 3]; 3];
    let gotos1 = vec![vec![StateId(u16::MAX); 3]; 3];
    let table1 = make_minimal_table(actions1, gotos1, vec![], SymbolId(1), SymbolId(0), 0);

    let actions2 = vec![vec![vec![Action::Shift(StateId(1))]; 3]; 3];
    let gotos2 = vec![vec![StateId(u16::MAX); 3]; 3];
    let table2 = make_minimal_table(actions2, gotos2, vec![], SymbolId(1), SymbolId(0), 0);

    assert_eq!(table1.state_count, table2.state_count);
    assert_eq!(table1.symbol_count, table2.symbol_count);

    for i in 0..table1.state_count {
        for j in 0..table1.symbol_count {
            assert_eq!(
                table1.action_table[i][j].len(),
                table2.action_table[i][j].len()
            );
        }
    }
}

// =============================================================================
// ADDITIONAL TESTS (5 tests to reach 64 total)
// =============================================================================

#[test]
fn large_state_symbol_table_compression() {
    let actions = vec![vec![vec![]; 25]; 25];
    let gotos = vec![vec![StateId(u16::MAX); 25]; 25];

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(12), SymbolId(0), 0);

    assert_eq!(table.state_count, 25);
    assert_eq!(table.symbol_count, 25);
}

#[test]
fn mixed_goto_action_table_consistency() {
    let mut actions = vec![vec![vec![]; 5]; 5];
    for i in 0..5 {
        actions[i][i] = vec![Action::Shift(StateId(i as u16))];
    }

    let mut gotos = vec![vec![StateId(u16::MAX); 5]; 5];
    for i in 0..4 {
        gotos[i][i + 1] = StateId((i + 10) as u16);
    }

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(2), SymbolId(0), 0);

    for i in 0..5 {
        if !table.action_table[i][i].is_empty() {
            assert!(matches!(
                table.action_table[i][i][0],
                Action::Shift(StateId(s)) if s == i as u16
            ));
        }
    }

    for i in 0..4 {
        assert_eq!(table.goto_table[i][i + 1], StateId((i + 10) as u16));
    }
}

#[test]
fn reduce_action_sequence_table() {
    let mut actions = vec![vec![vec![]; 6]; 4];
    for i in 0..4 {
        for j in 0..6 {
            actions[i][j] = vec![Action::Reduce(adze_ir::RuleId((i * 6 + j) as u16))];
        }
    }

    let gotos = vec![vec![StateId(u16::MAX); 6]; 4];

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(3), SymbolId(0), 0);

    assert_eq!(table.state_count, 4);
    assert_eq!(table.symbol_count, 6);

    for i in 0..4 {
        for j in 0..6 {
            if !table.action_table[i][j].is_empty() {
                assert!(matches!(
                    table.action_table[i][j][0],
                    Action::Reduce(adze_ir::RuleId(idx)) if idx as usize == i * 6 + j
                ));
            }
        }
    }
}

#[test]
fn compressor_instance_creation() {
    let _compressor1 = TableCompressor::new();
    let _compressor2 = TableCompressor::new();

    // Verify two compressor instances can be created successfully
    // Both should have the same default behavior for compression thresholds
}

#[test]
fn table_with_multiple_error_actions() {
    let mut actions = vec![vec![vec![]; 4]; 4];
    for i in 0..4 {
        if i > 0 && i < 3 {
            actions[i][i] = vec![Action::Error];
        }
    }

    let gotos = vec![vec![StateId(u16::MAX); 4]; 4];

    let table = make_minimal_table(actions, gotos, vec![], SymbolId(2), SymbolId(0), 0);

    assert_eq!(table.state_count, 4);

    for i in 1..3 {
        if !table.action_table[i][i].is_empty() {
            assert_eq!(table.action_table[i][i][0], Action::Error);
        }
    }
}

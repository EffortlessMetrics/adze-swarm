//! Comprehensive tests for the conflict inspection module.
//!
//! Covers all public types and functions:
//! - ConflictSummary, ConflictDetail, ConflictType
//! - count_conflicts, classify_conflict, state_has_conflicts
//! - get_state_conflicts, find_conflicts_for_symbol
//! - Display implementations
//! - Edge cases: no conflicts, all conflicts, out-of-bounds states

use adze_glr_core::conflict_inspection::{
    ConflictDetail, ConflictSummary, ConflictType, action_branch_count, cell_has_conflict,
    classify_conflict, count_conflicts, find_conflicts_for_symbol, get_state_conflicts,
    state_has_conflicts,
};
use adze_glr_core::{Action, GotoIndexing, ParseTable, StateId};
use adze_ir::{Grammar, RuleId, SymbolId};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a minimal ParseTable from an action table.
fn make_table(action_table: Vec<Vec<Vec<Action>>>) -> ParseTable {
    make_table_with_symbols(action_table, vec![], BTreeMap::new())
}

/// Build a ParseTable with explicit index_to_symbol mapping.
fn make_table_with_symbols(
    action_table: Vec<Vec<Vec<Action>>>,
    index_to_symbol: Vec<SymbolId>,
    symbol_to_index: BTreeMap<SymbolId, usize>,
) -> ParseTable {
    let state_count = action_table.len();
    ParseTable {
        action_table,
        goto_table: vec![],
        symbol_metadata: vec![],
        state_count,
        symbol_count: 1,
        symbol_to_index,
        index_to_symbol,
        external_scanner_states: vec![],
        rules: vec![],
        nonterminal_to_index: Default::default(),
        goto_indexing: GotoIndexing::NonterminalMap,
        eof_symbol: SymbolId(0),
        start_symbol: SymbolId(0),
        grammar: Grammar::new("test".to_string()),
        initial_state: StateId(0),
        token_count: 0,
        external_token_count: 0,
        lex_modes: vec![],
        extras: vec![],
        dynamic_prec_by_rule: vec![],
        rule_assoc_by_rule: vec![],
        alias_sequences: vec![],
        field_names: vec![],
        field_map: BTreeMap::new(),
    }
}

// ===========================================================================
// 1. classify_conflict – pure action-list classification
// ===========================================================================

#[test]
fn classify_shift_reduce() {
    let actions = vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(2))];
    assert_eq!(classify_conflict(&actions), ConflictType::ShiftReduce);
}

#[test]
fn classify_reduce_reduce() {
    let actions = vec![Action::Reduce(RuleId(1)), Action::Reduce(RuleId(2))];
    assert_eq!(classify_conflict(&actions), ConflictType::ReduceReduce);
}

#[test]
fn classify_mixed_two_shifts() {
    let actions = vec![Action::Shift(StateId(1)), Action::Shift(StateId(2))];
    assert_eq!(classify_conflict(&actions), ConflictType::Mixed);
}

#[test]
fn classify_single_shift_is_mixed() {
    // Only one shift, no reduce → Mixed (neither SR nor RR)
    let actions = vec![Action::Shift(StateId(0))];
    assert_eq!(classify_conflict(&actions), ConflictType::Mixed);
}

#[test]
fn classify_single_reduce_is_mixed() {
    // A single reduce is not a reduce/reduce conflict.
    let actions = vec![Action::Reduce(RuleId(0))];
    assert_eq!(classify_conflict(&actions), ConflictType::Mixed);
}

#[test]
fn classify_empty_actions_is_mixed() {
    assert_eq!(classify_conflict(&[]), ConflictType::Mixed);
}

#[test]
fn classify_accept_only_is_mixed() {
    let actions = vec![Action::Accept];
    assert_eq!(classify_conflict(&actions), ConflictType::Mixed);
}

#[test]
fn classify_error_only_is_mixed() {
    let actions = vec![Action::Error];
    assert_eq!(classify_conflict(&actions), ConflictType::Mixed);
}

#[test]
fn classify_recover_only_is_mixed() {
    let actions = vec![Action::Recover];
    assert_eq!(classify_conflict(&actions), ConflictType::Mixed);
}

#[test]
fn classify_shift_accept_is_mixed() {
    let actions = vec![Action::Shift(StateId(1)), Action::Accept];
    assert_eq!(classify_conflict(&actions), ConflictType::Mixed);
}

#[test]
fn classify_reduce_accept_is_mixed() {
    // Reduce + accept is neither shift/reduce nor reduce/reduce.
    let actions = vec![Action::Reduce(RuleId(0)), Action::Accept];
    assert_eq!(classify_conflict(&actions), ConflictType::Mixed);
}

// ---------------------------------------------------------------------------
// Fork classification (recursive)
// ---------------------------------------------------------------------------

#[test]
fn classify_fork_shift_reduce() {
    let actions = vec![Action::Fork(vec![
        Action::Shift(StateId(3)),
        Action::Reduce(RuleId(5)),
    ])];
    assert_eq!(classify_conflict(&actions), ConflictType::ShiftReduce);
}

#[test]
fn classify_fork_reduce_reduce() {
    let actions = vec![Action::Fork(vec![
        Action::Reduce(RuleId(1)),
        Action::Reduce(RuleId(2)),
    ])];
    assert_eq!(classify_conflict(&actions), ConflictType::ReduceReduce);
}

#[test]
fn classify_fork_with_two_shifts() {
    // Fork([Shift, Shift]) flattens to effective shift actions only.
    let actions = vec![Action::Fork(vec![
        Action::Shift(StateId(1)),
        Action::Shift(StateId(2)),
    ])];
    assert_eq!(classify_conflict(&actions), ConflictType::Mixed);
}

#[test]
fn classify_fork_plus_direct_action() {
    // Fork with reduces + a direct shift → ShiftReduce overall
    let actions = vec![
        Action::Fork(vec![Action::Reduce(RuleId(1)), Action::Reduce(RuleId(2))]),
        Action::Shift(StateId(5)),
    ];
    assert_eq!(classify_conflict(&actions), ConflictType::ShiftReduce);
}

#[test]
fn classify_nested_fork() {
    // Fork inside fork: inner is ShiftReduce → sets both flags
    let actions = vec![Action::Fork(vec![Action::Fork(vec![
        Action::Shift(StateId(0)),
        Action::Reduce(RuleId(0)),
    ])])];
    assert_eq!(classify_conflict(&actions), ConflictType::ShiftReduce);
}

// ===========================================================================
// 2. count_conflicts – full table scanning
// ===========================================================================

#[test]
fn count_conflicts_no_conflicts() {
    let table = make_table(vec![
        vec![vec![Action::Shift(StateId(1))]],
        vec![vec![Action::Accept]],
    ]);
    let summary = count_conflicts(&table);
    assert_eq!(summary.shift_reduce, 0);
    assert_eq!(summary.reduce_reduce, 0);
    assert!(summary.states_with_conflicts.is_empty());
    assert!(summary.conflict_details.is_empty());
}

#[test]
fn count_conflicts_one_shift_reduce() {
    let table = make_table(vec![vec![vec![
        Action::Shift(StateId(1)),
        Action::Reduce(RuleId(0)),
    ]]]);
    let summary = count_conflicts(&table);
    assert_eq!(summary.shift_reduce, 1);
    assert_eq!(summary.reduce_reduce, 0);
    assert_eq!(summary.states_with_conflicts, vec![StateId(0)]);
    assert_eq!(summary.conflict_details.len(), 1);
}

#[test]
fn count_conflicts_one_reduce_reduce() {
    let table = make_table(vec![vec![vec![
        Action::Reduce(RuleId(0)),
        Action::Reduce(RuleId(1)),
    ]]]);
    let summary = count_conflicts(&table);
    assert_eq!(summary.shift_reduce, 0);
    assert_eq!(summary.reduce_reduce, 1);
    assert_eq!(summary.states_with_conflicts.len(), 1);
}

#[test]
fn count_conflicts_mixed_counts_both() {
    // Two shifts → Mixed → counts as both SR and RR
    let table = make_table(vec![vec![vec![
        Action::Shift(StateId(0)),
        Action::Shift(StateId(1)),
    ]]]);
    let summary = count_conflicts(&table);
    assert_eq!(summary.shift_reduce, 1);
    assert_eq!(summary.reduce_reduce, 1);
}

#[test]
fn count_conflicts_empty_cells_ignored() {
    // Empty action cells are not conflicts
    let table = make_table(vec![vec![vec![]]]);
    let summary = count_conflicts(&table);
    assert_eq!(summary.shift_reduce, 0);
    assert_eq!(summary.reduce_reduce, 0);
    assert!(summary.states_with_conflicts.is_empty());
}

#[test]
fn count_conflicts_multiple_states() {
    let table = make_table(vec![
        // State 0: shift/reduce conflict
        vec![vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(0))]],
        // State 1: no conflict
        vec![vec![Action::Accept]],
        // State 2: reduce/reduce conflict
        vec![vec![Action::Reduce(RuleId(1)), Action::Reduce(RuleId(2))]],
    ]);
    let summary = count_conflicts(&table);
    assert_eq!(summary.shift_reduce, 1);
    assert_eq!(summary.reduce_reduce, 1);
    assert_eq!(summary.states_with_conflicts.len(), 2);
    assert!(summary.states_with_conflicts.contains(&StateId(0)));
    assert!(summary.states_with_conflicts.contains(&StateId(2)));
    assert_eq!(summary.conflict_details.len(), 2);
}

#[test]
fn count_conflicts_multiple_symbols_one_state() {
    // State 0 has conflicts on two different symbol columns
    let table = make_table(vec![vec![
        vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(0))],
        vec![Action::Reduce(RuleId(1)), Action::Reduce(RuleId(2))],
    ]]);
    let summary = count_conflicts(&table);
    assert_eq!(summary.shift_reduce, 1);
    assert_eq!(summary.reduce_reduce, 1);
    // Only one state has conflicts, even though two symbols conflict
    assert_eq!(summary.states_with_conflicts.len(), 1);
    assert_eq!(summary.conflict_details.len(), 2);
}

#[test]
fn nested_fork_conflict_cells_are_detected() {
    let nested_conflict = Action::Fork(vec![Action::Fork(vec![
        Action::Shift(StateId(7)),
        Action::Reduce(RuleId(4)),
    ])]);
    let clean_nested_fork = Action::Fork(vec![Action::Fork(vec![Action::Shift(StateId(8))])]);

    assert_eq!(action_branch_count(&nested_conflict), 2);
    assert_eq!(action_branch_count(&clean_nested_fork), 1);
    assert!(cell_has_conflict(std::slice::from_ref(&nested_conflict)));
    assert!(!cell_has_conflict(&[clean_nested_fork]));

    let table = make_table(vec![vec![vec![nested_conflict]]]);
    let summary = count_conflicts(&table);

    assert_eq!(summary.shift_reduce, 1);
    assert_eq!(summary.reduce_reduce, 0);
    assert_eq!(summary.states_with_conflicts, vec![StateId(0)]);
    assert_eq!(summary.conflict_details.len(), 1);
    assert_eq!(
        summary.conflict_details[0].conflict_type,
        ConflictType::ShiftReduce
    );
    assert!(state_has_conflicts(&table, StateId(0)));
}

// ===========================================================================
// 3. state_has_conflicts
// ===========================================================================

#[test]
fn state_has_conflicts_true_for_conflict_state() {
    let table = make_table(vec![vec![vec![
        Action::Shift(StateId(1)),
        Action::Reduce(RuleId(0)),
    ]]]);
    assert!(state_has_conflicts(&table, StateId(0)));
}

#[test]
fn state_has_conflicts_false_for_clean_state() {
    let table = make_table(vec![vec![vec![Action::Shift(StateId(1))]]]);
    assert!(!state_has_conflicts(&table, StateId(0)));
}

#[test]
fn state_has_conflicts_out_of_bounds_returns_false() {
    let table = make_table(vec![vec![vec![Action::Shift(StateId(1))]]]);
    assert!(!state_has_conflicts(&table, StateId(99)));
}

#[test]
fn state_has_conflicts_empty_cell_not_conflict() {
    let table = make_table(vec![vec![vec![]]]);
    assert!(!state_has_conflicts(&table, StateId(0)));
}

// ===========================================================================
// 4. get_state_conflicts
// ===========================================================================

#[test]
fn get_state_conflicts_returns_matching_details() {
    let table = make_table(vec![
        vec![vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(0))]],
        vec![vec![Action::Reduce(RuleId(1)), Action::Reduce(RuleId(2))]],
    ]);
    let conflicts = get_state_conflicts(&table, StateId(0));
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].state, StateId(0));
    assert_eq!(conflicts[0].conflict_type, ConflictType::ShiftReduce);
}

#[test]
fn get_state_conflicts_empty_for_clean_state() {
    let table = make_table(vec![
        vec![vec![Action::Shift(StateId(1))]],
        vec![vec![Action::Reduce(RuleId(0)), Action::Reduce(RuleId(1))]],
    ]);
    let conflicts = get_state_conflicts(&table, StateId(0));
    assert!(conflicts.is_empty());
}

#[test]
fn get_state_conflicts_out_of_bounds() {
    let table = make_table(vec![vec![vec![Action::Accept]]]);
    let conflicts = get_state_conflicts(&table, StateId(100));
    assert!(conflicts.is_empty());
}

// ===========================================================================
// 5. find_conflicts_for_symbol
// ===========================================================================

#[test]
fn find_conflicts_for_symbol_matches() {
    let mut sym_map = BTreeMap::new();
    sym_map.insert(SymbolId(10), 0usize);
    sym_map.insert(SymbolId(20), 1usize);
    let idx_to_sym = vec![SymbolId(10), SymbolId(20)];

    let table = make_table_with_symbols(
        vec![vec![
            // symbol index 0 → SymbolId(10): conflict
            vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(0))],
            // symbol index 1 → SymbolId(20): no conflict
            vec![Action::Accept],
        ]],
        idx_to_sym,
        sym_map,
    );
    let conflicts = find_conflicts_for_symbol(&table, SymbolId(10));
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].symbol, SymbolId(10));
}

#[test]
fn find_conflicts_for_symbol_no_match() {
    let table = make_table(vec![vec![vec![
        Action::Shift(StateId(1)),
        Action::Reduce(RuleId(0)),
    ]]]);
    // SymbolId(99) doesn't appear in the table
    let conflicts = find_conflicts_for_symbol(&table, SymbolId(99));
    assert!(conflicts.is_empty());
}

// ===========================================================================
// 6. ConflictDetail fields
// ===========================================================================

#[test]
fn conflict_detail_records_actions() {
    let table = make_table(vec![vec![vec![
        Action::Shift(StateId(5)),
        Action::Reduce(RuleId(3)),
        Action::Reduce(RuleId(7)),
    ]]]);
    let summary = count_conflicts(&table);
    assert_eq!(summary.conflict_details.len(), 1);
    let detail = &summary.conflict_details[0];
    assert_eq!(detail.actions.len(), 3);
    assert_eq!(detail.state, StateId(0));
}

#[test]
fn conflict_detail_priorities_default_to_zero() {
    let table = make_table(vec![vec![vec![
        Action::Shift(StateId(0)),
        Action::Reduce(RuleId(0)),
    ]]]);
    let summary = count_conflicts(&table);
    let detail = &summary.conflict_details[0];
    assert!(detail.priorities.iter().all(|&p| p == 0));
    assert_eq!(detail.priorities.len(), detail.actions.len());
}

#[test]
fn conflict_detail_symbol_name_format() {
    let table = make_table(vec![vec![vec![
        Action::Shift(StateId(0)),
        Action::Reduce(RuleId(0)),
    ]]]);
    let summary = count_conflicts(&table);
    let detail = &summary.conflict_details[0];
    // Symbol name should be "symbol_N" format
    assert!(detail.symbol_name.starts_with("symbol_"));
}

// ===========================================================================
// 7. ConflictSummary – clone, eq, debug
// ===========================================================================

#[test]
fn conflict_summary_clone_and_eq() {
    let table = make_table(vec![vec![vec![
        Action::Shift(StateId(1)),
        Action::Reduce(RuleId(0)),
    ]]]);
    let s1 = count_conflicts(&table);
    let s2 = s1.clone();
    assert_eq!(s1, s2);
}

#[test]
fn conflict_summary_debug_not_empty() {
    let summary = ConflictSummary {
        shift_reduce: 2,
        reduce_reduce: 1,
        states_with_conflicts: vec![StateId(0), StateId(3)],
        conflict_details: vec![],
    };
    let debug = format!("{:?}", summary);
    assert!(debug.contains("shift_reduce"));
    assert!(debug.contains("reduce_reduce"));
}

// ===========================================================================
// 8. ConflictType – clone, eq, copy, debug
// ===========================================================================

#[test]
fn conflict_type_copy_and_eq() {
    let a = ConflictType::ShiftReduce;
    let b = a; // Copy
    assert_eq!(a, b);
}

#[test]
fn conflict_type_all_variants_distinct() {
    assert_ne!(ConflictType::ShiftReduce, ConflictType::ReduceReduce);
    assert_ne!(ConflictType::ShiftReduce, ConflictType::Mixed);
    assert_ne!(ConflictType::ReduceReduce, ConflictType::Mixed);
}

#[test]
fn conflict_type_debug() {
    assert_eq!(format!("{:?}", ConflictType::ShiftReduce), "ShiftReduce");
    assert_eq!(format!("{:?}", ConflictType::ReduceReduce), "ReduceReduce");
    assert_eq!(format!("{:?}", ConflictType::Mixed), "Mixed");
}

// ===========================================================================
// 9. Display implementations
// ===========================================================================

#[test]
fn conflict_summary_display_includes_counts() {
    let table = make_table(vec![vec![vec![
        Action::Shift(StateId(1)),
        Action::Reduce(RuleId(0)),
    ]]]);
    let summary = count_conflicts(&table);
    let display = format!("{}", summary);
    assert!(display.contains("Shift/Reduce conflicts: 1"));
    assert!(display.contains("Reduce/Reduce conflicts: 0"));
    assert!(display.contains("States with conflicts: 1"));
}

#[test]
fn conflict_detail_display_format() {
    let detail = ConflictDetail {
        state: StateId(3),
        symbol: SymbolId(7),
        symbol_name: "plus".to_string(),
        conflict_type: ConflictType::ShiftReduce,
        actions: vec![Action::Shift(StateId(4)), Action::Reduce(RuleId(2))],
        priorities: vec![0, 0],
    };
    let display = format!("{}", detail);
    assert!(display.contains("State 3"));
    assert!(display.contains("plus"));
    assert!(display.contains("7"));
    assert!(display.contains("ShiftReduce"));
    assert!(display.contains("2 actions"));
}

// ===========================================================================
// 10. Edge cases
// ===========================================================================

#[test]
fn single_state_single_symbol_no_conflict() {
    let table = make_table(vec![vec![vec![Action::Accept]]]);
    let summary = count_conflicts(&table);
    assert_eq!(summary.shift_reduce, 0);
    assert_eq!(summary.reduce_reduce, 0);
    assert!(summary.states_with_conflicts.is_empty());
}

#[test]
fn all_states_have_conflicts() {
    let table = make_table(vec![
        vec![vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(0))]],
        vec![vec![Action::Reduce(RuleId(1)), Action::Reduce(RuleId(2))]],
        vec![vec![Action::Shift(StateId(0)), Action::Shift(StateId(1))]],
    ]);
    let summary = count_conflicts(&table);
    assert_eq!(summary.states_with_conflicts.len(), 3);
    // SR from state 0, RR from state 1, Mixed from state 2 (counts as both)
    assert_eq!(summary.shift_reduce, 2); // state 0 + state 2
    assert_eq!(summary.reduce_reduce, 2); // state 1 + state 2
}

#[test]
fn three_way_conflict() {
    let table = make_table(vec![vec![vec![
        Action::Shift(StateId(1)),
        Action::Reduce(RuleId(0)),
        Action::Reduce(RuleId(1)),
    ]]]);
    let summary = count_conflicts(&table);
    // Has both shift and reduce → ShiftReduce
    assert_eq!(summary.shift_reduce, 1);
    assert_eq!(summary.reduce_reduce, 0);
    assert_eq!(summary.conflict_details[0].actions.len(), 3);
}

#[test]
fn many_reduces_is_reduce_reduce() {
    let table = make_table(vec![vec![vec![
        Action::Reduce(RuleId(0)),
        Action::Reduce(RuleId(1)),
        Action::Reduce(RuleId(2)),
        Action::Reduce(RuleId(3)),
    ]]]);
    let summary = count_conflicts(&table);
    assert_eq!(summary.shift_reduce, 0);
    assert_eq!(summary.reduce_reduce, 1);
    assert_eq!(
        summary.conflict_details[0].conflict_type,
        ConflictType::ReduceReduce
    );
}

#[test]
fn find_conflicts_for_symbol_across_multiple_states() {
    let idx_to_sym = vec![SymbolId(5)];
    let mut sym_map = BTreeMap::new();
    sym_map.insert(SymbolId(5), 0usize);

    let table = make_table_with_symbols(
        vec![
            vec![vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(0))]],
            vec![vec![Action::Reduce(RuleId(1)), Action::Reduce(RuleId(2))]],
        ],
        idx_to_sym,
        sym_map,
    );
    let conflicts = find_conflicts_for_symbol(&table, SymbolId(5));
    assert_eq!(conflicts.len(), 2);
    assert!(conflicts.iter().all(|c| c.symbol == SymbolId(5)));
}

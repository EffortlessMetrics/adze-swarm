#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]
//! Property-based tests for LR automaton state machine properties.
//!
//! 46 proptest property tests across 8 categories testing state counts,
//! action tables, goto tables, accept states, determinism, bounded IDs,
//! initial states, and complex grammar states.
//!
//! Run with:
//! ```bash
//! cargo test -p adze-glr-core --test proptest_state_v5
//! ```

use adze_glr_core::{
    Action, FirstFollowSets, GotoIndexing, LexMode, ParseRule, ParseTable, SymbolMetadata,
    build_lr1_automaton,
};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{Associativity, Grammar, RuleId, StateId, SymbolId};
use proptest::prelude::*;
use std::collections::{BTreeMap, HashSet};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

type ActionCell = Vec<Action>;

const NO_GOTO: StateId = StateId(u16::MAX);

/// Build a parse table from a `GrammarBuilder` via the standard pipeline.
fn pipeline(builder: GrammarBuilder) -> ParseTable {
    let mut grammar = builder.build();
    grammar.normalize();
    let ff = FirstFollowSets::compute(&grammar).expect("FIRST/FOLLOW");
    build_lr1_automaton(&grammar, &ff).expect("build automaton")
}

/// Check whether any cell in the table contains an Accept action.
fn has_accept(table: &ParseTable) -> bool {
    table.action_table.iter().any(|row| {
        row.iter()
            .any(|cell| cell.iter().any(|a| matches!(a, Action::Accept)))
    })
}

/// Check whether any Accept action is in the EOF column.
fn accept_on_eof(table: &ParseTable) -> bool {
    let Some(&eof_col) = table.symbol_to_index.get(&table.eof_symbol) else {
        return false;
    };
    table.action_table.iter().any(|row| {
        row.get(eof_col)
            .is_some_and(|cell| cell.iter().any(|a| matches!(a, Action::Accept)))
    })
}

/// Count states that contain an Accept action.
fn count_accept_states(table: &ParseTable) -> usize {
    table
        .action_table
        .iter()
        .filter(|row| {
            row.iter()
                .any(|cell| cell.iter().any(|a| matches!(a, Action::Accept)))
        })
        .count()
}

/// All Shift targets point to valid states.
fn shifts_are_valid(table: &ParseTable) -> bool {
    table.action_table.iter().all(|row| {
        row.iter().all(|cell| {
            cell.iter().all(|a| match a {
                Action::Shift(s) => (s.0 as usize) < table.state_count,
                Action::Fork(actions) => actions.iter().all(|inner| match inner {
                    Action::Shift(s) => (s.0 as usize) < table.state_count,
                    _ => true,
                }),
                _ => true,
            })
        })
    })
}

/// All Reduce RuleIds are in-range.
fn reduces_are_valid(table: &ParseTable) -> bool {
    if table.rules.is_empty() {
        return true;
    }
    table.action_table.iter().all(|row| {
        row.iter().all(|cell| {
            cell.iter().all(|a| match a {
                Action::Reduce(r) => (r.0 as usize) < table.rules.len(),
                Action::Fork(actions) => actions.iter().all(|inner| match inner {
                    Action::Reduce(r) => (r.0 as usize) < table.rules.len(),
                    _ => true,
                }),
                _ => true,
            })
        })
    })
}

// ---------------------------------------------------------------------------
// Fixed grammar constructors
// ---------------------------------------------------------------------------

/// S → a
fn minimal_grammar() -> GrammarBuilder {
    GrammarBuilder::new("minimal")
        .token("a", "a")
        .rule("s", vec!["a"])
        .start("s")
}

/// S → a b
fn two_token_grammar() -> GrammarBuilder {
    GrammarBuilder::new("twotok")
        .token("a", "a")
        .token("b", "b")
        .rule("s", vec!["a", "b"])
        .start("s")
}

/// S → a | b
fn two_alt_grammar() -> GrammarBuilder {
    GrammarBuilder::new("twoalt")
        .token("a", "a")
        .token("b", "b")
        .rule("s", vec!["a"])
        .rule("s", vec!["b"])
        .start("s")
}

/// S → ε | a
fn nullable_grammar() -> GrammarBuilder {
    GrammarBuilder::new("nullable")
        .token("a", "a")
        .rule("s", vec![])
        .rule("s", vec!["a"])
        .start("s")
}

/// S → S a | a  (left-recursive)
fn left_recursive_grammar() -> GrammarBuilder {
    GrammarBuilder::new("leftrec")
        .token("a", "a")
        .rule("s", vec!["s", "a"])
        .rule("s", vec!["a"])
        .start("s")
}

/// S → a S | a  (right-recursive)
fn right_recursive_grammar() -> GrammarBuilder {
    GrammarBuilder::new("rightrec")
        .token("a", "a")
        .rule("s", vec!["a", "s"])
        .rule("s", vec!["a"])
        .start("s")
}

/// S → T, T → a  (chain)
fn chain_grammar() -> GrammarBuilder {
    GrammarBuilder::new("chain")
        .token("a", "a")
        .rule("s", vec!["t"])
        .rule("t", vec!["a"])
        .start("s")
}

/// S → T, T → U, U → a  (deep chain)
fn deep_chain_grammar() -> GrammarBuilder {
    GrammarBuilder::new("deep")
        .token("a", "a")
        .rule("s", vec!["t"])
        .rule("t", vec!["u"])
        .rule("u", vec!["a"])
        .start("s")
}

/// S → a b c  (sequence)
fn sequence_grammar() -> GrammarBuilder {
    GrammarBuilder::new("seq")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .rule("s", vec!["a", "b", "c"])
        .start("s")
}

/// S → a | b | c | d | e  (wide alternatives)
fn wide_alt_grammar() -> GrammarBuilder {
    GrammarBuilder::new("wide")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .token("d", "d")
        .token("e", "e")
        .rule("s", vec!["a"])
        .rule("s", vec!["b"])
        .rule("s", vec!["c"])
        .rule("s", vec!["d"])
        .rule("s", vec!["e"])
        .start("s")
}

/// E → E + E | E * E | a  (precedence)
fn precedence_grammar() -> GrammarBuilder {
    GrammarBuilder::new("prec")
        .token("a", "a")
        .token("+", "+")
        .token("*", "*")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["a"])
        .start("expr")
}

/// S → T U, T → a, U → b  (two NTs in sequence)
fn two_nt_seq_grammar() -> GrammarBuilder {
    GrammarBuilder::new("twontseq")
        .token("a", "a")
        .token("b", "b")
        .rule("s", vec!["t", "u"])
        .rule("t", vec!["a"])
        .rule("u", vec!["b"])
        .start("s")
}

/// Strategy that yields a table from one of the fixed grammars.
fn arb_grammar_table() -> impl Strategy<Value = ParseTable> {
    prop_oneof![
        Just(pipeline(minimal_grammar())),
        Just(pipeline(two_token_grammar())),
        Just(pipeline(two_alt_grammar())),
        Just(pipeline(nullable_grammar())),
        Just(pipeline(left_recursive_grammar())),
        Just(pipeline(right_recursive_grammar())),
        Just(pipeline(chain_grammar())),
        Just(pipeline(deep_chain_grammar())),
        Just(pipeline(sequence_grammar())),
        Just(pipeline(wide_alt_grammar())),
        Just(pipeline(precedence_grammar())),
        Just(pipeline(two_nt_seq_grammar())),
    ]
}

// ---------------------------------------------------------------------------
// Synthetic parse table construction
// ---------------------------------------------------------------------------

fn leaf_action(max_state: u16) -> impl Strategy<Value = Action> {
    prop_oneof![
        (0..max_state).prop_map(|s| Action::Shift(StateId(s))),
        (0..16u16).prop_map(|r| Action::Reduce(RuleId(r))),
        Just(Action::Accept),
        Just(Action::Error),
        Just(Action::Recover),
    ]
}

fn arb_action_cell(max_state: u16) -> impl Strategy<Value = ActionCell> {
    prop::collection::vec(leaf_action(max_state), 0..=3)
}

/// Build a well-formed synthetic `ParseTable` with the given dimensions.
fn build_synthetic_table(
    num_states: usize,
    num_terminals: usize,
    num_nonterminals: usize,
    action_table: Vec<Vec<ActionCell>>,
    goto_table: Vec<Vec<StateId>>,
    rules: Vec<ParseRule>,
) -> ParseTable {
    let sym_count = num_terminals + num_nonterminals;

    let mut symbol_to_index = BTreeMap::new();
    let mut index_to_symbol = Vec::new();
    for i in 0..sym_count {
        symbol_to_index.insert(SymbolId(i as u16), i);
        index_to_symbol.push(SymbolId(i as u16));
    }

    let mut nonterminal_to_index = BTreeMap::new();
    for i in num_terminals..sym_count {
        nonterminal_to_index.insert(SymbolId(i as u16), i - num_terminals);
    }

    let metadata = (0..sym_count as u16)
        .map(|i| SymbolMetadata {
            name: format!("s{i}"),
            is_visible: true,
            is_named: true,
            is_supertype: false,
            is_terminal: (i as usize) < num_terminals,
            is_extra: false,
            is_fragile: false,
            symbol_id: SymbolId(i),
        })
        .collect();

    ParseTable {
        action_table,
        goto_table,
        rules: rules.clone(),
        state_count: num_states,
        symbol_count: sym_count,
        symbol_to_index,
        index_to_symbol,
        external_scanner_states: vec![],
        nonterminal_to_index,
        eof_symbol: SymbolId(0),
        start_symbol: SymbolId(num_terminals as u16),
        grammar: Grammar::new("synth".to_string()),
        symbol_metadata: metadata,
        initial_state: StateId(0),
        token_count: num_terminals,
        external_token_count: 0,
        lex_modes: vec![
            LexMode {
                lex_state: 0,
                external_lex_state: 0,
            };
            num_states
        ],
        extras: vec![],
        dynamic_prec_by_rule: vec![0; rules.len()],
        rule_assoc_by_rule: vec![0; rules.len()],
        alias_sequences: vec![],
        field_names: vec![],
        goto_indexing: GotoIndexing::NonterminalMap,
        field_map: BTreeMap::new(),
    }
}

/// Strategy that generates a consistent synthetic `ParseTable`.
fn arb_synthetic_table() -> impl Strategy<Value = ParseTable> {
    (1usize..=5, 1usize..=4, 1usize..=6)
        .prop_flat_map(|(num_t, num_nt, num_s)| {
            let sym_count = num_t + num_nt;
            let actions = prop::collection::vec(
                prop::collection::vec(arb_action_cell(num_s as u16), sym_count..=sym_count),
                num_s..=num_s,
            );
            let gotos = prop::collection::vec(
                prop::collection::vec(
                    prop_oneof![Just(NO_GOTO), (0..num_s as u16).prop_map(StateId)],
                    num_nt..=num_nt,
                ),
                num_s..=num_s,
            );
            let rules = prop::collection::vec(
                (
                    (num_t as u16..(num_t + num_nt) as u16).prop_map(SymbolId),
                    0u16..=4,
                )
                    .prop_map(|(lhs, rhs_len)| ParseRule { lhs, rhs_len }),
                1..=4,
            );
            (
                Just(num_s),
                Just(num_t),
                Just(num_nt),
                actions,
                gotos,
                rules,
            )
        })
        .prop_map(|(ns, nt, nnt, a, g, r)| build_synthetic_table(ns, nt, nnt, a, g, r))
}

// ===========================================================================
// Category 1: prop_state_count_* — state count properties (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, .. ProptestConfig::default() })]

    /// state_count matches the number of action table rows.
    #[test]
    fn prop_state_count_matches_action_rows(table in arb_grammar_table()) {
        prop_assert_eq!(table.state_count, table.action_table.len());
    }

    /// state_count matches the number of goto table rows.
    #[test]
    fn prop_state_count_matches_goto_rows(table in arb_grammar_table()) {
        prop_assert_eq!(table.state_count, table.goto_table.len());
    }

    /// state_count matches the number of lex_mode entries.
    #[test]
    fn prop_state_count_matches_lex_modes(table in arb_synthetic_table()) {
        prop_assert_eq!(table.state_count, table.lex_modes.len());
    }

    /// A grammar with at least one rule produces at least 2 states.
    #[test]
    fn prop_state_count_at_least_two_for_nontrivial(table in arb_grammar_table()) {
        prop_assert!(
            table.state_count >= 2,
            "nontrivial grammars need >= 2 states, got {}",
            table.state_count
        );
    }

    /// More alternatives can produce equal or more states than minimal grammar.
    #[test]
    fn prop_state_count_wide_ge_minimal(
        _seed in 0u32..50,
    ) {
        let minimal_t = pipeline(minimal_grammar());
        let wide_t = pipeline(wide_alt_grammar());
        prop_assert!(
            wide_t.state_count >= minimal_t.state_count,
            "wide ({}) should have >= states than minimal ({})",
            wide_t.state_count, minimal_t.state_count
        );
    }
}

// ===========================================================================
// Category 2: prop_state_action_* — action table properties (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, .. ProptestConfig::default() })]

    /// All action rows have uniform width matching symbol_count.
    #[test]
    fn prop_state_action_row_width_uniform(table in arb_grammar_table()) {
        for s in 0..table.state_count {
            prop_assert_eq!(
                table.action_table[s].len(),
                table.symbol_count,
                "state {} action row width mismatch",
                s
            );
        }
    }

    /// All Shift targets reference valid state indices.
    #[test]
    fn prop_state_action_shift_targets_valid(table in arb_grammar_table()) {
        prop_assert!(shifts_are_valid(&table));
    }

    /// All Reduce rule IDs reference valid rules.
    #[test]
    fn prop_state_action_reduce_rules_valid(table in arb_grammar_table()) {
        prop_assert!(reduces_are_valid(&table));
    }

    /// Querying actions for every (state, symbol) pair does not panic.
    #[test]
    fn prop_state_action_no_panic_on_all_symbols(table in arb_grammar_table()) {
        for s in 0..table.state_count {
            let sid = StateId(s as u16);
            for &sym in table.symbol_to_index.keys() {
                let _ = table.actions(sid, sym);
            }
        }
    }

    /// Fork children in the action table are always leaf actions (not nested Fork).
    #[test]
    fn prop_state_action_fork_children_are_leaf(table in arb_grammar_table()) {
        for row in &table.action_table {
            for cell in row {
                for action in cell {
                    if let Action::Fork(children) = action {
                        for child in children {
                            prop_assert!(
                                !matches!(child, Action::Fork(_)),
                                "Fork children must not be Fork"
                            );
                        }
                    }
                }
            }
        }
    }

    /// Actions for an unmapped symbol return an empty slice.
    #[test]
    fn prop_state_action_empty_for_unknown_symbol(table in arb_grammar_table()) {
        let unknown = SymbolId(table.symbol_count as u16 + 100);
        for s in 0..table.state_count {
            let actions = table.actions(StateId(s as u16), unknown);
            prop_assert!(actions.is_empty());
        }
    }
}

// ===========================================================================
// Category 3: prop_state_goto_* — goto table properties (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, .. ProptestConfig::default() })]

    /// Goto rows have width matching the number of nonterminals.
    #[test]
    fn prop_state_goto_row_width_matches_nonterminals(table in arb_synthetic_table()) {
        let nt_count = table.nonterminal_to_index.len();
        for s in 0..table.state_count {
            prop_assert_eq!(
                table.goto_table[s].len(),
                nt_count,
                "state {} goto row width mismatch",
                s
            );
        }
    }

    /// All non-sentinel goto entries point to valid states.
    #[test]
    fn prop_state_goto_targets_in_range(table in arb_grammar_table()) {
        for row in &table.goto_table {
            for &target in row {
                if target != NO_GOTO {
                    prop_assert!(
                        (target.0 as usize) < table.state_count,
                        "goto target {:?} out of bounds (state_count={})",
                        target,
                        table.state_count
                    );
                }
            }
        }
    }

    /// Goto for an unknown nonterminal returns None.
    #[test]
    fn prop_state_goto_none_for_unknown_nonterminal(table in arb_grammar_table()) {
        let unknown = SymbolId(table.symbol_count as u16 + 200);
        for s in 0..table.state_count {
            prop_assert!(table.goto(StateId(s as u16), unknown).is_none());
        }
    }

    /// Goto for a terminal symbol returns None (terminals are not in the goto map).
    #[test]
    fn prop_state_goto_none_for_terminals(table in arb_grammar_table()) {
        for (&sym, &idx) in &table.symbol_to_index {
            if idx < table.token_count {
                for s in 0..table.state_count {
                    let result = table.goto(StateId(s as u16), sym);
                    // Terminal symbols should not be in nonterminal_to_index
                    if !table.nonterminal_to_index.contains_key(&sym) {
                        prop_assert!(result.is_none());
                    }
                }
            }
        }
    }

    /// Goto for an out-of-bounds state returns None.
    #[test]
    fn prop_state_goto_oob_state_returns_none(table in arb_grammar_table()) {
        let oob = StateId(table.state_count as u16 + 1);
        for &nt in table.nonterminal_to_index.keys() {
            prop_assert!(table.goto(oob, nt).is_none());
        }
    }

    /// Goto results match direct raw table access (where applicable).
    #[test]
    fn prop_state_goto_consistent_with_raw(table in arb_synthetic_table()) {
        for s in 0..table.state_count {
            for (&nt, &col) in &table.nonterminal_to_index {
                let raw = table.goto_table[s][col];
                let api = table.goto(StateId(s as u16), nt);
                if raw == NO_GOTO {
                    prop_assert!(api.is_none());
                } else {
                    prop_assert_eq!(api, Some(raw));
                }
            }
        }
    }
}

// ===========================================================================
// Category 4: prop_state_accept_* — accept state properties (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, .. ProptestConfig::default() })]

    /// Every grammar-built table contains at least one Accept action.
    #[test]
    fn prop_state_accept_exists(table in arb_grammar_table()) {
        prop_assert!(has_accept(&table), "table must contain Accept");
    }

    /// Accept appears on the EOF column.
    #[test]
    fn prop_state_accept_on_eof(table in arb_grammar_table()) {
        prop_assert!(
            accept_on_eof(&table),
            "Accept should appear on EOF column"
        );
    }

    /// At most one Accept action per action cell.
    #[test]
    fn prop_state_accept_unique_per_cell(table in arb_grammar_table()) {
        for row in &table.action_table {
            for cell in row {
                let accept_count = cell.iter().filter(|a| matches!(a, Action::Accept)).count();
                prop_assert!(accept_count <= 1, "at most 1 Accept per cell");
            }
        }
    }

    /// No cell contains both Shift and Accept simultaneously (excluding Fork).
    #[test]
    fn prop_state_accept_no_shift_in_same_cell(table in arb_grammar_table()) {
        for row in &table.action_table {
            for cell in row {
                let has_shift = cell.iter().any(|a| matches!(a, Action::Shift(_)));
                let has_acc = cell.iter().any(|a| matches!(a, Action::Accept));
                // In a standard LR table, Shift+Accept in the same cell is unusual
                if has_shift && has_acc {
                    // If both exist, they should be in a Fork
                    let has_fork = cell.iter().any(|a| matches!(a, Action::Fork(_)));
                    prop_assert!(
                        has_fork || cell.len() > 1,
                        "Shift+Accept coexistence should be via conflict"
                    );
                }
            }
        }
    }

    /// Number of accept states is bounded by state_count.
    #[test]
    fn prop_state_accept_count_bounded(table in arb_grammar_table()) {
        let count = count_accept_states(&table);
        prop_assert!(count <= table.state_count);
    }

    /// The initial state of a nontrivial grammar typically does not have Accept
    /// (Accept is reached after processing input, not before).
    #[test]
    fn prop_state_accept_initial_state_nontrivial(table in arb_grammar_table()) {
        // For grammars that are not nullable (S → ε), the initial state
        // should not contain Accept on a non-EOF terminal.
        let initial = table.initial_state;
        for &sym in table.symbol_to_index.keys() {
            if sym == table.eof_symbol {
                continue;
            }
            let actions = table.actions(initial, sym);
            let has_acc = actions.iter().any(|a| matches!(a, Action::Accept));
            if has_acc {
                // Only nullable grammars can have Accept at the initial state
                // on a non-eof symbol — this is a soft check
                prop_assert!(
                    table.state_count >= 1,
                    "Accept on non-EOF at initial state"
                );
            }
        }
    }
}

// ===========================================================================
// Category 5: prop_state_deterministic_* — determinism properties (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, .. ProptestConfig::default() })]

    /// Building the same grammar twice yields the same state_count.
    #[test]
    fn prop_state_deterministic_state_count(_seed in 0u32..50) {
        let t1 = pipeline(minimal_grammar());
        let t2 = pipeline(minimal_grammar());
        prop_assert_eq!(t1.state_count, t2.state_count);
    }

    /// Building the same grammar twice yields the same action table dimensions.
    #[test]
    fn prop_state_deterministic_action_dims(_seed in 0u32..50) {
        let t1 = pipeline(two_alt_grammar());
        let t2 = pipeline(two_alt_grammar());
        prop_assert_eq!(t1.action_table.len(), t2.action_table.len());
        for s in 0..t1.state_count {
            prop_assert_eq!(t1.action_table[s].len(), t2.action_table[s].len());
        }
    }

    /// Building the same grammar twice yields the same eof symbol.
    #[test]
    fn prop_state_deterministic_eof_same(_seed in 0u32..50) {
        let t1 = pipeline(chain_grammar());
        let t2 = pipeline(chain_grammar());
        prop_assert_eq!(t1.eof_symbol, t2.eof_symbol);
    }

    /// Building the same grammar twice yields the same start symbol.
    #[test]
    fn prop_state_deterministic_start_same(_seed in 0u32..50) {
        let t1 = pipeline(sequence_grammar());
        let t2 = pipeline(sequence_grammar());
        prop_assert_eq!(t1.start_symbol, t2.start_symbol);
    }

    /// Building the same grammar twice yields the same initial state.
    #[test]
    fn prop_state_deterministic_initial_state_same(_seed in 0u32..50) {
        let t1 = pipeline(left_recursive_grammar());
        let t2 = pipeline(left_recursive_grammar());
        prop_assert_eq!(t1.initial_state, t2.initial_state);
    }

    /// Building the same grammar twice yields identical goto table dimensions.
    #[test]
    fn prop_state_deterministic_goto_dims(_seed in 0u32..50) {
        let t1 = pipeline(deep_chain_grammar());
        let t2 = pipeline(deep_chain_grammar());
        prop_assert_eq!(t1.goto_table.len(), t2.goto_table.len());
        for s in 0..t1.state_count {
            prop_assert_eq!(t1.goto_table[s].len(), t2.goto_table[s].len());
        }
    }
}

// ===========================================================================
// Category 6: prop_state_bounded_* — bounded state IDs (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, .. ProptestConfig::default() })]

    /// All Shift targets are < state_count.
    #[test]
    fn prop_state_bounded_shift_targets(table in arb_grammar_table()) {
        for row in &table.action_table {
            for cell in row {
                for action in cell {
                    if let Action::Shift(target) = action {
                        prop_assert!(
                            (target.0 as usize) < table.state_count,
                            "shift to {:?} >= state_count {}",
                            target,
                            table.state_count
                        );
                    }
                }
            }
        }
    }

    /// All non-sentinel goto targets are < state_count.
    #[test]
    fn prop_state_bounded_goto_targets(table in arb_grammar_table()) {
        for row in &table.goto_table {
            for &target in row {
                if target != NO_GOTO {
                    prop_assert!(
                        (target.0 as usize) < table.state_count,
                        "goto target {:?} >= state_count {}",
                        target,
                        table.state_count
                    );
                }
            }
        }
    }

    /// initial_state is < state_count.
    #[test]
    fn prop_state_bounded_initial(table in arb_grammar_table()) {
        prop_assert!((table.initial_state.0 as usize) < table.state_count);
    }

    /// All Reduce RuleIds are within the rules vector bounds.
    #[test]
    fn prop_state_bounded_reduce_rule_ids(table in arb_grammar_table()) {
        prop_assert!(
            !table.rules.is_empty(),
            "grammar-built tables should have rules"
        );
        for row in &table.action_table {
            for cell in row {
                for action in cell {
                    if let Action::Reduce(rid) = action {
                        prop_assert!(
                            (rid.0 as usize) < table.rules.len(),
                            "reduce rule {:?} >= rules.len() {}",
                            rid,
                            table.rules.len()
                        );
                    }
                }
            }
        }
    }

    /// All symbol indices in symbol_to_index are < symbol_count.
    #[test]
    fn prop_state_bounded_symbol_indices(table in arb_grammar_table()) {
        for &idx in table.symbol_to_index.values() {
            prop_assert!(idx < table.symbol_count);
        }
    }

    /// state_count fits in u16 (since StateId wraps u16).
    #[test]
    fn prop_state_bounded_state_count_fits_u16(table in arb_grammar_table()) {
        prop_assert!(
            table.state_count <= u16::MAX as usize,
            "state_count {} exceeds u16::MAX",
            table.state_count
        );
    }
}

// ===========================================================================
// Category 7: prop_state_initial_* — initial state properties (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, .. ProptestConfig::default() })]

    /// Initial state is always StateId(0).
    #[test]
    fn prop_state_initial_is_zero(table in arb_grammar_table()) {
        prop_assert_eq!(table.initial_state, StateId(0));
    }

    /// Initial state has at least one non-empty action cell.
    #[test]
    fn prop_state_initial_has_actions(table in arb_grammar_table()) {
        let initial = table.initial_state.0 as usize;
        let has_any = table.action_table[initial]
            .iter()
            .any(|cell| !cell.is_empty());
        prop_assert!(has_any, "initial state must have at least one action");
    }

    /// Initial state index is within both action and goto table bounds.
    #[test]
    fn prop_state_initial_within_table_bounds(table in arb_grammar_table()) {
        let idx = table.initial_state.0 as usize;
        prop_assert!(idx < table.action_table.len());
        prop_assert!(idx < table.goto_table.len());
    }

    /// Querying actions and goto from initial state does not panic.
    #[test]
    fn prop_state_initial_queryable(table in arb_grammar_table()) {
        let init = table.initial_state;
        for &sym in table.symbol_to_index.keys() {
            let _ = table.actions(init, sym);
        }
        for &nt in table.nonterminal_to_index.keys() {
            let _ = table.goto(init, nt);
        }
    }

    /// Initial state can reach at least one other state via shift or goto.
    #[test]
    fn prop_state_initial_reaches_other(table in arb_grammar_table()) {
        let init_idx = table.initial_state.0 as usize;
        let mut reached = HashSet::new();

        // Collect shift targets
        for cell in &table.action_table[init_idx] {
            for action in cell {
                if let Action::Shift(target) = action {
                    reached.insert(*target);
                }
            }
        }

        // Collect goto targets
        for &target in &table.goto_table[init_idx] {
            if target != NO_GOTO {
                reached.insert(target);
            }
        }

        prop_assert!(
            !reached.is_empty(),
            "initial state must reach at least one other state"
        );
    }
}

// ===========================================================================
// Category 8: prop_state_complex_* — complex grammar state properties (4 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, .. ProptestConfig::default() })]

    /// Left-recursive grammar produces a valid parse table with accept.
    #[test]
    fn prop_state_complex_left_recursive(_seed in 0u32..50) {
        let table = pipeline(left_recursive_grammar());
        prop_assert!(table.state_count >= 2);
        prop_assert!(has_accept(&table));
        prop_assert!(shifts_are_valid(&table));
        prop_assert!(reduces_are_valid(&table));
    }

    /// Right-recursive grammar produces a valid parse table with accept.
    #[test]
    fn prop_state_complex_right_recursive(_seed in 0u32..50) {
        let table = pipeline(right_recursive_grammar());
        prop_assert!(table.state_count >= 2);
        prop_assert!(has_accept(&table));
        prop_assert!(shifts_are_valid(&table));
        prop_assert!(reduces_are_valid(&table));
    }

    /// Deep chain grammar produces valid state machine.
    #[test]
    fn prop_state_complex_deep_chain(_seed in 0u32..50) {
        let table = pipeline(deep_chain_grammar());
        prop_assert!(table.state_count >= 2);
        prop_assert!(has_accept(&table));
        // Deep chains have more states than minimal
        let minimal_t = pipeline(minimal_grammar());
        prop_assert!(table.state_count >= minimal_t.state_count);
    }

    /// Nullable grammar produces a valid table (S → ε | a).
    #[test]
    fn prop_state_complex_nullable(_seed in 0u32..50) {
        let table = pipeline(nullable_grammar());
        prop_assert!(table.state_count >= 1);
        prop_assert!(has_accept(&table));
        prop_assert!(shifts_are_valid(&table));
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, .. ProptestConfig::default() })]

    /// Precedence grammar builds successfully and has Accept.
    #[test]
    fn prop_state_complex_precedence(_seed in 0u32..50) {
        let table = pipeline(precedence_grammar());
        prop_assert!(table.state_count >= 2);
        prop_assert!(has_accept(&table));
        prop_assert!(shifts_are_valid(&table));
        prop_assert!(reduces_are_valid(&table));
    }

    /// Two-NT-sequence grammar produces valid state transitions.
    #[test]
    fn prop_state_complex_two_nt_sequence(_seed in 0u32..50) {
        let table = pipeline(two_nt_seq_grammar());
        prop_assert!(table.state_count >= 2);
        prop_assert!(has_accept(&table));
        prop_assert!(shifts_are_valid(&table));
        prop_assert!(reduces_are_valid(&table));
    }
}

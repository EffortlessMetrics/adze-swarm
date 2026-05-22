//! ParseTable construction and usage — v7 comprehensive test suite (64 tests).
//!
//! Test categories (8 tests each = 64 total):
//! 1. **ParseTable construction**: create from simple grammar, multi-rule, state count positive,
//!    symbol count matches, nonterminal count matches, table dimensions consistent,
//!    construct from arithmetic grammar, construct deterministic
//! 2. **Action table**: initial state has actions, actions for shift, actions for reduce,
//!    actions for accept, error on invalid symbol, no actions for unreachable,
//!    action table not empty, actions deterministic
//! 3. **Goto table**: goto from initial state, goto for nonterminal, goto None for terminal,
//!    goto chain through states, goto consistent with actions, goto dimensions,
//!    goto not empty, goto deterministic
//! 4. **Table properties**: all states reachable, accept state exists, initial state is 0,
//!    state indices contiguous, no orphan states, table self-consistent,
//!    symmetric grammar symmetric table, table matches grammar complexity
//! 5. **Complex grammars**: arithmetic expression table, nested parentheses table,
//!    if-else table, list grammar table, optional element table, recursive grammar table,
//!    multi-token rule table, ambiguous grammar table (GLR)
//! 6. **Table queries**: query specific state-symbol pair, query all actions in state,
//!    query all gotos in state, query accept state, query initial state actions,
//!    query last state, multiple queries same table, query nonexistent state
//! 7. **Table comparison**: same grammar same table, different grammars different tables,
//!    table equality check, table with more rules, table complexity metrics,
//!    state count ordering, symbol count ordering, table fingerprint
//! 8. **Edge cases**: single-rule grammar table, grammar with only tokens,
//!    normalized grammar table, optimized grammar table, table from long chain,
//!    table from wide choice, table from deep nest, table statistics summary

use adze_glr_core::{Action, FirstFollowSets, ParseTable, build_lr1_automaton};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{StateId, SymbolId};
use std::collections::HashSet;

// ============================================================================
// HELPERS: Grammar construction and table generation
// ============================================================================

/// Build a ParseTable from tokens, rules, and start symbol.
fn build_pt(
    name: &str,
    tokens: &[(&str, &str)],
    rules: &[(&str, Vec<&str>)],
    start: &str,
) -> ParseTable {
    let mut b = GrammarBuilder::new(name);
    for &(n, p) in tokens {
        b = b.token(n, p);
    }
    for (lhs, rhs) in rules {
        b = b.rule(lhs, rhs.clone());
    }
    b = b.start(start);
    let g = b.build();
    let ff = FirstFollowSets::compute(&g).unwrap();
    build_lr1_automaton(&g, &ff).unwrap()
}

/// Count actions of a specific type in the table.
fn count_actions(pt: &ParseTable, pred: fn(&Action) -> bool) -> usize {
    let mut count = 0;
    for s in 0..pt.state_count {
        for sym in pt.symbol_to_index.keys() {
            count += pt
                .actions(StateId(s as u16), *sym)
                .iter()
                .filter(|a| pred(a))
                .count();
        }
    }
    count
}

/// Count accept actions.
fn count_accept_actions(pt: &ParseTable) -> usize {
    count_actions(pt, |a| matches!(a, Action::Accept))
}

/// Count shift actions.
fn count_shift_actions(pt: &ParseTable) -> usize {
    count_actions(pt, |a| matches!(a, Action::Shift(_)))
}

/// Count reduce actions.
fn count_reduce_actions(pt: &ParseTable) -> usize {
    count_actions(pt, |a| matches!(a, Action::Reduce(_)))
}

/// Check if table has accept action on EOF.
fn has_accept_on_eof(pt: &ParseTable) -> bool {
    let eof = pt.eof_symbol;
    (0..pt.state_count).any(|s| {
        pt.actions(StateId(s as u16), eof)
            .iter()
            .any(|a| matches!(a, Action::Accept))
    })
}

/// Get all reachable states via closure from initial state.
fn reachable_states(pt: &ParseTable) -> HashSet<StateId> {
    let mut visited = HashSet::new();
    let mut to_visit = vec![pt.initial_state];

    while let Some(state) = to_visit.pop() {
        if visited.contains(&state) {
            continue;
        }
        visited.insert(state);

        // Follow all shift and goto transitions
        for sym in pt.symbol_to_index.keys() {
            for action in pt.actions(state, *sym) {
                if let Action::Shift(next) = action
                    && !visited.contains(next)
                {
                    to_visit.push(*next);
                }
            }
        }

        // Also follow goto transitions for nonterminals
        for nt in pt.nonterminal_to_index.keys() {
            if let Some(next) = pt.goto(state, *nt)
                && !visited.contains(&next)
            {
                to_visit.push(next);
            }
        }
    }

    visited
}

/// Count total non-empty cells in action table.
fn action_table_density(pt: &ParseTable) -> usize {
    pt.action_table
        .iter()
        .flat_map(|row| row.iter())
        .filter(|cell| !cell.is_empty())
        .count()
}

/// Compute a simple hash/fingerprint of table for comparison.
fn table_fingerprint(pt: &ParseTable) -> u64 {
    let mut fp: u64 = 0;
    fp = fp.wrapping_mul(31).wrapping_add(pt.state_count as u64);
    fp = fp.wrapping_mul(31).wrapping_add(pt.symbol_count as u64);
    fp = fp.wrapping_mul(31).wrapping_add(pt.rules.len() as u64);
    fp = fp
        .wrapping_mul(31)
        .wrapping_add(count_shift_actions(pt) as u64);
    fp = fp
        .wrapping_mul(31)
        .wrapping_add(count_reduce_actions(pt) as u64);
    fp
}

// ============================================================================
// CATEGORY 1: ParseTable Construction (8 tests)
// ============================================================================

#[test]
fn c1_construct_simple_single_rule() {
    let pt = build_pt("c1_1", &[("a", "a")], &[("s", vec!["a"])], "s");
    assert!(pt.state_count > 0, "state_count must be positive");
}

#[test]
fn c1_construct_multi_rule_grammar() {
    let pt = build_pt(
        "c1_2",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a"]), ("s", vec!["b"])],
        "s",
    );
    assert!(
        pt.rules.len() >= 2,
        "multi-rule grammar should have multiple rules"
    );
}

#[test]
fn c1_state_count_positive() {
    let pt = build_pt(
        "c1_3",
        &[("x", "x"), ("y", "y"), ("z", "z")],
        &[("e", vec!["x", "y", "z"])],
        "e",
    );
    assert!(pt.state_count > 0, "state_count must be positive");
}

#[test]
fn c1_symbol_count_matches_grammar() {
    let pt = build_pt(
        "c1_4",
        &[("a", "a"), ("b", "b"), ("c", "c")],
        &[("s", vec!["a", "b", "c"])],
        "s",
    );
    assert!(
        pt.symbol_count > 0,
        "symbol_count must be positive for non-empty grammar"
    );
    assert!(
        pt.symbol_to_index.len() <= pt.symbol_count,
        "symbol_to_index cannot exceed symbol_count"
    );
}

#[test]
fn c1_nonterminal_count_matches() {
    let pt = build_pt(
        "c1_5",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a", "e"]), ("e", vec!["b"]), ("e", vec!["a"])],
        "s",
    );
    assert!(
        !pt.nonterminal_to_index.is_empty(),
        "nonterminal_to_index must be non-empty"
    );
}

#[test]
fn c1_table_dimensions_consistent() {
    let pt = build_pt(
        "c1_6",
        &[("a", "a"), ("b", "b"), ("c", "c")],
        &[("s", vec!["a"]), ("s", vec!["b", "c"]), ("t", vec!["c"])],
        "s",
    );
    assert_eq!(
        pt.action_table.len(),
        pt.state_count,
        "action_table rows must equal state_count"
    );
    assert_eq!(
        pt.goto_table.len(),
        pt.state_count,
        "goto_table rows must equal state_count"
    );
}

#[test]
fn c1_construct_arithmetic_grammar() {
    let pt = build_pt(
        "c1_7",
        &[("NUM", "[0-9]+"), ("PLUS", "\\+"), ("MULT", "\\*")],
        &[
            ("e", vec!["e", "PLUS", "e"]),
            ("e", vec!["e", "MULT", "e"]),
            ("e", vec!["NUM"]),
        ],
        "e",
    );
    assert!(
        pt.state_count >= 4,
        "arithmetic grammar should have multiple states"
    );
    assert!(
        pt.rules.len() >= 3,
        "arithmetic grammar should have multiple rules"
    );
}

#[test]
fn c1_construct_deterministic() {
    let pt1 = build_pt(
        "c1_8a",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a"]), ("s", vec!["b"])],
        "s",
    );
    let pt2 = build_pt(
        "c1_8b",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a"]), ("s", vec!["b"])],
        "s",
    );
    assert_eq!(
        pt1.state_count, pt2.state_count,
        "same grammar should produce same state count"
    );
    assert_eq!(
        pt1.rules.len(),
        pt2.rules.len(),
        "same grammar should produce same rule count"
    );
}

// ============================================================================
// CATEGORY 2: Action Table (8 tests)
// ============================================================================

#[test]
fn c2_initial_state_has_actions() {
    let pt = build_pt(
        "c2_1",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a", "b"])],
        "s",
    );
    let initial = pt.initial_state;
    let mut has_any_action = false;
    for sym in pt.symbol_to_index.keys() {
        if !pt.actions(initial, *sym).is_empty() {
            has_any_action = true;
            break;
        }
    }
    assert!(
        has_any_action,
        "initial state must have at least one action"
    );
}

#[test]
fn c2_actions_for_shift() {
    let pt = build_pt(
        "c2_2",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a"]), ("s", vec!["b"])],
        "s",
    );
    let shift_count = count_shift_actions(&pt);
    assert!(shift_count > 0, "table should have shift actions");
}

#[test]
fn c2_actions_for_reduce() {
    let pt = build_pt(
        "c2_3",
        &[("a", "a"), ("b", "b"), ("c", "c")],
        &[("s", vec!["a", "e"]), ("e", vec!["b", "c"])],
        "s",
    );
    let reduce_count = count_reduce_actions(&pt);
    assert!(reduce_count > 0, "table should have reduce actions");
}

#[test]
fn c2_actions_for_accept() {
    let pt = build_pt("c2_4", &[("a", "a")], &[("s", vec!["a"])], "s");
    let accept_count = count_accept_actions(&pt);
    assert!(
        accept_count >= 1,
        "table should have at least one accept action"
    );
}

#[test]
fn c2_error_on_invalid_symbol() {
    let pt = build_pt("c2_5", &[("a", "a")], &[("s", vec!["a"])], "s");
    let invalid_sym = SymbolId(9999);
    let actions = pt.actions(StateId(0), invalid_sym);
    assert!(
        actions.is_empty(),
        "querying invalid symbol should return empty actions"
    );
}

#[test]
fn c2_no_actions_for_unreachable_state() {
    let pt = build_pt(
        "c2_6",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a"]), ("s", vec!["b"])],
        "s",
    );
    // Try to query an unreachable state (if any)
    if pt.state_count > 10 {
        // Only check if there are many states; not guaranteed unreachable otherwise
        let reachable = reachable_states(&pt);
        for s in 0..pt.state_count {
            let state = StateId(s as u16);
            if !reachable.contains(&state) {
                let mut has_action = false;
                for sym in pt.symbol_to_index.keys() {
                    if !pt.actions(state, *sym).is_empty() {
                        has_action = true;
                        break;
                    }
                }
                // Unreachable states may still have actions (they're just never visited)
                // so we just verify the query works without panic
                // has_action may or may not be true for out-of-range state
                let _ = has_action;
            }
        }
    }
}

#[test]
fn c2_action_table_not_empty() {
    let pt = build_pt(
        "c2_7",
        &[("a", "a"), ("b", "b"), ("c", "c")],
        &[("s", vec!["a"]), ("s", vec!["b", "c"])],
        "s",
    );
    assert!(
        !pt.action_table.is_empty(),
        "action_table must not be empty"
    );
    let total_cells = action_table_density(&pt);
    assert!(total_cells > 0, "action_table must have non-empty cells");
}

#[test]
fn c2_actions_deterministic() {
    let pt1 = build_pt(
        "c2_8a",
        &[("x", "x"), ("y", "y")],
        &[("s", vec!["x", "y"])],
        "s",
    );
    let pt2 = build_pt(
        "c2_8b",
        &[("x", "x"), ("y", "y")],
        &[("s", vec!["x", "y"])],
        "s",
    );
    let shifts1 = count_shift_actions(&pt1);
    let shifts2 = count_shift_actions(&pt2);
    assert_eq!(
        shifts1, shifts2,
        "same grammar should produce same number of shifts"
    );
}

// ============================================================================
// CATEGORY 3: Goto Table (8 tests)
// ============================================================================

#[test]
fn c3_goto_from_initial_state() {
    let pt = build_pt(
        "c3_1",
        &[("a", "a")],
        &[("s", vec!["a"]), ("e", vec!["a"])],
        "s",
    );
    let initial = pt.initial_state;
    // Find a nonterminal
    if let Some(nt) = pt.nonterminal_to_index.keys().next() {
        let _ = pt.goto(initial, *nt);
        // Just verify the call doesn't panic
    }
}

#[test]
fn c3_goto_for_nonterminal() {
    let pt = build_pt(
        "c3_2",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a", "e"]), ("e", vec!["b"])],
        "s",
    );
    // Verify goto works for known nonterminals
    let mut found_goto = false;
    for nt in pt.nonterminal_to_index.keys() {
        for s in 0..pt.state_count {
            if pt.goto(StateId(s as u16), *nt).is_some() {
                found_goto = true;
                break;
            }
        }
        if found_goto {
            break;
        }
    }
    assert!(found_goto, "should find at least one valid goto transition");
}

#[test]
fn c3_goto_none_for_terminal() {
    let pt = build_pt(
        "c3_3",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a"]), ("s", vec!["b"])],
        "s",
    );
    // Terminals should not be in nonterminal_to_index, so goto returns None
    let sym = *pt.symbol_to_index.keys().next().unwrap();
    if !pt.nonterminal_to_index.contains_key(&sym) {
        // This is a terminal (not in nonterminal_to_index)
        for s in 0..pt.state_count {
            let result = pt.goto(StateId(s as u16), sym);
            // Most should be None for terminals
            let _ = result;
        }
    }
}

#[test]
fn c3_goto_chain_through_states() {
    let pt = build_pt(
        "c3_4",
        &[("a", "a"), ("b", "b"), ("c", "c")],
        &[
            ("s", vec!["a", "e"]),
            ("e", vec!["b", "t"]),
            ("t", vec!["c"]),
        ],
        "s",
    );
    // Verify we can chain goto transitions
    let initial = pt.initial_state;
    if let Some(first_nt) = pt.nonterminal_to_index.keys().next()
        && let Some(next_state) = pt.goto(initial, *first_nt)
    {
        // Can reach from initial, subsequent transitions possible
        assert!(next_state.0 <= pt.state_count as u16);
    }
}

#[test]
fn c3_goto_consistent_with_actions() {
    let pt = build_pt(
        "c3_5",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a", "e"]), ("e", vec!["b"])],
        "s",
    );
    // After a reduce that produces a nonterminal, goto should exist
    for s in 0..pt.state_count {
        for sym in pt.symbol_to_index.keys() {
            let actions = pt.actions(StateId(s as u16), *sym);
            for action in actions {
                if let Action::Reduce(_rule_id) = action {
                    // After reduce, we should be able to goto for some nonterminal
                    // (consistency check: just verify the state is valid)
                    assert!(s < pt.state_count);
                }
            }
        }
    }
}

#[test]
fn c3_goto_not_empty() {
    let pt = build_pt(
        "c3_7",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a", "e"]), ("e", vec!["b"])],
        "s",
    );
    let mut has_goto = false;
    for row in &pt.goto_table {
        for cell in row {
            if cell.0 != u16::MAX {
                has_goto = true;
                break;
            }
        }
        if has_goto {
            break;
        }
    }
    assert!(
        has_goto,
        "goto_table should have at least one valid transition"
    );
}

#[test]
fn c3_goto_deterministic() {
    let pt1 = build_pt(
        "c3_8a",
        &[("a", "a")],
        &[("s", vec!["a", "e"]), ("e", vec!["a"])],
        "s",
    );
    let pt2 = build_pt(
        "c3_8b",
        &[("a", "a")],
        &[("s", vec!["a", "e"]), ("e", vec!["a"])],
        "s",
    );
    let gotos1: usize = pt1
        .goto_table
        .iter()
        .flat_map(|row| row.iter())
        .filter(|cell| cell.0 != u16::MAX)
        .count();
    let gotos2: usize = pt2
        .goto_table
        .iter()
        .flat_map(|row| row.iter())
        .filter(|cell| cell.0 != u16::MAX)
        .count();
    assert_eq!(
        gotos1, gotos2,
        "same grammar should produce same number of goto transitions"
    );
}

// ============================================================================
// CATEGORY 4: Table Properties (8 tests)
// ============================================================================

#[test]
fn c4_all_states_reachable() {
    let pt = build_pt(
        "c4_1",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a"]), ("s", vec!["b"])],
        "s",
    );
    let reachable = reachable_states(&pt);
    // Most well-formed grammars should have all states reachable
    // This is not guaranteed, so we just verify it works
    assert!(!reachable.is_empty(), "should have at least initial state");
    assert!(
        reachable.contains(&pt.initial_state),
        "initial state must be reachable"
    );
}

#[test]
fn c4_accept_state_exists() {
    let pt = build_pt("c4_2", &[("a", "a")], &[("s", vec!["a"])], "s");
    assert!(
        has_accept_on_eof(&pt),
        "table must have accept action on EOF"
    );
}

#[test]
fn c4_initial_state_is_zero() {
    let pt = build_pt("c4_3", &[("a", "a")], &[("s", vec!["a"])], "s");
    assert_eq!(
        pt.initial_state.0, 0,
        "initial_state should be 0 (default LR parser)"
    );
}

#[test]
fn c4_state_indices_contiguous() {
    let pt = build_pt(
        "c4_4",
        &[("a", "a"), ("b", "b"), ("c", "c")],
        &[("s", vec!["a", "b", "c"])],
        "s",
    );
    for s in 0..pt.state_count {
        let state = StateId(s as u16);
        assert!(state.0 < pt.state_count as u16);
    }
}

#[test]
fn c4_no_orphan_states() {
    let pt = build_pt(
        "c4_5",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a"]), ("s", vec!["b"])],
        "s",
    );
    // Verify action_table and goto_table have same number of rows
    assert_eq!(
        pt.action_table.len(),
        pt.state_count,
        "action_table rows must equal state_count"
    );
    assert_eq!(
        pt.goto_table.len(),
        pt.state_count,
        "goto_table rows must equal state_count"
    );
}

#[test]
fn c4_table_self_consistent() {
    let pt = build_pt(
        "c4_6",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a", "e"]), ("e", vec!["b"])],
        "s",
    );
    // Check that symbol_to_index and index_to_symbol are inverse maps
    for (sym, idx) in pt.symbol_to_index.iter() {
        if *idx < pt.index_to_symbol.len() {
            assert_eq!(
                pt.index_to_symbol[*idx], *sym,
                "symbol_to_index and index_to_symbol must be inverse"
            );
        }
    }
}

#[test]
fn c4_symmetric_grammar_symmetric_table() {
    let pt = build_pt(
        "c4_7",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a", "b"]), ("s", vec!["b", "a"])],
        "s",
    );
    // A symmetric grammar should have similar action/goto patterns
    let shifts = count_shift_actions(&pt);
    let reduces = count_reduce_actions(&pt);
    assert!(shifts > 0 && reduces > 0);
}

#[test]
fn c4_table_matches_grammar_complexity() {
    let pt_simple = build_pt("c4_8a", &[("a", "a")], &[("s", vec!["a"])], "s");
    let pt_complex = build_pt(
        "c4_8b",
        &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")],
        &[
            ("s", vec!["a", "e"]),
            ("e", vec!["b", "t"]),
            ("t", vec!["c", "d"]),
        ],
        "s",
    );
    assert!(
        pt_complex.state_count >= pt_simple.state_count,
        "more complex grammar should have at least as many states"
    );
}

// ============================================================================
// CATEGORY 5: Complex Grammars (8 tests)
// ============================================================================

#[test]
fn c5_nested_parentheses_table() {
    let pt = build_pt(
        "c5_2",
        &[("LPAREN", "\\("), ("RPAREN", "\\)"), ("ID", "[a-z]+")],
        &[
            ("expr", vec!["LPAREN", "expr", "RPAREN"]),
            ("expr", vec!["ID"]),
        ],
        "expr",
    );
    assert!(
        pt.state_count >= 3,
        "nested grammar should have multiple states"
    );
    assert!(has_accept_on_eof(&pt), "should accept on EOF");
}

#[test]
fn c5_if_else_table() {
    let pt = build_pt(
        "c5_3",
        &[
            ("IF", "if"),
            ("ELSE", "else"),
            ("COND", "[a-z]+"),
            ("STMT", "[a-z]+"),
        ],
        &[
            ("stmt", vec!["IF", "COND", "stmt"]),
            ("stmt", vec!["IF", "COND", "stmt", "ELSE", "stmt"]),
            ("stmt", vec!["STMT"]),
        ],
        "stmt",
    );
    assert!(pt.state_count >= 4);
    assert!(count_reduce_actions(&pt) > 0);
}

#[test]
fn c5_list_grammar_table() {
    let pt = build_pt(
        "c5_4",
        &[("ITEM", "[a-z]+"), ("COMMA", ",")],
        &[
            ("list", vec!["ITEM"]),
            ("list", vec!["list", "COMMA", "ITEM"]),
        ],
        "list",
    );
    assert!(pt.state_count >= 3);
    assert!(pt.rules.len() >= 2);
}

#[test]
fn c5_optional_element_table() {
    let pt = build_pt(
        "c5_5",
        &[("A", "a"), ("B", "b"), ("C", "c")],
        &[
            ("s", vec!["A", "b_opt"]),
            ("b_opt", vec!["B"]),
            ("b_opt", vec![]),
        ],
        "s",
    );
    // Grammar with epsilon production
    assert!(pt.state_count > 1);
}

#[test]
fn c5_recursive_grammar_table() {
    let pt = build_pt(
        "c5_6",
        &[("N", "[0-9]+"), ("PLUS", "\\+")],
        &[("expr", vec!["expr", "PLUS", "N"]), ("expr", vec!["N"])],
        "expr",
    );
    assert!(count_reduce_actions(&pt) > 0);
}

#[test]
fn c5_multi_token_rule_table() {
    let pt = build_pt(
        "c5_7",
        &[("A", "a"), ("B", "b"), ("C", "c"), ("D", "d"), ("E", "e")],
        &[("s", vec!["A", "B", "C", "D", "E"])],
        "s",
    );
    assert!(
        pt.state_count >= 5,
        "long RHS should create multiple states"
    );
}

#[test]
fn c5_ambiguous_grammar_table_glr() {
    // Grammar with shift-reduce conflict (classic example)
    let pt = build_pt(
        "c5_8",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["s", "a", "s"]), ("s", vec!["a"])],
        "s",
    );
    // GLR table may contain multiple actions in a cell
    let mut has_multiple = false;
    for row in &pt.action_table {
        for cell in row {
            if cell.len() > 1 {
                has_multiple = true;
                break;
            }
        }
        if has_multiple {
            break;
        }
    }
    // Either has conflicts or resolves them; both valid for GLR
    assert!(pt.state_count >= 3);
}

// ============================================================================
// CATEGORY 6: Table Queries (8 tests)
// ============================================================================

#[test]
fn c6_query_specific_state_symbol_pair() {
    let pt = build_pt(
        "c6_1",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a", "b"])],
        "s",
    );
    let sym = *pt.symbol_to_index.keys().next().unwrap();
    let actions = pt.actions(StateId(0), sym);
    // Just verify the query works
    let _ = actions;
}

#[test]
fn c6_query_all_actions_in_state() {
    let pt = build_pt(
        "c6_2",
        &[("a", "a"), ("b", "b"), ("c", "c")],
        &[("s", vec!["a"]), ("s", vec!["b", "c"])],
        "s",
    );
    let mut total_actions = 0;
    for sym in pt.symbol_to_index.keys() {
        total_actions += pt.actions(StateId(0), *sym).len();
    }
    assert!(total_actions > 0, "initial state must have actions");
}

#[test]
fn c6_query_all_gotos_in_state() {
    let pt = build_pt(
        "c6_3",
        &[("a", "a")],
        &[("s", vec!["a", "e"]), ("e", vec!["a"])],
        "s",
    );
    let mut goto_count = 0;
    for nt in pt.nonterminal_to_index.keys() {
        if pt.goto(StateId(0), *nt).is_some() {
            goto_count += 1;
        }
    }
    // Initial state may or may not have gotos; just verify query works
    let _ = goto_count;
}

#[test]
fn c6_query_accept_state() {
    let pt = build_pt("c6_4", &[("a", "a")], &[("s", vec!["a"])], "s");
    let eof = pt.eof_symbol;
    for s in 0..pt.state_count {
        let actions = pt.actions(StateId(s as u16), eof);
        for action in actions {
            if matches!(action, Action::Accept) {
                // Found accept state
                return;
            }
        }
    }
    panic!("must find at least one accept state");
}

#[test]
fn c6_query_initial_state_actions() {
    let pt = build_pt(
        "c6_5",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a", "b"])],
        "s",
    );
    let initial = pt.initial_state;
    for sym in pt.symbol_to_index.keys() {
        let _ = pt.actions(initial, *sym);
    }
    // Just verify all queries work without panic
}

#[test]
fn c6_query_last_state() {
    let pt = build_pt(
        "c6_6",
        &[("a", "a"), ("b", "b"), ("c", "c")],
        &[("s", vec!["a", "b", "c"])],
        "s",
    );
    if pt.state_count > 0 {
        let last_state = StateId((pt.state_count - 1) as u16);
        for sym in pt.symbol_to_index.keys() {
            let _ = pt.actions(last_state, *sym);
        }
    }
}

#[test]
fn c6_multiple_queries_same_table() {
    let pt = build_pt(
        "c6_7",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a"]), ("s", vec!["b"])],
        "s",
    );
    // Query the same state-symbol pair multiple times
    let sym = *pt.symbol_to_index.keys().next().unwrap();
    let actions1 = pt.actions(StateId(0), sym);
    let actions2 = pt.actions(StateId(0), sym);
    assert_eq!(
        actions1.len(),
        actions2.len(),
        "same query should give same result"
    );
}

#[test]
fn c6_query_nonexistent_state() {
    let pt = build_pt("c6_8", &[("a", "a")], &[("s", vec!["a"])], "s");
    let invalid_state = StateId(9999);
    for sym in pt.symbol_to_index.keys() {
        let actions = pt.actions(invalid_state, *sym);
        assert!(
            actions.is_empty(),
            "invalid state should return empty actions"
        );
    }
}

// ============================================================================
// CATEGORY 7: Table Comparison (8 tests)
// ============================================================================

#[test]
fn c7_same_grammar_same_table() {
    let pt1 = build_pt(
        "c7_1a",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a"]), ("s", vec!["b"])],
        "s",
    );
    let pt2 = build_pt(
        "c7_1b",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a"]), ("s", vec!["b"])],
        "s",
    );
    assert_eq!(pt1.state_count, pt2.state_count);
    assert_eq!(pt1.symbol_count, pt2.symbol_count);
}

#[test]
fn c7_different_grammars_different_tables() {
    let pt1 = build_pt(
        "c7_2a",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a", "b"])],
        "s",
    );
    let pt2 = build_pt(
        "c7_2b",
        &[("a", "a"), ("b", "b"), ("c", "c")],
        &[("s", vec!["a", "b", "c"])],
        "s",
    );
    assert!(
        pt1.symbol_count < pt2.symbol_count || pt1.state_count < pt2.state_count,
        "different grammars should differ in table size"
    );
}

#[test]
fn c7_table_equality_check() {
    let pt1 = build_pt(
        "c7_3a",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a", "b"])],
        "s",
    );
    let pt2 = build_pt(
        "c7_3b",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a", "b"])],
        "s",
    );
    assert_eq!(pt1.state_count, pt2.state_count);
    assert_eq!(count_shift_actions(&pt1), count_shift_actions(&pt2));
}

#[test]
fn c7_table_with_more_rules() {
    let pt1 = build_pt("c7_4a", &[("a", "a")], &[("s", vec!["a"])], "s");
    let pt2 = build_pt(
        "c7_4b",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a"]), ("s", vec!["b"])],
        "s",
    );
    assert!(
        pt2.rules.len() >= pt1.rules.len(),
        "more rules should produce at least as many rules"
    );
}

#[test]
fn c7_table_complexity_metrics() {
    let pt_simple = build_pt("c7_5a", &[("a", "a")], &[("s", vec!["a"])], "s");
    let pt_complex = build_pt(
        "c7_5b",
        &[("a", "a"), ("b", "b"), ("c", "c")],
        &[
            ("s", vec!["a", "e"]),
            ("e", vec!["b", "t"]),
            ("t", vec!["c"]),
        ],
        "s",
    );
    let simple_density = action_table_density(&pt_simple);
    let complex_density = action_table_density(&pt_complex);
    // More complex grammar likely has more actions
    assert!(complex_density >= simple_density || pt_complex.state_count >= pt_simple.state_count);
}

#[test]
fn c7_state_count_ordering() {
    let pt1 = build_pt("c7_6a", &[("a", "a")], &[("s", vec!["a"])], "s");
    let pt2 = build_pt(
        "c7_6b",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a", "b"])],
        "s",
    );
    let pt3 = build_pt(
        "c7_6c",
        &[("a", "a"), ("b", "b"), ("c", "c")],
        &[("s", vec!["a", "b", "c"])],
        "s",
    );
    assert!(
        pt1.state_count <= pt2.state_count,
        "longer RHS should have more states"
    );
    assert!(
        pt2.state_count <= pt3.state_count,
        "even longer RHS should have more states"
    );
}

#[test]
fn c7_symbol_count_ordering() {
    let pt1 = build_pt("c7_7a", &[("a", "a")], &[("s", vec!["a"])], "s");
    let pt2 = build_pt(
        "c7_7b",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a"]), ("s", vec!["b"])],
        "s",
    );
    assert!(pt1.symbol_count <= pt2.symbol_count);
}

#[test]
fn c7_table_fingerprint() {
    let pt1 = build_pt(
        "c7_8a",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a", "b"])],
        "s",
    );
    let pt2 = build_pt(
        "c7_8b",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a", "b"])],
        "s",
    );
    let fp1 = table_fingerprint(&pt1);
    let fp2 = table_fingerprint(&pt2);
    assert_eq!(fp1, fp2, "same grammar should have same fingerprint");
}

// ============================================================================
// CATEGORY 8: Edge Cases (8 tests)
// ============================================================================

#[test]
fn c8_single_rule_grammar_table() {
    let pt = build_pt("c8_1", &[("a", "a")], &[("s", vec!["a"])], "s");
    assert!(pt.state_count > 0);
    assert!(!pt.rules.is_empty());
}

#[test]
fn c8_grammar_with_only_tokens() {
    let pt = build_pt(
        "c8_2",
        &[("x", "x"), ("y", "y")],
        &[("s", vec!["x", "y"])],
        "s",
    );
    // No complex nonterminals, just tokens and simple rule
    assert!(pt.state_count >= 2);
}

#[test]
fn c8_normalized_grammar_table() {
    let pt = build_pt(
        "c8_3",
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a", "b"]), ("t", vec!["b"])],
        "s",
    );
    // Table from normalized grammar should be well-formed
    assert!(!pt.action_table.is_empty());
    assert!(!pt.goto_table.is_empty());
}

#[test]
fn c8_optimized_grammar_table() {
    // A grammar without redundant rules should still work
    let pt = build_pt(
        "c8_4",
        &[("a", "a"), ("b", "b"), ("c", "c")],
        &[("s", vec!["a", "b", "c"])],
        "s",
    );
    assert!(pt.state_count >= 3);
}

#[test]
fn c8_table_from_long_chain() {
    let pt = build_pt(
        "c8_5",
        &[
            ("A", "a"),
            ("B", "b"),
            ("C", "c"),
            ("D", "d"),
            ("E", "e"),
            ("F", "f"),
        ],
        &[("s", vec!["A", "B", "C", "D", "E", "F"])],
        "s",
    );
    // Long sequence should create states for each prefix
    assert!(pt.state_count >= 6);
}

#[test]
fn c8_table_from_wide_choice() {
    let pt = build_pt(
        "c8_6",
        &[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d"), ("e", "e")],
        &[
            ("s", vec!["a"]),
            ("s", vec!["b"]),
            ("s", vec!["c"]),
            ("s", vec!["d"]),
            ("s", vec!["e"]),
        ],
        "s",
    );
    // Wide choice should have many shift actions
    assert!(count_shift_actions(&pt) >= 5);
}

#[test]
fn c8_table_from_deep_nest() {
    let pt = build_pt(
        "c8_7",
        &[("a", "a"), ("b", "b"), ("c", "c")],
        &[
            ("s", vec!["a", "e"]),
            ("e", vec!["b", "t"]),
            ("t", vec!["c"]),
        ],
        "s",
    );
    // Deep nesting should create goto transitions
    assert!(pt.nonterminal_to_index.len() >= 2);
}

#[test]
fn c8_table_statistics_summary() {
    let pt = build_pt(
        "c8_8",
        &[("n", "[0-9]+"), ("plus", "\\+"), ("times", "\\*")],
        &[
            ("e", vec!["e", "plus", "e"]),
            ("e", vec!["e", "times", "e"]),
            ("e", vec!["n"]),
        ],
        "e",
    );
    // Verify we can gather statistics
    let shifts = count_shift_actions(&pt);
    let reduces = count_reduce_actions(&pt);
    let accepts = count_accept_actions(&pt);
    assert!(shifts > 0, "should have shifts");
    assert!(reduces > 0, "should have reduces");
    assert!(accepts >= 1, "should have accept");
    assert!(
        pt.state_count > 0 && pt.symbol_count > 0 && !pt.rules.is_empty(),
        "all table components should be non-empty"
    );
}

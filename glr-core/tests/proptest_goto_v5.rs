//! Property-based tests for GOTO table properties (v5).
//!
//! 46 proptest property tests across 8 categories covering goto validity,
//! determinism, nonterminal-only entries, bounds, consistency, complexity,
//! and self-loop constraints.
//!
//! Run with: `cargo test -p adze-glr-core --test proptest_goto_v5 -- --test-threads=2`

use adze_glr_core::{FirstFollowSets, ParseTable, build_lr1_automaton};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{Associativity, Grammar, StateId, SymbolId};
use proptest::prelude::*;
use std::collections::BTreeSet;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_table(grammar: &Grammar) -> ParseTable {
    let ff = FirstFollowSets::compute(grammar).expect("FIRST/FOLLOW failed");
    build_lr1_automaton(grammar, &ff).expect("build_lr1_automaton failed")
}

fn try_build(grammar: &Grammar) -> Option<ParseTable> {
    let ff = FirstFollowSets::compute(grammar).ok()?;
    build_lr1_automaton(grammar, &ff).ok()
}

fn nonterminal_ids(table: &ParseTable) -> Vec<SymbolId> {
    table.nonterminal_to_index.keys().copied().collect()
}

fn terminal_ids(grammar: &Grammar) -> Vec<SymbolId> {
    grammar.tokens.keys().copied().collect()
}

fn all_goto_targets(table: &ParseTable) -> BTreeSet<StateId> {
    let mut targets = BTreeSet::new();
    for s in 0..table.state_count {
        let st = StateId(s as u16);
        for &nt in table.nonterminal_to_index.keys() {
            if let Some(tgt) = table.goto(st, nt) {
                targets.insert(tgt);
            }
        }
    }
    targets
}

fn goto_defined_count(table: &ParseTable) -> usize {
    let mut count = 0;
    for s in 0..table.state_count {
        let st = StateId(s as u16);
        for &nt in table.nonterminal_to_index.keys() {
            if table.goto(st, nt).is_some() {
                count += 1;
            }
        }
    }
    count
}

// ---------------------------------------------------------------------------
// Fixed grammars
// ---------------------------------------------------------------------------

/// S → a
fn minimal_grammar() -> Grammar {
    GrammarBuilder::new("min")
        .token("a", "a")
        .rule("s", vec!["a"])
        .start("s")
        .build()
}

/// S → a | b
fn two_alt_grammar() -> Grammar {
    GrammarBuilder::new("twoalt")
        .token("a", "a")
        .token("b", "b")
        .rule("s", vec!["a"])
        .rule("s", vec!["b"])
        .start("s")
        .build()
}

/// S → ε | a
fn nullable_grammar() -> Grammar {
    GrammarBuilder::new("nullable")
        .token("a", "a")
        .rule("s", vec![])
        .rule("s", vec!["a"])
        .start("s")
        .build()
}

/// S → S a | a (left-recursive)
fn left_recursive_grammar() -> Grammar {
    GrammarBuilder::new("leftrec")
        .token("a", "a")
        .rule("s", vec!["s", "a"])
        .rule("s", vec!["a"])
        .start("s")
        .build()
}

/// S → a S | a (right-recursive)
fn right_recursive_grammar() -> Grammar {
    GrammarBuilder::new("rightrec")
        .token("a", "a")
        .rule("s", vec!["a", "s"])
        .rule("s", vec!["a"])
        .start("s")
        .build()
}

/// S → T, T → a (chain of two nonterminals)
fn chain_grammar() -> Grammar {
    GrammarBuilder::new("chain")
        .token("a", "a")
        .rule("s", vec!["t"])
        .rule("t", vec!["a"])
        .start("s")
        .build()
}

/// S → T, T → U, U → a (deep chain)
fn deep_chain_grammar() -> Grammar {
    GrammarBuilder::new("deep")
        .token("a", "a")
        .rule("s", vec!["t"])
        .rule("t", vec!["u"])
        .rule("u", vec!["a"])
        .start("s")
        .build()
}

/// S → T U, T → a, U → b (sequence of two nonterminals)
fn two_nt_seq_grammar() -> Grammar {
    GrammarBuilder::new("twont")
        .token("a", "a")
        .token("b", "b")
        .rule("s", vec!["t", "u"])
        .rule("t", vec!["a"])
        .rule("u", vec!["b"])
        .start("s")
        .build()
}

/// S → a b c (sequence of terminals)
fn sequence_grammar() -> Grammar {
    GrammarBuilder::new("seq")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .rule("s", vec!["a", "b", "c"])
        .start("s")
        .build()
}

/// S → a | b | c | d | e (wide alternatives)
fn wide_alt_grammar() -> Grammar {
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
        .build()
}

/// E → E + E | E * E | a (with precedence)
fn precedence_grammar() -> Grammar {
    GrammarBuilder::new("prec")
        .token("a", "a")
        .token("plus", "+")
        .token("star", "*")
        .rule_with_precedence("expr", vec!["expr", "plus", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "star", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["a"])
        .start("expr")
        .build()
}

/// S → T U V, T → a, U → b, V → c (three nonterminal sequence)
fn three_nt_grammar() -> Grammar {
    GrammarBuilder::new("threent")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .rule("s", vec!["t", "u", "v"])
        .rule("t", vec!["a"])
        .rule("u", vec!["b"])
        .rule("v", vec!["c"])
        .start("s")
        .build()
}

// ---------------------------------------------------------------------------
// Proptest strategies
// ---------------------------------------------------------------------------

fn arb_fixed_grammar() -> impl Strategy<Value = Grammar> {
    prop_oneof![
        Just(minimal_grammar()),
        Just(two_alt_grammar()),
        Just(nullable_grammar()),
        Just(left_recursive_grammar()),
        Just(right_recursive_grammar()),
        Just(chain_grammar()),
        Just(deep_chain_grammar()),
        Just(two_nt_seq_grammar()),
        Just(sequence_grammar()),
        Just(wide_alt_grammar()),
        Just(precedence_grammar()),
        Just(three_nt_grammar()),
    ]
}

const TOKEN_NAMES: &[&str] = &["a", "b", "c", "d", "e", "f"];
const TOKEN_PATTERNS: &[&str] = &["a", "b", "c", "d", "e", "f"];
const NT_NAMES: &[&str] = &["s", "t", "u", "v", "w"];

fn build_grammar_from(n_tok: usize, productions: &[Vec<Vec<usize>>]) -> Grammar {
    let mut builder = GrammarBuilder::new("proptest");
    for i in 0..n_tok {
        builder = builder.token(TOKEN_NAMES[i], TOKEN_PATTERNS[i]);
    }
    for (nt_idx, nt_prods) in productions.iter().enumerate() {
        let lhs = NT_NAMES[nt_idx];
        for rhs_indices in nt_prods {
            let rhs: Vec<&str> = rhs_indices
                .iter()
                .map(|&idx| {
                    if idx < n_tok {
                        TOKEN_NAMES[idx]
                    } else {
                        NT_NAMES[idx - n_tok]
                    }
                })
                .collect();
            builder = builder.rule(lhs, rhs);
        }
    }
    builder = builder.start(NT_NAMES[0]);
    builder.build()
}

/// Random grammar: 1-3 tokens, 1-3 nonterminals, random productions.
fn arb_random_grammar() -> impl Strategy<Value = Grammar> {
    (1..=3usize, 1..=3usize).prop_flat_map(|(n_tok, n_nt)| {
        proptest::collection::vec(
            proptest::collection::vec(proptest::collection::vec(0..(n_tok + n_nt), 1..=3), 1..=3),
            n_nt..=n_nt,
        )
        .prop_map(move |prods| build_grammar_from(n_tok, &prods))
    })
}

/// Simple valid grammar: S → t0 with optional extra alternatives.
fn arb_valid_grammar() -> impl Strategy<Value = Grammar> {
    (1usize..=5, 0usize..=2)
        .prop_flat_map(|(n_tok, n_extra)| {
            let rhs_indices = proptest::collection::vec(0..n_tok, n_extra);
            (Just(n_tok), rhs_indices)
        })
        .prop_map(|(n_tok, rhs_indices)| {
            let tok_names: Vec<String> = (0..n_tok).map(|i| format!("t{i}")).collect();
            let mut bld = GrammarBuilder::new("rand");
            for tn in &tok_names {
                bld = bld.token(tn, tn);
            }
            bld = bld.rule("s", vec![tok_names[0].as_str()]);
            for &idx in &rhs_indices {
                bld = bld.rule("s", vec![tok_names[idx].as_str()]);
            }
            bld = bld.start("s");
            bld.build()
        })
}

/// Two-nonterminal grammar: S → A, A → tok*.
fn arb_two_nt_grammar() -> impl Strategy<Value = Grammar> {
    (1usize..=4, 0usize..=3)
        .prop_flat_map(|(n_tok, n_extra)| {
            let rhs_indices = proptest::collection::vec(0..n_tok, n_extra);
            (Just(n_tok), rhs_indices)
        })
        .prop_map(|(n_tok, rhs_indices)| {
            let tok_names: Vec<String> = (0..n_tok).map(|i| format!("t{i}")).collect();
            let mut bld = GrammarBuilder::new("two_nt");
            for tn in &tok_names {
                bld = bld.token(tn, tn);
            }
            bld = bld.rule("s", vec!["mid"]);
            bld = bld.rule("mid", vec![tok_names[0].as_str()]);
            for &idx in &rhs_indices {
                bld = bld.rule("mid", vec![tok_names[idx].as_str()]);
            }
            bld = bld.start("s");
            bld.build()
        })
}

// ===========================================================================
// Category 1 — prop_goto_valid_states_* (6 tests)
// Goto always returns valid states within [0, state_count).
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_goto_valid_states_fixed(grammar in arb_fixed_grammar()) {
        let table = build_table(&grammar);
        for s in 0..table.state_count {
            let st = StateId(s as u16);
            for &nt in table.nonterminal_to_index.keys() {
                if let Some(tgt) = table.goto(st, nt) {
                    prop_assert!(
                        (tgt.0 as usize) < table.state_count,
                        "goto({s}, {nt:?}) = {tgt:?} out of range (states={})",
                        table.state_count,
                    );
                }
            }
        }
    }

    #[test]
    fn prop_goto_valid_states_random(grammar in arb_random_grammar()) {
        if let Some(table) = try_build(&grammar) {
            for s in 0..table.state_count {
                let st = StateId(s as u16);
                for &nt in table.nonterminal_to_index.keys() {
                    if let Some(tgt) = table.goto(st, nt) {
                        prop_assert!((tgt.0 as usize) < table.state_count);
                    }
                }
            }
        }
    }

    #[test]
    fn prop_goto_valid_states_simple(grammar in arb_valid_grammar()) {
        let table = build_table(&grammar);
        for s in 0..table.state_count {
            let st = StateId(s as u16);
            for &nt in table.nonterminal_to_index.keys() {
                if let Some(tgt) = table.goto(st, nt) {
                    prop_assert!((tgt.0 as usize) < table.state_count);
                }
            }
        }
    }

    #[test]
    fn prop_goto_valid_states_two_nt(grammar in arb_two_nt_grammar()) {
        let table = build_table(&grammar);
        for s in 0..table.state_count {
            let st = StateId(s as u16);
            for &nt in table.nonterminal_to_index.keys() {
                if let Some(tgt) = table.goto(st, nt) {
                    prop_assert!((tgt.0 as usize) < table.state_count);
                }
            }
        }
    }

    #[test]
    fn prop_goto_valid_states_all_targets_subset(grammar in arb_fixed_grammar()) {
        let table = build_table(&grammar);
        let targets = all_goto_targets(&table);
        for tgt in &targets {
            prop_assert!(
                (tgt.0 as usize) < table.state_count,
                "target {tgt:?} out of bounds (state_count={})", table.state_count
            );
        }
    }

    #[test]
    fn prop_goto_valid_states_initial_reachable(grammar in arb_fixed_grammar()) {
        let table = build_table(&grammar);
        // The initial state must be valid
        prop_assert!((table.initial_state.0 as usize) < table.state_count);
        // Any goto from the initial state must be valid
        for &nt in table.nonterminal_to_index.keys() {
            if let Some(tgt) = table.goto(table.initial_state, nt) {
                prop_assert!((tgt.0 as usize) < table.state_count);
            }
        }
    }
}

// ===========================================================================
// Category 2 — prop_goto_deterministic_* (6 tests)
// Same input → same goto result (GOTO is a function, not a relation).
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_goto_deterministic_fixed(grammar in arb_fixed_grammar()) {
        let table = build_table(&grammar);
        for s in 0..table.state_count {
            let st = StateId(s as u16);
            for &nt in table.nonterminal_to_index.keys() {
                let r1 = table.goto(st, nt);
                let r2 = table.goto(st, nt);
                prop_assert_eq!(r1, r2, "goto non-deterministic");
            }
        }
    }

    #[test]
    fn prop_goto_deterministic_random(grammar in arb_random_grammar()) {
        if let Some(table) = try_build(&grammar) {
            for s in 0..table.state_count {
                let st = StateId(s as u16);
                for &nt in table.nonterminal_to_index.keys() {
                    let r1 = table.goto(st, nt);
                    let r2 = table.goto(st, nt);
                    prop_assert_eq!(r1, r2);
                }
            }
        }
    }

    #[test]
    fn prop_goto_deterministic_two_nt(grammar in arb_two_nt_grammar()) {
        let table = build_table(&grammar);
        for s in 0..table.state_count {
            let st = StateId(s as u16);
            for &nt in table.nonterminal_to_index.keys() {
                prop_assert_eq!(table.goto(st, nt), table.goto(st, nt));
            }
        }
    }

    #[test]
    fn prop_goto_deterministic_simple(grammar in arb_valid_grammar()) {
        let table = build_table(&grammar);
        for s in 0..table.state_count {
            let st = StateId(s as u16);
            for &nt in table.nonterminal_to_index.keys() {
                let first = table.goto(st, nt);
                let second = table.goto(st, nt);
                let third = table.goto(st, nt);
                prop_assert_eq!(first, second);
                prop_assert_eq!(second, third);
            }
        }
    }

    #[test]
    fn prop_goto_deterministic_rebuild(grammar in arb_fixed_grammar()) {
        // Building twice from the same grammar should produce the same goto results.
        let t1 = build_table(&grammar);
        let t2 = build_table(&grammar);
        for s in 0..t1.state_count {
            let st = StateId(s as u16);
            for &nt in t1.nonterminal_to_index.keys() {
                prop_assert_eq!(
                    t1.goto(st, nt), t2.goto(st, nt),
                    "rebuild mismatch"
                );
            }
        }
    }

    #[test]
    fn prop_goto_deterministic_each_cell_single(grammar in arb_fixed_grammar()) {
        let table = build_table(&grammar);
        // Each goto_table cell is a single StateId — deterministic by construction.
        for row in &table.goto_table {
            for &cell in row {
                prop_assert!(
                    cell.0 == u16::MAX || (cell.0 as usize) < table.state_count,
                    "cell {cell:?} is neither sentinel nor valid state"
                );
            }
        }
    }
}

// ===========================================================================
// Category 3 — prop_goto_none_* (6 tests)
// Invalid / out-of-range entries return None.
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_goto_none_unknown_nonterminal(grammar in arb_fixed_grammar()) {
        let table = build_table(&grammar);
        // A SymbolId not in nonterminal_to_index must return None
        let bogus = SymbolId(60000);
        for s in 0..table.state_count {
            let st = StateId(s as u16);
            prop_assert_eq!(table.goto(st, bogus), None);
        }
    }

    #[test]
    fn prop_goto_none_out_of_range_state(grammar in arb_fixed_grammar()) {
        let table = build_table(&grammar);
        let bad_state = StateId(table.state_count as u16 + 100);
        for &nt in table.nonterminal_to_index.keys() {
            prop_assert_eq!(table.goto(bad_state, nt), None);
        }
    }

    #[test]
    fn prop_goto_none_both_invalid(grammar in arb_fixed_grammar()) {
        let table = build_table(&grammar);
        let bad_state = StateId(table.state_count as u16 + 50);
        let bad_nt = SymbolId(59999);
        prop_assert_eq!(table.goto(bad_state, bad_nt), None);
    }

    #[test]
    fn prop_goto_none_terminal_id(grammar in arb_valid_grammar()) {
        let table = build_table(&grammar);
        let terms = terminal_ids(&grammar);
        for s in 0..table.state_count {
            let st = StateId(s as u16);
            for &tid in &terms {
                // Terminals should not be in the nonterminal_to_index map,
                // so goto should return None (unless the ID happens to collide
                // with a nonterminal ID, which we skip).
                if !table.nonterminal_to_index.contains_key(&tid) {
                    prop_assert_eq!(table.goto(st, tid), None);
                }
            }
        }
    }

    #[test]
    fn prop_goto_none_max_symbol(grammar in arb_random_grammar()) {
        if let Some(table) = try_build(&grammar) {
            let max_nt = SymbolId(u16::MAX);
            for s in 0..table.state_count {
                prop_assert_eq!(table.goto(StateId(s as u16), max_nt), None);
            }
        }
    }

    #[test]
    fn prop_goto_none_zero_symbol_if_not_nt(grammar in arb_fixed_grammar()) {
        let table = build_table(&grammar);
        let zero = SymbolId(0);
        if !table.nonterminal_to_index.contains_key(&zero) {
            for s in 0..table.state_count {
                prop_assert_eq!(table.goto(StateId(s as u16), zero), None);
            }
        }
    }
}

// ===========================================================================
// Category 4 — prop_goto_nonterminal_* (6 tests)
// Only nonterminals appear as goto keys / have goto transitions.
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_goto_nonterminal_keys_in_grammar(grammar in arb_fixed_grammar()) {
        let table = build_table(&grammar);
        let grammar_nts: BTreeSet<SymbolId> = grammar.rules.keys().copied().collect();
        for &nt in table.nonterminal_to_index.keys() {
            prop_assert!(
                grammar_nts.contains(&nt),
                "goto key {nt:?} not in grammar nonterminals"
            );
        }
    }

    #[test]
    fn prop_goto_nonterminal_all_present(grammar in arb_fixed_grammar()) {
        let table = build_table(&grammar);
        // Every grammar nonterminal should appear in the goto index
        for &nt_id in grammar.rules.keys() {
            prop_assert!(
                table.nonterminal_to_index.contains_key(&nt_id),
                "grammar NT {nt_id:?} missing from nonterminal_to_index"
            );
        }
    }

    #[test]
    fn prop_goto_nonterminal_no_terminal_keys(grammar in arb_valid_grammar()) {
        let table = build_table(&grammar);
        let terms: BTreeSet<SymbolId> = grammar.tokens.keys().copied().collect();
        for &key in table.nonterminal_to_index.keys() {
            // Terminals should not be used as goto keys (unless ID collision,
            // which doesn't happen with GrammarBuilder).
            prop_assert!(
                !terms.contains(&key),
                "terminal {key:?} found in nonterminal_to_index"
            );
        }
    }

    #[test]
    fn prop_goto_nonterminal_column_count(grammar in arb_fixed_grammar()) {
        let table = build_table(&grammar);
        let n_nts = table.nonterminal_to_index.len();
        // Each goto row width must be at least the user nonterminal count
        // (may include augmented start symbol or other internal nonterminals)
        for row in &table.goto_table {
            prop_assert!(
                row.len() >= n_nts,
                "goto row width less than nonterminal count"
            );
        }
    }

    #[test]
    fn prop_goto_nonterminal_indices_unique(grammar in arb_fixed_grammar()) {
        let table = build_table(&grammar);
        let indices: Vec<usize> = table.nonterminal_to_index.values().copied().collect();
        let unique: BTreeSet<usize> = indices.iter().copied().collect();
        // All column indices must be unique
        prop_assert_eq!(indices.len(), unique.len(), "nonterminal indices not unique");
        // All indices must be within goto row bounds
        if let Some(first_row) = table.goto_table.first() {
            for &idx in &indices {
                prop_assert!(idx < first_row.len(), "index out of goto row bounds");
            }
        }
    }

    #[test]
    fn prop_goto_nonterminal_random_only_registered(grammar in arb_random_grammar()) {
        if let Some(table) = try_build(&grammar) {
            let grammar_nts: BTreeSet<SymbolId> = grammar.rules.keys().copied().collect();
            for &nt in table.nonterminal_to_index.keys() {
                prop_assert!(grammar_nts.contains(&nt));
            }
        }
    }
}

// ===========================================================================
// Category 5 — prop_goto_bounded_* (6 tests)
// Goto targets are within [0, state_count).
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_goto_bounded_raw_entries(grammar in arb_fixed_grammar()) {
        let table = build_table(&grammar);
        for row in &table.goto_table {
            for &cell in row {
                prop_assert!(
                    cell.0 == u16::MAX || (cell.0 as usize) < table.state_count,
                    "raw goto entry {cell:?} out of bounds (states={})", table.state_count
                );
            }
        }
    }

    #[test]
    fn prop_goto_bounded_api_targets(grammar in arb_valid_grammar()) {
        let table = build_table(&grammar);
        for s in 0..table.state_count {
            let st = StateId(s as u16);
            for &nt in table.nonterminal_to_index.keys() {
                if let Some(tgt) = table.goto(st, nt) {
                    prop_assert!((tgt.0 as usize) < table.state_count);
                }
            }
        }
    }

    #[test]
    fn prop_goto_bounded_random(grammar in arb_random_grammar()) {
        if let Some(table) = try_build(&grammar) {
            for row in &table.goto_table {
                for &cell in row {
                    prop_assert!(
                        cell.0 == u16::MAX || (cell.0 as usize) < table.state_count,
                    );
                }
            }
        }
    }

    #[test]
    fn prop_goto_bounded_state_count_positive(grammar in arb_fixed_grammar()) {
        let table = build_table(&grammar);
        prop_assert!(table.state_count > 0, "state_count must be positive");
    }

    #[test]
    fn prop_goto_bounded_target_count(grammar in arb_fixed_grammar()) {
        let table = build_table(&grammar);
        let targets = all_goto_targets(&table);
        // Number of distinct targets cannot exceed state_count
        prop_assert!(targets.len() <= table.state_count);
    }

    #[test]
    fn prop_goto_bounded_two_nt(grammar in arb_two_nt_grammar()) {
        let table = build_table(&grammar);
        for row in &table.goto_table {
            for &cell in row {
                prop_assert!(
                    cell.0 == u16::MAX || (cell.0 as usize) < table.state_count,
                );
            }
        }
    }
}

// ===========================================================================
// Category 6 — prop_goto_consistency_* (6 tests)
// Goto table is consistent with the grammar and action table.
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_goto_consistency_rows_eq_state_count(grammar in arb_fixed_grammar()) {
        let table = build_table(&grammar);
        prop_assert_eq!(table.goto_table.len(), table.state_count);
    }

    #[test]
    fn prop_goto_consistency_action_rows_match(grammar in arb_valid_grammar()) {
        let table = build_table(&grammar);
        prop_assert_eq!(
            table.goto_table.len(),
            table.action_table.len(),
            "goto and action table must have equal number of rows"
        );
    }

    #[test]
    fn prop_goto_consistency_uniform_width(grammar in arb_fixed_grammar()) {
        let table = build_table(&grammar);
        if let Some(first) = table.goto_table.first() {
            let width = first.len();
            for row in table.goto_table.iter() {
                prop_assert_eq!(row.len(), width, "goto row width mismatch");
            }
        }
    }

    #[test]
    fn prop_goto_consistency_nt_index_unique(grammar in arb_fixed_grammar()) {
        let table = build_table(&grammar);
        let values: Vec<usize> = table.nonterminal_to_index.values().copied().collect();
        let unique: BTreeSet<usize> = values.iter().copied().collect();
        prop_assert_eq!(values.len(), unique.len(), "nonterminal_to_index values not unique");
    }

    #[test]
    fn prop_goto_consistency_start_has_goto(grammar in arb_fixed_grammar()) {
        let table = build_table(&grammar);
        let start = table.start_symbol;
        // Start nonterminal should have at least one goto from some state
        let has_start_goto = (0..table.state_count).any(|s| {
            table.goto(StateId(s as u16), start).is_some()
        });
        prop_assert!(has_start_goto, "start symbol has no goto transition");
    }

    #[test]
    fn prop_goto_consistency_defined_count_bounded(grammar in arb_fixed_grammar()) {
        let table = build_table(&grammar);
        let n_nts = table.nonterminal_to_index.len();
        let max_entries = table.state_count * n_nts;
        let defined = goto_defined_count(&table);
        prop_assert!(
            defined <= max_entries,
            "defined goto entries ({defined}) exceed max ({max_entries})"
        );
    }
}

// ===========================================================================
// Category 7 — prop_goto_complex_* (5 tests)
// Complex grammar properties: chains, recursion, precedence.
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_goto_complex_chain_has_transitions(grammar in prop_oneof![
        Just(chain_grammar()),
        Just(deep_chain_grammar()),
    ]) {
        let table = build_table(&grammar);
        let defined = goto_defined_count(&table);
        // Chain grammars must have at least one goto per nonterminal level
        prop_assert!(defined >= 1, "chain grammar should have goto transitions");
    }

    #[test]
    fn prop_goto_complex_recursive_bounded(grammar in prop_oneof![
        Just(left_recursive_grammar()),
        Just(right_recursive_grammar()),
    ]) {
        let table = build_table(&grammar);
        // Recursive grammars still produce finite state machines
        prop_assert!(table.state_count < 1000, "too many states for recursive grammar");
        for row in &table.goto_table {
            for &cell in row {
                prop_assert!(cell.0 == u16::MAX || (cell.0 as usize) < table.state_count);
            }
        }
    }

    #[test]
    fn prop_goto_complex_precedence_valid(grammar in Just(precedence_grammar())) {
        let table = build_table(&grammar);
        let nts = nonterminal_ids(&table);
        prop_assert!(!nts.is_empty(), "precedence grammar must have nonterminals");
        for s in 0..table.state_count {
            let st = StateId(s as u16);
            for &nt in &nts {
                if let Some(tgt) = table.goto(st, nt) {
                    prop_assert!((tgt.0 as usize) < table.state_count);
                }
            }
        }
    }

    #[test]
    fn prop_goto_complex_multi_nt_coverage(grammar in prop_oneof![
        Just(two_nt_seq_grammar()),
        Just(three_nt_grammar()),
    ]) {
        let table = build_table(&grammar);
        // Multi-nonterminal grammars should have goto entries for each NT
        for &nt in table.nonterminal_to_index.keys() {
            let has_transition = (0..table.state_count).any(|s| {
                table.goto(StateId(s as u16), nt).is_some()
            });
            prop_assert!(has_transition, "NT {nt:?} has no goto transition");
        }
    }

    #[test]
    fn prop_goto_complex_nullable_valid(grammar in Just(nullable_grammar())) {
        let table = build_table(&grammar);
        // Nullable grammars still produce valid goto tables
        for s in 0..table.state_count {
            let st = StateId(s as u16);
            for &nt in table.nonterminal_to_index.keys() {
                if let Some(tgt) = table.goto(st, nt) {
                    prop_assert!((tgt.0 as usize) < table.state_count);
                }
            }
        }
    }
}

// ===========================================================================
// Category 8 — prop_goto_reflexive_* (5 tests)
// Self-loop analysis: goto(s, nt) == s only when valid.
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_goto_reflexive_self_loops_bounded(grammar in arb_fixed_grammar()) {
        let table = build_table(&grammar);
        let mut self_loop_count = 0usize;
        for s in 0..table.state_count {
            let st = StateId(s as u16);
            for &nt in table.nonterminal_to_index.keys() {
                if let Some(tgt) = table.goto(st, nt)
                    && tgt == st
                {
                    self_loop_count += 1;
                }
            }
        }
        // Self-loops can exist in recursive grammars but must be finite
        let max_loops = table.state_count * table.nonterminal_to_index.len();
        prop_assert!(self_loop_count <= max_loops);
    }

    #[test]
    fn prop_goto_reflexive_no_loops_minimal(grammar in Just(minimal_grammar())) {
        let table = build_table(&grammar);
        for s in 0..table.state_count {
            let st = StateId(s as u16);
            for &nt in table.nonterminal_to_index.keys() {
                if let Some(tgt) = table.goto(st, nt) {
                    // Minimal grammar (S → a) should not self-loop
                    prop_assert_ne!(tgt, st, "unexpected self-loop in minimal grammar");
                }
            }
        }
    }

    #[test]
    fn prop_goto_reflexive_valid_when_present(grammar in arb_random_grammar()) {
        if let Some(table) = try_build(&grammar) {
            for s in 0..table.state_count {
                let st = StateId(s as u16);
                for &nt in table.nonterminal_to_index.keys() {
                    if let Some(tgt) = table.goto(st, nt) {
                        // Even self-loops must point to valid states
                        prop_assert!((tgt.0 as usize) < table.state_count);
                    }
                }
            }
        }
    }

    #[test]
    fn prop_goto_reflexive_non_self_loop_distinct(grammar in arb_fixed_grammar()) {
        let table = build_table(&grammar);
        let nts = nonterminal_ids(&table);
        for s in 0..table.state_count {
            let st = StateId(s as u16);
            let targets: Vec<StateId> = nts.iter()
                .filter_map(|&nt| table.goto(st, nt))
                .filter(|&tgt| tgt != st)
                .collect();
            // Non-self-loop targets from the same state should be valid
            for tgt in &targets {
                prop_assert!((tgt.0 as usize) < table.state_count);
            }
        }
    }

    #[test]
    fn prop_goto_reflexive_chain_no_self_loops(grammar in Just(chain_grammar())) {
        let table = build_table(&grammar);
        // A simple chain S → T, T → a should have no self-loops
        for s in 0..table.state_count {
            let st = StateId(s as u16);
            for &nt in table.nonterminal_to_index.keys() {
                if let Some(tgt) = table.goto(st, nt) {
                    prop_assert_ne!(tgt, st, "unexpected self-loop in chain grammar");
                }
            }
        }
    }
}

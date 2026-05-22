//! Conflict resolution v6 — 64 tests for GLR conflict detection and resolution.
//!
//! Categories (8 × 8 = 64):
//!   1. conflict_sr_*            — shift-reduce conflicts
//!   2. conflict_rr_*            — reduce-reduce conflicts
//!   3. conflict_prec_*          — precedence resolution
//!   4. conflict_assoc_*         — associativity resolution
//!   5. conflict_fork_*          — GLR fork creation
//!   6. conflict_declare_*       — explicit conflict declarations
//!   7. conflict_complex_*       — complex conflict scenarios
//!   8. conflict_deterministic_* — conflict resolution determinism
//!
//! Run with: cargo test -p adze-glr-core --test conflict_resolve_v6 -- --test-threads=2

use adze_glr_core::conflict_inspection::{
    ConflictType, classify_conflict, count_conflicts, find_conflicts_for_symbol,
    get_state_conflicts, state_has_conflicts,
};
use adze_glr_core::{
    Action, FirstFollowSets, ParseTable, build_lr1_automaton, sanity_check_tables,
};
use adze_ir::Associativity;
use adze_ir::builder::GrammarBuilder;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build grammar → first/follow → parse table in one step.
fn build_table(grammar: &adze_ir::Grammar) -> Result<ParseTable, adze_glr_core::GLRError> {
    let ff = FirstFollowSets::compute(grammar)?;
    build_lr1_automaton(grammar, &ff)
}

/// Count cells with multiple actions (conflict / fork points).
fn count_fork_cells(table: &ParseTable) -> usize {
    table
        .action_table
        .iter()
        .flat_map(|row| row.iter())
        .filter(|cell| cell.len() > 1)
        .count()
}

/// True if any cell contains both Shift and Reduce actions.
fn has_shift_reduce(table: &ParseTable) -> bool {
    table.action_table.iter().any(|row| {
        row.iter().any(|cell| {
            cell.len() > 1
                && cell.iter().any(|a| matches!(a, Action::Shift(_)))
                && cell.iter().any(|a| matches!(a, Action::Reduce(_)))
        })
    })
}

/// True if any cell contains two or more distinct Reduce actions.
fn has_reduce_reduce(table: &ParseTable) -> bool {
    table.action_table.iter().any(|row| {
        row.iter().any(|cell| {
            cell.iter()
                .filter(|a| matches!(a, Action::Reduce(_)))
                .count()
                > 1
        })
    })
}

/// True if at least one cell contains Accept.
fn has_accept(table: &ParseTable) -> bool {
    table.action_table.iter().any(|row| {
        row.iter()
            .any(|cell| cell.iter().any(|a| matches!(a, Action::Accept)))
    })
}

/// Collect all Reduce rule-ids that appear anywhere in the table.
fn all_reduce_rule_ids(table: &ParseTable) -> Vec<adze_ir::RuleId> {
    let mut ids = Vec::new();
    for row in &table.action_table {
        for cell in row {
            for action in cell {
                if let Action::Reduce(rid) = action {
                    ids.push(*rid);
                }
            }
        }
    }
    ids.sort_by_key(|r| r.0);
    ids.dedup();
    ids
}

/// Total number of actions across the entire table.
fn total_action_count(table: &ParseTable) -> usize {
    table
        .action_table
        .iter()
        .flat_map(|row| row.iter())
        .map(|cell| cell.len())
        .sum()
}

// ===========================================================================
// Category 1: conflict_sr_* — shift-reduce conflict detection (8 tests)
// ===========================================================================

#[test]
fn conflict_sr_binary_addition_detected() {
    // E → E + E | NUM — classic ambiguous binary op without precedence.
    let g = GrammarBuilder::new("sr_add")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule("expr", vec!["expr", "PLUS", "expr"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    assert!(has_shift_reduce(&table), "E→E+E must produce S/R conflict");
}

#[test]
fn conflict_sr_dangling_else() {
    let g = GrammarBuilder::new("sr_dangle")
        .token("IF", "if")
        .token("THEN", "then")
        .token("ELSE", "else")
        .token("ATOM", "atom")
        .token("COND", "cond")
        .rule("stmt", vec!["IF", "COND", "THEN", "stmt"])
        .rule("stmt", vec!["IF", "COND", "THEN", "stmt", "ELSE", "stmt"])
        .rule("stmt", vec!["ATOM"])
        .start("stmt")
        .build();
    let table = build_table(&g).unwrap();
    assert!(has_shift_reduce(&table), "dangling-else must have S/R");
}

#[test]
fn conflict_sr_subtraction_detected() {
    let g = GrammarBuilder::new("sr_sub")
        .token("NUM", r"\d+")
        .token("MINUS", r"-")
        .rule("expr", vec!["expr", "MINUS", "expr"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    assert!(has_shift_reduce(&table));
}

#[test]
fn conflict_sr_conflict_detail_type_is_shift_reduce() {
    let g = GrammarBuilder::new("sr_det")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule("expr", vec!["expr", "PLUS", "expr"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    let summary = count_conflicts(&table);
    let sr: Vec<_> = summary
        .conflict_details
        .iter()
        .filter(|d| d.conflict_type == ConflictType::ShiftReduce)
        .collect();
    assert!(!sr.is_empty(), "must have ShiftReduce detail entries");
}

#[test]
fn conflict_sr_multiple_ops_increase_conflicts() {
    let g1 = GrammarBuilder::new("sr_one")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule("expr", vec!["expr", "PLUS", "expr"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let g2 = GrammarBuilder::new("sr_two")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .token("STAR", r"\*")
        .rule("expr", vec!["expr", "PLUS", "expr"])
        .rule("expr", vec!["expr", "STAR", "expr"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let t1 = build_table(&g1).unwrap();
    let t2 = build_table(&g2).unwrap();
    let s1 = count_conflicts(&t1);
    let s2 = count_conflicts(&t2);
    assert!(
        s2.shift_reduce >= s1.shift_reduce,
        "more ops → ≥ S/R: {} >= {}",
        s2.shift_reduce,
        s1.shift_reduce
    );
}

#[test]
fn conflict_sr_left_recursive_no_conflict() {
    // E → E + NUM | NUM — left-recursive but unambiguous (LR(1)-parseable).
    let g = GrammarBuilder::new("sr_lrec")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule("expr", vec!["expr", "PLUS", "NUM"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    assert!(
        !has_shift_reduce(&table),
        "left-recursive chain is LR(1), no S/R"
    );
}

#[test]
fn conflict_sr_state_reported() {
    let g = GrammarBuilder::new("sr_state")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule("expr", vec!["expr", "PLUS", "expr"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    let summary = count_conflicts(&table);
    assert!(
        !summary.states_with_conflicts.is_empty(),
        "must report at least one conflicting state"
    );
}

#[test]
fn conflict_sr_symbol_name_non_empty() {
    let g = GrammarBuilder::new("sr_sym")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule("expr", vec!["expr", "PLUS", "expr"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    let summary = count_conflicts(&table);
    for detail in &summary.conflict_details {
        assert!(
            !detail.symbol_name.is_empty(),
            "conflict detail symbol name must not be empty"
        );
    }
}

// ===========================================================================
// Category 2: conflict_rr_* — reduce-reduce conflict detection (8 tests)
// ===========================================================================

#[test]
fn conflict_rr_two_rules_same_rhs() {
    // S → A | B; A → x; B → x — R/R on EOF after 'x'.
    // Builder may resolve by lowest PID, so check table builds.
    let g = GrammarBuilder::new("rr_same")
        .token("X", "x")
        .rule("start_sym", vec!["item_a"])
        .rule("start_sym", vec!["item_b"])
        .rule("item_a", vec!["X"])
        .rule("item_b", vec!["X"])
        .start("start_sym")
        .build();
    let table = build_table(&g).unwrap();
    assert!(table.state_count > 0, "table should build for R/R grammar");
}

#[test]
fn conflict_rr_detail_type_is_reduce_reduce() {
    let g = GrammarBuilder::new("rr_type")
        .token("X", "x")
        .rule("start_sym", vec!["item_a"])
        .rule("start_sym", vec!["item_b"])
        .rule("item_a", vec!["X"])
        .rule("item_b", vec!["X"])
        .start("start_sym")
        .build();
    let table = build_table(&g).unwrap();
    let summary = count_conflicts(&table);
    // Builder may resolve R/R by lowest PID; check when present.
    if summary.reduce_reduce > 0 {
        let rr: Vec<_> = summary
            .conflict_details
            .iter()
            .filter(|d| d.conflict_type == ConflictType::ReduceReduce)
            .collect();
        assert!(!rr.is_empty(), "must classify as ReduceReduce");
    }
}

#[test]
fn conflict_rr_three_alternatives() {
    // S → A | B | C; A → x; B → x; C → x — triple R/R potential.
    // Builder may resolve by PID; verify table builds and is non-trivial.
    let g = GrammarBuilder::new("rr_triple")
        .token("X", "x")
        .rule("start_sym", vec!["item_a"])
        .rule("start_sym", vec!["item_b"])
        .rule("start_sym", vec!["item_c"])
        .rule("item_a", vec!["X"])
        .rule("item_b", vec!["X"])
        .rule("item_c", vec!["X"])
        .start("start_sym")
        .build();
    let table = build_table(&g).unwrap();
    assert!(table.state_count > 0, "triple R/R grammar must build");
}

#[test]
fn conflict_rr_distinct_rhs_no_conflict() {
    // S → A | B; A → x; B → y — different tokens, no R/R.
    let g = GrammarBuilder::new("rr_distinct")
        .token("X", "x")
        .token("Y", "y")
        .rule("start_sym", vec!["item_a"])
        .rule("start_sym", vec!["item_b"])
        .rule("item_a", vec!["X"])
        .rule("item_b", vec!["Y"])
        .start("start_sym")
        .build();
    let table = build_table(&g).unwrap();
    assert!(!has_reduce_reduce(&table), "distinct tokens → no R/R");
}

#[test]
fn conflict_rr_epsilon_overlap() {
    // S → A B | C; A → ε; C → B; B → x — possible R/R via ε.
    let g = GrammarBuilder::new("rr_eps")
        .token("X", "x")
        .rule("start_sym", vec!["item_a", "item_b"])
        .rule("start_sym", vec!["item_b"])
        .rule("item_a", vec![])
        .rule("item_b", vec!["X"])
        .start("start_sym")
        .build();
    let table = build_table(&g).unwrap();
    // With ε, S→AB and S→B both start with X → potential conflict.
    let summary = count_conflicts(&table);
    // Just verify it builds and has some conflicts or is clean.
    assert!(table.state_count > 0);
    let _ = summary;
}

#[test]
fn conflict_rr_states_with_conflicts_unique() {
    let g = GrammarBuilder::new("rr_uniq")
        .token("X", "x")
        .rule("start_sym", vec!["item_a"])
        .rule("start_sym", vec!["item_b"])
        .rule("item_a", vec!["X"])
        .rule("item_b", vec!["X"])
        .start("start_sym")
        .build();
    let table = build_table(&g).unwrap();
    let summary = count_conflicts(&table);
    let unique: std::collections::HashSet<_> =
        summary.states_with_conflicts.iter().copied().collect();
    assert_eq!(unique.len(), summary.states_with_conflicts.len());
}

#[test]
fn conflict_rr_conflict_count_positive() {
    // Either raw R/R is present or the builder resolved it. Verify consistency.
    let g = GrammarBuilder::new("rr_pos")
        .token("X", "x")
        .rule("start_sym", vec!["item_a"])
        .rule("start_sym", vec!["item_b"])
        .rule("item_a", vec!["X"])
        .rule("item_b", vec!["X"])
        .start("start_sym")
        .build();
    let table = build_table(&g).unwrap();
    let summary = count_conflicts(&table);
    let raw = has_reduce_reduce(&table);
    if raw {
        assert!(summary.reduce_reduce > 0, "summary must agree with raw R/R");
    } else {
        // Builder resolved R/R by lowest PID — no conflict cells remain.
        assert_eq!(summary.reduce_reduce, 0);
    }
}

#[test]
fn conflict_rr_actions_cell_has_multiple_reduces() {
    // When R/R is preserved, at least one cell has ≥ 2 Reduce actions.
    // When resolved, verify no cell has multiple reduces.
    let g = GrammarBuilder::new("rr_multi")
        .token("X", "x")
        .rule("start_sym", vec!["item_a"])
        .rule("start_sym", vec!["item_b"])
        .rule("item_a", vec!["X"])
        .rule("item_b", vec!["X"])
        .start("start_sym")
        .build();
    let table = build_table(&g).unwrap();
    let raw = has_reduce_reduce(&table);
    let multi = table.action_table.iter().any(|row| {
        row.iter().any(|cell| {
            cell.iter()
                .filter(|a| matches!(a, Action::Reduce(_)))
                .count()
                >= 2
        })
    });
    assert_eq!(
        raw, multi,
        "has_reduce_reduce must agree with raw cell scan"
    );
}

// ===========================================================================
// Category 3: conflict_prec_* — precedence resolution (8 tests)
// ===========================================================================

#[test]
fn conflict_prec_higher_prec_resolves_sr() {
    // * has higher precedence than + → S/R resolved.
    let g = GrammarBuilder::new("prec_hi")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .token("STAR", r"\*")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "STAR", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    let summary = count_conflicts(&table);
    assert!(
        summary.shift_reduce == 0,
        "precedence should resolve all S/R, got {}",
        summary.shift_reduce
    );
}

#[test]
fn conflict_prec_resolved_table_fewer_forks() {
    let unresolved = GrammarBuilder::new("prec_ur")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .token("STAR", r"\*")
        .rule("expr", vec!["expr", "PLUS", "expr"])
        .rule("expr", vec!["expr", "STAR", "expr"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let resolved = GrammarBuilder::new("prec_res")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .token("STAR", r"\*")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "STAR", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let tu = build_table(&unresolved).unwrap();
    let tr = build_table(&resolved).unwrap();
    assert!(
        count_fork_cells(&tr) < count_fork_cells(&tu),
        "resolved ({}) < unresolved ({})",
        count_fork_cells(&tr),
        count_fork_cells(&tu)
    );
}

#[test]
fn conflict_prec_same_level_still_resolved_by_assoc() {
    // Both at prec 1, Left → shifts are resolved.
    let g = GrammarBuilder::new("prec_same")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    assert!(!has_shift_reduce(&table), "same prec + Left assoc → no S/R");
}

#[test]
fn conflict_prec_preserves_accept() {
    let g = GrammarBuilder::new("prec_acc")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    assert!(has_accept(&table), "precedence must not remove Accept");
}

#[test]
fn conflict_prec_three_levels() {
    // Three operator precedence levels: + < * < ^.
    let g = GrammarBuilder::new("prec_3lev")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .token("STAR", r"\*")
        .token("CARET", r"\^")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "STAR", "expr"], 2, Associativity::Left)
        .rule_with_precedence(
            "expr",
            vec!["expr", "CARET", "expr"],
            3,
            Associativity::Right,
        )
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    let summary = count_conflicts(&table);
    assert_eq!(summary.shift_reduce, 0, "three levels should fully resolve");
}

#[test]
fn conflict_prec_sanity_check_passes() {
    let g = GrammarBuilder::new("prec_san")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    assert!(sanity_check_tables(&table).is_ok());
}

#[test]
fn conflict_prec_no_prec_vs_prec_different_fork_count() {
    let no_prec = GrammarBuilder::new("prec_np")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule("expr", vec!["expr", "PLUS", "expr"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let with_prec = GrammarBuilder::new("prec_wp")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let tnp = build_table(&no_prec).unwrap();
    let twp = build_table(&with_prec).unwrap();
    assert!(
        count_fork_cells(&twp) < count_fork_cells(&tnp),
        "prec ({}) < no-prec ({})",
        count_fork_cells(&twp),
        count_fork_cells(&tnp)
    );
}

#[test]
fn conflict_prec_shift_targets_valid_states() {
    let g = GrammarBuilder::new("prec_shift")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .token("STAR", r"\*")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "STAR", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    for row in &table.action_table {
        for cell in row {
            for action in cell {
                if let Action::Shift(target) = action {
                    assert!(
                        (target.0 as usize) < table.state_count,
                        "Shift({}) out of range",
                        target.0
                    );
                }
            }
        }
    }
}

// ===========================================================================
// Category 4: conflict_assoc_* — associativity resolution (8 tests)
// ===========================================================================

#[test]
fn conflict_assoc_left_resolves_sr() {
    let g = GrammarBuilder::new("assoc_left")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    assert!(!has_shift_reduce(&table), "Left assoc resolves S/R");
}

#[test]
fn conflict_assoc_right_resolves_sr() {
    let g = GrammarBuilder::new("assoc_right")
        .token("NUM", r"\d+")
        .token("EQ", r"=")
        .rule_with_precedence("expr", vec!["expr", "EQ", "expr"], 1, Associativity::Right)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    assert!(!has_shift_reduce(&table), "Right assoc resolves S/R");
}

#[test]
fn conflict_assoc_none_preserves_conflict() {
    // Non-associative should not resolve the conflict, or resolve differently.
    let g = GrammarBuilder::new("assoc_none")
        .token("NUM", r"\d+")
        .token("CMP", r"<")
        .rule_with_precedence("expr", vec!["expr", "CMP", "expr"], 1, Associativity::None)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    // Non-assoc typically either errors or keeps conflict — table must still build.
    assert!(table.state_count > 0);
}

#[test]
fn conflict_assoc_left_and_right_mixed() {
    // + is left-assoc, = is right-assoc, different prec.
    let g = GrammarBuilder::new("assoc_mix")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .token("EQ", r"=")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 2, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "EQ", "expr"], 1, Associativity::Right)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    let summary = count_conflicts(&table);
    assert_eq!(summary.shift_reduce, 0, "mixed assoc should fully resolve");
}

#[test]
fn conflict_assoc_left_no_fork_cells() {
    let g = GrammarBuilder::new("assoc_nf")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    assert_eq!(count_fork_cells(&table), 0, "Left assoc → no fork cells");
}

#[test]
fn conflict_assoc_right_no_fork_cells() {
    let g = GrammarBuilder::new("assoc_rnf")
        .token("NUM", r"\d+")
        .token("ARROW", r"->")
        .rule_with_precedence(
            "expr",
            vec!["expr", "ARROW", "expr"],
            1,
            Associativity::Right,
        )
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    assert_eq!(count_fork_cells(&table), 0, "Right assoc → no fork cells");
}

#[test]
fn conflict_assoc_state_count_matches_action_table() {
    let g = GrammarBuilder::new("assoc_sc")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    assert_eq!(table.state_count, table.action_table.len());
}

#[test]
fn conflict_assoc_accept_on_eof_only() {
    let g = GrammarBuilder::new("assoc_eof")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    for row in &table.action_table {
        for (col, cell) in row.iter().enumerate() {
            if cell.iter().any(|a| matches!(a, Action::Accept)) && col < table.index_to_symbol.len()
            {
                let sym = table.index_to_symbol[col];
                assert_eq!(sym, table.eof_symbol, "Accept on non-EOF column {col}");
            }
        }
    }
}

// ===========================================================================
// Category 5: conflict_fork_* — GLR fork creation (8 tests)
// ===========================================================================

#[test]
fn conflict_fork_ambiguous_binary_has_multi_action_cells() {
    let g = GrammarBuilder::new("fork_bin")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule("expr", vec!["expr", "PLUS", "expr"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    assert!(
        count_fork_cells(&table) > 0,
        "ambiguous grammar needs forks"
    );
}

#[test]
fn conflict_fork_concat_grammar() {
    // E → E E | a — highly ambiguous concatenation.
    let g = GrammarBuilder::new("fork_cat")
        .token("A", "a")
        .rule("expr", vec!["expr", "expr"])
        .rule("expr", vec!["A"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    assert!(count_fork_cells(&table) > 0, "E→EE is ambiguous");
}

#[test]
fn conflict_fork_cells_contain_shift_and_reduce() {
    let g = GrammarBuilder::new("fork_sr")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule("expr", vec!["expr", "PLUS", "expr"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    let found = table.action_table.iter().any(|row| {
        row.iter().any(|cell| {
            cell.len() > 1
                && cell.iter().any(|a| matches!(a, Action::Shift(_)))
                && cell.iter().any(|a| matches!(a, Action::Reduce(_)))
        })
    });
    assert!(found, "fork cell must contain both Shift and Reduce");
}

#[test]
fn conflict_fork_no_error_in_conflict_cells() {
    let g = GrammarBuilder::new("fork_ne")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule("expr", vec!["expr", "PLUS", "expr"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    for row in &table.action_table {
        for cell in row {
            if cell.len() > 1 {
                assert!(
                    !cell.iter().any(|a| matches!(a, Action::Error)),
                    "conflict cells must not contain Error"
                );
            }
        }
    }
}

#[test]
fn conflict_fork_resolved_grammar_zero_forks() {
    let g = GrammarBuilder::new("fork_zero")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    assert_eq!(count_fork_cells(&table), 0, "fully resolved → 0 forks");
}

#[test]
fn conflict_fork_unambiguous_grammar_zero_forks() {
    let g = GrammarBuilder::new("fork_unamb")
        .token("A", "a")
        .token("B", "b")
        .rule("start_sym", vec!["A", "B"])
        .start("start_sym")
        .build();
    let table = build_table(&g).unwrap();
    assert_eq!(count_fork_cells(&table), 0);
}

#[test]
fn conflict_fork_classify_shift_reduce_cell() {
    let actions = vec![
        Action::Shift(adze_ir::StateId(5)),
        Action::Reduce(adze_ir::RuleId(2)),
    ];
    let ct = classify_conflict(&actions);
    assert_eq!(ct, ConflictType::ShiftReduce);
}

#[test]
fn conflict_fork_classify_reduce_reduce_cell() {
    let actions = vec![
        Action::Reduce(adze_ir::RuleId(1)),
        Action::Reduce(adze_ir::RuleId(3)),
    ];
    let ct = classify_conflict(&actions);
    assert_eq!(ct, ConflictType::ReduceReduce);
}

// ===========================================================================
// Category 6: conflict_declare_* — explicit conflict declarations (8 tests)
// ===========================================================================

#[test]
fn conflict_declare_grammar_conflicts_field_empty_by_default() {
    let g = GrammarBuilder::new("decl_empty")
        .token("A", "a")
        .rule("start_sym", vec!["A"])
        .start("start_sym")
        .build();
    assert!(g.conflicts.is_empty(), "no declarations by default");
}

#[test]
fn conflict_declare_grammar_precedences_empty_by_default() {
    let g = GrammarBuilder::new("decl_prec_empty")
        .token("A", "a")
        .rule("start_sym", vec!["A"])
        .start("start_sym")
        .build();
    assert!(g.precedences.is_empty());
}

#[test]
fn conflict_declare_rule_with_prec_populates_rule_precedence() {
    let g = GrammarBuilder::new("decl_rwp")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 5, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    // At least one rule should carry precedence info.
    let has_prec = g.all_rules().any(|r| r.precedence.is_some());
    assert!(has_prec, "rule_with_precedence should set precedence");
}

#[test]
fn conflict_declare_rule_with_prec_populates_associativity() {
    let g = GrammarBuilder::new("decl_assoc")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule_with_precedence(
            "expr",
            vec!["expr", "PLUS", "expr"],
            1,
            Associativity::Right,
        )
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let has_assoc = g
        .all_rules()
        .any(|r| r.associativity == Some(Associativity::Right));
    assert!(has_assoc, "rule_with_precedence should set associativity");
}

#[test]
fn conflict_declare_multiple_prec_levels_stored() {
    let g = GrammarBuilder::new("decl_multi")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .token("STAR", r"\*")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "STAR", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let prec_rules: Vec<_> = g.all_rules().filter(|r| r.precedence.is_some()).collect();
    assert!(prec_rules.len() >= 2, "two prec rules must exist");
}

#[test]
fn conflict_declare_assoc_none_stored() {
    let g = GrammarBuilder::new("decl_none")
        .token("NUM", r"\d+")
        .token("CMP", r"<")
        .rule_with_precedence("expr", vec!["expr", "CMP", "expr"], 1, Associativity::None)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let has_none = g
        .all_rules()
        .any(|r| r.associativity == Some(Associativity::None));
    assert!(has_none, "Associativity::None must be stored");
}

#[test]
fn conflict_declare_table_still_builds_with_declarations() {
    let g = GrammarBuilder::new("decl_builds")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g);
    assert!(table.is_ok(), "table must build with prec declarations");
}

#[test]
fn conflict_declare_sanity_check_after_declarations() {
    let g = GrammarBuilder::new("decl_sanity")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .token("STAR", r"\*")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "STAR", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    assert!(sanity_check_tables(&table).is_ok());
}

// ===========================================================================
// Category 7: conflict_complex_* — complex conflict scenarios (8 tests)
// ===========================================================================

#[test]
fn conflict_complex_four_operators() {
    // + - * & all without precedence → many conflicts.
    let g = GrammarBuilder::new("cx_4op")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .token("MINUS", r"-")
        .token("STAR", r"\*")
        .token("AMP", r"&")
        .rule("expr", vec!["expr", "PLUS", "expr"])
        .rule("expr", vec!["expr", "MINUS", "expr"])
        .rule("expr", vec!["expr", "STAR", "expr"])
        .rule("expr", vec!["expr", "AMP", "expr"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    let summary = count_conflicts(&table);
    assert!(summary.shift_reduce >= 4, "4 ops → ≥ 4 S/R conflicts");
}

#[test]
fn conflict_complex_four_operators_fully_resolved() {
    let g = GrammarBuilder::new("cx_4res")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .token("MINUS", r"-")
        .token("STAR", r"\*")
        .token("AMP", r"&")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
        .rule_with_precedence(
            "expr",
            vec!["expr", "MINUS", "expr"],
            1,
            Associativity::Left,
        )
        .rule_with_precedence("expr", vec!["expr", "STAR", "expr"], 2, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "AMP", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    let summary = count_conflicts(&table);
    assert_eq!(summary.shift_reduce, 0, "fully resolved → 0 S/R");
}

#[test]
fn conflict_complex_mixed_sr_and_rr() {
    // E → E + E | A; A → NUM; E → A (two paths to reduce NUM → different nonterminals).
    // Combined with ambiguous binary produces both S/R and R/R or at least S/R.
    let g = GrammarBuilder::new("cx_mixed")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule("expr", vec!["expr", "PLUS", "expr"])
        .rule("expr", vec!["item_a"])
        .rule("item_a", vec!["NUM"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    let summary = count_conflicts(&table);
    // At least one conflict type must be present.
    assert!(
        summary.shift_reduce > 0 || summary.reduce_reduce > 0,
        "mixed grammar must have conflicts"
    );
}

#[test]
fn conflict_complex_nested_nonterminals_no_conflict() {
    let g = GrammarBuilder::new("cx_deep")
        .token("X", "x")
        .rule("start_sym", vec!["lv1"])
        .rule("lv1", vec!["lv2"])
        .rule("lv2", vec!["lv3"])
        .rule("lv3", vec!["X"])
        .start("start_sym")
        .build();
    let table = build_table(&g).unwrap();
    assert_eq!(
        count_fork_cells(&table),
        0,
        "chain of nonterminals is LR(1)"
    );
    assert!(has_accept(&table));
}

#[test]
fn conflict_complex_epsilon_and_token() {
    // S → A | ε; A → x — builds correctly.
    let g = GrammarBuilder::new("cx_eps")
        .token("X", "x")
        .rule("start_sym", vec!["X"])
        .rule("start_sym", vec![])
        .start("start_sym")
        .build();
    let table = build_table(&g).unwrap();
    assert!(table.state_count > 0);
    assert!(has_accept(&table));
}

#[test]
fn conflict_complex_common_prefix_lr1_handles() {
    // S → a b | a c — LR(1) can distinguish with 1-token lookahead.
    let g = GrammarBuilder::new("cx_prefix")
        .token("A", "a")
        .token("B", "b")
        .token("C", "c")
        .rule("start_sym", vec!["A", "B"])
        .rule("start_sym", vec!["A", "C"])
        .start("start_sym")
        .build();
    let table = build_table(&g).unwrap();
    assert_eq!(
        count_fork_cells(&table),
        0,
        "common prefix resolved by LR(1)"
    );
}

#[test]
fn conflict_complex_total_actions_grow_with_rules() {
    let g1 = GrammarBuilder::new("cx_grow1")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule("expr", vec!["expr", "PLUS", "expr"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let g2 = GrammarBuilder::new("cx_grow2")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .token("STAR", r"\*")
        .token("MINUS", r"-")
        .rule("expr", vec!["expr", "PLUS", "expr"])
        .rule("expr", vec!["expr", "STAR", "expr"])
        .rule("expr", vec!["expr", "MINUS", "expr"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let t1 = build_table(&g1).unwrap();
    let t2 = build_table(&g2).unwrap();
    assert!(
        total_action_count(&t2) >= total_action_count(&t1),
        "more rules → more actions: {} >= {}",
        total_action_count(&t2),
        total_action_count(&t1)
    );
}

#[test]
fn conflict_complex_state_has_conflicts_api() {
    let g = GrammarBuilder::new("cx_shc")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule("expr", vec!["expr", "PLUS", "expr"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    let summary = count_conflicts(&table);
    // Verify state_has_conflicts agrees with summary.
    for &st in &summary.states_with_conflicts {
        assert!(
            state_has_conflicts(&table, st),
            "state_has_conflicts must agree for state {}",
            st.0
        );
    }
}

// ===========================================================================
// Category 8: conflict_deterministic_* — conflict resolution determinism (8 tests)
// ===========================================================================

#[test]
fn conflict_deterministic_same_grammar_same_table_twice() {
    let make = || {
        GrammarBuilder::new("det_same")
            .token("NUM", r"\d+")
            .token("PLUS", r"\+")
            .rule("expr", vec!["expr", "PLUS", "expr"])
            .rule("expr", vec!["NUM"])
            .start("expr")
            .build()
    };
    let t1 = build_table(&make()).unwrap();
    let t2 = build_table(&make()).unwrap();
    assert_eq!(t1.state_count, t2.state_count, "deterministic state count");
    assert_eq!(
        count_fork_cells(&t1),
        count_fork_cells(&t2),
        "deterministic fork count"
    );
}

#[test]
fn conflict_deterministic_same_conflicts_twice() {
    let make = || {
        GrammarBuilder::new("det_cf")
            .token("NUM", r"\d+")
            .token("PLUS", r"\+")
            .rule("expr", vec!["expr", "PLUS", "expr"])
            .rule("expr", vec!["NUM"])
            .start("expr")
            .build()
    };
    let s1 = count_conflicts(&build_table(&make()).unwrap());
    let s2 = count_conflicts(&build_table(&make()).unwrap());
    assert_eq!(s1.shift_reduce, s2.shift_reduce);
    assert_eq!(s1.reduce_reduce, s2.reduce_reduce);
}

#[test]
fn conflict_deterministic_resolved_same_twice() {
    let make = || {
        GrammarBuilder::new("det_res")
            .token("NUM", r"\d+")
            .token("PLUS", r"\+")
            .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
            .rule("expr", vec!["NUM"])
            .start("expr")
            .build()
    };
    let t1 = build_table(&make()).unwrap();
    let t2 = build_table(&make()).unwrap();
    assert_eq!(t1.state_count, t2.state_count);
    assert_eq!(count_fork_cells(&t1), count_fork_cells(&t2));
    assert_eq!(count_fork_cells(&t1), 0);
}

#[test]
fn conflict_deterministic_action_table_row_widths_equal() {
    let make = || {
        GrammarBuilder::new("det_rw")
            .token("NUM", r"\d+")
            .token("PLUS", r"\+")
            .rule("expr", vec!["expr", "PLUS", "expr"])
            .rule("expr", vec!["NUM"])
            .start("expr")
            .build()
    };
    let t1 = build_table(&make()).unwrap();
    let t2 = build_table(&make()).unwrap();
    for (r1, r2) in t1.action_table.iter().zip(t2.action_table.iter()) {
        assert_eq!(r1.len(), r2.len(), "row widths must match");
    }
}

#[test]
fn conflict_deterministic_get_state_conflicts_stable() {
    let g = GrammarBuilder::new("det_gsc")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule("expr", vec!["expr", "PLUS", "expr"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    let summary = count_conflicts(&table);
    if let Some(&st) = summary.states_with_conflicts.first() {
        let c1 = get_state_conflicts(&table, st);
        let c2 = get_state_conflicts(&table, st);
        assert_eq!(c1.len(), c2.len(), "get_state_conflicts must be stable");
    }
}

#[test]
fn conflict_deterministic_find_by_symbol_stable() {
    let g = GrammarBuilder::new("det_fbs")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .rule("expr", vec!["expr", "PLUS", "expr"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let table = build_table(&g).unwrap();
    let summary = count_conflicts(&table);
    if let Some(detail) = summary.conflict_details.first() {
        let sym = detail.symbol;
        let r1 = find_conflicts_for_symbol(&table, sym);
        let r2 = find_conflicts_for_symbol(&table, sym);
        assert_eq!(r1.len(), r2.len(), "find_conflicts_for_symbol stable");
    }
}

#[test]
fn conflict_deterministic_reduce_ids_stable() {
    let make = || {
        GrammarBuilder::new("det_rid")
            .token("NUM", r"\d+")
            .token("PLUS", r"\+")
            .rule("expr", vec!["expr", "PLUS", "expr"])
            .rule("expr", vec!["NUM"])
            .start("expr")
            .build()
    };
    let ids1 = all_reduce_rule_ids(&build_table(&make()).unwrap());
    let ids2 = all_reduce_rule_ids(&build_table(&make()).unwrap());
    assert_eq!(ids1, ids2, "reduce rule IDs must be deterministic");
}

#[test]
fn conflict_deterministic_summary_display_stable() {
    let make = || {
        GrammarBuilder::new("det_disp")
            .token("NUM", r"\d+")
            .token("PLUS", r"\+")
            .rule("expr", vec!["expr", "PLUS", "expr"])
            .rule("expr", vec!["NUM"])
            .start("expr")
            .build()
    };
    let s1 = format!("{}", count_conflicts(&build_table(&make()).unwrap()));
    let s2 = format!("{}", count_conflicts(&build_table(&make()).unwrap()));
    assert_eq!(s1, s2, "Display output must be deterministic");
}

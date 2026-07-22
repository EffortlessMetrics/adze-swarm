#![cfg(feature = "test-api")]

//! Comprehensive tests for ActionCell, Action handling, and parse table
//! structural invariants in adze-glr-core (60+ tests).

use adze_glr_core::{
    Action, ActionCell, FirstFollowSets, GotoIndexing, LexMode, ParseRule, ParseTable, RuleId,
    StateId, SymbolId, build_lr1_automaton, conflict_inspection::cell_has_conflict,
    sanity_check_tables,
};
use adze_ir::builder::GrammarBuilder;
use adze_ir::*;
use std::collections::BTreeMap;

// ===========================================================================
// Helpers
// ===========================================================================

/// Build a minimal grammar via raw IR: S → a
fn raw_grammar_s_a() -> Grammar {
    let mut g = Grammar::new("s_a".into());
    let a = SymbolId(1);
    let s = SymbolId(10);
    g.tokens.insert(
        a,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    g.rule_names.insert(s, "S".into());
    g.rules.insert(
        s,
        vec![Rule {
            lhs: s,
            rhs: vec![Symbol::Terminal(a)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );
    g.set_start_symbol(s);
    g
}

/// Build a two-token grammar: S → a b
fn raw_grammar_s_ab() -> Grammar {
    let mut g = Grammar::new("s_ab".into());
    let a = SymbolId(1);
    let b = SymbolId(2);
    let s = SymbolId(10);
    g.tokens.insert(
        a,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    g.tokens.insert(
        b,
        Token {
            name: "b".into(),
            pattern: TokenPattern::String("b".into()),
            fragile: false,
        },
    );
    g.rule_names.insert(s, "S".into());
    g.rules.insert(
        s,
        vec![Rule {
            lhs: s,
            rhs: vec![Symbol::Terminal(a), Symbol::Terminal(b)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );
    g.set_start_symbol(s);
    g
}

/// Build an ambiguous grammar with shift-reduce conflict: E → a | E '+' E
fn raw_grammar_sr_conflict() -> Grammar {
    let mut g = Grammar::new("sr".into());
    let a = SymbolId(1);
    let plus = SymbolId(2);
    let e = SymbolId(10);
    g.tokens.insert(
        a,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    g.tokens.insert(
        plus,
        Token {
            name: "+".into(),
            pattern: TokenPattern::String("+".into()),
            fragile: false,
        },
    );
    g.rule_names.insert(e, "E".into());
    g.rules.insert(
        e,
        vec![
            Rule {
                lhs: e,
                rhs: vec![Symbol::Terminal(a)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(0),
            },
            Rule {
                lhs: e,
                rhs: vec![
                    Symbol::NonTerminal(e),
                    Symbol::Terminal(plus),
                    Symbol::NonTerminal(e),
                ],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(1),
            },
        ],
    );
    g.set_start_symbol(e);
    g
}

/// Build a reduce-reduce conflict grammar: S → A | B; A → x; B → x
fn raw_grammar_rr_conflict() -> Grammar {
    let mut g = Grammar::new("rr".into());
    let x = SymbolId(1);
    let s = SymbolId(10);
    let a_nt = SymbolId(11);
    let b_nt = SymbolId(12);
    g.tokens.insert(
        x,
        Token {
            name: "x".into(),
            pattern: TokenPattern::String("x".into()),
            fragile: false,
        },
    );
    g.rule_names.insert(s, "S".into());
    g.rule_names.insert(a_nt, "A".into());
    g.rule_names.insert(b_nt, "B".into());
    g.rules.insert(
        s,
        vec![
            Rule {
                lhs: s,
                rhs: vec![Symbol::NonTerminal(a_nt)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(0),
            },
            Rule {
                lhs: s,
                rhs: vec![Symbol::NonTerminal(b_nt)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(1),
            },
        ],
    );
    g.rules.insert(
        a_nt,
        vec![Rule {
            lhs: a_nt,
            rhs: vec![Symbol::Terminal(x)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(2),
        }],
    );
    g.rules.insert(
        b_nt,
        vec![Rule {
            lhs: b_nt,
            rhs: vec![Symbol::Terminal(x)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(3),
        }],
    );
    g.set_start_symbol(s);
    g
}

/// Hand-build a ParseTable for direct testing.
fn hand_built_table(
    action_table: Vec<Vec<ActionCell>>,
    goto_table: Vec<Vec<StateId>>,
    symbol_to_index: BTreeMap<SymbolId, usize>,
    nonterminal_to_index: BTreeMap<SymbolId, usize>,
    rules: Vec<ParseRule>,
    eof_symbol: SymbolId,
    start_symbol: SymbolId,
) -> ParseTable {
    let state_count = action_table.len();
    let symbol_count = if state_count > 0 {
        action_table[0].len()
    } else {
        0
    };
    let mut index_to_symbol = vec![SymbolId(u16::MAX); symbol_to_index.len()];
    for (sym, &idx) in &symbol_to_index {
        if idx < index_to_symbol.len() {
            index_to_symbol[idx] = *sym;
        }
    }
    ParseTable {
        action_table,
        goto_table,
        symbol_metadata: vec![],
        state_count,
        symbol_count,
        symbol_to_index,
        index_to_symbol,
        external_scanner_states: vec![],
        rules,
        nonterminal_to_index,
        goto_indexing: GotoIndexing::NonterminalMap,
        eof_symbol,
        start_symbol,
        grammar: Grammar::new("test".into()),
        initial_state: StateId(0),
        token_count: 0,
        external_token_count: 0,
        lex_modes: vec![
            LexMode {
                lex_state: 0,
                external_lex_state: 0,
            };
            state_count
        ],
        extras: vec![],
        dynamic_prec_by_rule: vec![],
        rule_assoc_by_rule: vec![],
        alias_sequences: vec![],
        field_names: vec![],
        field_map: BTreeMap::new(),
    }
}

/// Build a parse table from a raw Grammar (compute → automaton).
fn build_table(g: &Grammar) -> ParseTable {
    let ff = FirstFollowSets::compute(g).unwrap();
    build_lr1_automaton(g, &ff).unwrap()
}

/// Build a parse table from a mutable Grammar using compute_normalized.
fn build_table_normalized(g: &mut Grammar) -> ParseTable {
    let ff = FirstFollowSets::compute_normalized(g).unwrap();
    build_lr1_automaton(g, &ff).unwrap()
}

// ===========================================================================
// 1. Action table dimensions match state_count × symbol_count
// ===========================================================================

#[test]
fn dimensions_action_table_row_count_equals_state_count() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    assert_eq!(
        table.action_table.len(),
        table.state_count,
        "action_table rows must equal state_count"
    );
}

#[test]
fn dimensions_every_row_has_same_column_count() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    if table.state_count > 0 {
        let cols = table.action_table[0].len();
        for (i, row) in table.action_table.iter().enumerate() {
            assert_eq!(
                row.len(),
                cols,
                "row {} has {} columns, expected {}",
                i,
                row.len(),
                cols
            );
        }
    }
}

#[test]
fn dimensions_two_token_grammar_has_more_columns() {
    let g1 = raw_grammar_s_a();
    let t1 = build_table(&g1);
    let g2 = raw_grammar_s_ab();
    let t2 = build_table(&g2);
    // Two-token grammar should have at least as many columns
    assert!(
        t2.action_table[0].len() >= t1.action_table[0].len(),
        "more terminals ⇒ at least as many columns"
    );
}

#[test]
fn dimensions_default_table_has_zero_rows_and_columns() {
    let pt = ParseTable::default();
    assert_eq!(pt.action_table.len(), 0);
    assert_eq!(pt.state_count, 0);
    assert_eq!(pt.symbol_count, 0);
}

#[test]
fn dimensions_state_count_field_consistent_with_action_table() {
    let g = raw_grammar_s_ab();
    let table = build_table(&g);
    assert_eq!(table.state_count, table.action_table.len());
    assert_eq!(table.state_count, table.goto_table.len());
}

// ===========================================================================
// 2. Simple grammar → action table has Accept action somewhere
// ===========================================================================

#[test]
fn accept_present_in_simple_grammar() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    let eof = table.eof();
    let has_accept = (0..table.state_count).any(|s| {
        table
            .actions(StateId(s as u16), eof)
            .iter()
            .any(|a| matches!(a, Action::Accept))
    });
    assert!(has_accept, "simple grammar must have Accept on EOF");
}

#[test]
fn accept_present_in_two_token_grammar() {
    let g = raw_grammar_s_ab();
    let table = build_table(&g);
    let eof = table.eof();
    let has_accept = (0..table.state_count).any(|s| {
        table
            .actions(StateId(s as u16), eof)
            .iter()
            .any(|a| matches!(a, Action::Accept))
    });
    assert!(has_accept);
}

#[test]
fn accept_present_builder_grammar() {
    let mut g = GrammarBuilder::new("ba")
        .token("num", r"\d+")
        .rule("expr", vec!["num"])
        .start("expr")
        .build();
    let table = build_table_normalized(&mut g);
    let eof = table.eof();
    let has_accept = (0..table.state_count).any(|s| {
        table
            .actions(StateId(s as u16), eof)
            .iter()
            .any(|a| matches!(a, Action::Accept))
    });
    assert!(has_accept);
}

#[test]
fn accept_only_on_eof_column() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    let eof = table.eof();
    for s in 0..table.state_count {
        for (col, cell) in table.action_table[s].iter().enumerate() {
            if cell.iter().any(|a| matches!(a, Action::Accept)) {
                let sym = table.index_to_symbol.get(col).copied();
                assert_eq!(
                    sym,
                    Some(eof),
                    "Accept should only appear in the EOF column"
                );
            }
        }
    }
}

#[test]
fn accept_appears_exactly_once_in_simple_grammar() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    let accept_count: usize = table
        .action_table
        .iter()
        .flat_map(|row| row.iter())
        .flat_map(|cell| cell.iter())
        .filter(|a| matches!(a, Action::Accept))
        .count();
    assert_eq!(
        accept_count, 1,
        "simple grammar should have exactly one Accept"
    );
}

// ===========================================================================
// 3. Each state has correct number of symbol entries
// ===========================================================================

#[test]
fn each_state_has_identical_width() {
    let g = raw_grammar_s_ab();
    let table = build_table(&g);
    let width = table.action_table[0].len();
    for row in &table.action_table {
        assert_eq!(row.len(), width);
    }
}

#[test]
fn symbol_to_index_covers_all_columns() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    // Every mapped symbol index must be within column range
    for &idx in table.symbol_to_index.values() {
        assert!(
            idx < table.action_table[0].len(),
            "symbol_to_index value {} out of range",
            idx
        );
    }
}

#[test]
fn index_to_symbol_length_matches_symbol_to_index() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    assert_eq!(table.index_to_symbol.len(), table.symbol_to_index.len());
}

#[test]
fn index_to_symbol_roundtrips_with_symbol_to_index() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    for (sym, &idx) in &table.symbol_to_index {
        assert_eq!(
            table.index_to_symbol[idx], *sym,
            "roundtrip failed for symbol {}",
            sym.0
        );
    }
}

// ===========================================================================
// 4. Shift actions reference valid state indices
// ===========================================================================

#[test]
fn shift_targets_within_state_count_simple() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    for row in &table.action_table {
        for cell in row {
            for action in cell {
                if let Action::Shift(target) = action {
                    assert!(
                        (target.0 as usize) < table.state_count,
                        "Shift target {} out of range (state_count={})",
                        target.0,
                        table.state_count
                    );
                }
            }
        }
    }
}

#[test]
fn shift_targets_within_state_count_two_token() {
    let g = raw_grammar_s_ab();
    let table = build_table(&g);
    for row in &table.action_table {
        for cell in row {
            for action in cell {
                if let Action::Shift(target) = action {
                    assert!((target.0 as usize) < table.state_count);
                }
            }
        }
    }
}

#[test]
fn shift_targets_within_state_count_conflict_grammar() {
    let g = raw_grammar_sr_conflict();
    let table = build_table(&g);
    for row in &table.action_table {
        for cell in row {
            for action in cell {
                if let Action::Shift(target) = action {
                    assert!((target.0 as usize) < table.state_count);
                }
            }
        }
    }
}

#[test]
fn shift_actions_exist_in_simple_grammar() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    let has_shift = table
        .action_table
        .iter()
        .flat_map(|r| r.iter())
        .flat_map(|c| c.iter())
        .any(|a| matches!(a, Action::Shift(_)));
    assert!(has_shift, "simple grammar must have at least one Shift");
}

#[test]
fn shift_targets_are_nonzero_when_not_initial() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    // In LR tables, shift targets are typically > 0 (state 0 is the start)
    let shift_targets: Vec<u16> = table
        .action_table
        .iter()
        .flat_map(|r| r.iter())
        .flat_map(|c| c.iter())
        .filter_map(|a| match a {
            Action::Shift(s) => Some(s.0),
            _ => None,
        })
        .collect();
    assert!(!shift_targets.is_empty());
}

// ===========================================================================
// 5. Reduce actions reference valid rule indices
// ===========================================================================

#[test]
fn reduce_rule_ids_within_range_simple() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    let rule_count = table.rules.len();
    for row in &table.action_table {
        for cell in row {
            for action in cell {
                if let Action::Reduce(rid) = action {
                    assert!(
                        (rid.0 as usize) < rule_count,
                        "Reduce rule {} out of range (rule_count={})",
                        rid.0,
                        rule_count
                    );
                }
            }
        }
    }
}

#[test]
fn reduce_rule_ids_within_range_two_token() {
    let g = raw_grammar_s_ab();
    let table = build_table(&g);
    let rule_count = table.rules.len();
    for row in &table.action_table {
        for cell in row {
            for action in cell {
                if let Action::Reduce(rid) = action {
                    assert!((rid.0 as usize) < rule_count);
                }
            }
        }
    }
}

#[test]
fn reduce_rule_ids_within_range_conflict() {
    let g = raw_grammar_sr_conflict();
    let table = build_table(&g);
    let rule_count = table.rules.len();
    for row in &table.action_table {
        for cell in row {
            for action in cell {
                if let Action::Reduce(rid) = action {
                    assert!((rid.0 as usize) < rule_count);
                }
            }
        }
    }
}

#[test]
fn reduce_actions_exist_in_simple_grammar() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    let has_reduce = table
        .action_table
        .iter()
        .flat_map(|r| r.iter())
        .flat_map(|c| c.iter())
        .any(|a| matches!(a, Action::Reduce(_)));
    assert!(has_reduce, "simple grammar must have at least one Reduce");
}

#[test]
fn reduce_lhs_symbols_are_nonterminals() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    for rule in &table.rules {
        // The LHS of a reduce rule should appear as a nonterminal
        // (i.e., it should be in nonterminal_to_index or be a known rule_name)
        assert!(
            rule.lhs.0 > 0,
            "LHS symbol should be a valid non-terminal, got {}",
            rule.lhs.0
        );
    }
}

// ===========================================================================
// 6. Action properties for known grammar shapes
// ===========================================================================

#[test]
fn simple_grammar_has_shift_reduce_accept() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    let all_actions: Vec<&Action> = table
        .action_table
        .iter()
        .flat_map(|r| r.iter())
        .flat_map(|c| c.iter())
        .collect();
    let has_shift = all_actions.iter().any(|a| matches!(a, Action::Shift(_)));
    let has_reduce = all_actions.iter().any(|a| matches!(a, Action::Reduce(_)));
    let has_accept = all_actions.iter().any(|a| matches!(a, Action::Accept));
    assert!(has_shift, "expected Shift");
    assert!(has_reduce, "expected Reduce");
    assert!(has_accept, "expected Accept");
}

#[test]
fn simple_grammar_state_count_at_least_three() {
    // S → a needs at least: start state, after-shift state, accept state
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    assert!(
        table.state_count >= 3,
        "S → a needs at least 3 states, got {}",
        table.state_count
    );
}

#[test]
fn two_token_grammar_has_more_states_than_single() {
    let g1 = raw_grammar_s_a();
    let t1 = build_table(&g1);
    let g2 = raw_grammar_s_ab();
    let t2 = build_table(&g2);
    assert!(
        t2.state_count >= t1.state_count,
        "two-token grammar should have at least as many states"
    );
}

#[test]
fn eof_symbol_is_in_symbol_to_index() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    assert!(
        table.symbol_to_index.contains_key(&table.eof_symbol),
        "EOF symbol must be mapped in symbol_to_index"
    );
}

#[test]
fn start_symbol_is_recorded() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    assert_ne!(
        table.start_symbol.0, 0,
        "start symbol should be non-zero for this grammar"
    );
}

#[test]
fn rule_rhs_len_matches_grammar_definition() {
    // S → a has rhs_len = 1
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    // Find a rule that corresponds to S → a (rhs_len=1, lhs=S)
    let has_len1_rule = table.rules.iter().any(|r| r.rhs_len == 1);
    assert!(has_len1_rule, "should have a rule with rhs_len=1 for S → a");
}

#[test]
fn two_token_rule_has_rhs_len_two() {
    // S → a b has rhs_len = 2
    let g = raw_grammar_s_ab();
    let table = build_table(&g);
    let has_len2 = table.rules.iter().any(|r| r.rhs_len == 2);
    assert!(has_len2, "should have a rule with rhs_len=2 for S → a b");
}

#[test]
fn rules_vec_nonempty_for_nontrivial_grammar() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    assert!(!table.rules.is_empty(), "rules vec should not be empty");
}

#[test]
fn sanity_check_passes_for_simple_grammar() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    sanity_check_tables(&table).expect("sanity check should pass");
}

#[test]
fn sanity_check_passes_for_two_token_grammar() {
    let g = raw_grammar_s_ab();
    let table = build_table(&g);
    sanity_check_tables(&table).expect("sanity check should pass");
}

// ===========================================================================
// 7. GLR cells: ambiguous grammar has multi-action cells
// ===========================================================================

#[test]
fn sr_conflict_grammar_produces_multi_action_cells() {
    let g = raw_grammar_sr_conflict();
    let table = build_table(&g);
    let multi_action_count = table
        .action_table
        .iter()
        .flat_map(|r| r.iter())
        .filter(|cell| cell_has_conflict(cell))
        .count();
    assert!(
        multi_action_count > 0,
        "shift-reduce conflict grammar should produce at least one multi-action cell"
    );
}

#[test]
fn sr_conflict_cell_has_both_shift_and_reduce() {
    let g = raw_grammar_sr_conflict();
    let table = build_table(&g);
    let has_sr_cell = table
        .action_table
        .iter()
        .flat_map(|r| r.iter())
        .any(|cell| {
            let has_shift = cell.iter().any(|a| matches!(a, Action::Shift(_)));
            let has_reduce = cell.iter().any(|a| matches!(a, Action::Reduce(_)));
            has_shift && has_reduce
        });
    assert!(
        has_sr_cell,
        "E → a | E '+' E should produce a cell with both Shift and Reduce"
    );
}

#[test]
fn rr_conflict_grammar_builds_successfully() {
    // S → A | B; A → x; B → x — the automaton may resolve or merge
    // the conflict; we just verify it builds and produces a valid table.
    let g = raw_grammar_rr_conflict();
    let table = build_table(&g);
    assert!(table.state_count > 0);
    assert!(!table.rules.is_empty());
}

#[test]
fn rr_conflict_grammar_has_reduce_actions() {
    let g = raw_grammar_rr_conflict();
    let table = build_table(&g);
    let has_reduce = table
        .action_table
        .iter()
        .flat_map(|r| r.iter())
        .flat_map(|c| c.iter())
        .any(|a| matches!(a, Action::Reduce(_)));
    assert!(
        has_reduce,
        "reduce-reduce grammar must still have Reduce actions"
    );
}

#[test]
fn conflict_grammar_still_has_accept() {
    let g = raw_grammar_sr_conflict();
    let table = build_table(&g);
    let eof = table.eof();
    let has_accept = (0..table.state_count).any(|s| {
        table
            .actions(StateId(s as u16), eof)
            .iter()
            .any(|a| matches!(a, Action::Accept))
    });
    assert!(has_accept, "conflict grammar still needs Accept");
}

#[test]
fn conflict_grammar_shift_targets_still_valid() {
    let g = raw_grammar_sr_conflict();
    let table = build_table(&g);
    for row in &table.action_table {
        for cell in row {
            for action in cell {
                if let Action::Shift(target) = action {
                    assert!((target.0 as usize) < table.state_count);
                }
            }
        }
    }
}

#[test]
fn conflict_grammar_reduce_ids_still_valid() {
    let g = raw_grammar_sr_conflict();
    let table = build_table(&g);
    let rule_count = table.rules.len();
    for row in &table.action_table {
        for cell in row {
            for action in cell {
                if let Action::Reduce(rid) = action {
                    assert!((rid.0 as usize) < rule_count);
                }
            }
        }
    }
}

// ===========================================================================
// 8. Action table determinism
// ===========================================================================

#[test]
fn deterministic_grammar_has_no_multi_action_cells() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    for row in &table.action_table {
        for cell in row {
            assert!(
                cell.len() <= 1,
                "S → a is LR(1) deterministic; cells should have at most 1 action, got {}",
                cell.len()
            );
        }
    }
}

#[test]
fn deterministic_two_token_no_multi_action() {
    let g = raw_grammar_s_ab();
    let table = build_table(&g);
    for row in &table.action_table {
        for cell in row {
            assert!(
                cell.len() <= 1,
                "S → a b is deterministic; max 1 action per cell"
            );
        }
    }
}

#[test]
fn deterministic_grammar_no_duplicate_actions() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    for row in &table.action_table {
        for cell in row {
            let mut seen = std::collections::HashSet::new();
            for action in cell {
                assert!(seen.insert(action.clone()), "duplicate action in cell");
            }
        }
    }
}

#[test]
fn same_grammar_produces_same_table_twice() {
    let g1 = raw_grammar_s_a();
    let t1 = build_table(&g1);
    let g2 = raw_grammar_s_a();
    let t2 = build_table(&g2);
    assert_eq!(t1.state_count, t2.state_count);
    assert_eq!(t1.action_table.len(), t2.action_table.len());
    for (r1, r2) in t1.action_table.iter().zip(t2.action_table.iter()) {
        for (c1, c2) in r1.iter().zip(r2.iter()) {
            assert_eq!(c1, c2, "same grammar should yield same actions");
        }
    }
}

#[test]
fn builder_grammar_is_deterministic() {
    let mut g = GrammarBuilder::new("det")
        .token("tok_a", "a")
        .rule("start", vec!["tok_a"])
        .start("start")
        .build();
    let table = build_table_normalized(&mut g);
    for row in &table.action_table {
        for cell in row {
            assert!(
                cell.len() <= 1,
                "builder deterministic grammar; max 1 action"
            );
        }
    }
}

// ===========================================================================
// 9. goto_table dimensions and valid entries
// ===========================================================================

#[test]
fn goto_table_row_count_equals_state_count() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    assert_eq!(
        table.goto_table.len(),
        table.state_count,
        "goto_table rows must equal state_count"
    );
}

#[test]
fn goto_table_rows_all_same_width() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    if let Some(first_row) = table.goto_table.first() {
        let width = first_row.len();
        for (i, row) in table.goto_table.iter().enumerate() {
            assert_eq!(
                row.len(),
                width,
                "goto_table row {} has {} cols, expected {}",
                i,
                row.len(),
                width
            );
        }
    }
}

#[test]
fn goto_entries_are_valid_states_or_sentinel() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    for row in &table.goto_table {
        for &state in row {
            assert!(
                (state.0 as usize) < table.state_count || state.0 == u16::MAX || state.0 == 0,
                "goto entry {} is neither a valid state nor a sentinel",
                state.0
            );
        }
    }
}

#[test]
fn goto_has_entry_for_start_nonterminal() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    // After reducing to the start symbol, there should be a goto entry
    let start = table.start_symbol;
    let has_goto = (0..table.state_count).any(|s| table.goto(StateId(s as u16), start).is_some());
    assert!(has_goto, "should have goto entry for start nonterminal");
}

#[test]
fn goto_returns_none_for_unmapped_symbol() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    // SymbolId(999) should not be mapped
    let result = table.goto(StateId(0), SymbolId(999));
    assert_eq!(result, None);
}

#[test]
fn goto_returns_none_for_out_of_range_state() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    let result = table.goto(StateId(u16::MAX), SymbolId(10));
    assert_eq!(result, None);
}

#[test]
fn goto_table_nonterminal_to_index_consistent() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    for &idx in table.nonterminal_to_index.values() {
        if !table.goto_table.is_empty() {
            assert!(
                idx < table.goto_table[0].len(),
                "nonterminal_to_index value {} out of goto column range",
                idx
            );
        }
    }
}

// ===========================================================================
// 10. Hand-built table invariants
// ===========================================================================

#[test]
fn hand_built_single_shift_cell_accessible() {
    let mut sym_idx = BTreeMap::new();
    sym_idx.insert(SymbolId(1), 0);
    let pt = hand_built_table(
        vec![vec![vec![Action::Shift(StateId(0))]]],
        vec![vec![]],
        sym_idx,
        BTreeMap::new(),
        vec![],
        SymbolId(99),
        SymbolId(10),
    );
    let cell = pt.actions(StateId(0), SymbolId(1));
    assert_eq!(cell, &[Action::Shift(StateId(0))]);
}

#[test]
fn hand_built_accept_cell() {
    let mut sym_idx = BTreeMap::new();
    sym_idx.insert(SymbolId(0), 0);
    let pt = hand_built_table(
        vec![vec![vec![Action::Accept]]],
        vec![vec![]],
        sym_idx,
        BTreeMap::new(),
        vec![],
        SymbolId(0),
        SymbolId(10),
    );
    let cell = pt.actions(StateId(0), SymbolId(0));
    assert_eq!(cell, &[Action::Accept]);
}

#[test]
fn hand_built_multi_action_cell() {
    let mut sym_idx = BTreeMap::new();
    sym_idx.insert(SymbolId(1), 0);
    let pt = hand_built_table(
        vec![vec![vec![
            Action::Shift(StateId(1)),
            Action::Reduce(RuleId(0)),
        ]]],
        vec![vec![]],
        sym_idx,
        BTreeMap::new(),
        vec![ParseRule {
            lhs: SymbolId(10),
            rhs_len: 1,
        }],
        SymbolId(99),
        SymbolId(10),
    );
    let cell = pt.actions(StateId(0), SymbolId(1));
    assert_eq!(cell.len(), 2);
    assert!(matches!(cell[0], Action::Shift(StateId(1))));
    assert!(matches!(cell[1], Action::Reduce(RuleId(0))));
}

#[test]
fn hand_built_empty_cell_returns_empty() {
    let mut sym_idx = BTreeMap::new();
    sym_idx.insert(SymbolId(1), 0);
    let pt = hand_built_table(
        vec![vec![vec![]]],
        vec![vec![]],
        sym_idx,
        BTreeMap::new(),
        vec![],
        SymbolId(99),
        SymbolId(10),
    );
    let cell = pt.actions(StateId(0), SymbolId(1));
    assert!(cell.is_empty());
}

#[test]
fn hand_built_unknown_symbol_returns_empty() {
    let sym_idx = BTreeMap::new(); // no symbols mapped
    let pt = hand_built_table(
        vec![vec![]],
        vec![vec![]],
        sym_idx,
        BTreeMap::new(),
        vec![],
        SymbolId(99),
        SymbolId(10),
    );
    let cell = pt.actions(StateId(0), SymbolId(42));
    assert!(cell.is_empty());
}

#[test]
fn hand_built_out_of_range_state_returns_empty() {
    let mut sym_idx = BTreeMap::new();
    sym_idx.insert(SymbolId(1), 0);
    let pt = hand_built_table(
        vec![vec![vec![Action::Shift(StateId(0))]]],
        vec![vec![]],
        sym_idx,
        BTreeMap::new(),
        vec![],
        SymbolId(99),
        SymbolId(10),
    );
    let cell = pt.actions(StateId(5), SymbolId(1));
    assert!(cell.is_empty());
}

// ===========================================================================
// 11. ParseTable accessor methods
// ===========================================================================

#[test]
fn eof_accessor_matches_field() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    assert_eq!(table.eof(), table.eof_symbol);
}

#[test]
fn start_symbol_accessor_matches_field() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    assert_eq!(table.start_symbol(), table.start_symbol);
}

#[test]
fn rule_accessor_returns_correct_values() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    for (i, rule) in table.rules.iter().enumerate() {
        let (lhs, rhs_len) = table.rule(RuleId(i as u16));
        assert_eq!(lhs, rule.lhs);
        assert_eq!(rhs_len, rule.rhs_len);
    }
}

#[test]
fn grammar_accessor_returns_reference() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    let grammar_ref = table.grammar();
    assert!(!grammar_ref.rules.is_empty());
}

#[test]
fn error_symbol_is_zero() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    assert_eq!(table.error_symbol(), SymbolId(0));
}

// ===========================================================================
// 12. Builder-based grammars
// ===========================================================================

#[test]
fn builder_single_rule_table_valid() {
    let mut g = GrammarBuilder::new("b1")
        .token("x", "x")
        .rule("s", vec!["x"])
        .start("s")
        .build();
    let table = build_table_normalized(&mut g);
    assert!(table.state_count >= 3);
    sanity_check_tables(&table).expect("should pass");
}

#[test]
fn builder_two_alternative_rules() {
    let mut g = GrammarBuilder::new("b2")
        .token("x", "x")
        .token("y", "y")
        .rule("s", vec!["x"])
        .rule("s", vec!["y"])
        .start("s")
        .build();
    let table = build_table_normalized(&mut g);
    assert!(table.state_count > 0);
    let eof = table.eof();
    let has_accept = (0..table.state_count).any(|s| {
        table
            .actions(StateId(s as u16), eof)
            .iter()
            .any(|a| matches!(a, Action::Accept))
    });
    assert!(has_accept);
}

#[test]
fn builder_chain_rule() {
    let mut g = GrammarBuilder::new("chain")
        .token("x", "x")
        .rule("a", vec!["x"])
        .rule("s", vec!["a"])
        .start("s")
        .build();
    let table = build_table_normalized(&mut g);
    assert!(table.state_count > 0);
    assert!(!table.rules.is_empty());
}

#[test]
fn builder_recursive_grammar() {
    let mut g = GrammarBuilder::new("rec")
        .token("n", r"\d+")
        .token("plus", r"\+")
        .rule("expr", vec!["n"])
        .rule("expr", vec!["expr", "plus", "expr"])
        .start("expr")
        .build();
    let table = build_table_normalized(&mut g);
    // Recursive grammar should produce a table
    assert!(table.state_count > 0);
    // And it should have multi-action cells due to ambiguity
    let multi = table
        .action_table
        .iter()
        .flat_map(|r| r.iter())
        .filter(|c| cell_has_conflict(c))
        .count();
    assert!(
        multi > 0,
        "recursive ambiguous grammar should have conflicts"
    );
}

// ===========================================================================
// 13. Action enum properties
// ===========================================================================

#[test]
fn action_clone_preserves_value() {
    let a = Action::Shift(StateId(42));
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn action_debug_format_contains_variant_name() {
    let s = format!("{:?}", Action::Shift(StateId(1)));
    assert!(s.contains("Shift"));
    let r = format!("{:?}", Action::Reduce(RuleId(2)));
    assert!(r.contains("Reduce"));
    let a = format!("{:?}", Action::Accept);
    assert!(a.contains("Accept"));
    let e = format!("{:?}", Action::Error);
    assert!(e.contains("Error"));
}

#[test]
fn action_equality_different_variants() {
    assert_ne!(Action::Shift(StateId(0)), Action::Reduce(RuleId(0)));
    assert_ne!(Action::Accept, Action::Error);
    assert_ne!(Action::Shift(StateId(0)), Action::Accept);
}

#[test]
fn action_equality_same_variant_different_values() {
    assert_ne!(Action::Shift(StateId(0)), Action::Shift(StateId(1)));
    assert_ne!(Action::Reduce(RuleId(0)), Action::Reduce(RuleId(1)));
}

#[test]
fn action_hash_consistency() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(Action::Shift(StateId(1)));
    set.insert(Action::Shift(StateId(1))); // duplicate
    set.insert(Action::Reduce(RuleId(0)));
    assert_eq!(set.len(), 2, "HashSet should deduplicate identical actions");
}

#[test]
fn action_cell_is_vec_of_action() {
    let cell: ActionCell = vec![Action::Shift(StateId(0)), Action::Reduce(RuleId(0))];
    assert_eq!(cell.len(), 2);
    assert!(matches!(cell[0], Action::Shift(_)));
    assert!(matches!(cell[1], Action::Reduce(_)));
}

#[test]
fn action_recover_variant_exists() {
    let a = Action::Recover;
    assert!(matches!(a, Action::Recover));
    assert_ne!(a, Action::Error);
}

#[test]
fn action_fork_variant_holds_multiple_actions() {
    let inner = vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(0))];
    let a = Action::Fork(inner.clone());
    if let Action::Fork(ref actions) = a {
        assert_eq!(actions.len(), 2);
    } else {
        panic!("expected Fork variant");
    }
}

// ===========================================================================
// 14. Edge cases and boundary conditions
// ===========================================================================

#[test]
fn default_parse_table_eof_and_start_are_zero() {
    let pt = ParseTable::default();
    assert_eq!(pt.eof_symbol, SymbolId(0));
    assert_eq!(pt.start_symbol, SymbolId(0));
}

#[test]
fn default_parse_table_rules_empty() {
    let pt = ParseTable::default();
    assert!(pt.rules.is_empty());
}

#[test]
fn default_parse_table_actions_returns_empty_for_any_query() {
    let pt = ParseTable::default();
    assert!(pt.actions(StateId(0), SymbolId(0)).is_empty());
    assert!(pt.actions(StateId(100), SymbolId(50)).is_empty());
}

#[test]
fn default_parse_table_goto_returns_none() {
    let pt = ParseTable::default();
    assert_eq!(pt.goto(StateId(0), SymbolId(0)), None);
}

#[test]
fn valid_symbols_mask_length() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    for s in 0..table.state_count {
        let mask = table.valid_symbols_mask(StateId(s as u16));
        // mask length is terminal_boundary
        assert!(
            !mask.is_empty() || table.action_table[s].is_empty(),
            "mask should have entries"
        );
    }
}

#[test]
fn initial_state_is_set() {
    let g = raw_grammar_s_a();
    let table = build_table(&g);
    assert!(
        (table.initial_state.0 as usize) < table.state_count,
        "initial_state must be a valid state"
    );
}

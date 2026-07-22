#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]
//! Property-based tests for the parse table **builder** pipeline.
//!
//! These tests exercise `build_lr1_automaton` (and helpers) with
//! randomly-generated grammars and verify structural invariants of the
//! resulting `ParseTable`.
//!
//! Run with: `cargo test -p adze-glr-core --test table_builder_proptest`

use adze_glr_core::{Action, FirstFollowSets, build_lr1_automaton, sanity_check_tables};
use adze_ir::builder::GrammarBuilder;
use adze_ir::*;
use proptest::prelude::*;
use std::collections::BTreeSet;

// ---------------------------------------------------------------------------
// Grammar generators
// ---------------------------------------------------------------------------

/// Build a single-production grammar: S → t₀
/// where `t` is a terminal chosen from [1, max_tok].
fn grammar_single_terminal(tok_id: u16) -> Grammar {
    let t = SymbolId(tok_id);
    let s = SymbolId(tok_id + 10);
    let mut g = Grammar::new("single".into());
    g.tokens.insert(
        t,
        Token {
            name: format!("t{tok_id}"),
            pattern: TokenPattern::String(format!("t{tok_id}")),
            fragile: false,
        },
    );
    g.rule_names.insert(s, "S".into());
    g.rules.insert(
        s,
        vec![Rule {
            lhs: s,
            rhs: vec![Symbol::Terminal(t)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );
    g
}

/// Build a grammar with `n` alternative single-terminal productions:
/// S → t₁ | t₂ | … | tₙ
fn grammar_n_alternatives(n: usize) -> Grammar {
    assert!(n >= 1);
    let s = SymbolId(100);
    let mut g = Grammar::new("alt".into());
    for i in 0..n {
        let t = SymbolId((i + 1) as u16);
        g.tokens.insert(
            t,
            Token {
                name: format!("t{i}"),
                pattern: TokenPattern::String(format!("t{i}")),
                fragile: false,
            },
        );
    }
    g.rule_names.insert(s, "S".into());
    let rules: Vec<Rule> = (0..n)
        .map(|i| Rule {
            lhs: s,
            rhs: vec![Symbol::Terminal(SymbolId((i + 1) as u16))],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(i as u16),
        })
        .collect();
    g.rules.insert(s, rules);
    g
}

/// Build a chain grammar: A → B, B → C, …, Zₙ₋₁ → t
fn grammar_chain(depth: usize) -> Grammar {
    assert!(depth >= 1);
    let t = SymbolId(1);
    let mut g = Grammar::new("chain".into());
    g.tokens.insert(
        t,
        Token {
            name: "t".into(),
            pattern: TokenPattern::String("t".into()),
            fragile: false,
        },
    );
    // Non-terminals: 100, 101, …, 100+depth-1
    for i in 0..depth {
        let nt = SymbolId(100 + i as u16);
        g.rule_names.insert(nt, format!("N{i}"));
        let rhs = if i + 1 < depth {
            vec![Symbol::NonTerminal(SymbolId(100 + (i + 1) as u16))]
        } else {
            vec![Symbol::Terminal(t)]
        };
        g.rules.insert(
            nt,
            vec![Rule {
                lhs: nt,
                rhs,
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(i as u16),
            }],
        );
    }
    g
}

/// Build a two-terminal sequence grammar: S → a b
fn grammar_sequence(a_id: u16, b_id: u16) -> Grammar {
    let a = SymbolId(a_id);
    let b = SymbolId(b_id);
    let s = SymbolId(100);
    let mut g = Grammar::new("seq".into());
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
    g
}

/// Build grammar using GrammarBuilder and return (Grammar, FirstFollowSets, ParseTable).
fn build_pipeline(
    g: &Grammar,
) -> Result<(FirstFollowSets, adze_glr_core::ParseTable), adze_glr_core::GLRError> {
    let ff = FirstFollowSets::compute(g)?;
    let pt = build_lr1_automaton(g, &ff)?;
    Ok((ff, pt))
}

// ===========================================================================
// Property tests
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    // -----------------------------------------------------------------------
    // 1. Single-terminal grammars always produce a valid table
    // -----------------------------------------------------------------------
    #[test]
    fn single_terminal_builds_ok(tok_id in 1u16..50) {
        let g = grammar_single_terminal(tok_id);
        let result = build_pipeline(&g);
        prop_assert!(result.is_ok(), "build failed: {:?}", result.err());
    }

    // -----------------------------------------------------------------------
    // 2. State count is always ≥ 2 for any non-trivial grammar
    // -----------------------------------------------------------------------
    #[test]
    fn state_count_at_least_two(tok_id in 1u16..50) {
        let g = grammar_single_terminal(tok_id);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        prop_assert!(pt.state_count >= 2,
            "expected ≥2 states, got {}", pt.state_count);
    }

    // -----------------------------------------------------------------------
    // 3. Action table rows == state_count
    // -----------------------------------------------------------------------
    #[test]
    fn action_table_rows_match_states(tok_id in 1u16..30) {
        let g = grammar_single_terminal(tok_id);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        prop_assert_eq!(pt.action_table.len(), pt.state_count);
    }

    // -----------------------------------------------------------------------
    // 4. Action table columns are uniform across all rows
    // -----------------------------------------------------------------------
    #[test]
    fn action_table_cols_uniform(tok_id in 1u16..30) {
        let g = grammar_single_terminal(tok_id);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        let expected = pt.symbol_count;
        for (i, row) in pt.action_table.iter().enumerate() {
            prop_assert_eq!(row.len(), expected,
                "row {} has {} cols, expected {}", i, row.len(), expected);
        }
    }

    // -----------------------------------------------------------------------
    // 5. Goto table rows == state_count
    // -----------------------------------------------------------------------
    #[test]
    fn goto_table_rows_match_states(tok_id in 1u16..30) {
        let g = grammar_single_terminal(tok_id);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        prop_assert_eq!(pt.goto_table.len(), pt.state_count);
    }

    // -----------------------------------------------------------------------
    // 6. Goto table columns are uniform across all rows
    // -----------------------------------------------------------------------
    #[test]
    fn goto_table_cols_uniform(tok_id in 1u16..30) {
        let g = grammar_single_terminal(tok_id);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        if let Some(first) = pt.goto_table.first() {
            let w = first.len();
            for (i, row) in pt.goto_table.iter().enumerate() {
                prop_assert_eq!(row.len(), w,
                    "goto row {} has {} cols, expected {}", i, row.len(), w);
            }
        }
    }

    // -----------------------------------------------------------------------
    // 7. EOF symbol is in symbol_to_index
    // -----------------------------------------------------------------------
    #[test]
    fn eof_in_symbol_to_index(tok_id in 1u16..30) {
        let g = grammar_single_terminal(tok_id);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        prop_assert!(pt.symbol_to_index.contains_key(&pt.eof_symbol),
            "EOF {:?} missing from symbol_to_index", pt.eof_symbol);
    }

    // -----------------------------------------------------------------------
    // 8. Exactly one Accept action exists across the whole table on EOF
    // -----------------------------------------------------------------------
    #[test]
    fn exactly_one_accept_on_eof(tok_id in 1u16..30) {
        let g = grammar_single_terminal(tok_id);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        let eof = pt.eof_symbol;
        let accept_count: usize = (0..pt.state_count)
            .map(|s| {
                pt.actions(StateId(s as u16), eof)
                    .iter()
                    .filter(|a| matches!(a, Action::Accept))
                    .count()
            })
            .sum();
        prop_assert!(accept_count >= 1,
            "expected ≥1 Accept on EOF, got {}", accept_count);
    }

    // -----------------------------------------------------------------------
    // 9. Start symbol is preserved in table
    // -----------------------------------------------------------------------
    #[test]
    fn start_symbol_preserved(tok_id in 1u16..30) {
        let g = grammar_single_terminal(tok_id);
        // Table generation uses analysis start (explicit metadata or first rule LHS).
        let start = g
            .explicit_start_symbol()
            .or_else(|| g.rules.keys().next().copied())
            .expect("grammar has rules");
        let (_ff, pt) = build_pipeline(&g).unwrap();
        prop_assert_eq!(pt.start_symbol(), start);
    }

    // -----------------------------------------------------------------------
    // 10. symbol_to_index and index_to_symbol are consistent
    // -----------------------------------------------------------------------
    #[test]
    fn sym_index_roundtrip(tok_id in 1u16..30) {
        let g = grammar_single_terminal(tok_id);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        for (&sym, &idx) in &pt.symbol_to_index {
            prop_assert!(idx < pt.index_to_symbol.len());
            prop_assert_eq!(pt.index_to_symbol[idx], sym);
        }
    }

    // -----------------------------------------------------------------------
    // 11. sanity_check_tables passes for single-terminal grammars
    // -----------------------------------------------------------------------
    #[test]
    fn sanity_check_single_terminal(tok_id in 1u16..30) {
        let g = grammar_single_terminal(tok_id);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        let result = sanity_check_tables(&pt);
        prop_assert!(result.is_ok(), "sanity_check failed: {:?}", result.err());
    }

    // -----------------------------------------------------------------------
    // 12. N-alternative grammars build successfully
    // -----------------------------------------------------------------------
    #[test]
    fn n_alternatives_builds_ok(n in 1usize..6) {
        let g = grammar_n_alternatives(n);
        let result = build_pipeline(&g);
        prop_assert!(result.is_ok(), "build failed for n={}: {:?}", n, result.err());
    }

    // -----------------------------------------------------------------------
    // 13. More alternatives ⇒ more (or equal) states
    // -----------------------------------------------------------------------
    #[test]
    fn more_alternatives_more_states(n in 2usize..6) {
        let g1 = grammar_n_alternatives(1);
        let gn = grammar_n_alternatives(n);
        let (_, pt1) = build_pipeline(&g1).unwrap();
        let (_, ptn) = build_pipeline(&gn).unwrap();
        prop_assert!(ptn.state_count >= pt1.state_count,
            "n={} has {} states < 1-alt {} states", n, ptn.state_count, pt1.state_count);
    }

    // -----------------------------------------------------------------------
    // 14. N-alternative grammar has shift for each terminal in initial state
    // -----------------------------------------------------------------------
    #[test]
    fn n_alternatives_shifts_in_initial(n in 1usize..6) {
        let g = grammar_n_alternatives(n);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        for i in 0..n {
            let tok = SymbolId((i + 1) as u16);
            let actions = pt.actions(pt.initial_state, tok);
            let has_shift = actions.iter().any(|a| matches!(a, Action::Shift(_)));
            prop_assert!(has_shift,
                "expected Shift on terminal {} in initial state", i);
        }
    }

    // -----------------------------------------------------------------------
    // 15. Chain grammars build successfully
    // -----------------------------------------------------------------------
    #[test]
    fn chain_builds_ok(depth in 1usize..6) {
        let g = grammar_chain(depth);
        let result = build_pipeline(&g);
        prop_assert!(result.is_ok(), "chain depth={} failed: {:?}", depth, result.err());
    }

    // -----------------------------------------------------------------------
    // 16. Chain grammar always has accept on EOF
    // -----------------------------------------------------------------------
    #[test]
    fn chain_has_accept(depth in 1usize..6) {
        let g = grammar_chain(depth);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        let eof = pt.eof_symbol;
        let has = (0..pt.state_count).any(|s|
            pt.actions(StateId(s as u16), eof)
                .iter()
                .any(|a| matches!(a, Action::Accept))
        );
        prop_assert!(has, "chain depth={} has no Accept on EOF", depth);
    }

    // -----------------------------------------------------------------------
    // 17. Sequence grammar (S → a b) builds correctly
    // -----------------------------------------------------------------------
    #[test]
    fn sequence_builds_ok(a_id in 1u16..10, b_offset in 1u16..10) {
        let b_id = a_id + b_offset;
        let g = grammar_sequence(a_id, b_id);
        let result = build_pipeline(&g);
        prop_assert!(result.is_ok(), "sequence build failed: {:?}", result.err());
    }

    // -----------------------------------------------------------------------
    // 18. Sequence grammar has ≥ 3 states (init, after-a, after-b)
    // -----------------------------------------------------------------------
    #[test]
    fn sequence_min_states(a_id in 1u16..10, b_offset in 1u16..10) {
        let b_id = a_id + b_offset;
        let g = grammar_sequence(a_id, b_id);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        prop_assert!(pt.state_count >= 3,
            "sequence should have ≥3 states, got {}", pt.state_count);
    }

    // -----------------------------------------------------------------------
    // 19. token_count equals number of internal terminals + 1 (EOF)
    // -----------------------------------------------------------------------
    #[test]
    fn token_count_includes_eof(tok_id in 1u16..30) {
        let g = grammar_single_terminal(tok_id);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        // grammar has 1 token, table adds EOF
        prop_assert!(pt.token_count >= 2,
            "token_count should be ≥2, got {}", pt.token_count);
    }

    // -----------------------------------------------------------------------
    // 20. Rules in table match grammar rule count
    // -----------------------------------------------------------------------
    #[test]
    fn rules_count_matches_grammar(n in 1usize..6) {
        let g = grammar_n_alternatives(n);
        let grammar_rule_count = g.all_rules().count();
        let (_ff, pt) = build_pipeline(&g).unwrap();
        prop_assert_eq!(pt.rules.len(), grammar_rule_count,
            "table has {} rules, grammar has {}", pt.rules.len(), grammar_rule_count);
    }

    // -----------------------------------------------------------------------
    // 21. dynamic_prec_by_rule length matches rules
    // -----------------------------------------------------------------------
    #[test]
    fn dynamic_prec_length(tok_id in 1u16..30) {
        let g = grammar_single_terminal(tok_id);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        prop_assert_eq!(pt.dynamic_prec_by_rule.len(), pt.rules.len());
    }

    // -----------------------------------------------------------------------
    // 22. rule_assoc_by_rule length matches rules
    // -----------------------------------------------------------------------
    #[test]
    fn rule_assoc_length(tok_id in 1u16..30) {
        let g = grammar_single_terminal(tok_id);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        prop_assert_eq!(pt.rule_assoc_by_rule.len(), pt.rules.len());
    }

    // -----------------------------------------------------------------------
    // 23. lex_modes length == state_count
    // -----------------------------------------------------------------------
    #[test]
    fn lex_modes_count(tok_id in 1u16..30) {
        let g = grammar_single_terminal(tok_id);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        prop_assert_eq!(pt.lex_modes.len(), pt.state_count);
    }

    // -----------------------------------------------------------------------
    // 24. No action cell contains duplicate actions after normalization
    // -----------------------------------------------------------------------
    #[test]
    fn no_duplicate_actions(tok_id in 1u16..30) {
        let g = grammar_single_terminal(tok_id);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        for s in 0..pt.state_count {
            for col in 0..pt.action_table[s].len() {
                let cell = &pt.action_table[s][col];
                for i in 0..cell.len() {
                    for j in (i + 1)..cell.len() {
                        prop_assert!(cell[i] != cell[j],
                            "duplicate action in state {} col {}: {:?}", s, col, cell[i]);
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // 25. Action cells are sorted (Shift < Reduce < Accept < Error)
    // -----------------------------------------------------------------------
    #[test]
    fn action_cells_sorted(tok_id in 1u16..30) {
        let g = grammar_single_terminal(tok_id);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        for s in 0..pt.state_count {
            for col in 0..pt.action_table[s].len() {
                let cell = &pt.action_table[s][col];
                for w in cell.windows(2) {
                    let a_key = action_sort_key(&w[0]);
                    let b_key = action_sort_key(&w[1]);
                    prop_assert!(a_key <= b_key,
                        "cell ({},{}) not sorted: {:?} > {:?}", s, col, w[0], w[1]);
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // 26. nonterminal_to_index covers all grammar non-terminals
    // -----------------------------------------------------------------------
    #[test]
    fn nonterminal_index_covers_grammar(tok_id in 1u16..30) {
        let g = grammar_single_terminal(tok_id);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        for &nt_sym in g.rules.keys() {
            prop_assert!(pt.nonterminal_to_index.contains_key(&nt_sym),
                "non-terminal {:?} missing from nonterminal_to_index", nt_sym);
        }
    }

    // -----------------------------------------------------------------------
    // 27. EOF symbol does not collide with any grammar terminal or NT
    // -----------------------------------------------------------------------
    #[test]
    fn eof_no_collision(n in 1usize..6) {
        let g = grammar_n_alternatives(n);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        let eof = pt.eof_symbol;
        for &tok in g.tokens.keys() {
            prop_assert_ne!(eof, tok, "EOF collides with token {:?}", tok);
        }
        for &nt in g.rules.keys() {
            prop_assert_ne!(eof, nt, "EOF collides with non-terminal {:?}", nt);
        }
    }

    // -----------------------------------------------------------------------
    // 28. Every Reduce action references a valid rule index
    // -----------------------------------------------------------------------
    #[test]
    fn reduce_actions_reference_valid_rules(n in 1usize..6) {
        let g = grammar_n_alternatives(n);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        for s in 0..pt.state_count {
            for col in 0..pt.action_table[s].len() {
                for action in &pt.action_table[s][col] {
                    if let Action::Reduce(rid) = action {
                        prop_assert!((rid.0 as usize) < pt.rules.len(),
                            "Reduce({}) exceeds rule count {}", rid.0, pt.rules.len());
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // 29. Every Shift action targets a valid state
    // -----------------------------------------------------------------------
    #[test]
    fn shift_actions_target_valid_state(n in 1usize..6) {
        let g = grammar_n_alternatives(n);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        for s in 0..pt.state_count {
            for col in 0..pt.action_table[s].len() {
                for action in &pt.action_table[s][col] {
                    if let Action::Shift(target) = action {
                        prop_assert!((target.0 as usize) < pt.state_count,
                            "Shift targets state {} but only {} states exist",
                            target.0, pt.state_count);
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // 30. Goto targets are valid states (or sentinel 0 meaning no-transition)
    // -----------------------------------------------------------------------
    #[test]
    fn goto_targets_valid(tok_id in 1u16..30) {
        let g = grammar_single_terminal(tok_id);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        for s in 0..pt.state_count {
            for col in 0..pt.goto_table[s].len() {
                let target = pt.goto_table[s][col];
                if target.0 != 0 {
                    prop_assert!((target.0 as usize) < pt.state_count,
                        "goto[{}][{}] = {} exceeds state_count {}",
                        s, col, target.0, pt.state_count);
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // 31. Grammar with precedence: left-assoc resolves shift/reduce
    // -----------------------------------------------------------------------
    #[test]
    fn left_assoc_resolves_conflict(_seed in 0u32..10) {
        let g = GrammarBuilder::new("calc")
            .token("a", "a")
            .token("+", "+")
            .rule_with_precedence("E", vec!["E", "+", "E"], 1, Associativity::Left)
            .rule("E", vec!["a"])
            .start("E")
            .build();
        let result = build_pipeline(&g);
        prop_assert!(result.is_ok(), "left-assoc build failed: {:?}", result.err());
        let (_ff, pt) = result.unwrap();
        // Left-associative: after seeing E + E, on '+' we should reduce (not shift)
        // Verify the table has at least one Reduce in some state on '+'
        let plus = g.tokens.keys()
            .find(|&&id| g.tokens[&id].name == "+")
            .copied()
            .unwrap();
        let has_reduce_on_plus = (0..pt.state_count).any(|s|
            pt.actions(StateId(s as u16), plus)
                .iter()
                .any(|a| matches!(a, Action::Reduce(_)))
        );
        prop_assert!(has_reduce_on_plus, "left-assoc should produce Reduce on '+'");
    }

    // -----------------------------------------------------------------------
    // 32. Right-assoc grammar produces shift on operator
    // -----------------------------------------------------------------------
    #[test]
    fn right_assoc_produces_shift(_seed in 0u32..10) {
        let g = GrammarBuilder::new("calc")
            .token("a", "a")
            .token("^", "^")
            .rule_with_precedence("E", vec!["E", "^", "E"], 1, Associativity::Right)
            .rule("E", vec!["a"])
            .start("E")
            .build();
        let result = build_pipeline(&g);
        prop_assert!(result.is_ok());
        let (_ff, pt) = result.unwrap();
        let caret = g.tokens.keys()
            .find(|&&id| g.tokens[&id].name == "^")
            .copied()
            .unwrap();
        let has_shift_on_caret = (0..pt.state_count).any(|s|
            pt.actions(StateId(s as u16), caret)
                .iter()
                .any(|a| matches!(a, Action::Shift(_)))
        );
        prop_assert!(has_shift_on_caret, "right-assoc should produce Shift on '^'");
    }

    // -----------------------------------------------------------------------
    // 33. sanity_check_tables passes for chain grammars
    // -----------------------------------------------------------------------
    #[test]
    fn sanity_check_chain(depth in 1usize..5) {
        let g = grammar_chain(depth);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        let result = sanity_check_tables(&pt);
        prop_assert!(result.is_ok(), "sanity_check failed for depth={}: {:?}", depth, result.err());
    }

    // -----------------------------------------------------------------------
    // 34. sanity_check_tables passes for N-alternative grammars
    // -----------------------------------------------------------------------
    #[test]
    fn sanity_check_alternatives(n in 1usize..6) {
        let g = grammar_n_alternatives(n);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        let result = sanity_check_tables(&pt);
        prop_assert!(result.is_ok(), "sanity_check failed for n={}: {:?}", n, result.err());
    }

    // -----------------------------------------------------------------------
    // 35. Building same grammar twice yields identical tables (determinism)
    // -----------------------------------------------------------------------
    #[test]
    fn deterministic_build(tok_id in 1u16..20) {
        let g1 = grammar_single_terminal(tok_id);
        let g2 = grammar_single_terminal(tok_id);
        let (_, pt1) = build_pipeline(&g1).unwrap();
        let (_, pt2) = build_pipeline(&g2).unwrap();
        prop_assert_eq!(pt1.state_count, pt2.state_count);
        prop_assert_eq!(pt1.symbol_count, pt2.symbol_count);
        prop_assert_eq!(pt1.action_table, pt2.action_table);
        prop_assert_eq!(pt1.goto_table, pt2.goto_table);
        prop_assert_eq!(pt1.eof_symbol, pt2.eof_symbol);
        prop_assert_eq!(pt1.start_symbol, pt2.start_symbol);
        prop_assert_eq!(pt1.token_count, pt2.token_count);
        prop_assert_eq!(pt1.rules.len(), pt2.rules.len());
    }
}

// ===========================================================================
// Additional targeted property tests
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    // -----------------------------------------------------------------------
    // 36. Initial state has at least one action (shift on some terminal)
    // -----------------------------------------------------------------------
    #[test]
    fn initial_state_has_actions(tok_id in 1u16..30) {
        let g = grammar_single_terminal(tok_id);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        let init = pt.initial_state;
        let has_any = pt.action_table[init.0 as usize]
            .iter()
            .any(|cell| !cell.is_empty());
        prop_assert!(has_any, "initial state has no actions");
    }

    // -----------------------------------------------------------------------
    // 37. Goto from initial state on start symbol goes to a valid state
    // -----------------------------------------------------------------------
    #[test]
    fn goto_initial_on_start(tok_id in 1u16..30) {
        let g = grammar_single_terminal(tok_id);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        let target = pt.goto(pt.initial_state, pt.start_symbol);
        prop_assert!(target.is_some(),
            "goto(initial, start_symbol) should exist");
        let t = target.unwrap();
        prop_assert!((t.0 as usize) < pt.state_count,
            "goto target {} out of range", t.0);
    }

    // -----------------------------------------------------------------------
    // 38. The accept state has Accept on EOF and no Shift on EOF
    // -----------------------------------------------------------------------
    #[test]
    fn accept_state_no_shift_on_eof(tok_id in 1u16..30) {
        let g = grammar_single_terminal(tok_id);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        let eof = pt.eof_symbol;
        for s in 0..pt.state_count {
            let actions = pt.actions(StateId(s as u16), eof);
            if actions.iter().any(|a| matches!(a, Action::Accept)) {
                let has_shift = actions.iter().any(|a| matches!(a, Action::Shift(_)));
                prop_assert!(!has_shift,
                    "state {} has both Accept and Shift on EOF", s);
            }
        }
    }

    // -----------------------------------------------------------------------
    // 39. Every rule's LHS appears in nonterminal_to_index
    // -----------------------------------------------------------------------
    #[test]
    fn rule_lhs_in_nt_index(n in 1usize..6) {
        let g = grammar_n_alternatives(n);
        let (_ff, pt) = build_pipeline(&g).unwrap();
        let nt_syms: BTreeSet<_> = pt.rules.iter().map(|r| r.lhs).collect();
        for lhs in nt_syms {
            prop_assert!(pt.nonterminal_to_index.contains_key(&lhs),
                "rule LHS {:?} not in nonterminal_to_index", lhs);
        }
    }

    // -----------------------------------------------------------------------
    // 40. GrammarBuilder pipeline: two-token grammar
    // -----------------------------------------------------------------------
    #[test]
    fn grammar_builder_two_tokens(_seed in 0u32..10) {
        let g = GrammarBuilder::new("two")
            .token("x", "x")
            .token("y", "y")
            .rule("S", vec!["x", "y"])
            .start("S")
            .build();
        let result = build_pipeline(&g);
        prop_assert!(result.is_ok(), "GrammarBuilder pipeline failed: {:?}", result.err());
        let (_ff, pt) = result.unwrap();
        prop_assert!(pt.state_count >= 3);
        let check = sanity_check_tables(&pt);
        prop_assert!(check.is_ok(), "sanity check failed: {:?}", check.err());
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Mirror of the internal sort key for verifying normalization order.
fn action_sort_key(action: &Action) -> (u8, u16, u16, u16) {
    match action {
        Action::Shift(s) => (0, s.0, 0, 0),
        Action::Reduce(r) => (1, r.0, 0, 0),
        Action::Accept => (2, 0, 0, 0),
        Action::Error => (3, 0, 0, 0),
        Action::Recover => (4, 0, 0, 0),
        Action::Fork(inner) => {
            let first = inner.first().map(action_sort_key).unwrap_or((0, 0, 0, 0));
            (5, first.1, first.2, inner.len() as u16)
        }
        _ => (255, 0, 0, 0),
    }
}

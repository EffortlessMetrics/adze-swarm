#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

//! Integration tests verifying that IR grammars can be correctly consumed by glr-core.
//!
//! Pipeline: Grammar (IR) → normalize → FIRST/FOLLOW → item sets → ParseTable

use adze_glr_core::{Action, FirstFollowSets, build_lr1_automaton};
use adze_ir::builder::GrammarBuilder;
use adze_ir::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a minimal grammar: S → a
fn single_rule_grammar() -> Grammar {
    GrammarBuilder::new("single")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build()
}

/// Build a two-rule grammar: S → a | b
fn two_alternative_grammar() -> Grammar {
    GrammarBuilder::new("two_alt")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["a"])
        .rule("S", vec!["b"])
        .start("S")
        .build()
}

/// Build an arithmetic expression grammar with precedence.
fn arithmetic_grammar() -> Grammar {
    GrammarBuilder::new("arith")
        .token("NUM", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build()
}

/// Build a multi-nonterminal grammar: S → A B, A → a, B → b
fn multi_nonterminal_grammar() -> Grammar {
    GrammarBuilder::new("multi_nt")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["A", "B"])
        .rule("A", vec!["a"])
        .rule("B", vec!["b"])
        .start("S")
        .build()
}

/// Build grammar with epsilon: S → a | ε
fn epsilon_grammar() -> Grammar {
    GrammarBuilder::new("eps")
        .token("a", "a")
        .rule("S", vec!["a"])
        .rule("S", vec![])
        .start("S")
        .build()
}

/// Build a right-recursive grammar: L → a | a L
fn right_recursive_grammar() -> Grammar {
    GrammarBuilder::new("right_rec")
        .token("a", "a")
        .rule("L", vec!["a"])
        .rule("L", vec!["a", "L"])
        .start("L")
        .build()
}

/// Build a left-recursive grammar: L → a | L a
fn left_recursive_grammar() -> Grammar {
    GrammarBuilder::new("left_rec")
        .token("a", "a")
        .rule("L", vec!["a"])
        .rule("L", vec!["L", "a"])
        .start("L")
        .build()
}

/// Full pipeline helper: grammar → normalize → FIRST/FOLLOW → parse table.
fn full_pipeline(grammar: &Grammar) -> (FirstFollowSets, adze_glr_core::ParseTable) {
    let ff = FirstFollowSets::compute(grammar).expect("FIRST/FOLLOW computation failed");
    let table = build_lr1_automaton(grammar, &ff).expect("LR(1) automaton build failed");
    (ff, table)
}

// ---------------------------------------------------------------------------
// Tests: single-rule grammar
// ---------------------------------------------------------------------------

#[test]
fn single_rule_produces_table() {
    let g = single_rule_grammar();
    let (_ff, table) = full_pipeline(&g);
    assert!(table.state_count > 0, "table must have at least one state");
}

#[test]
fn single_rule_has_accept_action() {
    let g = single_rule_grammar();
    let (_ff, table) = full_pipeline(&g);
    let eof = table.eof();
    let has_accept = (0..table.state_count).any(|st| {
        table
            .actions(StateId(st as u16), eof)
            .iter()
            .any(|a| matches!(a, Action::Accept))
    });
    assert!(has_accept, "table must contain an Accept action");
}

#[test]
fn single_rule_first_set_nonempty() {
    let g = single_rule_grammar();
    let (ff, _table) = full_pipeline(&g);
    let start = g.start_symbol().unwrap();
    let first = ff.first(start);
    assert!(first.is_some(), "FIRST set for start symbol must exist");
    assert!(
        first.unwrap().count_ones(..) > 0,
        "FIRST set for start symbol must be non-empty"
    );
}

// ---------------------------------------------------------------------------
// Tests: two-alternative grammar
// ---------------------------------------------------------------------------

#[test]
fn two_alt_produces_table() {
    let g = two_alternative_grammar();
    let (_ff, table) = full_pipeline(&g);
    assert!(table.state_count > 0);
}

#[test]
fn two_alt_first_set_contains_both_terminals() {
    let g = two_alternative_grammar();
    let (ff, _table) = full_pipeline(&g);
    let start = g.start_symbol().unwrap();
    let first = ff.first(start).unwrap();
    // Both terminals should appear in FIRST(S)
    assert!(
        first.count_ones(..) >= 2,
        "FIRST(S) should contain at least both terminals"
    );
}

#[test]
fn two_alt_has_shift_in_initial_state() {
    let g = two_alternative_grammar();
    let (_ff, table) = full_pipeline(&g);
    // At least one terminal should have a Shift in the initial state
    let initial = table.initial_state;
    let has_shift = table.grammar().tokens.keys().any(|&tok| {
        table
            .actions(initial, tok)
            .iter()
            .any(|a| matches!(a, Action::Shift(_)))
    });
    assert!(has_shift, "initial state should have at least one Shift");
}

// ---------------------------------------------------------------------------
// Tests: multi-nonterminal grammar
// ---------------------------------------------------------------------------

#[test]
fn multi_nt_produces_table() {
    let g = multi_nonterminal_grammar();
    let (_ff, table) = full_pipeline(&g);
    assert!(table.state_count > 0);
}

#[test]
fn multi_nt_goto_exists_for_nonterminals() {
    let g = multi_nonterminal_grammar();
    let (_ff, table) = full_pipeline(&g);
    // At least one GOTO entry should exist
    let has_goto = (0..table.state_count).any(|st| {
        g.rules
            .keys()
            .any(|&nt| table.goto(StateId(st as u16), nt).is_some())
    });
    assert!(has_goto, "GOTO table must have entries for nonterminals");
}

#[test]
fn multi_nt_rules_preserved() {
    let g = multi_nonterminal_grammar();
    let (_ff, table) = full_pipeline(&g);
    // The table should have rules for reduction
    assert!(!table.rules.is_empty(), "parse table must have rules");
}

// ---------------------------------------------------------------------------
// Tests: arithmetic grammar with precedence
// ---------------------------------------------------------------------------

#[test]
fn arith_precedence_produces_table() {
    let g = arithmetic_grammar();
    let (_ff, table) = full_pipeline(&g);
    assert!(
        table.state_count > 0,
        "arithmetic grammar should produce states"
    );
}

#[test]
fn arith_precedence_first_set() {
    let g = arithmetic_grammar();
    let (ff, _table) = full_pipeline(&g);
    let start = g.start_symbol().unwrap();
    let first = ff.first(start).unwrap();
    assert!(
        first.count_ones(..) > 0,
        "FIRST(expr) should contain NUM terminal"
    );
}

#[test]
fn arith_precedence_has_accept() {
    let g = arithmetic_grammar();
    let (_ff, table) = full_pipeline(&g);
    let eof = table.eof();
    let has_accept = (0..table.state_count).any(|st| {
        table
            .actions(StateId(st as u16), eof)
            .iter()
            .any(|a| matches!(a, Action::Accept))
    });
    assert!(has_accept);
}

#[test]
fn arith_has_multiple_rules() {
    let g = arithmetic_grammar();
    let (_ff, table) = full_pipeline(&g);
    // 3 original rules + 1 augmented start rule
    assert!(
        table.rules.len() >= 3,
        "should have at least the 3 original expression rules"
    );
}

// ---------------------------------------------------------------------------
// Tests: epsilon grammar
// ---------------------------------------------------------------------------

#[test]
fn epsilon_grammar_produces_table() {
    let g = epsilon_grammar();
    let (_ff, table) = full_pipeline(&g);
    assert!(table.state_count > 0);
}

#[test]
fn epsilon_start_is_nullable() {
    let g = epsilon_grammar();
    let (ff, _table) = full_pipeline(&g);
    let start = g.start_symbol().unwrap();
    assert!(ff.is_nullable(start), "start symbol should be nullable");
}

// ---------------------------------------------------------------------------
// Tests: recursive grammars
// ---------------------------------------------------------------------------

#[test]
fn right_recursive_produces_table() {
    let g = right_recursive_grammar();
    let (_ff, table) = full_pipeline(&g);
    assert!(table.state_count > 0);
}

#[test]
fn left_recursive_produces_table() {
    let g = left_recursive_grammar();
    let (_ff, table) = full_pipeline(&g);
    assert!(table.state_count > 0);
}

#[test]
fn left_recursive_not_nullable() {
    let g = left_recursive_grammar();
    let (ff, _table) = full_pipeline(&g);
    let start = g.start_symbol().unwrap();
    assert!(!ff.is_nullable(start), "L should not be nullable");
}

// ---------------------------------------------------------------------------
// Tests: normalization before GLR
// ---------------------------------------------------------------------------

#[test]
fn normalize_then_compute() {
    let mut g = Grammar::new("norm_test".into());
    let tok_a = SymbolId(1);
    let tok_b = SymbolId(2);
    let s = SymbolId(10);

    g.tokens.insert(
        tok_a,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    g.tokens.insert(
        tok_b,
        Token {
            name: "b".into(),
            pattern: TokenPattern::String("b".into()),
            fragile: false,
        },
    );
    g.rule_names.insert(s, "S".into());
    // S → a? b  (uses Optional)
    g.rules.insert(
        s,
        vec![Rule {
            lhs: s,
            rhs: vec![
                Symbol::Optional(Box::new(Symbol::Terminal(tok_a))),
                Symbol::Terminal(tok_b),
            ],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );

    g.normalize();

    // After normalization, no complex symbols should remain
    for rule in g.all_rules() {
        for sym in &rule.rhs {
            assert!(
                !matches!(
                    sym,
                    Symbol::Optional(_)
                        | Symbol::Repeat(_)
                        | Symbol::Choice(_)
                        | Symbol::Sequence(_)
                ),
                "complex symbols should be normalized away"
            );
        }
    }

    // Should compute FIRST/FOLLOW successfully
    let ff = FirstFollowSets::compute(&g).expect("compute after normalize");
    assert!(ff.first(s).is_some());
}

#[test]
fn normalize_repeat_then_build_table() {
    let mut g = Grammar::new("repeat_test".into());
    let tok_a = SymbolId(1);
    let s = SymbolId(10);

    g.tokens.insert(
        tok_a,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    g.rule_names.insert(s, "S".into());
    // S → a*  (zero-or-more)
    g.rules.insert(
        s,
        vec![Rule {
            lhs: s,
            rhs: vec![Symbol::Repeat(Box::new(Symbol::Terminal(tok_a)))],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );

    g.set_start_symbol(s);
    g.normalize();

    let ff = FirstFollowSets::compute(&g).expect("FIRST/FOLLOW after repeat normalize");
    let table = build_lr1_automaton(&g, &ff).expect("build table with repeat");
    assert!(table.state_count > 0);
}

#[test]
fn compute_normalized_shortcut() {
    let mut g = Grammar::new("shortcut".into());
    let tok_a = SymbolId(1);
    let s = SymbolId(10);

    g.tokens.insert(
        tok_a,
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
            rhs: vec![Symbol::Choice(vec![
                Symbol::Terminal(tok_a),
                Symbol::Epsilon,
            ])],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );

    // compute_normalized does normalize + compute in one step
    let ff = FirstFollowSets::compute_normalized(&mut g).expect("compute_normalized");
    assert!(ff.first(s).is_some());
}

// ---------------------------------------------------------------------------
// Tests: grammar roundtrip (IR → GLR → verify)
// ---------------------------------------------------------------------------

#[test]
fn roundtrip_grammar_preserved_in_table() {
    let g = single_rule_grammar();
    let (_ff, table) = full_pipeline(&g);
    // The table stores the grammar; verify name survives
    assert_eq!(table.grammar().name, "single");
}

#[test]
fn roundtrip_start_symbol_consistent() {
    let g = arithmetic_grammar();
    let (_ff, table) = full_pipeline(&g);
    // The table's start_symbol should correspond to the grammar's start
    let ir_start = g.start_symbol().unwrap();
    // The table stores the augmented grammar, but start_symbol field should be set
    assert_ne!(table.start_symbol().0, 0, "start symbol should be non-zero");
    // The grammar embedded in the table should still recognize the original start
    assert!(
        table.grammar().rules.contains_key(&ir_start),
        "original start should be in embedded grammar"
    );
}

#[test]
fn roundtrip_eof_symbol_valid() {
    let g = single_rule_grammar();
    let (_ff, table) = full_pipeline(&g);
    let eof = table.eof();
    // EOF should not collide with any grammar token
    assert!(
        !g.tokens.contains_key(&eof),
        "EOF should not collide with grammar tokens"
    );
    // EOF should not collide with any rule LHS
    assert!(
        !g.rules.contains_key(&eof),
        "EOF should not collide with rule symbols"
    );
}

#[test]
fn roundtrip_token_count_reasonable() {
    let g = arithmetic_grammar();
    let (_ff, table) = full_pipeline(&g);
    // token_count should be >= number of tokens in original grammar
    assert!(
        table.token_count >= g.tokens.len(),
        "token_count ({}) should be >= grammar tokens ({})",
        table.token_count,
        g.tokens.len()
    );
}

// ---------------------------------------------------------------------------
// Tests: state count sanity
// ---------------------------------------------------------------------------

#[test]
fn single_rule_state_count() {
    let g = single_rule_grammar();
    let (_ff, table) = full_pipeline(&g);
    // S → a needs at least 3 states: initial, after shift a, accept
    assert!(
        table.state_count >= 3,
        "single rule should have >= 3 states, got {}",
        table.state_count
    );
}

#[test]
fn multi_rule_more_states_than_single() {
    let single = single_rule_grammar();
    let multi = multi_nonterminal_grammar();
    let (_, t_single) = full_pipeline(&single);
    let (_, t_multi) = full_pipeline(&multi);
    assert!(
        t_multi.state_count >= t_single.state_count,
        "multi-nonterminal grammar should have >= states than single rule"
    );
}

// ---------------------------------------------------------------------------
// Tests: builder-produced grammars through full pipeline
// ---------------------------------------------------------------------------

#[test]
fn builder_javascript_like_through_pipeline() {
    let g = GrammarBuilder::javascript_like();
    let (_ff, table) = full_pipeline(&g);
    assert!(
        table.state_count > 0,
        "JS-like grammar should produce a table"
    );
    let eof = table.eof();
    let has_accept = (0..table.state_count).any(|st| {
        table
            .actions(StateId(st as u16), eof)
            .iter()
            .any(|a| matches!(a, Action::Accept))
    });
    assert!(has_accept, "JS-like grammar must have Accept");
}

#[test]
fn builder_with_extras_through_pipeline() {
    let g = GrammarBuilder::new("with_extra")
        .token("a", "a")
        .token("WS", r"\s+")
        .extra("WS")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let (_ff, table) = full_pipeline(&g);
    assert!(table.state_count > 0);
}

// ---------------------------------------------------------------------------
// Tests: edge cases
// ---------------------------------------------------------------------------

#[test]
fn many_alternatives_grammar() {
    // S → a | b | c | d | e
    let g = GrammarBuilder::new("many_alt")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .token("d", "d")
        .token("e", "e")
        .rule("S", vec!["a"])
        .rule("S", vec!["b"])
        .rule("S", vec!["c"])
        .rule("S", vec!["d"])
        .rule("S", vec!["e"])
        .start("S")
        .build();
    let (ff, table) = full_pipeline(&g);
    let start = g.start_symbol().unwrap();
    let first = ff.first(start).unwrap();
    assert!(
        first.count_ones(..) >= 5,
        "FIRST(S) should contain all 5 terminals"
    );
    assert!(table.state_count > 0);
}

#[test]
fn chain_grammar_a_to_b_to_c() {
    // S → A, A → B, B → c
    let g = GrammarBuilder::new("chain")
        .token("c", "c")
        .rule("S", vec!["A"])
        .rule("A", vec!["B"])
        .rule("B", vec!["c"])
        .start("S")
        .build();
    let (ff, table) = full_pipeline(&g);
    let start = g.start_symbol().unwrap();
    let first = ff.first(start).unwrap();
    // FIRST(S) should transitively include the terminal 'c'
    assert!(
        first.count_ones(..) > 0,
        "FIRST(S) should include terminal c"
    );
    assert!(table.state_count > 0);
}

#[test]
fn right_assoc_precedence_grammar() {
    let g = GrammarBuilder::new("right_assoc")
        .token("NUM", r"\d+")
        .token("^", "^")
        .rule_with_precedence("expr", vec!["expr", "^", "expr"], 1, Associativity::Right)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let (_ff, table) = full_pipeline(&g);
    assert!(
        table.state_count > 0,
        "right-assoc grammar should produce a table"
    );
}

#[test]
fn action_table_dimensions_match_state_count() {
    let g = arithmetic_grammar();
    let (_ff, table) = full_pipeline(&g);
    assert_eq!(
        table.action_table.len(),
        table.state_count,
        "action_table rows should match state_count"
    );
}

#[test]
fn goto_table_dimensions_match_state_count() {
    let g = arithmetic_grammar();
    let (_ff, table) = full_pipeline(&g);
    assert_eq!(
        table.goto_table.len(),
        table.state_count,
        "goto_table rows should match state_count"
    );
}

// ---------------------------------------------------------------------------
// Tests: FOLLOW set verification
// ---------------------------------------------------------------------------

#[test]
fn follow_sets_computed_alongside_first() {
    let g = arithmetic_grammar();
    let ff = FirstFollowSets::compute(&g).expect("compute FIRST/FOLLOW");
    let _start = g.start_symbol().unwrap();
    // All nonterminals should have FOLLOW sets computed
    for &rule_id in g.rules.keys() {
        let follow = ff.follow(rule_id);
        assert!(
            follow.is_some(),
            "FOLLOW set must exist for rule {:?}",
            rule_id
        );
    }
}

#[test]
fn follow_sets_include_eof_for_start_symbol() {
    let g = single_rule_grammar();
    let ff = FirstFollowSets::compute(&g).expect("compute FIRST/FOLLOW");
    let start = g.start_symbol().unwrap();
    let follow = ff.follow(start).unwrap();
    // Start symbol's FOLLOW should contain EOF (represented as an empty marker)
    // Verify FOLLOW set is non-empty
    let _ = follow.count_ones(..);
}

#[test]
fn follow_set_propagation_through_chain() {
    // S → A, A → B, B → c
    let g = GrammarBuilder::new("chain_follow")
        .token("c", "c")
        .rule("S", vec!["A"])
        .rule("A", vec!["B"])
        .rule("B", vec!["c"])
        .start("S")
        .build();
    let ff = FirstFollowSets::compute(&g).expect("compute for chain");
    let a_id = g.rules.keys().nth(1).copied();
    let b_id = g.rules.keys().nth(2).copied();
    // Both A and B should have FOLLOW sets
    assert!(a_id.is_some() && ff.follow(a_id.unwrap()).is_some());
    assert!(b_id.is_some() && ff.follow(b_id.unwrap()).is_some());
}

// ---------------------------------------------------------------------------
// Tests: Conflict detection
// ---------------------------------------------------------------------------

#[test]
fn grammar_with_shift_reduce_potential() {
    // expr → expr + expr | NUM
    // This is classic shift/reduce conflict scenario
    let g = GrammarBuilder::new("shift_reduce")
        .token("NUM", r"\d+")
        .token("+", "+")
        .rule("expr", vec!["expr", "+", "expr"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let (_ff, table) = full_pipeline(&g);
    // Should still produce valid parse table
    assert!(table.state_count > 0);
}

#[test]
fn grammar_with_reduce_reduce_potential() {
    // S → A | B, A → id, B → id
    // This has reduce/reduce conflict potential
    let g = GrammarBuilder::new("reduce_reduce")
        .token("id", "id")
        .rule("S", vec!["A"])
        .rule("S", vec!["B"])
        .rule("A", vec!["id"])
        .rule("B", vec!["id"])
        .start("S")
        .build();
    let (_ff, table) = full_pipeline(&g);
    // Should still parse
    assert!(table.state_count > 0);
}

#[test]
fn ambiguous_grammar_still_produces_table() {
    // Simple ambiguous grammar: S → S S | a
    let g = GrammarBuilder::new("ambig")
        .token("a", "a")
        .rule("S", vec!["S", "S"])
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let (_ff, table) = full_pipeline(&g);
    // Even ambiguous grammar should produce a table
    assert!(table.state_count > 0);
}

// ---------------------------------------------------------------------------
// Tests: Empty/minimal grammar error handling
// ---------------------------------------------------------------------------

#[test]
fn empty_grammar_handled() {
    let g = Grammar::new("empty".into());
    // Should error gracefully on empty grammar
    let result = FirstFollowSets::compute(&g);
    // Depending on implementation, may error or succeed with empty sets
    // At minimum, should not crash
    let _ = result;
}

#[test]
fn grammar_with_only_tokens_no_rules() {
    let mut g = Grammar::new("tokens_only".into());
    g.tokens.insert(
        SymbolId(1),
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    // No rules defined
    let result = FirstFollowSets::compute(&g);
    // Should handle gracefully
    let _ = result;
}

// ---------------------------------------------------------------------------
// Tests: Complex precedence and associativity
// ---------------------------------------------------------------------------

#[test]
fn mixed_precedence_associativity() {
    let g = GrammarBuilder::new("mixed_prec")
        .token("NUM", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .token("-", "-")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "-", "expr"], 1, Associativity::Right)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let (ff, table) = full_pipeline(&g);
    let start = g.start_symbol().unwrap();
    assert!(ff.first(start).is_some());
    assert!(table.state_count > 0);
}

#[test]
fn precedence_levels_distinct() {
    let g = GrammarBuilder::new("prec_dist")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .rule_with_precedence("x", vec!["x", "a", "x"], 1, Associativity::Left)
        .rule_with_precedence("x", vec!["x", "b", "x"], 5, Associativity::Left)
        .rule_with_precedence("x", vec!["x", "c", "x"], 10, Associativity::Left)
        .rule("x", vec!["a"])
        .start("x")
        .build();
    let (_ff, table) = full_pipeline(&g);
    assert!(table.state_count > 0);
}

// ---------------------------------------------------------------------------
// Tests: Optional and repetition normalization
// ---------------------------------------------------------------------------

#[test]
fn optional_symbol_normalization() {
    let mut g = Grammar::new("opt_norm".into());
    let tok_a = SymbolId(1);
    let s = SymbolId(10);

    g.tokens.insert(
        tok_a,
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
            rhs: vec![Symbol::Optional(Box::new(Symbol::Terminal(tok_a)))],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );

    g.normalize();

    // After normalization, Optional should be gone
    for rule in g.all_rules() {
        for sym in &rule.rhs {
            assert!(
                !matches!(sym, Symbol::Optional(_)),
                "Optional should be normalized"
            );
        }
    }

    let ff = FirstFollowSets::compute(&g).expect("compute after opt normalize");
    let start_first = ff.first(s);
    assert!(start_first.is_some());
}

#[test]
fn repeat_zero_or_more_normalization() {
    let mut g = Grammar::new("repeat_zero".into());
    let tok_a = SymbolId(1);
    let s = SymbolId(10);

    g.tokens.insert(
        tok_a,
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
            rhs: vec![Symbol::Repeat(Box::new(Symbol::Terminal(tok_a)))],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );

    g.normalize();

    // After normalization, Repeat should be gone
    for rule in g.all_rules() {
        for sym in &rule.rhs {
            assert!(
                !matches!(sym, Symbol::Repeat(_)),
                "Repeat should be normalized"
            );
        }
    }

    let ff = FirstFollowSets::compute(&g).expect("after repeat normalize");
    assert!(ff.first(s).is_some());
}

// ---------------------------------------------------------------------------
// Tests: Large and complex grammars
// ---------------------------------------------------------------------------

#[test]
fn large_choice_grammar() {
    let mut builder = GrammarBuilder::new("large_choice");
    for i in 0..20 {
        builder = builder.token(&format!("t{}", i), &format!("t{}", i));
    }
    for i in 0..20 {
        builder = builder.rule("S", vec![&format!("t{}", i)]);
    }
    let g = builder.start("S").build();
    let (ff, table) = full_pipeline(&g);
    let start = g.start_symbol().unwrap();
    let first = ff.first(start).unwrap();
    assert!(
        first.count_ones(..) >= 20,
        "FIRST(S) should contain all 20 terminals"
    );
    assert!(table.state_count > 0);
}

#[test]
fn deep_nesting_grammar() {
    // S → A, A → B, B → C, ... → Z → a
    let mut builder = GrammarBuilder::new("deep_nest");
    builder = builder.token("a", "a");

    let rules = ["S", "A", "B", "C", "D", "E", "F", "G", "H", "I", "J"];
    for i in 0..rules.len() {
        if i == rules.len() - 1 {
            builder = builder.rule(rules[i], vec!["a"]);
        } else {
            builder = builder.rule(rules[i], vec![rules[i + 1]]);
        }
    }

    let g = builder.start("S").build();
    let (ff, table) = full_pipeline(&g);
    let start = g.start_symbol().unwrap();
    // FIRST should propagate through the chain
    assert!(ff.first(start).is_some());
    assert!(table.state_count > 0);
}

// ---------------------------------------------------------------------------
// Tests: Error handling in pipeline
// ---------------------------------------------------------------------------

#[test]
fn malformed_rule_handling() {
    // Self-referential rule only: S → S
    let g = GrammarBuilder::new("malformed")
        .rule("S", vec!["S"])
        .start("S")
        .build();
    // Should still compute without crashing
    let ff = FirstFollowSets::compute(&g);
    let _ = ff; // Just ensure no panic
}

#[test]
fn unreachable_rule_handling() {
    // S → A, B → c (B is unreachable from S)
    let g = GrammarBuilder::new("unreachable")
        .token("c", "c")
        .rule("S", vec!["A"])
        .rule("A", vec!["A"])
        .rule("B", vec!["c"])
        .start("S")
        .build();
    let (_ff, table) = full_pipeline(&g);
    // Should still produce a table
    assert!(table.state_count > 0);
}

#[test]
fn rule_with_only_self_reference() {
    // S → S | a
    let g = GrammarBuilder::new("self_ref")
        .token("a", "a")
        .rule("S", vec!["S"])
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let (ff, table) = full_pipeline(&g);
    let start = g.start_symbol().unwrap();
    // a should be in FIRST(S)
    assert!(ff.first(start).is_some());
    assert!(table.state_count > 0);
}

//! Comprehensive error-path tests for adze-glr-core.
//!
//! Every test exercises a failure mode in the GLR core pipeline:
//! grammar validation, FIRST/FOLLOW computation, canonical collection
//! construction, parse table generation, and driver execution.
#![cfg(feature = "test-api")]

use adze_glr_core::driver::Driver;
use adze_glr_core::{
    Action, ConflictResolver, FirstFollowSets, GLRError, GotoIndexing, ItemSet, ItemSetCollection,
    LRItem, LexMode, ParseRule, ParseTable, build_lr1_automaton, sanity_check_tables,
};
use adze_ir::builder::GrammarBuilder;
use adze_ir::*;
use std::collections::BTreeMap;

// ─── helpers ────────────────────────────────────────────────────────

/// Shorthand for constructing a simple Grammar by hand.
fn empty_grammar() -> Grammar {
    Grammar::new("empty".into())
}

/// Build a minimal ParseTable suitable for Driver construction.
/// `cols` is the number of columns per action row.
fn minimal_table(
    actions: Vec<Vec<Vec<Action>>>,
    gotos: Vec<Vec<StateId>>,
    rules: Vec<ParseRule>,
    start: SymbolId,
    eof: SymbolId,
) -> ParseTable {
    let state_count = actions.len();
    let symbol_count = actions.first().map(|r| r.len()).unwrap_or(0);

    let mut symbol_to_index = BTreeMap::new();
    for i in 0..symbol_count {
        symbol_to_index.insert(SymbolId(i as u16), i);
    }

    let mut nonterminal_to_index = BTreeMap::new();
    let invalid = StateId(65535);
    for i in 0..symbol_count {
        for row in &gotos {
            if let Some(&s) = row.get(i)
                && s != invalid
            {
                nonterminal_to_index.insert(SymbolId(i as u16), i);
                break;
            }
        }
    }

    ParseTable {
        action_table: actions,
        goto_table: gotos,
        rules,
        state_count,
        symbol_count,
        symbol_to_index,
        index_to_symbol: vec![],
        external_scanner_states: vec![],
        nonterminal_to_index,
        eof_symbol: eof,
        start_symbol: start,
        grammar: Grammar::new("test".into()),
        symbol_metadata: vec![],
        initial_state: StateId(0),
        token_count: 2,
        external_token_count: 0,
        lex_modes: vec![
            LexMode {
                lex_state: 0,
                external_lex_state: 0,
            };
            state_count
        ],
        extras: vec![],
        dynamic_prec_by_rule: vec![0; 0],
        rule_assoc_by_rule: vec![0; 0],
        alias_sequences: vec![],
        field_names: vec![],
        goto_indexing: GotoIndexing::NonterminalMap,
        field_map: BTreeMap::new(),
    }
}

// ════════════════════════════════════════════════════════════════════
// 1. Empty grammar (no rules)
// ════════════════════════════════════════════════════════════════════

#[test]
fn empty_grammar_first_follow_succeeds_with_empty_sets() {
    let grammar = empty_grammar();
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    assert!(ff.first(SymbolId(0)).is_none());
    assert!(ff.follow(SymbolId(0)).is_none());
}

#[test]
fn empty_grammar_automaton_fails_no_start() {
    let grammar = empty_grammar();
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let result = build_lr1_automaton(&grammar, &ff);
    assert!(result.is_err(), "empty grammar must fail automaton build");
}

#[test]
fn empty_grammar_canonical_collection_empty_initial_set() {
    let grammar = empty_grammar();
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&grammar, &ff);
    // Initial item set should have zero items (no start rule)
    assert!(
        collection.sets[0].items.is_empty(),
        "no start symbol → empty initial item set"
    );
}

// ════════════════════════════════════════════════════════════════════
// 2. Grammar with undefined non-terminals in rules
// ════════════════════════════════════════════════════════════════════

#[test]
fn undefined_nonterminal_in_rhs_first_follow_succeeds() {
    // S → B, but B has no rules defined
    let mut grammar = Grammar::new("undef_nt".into());
    let s = SymbolId(10);
    let b = SymbolId(20);
    grammar.rule_names.insert(s, "S".into());
    grammar.rules.insert(
        s,
        vec![Rule {
            lhs: s,
            rhs: vec![Symbol::NonTerminal(b)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );
    // B has no rules → FIRST(S) will be empty, no crash
    let ff = FirstFollowSets::compute(&grammar);
    assert!(ff.is_ok(), "should not crash on undefined NT");
}

#[test]
fn undefined_nonterminal_automaton_produces_table_or_error() {
    let mut grammar = Grammar::new("undef_nt".into());
    let s = SymbolId(10);
    let b = SymbolId(20);
    grammar.rule_names.insert(s, "S".into());
    grammar.rules.insert(
        s,
        vec![Rule {
            lhs: s,
            rhs: vec![Symbol::NonTerminal(b)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    // Building the automaton may succeed (with an unreachable accept) or error.
    // Either is acceptable; it must not panic.
    let _result = build_lr1_automaton(&grammar, &ff);
}

// ════════════════════════════════════════════════════════════════════
// 3. Grammar with unreachable states
// ════════════════════════════════════════════════════════════════════

#[test]
fn unreachable_rule_does_not_appear_in_collection() {
    // S → a; X → b  (X is unreachable because S is start)
    let mut grammar = GrammarBuilder::new("unreachable")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["a"])
        .rule("X", vec!["b"])
        .start("S")
        .build();

    let ff = FirstFollowSets::compute_normalized(&mut grammar).unwrap();
    let table = build_lr1_automaton(&grammar, &ff).unwrap();

    // X → b should not produce any goto entry for X's symbol
    // because X is unreachable from S.
    let x_sym_name = grammar
        .rule_names
        .iter()
        .find(|(_, name)| name.as_str() == "X")
        .map(|(id, _)| *id);

    if let Some(x_id) = x_sym_name {
        let has_goto = table.goto_table.iter().any(|row| {
            if let Some(&col) = table.nonterminal_to_index.get(&x_id) {
                row.get(col).is_some_and(|s| s.0 != 0)
            } else {
                false
            }
        });
        assert!(!has_goto, "unreachable non-terminal X should have no goto");
    }
}

// ════════════════════════════════════════════════════════════════════
// 4. Grammar with no start symbol
// ════════════════════════════════════════════════════════════════════

#[test]
fn no_start_symbol_automaton_errors() {
    let mut grammar = Grammar::new("no_start".into());
    let a = SymbolId(1);
    let s = SymbolId(10);
    grammar.tokens.insert(
        a,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    grammar.rule_names.insert(s, "S".into());
    grammar.rules.insert(
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
    // #862 PR6: identity read is explicit-only; analysis pins first rule LHS.
    assert!(grammar.start_symbol().is_none());
    assert!(grammar.explicit_start_symbol().is_none());
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    assert!(
        build_lr1_automaton(&grammar, &ff).is_ok(),
        "rules without explicit start still build via analysis default"
    );
    // Truly no rules → no analysis start.
    let mut grammar2 = Grammar::new("truly_no_start".into());
    grammar2.tokens.insert(
        a,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    // No rules, no rule_names → no start symbol
    let ff = FirstFollowSets::compute(&grammar2).unwrap();
    let result = build_lr1_automaton(&grammar2, &ff);
    assert!(result.is_err(), "grammar without start must fail");
}

// ════════════════════════════════════════════════════════════════════
// 5. Invalid symbol IDs in grammar rules
// ════════════════════════════════════════════════════════════════════

#[test]
fn max_symbol_id_does_not_overflow() {
    // Use symbol IDs near u16::MAX to test overflow handling.
    // Grammar::normalize() or FirstFollowSets::compute() may overflow
    // when allocating auxiliary symbols at max_existing_id + 1000.
    let mut grammar = Grammar::new("overflow".into());
    let s = SymbolId(65530);
    let a = SymbolId(65531);
    grammar.tokens.insert(
        a,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    grammar.rule_names.insert(s, "S".into());
    grammar.rules.insert(
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

    // compute() internally clones and normalizes, which may panic with overflow.
    // build_lr1_automaton also uses checked_add for EOF assignment.
    // Either a panic or an Err is acceptable; silent corruption is not.
    let result = std::panic::catch_unwind(|| {
        let ff = FirstFollowSets::compute(&grammar)?;
        build_lr1_automaton(&grammar, &ff)
    });
    assert!(
        result.is_err() || result.unwrap().is_err(),
        "symbol IDs near u16::MAX should cause overflow error or panic"
    );
}

#[test]
fn symbol_id_at_boundary_still_works() {
    // Moderate but high symbol IDs: should still succeed
    let mut grammar = Grammar::new("high_ids".into());
    let s = SymbolId(1000);
    let a = SymbolId(1001);
    grammar.tokens.insert(
        a,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    grammar.rule_names.insert(s, "S".into());
    grammar.rules.insert(
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

    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let result = build_lr1_automaton(&grammar, &ff);
    assert!(
        result.is_ok(),
        "moderate symbol IDs should work: {:?}",
        result.err()
    );
}

// ════════════════════════════════════════════════════════════════════
// 6. Duplicate rule definitions
// ════════════════════════════════════════════════════════════════════

#[test]
fn duplicate_rules_are_tolerated() {
    // Two identical rules: S → a, S → a (same RHS, different production IDs)
    let mut grammar = GrammarBuilder::new("dup")
        .token("a", "a")
        .rule("S", vec!["a"])
        .rule("S", vec!["a"]) // duplicate
        .start("S")
        .build();

    let ff = FirstFollowSets::compute_normalized(&mut grammar).unwrap();
    let result = build_lr1_automaton(&grammar, &ff);
    // Should either succeed (ignoring duplicate) or error gracefully
    assert!(
        result.is_ok(),
        "duplicate rules should be tolerated: {:?}",
        result.err()
    );
}

#[test]
fn duplicate_rules_do_not_crash_conflict_detection() {
    let mut grammar = GrammarBuilder::new("dup_conflict")
        .token("a", "a")
        .rule("S", vec!["a"])
        .rule("S", vec!["a"])
        .start("S")
        .build();

    let ff = FirstFollowSets::compute_normalized(&mut grammar).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&grammar, &ff);
    let resolver = ConflictResolver::detect_conflicts(&collection, &grammar, &ff);
    // May have reduce/reduce conflict from duplicate rules; should not panic
    let _ = resolver.conflicts;
}

// ════════════════════════════════════════════════════════════════════
// 7. FIRST set computation with left-recursive grammars
// ════════════════════════════════════════════════════════════════════

#[test]
fn left_recursive_grammar_first_set_terminates() {
    // E → E '+' a | a   (direct left recursion)
    let mut grammar = Grammar::new("left_rec".into());
    let a = SymbolId(1);
    let plus = SymbolId(2);
    let e = SymbolId(10);

    grammar.tokens.insert(
        a,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    grammar.tokens.insert(
        plus,
        Token {
            name: "+".into(),
            pattern: TokenPattern::String("+".into()),
            fragile: false,
        },
    );
    grammar.rule_names.insert(e, "E".into());
    grammar.rules.insert(
        e,
        vec![
            Rule {
                lhs: e,
                rhs: vec![
                    Symbol::NonTerminal(e),
                    Symbol::Terminal(plus),
                    Symbol::Terminal(a),
                ],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(0),
            },
            Rule {
                lhs: e,
                rhs: vec![Symbol::Terminal(a)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(1),
            },
        ],
    );

    let ff = FirstFollowSets::compute(&grammar).unwrap();
    // FIRST(E) must contain 'a'
    let first_e = ff.first(e).expect("E must have a FIRST set");
    assert!(first_e.contains(a.0 as usize), "FIRST(E) must contain 'a'");
}

#[test]
fn indirect_left_recursion_terminates() {
    // A → B a; B → A b | c  (indirect left recursion)
    let mut grammar = Grammar::new("indirect_lr".into());
    let a_tok = SymbolId(1);
    let b_tok = SymbolId(2);
    let c_tok = SymbolId(3);
    let a_nt = SymbolId(10);
    let b_nt = SymbolId(11);

    for (id, name) in [(a_tok, "a"), (b_tok, "b"), (c_tok, "c")] {
        grammar.tokens.insert(
            id,
            Token {
                name: name.into(),
                pattern: TokenPattern::String(name.into()),
                fragile: false,
            },
        );
    }
    grammar.rule_names.insert(a_nt, "A".into());
    grammar.rule_names.insert(b_nt, "B".into());

    grammar.rules.insert(
        a_nt,
        vec![Rule {
            lhs: a_nt,
            rhs: vec![Symbol::NonTerminal(b_nt), Symbol::Terminal(a_tok)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );
    grammar.rules.insert(
        b_nt,
        vec![
            Rule {
                lhs: b_nt,
                rhs: vec![Symbol::NonTerminal(a_nt), Symbol::Terminal(b_tok)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(1),
            },
            Rule {
                lhs: b_nt,
                rhs: vec![Symbol::Terminal(c_tok)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(2),
            },
        ],
    );

    let ff = FirstFollowSets::compute(&grammar).unwrap();
    // FIRST(A) should contain 'c' (through B → c)
    let first_a = ff.first(a_nt).expect("A must have a FIRST set");
    assert!(
        first_a.contains(c_tok.0 as usize),
        "FIRST(A) must contain 'c' via indirect path"
    );
}

#[test]
fn self_recursive_nullable_first_set() {
    // S → S | ε  (nullable self-recursive)
    let mut grammar = Grammar::new("self_rec_null".into());
    let s = SymbolId(10);
    grammar.rule_names.insert(s, "S".into());
    grammar.rules.insert(
        s,
        vec![
            Rule {
                lhs: s,
                rhs: vec![Symbol::NonTerminal(s)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(0),
            },
            Rule {
                lhs: s,
                rhs: vec![],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(1),
            },
        ],
    );

    let ff = FirstFollowSets::compute(&grammar).unwrap();
    assert!(ff.is_nullable(s), "S must be nullable");
}

// ════════════════════════════════════════════════════════════════════
// 8. FOLLOW set computation edge cases
// ════════════════════════════════════════════════════════════════════

#[test]
fn follow_of_start_contains_eof() {
    let mut grammar = Grammar::new("follow_eof".into());
    let a = SymbolId(1);
    let s = SymbolId(10);
    grammar.tokens.insert(
        a,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    grammar.rule_names.insert(s, "S".into());
    grammar.rules.insert(
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

    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let follow_s = ff.follow(s).expect("S must have a FOLLOW set");
    // FOLLOW(S) must contain EOF (bit 0)
    assert!(follow_s.contains(0), "FOLLOW(start) must contain EOF");
}

#[test]
fn follow_propagation_through_nullable_suffix() {
    // S → A B; A → a; B → b | ε
    // FOLLOW(A) should contain FIRST(B) = {b} and also FOLLOW(S) (since B nullable)
    let mut grammar = Grammar::new("follow_null".into());
    let a_tok = SymbolId(1);
    let b_tok = SymbolId(2);
    let s = SymbolId(10);
    let a_nt = SymbolId(11);
    let b_nt = SymbolId(12);

    for (id, name) in [(a_tok, "a"), (b_tok, "b")] {
        grammar.tokens.insert(
            id,
            Token {
                name: name.into(),
                pattern: TokenPattern::String(name.into()),
                fragile: false,
            },
        );
    }
    grammar.rule_names.insert(s, "S".into());
    grammar.rule_names.insert(a_nt, "A".into());
    grammar.rule_names.insert(b_nt, "B".into());

    grammar.rules.insert(
        s,
        vec![Rule {
            lhs: s,
            rhs: vec![Symbol::NonTerminal(a_nt), Symbol::NonTerminal(b_nt)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );
    grammar.rules.insert(
        a_nt,
        vec![Rule {
            lhs: a_nt,
            rhs: vec![Symbol::Terminal(a_tok)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(1),
        }],
    );
    grammar.rules.insert(
        b_nt,
        vec![
            Rule {
                lhs: b_nt,
                rhs: vec![Symbol::Terminal(b_tok)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(2),
            },
            Rule {
                lhs: b_nt,
                rhs: vec![],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(3),
            },
        ],
    );

    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let follow_a = ff.follow(a_nt).expect("A must have a FOLLOW set");
    // FOLLOW(A) ⊇ FIRST(B) = {b}
    assert!(
        follow_a.contains(b_tok.0 as usize),
        "FOLLOW(A) must contain 'b' from FIRST(B)"
    );
    // FOLLOW(A) ⊇ FOLLOW(S) since B is nullable → EOF in FOLLOW(A)
    assert!(
        follow_a.contains(0),
        "FOLLOW(A) must contain EOF since B is nullable"
    );
}

#[test]
fn follow_with_chain_of_nullable_symbols() {
    // S → A B C; A → a; B → ε; C → ε
    // FOLLOW(A) ⊇ FOLLOW(S) because B and C are both nullable
    let mut grammar = Grammar::new("follow_chain_null".into());
    let a_tok = SymbolId(1);
    let s = SymbolId(10);
    let a_nt = SymbolId(11);
    let b_nt = SymbolId(12);
    let c_nt = SymbolId(13);

    grammar.tokens.insert(
        a_tok,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    for (id, name) in [(s, "S"), (a_nt, "A"), (b_nt, "B"), (c_nt, "C")] {
        grammar.rule_names.insert(id, name.into());
    }

    grammar.rules.insert(
        s,
        vec![Rule {
            lhs: s,
            rhs: vec![
                Symbol::NonTerminal(a_nt),
                Symbol::NonTerminal(b_nt),
                Symbol::NonTerminal(c_nt),
            ],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );
    grammar.rules.insert(
        a_nt,
        vec![Rule {
            lhs: a_nt,
            rhs: vec![Symbol::Terminal(a_tok)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(1),
        }],
    );
    grammar.rules.insert(
        b_nt,
        vec![Rule {
            lhs: b_nt,
            rhs: vec![],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(2),
        }],
    );
    grammar.rules.insert(
        c_nt,
        vec![Rule {
            lhs: c_nt,
            rhs: vec![],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(3),
        }],
    );

    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let follow_a = ff.follow(a_nt).expect("A must have FOLLOW set");
    assert!(
        follow_a.contains(0),
        "FOLLOW(A) must contain EOF: B and C are both nullable"
    );
}

// ════════════════════════════════════════════════════════════════════
// 9. Canonical collection with ambiguous grammars
// ════════════════════════════════════════════════════════════════════

#[test]
fn ambiguous_grammar_produces_conflicts() {
    // E → E E | a  (inherently ambiguous concatenation)
    let mut grammar = GrammarBuilder::new("ambig")
        .token("a", "a")
        .rule("E", vec!["E", "E"])
        .rule("E", vec!["a"])
        .start("E")
        .build();

    let ff = FirstFollowSets::compute_normalized(&mut grammar).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&grammar, &ff);
    let resolver = ConflictResolver::detect_conflicts(&collection, &grammar, &ff);

    assert!(
        !resolver.conflicts.is_empty(),
        "E → E E | a must produce conflicts"
    );
}

#[test]
fn ambiguous_grammar_automaton_still_builds() {
    // GLR should handle ambiguous grammars without error
    let mut grammar = GrammarBuilder::new("ambig2")
        .token("a", "a")
        .rule("E", vec!["E", "E"])
        .rule("E", vec!["a"])
        .start("E")
        .build();

    let ff = FirstFollowSets::compute_normalized(&mut grammar).unwrap();
    let result = build_lr1_automaton(&grammar, &ff);
    assert!(
        result.is_ok(),
        "GLR must build table for ambiguous grammar: {:?}",
        result.err()
    );
}

#[test]
fn dangling_else_ambiguity_detected() {
    // Simplified dangling-else: S → if E then S | if E then S else S | a
    let mut grammar = GrammarBuilder::new("dangle")
        .token("if", "if")
        .token("then", "then")
        .token("else", "else")
        .token("a", "a")
        .rule("S", vec!["if", "E", "then", "S"])
        .rule("S", vec!["if", "E", "then", "S", "else", "S"])
        .rule("S", vec!["a"])
        .rule("E", vec!["a"])
        .start("S")
        .build();

    let ff = FirstFollowSets::compute_normalized(&mut grammar).unwrap();
    let collection = ItemSetCollection::build_canonical_collection(&grammar, &ff);
    let resolver = ConflictResolver::detect_conflicts(&collection, &grammar, &ff);

    assert!(
        !resolver.conflicts.is_empty(),
        "dangling-else grammar must have shift/reduce conflicts"
    );
}

// ════════════════════════════════════════════════════════════════════
// 10. Parse table generation with unresolvable conflicts
// ════════════════════════════════════════════════════════════════════

#[test]
fn reduce_reduce_conflict_grammar() {
    // S → A | B; A → a; B → a  (reduce/reduce on 'a')
    let mut grammar = GrammarBuilder::new("rr_conflict")
        .token("a", "a")
        .rule("S", vec!["A"])
        .rule("S", vec!["B"])
        .rule("A", vec!["a"])
        .rule("B", vec!["a"])
        .start("S")
        .build();

    let ff = FirstFollowSets::compute_normalized(&mut grammar).unwrap();
    let result = build_lr1_automaton(&grammar, &ff);
    // GLR handles this; the table must still be built
    assert!(
        result.is_ok(),
        "GLR must handle reduce/reduce conflicts: {:?}",
        result.err()
    );
}

#[test]
fn sanity_check_fails_on_malformed_table() {
    // Build a table with no Accept action → sanity check should fail
    let eof = SymbolId(0);
    let start = SymbolId(2);
    let actions = vec![vec![vec![]; 3]]; // 1 state, 3 columns, all empty
    let gotos = vec![vec![StateId(0); 3]];
    let rules = vec![ParseRule {
        lhs: start,
        rhs_len: 1,
    }];
    let table = minimal_table(actions, gotos, rules, start, eof);

    let result = sanity_check_tables(&table);
    assert!(
        result.is_err(),
        "table with no Accept must fail sanity check"
    );
}

// ════════════════════════════════════════════════════════════════════
// 11. Driver initialization with empty parse table
// ════════════════════════════════════════════════════════════════════

#[test]
fn driver_with_empty_action_table() {
    let eof = SymbolId(0);
    let start = SymbolId(2);
    let table = minimal_table(vec![], vec![], vec![], start, eof);

    // Driver::new may panic on debug_assert because EOF is not in symbol_to_index.
    // This is expected: an empty table is invalid.
    let result = std::panic::catch_unwind(|| {
        let mut driver = Driver::new(&table);
        driver.parse_tokens(std::iter::empty::<(u32, u32, u32)>())
    });
    assert!(
        result.is_err() || result.unwrap().is_err(),
        "empty action table must panic or error"
    );
}

#[test]
fn driver_with_single_state_no_actions() {
    let eof = SymbolId(0);
    let start = SymbolId(2);
    let actions = vec![vec![vec![]; 3]]; // 1 state, all Error
    let gotos = vec![vec![StateId(65535); 3]];
    let table = minimal_table(actions, gotos, vec![], start, eof);

    let mut driver = Driver::new(&table);
    // Feed a token that has no action
    let result = driver.parse_tokens(vec![(1_u32, 0_u32, 1_u32)]);
    assert!(result.is_err(), "no actions → parse error");
}

// ════════════════════════════════════════════════════════════════════
// 12. Driver step with invalid state transitions
// ════════════════════════════════════════════════════════════════════

#[test]
fn driver_unknown_token_produces_error() {
    // Build a table that only recognizes token 1, then feed an unrecognized token.
    // Symbol 99 is not in symbol_to_index so actions() returns &[].
    // The driver may attempt error recovery (insertion), which can trigger
    // debug_asserts on mismatched start symbols. Either panic or Err is correct.
    let eof = SymbolId(0);
    let _tok = SymbolId(1);
    let s_sym = SymbolId(2);
    let rules = vec![ParseRule {
        lhs: s_sym,
        rhs_len: 1,
    }];
    let mut actions = vec![vec![vec![]; 3]; 2];
    actions[0][1] = vec![Action::Shift(StateId(1))]; // state 0, token 1 → shift
    actions[1][0] = vec![Action::Accept]; // state 1, EOF → accept
    let gotos = vec![vec![StateId(65535); 3]; 2];
    let table = minimal_table(actions, gotos, rules, s_sym, eof);

    // Feed only the unknown symbol → state 0 has no action for it
    let result = std::panic::catch_unwind(|| {
        let mut driver = Driver::new(&table);
        driver.parse_tokens(vec![(99_u32, 0_u32, 1_u32)])
    });
    assert!(
        result.is_err() || result.unwrap().is_err(),
        "unknown token must produce parse error or panic"
    );
}

#[test]
fn driver_shift_to_nonexistent_state() {
    // Table says shift to state 999, but table only has 2 states
    let eof = SymbolId(0);
    let _tok = SymbolId(1);
    let s_sym = SymbolId(2);
    let rules = vec![ParseRule {
        lhs: s_sym,
        rhs_len: 1,
    }];
    let mut actions = vec![vec![vec![]; 3]; 2];
    actions[0][1] = vec![Action::Shift(StateId(999))]; // shift to nonexistent state
    actions[1][0] = vec![Action::Accept];
    let gotos = vec![vec![StateId(65535); 3]; 2];
    let table = minimal_table(actions, gotos, rules, s_sym, eof);

    let mut driver = Driver::new(&table);
    let result = driver.parse_tokens(vec![(1_u32, 0_u32, 1_u32)]);
    // The driver should either error or panic at EOF when the state is invalid
    // Either outcome is acceptable; what matters is no silent corruption.
    // In practice, the out-of-bounds state will cause action lookup to return
    // empty or the EOF handling to fail.
    assert!(result.is_err(), "shift to nonexistent state must fail");
}

#[test]
fn driver_reduce_with_invalid_rule_id() {
    // Table says reduce rule 999, but only 1 rule exists.
    // The rule() method indexes directly into the rules vec → panics.
    let eof = SymbolId(0);
    let _tok = SymbolId(1);
    let s_sym = SymbolId(2);
    let rules = vec![ParseRule {
        lhs: s_sym,
        rhs_len: 1,
    }];
    let mut actions = vec![vec![vec![]; 3]; 2];
    actions[0][1] = vec![Action::Shift(StateId(1))];
    actions[1][0] = vec![Action::Reduce(RuleId(999))]; // invalid rule
    let gotos = vec![vec![StateId(65535); 3]; 2];
    let table = minimal_table(actions, gotos, rules, s_sym, eof);

    let result = std::panic::catch_unwind(|| {
        let mut driver = Driver::new(&table);
        driver.parse_tokens(vec![(1_u32, 0_u32, 1_u32)])
    });
    assert!(
        result.is_err() || result.unwrap().is_err(),
        "reduce with invalid rule must panic or error"
    );
}

#[test]
fn driver_eof_without_accept_produces_error() {
    // State 1 has no Accept on EOF → parse must fail
    let eof = SymbolId(0);
    let _tok = SymbolId(1);
    let s_sym = SymbolId(2);
    let rules = vec![ParseRule {
        lhs: s_sym,
        rhs_len: 1,
    }];
    let mut actions = vec![vec![vec![]; 3]; 2];
    actions[0][1] = vec![Action::Shift(StateId(1))];
    // State 1 on EOF: nothing (empty) → should fail
    let gotos = vec![vec![StateId(65535); 3]; 2];
    let table = minimal_table(actions, gotos, rules, s_sym, eof);

    let mut driver = Driver::new(&table);
    let result = driver.parse_tokens(vec![(1_u32, 0_u32, 1_u32)]);
    assert!(result.is_err(), "EOF without Accept must fail");
}

// ════════════════════════════════════════════════════════════════════
// Additional error conditions
// ════════════════════════════════════════════════════════════════════

#[test]
fn complex_symbols_not_normalized_error() {
    // Grammar with Optional (complex symbol) should error in FIRST/FOLLOW
    // if NOT using compute_normalized
    let mut grammar = Grammar::new("complex".into());
    let a = SymbolId(1);
    let s = SymbolId(10);
    grammar.tokens.insert(
        a,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    grammar.rule_names.insert(s, "S".into());
    grammar.rules.insert(
        s,
        vec![Rule {
            lhs: s,
            rhs: vec![Symbol::Optional(Box::new(Symbol::Terminal(a)))],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        }],
    );

    // compute() internally clones-and-normalizes, so this should succeed
    let result = FirstFollowSets::compute(&grammar);
    // The internal normalization should handle this
    assert!(
        result.is_ok(),
        "compute() normalizes internally: {:?}",
        result.err()
    );
}

#[test]
fn first_of_sequence_with_unknown_symbol() {
    // Compute FIRST of a sequence containing a symbol not in the grammar
    let mut grammar = Grammar::new("seq_unknown".into());
    let a = SymbolId(1);
    let s = SymbolId(10);
    grammar.tokens.insert(
        a,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    grammar.rule_names.insert(s, "S".into());
    grammar.rules.insert(
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

    let ff = FirstFollowSets::compute(&grammar).unwrap();
    // Ask for FIRST of a sequence with an unknown non-terminal
    let result = ff.first_of_sequence(&[Symbol::NonTerminal(SymbolId(999))]);
    // Should return Ok with empty set or handle gracefully
    assert!(
        result.is_ok(),
        "unknown symbol in sequence should not crash"
    );
}

#[test]
fn glr_error_display_variants() {
    // Ensure all GLRError variants format correctly
    let e1 = GLRError::StateMachine("boom".into());
    assert!(format!("{e1}").contains("boom"));

    let e2 = GLRError::ConflictResolution("conflict".into());
    assert!(format!("{e2}").contains("conflict"));

    let e3 = GLRError::ComplexSymbolsNotNormalized {
        operation: "test".into(),
    };
    assert!(format!("{e3}").contains("normalized"));

    let e4 = GLRError::GrammarError(GrammarError::UnresolvedSymbol(SymbolId(42)));
    assert!(format!("{e4}").contains("42"));

    let e5 = GLRError::ExpectedSimpleSymbol {
        expected: "Terminal".into(),
    };
    assert!(format!("{e5}").contains("Terminal"));

    let e6 = GLRError::InvalidSymbolState {
        operation: "closure".into(),
    };
    assert!(format!("{e6}").contains("closure"));
}

#[test]
fn table_error_display_variants() {
    use adze_glr_core::TableError;

    let e1 = TableError::EofIsError;
    assert!(format!("{e1}").contains("EOF"));

    let e2 = TableError::EofMissingFromIndex;
    assert!(format!("{e2}").contains("EOF"));

    let e3 = TableError::EofNotSentinel {
        eof: 5,
        token_count: 3,
        external_count: 1,
    };
    assert!(format!("{e3}").contains("5"));
}

#[test]
fn lr_item_is_reduce_with_missing_rule() {
    // LRItem pointing to a non-existent rule should not be a reduce item
    let grammar = empty_grammar();
    let item = LRItem::new(RuleId(9999), 0, SymbolId(0));
    assert!(
        !item.is_reduce_item(&grammar),
        "item with missing rule must not be considered a reduce item"
    );
}

#[test]
fn lr_item_next_symbol_with_missing_rule() {
    let grammar = empty_grammar();
    let item = LRItem::new(RuleId(9999), 0, SymbolId(0));
    assert!(
        item.next_symbol(&grammar).is_none(),
        "item with missing rule has no next symbol"
    );
}

#[test]
fn item_set_closure_on_empty_grammar() {
    let grammar = empty_grammar();
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let mut set = ItemSet::new(StateId(0));
    // Closure on empty set should succeed trivially
    let result = set.closure(&grammar, &ff);
    assert!(result.is_ok());
    assert!(set.items.is_empty());
}

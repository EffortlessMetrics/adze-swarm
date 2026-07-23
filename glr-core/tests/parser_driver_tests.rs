//! Tests for the GLR parser driver (`adze_glr_core::driver`).
//!
//! Covers driver creation, token parsing, EOF handling, error paths,
//! shift-reduce / reduce-reduce conflicts, ambiguity, and driver reuse.
//!
//! Note: These tests use manually constructed parse tables that don't satisfy
//! all strict invariants (e.g., EOF/END parity). They are only compiled when
//! the `strict-invariants` feature is disabled.

#![cfg(not(feature = "strict-invariants"))]

use adze_glr_core::conflict_inspection::{ConflictType, cell_has_conflict, classify_conflict};
use adze_glr_core::driver::GlrError;
use adze_glr_core::{
    Action, Driver, Forest, GLRError, LexMode, ParseRule, ParseTable, SymbolMetadata,
};
use adze_ir::{Grammar, RuleId, StateId, SymbolId};
use std::collections::BTreeMap;

// ─── Helpers ─────────────────────────────────────────────────────────

/// Build a hand-crafted ParseTable from raw action/goto matrices.
///
/// `states`: `[state][symbol_col] -> ActionCell`
/// `gotos`:  `[state][nonterminal_col] -> StateId`  (use `INV` for no-goto)
fn make_table(
    states: Vec<Vec<Vec<Action>>>,
    gotos: Vec<Vec<StateId>>,
    rules: Vec<ParseRule>,
    start: SymbolId,
    eof: SymbolId,
    terminal_count: usize,
) -> ParseTable {
    let symbol_count = states.first().map(|s| s.len()).unwrap_or(0);
    let state_count = states.len();

    let mut symbol_to_index = BTreeMap::new();
    for i in 0..symbol_count {
        symbol_to_index.insert(SymbolId(i as u16), i);
    }

    let mut nonterminal_to_index = BTreeMap::new();
    let inv = StateId(65535);
    for i in 0..gotos.first().map(|g| g.len()).unwrap_or(0) {
        for row in &gotos {
            if row[i] != inv {
                nonterminal_to_index.insert(SymbolId(i as u16), i);
                break;
            }
        }
    }

    let sym_meta = vec![
        SymbolMetadata {
            name: String::new(),
            is_visible: false,
            is_named: false,
            is_supertype: false,
            is_terminal: false,
            is_extra: false,
            is_fragile: false,
            symbol_id: SymbolId(0),
        };
        symbol_count
    ];

    ParseTable {
        action_table: states,
        goto_table: gotos,
        rules,
        state_count,
        symbol_count,
        symbol_to_index,
        index_to_symbol: (0..terminal_count as u16).map(SymbolId).collect(),
        nonterminal_to_index,
        eof_symbol: eof,
        start_symbol: start,
        grammar: Grammar::new("test".to_string()),
        symbol_metadata: sym_meta,
        initial_state: StateId(0),
        token_count: terminal_count,
        external_token_count: 0,
        lex_modes: vec![
            LexMode {
                lex_state: 0,
                external_lex_state: 0,
            };
            state_count
        ],
        extras: vec![],
        dynamic_prec_by_rule: vec![0; 64],
        rule_assoc_by_rule: vec![0; 64],
        alias_sequences: vec![],
        field_names: vec![],
        goto_indexing: adze_glr_core::GotoIndexing::NonterminalMap,
        field_map: BTreeMap::new(),
        external_scanner_states: vec![],
    }
}

const INV: StateId = StateId(65535);

/// Grammar:  S → a          (single terminal)
/// Symbols:  0=EOF  1=a  2=S
/// States:   0: shift a→1         goto S→2
///           1: reduce(0) on EOF
///           2: accept on EOF
fn single_token_table() -> ParseTable {
    let eof = SymbolId(0);
    let s = SymbolId(2);

    let rules = vec![ParseRule { lhs: s, rhs_len: 1 }];

    let actions = vec![
        vec![vec![], vec![Action::Shift(StateId(1))], vec![]],
        vec![vec![Action::Reduce(RuleId(0))], vec![], vec![]],
        vec![vec![Action::Accept], vec![], vec![]],
    ];
    let gotos = vec![
        vec![INV, INV, StateId(2)],
        vec![INV, INV, INV],
        vec![INV, INV, INV],
    ];

    make_table(actions, gotos, rules, s, eof, 2)
}

/// Grammar:  S → a b        (two terminals)
/// Symbols:  0=EOF  1=a  2=b  3=S
fn two_token_table() -> ParseTable {
    let eof = SymbolId(0);
    let s = SymbolId(3);

    let rules = vec![ParseRule { lhs: s, rhs_len: 2 }];

    let actions = vec![
        vec![vec![], vec![Action::Shift(StateId(1))], vec![], vec![]],
        vec![vec![], vec![], vec![Action::Shift(StateId(2))], vec![]],
        vec![vec![Action::Reduce(RuleId(0))], vec![], vec![], vec![]],
        vec![vec![Action::Accept], vec![], vec![], vec![]],
    ];
    let gotos = vec![
        vec![INV, INV, INV, StateId(3)],
        vec![INV, INV, INV, INV],
        vec![INV, INV, INV, INV],
        vec![INV, INV, INV, INV],
    ];

    make_table(actions, gotos, rules, s, eof, 3)
}

/// Parse helper — feeds token tuples through a Driver.
fn parse_with(table: &ParseTable, tokens: &[(u32, u32, u32)]) -> Result<Forest, GlrError> {
    let mut driver = Driver::new(table);
    driver.parse_tokens(tokens.iter().copied())
}

// ═══════════════════════════════════════════════════════════════════════
// 1. Driver creation
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn driver_creation_from_single_token_table() {
    let table = single_token_table();
    let _driver = Driver::new(&table);
}

#[test]
fn driver_creation_from_two_token_table() {
    let table = two_token_table();
    let _driver = Driver::new(&table);
}

#[test]
fn driver_creation_from_default_table() {
    // Default table has eof_symbol = SymbolId(0) but empty action table,
    // so we must at least put EOF in symbol_to_index for the debug_assert.
    let mut table = ParseTable::default();
    table.symbol_to_index.insert(SymbolId(0), 0);
    let _driver = Driver::new(&table);
}

// ═══════════════════════════════════════════════════════════════════════
// 2. Parsing empty input
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn empty_input_errors_when_grammar_requires_token() {
    let table = single_token_table();
    let result = parse_with(&table, &[]);
    assert!(result.is_err(), "grammar S→a must reject empty input");
}

#[test]
fn empty_input_error_message_mentions_eof() {
    let table = single_token_table();
    let err = parse_with(&table, &[]).err().expect("should be Err");
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("eof") || msg.contains("not accepted") || msg.contains("no valid"),
        "error should mention EOF / not accepted: {msg}"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// 3. Parsing single token
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn single_token_parse_succeeds() {
    let table = single_token_table();
    let forest = parse_with(&table, &[(1, 0, 1)]).expect("S→a should accept 'a'");
    let view = forest.view();
    assert!(
        !view.roots().is_empty(),
        "forest must have at least one root"
    );
}

#[test]
fn single_token_root_span_correct() {
    let table = single_token_table();
    let forest = parse_with(&table, &[(1, 0, 5)]).unwrap();
    let view = forest.view();
    let root = view.roots()[0];
    assert_eq!(view.span(root).start, 0);
    assert_eq!(view.span(root).end, 5);
}

#[test]
fn single_token_root_has_children() {
    let table = single_token_table();
    let forest = parse_with(&table, &[(1, 0, 1)]).unwrap();
    let view = forest.view();
    let root = view.roots()[0];
    let children = view.best_children(root);
    // S → a  means root should have the terminal as a child
    assert!(
        !children.is_empty(),
        "root node S should have child terminal 'a'"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// 4. Parsing multiple tokens
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn two_token_parse_succeeds() {
    let table = two_token_table();
    let forest = parse_with(&table, &[(1, 0, 1), (2, 1, 2)]).expect("S→a b should accept 'ab'");
    let view = forest.view();
    assert!(!view.roots().is_empty());
}

#[test]
fn two_token_span_covers_full_input() {
    let table = two_token_table();
    let forest = parse_with(&table, &[(1, 0, 3), (2, 3, 7)]).unwrap();
    let view = forest.view();
    let root = view.roots()[0];
    assert_eq!(view.span(root).start, 0);
    assert_eq!(view.span(root).end, 7);
}

#[test]
fn two_token_root_has_two_children() {
    let table = two_token_table();
    let forest = parse_with(&table, &[(1, 0, 1), (2, 1, 2)]).unwrap();
    let view = forest.view();
    let root = view.roots()[0];
    assert_eq!(
        view.best_children(root).len(),
        2,
        "S → a b should have 2 children"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// 5. Shift-reduce conflict (GLR fork)
// ═══════════════════════════════════════════════════════════════════════

/// Grammar with a shift-reduce conflict:
///   S → E          rule 0
///   E → E a        rule 1   (left-recursive)
///   E → a          rule 2
///
/// Symbols: 0=EOF 1=a 2=S 3=E
///
/// State 2 on 'a': Shift(4) AND Reduce(2).
/// This is a classic shift-reduce conflict that GLR handles by forking.
fn shift_reduce_table() -> ParseTable {
    let eof = SymbolId(0);
    let s = SymbolId(2);
    let e = SymbolId(3);

    let rules = vec![
        ParseRule { lhs: s, rhs_len: 1 }, // S → E
        ParseRule { lhs: e, rhs_len: 2 }, // E → E a
        ParseRule { lhs: e, rhs_len: 1 }, // E → a
    ];

    // 5 states:
    //  0: shift a→1, goto E→2, goto S→3
    //  1: reduce(2) on EOF, reduce(2) on a
    //  2: shift a→4 AND reduce on EOF  (conflict on 'a': shift+reduce)
    //  3: accept on EOF
    //  4: reduce(1) on EOF, reduce(1) on a
    let actions = vec![
        // state 0
        vec![vec![], vec![Action::Shift(StateId(1))], vec![], vec![]],
        // state 1: after seeing first 'a', reduce E→a
        vec![
            vec![Action::Reduce(RuleId(2))],
            vec![Action::Reduce(RuleId(2))],
            vec![],
            vec![],
        ],
        // state 2: after goto E  — shift-reduce conflict on 'a'
        vec![
            vec![Action::Reduce(RuleId(0))], // EOF: reduce S→E
            vec![Action::Shift(StateId(4))], // 'a': shift (extend E→E a)
            vec![],
            vec![],
        ],
        // state 3: accept
        vec![vec![Action::Accept], vec![], vec![], vec![]],
        // state 4: after E a, reduce E→E a
        vec![
            vec![Action::Reduce(RuleId(1))],
            vec![Action::Reduce(RuleId(1))],
            vec![],
            vec![],
        ],
    ];

    let gotos = vec![
        vec![INV, INV, StateId(3), StateId(2)], // state 0: S→3, E→2
        vec![INV, INV, INV, INV],
        vec![INV, INV, INV, INV],
        vec![INV, INV, INV, INV],
        vec![INV, INV, INV, INV],
    ];

    make_table(actions, gotos, rules, s, eof, 2)
}

#[test]
fn shift_reduce_single_a_parses() {
    let table = shift_reduce_table();
    let forest = parse_with(&table, &[(1, 0, 1)]).expect("single 'a' should parse");
    assert!(!forest.view().roots().is_empty());
}

#[test]
fn shift_reduce_two_a_parses() {
    let table = shift_reduce_table();
    let forest = parse_with(&table, &[(1, 0, 1), (1, 1, 2)]).expect("'a a' should parse via E→E a");
    let view = forest.view();
    let root = view.roots()[0];
    assert_eq!(view.span(root).start, 0);
    assert_eq!(view.span(root).end, 2);
}

#[test]
fn shift_reduce_three_a_parses() {
    let table = shift_reduce_table();
    let forest = parse_with(&table, &[(1, 0, 1), (1, 1, 2), (1, 2, 3)])
        .expect("'a a a' should parse via repeated E→E a");
    let view = forest.view();
    let root = view.roots()[0];
    assert_eq!(view.span(root).end, 3);
}

// ═══════════════════════════════════════════════════════════════════════
// 6. Reduce-reduce conflict
// ═══════════════════════════════════════════════════════════════════════

/// Grammar with a reduce-reduce conflict:
///   S → A | B       (rule 0: S→A, rule 1: S→B)
///   A → a           (rule 2)
///   B → a           (rule 3)
///
/// Symbols: 0=EOF  1=a  2=S  3=A  4=B
///
/// After shifting 'a' (state 1), both Reduce(2) and Reduce(3) are valid on EOF.
fn reduce_reduce_table() -> ParseTable {
    let eof = SymbolId(0);
    let s = SymbolId(2);
    let a_nt = SymbolId(3);
    let b_nt = SymbolId(4);

    let rules = vec![
        ParseRule { lhs: s, rhs_len: 1 }, // S → A
        ParseRule { lhs: s, rhs_len: 1 }, // S → B
        ParseRule {
            lhs: a_nt,
            rhs_len: 1,
        }, // A → a
        ParseRule {
            lhs: b_nt,
            rhs_len: 1,
        }, // B → a
    ];

    // States:
    //  0: shift a→1, goto A→2, goto B→3, goto S→4
    //  1: reduce(2) AND reduce(3) on EOF  ← reduce-reduce conflict
    //  2: reduce(0) on EOF  (S→A)
    //  3: reduce(1) on EOF  (S→B)
    //  4: accept on EOF
    let actions = vec![
        // state 0
        vec![
            vec![],
            vec![Action::Shift(StateId(1))],
            vec![],
            vec![],
            vec![],
        ],
        // state 1: reduce-reduce conflict
        vec![
            vec![Action::Reduce(RuleId(2)), Action::Reduce(RuleId(3))],
            vec![],
            vec![],
            vec![],
            vec![],
        ],
        // state 2: S→A
        vec![
            vec![Action::Reduce(RuleId(0))],
            vec![],
            vec![],
            vec![],
            vec![],
        ],
        // state 3: S→B
        vec![
            vec![Action::Reduce(RuleId(1))],
            vec![],
            vec![],
            vec![],
            vec![],
        ],
        // state 4: accept
        vec![vec![Action::Accept], vec![], vec![], vec![], vec![]],
    ];

    let gotos = vec![
        vec![INV, INV, StateId(4), StateId(2), StateId(3)],
        vec![INV, INV, INV, INV, INV],
        vec![INV, INV, INV, INV, INV],
        vec![INV, INV, INV, INV, INV],
        vec![INV, INV, INV, INV, INV],
    ];

    make_table(actions, gotos, rules, s, eof, 2)
}

#[test]
fn reduce_reduce_parses_despite_conflict() {
    let table = reduce_reduce_table();
    let conflict_cell = &table.action_table[1][0];

    assert!(cell_has_conflict(conflict_cell));
    assert_eq!(classify_conflict(conflict_cell), ConflictType::ReduceReduce);

    let forest = parse_with(&table, &[(1, 0, 1)])
        .expect("reduce-reduce conflict should still produce a parse");
    assert!(!forest.view().roots().is_empty());
}

#[test]
fn reduce_reduce_forest_retains_multiple_roots() {
    let table = reduce_reduce_table();
    let forest = parse_with(&table, &[(1, 0, 1)])
        .expect("reduce-reduce conflict should still produce a parse");
    assert!(
        forest.view().roots().len() >= 2,
        "reduce/reduce should retain multiple complete roots for ambiguity materialization"
    );
}

#[test]
fn reduce_reduce_root_span() {
    let table = reduce_reduce_table();
    let forest = parse_with(&table, &[(1, 0, 1)]).unwrap();
    let view = forest.view();
    let root = view.roots()[0];
    assert_eq!(view.span(root).start, 0);
    assert_eq!(view.span(root).end, 1);
}

// ═══════════════════════════════════════════════════════════════════════
// 7. EOF handling
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn eof_triggers_accept_on_valid_input() {
    let table = single_token_table();
    // After the token stream ends the driver enters the EOF phase
    // which should find Accept in the table and succeed.
    let result = parse_with(&table, &[(1, 0, 1)]);
    assert!(result.is_ok(), "valid input should accept at EOF");
}

#[test]
fn eof_symbol_present_in_table() {
    let table = single_token_table();
    assert!(
        table.symbol_to_index.contains_key(&table.eof_symbol),
        "EOF symbol must be indexed"
    );
}

#[test]
fn eof_distinct_from_start_symbol() {
    let table = single_token_table();
    assert_ne!(
        table.eof_symbol, table.start_symbol,
        "EOF and start symbol must be distinct"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// 8. Error on invalid token
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn invalid_token_kind_errors_or_recovers() {
    let table = single_token_table();
    // Symbol 99 does not exist in the grammar.
    // The driver may either reject outright or attempt error recovery.
    // Either outcome is acceptable for a GLR parser with recovery.
    let _result = parse_with(&table, &[(99, 0, 1)]);
}

#[test]
fn wrong_token_sequence_errors_or_recovers() {
    let table = two_token_table();
    // Grammar expects a(1) b(2), feed b(2) a(1).
    // GLR driver may recover via insertion; we just verify no panic.
    let _result = parse_with(&table, &[(2, 0, 1), (1, 1, 2)]);
}

#[test]
fn extra_tokens_errors_or_recovers() {
    let table = single_token_table();
    // Feed two tokens when grammar only accepts one.
    // GLR driver may recover; we just verify no panic.
    let _result = parse_with(&table, &[(1, 0, 1), (1, 1, 2)]);
}

// ═══════════════════════════════════════════════════════════════════════
// 9. Ambiguity / multiple parse trees
// ═══════════════════════════════════════════════════════════════════════

/// Build an ambiguous grammar via the pipeline:
///   S → S S | a
/// Input "a a" can be parsed as (S (S a) (S a)) or differently
/// when more tokens are present. Even with two tokens the grammar
/// is unambiguous, but the table construction handles the conflict.
fn ambiguous_grammar_table() -> Result<ParseTable, GLRError> {
    use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
    use adze_ir::builder::GrammarBuilder;

    let mut grammar = GrammarBuilder::new("ambig")
        .token("a", "a")
        .rule("S", vec!["S", "S"])
        .rule("S", vec!["a"])
        .start("S")
        .build();

    let ff = FirstFollowSets::compute_normalized(&mut grammar)?;
    build_lr1_automaton(&grammar, &ff)
}

#[test]
fn ambiguous_grammar_table_builds() {
    ambiguous_grammar_table().expect("ambiguous grammar should build a parse table");
}

#[test]
fn ambiguous_grammar_two_tokens_parse() {
    let table = ambiguous_grammar_table().unwrap();
    adze_glr_core::sanity_check_tables(&table).expect("sanity");

    let a_id = {
        let mut id = None;
        for (&sym, tok) in &table.grammar.tokens {
            if tok.name == "a" {
                id = Some(sym);
            }
        }
        id.expect("token 'a' should exist")
    };

    let mut driver = Driver::new(&table);
    let forest = driver
        .parse_tokens(
            [(a_id.0 as u32, 0u32, 1u32), (a_id.0 as u32, 1, 2)]
                .iter()
                .copied(),
        )
        .expect("S→S S | a should parse 'a a'");

    let view = forest.view();
    assert!(!view.roots().is_empty(), "should produce at least one root");
    let root = view.roots()[0];
    assert_eq!(view.span(root).start, 0);
    assert_eq!(view.span(root).end, 2);
}

#[test]
fn ambiguous_grammar_small_input_remains_under_active_stack_limit() {
    let table = ambiguous_grammar_table().unwrap();

    let a_id = {
        let mut id = None;
        for (&sym, tok) in &table.grammar.tokens {
            if tok.name == "a" {
                id = Some(sym);
            }
        }
        id.expect("token 'a' should exist")
    };

    let tokens = (0..4).map(|i| (a_id.0 as u32, i, i + 1));
    let mut driver = Driver::new(&table);
    let forest = driver
        .parse_tokens(tokens)
        .expect("ambiguous grammar should parse short input within stack cap");

    let view = forest.view();
    let root = view.roots()[0];
    assert_eq!(view.span(root).start, 0);
    assert_eq!(view.span(root).end, 4);
}

#[test]
fn ambiguous_grammar_large_input_hits_active_stack_limit() {
    let mut symbol_to_index = std::collections::BTreeMap::new();
    symbol_to_index.insert(SymbolId(0), 0);
    symbol_to_index.insert(SymbolId(1), 1);

    let table = ParseTable {
        state_count: 1,
        symbol_count: 2,
        symbol_to_index,
        start_symbol: SymbolId(1),
        index_to_symbol: vec![SymbolId(0), SymbolId(1)],
        action_table: vec![vec![
            vec![Action::Accept],
            vec![Action::Fork(vec![
                Action::Shift(StateId(0)),
                Action::Shift(StateId(0)),
            ])],
        ]],
        goto_table: vec![vec![]],
        ..Default::default()
    };

    let tokens = (0..11).map(|i| (1_u32, i, i + 1));
    let mut driver = Driver::new(&table);
    let err = match driver.parse_tokens(tokens) {
        Ok(_) => panic!("ambiguous input should trip the active stack guard"),
        Err(err) => err,
    };

    assert!(
        err.to_string().contains("active stack limit"),
        "unexpected error: {err}"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// 10. Driver reset / reuse
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn driver_reuse_parses_twice_independently() {
    let table = single_token_table();
    let mut driver = Driver::new(&table);

    let f1 = driver
        .parse_tokens([(1, 0, 1)].iter().copied())
        .expect("first parse");
    let f2 = driver
        .parse_tokens([(1, 0, 5)].iter().copied())
        .expect("second parse");

    // Each parse should return an independent forest.
    let v1 = f1.view();
    let v2 = f2.view();
    assert_eq!(v1.span(v1.roots()[0]).end, 1);
    assert_eq!(v2.span(v2.roots()[0]).end, 5);
}

#[test]
fn driver_reuse_after_error() {
    let table = single_token_table();
    let mut driver = Driver::new(&table);

    // First parse fails.
    let err = driver.parse_tokens(std::iter::empty::<(u32, u32, u32)>());
    assert!(err.is_err());

    // Second parse should still work.
    let ok = driver.parse_tokens([(1, 0, 1)].iter().copied());
    assert!(ok.is_ok(), "driver should be reusable after an error");
}

#[test]
fn driver_reuse_multiple_successes() {
    let table = two_token_table();
    let mut driver = Driver::new(&table);

    for i in 0..5 {
        let start = i * 10;
        let mid = start + 3;
        let end = start + 7;
        let forest = driver
            .parse_tokens([(1, start, mid), (2, mid, end)].iter().copied())
            .unwrap_or_else(|e| panic!("iteration {i} failed: {e}"));
        let view = forest.view();
        assert_eq!(view.span(view.roots()[0]).start, start);
        assert_eq!(view.span(view.roots()[0]).end, end);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Additional coverage
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn forest_view_kind_returns_start_symbol_for_root() {
    let table = single_token_table();
    let forest = parse_with(&table, &[(1, 0, 1)]).unwrap();
    let view = forest.view();
    let root = view.roots()[0];
    // The root's kind should be the start symbol (SymbolId(2) == S).
    assert_eq!(
        view.kind(root),
        2, // S = SymbolId(2) → forest_view::SymbolId is u32
        "root kind should be the start symbol"
    );
}

#[test]
fn glr_error_display_lex() {
    let err = GlrError::Lex("bad char".into());
    assert!(err.to_string().contains("bad char"));
}

#[test]
fn glr_error_display_parse() {
    let err = GlrError::Parse("unexpected token".into());
    assert!(err.to_string().contains("unexpected token"));
}

#[test]
fn glr_error_display_other() {
    let err = GlrError::Other("misc".into());
    assert!(err.to_string().contains("misc"));
}

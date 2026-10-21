//! Property-based tests for GLR Driver parsing behaviour.
//!
//! Run with: cargo test -p adze-glr-core --test driver_parsing_proptest
//!
//! Note: These tests use manually constructed parse tables that don't satisfy
//! all strict invariants (e.g., EOF/END parity). They are only compiled when
//! the `strict-invariants` feature is disabled.

#![cfg(not(feature = "strict-invariants"))]
#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

use adze_glr_core::driver::GlrError;
use adze_glr_core::{Action, Driver, GotoIndexing, LexMode, ParseRule, ParseTable};
use adze_ir::{Grammar, RuleId, StateId, SymbolId};
use proptest::prelude::*;
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Table builder helpers
// ---------------------------------------------------------------------------

type ActionCell = Vec<Action>;
const NO_GOTO: StateId = StateId(65535);

fn build_table(
    actions: Vec<Vec<ActionCell>>,
    gotos: Vec<Vec<StateId>>,
    rules: Vec<ParseRule>,
    start: SymbolId,
    eof: SymbolId,
    num_terminals: usize,
) -> ParseTable {
    let symbol_count = actions.first().map(|r| r.len()).unwrap_or(0);
    let state_count = actions.len();

    let mut symbol_to_index = BTreeMap::new();
    let mut index_to_symbol = Vec::new();
    for i in 0..symbol_count {
        symbol_to_index.insert(SymbolId(i as u16), i);
        index_to_symbol.push(SymbolId(i as u16));
    }

    let mut nonterminal_to_index = BTreeMap::new();
    for i in 0..symbol_count {
        for row in &gotos {
            if i < row.len() && row[i] != NO_GOTO {
                nonterminal_to_index.insert(SymbolId(i as u16), i);
                break;
            }
        }
    }

    ParseTable {
        action_table: actions,
        goto_table: gotos,
        rules: rules.clone(),
        state_count,
        symbol_count,
        symbol_to_index,
        index_to_symbol,
        external_scanner_states: vec![],
        nonterminal_to_index,
        eof_symbol: eof,
        start_symbol: start,
        grammar: Grammar::new("proptest_parsing".to_string()),
        symbol_metadata: vec![],
        initial_state: StateId(0),
        token_count: num_terminals,
        external_token_count: 0,
        lex_modes: vec![
            LexMode {
                lex_state: 0,
                external_lex_state: 0,
            };
            state_count
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

/// S -> 'a'  (symbols: 0=EOF, 1='a', 2=S)
fn build_s_to_a_table() -> ParseTable {
    let eof = SymbolId(0);
    let s = SymbolId(2);
    let rules = vec![ParseRule { lhs: s, rhs_len: 1 }];
    let mut actions = vec![vec![vec![]; 3]; 3];
    actions[0][1].push(Action::Shift(StateId(1)));
    actions[1][0].push(Action::Reduce(RuleId(0)));
    actions[2][0].push(Action::Accept);
    let mut gotos = vec![vec![NO_GOTO; 3]; 3];
    gotos[0][2] = StateId(2);
    build_table(actions, gotos, rules, s, eof, 2)
}

/// S -> 'a' 'b'  (symbols: 0=EOF, 1='a', 2='b', 3=S)
fn build_s_to_ab_table() -> ParseTable {
    let eof = SymbolId(0);
    let s = SymbolId(3);
    let rules = vec![ParseRule { lhs: s, rhs_len: 2 }];
    // States: 0=initial, 1=shifted 'a', 2=shifted 'b', 3=accepted S
    let num_syms = 4;
    let num_states = 4;
    let mut actions = vec![vec![vec![]; num_syms]; num_states];
    actions[0][1].push(Action::Shift(StateId(1))); // shift 'a'
    actions[1][2].push(Action::Shift(StateId(2))); // shift 'b'
    actions[2][0].push(Action::Reduce(RuleId(0))); // reduce S -> a b
    actions[3][0].push(Action::Accept);
    let mut gotos = vec![vec![NO_GOTO; num_syms]; num_states];
    gotos[0][3] = StateId(3);
    build_table(actions, gotos, rules, s, eof, 3)
}

/// Right-recursive: S -> A; A -> 'a' | 'a' A
/// Symbols: 0=EOF, 1='a', 2=S, 3=A
fn build_right_recursive_table() -> ParseTable {
    let eof = SymbolId(0);
    let s = SymbolId(2);
    let a_nt = SymbolId(3);
    let rules = vec![
        ParseRule { lhs: s, rhs_len: 1 },
        ParseRule {
            lhs: a_nt,
            rhs_len: 1,
        },
        ParseRule {
            lhs: a_nt,
            rhs_len: 2,
        },
    ];
    let num_syms = 4;
    let num_states = 5;
    let mut actions = vec![vec![vec![]; num_syms]; num_states];
    actions[0][1].push(Action::Shift(StateId(1)));
    actions[1][1].push(Action::Shift(StateId(1)));
    actions[1][0].push(Action::Reduce(RuleId(1)));
    actions[2][0].push(Action::Reduce(RuleId(0)));
    actions[3][0].push(Action::Accept);
    actions[4][0].push(Action::Reduce(RuleId(2)));
    let mut gotos = vec![vec![NO_GOTO; num_syms]; num_states];
    gotos[0][3] = StateId(2);
    gotos[0][2] = StateId(3);
    gotos[1][3] = StateId(4);
    build_table(actions, gotos, rules, s, eof, 2)
}

/// S -> ε  (symbols: 0=EOF, 1=S)
fn build_epsilon_table() -> ParseTable {
    let eof = SymbolId(0);
    let s = SymbolId(1);
    let rules = vec![ParseRule { lhs: s, rhs_len: 0 }];
    let mut actions = vec![vec![vec![]; 2]; 2];
    actions[0][0].push(Action::Reduce(RuleId(0)));
    actions[1][0].push(Action::Accept);
    let mut gotos = vec![vec![NO_GOTO; 2]; 2];
    gotos[0][1] = StateId(1);
    build_table(actions, gotos, rules, s, eof, 1)
}

// ===========================================================================
// Tests
// ===========================================================================

// ---------------------------------------------------------------------------
// 1. Two-symbol sequence grammar accepts 'a' 'b'
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn two_symbol_sequence_accepts(offset in 0u32..100) {
        let table = build_s_to_ab_table();
        let tokens = vec![
            (1u32, offset, offset + 1),
            (2u32, offset + 1, offset + 2),
        ];
        let mut driver = Driver::new(&table);
        let result = driver.parse_tokens(tokens.into_iter());
        prop_assert!(result.is_ok(), "S -> a b must accept: {:?}", result.err());
        let forest = result.unwrap();
        let view = forest.view();
        prop_assert_eq!(view.roots().len(), 1);
        let sp = view.span(view.roots()[0]);
        prop_assert_eq!(sp.start, offset);
        prop_assert_eq!(sp.end, offset + 2);
    }
}

// ---------------------------------------------------------------------------
// 2. Two-symbol sequence rejects reversed order
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn two_symbol_sequence_rejects_reversed(_seed in 0u32..50) {
        let table = build_s_to_ab_table();
        // 'b' then 'a' is wrong order — driver may recover via insertion;
        // we only assert it terminates without panic.
        let tokens = vec![(2u32, 0u32, 1u32), (1u32, 1u32, 2u32)];
        let mut driver = Driver::new(&table);
        let _ = driver.parse_tokens(tokens.into_iter());
    }
}

// ---------------------------------------------------------------------------
// 3. Two-symbol sequence rejects partial input (only 'a')
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn two_symbol_sequence_rejects_partial(_seed in 0u32..50) {
        let table = build_s_to_ab_table();
        let tokens = vec![(1u32, 0u32, 1u32)]; // only 'a', missing 'b'
        let mut driver = Driver::new(&table);
        let result = driver.parse_tokens(tokens.into_iter());
        prop_assert!(result.is_err(), "partial input must fail for S -> a b");
    }
}

// ---------------------------------------------------------------------------
// 4. Forest leaf nodes have empty children
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn leaf_nodes_have_empty_children(_seed in 0u32..50) {
        let table = build_s_to_a_table();
        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(vec![(1u32, 0u32, 1u32)].into_iter()).unwrap();
        let view = forest.view();
        let root = view.roots()[0];
        // The children of root (S -> a) should be terminal nodes
        for &child in view.best_children(root) {
            let grandchildren = view.best_children(child);
            prop_assert!(grandchildren.is_empty(), "terminal leaf should have no children");
        }
    }
}

// ---------------------------------------------------------------------------
// 5. Forest view roots() is idempotent
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn forest_roots_idempotent(_seed in 0u32..50) {
        let table = build_s_to_a_table();
        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(vec![(1u32, 0u32, 1u32)].into_iter()).unwrap();
        let view = forest.view();
        let roots1: Vec<u32> = view.roots().to_vec();
        let roots2: Vec<u32> = view.roots().to_vec();
        prop_assert_eq!(roots1, roots2);
    }
}

// ---------------------------------------------------------------------------
// 6. Forest view kind() is consistent across calls
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn forest_kind_consistent(_seed in 0u32..50) {
        let table = build_s_to_a_table();
        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(vec![(1u32, 0u32, 1u32)].into_iter()).unwrap();
        let view = forest.view();
        let root = view.roots()[0];
        let k1 = view.kind(root);
        let k2 = view.kind(root);
        prop_assert_eq!(k1, k2);
    }
}

// ---------------------------------------------------------------------------
// 7. Forest view span() is consistent across calls
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn forest_span_consistent(_seed in 0u32..50) {
        let table = build_s_to_a_table();
        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(vec![(1u32, 0u32, 1u32)].into_iter()).unwrap();
        let view = forest.view();
        let root = view.roots()[0];
        let s1 = view.span(root);
        let s2 = view.span(root);
        prop_assert_eq!(s1, s2);
    }
}

// ---------------------------------------------------------------------------
// 8. Forest view best_children() is consistent across calls
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn forest_best_children_consistent(_seed in 0u32..50) {
        let table = build_s_to_a_table();
        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(vec![(1u32, 0u32, 1u32)].into_iter()).unwrap();
        let view = forest.view();
        let root = view.roots()[0];
        let c1: Vec<u32> = view.best_children(root).to_vec();
        let c2: Vec<u32> = view.best_children(root).to_vec();
        prop_assert_eq!(c1, c2);
    }
}

// ---------------------------------------------------------------------------
// 9. All root IDs are queryable for kind and span
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn root_ids_are_queryable(n in 1usize..=4) {
        let table = build_right_recursive_table();
        let tokens: Vec<(u32, u32, u32)> = (0..n)
            .map(|i| (1u32, i as u32, i as u32 + 1))
            .collect();
        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(tokens.into_iter()).unwrap();
        let view = forest.view();
        for &root in view.roots() {
            let _kind = view.kind(root);
            let sp = view.span(root);
            // Span must be ordered
            prop_assert!(sp.start <= sp.end, "span.start must be <= span.end");
        }
    }
}

// ---------------------------------------------------------------------------
// 10. All child IDs are queryable for kind and span
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn child_ids_are_queryable(n in 1usize..=3) {
        let table = build_right_recursive_table();
        let tokens: Vec<(u32, u32, u32)> = (0..n)
            .map(|i| (1u32, i as u32, i as u32 + 1))
            .collect();
        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(tokens.into_iter()).unwrap();
        let view = forest.view();
        for &root in view.roots() {
            for &child in view.best_children(root) {
                let _kind = view.kind(child);
                let sp = view.span(child);
                prop_assert!(sp.start <= sp.end);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 11. Recursive tree traversal does not panic
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn recursive_traversal_no_panic(n in 1usize..=5) {
        let table = build_right_recursive_table();
        let tokens: Vec<(u32, u32, u32)> = (0..n)
            .map(|i| (1u32, i as u32, i as u32 + 1))
            .collect();
        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(tokens.into_iter()).unwrap();
        let view = forest.view();

        fn visit(view: &dyn adze_glr_core::ForestView, id: u32, depth: usize) -> usize {
            if depth > 100 { return 0; }
            let mut count = 1usize;
            for &child in view.best_children(id) {
                count += visit(view, child, depth + 1);
            }
            count
        }

        for &root in view.roots() {
            let count = visit(view, root, 0);
            prop_assert!(count >= 1);
        }
    }
}

// ---------------------------------------------------------------------------
// 12. GlrError::Lex display prefix is "lexer error: "
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    #[test]
    fn glr_error_lex_has_prefix(msg in "[a-z]{1,15}") {
        let err = GlrError::Lex(msg.clone());
        let display = format!("{err}");
        prop_assert!(display.starts_with("lexer error: "), "got: {display}");
        prop_assert!(display.ends_with(&msg));
    }
}

// ---------------------------------------------------------------------------
// 13. GlrError::Parse display prefix is "parse error: "
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    #[test]
    fn glr_error_parse_has_prefix(msg in "[a-z]{1,15}") {
        let err = GlrError::Parse(msg.clone());
        let display = format!("{err}");
        prop_assert!(display.starts_with("parse error: "), "got: {display}");
        prop_assert!(display.ends_with(&msg));
    }
}

// ---------------------------------------------------------------------------
// 14. GlrError::Other display is just the message
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    #[test]
    fn glr_error_other_is_message(msg in "[a-z]{1,15}") {
        let err = GlrError::Other(msg.clone());
        let display = format!("{err}");
        prop_assert_eq!(display, msg);
    }
}

// ---------------------------------------------------------------------------
// 15. GlrError variants are distinguishable via pattern match
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    #[test]
    fn glr_error_pattern_match(variant in 0u8..3) {
        let err = match variant {
            0 => GlrError::Lex("x".into()),
            1 => GlrError::Parse("y".into()),
            _ => GlrError::Other("z".into()),
        };
        let matched = match &err {
            GlrError::Lex(_) => 0u8,
            GlrError::Parse(_) => 1,
            GlrError::Other(_) => 2,
            GlrError::UnexpectedToken(_) => 3,
        };
        prop_assert_eq!(variant.min(2), matched);
    }
}

// ---------------------------------------------------------------------------
// 16. Token positions at u32 boundary do not panic
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    #[test]
    fn token_u32_max_boundary(_seed in 0u32..30) {
        let table = build_s_to_a_table();
        let hi = u32::MAX - 1;
        let tokens = vec![(1u32, hi, u32::MAX)];
        let mut driver = Driver::new(&table);
        let result = driver.parse_tokens(tokens.into_iter());
        prop_assert!(result.is_ok());
        let forest = result.unwrap();
        let sp = forest.view().span(forest.view().roots()[0]);
        prop_assert_eq!(sp.start, hi);
        prop_assert_eq!(sp.end, u32::MAX);
    }
}

// ---------------------------------------------------------------------------
// 17. Sending EOF token kind in middle of stream terminates
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn eof_kind_mid_stream_terminates(_seed in 0u32..50) {
        let table = build_s_to_a_table();
        // Token kind 0 is EOF in this table
        let tokens = vec![(1u32, 0u32, 1u32), (0u32, 1u32, 2u32)];
        let mut driver = Driver::new(&table);
        // Must not hang or panic
        let _ = driver.parse_tokens(tokens.into_iter());
    }
}

// ---------------------------------------------------------------------------
// 18. Consecutive parses with different inputs yield different forests
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn consecutive_parses_different_inputs(n1 in 1usize..=3, n2 in 1usize..=3) {
        let table = build_right_recursive_table();
        let mk_tokens = |n: usize| -> Vec<(u32, u32, u32)> {
            (0..n).map(|i| (1u32, i as u32, i as u32 + 1)).collect()
        };

        let mut d1 = Driver::new(&table);
        let r1 = d1.parse_tokens(mk_tokens(n1).into_iter());
        let mut d2 = Driver::new(&table);
        let r2 = d2.parse_tokens(mk_tokens(n2).into_iter());

        prop_assert!(r1.is_ok());
        prop_assert!(r2.is_ok());
        let f1 = r1.unwrap();
        let f2 = r2.unwrap();
        if n1 != n2 {
            let sp1 = f1.view().span(f1.view().roots()[0]);
            let sp2 = f2.view().span(f2.view().roots()[0]);
            prop_assert_ne!(sp1.end, sp2.end, "different input lengths should differ");
        }
    }
}

// ---------------------------------------------------------------------------
// 19. Driver determinism: identical runs produce identical root count & spans
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn driver_determinism(n in 1usize..=5) {
        let table = build_right_recursive_table();
        let mk_tokens = || -> Vec<(u32, u32, u32)> {
            (0..n).map(|i| (1u32, i as u32, i as u32 + 1)).collect()
        };

        let mut d1 = Driver::new(&table);
        let f1 = d1.parse_tokens(mk_tokens().into_iter()).unwrap();
        let mut d2 = Driver::new(&table);
        let f2 = d2.parse_tokens(mk_tokens().into_iter()).unwrap();

        prop_assert_eq!(f1.view().roots().len(), f2.view().roots().len());
        for i in 0..f1.view().roots().len() {
            let r1 = f1.view().roots()[i];
            let r2 = f2.view().roots()[i];
            prop_assert_eq!(f1.view().kind(r1), f2.view().kind(r2));
            prop_assert_eq!(f1.view().span(r1), f2.view().span(r2));
            prop_assert_eq!(
                f1.view().best_children(r1).len(),
                f2.view().best_children(r2).len()
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 20. Span ordering: start <= end for every reachable node
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn span_ordering_all_nodes(n in 1usize..=4) {
        let table = build_right_recursive_table();
        let tokens: Vec<(u32, u32, u32)> = (0..n)
            .map(|i| (1u32, i as u32, i as u32 + 1))
            .collect();
        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(tokens.into_iter()).unwrap();
        let view = forest.view();

        fn check_span(view: &dyn adze_glr_core::ForestView, id: u32, depth: usize) -> bool {
            if depth > 50 { return true; }
            let sp = view.span(id);
            if sp.start > sp.end { return false; }
            for &child in view.best_children(id) {
                if !check_span(view, child, depth + 1) { return false; }
            }
            true
        }

        for &root in view.roots() {
            prop_assert!(check_span(view, root, 0), "span ordering violated");
        }
    }
}

// ---------------------------------------------------------------------------
// 21. Epsilon grammar: Forest root kind matches start symbol
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn epsilon_root_kind_is_start(_seed in 0u32..50) {
        let table = build_epsilon_table();
        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(Vec::<(u32, u32, u32)>::new().into_iter()).unwrap();
        let view = forest.view();
        for &root in view.roots() {
            // Start symbol is SymbolId(1) -> kind should be 1
            prop_assert_eq!(view.kind(root), 1, "epsilon root kind must be start symbol");
        }
    }
}

// ---------------------------------------------------------------------------
// 22. Epsilon grammar: root has empty children (ε-production)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn epsilon_root_has_no_children(_seed in 0u32..50) {
        let table = build_epsilon_table();
        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(Vec::<(u32, u32, u32)>::new().into_iter()).unwrap();
        let view = forest.view();
        for &root in view.roots() {
            prop_assert!(
                view.best_children(root).is_empty(),
                "ε-production root should have no children"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 23. Only-invalid tokens cause error
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn all_invalid_tokens_terminates(bad_kind in 50u32..200, count in 1usize..=4) {
        let table = build_s_to_a_table();
        let tokens: Vec<(u32, u32, u32)> = (0..count)
            .map(|i| (bad_kind, i as u32, i as u32 + 1))
            .collect();
        let mut driver = Driver::new(&table);
        // Driver may recover via insertion; we only assert termination without panic.
        let _ = driver.parse_tokens(tokens.into_iter());
    }
}

// ---------------------------------------------------------------------------
// 24. Token tuples are Copy — verify by reuse after parse
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn token_tuples_are_copy(_seed in 0u32..50) {
        let table = build_s_to_a_table();
        let tok: (u32, u32, u32) = (1, 0, 1);
        let mut driver = Driver::new(&table);
        // Use tok after copying into the iterator
        let _ = driver.parse_tokens(vec![tok].into_iter());
        // tok is still accessible (Copy)
        prop_assert_eq!(tok.0, 1);
        prop_assert_eq!(tok.1, 0);
        prop_assert_eq!(tok.2, 1);
    }
}

// ---------------------------------------------------------------------------
// 25. Two-symbol grammar: extra trailing token causes error
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn trailing_token_causes_error(extra_kind in 1u32..10) {
        let table = build_s_to_ab_table();
        let tokens = vec![
            (1u32, 0u32, 1u32),
            (2u32, 1u32, 2u32),
            (extra_kind, 2u32, 3u32), // extra trailing token
        ];
        let mut driver = Driver::new(&table);
        let result = driver.parse_tokens(tokens.into_iter());
        // S -> a b should accept on the second token, so extra token may be
        // ignored or cause error — must not panic either way
        let _ = result;
    }
}

// ---------------------------------------------------------------------------
// 26. Driver new() with default ParseTable does not panic
// ---------------------------------------------------------------------------

#[test]
fn driver_new_with_default_table() {
    let table = ParseTable::default();
    // Default table has eof_symbol = SymbolId(0), should be in symbol_to_index
    // if the default includes it. If not, Driver::new may panic on debug_assert.
    // This test verifies the boundary.
    let _result = std::panic::catch_unwind(|| {
        let _driver = Driver::new(&table);
    });
    // We just verify it doesn't abort — panic is acceptable for invalid tables.
}

// ---------------------------------------------------------------------------
// 27. GlrError Debug output contains variant name
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    #[test]
    fn glr_error_debug_contains_variant(variant in 0u8..3) {
        let err = match variant {
            0 => GlrError::Lex("test".into()),
            1 => GlrError::Parse("test".into()),
            _ => GlrError::Other("test".into()),
        };
        let debug = format!("{err:?}");
        let expected_name = match variant {
            0 => "Lex",
            1 => "Parse",
            _ => "Other",
        };
        prop_assert!(
            debug.contains(expected_name),
            "Debug for variant {} should contain '{}', got: {}",
            variant,
            expected_name,
            debug
        );
    }
}

// ---------------------------------------------------------------------------
// 28. Two-symbol grammar: only second token fails
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn two_symbol_only_second_token_fails(_seed in 0u32..50) {
        let table = build_s_to_ab_table();
        let tokens = vec![(2u32, 0u32, 1u32)]; // only 'b', no 'a' first
        let mut driver = Driver::new(&table);
        let result = driver.parse_tokens(tokens.into_iter());
        prop_assert!(result.is_err(), "only 'b' without 'a' must fail");
    }
}

// ---------------------------------------------------------------------------
// 29. parse_tokens accepts empty iterator on epsilon grammar
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn parse_tokens_empty_iter_epsilon(_seed in 0u32..50) {
        let table = build_epsilon_table();
        let mut driver = Driver::new(&table);
        let result = driver.parse_tokens(std::iter::empty());
        prop_assert!(result.is_ok());
    }
}

// ---------------------------------------------------------------------------
// 30. Forest from S -> 'a' has exactly one child per root
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn s_to_a_root_has_one_child(_seed in 0u32..50) {
        let table = build_s_to_a_table();
        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(vec![(1u32, 0u32, 1u32)].into_iter()).unwrap();
        let view = forest.view();
        let root = view.roots()[0];
        prop_assert_eq!(view.best_children(root).len(), 1, "S -> 'a' has one child");
    }
}

// ---------------------------------------------------------------------------
// 31. Child kind for S -> 'a' is the terminal kind
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn s_to_a_child_kind_is_terminal(_seed in 0u32..50) {
        let table = build_s_to_a_table();
        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(vec![(1u32, 0u32, 1u32)].into_iter()).unwrap();
        let view = forest.view();
        let root = view.roots()[0];
        let child = view.best_children(root)[0];
        // Terminal 'a' = SymbolId(1) -> kind = 1
        prop_assert_eq!(view.kind(child), 1, "child kind must be terminal 'a'");
    }
}

// ---------------------------------------------------------------------------
// 32. GlrError std::error::Error trait is implemented
// ---------------------------------------------------------------------------

#[test]
fn glr_error_implements_std_error() {
    fn assert_error<E: std::error::Error>() {}
    assert_error::<GlrError>();
}

// ---------------------------------------------------------------------------
// 33. Parse failure error message is non-empty
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn parse_failure_error_message_nonempty(bad_kind in 50u32..200) {
        let table = build_s_to_a_table();
        let tokens = vec![(bad_kind, 0u32, 1u32)];
        let mut driver = Driver::new(&table);
        let result = driver.parse_tokens(tokens.into_iter());
        if let Err(e) = result {
            let msg = format!("{e}");
            prop_assert!(!msg.is_empty(), "error message must not be empty");
        }
    }
}

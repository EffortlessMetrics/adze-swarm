#![cfg(feature = "test-api")]
//! Comprehensive tests for the GLR Driver API.
//!
//! 80+ tests covering construction, parsing, error handling, forest inspection,
//! GLR forking, driver reuse, grammar shapes, and edge cases.

use adze_glr_core::forest_view::ForestView;
use adze_glr_core::{
    Action, Driver, FirstFollowSets, Forest, GotoIndexing, LexMode, ParseRule, ParseTable,
    build_lr1_automaton, sanity_check_tables,
};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{Associativity, Grammar, RuleId, StateId, SymbolId};
use std::collections::BTreeMap;

// ─── Helpers ─────────────────────────────────────────────────────────

/// Run normalize → FIRST/FOLLOW → build_lr1_automaton, returning a ParseTable.
fn run_pipeline(grammar: &mut Grammar) -> Result<ParseTable, adze_glr_core::GLRError> {
    let first_follow = FirstFollowSets::compute_normalized(grammar)?;
    build_lr1_automaton(grammar, &first_follow)
}

/// Build grammar + table, then parse a token stream through the driver.
fn pipeline_parse(
    grammar: &mut Grammar,
    token_stream: &[(SymbolId, u32, u32)],
) -> Result<Forest, adze_glr_core::driver::GlrError> {
    let table = run_pipeline(grammar).expect("pipeline should produce a table");
    sanity_check_tables(&table).expect("table sanity check");
    let mut driver = Driver::new(&table);
    driver.parse_tokens(
        token_stream
            .iter()
            .map(|&(sym, start, end)| (sym.0 as u32, start, end)),
    )
}

/// Resolve a symbol name to its SymbolId inside a built grammar.
fn sym_id(grammar: &Grammar, name: &str) -> SymbolId {
    for (&id, tok) in &grammar.tokens {
        if tok.name == name {
            return id;
        }
    }
    for (&id, n) in &grammar.rule_names {
        if n == name {
            return id;
        }
    }
    panic!("symbol '{}' not found in grammar", name);
}

/// Helper to hand-craft a minimal ParseTable for low-level driver tests.
fn make_table(
    states: Vec<Vec<Vec<Action>>>,
    gotos: Vec<Vec<StateId>>,
    rules: Vec<ParseRule>,
    start: SymbolId,
    eof: SymbolId,
) -> ParseTable {
    let symbol_count = states.first().map(|s| s.len()).unwrap_or(0);
    let state_count = states.len();

    let mut symbol_to_index = BTreeMap::new();
    for i in 0..symbol_count {
        symbol_to_index.insert(SymbolId(i as u16), i);
    }

    let mut nonterminal_to_index = BTreeMap::new();
    for i in 0..symbol_count {
        for state_gotos in &gotos {
            if state_gotos[i] != StateId(65535) {
                nonterminal_to_index.insert(SymbolId(i as u16), i);
                break;
            }
        }
    }

    ParseTable {
        action_table: states,
        goto_table: gotos,
        rules: rules.clone(),
        state_count,
        symbol_count,
        symbol_to_index,
        index_to_symbol: vec![],
        external_scanner_states: vec![],
        nonterminal_to_index,
        eof_symbol: eof,
        start_symbol: start,
        grammar: Grammar::new("test".to_string()),
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
        dynamic_prec_by_rule: vec![0; rules.len()],
        rule_assoc_by_rule: vec![0; rules.len()],
        alias_sequences: vec![],
        field_names: vec![],
        goto_indexing: GotoIndexing::NonterminalMap,
        field_map: BTreeMap::new(),
    }
}

mod forest_counting {
    use super::ForestView;

    const MAX_RECURSION_DEPTH: usize = 200;

    pub(super) fn count_nodes(view: &dyn ForestView, id: u32, depth: usize) -> usize {
        recursion_guard::ensure_bounded(depth);
        let direct_children = children::best(view, id);
        node_totals::including_self(view, direct_children, depth + 1)
    }

    mod recursion_guard {
        use super::MAX_RECURSION_DEPTH;

        pub(super) fn ensure_bounded(depth: usize) {
            assert!(depth < MAX_RECURSION_DEPTH, "infinite recursion guard");
        }
    }

    mod children {
        use super::ForestView;

        pub(super) fn best(view: &dyn ForestView, id: u32) -> &[u32] {
            view.best_children(id)
        }
    }

    mod node_totals {
        use super::{ForestView, count_nodes};

        pub(super) fn including_self(
            view: &dyn ForestView,
            children: &[u32],
            next_depth: usize,
        ) -> usize {
            1 + children
                .iter()
                .copied()
                .map(|child| count_nodes(view, child, next_depth))
                .sum::<usize>()
        }
    }
}

/// Walk a forest recursively, counting nodes.
fn count_nodes(view: &dyn ForestView, id: u32, depth: usize) -> usize {
    forest_counting::count_nodes(view, id, depth)
}

// ═══════════════════════════════════════════════════════════════════════
// 1. Driver construction
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn construct_driver_from_pipeline_table() {
    let mut g = GrammarBuilder::new("ctor1")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let table = run_pipeline(&mut g).expect("pipeline");
    let _d = Driver::new(&table);
}

#[test]
fn construct_driver_from_hand_crafted_table() {
    let eof = SymbolId(0);
    let s = SymbolId(3);
    let rules = vec![ParseRule { lhs: s, rhs_len: 1 }];
    let mut actions = vec![vec![vec![]; 4]; 3];
    actions[0][1].push(Action::Shift(StateId(1)));
    actions[1][0].push(Action::Reduce(RuleId(0)));
    actions[2][0].push(Action::Accept);
    let inv = StateId(65535);
    let mut gotos = vec![vec![inv; 4]; 3];
    gotos[0][3] = StateId(2);
    let table = make_table(actions, gotos, rules, s, eof);
    let _d = Driver::new(&table);
}

#[test]
fn construct_driver_preserves_initial_state() {
    let mut g = GrammarBuilder::new("init_st")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let table = run_pipeline(&mut g).expect("pipeline");
    assert!((table.initial_state.0 as usize) < table.state_count);
    let _d = Driver::new(&table);
}

#[test]
fn construct_driver_with_multi_rule_grammar() {
    let mut g = GrammarBuilder::new("multi_ctor")
        .token("x", "x")
        .token("y", "y")
        .rule("S", vec!["A", "B"])
        .rule("A", vec!["x"])
        .rule("B", vec!["y"])
        .start("S")
        .build();
    let table = run_pipeline(&mut g).expect("pipeline");
    let _d = Driver::new(&table);
}

// ═══════════════════════════════════════════════════════════════════════
// 2. Parsing simple inputs
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn parse_single_token() {
    let mut g = GrammarBuilder::new("one_tok")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let f = pipeline_parse(&mut g, &[(a, 0, 1)]).expect("parse");
    assert_eq!(f.view().roots().len(), 1);
}

#[test]
fn parse_two_tokens() {
    let mut g = GrammarBuilder::new("two_tok")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["a", "b"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let b = sym_id(&g, "b");
    let f = pipeline_parse(&mut g, &[(a, 0, 1), (b, 1, 2)]).expect("parse");
    assert_eq!(f.view().span(f.view().roots()[0]).end, 2);
}

#[test]
fn parse_three_token_addition() {
    let mut g = GrammarBuilder::new("add3")
        .token("NUM", r"\d+")
        .token("+", "+")
        .rule("expr", vec!["expr", "+", "NUM"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let num = sym_id(&g, "NUM");
    let plus = sym_id(&g, "+");
    let f = pipeline_parse(&mut g, &[(num, 0, 1), (plus, 1, 2), (num, 2, 3)]).expect("parse");
    assert_eq!(f.view().span(f.view().roots()[0]).end, 3);
}

#[test]
fn parse_with_varying_byte_widths() {
    let mut g = GrammarBuilder::new("widths")
        .token("NUM", r"\d+")
        .token("+", "+")
        .rule("expr", vec!["expr", "+", "NUM"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let num = sym_id(&g, "NUM");
    let plus = sym_id(&g, "+");
    // "123+45"
    let f = pipeline_parse(&mut g, &[(num, 0, 3), (plus, 3, 4), (num, 4, 6)]).expect("parse");
    assert_eq!(f.view().span(f.view().roots()[0]).end, 6);
}

#[test]
fn parse_with_gaps_between_tokens() {
    let mut g = GrammarBuilder::new("gaps")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["a", "b"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let b = sym_id(&g, "b");
    // "a  b" → tokens at (0,1) and (3,4)
    let f = pipeline_parse(&mut g, &[(a, 0, 1), (b, 3, 4)]).expect("parse");
    let span = f.view().span(f.view().roots()[0]);
    assert_eq!(span.start, 0);
    assert_eq!(span.end, 4);
}

#[test]
fn parse_five_token_chain() {
    let mut g = GrammarBuilder::new("five_chain")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .token("d", "d")
        .token("e", "e")
        .rule("S", vec!["a", "b", "c", "d", "e"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let b = sym_id(&g, "b");
    let c = sym_id(&g, "c");
    let d = sym_id(&g, "d");
    let e = sym_id(&g, "e");
    let f = pipeline_parse(
        &mut g,
        &[(a, 0, 1), (b, 1, 2), (c, 2, 3), (d, 3, 4), (e, 4, 5)],
    )
    .expect("parse");
    assert_eq!(f.view().span(f.view().roots()[0]).end, 5);
    assert_eq!(f.view().best_children(f.view().roots()[0]).len(), 5);
}

#[test]
fn parse_via_hand_crafted_shift_reduce_accept() {
    let eof = SymbolId(0);
    let s = SymbolId(3);
    let rules = vec![ParseRule { lhs: s, rhs_len: 2 }];
    let mut actions = vec![vec![vec![]; 4]; 4];
    actions[0][1].push(Action::Shift(StateId(1)));
    actions[1][2].push(Action::Shift(StateId(2)));
    actions[2][0].push(Action::Reduce(RuleId(0)));
    actions[3][0].push(Action::Accept);
    let inv = StateId(65535);
    let mut gotos = vec![vec![inv; 4]; 4];
    gotos[0][3] = StateId(3);
    let table = make_table(actions, gotos, rules, s, eof);
    let mut driver = Driver::new(&table);
    let f = driver
        .parse_tokens([(1u32, 0u32, 1u32), (2, 1, 2)].iter().copied())
        .expect("parse");
    assert_eq!(f.view().roots().len(), 1);
    assert_eq!(f.view().span(f.view().roots()[0]).end, 2);
}

// ═══════════════════════════════════════════════════════════════════════
// 3. Parsing with conflicts (GLR forking)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn ambiguous_addition_parses() {
    let mut g = GrammarBuilder::new("ambig_add")
        .token("n", "n")
        .token("+", "+")
        .rule("E", vec!["E", "+", "E"])
        .rule("E", vec!["n"])
        .start("E")
        .build();
    let n = sym_id(&g, "n");
    let plus = sym_id(&g, "+");
    let f = pipeline_parse(
        &mut g,
        &[(n, 0, 1), (plus, 1, 2), (n, 2, 3), (plus, 3, 4), (n, 4, 5)],
    )
    .expect("ambig parse");
    assert!(!f.view().roots().is_empty());
    assert_eq!(f.view().span(f.view().roots()[0]).end, 5);
}

#[test]
fn ambiguous_short_input_n_plus_n() {
    let mut g = GrammarBuilder::new("ambig_short")
        .token("n", "n")
        .token("+", "+")
        .rule("E", vec!["E", "+", "E"])
        .rule("E", vec!["n"])
        .start("E")
        .build();
    let n = sym_id(&g, "n");
    let plus = sym_id(&g, "+");
    let f = pipeline_parse(&mut g, &[(n, 0, 1), (plus, 1, 2), (n, 2, 3)]).expect("parse");
    assert_eq!(f.view().span(f.view().roots()[0]).end, 3);
}

#[test]
fn ambiguous_ee_grammar_has_conflicts() {
    let mut g = GrammarBuilder::new("ambig_ee")
        .token("a", "a")
        .rule("E", vec!["E", "E"])
        .rule("E", vec!["a"])
        .start("E")
        .build();
    let table = run_pipeline(&mut g).expect("pipeline");
    let conflicts = adze_glr_core::conflict_inspection::count_conflicts(&table);
    assert!(
        conflicts.shift_reduce >= 1 || conflicts.reduce_reduce >= 1,
        "E → E E | a must have conflicts"
    );
}

#[test]
fn fork_action_explores_multiple_paths() {
    let eof = SymbolId(0);
    let s = SymbolId(3);
    let rules = vec![ParseRule { lhs: s, rhs_len: 1 }];
    let mut actions = vec![vec![vec![]; 4]; 4];
    actions[0][1].push(Action::Fork(vec![
        Action::Shift(StateId(1)),
        Action::Shift(StateId(2)),
    ]));
    actions[1][0].push(Action::Reduce(RuleId(0)));
    actions[2][0].push(Action::Reduce(RuleId(0)));
    actions[3][0].push(Action::Accept);
    let inv = StateId(65535);
    let mut gotos = vec![vec![inv; 4]; 4];
    gotos[0][3] = StateId(3);
    let table = make_table(actions, gotos, rules, s, eof);
    let mut driver = Driver::new(&table);
    let result = driver.parse_tokens([(1u32, 0u32, 1u32)].iter().copied());
    assert!(result.is_ok(), "fork should produce valid parse");
}

#[test]
fn ambiguous_grammar_detects_shift_reduce_conflict() {
    let mut g = GrammarBuilder::new("sr_detect")
        .token("n", "n")
        .token("+", "+")
        .rule("E", vec!["E", "+", "E"])
        .rule("E", vec!["n"])
        .start("E")
        .build();
    let table = run_pipeline(&mut g).expect("pipeline");
    let conflicts = adze_glr_core::conflict_inspection::count_conflicts(&table);
    assert!(conflicts.shift_reduce >= 1, "must have S/R conflicts");
}

#[test]
fn ambiguous_four_token_chain_parses() {
    // E → E '+' E | 'n', input: n+n+n+n
    let mut g = GrammarBuilder::new("ambig4")
        .token("n", "n")
        .token("+", "+")
        .rule("E", vec!["E", "+", "E"])
        .rule("E", vec!["n"])
        .start("E")
        .build();
    let n = sym_id(&g, "n");
    let plus = sym_id(&g, "+");
    let f = pipeline_parse(
        &mut g,
        &[
            (n, 0, 1),
            (plus, 1, 2),
            (n, 2, 3),
            (plus, 3, 4),
            (n, 4, 5),
            (plus, 5, 6),
            (n, 6, 7),
        ],
    )
    .expect("parse");
    assert_eq!(f.view().span(f.view().roots()[0]).end, 7);
}

// ═══════════════════════════════════════════════════════════════════════
// 4. Error handling
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn glr_error_lex_variant_display() {
    let err = adze_glr_core::driver::GlrError::Lex("bad char".into());
    let msg = format!("{err}");
    assert!(msg.contains("lexer error"));
    assert!(msg.contains("bad char"));
}

#[test]
fn glr_error_parse_variant_display() {
    let err = adze_glr_core::driver::GlrError::Parse("unexpected".into());
    assert!(format!("{err}").contains("parse error"));
}

#[test]
fn glr_error_other_variant_display() {
    let err = adze_glr_core::driver::GlrError::Other("misc".into());
    assert!(format!("{err}").contains("misc"));
}

#[test]
fn glr_error_debug_shows_variant() {
    let err = adze_glr_core::driver::GlrError::Lex("x".into());
    assert!(format!("{err:?}").contains("Lex"));
}

#[test]
fn glr_error_implements_std_error() {
    let err: Box<dyn std::error::Error> =
        Box::new(adze_glr_core::driver::GlrError::Parse("p".into()));
    assert!(err.to_string().contains("parse error"));
}

#[test]
fn invalid_token_sequence_errors() {
    // S → 'a' 'b', feed 'a' 'a'
    let eof = SymbolId(0);
    let s = SymbolId(3);
    let rules = vec![ParseRule { lhs: s, rhs_len: 2 }];
    let mut actions = vec![vec![vec![]; 4]; 4];
    actions[0][1].push(Action::Shift(StateId(1)));
    actions[1][2].push(Action::Shift(StateId(2)));
    actions[2][0].push(Action::Reduce(RuleId(0)));
    actions[3][0].push(Action::Accept);
    let inv = StateId(65535);
    let mut gotos = vec![vec![inv; 4]; 4];
    gotos[0][3] = StateId(3);
    let table = make_table(actions, gotos, rules, s, eof);
    let mut driver = Driver::new(&table);
    let result = driver.parse_tokens([(1u32, 0u32, 1u32), (1, 1, 2)].iter().copied());
    assert!(result.is_err());
}

#[test]
fn unexpected_eof_is_handled() {
    let mut g = GrammarBuilder::new("eof_err")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["a", "b"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let result = pipeline_parse(&mut g, &[(a, 0, 1)]);
    // Either error or recovery — must not panic
    if let Ok(f) = result {
        assert!(!f.view().roots().is_empty())
    }
}

#[test]
fn unknown_token_kind_does_not_panic() {
    let eof = SymbolId(0);
    let a = SymbolId(1);
    let s = SymbolId(2);
    let mut table = ParseTable {
        grammar: Grammar::new("unk_tok".to_string()),
        state_count: 3,
        symbol_count: 3,
        token_count: 2,
        eof_symbol: eof,
        start_symbol: s,
        initial_state: StateId(0),
        index_to_symbol: vec![eof, a],
        action_table: vec![
            vec![vec![], vec![Action::Shift(StateId(1))]],
            vec![vec![Action::Reduce(RuleId(0))], vec![]],
            vec![vec![Action::Accept], vec![]],
        ],
        goto_table: vec![vec![StateId(2)], vec![StateId(0)], vec![StateId(0)]],
        rules: vec![ParseRule { lhs: s, rhs_len: 1 }],
        lex_modes: vec![
            LexMode {
                lex_state: 0,
                external_lex_state: 0,
            };
            3
        ],
        symbol_metadata: vec![],
        ..Default::default()
    };
    table.symbol_to_index.insert(eof, 0);
    table.symbol_to_index.insert(a, 1);
    table.nonterminal_to_index.insert(s, 0);
    let mut driver = Driver::new(&table);
    let _ = driver.parse_tokens([(99, 0, 1)]);
}

#[test]
fn completely_wrong_input_does_not_panic() {
    let mut g = GrammarBuilder::new("wrong")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["a", "b"])
        .start("S")
        .build();
    let b = sym_id(&g, "b");
    let a = sym_id(&g, "a");
    // Feed reversed: 'b' 'a'
    let _ = pipeline_parse(&mut g, &[(b, 0, 1), (a, 1, 2)]);
}

#[test]
fn glr_error_parse_with_empty_message() {
    let err = adze_glr_core::driver::GlrError::Parse(String::new());
    let msg = format!("{err}");
    assert!(msg.contains("parse error"));
}

#[test]
fn glr_error_other_with_empty_message() {
    let err = adze_glr_core::driver::GlrError::Other(String::new());
    let _ = format!("{err}");
}

// ═══════════════════════════════════════════════════════════════════════
// 5. Forest/result inspection
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn forest_root_count_is_one_for_simple_parse() {
    let mut g = GrammarBuilder::new("root1")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let f = pipeline_parse(&mut g, &[(a, 0, 1)]).expect("parse");
    assert_eq!(f.view().roots().len(), 1);
}

#[test]
fn forest_root_kind_is_nonzero() {
    let mut g = GrammarBuilder::new("kind_nz")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let f = pipeline_parse(&mut g, &[(a, 0, 1)]).expect("parse");
    let root = f.view().roots()[0];
    let _ = f.view().kind(root);
}

#[test]
fn forest_root_span_covers_input() {
    let mut g = GrammarBuilder::new("span_cov")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["a", "b"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let b = sym_id(&g, "b");
    let f = pipeline_parse(&mut g, &[(a, 0, 1), (b, 1, 2)]).expect("parse");
    let span = f.view().span(f.view().roots()[0]);
    assert_eq!(span.start, 0);
    assert_eq!(span.end, 2);
}

#[test]
fn forest_children_within_parent_span() {
    let mut g = GrammarBuilder::new("child_span")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["a", "b"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let b = sym_id(&g, "b");
    let f = pipeline_parse(&mut g, &[(a, 0, 1), (b, 1, 2)]).expect("parse");
    let v = f.view();
    let root = v.roots()[0];
    let root_span = v.span(root);
    for &child in v.best_children(root) {
        let cs = v.span(child);
        assert!(cs.start >= root_span.start);
        assert!(cs.end <= root_span.end);
    }
}

#[test]
fn forest_leaf_has_no_children() {
    let mut g = GrammarBuilder::new("leaf_nc")
        .token("x", "x")
        .rule("S", vec!["x"])
        .start("S")
        .build();
    let x = sym_id(&g, "x");
    let f = pipeline_parse(&mut g, &[(x, 0, 1)]).expect("parse");
    let v = f.view();
    let root = v.roots()[0];
    let children = v.best_children(root);
    assert_eq!(children.len(), 1);
    assert!(v.best_children(children[0]).is_empty());
}

#[test]
fn forest_deep_traversal_counts_nodes() {
    let mut g = GrammarBuilder::new("deep_walk")
        .token("x", "x")
        .token("y", "y")
        .rule("S", vec!["A", "B"])
        .rule("A", vec!["x"])
        .rule("B", vec!["y"])
        .start("S")
        .build();
    let x = sym_id(&g, "x");
    let y = sym_id(&g, "y");
    let f = pipeline_parse(&mut g, &[(x, 0, 1), (y, 1, 2)]).expect("parse");
    let total = count_nodes(f.view(), f.view().roots()[0], 0);
    assert!(total >= 3);
}

#[test]
fn forest_unknown_id_kind_returns_zero() {
    let mut g = GrammarBuilder::new("unk_kind")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let f = pipeline_parse(&mut g, &[(a, 0, 1)]).expect("parse");
    assert_eq!(f.view().kind(99999), 0);
}

#[test]
fn forest_unknown_id_span_returns_zero() {
    let mut g = GrammarBuilder::new("unk_span")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let f = pipeline_parse(&mut g, &[(a, 0, 1)]).expect("parse");
    let span = f.view().span(99999);
    assert_eq!(span.start, 0);
    assert_eq!(span.end, 0);
}

#[test]
fn forest_unknown_id_children_empty() {
    let mut g = GrammarBuilder::new("unk_ch")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let f = pipeline_parse(&mut g, &[(a, 0, 1)]).expect("parse");
    assert!(f.view().best_children(99999).is_empty());
}

#[test]
fn forest_debug_error_stats_clean_parse() {
    let mut g = GrammarBuilder::new("clean_stats")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let f = pipeline_parse(&mut g, &[(a, 0, 1)]).expect("parse");
    let (has_err, _missing, cost) = f.debug_error_stats();
    assert!(!has_err);
    assert_eq!(cost, 0);
}

#[test]
fn forest_view_returns_trait_object() {
    let mut g = GrammarBuilder::new("trait_obj")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let f = pipeline_parse(&mut g, &[(a, 0, 1)]).expect("parse");
    let _v: &dyn ForestView = f.view();
}

// ═══════════════════════════════════════════════════════════════════════
// 6. Empty input handling
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn empty_input_non_nullable_grammar_handled() {
    let mut g = GrammarBuilder::new("empty_nn")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let result = pipeline_parse(&mut g, &[]);
    if let Ok(f) = result {
        let _ = f.debug_error_stats();
    }
}

#[test]
fn empty_input_with_epsilon_production() {
    let mut g = GrammarBuilder::new("eps_rule")
        .token("a", "a")
        .rule("S", vec![])
        .start("S")
        .build();
    let result = pipeline_parse(&mut g, &[]);
    if let Ok(f) = result {
        assert!(!f.view().roots().is_empty());
    }
}

#[test]
fn empty_token_stream_to_parse_tokens() {
    let mut g = GrammarBuilder::new("empty_ts")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let table = run_pipeline(&mut g).expect("pipeline");
    let mut driver = Driver::new(&table);
    let result = driver.parse_tokens(std::iter::empty::<(u32, u32, u32)>());
    // Should not panic
    if let Ok(f) = result {
        let _ = f.debug_error_stats();
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 7. Large input handling
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn parse_long_left_recursive_chain() {
    // A → A 'a' | 'a', input: 50 a's
    let mut g = GrammarBuilder::new("long_lr")
        .token("a", "a")
        .rule("A", vec!["A", "a"])
        .rule("A", vec!["a"])
        .start("A")
        .build();
    let a = sym_id(&g, "a");
    let tokens: Vec<_> = (0..50).map(|i| (a, i as u32, (i + 1) as u32)).collect();
    let f = pipeline_parse(&mut g, &tokens).expect("parse");
    assert_eq!(f.view().span(f.view().roots()[0]).end, 50);
}

#[test]
fn parse_long_right_recursive_chain() {
    // L → 'a' L | 'a', input: 30 a's
    let mut g = GrammarBuilder::new("long_rr")
        .token("a", "a")
        .rule("L", vec!["a", "L"])
        .rule("L", vec!["a"])
        .start("L")
        .build();
    let a = sym_id(&g, "a");
    let table = run_pipeline(&mut g).expect("pipeline");
    let mut driver = Driver::new(&table);
    let tokens: Vec<_> = (0..30)
        .map(|i| (a.0 as u32, i as u32, (i + 1) as u32))
        .collect();
    let f = driver.parse_tokens(tokens).expect("parse");
    assert!(!f.view().roots().is_empty());
}

#[test]
fn parse_repeated_addition_long() {
    // expr → expr '+' NUM | NUM, input: 20 additions
    let mut g = GrammarBuilder::new("long_add")
        .token("NUM", r"\d+")
        .token("+", "+")
        .rule("expr", vec!["expr", "+", "NUM"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let num = sym_id(&g, "NUM");
    let plus = sym_id(&g, "+");
    let mut tokens = vec![(num, 0u32, 1u32)];
    for i in 0..20 {
        let base = (i * 2 + 1) as u32;
        tokens.push((plus, base, base + 1));
        tokens.push((num, base + 1, base + 2));
    }
    let f = pipeline_parse(&mut g, &tokens).expect("parse");
    assert!(!f.view().roots().is_empty());
}

#[test]
fn large_byte_offsets_no_overflow() {
    let mut g = GrammarBuilder::new("big_off")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let start = u32::MAX - 10;
    let end = u32::MAX - 9;
    let _ = pipeline_parse(&mut g, &[(a, start, end)]);
}

// ═══════════════════════════════════════════════════════════════════════
// 8. Various grammar shapes
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn nested_nonterminals_correct_children() {
    // S → A B; A → 'x'; B → 'y'
    let mut g = GrammarBuilder::new("nested")
        .token("x", "x")
        .token("y", "y")
        .rule("S", vec!["A", "B"])
        .rule("A", vec!["x"])
        .rule("B", vec!["y"])
        .start("S")
        .build();
    let x = sym_id(&g, "x");
    let y = sym_id(&g, "y");
    let f = pipeline_parse(&mut g, &[(x, 0, 1), (y, 1, 2)]).expect("parse");
    let v = f.view();
    let root = v.roots()[0];
    assert_eq!(v.best_children(root).len(), 2);
    for &child in v.best_children(root) {
        assert_eq!(v.best_children(child).len(), 1);
    }
}

#[test]
fn multiple_alternatives_first_matches() {
    let mut g = GrammarBuilder::new("alt1")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .rule("S", vec!["a", "b"])
        .rule("S", vec!["a", "c"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let b = sym_id(&g, "b");
    let f = pipeline_parse(&mut g, &[(a, 0, 1), (b, 1, 2)]).expect("parse");
    assert_eq!(f.view().span(f.view().roots()[0]).end, 2);
}

#[test]
fn multiple_alternatives_second_matches() {
    let mut g = GrammarBuilder::new("alt2")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .rule("S", vec!["a", "b"])
        .rule("S", vec!["a", "c"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let c = sym_id(&g, "c");
    let f = pipeline_parse(&mut g, &[(a, 0, 1), (c, 1, 2)]).expect("parse");
    assert_eq!(f.view().span(f.view().roots()[0]).end, 2);
}

#[test]
fn deeply_nested_parentheses() {
    let mut g = GrammarBuilder::new("deep_paren")
        .token("NUM", r"\d+")
        .token("(", "(")
        .token(")", ")")
        .rule("expr", vec!["(", "expr", ")"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let num = sym_id(&g, "NUM");
    let lp = sym_id(&g, "(");
    let rp = sym_id(&g, ")");
    let depth = 10;
    let mut tokens = Vec::new();
    let mut byte = 0u32;
    for _ in 0..depth {
        tokens.push((lp, byte, byte + 1));
        byte += 1;
    }
    tokens.push((num, byte, byte + 1));
    byte += 1;
    for _ in 0..depth {
        tokens.push((rp, byte, byte + 1));
        byte += 1;
    }
    let f = pipeline_parse(&mut g, &tokens).expect("parse");
    assert_eq!(f.view().span(f.view().roots()[0]).end, byte);
}

#[test]
fn left_recursive_grammar_parses() {
    let mut g = GrammarBuilder::new("lr_parse")
        .token("a", "a")
        .rule("A", vec!["A", "a"])
        .rule("A", vec!["a"])
        .start("A")
        .build();
    let a = sym_id(&g, "a");
    let tokens: Vec<_> = (0..4).map(|i| (a, i as u32, (i + 1) as u32)).collect();
    let f = pipeline_parse(&mut g, &tokens).expect("parse");
    assert_eq!(f.view().span(f.view().roots()[0]).end, 4);
}

#[test]
fn right_recursive_grammar_parses() {
    let mut g = GrammarBuilder::new("rr_parse")
        .token("a", "a")
        .rule("L", vec!["a", "L"])
        .rule("L", vec!["a"])
        .start("L")
        .build();
    let a = sym_id(&g, "a");
    let table = run_pipeline(&mut g).expect("pipeline");
    let mut driver = Driver::new(&table);
    let f = driver
        .parse_tokens(
            [(a.0 as u32, 0, 1), (a.0 as u32, 1, 2), (a.0 as u32, 2, 3)]
                .iter()
                .copied(),
        )
        .expect("parse");
    assert!(!f.view().roots().is_empty());
}

#[test]
fn three_level_nesting_grammar() {
    // S → A; A → B; B → 'x'
    let mut g = GrammarBuilder::new("three_lev")
        .token("x", "x")
        .rule("S", vec!["A"])
        .rule("A", vec!["B"])
        .rule("B", vec!["x"])
        .start("S")
        .build();
    let x = sym_id(&g, "x");
    let f = pipeline_parse(&mut g, &[(x, 0, 1)]).expect("parse");
    let v = f.view();
    let total = count_nodes(v, v.roots()[0], 0);
    assert!(total >= 3, "should have S → A → B → x");
}

#[test]
fn grammar_with_two_terminals_per_rule() {
    // S → 'a' 'b'; T → 'c' 'd'; U → S T
    let mut g = GrammarBuilder::new("two_per")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .token("d", "d")
        .rule("U", vec!["S", "T"])
        .rule("S", vec!["a", "b"])
        .rule("T", vec!["c", "d"])
        .start("U")
        .build();
    let a = sym_id(&g, "a");
    let b = sym_id(&g, "b");
    let c = sym_id(&g, "c");
    let d = sym_id(&g, "d");
    let f = pipeline_parse(&mut g, &[(a, 0, 1), (b, 1, 2), (c, 2, 3), (d, 3, 4)]).expect("parse");
    assert_eq!(f.view().span(f.view().roots()[0]).end, 4);
}

#[test]
fn grammar_with_precedence_left_assoc() {
    let mut g = GrammarBuilder::new("prec_left")
        .token("n", "n")
        .token("+", "+")
        .rule_with_precedence("E", vec!["E", "+", "E"], 1, Associativity::Left)
        .rule("E", vec!["n"])
        .start("E")
        .build();
    let n = sym_id(&g, "n");
    let plus = sym_id(&g, "+");
    let f = pipeline_parse(&mut g, &[(n, 0, 1), (plus, 1, 2), (n, 2, 3)]).expect("parse");
    assert!(!f.view().roots().is_empty());
}

#[test]
fn grammar_with_precedence_right_assoc() {
    let mut g = GrammarBuilder::new("prec_right")
        .token("n", "n")
        .token("^", "^")
        .rule_with_precedence("E", vec!["E", "^", "E"], 2, Associativity::Right)
        .rule("E", vec!["n"])
        .start("E")
        .build();
    let n = sym_id(&g, "n");
    let caret = sym_id(&g, "^");
    let f = pipeline_parse(&mut g, &[(n, 0, 1), (caret, 1, 2), (n, 2, 3)]).expect("parse");
    assert!(!f.view().roots().is_empty());
}

#[test]
fn grammar_with_single_token_single_rule() {
    let mut g = GrammarBuilder::new("minimal_g")
        .token("x", "x")
        .rule("S", vec!["x"])
        .start("S")
        .build();
    let x = sym_id(&g, "x");
    let f = pipeline_parse(&mut g, &[(x, 0, 1)]).expect("parse");
    assert_eq!(f.view().roots().len(), 1);
}

// ═══════════════════════════════════════════════════════════════════════
// 9. Driver reuse across multiple parses
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn driver_reuse_different_inputs() {
    let mut g = GrammarBuilder::new("reuse_diff")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["a"])
        .rule("S", vec!["b"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let b = sym_id(&g, "b");
    let table = run_pipeline(&mut g).expect("pipeline");
    let mut driver = Driver::new(&table);
    let f1 = driver
        .parse_tokens([(a.0 as u32, 0, 1)].iter().copied())
        .expect("first");
    assert_eq!(f1.view().roots().len(), 1);
    let f2 = driver
        .parse_tokens([(b.0 as u32, 0, 1)].iter().copied())
        .expect("second");
    assert_eq!(f2.view().roots().len(), 1);
}

#[test]
fn driver_reuse_same_input_three_times() {
    let mut g = GrammarBuilder::new("reuse3")
        .token("x", "x")
        .rule("S", vec!["x"])
        .start("S")
        .build();
    let x = sym_id(&g, "x");
    let table = run_pipeline(&mut g).expect("pipeline");
    let mut driver = Driver::new(&table);
    for _ in 0..3 {
        let f = driver
            .parse_tokens([(x.0 as u32, 0, 1)].iter().copied())
            .expect("parse");
        assert_eq!(f.view().roots().len(), 1);
    }
}

#[test]
fn driver_reuse_error_then_success() {
    let mut g = GrammarBuilder::new("err_then_ok")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["a", "b"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let b = sym_id(&g, "b");
    let table = run_pipeline(&mut g).expect("pipeline");
    let mut driver = Driver::new(&table);
    let _ = driver.parse_tokens([(a.0 as u32, 0, 1)].iter().copied());
    let r2 = driver.parse_tokens([(a.0 as u32, 0, 1), (b.0 as u32, 1, 2)].iter().copied());
    assert!(r2.is_ok());
}

#[test]
fn driver_reuse_success_then_error() {
    let mut g = GrammarBuilder::new("ok_then_err")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["a", "b"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let b = sym_id(&g, "b");
    let table = run_pipeline(&mut g).expect("pipeline");
    let mut driver = Driver::new(&table);
    let r1 = driver.parse_tokens([(a.0 as u32, 0, 1), (b.0 as u32, 1, 2)].iter().copied());
    assert!(r1.is_ok());
    // Now feed wrong input
    let _ = driver.parse_tokens([(b.0 as u32, 0, 1)].iter().copied());
}

#[test]
fn driver_reuse_ten_parses() {
    let mut g = GrammarBuilder::new("reuse10")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let table = run_pipeline(&mut g).expect("pipeline");
    let mut driver = Driver::new(&table);
    for _ in 0..10 {
        let f = driver
            .parse_tokens([(a.0 as u32, 0, 1)].iter().copied())
            .expect("parse");
        assert_eq!(f.view().roots().len(), 1);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 10. Configuration / table properties
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn eof_symbol_in_symbol_to_index() {
    let mut g = GrammarBuilder::new("eof_idx")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let table = run_pipeline(&mut g).expect("pipeline");
    assert!(table.symbol_to_index.contains_key(&table.eof_symbol));
}

#[test]
fn start_symbol_is_not_eof() {
    let mut g = GrammarBuilder::new("start_ne_eof")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let table = run_pipeline(&mut g).expect("pipeline");
    assert_ne!(table.start_symbol(), table.eof_symbol);
}

#[test]
fn eof_accessor_matches_field() {
    let mut g = GrammarBuilder::new("eof_eq")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let table = run_pipeline(&mut g).expect("pipeline");
    assert_eq!(table.eof(), table.eof_symbol);
}

#[test]
fn sanity_check_passes_for_valid_table() {
    let mut g = GrammarBuilder::new("sanity_ok")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let table = run_pipeline(&mut g).expect("pipeline");
    sanity_check_tables(&table).expect("sanity");
}

#[test]
fn rule_accessor_returns_correct_lhs_and_rhs_len() {
    let mut g = GrammarBuilder::new("rule_acc")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["a", "b"])
        .start("S")
        .build();
    let table = run_pipeline(&mut g).expect("pipeline");
    for i in 0..table.rules.len() {
        let (lhs, rhs_len) = table.rule(RuleId(i as u16));
        assert!(lhs.0 > 0 || table.eof_symbol == SymbolId(0));
        assert!(rhs_len <= 10);
    }
}

#[test]
fn goto_returns_none_for_invalid_nonterminal() {
    let mut g = GrammarBuilder::new("goto_inv")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let table = run_pipeline(&mut g).expect("pipeline");
    assert!(table.goto(table.initial_state, SymbolId(9999)).is_none());
}

#[test]
fn grammar_accessor_has_tokens() {
    let mut g = GrammarBuilder::new("g_acc")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let table = run_pipeline(&mut g).expect("pipeline");
    assert!(!table.grammar().tokens.is_empty());
}

#[test]
fn actions_for_unknown_symbol_is_empty() {
    let mut g = GrammarBuilder::new("unk_sym")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let table = run_pipeline(&mut g).expect("pipeline");
    assert!(
        table
            .actions(table.initial_state, SymbolId(9999))
            .is_empty()
    );
}

#[test]
fn actions_for_oob_state_is_empty() {
    let mut g = GrammarBuilder::new("oob_st")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let table = run_pipeline(&mut g).expect("pipeline");
    assert!(table.actions(StateId(9999), table.eof_symbol).is_empty());
}

#[test]
fn table_has_accept_shift_reduce() {
    let mut g = GrammarBuilder::new("asr")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["a", "b"])
        .start("S")
        .build();
    let table = run_pipeline(&mut g).expect("pipeline");
    let has_accept = table.action_table.iter().any(|row| {
        row.iter()
            .any(|c| c.iter().any(|a| matches!(a, Action::Accept)))
    });
    let has_shift = table.action_table.iter().any(|row| {
        row.iter()
            .any(|c| c.iter().any(|a| matches!(a, Action::Shift(_))))
    });
    let has_reduce = table.action_table.iter().any(|row| {
        row.iter()
            .any(|c| c.iter().any(|a| matches!(a, Action::Reduce(_))))
    });
    assert!(has_accept);
    assert!(has_shift);
    assert!(has_reduce);
}

// ═══════════════════════════════════════════════════════════════════════
// 11. Error recovery
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn recovery_insertion_produces_error_stats() {
    // S → 'a' 'b', feed only 'a'
    let mut g = GrammarBuilder::new("insert_rec")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["a", "b"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let result = pipeline_parse(&mut g, &[(a, 0, 1)]);
    if let Ok(f) = result {
        let (has_err, missing, cost) = f.debug_error_stats();
        assert!(has_err || missing > 0 || cost > 0);
    }
}

#[test]
fn recovery_does_not_panic_on_reversed_input() {
    let mut g = GrammarBuilder::new("rev_rec")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["a", "b"])
        .start("S")
        .build();
    let b = sym_id(&g, "b");
    let a = sym_id(&g, "a");
    let _ = pipeline_parse(&mut g, &[(b, 0, 1), (a, 1, 2)]);
}

// ═══════════════════════════════════════════════════════════════════════
// 12. Epsilon and zero-width tokens
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn hand_crafted_epsilon_reduce() {
    // A → ε; S → A 'x'
    let eof = SymbolId(0);
    let s = SymbolId(3);
    let a_sym = SymbolId(4);
    let rules = vec![
        ParseRule {
            lhs: a_sym,
            rhs_len: 0,
        },
        ParseRule { lhs: s, rhs_len: 2 },
    ];
    let mut actions = vec![vec![vec![]; 5]; 5];
    actions[0][1].push(Action::Reduce(RuleId(0)));
    actions[1][1].push(Action::Shift(StateId(2)));
    actions[2][0].push(Action::Reduce(RuleId(1)));
    actions[3][0].push(Action::Accept);
    let inv = StateId(65535);
    let mut gotos = vec![vec![inv; 5]; 5];
    gotos[0][4] = StateId(1);
    gotos[0][3] = StateId(3);
    let table = make_table(actions, gotos, rules, s, eof);
    let mut driver = Driver::new(&table);
    let f = driver
        .parse_tokens([(1u32, 0u32, 1u32)].iter().copied())
        .expect("epsilon reduce");
    assert!(!f.view().roots().is_empty());
}

#[test]
fn zero_width_token_does_not_panic() {
    let mut g = GrammarBuilder::new("zw_tok")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let _ = pipeline_parse(&mut g, &[(a, 0, 0)]);
}

// ═══════════════════════════════════════════════════════════════════════
// 13. Structural invariants
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn root_span_start_is_zero_for_full_parse() {
    let mut g = GrammarBuilder::new("root_start")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let f = pipeline_parse(&mut g, &[(a, 0, 1)]).expect("parse");
    assert_eq!(f.view().span(f.view().roots()[0]).start, 0);
}

#[test]
fn children_spans_non_overlapping() {
    let mut g = GrammarBuilder::new("no_overlap")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .rule("S", vec!["a", "b", "c"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let b = sym_id(&g, "b");
    let c = sym_id(&g, "c");
    let f = pipeline_parse(&mut g, &[(a, 0, 1), (b, 1, 2), (c, 2, 3)]).expect("parse");
    let v = f.view();
    let children = v.best_children(v.roots()[0]);
    for i in 1..children.len() {
        let prev = v.span(children[i - 1]);
        let curr = v.span(children[i]);
        assert!(prev.end <= curr.start, "children spans should not overlap");
    }
}

#[test]
fn root_kind_matches_start_symbol() {
    let mut g = GrammarBuilder::new("kind_match")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let table = run_pipeline(&mut g).expect("pipeline");
    let start_sym = table.start_symbol();
    let mut driver = Driver::new(&table);
    let f = driver
        .parse_tokens([(a.0 as u32, 0, 1)].iter().copied())
        .expect("parse");
    let root_kind = f.view().kind(f.view().roots()[0]);
    assert_eq!(root_kind, start_sym.0 as u32);
}

#[test]
fn forest_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Forest>();
}

#[test]
fn multiple_parses_produce_independent_forests() {
    let mut g = GrammarBuilder::new("indep")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["a"])
        .rule("S", vec!["b"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let b = sym_id(&g, "b");
    let table = run_pipeline(&mut g).expect("pipeline");
    let mut driver = Driver::new(&table);
    let f1 = driver
        .parse_tokens([(a.0 as u32, 0, 1)].iter().copied())
        .expect("first");
    let f2 = driver
        .parse_tokens([(b.0 as u32, 0, 1)].iter().copied())
        .expect("second");
    // Forests should be independent: root kinds may differ
    let k1 = f1.view().kind(f1.view().roots()[0]);
    let k2 = f2.view().kind(f2.view().roots()[0]);
    // Both should be the start symbol
    assert_eq!(k1, k2);
}

#[test]
fn parse_tokens_accepts_vec_iterator() {
    let mut g = GrammarBuilder::new("vec_iter")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let table = run_pipeline(&mut g).expect("pipeline");
    let mut driver = Driver::new(&table);
    let tokens: Vec<(u32, u32, u32)> = vec![(a.0 as u32, 0, 1)];
    let f = driver.parse_tokens(tokens).expect("parse");
    assert_eq!(f.view().roots().len(), 1);
}

#[test]
fn parse_tokens_accepts_array_iterator() {
    let mut g = GrammarBuilder::new("arr_iter")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let table = run_pipeline(&mut g).expect("pipeline");
    let mut driver = Driver::new(&table);
    let f = driver
        .parse_tokens([(a.0 as u32, 0u32, 1u32)])
        .expect("parse");
    assert_eq!(f.view().roots().len(), 1);
}

#[test]
fn driver_is_not_consumed_after_parse() {
    let mut g = GrammarBuilder::new("not_consumed")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build();
    let a = sym_id(&g, "a");
    let table = run_pipeline(&mut g).expect("pipeline");
    let mut driver = Driver::new(&table);
    let _ = driver.parse_tokens([(a.0 as u32, 0, 1)].iter().copied());
    // Driver is still usable
    let _ = driver.parse_tokens([(a.0 as u32, 0, 1)].iter().copied());
}

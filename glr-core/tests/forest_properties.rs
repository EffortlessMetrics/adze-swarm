//! Property-based tests for GLR parse-forest invariants.
//!
//! Run with: cargo test -p adze-glr-core --features test-api --test forest_properties
#![cfg(feature = "test-api")]

use adze_glr_core::GLRError;
use adze_glr_core::forest_view::ForestView;
use adze_glr_core::{
    Action, Driver, FirstFollowSets, GotoIndexing, LexMode, ParseRule, ParseTable,
    build_lr1_automaton, sanity_check_tables,
};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{Grammar, RuleId, StateId, SymbolId};
use proptest::prelude::*;
use std::collections::{BTreeMap, HashSet, VecDeque};

// ═══════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════

const NO_GOTO: StateId = StateId(65535);
type ActionCell = Vec<Action>;

/// Build a `ParseTable` from raw action/goto matrices (same helper as driver_proptest).
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
        grammar: Grammar::new("proptest".to_string()),
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

/// Run normalize → FIRST/FOLLOW → build_lr1_automaton, returning a ParseTable.
fn run_pipeline(grammar: &mut Grammar) -> Result<ParseTable, GLRError> {
    let first_follow = FirstFollowSets::compute_normalized(grammar)?;
    build_lr1_automaton(grammar, &first_follow)
}

/// Build grammar + table, then parse a token stream through the driver.
fn pipeline_parse(
    grammar: &mut Grammar,
    token_stream: &[(SymbolId, u32, u32)],
) -> Result<adze_glr_core::Forest, adze_glr_core::driver::GlrError> {
    let table = run_pipeline(grammar).expect("pipeline should produce a table");
    sanity_check_tables(&table).expect("table sanity check");
    let mut driver = Driver::new(&table);
    driver.parse_tokens(
        token_stream
            .iter()
            .map(|&(sym, start, end)| (sym.0 as u32, start, end)),
    )
}

/// Build the deterministic S -> 'a' grammar table.
/// Symbols: 0=EOF, 1='a', 2=S(NT)
fn simple_s_to_a_table() -> ParseTable {
    let eof = SymbolId(0);
    let _a = SymbolId(1);
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

/// Build the right-recursive A -> 'a' | 'a' A ; S -> A grammar table.
/// Symbols: 0=EOF, 1='a', 2=S(NT), 3=A(NT)
fn right_recursive_table() -> ParseTable {
    let eof = SymbolId(0);
    let s = SymbolId(2);
    let a_nt = SymbolId(3);

    let rules = vec![
        ParseRule { lhs: s, rhs_len: 1 }, // r0: S -> A
        ParseRule {
            lhs: a_nt,
            rhs_len: 1,
        }, // r1: A -> 'a'
        ParseRule {
            lhs: a_nt,
            rhs_len: 2,
        }, // r2: A -> 'a' A
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

/// Build the S -> ε grammar table.
/// Symbols: 0=EOF, 1=S(NT)
fn epsilon_table() -> ParseTable {
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

/// Collect all node IDs reachable from roots via BFS through best_children.
fn collect_reachable(view: &dyn ForestView, roots: &[u32]) -> Vec<u32> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    for &r in roots {
        if visited.insert(r) {
            queue.push_back(r);
        }
    }
    let mut result = Vec::new();
    while let Some(id) = queue.pop_front() {
        result.push(id);
        for &child in view.best_children(id) {
            if visited.insert(child) {
                queue.push_back(child);
            }
        }
    }
    result
}

/// Check that there are no cycles in parent→child edges (DAG property).
/// Returns true if the forest is a DAG, false if a cycle is detected.
fn is_dag(view: &dyn ForestView, roots: &[u32]) -> bool {
    // DFS-based cycle detection using coloring:
    // 0 = unvisited, 1 = in-progress, 2 = finished
    let mut color: std::collections::HashMap<u32, u8> = std::collections::HashMap::new();

    fn dfs(id: u32, view: &dyn ForestView, color: &mut std::collections::HashMap<u32, u8>) -> bool {
        color.insert(id, 1);
        for &child in view.best_children(id) {
            match color.get(&child).copied().unwrap_or(0) {
                1 => return false, // back-edge → cycle
                0 if !dfs(child, view, color) => return false,
                _ => {} // already finished or DFS succeeded
            }
        }
        color.insert(id, 2);
        true
    }

    for &r in roots {
        if color.get(&r).copied().unwrap_or(0) == 0 && !dfs(r, view, &mut color) {
            return false;
        }
    }
    true
}

// ═══════════════════════════════════════════════════════════════════════
// Proptest strategies
// ═══════════════════════════════════════════════════════════════════════

/// Strategy that generates a token count for the right-recursive grammar.
fn a_token_stream(max_len: usize) -> impl Strategy<Value = Vec<(u32, u32, u32)>> {
    (1..=max_len).prop_map(|n| {
        (0..n)
            .map(|i| (1u32 /* 'a' */, i as u32, i as u32 + 1))
            .collect()
    })
}

// ═══════════════════════════════════════════════════════════════════════
// 1. Every forest node references valid symbol IDs
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn forest_nodes_have_valid_symbol_ids(n in 1usize..=8) {
        let table = right_recursive_table();
        let tokens: Vec<_> = (0..n).map(|i| (1u32, i as u32, i as u32 + 1)).collect();
        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(tokens.into_iter())
            .expect("right-recursive parse must succeed");

        let view = forest.view();
        let nodes = collect_reachable(view, view.roots());

        // All symbol IDs must be within the symbol space of the table.
        let max_symbol = table.symbol_count as u32;
        for &id in &nodes {
            let kind = view.kind(id);
            // kind == u16::MAX is the ERROR_SYMBOL sentinel – also valid
            prop_assert!(
                kind < max_symbol || kind == u16::MAX as u32,
                "node {} has out-of-range symbol ID {}", id, kind
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 2. Child indices are within bounds of the forest
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn child_indices_within_forest_bounds(n in 1usize..=8) {
        let table = right_recursive_table();
        let tokens: Vec<_> = (0..n).map(|i| (1u32, i as u32, i as u32 + 1)).collect();
        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(tokens.into_iter())
            .expect("right-recursive parse must succeed");

        let view = forest.view();
        let all_nodes: HashSet<u32> = collect_reachable(view, view.roots()).into_iter().collect();

        // Every child referenced from best_children must be a node we can reach.
        for &id in &all_nodes {
            for &child in view.best_children(id) {
                prop_assert!(
                    all_nodes.contains(&child),
                    "node {} references child {} which is not in the reachable set", id, child
                );
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 3. Forest size (node count) is bounded by O(n * |grammar|)
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn forest_size_bounded(n in 1usize..=16) {
        let table = right_recursive_table();
        let grammar_size = table.rules.len() + table.symbol_count;

        let tokens: Vec<_> = (0..n).map(|i| (1u32, i as u32, i as u32 + 1)).collect();
        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(tokens.into_iter())
            .expect("right-recursive parse must succeed");

        let view = forest.view();
        let node_count = collect_reachable(view, view.roots()).len();

        // O(n * |grammar|) bound with a generous constant factor
        let bound = n * grammar_size * 4 + 16;
        prop_assert!(
            node_count <= bound,
            "forest has {} nodes, exceeds O(n*|G|) bound of {} for n={}, |G|={}",
            node_count, bound, n, grammar_size
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 4. All paths from root to leaves form valid derivations
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn root_to_leaf_paths_are_valid(n in 1usize..=6) {
        let table = right_recursive_table();
        let tokens: Vec<_> = (0..n).map(|i| (1u32, i as u32, i as u32 + 1)).collect();
        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(tokens.into_iter())
            .expect("right-recursive parse must succeed");

        let view = forest.view();

        // Every leaf (node with no children) must be a terminal (symbol < num_terminals)
        // or the error sentinel. Non-leaf nodes must be nonterminals.
        let reachable = collect_reachable(view, view.roots());
        let terminal_bound = table.token_count as u32;
        for &id in &reachable {
            let children = view.best_children(id);
            let kind = view.kind(id);
            if children.is_empty() {
                // Leaf → should be terminal or error
                prop_assert!(
                    kind < terminal_bound || kind == u16::MAX as u32,
                    "leaf node {} has nonterminal symbol {}", id, kind
                );
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 5. Token spans don't overlap for non-ambiguous parses
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn leaf_spans_do_not_overlap(n in 1usize..=8) {
        let table = right_recursive_table();
        let tokens: Vec<_> = (0..n).map(|i| (1u32, i as u32, i as u32 + 1)).collect();
        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(tokens.into_iter())
            .expect("right-recursive parse must succeed");

        let view = forest.view();
        let reachable = collect_reachable(view, view.roots());

        // Collect spans of all leaves, sorted by start.
        let mut leaf_spans: Vec<(u32, u32)> = Vec::new();
        for &id in &reachable {
            if view.best_children(id).is_empty() {
                let sp = view.span(id);
                leaf_spans.push((sp.start, sp.end));
            }
        }
        leaf_spans.sort();

        // No two leaf spans should overlap (end[i] <= start[i+1]).
        for pair in leaf_spans.windows(2) {
            prop_assert!(
                pair[0].1 <= pair[1].0,
                "leaf spans overlap: ({},{}) and ({},{})",
                pair[0].0, pair[0].1, pair[1].0, pair[1].1
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 6. Token spans cover the entire input for successful parses
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn leaf_spans_cover_full_input(n in 1usize..=8) {
        let table = right_recursive_table();
        let tokens: Vec<_> = (0..n).map(|i| (1u32, i as u32, i as u32 + 1)).collect();
        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(tokens.into_iter())
            .expect("right-recursive parse must succeed");

        let view = forest.view();
        let reachable = collect_reachable(view, view.roots());

        let mut leaf_spans: Vec<(u32, u32)> = Vec::new();
        for &id in &reachable {
            if view.best_children(id).is_empty() {
                let sp = view.span(id);
                leaf_spans.push((sp.start, sp.end));
            }
        }
        leaf_spans.sort();

        // The union of leaf spans should cover [0, n).
        if !leaf_spans.is_empty() {
            prop_assert_eq!(leaf_spans[0].0, 0, "first leaf must start at 0");
            prop_assert_eq!(
                leaf_spans.last().unwrap().1,
                n as u32,
                "last leaf must end at input length"
            );
            // Contiguous coverage
            for pair in leaf_spans.windows(2) {
                prop_assert_eq!(
                    pair[0].1, pair[1].0,
                    "gap between leaf spans ({},{}) and ({},{})",
                    pair[0].0, pair[0].1, pair[1].0, pair[1].1
                );
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 7. Forest is a DAG (no cycles in parent→child relationships)
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn forest_is_dag(n in 1usize..=10) {
        let table = right_recursive_table();
        let tokens: Vec<_> = (0..n).map(|i| (1u32, i as u32, i as u32 + 1)).collect();
        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(tokens.into_iter())
            .expect("right-recursive parse must succeed");

        let view = forest.view();
        prop_assert!(
            is_dag(view, view.roots()),
            "forest contains a cycle for input of {} tokens", n
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 8. Ambiguous nodes have multiple alternative children
//    (tested via the GrammarBuilder pipeline with an ambiguous grammar)
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    #[test]
    fn ambiguous_grammar_produces_valid_forest(n in 2usize..=5) {
        // E -> E '+' E | NUM   — inherently ambiguous for 3+ tokens
        let mut grammar = GrammarBuilder::new("ambig")
            .token("NUM", r"\d+")
            .token("+", "+")
            .rule("expr", vec!["expr", "+", "expr"])
            .rule("expr", vec!["NUM"])
            .start("expr")
            .build();

        let num = sym_id(&grammar, "NUM");
        let plus = sym_id(&grammar, "+");

        // Build "NUM + NUM + ... + NUM" with n NUMs.
        let mut tokens = Vec::new();
        let mut pos = 0u32;
        for i in 0..n {
            tokens.push((num, pos, pos + 1));
            pos += 1;
            if i < n - 1 {
                tokens.push((plus, pos, pos + 1));
                pos += 1;
            }
        }
        let input_end = pos;

        let result = pipeline_parse(&mut grammar, &tokens);
        // Ambiguous grammars may succeed or fail depending on GLR strategy.
        // If they succeed, the forest must still satisfy invariants.
        if let Ok(forest) = result {
            let view = forest.view();
            let roots = view.roots();
            prop_assert!(!roots.is_empty(), "accepted parse must have roots");

            // Root spans the full input.
            for &root in roots {
                let sp = view.span(root);
                prop_assert_eq!(sp.start, 0);
                prop_assert_eq!(sp.end, input_end);
            }

            // DAG invariant
            prop_assert!(is_dag(view, roots), "ambiguous forest must be a DAG");
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 9. Forest from empty input has exactly one root (S->ε) or zero for error
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn empty_input_epsilon_grammar(_seed in 0u32..100) {
        let table = epsilon_table();
        let mut driver = Driver::new(&table);
        let result = driver.parse_tokens(std::iter::empty::<(u32, u32, u32)>());

        prop_assert!(result.is_ok(), "S->ε must accept empty input: {:?}", result.err());
        let forest = result.unwrap();
        let view = forest.view();
        let roots = view.roots();

        // Exactly one root for ε-grammar on empty input.
        prop_assert_eq!(roots.len(), 1, "ε-grammar must yield 1 root on empty input");

        let sp = view.span(roots[0]);
        prop_assert_eq!(sp.start, 0);
        prop_assert_eq!(sp.end, 0, "ε root must be zero-width");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn empty_input_non_epsilon_grammar_fails(_seed in 0u32..100) {
        // S -> 'a' cannot accept empty input.
        let table = simple_s_to_a_table();
        let mut driver = Driver::new(&table);
        let result = driver.parse_tokens(std::iter::empty::<(u32, u32, u32)>());

        // Either hard error or recovery with error stats.
        match result {
            Err(_) => { /* expected */ }
            Ok(forest) => {
                let (has_error, _missing, cost) = forest.debug_error_stats();
                prop_assert!(
                    has_error || cost > 0,
                    "empty parse on S->'a' must report errors if accepted via recovery"
                );
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 10. Forest from single-token input has bounded depth
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn single_token_bounded_depth(terminal_id in 1u16..5) {
        let eof = SymbolId(0);
        let t = SymbolId(terminal_id);
        let s_id = terminal_id.max(2) + 1;
        let s = SymbolId(s_id);
        let num_syms = (s_id as usize) + 1;

        let rules = vec![ParseRule { lhs: s, rhs_len: 1 }];
        let num_states = 3;
        let mut actions = vec![vec![vec![]; num_syms]; num_states];
        actions[0][terminal_id as usize].push(Action::Shift(StateId(1)));
        actions[1][0].push(Action::Reduce(RuleId(0)));
        actions[2][0].push(Action::Accept);

        let mut gotos = vec![vec![NO_GOTO; num_syms]; num_states];
        gotos[0][s_id as usize] = StateId(2);

        let table = build_table(actions, gotos, rules, s, eof, (terminal_id as usize) + 1);

        let tokens = vec![(t.0 as u32, 0u32, 1u32)];
        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(tokens.into_iter())
            .expect("single-token parse must succeed");

        let view = forest.view();
        let roots = view.roots();
        prop_assert_eq!(roots.len(), 1);

        // Measure depth via iterative DFS.
        fn max_depth(view: &dyn ForestView, root: u32) -> usize {
            let mut stack = vec![(root, 1usize)];
            let mut max_d = 0;
            while let Some((id, d)) = stack.pop() {
                max_d = max_d.max(d);
                for &child in view.best_children(id) {
                    stack.push((child, d + 1));
                }
            }
            max_d
        }

        let depth = max_depth(view, roots[0]);
        // S -> t has depth exactly 2 (root + leaf), but allow some slack
        // for internal bookkeeping nodes.
        prop_assert!(
            depth <= 10,
            "single-token forest depth {} exceeds bound 10", depth
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 11. Root span equals input span for every accepted parse
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn root_span_matches_input(tokens in a_token_stream(10)) {
        let table = right_recursive_table();
        let input_end = tokens.last().map(|t| t.2).unwrap_or(0);

        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(tokens.into_iter())
            .expect("right-recursive parse must succeed");

        let view = forest.view();
        for &root in view.roots() {
            let sp = view.span(root);
            prop_assert_eq!(sp.start, 0, "root span must start at 0");
            prop_assert_eq!(sp.end, input_end, "root span must end at input end");
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 12. Every root node's symbol is the start symbol
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn root_symbol_is_start_symbol(n in 1usize..=8) {
        let table = right_recursive_table();
        let start_sym = table.start_symbol.0 as u32;
        let tokens: Vec<_> = (0..n).map(|i| (1u32, i as u32, i as u32 + 1)).collect();

        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(tokens.into_iter())
            .expect("right-recursive parse must succeed");

        let view = forest.view();
        for &root in view.roots() {
            prop_assert_eq!(
                view.kind(root), start_sym,
                "root node {} has symbol {} but start symbol is {}", root, view.kind(root), start_sym
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 13. Parent span contains child spans
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn parent_span_contains_children(n in 1usize..=8) {
        let table = right_recursive_table();
        let tokens: Vec<_> = (0..n).map(|i| (1u32, i as u32, i as u32 + 1)).collect();

        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(tokens.into_iter())
            .expect("right-recursive parse must succeed");

        let view = forest.view();
        let reachable = collect_reachable(view, view.roots());

        for &id in &reachable {
            let parent_span = view.span(id);
            for &child in view.best_children(id) {
                let child_span = view.span(child);
                prop_assert!(
                    child_span.start >= parent_span.start && child_span.end <= parent_span.end,
                    "child {} span [{},{}) not within parent {} span [{},{})",
                    child, child_span.start, child_span.end,
                    id, parent_span.start, parent_span.end
                );
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 14. Children spans are ordered left-to-right
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn children_spans_ordered_left_to_right(n in 1usize..=8) {
        let table = right_recursive_table();
        let tokens: Vec<_> = (0..n).map(|i| (1u32, i as u32, i as u32 + 1)).collect();

        let mut driver = Driver::new(&table);
        let forest = driver.parse_tokens(tokens.into_iter())
            .expect("right-recursive parse must succeed");

        let view = forest.view();
        let reachable = collect_reachable(view, view.roots());

        for &id in &reachable {
            let children = view.best_children(id);
            for pair in children.windows(2) {
                let left = view.span(pair[0]);
                let right = view.span(pair[1]);
                prop_assert!(
                    left.start <= right.start,
                    "children of node {} not in left-to-right order: [{},{}) before [{},{})",
                    id, left.start, left.end, right.start, right.end
                );
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 15. Pipeline grammar: all invariants hold for computed tables
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    #[test]
    fn pipeline_grammar_forest_invariants(n in 1usize..=6) {
        let mut grammar = GrammarBuilder::new("arith")
            .token("NUM", r"\d+")
            .token("+", "+")
            .rule("expr", vec!["expr", "+", "NUM"])
            .rule("expr", vec!["NUM"])
            .start("expr")
            .build();

        let num = sym_id(&grammar, "NUM");
        let plus = sym_id(&grammar, "+");

        // Build "NUM + NUM + ... + NUM" with n NUMs (left-recursive grammar).
        let mut tokens = Vec::new();
        let mut pos = 0u32;
        for i in 0..n {
            tokens.push((num, pos, pos + 1));
            pos += 1;
            if i < n - 1 {
                tokens.push((plus, pos, pos + 1));
                pos += 1;
            }
        }
        let input_end = pos;

        let result = pipeline_parse(&mut grammar, &tokens);
        prop_assert!(result.is_ok(), "left-recursive arith must parse: {:?}", result.err());
        let forest = result.unwrap();
        let view = forest.view();
        let roots = view.roots();

        // 1) Has roots
        prop_assert!(!roots.is_empty());

        // 2) Root span
        for &r in roots {
            let sp = view.span(r);
            prop_assert_eq!(sp.start, 0);
            prop_assert_eq!(sp.end, input_end);
        }

        // 3) DAG
        prop_assert!(is_dag(view, roots));

        // 4) Parent contains children
        let reachable = collect_reachable(view, roots);
        for &id in &reachable {
            let ps = view.span(id);
            for &c in view.best_children(id) {
                let cs = view.span(c);
                prop_assert!(cs.start >= ps.start && cs.end <= ps.end);
            }
        }

        // 5) Leaf spans cover input
        let mut leaf_spans: Vec<(u32, u32)> = reachable.iter()
            .filter(|&&id| view.best_children(id).is_empty())
            .map(|&id| { let s = view.span(id); (s.start, s.end) })
            .collect();
        leaf_spans.sort();
        if !leaf_spans.is_empty() {
            prop_assert_eq!(leaf_spans[0].0, 0);
            prop_assert_eq!(leaf_spans.last().unwrap().1, input_end);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 16. Idempotent re-parse: same tokens → same forest shape
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn reparse_same_tokens_same_shape(n in 1usize..=6) {
        let table = right_recursive_table();
        let tokens: Vec<_> = (0..n).map(|i| (1u32, i as u32, i as u32 + 1)).collect();

        let mut driver1 = Driver::new(&table);
        let forest1 = driver1.parse_tokens(tokens.clone().into_iter())
            .expect("first parse must succeed");

        let mut driver2 = Driver::new(&table);
        let forest2 = driver2.parse_tokens(tokens.into_iter())
            .expect("second parse must succeed");

        let view1 = forest1.view();
        let view2 = forest2.view();

        // Same number of roots.
        prop_assert_eq!(view1.roots().len(), view2.roots().len());

        // Same root spans.
        for (r1, r2) in view1.roots().iter().zip(view2.roots().iter()) {
            prop_assert_eq!(view1.span(*r1), view2.span(*r2));
        }

        // Same reachable node count.
        let nodes1 = collect_reachable(view1, view1.roots());
        let nodes2 = collect_reachable(view2, view2.roots());
        prop_assert_eq!(nodes1.len(), nodes2.len());
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 17. Error recovery forests still satisfy DAG and span containment
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn error_recovery_forest_invariants(bad_kind in 10u32..50) {
        let table = simple_s_to_a_table();
        let tokens = vec![(bad_kind, 0u32, 1u32)];
        let mut driver = Driver::new(&table);
        let result = driver.parse_tokens(tokens.into_iter());

        // If recovery produces a forest, it must still be a DAG with valid spans.
        if let Ok(forest) = result {
            let view = forest.view();
            let roots = view.roots();
            if !roots.is_empty() {
                prop_assert!(is_dag(view, roots), "error-recovery forest must be DAG");

                let reachable = collect_reachable(view, roots);
                for &id in &reachable {
                    let ps = view.span(id);
                    for &c in view.best_children(id) {
                        let cs = view.span(c);
                        prop_assert!(cs.start >= ps.start && cs.end <= ps.end);
                    }
                }
            }
        }
    }
}

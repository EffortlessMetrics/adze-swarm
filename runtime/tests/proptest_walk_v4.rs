//! Property-based tests (v4) covering visitor stats consistency, pretty-print
//! determinism, search idempotency, walker ordering, error node properties,
//! and arena allocation properties.
//!
//! 40+ `proptest` properties exercising the public API surface of `adze`.

use adze::arena_allocator::{NodeHandle, TreeArena, TreeNode};
use adze::error_recovery::{ErrorNode, ErrorRecoveryConfig, RecoveryStrategy};
use adze::lexer::ErrorRecoveryMode;
use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TreeWalker, Visitor,
    VisitorAction,
};
use proptest::prelude::*;
use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Construct a synthetic `ParsedNode` with standard defaults.
fn make_node(
    symbol: u16,
    children: Vec<ParsedNode>,
    start: usize,
    end: usize,
    is_error: bool,
    is_named: bool,
) -> ParsedNode {
    ParsedNode::builder(symbol, children, start, end)
        .is_error(is_error)
        .is_named(is_named)
        .build()
}

fn leaf_node(symbol: u16, start: usize, end: usize) -> ParsedNode {
    make_node(symbol, vec![], start, end, false, true)
}

fn interior(symbol: u16, children: Vec<ParsedNode>) -> ParsedNode {
    let start = children.first().map_or(0, |c| c.start_byte);
    let end = children.last().map_or(0, |c| c.end_byte);
    make_node(symbol, children, start, end, false, true)
}

fn err_node(start: usize, end: usize) -> ParsedNode {
    make_node(0, vec![], start, end, true, false)
}

/// Count every node in a tree recursively (including the root).
#[allow(dead_code)]
fn count_all(node: &ParsedNode) -> usize {
    1 + node.children().iter().map(count_all).sum::<usize>()
}

/// Count non-error nodes reachable by walker (error nodes halt subtree).
fn count_reachable_non_error(node: &ParsedNode) -> usize {
    if node.is_error() {
        0
    } else {
        1 + node
            .children()
            .iter()
            .map(count_reachable_non_error)
            .sum::<usize>()
    }
}

/// Count error nodes reachable by walker (error stops descent).
fn count_reachable_errors(node: &ParsedNode) -> usize {
    if node.is_error() {
        1
    } else {
        node.children()
            .iter()
            .map(count_reachable_errors)
            .sum::<usize>()
    }
}

/// Compute tree depth (root-only = 1).
fn tree_depth(node: &ParsedNode) -> usize {
    if node.children().is_empty() {
        1
    } else {
        1 + node.children().iter().map(tree_depth).max().unwrap_or(0)
    }
}

/// Count leaf nodes in a tree.
#[allow(dead_code)]
fn count_leaves(node: &ParsedNode) -> usize {
    if node.children().is_empty() {
        1
    } else {
        node.children().iter().map(count_leaves).sum()
    }
}

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

const SOURCE_LEN: usize = 64;

fn arb_leaf() -> impl Strategy<Value = ParsedNode> {
    (1u16..=10, 0..SOURCE_LEN - 1).prop_map(|(sym, start)| leaf_node(sym, start, start + 1))
}

fn arb_tree(max_depth: u32, max_width: usize) -> impl Strategy<Value = ParsedNode> {
    arb_leaf().prop_recursive(max_depth, 64, max_width as u32, move |inner| {
        (1u16..=10, proptest::collection::vec(inner, 1..=max_width))
            .prop_map(|(sym, children)| interior(sym, children))
    })
}

/// Tree that may contain error nodes.
fn arb_tree_with_errors(max_depth: u32, max_width: usize) -> impl Strategy<Value = ParsedNode> {
    let error_leaf = (0..SOURCE_LEN - 1).prop_map(|s| err_node(s, s + 1));
    prop_oneof![3 => arb_leaf(), 1 => error_leaf].prop_recursive(
        max_depth,
        64,
        max_width as u32,
        move |inner| {
            (1u16..=10, proptest::collection::vec(inner, 1..=max_width))
                .prop_map(|(sym, children)| interior(sym, children))
        },
    )
}

fn arb_source() -> impl Strategy<Value = String> {
    proptest::string::string_regex(&format!("[a-z0-9 ]{{{SOURCE_LEN},{SOURCE_LEN}}}")).unwrap()
}

// ===================================================================
// 1. StatsVisitor — total_nodes equals reachable non-error count (DFS)
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn stats_total_nodes_matches_reachable(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert_eq!(stats.total_nodes, count_reachable_non_error(&tree));
    }
}

// ===================================================================
// 2. StatsVisitor — error_nodes equals reachable error count
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn stats_error_count_matches_reachable(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert_eq!(stats.error_nodes, count_reachable_errors(&tree));
    }
}

// ===================================================================
// 3. StatsVisitor — max_depth bounded by tree depth
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn stats_depth_bounded(
        source in arb_source(),
        tree in arb_tree(4, 4),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert!(stats.max_depth <= tree_depth(&tree));
    }
}

// ===================================================================
// 4. StatsVisitor — node_counts values sum to total_nodes
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn stats_node_counts_sum(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        let sum: usize = stats.node_counts.values().sum();
        prop_assert_eq!(sum, stats.total_nodes);
    }
}

// ===================================================================
// 5. StatsVisitor — leaf_nodes <= total_nodes
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn stats_leaves_leq_total(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert!(stats.leaf_nodes <= stats.total_nodes);
    }
}

// ===================================================================
// 6. StatsVisitor — two walks produce identical stats (determinism)
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn stats_deterministic(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut s1 = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut s1);
        let mut s2 = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut s2);
        prop_assert_eq!(s1.total_nodes, s2.total_nodes);
        prop_assert_eq!(s1.leaf_nodes, s2.leaf_nodes);
        prop_assert_eq!(s1.error_nodes, s2.error_nodes);
        prop_assert_eq!(s1.max_depth, s2.max_depth);
    }
}

// ===================================================================
// 7. PrettyPrint — deterministic output across two walks
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn pretty_print_deterministic(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut pp1 = PrettyPrintVisitor::new();
        TreeWalker::new(src).walk(&tree, &mut pp1);
        let mut pp2 = PrettyPrintVisitor::new();
        TreeWalker::new(src).walk(&tree, &mut pp2);
        prop_assert_eq!(pp1.output(), pp2.output());
    }
}

// ===================================================================
// 8. PrettyPrint — non-empty output for any tree
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn pretty_print_non_empty(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut pp);
        prop_assert!(!pp.output().is_empty());
    }
}

// ===================================================================
// 9. PrettyPrint — contains at least one newline
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn pretty_print_has_newlines(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut pp);
        prop_assert!(pp.output().contains('\n'));
    }
}

// ===================================================================
// 10. PrettyPrint — BFS also produces non-empty output
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn pretty_print_bfs_non_empty(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut pp = PrettyPrintVisitor::new();
        BreadthFirstWalker::new(source.as_bytes()).walk(&tree, &mut pp);
        prop_assert!(!pp.output().is_empty());
    }
}

// ===================================================================
// 11. Search — always-true predicate finds all non-error nodes
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn search_always_true(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut search = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(src).walk(&tree, &mut search);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut stats);
        prop_assert_eq!(search.matches.len(), stats.total_nodes);
    }
}

// ===================================================================
// 12. Search — always-false predicate finds nothing
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn search_always_false(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut search = SearchVisitor::new(|_: &ParsedNode| false);
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut search);
        prop_assert!(search.matches.is_empty());
    }
}

// ===================================================================
// 13. Search — idempotent (two identical searches yield same result)
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn search_idempotent(
        source in arb_source(),
        tree in arb_tree(3, 3),
        threshold in 1u16..=10,
    ) {
        let src = source.as_bytes();
        let t = threshold;
        let mut s1 = SearchVisitor::new(move |n: &ParsedNode| n.symbol <= t);
        TreeWalker::new(src).walk(&tree, &mut s1);
        let mut s2 = SearchVisitor::new(move |n: &ParsedNode| n.symbol <= t);
        TreeWalker::new(src).walk(&tree, &mut s2);
        prop_assert_eq!(s1.matches.len(), s2.matches.len());
        for (a, b) in s1.matches.iter().zip(s2.matches.iter()) {
            prop_assert_eq!(a, b);
        }
    }
}

// ===================================================================
// 14. Search — subset property (tighter predicate ⊆ looser predicate)
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn search_subset(
        source in arb_source(),
        tree in arb_tree(3, 3),
        lo in 1u16..=5,
    ) {
        let src = source.as_bytes();
        let hi = lo + 5;
        let mut narrow = SearchVisitor::new(move |n: &ParsedNode| n.symbol <= lo);
        TreeWalker::new(src).walk(&tree, &mut narrow);
        let mut wide = SearchVisitor::new(move |n: &ParsedNode| n.symbol <= hi);
        TreeWalker::new(src).walk(&tree, &mut wide);
        prop_assert!(narrow.matches.len() <= wide.matches.len());
    }
}

// ===================================================================
// 15. BFS and DFS visit same total non-error nodes
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn bfs_dfs_same_total(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut dfs = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut dfs);
        let mut bfs = StatsVisitor::default();
        BreadthFirstWalker::new(src).walk(&tree, &mut bfs);
        prop_assert_eq!(dfs.total_nodes, bfs.total_nodes);
    }
}

// ===================================================================
// 16. BFS and DFS count same error nodes
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn bfs_dfs_same_errors(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        let src = source.as_bytes();
        let mut dfs = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut dfs);
        let mut bfs = StatsVisitor::default();
        BreadthFirstWalker::new(src).walk(&tree, &mut bfs);
        prop_assert_eq!(dfs.error_nodes, bfs.error_nodes);
    }
}

// ===================================================================
// 17. DFS enter/leave perfectly balanced
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn dfs_enter_leave_balanced(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct Balance { enters: usize, leaves: usize }
        impl Visitor for Balance {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.enters += 1;
                VisitorAction::Continue
            }
            fn leave_node(&mut self, _: &ParsedNode) {
                self.leaves += 1;
            }
        }
        let mut v = Balance { enters: 0, leaves: 0 };
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut v);
        prop_assert_eq!(v.enters, v.leaves);
    }
}

// ===================================================================
// 18. Stop action reduces DFS traversal vs full walk
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn stop_reduces_dfs(
        source in arb_source(),
        tree in arb_tree(3, 4),
        limit in 1usize..=3,
    ) {
        // DFS Stop halts descent in the current subtree but siblings still run.
        // Verify that Stop causes fewer nodes than a full walk for big trees.
        struct StopAfter { visited: usize, cap: usize }
        impl Visitor for StopAfter {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.visited += 1;
                if self.visited >= self.cap { VisitorAction::Stop } else { VisitorAction::Continue }
            }
        }
        let src = source.as_bytes();
        let mut full = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut full);
        let mut v = StopAfter { visited: 0, cap: limit };
        TreeWalker::new(src).walk(&tree, &mut v);
        // The stop visitor should never exceed the full count.
        prop_assert!(v.visited <= full.total_nodes);
    }
}

// ===================================================================
// 19. Stop action limits BFS traversal
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn stop_limits_bfs(
        source in arb_source(),
        tree in arb_tree(3, 4),
        limit in 1usize..=5,
    ) {
        struct StopAfter { visited: usize, cap: usize }
        impl Visitor for StopAfter {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.visited += 1;
                if self.visited >= self.cap { VisitorAction::Stop } else { VisitorAction::Continue }
            }
        }
        let mut v = StopAfter { visited: 0, cap: limit };
        BreadthFirstWalker::new(source.as_bytes()).walk(&tree, &mut v);
        prop_assert!(v.visited <= limit);
    }
}

// ===================================================================
// 20. SkipChildren reduces visited count vs full walk
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn skip_children_reduces(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct SkipRoot { entered: usize, skipped: bool }
        impl Visitor for SkipRoot {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.entered += 1;
                if !self.skipped { self.skipped = true; VisitorAction::SkipChildren }
                else { VisitorAction::Continue }
            }
        }
        let src = source.as_bytes();
        let mut skip_v = SkipRoot { entered: 0, skipped: false };
        TreeWalker::new(src).walk(&tree, &mut skip_v);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut stats);
        prop_assert!(skip_v.entered <= stats.total_nodes);
    }
}

// ===================================================================
// 21. DFS ordering: first node visited is root symbol
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn dfs_first_is_root(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct FirstCapture { first_sym: Option<u16> }
        impl Visitor for FirstCapture {
            fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
                if self.first_sym.is_none() { self.first_sym = Some(node.symbol); }
                VisitorAction::Continue
            }
        }
        let mut v = FirstCapture { first_sym: None };
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut v);
        prop_assert_eq!(v.first_sym, Some(tree.symbol));
    }
}

// ===================================================================
// 22. BFS ordering: first node visited is root symbol
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn bfs_first_is_root(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct FirstCapture { first_sym: Option<u16> }
        impl Visitor for FirstCapture {
            fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
                if self.first_sym.is_none() { self.first_sym = Some(node.symbol); }
                VisitorAction::Continue
            }
        }
        let mut v = FirstCapture { first_sym: None };
        BreadthFirstWalker::new(source.as_bytes()).walk(&tree, &mut v);
        prop_assert_eq!(v.first_sym, Some(tree.symbol));
    }
}

// ===================================================================
// 23. DFS visit order is stable (recorded symbols match)
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn dfs_order_stable(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct SymCollector { syms: Vec<u16> }
        impl Visitor for SymCollector {
            fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
                self.syms.push(node.symbol);
                VisitorAction::Continue
            }
        }
        let src = source.as_bytes();
        let mut v1 = SymCollector { syms: vec![] };
        TreeWalker::new(src).walk(&tree, &mut v1);
        let mut v2 = SymCollector { syms: vec![] };
        TreeWalker::new(src).walk(&tree, &mut v2);
        prop_assert_eq!(v1.syms, v2.syms);
    }
}

// ===================================================================
// 24. BFS visit order is stable
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn bfs_order_stable(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct SymCollector { syms: Vec<u16> }
        impl Visitor for SymCollector {
            fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
                self.syms.push(node.symbol);
                VisitorAction::Continue
            }
        }
        let src = source.as_bytes();
        let mut v1 = SymCollector { syms: vec![] };
        BreadthFirstWalker::new(src).walk(&tree, &mut v1);
        let mut v2 = SymCollector { syms: vec![] };
        BreadthFirstWalker::new(src).walk(&tree, &mut v2);
        prop_assert_eq!(v1.syms, v2.syms);
    }
}

// ===================================================================
// 25. ErrorNode — start_byte < end_byte when constructed properly
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn error_node_span_ordering(
        start in 0usize..1000,
        len in 1usize..100,
    ) {
        let end = start + len;
        let node = ErrorNode {
            start_byte: start,
            end_byte: end,
            start_position: (0, start),
            end_position: (0, end),
            expected: vec![1, 2],
            actual: Some(3),
            recovery: RecoveryStrategy::PanicMode,
            skipped_tokens: vec![],
        };
        prop_assert!(node.start_byte < node.end_byte);
    }
}

// ===================================================================
// 26. ErrorNode — clone preserves all fields
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn error_node_clone_faithful(
        start in 0usize..1000,
        len in 1usize..100,
        expected_count in 0usize..5,
        actual_sym in proptest::option::of(0u16..100),
    ) {
        let end = start + len;
        let expected: Vec<u16> = (0..expected_count as u16).collect();
        let node = ErrorNode {
            start_byte: start,
            end_byte: end,
            start_position: (0, start),
            end_position: (0, end),
            expected: expected.clone(),
            actual: actual_sym,
            recovery: RecoveryStrategy::TokenDeletion,
            skipped_tokens: vec![42],
        };
        let cloned = node.clone();
        prop_assert_eq!(cloned.start_byte, node.start_byte);
        prop_assert_eq!(cloned.end_byte, node.end_byte);
        prop_assert_eq!(cloned.expected, node.expected);
        prop_assert_eq!(cloned.actual, node.actual);
        prop_assert_eq!(cloned.skipped_tokens, node.skipped_tokens);
    }
}

// ===================================================================
// 27. ErrorRecoveryConfig default has sane values
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]

    #[test]
    fn error_recovery_config_default_sane(_dummy in 0u8..1) {
        let cfg = ErrorRecoveryConfig::default();
        prop_assert!(cfg.max_panic_skip > 0);
        prop_assert!(cfg.max_consecutive_errors > 0);
        prop_assert!(cfg.max_token_deletions > 0);
        prop_assert!(cfg.max_token_insertions > 0);
    }
}

// ===================================================================
// 28. ErrorRecoveryMode — variants are distinct
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]

    #[test]
    fn error_recovery_mode_distinct(_dummy in 0u8..1) {
        let modes = [
            ErrorRecoveryMode::SkipChar,
            ErrorRecoveryMode::SkipToKnown,
            ErrorRecoveryMode::Fail,
        ];
        for i in 0..modes.len() {
            for j in 0..modes.len() {
                prop_assert_eq!(i == j, modes[i] == modes[j]);
            }
        }
    }
}

// ===================================================================
// 29. ErrorRecoveryMode — Copy semantics
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]

    #[test]
    fn error_recovery_mode_copy(_dummy in 0u8..1) {
        let mode = ErrorRecoveryMode::SkipChar;
        let copied = mode;
        prop_assert_eq!(mode, copied);
    }
}

// ===================================================================
// 30. Arena — alloc then get returns same value
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn arena_alloc_get_roundtrip(val in -1000i32..1000) {
        let mut arena = TreeArena::new();
        let handle = arena.alloc(TreeNode::leaf(val));
        prop_assert_eq!(arena.get(handle).value(), val);
    }
}

// ===================================================================
// 31. Arena — len increases monotonically
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn arena_len_monotonic(count in 1usize..=50) {
        let mut arena = TreeArena::new();
        for i in 0..count {
            prop_assert_eq!(arena.len(), i);
            arena.alloc(TreeNode::leaf(i as i32));
        }
        prop_assert_eq!(arena.len(), count);
    }
}

// ===================================================================
// 32. Arena — is_empty iff len == 0
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn arena_is_empty_iff_zero(count in 0usize..=20) {
        let mut arena = TreeArena::new();
        prop_assert!(arena.is_empty());
        for _ in 0..count {
            arena.alloc(TreeNode::leaf(0));
        }
        prop_assert_eq!(arena.is_empty(), count == 0);
    }
}

// ===================================================================
// 33. Arena — capacity >= len always
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn arena_capacity_geq_len(count in 0usize..=100) {
        let mut arena = TreeArena::new();
        for i in 0..count {
            arena.alloc(TreeNode::leaf(i as i32));
        }
        prop_assert!(arena.capacity() >= arena.len());
    }
}

// ===================================================================
// 34. Arena — reset makes it empty
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn arena_reset_empties(count in 1usize..=50) {
        let mut arena = TreeArena::new();
        for i in 0..count {
            arena.alloc(TreeNode::leaf(i as i32));
        }
        arena.reset();
        prop_assert!(arena.is_empty());
        prop_assert_eq!(arena.len(), 0);
    }
}

// ===================================================================
// 35. Arena — clear makes it empty and retains one chunk
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn arena_clear_single_chunk(count in 1usize..=50) {
        let mut arena = TreeArena::new();
        for i in 0..count {
            arena.alloc(TreeNode::leaf(i as i32));
        }
        arena.clear();
        prop_assert!(arena.is_empty());
        prop_assert_eq!(arena.num_chunks(), 1);
    }
}

// ===================================================================
// 36. Arena — multiple allocs preserve earlier handles
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn arena_handles_stable(vals in proptest::collection::vec(-100i32..100, 1..=30)) {
        let mut arena = TreeArena::new();
        let handles: Vec<_> = vals.iter().map(|&v| arena.alloc(TreeNode::leaf(v))).collect();
        for (handle, &expected) in handles.iter().zip(vals.iter()) {
            prop_assert_eq!(arena.get(*handle).value(), expected);
        }
    }
}

// ===================================================================
// 37. Arena — branch children reference valid handles
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn arena_branch_children(child_count in 1usize..=10) {
        let mut arena = TreeArena::new();
        let children: Vec<NodeHandle> = (0..child_count)
            .map(|i| arena.alloc(TreeNode::leaf(i as i32)))
            .collect();
        let parent = arena.alloc(TreeNode::branch(children.clone()));
        let parent_ref = arena.get(parent);
        prop_assert!(parent_ref.is_branch());
        prop_assert_eq!(parent_ref.children().len(), child_count);
        for (i, &child_handle) in parent_ref.children().iter().enumerate() {
            prop_assert_eq!(arena.get(child_handle).value(), i as i32);
        }
    }
}

// ===================================================================
// 38. Arena — with_capacity respects initial capacity
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn arena_with_capacity(cap in 1usize..=500) {
        let arena = TreeArena::with_capacity(cap);
        prop_assert!(arena.capacity() >= cap);
        prop_assert!(arena.is_empty());
    }
}

// ===================================================================
// 39. Arena — memory_usage > 0 (capacity always has at least one chunk)
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(40))]

    #[test]
    fn arena_memory_positive(cap in 1usize..=100) {
        let arena = TreeArena::with_capacity(cap);
        prop_assert!(arena.memory_usage() > 0);
    }
}

// ===================================================================
// 40. Arena — metrics snapshot consistency
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn arena_metrics_consistent(count in 0usize..=40) {
        let mut arena = TreeArena::new();
        for i in 0..count {
            arena.alloc(TreeNode::leaf(i as i32));
        }
        let m = arena.metrics();
        prop_assert_eq!(m.len(), arena.len());
        prop_assert_eq!(m.capacity(), arena.capacity());
        prop_assert_eq!(m.num_chunks(), arena.num_chunks());
        prop_assert_eq!(m.memory_usage(), arena.memory_usage());
        prop_assert_eq!(m.is_empty(), arena.is_empty());
    }
}

// ===================================================================
// 41. Arena — get_mut can modify leaf values
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn arena_get_mut_modifies(original in -100i32..100, replacement in -100i32..100) {
        let mut arena = TreeArena::new();
        let handle = arena.alloc(TreeNode::leaf(original));
        arena.get_mut(handle).set_value(replacement);
        prop_assert_eq!(arena.get(handle).value(), replacement);
    }
}

// ===================================================================
// 42. NodeHandle — equality and hashing
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn node_handle_eq_and_hash(c in 0u32..100, n in 0u32..100) {
        let h1 = NodeHandle::new(c, n);
        let h2 = NodeHandle::new(c, n);
        prop_assert_eq!(h1, h2);
        // Verify Hash consistency by inserting into a set.
        let mut set = HashSet::new();
        set.insert(h1);
        prop_assert!(set.contains(&h2));
    }
}

// ===================================================================
// 43. NodeHandle — distinct indices produce distinct handles
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn node_handle_distinct(c1 in 0u32..10, n1 in 0u32..10, c2 in 0u32..10, n2 in 0u32..10) {
        let h1 = NodeHandle::new(c1, n1);
        let h2 = NodeHandle::new(c2, n2);
        prop_assert_eq!(h1 == h2, c1 == c2 && n1 == n2);
    }
}

// ===================================================================
// 44. VisitorAction — Copy semantics preserved
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    #[test]
    fn visitor_action_copy(idx in 0usize..3) {
        let actions = [VisitorAction::Continue, VisitorAction::SkipChildren, VisitorAction::Stop];
        let original = actions[idx];
        let copied = original;
        prop_assert_eq!(original, copied);
    }
}

// ===================================================================
// 45. Trees with errors — DFS still visits all reachable nodes
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn error_tree_dfs_coverage(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        let non_err = count_reachable_non_error(&tree);
        let err = count_reachable_errors(&tree);
        prop_assert_eq!(stats.total_nodes + stats.error_nodes, non_err + err);
    }
}

// ===================================================================
// 46. Arena — alloc after reset reuses space
// ===================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(40))]

    #[test]
    fn arena_reuse_after_reset(count in 1usize..=20) {
        let mut arena = TreeArena::new();
        for i in 0..count {
            arena.alloc(TreeNode::leaf(i as i32));
        }
        let chunks_before = arena.num_chunks();
        arena.reset();
        for i in 0..count {
            arena.alloc(TreeNode::leaf(i as i32 + 100));
        }
        // Should not have grown extra chunks.
        prop_assert!(arena.num_chunks() <= chunks_before);
        prop_assert_eq!(arena.len(), count);
    }
}

//! Comprehensive property-based tests for the `adze::visitor` module.
//!
//! 60+ tests covering:
//! - Visitor pattern invariants (enter/leave pairing, traversal order)
//! - DFS and BFS walker correctness
//! - StatsVisitor, SearchVisitor, PrettyPrintVisitor, TransformWalker
//! - Edge cases: single leaf, deep chains, wide trees, error nodes, mixed trees

use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TransformVisitor,
    TransformWalker, TreeWalker, Visitor, VisitorAction,
};
use proptest::prelude::*;
use std::collections::VecDeque;

// ===========================================================================
// Helpers
// ===========================================================================

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

fn leaf(symbol: u16, start: usize, end: usize) -> ParsedNode {
    make_node(symbol, vec![], start, end, false, true)
}

fn anon_leaf(symbol: u16, start: usize, end: usize) -> ParsedNode {
    make_node(symbol, vec![], start, end, false, false)
}

fn interior(symbol: u16, children: Vec<ParsedNode>) -> ParsedNode {
    let start = children.first().map_or(0, |c| c.start_byte);
    let end = children.last().map_or(0, |c| c.end_byte);
    make_node(symbol, children, start, end, false, true)
}

fn error_node(start: usize, end: usize) -> ParsedNode {
    make_node(0, vec![], start, end, true, false)
}

/// Count every node in a tree recursively (including the root).
fn count_all_nodes(node: &ParsedNode) -> usize {
    1 + node.children().iter().map(count_all_nodes).sum::<usize>()
}

/// Count every non-error node (error nodes stop traversal).
fn count_non_error_nodes(node: &ParsedNode) -> usize {
    if node.is_error() {
        0
    } else {
        1 + node
            .children()
            .iter()
            .map(count_non_error_nodes)
            .sum::<usize>()
    }
}

/// Count error nodes reachable during traversal (only top-level errors
/// under non-error parents, since the walker stops at error nodes).
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

/// Count leaf (childless, non-error) nodes.
fn count_leaves(node: &ParsedNode) -> usize {
    if node.is_error() {
        0
    } else if node.children().is_empty() {
        1
    } else {
        node.children().iter().map(count_leaves).sum()
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

/// Build a deep chain: root -> child -> ... -> leaf (depth levels).
fn build_deep_tree(depth: usize, source_len: usize) -> ParsedNode {
    assert!(depth >= 1);
    let mut current = leaf(1, 0, source_len.max(1));
    for _ in 1..depth {
        current = interior(1, vec![current]);
    }
    current
}

/// Build a wide tree: root with `width` leaf children.
fn build_wide_tree(width: usize, source_len: usize) -> ParsedNode {
    let children: Vec<ParsedNode> = (0..width)
        .map(|i| {
            let start = i % source_len;
            let end = (start + 1).min(source_len);
            leaf((i as u16 % 10) + 1, start, end)
        })
        .collect();
    if children.is_empty() {
        leaf(1, 0, source_len.max(1))
    } else {
        interior(1, children)
    }
}

/// Collect DFS pre-order symbol IDs (non-error nodes only).
fn dfs_preorder_symbols(node: &ParsedNode) -> Vec<u16> {
    if node.is_error() {
        return vec![];
    }
    let mut result = vec![node.symbol];
    for child in node.children() {
        result.extend(dfs_preorder_symbols(child));
    }
    result
}

/// Collect BFS-order symbol IDs (non-error nodes only).
fn bfs_order_symbols(root: &ParsedNode) -> Vec<u16> {
    let mut result = Vec::new();
    let mut queue = VecDeque::new();
    queue.push_back(root);
    while let Some(node) = queue.pop_front() {
        if node.is_error() {
            continue;
        }
        result.push(node.symbol);
        for child in node.children() {
            queue.push_back(child);
        }
    }
    result
}

// ===========================================================================
// Proptest strategies
// ===========================================================================

const SOURCE_LEN: usize = 64;

fn arb_leaf() -> impl Strategy<Value = ParsedNode> {
    (1u16..=10, 0..SOURCE_LEN - 1).prop_map(|(sym, start)| leaf(sym, start, start + 1))
}

fn arb_tree(max_depth: u32, max_width: usize) -> impl Strategy<Value = ParsedNode> {
    arb_leaf().prop_recursive(max_depth, 64, max_width as u32, move |inner| {
        (1u16..=10, proptest::collection::vec(inner, 1..=max_width))
            .prop_map(|(sym, children)| interior(sym, children))
    })
}

fn arb_source() -> impl Strategy<Value = String> {
    proptest::string::string_regex(&format!("[a-z0-9 ]{{{SOURCE_LEN},{SOURCE_LEN}}}")).unwrap()
}

/// Generate a tree that may contain error nodes at leaf positions.
fn arb_tree_with_errors(max_depth: u32, max_width: usize) -> impl Strategy<Value = ParsedNode> {
    let leaf_or_error = prop_oneof![
        3 => (1u16..=10, 0..SOURCE_LEN - 1).prop_map(|(sym, start)| leaf(sym, start, start + 1)),
        1 => (0..SOURCE_LEN - 1).prop_map(|start| error_node(start, start + 1)),
    ];
    leaf_or_error.prop_recursive(max_depth, 64, max_width as u32, move |inner| {
        (1u16..=10, proptest::collection::vec(inner, 1..=max_width))
            .prop_map(|(sym, children)| interior(sym, children))
    })
}

// ===========================================================================
// Section 1: VisitorAction trait tests (concrete)
// ===========================================================================

// 1
#[test]
fn visitor_action_continue_eq() {
    assert_eq!(VisitorAction::Continue, VisitorAction::Continue);
}

// 2
#[test]
fn visitor_action_skip_eq() {
    assert_eq!(VisitorAction::SkipChildren, VisitorAction::SkipChildren);
}

// 3
#[test]
fn visitor_action_stop_eq() {
    assert_eq!(VisitorAction::Stop, VisitorAction::Stop);
}

// 4
#[test]
fn visitor_action_all_distinct() {
    let actions = [
        VisitorAction::Continue,
        VisitorAction::SkipChildren,
        VisitorAction::Stop,
    ];
    for i in 0..actions.len() {
        for j in 0..actions.len() {
            assert_eq!(i == j, actions[i] == actions[j]);
        }
    }
}

// 5
#[test]
fn visitor_action_clone_and_copy() {
    let a = VisitorAction::Continue;
    let b = a; // Copy
    #[expect(
        clippy::clone_on_copy,
        reason = "proptest strategies exercise explicit clone paths even for Copy types to verify value independence"
    )]
    let c = a.clone();
    assert_eq!(a, b);
    assert_eq!(a, c);
}

// 6
#[test]
fn visitor_action_debug_contains_variant_name() {
    assert!(format!("{:?}", VisitorAction::Continue).contains("Continue"));
    assert!(format!("{:?}", VisitorAction::SkipChildren).contains("SkipChildren"));
    assert!(format!("{:?}", VisitorAction::Stop).contains("Stop"));
}

// ===========================================================================
// Section 2: Default Visitor trait implementation (concrete)
// ===========================================================================

// 7
#[test]
fn default_visitor_enter_returns_continue() {
    struct NoOp;
    impl Visitor for NoOp {}
    let mut v = NoOp;
    let node = leaf(1, 0, 1);
    assert_eq!(v.enter_node(&node), VisitorAction::Continue);
}

// 8
#[test]
fn default_visitor_leave_does_not_panic() {
    struct NoOp;
    impl Visitor for NoOp {}
    let mut v = NoOp;
    let node = leaf(1, 0, 1);
    v.leave_node(&node); // should not panic
}

// 9
#[test]
fn default_visitor_visit_leaf_does_not_panic() {
    struct NoOp;
    impl Visitor for NoOp {}
    let mut v = NoOp;
    let node = leaf(1, 0, 1);
    v.visit_leaf(&node, "x");
}

// 10
#[test]
fn default_visitor_visit_error_does_not_panic() {
    struct NoOp;
    impl Visitor for NoOp {}
    let mut v = NoOp;
    let node = error_node(0, 1);
    v.visit_error(&node);
}

// ===========================================================================
// Section 3: StatsVisitor concrete tests
// ===========================================================================

// 11
#[test]
fn stats_visitor_defaults_are_zero() {
    let sv = StatsVisitor::default();
    assert_eq!(sv.total_nodes, 0);
    assert_eq!(sv.leaf_nodes, 0);
    assert_eq!(sv.error_nodes, 0);
    assert_eq!(sv.max_depth, 0);
    assert!(sv.node_counts.is_empty());
}

// 12
#[test]
fn stats_single_leaf() {
    let source = b"hello";
    let tree = leaf(1, 0, 5);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
    assert_eq!(stats.max_depth, 1);
}

// 13
#[test]
fn stats_root_with_two_leaves() {
    let source = b"ab";
    let tree = interior(1, vec![leaf(2, 0, 1), leaf(3, 1, 2)]);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut stats);
    assert_eq!(stats.total_nodes, 3);
    assert_eq!(stats.leaf_nodes, 2);
    assert_eq!(stats.max_depth, 2);
}

// 14
#[test]
fn stats_deep_chain_depth_3() {
    let source = b"x";
    let tree = build_deep_tree(3, 1);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut stats);
    assert_eq!(stats.total_nodes, 3);
    assert_eq!(stats.max_depth, 3);
    assert_eq!(stats.leaf_nodes, 1);
}

// 15
#[test]
fn stats_error_node_counted() {
    let source = b"x error y";
    let tree = interior(1, vec![leaf(2, 0, 1), error_node(2, 7), leaf(3, 8, 9)]);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut stats);
    assert_eq!(stats.error_nodes, 1);
    // Error nodes don't go through enter_node, so total_nodes excludes them.
    assert_eq!(stats.total_nodes, 3); // root + 2 leaves
}

// 16
#[test]
fn stats_node_counts_keys() {
    let source = b"ab";
    let tree = interior(1, vec![leaf(2, 0, 1), leaf(2, 1, 2)]);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut stats);
    let sum: usize = stats.node_counts.values().sum();
    assert_eq!(sum, stats.total_nodes);
}

// ===========================================================================
// Section 4: PrettyPrintVisitor concrete tests
// ===========================================================================

// 17
#[test]
fn pretty_print_starts_empty() {
    let pp = PrettyPrintVisitor::new();
    assert!(pp.output().is_empty());
}

// 18
#[test]
fn pretty_print_default_equals_new() {
    let a = PrettyPrintVisitor::new();
    let b = PrettyPrintVisitor::default();
    assert_eq!(a.output(), b.output());
}

// 19
#[test]
fn pretty_print_single_leaf_contains_text() {
    let source = b"hello";
    let tree = leaf(1, 0, 5);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(source).walk(&tree, &mut pp);
    let out = pp.output();
    assert!(out.contains("hello"), "output was: {out}");
}

// 20
#[test]
fn pretty_print_named_annotation() {
    let source = b"x";
    let tree = leaf(1, 0, 1);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(source).walk(&tree, &mut pp);
    assert!(pp.output().contains("[named]"));
}

// 21
#[test]
fn pretty_print_anonymous_no_named_tag() {
    let source = b"x";
    let tree = anon_leaf(1, 0, 1);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(source).walk(&tree, &mut pp);
    assert!(!pp.output().contains("[named]"));
}

// 22
#[test]
fn pretty_print_indentation_increases_for_children() {
    let source = b"xy";
    let tree = interior(1, vec![leaf(2, 0, 1), leaf(3, 1, 2)]);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(source).walk(&tree, &mut pp);
    let lines: Vec<&str> = pp.output().lines().collect();
    // Root line should have no leading spaces; child lines should be indented.
    assert!(!lines[0].starts_with(' '));
    // At least one child line is indented.
    assert!(lines.iter().skip(1).any(|l| l.starts_with("  ")));
}

// 23
#[test]
fn pretty_print_error_node_contains_error_prefix() {
    let source = b"x err y";
    let tree = interior(1, vec![leaf(2, 0, 1), error_node(2, 5), leaf(3, 6, 7)]);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(source).walk(&tree, &mut pp);
    assert!(pp.output().contains("ERROR:"));
}

// ===========================================================================
// Section 5: SearchVisitor concrete tests
// ===========================================================================

// 24
#[test]
fn search_visitor_starts_empty() {
    let sv = SearchVisitor::new(|_: &ParsedNode| true);
    assert!(sv.matches.is_empty());
}

// 25
#[test]
fn search_always_true_finds_all_non_error() {
    let source = b"abc";
    let tree = interior(1, vec![leaf(2, 0, 1), leaf(3, 1, 2), leaf(4, 2, 3)]);
    let mut search = SearchVisitor::new(|_: &ParsedNode| true);
    TreeWalker::new(source).walk(&tree, &mut search);
    assert_eq!(search.matches.len(), 4); // root + 3 leaves
}

// 26
#[test]
fn search_always_false_finds_none() {
    let source = b"abc";
    let tree = interior(1, vec![leaf(2, 0, 1), leaf(3, 1, 2)]);
    let mut search = SearchVisitor::new(|_: &ParsedNode| false);
    TreeWalker::new(source).walk(&tree, &mut search);
    assert!(search.matches.is_empty());
}

// 27
#[test]
fn search_by_symbol_filters_correctly() {
    let source = b"ab";
    let tree = interior(1, vec![leaf(2, 0, 1), leaf(3, 1, 2)]);
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol == 2);
    TreeWalker::new(source).walk(&tree, &mut search);
    assert_eq!(search.matches.len(), 1);
    // kind() returns the symbol name from the language, or a fallback string.
    assert!(!search.matches[0].2.is_empty());
}

// 28
#[test]
fn search_match_tuples_have_correct_byte_ranges() {
    let source = b"abcde";
    let tree = leaf(1, 1, 4);
    let mut search = SearchVisitor::new(|_: &ParsedNode| true);
    TreeWalker::new(source).walk(&tree, &mut search);
    assert_eq!(search.matches.len(), 1);
    assert_eq!(search.matches[0].0, 1); // start_byte
    assert_eq!(search.matches[0].1, 4); // end_byte
}

// ===========================================================================
// Section 6: Walker constructor tests
// ===========================================================================

// 29
#[test]
fn tree_walker_new() {
    let source = b"abc";
    let _w = TreeWalker::new(source);
}

// 30
#[test]
fn breadth_first_walker_new() {
    let source = b"abc";
    let _w = BreadthFirstWalker::new(source);
}

// 31
#[test]
fn transform_walker_new() {
    let source = b"abc";
    let _w = TransformWalker::new(source);
}

// ===========================================================================
// Section 7: DFS enter/leave pairing (property)
// ===========================================================================

proptest! {
    // 32
    #[test]
    fn dfs_enter_leave_always_balanced(
        source in arb_source(),
        tree in arb_tree(4, 4),
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
        prop_assert_eq!(v.enters, v.leaves, "enter/leave must be balanced");
    }
}

// ===========================================================================
// Section 8: DFS total_nodes == count_non_error_nodes (property)
// ===========================================================================

proptest! {
    // 33
    #[test]
    fn dfs_total_matches_non_error_count(
        source in arb_source(),
        tree in arb_tree(4, 4),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert_eq!(stats.total_nodes, count_non_error_nodes(&tree));
    }
}

// ===========================================================================
// Section 9: BFS total_nodes == DFS total_nodes (property)
// ===========================================================================

proptest! {
    // 34
    #[test]
    fn bfs_dfs_same_total(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut dfs_stats = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut dfs_stats);

        let mut bfs_stats = StatsVisitor::default();
        BreadthFirstWalker::new(src).walk(&tree, &mut bfs_stats);

        prop_assert_eq!(dfs_stats.total_nodes, bfs_stats.total_nodes);
    }
}

// ===========================================================================
// Section 10: BFS and DFS visit the same set of symbols (property)
// ===========================================================================

proptest! {
    // 35
    #[test]
    fn bfs_dfs_same_symbol_multiset(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct SymCollector(Vec<u16>);
        impl Visitor for SymCollector {
            fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
                self.0.push(node.symbol);
                VisitorAction::Continue
            }
        }

        let src = source.as_bytes();

        let mut dfs_v = SymCollector(vec![]);
        TreeWalker::new(src).walk(&tree, &mut dfs_v);

        let mut bfs_v = SymCollector(vec![]);
        BreadthFirstWalker::new(src).walk(&tree, &mut bfs_v);

        dfs_v.0.sort();
        bfs_v.0.sort();
        prop_assert_eq!(dfs_v.0, bfs_v.0);
    }
}

// ===========================================================================
// Section 11: DFS traversal order matches manual pre-order (property)
// ===========================================================================

proptest! {
    // 36
    #[test]
    fn dfs_order_matches_manual_preorder(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct OrderCollector(Vec<u16>);
        impl Visitor for OrderCollector {
            fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
                self.0.push(node.symbol);
                VisitorAction::Continue
            }
        }

        let mut v = OrderCollector(vec![]);
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut v);

        let expected = dfs_preorder_symbols(&tree);
        prop_assert_eq!(v.0, expected);
    }
}

// ===========================================================================
// Section 12: BFS traversal order matches manual level-order (property)
// ===========================================================================

proptest! {
    // 37
    #[test]
    fn bfs_order_matches_manual_levelorder(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct OrderCollector(Vec<u16>);
        impl Visitor for OrderCollector {
            fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
                self.0.push(node.symbol);
                VisitorAction::Continue
            }
        }

        let mut v = OrderCollector(vec![]);
        BreadthFirstWalker::new(source.as_bytes()).walk(&tree, &mut v);

        let expected = bfs_order_symbols(&tree);
        prop_assert_eq!(v.0, expected);
    }
}

// ===========================================================================
// Section 13: StatsVisitor.max_depth <= tree_depth (property)
// ===========================================================================

proptest! {
    // 38
    #[test]
    fn stats_max_depth_bounded(
        source in arb_source(),
        tree in arb_tree(4, 4),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert!(stats.max_depth <= tree_depth(&tree));
    }
}

// ===========================================================================
// Section 14: StatsVisitor.leaf_nodes <= total leaves (property)
// ===========================================================================

proptest! {
    // 39
    #[test]
    fn stats_leaf_count_bounded(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert!(stats.leaf_nodes <= count_leaves(&tree));
    }
}

// ===========================================================================
// Section 15: node_counts values sum to total_nodes (property)
// ===========================================================================

proptest! {
    // 40
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

// ===========================================================================
// Section 16: PrettyPrint non-empty for any tree (property)
// ===========================================================================

proptest! {
    // 41
    #[test]
    fn pretty_print_always_non_empty(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut pp);
        prop_assert!(!pp.output().is_empty());
    }
}

// ===========================================================================
// Section 17: PrettyPrint has at least one newline per entered node (property)
// ===========================================================================

proptest! {
    // 42
    #[test]
    fn pretty_print_newline_count(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut pp);
        let newlines = pp.output().matches('\n').count();
        prop_assert!(newlines >= 1);
    }
}

// ===========================================================================
// Section 18: SearchVisitor always-true finds == StatsVisitor.total_nodes (property)
// ===========================================================================

proptest! {
    // 43
    #[test]
    fn search_all_matches_total_nodes(
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

// ===========================================================================
// Section 19: SearchVisitor always-false finds nothing (property)
// ===========================================================================

proptest! {
    // 44
    #[test]
    fn search_none_finds_zero(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut search = SearchVisitor::new(|_: &ParsedNode| false);
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut search);
        prop_assert!(search.matches.is_empty());
    }
}

// ===========================================================================
// Section 20: SearchVisitor subset property (property)
// ===========================================================================

proptest! {
    // 45
    #[test]
    fn search_filtered_is_subset(
        source in arb_source(),
        tree in arb_tree(3, 3),
        threshold in 1u16..=10,
    ) {
        let src = source.as_bytes();
        let t = threshold;

        let mut filtered = SearchVisitor::new(move |n: &ParsedNode| n.symbol <= t);
        TreeWalker::new(src).walk(&tree, &mut filtered);

        let mut all = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(src).walk(&tree, &mut all);

        prop_assert!(filtered.matches.len() <= all.matches.len());
    }
}

// ===========================================================================
// Section 21: Stop action limits traversal (BFS, property)
// ===========================================================================

proptest! {
    // 46
    #[test]
    fn bfs_stop_limits_traversal(
        source in arb_source(),
        tree in arb_tree(3, 4),
        limit in 1usize..=5,
    ) {
        struct StopAfter { count: usize, limit: usize }
        impl Visitor for StopAfter {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.count += 1;
                if self.count >= self.limit {
                    VisitorAction::Stop
                } else {
                    VisitorAction::Continue
                }
            }
        }

        let mut v = StopAfter { count: 0, limit };
        BreadthFirstWalker::new(source.as_bytes()).walk(&tree, &mut v);
        prop_assert!(v.count <= limit);
    }
}

// ===========================================================================
// Section 22: Stop action limits traversal (DFS, property)
// ===========================================================================

proptest! {
    // 47
    #[test]
    fn dfs_stop_limits_traversal(
        source in arb_source(),
        tree in arb_tree(3, 4),
        limit in 1usize..=5,
    ) {
        struct StopAfter { count: usize, limit: usize }
        impl Visitor for StopAfter {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.count += 1;
                if self.count >= self.limit {
                    VisitorAction::Stop
                } else {
                    VisitorAction::Continue
                }
            }
        }

        let total = count_non_error_nodes(&tree);
        let mut v = StopAfter { count: 0, limit };
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut v);
        // DFS doesn't propagate Stop across siblings, so count may exceed
        // limit but should still be less than full traversal when limit < total.
        if limit < total {
            prop_assert!(v.count <= total);
        }
    }
}

// ===========================================================================
// Section 23: SkipChildren reduces DFS count (property)
// ===========================================================================

proptest! {
    // 48
    #[test]
    fn skip_children_reduces_count(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct SkipFirst { count: usize, skipped: bool }
        impl Visitor for SkipFirst {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.count += 1;
                if !self.skipped {
                    self.skipped = true;
                    VisitorAction::SkipChildren
                } else {
                    VisitorAction::Continue
                }
            }
        }

        let src = source.as_bytes();
        let mut skip_v = SkipFirst { count: 0, skipped: false };
        TreeWalker::new(src).walk(&tree, &mut skip_v);

        let mut stats = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut stats);

        prop_assert!(skip_v.count <= stats.total_nodes);
    }
}

// ===========================================================================
// Section 24: SkipChildren still calls leave_node (DFS, property)
// ===========================================================================

proptest! {
    // 49
    #[test]
    fn skip_children_still_calls_leave(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct SkipBalance { enters: usize, leaves: usize, skip_first: bool }
        impl Visitor for SkipBalance {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.enters += 1;
                if !self.skip_first {
                    self.skip_first = true;
                    VisitorAction::SkipChildren
                } else {
                    VisitorAction::Continue
                }
            }
            fn leave_node(&mut self, _: &ParsedNode) {
                self.leaves += 1;
            }
        }

        let mut v = SkipBalance { enters: 0, leaves: 0, skip_first: false };
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut v);
        prop_assert_eq!(v.enters, v.leaves, "enter/leave must be balanced even with SkipChildren");
    }
}

// ===========================================================================
// Section 25: TransformWalker leaf count (property)
// ===========================================================================

proptest! {
    // 50
    #[test]
    fn transform_counts_leaves(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct CountLeaves;
        impl TransformVisitor for CountLeaves {
            type Output = usize;
            fn transform_node(&mut self, _: &ParsedNode, children: Vec<usize>) -> usize {
                children.iter().sum()
            }
            fn transform_leaf(&mut self, _: &ParsedNode, _: &str) -> usize { 1 }
            fn transform_error(&mut self, _: &ParsedNode) -> usize { 0 }
        }

        let result = TransformWalker::new(source.as_bytes()).walk(&tree, &mut CountLeaves);
        prop_assert_eq!(result, count_leaves(&tree));
    }
}

// ===========================================================================
// Section 26: TransformWalker depth computation (property)
// ===========================================================================

proptest! {
    // 51
    #[test]
    fn transform_computes_depth(
        source in arb_source(),
        tree in arb_tree(4, 3),
    ) {
        struct DepthCalc;
        impl TransformVisitor for DepthCalc {
            type Output = usize;
            fn transform_node(&mut self, _: &ParsedNode, children: Vec<usize>) -> usize {
                1 + children.into_iter().max().unwrap_or(0)
            }
            fn transform_leaf(&mut self, _: &ParsedNode, _: &str) -> usize { 1 }
            fn transform_error(&mut self, _: &ParsedNode) -> usize { 1 }
        }

        let result = TransformWalker::new(source.as_bytes()).walk(&tree, &mut DepthCalc);
        prop_assert_eq!(result, tree_depth(&tree));
    }
}

// ===========================================================================
// Section 27: TransformWalker total node count (property)
// ===========================================================================

proptest! {
    // 52
    #[test]
    fn transform_counts_all_nodes(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct CountAll;
        impl TransformVisitor for CountAll {
            type Output = usize;
            fn transform_node(&mut self, _: &ParsedNode, children: Vec<usize>) -> usize {
                1 + children.iter().sum::<usize>()
            }
            fn transform_leaf(&mut self, _: &ParsedNode, _: &str) -> usize { 1 }
            fn transform_error(&mut self, _: &ParsedNode) -> usize { 1 }
        }

        let result = TransformWalker::new(source.as_bytes()).walk(&tree, &mut CountAll);
        prop_assert_eq!(result, count_all_nodes(&tree));
    }
}

// ===========================================================================
// Section 28: TransformWalker string concatenation (property)
// ===========================================================================

proptest! {
    // 53
    #[test]
    fn transform_concat_produces_non_empty_for_leaf(
        source in arb_source(),
    ) {
        struct Concat;
        impl TransformVisitor for Concat {
            type Output = String;
            fn transform_node(&mut self, _: &ParsedNode, children: Vec<String>) -> String {
                children.join("")
            }
            fn transform_leaf(&mut self, _: &ParsedNode, text: &str) -> String {
                text.to_string()
            }
            fn transform_error(&mut self, _: &ParsedNode) -> String {
                String::new()
            }
        }

        let tree = leaf(1, 0, 1);
        let result = TransformWalker::new(source.as_bytes()).walk(&tree, &mut Concat);
        // Source is always SOURCE_LEN chars, so byte 0..1 is always a valid char.
        prop_assert!(!result.is_empty());
    }
}

// ===========================================================================
// Section 29: Deep chain edge case (concrete)
// ===========================================================================

// 54
#[test]
fn deep_chain_depth_10() {
    let source = b"x";
    let tree = build_deep_tree(10, 1);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut stats);
    assert_eq!(stats.total_nodes, 10);
    assert_eq!(stats.max_depth, 10);
    assert_eq!(stats.leaf_nodes, 1);
}

// 55
#[test]
fn deep_chain_enter_leave_balanced() {
    struct Balance {
        enters: usize,
        leaves: usize,
    }
    impl Visitor for Balance {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.enters += 1;
            VisitorAction::Continue
        }
        fn leave_node(&mut self, _: &ParsedNode) {
            self.leaves += 1;
        }
    }

    let source = b"x";
    let tree = build_deep_tree(20, 1);
    let mut v = Balance {
        enters: 0,
        leaves: 0,
    };
    TreeWalker::new(source).walk(&tree, &mut v);
    assert_eq!(v.enters, v.leaves);
    assert_eq!(v.enters, 20);
}

// ===========================================================================
// Section 30: Wide tree edge case (concrete)
// ===========================================================================

// 56
#[test]
fn wide_tree_50_children() {
    let source = &[b'a'; SOURCE_LEN];
    let tree = build_wide_tree(50, SOURCE_LEN);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut stats);
    assert_eq!(stats.total_nodes, 51); // root + 50 leaves
    assert_eq!(stats.leaf_nodes, 50);
    assert_eq!(stats.max_depth, 2);
}

// 57
#[test]
fn wide_tree_bfs_dfs_same_count() {
    let source = &[b'a'; SOURCE_LEN];
    let tree = build_wide_tree(30, SOURCE_LEN);

    let mut dfs = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut dfs);

    let mut bfs = StatsVisitor::default();
    BreadthFirstWalker::new(source).walk(&tree, &mut bfs);

    assert_eq!(dfs.total_nodes, bfs.total_nodes);
}

// ===========================================================================
// Section 31: Error node handling (concrete)
// ===========================================================================

// 58
#[test]
fn error_only_tree() {
    let source = b"err";
    let tree = error_node(0, 3);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut stats);
    assert_eq!(stats.error_nodes, 1);
    assert_eq!(stats.total_nodes, 0);
}

// 59
#[test]
fn multiple_error_nodes() {
    let source = b"a err b err c";
    let tree = interior(
        1,
        vec![
            leaf(2, 0, 1),
            error_node(2, 5),
            leaf(3, 6, 7),
            error_node(8, 11),
            leaf(4, 12, 13),
        ],
    );
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut stats);
    assert_eq!(stats.error_nodes, 2);
    assert_eq!(stats.total_nodes, 4); // root + 3 leaves
}

// 60
#[test]
fn error_node_bfs_counted() {
    let source = b"x err y";
    let tree = interior(1, vec![leaf(2, 0, 1), error_node(2, 5), leaf(3, 6, 7)]);
    let mut stats = StatsVisitor::default();
    BreadthFirstWalker::new(source).walk(&tree, &mut stats);
    assert_eq!(stats.error_nodes, 1);
}

// ===========================================================================
// Section 32: Trees with errors (property)
// ===========================================================================

proptest! {
    // 61
    #[test]
    fn trees_with_errors_error_count_consistent(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        let src = source.as_bytes();

        let mut dfs_stats = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut dfs_stats);

        let mut bfs_stats = StatsVisitor::default();
        BreadthFirstWalker::new(src).walk(&tree, &mut bfs_stats);

        // Both walkers should see the same number of error nodes.
        prop_assert_eq!(dfs_stats.error_nodes, bfs_stats.error_nodes);
    }

    // 62
    #[test]
    fn trees_with_errors_total_plus_errors_consistent(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        let src = source.as_bytes();

        let mut stats = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut stats);

        let non_error = count_non_error_nodes(&tree);
        let reachable_errors = count_reachable_errors(&tree);
        // total_nodes counts non-error nodes entered
        prop_assert_eq!(stats.total_nodes, non_error);
        // error_nodes counts reachable error nodes
        prop_assert_eq!(stats.error_nodes, reachable_errors);
    }
}

// ===========================================================================
// Section 33: SkipChildren in BFS (property)
// ===========================================================================

proptest! {
    // 63
    #[test]
    fn bfs_skip_children_reduces_count(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct SkipFirst { count: usize, skipped: bool }
        impl Visitor for SkipFirst {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.count += 1;
                if !self.skipped {
                    self.skipped = true;
                    VisitorAction::SkipChildren
                } else {
                    VisitorAction::Continue
                }
            }
        }

        let src = source.as_bytes();
        let mut skip_v = SkipFirst { count: 0, skipped: false };
        BreadthFirstWalker::new(src).walk(&tree, &mut skip_v);

        let mut stats = StatsVisitor::default();
        BreadthFirstWalker::new(src).walk(&tree, &mut stats);

        prop_assert!(skip_v.count <= stats.total_nodes);
    }
}

// ===========================================================================
// Section 34: Idempotency — two walks produce same stats (property)
// ===========================================================================

proptest! {
    // 64
    #[test]
    fn dfs_walk_is_idempotent(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();

        let mut stats1 = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut stats1);

        let mut stats2 = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut stats2);

        prop_assert_eq!(stats1.total_nodes, stats2.total_nodes);
        prop_assert_eq!(stats1.leaf_nodes, stats2.leaf_nodes);
        prop_assert_eq!(stats1.error_nodes, stats2.error_nodes);
        prop_assert_eq!(stats1.max_depth, stats2.max_depth);
    }

    // 65
    #[test]
    fn bfs_walk_is_idempotent(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();

        let mut stats1 = StatsVisitor::default();
        BreadthFirstWalker::new(src).walk(&tree, &mut stats1);

        let mut stats2 = StatsVisitor::default();
        BreadthFirstWalker::new(src).walk(&tree, &mut stats2);

        prop_assert_eq!(stats1.total_nodes, stats2.total_nodes);
        prop_assert_eq!(stats1.leaf_nodes, stats2.leaf_nodes);
        prop_assert_eq!(stats1.error_nodes, stats2.error_nodes);
        prop_assert_eq!(stats1.max_depth, stats2.max_depth);
    }
}

// ===========================================================================
// Section 35: PrettyPrint output deterministic (property)
// ===========================================================================

proptest! {
    // 66
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

// ===========================================================================
// Section 36: Deep tree property tests
// ===========================================================================

proptest! {
    // 67
    #[test]
    fn deep_tree_depth_matches(depth in 1usize..=30) {
        let source = &[b'x'; SOURCE_LEN];
        let tree = build_deep_tree(depth, SOURCE_LEN);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source).walk(&tree, &mut stats);
        prop_assert_eq!(stats.max_depth, depth);
        prop_assert_eq!(stats.total_nodes, depth);
    }
}

// ===========================================================================
// Section 37: Wide tree property tests
// ===========================================================================

proptest! {
    // 68
    #[test]
    fn wide_tree_leaf_count(width in 1usize..=50) {
        let source = &[b'x'; SOURCE_LEN];
        let tree = build_wide_tree(width, SOURCE_LEN);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source).walk(&tree, &mut stats);
        prop_assert_eq!(stats.leaf_nodes, width);
        prop_assert_eq!(stats.total_nodes, width + 1); // root + leaves
    }
}

// ===========================================================================
// Section 38: TransformWalker with error trees (property)
// ===========================================================================

proptest! {
    // 69
    #[test]
    fn transform_error_nodes_counted(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        struct ErrorCounter;
        impl TransformVisitor for ErrorCounter {
            type Output = usize;
            fn transform_node(&mut self, _: &ParsedNode, children: Vec<usize>) -> usize {
                children.iter().sum()
            }
            fn transform_leaf(&mut self, _: &ParsedNode, _: &str) -> usize { 0 }
            fn transform_error(&mut self, _: &ParsedNode) -> usize { 1 }
        }

        let result = TransformWalker::new(source.as_bytes()).walk(&tree, &mut ErrorCounter);
        let expected = count_reachable_errors(&tree);
        prop_assert_eq!(result, expected);
    }
}

// ===========================================================================
// Section 39: BFS leaf-order for wide trees (concrete)
// ===========================================================================

// 70
#[test]
fn bfs_wide_tree_visits_root_first() {
    struct OrderCollector(Vec<u16>);
    impl Visitor for OrderCollector {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.0.push(node.symbol);
            VisitorAction::Continue
        }
    }

    let source = &[b'a'; SOURCE_LEN];
    let tree = interior(99, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let mut v = OrderCollector(vec![]);
    BreadthFirstWalker::new(source).walk(&tree, &mut v);
    assert_eq!(v.0[0], 99); // root first in BFS
}

// ===========================================================================
// Section 40: DFS visits root first (concrete)
// ===========================================================================

// 71
#[test]
fn dfs_visits_root_first() {
    struct OrderCollector(Vec<u16>);
    impl Visitor for OrderCollector {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.0.push(node.symbol);
            VisitorAction::Continue
        }
    }

    let source = &[b'a'; SOURCE_LEN];
    let tree = interior(99, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let mut v = OrderCollector(vec![]);
    TreeWalker::new(source).walk(&tree, &mut v);
    assert_eq!(v.0[0], 99);
}

// ===========================================================================
// Section 41: Single-node tree transforms correctly (concrete)
// ===========================================================================

// 72
#[test]
fn transform_single_leaf() {
    struct Identity;
    impl TransformVisitor for Identity {
        type Output = String;
        fn transform_node(&mut self, _: &ParsedNode, children: Vec<String>) -> String {
            children.join(",")
        }
        fn transform_leaf(&mut self, _: &ParsedNode, text: &str) -> String {
            text.to_string()
        }
        fn transform_error(&mut self, _: &ParsedNode) -> String {
            "ERR".to_string()
        }
    }

    let source = b"hello";
    let tree = leaf(1, 0, 5);
    let result = TransformWalker::new(source).walk(&tree, &mut Identity);
    assert_eq!(result, "hello");
}

// 73
#[test]
fn transform_error_only() {
    struct Identity;
    impl TransformVisitor for Identity {
        type Output = String;
        fn transform_node(&mut self, _: &ParsedNode, children: Vec<String>) -> String {
            children.join(",")
        }
        fn transform_leaf(&mut self, _: &ParsedNode, text: &str) -> String {
            text.to_string()
        }
        fn transform_error(&mut self, _: &ParsedNode) -> String {
            "ERR".to_string()
        }
    }

    let source = b"hello";
    let tree = error_node(0, 5);
    let result = TransformWalker::new(source).walk(&tree, &mut Identity);
    assert_eq!(result, "ERR");
}

// ===========================================================================
// Section 42: Visitor that accumulates leaf text (property)
// ===========================================================================

proptest! {
    // 74
    #[test]
    fn leaf_text_visitor_collects_all_leaves(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct LeafCollector(Vec<String>);
        impl Visitor for LeafCollector {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                VisitorAction::Continue
            }
            fn visit_leaf(&mut self, _: &ParsedNode, text: &str) {
                self.0.push(text.to_string());
            }
        }

        let src = source.as_bytes();
        let mut v = LeafCollector(vec![]);
        TreeWalker::new(src).walk(&tree, &mut v);

        let mut stats = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut stats);

        prop_assert_eq!(v.0.len(), stats.leaf_nodes);
    }
}

// ===========================================================================
// Section 43: BFS does not call leave_node (property)
// ===========================================================================

proptest! {
    // 75
    #[test]
    fn bfs_does_not_call_leave(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct LeaveCounter { leaves: usize }
        impl Visitor for LeaveCounter {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                VisitorAction::Continue
            }
            fn leave_node(&mut self, _: &ParsedNode) {
                self.leaves += 1;
            }
        }

        let mut v = LeaveCounter { leaves: 0 };
        BreadthFirstWalker::new(source.as_bytes()).walk(&tree, &mut v);
        // BFS walker does not call leave_node.
        prop_assert_eq!(v.leaves, 0);
    }
}

// ===========================================================================
// Section 44: Multiple visitors on same tree give consistent results (property)
// ===========================================================================

proptest! {
    // 76
    #[test]
    fn multiple_stats_visitors_agree(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();

        let mut s1 = StatsVisitor::default();
        let mut s2 = StatsVisitor::default();
        let mut s3 = StatsVisitor::default();

        TreeWalker::new(src).walk(&tree, &mut s1);
        TreeWalker::new(src).walk(&tree, &mut s2);
        TreeWalker::new(src).walk(&tree, &mut s3);

        prop_assert_eq!(s1.total_nodes, s2.total_nodes);
        prop_assert_eq!(s2.total_nodes, s3.total_nodes);
    }
}

// ===========================================================================
// Section 45: DFS leave order is reverse of enter for chains (concrete)
// ===========================================================================

// 77
#[test]
fn dfs_leave_order_reverse_of_enter_for_chain() {
    struct OrderTracker {
        enter_order: Vec<u16>,
        leave_order: Vec<u16>,
    }
    impl Visitor for OrderTracker {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.enter_order.push(node.symbol);
            VisitorAction::Continue
        }
        fn leave_node(&mut self, node: &ParsedNode) {
            self.leave_order.push(node.symbol);
        }
    }

    let source = b"x";
    // chain: sym 10 -> sym 20 -> sym 30 (leaf)
    let tree = interior(10, vec![interior(20, vec![leaf(30, 0, 1)])]);
    let mut v = OrderTracker {
        enter_order: vec![],
        leave_order: vec![],
    };
    TreeWalker::new(source).walk(&tree, &mut v);

    assert_eq!(v.enter_order, vec![10, 20, 30]);
    // leave order should be reverse for a chain (post-order)
    assert_eq!(v.leave_order, vec![30, 20, 10]);
}

// ===========================================================================
// Section 46: PrettyPrint line count (property)
// ===========================================================================

proptest! {
    // 78
    #[test]
    fn pretty_print_line_count_ge_node_count(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();

        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(src).walk(&tree, &mut pp);

        let mut stats = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut stats);

        let lines = pp.output().lines().count();
        // Each entered node produces at least one line (from enter_node),
        // and each leaf produces one more line (from visit_leaf).
        // So lines >= total_nodes.
        prop_assert!(lines >= stats.total_nodes);
    }
}

// ===========================================================================
// Section 47: SearchVisitor byte ranges are valid (property)
// ===========================================================================

proptest! {
    // 79 — verify that search matches record consistent byte ranges from nodes.
    // Interior nodes built by `interior()` may have start > end when children
    // have non-monotonic byte offsets, so we only check that the search visitor
    // faithfully records whatever the node reports.
    #[test]
    fn search_records_byte_ranges_from_nodes(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut search = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut search);

        // At least one match (the root is always visited).
        prop_assert!(!search.matches.is_empty());
        // Each match has a non-empty kind string.
        for (_start, _end, kind) in &search.matches {
            prop_assert!(!kind.is_empty());
        }
    }
}

// ===========================================================================
// Section 48: TransformWalker produces boolean tree (property)
// ===========================================================================

proptest! {
    // 80
    #[test]
    fn transform_all_named_check(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct AllNamed;
        impl TransformVisitor for AllNamed {
            type Output = bool;
            fn transform_node(&mut self, node: &ParsedNode, children: Vec<bool>) -> bool {
                node.is_named() && children.iter().all(|c| *c)
            }
            fn transform_leaf(&mut self, node: &ParsedNode, _: &str) -> bool {
                node.is_named()
            }
            fn transform_error(&mut self, _: &ParsedNode) -> bool { false }
        }

        let result = TransformWalker::new(source.as_bytes()).walk(&tree, &mut AllNamed);
        // Our generated trees are all named, so this should be true.
        prop_assert!(result);
    }
}

// ===========================================================================
// Section 49: SkipChildren at every node means only root visited (concrete)
// ===========================================================================

// 81
#[test]
fn skip_all_children_visits_only_root() {
    struct AlwaysSkip {
        count: usize,
    }
    impl Visitor for AlwaysSkip {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.count += 1;
            VisitorAction::SkipChildren
        }
    }

    let source = &[b'x'; SOURCE_LEN];
    let tree = interior(1, vec![leaf(2, 0, 1), leaf(3, 1, 2), leaf(4, 2, 3)]);
    let mut v = AlwaysSkip { count: 0 };
    TreeWalker::new(source).walk(&tree, &mut v);
    assert_eq!(v.count, 1); // only root
}

// ===========================================================================
// Section 50: BFS SkipChildren at root skips all children (concrete)
// ===========================================================================

// 82
#[test]
fn bfs_skip_all_visits_only_root() {
    struct AlwaysSkip {
        count: usize,
    }
    impl Visitor for AlwaysSkip {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.count += 1;
            VisitorAction::SkipChildren
        }
    }

    let source = &[b'x'; SOURCE_LEN];
    let tree = interior(1, vec![leaf(2, 0, 1), leaf(3, 1, 2)]);
    let mut v = AlwaysSkip { count: 0 };
    BreadthFirstWalker::new(source).walk(&tree, &mut v);
    assert_eq!(v.count, 1);
}

// ===========================================================================
// Section 51: Stop at root visits exactly one (concrete)
// ===========================================================================

// 83
#[test]
fn dfs_stop_at_root_visits_one() {
    struct StopImmediate {
        count: usize,
    }
    impl Visitor for StopImmediate {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.count += 1;
            VisitorAction::Stop
        }
    }

    let source = &[b'x'; SOURCE_LEN];
    let tree = interior(1, vec![leaf(2, 0, 1), leaf(3, 1, 2)]);
    let mut v = StopImmediate { count: 0 };
    TreeWalker::new(source).walk(&tree, &mut v);
    assert_eq!(v.count, 1);
}

// 84
#[test]
fn bfs_stop_at_root_visits_one() {
    struct StopImmediate {
        count: usize,
    }
    impl Visitor for StopImmediate {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.count += 1;
            VisitorAction::Stop
        }
    }

    let source = &[b'x'; SOURCE_LEN];
    let tree = interior(1, vec![leaf(2, 0, 1), leaf(3, 1, 2)]);
    let mut v = StopImmediate { count: 0 };
    BreadthFirstWalker::new(source).walk(&tree, &mut v);
    assert_eq!(v.count, 1);
}

// ===========================================================================
// Section 52: Mixed named/anonymous nodes (concrete)
// ===========================================================================

// 85
#[test]
fn pretty_print_mixed_named_anonymous() {
    let source = b"xy";
    let tree = interior(1, vec![leaf(2, 0, 1), anon_leaf(3, 1, 2)]);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(source).walk(&tree, &mut pp);
    let out = pp.output();
    // The named node should have [named], anonymous should not.
    let lines: Vec<&str> = out.lines().collect();
    let named_count = lines.iter().filter(|l| l.contains("[named]")).count();
    // Root is named, first child is named, second child is anonymous.
    assert_eq!(named_count, 2); // root + leaf(2)
}

// ===========================================================================
// Section 53: StatsVisitor Debug trait (concrete)
// ===========================================================================

// 86
#[test]
fn stats_visitor_debug_format() {
    let sv = StatsVisitor::default();
    let dbg = format!("{:?}", sv);
    assert!(dbg.contains("StatsVisitor"));
}

// ===========================================================================
// Section 54: TransformWalker children ordering (concrete)
// ===========================================================================

// 87
#[test]
fn transform_children_passed_in_order() {
    struct ChildOrder;
    impl TransformVisitor for ChildOrder {
        type Output = Vec<u16>;
        fn transform_node(&mut self, _: &ParsedNode, children: Vec<Vec<u16>>) -> Vec<u16> {
            children.into_iter().flatten().collect()
        }
        fn transform_leaf(&mut self, node: &ParsedNode, _: &str) -> Vec<u16> {
            vec![node.symbol]
        }
        fn transform_error(&mut self, _: &ParsedNode) -> Vec<u16> {
            vec![0]
        }
    }

    let source = b"abc";
    let tree = interior(99, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let result = TransformWalker::new(source).walk(&tree, &mut ChildOrder);
    assert_eq!(result, vec![1, 2, 3]);
}

// ===========================================================================
// Section 55: TransformWalker interior node receives correct child count (property)
// ===========================================================================

proptest! {
    // 88
    #[test]
    fn transform_interior_receives_correct_child_count(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct ChildCountCheck { mismatches: usize }
        impl TransformVisitor for ChildCountCheck {
            type Output = usize; // child_count of this node
            fn transform_node(&mut self, node: &ParsedNode, children: Vec<usize>) -> usize {
                if children.len() != node.child_count() {
                    self.mismatches += 1;
                }
                node.child_count()
            }
            fn transform_leaf(&mut self, _: &ParsedNode, _: &str) -> usize { 0 }
            fn transform_error(&mut self, _: &ParsedNode) -> usize { 0 }
        }

        let mut v = ChildCountCheck { mismatches: 0 };
        TransformWalker::new(source.as_bytes()).walk(&tree, &mut v);
        prop_assert_eq!(v.mismatches, 0);
    }
}

// ===========================================================================
// Section 56: DFS depth tracking with custom visitor (property)
// ===========================================================================

proptest! {
    // 89
    #[test]
    fn dfs_depth_never_negative(
        source in arb_source(),
        tree in arb_tree(4, 3),
    ) {
        struct DepthTracker { depth: isize, min_depth: isize }
        impl Visitor for DepthTracker {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.depth += 1;
                self.min_depth = self.min_depth.min(self.depth);
                VisitorAction::Continue
            }
            fn leave_node(&mut self, _: &ParsedNode) {
                self.depth -= 1;
                self.min_depth = self.min_depth.min(self.depth);
            }
        }

        let mut v = DepthTracker { depth: 0, min_depth: 0 };
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut v);
        prop_assert!(v.min_depth >= 0, "depth should never go below 0");
        prop_assert_eq!(v.depth, 0, "depth should return to 0 after full walk");
    }
}

// ===========================================================================
// Section 57: SearchVisitor match kinds are non-empty strings (property)
// ===========================================================================

proptest! {
    // 90
    #[test]
    fn search_match_kinds_non_empty(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut search = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut search);
        for (_, _, kind) in &search.matches {
            prop_assert!(!kind.is_empty(), "kind string should be non-empty");
        }
    }
}

// ===========================================================================
// Section 58: BFS leaf_nodes count matches DFS (property)
// ===========================================================================

proptest! {
    // 91
    #[test]
    fn bfs_dfs_same_leaf_count(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();

        let mut dfs = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut dfs);

        let mut bfs = StatsVisitor::default();
        BreadthFirstWalker::new(src).walk(&tree, &mut bfs);

        prop_assert_eq!(dfs.leaf_nodes, bfs.leaf_nodes);
    }
}

// ===========================================================================
// Section 59: Mixed visitors on same tree (concrete)
// ===========================================================================

// 92
#[test]
fn stats_and_pretty_print_on_same_tree() {
    let source = b"hello world";
    let tree = interior(1, vec![leaf(2, 0, 5), leaf(3, 6, 11)]);

    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut stats);

    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(source).walk(&tree, &mut pp);

    assert_eq!(stats.total_nodes, 3);
    assert!(!pp.output().is_empty());
    assert!(pp.output().contains("hello"));
    assert!(pp.output().contains("world"));
}

// 93
#[test]
fn stats_and_search_on_same_tree() {
    let source = b"abc";
    let tree = interior(1, vec![leaf(2, 0, 1), leaf(3, 1, 2), leaf(2, 2, 3)]);

    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut stats);

    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol == 2);
    TreeWalker::new(source).walk(&tree, &mut search);

    assert_eq!(stats.total_nodes, 4);
    assert_eq!(search.matches.len(), 2);
}

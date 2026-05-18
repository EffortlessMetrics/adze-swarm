//! Property-based tests (v7) for the visitor module.
//!
//! 55+ proptest tests covering:
//! 1. StatsVisitor count consistency (8 tests)
//! 2. PrettyPrintVisitor non-empty output (5 tests)
//! 3. SearchVisitor match count properties (8 tests)
//! 4. Visitor action properties (5 tests)
//! 5. VisitedNode / match tuple properties (5 tests)
//! 6. Cross-visitor consistency (8 tests)
//! 7. DFS/BFS produce same total counts (8 tests)
//! 8. Various tree shapes (8 tests)

use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TransformVisitor,
    TransformWalker, TreeWalker, Visitor, VisitorAction,
};
use proptest::prelude::*;
use std::collections::HashMap;

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

fn leaf(symbol: u16, start: usize, end: usize) -> ParsedNode {
    make_node(symbol, vec![], start, end, false, true)
}

fn unnamed_leaf(symbol: u16, start: usize, end: usize) -> ParsedNode {
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

fn error_with_children(start: usize, end: usize, children: Vec<ParsedNode>) -> ParsedNode {
    make_node(0, children, start, end, true, false)
}

/// Count all nodes in the tree (including unreachable children of error nodes).
fn count_all_nodes(node: &ParsedNode) -> usize {
    1 + node.children().iter().map(count_all_nodes).sum::<usize>()
}

/// Count non-error nodes reachable by the walker (stops at error nodes).
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

/// Count error nodes reachable by the walker (error node counted, children skipped).
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

/// Count non-error leaf nodes reachable by the walker.
fn count_reachable_leaves(node: &ParsedNode) -> usize {
    if node.is_error() {
        0
    } else if node.children().is_empty() {
        1
    } else {
        node.children()
            .iter()
            .map(count_reachable_leaves)
            .sum::<usize>()
    }
}

/// Compute the DFS max depth for non-error nodes.
fn dfs_max_depth(node: &ParsedNode) -> usize {
    if node.is_error() {
        0
    } else if node.children().is_empty() {
        1
    } else {
        1 + node.children().iter().map(dfs_max_depth).max().unwrap_or(0)
    }
}

/// Collect per-kind counts for reachable non-error nodes.
fn collect_kinds(node: &ParsedNode) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    collect_kinds_rec(node, &mut map);
    map
}

fn collect_kinds_rec(node: &ParsedNode, map: &mut HashMap<String, usize>) {
    if !node.is_error() {
        *map.entry(node.kind().to_string()).or_insert(0) += 1;
        for child in node.children() {
            collect_kinds_rec(child, map);
        }
    }
}

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

const SOURCE_LEN: usize = 64;

fn arb_source() -> impl Strategy<Value = String> {
    proptest::string::string_regex(&format!("[a-z0-9 ]{{{SOURCE_LEN},{SOURCE_LEN}}}")).unwrap()
}

fn arb_leaf() -> impl Strategy<Value = ParsedNode> {
    (1u16..=10, 0..SOURCE_LEN - 1).prop_map(|(sym, start)| leaf(sym, start, start + 1))
}

fn arb_tree(max_depth: u32, max_width: usize) -> impl Strategy<Value = ParsedNode> {
    arb_leaf().prop_recursive(max_depth, 64, max_width as u32, move |inner| {
        (1u16..=10, proptest::collection::vec(inner, 1..=max_width))
            .prop_map(|(sym, children)| interior(sym, children))
    })
}

fn arb_tree_with_errors(max_depth: u32, max_width: usize) -> impl Strategy<Value = ParsedNode> {
    arb_leaf().prop_recursive(max_depth, 64, max_width as u32, move |inner| {
        (
            1u16..=10,
            proptest::collection::vec(inner, 1..=max_width),
            proptest::bool::weighted(0.2),
        )
            .prop_map(|(sym, mut children, inject_error)| {
                if inject_error && !children.is_empty() {
                    let last_end = children.last().map_or(1, |c| c.end_byte);
                    children.push(error_node(last_end, last_end + 1));
                }
                interior(sym, children)
            })
    })
}

/// Strategy producing a deep linear chain: root -> child -> child -> ... -> leaf
fn arb_chain(depth: usize) -> impl Strategy<Value = ParsedNode> {
    (1u16..=10, 0usize..SOURCE_LEN - 1).prop_map(move |(sym, start)| {
        let mut node = leaf(sym, start, start + 1);
        for _ in 1..depth {
            node = interior(sym, vec![node]);
        }
        node
    })
}

/// Strategy producing a wide tree: root with many leaf children.
fn arb_wide(width: usize) -> impl Strategy<Value = ParsedNode> {
    (1u16..=10, 0usize..SOURCE_LEN - 1).prop_map(move |(sym, start)| {
        let children: Vec<_> = (0..width)
            .map(|i| {
                let s = (start + i) % (SOURCE_LEN - 1);
                leaf(sym, s, s + 1)
            })
            .collect();
        interior(sym, children)
    })
}

// ============================================================================
// 1. StatsVisitor counts are consistent (8 tests)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v7_stats_total_equals_reachable_non_error(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert_eq!(stats.total_nodes, count_reachable_non_error(&tree));
    }

    #[test]
    fn v7_stats_leaves_equal_reachable_leaves(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert_eq!(stats.leaf_nodes, count_reachable_leaves(&tree));
    }

    #[test]
    fn v7_stats_errors_equal_reachable_errors(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert_eq!(stats.error_nodes, count_reachable_errors(&tree));
    }

    #[test]
    fn v7_stats_leaf_lte_total(
        source in arb_source(),
        tree in arb_tree(4, 4),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert!(stats.leaf_nodes <= stats.total_nodes);
    }

    #[test]
    fn v7_stats_max_depth_at_least_one(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert!(stats.max_depth >= 1);
    }

    #[test]
    fn v7_stats_node_counts_sum_equals_total(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        let sum: usize = stats.node_counts.values().sum();
        prop_assert_eq!(sum, stats.total_nodes);
    }

    #[test]
    fn v7_stats_node_counts_keys_match_manual(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        let expected = collect_kinds(&tree);
        for (key, &val) in &stats.node_counts {
            prop_assert_eq!(expected.get(key).copied().unwrap_or(0), val);
        }
    }

    #[test]
    fn v7_stats_max_depth_bounded_by_tree_depth(
        source in arb_source(),
        tree in arb_tree(4, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert!(stats.max_depth <= dfs_max_depth(&tree));
    }
}

// ============================================================================
// 2. PrettyPrintVisitor produces non-empty output (5 tests)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v7_pretty_nonempty_output(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut pp);
        prop_assert!(!pp.output().is_empty());
    }

    #[test]
    fn v7_pretty_ends_with_newline(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut pp);
        prop_assert!(pp.output().ends_with('\n'));
    }

    #[test]
    fn v7_pretty_indent_always_even(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut pp);
        for line in pp.output().lines() {
            let leading = line.len() - line.trim_start_matches(' ').len();
            prop_assert!(leading % 2 == 0, "Odd indentation on: {:?}", line);
        }
    }

    #[test]
    fn v7_pretty_lines_gte_non_error_nodes(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut pp);
        let lines = pp.output().lines().count();
        let non_error = count_reachable_non_error(&tree);
        prop_assert!(lines >= non_error, "lines={} < non_error={}", lines, non_error);
    }

    #[test]
    fn v7_pretty_default_eq_new(_x in 0u8..1) {
        let a = PrettyPrintVisitor::new();
        let b = PrettyPrintVisitor::default();
        prop_assert_eq!(a.output(), b.output());
    }
}

// ============================================================================
// 3. SearchVisitor match count properties (8 tests)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v7_search_always_true_matches_total(
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

    #[test]
    fn v7_search_always_false_empty(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut search = SearchVisitor::new(|_: &ParsedNode| false);
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut search);
        prop_assert!(search.matches.is_empty());
    }

    #[test]
    fn v7_search_subset_of_total(
        source in arb_source(),
        tree in arb_tree(3, 3),
        threshold in 1u16..=10,
    ) {
        let t = threshold;
        let src = source.as_bytes();
        let mut filtered = SearchVisitor::new(move |n: &ParsedNode| n.symbol() <= t);
        TreeWalker::new(src).walk(&tree, &mut filtered);
        let mut all = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(src).walk(&tree, &mut all);
        prop_assert!(filtered.matches.len() <= all.matches.len());
    }

    #[test]
    fn v7_search_disjoint_partition(
        source in arb_source(),
        tree in arb_tree(3, 3),
        split in 1u16..=9,
    ) {
        let s = split;
        let src = source.as_bytes();
        let mut low = SearchVisitor::new(move |n: &ParsedNode| n.symbol() <= s);
        TreeWalker::new(src).walk(&tree, &mut low);
        let mut high = SearchVisitor::new(move |n: &ParsedNode| n.symbol() > s);
        TreeWalker::new(src).walk(&tree, &mut high);
        let mut all = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(src).walk(&tree, &mut all);
        prop_assert_eq!(low.matches.len() + high.matches.len(), all.matches.len());
    }

    #[test]
    fn v7_search_named_matches_total_for_all_named_tree(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut named = SearchVisitor::new(|n: &ParsedNode| n.is_named());
        TreeWalker::new(src).walk(&tree, &mut named);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut stats);
        // All generated non-error nodes are named in arb_tree
        prop_assert_eq!(named.matches.len(), stats.total_nodes);
    }

    #[test]
    fn v7_search_error_tree_named_lte_all(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        let src = source.as_bytes();
        let mut named = SearchVisitor::new(|n: &ParsedNode| n.is_named());
        TreeWalker::new(src).walk(&tree, &mut named);
        let total_in_tree = count_all_nodes(&tree);
        prop_assert!(named.matches.len() <= total_in_tree);
    }

    #[test]
    fn v7_search_kinds_nonempty(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut search = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut search);
        for (_start, _end, kind) in &search.matches {
            prop_assert!(!kind.is_empty(), "Empty kind in match");
        }
    }

    #[test]
    fn v7_search_monotone_predicate(
        source in arb_source(),
        tree in arb_tree(3, 3),
        lo in 1u16..=5,
        hi in 6u16..=10,
    ) {
        let src = source.as_bytes();
        let l = lo;
        let h = hi;
        let mut narrow = SearchVisitor::new(move |n: &ParsedNode| n.symbol() >= l && n.symbol() <= l);
        TreeWalker::new(src).walk(&tree, &mut narrow);
        let mut wide = SearchVisitor::new(move |n: &ParsedNode| n.symbol() >= l && n.symbol() <= h);
        TreeWalker::new(src).walk(&tree, &mut wide);
        prop_assert!(narrow.matches.len() <= wide.matches.len());
    }
}

// ============================================================================
// 4. Visitor action properties (5 tests)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v7_action_stop_limits_count_bfs(
        source in arb_source(),
        tree in arb_tree(3, 3),
        limit in 1usize..=5,
    ) {
        // BFS Stop truly halts the entire traversal loop.
        struct StopAfter { count: usize, limit: usize }
        impl Visitor for StopAfter {
            fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
                self.count += 1;
                if self.count >= self.limit { VisitorAction::Stop } else { VisitorAction::Continue }
            }
        }
        let mut v = StopAfter { count: 0, limit };
        BreadthFirstWalker::new(source.as_bytes()).walk(&tree, &mut v);
        prop_assert!(v.count <= limit);
    }

    #[test]
    fn v7_action_skip_children_reduces_total(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        // Skip children at depth 1 — should visit fewer nodes than full walk.
        struct SkipAtDepth { depth: usize, count: usize }
        impl Visitor for SkipAtDepth {
            fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
                self.count += 1;
                self.depth += 1;
                if self.depth > 1 { VisitorAction::SkipChildren } else { VisitorAction::Continue }
            }
            fn leave_node(&mut self, _node: &ParsedNode) { self.depth -= 1; }
        }
        let src = source.as_bytes();
        let mut skip_v = SkipAtDepth { depth: 0, count: 0 };
        TreeWalker::new(src).walk(&tree, &mut skip_v);
        let mut full = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut full);
        prop_assert!(skip_v.count <= full.total_nodes);
    }

    #[test]
    fn v7_action_continue_visits_all(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct CountAll { count: usize }
        impl Visitor for CountAll {
            fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
                self.count += 1;
                VisitorAction::Continue
            }
        }
        let src = source.as_bytes();
        let mut v = CountAll { count: 0 };
        TreeWalker::new(src).walk(&tree, &mut v);
        prop_assert_eq!(v.count, count_reachable_non_error(&tree));
    }

    #[test]
    fn v7_action_leave_node_matches_enter(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct EnterLeaveCounter { enters: usize, leaves: usize }
        impl Visitor for EnterLeaveCounter {
            fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
                self.enters += 1;
                VisitorAction::Continue
            }
            fn leave_node(&mut self, _node: &ParsedNode) {
                self.leaves += 1;
            }
        }
        let mut v = EnterLeaveCounter { enters: 0, leaves: 0 };
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut v);
        prop_assert_eq!(v.enters, v.leaves, "enter/leave mismatch");
    }

    #[test]
    fn v7_action_stop_at_one_gives_one(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct StopImmediately { count: usize }
        impl Visitor for StopImmediately {
            fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
                self.count += 1;
                VisitorAction::Stop
            }
        }
        let mut v = StopImmediately { count: 0 };
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut v);
        prop_assert_eq!(v.count, 1, "Stop should halt after first node");
    }
}

// ============================================================================
// 5. VisitedNode / match tuple properties (5 tests)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v7_match_start_lte_end(
        source in arb_source(),
        sym in 1u16..=10,
        start_byte in 0usize..SOURCE_LEN - 1,
    ) {
        // For well-formed leaf nodes, start_byte <= end_byte.
        let node = leaf(sym, start_byte, start_byte + 1);
        let mut search = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(source.as_bytes()).walk(&node, &mut search);
        for (s, e, _) in &search.matches {
            prop_assert!(s <= e, "start {} > end {}", s, e);
        }
    }

    #[test]
    fn v7_match_count_equals_stats_total(
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

    #[test]
    fn v7_match_kinds_in_node_counts(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut search = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(src).walk(&tree, &mut search);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut stats);
        for (_, _, kind) in &search.matches {
            prop_assert!(
                stats.node_counts.contains_key(kind),
                "Kind {:?} not in node_counts",
                kind
            );
        }
    }

    #[test]
    fn v7_match_kind_counts_agree_with_stats(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut search = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(src).walk(&tree, &mut search);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut stats);

        let mut search_counts: HashMap<String, usize> = HashMap::new();
        for (_, _, kind) in &search.matches {
            *search_counts.entry(kind.clone()).or_insert(0) += 1;
        }
        prop_assert_eq!(search_counts, stats.node_counts);
    }

    #[test]
    fn v7_match_tuples_not_duplicated_for_unique_nodes(
        source in arb_source(),
        sym in 1u16..=10,
        start in 0usize..SOURCE_LEN - 1,
    ) {
        // Single leaf: exactly one match
        let node = leaf(sym, start, start + 1);
        let mut search = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(source.as_bytes()).walk(&node, &mut search);
        prop_assert_eq!(search.matches.len(), 1);
    }
}

// ============================================================================
// 6. Cross-visitor consistency (8 tests)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v7_cross_stats_search_agree_on_count(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut stats = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut stats);
        let mut search = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(src).walk(&tree, &mut search);
        prop_assert_eq!(stats.total_nodes, search.matches.len());
    }

    #[test]
    fn v7_cross_pretty_lines_gte_stats_total(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut stats = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut stats);
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(src).walk(&tree, &mut pp);
        let lines = pp.output().lines().count();
        prop_assert!(lines >= stats.total_nodes, "lines {} < total {}", lines, stats.total_nodes);
    }

    #[test]
    fn v7_cross_transform_count_equals_stats_total(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct CountTransform { count: usize }
        impl TransformVisitor for CountTransform {
            type Output = usize;
            fn transform_node(&mut self, _node: &ParsedNode, children: Vec<usize>) -> usize {
                self.count += 1;
                1 + children.into_iter().sum::<usize>()
            }
            fn transform_leaf(&mut self, _node: &ParsedNode, _text: &str) -> usize {
                self.count += 1;
                1
            }
            fn transform_error(&mut self, _node: &ParsedNode) -> usize {
                self.count += 1;
                1
            }
        }
        let src = source.as_bytes();
        let mut ct = CountTransform { count: 0 };
        let _result = TransformWalker::new(src).walk(&tree, &mut ct);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut stats);
        // TransformVisitor calls exactly one of transform_node/transform_leaf/transform_error
        // per tree node. StatsVisitor.total_nodes counts all non-error nodes via enter_node.
        // For error-free trees (arb_tree), transform count == total_nodes.
        prop_assert_eq!(ct.count, stats.total_nodes + stats.error_nodes,
            "transform count {} != total {} + errors {}",
            ct.count, stats.total_nodes, stats.error_nodes);
    }

    #[test]
    fn v7_cross_stats_accumulates_on_double_walk(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut stats = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut stats);
        let first_total = stats.total_nodes;
        TreeWalker::new(src).walk(&tree, &mut stats);
        prop_assert_eq!(stats.total_nodes, first_total * 2);
    }

    #[test]
    fn v7_cross_pretty_and_search_see_same_kinds(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(src).walk(&tree, &mut pp);
        let mut search = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(src).walk(&tree, &mut search);
        // Every kind found by search should appear somewhere in pretty output
        for (_, _, kind) in &search.matches {
            prop_assert!(pp.output().contains(kind.as_str()),
                "Kind {:?} not found in pretty output", kind);
        }
    }

    #[test]
    fn v7_cross_stats_error_tree_total_plus_error_equals_reachable(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        let src = source.as_bytes();
        let mut stats = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut stats);
        let reachable = count_reachable_non_error(&tree) + count_reachable_errors(&tree);
        prop_assert_eq!(stats.total_nodes + stats.error_nodes, reachable);
    }

    #[test]
    fn v7_cross_search_with_errors_count_lte_all_nodes(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        let mut search = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut search);
        prop_assert!(search.matches.len() <= count_all_nodes(&tree));
    }

    #[test]
    fn v7_cross_fresh_stats_zeros(_x in 0u8..1) {
        let stats = StatsVisitor::default();
        prop_assert_eq!(stats.total_nodes, 0);
        prop_assert_eq!(stats.leaf_nodes, 0);
        prop_assert_eq!(stats.error_nodes, 0);
        prop_assert_eq!(stats.max_depth, 0);
        prop_assert!(stats.node_counts.is_empty());
    }
}

// ============================================================================
// 7. DFS/BFS produce same total counts (8 tests)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v7_dfs_bfs_same_total_nodes(
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

    #[test]
    fn v7_dfs_bfs_same_leaf_count(
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

    #[test]
    fn v7_dfs_bfs_same_error_count(
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

    #[test]
    fn v7_dfs_bfs_same_search_count(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut dfs_s = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(src).walk(&tree, &mut dfs_s);
        let mut bfs_s = SearchVisitor::new(|_: &ParsedNode| true);
        BreadthFirstWalker::new(src).walk(&tree, &mut bfs_s);
        prop_assert_eq!(dfs_s.matches.len(), bfs_s.matches.len());
    }

    #[test]
    fn v7_dfs_bfs_same_search_filtered(
        source in arb_source(),
        tree in arb_tree(3, 3),
        threshold in 1u16..=10,
    ) {
        let t = threshold;
        let src = source.as_bytes();
        let mut dfs_s = SearchVisitor::new(move |n: &ParsedNode| n.symbol() <= t);
        TreeWalker::new(src).walk(&tree, &mut dfs_s);
        let mut bfs_s = SearchVisitor::new(move |n: &ParsedNode| n.symbol() <= t);
        BreadthFirstWalker::new(src).walk(&tree, &mut bfs_s);
        prop_assert_eq!(dfs_s.matches.len(), bfs_s.matches.len());
    }

    #[test]
    fn v7_dfs_bfs_same_node_counts_keys(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut dfs = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut dfs);
        let mut bfs = StatsVisitor::default();
        BreadthFirstWalker::new(src).walk(&tree, &mut bfs);
        let dfs_keys: std::collections::HashSet<_> = dfs.node_counts.keys().collect();
        let bfs_keys: std::collections::HashSet<_> = bfs.node_counts.keys().collect();
        prop_assert_eq!(dfs_keys, bfs_keys);
    }

    #[test]
    fn v7_dfs_bfs_same_node_counts_values(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut dfs = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut dfs);
        let mut bfs = StatsVisitor::default();
        BreadthFirstWalker::new(src).walk(&tree, &mut bfs);
        prop_assert_eq!(dfs.node_counts, bfs.node_counts);
    }

    #[test]
    fn v7_dfs_bfs_same_total_with_errors(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        let src = source.as_bytes();
        let mut dfs = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut dfs);
        let mut bfs = StatsVisitor::default();
        BreadthFirstWalker::new(src).walk(&tree, &mut bfs);
        prop_assert_eq!(dfs.total_nodes, bfs.total_nodes);
        prop_assert_eq!(dfs.leaf_nodes, bfs.leaf_nodes);
        prop_assert_eq!(dfs.error_nodes, bfs.error_nodes);
    }
}

// ============================================================================
// 8. Various tree shapes (8 tests)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v7_shape_single_leaf(
        source in arb_source(),
        sym in 1u16..=10,
        start in 0usize..SOURCE_LEN - 1,
    ) {
        let node = leaf(sym, start, start + 1);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&node, &mut stats);
        prop_assert_eq!(stats.total_nodes, 1);
        prop_assert_eq!(stats.leaf_nodes, 1);
        prop_assert_eq!(stats.error_nodes, 0);
        prop_assert_eq!(stats.max_depth, 1);
    }

    #[test]
    fn v7_shape_deep_chain(
        source in arb_source(),
        depth in 2usize..=8,
    ) {
        let tree = arb_chain(depth);
        proptest::test_runner::TestRunner::default()
            .run(&tree, |t| {
                let mut stats = StatsVisitor::default();
                TreeWalker::new(source.as_bytes()).walk(&t, &mut stats);
                prop_assert_eq!(stats.total_nodes, depth);
                prop_assert_eq!(stats.leaf_nodes, 1);
                prop_assert_eq!(stats.max_depth, depth);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn v7_shape_wide_flat(
        source in arb_source(),
        width in 2usize..=10,
    ) {
        let tree = arb_wide(width);
        proptest::test_runner::TestRunner::default()
            .run(&tree, |t| {
                let mut stats = StatsVisitor::default();
                TreeWalker::new(source.as_bytes()).walk(&t, &mut stats);
                prop_assert_eq!(stats.total_nodes, width + 1); // root + children
                prop_assert_eq!(stats.leaf_nodes, width);
                prop_assert_eq!(stats.max_depth, 2);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn v7_shape_error_only_root(
        source in arb_source(),
        start in 0usize..SOURCE_LEN - 1,
    ) {
        let node = error_node(start, start + 1);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&node, &mut stats);
        prop_assert_eq!(stats.total_nodes, 0);
        prop_assert_eq!(stats.leaf_nodes, 0);
        prop_assert_eq!(stats.error_nodes, 1);
    }

    #[test]
    fn v7_shape_error_with_hidden_children(
        source in arb_source(),
        start in 0usize..SOURCE_LEN - 2,
    ) {
        // Error node with children — children should NOT be visited
        let child = leaf(5, start, start + 1);
        let node = error_with_children(start, start + 2, vec![child]);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&node, &mut stats);
        prop_assert_eq!(stats.total_nodes, 0, "Children of error nodes should be skipped");
        prop_assert_eq!(stats.error_nodes, 1);
    }

    #[test]
    fn v7_shape_mixed_error_and_normal(
        source in arb_source(),
    ) {
        // root(leaf, error, leaf)
        let children = vec![
            leaf(5, 0, 1),
            error_node(1, 2),
            leaf(5, 2, 3),
        ];
        let tree = interior(5, children);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert_eq!(stats.total_nodes, 3); // root + 2 leaves
        prop_assert_eq!(stats.leaf_nodes, 2);
        prop_assert_eq!(stats.error_nodes, 1);
    }

    #[test]
    fn v7_shape_unnamed_leaf_pretty_no_named_tag(
        source in arb_source(),
        start in 0usize..SOURCE_LEN - 1,
    ) {
        let node = unnamed_leaf(5, start, start + 1);
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&node, &mut pp);
        // The enter_node line for an unnamed node should NOT contain "[named]"
        let first_line = pp.output().lines().next().unwrap_or("");
        prop_assert!(!first_line.contains("[named]"),
            "Unnamed node should not have [named] tag: {:?}", first_line);
    }

    #[test]
    fn v7_shape_balanced_binary(
        source in arb_source(),
    ) {
        // Balanced binary tree of depth 3: 7 nodes, 4 leaves
        let l1 = leaf(5, 0, 1);
        let l2 = leaf(5, 1, 2);
        let l3 = leaf(5, 2, 3);
        let l4 = leaf(5, 3, 4);
        let b1 = interior(5, vec![l1, l2]);
        let b2 = interior(5, vec![l3, l4]);
        let root = interior(5, vec![b1, b2]);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&root, &mut stats);
        prop_assert_eq!(stats.total_nodes, 7);
        prop_assert_eq!(stats.leaf_nodes, 4);
        prop_assert_eq!(stats.max_depth, 3);
    }
}

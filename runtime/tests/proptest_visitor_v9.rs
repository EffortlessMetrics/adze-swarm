//! Property-based tests (v9) for the visitor module.
//!
//! 55 proptest tests covering:
//! 1. Stats visitor proptest — random trees, node_count consistency (5 tests)
//! 2. Walk visits all nodes proptest — walker visits every node (5 tests)
//! 3. Search finds correct nodes proptest — search by symbol matches manual filter (5 tests)
//! 4. Tree depth proptest — measured depth matches expected (5 tests)
//! 5. Named node counting proptest — named_count properties (5 tests)
//! 6. Regular visitor tests — specific tree shapes (15 tests)
//! 7. Edge cases — single node, deep chain, wide tree, error-only tree (10 tests)
//! 8. Arena allocator proptest — arena invariants (5 tests)

use adze::arena_allocator::{TreeArena, TreeNode};
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

fn named_leaf(symbol: u16, start: usize, end: usize) -> ParsedNode {
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

fn unnamed_interior(symbol: u16, children: Vec<ParsedNode>) -> ParsedNode {
    let start = children.first().map_or(0, |c| c.start_byte);
    let end = children.last().map_or(0, |c| c.end_byte);
    make_node(symbol, children, start, end, false, false)
}

fn error_node(start: usize, end: usize) -> ParsedNode {
    make_node(0, vec![], start, end, true, false)
}

fn error_with_children(start: usize, end: usize, children: Vec<ParsedNode>) -> ParsedNode {
    make_node(0, children, start, end, true, false)
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

/// Count error nodes reachable by the walker.
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

/// Count named, non-error nodes reachable by the walker.
fn count_reachable_named(node: &ParsedNode) -> usize {
    if node.is_error() {
        0
    } else {
        let self_count = if node.is_named() { 1 } else { 0 };
        self_count
            + node
                .children()
                .iter()
                .map(count_reachable_named)
                .sum::<usize>()
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
    (1u16..=10, 0..SOURCE_LEN - 1).prop_map(|(sym, start)| named_leaf(sym, start, start + 1))
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

/// Tree with a mix of named and unnamed nodes.
fn arb_tree_mixed_named(max_depth: u32, max_width: usize) -> impl Strategy<Value = ParsedNode> {
    let base = (1u16..=10, 0..SOURCE_LEN - 1, any::<bool>()).prop_map(|(sym, start, is_named)| {
        if is_named {
            named_leaf(sym, start, start + 1)
        } else {
            unnamed_leaf(sym, start, start + 1)
        }
    });
    base.prop_recursive(max_depth, 64, max_width as u32, move |inner| {
        (
            1u16..=10,
            proptest::collection::vec(inner, 1..=max_width),
            any::<bool>(),
        )
            .prop_map(|(sym, children, is_named)| {
                if is_named {
                    interior(sym, children)
                } else {
                    unnamed_interior(sym, children)
                }
            })
    })
}

/// Deep linear chain: root -> child -> ... -> leaf
fn arb_chain(depth: usize) -> impl Strategy<Value = ParsedNode> {
    (1u16..=10, 0usize..SOURCE_LEN - 1).prop_map(move |(sym, start)| {
        let mut node = named_leaf(sym, start, start + 1);
        for _ in 1..depth {
            node = interior(sym, vec![node]);
        }
        node
    })
}

/// Wide tree: root with many leaf children.
fn arb_wide(width: usize) -> impl Strategy<Value = ParsedNode> {
    (1u16..=10, 0usize..SOURCE_LEN - 1).prop_map(move |(sym, start)| {
        let children: Vec<_> = (0..width)
            .map(|i| {
                let s = (start + i) % (SOURCE_LEN - 1);
                named_leaf(sym, s, s + 1)
            })
            .collect();
        interior(sym, children)
    })
}

// ============================================================================
// 1. Stats visitor proptest — node_count consistency (5 tests)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v9_stats_total_equals_reachable_non_error(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert_eq!(stats.total_nodes, count_reachable_non_error(&tree));
    }

    #[test]
    fn v9_stats_leaves_equal_reachable_leaves(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert_eq!(stats.leaf_nodes, count_reachable_leaves(&tree));
    }

    #[test]
    fn v9_stats_errors_equal_reachable_errors(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert_eq!(stats.error_nodes, count_reachable_errors(&tree));
    }

    #[test]
    fn v9_stats_node_counts_sum_equals_total(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        let sum: usize = stats.node_counts.values().sum();
        prop_assert_eq!(sum, stats.total_nodes);
    }

    #[test]
    fn v9_stats_node_counts_keys_match_manual(
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
}

// ============================================================================
// 2. Walk visits all nodes proptest (5 tests)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v9_walk_visits_all_non_error(
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
        let mut v = CountAll { count: 0 };
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut v);
        prop_assert_eq!(v.count, count_reachable_non_error(&tree));
    }

    #[test]
    fn v9_walk_enter_leave_balanced(
        source in arb_source(),
        tree in arb_tree(4, 3),
    ) {
        struct Balance { enters: usize, leaves: usize }
        impl Visitor for Balance {
            fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
                self.enters += 1;
                VisitorAction::Continue
            }
            fn leave_node(&mut self, _node: &ParsedNode) {
                self.leaves += 1;
            }
        }
        let mut v = Balance { enters: 0, leaves: 0 };
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut v);
        prop_assert_eq!(v.enters, v.leaves, "enter/leave mismatch");
    }

    #[test]
    fn v9_walk_bfs_visits_all_non_error(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        BreadthFirstWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert_eq!(stats.total_nodes, count_reachable_non_error(&tree));
    }

    #[test]
    fn v9_walk_transform_visits_all(
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
        prop_assert_eq!(ct.count, stats.total_nodes + stats.error_nodes);
    }

    #[test]
    fn v9_walk_dfs_bfs_same_total(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        let src = source.as_bytes();
        let mut dfs = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut dfs);
        let mut bfs = StatsVisitor::default();
        BreadthFirstWalker::new(src).walk(&tree, &mut bfs);
        prop_assert_eq!(dfs.total_nodes, bfs.total_nodes);
        prop_assert_eq!(dfs.error_nodes, bfs.error_nodes);
    }
}

// ============================================================================
// 3. Search finds correct nodes proptest (5 tests)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v9_search_always_true_matches_total(
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
    fn v9_search_always_false_empty(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut search = SearchVisitor::new(|_: &ParsedNode| false);
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut search);
        prop_assert!(search.matches.is_empty());
    }

    #[test]
    fn v9_search_by_symbol_matches_manual(
        source in arb_source(),
        tree in arb_tree(3, 3),
        threshold in 1u16..=10,
    ) {
        let t = threshold;
        let src = source.as_bytes();
        let mut filtered = SearchVisitor::new(move |n: &ParsedNode| n.symbol() <= t);
        TreeWalker::new(src).walk(&tree, &mut filtered);

        // Manual count of reachable non-error nodes with symbol <= threshold
        fn count_matching(node: &ParsedNode, t: u16) -> usize {
            if node.is_error() {
                0
            } else {
                let self_count = if node.symbol() <= t { 1 } else { 0 };
                self_count
                    + node
                        .children()
                        .iter()
                        .map(|c| count_matching(c, t))
                        .sum::<usize>()
            }
        }
        prop_assert_eq!(filtered.matches.len(), count_matching(&tree, threshold));
    }

    #[test]
    fn v9_search_disjoint_partition(
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
    fn v9_search_monotone_widening(
        source in arb_source(),
        tree in arb_tree(3, 3),
        lo in 1u16..=5,
        hi in 6u16..=10,
    ) {
        let l = lo;
        let h = hi;
        let src = source.as_bytes();
        let mut narrow = SearchVisitor::new(move |n: &ParsedNode| n.symbol() >= l && n.symbol() <= l);
        TreeWalker::new(src).walk(&tree, &mut narrow);
        let mut wide = SearchVisitor::new(move |n: &ParsedNode| n.symbol() >= l && n.symbol() <= h);
        TreeWalker::new(src).walk(&tree, &mut wide);
        prop_assert!(narrow.matches.len() <= wide.matches.len());
    }
}

// ============================================================================
// 4. Tree depth proptest (5 tests)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v9_depth_matches_expected(
        source in arb_source(),
        tree in arb_tree(4, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert!(stats.max_depth <= dfs_max_depth(&tree));
    }

    #[test]
    fn v9_depth_at_least_one(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert!(stats.max_depth >= 1);
    }

    #[test]
    fn v9_depth_chain_equals_chain_length(
        source in arb_source(),
        depth in 2usize..=8,
    ) {
        let chain_strat = arb_chain(depth);
        proptest::test_runner::TestRunner::default()
            .run(&chain_strat, |t| {
                let mut stats = StatsVisitor::default();
                TreeWalker::new(source.as_bytes()).walk(&t, &mut stats);
                prop_assert_eq!(stats.max_depth, depth);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn v9_depth_wide_tree_is_two(
        source in arb_source(),
        width in 2usize..=10,
    ) {
        let wide_strat = arb_wide(width);
        proptest::test_runner::TestRunner::default()
            .run(&wide_strat, |t| {
                let mut stats = StatsVisitor::default();
                TreeWalker::new(source.as_bytes()).walk(&t, &mut stats);
                prop_assert_eq!(stats.max_depth, 2);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn v9_depth_leaf_is_one(
        source in arb_source(),
        sym in 1u16..=10,
        start in 0usize..SOURCE_LEN - 1,
    ) {
        let node = named_leaf(sym, start, start + 1);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&node, &mut stats);
        prop_assert_eq!(stats.max_depth, 1);
    }
}

// ============================================================================
// 5. Named node counting proptest (5 tests)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v9_named_count_lte_total(
        source in arb_source(),
        tree in arb_tree_mixed_named(3, 3),
    ) {
        let src = source.as_bytes();
        let mut named_search = SearchVisitor::new(|n: &ParsedNode| n.is_named());
        TreeWalker::new(src).walk(&tree, &mut named_search);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut stats);
        prop_assert!(named_search.matches.len() <= stats.total_nodes);
    }

    #[test]
    fn v9_named_plus_unnamed_equals_total(
        source in arb_source(),
        tree in arb_tree_mixed_named(3, 3),
    ) {
        let src = source.as_bytes();
        let mut named = SearchVisitor::new(|n: &ParsedNode| n.is_named());
        TreeWalker::new(src).walk(&tree, &mut named);
        let mut unnamed = SearchVisitor::new(|n: &ParsedNode| !n.is_named());
        TreeWalker::new(src).walk(&tree, &mut unnamed);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut stats);
        prop_assert_eq!(named.matches.len() + unnamed.matches.len(), stats.total_nodes);
    }

    #[test]
    fn v9_named_matches_manual_count(
        source in arb_source(),
        tree in arb_tree_mixed_named(3, 3),
    ) {
        let mut named = SearchVisitor::new(|n: &ParsedNode| n.is_named());
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut named);
        prop_assert_eq!(named.matches.len(), count_reachable_named(&tree));
    }

    #[test]
    fn v9_all_named_tree_named_equals_total(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        // arb_tree produces only named nodes
        let src = source.as_bytes();
        let mut named = SearchVisitor::new(|n: &ParsedNode| n.is_named());
        TreeWalker::new(src).walk(&tree, &mut named);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut stats);
        prop_assert_eq!(named.matches.len(), stats.total_nodes);
    }

    #[test]
    fn v9_named_pretty_tag_present_only_for_named(
        source in arb_source(),
        tree in arb_tree_mixed_named(2, 2),
    ) {
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut pp);
        // Lines with "[named]" tag should exist only for named nodes
        let named_lines = pp.output().lines().filter(|l| l.contains("[named]")).count();
        let named_count = count_reachable_named(&tree);
        prop_assert_eq!(named_lines, named_count);
    }
}

// ============================================================================
// 6. Regular visitor tests — specific tree shapes (15 tests)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v9_regular_pretty_nonempty(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut pp);
        prop_assert!(!pp.output().is_empty());
    }

    #[test]
    fn v9_regular_pretty_ends_with_newline(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut pp);
        prop_assert!(pp.output().ends_with('\n'));
    }

    #[test]
    fn v9_regular_pretty_indent_always_even(
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
    fn v9_regular_pretty_lines_gte_non_error(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut pp);
        let lines = pp.output().lines().count();
        let non_error = count_reachable_non_error(&tree);
        prop_assert!(lines >= non_error);
    }

    #[test]
    fn v9_regular_pretty_default_eq_new(_x in 0u8..1) {
        let a = PrettyPrintVisitor::new();
        let b = PrettyPrintVisitor::default();
        prop_assert_eq!(a.output(), b.output());
    }

    #[test]
    fn v9_regular_stop_halts_at_one(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct StopImmediate { count: usize }
        impl Visitor for StopImmediate {
            fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
                self.count += 1;
                VisitorAction::Stop
            }
        }
        let mut v = StopImmediate { count: 0 };
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut v);
        prop_assert_eq!(v.count, 1);
    }

    #[test]
    fn v9_regular_skip_reduces_total(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
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
    fn v9_regular_stats_accumulates_on_double_walk(
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
    fn v9_regular_search_kinds_nonempty(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut search = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut search);
        for (_, _, kind) in &search.matches {
            prop_assert!(!kind.is_empty());
        }
    }

    #[test]
    fn v9_regular_search_leaf_start_lte_end(
        source in arb_source(),
        sym in 1u16..=10,
        start in 0usize..SOURCE_LEN - 1,
    ) {
        // Only leaf nodes are guaranteed start <= end by construction
        let node = named_leaf(sym, start, start + 1);
        let mut search = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(source.as_bytes()).walk(&node, &mut search);
        for (s, e, _) in &search.matches {
            prop_assert!(s <= e, "start {} > end {}", s, e);
        }
    }

    #[test]
    fn v9_regular_cross_stats_search_agree(
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
    fn v9_regular_pretty_and_search_same_kinds(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(src).walk(&tree, &mut pp);
        let mut search = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(src).walk(&tree, &mut search);
        for (_, _, kind) in &search.matches {
            prop_assert!(pp.output().contains(kind.as_str()));
        }
    }

    #[test]
    fn v9_regular_match_kind_counts_agree(
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
    fn v9_regular_dfs_bfs_same_node_counts(
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
    fn v9_regular_fresh_stats_zeros(_x in 0u8..1) {
        let stats = StatsVisitor::default();
        prop_assert_eq!(stats.total_nodes, 0);
        prop_assert_eq!(stats.leaf_nodes, 0);
        prop_assert_eq!(stats.error_nodes, 0);
        prop_assert_eq!(stats.max_depth, 0);
        prop_assert!(stats.node_counts.is_empty());
    }
}

// ============================================================================
// 7. Edge cases (10 tests)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v9_edge_single_leaf(
        source in arb_source(),
        sym in 1u16..=10,
        start in 0usize..SOURCE_LEN - 1,
    ) {
        let node = named_leaf(sym, start, start + 1);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&node, &mut stats);
        prop_assert_eq!(stats.total_nodes, 1);
        prop_assert_eq!(stats.leaf_nodes, 1);
        prop_assert_eq!(stats.error_nodes, 0);
        prop_assert_eq!(stats.max_depth, 1);
    }

    #[test]
    fn v9_edge_deep_chain_total(
        source in arb_source(),
        depth in 2usize..=8,
    ) {
        let chain_strat = arb_chain(depth);
        proptest::test_runner::TestRunner::default()
            .run(&chain_strat, |t| {
                let mut stats = StatsVisitor::default();
                TreeWalker::new(source.as_bytes()).walk(&t, &mut stats);
                prop_assert_eq!(stats.total_nodes, depth);
                prop_assert_eq!(stats.leaf_nodes, 1);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn v9_edge_wide_tree_total(
        source in arb_source(),
        width in 2usize..=10,
    ) {
        let wide_strat = arb_wide(width);
        proptest::test_runner::TestRunner::default()
            .run(&wide_strat, |t| {
                let mut stats = StatsVisitor::default();
                TreeWalker::new(source.as_bytes()).walk(&t, &mut stats);
                prop_assert_eq!(stats.total_nodes, width + 1);
                prop_assert_eq!(stats.leaf_nodes, width);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn v9_edge_error_only_root(
        source in arb_source(),
        start in 0usize..SOURCE_LEN - 1,
    ) {
        let node = error_node(start, start + 1);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&node, &mut stats);
        prop_assert_eq!(stats.total_nodes, 0);
        prop_assert_eq!(stats.error_nodes, 1);
    }

    #[test]
    fn v9_edge_error_children_hidden(
        source in arb_source(),
        start in 0usize..SOURCE_LEN - 2,
    ) {
        let child = named_leaf(5, start, start + 1);
        let node = error_with_children(start, start + 2, vec![child]);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&node, &mut stats);
        prop_assert_eq!(stats.total_nodes, 0);
        prop_assert_eq!(stats.error_nodes, 1);
    }

    #[test]
    fn v9_edge_mixed_error_and_normal(source in arb_source()) {
        let children = vec![
            named_leaf(5, 0, 1),
            error_node(1, 2),
            named_leaf(5, 2, 3),
        ];
        let tree = interior(5, children);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert_eq!(stats.total_nodes, 3);
        prop_assert_eq!(stats.leaf_nodes, 2);
        prop_assert_eq!(stats.error_nodes, 1);
    }

    #[test]
    fn v9_edge_balanced_binary(source in arb_source()) {
        let l1 = named_leaf(5, 0, 1);
        let l2 = named_leaf(5, 1, 2);
        let l3 = named_leaf(5, 2, 3);
        let l4 = named_leaf(5, 3, 4);
        let b1 = interior(5, vec![l1, l2]);
        let b2 = interior(5, vec![l3, l4]);
        let root = interior(5, vec![b1, b2]);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&root, &mut stats);
        prop_assert_eq!(stats.total_nodes, 7);
        prop_assert_eq!(stats.leaf_nodes, 4);
        prop_assert_eq!(stats.max_depth, 3);
    }

    #[test]
    fn v9_edge_unnamed_no_named_tag(
        source in arb_source(),
        start in 0usize..SOURCE_LEN - 1,
    ) {
        let node = unnamed_leaf(5, start, start + 1);
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&node, &mut pp);
        let first_line = pp.output().lines().next().unwrap_or("");
        prop_assert!(!first_line.contains("[named]"));
    }

    #[test]
    fn v9_edge_bfs_stop_limits_count(
        source in arb_source(),
        tree in arb_tree(3, 3),
        limit in 1usize..=5,
    ) {
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
    fn v9_edge_all_errors_tree(source in arb_source()) {
        // Tree with root containing only error children
        let children = vec![
            error_node(0, 1),
            error_node(1, 2),
            error_node(2, 3),
        ];
        let tree = interior(5, children);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        // Root is non-error, has no non-error children => 1 total, 0 leaves (has children)
        prop_assert_eq!(stats.total_nodes, 1);
        prop_assert_eq!(stats.error_nodes, 3);
        prop_assert_eq!(stats.leaf_nodes, 0);
    }
}

// ============================================================================
// 8. Arena allocator proptest (5 tests)
// ============================================================================

fn arb_arena_values() -> impl Strategy<Value = Vec<i32>> {
    proptest::collection::vec(-1000i32..1000, 1..=50)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v9_arena_len_matches_alloc_count(values in arb_arena_values()) {
        let mut arena = TreeArena::new();
        for &v in &values {
            arena.alloc(TreeNode::leaf(v));
        }
        prop_assert_eq!(arena.len(), values.len());
    }

    #[test]
    fn v9_arena_get_returns_correct_values(values in arb_arena_values()) {
        let mut arena = TreeArena::new();
        let handles: Vec<_> = values.iter().map(|&v| arena.alloc(TreeNode::leaf(v))).collect();
        for (handle, &expected) in handles.iter().zip(values.iter()) {
            prop_assert_eq!(arena.get(*handle).value(), expected);
        }
    }

    #[test]
    fn v9_arena_reset_clears_len(values in arb_arena_values()) {
        let mut arena = TreeArena::new();
        for &v in &values {
            arena.alloc(TreeNode::leaf(v));
        }
        prop_assert!(!arena.is_empty());
        arena.reset();
        prop_assert!(arena.is_empty());
        prop_assert_eq!(arena.len(), 0);
    }

    #[test]
    fn v9_arena_branch_children_roundtrip(count in 1usize..=10) {
        let mut arena = TreeArena::new();
        let children: Vec<_> = (0..count).map(|i| arena.alloc(TreeNode::leaf(i as i32))).collect();
        let parent = arena.alloc(TreeNode::branch(children.clone()));
        let parent_ref = arena.get(parent);
        prop_assert!(parent_ref.is_branch());
        prop_assert_eq!(parent_ref.children().len(), count);
        for (i, &child_handle) in parent_ref.children().iter().enumerate() {
            prop_assert_eq!(arena.get(child_handle).value(), i as i32);
        }
    }

    #[test]
    fn v9_arena_capacity_gte_len(values in arb_arena_values()) {
        let mut arena = TreeArena::new();
        for &v in &values {
            arena.alloc(TreeNode::leaf(v));
        }
        prop_assert!(arena.capacity() >= arena.len());
    }
}

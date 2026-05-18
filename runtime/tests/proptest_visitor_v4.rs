//! Property-based tests (v4) for the visitor module.
//!
//! 50 proptest properties covering:
//! 1. StatsVisitor node_count >= leaf_count (5 properties)
//! 2. StatsVisitor max_depth >= 1 for non-empty trees (5 properties)
//! 3. DFS and BFS visit same node count (5 properties)
//! 4. SearchVisitor finds subset of all nodes (5 properties)
//! 5. Empty tree gives zero stats (5 properties)
//! 6. Custom visitor accumulation is deterministic (5 properties)
//! 7. PrettyPrintVisitor output length scales with tree size (5 properties)
//! 8. Nested tree construction properties (5 properties)
//! 9. Edge cases (5+ properties)

use adze::arena_allocator::{TreeArena, TreeNode};
use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TreeWalker, Visitor,
    VisitorAction,
};
use proptest::prelude::*;

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

/// Count reachable leaf nodes (non-error, childless).
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

/// Count reachable error nodes.
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

/// Compute DFS max depth for non-error nodes.
fn dfs_max_depth(node: &ParsedNode) -> usize {
    if node.is_error() {
        0
    } else if node.children().is_empty() {
        1
    } else {
        1 + node.children().iter().map(dfs_max_depth).max().unwrap_or(0)
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

fn arb_tree_mixed_named(max_depth: u32, max_width: usize) -> impl Strategy<Value = ParsedNode> {
    let base = (1u16..=10, 0..SOURCE_LEN - 1, any::<bool>()).prop_map(|(sym, start, named)| {
        if named {
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
            .prop_map(|(sym, children, named)| {
                if named {
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
// 1. StatsVisitor node_count >= leaf_count (5 properties)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v4_stats_total_gte_leaves(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert!(stats.total_nodes >= stats.leaf_nodes);
    }

    #[test]
    fn v4_stats_total_equals_reachable_non_error(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert_eq!(stats.total_nodes, count_reachable_non_error(&tree));
    }

    #[test]
    fn v4_stats_leaves_equal_reachable_leaves(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert_eq!(stats.leaf_nodes, count_reachable_leaves(&tree));
    }

    #[test]
    fn v4_stats_errors_equal_reachable_errors(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert_eq!(stats.error_nodes, count_reachable_errors(&tree));
    }

    #[test]
    fn v4_stats_node_counts_sum_equals_total(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        let sum: usize = stats.node_counts.values().sum();
        prop_assert_eq!(sum, stats.total_nodes);
    }
}

// ============================================================================
// 2. StatsVisitor max_depth >= 1 for non-empty trees (5 properties)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v4_depth_at_least_one(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert!(stats.max_depth >= 1);
    }

    #[test]
    fn v4_depth_lte_manual_dfs_depth(
        source in arb_source(),
        tree in arb_tree(4, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert!(stats.max_depth <= dfs_max_depth(&tree));
    }

    #[test]
    fn v4_depth_chain_equals_chain_length(
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
    fn v4_depth_wide_tree_is_two(
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
    fn v4_depth_leaf_is_one(
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
// 3. DFS and BFS visit same node count (5 properties)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v4_dfs_bfs_same_total(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        let src = source.as_bytes();
        let mut dfs = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut dfs);
        let mut bfs = StatsVisitor::default();
        BreadthFirstWalker::new(src).walk(&tree, &mut bfs);
        prop_assert_eq!(dfs.total_nodes, bfs.total_nodes);
    }

    #[test]
    fn v4_dfs_bfs_same_errors(
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
    fn v4_dfs_bfs_same_leaf_count(
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
    fn v4_dfs_bfs_same_node_counts_map(
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
    fn v4_dfs_bfs_chain_same_depth(
        source in arb_source(),
        depth in 2usize..=6,
    ) {
        let chain_strat = arb_chain(depth);
        proptest::test_runner::TestRunner::default()
            .run(&chain_strat, |t| {
                let src = source.as_bytes();
                let mut dfs = StatsVisitor::default();
                TreeWalker::new(src).walk(&t, &mut dfs);
                let mut bfs = StatsVisitor::default();
                BreadthFirstWalker::new(src).walk(&t, &mut bfs);
                prop_assert_eq!(dfs.total_nodes, bfs.total_nodes);
                Ok(())
            })
            .unwrap();
    }
}

// ============================================================================
// 4. SearchVisitor finds subset of all nodes (5 properties)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v4_search_always_true_matches_total(
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
    fn v4_search_always_false_empty(
        source in arb_source(),
        tree in arb_tree_mixed_named(3, 3),
    ) {
        let mut search = SearchVisitor::new(|_: &ParsedNode| false);
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut search);
        prop_assert!(search.matches.is_empty());
    }

    #[test]
    fn v4_search_disjoint_partition(
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
    fn v4_search_monotone_widening(
        source in arb_source(),
        tree in arb_tree(3, 3),
        lo in 1u16..=5,
        hi in 6u16..=10,
    ) {
        let l = lo;
        let h = hi;
        let src = source.as_bytes();
        let mut narrow = SearchVisitor::new(move |n: &ParsedNode| n.symbol() == l);
        TreeWalker::new(src).walk(&tree, &mut narrow);
        let mut wide = SearchVisitor::new(move |n: &ParsedNode| n.symbol() >= l && n.symbol() <= h);
        TreeWalker::new(src).walk(&tree, &mut wide);
        prop_assert!(narrow.matches.len() <= wide.matches.len());
    }

    #[test]
    fn v4_search_by_symbol_matches_manual(
        source in arb_source(),
        tree in arb_tree(3, 3),
        threshold in 1u16..=10,
    ) {
        let t = threshold;
        let mut filtered = SearchVisitor::new(move |n: &ParsedNode| n.symbol() <= t);
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut filtered);

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
}

// ============================================================================
// 5. Empty tree gives zero stats (5 properties)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v4_empty_error_root_zero_total(
        source in arb_source(),
        start in 0usize..SOURCE_LEN - 1,
    ) {
        let node = error_node(start, start + 1);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&node, &mut stats);
        prop_assert_eq!(stats.total_nodes, 0);
    }

    #[test]
    fn v4_empty_error_root_one_error(
        source in arb_source(),
        start in 0usize..SOURCE_LEN - 1,
    ) {
        let node = error_node(start, start + 1);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&node, &mut stats);
        prop_assert_eq!(stats.error_nodes, 1);
    }

    #[test]
    fn v4_empty_error_root_zero_leaves(
        source in arb_source(),
        start in 0usize..SOURCE_LEN - 1,
    ) {
        let node = error_node(start, start + 1);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&node, &mut stats);
        prop_assert_eq!(stats.leaf_nodes, 0);
    }

    #[test]
    fn v4_empty_error_root_zero_depth(
        source in arb_source(),
        start in 0usize..SOURCE_LEN - 1,
    ) {
        let node = error_node(start, start + 1);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&node, &mut stats);
        prop_assert_eq!(stats.max_depth, 0);
    }

    #[test]
    fn v4_empty_error_root_search_empty(
        source in arb_source(),
        start in 0usize..SOURCE_LEN - 1,
    ) {
        let node = error_node(start, start + 1);
        let mut search = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(source.as_bytes()).walk(&node, &mut search);
        prop_assert!(search.matches.is_empty());
    }
}

// ============================================================================
// 6. Custom visitor accumulation is deterministic (5 properties)
// ============================================================================

/// Counts nodes entered during DFS.
struct CountVisitor {
    count: usize,
}

impl Visitor for CountVisitor {
    fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
        self.count += 1;
        VisitorAction::Continue
    }
}

/// Sums symbols of entered nodes.
struct SymbolSumVisitor {
    sum: u64,
}

impl Visitor for SymbolSumVisitor {
    fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
        self.sum += node.symbol() as u64;
        VisitorAction::Continue
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v4_custom_count_deterministic(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut v1 = CountVisitor { count: 0 };
        TreeWalker::new(src).walk(&tree, &mut v1);
        let mut v2 = CountVisitor { count: 0 };
        TreeWalker::new(src).walk(&tree, &mut v2);
        prop_assert_eq!(v1.count, v2.count);
    }

    #[test]
    fn v4_custom_symbol_sum_deterministic(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut v1 = SymbolSumVisitor { sum: 0 };
        TreeWalker::new(src).walk(&tree, &mut v1);
        let mut v2 = SymbolSumVisitor { sum: 0 };
        TreeWalker::new(src).walk(&tree, &mut v2);
        prop_assert_eq!(v1.sum, v2.sum);
    }

    #[test]
    fn v4_custom_stats_accumulates_on_double_walk(
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
    fn v4_custom_stop_halts_at_one(
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
    fn v4_custom_skip_reduces_total(
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
}

// ============================================================================
// 7. PrettyPrintVisitor output length scales with tree size (5 properties)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v4_pretty_nonempty_for_any_tree(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut pp);
        prop_assert!(!pp.output().is_empty());
    }

    #[test]
    fn v4_pretty_ends_with_newline(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut pp);
        prop_assert!(pp.output().ends_with('\n'));
    }

    #[test]
    fn v4_pretty_lines_gte_non_error_count(
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
    fn v4_pretty_indent_always_even(
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
    fn v4_pretty_larger_tree_longer_output(
        source in arb_source(),
        sym in 1u16..=10,
        start in 0usize..SOURCE_LEN - 1,
    ) {
        let leaf = named_leaf(sym, start, start + 1);
        let two = interior(sym, vec![
            named_leaf(sym, start, start + 1),
            named_leaf(sym, start, start + 1),
        ]);
        let src = source.as_bytes();
        let mut pp1 = PrettyPrintVisitor::new();
        TreeWalker::new(src).walk(&leaf, &mut pp1);
        let mut pp2 = PrettyPrintVisitor::new();
        TreeWalker::new(src).walk(&two, &mut pp2);
        prop_assert!(pp2.output().len() > pp1.output().len());
    }
}

// ============================================================================
// 8. Nested tree construction properties (5 properties)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v4_nested_chain_node_count_equals_depth(
        source in arb_source(),
        depth in 1usize..=8,
    ) {
        let chain_strat = arb_chain(depth);
        proptest::test_runner::TestRunner::default()
            .run(&chain_strat, |t| {
                let mut stats = StatsVisitor::default();
                TreeWalker::new(source.as_bytes()).walk(&t, &mut stats);
                prop_assert_eq!(stats.total_nodes, depth);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn v4_nested_wide_node_count(
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
    fn v4_nested_balanced_binary(source in arb_source()) {
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
    fn v4_nested_error_children_hidden(
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
    fn v4_nested_arena_branch_children_roundtrip(count in 1usize..=10) {
        let mut arena = TreeArena::new();
        let children: Vec<_> = (0..count)
            .map(|i| arena.alloc(TreeNode::leaf(i as i32)))
            .collect();
        let parent = arena.alloc(TreeNode::branch(children));
        let parent_ref = arena.get(parent);
        prop_assert!(parent_ref.is_branch());
        prop_assert_eq!(parent_ref.children().len(), count);
        for (i, &child_handle) in parent_ref.children().iter().enumerate() {
            prop_assert_eq!(arena.get(child_handle).value(), i as i32);
        }
    }
}

// ============================================================================
// 9. Edge cases (5+ properties)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn v4_edge_single_leaf_stats(
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
    fn v4_edge_mixed_error_and_normal(source in arb_source()) {
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
    fn v4_edge_all_errors_tree(source in arb_source()) {
        let children = vec![
            error_node(0, 1),
            error_node(1, 2),
            error_node(2, 3),
        ];
        let tree = unnamed_interior(5, children);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert_eq!(stats.total_nodes, 1);
        prop_assert_eq!(stats.error_nodes, 3);
        prop_assert_eq!(stats.leaf_nodes, 0);
    }

    #[test]
    fn v4_edge_unnamed_no_named_tag(
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
    fn v4_edge_bfs_stop_limits_count(
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
    fn v4_edge_fresh_stats_zeros(_x in 0u8..1) {
        let stats = StatsVisitor::default();
        prop_assert_eq!(stats.total_nodes, 0);
        prop_assert_eq!(stats.leaf_nodes, 0);
        prop_assert_eq!(stats.error_nodes, 0);
        prop_assert_eq!(stats.max_depth, 0);
        prop_assert!(stats.node_counts.is_empty());
    }

    #[test]
    fn v4_edge_arena_len_matches_alloc_count(count in 1usize..=50) {
        let mut arena = TreeArena::new();
        for i in 0..count {
            arena.alloc(TreeNode::leaf(i as i32));
        }
        prop_assert_eq!(arena.len(), count);
    }
}

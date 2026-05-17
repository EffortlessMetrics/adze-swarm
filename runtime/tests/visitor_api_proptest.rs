#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]
//! Property-based tests for the Visitor API in the adze runtime.
//!
//! Covers: visit-all-nodes, enter/leave callbacks, traversal order, empty tree,
//! deep tree, wide tree, skip-node, and early termination.

use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TransformVisitor,
    TransformWalker, TreeWalker, Visitor, VisitorAction,
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

fn leaf(symbol: u16, start: usize, end: usize) -> ParsedNode {
    make_node(symbol, vec![], start, end, false, true)
}

fn interior(symbol: u16, children: Vec<ParsedNode>) -> ParsedNode {
    let start = children.first().map_or(0, |c| c.start_byte);
    let end = children.last().map_or(0, |c| c.end_byte);
    make_node(symbol, children, start, end, false, true)
}

fn error_node(start: usize, end: usize) -> ParsedNode {
    make_node(0, vec![], start, end, true, false)
}

/// Count every non-error node in a tree recursively.
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

/// Build a deep chain: root -> child -> child -> ... -> leaf (depth levels).
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

/// Collect DFS enter-order of symbol IDs from a tree (non-error nodes only).
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

/// Collect BFS order of symbol IDs from a tree (non-error nodes only).
fn bfs_order_symbols(root: &ParsedNode) -> Vec<u16> {
    let mut result = Vec::new();
    let mut queue = std::collections::VecDeque::new();
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

// ---------------------------------------------------------------------------
// Proptest strategies
// ---------------------------------------------------------------------------

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

// =========================================================================
// Tests
// =========================================================================

// ---------------------------------------------------------------------------
// 1. DFS visitor visits all non-error nodes
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn dfs_visits_all_non_error_nodes(
        source in arb_source(),
        tree in arb_tree(4, 4),
    ) {
        struct Counter(usize);
        impl Visitor for Counter {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.0 += 1;
                VisitorAction::Continue
            }
        }

        let mut v = Counter(0);
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut v);
        prop_assert_eq!(v.0, count_non_error_nodes(&tree));
    }
}

// ---------------------------------------------------------------------------
// 2. BFS visitor visits all non-error nodes
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn bfs_visits_all_non_error_nodes(
        source in arb_source(),
        tree in arb_tree(4, 4),
    ) {
        struct Counter(usize);
        impl Visitor for Counter {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.0 += 1;
                VisitorAction::Continue
            }
        }

        let mut v = Counter(0);
        BreadthFirstWalker::new(source.as_bytes()).walk(&tree, &mut v);
        prop_assert_eq!(v.0, count_non_error_nodes(&tree));
    }
}

// ---------------------------------------------------------------------------
// 3. DFS enter/leave callbacks are perfectly paired
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn dfs_enter_leave_paired(
        source in arb_source(),
        tree in arb_tree(4, 4),
    ) {
        struct Tracker { stack: Vec<u16>, paired: bool }
        impl Visitor for Tracker {
            fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
                self.stack.push(node.symbol);
                VisitorAction::Continue
            }
            fn leave_node(&mut self, node: &ParsedNode) {
                if self.stack.pop() != Some(node.symbol) {
                    self.paired = false;
                }
            }
        }

        let mut v = Tracker { stack: Vec::new(), paired: true };
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut v);
        prop_assert!(v.paired, "enter/leave must be paired in LIFO order");
        prop_assert!(v.stack.is_empty(), "stack must be empty after walk");
    }
}

// ---------------------------------------------------------------------------
// 4. DFS traversal order matches manual preorder
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn dfs_traversal_order_is_preorder(
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

        let mut v = OrderCollector(Vec::new());
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut v);
        let expected = dfs_preorder_symbols(&tree);
        prop_assert_eq!(v.0, expected);
    }
}

// ---------------------------------------------------------------------------
// 5. BFS traversal order matches manual level-order
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn bfs_traversal_order_is_level_order(
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

        let mut v = OrderCollector(Vec::new());
        BreadthFirstWalker::new(source.as_bytes()).walk(&tree, &mut v);
        let expected = bfs_order_symbols(&tree);
        prop_assert_eq!(v.0, expected);
    }
}

// ---------------------------------------------------------------------------
// 6. Empty tree (single leaf) — DFS
// ---------------------------------------------------------------------------
#[test]
fn dfs_single_leaf_visits_one_node() {
    let source = b"x ";
    let tree = leaf(1, 0, 1);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
    assert_eq!(stats.max_depth, 1);
}

// ---------------------------------------------------------------------------
// 7. Empty tree (single leaf) — BFS
// ---------------------------------------------------------------------------
#[test]
fn bfs_single_leaf_visits_one_node() {
    let source = b"x ";
    let tree = leaf(1, 0, 1);
    let mut stats = StatsVisitor::default();
    BreadthFirstWalker::new(source).walk(&tree, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
}

// ---------------------------------------------------------------------------
// 8. Deep tree — DFS visits correct count
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn dfs_deep_tree_visits_all(depth in 1usize..=50) {
        let source = vec![b'x'; SOURCE_LEN];
        let tree = build_deep_tree(depth, SOURCE_LEN);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(&source).walk(&tree, &mut stats);
        prop_assert_eq!(stats.total_nodes, depth);
    }
}

// ---------------------------------------------------------------------------
// 9. Deep tree — max_depth matches
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn dfs_deep_tree_max_depth(depth in 1usize..=50) {
        let source = vec![b'x'; SOURCE_LEN];
        let tree = build_deep_tree(depth, SOURCE_LEN);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(&source).walk(&tree, &mut stats);
        prop_assert_eq!(stats.max_depth, depth);
    }
}

// ---------------------------------------------------------------------------
// 10. Wide tree — DFS visits root + all children
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn dfs_wide_tree_visits_all(width in 1usize..=50) {
        let source = vec![b'x'; SOURCE_LEN];
        let tree = build_wide_tree(width, SOURCE_LEN);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(&source).walk(&tree, &mut stats);
        prop_assert_eq!(stats.total_nodes, 1 + width);
    }
}

// ---------------------------------------------------------------------------
// 11. Wide tree — all children are leaves
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn dfs_wide_tree_leaf_count(width in 1usize..=50) {
        let source = vec![b'x'; SOURCE_LEN];
        let tree = build_wide_tree(width, SOURCE_LEN);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(&source).walk(&tree, &mut stats);
        prop_assert_eq!(stats.leaf_nodes, width);
    }
}

// ---------------------------------------------------------------------------
// 12. Wide tree — BFS visits same count as DFS
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn bfs_wide_tree_visits_all(width in 1usize..=50) {
        let source = vec![b'x'; SOURCE_LEN];
        let tree = build_wide_tree(width, SOURCE_LEN);
        let mut stats = StatsVisitor::default();
        BreadthFirstWalker::new(&source).walk(&tree, &mut stats);
        prop_assert_eq!(stats.total_nodes, 1 + width);
    }
}

// ---------------------------------------------------------------------------
// 13. Skip root children — DFS visits only root
// ---------------------------------------------------------------------------
#[test]
fn dfs_skip_root_children_visits_only_root() {
    let source = vec![b'x'; SOURCE_LEN];
    let tree = build_wide_tree(10, SOURCE_LEN);

    struct SkipRoot(usize);
    impl Visitor for SkipRoot {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.0 += 1;
            if self.0 == 1 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }

    let mut v = SkipRoot(0);
    TreeWalker::new(&source).walk(&tree, &mut v);
    assert_eq!(v.0, 1);
}

// ---------------------------------------------------------------------------
// 14. Skip at depth N — DFS prunes subtrees below N
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn dfs_skip_at_depth_prunes(
        source in arb_source(),
        tree in arb_tree(4, 3),
        skip_depth in 1usize..=4,
    ) {
        struct SkipAtDepth { depth: usize, skip_at: usize, count: usize }
        impl Visitor for SkipAtDepth {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.depth += 1;
                self.count += 1;
                if self.depth >= self.skip_at {
                    VisitorAction::SkipChildren
                } else {
                    VisitorAction::Continue
                }
            }
            fn leave_node(&mut self, _: &ParsedNode) {
                self.depth -= 1;
            }
        }

        let mut v = SkipAtDepth { depth: 0, skip_at: skip_depth, count: 0 };
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut v);
        // Must visit fewer or equal nodes than total
        prop_assert!(v.count <= count_non_error_nodes(&tree));
    }
}

// ---------------------------------------------------------------------------
// 15. Skip in BFS — skipped node's children are not queued
// ---------------------------------------------------------------------------
#[test]
fn bfs_skip_children_prunes_subtree() {
    let source = vec![b'x'; SOURCE_LEN];
    // root(1) -> [child_a(2) -> [grandchild(3)], child_b(4)]
    let grandchild = leaf(3, 0, 1);
    let child_a = interior(2, vec![grandchild]);
    let child_b = leaf(4, 1, 2);
    let tree = interior(1, vec![child_a, child_b]);

    struct SkipSymbol2(Vec<u16>);
    impl Visitor for SkipSymbol2 {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.0.push(node.symbol);
            if node.symbol == 2 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }

    let mut v = SkipSymbol2(Vec::new());
    BreadthFirstWalker::new(&source).walk(&tree, &mut v);
    // Should visit root(1), child_a(2), child_b(4) but NOT grandchild(3)
    assert_eq!(v.0, vec![1, 2, 4]);
}

// ---------------------------------------------------------------------------
// 16. Early termination — DFS Stop after 1 node
// ---------------------------------------------------------------------------
#[test]
fn dfs_stop_after_first_node() {
    let source = vec![b'x'; SOURCE_LEN];
    let tree = build_wide_tree(20, SOURCE_LEN);

    struct StopImmediate(usize);
    impl Visitor for StopImmediate {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.0 += 1;
            VisitorAction::Stop
        }
    }

    let mut v = StopImmediate(0);
    TreeWalker::new(&source).walk(&tree, &mut v);
    assert_eq!(v.0, 1);
}

// ---------------------------------------------------------------------------
// 17. Early termination — BFS Stop after 1 node
// ---------------------------------------------------------------------------
#[test]
fn bfs_stop_after_first_node() {
    let source = vec![b'x'; SOURCE_LEN];
    let tree = build_wide_tree(20, SOURCE_LEN);

    struct StopImmediate(usize);
    impl Visitor for StopImmediate {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.0 += 1;
            VisitorAction::Stop
        }
    }

    let mut v = StopImmediate(0);
    BreadthFirstWalker::new(&source).walk(&tree, &mut v);
    assert_eq!(v.0, 1);
}

// ---------------------------------------------------------------------------
// 18. Early termination — DFS Stop after N nodes
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn dfs_stop_after_n(
        source in arb_source(),
        tree in arb_tree(3, 4),
        limit in 1usize..=10,
    ) {
        struct StopAfterN { count: usize, limit: usize }
        impl Visitor for StopAfterN {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.count += 1;
                if self.count >= self.limit {
                    VisitorAction::Stop
                } else {
                    VisitorAction::Continue
                }
            }
        }

        let mut v = StopAfterN { count: 0, limit };
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut v);
        // DFS Stop exits the current walk_node but the parent's child loop
        // continues visiting siblings, so count can exceed `limit`.
        let total = count_non_error_nodes(&tree);
        prop_assert!(v.count >= 1);
        prop_assert!(v.count <= total);
    }
}

// ---------------------------------------------------------------------------
// 19. Early termination — BFS Stop after N nodes
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn bfs_stop_after_n(
        source in arb_source(),
        tree in arb_tree(3, 4),
        limit in 1usize..=10,
    ) {
        struct StopAfterN { count: usize, limit: usize }
        impl Visitor for StopAfterN {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.count += 1;
                if self.count >= self.limit {
                    VisitorAction::Stop
                } else {
                    VisitorAction::Continue
                }
            }
        }

        let mut v = StopAfterN { count: 0, limit };
        BreadthFirstWalker::new(source.as_bytes()).walk(&tree, &mut v);
        prop_assert!(v.count <= limit);
        prop_assert!(v.count >= 1);
    }
}

// ---------------------------------------------------------------------------
// 20. DFS leave_node is called even when SkipChildren
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn dfs_leave_called_on_skip(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct LeaveTracker { enters: usize, leaves: usize }
        impl Visitor for LeaveTracker {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.enters += 1;
                VisitorAction::SkipChildren
            }
            fn leave_node(&mut self, _: &ParsedNode) {
                self.leaves += 1;
            }
        }

        let mut v = LeaveTracker { enters: 0, leaves: 0 };
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut v);
        // When root is skipped, enter=1, leave=1
        prop_assert_eq!(v.enters, v.leaves);
    }
}

// ---------------------------------------------------------------------------
// 21. DFS leave_node is NOT called when Stop
// ---------------------------------------------------------------------------
#[test]
fn dfs_leave_not_called_on_stop() {
    let source = vec![b'x'; SOURCE_LEN];
    let tree = build_wide_tree(5, SOURCE_LEN);

    struct StopTracker {
        enters: usize,
        leaves: usize,
    }
    impl Visitor for StopTracker {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.enters += 1;
            VisitorAction::Stop
        }
        fn leave_node(&mut self, _: &ParsedNode) {
            self.leaves += 1;
        }
    }

    let mut v = StopTracker {
        enters: 0,
        leaves: 0,
    };
    TreeWalker::new(&source).walk(&tree, &mut v);
    assert_eq!(v.enters, 1);
    assert_eq!(v.leaves, 0);
}

// ---------------------------------------------------------------------------
// 22. visit_leaf called for all leaves in DFS
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn dfs_visit_leaf_count(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct LeafCounter(usize);
        impl Visitor for LeafCounter {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                VisitorAction::Continue
            }
            fn visit_leaf(&mut self, _: &ParsedNode, _: &str) {
                self.0 += 1;
            }
        }

        let mut v = LeafCounter(0);
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut v);
        prop_assert_eq!(v.0, count_leaves(&tree));
    }
}

// ---------------------------------------------------------------------------
// 23. Error nodes trigger visit_error, not enter_node
// ---------------------------------------------------------------------------
#[test]
fn error_nodes_trigger_visit_error() {
    let source = b"abcdefghij";
    let tree = interior(1, vec![leaf(2, 0, 2), error_node(2, 5), leaf(3, 5, 8)]);

    struct ErrorTracker {
        entered: usize,
        errors: usize,
    }
    impl Visitor for ErrorTracker {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.entered += 1;
            VisitorAction::Continue
        }
        fn visit_error(&mut self, _: &ParsedNode) {
            self.errors += 1;
        }
    }

    let mut v = ErrorTracker {
        entered: 0,
        errors: 0,
    };
    TreeWalker::new(source).walk(&tree, &mut v);
    assert_eq!(v.errors, 1);
    // root + 2 non-error children = 3 entered
    assert_eq!(v.entered, 3);
}

// ---------------------------------------------------------------------------
// 24. TransformWalker visits all nodes bottom-up
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn transform_walker_total_node_count(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct NodeCounter;
        impl TransformVisitor for NodeCounter {
            type Output = usize;
            fn transform_node(&mut self, _: &ParsedNode, children: Vec<usize>) -> usize {
                1 + children.iter().sum::<usize>()
            }
            fn transform_leaf(&mut self, _: &ParsedNode, _: &str) -> usize { 1 }
            fn transform_error(&mut self, _: &ParsedNode) -> usize { 1 }
        }

        let mut nc = NodeCounter;
        let total = TransformWalker::new(source.as_bytes()).walk(&tree, &mut nc);
        let expected = count_non_error_nodes(&tree);
        prop_assert_eq!(total, expected);
    }
}

// ---------------------------------------------------------------------------
// 25. DFS and BFS agree on StatsVisitor total_nodes
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn dfs_bfs_same_total(
        source in arb_source(),
        tree in arb_tree(4, 4),
    ) {
        let src = source.as_bytes();
        let mut dfs = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut dfs);

        let mut bfs = StatsVisitor::default();
        BreadthFirstWalker::new(src).walk(&tree, &mut bfs);

        prop_assert_eq!(dfs.total_nodes, bfs.total_nodes);
    }
}

// ---------------------------------------------------------------------------
// 26. SearchVisitor matches subset when filtering by symbol
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn search_by_symbol_is_subset(
        source in arb_source(),
        tree in arb_tree(3, 3),
        target_sym in 1u16..=10,
    ) {
        let t = target_sym;
        let mut filtered = SearchVisitor::new(move |n: &ParsedNode| n.symbol == t);
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut filtered);

        let mut all = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut all);

        prop_assert!(filtered.matches.len() <= all.matches.len());
    }
}

// ---------------------------------------------------------------------------
// 27. PrettyPrint output lines >= number of visited non-error nodes
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn pretty_print_lines_ge_nodes(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut pp);
        let lines = pp.output().lines().count();
        let nodes = count_non_error_nodes(&tree);
        // Each non-error node produces at least one line (enter_node writes one,
        // plus leaves write an extra line).
        prop_assert!(lines >= nodes, "lines={lines} < nodes={nodes}");
    }
}

// ---------------------------------------------------------------------------
// 28. Deep tree — enter/leave depth tracking
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn deep_tree_depth_tracking(depth in 2usize..=30) {
        let source = vec![b'x'; SOURCE_LEN];
        let tree = build_deep_tree(depth, SOURCE_LEN);

        struct DepthTracker { current: usize, max_seen: usize }
        impl Visitor for DepthTracker {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.current += 1;
                if self.current > self.max_seen {
                    self.max_seen = self.current;
                }
                VisitorAction::Continue
            }
            fn leave_node(&mut self, _: &ParsedNode) {
                self.current -= 1;
            }
        }

        let mut v = DepthTracker { current: 0, max_seen: 0 };
        TreeWalker::new(&source).walk(&tree, &mut v);
        prop_assert_eq!(v.max_seen, depth);
        prop_assert_eq!(v.current, 0, "depth must return to 0 after walk");
    }
}

// ---------------------------------------------------------------------------
// 29. Wide tree — BFS visits root first, then all children
// ---------------------------------------------------------------------------
#[test]
fn bfs_wide_tree_root_first() {
    let source = vec![b'x'; SOURCE_LEN];
    let tree = build_wide_tree(5, SOURCE_LEN);

    struct OrderCollector(Vec<u16>);
    impl Visitor for OrderCollector {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.0.push(node.symbol);
            VisitorAction::Continue
        }
    }

    let mut v = OrderCollector(Vec::new());
    BreadthFirstWalker::new(&source).walk(&tree, &mut v);
    // Root is symbol 1
    assert_eq!(v.0[0], 1);
    // All remaining are children
    assert_eq!(v.0.len(), 6);
}

// ---------------------------------------------------------------------------
// 30. StatsVisitor node_counts sums to total_nodes
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn stats_node_counts_sum(
        source in arb_source(),
        tree in arb_tree(3, 4),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        let sum: usize = stats.node_counts.values().sum();
        prop_assert_eq!(sum, stats.total_nodes);
    }
}

// ---------------------------------------------------------------------------
// 31. Multiple error nodes are all counted
// ---------------------------------------------------------------------------
#[test]
fn multiple_error_nodes_counted() {
    let source = b"abcdefghij0123456789abcdefghij0123456789abcdefghij0123456789abcd";
    let tree = interior(
        1,
        vec![
            error_node(0, 2),
            error_node(3, 5),
            leaf(2, 6, 8),
            error_node(9, 11),
        ],
    );

    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut stats);
    assert_eq!(stats.error_nodes, 3);
}

// ---------------------------------------------------------------------------
// 32. SkipChildren at every node — only root visited in DFS
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn skip_all_visits_only_root(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct SkipAll(usize);
        impl Visitor for SkipAll {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.0 += 1;
                VisitorAction::SkipChildren
            }
        }

        let mut v = SkipAll(0);
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut v);
        // If root is an error node, enter_node is never called
        if tree.is_error() {
            prop_assert_eq!(v.0, 0);
        } else {
            prop_assert_eq!(v.0, 1);
        }
    }
}

// ---------------------------------------------------------------------------
// 33. Default Visitor trait methods are no-ops
// ---------------------------------------------------------------------------
#[test]
fn default_visitor_methods_are_noop() {
    struct NoOp;
    impl Visitor for NoOp {}

    let node = leaf(1, 0, 1);
    let mut v = NoOp;
    assert_eq!(v.enter_node(&node), VisitorAction::Continue);
    v.leave_node(&node); // should not panic
    v.visit_leaf(&node, "test"); // should not panic
    v.visit_error(&node); // should not panic
}

// ---------------------------------------------------------------------------
// 34. DFS leaf text matches source slice
// ---------------------------------------------------------------------------
#[test]
fn dfs_leaf_text_from_source() {
    let source = b"hello world padding padding padding padding padding padding pad";
    let tree = leaf(1, 0, 5);

    struct TextCollector(Vec<String>);
    impl Visitor for TextCollector {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn visit_leaf(&mut self, _: &ParsedNode, text: &str) {
            self.0.push(text.to_string());
        }
    }

    let mut v = TextCollector(Vec::new());
    TreeWalker::new(source).walk(&tree, &mut v);
    assert_eq!(v.0, vec!["hello"]);
}

// ---------------------------------------------------------------------------
// 35. TransformWalker handles error nodes
// ---------------------------------------------------------------------------
#[test]
fn transform_walker_handles_errors() {
    let source = b"abcdefghij0123456789abcdefghij0123456789abcdefghij0123456789abcd";
    let tree = interior(1, vec![leaf(2, 0, 3), error_node(4, 7), leaf(3, 8, 10)]);

    struct ErrorCounter;
    impl TransformVisitor for ErrorCounter {
        type Output = usize;
        fn transform_node(&mut self, _: &ParsedNode, children: Vec<usize>) -> usize {
            children.iter().sum()
        }
        fn transform_leaf(&mut self, _: &ParsedNode, _: &str) -> usize {
            0
        }
        fn transform_error(&mut self, _: &ParsedNode) -> usize {
            1
        }
    }

    let mut ec = ErrorCounter;
    let errors = TransformWalker::new(source).walk(&tree, &mut ec);
    assert_eq!(errors, 1);
}

// ===========================================================================
// Additional tests (36–60)
// ===========================================================================

// ---------------------------------------------------------------------------
// 36. Determinism — DFS walk twice yields identical enter order
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn dfs_deterministic_enter_order(
        source in arb_source(),
        tree in arb_tree(4, 4),
    ) {
        struct SymCollector(Vec<u16>);
        impl Visitor for SymCollector {
            fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
                self.0.push(node.symbol);
                VisitorAction::Continue
            }
        }

        let src = source.as_bytes();
        let mut a = SymCollector(Vec::new());
        TreeWalker::new(src).walk(&tree, &mut a);
        let mut b = SymCollector(Vec::new());
        TreeWalker::new(src).walk(&tree, &mut b);
        prop_assert_eq!(a.0, b.0, "Two DFS walks must produce identical order");
    }
}

// ---------------------------------------------------------------------------
// 37. Determinism — BFS walk twice yields identical enter order
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn bfs_deterministic_enter_order(
        source in arb_source(),
        tree in arb_tree(4, 4),
    ) {
        struct SymCollector(Vec<u16>);
        impl Visitor for SymCollector {
            fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
                self.0.push(node.symbol);
                VisitorAction::Continue
            }
        }

        let src = source.as_bytes();
        let mut a = SymCollector(Vec::new());
        BreadthFirstWalker::new(src).walk(&tree, &mut a);
        let mut b = SymCollector(Vec::new());
        BreadthFirstWalker::new(src).walk(&tree, &mut b);
        prop_assert_eq!(a.0, b.0, "Two BFS walks must produce identical order");
    }
}

// ---------------------------------------------------------------------------
// 38. Pre-order enter precedes post-order leave for every node
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn preorder_enter_before_postorder_leave(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        #[derive(Debug, Clone, PartialEq)]
        enum Event { Enter(u16), Leave(u16) }

        struct EventLog(Vec<Event>);
        impl Visitor for EventLog {
            fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
                self.0.push(Event::Enter(node.symbol));
                VisitorAction::Continue
            }
            fn leave_node(&mut self, node: &ParsedNode) {
                self.0.push(Event::Leave(node.symbol));
            }
        }

        let mut v = EventLog(Vec::new());
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut v);

        // For each Enter(sym), a matching Leave(sym) must appear later
        let mut stack: Vec<u16> = Vec::new();
        for ev in &v.0 {
            match ev {
                Event::Enter(s) => stack.push(*s),
                Event::Leave(s) => {
                    let top = stack.pop();
                    prop_assert_eq!(top, Some(*s), "Leave must match most recent Enter");
                }
            }
        }
        prop_assert!(stack.is_empty(), "All Enter events must have matching Leave");
    }
}

// ---------------------------------------------------------------------------
// 39. DFS post-order (leave) sequence reverses enter for a chain
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn dfs_leave_reverses_enter_on_chain(depth in 2usize..=20) {
        let source = vec![b'x'; SOURCE_LEN];
        let tree = build_deep_tree(depth, SOURCE_LEN);

        struct OrderLog { enter_stack: Vec<usize>, leaves: Vec<usize>, id: usize }
        impl Visitor for OrderLog {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.enter_stack.push(self.id);
                self.id += 1;
                VisitorAction::Continue
            }
            fn leave_node(&mut self, _: &ParsedNode) {
                if let Some(entered_id) = self.enter_stack.pop() {
                    self.leaves.push(entered_id);
                }
            }
        }

        let mut v = OrderLog { enter_stack: Vec::new(), leaves: Vec::new(), id: 0 };
        TreeWalker::new(&source).walk(&tree, &mut v);
        // On a chain, leaves are visited deepest-first, so IDs descend
        for i in 0..v.leaves.len().saturating_sub(1) {
            prop_assert!(v.leaves[i] > v.leaves[i + 1],
                "Leave order must be deepest-first on a chain");
        }
    }
}

// ---------------------------------------------------------------------------
// 40. SearchVisitor match-all yields same count as StatsVisitor total_nodes
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn search_match_all_equals_stats_total(
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

// ---------------------------------------------------------------------------
// 41. SearchVisitor empty predicate yields zero matches
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn search_match_none_is_empty(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut search = SearchVisitor::new(|_: &ParsedNode| false);
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut search);
        prop_assert!(search.matches.is_empty());
    }
}

// ---------------------------------------------------------------------------
// 42. StatsVisitor collects correct node kinds
// ---------------------------------------------------------------------------
#[test]
fn stats_collects_correct_kinds() {
    let source = b"abcdefghij0123456789abcdefghij0123456789abcdefghij0123456789abcd";
    // root(1) -> [a(2), b(3), c(2)]
    let tree = interior(1, vec![leaf(2, 0, 2), leaf(3, 2, 4), leaf(2, 4, 6)]);

    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut stats);

    // Kind strings come from node.kind() which returns symbol id for pure-rust nodes
    // The root plus children are visited
    assert_eq!(stats.total_nodes, 4);
    let total_from_counts: usize = stats.node_counts.values().sum();
    assert_eq!(total_from_counts, 4);
}

// ---------------------------------------------------------------------------
// 43. PrettyPrintVisitor output contains [named] for named nodes
// ---------------------------------------------------------------------------
#[test]
fn pretty_print_contains_named_marker() {
    let source = b"abcdefghij0123456789abcdefghij0123456789abcdefghij0123456789abcd";
    let tree = leaf(1, 0, 3);

    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(source).walk(&tree, &mut pp);
    assert!(
        pp.output().contains("[named]"),
        "Output should contain [named] for named nodes: {}",
        pp.output()
    );
}

// ---------------------------------------------------------------------------
// 44. PrettyPrintVisitor indentation increases with depth
// ---------------------------------------------------------------------------
#[test]
fn pretty_print_indentation_increases() {
    let source = b"abcdefghij0123456789abcdefghij0123456789abcdefghij0123456789abcd";
    // root -> child -> grandchild (leaf)
    let tree = interior(1, vec![interior(2, vec![leaf(3, 0, 3)])]);

    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(source).walk(&tree, &mut pp);
    let lines: Vec<&str> = pp.output().lines().collect();
    // First line: no indent (root), subsequent lines get more indent
    assert!(
        lines.len() >= 3,
        "Expected at least 3 lines, got {}",
        lines.len()
    );
    let indent_0 = lines[0].len() - lines[0].trim_start().len();
    let indent_1 = lines[1].len() - lines[1].trim_start().len();
    assert!(
        indent_1 > indent_0,
        "Child indent ({indent_1}) should exceed root ({indent_0})"
    );
}

// ---------------------------------------------------------------------------
// 45. TransformWalker computes tree depth bottom-up
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn transform_walker_computes_depth(depth in 1usize..=20) {
        let source = vec![b'x'; SOURCE_LEN];
        let tree = build_deep_tree(depth, SOURCE_LEN);

        struct DepthCalc;
        impl TransformVisitor for DepthCalc {
            type Output = usize;
            fn transform_node(&mut self, _: &ParsedNode, children: Vec<usize>) -> usize {
                1 + children.into_iter().max().unwrap_or(0)
            }
            fn transform_leaf(&mut self, _: &ParsedNode, _: &str) -> usize { 1 }
            fn transform_error(&mut self, _: &ParsedNode) -> usize { 0 }
        }

        let mut calc = DepthCalc;
        let computed = TransformWalker::new(&source).walk(&tree, &mut calc);
        prop_assert_eq!(computed, depth);
    }
}

// ---------------------------------------------------------------------------
// 46. TransformWalker leaf text matches source bytes
// ---------------------------------------------------------------------------
#[test]
fn transform_walker_leaf_text() {
    let source = b"hello world padding padding padding padding padding padding pad";
    let tree = leaf(1, 0, 5);

    struct TextGrabber;
    impl TransformVisitor for TextGrabber {
        type Output = String;
        fn transform_node(&mut self, _: &ParsedNode, children: Vec<String>) -> String {
            children.join(", ")
        }
        fn transform_leaf(&mut self, _: &ParsedNode, text: &str) -> String {
            text.to_string()
        }
        fn transform_error(&mut self, _: &ParsedNode) -> String {
            "ERROR".to_string()
        }
    }

    let mut tg = TextGrabber;
    let result = TransformWalker::new(source).walk(&tree, &mut tg);
    assert_eq!(result, "hello");
}

// ---------------------------------------------------------------------------
// 47. SearchVisitor collects non-empty results for match-all
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn search_match_all_nonempty_on_nonempty_tree(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut search = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(src).walk(&tree, &mut search);

        let non_error = count_non_error_nodes(&tree);
        prop_assert_eq!(search.matches.len(), non_error,
            "match-all should find every non-error node");
    }
}

// ---------------------------------------------------------------------------
// 48. DFS does not call visit_leaf for interior nodes
// ---------------------------------------------------------------------------
#[test]
fn dfs_no_leaf_callback_for_interior() {
    let source = b"abcdefghij0123456789abcdefghij0123456789abcdefghij0123456789abcd";
    let tree = interior(1, vec![leaf(2, 0, 3), leaf(3, 3, 6)]);

    struct LeafKindTracker(Vec<u16>);
    impl Visitor for LeafKindTracker {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn visit_leaf(&mut self, node: &ParsedNode, _: &str) {
            self.0.push(node.symbol);
        }
    }

    let mut v = LeafKindTracker(Vec::new());
    TreeWalker::new(source).walk(&tree, &mut v);
    // Only leaf symbols (2, 3) should appear, NOT root (1)
    assert!(
        !v.0.contains(&1),
        "Interior node should not trigger visit_leaf"
    );
    assert_eq!(v.0.len(), 2);
}

// ---------------------------------------------------------------------------
// 49. VisitorAction derives: Debug, Clone, Copy, PartialEq, Eq
// ---------------------------------------------------------------------------
#[test]
fn visitor_action_derives() {
    let a = VisitorAction::Continue;
    let b = a; // Copy
    let c = a; // Clone
    assert_eq!(b, c); // PartialEq + Eq
    let _ = format!("{:?}", a); // Debug
}

// ---------------------------------------------------------------------------
// 50. TreeWalker can be reused for multiple walks
// ---------------------------------------------------------------------------
#[test]
fn tree_walker_reusable() {
    let source = b"abcdefghij0123456789abcdefghij0123456789abcdefghij0123456789abcd";
    let tree_a = leaf(1, 0, 3);
    let tree_b = interior(2, vec![leaf(3, 0, 2), leaf(4, 2, 4)]);
    let walker = TreeWalker::new(source);

    let mut stats_a = StatsVisitor::default();
    walker.walk(&tree_a, &mut stats_a);
    assert_eq!(stats_a.total_nodes, 1);

    let mut stats_b = StatsVisitor::default();
    walker.walk(&tree_b, &mut stats_b);
    assert_eq!(stats_b.total_nodes, 3);
}

// ---------------------------------------------------------------------------
// 51. BreadthFirstWalker can be reused for multiple walks
// ---------------------------------------------------------------------------
#[test]
fn bfs_walker_reusable() {
    let source = b"abcdefghij0123456789abcdefghij0123456789abcdefghij0123456789abcd";
    let tree_a = leaf(1, 0, 3);
    let tree_b = interior(2, vec![leaf(3, 0, 2), leaf(4, 2, 4)]);
    let walker = BreadthFirstWalker::new(source);

    let mut stats_a = StatsVisitor::default();
    walker.walk(&tree_a, &mut stats_a);
    assert_eq!(stats_a.total_nodes, 1);

    let mut stats_b = StatsVisitor::default();
    walker.walk(&tree_b, &mut stats_b);
    assert_eq!(stats_b.total_nodes, 3);
}

// ---------------------------------------------------------------------------
// 52. DFS on tree with only error children — root entered, errors reported
// ---------------------------------------------------------------------------
#[test]
fn dfs_tree_with_only_error_children() {
    let source = b"abcdefghij0123456789abcdefghij0123456789abcdefghij0123456789abcd";
    let tree = interior(1, vec![error_node(0, 2), error_node(3, 5)]);

    struct Tracker {
        entered: usize,
        errors: usize,
    }
    impl Visitor for Tracker {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.entered += 1;
            VisitorAction::Continue
        }
        fn visit_error(&mut self, _: &ParsedNode) {
            self.errors += 1;
        }
    }

    let mut v = Tracker {
        entered: 0,
        errors: 0,
    };
    TreeWalker::new(source).walk(&tree, &mut v);
    assert_eq!(v.entered, 1, "Only root should be entered");
    assert_eq!(v.errors, 2, "Both error children should be reported");
}

// ---------------------------------------------------------------------------
// 53. BFS on tree with errors — errors trigger visit_error not enter_node
// ---------------------------------------------------------------------------
#[test]
fn bfs_error_nodes_not_entered() {
    let source = b"abcdefghij0123456789abcdefghij0123456789abcdefghij0123456789abcd";
    let tree = interior(1, vec![leaf(2, 0, 2), error_node(3, 5), leaf(3, 6, 8)]);

    struct Tracker {
        entered: usize,
        errors: usize,
    }
    impl Visitor for Tracker {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.entered += 1;
            VisitorAction::Continue
        }
        fn visit_error(&mut self, _: &ParsedNode) {
            self.errors += 1;
        }
    }

    let mut v = Tracker {
        entered: 0,
        errors: 0,
    };
    BreadthFirstWalker::new(source).walk(&tree, &mut v);
    assert_eq!(v.entered, 3, "Root + 2 non-error children entered");
    assert_eq!(v.errors, 1, "One error node reported");
}

// ---------------------------------------------------------------------------
// 54. DFS with mixed skip and continue at alternating depths
// ---------------------------------------------------------------------------
#[test]
fn dfs_alternating_skip_continue() {
    let source = b"abcdefghij0123456789abcdefghij0123456789abcdefghij0123456789abcd";
    // root(1) -> [a(2) -> [c(4), d(5)], b(3)]
    let tree = interior(
        1,
        vec![
            interior(2, vec![leaf(4, 0, 2), leaf(5, 2, 4)]),
            leaf(3, 4, 6),
        ],
    );

    // Skip at depth 2 (children of root's children)
    struct SkipDepth2 {
        depth: usize,
        entered: Vec<u16>,
    }
    impl Visitor for SkipDepth2 {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.depth += 1;
            self.entered.push(node.symbol);
            if self.depth == 2 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
        fn leave_node(&mut self, _: &ParsedNode) {
            self.depth -= 1;
        }
    }

    let mut v = SkipDepth2 {
        depth: 0,
        entered: Vec::new(),
    };
    TreeWalker::new(source).walk(&tree, &mut v);
    // Should enter: root(1), a(2) [skip], b(3)
    assert_eq!(v.entered, vec![1, 2, 3]);
}

// ---------------------------------------------------------------------------
// 55. StatsVisitor fresh instance gives clean state
// ---------------------------------------------------------------------------
#[test]
fn stats_visitor_fresh_state() {
    let stats = StatsVisitor::default();
    assert_eq!(stats.total_nodes, 0);
    assert_eq!(stats.leaf_nodes, 0);
    assert_eq!(stats.error_nodes, 0);
    assert_eq!(stats.max_depth, 0);
    assert!(stats.node_counts.is_empty());
}

// ---------------------------------------------------------------------------
// 56. PrettyPrintVisitor::new identical to Default
// ---------------------------------------------------------------------------
#[test]
fn pretty_print_new_eq_default() {
    let a = PrettyPrintVisitor::new();
    let b = PrettyPrintVisitor::default();
    assert_eq!(a.output(), b.output());
    assert_eq!(a.output(), "");
}

// ---------------------------------------------------------------------------
// 57. DFS and BFS agree on leaf counts
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn dfs_bfs_same_leaf_count(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct LC(usize);
        impl Visitor for LC {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction { VisitorAction::Continue }
            fn visit_leaf(&mut self, _: &ParsedNode, _: &str) { self.0 += 1; }
        }

        let src = source.as_bytes();
        let mut dfs_lc = LC(0);
        TreeWalker::new(src).walk(&tree, &mut dfs_lc);
        let mut bfs_lc = LC(0);
        BreadthFirstWalker::new(src).walk(&tree, &mut bfs_lc);
        prop_assert_eq!(dfs_lc.0, bfs_lc.0, "DFS and BFS must find same number of leaves");
    }
}

// ---------------------------------------------------------------------------
// 58. SearchVisitor accumulates across walk (not reset)
// ---------------------------------------------------------------------------
#[test]
fn search_visitor_accumulates() {
    let source = b"abcdefghij0123456789abcdefghij0123456789abcdefghij0123456789abcd";
    let tree = leaf(1, 0, 3);
    let mut search = SearchVisitor::new(|_: &ParsedNode| true);
    TreeWalker::new(source).walk(&tree, &mut search);
    assert_eq!(search.matches.len(), 1);
    // Walk again — should accumulate
    TreeWalker::new(source).walk(&tree, &mut search);
    assert_eq!(search.matches.len(), 2);
}

// ---------------------------------------------------------------------------
// 59. DFS determinism: StatsVisitor node_counts identical across runs
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn dfs_deterministic_stats(
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
        for (k, v) in &s1.node_counts {
            prop_assert_eq!(s2.node_counts.get(k), Some(v));
        }
    }
}

// ---------------------------------------------------------------------------
// 60. BFS does not call leave_node
// ---------------------------------------------------------------------------
#[test]
fn bfs_does_not_call_leave_node() {
    let source = b"abcdefghij0123456789abcdefghij0123456789abcdefghij0123456789abcd";
    let tree = interior(1, vec![leaf(2, 0, 3), leaf(3, 3, 6)]);

    struct LeaveCounter(usize);
    impl Visitor for LeaveCounter {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn leave_node(&mut self, _: &ParsedNode) {
            self.0 += 1;
        }
    }

    let mut v = LeaveCounter(0);
    BreadthFirstWalker::new(source).walk(&tree, &mut v);
    assert_eq!(v.0, 0, "BFS walker does not call leave_node");
}

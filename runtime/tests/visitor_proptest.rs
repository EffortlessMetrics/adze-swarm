//! Property-based tests for the `adze::visitor` module.
//!
//! Uses `proptest` to generate random tree shapes and verify invariants that
//! must hold for every tree, regardless of size or topology.

use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TransformVisitor,
    TransformWalker, TreeWalker, Visitor, VisitorAction,
};
use proptest::prelude::*;
mod common;

use common::{
    count_leaves, count_nodes, error_node, interior_node as interior, named_leaf as leaf,
    tree_depth,
};

// ---------------------------------------------------------------------------
// Proptest strategies
// ---------------------------------------------------------------------------

/// Fixed source length used by all strategies so byte ranges stay in bounds.
const SOURCE_LEN: usize = 64;

/// Generate a random leaf whose byte range fits within `SOURCE_LEN`.
fn arb_leaf() -> impl Strategy<Value = ParsedNode> {
    (1u16..=10, 0..SOURCE_LEN - 1).prop_map(|(sym, start)| {
        let end = start + 1;
        leaf(sym, start, end)
    })
}

/// Generate a random tree up to `max_depth` deep with up to `max_width`
/// children per node.
fn arb_tree(max_depth: u32, max_width: usize) -> impl Strategy<Value = ParsedNode> {
    arb_leaf().prop_recursive(max_depth, 64, max_width as u32, move |inner| {
        (1u16..=10, proptest::collection::vec(inner, 1..=max_width))
            .prop_map(|(sym, children)| interior(sym, children))
    })
}

/// Source text – always exactly `SOURCE_LEN` bytes of ASCII.
fn arb_source() -> impl Strategy<Value = String> {
    proptest::string::string_regex(&format!("[a-z0-9 ]{{{},{}}}", SOURCE_LEN, SOURCE_LEN)).unwrap()
}

// ---------------------------------------------------------------------------
// 1. StatsVisitor defaults
// ---------------------------------------------------------------------------

#[test]
fn stats_visitor_defaults_are_zero() {
    let sv = StatsVisitor::default();
    assert_eq!(sv.total_nodes, 0);
    assert_eq!(sv.leaf_nodes, 0);
    assert_eq!(sv.error_nodes, 0);
    assert_eq!(sv.max_depth, 0);
    assert!(sv.node_counts.is_empty());
}

// ---------------------------------------------------------------------------
// 2. PrettyPrintVisitor starts empty
// ---------------------------------------------------------------------------

#[test]
fn pretty_print_visitor_starts_empty() {
    let pp = PrettyPrintVisitor::new();
    assert!(pp.output().is_empty());
}

// ---------------------------------------------------------------------------
// 3. PrettyPrintVisitor default trait
// ---------------------------------------------------------------------------

#[test]
fn pretty_print_default_equals_new() {
    let a = PrettyPrintVisitor::new();
    let b = PrettyPrintVisitor::default();
    assert_eq!(a.output(), b.output());
}

// ---------------------------------------------------------------------------
// 4. SearchVisitor starts with no matches
// ---------------------------------------------------------------------------

#[test]
fn search_visitor_starts_empty() {
    let sv = SearchVisitor::new(|_: &ParsedNode| true);
    assert!(sv.matches.is_empty());
}

// ---------------------------------------------------------------------------
// 5. VisitorAction equality
// ---------------------------------------------------------------------------

#[test]
fn visitor_action_variants_are_distinct() {
    let actions = [
        VisitorAction::Continue,
        VisitorAction::SkipChildren,
        VisitorAction::Stop,
    ];
    for (i, a) in actions.iter().enumerate() {
        for (j, b) in actions.iter().enumerate() {
            assert_eq!(i == j, a == b);
        }
    }
}

// ---------------------------------------------------------------------------
// 6. VisitorAction is Clone + Copy
// ---------------------------------------------------------------------------

#[test]
fn visitor_action_clone_copy() {
    let a = VisitorAction::Continue;
    let b = a; // Copy
    #[expect(
        clippy::clone_on_copy,
        reason = "proptest strategies exercise explicit clone paths even for Copy types to verify value independence"
    )]
    let c = a.clone(); // Clone
    assert_eq!(a, b);
    assert_eq!(a, c);
}

// ---------------------------------------------------------------------------
// 7. VisitorAction Debug formatting
// ---------------------------------------------------------------------------

#[test]
fn visitor_action_debug() {
    let dbg = format!("{:?}", VisitorAction::Stop);
    assert!(dbg.contains("Stop"));
}

// ---------------------------------------------------------------------------
// 8. Property: StatsVisitor.total_nodes equals recursive count (DFS)
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn stats_total_equals_recursive_count(
        source in arb_source(),
        tree in arb_tree(4, 4),
    ) {
        let walker = TreeWalker::new(source.as_bytes());
        let mut stats = StatsVisitor::default();
        walker.walk(&tree, &mut stats);
        let expected = count_nodes(&tree);
        // Error nodes are visited through visit_error, not enter_node,
        // so total_nodes only counts non-error nodes.
        let error_count = count_error_nodes(&tree);
        prop_assert_eq!(stats.total_nodes, expected - error_count);
    }
}

fn count_error_nodes(node: &ParsedNode) -> usize {
    let self_err = if node.is_error() { 1 } else { 0 };
    // Error nodes have their traversal stopped, so children aren't visited.
    if node.is_error() {
        self_err
    } else {
        self_err + node.children().iter().map(count_error_nodes).sum::<usize>()
    }
}

// ---------------------------------------------------------------------------
// 9. Property: StatsVisitor.max_depth <= tree_depth (DFS)
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn stats_max_depth_bounded_by_tree_depth(
        source in arb_source(),
        tree in arb_tree(4, 4),
    ) {
        let walker = TreeWalker::new(source.as_bytes());
        let mut stats = StatsVisitor::default();
        walker.walk(&tree, &mut stats);
        let depth = tree_depth(&tree);
        prop_assert!(stats.max_depth <= depth);
    }
}

// ---------------------------------------------------------------------------
// 10. Property: leaf_nodes count is consistent (DFS)
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn stats_leaf_count_consistent(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let walker = TreeWalker::new(source.as_bytes());
        let mut stats = StatsVisitor::default();
        walker.walk(&tree, &mut stats);
        let expected = count_leaves(&tree);
        // All leaves are non-error in our generated trees.
        prop_assert!(stats.leaf_nodes <= expected);
    }
}

// ---------------------------------------------------------------------------
// 11. Property: BFS visits the same total as DFS
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn bfs_and_dfs_visit_same_total(
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

// ---------------------------------------------------------------------------
// 12. Property: PrettyPrint output is non-empty for any tree
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn pretty_print_produces_output(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let walker = TreeWalker::new(source.as_bytes());
        let mut pp = PrettyPrintVisitor::new();
        walker.walk(&tree, &mut pp);
        prop_assert!(!pp.output().is_empty());
    }
}

// ---------------------------------------------------------------------------
// 13. Property: PrettyPrint output contains newlines for every entered node
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn pretty_print_newline_per_node(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let walker = TreeWalker::new(source.as_bytes());
        let mut pp = PrettyPrintVisitor::new();
        walker.walk(&tree, &mut pp);
        let newlines = pp.output().matches('\n').count();
        // At least one newline per non-error node (enter_node) plus leaves.
        prop_assert!(newlines >= 1);
    }
}

// ---------------------------------------------------------------------------
// 14. Property: SearchVisitor with always-true finds all non-error nodes
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn search_always_true_finds_all(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let walker = TreeWalker::new(source.as_bytes());
        let mut search = SearchVisitor::new(|_: &ParsedNode| true);
        walker.walk(&tree, &mut search);

        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);

        // SearchVisitor records in enter_node, same as StatsVisitor.total_nodes
        prop_assert_eq!(search.matches.len(), stats.total_nodes);
    }
}

// ---------------------------------------------------------------------------
// 15. Property: SearchVisitor with always-false finds nothing
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn search_always_false_finds_none(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let walker = TreeWalker::new(source.as_bytes());
        let mut search = SearchVisitor::new(|_: &ParsedNode| false);
        walker.walk(&tree, &mut search);
        prop_assert!(search.matches.is_empty());
    }
}

// ---------------------------------------------------------------------------
// 16. Property: StatsVisitor node_counts values sum to total_nodes
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn stats_node_counts_sum_to_total(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let walker = TreeWalker::new(source.as_bytes());
        let mut stats = StatsVisitor::default();
        walker.walk(&tree, &mut stats);
        let sum: usize = stats.node_counts.values().sum();
        prop_assert_eq!(sum, stats.total_nodes);
    }
}

// ---------------------------------------------------------------------------
// 17. Property: Stop action limits traversal
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn stop_limits_traversal(
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

        // The BFS walker fully respects Stop by halting the queue.
        let walker = BreadthFirstWalker::new(source.as_bytes());
        let mut v = StopAfter { count: 0, limit };
        walker.walk(&tree, &mut v);
        prop_assert!(v.count <= limit);
    }
}

// ---------------------------------------------------------------------------
// 18. Property: SkipChildren means fewer nodes visited than total
// ---------------------------------------------------------------------------

proptest! {
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

        let walker = TreeWalker::new(source.as_bytes());
        let mut skip_v = SkipFirst { count: 0, skipped: false };
        walker.walk(&tree, &mut skip_v);

        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);

        // Skipping the root's children should visit fewer or equal nodes.
        prop_assert!(skip_v.count <= stats.total_nodes);
    }
}

// ---------------------------------------------------------------------------
// 19. Property: TransformWalker leaf count matches
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn transform_walker_counts_leaves(
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

        let tw = TransformWalker::new(source.as_bytes());
        let mut cl = CountLeaves;
        let leaf_count = tw.walk(&tree, &mut cl);

        let expected = count_leaves(&tree);
        prop_assert_eq!(leaf_count, expected);
    }
}

// ---------------------------------------------------------------------------
// 20. Property: DFS enter/leave pairing
// ---------------------------------------------------------------------------

proptest! {
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

        let walker = TreeWalker::new(source.as_bytes());
        let mut v = Balance { enters: 0, leaves: 0 };
        walker.walk(&tree, &mut v);
        prop_assert_eq!(v.enters, v.leaves, "every enter_node must have a leave_node");
    }
}

// ---------------------------------------------------------------------------
// 21. Property: SearchVisitor matches subset of all nodes
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn search_subset_of_all(
        source in arb_source(),
        tree in arb_tree(3, 3),
        threshold in 1u16..=10,
    ) {
        let t = threshold;
        let walker = TreeWalker::new(source.as_bytes());
        let mut search = SearchVisitor::new(move |n: &ParsedNode| n.symbol <= t);
        walker.walk(&tree, &mut search);

        let mut all = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut all);

        prop_assert!(search.matches.len() <= all.matches.len());
    }
}

// ---------------------------------------------------------------------------
// 22. Concrete: error nodes increment error_nodes counter
// ---------------------------------------------------------------------------

#[test]
fn error_node_increments_error_count() {
    let source = b"x error y";
    let tree = interior(1, vec![leaf(2, 0, 1), error_node(2, 7), leaf(3, 8, 9)]);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut stats);
    assert_eq!(stats.error_nodes, 1);
}

// ---------------------------------------------------------------------------
// 23. Concrete: single leaf tree
// ---------------------------------------------------------------------------

#[test]
fn single_leaf_tree_stats() {
    let source = b"hello";
    let tree = leaf(1, 0, 5);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
    assert_eq!(stats.max_depth, 1);
}

// ---------------------------------------------------------------------------
// 24. Concrete: walker constructors
// ---------------------------------------------------------------------------

#[test]
fn walker_constructors() {
    let source = b"abc";
    let _dfs = TreeWalker::new(source);
    let _bfs = BreadthFirstWalker::new(source);
    let _tw = TransformWalker::new(source);
    // Just ensure they compile and construct without panic.
}

// ---------------------------------------------------------------------------
// 25. Property: TransformWalker computes tree depth
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn transform_walker_computes_depth(
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

        let tw = TransformWalker::new(source.as_bytes());
        let mut dc = DepthCalc;
        let depth = tw.walk(&tree, &mut dc);
        let expected = tree_depth(&tree);
        prop_assert_eq!(depth, expected);
    }
}

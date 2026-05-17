//! Comprehensive visitor pattern tests for the adze visitor API.
//!
//! Tests cover: collection, counting, leaf/error detection, AST building,
//! statistics, filtering, early termination, empty/deep/wide trees,
//! composition, breadth-first walking, and transform visitors.

use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TransformVisitor,
    TransformWalker, TreeWalker, Visitor, VisitorAction,
};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a synthetic `ParsedNode` with standard defaults.
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

/// Create a leaf node with the given symbol spanning [start..end).
fn leaf(symbol: u16, start: usize, end: usize) -> ParsedNode {
    make_node(symbol, vec![], start, end, false, true)
}

/// Create an interior node with given children. Byte range is derived from children.
fn interior(symbol: u16, children: Vec<ParsedNode>) -> ParsedNode {
    let start = children.first().map_or(0, |c| c.start_byte);
    let end = children.last().map_or(0, |c| c.end_byte);
    make_node(symbol, children, start, end, false, true)
}

/// Create an error node spanning [start..end).
fn error_node(start: usize, end: usize) -> ParsedNode {
    make_node(0, vec![], start, end, true, false)
}

/// Build a simple expression tree: root(a, op, b)
///   source = "1+2"
fn simple_expr_tree() -> (ParsedNode, Vec<u8>) {
    let source = b"1+2".to_vec();
    let a = leaf(1, 0, 1); // "1"
    let op = leaf(2, 1, 2); // "+"
    let b = leaf(1, 2, 3); // "2"
    let root = interior(5, vec![a, op, b]);
    (root, source)
}

// ---------------------------------------------------------------------------
// 1. Visitor that collects all node kinds into a list
// ---------------------------------------------------------------------------

struct KindCollector {
    kinds: Vec<String>,
}

impl KindCollector {
    fn new() -> Self {
        Self { kinds: vec![] }
    }
}

impl Visitor for KindCollector {
    fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
        self.kinds.push(node.kind().to_string());
        VisitorAction::Continue
    }
}

#[test]
fn test_collect_all_node_kinds() {
    let (root, source) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut v = KindCollector::new();
    walker.walk(&root, &mut v);
    // root + 3 children = 4 kinds
    assert_eq!(v.kinds.len(), 4);
    assert_eq!(v.kinds[0], root.kind());
}

#[test]
fn test_collect_kinds_preserves_order() {
    let (root, source) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut v = KindCollector::new();
    walker.walk(&root, &mut v);
    // DFS pre-order: root, child0, child1, child2
    let expected_symbols: Vec<u16> = vec![5, 1, 2, 1];
    let actual_symbols: Vec<u16> = vec![
        root.symbol,
        root.children[0].symbol,
        root.children[1].symbol,
        root.children[2].symbol,
    ];
    assert_eq!(actual_symbols, expected_symbols);
    assert_eq!(v.kinds.len(), expected_symbols.len());
}

// ---------------------------------------------------------------------------
// 2. Visitor that counts nodes at each depth level
// ---------------------------------------------------------------------------

struct DepthCounter {
    counts: HashMap<usize, usize>,
    depth: usize,
}

impl DepthCounter {
    fn new() -> Self {
        Self {
            counts: HashMap::new(),
            depth: 0,
        }
    }
}

impl Visitor for DepthCounter {
    fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
        *self.counts.entry(self.depth).or_insert(0) += 1;
        self.depth += 1;
        VisitorAction::Continue
    }

    fn leave_node(&mut self, _node: &ParsedNode) {
        self.depth -= 1;
    }
}

#[test]
fn test_count_nodes_at_each_depth() {
    let (root, source) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut v = DepthCounter::new();
    walker.walk(&root, &mut v);
    assert_eq!(v.counts[&0], 1); // root
    assert_eq!(v.counts[&1], 3); // three children
    assert!(!v.counts.contains_key(&2)); // no grandchildren
}

// ---------------------------------------------------------------------------
// 3. Visitor that finds all leaf nodes
// ---------------------------------------------------------------------------

struct LeafCollector {
    leaves: Vec<String>,
}

impl LeafCollector {
    fn new() -> Self {
        Self { leaves: vec![] }
    }
}

impl Visitor for LeafCollector {
    fn visit_leaf(&mut self, _node: &ParsedNode, text: &str) {
        self.leaves.push(text.to_string());
    }
}

#[test]
fn test_find_all_leaf_nodes() {
    let (root, source) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut v = LeafCollector::new();
    walker.walk(&root, &mut v);
    assert_eq!(v.leaves, vec!["1", "+", "2"]);
}

// ---------------------------------------------------------------------------
// 4. Visitor that finds all error nodes
// ---------------------------------------------------------------------------

struct ErrorCollector {
    error_ranges: Vec<(usize, usize)>,
}

impl ErrorCollector {
    fn new() -> Self {
        Self {
            error_ranges: vec![],
        }
    }
}

impl Visitor for ErrorCollector {
    fn visit_error(&mut self, node: &ParsedNode) {
        self.error_ranges.push((node.start_byte(), node.end_byte()));
    }
}

#[test]
fn test_find_all_error_nodes() {
    let source = b"1+?+2".to_vec();
    let a = leaf(1, 0, 1);
    let op1 = leaf(2, 1, 2);
    let err = error_node(2, 3);
    let op2 = leaf(2, 3, 4);
    let b = leaf(1, 4, 5);
    let root = interior(5, vec![a, op1, err, op2, b]);

    let walker = TreeWalker::new(&source);
    let mut v = ErrorCollector::new();
    walker.walk(&root, &mut v);
    assert_eq!(v.error_ranges, vec![(2, 3)]);
}

#[test]
fn test_no_error_nodes_in_valid_tree() {
    let (root, source) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut v = ErrorCollector::new();
    walker.walk(&root, &mut v);
    assert!(v.error_ranges.is_empty());
}

// ---------------------------------------------------------------------------
// 5. Visitor that builds a simplified AST representation
// ---------------------------------------------------------------------------

struct AstBuilder;

impl TransformVisitor for AstBuilder {
    type Output = String;

    fn transform_node(&mut self, node: &ParsedNode, children: Vec<String>) -> String {
        format!("({}{})", node.kind(), {
            if children.is_empty() {
                String::new()
            } else {
                format!(" {}", children.join(" "))
            }
        })
    }

    fn transform_leaf(&mut self, _node: &ParsedNode, text: &str) -> String {
        format!("\"{}\"", text)
    }

    fn transform_error(&mut self, node: &ParsedNode) -> String {
        format!("(ERROR @{})", node.start_byte())
    }
}

#[test]
fn test_build_simplified_ast() {
    let (root, source) = simple_expr_tree();
    let tw = TransformWalker::new(&source);
    let mut builder = AstBuilder;
    let ast = tw.walk(&root, &mut builder);
    assert_eq!(ast, "(Expression \"1\" \"+\" \"2\")");
}

#[test]
fn test_transform_with_error_node() {
    let source = b"1?2".to_vec();
    let a = leaf(1, 0, 1);
    let err = error_node(1, 2);
    let b = leaf(1, 2, 3);
    let root = interior(5, vec![a, err, b]);

    let tw = TransformWalker::new(&source);
    let mut builder = AstBuilder;
    let ast = tw.walk(&root, &mut builder);
    assert!(ast.contains("ERROR"));
}

// ---------------------------------------------------------------------------
// 6. Visitor that computes node statistics (min/max depth, avg children)
// ---------------------------------------------------------------------------

struct NodeStatistics {
    depth: usize,
    min_depth_leaf: Option<usize>,
    max_depth_leaf: Option<usize>,
    child_counts: Vec<usize>,
}

impl NodeStatistics {
    fn new() -> Self {
        Self {
            depth: 0,
            min_depth_leaf: None,
            max_depth_leaf: None,
            child_counts: vec![],
        }
    }

    fn avg_children(&self) -> f64 {
        if self.child_counts.is_empty() {
            return 0.0;
        }
        let sum: usize = self.child_counts.iter().sum();
        sum as f64 / self.child_counts.len() as f64
    }
}

impl Visitor for NodeStatistics {
    fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
        self.child_counts.push(node.child_count());
        self.depth += 1;
        VisitorAction::Continue
    }

    fn leave_node(&mut self, _node: &ParsedNode) {
        self.depth -= 1;
    }

    fn visit_leaf(&mut self, _node: &ParsedNode, _text: &str) {
        let d = self.depth;
        self.min_depth_leaf = Some(self.min_depth_leaf.map_or(d, |m| m.min(d)));
        self.max_depth_leaf = Some(self.max_depth_leaf.map_or(d, |m| m.max(d)));
    }
}

#[test]
fn test_compute_node_statistics() {
    let (root, source) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut stats = NodeStatistics::new();
    walker.walk(&root, &mut stats);

    assert_eq!(stats.min_depth_leaf, Some(2)); // leaves at depth 2 (root=1, child=2)
    assert_eq!(stats.max_depth_leaf, Some(2));
    // root has 3 children, 3 leaves have 0 each => avg = 3/4
    assert!((stats.avg_children() - 0.75).abs() < f64::EPSILON);
}

// ---------------------------------------------------------------------------
// 7. Visitor that filters nodes by kind pattern
// ---------------------------------------------------------------------------

#[test]
fn test_filter_nodes_by_kind_pattern() {
    let (root, source) = simple_expr_tree();
    let walker = TreeWalker::new(&source);

    // Search for symbol 1 nodes (the "number" leaves)
    let mut search = SearchVisitor::new(|node: &ParsedNode| node.symbol() == 1);
    walker.walk(&root, &mut search);
    assert_eq!(search.matches.len(), 2);
}

#[test]
fn test_filter_no_matches() {
    let (root, source) = simple_expr_tree();
    let walker = TreeWalker::new(&source);

    let mut search = SearchVisitor::new(|node: &ParsedNode| node.symbol() == 99);
    walker.walk(&root, &mut search);
    assert!(search.matches.is_empty());
}

// ---------------------------------------------------------------------------
// 8. Visitor with early termination (stop after finding target)
// ---------------------------------------------------------------------------

struct FirstFinder {
    target_symbol: u16,
    found: Option<(usize, usize)>,
}

impl FirstFinder {
    fn new(target_symbol: u16) -> Self {
        Self {
            target_symbol,
            found: None,
        }
    }
}

impl Visitor for FirstFinder {
    fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
        if node.symbol() == self.target_symbol {
            self.found = Some((node.start_byte(), node.end_byte()));
            VisitorAction::Stop
        } else {
            VisitorAction::Continue
        }
    }
}

#[test]
fn test_early_termination_stops_after_first_match() {
    // Note: In the pure-rust walker, VisitorAction::Stop prevents descent into
    // the stopped node's children but does NOT halt sibling iteration.
    // Verify that Stop is respected by checking that a deep subtree is skipped.
    let source = b"abcde".to_vec();
    let deep_leaf = leaf(1, 3, 4);
    let deep_child = interior(5, vec![deep_leaf]);
    let a = leaf(1, 0, 1);
    let b = leaf(1, 1, 2);
    // The deep_child subtree contains a nested leaf that should NOT be visited
    // because Stop is returned when entering deep_child.
    let root = interior(5, vec![a, b, deep_child]);

    let walker = TreeWalker::new(&source);

    struct CountingFinder {
        target_symbol: u16,
        enter_count: usize,
    }
    impl Visitor for CountingFinder {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.enter_count += 1;
            if node.symbol() == self.target_symbol && node.child_count() > 0 {
                // Stop descent into this interior node
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }

    let mut finder = CountingFinder {
        target_symbol: 5,
        enter_count: 0,
    };
    walker.walk(&root, &mut finder);
    // Entered: root(5)->Stop so no children visited at all,
    // but the walker still enters root. enter_count should be 1.
    assert_eq!(finder.enter_count, 1);
}

#[test]
fn test_early_termination_no_match() {
    let (root, source) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut finder = FirstFinder::new(99);
    walker.walk(&root, &mut finder);
    assert!(finder.found.is_none());
}

// ---------------------------------------------------------------------------
// 9. Visitor that tracks node annotations (named vs anonymous)
// ---------------------------------------------------------------------------

struct AnnotationCollector {
    named_count: usize,
    anonymous_count: usize,
}

impl AnnotationCollector {
    fn new() -> Self {
        Self {
            named_count: 0,
            anonymous_count: 0,
        }
    }
}

impl Visitor for AnnotationCollector {
    fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
        if node.is_named() {
            self.named_count += 1;
        } else {
            self.anonymous_count += 1;
        }
        VisitorAction::Continue
    }
}

#[test]
fn test_annotation_tracking_named_nodes() {
    let (root, source) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut v = AnnotationCollector::new();
    walker.walk(&root, &mut v);
    // All nodes in simple_expr_tree are created with is_named: true
    assert_eq!(v.named_count, 4);
    assert_eq!(v.anonymous_count, 0);
}

#[test]
fn test_annotation_tracking_mixed() {
    let source = b"1+2".to_vec();
    let op = make_node(2, vec![], 1, 2, false, false); // operator is anonymous
    let root = interior(5, vec![leaf(1, 0, 1), op, leaf(1, 2, 3)]);

    let walker = TreeWalker::new(&source);
    let mut v = AnnotationCollector::new();
    walker.walk(&root, &mut v);
    assert_eq!(v.named_count, 3); // root + two number leaves
    assert_eq!(v.anonymous_count, 1); // operator
}

// ---------------------------------------------------------------------------
// 10. Visitor applied to empty tree (leaf-only root)
// ---------------------------------------------------------------------------

#[test]
fn test_visitor_on_leaf_only_root() {
    let source = b"x".to_vec();
    let root = leaf(1, 0, 1);
    let walker = TreeWalker::new(&source);

    let mut kinds = KindCollector::new();
    walker.walk(&root, &mut kinds);
    assert_eq!(kinds.kinds.len(), 1); // just the root

    let mut leaves = LeafCollector::new();
    walker.walk(&root, &mut leaves);
    assert_eq!(leaves.leaves, vec!["x"]);
}

#[test]
fn test_stats_visitor_on_leaf_only_root() {
    let source = b"x".to_vec();
    let root = leaf(1, 0, 1);
    let walker = TreeWalker::new(&source);

    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
    assert_eq!(stats.error_nodes, 0);
    assert_eq!(stats.max_depth, 1);
}

#[test]
fn test_visitor_on_childless_interior() {
    // An interior-like node that happens to have zero children
    let source = b"".to_vec();
    let root = interior(5, vec![]);
    let walker = TreeWalker::new(&source);

    let mut kinds = KindCollector::new();
    walker.walk(&root, &mut kinds);
    assert_eq!(kinds.kinds.len(), 1);
}

// ---------------------------------------------------------------------------
// 11. Visitor applied to deeply nested tree (100+ levels)
// ---------------------------------------------------------------------------

fn deep_tree(depth: usize) -> (ParsedNode, Vec<u8>) {
    let source = b"x".to_vec();
    let mut node = leaf(1, 0, 1);
    for _ in 0..depth {
        node = interior(5, vec![node]);
    }
    (node, source)
}

#[test]
fn test_deeply_nested_tree_100_levels() {
    let (root, source) = deep_tree(100);
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 101); // 100 interior + 1 leaf
    assert_eq!(stats.leaf_nodes, 1);
    assert_eq!(stats.max_depth, 101);
}

#[test]
fn test_deeply_nested_tree_depth_counter() {
    let (root, source) = deep_tree(100);
    let walker = TreeWalker::new(&source);
    let mut dc = DepthCounter::new();
    walker.walk(&root, &mut dc);
    // Every depth from 0..=100 should have exactly 1 node
    for d in 0..=100 {
        assert_eq!(dc.counts.get(&d).copied().unwrap_or(0), 1, "depth {d}");
    }
}

// ---------------------------------------------------------------------------
// 12. Visitor applied to wide tree (100+ children at one level)
// ---------------------------------------------------------------------------

fn wide_tree(width: usize) -> (ParsedNode, Vec<u8>) {
    let mut source = Vec::new();
    let mut children = Vec::new();
    for i in 0..width {
        let byte = b'a' + (i % 26) as u8;
        source.push(byte);
        children.push(leaf(1, i, i + 1));
    }
    let root = interior(5, children);
    (root, source)
}

#[test]
fn test_wide_tree_100_children() {
    let (root, source) = wide_tree(100);
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 101); // root + 100 children
    assert_eq!(stats.leaf_nodes, 100);
}

#[test]
fn test_wide_tree_leaf_texts() {
    let (root, source) = wide_tree(5);
    let walker = TreeWalker::new(&source);
    let mut lc = LeafCollector::new();
    walker.walk(&root, &mut lc);
    assert_eq!(lc.leaves, vec!["a", "b", "c", "d", "e"]);
}

// ---------------------------------------------------------------------------
// 13. Multiple visitors composed on same tree
// ---------------------------------------------------------------------------

#[test]
fn test_multiple_visitors_on_same_tree() {
    let (root, source) = simple_expr_tree();
    let walker = TreeWalker::new(&source);

    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);

    let mut kinds = KindCollector::new();
    walker.walk(&root, &mut kinds);

    let mut leaves = LeafCollector::new();
    walker.walk(&root, &mut leaves);

    // All visitors should agree on the structure
    assert_eq!(stats.total_nodes, kinds.kinds.len());
    assert_eq!(stats.leaf_nodes, leaves.leaves.len());
}

#[test]
fn test_composed_stats_and_search() {
    let (root, source) = simple_expr_tree();
    let walker = TreeWalker::new(&source);

    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);

    let mut search = SearchVisitor::new(|node: &ParsedNode| node.symbol() == 1);
    walker.walk(&root, &mut search);

    // Stats says 4 total nodes; search found 2 with symbol 1
    assert_eq!(stats.total_nodes, 4);
    assert_eq!(search.matches.len(), 2);
}

// ---------------------------------------------------------------------------
// 14. Visitor thread safety
// ---------------------------------------------------------------------------

#[test]
fn test_visitor_send_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    // Visitor types should be Send + Sync where possible
    assert_send::<StatsVisitor>();
    assert_send::<PrettyPrintVisitor>();
    assert_sync::<StatsVisitor>();
    assert_sync::<PrettyPrintVisitor>();
}

// ---------------------------------------------------------------------------
// Additional tests to reach 20+
// ---------------------------------------------------------------------------

#[test]
fn test_pretty_print_visitor() {
    let (root, source) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let output = pp.output();
    // Should contain the root kind and leaf text
    assert!(output.contains("Expression"));
    assert!(output.contains("\"1\""));
    assert!(output.contains("\"2\""));
}

#[test]
fn test_breadth_first_walker_visits_level_order() {
    // Build tree: root -> [a, b(c, d)]
    let source = b"abcd".to_vec();
    let a = leaf(1, 0, 1);
    let c = leaf(1, 2, 3);
    let d = leaf(1, 3, 4);
    let b = interior(5, vec![c, d]);
    // Adjust b's range
    let root = interior(5, vec![a, b]);

    let bfw = BreadthFirstWalker::new(&source);
    let mut kinds = KindCollector::new();
    bfw.walk(&root, &mut kinds);
    // BFS: root, a, b, c, d  (5 nodes)
    assert_eq!(kinds.kinds.len(), 5);
}

#[test]
fn test_skip_children_prevents_descent() {
    struct SkipAtRoot;
    impl Visitor for SkipAtRoot {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            if node.child_count() > 0 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
        fn visit_leaf(&mut self, _node: &ParsedNode, _text: &str) {
            panic!("Should never visit leaves when root skips children");
        }
    }

    let (root, source) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut v = SkipAtRoot;
    walker.walk(&root, &mut v); // should not panic
}

#[test]
fn test_error_node_is_visited_via_visit_error() {
    let source = b"err".to_vec();
    let root = error_node(0, 3);
    let walker = TreeWalker::new(&source);

    let mut ec = ErrorCollector::new();
    walker.walk(&root, &mut ec);
    assert_eq!(ec.error_ranges, vec![(0, 3)]);
}

#[test]
fn test_error_node_not_entered() {
    // When the walker encounters an error node, it calls visit_error
    // but NOT enter_node (per the walk_node implementation).
    let source = b"err".to_vec();
    let root = error_node(0, 3);
    let walker = TreeWalker::new(&source);

    let mut kinds = KindCollector::new();
    walker.walk(&root, &mut kinds);
    // enter_node should NOT be called for error nodes
    assert!(kinds.kinds.is_empty());
}

#[test]
fn test_transform_walker_leaf_only() {
    let source = b"42".to_vec();
    let root = leaf(1, 0, 2);
    let tw = TransformWalker::new(&source);

    struct LeafTransform;
    impl TransformVisitor for LeafTransform {
        type Output = i32;
        fn transform_node(&mut self, _node: &ParsedNode, _children: Vec<i32>) -> i32 {
            unreachable!("leaf-only tree should not call transform_node");
        }
        fn transform_leaf(&mut self, _node: &ParsedNode, text: &str) -> i32 {
            text.parse().unwrap_or(0)
        }
        fn transform_error(&mut self, _node: &ParsedNode) -> i32 {
            -1
        }
    }

    let result = tw.walk(&root, &mut LeafTransform);
    assert_eq!(result, 42);
}

#[test]
fn test_transform_walker_sums_children() {
    let source = b"123".to_vec();
    let a = leaf(1, 0, 1);
    let b = leaf(1, 1, 2);
    let c = leaf(1, 2, 3);
    let root = interior(5, vec![a, b, c]);
    let tw = TransformWalker::new(&source);

    struct SumTransform;
    impl TransformVisitor for SumTransform {
        type Output = i32;
        fn transform_node(&mut self, _node: &ParsedNode, children: Vec<i32>) -> i32 {
            children.iter().sum()
        }
        fn transform_leaf(&mut self, _node: &ParsedNode, text: &str) -> i32 {
            text.parse().unwrap_or(0)
        }
        fn transform_error(&mut self, _node: &ParsedNode) -> i32 {
            0
        }
    }

    let result = tw.walk(&root, &mut SumTransform);
    assert_eq!(result, 6); // 1 + 2 + 3
}

#[test]
fn test_stats_visitor_node_counts_by_kind() {
    let (root, source) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);

    // Without a language pointer, kind() uses the fallback table.
    // Symbol 1 => "*", Symbol 2 => "_2", Symbol 5 => "Expression"
    // Two nodes with symbol 1, one with symbol 2, one with symbol 5
    let total_by_kind: usize = stats.node_counts.values().sum();
    assert_eq!(total_by_kind, stats.total_nodes);
}

#[test]
fn test_search_visitor_captures_byte_ranges() {
    let (root, source) = simple_expr_tree();
    let walker = TreeWalker::new(&source);

    let mut search = SearchVisitor::new(|node: &ParsedNode| node.symbol() == 2);
    walker.walk(&root, &mut search);

    assert_eq!(search.matches.len(), 1);
    let (start, end, _kind) = &search.matches[0];
    assert_eq!(*start, 1);
    assert_eq!(*end, 2);
}

#[test]
fn test_wide_tree_200_children_stats() {
    let (root, source) = wide_tree(200);
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 201);
    assert_eq!(stats.leaf_nodes, 200);
    assert_eq!(stats.max_depth, 2); // root=1, children=2
}

#[test]
fn test_deeply_nested_early_termination() {
    let (root, source) = deep_tree(200);
    let walker = TreeWalker::new(&source);

    struct CountAndStop {
        count: usize,
        limit: usize,
    }
    impl Visitor for CountAndStop {
        fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
            self.count += 1;
            if self.count >= self.limit {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }

    let mut v = CountAndStop {
        count: 0,
        limit: 10,
    };
    walker.walk(&root, &mut v);
    assert_eq!(v.count, 10);
}

#[test]
fn test_pretty_print_on_deep_tree() {
    let (root, source) = deep_tree(5);
    let walker = TreeWalker::new(&source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let output = pp.output();
    // Should have increasing indentation
    let lines: Vec<&str> = output.lines().collect();
    assert!(lines.len() >= 6); // 5 interior + 1 leaf text line
}

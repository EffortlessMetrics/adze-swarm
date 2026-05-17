#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]
//! Comprehensive tests for all visitor patterns in the adze runtime.

use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TransformVisitor,
    TransformWalker, TreeWalker, Visitor, VisitorAction,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Construct a `ParsedNode`. The `language` field is `pub(crate)` so we
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

// Symbol-to-kind mapping (when language is None):
// 0 => "end", 1 => "*", 2 => "_2", 3 => "_6", 4 => "-",
// 5 => "Expression", 6 => "Whitespace__whitespace", 7 => "Whitespace",
// 8 => "Expression_Sub_1", 9 => "Expression_Sub", 10 => "rule_10",
// _ => "unknown"

/// Build a sample tree:
///   root(10)
///   ├── a(1)          "a"
///   ├── mid(5)
///   │   ├── b(2)      "b"
///   │   └── c(3)      "c"
///   └── d(4)          "d"
/// Source: "abcd"
fn sample_tree() -> (ParsedNode, Vec<u8>) {
    let src = b"abcd".to_vec();
    let a = leaf(1, 0, 1);
    let b = leaf(2, 1, 2);
    let c = unnamed_leaf(3, 2, 3);
    let mid = interior(5, vec![b, c]);
    let d = leaf(4, 3, 4);
    let root = interior(10, vec![a, mid, d]);
    (root, src)
}

/// Build a deep (linear) tree of `depth` interior nodes with a single leaf.
/// Source: "x"
fn deep_tree(depth: usize) -> (ParsedNode, Vec<u8>) {
    let src = b"x".to_vec();
    let mut node = leaf(1, 0, 1);
    for _ in 0..depth {
        node = interior(10, vec![node]);
    }
    (node, src)
}

/// Build a wide tree: root with `width` leaf children.
/// Source: "a" repeated `width` times.
fn wide_tree(width: usize) -> (ParsedNode, Vec<u8>) {
    let src: Vec<u8> = (0..width).map(|_| b'a').collect();
    let children: Vec<ParsedNode> = (0..width).map(|i| leaf(1, i, i + 1)).collect();
    let root = interior(10, children);
    (root, src)
}

/// A visitor that records the order of entered symbols.
#[derive(Default)]
struct OrderVisitor {
    entered: Vec<u16>,
}

impl Visitor for OrderVisitor {
    fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
        self.entered.push(node.symbol());
        VisitorAction::Continue
    }
}

/// A visitor that records leaf texts in visit order.
#[derive(Default)]
struct LeafCollector {
    leaves: Vec<String>,
}

impl Visitor for LeafCollector {
    fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
        VisitorAction::Continue
    }
    fn visit_leaf(&mut self, _node: &ParsedNode, text: &str) {
        self.leaves.push(text.to_string());
    }
}

// ===================================================================
// DFS traversal order
// ===================================================================

#[test]
fn dfs_visits_root_first() {
    let (tree, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut v = OrderVisitor::default();
    walker.walk(&tree, &mut v);
    assert_eq!(v.entered[0], 10, "DFS should visit root first");
}

#[test]
fn dfs_preorder_symbol_sequence() {
    let (tree, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut v = OrderVisitor::default();
    walker.walk(&tree, &mut v);
    // Pre-order: root(10), a(1), mid(5), b(2), c(3), d(4)
    assert_eq!(v.entered, vec![10, 1, 5, 2, 3, 4]);
}

#[test]
fn dfs_leaf_text_order() {
    let (tree, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut v = LeafCollector::default();
    walker.walk(&tree, &mut v);
    assert_eq!(v.leaves, vec!["a", "b", "c", "d"]);
}

#[test]
fn dfs_leave_node_called_in_postorder() {
    #[derive(Default)]
    struct PostOrderVisitor {
        left: Vec<u16>,
    }
    impl Visitor for PostOrderVisitor {
        fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn leave_node(&mut self, node: &ParsedNode) {
            self.left.push(node.symbol());
        }
    }

    let (tree, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut v = PostOrderVisitor::default();
    walker.walk(&tree, &mut v);
    // Post-order: a(1), b(2), c(3), mid(5), d(4), root(10)
    assert_eq!(v.left, vec![1, 2, 3, 5, 4, 10]);
}

#[test]
fn dfs_stop_halts_traversal() {
    struct StopAfterTwo {
        count: usize,
        entered: Vec<u16>,
    }
    impl Visitor for StopAfterTwo {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.count += 1;
            self.entered.push(node.symbol());
            if self.count >= 2 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }

    let (tree, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut v = StopAfterTwo {
        count: 0,
        entered: vec![],
    };
    walker.walk(&tree, &mut v);
    // Stop returns from walk_node for that node; the parent's child loop
    // continues visiting siblings. So root enters (Continue), then each
    // direct child enters and immediately returns (Stop).
    assert_eq!(v.entered, vec![10, 1, 5, 4]);
}

#[test]
fn dfs_skip_children_skips_subtree() {
    struct SkipMid {
        entered: Vec<u16>,
    }
    impl Visitor for SkipMid {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.entered.push(node.symbol());
            if node.symbol() == 5 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }

    let (tree, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut v = SkipMid { entered: vec![] };
    walker.walk(&tree, &mut v);
    // Skipping mid(5) means b(2) and c(3) are not visited
    assert_eq!(v.entered, vec![10, 1, 5, 4]);
}

// ===================================================================
// BFS traversal order
// ===================================================================

#[test]
fn bfs_visits_root_first() {
    let (tree, src) = sample_tree();
    let walker = BreadthFirstWalker::new(&src);
    let mut v = OrderVisitor::default();
    walker.walk(&tree, &mut v);
    assert_eq!(v.entered[0], 10);
}

#[test]
fn bfs_level_order_symbol_sequence() {
    let (tree, src) = sample_tree();
    let walker = BreadthFirstWalker::new(&src);
    let mut v = OrderVisitor::default();
    walker.walk(&tree, &mut v);
    // Level-order: root(10), a(1), mid(5), d(4), b(2), c(3)
    assert_eq!(v.entered, vec![10, 1, 5, 4, 2, 3]);
}

#[test]
fn bfs_leaf_text_order() {
    let (tree, src) = sample_tree();
    let walker = BreadthFirstWalker::new(&src);
    let mut v = LeafCollector::default();
    walker.walk(&tree, &mut v);
    // BFS visits leaves in level order: a, d come before b, c
    assert_eq!(v.leaves, vec!["a", "d", "b", "c"]);
}

#[test]
fn bfs_stop_halts_traversal() {
    struct StopAfterThree {
        count: usize,
        entered: Vec<u16>,
    }
    impl Visitor for StopAfterThree {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.count += 1;
            self.entered.push(node.symbol());
            if self.count >= 3 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }

    let (tree, src) = sample_tree();
    let walker = BreadthFirstWalker::new(&src);
    let mut v = StopAfterThree {
        count: 0,
        entered: vec![],
    };
    walker.walk(&tree, &mut v);
    assert_eq!(v.entered, vec![10, 1, 5]);
}

#[test]
fn bfs_skip_children_skips_subtree() {
    struct SkipMid {
        entered: Vec<u16>,
    }
    impl Visitor for SkipMid {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.entered.push(node.symbol());
            if node.symbol() == 5 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }

    let (tree, src) = sample_tree();
    let walker = BreadthFirstWalker::new(&src);
    let mut v = SkipMid { entered: vec![] };
    walker.walk(&tree, &mut v);
    // mid(5) visited but children b(2), c(3) are not queued
    assert_eq!(v.entered, vec![10, 1, 5, 4]);
}

// ===================================================================
// SearchVisitor
// ===================================================================

#[test]
fn search_finds_matching_node() {
    let (tree, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 2);
    walker.walk(&tree, &mut sv);
    assert_eq!(sv.matches.len(), 1);
    assert_eq!(sv.matches[0], (1, 2, "_2".to_string()));
}

#[test]
fn search_returns_empty_when_not_found() {
    let (tree, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 99);
    walker.walk(&tree, &mut sv);
    assert!(sv.matches.is_empty());
}

#[test]
fn search_finds_multiple_matches() {
    // All named leaves
    let (tree, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.is_named() && n.child_count() == 0);
    walker.walk(&tree, &mut sv);
    // a(1), b(2) are named leaves; c(3) is unnamed; d(4) is named
    assert_eq!(sv.matches.len(), 3);
}

#[test]
fn search_by_byte_range() {
    let (tree, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.start_byte() >= 2 && n.end_byte() <= 4);
    walker.walk(&tree, &mut sv);
    // Nodes fully within [2..4]: c(3) at [2,3] and d(4) at [3,4]
    assert!(sv.matches.len() >= 2);
}

#[test]
fn search_with_bfs_walker() {
    let (tree, src) = sample_tree();
    let walker = BreadthFirstWalker::new(&src);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 4);
    walker.walk(&tree, &mut sv);
    assert_eq!(sv.matches.len(), 1);
    assert_eq!(sv.matches[0].2, "-");
}

// ===================================================================
// PrettyPrintVisitor
// ===================================================================

#[test]
fn pretty_print_contains_node_kinds() {
    let (tree, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&tree, &mut pp);
    let out = pp.output();
    assert!(out.contains("rule_10")); // root kind
    assert!(out.contains("Expression")); // mid kind (symbol 5)
}

#[test]
fn pretty_print_contains_leaf_text() {
    let (tree, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&tree, &mut pp);
    let out = pp.output();
    assert!(out.contains("\"a\""));
    assert!(out.contains("\"b\""));
    assert!(out.contains("\"d\""));
}

#[test]
fn pretty_print_indentation_increases_with_depth() {
    let (tree, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&tree, &mut pp);
    let lines: Vec<&str> = pp.output().lines().collect();
    // Root line has 0 indent, children have 2-space indent, grandchildren 4-space
    assert!(lines[0].starts_with("rule_10")); // root: no indent
    // A child line should start with "  "
    let child_line = lines.iter().find(|l| l.contains("\"a\"")).unwrap();
    assert!(child_line.starts_with("    ")); // depth 2 (inside root enter + leaf)
}

#[test]
fn pretty_print_named_annotation() {
    let (tree, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&tree, &mut pp);
    let out = pp.output();
    // Named nodes get " [named]" annotation
    assert!(out.contains("[named]"));
}

#[test]
fn pretty_print_default_creates_empty() {
    let pp = PrettyPrintVisitor::default();
    assert!(pp.output().is_empty());
}

// ===================================================================
// TransformVisitor
// ===================================================================

/// A transform that counts the total number of leaf characters.
struct CharCountTransform;

impl TransformVisitor for CharCountTransform {
    type Output = usize;

    fn transform_node(&mut self, _node: &ParsedNode, children: Vec<usize>) -> usize {
        children.iter().sum()
    }

    fn transform_leaf(&mut self, _node: &ParsedNode, text: &str) -> usize {
        text.len()
    }

    fn transform_error(&mut self, _node: &ParsedNode) -> usize {
        0
    }
}

#[test]
fn transform_char_count() {
    let (tree, src) = sample_tree();
    let tw = TransformWalker::new(&src);
    let mut t = CharCountTransform;
    let count = tw.walk(&tree, &mut t);
    assert_eq!(count, 4); // "abcd"
}

/// A transform that builds an S-expression string.
struct SexpTransform;

impl TransformVisitor for SexpTransform {
    type Output = String;

    fn transform_node(&mut self, node: &ParsedNode, children: Vec<String>) -> String {
        format!("({} {})", node.kind(), children.join(" "))
    }

    fn transform_leaf(&mut self, _node: &ParsedNode, text: &str) -> String {
        format!("\"{}\"", text)
    }

    fn transform_error(&mut self, _node: &ParsedNode) -> String {
        "ERROR".to_string()
    }
}

#[test]
fn transform_sexp_output() {
    let (tree, src) = sample_tree();
    let tw = TransformWalker::new(&src);
    let mut t = SexpTransform;
    let result = tw.walk(&tree, &mut t);
    assert!(result.starts_with("(rule_10 "));
    assert!(result.contains("\"a\""));
    assert!(result.contains("\"b\""));
    assert!(result.contains("\"d\""));
}

#[test]
fn transform_with_error_node() {
    let src = b"x".to_vec();
    let err = error_node(0, 1);
    let tw = TransformWalker::new(&src);
    let mut t = SexpTransform;
    let result = tw.walk(&err, &mut t);
    assert_eq!(result, "ERROR");
}

/// A transform that computes tree depth.
struct DepthTransform;

impl TransformVisitor for DepthTransform {
    type Output = usize;

    fn transform_node(&mut self, _node: &ParsedNode, children: Vec<usize>) -> usize {
        children.iter().copied().max().unwrap_or(0) + 1
    }

    fn transform_leaf(&mut self, _node: &ParsedNode, _text: &str) -> usize {
        1
    }

    fn transform_error(&mut self, _node: &ParsedNode) -> usize {
        1
    }
}

#[test]
fn transform_computes_depth() {
    let (tree, src) = sample_tree();
    let tw = TransformWalker::new(&src);
    let mut t = DepthTransform;
    let depth = tw.walk(&tree, &mut t);
    // root -> mid -> leaf = 3 levels
    assert_eq!(depth, 3);
}

// ===================================================================
// StatsVisitor
// ===================================================================

#[test]
fn stats_counts_all_nodes() {
    let (tree, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&tree, &mut stats);
    // 6 nodes: root, a, mid, b, c, d
    assert_eq!(stats.total_nodes, 6);
}

#[test]
fn stats_counts_leaves() {
    let (tree, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&tree, &mut stats);
    // 4 leaves: a, b, c, d
    assert_eq!(stats.leaf_nodes, 4);
}

#[test]
fn stats_max_depth() {
    let (tree, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&tree, &mut stats);
    // root(1) -> mid(2) -> leaf(3)
    assert_eq!(stats.max_depth, 3);
}

// ===================================================================
// Empty tree (single leaf, no children)
// ===================================================================

#[test]
fn dfs_single_leaf() {
    let src = b"z".to_vec();
    let node = leaf(1, 0, 1);
    let walker = TreeWalker::new(&src);
    let mut v = LeafCollector::default();
    walker.walk(&node, &mut v);
    assert_eq!(v.leaves, vec!["z"]);
}

#[test]
fn bfs_single_leaf() {
    let src = b"z".to_vec();
    let node = leaf(1, 0, 1);
    let walker = BreadthFirstWalker::new(&src);
    let mut v = LeafCollector::default();
    walker.walk(&node, &mut v);
    assert_eq!(v.leaves, vec!["z"]);
}

#[test]
fn stats_single_leaf() {
    let src = b"z".to_vec();
    let node = leaf(1, 0, 1);
    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&node, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
    assert_eq!(stats.max_depth, 1);
}

#[test]
fn transform_single_leaf() {
    let src = b"z".to_vec();
    let node = leaf(1, 0, 1);
    let tw = TransformWalker::new(&src);
    let mut t = CharCountTransform;
    assert_eq!(tw.walk(&node, &mut t), 1);
}

// ===================================================================
// Deep tree
// ===================================================================

#[test]
fn dfs_deep_tree_visits_all() {
    let (tree, src) = deep_tree(20);
    let walker = TreeWalker::new(&src);
    let mut v = OrderVisitor::default();
    walker.walk(&tree, &mut v);
    // 20 interior nodes + 1 leaf = 21
    assert_eq!(v.entered.len(), 21);
}

#[test]
fn stats_deep_tree_depth() {
    let (tree, src) = deep_tree(15);
    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&tree, &mut stats);
    assert_eq!(stats.max_depth, 16); // 15 interior + 1 leaf
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn transform_deep_tree_depth() {
    let (tree, src) = deep_tree(10);
    let tw = TransformWalker::new(&src);
    let mut t = DepthTransform;
    let depth = tw.walk(&tree, &mut t);
    assert_eq!(depth, 11); // 10 interior + 1 leaf
}

// ===================================================================
// Wide tree
// ===================================================================

#[test]
fn dfs_wide_tree_visits_all() {
    let (tree, src) = wide_tree(50);
    let walker = TreeWalker::new(&src);
    let mut v = OrderVisitor::default();
    walker.walk(&tree, &mut v);
    // root + 50 leaves
    assert_eq!(v.entered.len(), 51);
}

#[test]
fn bfs_wide_tree_visits_all() {
    let (tree, src) = wide_tree(50);
    let walker = BreadthFirstWalker::new(&src);
    let mut v = OrderVisitor::default();
    walker.walk(&tree, &mut v);
    assert_eq!(v.entered.len(), 51);
}

#[test]
fn stats_wide_tree() {
    let (tree, src) = wide_tree(30);
    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&tree, &mut stats);
    assert_eq!(stats.total_nodes, 31);
    assert_eq!(stats.leaf_nodes, 30);
    assert_eq!(stats.max_depth, 2); // root + leaf
}

#[test]
fn transform_wide_tree_char_count() {
    let (tree, src) = wide_tree(25);
    let tw = TransformWalker::new(&src);
    let mut t = CharCountTransform;
    assert_eq!(tw.walk(&tree, &mut t), 25);
}

// ===================================================================
// Error node handling
// ===================================================================

#[test]
fn dfs_error_node_triggers_visit_error() {
    let src = b"e".to_vec();
    let err = error_node(0, 1);
    let root = interior(10, vec![err]);

    #[derive(Default)]
    struct ErrorCounter {
        errors: usize,
    }
    impl Visitor for ErrorCounter {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn visit_error(&mut self, _: &ParsedNode) {
            self.errors += 1;
        }
    }

    let walker = TreeWalker::new(&src);
    let mut v = ErrorCounter::default();
    walker.walk(&root, &mut v);
    assert_eq!(v.errors, 1);
}

#[test]
fn bfs_error_node_triggers_visit_error() {
    let src = b"e".to_vec();
    let err = error_node(0, 1);
    let root = interior(10, vec![err]);

    #[derive(Default)]
    struct ErrorCounter {
        errors: usize,
    }
    impl Visitor for ErrorCounter {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn visit_error(&mut self, _: &ParsedNode) {
            self.errors += 1;
        }
    }

    let walker = BreadthFirstWalker::new(&src);
    let mut v = ErrorCounter::default();
    walker.walk(&root, &mut v);
    assert_eq!(v.errors, 1);
}

#[test]
fn stats_counts_error_nodes() {
    let src = b"abc".to_vec();
    let normal = leaf(1, 0, 1);
    let err = error_node(1, 2);
    let root = interior(10, vec![normal, err]);
    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
}

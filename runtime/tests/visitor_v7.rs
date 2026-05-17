//! Visitor pattern v7 — 64 tests (8 categories × 8 tests each).
//!
//! Categories:
//!   visitor_stats_*    — StatsVisitor behaviour
//!   visitor_pretty_*   — PrettyPrintVisitor behaviour
//!   visitor_search_*   — SearchVisitor behaviour
//!   visitor_custom_*   — custom Visitor implementations
//!   visitor_walker_*   — TreeWalker (depth-first) behaviour
//!   visitor_breadth_*  — BreadthFirstWalker (level-order) behaviour
//!   visitor_combined_* — composing multiple visitors / walkers
//!   visitor_edge_*     — edge cases (empty trees, errors, single nodes)

use adze::arena_allocator::{NodeHandle, TreeArena, TreeNode};
use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TransformVisitor,
    TransformWalker, TreeWalker, Visitor, VisitorAction,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Construct a synthetic `ParsedNode` with standard defaults.
#[allow(dead_code)]
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

#[allow(dead_code)]
fn leaf(sym: u16, start: usize, end: usize) -> ParsedNode {
    make_node(sym, vec![], start, end, false, true)
}

#[allow(dead_code)]
fn unnamed_leaf(sym: u16, start: usize, end: usize) -> ParsedNode {
    make_node(sym, vec![], start, end, false, false)
}

#[allow(dead_code)]
fn interior(sym: u16, children: Vec<ParsedNode>) -> ParsedNode {
    let start = children.first().map_or(0, |c| c.start_byte);
    let end = children.last().map_or(0, |c| c.end_byte);
    make_node(sym, children, start, end, false, true)
}

#[allow(dead_code)]
fn error_node(start: usize, end: usize) -> ParsedNode {
    make_node(0, vec![], start, end, true, false)
}

/// ```text
/// root(10)
///  ├── a(1) leaf "a"
///  ├── mid(11)
///  │    ├── b(2) leaf "b"
///  │    └── c(3) unnamed leaf "c"
///  └── d(4) leaf "d"
/// ```
/// Source: `"abcd"`
#[allow(dead_code)]
fn sample_tree() -> (ParsedNode, Vec<u8>) {
    let src = b"abcd".to_vec();
    let a = leaf(1, 0, 1);
    let b = leaf(2, 1, 2);
    let c = unnamed_leaf(3, 2, 3);
    let mid = interior(11, vec![b, c]);
    let d = leaf(4, 3, 4);
    let root = interior(10, vec![a, mid, d]);
    (root, src)
}

/// A deeper tree for depth-sensitive tests.
/// ```text
/// root(100)
///  └── n1(101)
///       └── n2(102)
///            └── n3(103) leaf "deep"
/// ```
#[allow(dead_code)]
fn deep_tree() -> (ParsedNode, Vec<u8>) {
    let src = b"deep".to_vec();
    let n3 = leaf(103, 0, 4);
    let n2 = interior(102, vec![n3]);
    let n1 = interior(101, vec![n2]);
    let root = interior(100, vec![n1]);
    (root, src)
}

/// A flat tree (root + many direct children) for breadth-first tests.
/// Source: `"12345"`
#[allow(dead_code)]
fn flat_tree() -> (ParsedNode, Vec<u8>) {
    let src = b"12345".to_vec();
    let children: Vec<ParsedNode> = (0..5)
        .map(|i| leaf(i + 1, i as usize, i as usize + 1))
        .collect();
    let root = interior(50, children);
    (root, src)
}

// ===================================================================
// Category 1 — visitor_stats_* (8 tests)
// ===================================================================

#[test]
fn visitor_stats_total_nodes_sample_tree() {
    let (root, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    // 6 nodes: root, a, mid, b, c, d
    assert_eq!(stats.total_nodes, 6);
}

#[test]
fn visitor_stats_leaf_count_sample_tree() {
    let (root, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    // Leaves: a, b, c, d
    assert_eq!(stats.leaf_nodes, 4);
}

#[test]
fn visitor_stats_max_depth_sample_tree() {
    let (root, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    // root=1, mid=2, b/c=3; max_depth ≥ 2
    assert!(stats.max_depth >= 2);
}

#[test]
fn visitor_stats_error_nodes_counted() {
    let err = error_node(0, 1);
    let root = interior(10, vec![leaf(1, 0, 1), err]);
    let src = b"xe".to_vec();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
}

#[test]
fn visitor_stats_deep_tree_max_depth() {
    let (root, src) = deep_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    // 4 levels: root, n1, n2, n3
    assert!(stats.max_depth >= 4);
}

#[test]
fn visitor_stats_single_leaf() {
    let l = leaf(1, 0, 1);
    let src = b"x".to_vec();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&l, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn visitor_stats_node_counts_map_populated() {
    let (root, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    // node_counts should have at least one entry
    assert!(!stats.node_counts.is_empty());
}

#[test]
fn visitor_stats_breadth_first_same_totals() {
    let (root, src) = sample_tree();
    let mut dfs = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut dfs);

    let mut bfs = StatsVisitor::default();
    BreadthFirstWalker::new(&src).walk(&root, &mut bfs);

    assert_eq!(dfs.total_nodes, bfs.total_nodes);
    assert_eq!(dfs.leaf_nodes, bfs.leaf_nodes);
}

// ===================================================================
// Category 2 — visitor_pretty_* (8 tests)
// ===================================================================

#[test]
fn visitor_pretty_new_output_empty() {
    let pp = PrettyPrintVisitor::new();
    assert!(pp.output().is_empty());
}

#[test]
fn visitor_pretty_default_output_empty() {
    let pp = PrettyPrintVisitor::default();
    assert!(pp.output().is_empty());
}

#[test]
fn visitor_pretty_single_leaf_has_content() {
    let l = leaf(1, 0, 1);
    let src = b"x".to_vec();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&l, &mut pp);
    assert!(!pp.output().is_empty());
}

#[test]
fn visitor_pretty_leaf_text_appears() {
    let l = leaf(1, 0, 3);
    let src = b"abc".to_vec();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&l, &mut pp);
    assert!(pp.output().contains("abc"));
}

#[test]
fn visitor_pretty_indentation_increases() {
    let (root, src) = sample_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    let output = pp.output();
    // nested nodes should have indented lines (starting with spaces)
    let indented = output.lines().filter(|l| l.starts_with("  ")).count();
    assert!(indented > 0);
}

#[test]
fn visitor_pretty_named_marker_present() {
    let l = leaf(1, 0, 1);
    let src = b"x".to_vec();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&l, &mut pp);
    // Named nodes get a "[named]" annotation
    assert!(pp.output().contains("[named]"));
}

#[test]
fn visitor_pretty_unnamed_no_named_marker() {
    let l = unnamed_leaf(1, 0, 1);
    let src = b"x".to_vec();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&l, &mut pp);
    // Unnamed leaf's enter_node line should NOT contain [named]
    let lines: Vec<&str> = pp.output().lines().collect();
    // First line is the enter_node output for the unnamed node
    let first = lines.first().unwrap_or(&"");
    assert!(!first.contains("[named]"));
}

#[test]
fn visitor_pretty_multiline_output() {
    let (root, src) = sample_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    let line_count = pp.output().lines().count();
    // Multiple nodes → multiple lines
    assert!(line_count >= 6);
}

// ===================================================================
// Category 3 — visitor_search_* (8 tests)
// ===================================================================

#[test]
fn visitor_search_no_match_empty_results() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|_: &ParsedNode| false);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert!(search.matches.is_empty());
}

#[test]
fn visitor_search_match_all() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|_: &ParsedNode| true);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 6);
}

#[test]
fn visitor_search_by_symbol() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 2);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 1);
}

#[test]
fn visitor_search_captures_byte_range() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 4);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 1);
    let (start, end, _) = &search.matches[0];
    assert_eq!(*start, 3);
    assert_eq!(*end, 4);
}

#[test]
fn visitor_search_multiple_matches() {
    let (root, src) = sample_tree();
    // Match leaves (nodes with no children → symbol ∈ {1,2,3,4})
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.child_count() == 0);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 4);
}

#[test]
fn visitor_search_named_only() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.is_named());
    TreeWalker::new(&src).walk(&root, &mut search);
    // c(3) is unnamed; 5 named nodes
    assert_eq!(search.matches.len(), 5);
}

#[test]
fn visitor_search_breadth_first_same_count() {
    let (root, src) = sample_tree();
    let pred = |n: &ParsedNode| n.child_count() == 0;

    let mut dfs_s = SearchVisitor::new(pred);
    TreeWalker::new(&src).walk(&root, &mut dfs_s);

    let mut bfs_s = SearchVisitor::new(pred);
    BreadthFirstWalker::new(&src).walk(&root, &mut bfs_s);

    assert_eq!(dfs_s.matches.len(), bfs_s.matches.len());
}

#[test]
fn visitor_search_error_nodes_not_entered() {
    let err = error_node(0, 1);
    let root = interior(10, vec![err, leaf(1, 1, 2)]);
    let src = b"ex".to_vec();
    let mut search = SearchVisitor::new(|_: &ParsedNode| true);
    TreeWalker::new(&src).walk(&root, &mut search);
    // Error nodes trigger visit_error, not enter_node, so predicate won't fire
    // Only root + leaf(1) enter
    assert_eq!(search.matches.len(), 2);
}

// ===================================================================
// Category 4 — visitor_custom_* (8 tests)
// ===================================================================

#[test]
fn visitor_custom_counting_visitor() {
    #[allow(dead_code)]
    struct Counter(usize);
    impl Visitor for Counter {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.0 += 1;
            VisitorAction::Continue
        }
    }
    let (root, src) = sample_tree();
    let mut c = Counter(0);
    TreeWalker::new(&src).walk(&root, &mut c);
    assert_eq!(c.0, 6);
}

#[test]
fn visitor_custom_symbol_collector() {
    struct Syms(Vec<u16>);
    impl Visitor for Syms {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            VisitorAction::Continue
        }
    }
    let (root, src) = sample_tree();
    let mut s = Syms(Vec::new());
    TreeWalker::new(&src).walk(&root, &mut s);
    // DFS pre-order: root(10), a(1), mid(11), b(2), c(3), d(4)
    assert_eq!(s.0, vec![10, 1, 11, 2, 3, 4]);
}

#[test]
fn visitor_custom_leaf_text_collector() {
    struct Leaves(Vec<String>);
    impl Visitor for Leaves {
        fn visit_leaf(&mut self, _: &ParsedNode, text: &str) {
            self.0.push(text.to_string());
        }
    }
    let (root, src) = sample_tree();
    let mut lv = Leaves(Vec::new());
    TreeWalker::new(&src).walk(&root, &mut lv);
    assert_eq!(lv.0, vec!["a", "b", "c", "d"]);
}

#[test]
fn visitor_custom_skip_subtree() {
    struct SkipMid(Vec<u16>);
    impl Visitor for SkipMid {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            if n.symbol() == 11 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }
    let (root, src) = sample_tree();
    let mut s = SkipMid(Vec::new());
    TreeWalker::new(&src).walk(&root, &mut s);
    // mid's children (b, c) are skipped
    assert_eq!(s.0, vec![10, 1, 11, 4]);
}

#[test]
fn visitor_custom_leave_order() {
    struct LeaveOrder(Vec<u16>);
    impl Visitor for LeaveOrder {
        fn leave_node(&mut self, n: &ParsedNode) {
            self.0.push(n.symbol());
        }
    }
    let (root, src) = sample_tree();
    let mut lo = LeaveOrder(Vec::new());
    TreeWalker::new(&src).walk(&root, &mut lo);
    // DFS post-order: a(1), b(2), c(3), mid(11), d(4), root(10)
    assert_eq!(lo.0, vec![1, 2, 3, 11, 4, 10]);
}

#[test]
fn visitor_custom_error_counter() {
    struct ErrCount(usize);
    impl Visitor for ErrCount {
        fn visit_error(&mut self, _: &ParsedNode) {
            self.0 += 1;
        }
    }
    let root = interior(10, vec![error_node(0, 1), error_node(1, 2), leaf(1, 2, 3)]);
    let src = b"eex".to_vec();
    let mut ec = ErrCount(0);
    TreeWalker::new(&src).walk(&root, &mut ec);
    assert_eq!(ec.0, 2);
}

#[test]
fn visitor_custom_transform_sum() {
    struct Summer;
    impl TransformVisitor for Summer {
        type Output = i64;
        fn transform_node(&mut self, _: &ParsedNode, children: Vec<i64>) -> i64 {
            children.iter().sum()
        }
        fn transform_leaf(&mut self, node: &ParsedNode, _text: &str) -> i64 {
            node.symbol() as i64
        }
        fn transform_error(&mut self, _: &ParsedNode) -> i64 {
            0
        }
    }
    let (root, src) = sample_tree();
    let mut summer = Summer;
    let total = TransformWalker::new(&src).walk(&root, &mut summer);
    // leaves: 1+2+3+4 = 10
    assert_eq!(total, 10);
}

#[test]
fn visitor_custom_transform_depth() {
    struct DepthCalc;
    impl TransformVisitor for DepthCalc {
        type Output = usize;
        fn transform_node(&mut self, _: &ParsedNode, children: Vec<usize>) -> usize {
            children.iter().copied().max().unwrap_or(0) + 1
        }
        fn transform_leaf(&mut self, _: &ParsedNode, _: &str) -> usize {
            1
        }
        fn transform_error(&mut self, _: &ParsedNode) -> usize {
            1
        }
    }
    let (root, src) = deep_tree();
    let mut dc = DepthCalc;
    let depth = TransformWalker::new(&src).walk(&root, &mut dc);
    assert_eq!(depth, 4);
}

// ===================================================================
// Category 5 — visitor_walker_* (depth-first TreeWalker, 8 tests)
// ===================================================================

#[test]
fn visitor_walker_new_does_not_panic() {
    let _w = TreeWalker::new(b"hello");
}

#[test]
fn visitor_walker_empty_source() {
    let l = leaf(1, 0, 0);
    let src: Vec<u8> = vec![];
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&l, &mut stats);
    assert_eq!(stats.total_nodes, 1);
}

#[test]
fn visitor_walker_preorder_sequence() {
    struct Seq(Vec<u16>);
    impl Visitor for Seq {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            VisitorAction::Continue
        }
    }
    let (root, src) = sample_tree();
    let mut s = Seq(Vec::new());
    TreeWalker::new(&src).walk(&root, &mut s);
    assert_eq!(s.0, vec![10, 1, 11, 2, 3, 4]);
}

#[test]
fn visitor_walker_postorder_sequence() {
    struct Post(Vec<u16>);
    impl Visitor for Post {
        fn leave_node(&mut self, n: &ParsedNode) {
            self.0.push(n.symbol());
        }
    }
    let (root, src) = sample_tree();
    let mut p = Post(Vec::new());
    TreeWalker::new(&src).walk(&root, &mut p);
    assert_eq!(p.0, vec![1, 2, 3, 11, 4, 10]);
}

#[test]
fn visitor_walker_leaf_text_deep() {
    struct LT(Vec<String>);
    impl Visitor for LT {
        fn visit_leaf(&mut self, _: &ParsedNode, text: &str) {
            self.0.push(text.to_string());
        }
    }
    let (root, src) = deep_tree();
    let mut lt = LT(Vec::new());
    TreeWalker::new(&src).walk(&root, &mut lt);
    assert_eq!(lt.0, vec!["deep"]);
}

#[test]
fn visitor_walker_stop_at_symbol() {
    struct StopAt(u16, Vec<u16>);
    impl Visitor for StopAt {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.1.push(n.symbol());
            if n.symbol() == self.0 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }
    let (root, src) = sample_tree();
    let mut v = StopAt(11, Vec::new());
    TreeWalker::new(&src).walk(&root, &mut v);
    // After Stop at mid(11), DFS returns up; remaining siblings still entered
    assert!(v.1.contains(&11));
}

#[test]
fn visitor_walker_skip_children_no_leaves() {
    struct SkipAll {
        leaves: usize,
    }
    impl Visitor for SkipAll {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            VisitorAction::SkipChildren
        }
        fn visit_leaf(&mut self, _: &ParsedNode, _: &str) {
            self.leaves += 1;
        }
    }
    let (root, src) = sample_tree();
    let mut v = SkipAll { leaves: 0 };
    TreeWalker::new(&src).walk(&root, &mut v);
    // SkipChildren at root → no children visited at all
    assert_eq!(v.leaves, 0);
}

#[test]
fn visitor_walker_flat_tree_all_visited() {
    let (root, src) = flat_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    // root + 5 children
    assert_eq!(stats.total_nodes, 6);
}

// ===================================================================
// Category 6 — visitor_breadth_* (BreadthFirstWalker, 8 tests)
// ===================================================================

#[test]
fn visitor_breadth_new_does_not_panic() {
    let _w = BreadthFirstWalker::new(b"hello");
}

#[test]
fn visitor_breadth_level_order() {
    struct Seq(Vec<u16>);
    impl Visitor for Seq {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            VisitorAction::Continue
        }
    }
    let (root, src) = sample_tree();
    let mut s = Seq(Vec::new());
    BreadthFirstWalker::new(&src).walk(&root, &mut s);
    // BFS: root(10), a(1), mid(11), d(4), b(2), c(3)
    assert_eq!(s.0, vec![10, 1, 11, 4, 2, 3]);
}

#[test]
fn visitor_breadth_stop_halts_immediately() {
    struct StopAfter(usize, usize);
    impl Visitor for StopAfter {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.1 += 1;
            if self.1 >= self.0 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }
    let (root, src) = sample_tree();
    let mut v = StopAfter(3, 0);
    BreadthFirstWalker::new(&src).walk(&root, &mut v);
    assert_eq!(v.1, 3);
}

#[test]
fn visitor_breadth_skip_children_prevents_enqueue() {
    struct SkipMid(Vec<u16>);
    impl Visitor for SkipMid {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            if n.symbol() == 11 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }
    let (root, src) = sample_tree();
    let mut v = SkipMid(Vec::new());
    BreadthFirstWalker::new(&src).walk(&root, &mut v);
    // mid's children (b=2, c=3) should not appear
    assert!(!v.0.contains(&2));
    assert!(!v.0.contains(&3));
}

#[test]
fn visitor_breadth_flat_tree_order() {
    struct Seq(Vec<u16>);
    impl Visitor for Seq {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            VisitorAction::Continue
        }
    }
    let (root, src) = flat_tree();
    let mut s = Seq(Vec::new());
    BreadthFirstWalker::new(&src).walk(&root, &mut s);
    // root=50, then children 1..5
    assert_eq!(s.0, vec![50, 1, 2, 3, 4, 5]);
}

#[test]
fn visitor_breadth_leaf_text_collected() {
    struct Leaves(Vec<String>);
    impl Visitor for Leaves {
        fn visit_leaf(&mut self, _: &ParsedNode, text: &str) {
            self.0.push(text.to_string());
        }
    }
    let (root, src) = sample_tree();
    let mut lv = Leaves(Vec::new());
    BreadthFirstWalker::new(&src).walk(&root, &mut lv);
    // BFS leaf order: a, d, b, c
    assert_eq!(lv.0, vec!["a", "d", "b", "c"]);
}

#[test]
fn visitor_breadth_deep_tree_visits_all() {
    let (root, src) = deep_tree();
    let mut stats = StatsVisitor::default();
    BreadthFirstWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 4);
}

#[test]
fn visitor_breadth_error_triggers_visit_error() {
    struct ErrFlag(bool);
    impl Visitor for ErrFlag {
        fn visit_error(&mut self, _: &ParsedNode) {
            self.0 = true;
        }
    }
    let root = interior(10, vec![error_node(0, 1)]);
    let src = b"e".to_vec();
    let mut ef = ErrFlag(false);
    BreadthFirstWalker::new(&src).walk(&root, &mut ef);
    assert!(ef.0);
}

// ===================================================================
// Category 7 — visitor_combined_* (8 tests)
// ===================================================================

#[test]
fn visitor_combined_stats_then_pretty() {
    let (root, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);

    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);

    assert!(stats.total_nodes > 0);
    assert!(!pp.output().is_empty());
}

#[test]
fn visitor_combined_search_then_stats() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.is_named());
    TreeWalker::new(&src).walk(&root, &mut search);

    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);

    assert!(search.matches.len() <= stats.total_nodes);
}

#[test]
fn visitor_combined_dfs_bfs_same_leaf_set() {
    let (root, src) = sample_tree();
    struct Leaves(Vec<String>);
    impl Visitor for Leaves {
        fn visit_leaf(&mut self, _: &ParsedNode, text: &str) {
            self.0.push(text.to_string());
        }
    }
    let mut dfs = Leaves(Vec::new());
    TreeWalker::new(&src).walk(&root, &mut dfs);

    let mut bfs = Leaves(Vec::new());
    BreadthFirstWalker::new(&src).walk(&root, &mut bfs);

    let mut dfs_sorted = dfs.0.clone();
    dfs_sorted.sort();
    let mut bfs_sorted = bfs.0.clone();
    bfs_sorted.sort();
    assert_eq!(dfs_sorted, bfs_sorted);
}

#[test]
fn visitor_combined_transform_and_stats() {
    struct Sizer;
    impl TransformVisitor for Sizer {
        type Output = usize;
        fn transform_node(&mut self, _: &ParsedNode, ch: Vec<usize>) -> usize {
            ch.iter().sum::<usize>() + 1
        }
        fn transform_leaf(&mut self, _: &ParsedNode, _: &str) -> usize {
            1
        }
        fn transform_error(&mut self, _: &ParsedNode) -> usize {
            1
        }
    }
    let (root, src) = sample_tree();
    let mut sizer = Sizer;
    let total_from_transform = TransformWalker::new(&src).walk(&root, &mut sizer);

    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);

    assert_eq!(total_from_transform, stats.total_nodes);
}

#[test]
fn visitor_combined_arena_and_visitor() {
    let mut arena = TreeArena::new();
    let c1 = arena.alloc(TreeNode::leaf(1));
    let c2 = arena.alloc(TreeNode::leaf(2));
    let _parent = arena.alloc(TreeNode::branch(vec![c1, c2]));

    // Run visitor on a ParsedNode tree independently
    let (root, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);

    assert_eq!(arena.len(), 3);
    assert!(stats.total_nodes > 0);
}

#[test]
fn visitor_combined_pretty_deep_tree() {
    let (root, src) = deep_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    // Deep nesting → deep indentation
    let max_indent = pp
        .output()
        .lines()
        .map(|l| l.len() - l.trim_start().len())
        .max()
        .unwrap_or(0);
    assert!(max_indent >= 4);
}

#[test]
fn visitor_combined_search_flat_tree() {
    let (root, src) = flat_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.child_count() == 0);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 5);
}

#[test]
fn visitor_combined_two_transforms() {
    struct CountNodes;
    impl TransformVisitor for CountNodes {
        type Output = usize;
        fn transform_node(&mut self, _: &ParsedNode, ch: Vec<usize>) -> usize {
            ch.iter().sum::<usize>() + 1
        }
        fn transform_leaf(&mut self, _: &ParsedNode, _: &str) -> usize {
            1
        }
        fn transform_error(&mut self, _: &ParsedNode) -> usize {
            1
        }
    }
    struct MaxDepth;
    impl TransformVisitor for MaxDepth {
        type Output = usize;
        fn transform_node(&mut self, _: &ParsedNode, ch: Vec<usize>) -> usize {
            ch.iter().copied().max().unwrap_or(0) + 1
        }
        fn transform_leaf(&mut self, _: &ParsedNode, _: &str) -> usize {
            1
        }
        fn transform_error(&mut self, _: &ParsedNode) -> usize {
            1
        }
    }
    let (root, src) = sample_tree();
    let count = TransformWalker::new(&src).walk(&root, &mut CountNodes);
    let depth = TransformWalker::new(&src).walk(&root, &mut MaxDepth);
    assert_eq!(count, 6);
    assert_eq!(depth, 3);
}

// ===================================================================
// Category 8 — visitor_edge_* (8 tests)
// ===================================================================

#[test]
fn visitor_edge_single_error_node() {
    let err = error_node(0, 1);
    let src = b"e".to_vec();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&err, &mut stats);
    assert_eq!(stats.error_nodes, 1);
    assert_eq!(stats.total_nodes, 0);
}

#[test]
fn visitor_edge_empty_interior() {
    let node = interior(99, vec![]);
    let src: Vec<u8> = vec![];
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&node, &mut stats);
    // Interior with no children → treated as leaf by walker
    assert_eq!(stats.total_nodes, 1);
}

#[test]
fn visitor_edge_arena_new_empty() {
    let arena = TreeArena::new();
    assert!(arena.is_empty());
    assert_eq!(arena.len(), 0);
}

#[test]
fn visitor_edge_arena_alloc_and_get() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(42));
    assert_eq!(arena.get(h).value(), 42);
    assert!(!arena.is_empty());
}

#[test]
fn visitor_edge_arena_branch_children() {
    let mut arena = TreeArena::new();
    let c1 = arena.alloc(TreeNode::leaf(1));
    let c2 = arena.alloc(TreeNode::leaf(2));
    let parent = arena.alloc(TreeNode::branch(vec![c1, c2]));
    let parent_ref = arena.get(parent);
    let children = parent_ref.children();
    assert_eq!(children.len(), 2);
    assert_eq!(children[0], c1);
    assert_eq!(children[1], c2);
}

#[test]
fn visitor_edge_node_handle_is_copy() {
    let mut arena = TreeArena::new();
    let h: NodeHandle = arena.alloc(TreeNode::leaf(7));
    let h2 = h; // Copy
    assert_eq!(arena.get(h).value(), arena.get(h2).value());
}

#[test]
fn visitor_edge_arena_reset_clears() {
    let mut arena = TreeArena::new();
    arena.alloc(TreeNode::leaf(1));
    arena.alloc(TreeNode::leaf(2));
    assert_eq!(arena.len(), 2);
    arena.reset();
    assert_eq!(arena.len(), 0);
    assert!(arena.is_empty());
}

#[test]
fn visitor_edge_arena_metrics() {
    let mut arena = TreeArena::new();
    arena.alloc(TreeNode::leaf(1));
    arena.alloc(TreeNode::leaf(2));
    arena.alloc(TreeNode::leaf(3));
    let m = arena.metrics();
    assert_eq!(m.len(), 3);
    assert!(!m.is_empty());
    assert!(m.capacity() >= 3);
    assert!(m.num_chunks() >= 1);
    assert!(m.memory_usage() > 0);
}

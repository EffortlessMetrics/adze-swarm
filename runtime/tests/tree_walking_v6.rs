//! Tree walking v6 — 64 `#[test]` functions exercising tree visitor/walker
//! infrastructure across depth-first, breadth-first, stats, pretty-print,
//! search, empty-tree, deeply-nested, and mixed visitor/walker scenarios.

use adze::arena_allocator::{TreeArena, TreeNode};
use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TransformVisitor,
    TransformWalker, TreeWalker, Visitor, VisitorAction,
};

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

fn leaf(sym: u16, start: usize, end: usize) -> ParsedNode {
    make_node(sym, vec![], start, end, false, true)
}

fn anon_leaf(sym: u16, start: usize, end: usize) -> ParsedNode {
    make_node(sym, vec![], start, end, false, false)
}

fn interior(sym: u16, children: Vec<ParsedNode>) -> ParsedNode {
    let start = children.first().map_or(0, |c| c.start_byte);
    let end = children.last().map_or(0, |c| c.end_byte);
    make_node(sym, children, start, end, false, true)
}

fn error_node(start: usize, end: usize) -> ParsedNode {
    make_node(0, vec![], start, end, true, false)
}

/// Build a left-skewed chain: root wraps one child repeated `depth` times.
fn left_chain(depth: usize) -> ParsedNode {
    let mut node = leaf(1, 0, 1);
    for i in 1..depth {
        node = interior((i + 1) as u16, vec![node]);
    }
    node
}

/// `root(10)( a(1)[0..1], mid(11)( b(2)[1..2], c(3)[2..3] ), d(4)[3..4] )`
/// source = "abcd"
fn sample_tree() -> (ParsedNode, Vec<u8>) {
    let src = b"abcd".to_vec();
    let a = leaf(1, 0, 1);
    let b = leaf(2, 1, 2);
    let c = anon_leaf(3, 2, 3);
    let mid = interior(11, vec![b, c]);
    let d = leaf(4, 3, 4);
    let root = interior(10, vec![a, mid, d]);
    (root, src)
}

// ---------------------------------------------------------------------------
// Reusable visitors
// ---------------------------------------------------------------------------

/// Records symbol ids in `enter_node` order.
struct SymRecorder(Vec<u16>);

impl Visitor for SymRecorder {
    fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
        self.0.push(node.symbol());
        VisitorAction::Continue
    }
}

/// Records leaf text.
struct LeafRecorder(Vec<String>);

impl Visitor for LeafRecorder {
    fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
        VisitorAction::Continue
    }
    fn visit_leaf(&mut self, _: &ParsedNode, text: &str) {
        self.0.push(text.to_string());
    }
}

/// Counts enter/leave/leaf/error calls.
struct CallCounter {
    enters: usize,
    leaves: usize,
    leaf_visits: usize,
    errors: usize,
}

impl CallCounter {
    fn new() -> Self {
        Self {
            enters: 0,
            leaves: 0,
            leaf_visits: 0,
            errors: 0,
        }
    }
}

impl Visitor for CallCounter {
    fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
        self.enters += 1;
        VisitorAction::Continue
    }
    fn leave_node(&mut self, _: &ParsedNode) {
        self.leaves += 1;
    }
    fn visit_leaf(&mut self, _: &ParsedNode, _: &str) {
        self.leaf_visits += 1;
    }
    fn visit_error(&mut self, _: &ParsedNode) {
        self.errors += 1;
    }
}

// ===================================================================
// Category 1 — walk_depth_* (depth-first traversal, 8 tests)
// ===================================================================

#[test]
fn walk_depth_pre_order_symbol_sequence() {
    let (root, src) = sample_tree();
    let mut rec = SymRecorder(vec![]);
    TreeWalker::new(&src).walk(&root, &mut rec);
    // DFS pre-order: root(10), a(1), mid(11), b(2), c(3), d(4)
    assert_eq!(rec.0, vec![10, 1, 11, 2, 3, 4]);
}

#[test]
fn walk_depth_leaf_text_order() {
    let (root, src) = sample_tree();
    let mut rec = LeafRecorder(vec![]);
    TreeWalker::new(&src).walk(&root, &mut rec);
    assert_eq!(rec.0, vec!["a", "b", "c", "d"]);
}

#[test]
fn walk_depth_enter_leave_balanced() {
    let (root, src) = sample_tree();
    let mut cc = CallCounter::new();
    TreeWalker::new(&src).walk(&root, &mut cc);
    assert_eq!(cc.enters, cc.leaves);
}

#[test]
fn walk_depth_single_leaf() {
    let node = leaf(42, 0, 2);
    let src = b"hi";
    let mut rec = SymRecorder(vec![]);
    TreeWalker::new(src).walk(&node, &mut rec);
    assert_eq!(rec.0, vec![42]);
}

#[test]
fn walk_depth_error_node_triggers_visit_error() {
    let err = error_node(0, 3);
    let src = b"bad";
    let mut cc = CallCounter::new();
    TreeWalker::new(src).walk(&err, &mut cc);
    assert_eq!(cc.errors, 1);
    assert_eq!(cc.enters, 0);
}

#[test]
fn walk_depth_error_inside_tree() {
    let err = error_node(1, 2);
    let root = interior(10, vec![leaf(1, 0, 1), err, leaf(3, 2, 3)]);
    let src = b"abc";
    let mut cc = CallCounter::new();
    TreeWalker::new(src).walk(&root, &mut cc);
    assert_eq!(cc.errors, 1);
}

#[test]
fn walk_depth_wide_tree() {
    let children: Vec<_> = (0..10)
        .map(|i| leaf(i, i as usize, i as usize + 1))
        .collect();
    let root = interior(100, children);
    let src: Vec<u8> = (0..10).map(|i| b'a' + i).collect();
    let mut rec = SymRecorder(vec![]);
    TreeWalker::new(&src).walk(&root, &mut rec);
    assert_eq!(rec.0.len(), 11); // root + 10 children
    assert_eq!(rec.0[0], 100);
}

#[test]
fn walk_depth_skip_children_prevents_descent() {
    let (root, src) = sample_tree();
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
    let mut v = SkipMid(vec![]);
    TreeWalker::new(&src).walk(&root, &mut v);
    // b(2) and c(3) should not appear
    assert!(!v.0.contains(&2));
    assert!(!v.0.contains(&3));
    assert!(v.0.contains(&11));
    assert!(v.0.contains(&4));
}

// ===================================================================
// Category 2 — walk_breadth_* (breadth-first traversal, 8 tests)
// ===================================================================

#[test]
fn walk_breadth_level_order_symbols() {
    let (root, src) = sample_tree();
    let mut rec = SymRecorder(vec![]);
    BreadthFirstWalker::new(&src).walk(&root, &mut rec);
    // BFS: root(10), a(1), mid(11), d(4), b(2), c(3)
    assert_eq!(rec.0, vec![10, 1, 11, 4, 2, 3]);
}

#[test]
fn walk_breadth_leaf_text_order() {
    let (root, src) = sample_tree();
    let mut rec = LeafRecorder(vec![]);
    BreadthFirstWalker::new(&src).walk(&root, &mut rec);
    // BFS visits a, d before b, c
    assert_eq!(rec.0, vec!["a", "d", "b", "c"]);
}

#[test]
fn walk_breadth_single_leaf() {
    let node = leaf(7, 0, 1);
    let src = b"x";
    let mut rec = SymRecorder(vec![]);
    BreadthFirstWalker::new(src).walk(&node, &mut rec);
    assert_eq!(rec.0, vec![7]);
}

#[test]
fn walk_breadth_error_node_handled() {
    let err = error_node(0, 2);
    let src = b"??";
    let mut cc = CallCounter::new();
    BreadthFirstWalker::new(src).walk(&err, &mut cc);
    assert_eq!(cc.errors, 1);
    assert_eq!(cc.enters, 0);
}

#[test]
fn walk_breadth_skip_children_skips_subtree() {
    let (root, src) = sample_tree();
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
    let mut v = SkipMid(vec![]);
    BreadthFirstWalker::new(&src).walk(&root, &mut v);
    // mid(11) is visited but b,c should not be enqueued
    assert!(v.0.contains(&11));
    assert!(!v.0.contains(&2));
    assert!(!v.0.contains(&3));
}

#[test]
fn walk_breadth_stop_halts_immediately() {
    let (root, src) = sample_tree();
    struct StopAt2(usize);
    impl Visitor for StopAt2 {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.0 += 1;
            if self.0 >= 2 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }
    let mut v = StopAt2(0);
    BreadthFirstWalker::new(&src).walk(&root, &mut v);
    assert_eq!(v.0, 2);
}

#[test]
fn walk_breadth_wide_tree_count() {
    let children: Vec<_> = (0..8)
        .map(|i| leaf(i, i as usize, i as usize + 1))
        .collect();
    let root = interior(99, children);
    let src = b"abcdefgh";
    let mut cc = CallCounter::new();
    BreadthFirstWalker::new(src).walk(&root, &mut cc);
    assert_eq!(cc.enters, 9);
}

#[test]
fn walk_breadth_nested_two_levels() {
    // root -> [left -> [ll], right]
    let ll = leaf(1, 0, 1);
    let left = interior(2, vec![ll]);
    let right = leaf(3, 1, 2);
    let root = interior(4, vec![left, right]);
    let src = b"ab";
    let mut rec = SymRecorder(vec![]);
    BreadthFirstWalker::new(src).walk(&root, &mut rec);
    // BFS: 4, 2, 3, 1
    assert_eq!(rec.0, vec![4, 2, 3, 1]);
}

// ===================================================================
// Category 3 — visit_stats_* (StatsVisitor, 8 tests)
// ===================================================================

#[test]
fn visit_stats_total_nodes_sample_tree() {
    let (root, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 6);
}

#[test]
fn visit_stats_leaf_count() {
    let (root, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.leaf_nodes, 4);
}

#[test]
fn visit_stats_max_depth_sample() {
    let (root, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    // root(1) -> mid(2) -> b/c(3) => max_depth 3
    assert_eq!(stats.max_depth, 3);
}

#[test]
fn visit_stats_error_count() {
    let err = error_node(0, 1);
    let root = interior(10, vec![leaf(1, 0, 1), err]);
    let src = b"xy";
    let mut stats = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
}

#[test]
fn visit_stats_single_leaf() {
    let node = leaf(5, 0, 1);
    let src = b"z";
    let mut stats = StatsVisitor::default();
    TreeWalker::new(src).walk(&node, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
    assert_eq!(stats.max_depth, 1);
}

#[test]
fn visit_stats_node_counts_map() {
    let (root, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    // Each kind string visited at least once
    assert!(!stats.node_counts.is_empty());
}

#[test]
fn visit_stats_depth_resets_after_leave() {
    // Two sibling leaves under root — both at depth 2
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let src = b"ab";
    let mut stats = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut stats);
    assert_eq!(stats.max_depth, 2);
}

#[test]
fn visit_stats_breadth_first_same_totals() {
    let (root, src) = sample_tree();
    let mut dfs = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut dfs);
    let mut bfs = StatsVisitor::default();
    BreadthFirstWalker::new(&src).walk(&root, &mut bfs);
    // Total node + leaf counts should agree (BFS also calls enter_node/visit_leaf).
    assert_eq!(dfs.total_nodes, bfs.total_nodes);
    assert_eq!(dfs.leaf_nodes, bfs.leaf_nodes);
}

// ===================================================================
// Category 4 — visit_pretty_* (PrettyPrintVisitor, 8 tests)
// ===================================================================

#[test]
fn visit_pretty_contains_leaf_text() {
    let (root, src) = sample_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    let output = pp.output();
    assert!(output.contains("\"a\""));
    assert!(output.contains("\"b\""));
}

#[test]
fn visit_pretty_output_not_empty() {
    let node = leaf(1, 0, 1);
    let src = b"x";
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&node, &mut pp);
    assert!(!pp.output().is_empty());
}

#[test]
fn visit_pretty_indentation_increases() {
    let child = leaf(2, 0, 1);
    let root = interior(1, vec![child]);
    let src = b"a";
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&root, &mut pp);
    let lines: Vec<&str> = pp.output().lines().collect();
    // First line at indent 0, second at indent >= 1
    assert!(lines.len() >= 2);
    assert!(!lines[0].starts_with(' '));
    assert!(lines[1].starts_with(' '));
}

#[test]
fn visit_pretty_default_equivalent_to_new() {
    let a = PrettyPrintVisitor::new();
    let b = PrettyPrintVisitor::default();
    assert_eq!(a.output(), b.output());
}

#[test]
fn visit_pretty_named_annotation() {
    let named = leaf(1, 0, 1);
    let root = interior(10, vec![named]);
    let src = b"a";
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&root, &mut pp);
    // Named nodes get "[named]" annotation in the output
    assert!(pp.output().contains("[named]"));
}

#[test]
fn visit_pretty_error_node_label() {
    let err = error_node(0, 1);
    let root = interior(10, vec![err]);
    let src = b"!";
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&root, &mut pp);
    assert!(pp.output().contains("ERROR"));
}

#[test]
fn visit_pretty_multiline_output() {
    let (root, src) = sample_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    let line_count = pp.output().lines().count();
    // At least one line per non-error node + one per leaf text
    assert!(line_count >= 6);
}

#[test]
fn visit_pretty_leaf_text_quoted() {
    let node = leaf(1, 0, 3);
    let src = b"foo";
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&node, &mut pp);
    assert!(pp.output().contains("\"foo\""));
}

// ===================================================================
// Category 5 — visit_search_* (SearchVisitor, 8 tests)
// ===================================================================

#[test]
fn visit_search_finds_by_symbol() {
    let (root, src) = sample_tree();
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 2);
    TreeWalker::new(&src).walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
}

#[test]
fn visit_search_finds_multiple() {
    let (root, src) = sample_tree();
    // Find all named nodes
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.is_named());
    TreeWalker::new(&src).walk(&root, &mut sv);
    // root, a, mid, b, d are named; c is anon
    assert_eq!(sv.matches.len(), 5);
}

#[test]
fn visit_search_no_match() {
    let (root, src) = sample_tree();
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 255);
    TreeWalker::new(&src).walk(&root, &mut sv);
    assert!(sv.matches.is_empty());
}

#[test]
fn visit_search_match_records_byte_range() {
    let node = leaf(5, 10, 20);
    let src = vec![b' '; 20];
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 5);
    TreeWalker::new(&src).walk(&node, &mut sv);
    assert_eq!(sv.matches.len(), 1);
    assert_eq!(sv.matches[0].0, 10); // start_byte
    assert_eq!(sv.matches[0].1, 20); // end_byte
}

#[test]
fn visit_search_match_records_kind() {
    let node = leaf(1, 0, 1);
    let src = b"a";
    let mut sv = SearchVisitor::new(|_: &ParsedNode| true);
    TreeWalker::new(src).walk(&node, &mut sv);
    assert!(!sv.matches.is_empty());
    // kind() should return a non-empty string
    assert!(!sv.matches[0].2.is_empty());
}

#[test]
fn visit_search_breadth_first() {
    let (root, src) = sample_tree();
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 4);
    BreadthFirstWalker::new(&src).walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
}

#[test]
fn visit_search_error_nodes_not_entered() {
    let err = error_node(0, 1);
    let root = interior(10, vec![err, leaf(1, 1, 2)]);
    let src = b"!a";
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.is_error());
    TreeWalker::new(src).walk(&root, &mut sv);
    // Error nodes trigger visit_error, not enter_node — so predicate is not called on them.
    assert!(sv.matches.is_empty());
}

#[test]
fn visit_search_all_leaves() {
    let (root, src) = sample_tree();
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.child_count() == 0);
    TreeWalker::new(&src).walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 4);
}

// ===================================================================
// Category 6 — walk_empty_* (empty / edge-case trees, 8 tests)
// ===================================================================

#[test]
fn walk_empty_interior_no_children() {
    let root = interior(10, vec![]);
    let src = b"";
    let mut cc = CallCounter::new();
    TreeWalker::new(src).walk(&root, &mut cc);
    // Interior with no children is treated as a leaf
    assert_eq!(cc.enters, 1);
}

#[test]
fn walk_empty_interior_bfs() {
    let root = interior(10, vec![]);
    let src = b"";
    let mut cc = CallCounter::new();
    BreadthFirstWalker::new(src).walk(&root, &mut cc);
    assert_eq!(cc.enters, 1);
}

#[test]
fn walk_empty_source_bytes() {
    let node = leaf(1, 0, 0);
    let src: &[u8] = b"";
    let mut rec = LeafRecorder(vec![]);
    TreeWalker::new(src).walk(&node, &mut rec);
    assert_eq!(rec.0, vec![""]);
}

#[test]
fn walk_empty_stats_on_lone_leaf() {
    let node = leaf(1, 0, 0);
    let src = b"";
    let mut stats = StatsVisitor::default();
    TreeWalker::new(src).walk(&node, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn walk_empty_pretty_print_single() {
    let node = leaf(1, 0, 0);
    let src = b"";
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&node, &mut pp);
    assert!(!pp.output().is_empty());
}

#[test]
fn walk_empty_search_on_leaf() {
    let node = leaf(1, 0, 0);
    let src = b"";
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    TreeWalker::new(src).walk(&node, &mut sv);
    assert_eq!(sv.matches.len(), 1);
}

#[test]
fn walk_empty_only_error() {
    let err = error_node(0, 0);
    let src = b"";
    let mut cc = CallCounter::new();
    TreeWalker::new(src).walk(&err, &mut cc);
    assert_eq!(cc.errors, 1);
    assert_eq!(cc.enters, 0);
    assert_eq!(cc.leaves, 0);
}

#[test]
fn walk_empty_many_empty_children() {
    let kids: Vec<_> = (0..5).map(|i| leaf(i, 0, 0)).collect();
    let root = interior(99, kids);
    let src = b"";
    let mut cc = CallCounter::new();
    TreeWalker::new(src).walk(&root, &mut cc);
    assert_eq!(cc.enters, 6);
}

// ===================================================================
// Category 7 — walk_deep_* (deeply nested, 8 tests)
// ===================================================================

#[test]
fn walk_deep_chain_dfs_visits_all() {
    let chain = left_chain(50);
    let src = b"x";
    let mut cc = CallCounter::new();
    TreeWalker::new(src).walk(&chain, &mut cc);
    assert_eq!(cc.enters, 50);
}

#[test]
fn walk_deep_chain_bfs_visits_all() {
    let chain = left_chain(50);
    let src = b"x";
    let mut cc = CallCounter::new();
    BreadthFirstWalker::new(src).walk(&chain, &mut cc);
    assert_eq!(cc.enters, 50);
}

#[test]
fn walk_deep_stats_max_depth() {
    let chain = left_chain(20);
    let src = b"x";
    let mut stats = StatsVisitor::default();
    TreeWalker::new(src).walk(&chain, &mut stats);
    assert_eq!(stats.max_depth, 20);
}

#[test]
fn walk_deep_leaf_at_bottom() {
    let chain = left_chain(30);
    let src = b"x";
    let mut rec = LeafRecorder(vec![]);
    TreeWalker::new(src).walk(&chain, &mut rec);
    assert_eq!(rec.0.len(), 1);
    assert_eq!(rec.0[0], "x");
}

#[test]
fn walk_deep_enter_leave_balanced() {
    let chain = left_chain(40);
    let src = b"x";
    let mut cc = CallCounter::new();
    TreeWalker::new(src).walk(&chain, &mut cc);
    assert_eq!(cc.enters, cc.leaves);
}

#[test]
fn walk_deep_pretty_print_indentation() {
    let chain = left_chain(5);
    let src = b"x";
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&chain, &mut pp);
    let lines: Vec<&str> = pp.output().lines().collect();
    // Deepest line should have at least 4 levels of indentation (8 spaces)
    let max_indent = lines
        .iter()
        .map(|l| l.len() - l.trim_start().len())
        .max()
        .unwrap_or(0);
    assert!(max_indent >= 8);
}

#[test]
fn walk_deep_search_finds_leaf() {
    let chain = left_chain(25);
    let src = b"x";
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    TreeWalker::new(src).walk(&chain, &mut sv);
    assert_eq!(sv.matches.len(), 1);
}

#[test]
fn walk_deep_right_chain() {
    // Right-skewed: root -> right -> right -> ... -> leaf
    fn right_chain(depth: usize) -> ParsedNode {
        let mut node = leaf(1, 0, 1);
        for i in 1..depth {
            // Add a sibling leaf and a wrapping parent
            let sibling = leaf(99, 0, 1);
            node = interior((i + 1) as u16, vec![sibling, node]);
        }
        node
    }
    let chain = right_chain(15);
    let src = b"x";
    let mut cc = CallCounter::new();
    TreeWalker::new(src).walk(&chain, &mut cc);
    // 15 interior nodes + 14 sibling leaves = 29 total
    assert_eq!(cc.enters, 15 + 14);
}

// ===================================================================
// Category 8 — walk_mixed_* (mixed visitor/walker combinations, 8 tests)
// ===================================================================

#[test]
fn walk_mixed_dfs_then_bfs_same_node_count() {
    let (root, src) = sample_tree();
    let mut dfs = CallCounter::new();
    TreeWalker::new(&src).walk(&root, &mut dfs);
    let mut bfs = CallCounter::new();
    BreadthFirstWalker::new(&src).walk(&root, &mut bfs);
    assert_eq!(dfs.enters, bfs.enters);
}

#[test]
fn walk_mixed_stats_and_pretty_agree_on_node_count() {
    let (root, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    // Pretty print produces at least one line per node + per leaf text
    let pp_lines = pp.output().lines().count();
    assert!(pp_lines >= stats.total_nodes);
}

#[test]
fn walk_mixed_search_dfs_vs_bfs_same_matches() {
    let (root, src) = sample_tree();
    let mut dfs_search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 2);
    TreeWalker::new(&src).walk(&root, &mut dfs_search);
    let mut bfs_search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 2);
    BreadthFirstWalker::new(&src).walk(&root, &mut bfs_search);
    assert_eq!(dfs_search.matches.len(), bfs_search.matches.len());
}

#[test]
fn walk_mixed_transform_evaluates_leaf_values() {
    struct SumTransform;
    impl TransformVisitor for SumTransform {
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
    let mut xform = SumTransform;
    let total = TransformWalker::new(&src).walk(&root, &mut xform);
    // Leaves: 1 + 2 + 3 + 4 = 10
    assert_eq!(total, 10);
}

#[test]
fn walk_mixed_transform_leaf_text() {
    struct TextCollector;
    impl TransformVisitor for TextCollector {
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
    let (root, src) = sample_tree();
    let mut xform = TextCollector;
    let result = TransformWalker::new(&src).walk(&root, &mut xform);
    assert!(result.contains('a'));
    assert!(result.contains('d'));
}

#[test]
fn walk_mixed_arena_and_visitor_coexist() {
    // Verify arena tree structures and visitor-based parsed trees coexist
    let mut arena = TreeArena::new();
    let h1 = arena.alloc(TreeNode::leaf(1));
    let h2 = arena.alloc(TreeNode::leaf(2));
    let _parent = arena.alloc(TreeNode::branch(vec![h1, h2]));

    let (root, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);

    assert_eq!(arena.len(), 3);
    assert_eq!(stats.total_nodes, 6);
}

#[test]
fn walk_mixed_visitor_reuse_across_trees() {
    let tree_a = interior(10, vec![leaf(1, 0, 1)]);
    let tree_b = interior(20, vec![leaf(2, 0, 1), leaf(3, 1, 2)]);
    let src_a = b"a";
    let src_b = b"bc";

    let mut cc = CallCounter::new();
    TreeWalker::new(src_a).walk(&tree_a, &mut cc);
    TreeWalker::new(src_b).walk(&tree_b, &mut cc);
    // tree_a: 2 nodes, tree_b: 3 nodes => 5 total
    assert_eq!(cc.enters, 5);
}

#[test]
fn walk_mixed_transform_with_error() {
    struct CountTransform;
    impl TransformVisitor for CountTransform {
        type Output = usize;
        fn transform_node(&mut self, _: &ParsedNode, children: Vec<usize>) -> usize {
            1 + children.iter().sum::<usize>()
        }
        fn transform_leaf(&mut self, _: &ParsedNode, _: &str) -> usize {
            1
        }
        fn transform_error(&mut self, _: &ParsedNode) -> usize {
            1
        }
    }
    let err = error_node(1, 2);
    let root = interior(10, vec![leaf(1, 0, 1), err, leaf(3, 2, 3)]);
    let src = b"abc";
    let mut xform = CountTransform;
    let count = TransformWalker::new(src).walk(&root, &mut xform);
    // root(1) + leaf(1) + error(1) + leaf(1) = 4
    assert_eq!(count, 4);
}

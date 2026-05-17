//! Tests for DFS/BFS tree walking, visitor actions, early termination,
//! skip behaviour, and custom visitors (`adze::visitor` module).

use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TransformVisitor,
    TransformWalker, TreeWalker, Visitor, VisitorAction,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a `ParsedNode`. The `language` field is `pub(crate)` so we use
/// Shared synthetic parsed-node construction.
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
    let start = children.first().map_or(0, |c| c.start_byte());
    let end = children.last().map_or(0, |c| c.end_byte());
    make_node(symbol, children, start, end, false, true)
}

fn error_node(start: usize, end: usize) -> ParsedNode {
    make_node(0, vec![], start, end, true, false)
}

// ---------------------------------------------------------------------------
// Reusable custom visitors
// ---------------------------------------------------------------------------

/// Records symbols in the order `enter_node` is called.
struct OrderRecorder {
    order: Vec<u16>,
}

impl OrderRecorder {
    fn new() -> Self {
        Self { order: Vec::new() }
    }
}

impl Visitor for OrderRecorder {
    fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
        self.order.push(node.symbol);
        VisitorAction::Continue
    }
}

/// Stops the walk when the given symbol is entered.
struct StopOnSymbol {
    target: u16,
    visited: Vec<u16>,
}

impl Visitor for StopOnSymbol {
    fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
        self.visited.push(node.symbol);
        if node.symbol == self.target {
            VisitorAction::Stop
        } else {
            VisitorAction::Continue
        }
    }
}

/// Skips children of any node whose symbol matches `skip`.
struct SkipOnSymbol {
    skip: u16,
    visited: Vec<u16>,
}

impl Visitor for SkipOnSymbol {
    fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
        self.visited.push(node.symbol);
        if node.symbol == self.skip {
            VisitorAction::SkipChildren
        } else {
            VisitorAction::Continue
        }
    }
}

/// Tracks enter/leave events for depth verification.
struct DepthEvents {
    events: Vec<String>,
}

impl DepthEvents {
    fn new() -> Self {
        Self { events: Vec::new() }
    }
}

impl Visitor for DepthEvents {
    fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
        self.events.push(format!("E{}", node.symbol));
        VisitorAction::Continue
    }
    fn leave_node(&mut self, node: &ParsedNode) {
        self.events.push(format!("L{}", node.symbol));
    }
    fn visit_leaf(&mut self, _node: &ParsedNode, text: &str) {
        self.events.push(format!("T:{text}"));
    }
    fn visit_error(&mut self, _node: &ParsedNode) {
        self.events.push("ERR".to_string());
    }
}

// ===================================================================
// 1–3. DFS ordering
// ===================================================================

#[test]
fn dfs_visits_root_first() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let mut rec = OrderRecorder::new();
    TreeWalker::new(source).walk(&root, &mut rec);
    assert_eq!(rec.order[0], 5);
}

#[test]
fn dfs_preorder_left_right() {
    let source = b"abc";
    //     5
    //    / \
    //   2   3
    //  /
    // 1
    let left = interior(2, vec![leaf(1, 0, 1)]);
    let right = leaf(3, 1, 3);
    let root = interior(5, vec![left, right]);
    let mut rec = OrderRecorder::new();
    TreeWalker::new(source).walk(&root, &mut rec);
    assert_eq!(rec.order, vec![5, 2, 1, 3]);
}

#[test]
fn dfs_three_level_balanced() {
    let source = b"abcd";
    //       5
    //      / \
    //     2   3
    //    / \
    //   1   4
    let left = interior(2, vec![leaf(1, 0, 1), leaf(4, 1, 2)]);
    let right = leaf(3, 2, 4);
    let root = interior(5, vec![left, right]);
    let mut rec = OrderRecorder::new();
    TreeWalker::new(source).walk(&root, &mut rec);
    assert_eq!(rec.order, vec![5, 2, 1, 4, 3]);
}

// ===================================================================
// 4–6. BFS ordering
// ===================================================================

#[test]
fn bfs_visits_level_by_level() {
    let source = b"abc";
    let left = interior(2, vec![leaf(1, 0, 1)]);
    let right = leaf(3, 1, 3);
    let root = interior(5, vec![left, right]);
    let mut rec = OrderRecorder::new();
    BreadthFirstWalker::new(source).walk(&root, &mut rec);
    assert_eq!(rec.order, vec![5, 2, 3, 1]);
}

#[test]
fn bfs_wide_tree_order() {
    let source = b"abcde";
    let children: Vec<_> = (0u16..5)
        .map(|i| leaf(i + 1, i as usize, i as usize + 1))
        .collect();
    let root = interior(10, children);
    let mut rec = OrderRecorder::new();
    BreadthFirstWalker::new(source).walk(&root, &mut rec);
    assert_eq!(rec.order, vec![10, 1, 2, 3, 4, 5]);
}

#[test]
fn bfs_deep_chain() {
    let source = b"x";
    let n = interior(2, vec![interior(3, vec![leaf(1, 0, 1)])]);
    let root = interior(5, vec![n]);
    let mut rec = OrderRecorder::new();
    BreadthFirstWalker::new(source).walk(&root, &mut rec);
    // level 0: 5, level 1: 2, level 2: 3, level 3: 1
    assert_eq!(rec.order, vec![5, 2, 3, 1]);
}

// ===================================================================
// 7–10. DFS visitor actions
// ===================================================================

#[test]
fn dfs_skip_children_prevents_descent() {
    let source = b"abc";
    let subtree = interior(2, vec![leaf(1, 0, 1)]);
    let sibling = leaf(3, 1, 3);
    let root = interior(5, vec![subtree, sibling]);
    let mut sv = SkipOnSymbol {
        skip: 2,
        visited: Vec::new(),
    };
    TreeWalker::new(source).walk(&root, &mut sv);
    assert_eq!(sv.visited, vec![5, 2, 3]);
}

#[test]
fn dfs_skip_at_root_visits_only_root() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let mut sv = SkipOnSymbol {
        skip: 5,
        visited: Vec::new(),
    };
    TreeWalker::new(source).walk(&root, &mut sv);
    assert_eq!(sv.visited, vec![5]);
}

#[test]
fn dfs_stop_halts_in_subtree() {
    let source = b"abc";
    let root = interior(5, vec![leaf(2, 0, 1), leaf(3, 1, 2), leaf(4, 2, 3)]);
    let mut sv = StopOnSymbol {
        target: 2,
        visited: Vec::new(),
    };
    TreeWalker::new(source).walk(&root, &mut sv);
    // DFS: stop returns from walk_node(2) but siblings still visited
    assert_eq!(sv.visited, vec![5, 2, 3, 4]);
}

#[test]
fn dfs_stop_at_root_visits_only_root() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1)]);
    let mut sv = StopOnSymbol {
        target: 5,
        visited: Vec::new(),
    };
    TreeWalker::new(source).walk(&root, &mut sv);
    assert_eq!(sv.visited, vec![5]);
}

// ===================================================================
// 11–14. BFS visitor actions
// ===================================================================

#[test]
fn bfs_skip_children_prevents_enqueue() {
    let source = b"abc";
    let subtree = interior(2, vec![leaf(1, 0, 1)]);
    let sibling = leaf(3, 1, 3);
    let root = interior(5, vec![subtree, sibling]);
    let mut sv = SkipOnSymbol {
        skip: 2,
        visited: Vec::new(),
    };
    BreadthFirstWalker::new(source).walk(&root, &mut sv);
    assert_eq!(sv.visited, vec![5, 2, 3]);
}

#[test]
fn bfs_stop_halts_queue() {
    let source = b"abc";
    let root = interior(5, vec![leaf(2, 0, 1), leaf(3, 1, 2), leaf(4, 2, 3)]);
    let mut sv = StopOnSymbol {
        target: 2,
        visited: Vec::new(),
    };
    BreadthFirstWalker::new(source).walk(&root, &mut sv);
    // BFS pops 5 (continue), then pops 2 (stop) -> done
    assert_eq!(sv.visited, vec![5, 2]);
}

#[test]
fn bfs_stop_at_root() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1)]);
    let mut sv = StopOnSymbol {
        target: 5,
        visited: Vec::new(),
    };
    BreadthFirstWalker::new(source).walk(&root, &mut sv);
    assert_eq!(sv.visited, vec![5]);
}

#[test]
fn bfs_skip_at_root_visits_only_root() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let mut sv = SkipOnSymbol {
        skip: 5,
        visited: Vec::new(),
    };
    BreadthFirstWalker::new(source).walk(&root, &mut sv);
    assert_eq!(sv.visited, vec![5]);
}

// ===================================================================
// 15–18. Enter/leave pairing (DFS only)
// ===================================================================

#[test]
fn enter_leave_single_leaf() {
    let source = b"x";
    let root = leaf(1, 0, 1);
    let mut de = DepthEvents::new();
    TreeWalker::new(source).walk(&root, &mut de);
    assert_eq!(de.events, vec!["E1", "T:x", "L1"]);
}

#[test]
fn enter_leave_interior_with_two_leaves() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let mut de = DepthEvents::new();
    TreeWalker::new(source).walk(&root, &mut de);
    assert_eq!(
        de.events,
        vec!["E5", "E1", "T:a", "L1", "E2", "T:b", "L2", "L5"]
    );
}

#[test]
fn enter_leave_skip_still_calls_leave() {
    let source = b"ab";
    let root = interior(5, vec![interior(2, vec![leaf(1, 0, 1)])]);

    struct SkipAndTrack {
        events: Vec<String>,
    }
    impl Visitor for SkipAndTrack {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.events.push(format!("E{}", node.symbol));
            if node.symbol == 2 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
        fn leave_node(&mut self, node: &ParsedNode) {
            self.events.push(format!("L{}", node.symbol));
        }
    }

    let mut v = SkipAndTrack { events: Vec::new() };
    TreeWalker::new(source).walk(&root, &mut v);
    // Skip calls leave_node for the skipped node
    assert_eq!(v.events, vec!["E5", "E2", "L2", "L5"]);
}

#[test]
fn enter_leave_error_not_entered() {
    let source = b"err";
    let root = interior(5, vec![error_node(0, 3)]);
    let mut de = DepthEvents::new();
    TreeWalker::new(source).walk(&root, &mut de);
    assert_eq!(de.events, vec!["E5", "ERR", "L5"]);
}

// ===================================================================
// 19–22. StatsVisitor basics
// ===================================================================

#[test]
fn stats_default_zeroed() {
    let s = StatsVisitor::default();
    assert_eq!(s.total_nodes, 0);
    assert_eq!(s.leaf_nodes, 0);
    assert_eq!(s.error_nodes, 0);
    assert_eq!(s.max_depth, 0);
    assert!(s.node_counts.is_empty());
}

#[test]
fn stats_single_leaf() {
    let source = b"x";
    let root = leaf(1, 0, 1);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
    assert_eq!(stats.max_depth, 1);
}

#[test]
fn stats_interior_with_children() {
    let source = b"abc";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 4);
    assert_eq!(stats.leaf_nodes, 3);
    assert_eq!(stats.max_depth, 2);
}

#[test]
fn stats_error_counted_separately() {
    let source = b"aerr";
    let root = interior(5, vec![leaf(1, 0, 1), error_node(1, 4)]);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
    // Error nodes are not entered, root + leaf = 2
    assert_eq!(stats.total_nodes, 2);
}

// ===================================================================
// 23–26. StatsVisitor — depth tracking
// ===================================================================

#[test]
fn stats_depth_chain_five() {
    let source = b"x";
    let mut node = leaf(1, 0, 1);
    for _ in 0..4 {
        node = interior(2, vec![node]);
    }
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&node, &mut stats);
    assert_eq!(stats.max_depth, 5);
}

#[test]
fn stats_depth_wide_is_two() {
    let source = b"abcdefghij";
    let children: Vec<_> = (0..10).map(|i| leaf(1, i, i + 1)).collect();
    let root = interior(5, children);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&root, &mut stats);
    assert_eq!(stats.max_depth, 2);
}

#[test]
fn stats_depth_asymmetric() {
    let source = b"abc";
    //    5
    //   / \
    //  2   1
    //  |
    //  3
    //  |
    //  1
    let deep = interior(2, vec![interior(3, vec![leaf(1, 0, 1)])]);
    let shallow = leaf(1, 1, 3);
    let root = interior(5, vec![deep, shallow]);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&root, &mut stats);
    assert_eq!(stats.max_depth, 4);
}

#[test]
fn stats_depth_fifty() {
    let source = b"x";
    let mut node = leaf(1, 0, 1);
    for _ in 0..49 {
        node = interior(2, vec![node]);
    }
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&node, &mut stats);
    assert_eq!(stats.max_depth, 50);
    assert_eq!(stats.total_nodes, 50);
    assert_eq!(stats.leaf_nodes, 1);
}

// ===================================================================
// 27–29. StatsVisitor — accumulation and reuse
// ===================================================================

#[test]
fn stats_accumulates_over_two_walks() {
    let source = b"ab";
    let t1 = interior(5, vec![leaf(1, 0, 1)]);
    let t2 = interior(5, vec![leaf(2, 0, 1), leaf(3, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&t1, &mut stats);
    walker.walk(&t2, &mut stats);
    assert_eq!(stats.total_nodes, 5);
    assert_eq!(stats.leaf_nodes, 3);
}

#[test]
fn stats_node_counts_per_kind() {
    let source = b"abc";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2), leaf(2, 2, 3)]);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&root, &mut stats);
    assert_eq!(*stats.node_counts.get("*").unwrap(), 2); // symbol 1 → "*"
    assert_eq!(*stats.node_counts.get("_2").unwrap(), 1); // symbol 2 → "_2"
    assert_eq!(*stats.node_counts.get("Expression").unwrap(), 1); // symbol 5
}

#[test]
fn stats_debug_format() {
    let stats = StatsVisitor::default();
    let dbg = format!("{stats:?}");
    assert!(dbg.contains("StatsVisitor"));
}

// ===================================================================
// 30–32. DFS/BFS node count parity
// ===================================================================

#[test]
fn dfs_bfs_same_total_nodes() {
    let source = b"abcde";
    let root = interior(
        5,
        vec![
            interior(2, vec![leaf(1, 0, 1), leaf(3, 1, 2)]),
            interior(4, vec![leaf(1, 2, 3)]),
            leaf(2, 3, 5),
        ],
    );
    let mut dfs = StatsVisitor::default();
    TreeWalker::new(source).walk(&root, &mut dfs);
    let mut bfs = StatsVisitor::default();
    BreadthFirstWalker::new(source).walk(&root, &mut bfs);
    assert_eq!(dfs.total_nodes, bfs.total_nodes);
    assert_eq!(dfs.leaf_nodes, bfs.leaf_nodes);
}

#[test]
fn dfs_bfs_same_error_count() {
    let source = b"ab_err";
    let root = interior(5, vec![leaf(1, 0, 1), error_node(1, 4), leaf(2, 4, 6)]);
    let mut dfs = StatsVisitor::default();
    TreeWalker::new(source).walk(&root, &mut dfs);
    let mut bfs = StatsVisitor::default();
    BreadthFirstWalker::new(source).walk(&root, &mut bfs);
    assert_eq!(dfs.error_nodes, bfs.error_nodes);
}

#[test]
fn dfs_bfs_search_same_match_count() {
    let source = b"abcabc";
    let root = interior(
        5,
        vec![
            leaf(1, 0, 1),
            interior(2, vec![leaf(1, 1, 2)]),
            leaf(1, 2, 3),
        ],
    );
    let mut dfs_s = SearchVisitor::new(|n: &ParsedNode| n.symbol == 1);
    TreeWalker::new(source).walk(&root, &mut dfs_s);
    let mut bfs_s = SearchVisitor::new(|n: &ParsedNode| n.symbol == 1);
    BreadthFirstWalker::new(source).walk(&root, &mut bfs_s);
    assert_eq!(dfs_s.matches.len(), bfs_s.matches.len());
}

// ===================================================================
// 33–36. SearchVisitor
// ===================================================================

#[test]
fn search_no_match() {
    let source = b"x";
    let root = leaf(1, 0, 1);
    let mut s = SearchVisitor::new(|n: &ParsedNode| n.symbol == 99);
    TreeWalker::new(source).walk(&root, &mut s);
    assert!(s.matches.is_empty());
}

#[test]
fn search_match_records_offsets() {
    let source = b"abcd";
    let root = interior(5, vec![leaf(1, 0, 2), leaf(2, 2, 4)]);
    let mut s = SearchVisitor::new(|n: &ParsedNode| n.symbol == 2);
    TreeWalker::new(source).walk(&root, &mut s);
    assert_eq!(s.matches.len(), 1);
    assert_eq!(s.matches[0].0, 2);
    assert_eq!(s.matches[0].1, 4);
}

#[test]
fn search_multiple_matches() {
    let source = b"abc";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2), leaf(1, 2, 3)]);
    let mut s = SearchVisitor::new(|n: &ParsedNode| n.symbol == 1);
    TreeWalker::new(source).walk(&root, &mut s);
    assert_eq!(s.matches.len(), 3);
}

#[test]
fn search_always_true_matches_all_entered() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let mut s = SearchVisitor::new(|_: &ParsedNode| true);
    TreeWalker::new(source).walk(&root, &mut s);
    assert_eq!(s.matches.len(), 3);
}

// ===================================================================
// 37–39. PrettyPrintVisitor
// ===================================================================

#[test]
fn pretty_empty_on_new() {
    let pp = PrettyPrintVisitor::new();
    assert!(pp.output().is_empty());
}

#[test]
fn pretty_single_leaf_output() {
    let source = b"x";
    let root = leaf(1, 0, 1);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(source).walk(&root, &mut pp);
    assert!(pp.output().contains("[named]"));
    assert!(pp.output().contains("\"x\""));
}

#[test]
fn pretty_indentation_increases_with_depth() {
    let source = b"x";
    let deep = interior(2, vec![interior(3, vec![leaf(1, 0, 1)])]);
    let root = interior(5, vec![deep]);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(source).walk(&root, &mut pp);
    // Deepest leaf text at depth 3 → 6 spaces
    assert!(pp.output().contains("      \"x\""));
}

// ===================================================================
// 40–43. Error node handling
// ===================================================================

#[test]
fn dfs_error_root_counted() {
    let source = b"err";
    let root = error_node(0, 3);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
    assert_eq!(stats.total_nodes, 0);
}

#[test]
fn bfs_error_root_counted() {
    let source = b"err";
    let root = error_node(0, 3);
    let mut stats = StatsVisitor::default();
    BreadthFirstWalker::new(source).walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
    assert_eq!(stats.total_nodes, 0);
}

#[test]
fn dfs_all_error_children() {
    let source = b"e1e2";
    let root = interior(5, vec![error_node(0, 2), error_node(2, 4)]);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 2);
    assert_eq!(stats.total_nodes, 1);
}

#[test]
fn pretty_error_shows_error_label() {
    let source = b"err";
    let root = interior(5, vec![error_node(0, 3)]);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(source).walk(&root, &mut pp);
    assert!(pp.output().contains("ERROR"));
}

// ===================================================================
// 44–47. TransformVisitor
// ===================================================================

struct LeafCounter;
impl TransformVisitor for LeafCounter {
    type Output = usize;
    fn transform_node(&mut self, _node: &ParsedNode, children: Vec<usize>) -> usize {
        children.iter().sum()
    }
    fn transform_leaf(&mut self, _node: &ParsedNode, _text: &str) -> usize {
        1
    }
    fn transform_error(&mut self, _node: &ParsedNode) -> usize {
        0
    }
}

#[test]
fn transform_counts_leaves() {
    let source = b"abc";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let mut lc = LeafCounter;
    let count = TransformWalker::new(source).walk(&root, &mut lc);
    assert_eq!(count, 3);
}

struct DepthCalc;
impl TransformVisitor for DepthCalc {
    type Output = usize;
    fn transform_node(&mut self, _node: &ParsedNode, children: Vec<usize>) -> usize {
        children.iter().max().copied().unwrap_or(0) + 1
    }
    fn transform_leaf(&mut self, _node: &ParsedNode, _text: &str) -> usize {
        1
    }
    fn transform_error(&mut self, _node: &ParsedNode) -> usize {
        0
    }
}

#[test]
fn transform_depth_calculation() {
    let source = b"ab";
    let inner = interior(2, vec![leaf(1, 0, 1)]);
    let root = interior(5, vec![inner, leaf(3, 1, 2)]);
    let mut dc = DepthCalc;
    let depth = TransformWalker::new(source).walk(&root, &mut dc);
    assert_eq!(depth, 3);
}

struct TextConcat;
impl TransformVisitor for TextConcat {
    type Output = String;
    fn transform_node(&mut self, _node: &ParsedNode, children: Vec<String>) -> String {
        children.join("")
    }
    fn transform_leaf(&mut self, _node: &ParsedNode, text: &str) -> String {
        text.to_string()
    }
    fn transform_error(&mut self, _node: &ParsedNode) -> String {
        "<ERR>".to_string()
    }
}

#[test]
fn transform_text_concat() {
    let source = b"hello";
    let root = interior(5, vec![leaf(1, 0, 3), leaf(2, 3, 5)]);
    let mut tc = TextConcat;
    let result = TransformWalker::new(source).walk(&root, &mut tc);
    assert_eq!(result, "hello");
}

#[test]
fn transform_error_node_handled() {
    let source = b"ok err";
    let root = interior(5, vec![leaf(1, 0, 2), error_node(3, 6)]);
    let mut tc = TextConcat;
    let result = TransformWalker::new(source).walk(&root, &mut tc);
    assert_eq!(result, "ok<ERR>");
}

// ===================================================================
// 48–50. Custom visitors — leaf text collector, error counter
// ===================================================================

struct LeafCollector {
    texts: Vec<String>,
}

impl Visitor for LeafCollector {
    fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
        VisitorAction::Continue
    }
    fn visit_leaf(&mut self, _node: &ParsedNode, text: &str) {
        self.texts.push(text.to_string());
    }
}

#[test]
fn custom_leaf_collector() {
    let source = b"hello world";
    let root = interior(5, vec![leaf(1, 0, 5), leaf(2, 6, 11)]);
    let mut lc = LeafCollector { texts: Vec::new() };
    TreeWalker::new(source).walk(&root, &mut lc);
    assert_eq!(lc.texts, vec!["hello", "world"]);
}

struct ErrorCounter {
    count: usize,
}

impl Visitor for ErrorCounter {
    fn visit_error(&mut self, _node: &ParsedNode) {
        self.count += 1;
    }
}

#[test]
fn custom_error_counter() {
    let source = b"e1e2e3";
    let root = interior(
        5,
        vec![error_node(0, 2), error_node(2, 4), error_node(4, 6)],
    );
    let mut ec = ErrorCounter { count: 0 };
    TreeWalker::new(source).walk(&root, &mut ec);
    assert_eq!(ec.count, 3);
}

#[test]
fn custom_depth_tracker_returns_to_zero() {
    struct DepthTracker {
        current: usize,
        max: usize,
    }
    impl Visitor for DepthTracker {
        fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
            self.current += 1;
            if self.current > self.max {
                self.max = self.current;
            }
            VisitorAction::Continue
        }
        fn leave_node(&mut self, _node: &ParsedNode) {
            self.current -= 1;
        }
    }

    let source = b"x";
    let chain = interior(5, vec![interior(2, vec![interior(3, vec![leaf(1, 0, 1)])])]);
    let mut dt = DepthTracker { current: 0, max: 0 };
    TreeWalker::new(source).walk(&chain, &mut dt);
    assert_eq!(dt.max, 4);
    assert_eq!(dt.current, 0);
}

// ===================================================================
// 51–53. Edge cases
// ===================================================================

#[test]
fn edge_empty_source_leaf() {
    let source = b"";
    let root = leaf(1, 0, 0);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn edge_interior_no_children_treated_as_leaf() {
    let source = b"";
    let root = make_node(5, vec![], 0, 0, false, true);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn edge_anonymous_leaf_not_named() {
    let source = b"+";
    let root = anon_leaf(4, 0, 1);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(source).walk(&root, &mut pp);
    assert!(!pp.output().contains("[named]"));
}

// ===================================================================
// 54–55. VisitorAction derives
// ===================================================================

#[test]
fn visitor_action_equality() {
    assert_eq!(VisitorAction::Continue, VisitorAction::Continue);
    assert_eq!(VisitorAction::SkipChildren, VisitorAction::SkipChildren);
    assert_eq!(VisitorAction::Stop, VisitorAction::Stop);
    assert_ne!(VisitorAction::Continue, VisitorAction::Stop);
    assert_ne!(VisitorAction::Continue, VisitorAction::SkipChildren);
    assert_ne!(VisitorAction::SkipChildren, VisitorAction::Stop);
}

#[test]
fn visitor_action_debug_and_copy() {
    let a = VisitorAction::SkipChildren;
    let b = a; // Copy
    assert_eq!(a, b);
    assert_eq!(format!("{a:?}"), "SkipChildren");
}

// ===================================================================
// 56–58. Multiple walkers, composition
// ===================================================================

#[test]
fn multiple_walkers_same_source_same_result() {
    let source = b"hello";
    let tree = interior(5, vec![leaf(1, 0, 3), leaf(2, 3, 5)]);
    let mut s1 = StatsVisitor::default();
    let mut s2 = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut s1);
    TreeWalker::new(source).walk(&tree, &mut s2);
    assert_eq!(s1.total_nodes, s2.total_nodes);
    assert_eq!(s1.leaf_nodes, s2.leaf_nodes);
}

#[test]
fn composition_stats_then_pretty() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert_eq!(stats.total_nodes, 3);
    assert!(!pp.output().is_empty());
}

#[test]
fn composition_dfs_stats_and_bfs_search() {
    let source = b"abc";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(1, 2, 3)]);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&root, &mut stats);
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol == 1);
    BreadthFirstWalker::new(source).walk(&root, &mut search);
    assert_eq!(stats.total_nodes, 4);
    assert_eq!(search.matches.len(), 2);
}

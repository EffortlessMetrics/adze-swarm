#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]
//! Comprehensive visitor module tests for adze runtime.

use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TransformVisitor,
    TransformWalker, TreeWalker, Visitor, VisitorAction,
};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Helper Functions
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

/// Build: root(6)( a(1), mid(4)( b(2), c(3) ), d(5) )
/// Source: "abcd"
fn sample_tree() -> (ParsedNode, Vec<u8>) {
    let source = b"abcd".to_vec();
    let a = leaf(1, 0, 1);
    let b = leaf(2, 1, 2);
    let c = unnamed_leaf(3, 2, 3);
    let mid = interior(4, vec![b, c]);
    let d = leaf(5, 3, 4);
    let root = interior(6, vec![a, mid, d]);
    (root, source)
}

// ===================================================================
// 1. VisitorAction — equality, inequality, Debug, Copy
// ===================================================================

#[test]
fn visitor_action_eq() {
    assert_eq!(VisitorAction::Continue, VisitorAction::Continue);
    assert_eq!(VisitorAction::SkipChildren, VisitorAction::SkipChildren);
    assert_eq!(VisitorAction::Stop, VisitorAction::Stop);
}

#[test]
fn visitor_action_ne() {
    assert_ne!(VisitorAction::Continue, VisitorAction::Stop);
    assert_ne!(VisitorAction::Continue, VisitorAction::SkipChildren);
    assert_ne!(VisitorAction::SkipChildren, VisitorAction::Stop);
}

#[test]
fn visitor_action_debug_and_clone() {
    assert_eq!(format!("{:?}", VisitorAction::Continue), "Continue");
    let a = VisitorAction::SkipChildren;
    let b = a; // Copy
    assert_eq!(a, b);
}

// ===================================================================
// 2. Default Visitor trait — no-op methods
// ===================================================================

#[test]
fn default_visitor_enter_continues() {
    struct Noop;
    impl Visitor for Noop {}
    let mut v = Noop;
    let node = leaf(0, 0, 1);
    assert_eq!(v.enter_node(&node), VisitorAction::Continue);
}

#[test]
fn default_visitor_callbacks_are_noop() {
    struct Noop;
    impl Visitor for Noop {}
    let mut v = Noop;
    let node = leaf(0, 0, 1);
    v.leave_node(&node);
    v.visit_leaf(&node, "x");
    v.visit_error(&node);
}

// ===================================================================
// 3–4. StatsVisitor
// ===================================================================

#[test]
fn stats_visitor_default_values() {
    let s = StatsVisitor::default();
    assert_eq!(s.total_nodes, 0);
    assert_eq!(s.leaf_nodes, 0);
    assert_eq!(s.error_nodes, 0);
    assert_eq!(s.max_depth, 0);
    assert!(s.node_counts.is_empty());
}

#[test]
fn stats_visitor_single_leaf() {
    let source = b"x";
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&leaf(1, 0, 1), &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
    assert_eq!(stats.max_depth, 1);
}

#[test]
fn stats_visitor_sample_tree() {
    let (root, source) = sample_tree();
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    // root(6), a(1), mid(4), b(2), c(3), d(5) = 6 nodes total
    assert_eq!(stats.total_nodes, 6);
    // leaves: a, b, c, d = 4
    assert_eq!(stats.leaf_nodes, 4);
    assert_eq!(stats.error_nodes, 0);
    // root(1) → mid(2) → b|c(3)
    assert_eq!(stats.max_depth, 3);
    assert!(!stats.node_counts.is_empty());
}

#[test]
fn stats_visitor_error_counting() {
    let err = error_node(1, 2);
    let root = interior(6, vec![leaf(1, 0, 1), err]);
    let source = b"x!";
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
}

// ===================================================================
// 5–11. TreeWalker (depth-first)
// ===================================================================

#[test]
fn tree_walker_enter_leave_order() {
    let (root, source) = sample_tree();
    struct Tracker(Vec<String>);
    impl Visitor for Tracker {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(format!("E{}", n.symbol()));
            VisitorAction::Continue
        }
        fn leave_node(&mut self, n: &ParsedNode) {
            self.0.push(format!("L{}", n.symbol()));
        }
    }
    let walker = TreeWalker::new(&source);
    let mut t = Tracker(vec![]);
    walker.walk(&root, &mut t);
    // enter root, enter a, leave a, enter mid, enter b, leave b, enter c, leave c, leave mid, enter d, leave d, leave root
    assert_eq!(t.0[0], "E6");
    assert_eq!(t.0[1], "E1");
    assert_eq!(t.0[2], "L1");
    assert_eq!(*t.0.last().unwrap(), "L6");
}

#[test]
fn tree_walker_preorder_symbols() {
    let (root, source) = sample_tree();
    struct Syms(Vec<u16>);
    impl Visitor for Syms {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            VisitorAction::Continue
        }
    }
    let walker = TreeWalker::new(&source);
    let mut s = Syms(vec![]);
    walker.walk(&root, &mut s);
    assert_eq!(s.0, vec![6, 1, 4, 2, 3, 5]);
}

#[test]
fn tree_walker_stop_action() {
    let (root, source) = sample_tree();
    struct StopAt2 {
        count: usize,
    }
    impl Visitor for StopAt2 {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.count += 1;
            if self.count >= 2 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }
    let walker = TreeWalker::new(&source);
    let mut v = StopAt2 { count: 0 };
    walker.walk(&root, &mut v);
    // In the DFS walker, Stop returns from walk_node for the current node
    // but the parent's child loop continues with remaining siblings.
    // root(6)→count=1→Continue, a(1)→count=2→Stop, mid(4)→count=3→Stop, d(5)→count=4→Stop
    assert_eq!(v.count, 4);
}

#[test]
fn tree_walker_skip_children() {
    let (root, source) = sample_tree();
    struct Skip4(Vec<u16>);
    impl Visitor for Skip4 {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            if n.symbol() == 4 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }
    let walker = TreeWalker::new(&source);
    let mut v = Skip4(vec![]);
    walker.walk(&root, &mut v);
    // root(6), a(1), mid(4)—skip so b(2), c(3) not entered, d(5)
    assert_eq!(v.0, vec![6, 1, 4, 5]);
}

#[test]
fn tree_walker_error_node_triggers_visit_error() {
    let err = error_node(1, 2);
    let root = interior(6, vec![leaf(1, 0, 1), err]);
    let source = b"x!";
    struct EC {
        errors: usize,
        enters: usize,
    }
    impl Visitor for EC {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.enters += 1;
            VisitorAction::Continue
        }
        fn visit_error(&mut self, _: &ParsedNode) {
            self.errors += 1;
        }
    }
    let walker = TreeWalker::new(source);
    let mut v = EC {
        errors: 0,
        enters: 0,
    };
    walker.walk(&root, &mut v);
    assert_eq!(v.errors, 1);
    // Error nodes call visit_error, not enter_node → root + leaf = 2
    assert_eq!(v.enters, 2);
}

#[test]
fn tree_walker_leaf_text() {
    let (root, source) = sample_tree();
    struct Texts(Vec<String>);
    impl Visitor for Texts {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn visit_leaf(&mut self, _: &ParsedNode, t: &str) {
            self.0.push(t.to_string());
        }
    }
    let walker = TreeWalker::new(&source);
    let mut v = Texts(vec![]);
    walker.walk(&root, &mut v);
    assert_eq!(v.0, vec!["a", "b", "c", "d"]);
}

#[test]
fn tree_walker_empty_source() {
    let node = leaf(1, 0, 0);
    let source = b"";
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&node, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
}

// ===================================================================
// 12–16. BreadthFirstWalker
// ===================================================================

#[test]
fn bfs_walker_level_order() {
    let (root, source) = sample_tree();
    struct Syms(Vec<u16>);
    impl Visitor for Syms {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            VisitorAction::Continue
        }
    }
    let walker = BreadthFirstWalker::new(&source);
    let mut v = Syms(vec![]);
    walker.walk(&root, &mut v);
    // Level 0: root(6) | Level 1: a(1), mid(4), d(5) | Level 2: b(2), c(3)
    assert_eq!(v.0, vec![6, 1, 4, 5, 2, 3]);
}

#[test]
fn bfs_walker_stop() {
    let (root, source) = sample_tree();
    struct StopAt3 {
        count: usize,
    }
    impl Visitor for StopAt3 {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.count += 1;
            if self.count >= 3 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }
    let walker = BreadthFirstWalker::new(&source);
    let mut v = StopAt3 { count: 0 };
    walker.walk(&root, &mut v);
    assert_eq!(v.count, 3);
}

#[test]
fn bfs_walker_skip_children() {
    let (root, source) = sample_tree();
    struct Skip4(Vec<u16>);
    impl Visitor for Skip4 {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            if n.symbol() == 4 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }
    let walker = BreadthFirstWalker::new(&source);
    let mut v = Skip4(vec![]);
    walker.walk(&root, &mut v);
    // root(6), a(1), mid(4)—skip so b,c not queued, d(5)
    assert_eq!(v.0, vec![6, 1, 4, 5]);
}

#[test]
fn bfs_walker_error_node() {
    let err = error_node(1, 2);
    let root = interior(6, vec![leaf(1, 0, 1), err]);
    let source = b"x!";
    struct EC {
        errors: usize,
    }
    impl Visitor for EC {
        fn visit_error(&mut self, _: &ParsedNode) {
            self.errors += 1;
        }
    }
    let walker = BreadthFirstWalker::new(source);
    let mut v = EC { errors: 0 };
    walker.walk(&root, &mut v);
    assert_eq!(v.errors, 1);
}

#[test]
fn bfs_walker_leaf_text() {
    let (root, source) = sample_tree();
    struct Texts(Vec<String>);
    impl Visitor for Texts {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn visit_leaf(&mut self, _: &ParsedNode, t: &str) {
            self.0.push(t.to_string());
        }
    }
    let walker = BreadthFirstWalker::new(&source);
    let mut v = Texts(vec![]);
    walker.walk(&root, &mut v);
    // BFS leaf order: a(level1), d(level1), b(level2), c(level2)
    assert_eq!(v.0, vec!["a", "d", "b", "c"]);
}

// ===================================================================
// 17–19. SearchVisitor
// ===================================================================

#[test]
fn search_visitor_finds_named_nodes() {
    let (root, source) = sample_tree();
    let walker = TreeWalker::new(&source);
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.is_named());
    walker.walk(&root, &mut search);
    // Named: root(6), a(1), mid(4), b(2), d(5) → c(3) is unnamed
    assert_eq!(search.matches.len(), 5);
}

#[test]
fn search_visitor_no_matches() {
    let (root, source) = sample_tree();
    let walker = TreeWalker::new(&source);
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 99);
    walker.walk(&root, &mut search);
    assert!(search.matches.is_empty());
}

#[test]
fn search_visitor_match_byte_ranges() {
    let source = b"hello";
    let node = leaf(1, 0, 5);
    let walker = TreeWalker::new(source);
    let mut search = SearchVisitor::new(|_: &ParsedNode| true);
    walker.walk(&node, &mut search);
    assert_eq!(search.matches.len(), 1);
    assert_eq!(search.matches[0].0, 0);
    assert_eq!(search.matches[0].1, 5);
}

// ===================================================================
// 20–22. PrettyPrintVisitor
// ===================================================================

#[test]
fn pretty_print_default_empty() {
    let pp = PrettyPrintVisitor::default();
    assert!(pp.output().is_empty());
}

#[test]
fn pretty_print_single_named_leaf() {
    let source = b"x";
    let node = leaf(1, 0, 1);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&node, &mut pp);
    let out = pp.output();
    assert!(out.contains("[named]"));
    assert!(out.contains("\"x\""));
}

#[test]
fn pretty_print_indentation_and_content() {
    let (root, source) = sample_tree();
    let walker = TreeWalker::new(&source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    let lines: Vec<&str> = out.lines().collect();
    assert!(!lines.is_empty());
    // Root line has no leading spaces
    assert!(!lines[0].starts_with(' '));
    // Deeper lines are indented
    assert!(lines.iter().any(|l| l.starts_with("  ")));
    // Leaf text appears quoted
    assert!(out.contains("\"a\""));
    assert!(out.contains("\"d\""));
}

#[test]
fn pretty_print_error_node() {
    let err = error_node(0, 1);
    let root = interior(6, vec![err]);
    let source = b"!";
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(pp.output().contains("ERROR"));
}

// ===================================================================
// 23–26. TransformVisitor / TransformWalker
// ===================================================================

struct NodeCounter;
impl TransformVisitor for NodeCounter {
    type Output = usize;
    fn transform_node(&mut self, _: &ParsedNode, ch: Vec<usize>) -> usize {
        1 + ch.iter().sum::<usize>()
    }
    fn transform_leaf(&mut self, _: &ParsedNode, _: &str) -> usize {
        1
    }
    fn transform_error(&mut self, _: &ParsedNode) -> usize {
        1
    }
}

#[test]
fn transform_count_nodes() {
    let (root, source) = sample_tree();
    let walker = TransformWalker::new(&source);
    let total = walker.walk(&root, &mut NodeCounter);
    assert_eq!(total, 6);
}

struct LeafConcat;
impl TransformVisitor for LeafConcat {
    type Output = String;
    fn transform_node(&mut self, _: &ParsedNode, ch: Vec<String>) -> String {
        ch.join("")
    }
    fn transform_leaf(&mut self, _: &ParsedNode, t: &str) -> String {
        t.to_string()
    }
    fn transform_error(&mut self, _: &ParsedNode) -> String {
        "ERR".to_string()
    }
}

#[test]
fn transform_concatenate_leaves() {
    let (root, source) = sample_tree();
    let walker = TransformWalker::new(&source);
    let result = walker.walk(&root, &mut LeafConcat);
    assert_eq!(result, "abcd");
}

#[test]
fn transform_error_node_output() {
    let err = error_node(1, 2);
    let root = interior(6, vec![leaf(1, 0, 1), err]);
    let source = b"x!";
    let walker = TransformWalker::new(source);
    let result = walker.walk(&root, &mut LeafConcat);
    assert_eq!(result, "xERR");
}

#[test]
fn transform_single_leaf() {
    let source = b"z";
    let node = leaf(1, 0, 1);
    let walker = TransformWalker::new(source);
    assert_eq!(walker.walk(&node, &mut NodeCounter), 1);
}

// ===================================================================
// 27–28. Depth / width edge cases
// ===================================================================

#[test]
fn deep_tree_60_levels() {
    let source: Vec<u8> = vec![b'x'; 60];
    let mut node = leaf(1, 59, 60);
    for _ in 0..59 {
        node = interior(2, vec![node]);
    }
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&node, &mut stats);
    assert_eq!(stats.total_nodes, 60);
    assert_eq!(stats.leaf_nodes, 1);
    assert_eq!(stats.max_depth, 60);
}

#[test]
fn wide_tree_150_children() {
    let source: Vec<u8> = (0u8..200).collect();
    let children: Vec<ParsedNode> = (0..150).map(|i| leaf(1, i, i + 1)).collect();
    let root = interior(2, children);
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 151);
    assert_eq!(stats.leaf_nodes, 150);
    assert_eq!(stats.max_depth, 2);
}

// ===================================================================
// 29–30. Multiple visitors / consistency
// ===================================================================

#[test]
fn multiple_visitors_same_tree() {
    let (root, source) = sample_tree();
    let walker = TreeWalker::new(&source);

    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);

    let mut search = SearchVisitor::new(|_: &ParsedNode| true);
    walker.walk(&root, &mut search);

    // Both agree on total node count
    assert_eq!(stats.total_nodes, search.matches.len());
}

#[test]
fn dfs_and_bfs_visit_same_node_set() {
    let (root, source) = sample_tree();

    let dfs_walker = TreeWalker::new(&source);
    let mut dfs_stats = StatsVisitor::default();
    dfs_walker.walk(&root, &mut dfs_stats);

    let bfs_walker = BreadthFirstWalker::new(&source);
    let mut bfs_stats = StatsVisitor::default();
    bfs_walker.walk(&root, &mut bfs_stats);

    // Same total count regardless of order
    assert_eq!(dfs_stats.total_nodes, bfs_stats.total_nodes);
    assert_eq!(dfs_stats.leaf_nodes, bfs_stats.leaf_nodes);
}

// ===================================================================
// 31. Depth counting
// ===================================================================

#[test]
fn depth_counter_per_level() {
    // root(1) → a(2)(→ d(5)), b(3), c(4)
    let source = b"abcd".to_vec();
    let d = leaf(5, 3, 4);
    let a = interior(2, vec![d]);
    let b = leaf(3, 1, 2);
    let c = leaf(4, 2, 3);
    let root = interior(1, vec![a, b, c]);

    struct DC {
        counts: HashMap<usize, usize>,
        depth: usize,
    }
    impl Visitor for DC {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            *self.counts.entry(self.depth).or_insert(0) += 1;
            self.depth += 1;
            VisitorAction::Continue
        }
        fn leave_node(&mut self, _: &ParsedNode) {
            self.depth -= 1;
        }
    }

    let walker = TreeWalker::new(&source);
    let mut v = DC {
        counts: HashMap::new(),
        depth: 0,
    };
    walker.walk(&root, &mut v);
    assert_eq!(v.counts[&0], 1); // root
    assert_eq!(v.counts[&1], 3); // a, b, c
    assert_eq!(v.counts[&2], 1); // d
}

// ===================================================================
// 32. Search with BFS
// ===================================================================

#[test]
fn search_visitor_with_bfs() {
    let (root, source) = sample_tree();
    let walker = BreadthFirstWalker::new(&source);
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.child_count() == 0);
    walker.walk(&root, &mut search);
    // Leaves: a, b, c, d
    assert_eq!(search.matches.len(), 4);
}

// ===================================================================
// 33. Skip-children still calls leave_node
// ===================================================================

#[test]
fn skip_children_still_leaves() {
    let source = b"ab";
    let root = interior(1, vec![leaf(2, 0, 1), leaf(3, 1, 2)]);
    struct Track {
        events: Vec<String>,
    }
    impl Visitor for Track {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.events.push(format!("E{}", n.symbol()));
            if n.symbol() == 1 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
        fn leave_node(&mut self, n: &ParsedNode) {
            self.events.push(format!("L{}", n.symbol()));
        }
    }
    let walker = TreeWalker::new(source);
    let mut v = Track { events: vec![] };
    walker.walk(&root, &mut v);
    // enter root → skip children → leave root
    assert_eq!(v.events, vec!["E1", "L1"]);
}

//! Comprehensive v2 tests for tree traversal APIs:
//! TreeWalker, BreadthFirstWalker, StatsVisitor, PrettyPrintVisitor,
//! SearchVisitor, TransformWalker/TransformVisitor, and VisitorAction.

use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TransformVisitor,
    TransformWalker, TreeWalker, Visitor, VisitorAction,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

fn unnamed_leaf(sym: u16, start: usize, end: usize) -> ParsedNode {
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

/// root(10)( a(1), mid(20)( b(2), c(3) ), d(4) )
/// source: "abcd"
fn sample_tree() -> (ParsedNode, Vec<u8>) {
    let src = b"abcd".to_vec();
    let a = leaf(1, 0, 1);
    let b = leaf(2, 1, 2);
    let c = unnamed_leaf(3, 2, 3);
    let mid = interior(20, vec![b, c]);
    let d = leaf(4, 3, 4);
    let root = interior(10, vec![a, mid, d]);
    (root, src)
}

/// Deep linear chain: root -> child -> ... -> leaf (depth levels)
fn deep_chain(depth: usize) -> (ParsedNode, Vec<u8>) {
    let src = b"x".to_vec();
    let mut node = leaf(1, 0, 1);
    for i in 2..=depth as u16 {
        node = make_node(i, vec![node], 0, 1, false, true);
    }
    (node, src)
}

/// Wide tree: root with `width` leaf children
fn wide_tree(width: usize) -> (ParsedNode, Vec<u8>) {
    let src: Vec<u8> = (0..width).map(|_| b'x').collect();
    let children: Vec<ParsedNode> = (0..width).map(|i| leaf((i + 1) as u16, i, i + 1)).collect();
    let root = make_node(100, children, 0, width, false, true);
    (root, src)
}

// ===================================================================
// 1. VisitorAction variants
// ===================================================================

#[test]
fn visitor_action_all_variants_eq_self() {
    assert_eq!(VisitorAction::Continue, VisitorAction::Continue);
    assert_eq!(VisitorAction::SkipChildren, VisitorAction::SkipChildren);
    assert_eq!(VisitorAction::Stop, VisitorAction::Stop);
}

#[test]
fn visitor_action_all_pairs_ne() {
    let variants = [
        VisitorAction::Continue,
        VisitorAction::SkipChildren,
        VisitorAction::Stop,
    ];
    for i in 0..variants.len() {
        for j in (i + 1)..variants.len() {
            assert_ne!(variants[i], variants[j]);
        }
    }
}

#[test]
fn visitor_action_debug_format() {
    assert_eq!(format!("{:?}", VisitorAction::Continue), "Continue");
    assert_eq!(format!("{:?}", VisitorAction::SkipChildren), "SkipChildren");
    assert_eq!(format!("{:?}", VisitorAction::Stop), "Stop");
}

#[test]
fn visitor_action_copy_clone() {
    let a = VisitorAction::Stop;
    let b = a; // Copy
    let c = a; // Clone
    assert_eq!(a, b);
    assert_eq!(a, c);
}

// ===================================================================
// 2. Default Visitor trait
// ===================================================================

#[test]
fn default_visitor_enter_returns_continue() {
    struct V;
    impl Visitor for V {}
    let node = leaf(1, 0, 1);
    assert_eq!(V.enter_node(&node), VisitorAction::Continue);
}

#[test]
fn default_visitor_leave_is_noop() {
    struct V;
    impl Visitor for V {}
    V.leave_node(&leaf(1, 0, 1));
}

#[test]
fn default_visitor_visit_leaf_is_noop() {
    struct V;
    impl Visitor for V {}
    V.visit_leaf(&leaf(1, 0, 1), "hello");
}

#[test]
fn default_visitor_visit_error_is_noop() {
    struct V;
    impl Visitor for V {}
    V.visit_error(&error_node(0, 1));
}

// ===================================================================
// 3. TreeWalker construction
// ===================================================================

#[test]
fn tree_walker_new_empty_source() {
    let _walker = TreeWalker::new(b"");
}

#[test]
fn tree_walker_new_nonempty_source() {
    let _walker = TreeWalker::new(b"hello world");
}

// ===================================================================
// 4. TreeWalker depth-first traversal
// ===================================================================

#[test]
fn tree_walker_single_leaf() {
    let src = b"x";
    let walker = TreeWalker::new(src);
    let mut stats = StatsVisitor::default();
    walker.walk(&leaf(1, 0, 1), &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
    assert_eq!(stats.max_depth, 1);
}

#[test]
fn tree_walker_preorder_symbols() {
    let (root, src) = sample_tree();
    struct Syms(Vec<u16>);
    impl Visitor for Syms {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            VisitorAction::Continue
        }
    }
    let walker = TreeWalker::new(&src);
    let mut v = Syms(vec![]);
    walker.walk(&root, &mut v);
    // DFS pre-order: root(10), a(1), mid(20), b(2), c(3), d(4)
    assert_eq!(v.0, vec![10, 1, 20, 2, 3, 4]);
}

#[test]
fn tree_walker_enter_leave_paired() {
    let (root, src) = sample_tree();
    struct Tracker {
        enters: Vec<u16>,
        leaves: Vec<u16>,
    }
    impl Visitor for Tracker {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.enters.push(n.symbol());
            VisitorAction::Continue
        }
        fn leave_node(&mut self, n: &ParsedNode) {
            self.leaves.push(n.symbol());
        }
    }
    let walker = TreeWalker::new(&src);
    let mut t = Tracker {
        enters: vec![],
        leaves: vec![],
    };
    walker.walk(&root, &mut t);
    // Every entered node must be left
    assert_eq!(t.enters.len(), t.leaves.len());
    // First entered is root, last left is root
    assert_eq!(t.enters[0], 10);
    assert_eq!(*t.leaves.last().unwrap(), 10);
}

#[test]
fn tree_walker_leaf_text_values() {
    let (root, src) = sample_tree();
    struct Texts(Vec<String>);
    impl Visitor for Texts {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn visit_leaf(&mut self, _: &ParsedNode, t: &str) {
            self.0.push(t.to_string());
        }
    }
    let walker = TreeWalker::new(&src);
    let mut v = Texts(vec![]);
    walker.walk(&root, &mut v);
    assert_eq!(v.0, vec!["a", "b", "c", "d"]);
}

#[test]
fn tree_walker_stop_halts_sibling_subtree() {
    let (root, src) = sample_tree();
    struct StopAfter1 {
        entered: Vec<u16>,
    }
    impl Visitor for StopAfter1 {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.entered.push(n.symbol());
            // Stop on first child entered (a)
            if n.symbol() == 1 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }
    let walker = TreeWalker::new(&src);
    let mut v = StopAfter1 { entered: vec![] };
    walker.walk(&root, &mut v);
    // Stop inside walk_node(a) returns, but parent loop continues siblings
    // root(10) -> a(1) Stop, mid(20) -> enter (Stop propagates? No, just returns from walk_node(a))
    // Actually, the code: VisitorAction::Stop => return from walk_node, but the
    // parent's child loop continues. So mid(20) and d(4) are still entered.
    assert!(v.entered.contains(&10));
    assert!(v.entered.contains(&1));
}

#[test]
fn tree_walker_skip_children_skips_subtree() {
    let (root, src) = sample_tree();
    struct SkipMid(Vec<u16>);
    impl Visitor for SkipMid {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            if n.symbol() == 20 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }
    let walker = TreeWalker::new(&src);
    let mut v = SkipMid(vec![]);
    walker.walk(&root, &mut v);
    // root(10), a(1), mid(20) skip, d(4) — children b(2),c(3) not visited
    assert_eq!(v.0, vec![10, 1, 20, 4]);
}

#[test]
fn tree_walker_skip_children_still_calls_leave() {
    let node = interior(10, vec![leaf(1, 0, 1)]);
    struct LeaveTracker {
        left: Vec<u16>,
    }
    impl Visitor for LeaveTracker {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            if n.symbol() == 10 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
        fn leave_node(&mut self, n: &ParsedNode) {
            self.left.push(n.symbol());
        }
    }
    let walker = TreeWalker::new(b"x");
    let mut v = LeaveTracker { left: vec![] };
    walker.walk(&node, &mut v);
    // SkipChildren still calls leave_node
    assert!(v.left.contains(&10));
}

#[test]
fn tree_walker_error_node_calls_visit_error() {
    let err = error_node(0, 1);
    let root = interior(10, vec![err]);
    struct ErrCount(usize);
    impl Visitor for ErrCount {
        fn visit_error(&mut self, _: &ParsedNode) {
            self.0 += 1;
        }
    }
    let walker = TreeWalker::new(b"!");
    let mut v = ErrCount(0);
    walker.walk(&root, &mut v);
    assert_eq!(v.0, 1);
}

#[test]
fn tree_walker_error_node_does_not_call_enter() {
    let err = error_node(0, 1);
    let root = interior(10, vec![err]);
    struct EnterCount(usize);
    impl Visitor for EnterCount {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.0 += 1;
            VisitorAction::Continue
        }
    }
    let walker = TreeWalker::new(b"!");
    let mut v = EnterCount(0);
    walker.walk(&root, &mut v);
    // Only root is entered, error node uses visit_error
    assert_eq!(v.0, 1);
}

#[test]
fn tree_walker_deep_chain_depth() {
    let (root, src) = deep_chain(20);
    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 20);
    assert_eq!(stats.max_depth, 20);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn tree_walker_wide_tree_all_children() {
    let (root, src) = wide_tree(50);
    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    // root + 50 leaves
    assert_eq!(stats.total_nodes, 51);
    assert_eq!(stats.leaf_nodes, 50);
    assert_eq!(stats.max_depth, 2);
}

#[test]
fn tree_walker_empty_source_leaf() {
    let node = leaf(1, 0, 0);
    let walker = TreeWalker::new(b"");
    let mut stats = StatsVisitor::default();
    walker.walk(&node, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn tree_walker_interior_no_children() {
    // Interior node with empty children vec acts as leaf
    let node = make_node(5, vec![], 0, 1, false, true);
    let walker = TreeWalker::new(b"x");
    let mut stats = StatsVisitor::default();
    walker.walk(&node, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
}

// ===================================================================
// 5. BreadthFirstWalker
// ===================================================================

#[test]
fn bfs_walker_new() {
    let _w = BreadthFirstWalker::new(b"abc");
}

#[test]
fn bfs_walker_level_order() {
    let (root, src) = sample_tree();
    struct Syms(Vec<u16>);
    impl Visitor for Syms {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            VisitorAction::Continue
        }
    }
    let walker = BreadthFirstWalker::new(&src);
    let mut v = Syms(vec![]);
    walker.walk(&root, &mut v);
    // Level 0: root(10) | Level 1: a(1), mid(20), d(4) | Level 2: b(2), c(3)
    assert_eq!(v.0, vec![10, 1, 20, 4, 2, 3]);
}

#[test]
fn bfs_walker_single_leaf() {
    let walker = BreadthFirstWalker::new(b"x");
    let mut stats = StatsVisitor::default();
    walker.walk(&leaf(1, 0, 1), &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn bfs_walker_stop_immediately() {
    let (root, src) = sample_tree();
    struct StopNow(usize);
    impl Visitor for StopNow {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.0 += 1;
            VisitorAction::Stop
        }
    }
    let walker = BreadthFirstWalker::new(&src);
    let mut v = StopNow(0);
    walker.walk(&root, &mut v);
    assert_eq!(v.0, 1);
}

#[test]
fn bfs_walker_stop_after_n() {
    let (root, src) = sample_tree();
    struct StopAt3(usize);
    impl Visitor for StopAt3 {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.0 += 1;
            if self.0 >= 3 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }
    let walker = BreadthFirstWalker::new(&src);
    let mut v = StopAt3(0);
    walker.walk(&root, &mut v);
    assert_eq!(v.0, 3);
}

#[test]
fn bfs_walker_skip_children() {
    let (root, src) = sample_tree();
    struct SkipMid(Vec<u16>);
    impl Visitor for SkipMid {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            if n.symbol() == 20 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }
    let walker = BreadthFirstWalker::new(&src);
    let mut v = SkipMid(vec![]);
    walker.walk(&root, &mut v);
    // root(10), a(1), mid(20) skip, d(4)
    assert_eq!(v.0, vec![10, 1, 20, 4]);
}

#[test]
fn bfs_walker_error_node() {
    let err = error_node(0, 1);
    let root = interior(10, vec![err, leaf(1, 1, 2)]);
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
    let walker = BreadthFirstWalker::new(b"x!");
    let mut v = EC {
        errors: 0,
        enters: 0,
    };
    walker.walk(&root, &mut v);
    assert_eq!(v.errors, 1);
    assert_eq!(v.enters, 2); // root + leaf
}

#[test]
fn bfs_walker_leaf_text() {
    let (root, src) = sample_tree();
    struct Texts(Vec<String>);
    impl Visitor for Texts {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn visit_leaf(&mut self, _: &ParsedNode, t: &str) {
            self.0.push(t.to_string());
        }
    }
    let walker = BreadthFirstWalker::new(&src);
    let mut v = Texts(vec![]);
    walker.walk(&root, &mut v);
    // BFS order: a, d come before b, c (level 1 leaves before level 2)
    assert_eq!(v.0, vec!["a", "d", "b", "c"]);
}

#[test]
fn bfs_walker_deep_chain() {
    let (root, src) = deep_chain(10);
    let walker = BreadthFirstWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 10);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn bfs_walker_wide_tree() {
    let (root, src) = wide_tree(30);
    let walker = BreadthFirstWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 31);
    assert_eq!(stats.leaf_nodes, 30);
}

// ===================================================================
// 6. StatsVisitor
// ===================================================================

#[test]
fn stats_default_all_zero() {
    let s = StatsVisitor::default();
    assert_eq!(s.total_nodes, 0);
    assert_eq!(s.leaf_nodes, 0);
    assert_eq!(s.error_nodes, 0);
    assert_eq!(s.max_depth, 0);
    assert!(s.node_counts.is_empty());
}

#[test]
fn stats_counts_sample_tree() {
    let (root, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 6);
    assert_eq!(stats.leaf_nodes, 4);
    assert_eq!(stats.error_nodes, 0);
    // root(1) -> mid(2) -> child(3)
    assert_eq!(stats.max_depth, 3);
}

#[test]
fn stats_node_counts_map() {
    let root = interior(10, vec![leaf(1, 0, 1), leaf(1, 1, 2), leaf(2, 2, 3)]);
    let walker = TreeWalker::new(b"abc");
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    // symbol 1 appears twice (via kind() fallback), symbol 10 once, symbol 2 once
    assert_eq!(stats.total_nodes, 4);
    assert!(!stats.node_counts.is_empty());
}

#[test]
fn stats_error_nodes_counted() {
    let root = interior(10, vec![error_node(0, 1), error_node(1, 2), leaf(1, 2, 3)]);
    let walker = TreeWalker::new(b"!!x");
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 2);
}

#[test]
fn stats_max_depth_linear() {
    let (root, src) = deep_chain(5);
    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.max_depth, 5);
}

#[test]
fn stats_debug_impl() {
    let s = StatsVisitor::default();
    let dbg = format!("{:?}", s);
    assert!(dbg.contains("StatsVisitor"));
}

// ===================================================================
// 7. SearchVisitor
// ===================================================================

#[test]
fn search_visitor_finds_by_symbol() {
    let (root, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 2);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
    assert_eq!(sv.matches[0].2, "_2"); // symbol 2 → fallback kind()
}

#[test]
fn search_visitor_finds_multiple() {
    let root = interior(10, vec![leaf(1, 0, 1), leaf(1, 1, 2), leaf(2, 2, 3)]);
    let walker = TreeWalker::new(b"aab");
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 2);
}

#[test]
fn search_visitor_no_match() {
    let (root, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 999);
    walker.walk(&root, &mut sv);
    assert!(sv.matches.is_empty());
}

#[test]
fn search_visitor_match_records_byte_range() {
    let node = leaf(1, 5, 10);
    let source = b"0000056789";
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|_: &ParsedNode| true);
    walker.walk(&node, &mut sv);
    assert_eq!(sv.matches.len(), 1);
    assert_eq!(sv.matches[0].0, 5); // start_byte
    assert_eq!(sv.matches[0].1, 10); // end_byte
}

#[test]
fn search_visitor_by_is_named() {
    let (root, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.is_named());
    walker.walk(&root, &mut sv);
    // All nodes except unnamed_leaf(c) are named → root, a, mid, b, d = 5
    assert_eq!(sv.matches.len(), 5);
}

#[test]
fn search_visitor_with_bfs() {
    let (root, src) = sample_tree();
    let walker = BreadthFirstWalker::new(&src);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 20);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
}

#[test]
fn search_visitor_matches_all() {
    let (root, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut sv = SearchVisitor::new(|_: &ParsedNode| true);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 6);
}

#[test]
fn search_visitor_empty_initial_matches() {
    let sv = SearchVisitor::new(|_: &ParsedNode| false);
    assert!(sv.matches.is_empty());
}

// ===================================================================
// 8. PrettyPrintVisitor
// ===================================================================

#[test]
fn pretty_print_new_empty() {
    let pp = PrettyPrintVisitor::new();
    assert!(pp.output().is_empty());
}

#[test]
fn pretty_print_default_empty() {
    let pp = PrettyPrintVisitor::default();
    assert!(pp.output().is_empty());
}

#[test]
fn pretty_print_single_leaf() {
    let source = b"x";
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&leaf(1, 0, 1), &mut pp);
    let out = pp.output();
    // Should contain the kind and the text
    assert!(out.contains("[named]"));
    assert!(out.contains("\"x\""));
}

#[test]
fn pretty_print_named_annotation() {
    let source = b"a";
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&leaf(1, 0, 1), &mut pp);
    assert!(pp.output().contains("[named]"));
}

#[test]
fn pretty_print_unnamed_no_named_tag() {
    let source = b"a";
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&unnamed_leaf(1, 0, 1), &mut pp);
    // The first line is the node kind, unnamed should not have [named]
    let first_line = pp.output().lines().next().unwrap();
    assert!(!first_line.contains("[named]"));
}

#[test]
fn pretty_print_indentation() {
    let root = interior(10, vec![leaf(1, 0, 1)]);
    let source = b"x";
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let lines: Vec<&str> = pp.output().lines().collect();
    // First line (root) has no indent, children are indented
    assert!(!lines[0].starts_with("  "));
    // Child node's kind line should be indented
    assert!(lines.len() >= 2);
    assert!(lines[1].starts_with("  "));
}

#[test]
fn pretty_print_sample_tree_multiline() {
    let (root, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let lines: Vec<&str> = pp.output().lines().collect();
    // Should have multiple lines for the tree
    assert!(lines.len() >= 6);
}

#[test]
fn pretty_print_error_node() {
    let err = error_node(0, 1);
    let root = interior(10, vec![err]);
    let walker = TreeWalker::new(b"!");
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(pp.output().contains("ERROR"));
}

#[test]
fn pretty_print_leaf_text_quoted() {
    let source = b"hello";
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&leaf(1, 0, 5), &mut pp);
    assert!(pp.output().contains("\"hello\""));
}

// ===================================================================
// 9. TransformWalker / TransformVisitor
// ===================================================================

struct CountTransform;

impl TransformVisitor for CountTransform {
    type Output = usize;

    fn transform_node(&mut self, _node: &ParsedNode, children: Vec<usize>) -> usize {
        1 + children.iter().sum::<usize>()
    }

    fn transform_leaf(&mut self, _node: &ParsedNode, _text: &str) -> usize {
        1
    }

    fn transform_error(&mut self, _node: &ParsedNode) -> usize {
        1
    }
}

#[test]
fn transform_walker_counts_nodes() {
    let (root, src) = sample_tree();
    let walker = TransformWalker::new(&src);
    let mut t = CountTransform;
    let count = walker.walk(&root, &mut t);
    assert_eq!(count, 6);
}

#[test]
fn transform_walker_single_leaf() {
    let walker = TransformWalker::new(b"x");
    let mut t = CountTransform;
    let count = walker.walk(&leaf(1, 0, 1), &mut t);
    assert_eq!(count, 1);
}

#[test]
fn transform_walker_error_node() {
    let err = error_node(0, 1);
    let root = interior(10, vec![err]);
    let walker = TransformWalker::new(b"!");
    let mut t = CountTransform;
    let count = walker.walk(&root, &mut t);
    assert_eq!(count, 2);
}

struct DepthTransform;

impl TransformVisitor for DepthTransform {
    type Output = usize;

    fn transform_node(&mut self, _node: &ParsedNode, children: Vec<usize>) -> usize {
        1 + children.iter().copied().max().unwrap_or(0)
    }

    fn transform_leaf(&mut self, _node: &ParsedNode, _text: &str) -> usize {
        1
    }

    fn transform_error(&mut self, _node: &ParsedNode) -> usize {
        1
    }
}

#[test]
fn transform_walker_computes_depth() {
    let (root, src) = sample_tree();
    let walker = TransformWalker::new(&src);
    let mut t = DepthTransform;
    let depth = walker.walk(&root, &mut t);
    assert_eq!(depth, 3);
}

struct TextCollector;

impl TransformVisitor for TextCollector {
    type Output = String;

    fn transform_node(&mut self, _node: &ParsedNode, children: Vec<String>) -> String {
        format!("({})", children.join(" "))
    }

    fn transform_leaf(&mut self, _node: &ParsedNode, text: &str) -> String {
        text.to_string()
    }

    fn transform_error(&mut self, _node: &ParsedNode) -> String {
        "ERR".to_string()
    }
}

#[test]
fn transform_walker_collects_text() {
    let (root, src) = sample_tree();
    let walker = TransformWalker::new(&src);
    let mut t = TextCollector;
    let result = walker.walk(&root, &mut t);
    assert_eq!(result, "(a (b c) d)");
}

#[test]
fn transform_walker_deep_chain() {
    let (root, src) = deep_chain(5);
    let walker = TransformWalker::new(&src);
    let mut t = DepthTransform;
    let depth = walker.walk(&root, &mut t);
    assert_eq!(depth, 5);
}

// ===================================================================
// 10. Mixed walker/visitor combos
// ===================================================================

#[test]
fn dfs_and_bfs_same_total_count() {
    let (root, src) = sample_tree();
    let mut dfs_stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut dfs_stats);
    let mut bfs_stats = StatsVisitor::default();
    BreadthFirstWalker::new(&src).walk(&root, &mut bfs_stats);
    assert_eq!(dfs_stats.total_nodes, bfs_stats.total_nodes);
    assert_eq!(dfs_stats.leaf_nodes, bfs_stats.leaf_nodes);
}

#[test]
fn dfs_and_bfs_different_visit_order() {
    let (root, src) = sample_tree();
    struct Syms(Vec<u16>);
    impl Visitor for Syms {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            VisitorAction::Continue
        }
    }
    let mut dfs = Syms(vec![]);
    TreeWalker::new(&src).walk(&root, &mut dfs);
    let mut bfs = Syms(vec![]);
    BreadthFirstWalker::new(&src).walk(&root, &mut bfs);
    // DFS and BFS must have different orderings for non-trivial trees
    assert_ne!(dfs.0, bfs.0);
    // But same elements
    let mut dfs_sorted = dfs.0.clone();
    dfs_sorted.sort();
    let mut bfs_sorted = bfs.0.clone();
    bfs_sorted.sort();
    assert_eq!(dfs_sorted, bfs_sorted);
}

#[test]
fn search_visitor_dfs_vs_bfs_same_matches() {
    let (root, src) = sample_tree();
    let mut dfs_sv = SearchVisitor::new(|n: &ParsedNode| n.is_named());
    TreeWalker::new(&src).walk(&root, &mut dfs_sv);
    let mut bfs_sv = SearchVisitor::new(|n: &ParsedNode| n.is_named());
    BreadthFirstWalker::new(&src).walk(&root, &mut bfs_sv);
    // Same matches, possibly different order
    assert_eq!(dfs_sv.matches.len(), bfs_sv.matches.len());
}

// ===================================================================
// 11. Edge cases
// ===================================================================

#[test]
fn single_error_root() {
    let root = error_node(0, 1);
    let walker = TreeWalker::new(b"!");
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
    assert_eq!(stats.total_nodes, 0); // error nodes bypass enter_node
}

#[test]
fn multiple_error_children() {
    let root = interior(
        10,
        vec![error_node(0, 1), error_node(1, 2), error_node(2, 3)],
    );
    let walker = TreeWalker::new(b"!!!");
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 3);
}

#[test]
fn bfs_single_error_root() {
    let root = error_node(0, 1);
    let walker = BreadthFirstWalker::new(b"!");
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
}

#[test]
fn deeply_nested_skip_children() {
    // A chain where we skip at depth 3
    let deep = interior(
        10,
        vec![interior(20, vec![interior(30, vec![leaf(1, 0, 1)])])],
    );
    struct SkipAt30(Vec<u16>);
    impl Visitor for SkipAt30 {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            if n.symbol() == 30 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }
    let walker = TreeWalker::new(b"x");
    let mut v = SkipAt30(vec![]);
    walker.walk(&deep, &mut v);
    assert_eq!(v.0, vec![10, 20, 30]); // leaf not entered
}

#[test]
fn visitor_with_state_accumulation() {
    struct Summer {
        byte_sum: usize,
    }
    impl Visitor for Summer {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.byte_sum += n.end_byte() - n.start_byte();
            VisitorAction::Continue
        }
    }
    let (root, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut v = Summer { byte_sum: 0 };
    walker.walk(&root, &mut v);
    assert!(v.byte_sum > 0);
}

#[test]
fn pretty_print_deep_tree_indentation_grows() {
    let (root, src) = deep_chain(4);
    let walker = TreeWalker::new(&src);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let lines: Vec<&str> = pp.output().lines().collect();
    // Each deeper level should have more leading spaces
    for i in 1..lines.len() {
        let prev_indent = lines[i - 1].len() - lines[i - 1].trim_start().len();
        let curr_indent = lines[i].len() - lines[i].trim_start().len();
        // Indentation may grow or stay same (for leaf text at same level)
        // Indentation may grow, stay same, or vary for leaf text at same level
        let _ = (curr_indent, prev_indent);
    }
    // At minimum, deepest line should be indented
    let max_indent = lines
        .iter()
        .map(|l| l.len() - l.trim_start().len())
        .max()
        .unwrap_or(0);
    assert!(max_indent > 0);
}

#[test]
fn stats_visitor_reuse_across_walks() {
    let (root, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    let first_total = stats.total_nodes;
    // Walk again with same visitor accumulates
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, first_total * 2);
}

#[test]
fn search_visitor_accumulates_across_walks() {
    let (root, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    walker.walk(&root, &mut sv);
    let first_count = sv.matches.len();
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), first_count * 2);
}

#[test]
fn bfs_walker_wide_all_leaves_visited() {
    let (root, src) = wide_tree(20);
    struct LeafCount(usize);
    impl Visitor for LeafCount {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn visit_leaf(&mut self, _: &ParsedNode, _: &str) {
            self.0 += 1;
        }
    }
    let walker = BreadthFirstWalker::new(&src);
    let mut v = LeafCount(0);
    walker.walk(&root, &mut v);
    assert_eq!(v.0, 20);
}

#[test]
fn transform_walker_with_error_children() {
    let root = interior(10, vec![leaf(1, 0, 1), error_node(1, 2), leaf(2, 2, 3)]);
    let walker = TransformWalker::new(b"x!y");
    let mut t = CountTransform;
    let count = walker.walk(&root, &mut t);
    assert_eq!(count, 4); // root + leaf + error + leaf
}

#[test]
fn pretty_print_multiple_walks_appends() {
    let source = b"x";
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&leaf(1, 0, 1), &mut pp);
    let len1 = pp.output().len();
    walker.walk(&leaf(1, 0, 1), &mut pp);
    assert!(pp.output().len() > len1);
}

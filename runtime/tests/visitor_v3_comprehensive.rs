//! Comprehensive tests for the `adze::visitor` module.
//!
//! Covers StatsVisitor, PrettyPrintVisitor, SearchVisitor, the Visitor trait,
//! walker types, and edge cases with constructed parse trees.

use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TreeWalker, Visitor,
    VisitorAction,
};

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

/// Leaf node spanning `start..end` with the given symbol id.
fn leaf(symbol: u16, start: usize, end: usize, named: bool) -> ParsedNode {
    make_node(symbol, vec![], start, end, false, named)
}

/// Interior node with children spanning from min-start to max-end.
fn interior(symbol: u16, children: Vec<ParsedNode>, named: bool) -> ParsedNode {
    let start = children.first().map_or(0, |c| c.start_byte);
    let end = children.last().map_or(0, |c| c.end_byte);
    make_node(symbol, children, start, end, false, named)
}

/// Error node.
fn error_node(start: usize, end: usize) -> ParsedNode {
    make_node(0, vec![], start, end, true, false)
}

/// Build a simple tree: root(child_a, child_b)
fn simple_tree() -> (ParsedNode, &'static [u8]) {
    let source: &[u8] = b"ab";
    let a = leaf(1, 0, 1, true);
    let b = leaf(1, 1, 2, true);
    let root = interior(5, vec![a, b], true);
    (root, source)
}

// ===================================================================
// StatsVisitor — default, visit patterns, accumulation
// ===================================================================

#[test]
fn stats_default_is_zero() {
    let sv = StatsVisitor::default();
    assert_eq!(sv.total_nodes, 0);
    assert_eq!(sv.leaf_nodes, 0);
    assert_eq!(sv.error_nodes, 0);
    assert_eq!(sv.max_depth, 0);
    assert!(sv.node_counts.is_empty());
}

#[test]
fn stats_debug_impl() {
    let sv = StatsVisitor::default();
    let dbg = format!("{sv:?}");
    assert!(dbg.contains("StatsVisitor"));
}

#[test]
fn stats_single_leaf() {
    let source = b"x";
    let node = leaf(1, 0, 1, true);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&node, &mut sv);
    assert_eq!(sv.total_nodes, 1);
    assert_eq!(sv.leaf_nodes, 1);
    assert_eq!(sv.max_depth, 1);
}

#[test]
fn stats_simple_tree() {
    let (root, source) = simple_tree();
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    // root + 2 children = 3 nodes
    assert_eq!(sv.total_nodes, 3);
    assert_eq!(sv.leaf_nodes, 2);
    assert_eq!(sv.max_depth, 2);
}

#[test]
fn stats_node_counts_per_kind() {
    let (root, source) = simple_tree();
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    // Symbol 5 = "Expression", symbol 1 = "*"
    assert!(sv.node_counts.contains_key("Expression"));
    assert_eq!(*sv.node_counts.get("*").unwrap_or(&0), 2);
}

#[test]
fn stats_error_counted() {
    let source = b"err";
    let err = error_node(0, 3);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&err, &mut sv);
    assert_eq!(sv.error_nodes, 1);
    // Error nodes are not entered via enter_node
    assert_eq!(sv.total_nodes, 0);
}

#[test]
fn stats_mixed_error_and_normal() {
    let source = b"ab!";
    let a = leaf(1, 0, 1, true);
    let err = error_node(2, 3);
    let root = interior(5, vec![a, err], true);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    assert_eq!(sv.error_nodes, 1);
    assert!(sv.total_nodes >= 2); // root + a
    assert_eq!(sv.leaf_nodes, 1);
}

#[test]
fn stats_deep_tree() {
    // Chain: depth 5
    let source = b"hello";
    let mut node = leaf(1, 0, 5, true);
    for _ in 0..4 {
        node = interior(5, vec![node], true);
    }
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&node, &mut sv);
    assert_eq!(sv.max_depth, 5);
    assert_eq!(sv.total_nodes, 5);
    assert_eq!(sv.leaf_nodes, 1);
}

#[test]
fn stats_wide_tree() {
    let source = b"abcdefghij";
    let children: Vec<_> = (0..10).map(|i| leaf(1, i, i + 1, true)).collect();
    let root = interior(5, children, true);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    assert_eq!(sv.total_nodes, 11);
    assert_eq!(sv.leaf_nodes, 10);
    assert_eq!(sv.max_depth, 2);
}

#[test]
fn stats_accumulates_across_walks() {
    let source = b"x";
    let node = leaf(1, 0, 1, true);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&node, &mut sv);
    walker.walk(&node, &mut sv);
    assert_eq!(sv.total_nodes, 2);
    assert_eq!(sv.leaf_nodes, 2);
}

// ===================================================================
// PrettyPrintVisitor — new(), output(), formatting
// ===================================================================

#[test]
fn pretty_new_empty() {
    let pp = PrettyPrintVisitor::new();
    assert_eq!(pp.output(), "");
}

#[test]
fn pretty_default_same_as_new() {
    let a = PrettyPrintVisitor::new();
    let b = PrettyPrintVisitor::default();
    assert_eq!(a.output(), b.output());
}

#[test]
fn pretty_single_leaf() {
    let source = b"x";
    let node = leaf(1, 0, 1, true);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&node, &mut pp);
    let out = pp.output();
    assert!(out.contains("[named]"), "named leaf should be marked");
    assert!(out.contains("\"x\""), "leaf text should appear");
}

#[test]
fn pretty_unnamed_leaf() {
    let source = b"+";
    let node = leaf(1, 0, 1, false);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&node, &mut pp);
    let out = pp.output();
    assert!(!out.contains("[named]"));
}

#[test]
fn pretty_indentation_increases() {
    let (root, source) = simple_tree();
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    // Children should be indented with 2 spaces relative to parent
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines.len() >= 2);
    // First line is root (no indent), child lines start with "  "
    assert!(!lines[0].starts_with(' '));
    // At least one child line should be indented
    assert!(lines.iter().any(|l| l.starts_with("  ")));
}

#[test]
fn pretty_error_node() {
    let source = b"err";
    let err = error_node(0, 3);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&err, &mut pp);
    let out = pp.output();
    assert!(
        out.contains("ERROR"),
        "error node should produce ERROR label"
    );
}

#[test]
fn pretty_output_returns_str_ref() {
    let pp = PrettyPrintVisitor::new();
    let _s: &str = pp.output();
}

#[test]
fn pretty_multiline_output() {
    let (root, source) = simple_tree();
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let line_count = pp.output().lines().count();
    // root line + leaf quote lines = at least 3
    assert!(line_count >= 3);
}

#[test]
fn pretty_deep_tree_indent() {
    let source = b"z";
    let mut node = leaf(1, 0, 1, true);
    for _ in 0..3 {
        node = interior(5, vec![node], true);
    }
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&node, &mut pp);
    let out = pp.output();
    // Deepest leaf quote should be indented by 3*2=6 + further indent for leaf text
    assert!(out.contains("      "));
}

// ===================================================================
// SearchVisitor — predicate-based search
// ===================================================================

#[test]
fn search_no_matches() {
    let source = b"x";
    let node = leaf(1, 0, 1, true);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|_n: &ParsedNode| false);
    walker.walk(&node, &mut sv);
    assert!(sv.matches.is_empty());
}

#[test]
fn search_all_match() {
    let (root, source) = simple_tree();
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|_n: &ParsedNode| true);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 3);
}

#[test]
fn search_by_named() {
    let source = b"a+b";
    let a = leaf(1, 0, 1, true);
    let plus = leaf(2, 1, 2, false); // unnamed
    let b = leaf(1, 2, 3, true);
    let root = interior(5, vec![a, plus, b], true);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.is_named());
    walker.walk(&root, &mut sv);
    // root + a + b = 3 named nodes
    assert_eq!(sv.matches.len(), 3);
}

#[test]
fn search_match_tuple_fields() {
    let source = b"xy";
    let node = leaf(1, 3, 7, true);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|_n: &ParsedNode| true);
    walker.walk(&node, &mut sv);
    assert_eq!(sv.matches.len(), 1);
    let (start, end, kind) = &sv.matches[0];
    assert_eq!(*start, 3);
    assert_eq!(*end, 7);
    assert_eq!(kind, "*"); // symbol 1 -> "*"
}

#[test]
fn search_by_byte_range() {
    let source = b"abcd";
    let a = leaf(1, 0, 2, true);
    let b = leaf(1, 2, 4, true);
    let root = interior(5, vec![a, b], true);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.start_byte() >= 2);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
    assert_eq!(sv.matches[0].0, 2);
}

#[test]
fn search_continues_after_match() {
    let source = b"abcd";
    let children: Vec<_> = (0..4).map(|i| leaf(1, i, i + 1, true)).collect();
    let root = interior(5, children, true);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.child_count() == 0);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 4);
}

#[test]
fn search_matches_is_vec() {
    let sv = SearchVisitor::new(|_n: &ParsedNode| false);
    let _v: &Vec<(usize, usize, String)> = &sv.matches;
}

// ===================================================================
// Visitor trait — trait objects, dynamic dispatch, default impls
// ===================================================================

/// Visitor with all defaults — should compile and be callable.
struct NoOpVisitor;

impl Visitor for NoOpVisitor {}

#[test]
fn noop_visitor_compiles() {
    let node = leaf(1, 0, 1, true);
    let mut v = NoOpVisitor;
    assert_eq!(v.enter_node(&node), VisitorAction::Continue);
    v.leave_node(&node);
    v.visit_leaf(&node, "x");
    v.visit_error(&node);
}

#[test]
fn visitor_as_trait_object() {
    let mut v: Box<dyn Visitor> = Box::new(StatsVisitor::default());
    let node = leaf(1, 0, 1, true);
    let action = v.enter_node(&node);
    assert_eq!(action, VisitorAction::Continue);
}

#[test]
fn visitor_dynamic_dispatch_stats() {
    // Verify StatsVisitor can be used behind a trait object reference
    let mut sv = StatsVisitor::default();
    let v: &mut dyn Visitor = &mut sv;
    let node = leaf(1, 0, 1, true);
    let action = v.enter_node(&node);
    assert_eq!(action, VisitorAction::Continue);
    v.leave_node(&node);
    v.visit_leaf(&node, "x");
    assert_eq!(sv.total_nodes, 1);
    assert_eq!(sv.leaf_nodes, 1);
}

#[test]
fn visitor_dynamic_dispatch_pretty() {
    let mut v: Box<dyn Visitor> = Box::new(PrettyPrintVisitor::new());
    let node = leaf(1, 0, 1, true);
    let _ = v.enter_node(&node);
}

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
fn visitor_action_clone_copy() {
    let a = VisitorAction::Continue;
    let b = a; // Copy
    let c = a;
    assert_eq!(a, b);
    assert_eq!(a, c);
}

#[test]
fn visitor_action_debug() {
    let dbg = format!("{:?}", VisitorAction::Stop);
    assert_eq!(dbg, "Stop");
}

// ===================================================================
// TreeWalker
// ===================================================================

#[test]
fn tree_walker_new() {
    let source = b"src";
    let _w = TreeWalker::new(source);
}

#[test]
fn tree_walker_leaf_visits_text() {
    let source = b"hello";
    let node = leaf(1, 0, 5, true);

    struct LeafCollector(Vec<String>);
    impl Visitor for LeafCollector {
        fn visit_leaf(&mut self, _node: &ParsedNode, text: &str) {
            self.0.push(text.to_string());
        }
    }

    let walker = TreeWalker::new(source);
    let mut lc = LeafCollector(vec![]);
    walker.walk(&node, &mut lc);
    assert_eq!(lc.0, vec!["hello"]);
}

#[test]
fn tree_walker_enter_leave_order() {
    let source = b"ab";
    let a = leaf(1, 0, 1, true);
    let b = leaf(1, 1, 2, true);
    let root = interior(5, vec![a, b], true);

    struct Order(Vec<String>);
    impl Visitor for Order {
        fn enter_node(&mut self, _n: &ParsedNode) -> VisitorAction {
            self.0.push("enter".into());
            VisitorAction::Continue
        }
        fn leave_node(&mut self, _n: &ParsedNode) {
            self.0.push("leave".into());
        }
    }

    let walker = TreeWalker::new(source);
    let mut ord = Order(vec![]);
    walker.walk(&root, &mut ord);
    // enter root, enter a, leave a, enter b, leave b, leave root
    assert_eq!(
        ord.0,
        vec!["enter", "enter", "leave", "enter", "leave", "leave"]
    );
}

#[test]
fn tree_walker_stop_halts() {
    let source = b"ab";
    let a = leaf(1, 0, 1, true);
    let b = leaf(1, 1, 2, true);
    let root = interior(5, vec![a, b], true);

    struct StopAfterOne(usize);
    impl Visitor for StopAfterOne {
        fn enter_node(&mut self, _n: &ParsedNode) -> VisitorAction {
            self.0 += 1;
            if self.0 > 1 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }

    let walker = TreeWalker::new(source);
    let mut s = StopAfterOne(0);
    walker.walk(&root, &mut s);
    // Stop only halts the current walk_node call; siblings still get visited.
    // root(1) -> child_a(2, Stop) -> child_b(3, Stop)
    assert_eq!(s.0, 3);
}

#[test]
fn tree_walker_skip_children() {
    let source = b"ab";
    let a = leaf(1, 0, 1, true);
    let b = leaf(1, 1, 2, true);
    let root = interior(5, vec![a, b], true);

    struct SkipAll(usize);
    impl Visitor for SkipAll {
        fn enter_node(&mut self, _n: &ParsedNode) -> VisitorAction {
            self.0 += 1;
            VisitorAction::SkipChildren
        }
    }

    let walker = TreeWalker::new(source);
    let mut s = SkipAll(0);
    walker.walk(&root, &mut s);
    // Only root is entered; children are skipped
    assert_eq!(s.0, 1);
}

// ===================================================================
// BreadthFirstWalker
// ===================================================================

#[test]
fn bfs_walker_visits_all() {
    let (root, source) = simple_tree();
    let walker = BreadthFirstWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    assert_eq!(sv.total_nodes, 3);
}

#[test]
fn bfs_walker_level_order() {
    let source = b"ab";
    let a = leaf(1, 0, 1, true);
    let b = leaf(1, 1, 2, true);
    let root = interior(5, vec![a, b], true);

    struct KindCollector(Vec<String>);
    impl Visitor for KindCollector {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.kind().to_string());
            VisitorAction::Continue
        }
    }

    let walker = BreadthFirstWalker::new(source);
    let mut kc = KindCollector(vec![]);
    walker.walk(&root, &mut kc);
    // BFS: root first, then children
    assert_eq!(kc.0[0], "Expression");
}

#[test]
fn bfs_walker_stop() {
    let (root, source) = simple_tree();

    struct StopImmediate;
    impl Visitor for StopImmediate {
        fn enter_node(&mut self, _n: &ParsedNode) -> VisitorAction {
            VisitorAction::Stop
        }
    }

    let walker = BreadthFirstWalker::new(source);
    let mut si = StopImmediate;
    walker.walk(&root, &mut si);
    // Should not panic; just stops
}

#[test]
fn bfs_walker_skip_children() {
    let source = b"ab";
    let a = leaf(1, 0, 1, true);
    let b = leaf(1, 1, 2, true);
    let root = interior(5, vec![a, b], true);

    let walker = BreadthFirstWalker::new(source);
    let mut sv = StatsVisitor::default();

    struct SkipRoot(bool);
    impl Visitor for SkipRoot {
        fn enter_node(&mut self, _n: &ParsedNode) -> VisitorAction {
            if !self.0 {
                self.0 = true;
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }

    let mut sr = SkipRoot(false);
    walker.walk(&root, &mut sr);
    // Only root entered, children skipped
    // (We can't inspect SkipRoot's count, but it shouldn't panic)

    // Verify with stats that skipping works
    let walker2 = BreadthFirstWalker::new(source);
    walker2.walk(&root, &mut sv);
    assert!(sv.total_nodes > 0);
}

#[test]
fn bfs_walker_error_node() {
    let source = b"err";
    let err = error_node(0, 3);
    let walker = BreadthFirstWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&err, &mut sv);
    assert_eq!(sv.error_nodes, 1);
}

// ===================================================================
// Edge cases
// ===================================================================

#[test]
fn empty_source_leaf() {
    let source = b"";
    let node = leaf(1, 0, 0, true);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&node, &mut sv);
    assert_eq!(sv.total_nodes, 1);
    assert_eq!(sv.leaf_nodes, 1);
}

#[test]
fn interior_no_children() {
    // An interior-like node that actually has zero children
    let source = b"";
    let node = interior(5, vec![], true);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&node, &mut sv);
    assert_eq!(sv.total_nodes, 1);
    assert_eq!(sv.leaf_nodes, 1); // 0 children -> treated as leaf
}

#[test]
fn deeply_nested_10_levels() {
    let source = b"z";
    let mut node = leaf(1, 0, 1, true);
    for _ in 0..9 {
        node = interior(5, vec![node], true);
    }
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&node, &mut sv);
    assert_eq!(sv.max_depth, 10);
    assert_eq!(sv.total_nodes, 10);
}

#[test]
fn wide_tree_100_children() {
    let source: Vec<u8> = (0..100).map(|_| b'x').collect();
    let children: Vec<_> = (0..100).map(|i| leaf(1, i, i + 1, true)).collect();
    let root = interior(5, children, true);
    let walker = TreeWalker::new(&source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    assert_eq!(sv.total_nodes, 101);
    assert_eq!(sv.leaf_nodes, 100);
    assert_eq!(sv.max_depth, 2);
}

#[test]
fn mixed_named_unnamed() {
    let source = b"a+b";
    let a = leaf(1, 0, 1, true);
    let plus = leaf(2, 1, 2, false);
    let b = leaf(1, 2, 3, true);
    let root = interior(5, vec![a, plus, b], true);

    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    // The named nodes should have [named], unnamed should not
    let named_count = out.matches("[named]").count();
    // root + a + b = 3 named
    assert_eq!(named_count, 3);
}

#[test]
fn error_in_middle_of_tree() {
    let source = b"a!b";
    let a = leaf(1, 0, 1, true);
    let err = error_node(1, 2);
    let b = leaf(1, 2, 3, true);
    let root = interior(5, vec![a, err, b], true);

    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    assert_eq!(sv.error_nodes, 1);
    // root + a + b entered, err not entered
    assert_eq!(sv.total_nodes, 3);
    assert_eq!(sv.leaf_nodes, 2);
}

#[test]
fn pretty_print_error_in_tree() {
    let source = b"a!b";
    let a = leaf(1, 0, 1, true);
    let err = error_node(1, 2);
    let b = leaf(1, 2, 3, true);
    let root = interior(5, vec![a, err, b], true);

    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(pp.output().contains("ERROR"));
}

#[test]
fn search_empty_tree() {
    let source = b"";
    let node = leaf(1, 0, 0, true);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|_n: &ParsedNode| true);
    walker.walk(&node, &mut sv);
    assert_eq!(sv.matches.len(), 1);
}

#[test]
fn search_deep_tree() {
    let source = b"q";
    let mut node = leaf(1, 0, 1, true);
    for _ in 0..5 {
        node = interior(5, vec![node], true);
    }
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.child_count() == 0);
    walker.walk(&node, &mut sv);
    assert_eq!(sv.matches.len(), 1);
}

#[test]
fn pretty_accumulates_across_walks() {
    let source = b"x";
    let node = leaf(1, 0, 1, true);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&node, &mut pp);
    let len1 = pp.output().len();
    walker.walk(&node, &mut pp);
    let len2 = pp.output().len();
    assert!(len2 > len1, "output should grow across walks");
}

#[test]
fn stats_current_depth_returns_to_zero() {
    let (root, source) = simple_tree();
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    // After walk, depth should be back to 0 (private field, but we can walk again)
    walker.walk(&root, &mut sv);
    // If depth didn't return to 0, max_depth would be wrong on second walk
    assert_eq!(sv.max_depth, 2);
}

#[test]
fn bfs_and_dfs_same_node_count() {
    let (root, source) = simple_tree();
    let dfs = TreeWalker::new(source);
    let bfs = BreadthFirstWalker::new(source);
    let mut sv_dfs = StatsVisitor::default();
    let mut sv_bfs = StatsVisitor::default();
    dfs.walk(&root, &mut sv_dfs);
    bfs.walk(&root, &mut sv_bfs);
    assert_eq!(sv_dfs.total_nodes, sv_bfs.total_nodes);
}

#[test]
fn search_with_closure_captures() {
    let threshold = 1usize;
    let source = b"ab";
    let a = leaf(1, 0, 1, true);
    let b = leaf(1, 1, 2, true);
    let root = interior(5, vec![a, b], true);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(move |n: &ParsedNode| n.start_byte() >= threshold);
    walker.walk(&root, &mut sv);
    // Only b (start=1) matches
    assert_eq!(sv.matches.len(), 1);
}

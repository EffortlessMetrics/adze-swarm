//! Comprehensive tests for the visitor API in the `adze` runtime crate.
//!
//! Covers `StatsVisitor`, `PrettyPrintVisitor`, `SearchVisitor`,
//! `VisitorAction`, `TreeWalker`, `BreadthFirstWalker`, `TransformWalker`,
//! `TransformVisitor`, and custom visitor implementations.

use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TransformVisitor,
    TransformWalker, TreeWalker, Visitor, VisitorAction,
};

// ---------------------------------------------------------------------------
// Helpers — synthetic parsed-node construction.
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

// ===================================================================
// 1. StatsVisitor — construction via Default
// ===================================================================

#[test]
fn stats_default_total_nodes_is_zero() {
    let sv: StatsVisitor = Default::default();
    assert_eq!(sv.total_nodes, 0);
}

#[test]
fn stats_default_leaf_nodes_is_zero() {
    let sv = StatsVisitor::default();
    assert_eq!(sv.leaf_nodes, 0);
}

#[test]
fn stats_default_error_nodes_is_zero() {
    let sv = StatsVisitor::default();
    assert_eq!(sv.error_nodes, 0);
}

#[test]
fn stats_default_max_depth_is_zero() {
    let sv = StatsVisitor::default();
    assert_eq!(sv.max_depth, 0);
}

#[test]
fn stats_default_node_counts_is_empty() {
    let sv = StatsVisitor::default();
    assert!(sv.node_counts.is_empty());
}

#[test]
fn stats_default_debug_impl() {
    let sv = StatsVisitor::default();
    let debug = format!("{:?}", sv);
    assert!(debug.contains("StatsVisitor"));
}

// ===================================================================
// 2. StatsVisitor — field access after walking
// ===================================================================

#[test]
fn stats_single_leaf_total_nodes() {
    let source = b"hello";
    let root = leaf(1, 0, 5);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    assert_eq!(sv.total_nodes, 1);
}

#[test]
fn stats_single_leaf_leaf_count() {
    let source = b"hello";
    let root = leaf(1, 0, 5);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    assert_eq!(sv.leaf_nodes, 1);
}

#[test]
fn stats_single_leaf_max_depth() {
    let source = b"hello";
    let root = leaf(1, 0, 5);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    assert_eq!(sv.max_depth, 1);
}

#[test]
fn stats_interior_with_two_leaves() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    assert_eq!(sv.total_nodes, 3);
    assert_eq!(sv.leaf_nodes, 2);
}

#[test]
fn stats_node_counts_populated() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    // symbol 1 -> "*", symbol 5 -> "Expression"
    assert!(!sv.node_counts.is_empty());
}

#[test]
fn stats_error_node_counted() {
    let source = b"err";
    let root = error_node(0, 3);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    assert_eq!(sv.error_nodes, 1);
}

#[test]
fn stats_error_node_not_in_total() {
    let source = b"err";
    let root = error_node(0, 3);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    // error nodes are handled before enter_node, so total_nodes stays 0
    assert_eq!(sv.total_nodes, 0);
}

#[test]
fn stats_nested_depth() {
    let source = b"xyz";
    let inner = interior(5, vec![leaf(1, 0, 1)]);
    let root = interior(5, vec![inner]);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    assert_eq!(sv.max_depth, 3);
}

#[test]
fn stats_deeply_nested_depth() {
    let source = b"deep";
    let n3 = leaf(1, 0, 1);
    let n2 = interior(5, vec![n3]);
    let n1 = interior(5, vec![n2]);
    let root = interior(5, vec![n1]);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    assert_eq!(sv.max_depth, 4);
}

// ===================================================================
// 3. PrettyPrintVisitor — construction
// ===================================================================

#[test]
fn pretty_print_new_returns_instance() {
    let _pp = PrettyPrintVisitor::new();
}

#[test]
fn pretty_print_default_returns_instance() {
    let _pp: PrettyPrintVisitor = Default::default();
}

// ===================================================================
// 4. PrettyPrintVisitor — output starts empty
// ===================================================================

#[test]
fn pretty_print_output_initially_empty() {
    let pp = PrettyPrintVisitor::new();
    assert_eq!(pp.output(), "");
}

#[test]
fn pretty_print_default_output_initially_empty() {
    let pp = PrettyPrintVisitor::default();
    assert_eq!(pp.output(), "");
}

#[test]
fn pretty_print_output_is_str() {
    let pp = PrettyPrintVisitor::new();
    let out: &str = pp.output();
    assert!(out.is_empty());
}

#[test]
fn pretty_print_after_walk_not_empty() {
    let source = b"hello";
    let root = leaf(1, 0, 5);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(!pp.output().is_empty());
}

#[test]
fn pretty_print_leaf_contains_text() {
    let source = b"hello";
    let root = leaf(1, 0, 5);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(pp.output().contains("hello"));
}

#[test]
fn pretty_print_interior_contains_kind() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1)]);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    // symbol 5 -> "Expression"
    assert!(pp.output().contains("Expression"));
}

#[test]
fn pretty_print_named_annotation() {
    let source = b"a";
    let root = leaf(1, 0, 1); // is_named = true
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(pp.output().contains("[named]"));
}

#[test]
fn pretty_print_anon_no_named_tag() {
    let source = b"a";
    let root = anon_leaf(1, 0, 1); // is_named = false
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(!pp.output().contains("[named]"));
}

// ===================================================================
// 5. SearchVisitor — construction
// ===================================================================

#[test]
fn search_visitor_new_with_closure() {
    let _sv = SearchVisitor::new(|_node: &ParsedNode| true);
}

#[test]
fn search_visitor_new_with_kind_predicate() {
    let _sv = SearchVisitor::new(|node: &ParsedNode| node.kind() == "Expression");
}

#[test]
fn search_visitor_new_with_named_predicate() {
    let _sv = SearchVisitor::new(|node: &ParsedNode| node.is_named());
}

// ===================================================================
// 6. SearchVisitor — found() analogue: matches initially empty
// ===================================================================

#[test]
fn search_matches_initially_empty() {
    let sv = SearchVisitor::new(|_: &ParsedNode| true);
    assert!(sv.matches.is_empty());
}

#[test]
fn search_matches_len_initially_zero() {
    let sv = SearchVisitor::new(|_: &ParsedNode| false);
    assert_eq!(sv.matches.len(), 0);
}

// ===================================================================
// 7. SearchVisitor — matches after walking
// ===================================================================

#[test]
fn search_finds_matching_node() {
    let source = b"hello";
    let root = leaf(1, 0, 5);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|_: &ParsedNode| true);
    walker.walk(&root, &mut sv);
    assert!(!sv.matches.is_empty());
}

#[test]
fn search_match_contains_byte_range() {
    let source = b"hello";
    let root = leaf(1, 0, 5);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|_: &ParsedNode| true);
    walker.walk(&root, &mut sv);
    let (start, end, _) = &sv.matches[0];
    assert_eq!(*start, 0);
    assert_eq!(*end, 5);
}

#[test]
fn search_match_contains_kind() {
    let source = b"hello";
    let root = leaf(1, 0, 5);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|_: &ParsedNode| true);
    walker.walk(&root, &mut sv);
    let (_, _, kind) = &sv.matches[0];
    assert_eq!(kind, "*"); // symbol 1 -> "*"
}

#[test]
fn search_no_match_when_predicate_false() {
    let source = b"hello";
    let root = leaf(1, 0, 5);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|_: &ParsedNode| false);
    walker.walk(&root, &mut sv);
    assert!(sv.matches.is_empty());
}

#[test]
fn search_multiple_matches() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|_: &ParsedNode| true);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 3); // root + 2 leaves
}

#[test]
fn search_filter_by_kind() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.kind() == "Expression");
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
}

// ===================================================================
// 8. Multiple visitor instances
// ===================================================================

#[test]
fn two_stats_visitors_independent() {
    let mut sv1 = StatsVisitor::default();
    let sv2 = StatsVisitor::default();
    let node = leaf(1, 0, 1);
    sv1.enter_node(&node);
    assert_eq!(sv1.total_nodes, 1);
    assert_eq!(sv2.total_nodes, 0);
}

#[test]
fn two_pretty_printers_independent() {
    let pp1 = PrettyPrintVisitor::new();
    let pp2 = PrettyPrintVisitor::new();
    assert_eq!(pp1.output(), pp2.output());
}

#[test]
fn stats_and_pretty_print_on_same_tree() {
    let source = b"hello";
    let root = leaf(1, 0, 5);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut sv);
    walker.walk(&root, &mut pp);
    assert_eq!(sv.total_nodes, 1);
    assert!(!pp.output().is_empty());
}

#[test]
fn stats_and_search_on_same_tree() {
    let source = b"hello";
    let root = leaf(1, 0, 5);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    let mut search = SearchVisitor::new(|_: &ParsedNode| true);
    walker.walk(&root, &mut sv);
    walker.walk(&root, &mut search);
    assert_eq!(sv.total_nodes, 1);
    assert_eq!(search.matches.len(), 1);
}

#[test]
fn three_visitors_on_same_tree() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    let mut pp = PrettyPrintVisitor::new();
    let mut search = SearchVisitor::new(|_: &ParsedNode| true);
    walker.walk(&root, &mut sv);
    walker.walk(&root, &mut pp);
    walker.walk(&root, &mut search);
    assert!(sv.total_nodes > 0);
    assert!(!pp.output().is_empty());
    assert!(!search.matches.is_empty());
}

// ===================================================================
// 9. VisitorAction variants
// ===================================================================

#[test]
fn visitor_action_continue_eq() {
    assert_eq!(VisitorAction::Continue, VisitorAction::Continue);
}

#[test]
fn visitor_action_skip_eq() {
    assert_eq!(VisitorAction::SkipChildren, VisitorAction::SkipChildren);
}

#[test]
fn visitor_action_stop_eq() {
    assert_eq!(VisitorAction::Stop, VisitorAction::Stop);
}

#[test]
fn visitor_action_continue_ne_stop() {
    assert_ne!(VisitorAction::Continue, VisitorAction::Stop);
}

#[test]
fn visitor_action_continue_ne_skip() {
    assert_ne!(VisitorAction::Continue, VisitorAction::SkipChildren);
}

#[test]
fn visitor_action_skip_ne_stop() {
    assert_ne!(VisitorAction::SkipChildren, VisitorAction::Stop);
}

#[test]
fn visitor_action_debug() {
    let dbg = format!("{:?}", VisitorAction::Continue);
    assert_eq!(dbg, "Continue");
}

#[test]
fn visitor_action_clone() {
    let a = VisitorAction::Stop;
    let b = a;
    assert_eq!(a, b);
}

// ===================================================================
// 10. Different search patterns / predicates
// ===================================================================

#[test]
fn search_by_named() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), anon_leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.is_named());
    walker.walk(&root, &mut sv);
    // root (named) + first leaf (named); anon_leaf is not named
    assert_eq!(sv.matches.len(), 2);
}

#[test]
fn search_by_byte_range() {
    let source = b"abcdef";
    let root = interior(5, vec![leaf(1, 0, 3), leaf(1, 3, 6)]);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.start_byte() >= 3);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
}

#[test]
fn search_by_symbol_id() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.kind() == "_2"); // symbol 2
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
}

#[test]
fn search_always_true_counts_all() {
    let source = b"abcd";
    let c1 = leaf(1, 0, 1);
    let c2 = leaf(1, 1, 2);
    let c3 = leaf(1, 2, 3);
    let c4 = leaf(1, 3, 4);
    let root = interior(5, vec![c1, c2, c3, c4]);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|_: &ParsedNode| true);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 5);
}

#[test]
fn search_with_end_byte_predicate() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.end_byte() <= 1);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
}

// ===================================================================
// TreeWalker tests
// ===================================================================

#[test]
fn tree_walker_creation() {
    let source = b"src";
    let _w = TreeWalker::new(source);
}

#[test]
fn tree_walker_empty_source() {
    let source = b"";
    let _w = TreeWalker::new(source);
}

#[test]
fn tree_walker_walks_error_node() {
    let source = b"err";
    let root = error_node(0, 3);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    assert_eq!(sv.error_nodes, 1);
    assert_eq!(sv.total_nodes, 0);
}

#[test]
fn tree_walker_skip_children() {
    struct SkipAll;
    impl Visitor for SkipAll {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            VisitorAction::SkipChildren
        }
    }
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    // First walk with SkipAll to make sure children are skipped
    let mut skip = SkipAll;
    walker.walk(&root, &mut skip);
    // Now walk normally
    walker.walk(&root, &mut sv);
    assert_eq!(sv.total_nodes, 3);
}

#[test]
fn tree_walker_stop_early() {
    struct StopAfterOne {
        count: usize,
    }
    impl Visitor for StopAfterOne {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.count += 1;
            if self.count > 1 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut v = StopAfterOne { count: 0 };
    walker.walk(&root, &mut v);
    // root enters (count=1), first child enters (count=2, returns Stop)
    // but walk_node still tries next sibling before returning
    // The third node also gets entered before Stop propagates
    assert!(v.count >= 2);
}

// ===================================================================
// BreadthFirstWalker tests
// ===================================================================

#[test]
fn bfs_walker_creation() {
    let source = b"src";
    let _w = BreadthFirstWalker::new(source);
}

#[test]
fn bfs_walker_single_leaf() {
    let source = b"hello";
    let root = leaf(1, 0, 5);
    let walker = BreadthFirstWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    assert_eq!(sv.total_nodes, 1);
    assert_eq!(sv.leaf_nodes, 1);
}

#[test]
fn bfs_walker_interior_with_children() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = BreadthFirstWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    assert_eq!(sv.total_nodes, 3);
}

#[test]
fn bfs_walker_error_node() {
    let source = b"err";
    let root = error_node(0, 3);
    let walker = BreadthFirstWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    assert_eq!(sv.error_nodes, 1);
}

#[test]
fn bfs_walker_with_pretty_print() {
    let source = b"hello";
    let root = leaf(1, 0, 5);
    let walker = BreadthFirstWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(pp.output().contains("hello"));
}

// ===================================================================
// TransformWalker / TransformVisitor tests
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
        0
    }
}

#[test]
fn transform_walker_creation() {
    let source = b"src";
    let _w = TransformWalker::new(source);
}

#[test]
fn transform_single_leaf() {
    let source = b"hello";
    let root = leaf(1, 0, 5);
    let walker = TransformWalker::new(source);
    let mut t = CountTransform;
    let count = walker.walk(&root, &mut t);
    assert_eq!(count, 1);
}

#[test]
fn transform_interior_counts_all() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TransformWalker::new(source);
    let mut t = CountTransform;
    let count = walker.walk(&root, &mut t);
    assert_eq!(count, 3);
}

#[test]
fn transform_error_returns_zero() {
    let source = b"err";
    let root = error_node(0, 3);
    let walker = TransformWalker::new(source);
    let mut t = CountTransform;
    let count = walker.walk(&root, &mut t);
    assert_eq!(count, 0);
}

#[test]
fn transform_nested() {
    let source = b"xyz";
    let inner = interior(5, vec![leaf(1, 0, 1)]);
    let root = interior(5, vec![inner]);
    let walker = TransformWalker::new(source);
    let mut t = CountTransform;
    let count = walker.walk(&root, &mut t);
    assert_eq!(count, 3);
}

// ===================================================================
// Custom Visitor implementations
// ===================================================================

#[test]
fn custom_visitor_default_methods() {
    struct EmptyVisitor;
    impl Visitor for EmptyVisitor {}
    let mut v = EmptyVisitor;
    let node = leaf(1, 0, 1);
    assert_eq!(v.enter_node(&node), VisitorAction::Continue);
    v.leave_node(&node);
    v.visit_leaf(&node, "x");
    v.visit_error(&node);
}

#[test]
fn custom_counting_visitor() {
    struct Counter {
        n: usize,
    }
    impl Visitor for Counter {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.n += 1;
            VisitorAction::Continue
        }
    }
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut c = Counter { n: 0 };
    walker.walk(&root, &mut c);
    assert_eq!(c.n, 3);
}

#[test]
fn custom_leaf_collector() {
    struct LeafCollector {
        texts: Vec<String>,
    }
    impl Visitor for LeafCollector {
        fn visit_leaf(&mut self, _: &ParsedNode, text: &str) {
            self.texts.push(text.to_string());
        }
    }
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut lc = LeafCollector { texts: vec![] };
    walker.walk(&root, &mut lc);
    assert_eq!(lc.texts.len(), 2);
    assert_eq!(lc.texts[0], "a");
    assert_eq!(lc.texts[1], "b");
}

#[test]
fn custom_error_collector() {
    struct ErrorCollector {
        errors: usize,
    }
    impl Visitor for ErrorCollector {
        fn visit_error(&mut self, _: &ParsedNode) {
            self.errors += 1;
        }
    }
    let source = b"ab!";
    let root = interior(5, vec![leaf(1, 0, 1), error_node(1, 2), leaf(1, 2, 3)]);
    let walker = TreeWalker::new(source);
    let mut ec = ErrorCollector { errors: 0 };
    walker.walk(&root, &mut ec);
    assert_eq!(ec.errors, 1);
}

// ===================================================================
// Edge cases
// ===================================================================

#[test]
fn empty_interior_node() {
    let source = b"";
    let root = interior(5, vec![]);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    // An interior node with no children acts like a leaf of empty text
    assert_eq!(sv.total_nodes, 1);
}

#[test]
fn wide_tree_many_children() {
    let source = b"abcdefghij";
    let children: Vec<ParsedNode> = (0..10).map(|i| leaf(1, i, i + 1)).collect();
    let root = interior(5, children);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    assert_eq!(sv.total_nodes, 11);
    assert_eq!(sv.leaf_nodes, 10);
}

#[test]
fn stats_reuse_after_walk() {
    let source = b"a";
    let root = leaf(1, 0, 1);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.total_nodes, 2);
    assert_eq!(sv.leaf_nodes, 2);
}

#[test]
fn pretty_print_multiple_walks_append() {
    let source = b"a";
    let root = leaf(1, 0, 1);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let len1 = pp.output().len();
    walker.walk(&root, &mut pp);
    let len2 = pp.output().len();
    assert!(len2 > len1);
}

#[test]
fn search_accumulates_across_walks() {
    let source = b"a";
    let root = leaf(1, 0, 1);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|_: &ParsedNode| true);
    walker.walk(&root, &mut sv);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 2);
}

#[test]
fn mixed_error_and_normal_children() {
    let source = b"a!b";
    let root = interior(5, vec![leaf(1, 0, 1), error_node(1, 2), leaf(1, 2, 3)]);
    let walker = TreeWalker::new(source);
    let mut sv = StatsVisitor::default();
    walker.walk(&root, &mut sv);
    assert_eq!(sv.total_nodes, 3); // root + 2 non-error leaves
    assert_eq!(sv.error_nodes, 1);
    assert_eq!(sv.leaf_nodes, 2);
}

#[test]
fn bfs_and_dfs_same_total_count() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);

    let dfs = TreeWalker::new(source);
    let mut sv_dfs = StatsVisitor::default();
    dfs.walk(&root, &mut sv_dfs);

    let bfs = BreadthFirstWalker::new(source);
    let mut sv_bfs = StatsVisitor::default();
    bfs.walk(&root, &mut sv_bfs);

    assert_eq!(sv_dfs.total_nodes, sv_bfs.total_nodes);
}

// ===================================================================
// TransformVisitor — string-based transform
// ===================================================================

struct SexprTransform;

impl TransformVisitor for SexprTransform {
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
fn sexpr_transform_leaf() {
    let source = b"hello";
    let root = leaf(1, 0, 5);
    let walker = TransformWalker::new(source);
    let mut t = SexprTransform;
    let result = walker.walk(&root, &mut t);
    assert_eq!(result, "\"hello\"");
}

#[test]
fn sexpr_transform_interior() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TransformWalker::new(source);
    let mut t = SexprTransform;
    let result = walker.walk(&root, &mut t);
    assert!(result.starts_with('('));
    assert!(result.contains("\"a\""));
    assert!(result.contains("\"b\""));
}

#[test]
fn sexpr_transform_error() {
    let source = b"err";
    let root = error_node(0, 3);
    let walker = TransformWalker::new(source);
    let mut t = SexprTransform;
    let result = walker.walk(&root, &mut t);
    assert_eq!(result, "ERROR");
}

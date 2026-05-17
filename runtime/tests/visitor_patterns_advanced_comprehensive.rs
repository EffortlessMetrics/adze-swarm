//! Advanced comprehensive tests for visitor pattern types in the adze runtime.
//!
//! Covers: VisitorAction variants/traits, StatsVisitor, PrettyPrintVisitor,
//! SearchVisitor, multiple-instance scenarios, and edge cases.

use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TreeWalker, VisitorAction,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Construct a `ParsedNode`.  The `language` field is `pub(crate)` so we
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

/// Simple tree: root(10) -> [a(1), b(2)]  source "ab"
fn simple_tree() -> (ParsedNode, Vec<u8>) {
    let src = b"ab".to_vec();
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    (root, src)
}

// ===================================================================
// 1. VisitorAction variants
// ===================================================================

#[test]
fn va_continue_exists() {
    let _a = VisitorAction::Continue;
}

#[test]
fn va_skip_children_exists() {
    let _a = VisitorAction::SkipChildren;
}

#[test]
fn va_stop_exists() {
    let _a = VisitorAction::Stop;
}

#[test]
fn va_continue_is_continue() {
    assert!(matches!(VisitorAction::Continue, VisitorAction::Continue));
}

#[test]
fn va_skip_is_skip() {
    assert!(matches!(
        VisitorAction::SkipChildren,
        VisitorAction::SkipChildren
    ));
}

#[test]
fn va_stop_is_stop() {
    assert!(matches!(VisitorAction::Stop, VisitorAction::Stop));
}

// ===================================================================
// 2. VisitorAction Clone, Debug, PartialEq
// ===================================================================

#[test]
fn va_clone_continue() {
    let a = VisitorAction::Continue;
    assert_eq!(a, a.clone());
}

#[test]
fn va_clone_skip() {
    let a = VisitorAction::SkipChildren;
    assert_eq!(a, a.clone());
}

#[test]
fn va_clone_stop() {
    let a = VisitorAction::Stop;
    assert_eq!(a, a.clone());
}

#[test]
fn va_debug_continue_format() {
    let s = format!("{:?}", VisitorAction::Continue);
    assert_eq!(s, "Continue");
}

#[test]
fn va_debug_skip_format() {
    let s = format!("{:?}", VisitorAction::SkipChildren);
    assert_eq!(s, "SkipChildren");
}

#[test]
fn va_debug_stop_format() {
    let s = format!("{:?}", VisitorAction::Stop);
    assert_eq!(s, "Stop");
}

#[test]
fn va_eq_same_variant() {
    assert_eq!(VisitorAction::Continue, VisitorAction::Continue);
    assert_eq!(VisitorAction::SkipChildren, VisitorAction::SkipChildren);
    assert_eq!(VisitorAction::Stop, VisitorAction::Stop);
}

#[test]
fn va_ne_continue_vs_skip() {
    assert_ne!(VisitorAction::Continue, VisitorAction::SkipChildren);
}

#[test]
fn va_ne_continue_vs_stop() {
    assert_ne!(VisitorAction::Continue, VisitorAction::Stop);
}

#[test]
fn va_ne_skip_vs_stop() {
    assert_ne!(VisitorAction::SkipChildren, VisitorAction::Stop);
}

#[test]
fn va_copy_semantics() {
    let a = VisitorAction::Continue;
    let b = a; // Copy
    let c = a; // still valid after copy
    assert_eq!(b, c);
}

#[test]
fn va_trait_bound_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<VisitorAction>();
}

#[test]
fn va_trait_bound_clone() {
    fn assert_clone<T: Clone>() {}
    assert_clone::<VisitorAction>();
}

#[test]
fn va_trait_bound_copy() {
    fn assert_copy<T: Copy>() {}
    assert_copy::<VisitorAction>();
}

#[test]
fn va_trait_bound_partial_eq() {
    fn assert_partial_eq<T: PartialEq>() {}
    assert_partial_eq::<VisitorAction>();
}

#[test]
fn va_trait_bound_eq() {
    fn assert_eq_trait<T: Eq>() {}
    assert_eq_trait::<VisitorAction>();
}

#[test]
fn va_size_is_small() {
    assert!(std::mem::size_of::<VisitorAction>() <= 1);
}

#[test]
fn va_all_variants_in_vec() {
    let v = [
        VisitorAction::Continue,
        VisitorAction::SkipChildren,
        VisitorAction::Stop,
    ];
    assert_eq!(v.len(), 3);
    // All pairwise distinct
    assert_ne!(v[0], v[1]);
    assert_ne!(v[0], v[2]);
    assert_ne!(v[1], v[2]);
}

#[test]
fn va_exhaustive_match() {
    for action in [
        VisitorAction::Continue,
        VisitorAction::SkipChildren,
        VisitorAction::Stop,
    ] {
        let label = match action {
            VisitorAction::Continue => "c",
            VisitorAction::SkipChildren => "s",
            VisitorAction::Stop => "x",
        };
        assert!(!label.is_empty());
    }
}

#[test]
fn va_clone_preserves_equality() {
    let actions = [
        VisitorAction::Continue,
        VisitorAction::SkipChildren,
        VisitorAction::Stop,
    ];
    for a in &actions {
        assert_eq!(*a, a.clone());
    }
}

// ===================================================================
// 3. StatsVisitor creation via Default
// ===================================================================

#[test]
fn stats_default_creates_instance() {
    let _v = StatsVisitor::default();
}

#[test]
fn stats_default_total_nodes_zero() {
    let v = StatsVisitor::default();
    assert_eq!(v.total_nodes, 0);
}

#[test]
fn stats_default_leaf_nodes_zero() {
    let v = StatsVisitor::default();
    assert_eq!(v.leaf_nodes, 0);
}

#[test]
fn stats_default_error_nodes_zero() {
    let v = StatsVisitor::default();
    assert_eq!(v.error_nodes, 0);
}

#[test]
fn stats_default_max_depth_zero() {
    let v = StatsVisitor::default();
    assert_eq!(v.max_depth, 0);
}

#[test]
fn stats_default_node_counts_empty() {
    let v = StatsVisitor::default();
    assert!(v.node_counts.is_empty());
}

#[test]
fn stats_two_defaults_identical() {
    let v1 = StatsVisitor::default();
    let v2 = StatsVisitor::default();
    assert_eq!(v1.total_nodes, v2.total_nodes);
    assert_eq!(v1.leaf_nodes, v2.leaf_nodes);
    assert_eq!(v1.error_nodes, v2.error_nodes);
    assert_eq!(v1.max_depth, v2.max_depth);
    assert_eq!(v1.node_counts.len(), v2.node_counts.len());
}

// ===================================================================
// 4. StatsVisitor Debug format
// ===================================================================

#[test]
fn stats_debug_non_empty() {
    let v = StatsVisitor::default();
    let s = format!("{:?}", v);
    assert!(!s.is_empty());
}

#[test]
fn stats_debug_contains_struct_name() {
    let v = StatsVisitor::default();
    let s = format!("{:?}", v);
    assert!(s.contains("StatsVisitor"));
}

#[test]
fn stats_debug_contains_total_nodes() {
    let v = StatsVisitor::default();
    let s = format!("{:?}", v);
    assert!(s.contains("total_nodes"));
}

#[test]
fn stats_debug_contains_leaf_nodes() {
    let v = StatsVisitor::default();
    let s = format!("{:?}", v);
    assert!(s.contains("leaf_nodes"));
}

#[test]
fn stats_debug_contains_max_depth() {
    let v = StatsVisitor::default();
    let s = format!("{:?}", v);
    assert!(s.contains("max_depth"));
}

#[test]
fn stats_debug_after_walk() {
    let (tree, src) = simple_tree();
    let walker = TreeWalker::new(&src);
    let mut v = StatsVisitor::default();
    walker.walk(&tree, &mut v);
    let s = format!("{:?}", v);
    assert!(s.contains("StatsVisitor"));
    // total_nodes should be non-zero now
    assert!(v.total_nodes > 0);
}

#[test]
fn stats_trait_bound_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<StatsVisitor>();
}

#[test]
fn stats_trait_bound_default() {
    fn assert_default<T: Default>() {}
    assert_default::<StatsVisitor>();
}

// ===================================================================
// 5. PrettyPrintVisitor::new() creation
// ===================================================================

#[test]
fn pp_new_creates_instance() {
    let _v = PrettyPrintVisitor::new();
}

#[test]
fn pp_default_creates_instance() {
    let _v = PrettyPrintVisitor::default();
}

#[test]
fn pp_new_and_default_equivalent() {
    let v1 = PrettyPrintVisitor::new();
    let v2 = PrettyPrintVisitor::default();
    assert_eq!(v1.output(), v2.output());
}

// ===================================================================
// 6. PrettyPrintVisitor output is initially empty
// ===================================================================

#[test]
fn pp_output_initially_empty() {
    let v = PrettyPrintVisitor::new();
    assert!(v.output().is_empty());
}

#[test]
fn pp_output_initially_empty_len() {
    let v = PrettyPrintVisitor::new();
    assert_eq!(v.output().len(), 0);
}

#[test]
fn pp_output_initially_eq_empty_str() {
    let v = PrettyPrintVisitor::new();
    assert_eq!(v.output(), "");
}

#[test]
fn pp_default_output_empty() {
    let v = PrettyPrintVisitor::default();
    assert!(v.output().is_empty());
}

#[test]
fn pp_output_non_empty_after_walk() {
    let (tree, src) = simple_tree();
    let walker = TreeWalker::new(&src);
    let mut v = PrettyPrintVisitor::new();
    walker.walk(&tree, &mut v);
    assert!(!v.output().is_empty());
}

#[test]
fn pp_output_contains_newline_after_walk() {
    let (tree, src) = simple_tree();
    let walker = TreeWalker::new(&src);
    let mut v = PrettyPrintVisitor::new();
    walker.walk(&tree, &mut v);
    assert!(v.output().contains('\n'));
}

// ===================================================================
// 7. SearchVisitor creation with various predicates
// ===================================================================

#[test]
fn search_new_with_always_true() {
    let _v = SearchVisitor::new(|_node: &ParsedNode| true);
}

#[test]
fn search_new_with_always_false() {
    let _v = SearchVisitor::new(|_node: &ParsedNode| false);
}

#[test]
fn search_new_matches_empty() {
    let v = SearchVisitor::new(|_node: &ParsedNode| true);
    assert!(v.matches.is_empty());
}

#[test]
fn search_new_with_kind_predicate() {
    let _v = SearchVisitor::new(|node: &ParsedNode| node.kind() == "identifier");
}

#[test]
fn search_new_with_byte_range_predicate() {
    let _v = SearchVisitor::new(|node: &ParsedNode| node.start_byte() < 10);
}

#[test]
fn search_new_with_named_predicate() {
    let _v = SearchVisitor::new(|node: &ParsedNode| node.is_named());
}

#[test]
fn search_new_with_error_predicate() {
    let _v = SearchVisitor::new(|node: &ParsedNode| node.is_error());
}

#[test]
fn search_new_with_symbol_predicate() {
    let _v = SearchVisitor::new(|node: &ParsedNode| node.symbol() == 5);
}

#[test]
fn search_new_with_composite_predicate() {
    let _v = SearchVisitor::new(|node: &ParsedNode| node.is_named() && node.start_byte() < 100);
}

#[test]
fn search_walk_always_true_matches_all() {
    let (tree, src) = simple_tree();
    let walker = TreeWalker::new(&src);
    let mut v = SearchVisitor::new(|_node: &ParsedNode| true);
    walker.walk(&tree, &mut v);
    // root + 2 leaves = 3 nodes
    assert_eq!(v.matches.len(), 3);
}

#[test]
fn search_walk_always_false_matches_none() {
    let (tree, src) = simple_tree();
    let walker = TreeWalker::new(&src);
    let mut v = SearchVisitor::new(|_node: &ParsedNode| false);
    walker.walk(&tree, &mut v);
    assert!(v.matches.is_empty());
}

#[test]
fn search_walk_by_symbol() {
    let (tree, src) = simple_tree();
    let walker = TreeWalker::new(&src);
    let mut v = SearchVisitor::new(|node: &ParsedNode| node.symbol() == 1);
    walker.walk(&tree, &mut v);
    assert_eq!(v.matches.len(), 1);
}

#[test]
fn search_match_tuples_contain_bytes_and_kind() {
    let (tree, src) = simple_tree();
    let walker = TreeWalker::new(&src);
    let mut v = SearchVisitor::new(|node: &ParsedNode| node.symbol() == 1);
    walker.walk(&tree, &mut v);
    let (start, end, ref kind) = v.matches[0];
    assert_eq!(start, 0);
    assert_eq!(end, 1);
    assert!(!kind.is_empty());
}

// ===================================================================
// 8. Multiple visitor instances
// ===================================================================

#[test]
fn multiple_stats_visitors_independent() {
    let (tree, src) = simple_tree();
    let walker = TreeWalker::new(&src);

    let mut v1 = StatsVisitor::default();
    walker.walk(&tree, &mut v1);

    let v2 = StatsVisitor::default();
    assert_ne!(v1.total_nodes, v2.total_nodes);
}

#[test]
fn multiple_pp_visitors_independent() {
    let (tree, src) = simple_tree();
    let walker = TreeWalker::new(&src);

    let mut v1 = PrettyPrintVisitor::new();
    walker.walk(&tree, &mut v1);

    let v2 = PrettyPrintVisitor::new();
    assert_ne!(v1.output(), v2.output());
}

#[test]
fn two_search_visitors_same_result() {
    let (tree, src) = simple_tree();
    let walker = TreeWalker::new(&src);

    let mut v1 = SearchVisitor::new(|_n: &ParsedNode| true);
    walker.walk(&tree, &mut v1);

    let mut v2 = SearchVisitor::new(|_n: &ParsedNode| true);
    walker.walk(&tree, &mut v2);

    assert_eq!(v1.matches.len(), v2.matches.len());
}

#[test]
fn stats_pp_search_all_at_once() {
    let (tree, src) = simple_tree();
    let walker = TreeWalker::new(&src);

    let mut stats = StatsVisitor::default();
    walker.walk(&tree, &mut stats);

    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&tree, &mut pp);

    let mut search = SearchVisitor::new(|_n: &ParsedNode| true);
    walker.walk(&tree, &mut search);

    assert!(stats.total_nodes > 0);
    assert!(!pp.output().is_empty());
    assert!(!search.matches.is_empty());
}

#[test]
fn three_stats_visitors_same_results() {
    let (tree, src) = simple_tree();
    let walker = TreeWalker::new(&src);

    let mut visitors: Vec<StatsVisitor> = (0..3).map(|_| StatsVisitor::default()).collect();
    for v in &mut visitors {
        walker.walk(&tree, v);
    }
    assert_eq!(visitors[0].total_nodes, visitors[1].total_nodes);
    assert_eq!(visitors[1].total_nodes, visitors[2].total_nodes);
}

// ===================================================================
// 9. VisitorAction comparisons (additional)
// ===================================================================

#[test]
fn va_ne_is_symmetric_continue_skip() {
    assert_ne!(VisitorAction::Continue, VisitorAction::SkipChildren);
    assert_ne!(VisitorAction::SkipChildren, VisitorAction::Continue);
}

#[test]
fn va_ne_is_symmetric_continue_stop() {
    assert_ne!(VisitorAction::Continue, VisitorAction::Stop);
    assert_ne!(VisitorAction::Stop, VisitorAction::Continue);
}

#[test]
fn va_ne_is_symmetric_skip_stop() {
    assert_ne!(VisitorAction::SkipChildren, VisitorAction::Stop);
    assert_ne!(VisitorAction::Stop, VisitorAction::SkipChildren);
}

#[test]
fn va_eq_is_reflexive() {
    let a = VisitorAction::Continue;
    assert_eq!(a, a);
}

#[test]
fn va_eq_is_transitive() {
    let a = VisitorAction::Stop;
    let b = VisitorAction::Stop;
    let c = VisitorAction::Stop;
    assert_eq!(a, b);
    assert_eq!(b, c);
    assert_eq!(a, c);
}

// ===================================================================
// 10. PrettyPrintVisitor output method (additional)
// ===================================================================

#[test]
fn pp_output_returns_str_ref() {
    let v = PrettyPrintVisitor::new();
    let s: &str = v.output();
    assert_eq!(s, "");
}

#[test]
fn pp_output_grows_after_walk() {
    let (tree, src) = simple_tree();
    let walker = TreeWalker::new(&src);
    let mut v = PrettyPrintVisitor::new();
    let before = v.output().len();
    walker.walk(&tree, &mut v);
    let after = v.output().len();
    assert!(after > before);
}

#[test]
fn pp_output_contains_leaf_text() {
    let src = b"ab".to_vec();
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let walker = TreeWalker::new(&src);
    let mut v = PrettyPrintVisitor::new();
    walker.walk(&root, &mut v);
    // Leaf text "a" and "b" should appear in the pretty-print
    assert!(v.output().contains("\"a\""));
    assert!(v.output().contains("\"b\""));
}

#[test]
fn pp_output_has_indentation() {
    let src = b"ab".to_vec();
    let root = interior(10, vec![leaf(1, 0, 1)]);
    let walker = TreeWalker::new(&src);
    let mut v = PrettyPrintVisitor::new();
    walker.walk(&root, &mut v);
    // Child should be indented with spaces
    assert!(v.output().contains("  "));
}

#[test]
fn pp_output_deterministic() {
    let (tree, src) = simple_tree();
    let walker = TreeWalker::new(&src);

    let mut v1 = PrettyPrintVisitor::new();
    walker.walk(&tree, &mut v1);

    let mut v2 = PrettyPrintVisitor::new();
    walker.walk(&tree, &mut v2);

    assert_eq!(v1.output(), v2.output());
}

// ===================================================================
// Additional edge cases and coverage
// ===================================================================

#[test]
fn stats_walk_simple_tree_counts() {
    let (tree, src) = simple_tree();
    let walker = TreeWalker::new(&src);
    let mut v = StatsVisitor::default();
    walker.walk(&tree, &mut v);
    assert_eq!(v.total_nodes, 3); // root + 2 leaves
    assert_eq!(v.leaf_nodes, 2);
    assert_eq!(v.error_nodes, 0);
}

#[test]
fn stats_walk_depth() {
    let (tree, src) = simple_tree();
    let walker = TreeWalker::new(&src);
    let mut v = StatsVisitor::default();
    walker.walk(&tree, &mut v);
    assert_eq!(v.max_depth, 2); // root=1, leaves=2
}

#[test]
fn stats_node_counts_populated() {
    let (tree, src) = simple_tree();
    let walker = TreeWalker::new(&src);
    let mut v = StatsVisitor::default();
    walker.walk(&tree, &mut v);
    assert!(!v.node_counts.is_empty());
}

#[test]
fn stats_error_node_counted() {
    let src = b"e".to_vec();
    let root = interior(10, vec![error_node(0, 1)]);
    let walker = TreeWalker::new(&src);
    let mut v = StatsVisitor::default();
    walker.walk(&root, &mut v);
    assert_eq!(v.error_nodes, 1);
}

#[test]
fn search_walk_named_only() {
    let src = b"ab".to_vec();
    let root = interior(10, vec![leaf(1, 0, 1), unnamed_leaf(2, 1, 2)]);
    let walker = TreeWalker::new(&src);
    let mut v = SearchVisitor::new(|node: &ParsedNode| node.is_named());
    walker.walk(&root, &mut v);
    // root (named) + leaf(1) (named) = 2; unnamed_leaf is not named
    assert_eq!(v.matches.len(), 2);
}

#[test]
fn breadth_first_walker_creation() {
    let src = b"test";
    let _w = BreadthFirstWalker::new(src);
}

#[test]
fn tree_walker_creation() {
    let src = b"test";
    let _w = TreeWalker::new(src);
}

#[test]
fn pp_size_positive() {
    assert!(std::mem::size_of::<PrettyPrintVisitor>() > 0);
}

#[test]
fn stats_size_positive() {
    assert!(std::mem::size_of::<StatsVisitor>() > 0);
}

#[test]
fn va_option_some() {
    let opt: Option<VisitorAction> = Some(VisitorAction::Continue);
    assert!(opt.is_some());
}

#[test]
fn va_option_none() {
    let opt: Option<VisitorAction> = None;
    assert!(opt.is_none());
}

#[test]
fn va_in_result() {
    let action = VisitorAction::Stop;
    let res: Result<VisitorAction, ()> = Ok(action);
    assert_eq!(res, Ok(VisitorAction::Stop));
}

#[test]
fn stats_after_empty_tree() {
    let src = b"x".to_vec();
    let root = leaf(1, 0, 1); // single leaf, no children
    let walker = TreeWalker::new(&src);
    let mut v = StatsVisitor::default();
    walker.walk(&root, &mut v);
    assert_eq!(v.total_nodes, 1);
    assert_eq!(v.leaf_nodes, 1);
}

#[test]
fn search_closure_captures_variable() {
    let target_sym: u16 = 2;
    let _v = SearchVisitor::new(move |node: &ParsedNode| node.symbol() == target_sym);
}

#[test]
fn search_closure_with_string_capture() {
    let target_kind = "identifier".to_string();
    let _v = SearchVisitor::new(move |node: &ParsedNode| node.kind() == target_kind);
}

#[test]
fn pp_named_annotation() {
    let src = b"x".to_vec();
    let root = leaf(1, 0, 1); // is_named = true
    let walker = TreeWalker::new(&src);
    let mut v = PrettyPrintVisitor::new();
    walker.walk(&root, &mut v);
    assert!(v.output().contains("[named]"));
}

#[test]
fn pp_no_named_for_unnamed() {
    let src = b"x".to_vec();
    let root = unnamed_leaf(1, 0, 1);
    let walker = TreeWalker::new(&src);
    let mut v = PrettyPrintVisitor::new();
    walker.walk(&root, &mut v);
    // The leaf text line should not say [named]
    let lines: Vec<&str> = v.output().lines().collect();
    // First line is enter_node, second is visit_leaf text
    // The leaf line with "x" should not contain [named]
    let leaf_lines: Vec<&&str> = lines.iter().filter(|l| l.contains("\"x\"")).collect();
    for ll in leaf_lines {
        assert!(!ll.contains("[named]"));
    }
}

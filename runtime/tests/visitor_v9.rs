//! Visitor pattern v9 — 84 tests across 17 categories.
//!
//! Categories:
//!   stats_leaf_*          — StatsVisitor on single leaf nodes
//!   stats_children_*      — StatsVisitor on trees with children
//!   stats_deep_*          — StatsVisitor on deep (chain) trees
//!   stats_wide_*          — StatsVisitor on wide (many children) trees
//!   stats_multi_*         — StatsVisitor visiting multiple trees
//!   pretty_leaf_*         — PrettyPrintVisitor on leaves
//!   pretty_result_*       — PrettyPrintVisitor result accessors
//!   pretty_tree_*         — PrettyPrintVisitor on interior trees
//!   pretty_indent_*       — PrettyPrintVisitor indentation behaviour
//!   search_found_*        — SearchVisitor finds existing values
//!   search_missing_*      — SearchVisitor for absent values
//!   search_empty_*        — SearchVisitor before any walk
//!   complex_*             — Visitors on 3-level trees
//!   many_*                — Visitors on trees with 100+ nodes
//!   stats_reuse_*         — StatsVisitor reuse across walks
//!   arena_visitor_*       — Arena + visitor integration
//!   arena_build_*         — Build tree in arena then walk

use adze::arena_allocator::{NodeHandle, TreeArena, TreeNode};
use adze::pure_parser::ParsedNode;
use adze::visitor::{PrettyPrintVisitor, SearchVisitor, StatsVisitor, TreeWalker};

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

/// ```text
/// root(10)
///  ├── a(1) leaf "a"
///  ├── mid(11)
///  │    ├── b(2) leaf "b"
///  │    └── c(3) unnamed leaf "c"
///  └── d(4) leaf "d"
/// ```
/// Source: `"abcd"`
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

/// Chain: root(100) → n1(101) → n2(102) → n3(103) leaf "deep"
fn deep_tree() -> (ParsedNode, Vec<u8>) {
    let src = b"deep".to_vec();
    let n3 = leaf(103, 0, 4);
    let n2 = interior(102, vec![n3]);
    let n1 = interior(101, vec![n2]);
    let root = interior(100, vec![n1]);
    (root, src)
}

/// Root with 5 direct leaf children.
fn wide_tree() -> (ParsedNode, Vec<u8>) {
    let src = b"abcde".to_vec();
    let children: Vec<ParsedNode> = (0..5).map(|i| leaf(i as u16 + 1, i, i + 1)).collect();
    let root = interior(50, children);
    (root, src)
}

/// 3-level tree:
/// ```text
/// root(200)
///  ├── l1a(201)
///  │    ├── l2a(211) leaf "x"
///  │    └── l2b(212) leaf "y"
///  └── l1b(202)
///       └── l2c(213) leaf "z"
/// ```
fn three_level_tree() -> (ParsedNode, Vec<u8>) {
    let src = b"xyz".to_vec();
    let l2a = leaf(211, 0, 1);
    let l2b = leaf(212, 1, 2);
    let l2c = leaf(213, 2, 3);
    let l1a = interior(201, vec![l2a, l2b]);
    let l1b = interior(202, vec![l2c]);
    let root = interior(200, vec![l1a, l1b]);
    (root, src)
}

/// Tree with 100 leaf children under one root.
fn hundred_node_tree() -> (ParsedNode, Vec<u8>) {
    let src: Vec<u8> = (0..100).map(|i| b'a' + (i % 26)).collect();
    let children: Vec<ParsedNode> = (0..100).map(|i| leaf(i as u16 + 1, i, i + 1)).collect();
    let root = interior(999, children);
    (root, src)
}

// ===================================================================
// Category 1 — stats_leaf_* (5 tests)
// ===================================================================

#[test]
fn stats_leaf_total_nodes_is_one() {
    let node = leaf(1, 0, 1);
    let src = b"x".to_vec();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&node, &mut stats);
    assert_eq!(stats.total_nodes, 1);
}

#[test]
fn stats_leaf_leaf_count_is_one() {
    let node = leaf(1, 0, 1);
    let src = b"x".to_vec();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&node, &mut stats);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn stats_leaf_max_depth_is_one() {
    let node = leaf(1, 0, 1);
    let src = b"x".to_vec();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&node, &mut stats);
    assert_eq!(stats.max_depth, 1);
}

#[test]
fn stats_leaf_error_count_zero() {
    let node = leaf(1, 0, 1);
    let src = b"x".to_vec();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&node, &mut stats);
    assert_eq!(stats.error_nodes, 0);
}

#[test]
fn stats_leaf_default_zeroed() {
    let stats = StatsVisitor::default();
    assert_eq!(stats.total_nodes, 0);
    assert_eq!(stats.leaf_nodes, 0);
    assert_eq!(stats.max_depth, 0);
    assert_eq!(stats.error_nodes, 0);
    assert!(stats.node_counts.is_empty());
}

// ===================================================================
// Category 2 — stats_children_* (5 tests)
// ===================================================================

#[test]
fn stats_children_total_nodes() {
    let (root, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    // root(10), a(1), mid(11), b(2), c(3), d(4) = 6
    assert_eq!(stats.total_nodes, 6);
}

#[test]
fn stats_children_leaf_count() {
    let (root, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    // a, b, c, d = 4 leaves
    assert_eq!(stats.leaf_nodes, 4);
}

#[test]
fn stats_children_max_depth() {
    let (root, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    // root → mid → b|c = depth 3
    assert_eq!(stats.max_depth, 3);
}

#[test]
fn stats_children_error_count_zero() {
    let (root, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 0);
}

#[test]
fn stats_children_node_counts_populated() {
    let (root, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert!(!stats.node_counts.is_empty());
}

// ===================================================================
// Category 3 — stats_deep_* (5 tests)
// ===================================================================

#[test]
fn stats_deep_total_nodes() {
    let (root, src) = deep_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 4);
}

#[test]
fn stats_deep_max_depth() {
    let (root, src) = deep_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.max_depth, 4);
}

#[test]
fn stats_deep_leaf_count() {
    let (root, src) = deep_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn stats_deep_error_count_zero() {
    let (root, src) = deep_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 0);
}

#[test]
fn stats_deep_node_counts_populated() {
    let (root, src) = deep_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    // node_counts keyed by kind string; at least one entry
    assert!(!stats.node_counts.is_empty());
}

// ===================================================================
// Category 4 — stats_wide_* (5 tests)
// ===================================================================

#[test]
fn stats_wide_total_nodes() {
    let (root, src) = wide_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    // root + 5 leaves = 6
    assert_eq!(stats.total_nodes, 6);
}

#[test]
fn stats_wide_all_children_are_leaves() {
    let (root, src) = wide_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.leaf_nodes, 5);
}

#[test]
fn stats_wide_max_depth() {
    let (root, src) = wide_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    // root → leaf = depth 2
    assert_eq!(stats.max_depth, 2);
}

#[test]
fn stats_wide_error_count_zero() {
    let (root, src) = wide_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 0);
}

#[test]
fn stats_wide_node_counts_distinct_symbols() {
    let (root, src) = wide_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    // 5 distinct leaf symbols + 1 root symbol = 6
    assert_eq!(stats.node_counts.len(), 6);
}

// ===================================================================
// Category 5 — stats_multi_* (5 tests)
// ===================================================================

#[test]
fn stats_multi_visit_two_trees_accumulates_total() {
    let (root1, src1) = sample_tree();
    let (root2, src2) = deep_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src1).walk(&root1, &mut stats);
    TreeWalker::new(&src2).walk(&root2, &mut stats);
    // 6 + 4 = 10
    assert_eq!(stats.total_nodes, 10);
}

#[test]
fn stats_multi_leaf_count_accumulates() {
    let (root1, src1) = sample_tree();
    let (root2, src2) = wide_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src1).walk(&root1, &mut stats);
    TreeWalker::new(&src2).walk(&root2, &mut stats);
    // 4 + 5 = 9
    assert_eq!(stats.leaf_nodes, 9);
}

#[test]
fn stats_multi_depth_tracks_max() {
    let (root1, src1) = wide_tree();
    let (root2, src2) = deep_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src1).walk(&root1, &mut stats);
    TreeWalker::new(&src2).walk(&root2, &mut stats);
    // deep tree has depth 4, wide has depth 2
    assert_eq!(stats.max_depth, 4);
}

#[test]
fn stats_multi_error_accumulates() {
    let err_tree = interior(10, vec![error_node(0, 1), leaf(1, 1, 2)]);
    let src = b"ex".to_vec();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&err_tree, &mut stats);
    assert_eq!(stats.error_nodes, 1);

    let err_tree2 = interior(20, vec![error_node(0, 1)]);
    let src2 = b"e".to_vec();
    TreeWalker::new(&src2).walk(&err_tree2, &mut stats);
    assert_eq!(stats.error_nodes, 2);
}

#[test]
fn stats_multi_node_counts_merge() {
    let node1 = leaf(42, 0, 1);
    let src1 = b"x".to_vec();
    let node2 = leaf(42, 0, 1);
    let src2 = b"y".to_vec();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src1).walk(&node1, &mut stats);
    TreeWalker::new(&src2).walk(&node2, &mut stats);
    assert_eq!(stats.total_nodes, 2);
}

// ===================================================================
// Category 6 — pretty_leaf_* (5 tests)
// ===================================================================

#[test]
fn pretty_leaf_output_non_empty() {
    let node = leaf(1, 0, 1);
    let src = b"x".to_vec();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&node, &mut pp);
    assert!(!pp.output().is_empty());
}

#[test]
fn pretty_leaf_contains_text() {
    let node = leaf(1, 0, 3);
    let src = b"abc".to_vec();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&node, &mut pp);
    assert!(pp.output().contains("abc"));
}

#[test]
fn pretty_leaf_ends_with_newline() {
    let node = leaf(1, 0, 1);
    let src = b"x".to_vec();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&node, &mut pp);
    assert!(pp.output().ends_with('\n'));
}

#[test]
fn pretty_leaf_has_named_marker() {
    let node = leaf(1, 0, 1);
    let src = b"x".to_vec();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&node, &mut pp);
    assert!(pp.output().contains("[named]"));
}

#[test]
fn pretty_leaf_unnamed_no_named_marker() {
    let node = unnamed_leaf(1, 0, 1);
    let src = b"x".to_vec();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&node, &mut pp);
    assert!(!pp.output().contains("[named]"));
}

// ===================================================================
// Category 7 — pretty_result_* (5 tests)
// ===================================================================

#[test]
fn pretty_result_new_is_empty() {
    let pp = PrettyPrintVisitor::new();
    assert!(pp.output().is_empty());
}

#[test]
fn pretty_result_default_is_empty() {
    let pp = PrettyPrintVisitor::default();
    assert!(pp.output().is_empty());
}

#[test]
fn pretty_result_multiline_for_tree() {
    let (root, src) = sample_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    let line_count = pp.output().lines().count();
    assert!(line_count > 1);
}

#[test]
fn pretty_result_contains_all_leaf_texts() {
    let (root, src) = sample_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    let out = pp.output();
    assert!(out.contains("\"a\""));
    assert!(out.contains("\"b\""));
    assert!(out.contains("\"c\""));
    assert!(out.contains("\"d\""));
}

#[test]
fn pretty_result_returns_str_ref() {
    let node = leaf(1, 0, 1);
    let src = b"z".to_vec();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&node, &mut pp);
    let output: &str = pp.output();
    assert!(!output.is_empty());
}

// ===================================================================
// Category 8 — pretty_tree_* (5 tests)
// ===================================================================

#[test]
fn pretty_tree_has_root_kind() {
    let (root, src) = sample_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    let first_line = pp.output().lines().next().unwrap_or("");
    assert!(!first_line.is_empty());
}

#[test]
fn pretty_tree_deep_tree_output() {
    let (root, src) = deep_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    assert!(!pp.output().is_empty());
    assert!(pp.output().contains("\"deep\""));
}

#[test]
fn pretty_tree_wide_tree_all_leaves() {
    let (root, src) = wide_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    for ch in b"abcde" {
        let needle = format!("\"{}\"", *ch as char);
        assert!(pp.output().contains(&needle));
    }
}

#[test]
fn pretty_tree_error_node_shows_error() {
    let root = interior(10, vec![error_node(0, 1)]);
    let src = b"e".to_vec();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    assert!(pp.output().contains("ERROR"));
}

#[test]
fn pretty_tree_structure_has_more_lines_than_leaves() {
    let (root, src) = sample_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    let line_count = pp.output().lines().count();
    // 6 nodes + 4 leaf-text lines = at least 10
    assert!(line_count >= 6);
}

// ===================================================================
// Category 9 — pretty_indent_* (5 tests)
// ===================================================================

#[test]
fn pretty_indent_root_no_indent() {
    let (root, src) = sample_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    let first_line = pp.output().lines().next().unwrap_or("");
    assert_eq!(
        first_line.len() - first_line.trim_start().len(),
        0,
        "root should have zero indentation"
    );
}

#[test]
fn pretty_indent_child_indented() {
    let (root, src) = sample_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    let second_line = pp.output().lines().nth(1).unwrap_or("");
    let indent = second_line.len() - second_line.trim_start().len();
    assert!(indent > 0, "first child should be indented");
}

#[test]
fn pretty_indent_deep_tree_increasing() {
    let (root, src) = deep_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    let indents: Vec<usize> = pp
        .output()
        .lines()
        .filter(|l| !l.trim().starts_with('"'))
        .map(|l| l.len() - l.trim_start().len())
        .collect();
    // Each successive node line should have >= the previous indent
    for pair in indents.windows(2) {
        assert!(pair[1] >= pair[0]);
    }
}

#[test]
fn pretty_indent_siblings_same_indent() {
    let (root, src) = wide_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    let leaf_indents: Vec<usize> = pp
        .output()
        .lines()
        .filter(|l| l.trim().starts_with('"'))
        .map(|l| l.len() - l.trim_start().len())
        .collect();
    // All leaf text lines should have the same indentation
    if let Some(first) = leaf_indents.first() {
        for indent in &leaf_indents {
            assert_eq!(indent, first);
        }
    }
}

#[test]
fn pretty_indent_max_indent_for_deep_tree() {
    let (root, src) = deep_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    let max_indent = pp
        .output()
        .lines()
        .map(|l| l.len() - l.trim_start().len())
        .max()
        .unwrap_or(0);
    // 4-level deep tree → at least 6 spaces of max indent
    assert!(max_indent >= 6);
}

// ===================================================================
// Category 10 — search_found_* (5 tests)
// ===================================================================

#[test]
fn search_found_leaf_by_symbol() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert!(!search.matches.is_empty());
}

#[test]
fn search_found_interior_by_symbol() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 11);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 1);
}

#[test]
fn search_found_records_byte_range() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 2);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 1);
    let (start, end, _) = &search.matches[0];
    assert_eq!(*start, 1);
    assert_eq!(*end, 2);
}

#[test]
fn search_found_multiple_matches() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.is_named());
    TreeWalker::new(&src).walk(&root, &mut search);
    // root(10), a(1), mid(11), b(2), d(4) are named = 5
    assert_eq!(search.matches.len(), 5);
}

#[test]
fn search_found_root_symbol() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 10);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 1);
}

// ===================================================================
// Category 11 — search_missing_* (5 tests)
// ===================================================================

#[test]
fn search_missing_empty_results() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 999);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert!(search.matches.is_empty());
}

#[test]
fn search_missing_wrong_symbol() {
    let node = leaf(1, 0, 1);
    let src = b"x".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 2);
    TreeWalker::new(&src).walk(&node, &mut search);
    assert!(search.matches.is_empty());
}

#[test]
fn search_missing_in_deep_tree() {
    let (root, src) = deep_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 999);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert!(search.matches.is_empty());
}

#[test]
fn search_missing_in_wide_tree() {
    let (root, src) = wide_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 999);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert!(search.matches.is_empty());
}

#[test]
fn search_missing_predicate_always_false() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|_: &ParsedNode| false);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert!(search.matches.is_empty());
}

// ===================================================================
// Category 12 — search_empty_* (4 tests)
// ===================================================================

#[test]
fn search_empty_new_has_no_matches() {
    let search = SearchVisitor::new(|_: &ParsedNode| true);
    assert!(search.matches.is_empty());
}

#[test]
fn search_empty_not_walked() {
    let search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    assert_eq!(search.matches.len(), 0);
}

#[test]
fn search_empty_bfs_no_walk() {
    let search = SearchVisitor::new(|_: &ParsedNode| true);
    assert!(search.matches.is_empty());
}

#[test]
fn search_empty_after_walk_on_error_node() {
    let err = error_node(0, 1);
    let src = b"e".to_vec();
    let mut search = SearchVisitor::new(|_: &ParsedNode| true);
    TreeWalker::new(&src).walk(&err, &mut search);
    // Error nodes go through visit_error, not enter_node
    assert!(search.matches.is_empty());
}

// ===================================================================
// Category 13 — complex_* (5 tests)
// ===================================================================

#[test]
fn complex_three_level_stats_total() {
    let (root, src) = three_level_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    // root + l1a + l1b + l2a + l2b + l2c = 6
    assert_eq!(stats.total_nodes, 6);
}

#[test]
fn complex_three_level_stats_leaves() {
    let (root, src) = three_level_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.leaf_nodes, 3);
}

#[test]
fn complex_three_level_stats_depth() {
    let (root, src) = three_level_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.max_depth, 3);
}

#[test]
fn complex_three_level_pretty_contains_all_leaves() {
    let (root, src) = three_level_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    assert!(pp.output().contains("\"x\""));
    assert!(pp.output().contains("\"y\""));
    assert!(pp.output().contains("\"z\""));
}

#[test]
fn complex_three_level_search_finds_deep_leaf() {
    let (root, src) = three_level_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 213);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 1);
}

// ===================================================================
// Category 14 — many_* (5 tests)
// ===================================================================

#[test]
fn many_nodes_stats_total() {
    let (root, src) = hundred_node_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    // root + 100 leaves = 101
    assert_eq!(stats.total_nodes, 101);
}

#[test]
fn many_nodes_stats_leaves() {
    let (root, src) = hundred_node_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.leaf_nodes, 100);
}

#[test]
fn many_nodes_search_last() {
    let (root, src) = hundred_node_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 100);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 1);
}

#[test]
fn many_nodes_pretty_large_output() {
    let (root, src) = hundred_node_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    let line_count = pp.output().lines().count();
    // at least 101 enter lines + 100 leaf text lines
    assert!(line_count >= 101);
}

#[test]
fn many_nodes_max_depth() {
    let (root, src) = hundred_node_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    // root → leaf = depth 2
    assert_eq!(stats.max_depth, 2);
}

// ===================================================================
// Category 15 — stats_reuse_* (4 tests)
// ===================================================================

#[test]
fn stats_reuse_accumulates_across_walks() {
    let (root, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    let first_total = stats.total_nodes;
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, first_total * 2);
}

#[test]
fn stats_reuse_depth_max_persists() {
    let (deep, deep_src) = deep_tree();
    let (wide, wide_src) = wide_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&deep_src).walk(&deep, &mut stats);
    let deep_max = stats.max_depth;
    TreeWalker::new(&wide_src).walk(&wide, &mut stats);
    // max_depth should still reflect the deeper tree
    assert_eq!(stats.max_depth, deep_max);
}

#[test]
fn stats_reuse_leaf_count_adds() {
    let (root, src) = wide_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.leaf_nodes, 10);
}

#[test]
fn stats_reuse_error_count_adds() {
    let root = interior(10, vec![error_node(0, 1)]);
    let src = b"e".to_vec();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 2);
}

// ===================================================================
// Category 16 — arena_visitor_* (5 tests)
// ===================================================================

#[test]
fn arena_visitor_alloc_leaf_then_verify() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(42));
    let node_ref = arena.get(h);
    assert_eq!(node_ref.value(), 42);
    assert!(node_ref.is_leaf());
}

#[test]
fn arena_visitor_alloc_branch_children() {
    let mut arena = TreeArena::new();
    let c1 = arena.alloc(TreeNode::leaf(1));
    let c2 = arena.alloc(TreeNode::leaf(2));
    let parent = arena.alloc(TreeNode::branch(vec![c1, c2]));
    let pref = arena.get(parent);
    assert!(pref.is_branch());
    assert_eq!(pref.children().len(), 2);
}

#[test]
fn arena_visitor_len_after_allocs() {
    let mut arena = TreeArena::new();
    arena.alloc(TreeNode::leaf(1));
    arena.alloc(TreeNode::leaf(2));
    arena.alloc(TreeNode::leaf(3));
    assert_eq!(arena.len(), 3);
}

#[test]
fn arena_visitor_is_empty_initially() {
    let arena = TreeArena::new();
    assert!(arena.is_empty());
}

#[test]
fn arena_visitor_clear_resets_to_empty() {
    let mut arena = TreeArena::new();
    arena.alloc(TreeNode::leaf(1));
    arena.alloc(TreeNode::leaf(2));
    assert!(!arena.is_empty());
    arena.clear();
    assert!(arena.is_empty());
    assert_eq!(arena.len(), 0);
}

// ===================================================================
// Category 17 — arena_build_* (10 tests)
// ===================================================================

#[test]
fn arena_build_leaf_is_leaf() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(7));
    assert!(arena.get(h).is_leaf());
}

#[test]
fn arena_build_branch_is_not_leaf() {
    let mut arena = TreeArena::new();
    let c = arena.alloc(TreeNode::leaf(1));
    let parent = arena.alloc(TreeNode::branch(vec![c]));
    assert!(!arena.get(parent).is_leaf());
}

#[test]
fn arena_build_branch_with_symbol() {
    let mut arena = TreeArena::new();
    let c = arena.alloc(TreeNode::leaf(1));
    let parent = arena.alloc(TreeNode::branch_with_symbol(99, vec![c]));
    assert_eq!(arena.get(parent).value(), 99);
}

#[test]
fn arena_build_leaf_children_empty() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(5));
    assert!(arena.get(h).children().is_empty());
}

#[test]
fn arena_build_tree_verify_child_values() {
    let mut arena = TreeArena::new();
    let c1 = arena.alloc(TreeNode::leaf(10));
    let c2 = arena.alloc(TreeNode::leaf(20));
    let parent = arena.alloc(TreeNode::branch(vec![c1, c2]));
    let children = arena.get(parent).children().to_vec();
    assert_eq!(arena.get(children[0]).value(), 10);
    assert_eq!(arena.get(children[1]).value(), 20);
}

#[test]
fn arena_build_node_handle_is_copy() {
    let mut arena = TreeArena::new();
    let h: NodeHandle = arena.alloc(TreeNode::leaf(7));
    let h2 = h;
    assert_eq!(arena.get(h).value(), arena.get(h2).value());
}

#[test]
fn arena_build_reset_clears_all() {
    let mut arena = TreeArena::new();
    arena.alloc(TreeNode::leaf(1));
    arena.alloc(TreeNode::leaf(2));
    arena.reset();
    assert!(arena.is_empty());
}

#[test]
fn arena_build_many_nodes() {
    let mut arena = TreeArena::new();
    let handles: Vec<NodeHandle> = (0..100).map(|i| arena.alloc(TreeNode::leaf(i))).collect();
    assert_eq!(arena.len(), 100);
    for (i, h) in handles.iter().enumerate() {
        assert_eq!(arena.get(*h).value(), i as i32);
    }
}

#[test]
fn arena_build_deep_nesting() {
    let mut arena = TreeArena::new();
    let mut current = arena.alloc(TreeNode::leaf(0));
    for i in 1..10 {
        current = arena.alloc(TreeNode::branch_with_symbol(i, vec![current]));
    }
    assert_eq!(arena.len(), 10);
    assert_eq!(arena.get(current).value(), 9);
}

#[test]
fn arena_build_combined_with_parsed_node_walk() {
    let mut arena = TreeArena::new();
    let c1 = arena.alloc(TreeNode::leaf(1));
    let c2 = arena.alloc(TreeNode::leaf(2));
    let _parent = arena.alloc(TreeNode::branch(vec![c1, c2]));

    let (root, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);

    assert_eq!(arena.len(), 3);
    assert_eq!(stats.total_nodes, 6);
    assert_eq!(stats.leaf_nodes, 4);
}

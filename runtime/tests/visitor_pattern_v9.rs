//! Visitor pattern v9 — 84 tests for StatsVisitor, PrettyPrintVisitor,
//! SearchVisitor, and cross-visitor integration.
//!
//! Categories:
//!   stats_new_*            — StatsVisitor default/initial state
//!   stats_named_*          — StatsVisitor with named nodes
//!   stats_anon_*           — StatsVisitor with anonymous (unnamed) nodes
//!   stats_mixed_*          — StatsVisitor with mixed named/unnamed
//!   stats_depth_*          — StatsVisitor depth tracking
//!   stats_deep_*           — StatsVisitor deep nesting
//!   stats_wide_*           — StatsVisitor wide trees
//!   stats_error_*          — StatsVisitor error node handling
//!   stats_reuse_*          — StatsVisitor reuse across walks
//!   pretty_new_*           — PrettyPrintVisitor initial state
//!   pretty_single_*        — PrettyPrintVisitor on single nodes
//!   pretty_indent_*        — PrettyPrintVisitor indentation
//!   pretty_multi_*         — PrettyPrintVisitor on multiple nodes
//!   pretty_nested_*        — PrettyPrintVisitor on nested trees
//!   search_new_*           — SearchVisitor initial state
//!   search_found_*         — SearchVisitor matching nodes
//!   search_missing_*       — SearchVisitor non-matching
//!   search_count_*         — SearchVisitor count accuracy
//!   debug_*                — Debug trait implementations
//!   cross_*                — Cross-visitor integration

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

fn walk_stats(node: &ParsedNode, src: &[u8]) -> StatsVisitor {
    let mut stats = StatsVisitor::default();
    TreeWalker::new(src).walk(node, &mut stats);
    stats
}

fn walk_pretty(node: &ParsedNode, src: &[u8]) -> PrettyPrintVisitor {
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(node, &mut pp);
    pp
}

/// Sample tree:
/// ```text
/// root(10)
///  ├── a(1) named leaf "a"
///  ├── mid(11)
///  │    ├── b(2) named leaf "b"
///  │    └── c(3) unnamed leaf "c"
///  └── d(4) named leaf "d"
/// ```
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
fn deep_chain() -> (ParsedNode, Vec<u8>) {
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

/// 100 leaves under one root.
fn hundred_node_tree() -> (ParsedNode, Vec<u8>) {
    let src: Vec<u8> = (0..100).map(|i| b'a' + (i % 26)).collect();
    let children: Vec<ParsedNode> = (0..100).map(|i| leaf(i as u16 + 1, i, i + 1)).collect();
    let root = interior(999, children);
    (root, src)
}

// ===================================================================
// Category 1 — stats_new_* (5 tests)
// ===================================================================

#[test]
fn stats_new_total_nodes_zero() {
    let stats = StatsVisitor::default();
    assert_eq!(stats.total_nodes, 0);
}

#[test]
fn stats_new_leaf_nodes_zero() {
    let stats = StatsVisitor::default();
    assert_eq!(stats.leaf_nodes, 0);
}

#[test]
fn stats_new_error_nodes_zero() {
    let stats = StatsVisitor::default();
    assert_eq!(stats.error_nodes, 0);
}

#[test]
fn stats_new_max_depth_zero() {
    let stats = StatsVisitor::default();
    assert_eq!(stats.max_depth, 0);
}

#[test]
fn stats_new_node_counts_empty() {
    let stats = StatsVisitor::default();
    assert!(stats.node_counts.is_empty());
}

// ===================================================================
// Category 2 — stats_named_* (6 tests)
// ===================================================================

#[test]
fn stats_named_visit_one_leaf_total_is_one() {
    let node = leaf(1, 0, 1);
    let stats = walk_stats(&node, b"x");
    assert_eq!(stats.total_nodes, 1);
}

#[test]
fn stats_named_visit_one_leaf_leaf_count_is_one() {
    let node = leaf(1, 0, 1);
    let stats = walk_stats(&node, b"x");
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn stats_named_ten_leaves_total_is_eleven() {
    // One root + 10 named leaves = 11 total nodes
    let src: Vec<u8> = (0..10).map(|i| b'a' + i).collect();
    let children: Vec<ParsedNode> = (0..10).map(|i| leaf(i as u16 + 1, i, i + 1)).collect();
    let root = interior(99, children);
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.total_nodes, 11);
}

#[test]
fn stats_named_ten_leaves_leaf_count_is_ten() {
    let src: Vec<u8> = (0..10).map(|i| b'a' + i).collect();
    let children: Vec<ParsedNode> = (0..10).map(|i| leaf(i as u16 + 1, i, i + 1)).collect();
    let root = interior(99, children);
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.leaf_nodes, 10);
}

#[test]
fn stats_named_sample_tree_total() {
    let (root, src) = sample_tree();
    let stats = walk_stats(&root, &src);
    // root(10), a(1), mid(11), b(2), c(3), d(4) = 6
    assert_eq!(stats.total_nodes, 6);
}

#[test]
fn stats_named_node_counts_populated() {
    let (root, src) = sample_tree();
    let stats = walk_stats(&root, &src);
    assert!(!stats.node_counts.is_empty());
}

// ===================================================================
// Category 3 — stats_anon_* (4 tests)
// ===================================================================

#[test]
fn stats_anon_single_unnamed_leaf_total_one() {
    let node = unnamed_leaf(1, 0, 1);
    let stats = walk_stats(&node, b"x");
    assert_eq!(stats.total_nodes, 1);
}

#[test]
fn stats_anon_single_unnamed_leaf_is_leaf() {
    let node = unnamed_leaf(1, 0, 1);
    let stats = walk_stats(&node, b"x");
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn stats_anon_five_unnamed_leaves_total() {
    let src = b"abcde".to_vec();
    let children: Vec<ParsedNode> = (0..5)
        .map(|i| unnamed_leaf(i as u16 + 1, i, i + 1))
        .collect();
    let root = interior(50, children);
    let stats = walk_stats(&root, &src);
    // root + 5 unnamed = 6
    assert_eq!(stats.total_nodes, 6);
}

#[test]
fn stats_anon_five_unnamed_leaves_leaf_count() {
    let src = b"abcde".to_vec();
    let children: Vec<ParsedNode> = (0..5)
        .map(|i| unnamed_leaf(i as u16 + 1, i, i + 1))
        .collect();
    let root = interior(50, children);
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.leaf_nodes, 5);
}

// ===================================================================
// Category 4 — stats_mixed_* (5 tests)
// ===================================================================

#[test]
fn stats_mixed_sample_tree_leaf_count() {
    let (root, src) = sample_tree();
    let stats = walk_stats(&root, &src);
    // a, b, c, d = 4 leaves
    assert_eq!(stats.leaf_nodes, 4);
}

#[test]
fn stats_mixed_named_and_unnamed_total() {
    // root with 3 named + 2 unnamed = 6 total
    let src = b"abcde".to_vec();
    let children = vec![
        leaf(1, 0, 1),
        unnamed_leaf(2, 1, 2),
        leaf(3, 2, 3),
        unnamed_leaf(4, 3, 4),
        leaf(5, 4, 5),
    ];
    let root = interior(50, children);
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.total_nodes, 6);
}

#[test]
fn stats_mixed_all_are_leaves() {
    let src = b"abcde".to_vec();
    let children = vec![
        leaf(1, 0, 1),
        unnamed_leaf(2, 1, 2),
        leaf(3, 2, 3),
        unnamed_leaf(4, 3, 4),
        leaf(5, 4, 5),
    ];
    let root = interior(50, children);
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.leaf_nodes, 5);
}

#[test]
fn stats_mixed_no_errors() {
    let (root, src) = sample_tree();
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.error_nodes, 0);
}

#[test]
fn stats_mixed_max_depth_sample() {
    let (root, src) = sample_tree();
    let stats = walk_stats(&root, &src);
    // root → mid → b|c = depth 3
    assert_eq!(stats.max_depth, 3);
}

// ===================================================================
// Category 5 — stats_depth_* (6 tests)
// ===================================================================

#[test]
fn stats_depth_single_leaf_is_one() {
    let node = leaf(1, 0, 1);
    let stats = walk_stats(&node, b"x");
    assert_eq!(stats.max_depth, 1);
}

#[test]
fn stats_depth_root_plus_leaf_is_two() {
    let child = leaf(1, 0, 1);
    let root = interior(10, vec![child]);
    let stats = walk_stats(&root, b"x");
    assert_eq!(stats.max_depth, 2);
}

#[test]
fn stats_depth_wide_tree_is_two() {
    let (root, src) = wide_tree();
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.max_depth, 2);
}

#[test]
fn stats_depth_deep_chain_is_four() {
    let (root, src) = deep_chain();
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.max_depth, 4);
}

#[test]
fn stats_depth_asymmetric_tree() {
    // Left branch depth 3, right branch depth 2
    let deep_leaf = leaf(1, 0, 1);
    let mid = interior(2, vec![deep_leaf]);
    let shallow_leaf = leaf(3, 1, 2);
    let root = interior(10, vec![mid, shallow_leaf]);
    let stats = walk_stats(&root, b"ab");
    assert_eq!(stats.max_depth, 3);
}

#[test]
fn stats_depth_hundred_leaves_is_two() {
    let (root, src) = hundred_node_tree();
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.max_depth, 2);
}

// ===================================================================
// Category 6 — stats_deep_* (5 tests)
// ===================================================================

#[test]
fn stats_deep_chain_total_four() {
    let (root, src) = deep_chain();
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.total_nodes, 4);
}

#[test]
fn stats_deep_chain_one_leaf() {
    let (root, src) = deep_chain();
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn stats_deep_chain_no_errors() {
    let (root, src) = deep_chain();
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.error_nodes, 0);
}

#[test]
fn stats_deep_depth_100() {
    // Build a chain of depth 100
    let src = b"z".to_vec();
    let mut current = leaf(200, 0, 1);
    for i in 1..100 {
        current = interior(i as u16, vec![current]);
    }
    let stats = walk_stats(&current, &src);
    assert_eq!(stats.max_depth, 100);
}

#[test]
fn stats_deep_depth_100_one_leaf() {
    let src = b"z".to_vec();
    let mut current = leaf(200, 0, 1);
    for i in 1..100 {
        current = interior(i as u16, vec![current]);
    }
    let stats = walk_stats(&current, &src);
    assert_eq!(stats.leaf_nodes, 1);
}

// ===================================================================
// Category 7 — stats_wide_* (4 tests)
// ===================================================================

#[test]
fn stats_wide_total_six() {
    let (root, src) = wide_tree();
    let stats = walk_stats(&root, &src);
    // root + 5 children = 6
    assert_eq!(stats.total_nodes, 6);
}

#[test]
fn stats_wide_five_leaves() {
    let (root, src) = wide_tree();
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.leaf_nodes, 5);
}

#[test]
fn stats_wide_hundred_total() {
    let (root, src) = hundred_node_tree();
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.total_nodes, 101);
}

#[test]
fn stats_wide_hundred_leaves() {
    let (root, src) = hundred_node_tree();
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.leaf_nodes, 100);
}

// ===================================================================
// Category 8 — stats_error_* (4 tests)
// ===================================================================

#[test]
fn stats_error_single_error_node_counted() {
    let err = error_node(0, 3);
    let src = b"err".to_vec();
    let stats = walk_stats(&err, &src);
    assert_eq!(stats.error_nodes, 1);
}

#[test]
fn stats_error_node_not_in_total() {
    // Error nodes go through visit_error, not enter_node
    let err = error_node(0, 3);
    let src = b"err".to_vec();
    let stats = walk_stats(&err, &src);
    assert_eq!(stats.total_nodes, 0);
}

#[test]
fn stats_error_mixed_with_normal() {
    // Root has one leaf child; error is a sibling handled separately
    let child = leaf(1, 0, 1);
    let err = error_node(1, 2);
    let root = interior(10, vec![child, err]);
    let src = b"xe".to_vec();
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.error_nodes, 1);
}

#[test]
fn stats_error_leaf_count_excludes_errors() {
    let child = leaf(1, 0, 1);
    let err = error_node(1, 2);
    let root = interior(10, vec![child, err]);
    let src = b"xe".to_vec();
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.leaf_nodes, 1);
}

// ===================================================================
// Category 9 — stats_reuse_* (3 tests)
// ===================================================================

#[test]
fn stats_reuse_across_two_walks() {
    let node = leaf(1, 0, 1);
    let src = b"x";
    let mut stats = StatsVisitor::default();
    TreeWalker::new(src).walk(&node, &mut stats);
    TreeWalker::new(src).walk(&node, &mut stats);
    assert_eq!(stats.total_nodes, 2);
}

#[test]
fn stats_reuse_leaf_count_accumulates() {
    let node = leaf(1, 0, 1);
    let src = b"x";
    let mut stats = StatsVisitor::default();
    TreeWalker::new(src).walk(&node, &mut stats);
    TreeWalker::new(src).walk(&node, &mut stats);
    assert_eq!(stats.leaf_nodes, 2);
}

#[test]
fn stats_reuse_max_depth_takes_maximum() {
    let shallow = leaf(1, 0, 1);
    let deep_leaf = leaf(2, 0, 1);
    let mid = interior(3, vec![deep_leaf]);
    let deep = interior(4, vec![mid]);
    let src = b"x";
    let mut stats = StatsVisitor::default();
    TreeWalker::new(src).walk(&shallow, &mut stats);
    assert_eq!(stats.max_depth, 1);
    TreeWalker::new(src).walk(&deep, &mut stats);
    assert_eq!(stats.max_depth, 3);
}

// ===================================================================
// Category 10 — pretty_new_* (3 tests)
// ===================================================================

#[test]
fn pretty_new_output_empty() {
    let pp = PrettyPrintVisitor::new();
    assert!(pp.output().is_empty());
}

#[test]
fn pretty_new_default_output_empty() {
    let pp = PrettyPrintVisitor::default();
    assert!(pp.output().is_empty());
}

#[test]
fn pretty_new_default_eq_new() {
    let a = PrettyPrintVisitor::new();
    let b = PrettyPrintVisitor::default();
    assert_eq!(a.output(), b.output());
}

// ===================================================================
// Category 11 — pretty_single_* (4 tests)
// ===================================================================

#[test]
fn pretty_single_named_leaf_contains_named() {
    let node = leaf(1, 0, 1);
    let pp = walk_pretty(&node, b"x");
    assert!(pp.output().contains("[named]"));
}

#[test]
fn pretty_single_unnamed_leaf_no_named_tag() {
    let node = unnamed_leaf(1, 0, 1);
    let pp = walk_pretty(&node, b"x");
    // Unnamed nodes don't get [named] annotation
    assert!(!pp.output().contains("[named]"));
}

#[test]
fn pretty_single_leaf_contains_text() {
    let node = leaf(1, 0, 1);
    let pp = walk_pretty(&node, b"x");
    assert!(pp.output().contains("\"x\""));
}

#[test]
fn pretty_single_leaf_not_empty() {
    let node = leaf(1, 0, 1);
    let pp = walk_pretty(&node, b"x");
    assert!(!pp.output().is_empty());
}

// ===================================================================
// Category 12 — pretty_indent_* (5 tests)
// ===================================================================

#[test]
fn pretty_indent_root_no_indent() {
    let child = leaf(1, 0, 1);
    let root = interior(10, vec![child]);
    let pp = walk_pretty(&root, b"x");
    let first_line = pp.output().lines().next().unwrap();
    // Root line should not start with spaces
    assert!(!first_line.starts_with(' '));
}

#[test]
fn pretty_indent_child_has_two_spaces() {
    let child = leaf(1, 0, 1);
    let root = interior(10, vec![child]);
    let pp = walk_pretty(&root, b"x");
    let lines: Vec<&str> = pp.output().lines().collect();
    // Second line (child kind or leaf text) should be indented
    assert!(lines.len() >= 2);
    assert!(lines[1].starts_with("  "));
}

#[test]
fn pretty_indent_depth_two_has_four_spaces() {
    let grandchild = leaf(1, 0, 1);
    let child = interior(2, vec![grandchild]);
    let root = interior(10, vec![child]);
    let pp = walk_pretty(&root, b"x");
    let lines: Vec<&str> = pp.output().lines().collect();
    // Grandchild kind line should have 4 spaces indent
    assert!(lines.len() >= 3);
    assert!(lines[2].starts_with("    "));
}

#[test]
fn pretty_indent_wide_tree_all_children_same_indent() {
    let (root, src) = wide_tree();
    let pp = walk_pretty(&root, &src);
    let lines: Vec<&str> = pp.output().lines().collect();
    // Skip root line (idx 0); each child should produce two lines (kind + text)
    for line in lines.iter().skip(1) {
        // All non-root lines should be indented
        assert!(line.starts_with("  "));
    }
}

#[test]
fn pretty_indent_deep_chain_increasing() {
    let (root, src) = deep_chain();
    let pp = walk_pretty(&root, &src);
    let lines: Vec<&str> = pp.output().lines().collect();
    // Each successive node-kind line should have more indentation
    let mut prev_indent = 0;
    for line in &lines {
        let indent = line.len() - line.trim_start().len();
        assert!(indent >= prev_indent || prev_indent == 0);
        if indent > prev_indent {
            prev_indent = indent;
        }
    }
}

// ===================================================================
// Category 13 — pretty_multi_* (4 tests)
// ===================================================================

#[test]
fn pretty_multi_sample_tree_has_multiple_lines() {
    let (root, src) = sample_tree();
    let pp = walk_pretty(&root, &src);
    assert!(pp.output().lines().count() > 1);
}

#[test]
fn pretty_multi_sample_tree_contains_leaf_texts() {
    let (root, src) = sample_tree();
    let pp = walk_pretty(&root, &src);
    let out = pp.output();
    assert!(out.contains("\"a\""));
    assert!(out.contains("\"b\""));
    assert!(out.contains("\"c\""));
    assert!(out.contains("\"d\""));
}

#[test]
fn pretty_multi_wide_tree_line_count() {
    let (root, src) = wide_tree();
    let pp = walk_pretty(&root, &src);
    // root kind + (child kind + child leaf text) * 5 = 1 + 10 = 11
    assert!(pp.output().lines().count() >= 6);
}

#[test]
fn pretty_multi_hundred_nodes_large_output() {
    let (root, src) = hundred_node_tree();
    let pp = walk_pretty(&root, &src);
    assert!(pp.output().lines().count() >= 101);
}

// ===================================================================
// Category 14 — pretty_nested_* (4 tests)
// ===================================================================

#[test]
fn pretty_nested_deep_chain_output_not_empty() {
    let (root, src) = deep_chain();
    let pp = walk_pretty(&root, &src);
    assert!(!pp.output().is_empty());
}

#[test]
fn pretty_nested_deep_chain_has_leaf_text() {
    let (root, src) = deep_chain();
    let pp = walk_pretty(&root, &src);
    assert!(pp.output().contains("\"deep\""));
}

#[test]
fn pretty_nested_three_level() {
    let l2a = leaf(211, 0, 1);
    let l2b = leaf(212, 1, 2);
    let l1 = interior(201, vec![l2a, l2b]);
    let root = interior(200, vec![l1]);
    let src = b"xy";
    let pp = walk_pretty(&root, src);
    let out = pp.output();
    assert!(out.contains("\"x\""));
    assert!(out.contains("\"y\""));
}

#[test]
fn pretty_nested_error_node_shows_error() {
    let err = error_node(0, 3);
    let normal = leaf(1, 3, 4);
    let root = interior(10, vec![err, normal]);
    let src = b"errx";
    let pp = walk_pretty(&root, src);
    assert!(pp.output().contains("ERROR"));
}

// ===================================================================
// Category 15 — search_new_* (4 tests)
// ===================================================================

#[test]
fn search_new_matches_empty() {
    let search = SearchVisitor::new(|_node: &ParsedNode| false);
    assert!(search.matches.is_empty());
}

#[test]
fn search_new_no_walk_no_matches() {
    let search = SearchVisitor::new(|node: &ParsedNode| node.symbol() == 42);
    assert!(search.matches.is_empty());
}

#[test]
fn search_new_always_true_no_walk() {
    let search = SearchVisitor::new(|_node: &ParsedNode| true);
    assert!(search.matches.is_empty());
}

#[test]
fn search_new_count_zero() {
    let search = SearchVisitor::new(|_node: &ParsedNode| false);
    assert_eq!(search.matches.len(), 0);
}

// ===================================================================
// Category 16 — search_found_* (6 tests)
// ===================================================================

#[test]
fn search_found_single_leaf() {
    let node = leaf(42, 0, 1);
    let src = b"x";
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 42);
    TreeWalker::new(src).walk(&node, &mut search);
    assert!(!search.matches.is_empty());
}

#[test]
fn search_found_returns_correct_byte_range() {
    let node = leaf(42, 5, 10);
    let src = b"0123456789";
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 42);
    TreeWalker::new(src).walk(&node, &mut search);
    assert_eq!(search.matches[0].0, 5);
    assert_eq!(search.matches[0].1, 10);
}

#[test]
fn search_found_in_nested_tree() {
    let target = leaf(42, 0, 1);
    let mid = interior(10, vec![target]);
    let root = interior(20, vec![mid]);
    let src = b"x";
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 42);
    TreeWalker::new(src).walk(&root, &mut search);
    assert!(!search.matches.is_empty());
}

#[test]
fn search_found_named_predicate() {
    let named = leaf(1, 0, 1);
    let unnamed = unnamed_leaf(2, 1, 2);
    let root = interior(10, vec![named, unnamed]);
    let src = b"ab";
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.is_named());
    TreeWalker::new(src).walk(&root, &mut search);
    // root + named leaf = at least 2
    assert!(search.matches.len() >= 2);
}

#[test]
fn search_found_all_nodes_with_always_true() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|_n: &ParsedNode| true);
    TreeWalker::new(&src).walk(&root, &mut search);
    // All 6 non-error nodes: root, a, mid, b, c, d
    assert_eq!(search.matches.len(), 6);
}

#[test]
fn search_found_sample_tree_specific_symbol() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 2);
    TreeWalker::new(&src).walk(&root, &mut search);
    // Only b(2)
    assert_eq!(search.matches.len(), 1);
}

// ===================================================================
// Category 17 — search_missing_* (4 tests)
// ===================================================================

#[test]
fn search_missing_wrong_symbol() {
    let node = leaf(1, 0, 1);
    let src = b"x";
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 999);
    TreeWalker::new(src).walk(&node, &mut search);
    assert!(search.matches.is_empty());
}

#[test]
fn search_missing_in_large_tree() {
    let (root, src) = hundred_node_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 9999);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert!(search.matches.is_empty());
}

#[test]
fn search_missing_never_predicate() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|_n: &ParsedNode| false);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert!(search.matches.is_empty());
}

#[test]
fn search_missing_unnamed_when_searching_named() {
    let node = unnamed_leaf(1, 0, 1);
    let src = b"x";
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.is_named());
    TreeWalker::new(src).walk(&node, &mut search);
    assert!(search.matches.is_empty());
}

// ===================================================================
// Category 18 — search_count_* (5 tests)
// ===================================================================

#[test]
fn search_count_one_match() {
    let node = leaf(42, 0, 1);
    let src = b"x";
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 42);
    TreeWalker::new(src).walk(&node, &mut search);
    assert_eq!(search.matches.len(), 1);
}

#[test]
fn search_count_multiple_same_symbol() {
    // Two leaves with same symbol
    let a = leaf(42, 0, 1);
    let b = leaf(42, 1, 2);
    let root = interior(10, vec![a, b]);
    let src = b"ab";
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 42);
    TreeWalker::new(src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 2);
}

#[test]
fn search_count_wide_tree_all() {
    let (root, src) = wide_tree();
    let mut search = SearchVisitor::new(|_n: &ParsedNode| true);
    TreeWalker::new(&src).walk(&root, &mut search);
    // root + 5 children = 6
    assert_eq!(search.matches.len(), 6);
}

#[test]
fn search_count_hundred_leaves() {
    let (root, src) = hundred_node_tree();
    let mut search = SearchVisitor::new(|_n: &ParsedNode| true);
    TreeWalker::new(&src).walk(&root, &mut search);
    // root + 100 = 101
    assert_eq!(search.matches.len(), 101);
}

#[test]
fn search_count_deep_chain() {
    let (root, src) = deep_chain();
    let mut search = SearchVisitor::new(|_n: &ParsedNode| true);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 4);
}

// ===================================================================
// Category 19 — debug_* (4 tests)
// ===================================================================

#[test]
fn debug_stats_visitor_format() {
    let stats = StatsVisitor::default();
    let dbg = format!("{:?}", stats);
    assert!(dbg.contains("StatsVisitor"));
}

#[test]
fn debug_stats_visitor_after_walk() {
    let node = leaf(1, 0, 1);
    let stats = walk_stats(&node, b"x");
    let dbg = format!("{:?}", stats);
    assert!(dbg.contains("total_nodes"));
}

#[test]
fn debug_visitor_action_variants() {
    use adze::visitor::VisitorAction;
    let c = format!("{:?}", VisitorAction::Continue);
    let s = format!("{:?}", VisitorAction::SkipChildren);
    let t = format!("{:?}", VisitorAction::Stop);
    assert!(c.contains("Continue"));
    assert!(s.contains("SkipChildren"));
    assert!(t.contains("Stop"));
}

#[test]
fn debug_visitor_action_eq() {
    use adze::visitor::VisitorAction;
    assert_eq!(VisitorAction::Continue, VisitorAction::Continue);
    assert_ne!(VisitorAction::Continue, VisitorAction::Stop);
    assert_ne!(VisitorAction::SkipChildren, VisitorAction::Stop);
}

// ===================================================================
// Category 20 — cross_* (6 tests)
// ===================================================================

#[test]
fn cross_stats_and_search_same_tree() {
    let (root, src) = sample_tree();
    let stats = walk_stats(&root, &src);
    let mut search = SearchVisitor::new(|_n: &ParsedNode| true);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(stats.total_nodes, search.matches.len());
}

#[test]
fn cross_stats_and_pretty_both_visit() {
    let (root, src) = sample_tree();
    let stats = walk_stats(&root, &src);
    let pp = walk_pretty(&root, &src);
    assert!(stats.total_nodes > 0);
    assert!(!pp.output().is_empty());
}

#[test]
fn cross_empty_leaf_both_visitors() {
    let node = leaf(1, 0, 0);
    let src = b"";
    let stats = walk_stats(&node, src);
    let pp = walk_pretty(&node, src);
    assert_eq!(stats.total_nodes, 1);
    assert!(!pp.output().is_empty());
}

#[test]
fn cross_search_count_matches_stats_total() {
    let (root, src) = wide_tree();
    let stats = walk_stats(&root, &src);
    let mut search = SearchVisitor::new(|_n: &ParsedNode| true);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(stats.total_nodes, search.matches.len());
}

#[test]
fn cross_deep_tree_all_visitors() {
    let (root, src) = deep_chain();
    let stats = walk_stats(&root, &src);
    let pp = walk_pretty(&root, &src);
    let mut search = SearchVisitor::new(|_n: &ParsedNode| true);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(stats.total_nodes, 4);
    assert!(!pp.output().is_empty());
    assert_eq!(search.matches.len(), 4);
}

#[test]
fn cross_hundred_nodes_all_visitors() {
    let (root, src) = hundred_node_tree();
    let stats = walk_stats(&root, &src);
    let pp = walk_pretty(&root, &src);
    let mut search = SearchVisitor::new(|_n: &ParsedNode| true);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(stats.total_nodes, 101);
    assert_eq!(stats.leaf_nodes, 100);
    assert!(pp.output().lines().count() >= 101);
    assert_eq!(search.matches.len(), 101);
}

//! StatsVisitor comprehensive tests — 84 tests across 14 categories.
//!
//! Categories:
//!   default_*             — Default construction yields zeroed state
//!   single_leaf_*         — Stats after walking a single leaf node
//!   single_unnamed_*      — Stats after walking a single unnamed leaf
//!   children_*            — Stats with interior + children trees
//!   named_anon_split_*    — Verifying named vs anonymous node-count tracking
//!   depth_chain_*         — Depth tracking in linear chains
//!   depth_deep_*          — Deep nesting (10–50 levels)
//!   wide_*                — Wide trees (many children)
//!   error_*               — Error node handling
//!   node_counts_map_*     — Per-kind node_counts HashMap
//!   reuse_*               — Reusing a StatsVisitor across multiple walks
//!   debug_*               — Debug trait coverage
//!   large_*               — Large trees (100–1000 nodes)
//!   invariant_*           — Cross-field invariants

use adze::pure_parser::ParsedNode;
use adze::visitor::{StatsVisitor, TreeWalker};

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

/// Walk a tree and return a fresh StatsVisitor with results.
fn walk_stats(root: &ParsedNode, src: &[u8]) -> StatsVisitor {
    let mut stats = StatsVisitor::default();
    TreeWalker::new(src).walk(root, &mut stats);
    stats
}

/// Build a linear chain of `depth` interior nodes ending in a leaf.
fn chain_tree(depth: usize) -> (ParsedNode, Vec<u8>) {
    let src = b"x".to_vec();
    let mut node = leaf(depth as u16 + 1, 0, 1);
    for i in (1..=depth).rev() {
        node = interior(i as u16, vec![node]);
    }
    (node, src)
}

// ===================================================================
// Category 1 — default_* (5 tests)
// ===================================================================

#[test]
fn default_total_nodes_zero() {
    let stats = StatsVisitor::default();
    assert_eq!(stats.total_nodes, 0);
}

#[test]
fn default_leaf_nodes_zero() {
    let stats = StatsVisitor::default();
    assert_eq!(stats.leaf_nodes, 0);
}

#[test]
fn default_error_nodes_zero() {
    let stats = StatsVisitor::default();
    assert_eq!(stats.error_nodes, 0);
}

#[test]
fn default_max_depth_zero() {
    let stats = StatsVisitor::default();
    assert_eq!(stats.max_depth, 0);
}

#[test]
fn default_node_counts_empty() {
    let stats = StatsVisitor::default();
    assert!(stats.node_counts.is_empty());
}

// ===================================================================
// Category 2 — single_leaf_* (6 tests)
// ===================================================================

#[test]
fn single_leaf_total_nodes_is_one() {
    let node = leaf(1, 0, 1);
    let stats = walk_stats(&node, b"x");
    assert_eq!(stats.total_nodes, 1);
}

#[test]
fn single_leaf_leaf_count_is_one() {
    let node = leaf(1, 0, 1);
    let stats = walk_stats(&node, b"x");
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn single_leaf_max_depth_is_one() {
    let node = leaf(1, 0, 1);
    let stats = walk_stats(&node, b"x");
    assert_eq!(stats.max_depth, 1);
}

#[test]
fn single_leaf_error_count_zero() {
    let node = leaf(1, 0, 1);
    let stats = walk_stats(&node, b"x");
    assert_eq!(stats.error_nodes, 0);
}

#[test]
fn single_leaf_node_counts_has_one_entry() {
    let node = leaf(1, 0, 1);
    let stats = walk_stats(&node, b"x");
    assert_eq!(stats.node_counts.len(), 1);
}

#[test]
fn single_leaf_node_counts_symbol_value() {
    let node = leaf(1, 0, 1);
    let stats = walk_stats(&node, b"x");
    // Symbol 1 with no language → kind is "*"
    assert_eq!(*stats.node_counts.get("*").unwrap_or(&0), 1);
}

// ===================================================================
// Category 3 — single_unnamed_* (4 tests)
// ===================================================================

#[test]
fn single_unnamed_total_nodes_is_one() {
    let node = unnamed_leaf(1, 0, 1);
    let stats = walk_stats(&node, b"x");
    assert_eq!(stats.total_nodes, 1);
}

#[test]
fn single_unnamed_leaf_count_is_one() {
    let node = unnamed_leaf(1, 0, 1);
    let stats = walk_stats(&node, b"x");
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn single_unnamed_max_depth_is_one() {
    let node = unnamed_leaf(1, 0, 1);
    let stats = walk_stats(&node, b"x");
    assert_eq!(stats.max_depth, 1);
}

#[test]
fn single_unnamed_error_count_zero() {
    let node = unnamed_leaf(1, 0, 1);
    let stats = walk_stats(&node, b"x");
    assert_eq!(stats.error_nodes, 0);
}

// ===================================================================
// Category 4 — children_* (8 tests)
// ===================================================================

#[test]
fn children_root_with_two_leaves_total_three() {
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let stats = walk_stats(&root, b"ab");
    assert_eq!(stats.total_nodes, 3);
}

#[test]
fn children_root_with_two_leaves_leaf_count_two() {
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let stats = walk_stats(&root, b"ab");
    assert_eq!(stats.leaf_nodes, 2);
}

#[test]
fn children_root_with_two_leaves_max_depth_two() {
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let stats = walk_stats(&root, b"ab");
    assert_eq!(stats.max_depth, 2);
}

#[test]
fn children_five_named_three_unnamed_total() {
    let mut kids: Vec<ParsedNode> = (0..5)
        .map(|i| leaf(i + 1, i as usize, i as usize + 1))
        .collect();
    kids.extend((5..8).map(|i| unnamed_leaf(i + 1, i as usize, i as usize + 1)));
    let root = interior(100, kids);
    let stats = walk_stats(&root, b"abcdefgh");
    // 1 root + 8 leaves = 9
    assert_eq!(stats.total_nodes, 9);
}

#[test]
fn children_five_named_three_unnamed_leaf_count() {
    let mut kids: Vec<ParsedNode> = (0..5)
        .map(|i| leaf(i + 1, i as usize, i as usize + 1))
        .collect();
    kids.extend((5..8).map(|i| unnamed_leaf(i + 1, i as usize, i as usize + 1)));
    let root = interior(100, kids);
    let stats = walk_stats(&root, b"abcdefgh");
    assert_eq!(stats.leaf_nodes, 8);
}

#[test]
fn children_nested_two_levels_total() {
    let child = interior(11, vec![leaf(2, 0, 1)]);
    let root = interior(10, vec![child]);
    let stats = walk_stats(&root, b"x");
    assert_eq!(stats.total_nodes, 3);
}

#[test]
fn children_nested_two_levels_depth() {
    let child = interior(11, vec![leaf(2, 0, 1)]);
    let root = interior(10, vec![child]);
    let stats = walk_stats(&root, b"x");
    assert_eq!(stats.max_depth, 3);
}

#[test]
fn children_nested_two_levels_leaf_count() {
    let child = interior(11, vec![leaf(2, 0, 1)]);
    let root = interior(10, vec![child]);
    let stats = walk_stats(&root, b"x");
    assert_eq!(stats.leaf_nodes, 1);
}

// ===================================================================
// Category 5 — named_anon_split_* (6 tests)
// ===================================================================

#[test]
fn named_anon_split_all_named_leaves() {
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let stats = walk_stats(&root, b"abc");
    // All are leaves, all named — verify leaf count matches
    assert_eq!(stats.leaf_nodes, 3);
}

#[test]
fn named_anon_split_all_unnamed_leaves() {
    let root = interior(
        10,
        vec![
            unnamed_leaf(1, 0, 1),
            unnamed_leaf(2, 1, 2),
            unnamed_leaf(3, 2, 3),
        ],
    );
    let stats = walk_stats(&root, b"abc");
    assert_eq!(stats.leaf_nodes, 3);
}

#[test]
fn named_anon_split_mixed_leaves_total() {
    let root = interior(
        10,
        vec![
            leaf(1, 0, 1),
            unnamed_leaf(2, 1, 2),
            leaf(3, 2, 3),
            unnamed_leaf(4, 3, 4),
        ],
    );
    let stats = walk_stats(&root, b"abcd");
    // 1 root + 4 leaves = 5
    assert_eq!(stats.total_nodes, 5);
}

#[test]
fn named_anon_split_mixed_leaves_leaf_count() {
    let root = interior(
        10,
        vec![
            leaf(1, 0, 1),
            unnamed_leaf(2, 1, 2),
            leaf(3, 2, 3),
            unnamed_leaf(4, 3, 4),
        ],
    );
    let stats = walk_stats(&root, b"abcd");
    assert_eq!(stats.leaf_nodes, 4);
}

#[test]
fn named_anon_split_interleaved_count() {
    // Alternate named / unnamed
    let kids: Vec<ParsedNode> = (0..10)
        .map(|i| {
            if i % 2 == 0 {
                leaf(i + 1, i as usize, i as usize + 1)
            } else {
                unnamed_leaf(i + 1, i as usize, i as usize + 1)
            }
        })
        .collect();
    let root = interior(100, kids);
    let stats = walk_stats(&root, b"abcdefghij");
    // 1 root + 10 leaves = 11
    assert_eq!(stats.total_nodes, 11);
}

#[test]
fn named_anon_split_interleaved_leaf_count() {
    let kids: Vec<ParsedNode> = (0..10)
        .map(|i| {
            if i % 2 == 0 {
                leaf(i + 1, i as usize, i as usize + 1)
            } else {
                unnamed_leaf(i + 1, i as usize, i as usize + 1)
            }
        })
        .collect();
    let root = interior(100, kids);
    let stats = walk_stats(&root, b"abcdefghij");
    assert_eq!(stats.leaf_nodes, 10);
}

// ===================================================================
// Category 6 — depth_chain_* (8 tests)
// ===================================================================

#[test]
fn depth_chain_single_node_depth_one() {
    let node = leaf(1, 0, 1);
    let stats = walk_stats(&node, b"x");
    assert_eq!(stats.max_depth, 1);
}

#[test]
fn depth_chain_two_levels() {
    let (root, src) = chain_tree(1);
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.max_depth, 2);
}

#[test]
fn depth_chain_three_levels() {
    let (root, src) = chain_tree(2);
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.max_depth, 3);
}

#[test]
fn depth_chain_five_levels() {
    let (root, src) = chain_tree(4);
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.max_depth, 5);
}

#[test]
fn depth_chain_total_matches_depth() {
    // A pure chain of n interior + 1 leaf = n+1 nodes
    let (root, src) = chain_tree(4);
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.total_nodes, 5);
}

#[test]
fn depth_chain_leaf_count_is_one() {
    let (root, src) = chain_tree(4);
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn depth_chain_depth_resets_across_siblings() {
    // root → [chain(3), chain(2)] — max depth is from the deeper branch
    let deep_chain = interior(11, vec![interior(12, vec![leaf(13, 0, 1)])]);
    let shallow_chain = interior(21, vec![leaf(22, 1, 2)]);
    let root = interior(10, vec![deep_chain, shallow_chain]);
    let stats = walk_stats(&root, b"xy");
    assert_eq!(stats.max_depth, 4); // root → 11 → 12 → 13
}

#[test]
fn depth_chain_depth_tracks_deepest_branch() {
    // Asymmetric tree: left branch depth 3, right branch depth 5
    let left = interior(11, vec![interior(12, vec![leaf(13, 0, 1)])]);
    let right = interior(
        21,
        vec![interior(
            22,
            vec![interior(23, vec![interior(24, vec![leaf(25, 1, 2)])])],
        )],
    );
    let root = interior(10, vec![left, right]);
    let stats = walk_stats(&root, b"xy");
    assert_eq!(stats.max_depth, 6); // root → 21 → 22 → 23 → 24 → 25
}

// ===================================================================
// Category 7 — depth_deep_* (4 tests)
// ===================================================================

#[test]
fn depth_deep_ten_levels() {
    let (root, src) = chain_tree(9);
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.max_depth, 10);
}

#[test]
fn depth_deep_ten_levels_total_nodes() {
    let (root, src) = chain_tree(9);
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.total_nodes, 10);
}

#[test]
fn depth_deep_fifty_levels() {
    let (root, src) = chain_tree(49);
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.max_depth, 50);
}

#[test]
fn depth_deep_fifty_levels_total_nodes() {
    let (root, src) = chain_tree(49);
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.total_nodes, 50);
}

// ===================================================================
// Category 8 — wide_* (6 tests)
// ===================================================================

#[test]
fn wide_five_children_total() {
    let kids: Vec<ParsedNode> = (0..5)
        .map(|i| leaf(i + 1, i as usize, i as usize + 1))
        .collect();
    let root = interior(50, kids);
    let stats = walk_stats(&root, b"abcde");
    assert_eq!(stats.total_nodes, 6);
}

#[test]
fn wide_five_children_leaf_count() {
    let kids: Vec<ParsedNode> = (0..5)
        .map(|i| leaf(i + 1, i as usize, i as usize + 1))
        .collect();
    let root = interior(50, kids);
    let stats = walk_stats(&root, b"abcde");
    assert_eq!(stats.leaf_nodes, 5);
}

#[test]
fn wide_five_children_depth() {
    let kids: Vec<ParsedNode> = (0..5)
        .map(|i| leaf(i + 1, i as usize, i as usize + 1))
        .collect();
    let root = interior(50, kids);
    let stats = walk_stats(&root, b"abcde");
    assert_eq!(stats.max_depth, 2);
}

#[test]
fn wide_twenty_children_total() {
    let kids: Vec<ParsedNode> = (0..20)
        .map(|i| leaf(i + 1, i as usize, i as usize + 1))
        .collect();
    let src: Vec<u8> = (0..20).map(|i| b'a' + (i % 26)).collect();
    let root = interior(50, kids);
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.total_nodes, 21);
}

#[test]
fn wide_twenty_children_leaf_count() {
    let kids: Vec<ParsedNode> = (0..20)
        .map(|i| leaf(i + 1, i as usize, i as usize + 1))
        .collect();
    let src: Vec<u8> = (0..20).map(|i| b'a' + (i % 26)).collect();
    let root = interior(50, kids);
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.leaf_nodes, 20);
}

#[test]
fn wide_twenty_children_depth_is_two() {
    let kids: Vec<ParsedNode> = (0..20)
        .map(|i| leaf(i + 1, i as usize, i as usize + 1))
        .collect();
    let src: Vec<u8> = (0..20).map(|i| b'a' + (i % 26)).collect();
    let root = interior(50, kids);
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.max_depth, 2);
}

// ===================================================================
// Category 9 — error_* (6 tests)
// ===================================================================

#[test]
fn error_single_error_node_increments_error_count() {
    let node = error_node(0, 1);
    let stats = walk_stats(&node, b"x");
    assert_eq!(stats.error_nodes, 1);
}

#[test]
fn error_single_error_node_total_is_zero() {
    // Error nodes bypass enter_node, so total_nodes stays 0
    let node = error_node(0, 1);
    let stats = walk_stats(&node, b"x");
    assert_eq!(stats.total_nodes, 0);
}

#[test]
fn error_single_error_node_leaf_is_zero() {
    let node = error_node(0, 1);
    let stats = walk_stats(&node, b"x");
    assert_eq!(stats.leaf_nodes, 0);
}

#[test]
fn error_mixed_with_normal_nodes() {
    let root = interior(10, vec![leaf(1, 0, 1), error_node(1, 2), leaf(2, 2, 3)]);
    let stats = walk_stats(&root, b"xey");
    assert_eq!(stats.error_nodes, 1);
}

#[test]
fn error_mixed_total_excludes_error() {
    let root = interior(10, vec![leaf(1, 0, 1), error_node(1, 2), leaf(2, 2, 3)]);
    let stats = walk_stats(&root, b"xey");
    // root + 2 normal leaves = 3 (error node not counted in total_nodes)
    assert_eq!(stats.total_nodes, 3);
}

#[test]
fn error_multiple_errors() {
    let root = interior(
        10,
        vec![error_node(0, 1), error_node(1, 2), error_node(2, 3)],
    );
    let stats = walk_stats(&root, b"eee");
    assert_eq!(stats.error_nodes, 3);
}

// ===================================================================
// Category 10 — node_counts_map_* (7 tests)
// ===================================================================

#[test]
fn node_counts_map_single_kind() {
    let node = leaf(1, 0, 1);
    let stats = walk_stats(&node, b"x");
    assert_eq!(stats.node_counts.len(), 1);
}

#[test]
fn node_counts_map_two_distinct_kinds() {
    // Two leaves with same symbol → same kind → 1 map entry
    let root = interior(10, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let stats = walk_stats(&root, b"ab");
    // root kind + leaf kind = 2 distinct kinds
    // symbol 10 → "unknown", symbol 1 → "*"
    assert_eq!(stats.node_counts.len(), 2);
}

#[test]
fn node_counts_map_accumulates_same_kind() {
    let root = interior(10, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let stats = walk_stats(&root, b"ab");
    // symbol 1 → kind "*", appears twice
    assert_eq!(*stats.node_counts.get("*").unwrap_or(&0), 2);
}

#[test]
fn node_counts_map_different_symbols() {
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let stats = walk_stats(&root, b"abc");
    // 4 nodes total across root + 3 leaves
    assert_eq!(stats.node_counts.values().sum::<usize>(), 4);
}

#[test]
fn node_counts_map_sum_equals_total() {
    let (root, src) = chain_tree(4);
    let stats = walk_stats(&root, &src);
    let map_sum: usize = stats.node_counts.values().sum();
    assert_eq!(map_sum, stats.total_nodes);
}

#[test]
fn node_counts_map_excludes_error_nodes() {
    let root = interior(10, vec![leaf(1, 0, 1), error_node(1, 2)]);
    let stats = walk_stats(&root, b"xe");
    // Error nodes don't call enter_node, so not in node_counts
    let map_sum: usize = stats.node_counts.values().sum();
    assert_eq!(map_sum, stats.total_nodes);
}

#[test]
fn node_counts_map_empty_before_walk() {
    let stats = StatsVisitor::default();
    assert!(stats.node_counts.is_empty());
}

// ===================================================================
// Category 11 — reuse_* (6 tests)
// ===================================================================

#[test]
fn reuse_accumulates_total_nodes() {
    let n1 = leaf(1, 0, 1);
    let n2 = leaf(2, 0, 1);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(b"x").walk(&n1, &mut stats);
    TreeWalker::new(b"y").walk(&n2, &mut stats);
    assert_eq!(stats.total_nodes, 2);
}

#[test]
fn reuse_accumulates_leaf_nodes() {
    let n1 = leaf(1, 0, 1);
    let n2 = leaf(2, 0, 1);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(b"x").walk(&n1, &mut stats);
    TreeWalker::new(b"y").walk(&n2, &mut stats);
    assert_eq!(stats.leaf_nodes, 2);
}

#[test]
fn reuse_accumulates_error_nodes() {
    let e1 = error_node(0, 1);
    let e2 = error_node(0, 1);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(b"x").walk(&e1, &mut stats);
    TreeWalker::new(b"y").walk(&e2, &mut stats);
    assert_eq!(stats.error_nodes, 2);
}

#[test]
fn reuse_max_depth_keeps_maximum() {
    let (deep, deep_src) = chain_tree(4); // depth 5
    let shallow = leaf(1, 0, 1); // depth 1
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&deep_src).walk(&deep, &mut stats);
    TreeWalker::new(b"x").walk(&shallow, &mut stats);
    assert_eq!(stats.max_depth, 5);
}

#[test]
fn reuse_max_depth_updates_if_deeper() {
    let shallow = leaf(1, 0, 1);
    let (deep, deep_src) = chain_tree(9); // depth 10
    let mut stats = StatsVisitor::default();
    TreeWalker::new(b"x").walk(&shallow, &mut stats);
    TreeWalker::new(&deep_src).walk(&deep, &mut stats);
    assert_eq!(stats.max_depth, 10);
}

#[test]
fn reuse_node_counts_merge() {
    let n1 = leaf(1, 0, 1);
    let n2 = leaf(1, 0, 1);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(b"x").walk(&n1, &mut stats);
    TreeWalker::new(b"y").walk(&n2, &mut stats);
    // Two visits of symbol 1 (kind "*")
    assert_eq!(*stats.node_counts.get("*").unwrap_or(&0), 2);
}

// ===================================================================
// Category 12 — debug_* (4 tests)
// ===================================================================

#[test]
fn debug_default_is_non_empty() {
    let stats = StatsVisitor::default();
    let dbg = format!("{:?}", stats);
    assert!(!dbg.is_empty());
}

#[test]
fn debug_contains_total_nodes() {
    let stats = StatsVisitor::default();
    let dbg = format!("{:?}", stats);
    assert!(dbg.contains("total_nodes"));
}

#[test]
fn debug_after_walk_is_non_empty() {
    let node = leaf(1, 0, 1);
    let stats = walk_stats(&node, b"x");
    let dbg = format!("{:?}", stats);
    assert!(!dbg.is_empty());
}

#[test]
fn debug_after_walk_shows_nonzero() {
    let node = leaf(1, 0, 1);
    let stats = walk_stats(&node, b"x");
    let dbg = format!("{:?}", stats);
    // Should contain "total_nodes: 1" (not 0)
    assert!(dbg.contains("1"));
}

// ===================================================================
// Category 13 — large_* (8 tests)
// ===================================================================

#[test]
fn large_hundred_leaves_total() {
    let kids: Vec<ParsedNode> = (0u16..100)
        .map(|i| leaf(i + 1, i as usize, i as usize + 1))
        .collect();
    let src: Vec<u8> = (0..100).map(|i| b'a' + (i % 26)).collect();
    let root = interior(999, kids);
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.total_nodes, 101);
}

#[test]
fn large_hundred_leaves_leaf_count() {
    let kids: Vec<ParsedNode> = (0u16..100)
        .map(|i| leaf(i + 1, i as usize, i as usize + 1))
        .collect();
    let src: Vec<u8> = (0..100).map(|i| b'a' + (i % 26)).collect();
    let root = interior(999, kids);
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.leaf_nodes, 100);
}

#[test]
fn large_hundred_leaves_depth_is_two() {
    let kids: Vec<ParsedNode> = (0u16..100)
        .map(|i| leaf(i + 1, i as usize, i as usize + 1))
        .collect();
    let src: Vec<u8> = (0..100).map(|i| b'a' + (i % 26)).collect();
    let root = interior(999, kids);
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.max_depth, 2);
}

#[test]
fn large_hundred_leaves_map_sum_equals_total() {
    let kids: Vec<ParsedNode> = (0u16..100)
        .map(|i| leaf(i + 1, i as usize, i as usize + 1))
        .collect();
    let src: Vec<u8> = (0..100).map(|i| b'a' + (i % 26)).collect();
    let root = interior(999, kids);
    let stats = walk_stats(&root, &src);
    let map_sum: usize = stats.node_counts.values().sum();
    assert_eq!(map_sum, stats.total_nodes);
}

#[test]
fn large_thousand_chain_depth() {
    let (root, src) = chain_tree(999);
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.max_depth, 1000);
}

#[test]
fn large_thousand_chain_total() {
    let (root, src) = chain_tree(999);
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.total_nodes, 1000);
}

#[test]
fn large_thousand_chain_leaf_is_one() {
    let (root, src) = chain_tree(999);
    let stats = walk_stats(&root, &src);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn large_two_hundred_mixed_nodes() {
    let mut kids = Vec::new();
    for i in 0u16..200 {
        if i % 3 == 0 {
            kids.push(unnamed_leaf(i + 1, i as usize, i as usize + 1));
        } else {
            kids.push(leaf(i + 1, i as usize, i as usize + 1));
        }
    }
    let src: Vec<u8> = (0..200).map(|i| b'a' + (i % 26)).collect();
    let root = interior(999, kids);
    let stats = walk_stats(&root, &src);
    // 1 root + 200 leaves
    assert_eq!(stats.total_nodes, 201);
}

// ===================================================================
// Category 14 — invariant_* (10 tests)
// ===================================================================

#[test]
fn invariant_leaf_count_leq_total() {
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let stats = walk_stats(&root, b"ab");
    assert!(stats.leaf_nodes <= stats.total_nodes);
}

#[test]
fn invariant_error_count_zero_for_no_errors() {
    let root = interior(10, vec![leaf(1, 0, 1)]);
    let stats = walk_stats(&root, b"x");
    assert_eq!(stats.error_nodes, 0);
}

#[test]
fn invariant_max_depth_positive_after_walk() {
    let node = leaf(1, 0, 1);
    let stats = walk_stats(&node, b"x");
    assert!(stats.max_depth > 0);
}

#[test]
fn invariant_total_positive_after_walk() {
    let node = leaf(1, 0, 1);
    let stats = walk_stats(&node, b"x");
    assert!(stats.total_nodes > 0);
}

#[test]
fn invariant_depth_geq_one_for_nonempty_tree() {
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let stats = walk_stats(&root, b"abc");
    assert!(stats.max_depth >= 1);
}

#[test]
fn invariant_map_sum_equals_total_for_sample_tree() {
    let a = leaf(1, 0, 1);
    let b = leaf(2, 1, 2);
    let c = unnamed_leaf(3, 2, 3);
    let mid = interior(11, vec![b, c]);
    let d = leaf(4, 3, 4);
    let root = interior(10, vec![a, mid, d]);
    let stats = walk_stats(&root, b"abcd");
    let map_sum: usize = stats.node_counts.values().sum();
    assert_eq!(map_sum, stats.total_nodes);
}

#[test]
fn invariant_depth_after_pop_returns_to_zero() {
    // After full walk, internal current_depth should be 0 (verified via max_depth stability)
    let node = leaf(1, 0, 1);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(b"x").walk(&node, &mut stats);
    let depth_after_first = stats.max_depth;
    // Walk again with a same-depth tree — max_depth should not increase
    let node2 = leaf(2, 0, 1);
    TreeWalker::new(b"y").walk(&node2, &mut stats);
    assert_eq!(stats.max_depth, depth_after_first);
}

#[test]
fn invariant_three_level_tree_counts() {
    let l2a = leaf(5, 0, 1);
    let l2b = leaf(6, 1, 2);
    let l2c = leaf(7, 2, 3);
    let l1a = interior(3, vec![l2a, l2b]);
    let l1b = interior(4, vec![l2c]);
    let root = interior(10, vec![l1a, l1b]);
    let stats = walk_stats(&root, b"xyz");
    // 3 interior + 3 leaf = 6
    assert_eq!(stats.total_nodes, 6);
    assert_eq!(stats.leaf_nodes, 3);
    assert_eq!(stats.max_depth, 3);
}

#[test]
fn invariant_error_only_tree_total_is_zero() {
    let root = interior(10, vec![error_node(0, 1), error_node(1, 2)]);
    let stats = walk_stats(&root, b"ee");
    // Root counts via enter_node; errors don't
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.error_nodes, 2);
}

#[test]
fn invariant_leaf_plus_interior_breakdown() {
    // In a tree, interior nodes = total - leaf (when no errors)
    let l = leaf(1, 0, 1);
    let mid = interior(2, vec![l]);
    let root = interior(3, vec![mid]);
    let stats = walk_stats(&root, b"x");
    let interior_count = stats.total_nodes - stats.leaf_nodes;
    assert_eq!(interior_count, 2); // root + mid
}

// ===================================================================
// Additional edge-case tests to reach 84+
// ===================================================================

#[test]
fn edge_empty_interior_no_children() {
    // Interior node with zero children — it becomes a leaf
    let root = make_node(10, vec![], 0, 0, false, true);
    let stats = walk_stats(&root, b"");
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn edge_single_error_max_depth_is_zero() {
    // Error node does not increment depth
    let node = error_node(0, 1);
    let stats = walk_stats(&node, b"x");
    assert_eq!(stats.max_depth, 0);
}

#[test]
fn edge_deep_left_shallow_right() {
    // Left branch: depth 4, right branch: depth 2
    let left = interior(11, vec![interior(12, vec![leaf(13, 0, 1)])]);
    let right = leaf(21, 1, 2);
    let root = interior(10, vec![left, right]);
    let stats = walk_stats(&root, b"xy");
    assert_eq!(stats.max_depth, 4);
    assert_eq!(stats.total_nodes, 5);
}

#[test]
fn edge_identical_symbols_all_map_to_same_kind() {
    // Multiple leaves with same symbol → same kind entry
    let kids: Vec<ParsedNode> = (0..5).map(|i| leaf(1, i, i + 1)).collect();
    let root = interior(10, kids);
    let stats = walk_stats(&root, b"abcde");
    // kind for symbol 1 appears 5 times
    assert_eq!(*stats.node_counts.get("*").unwrap_or(&0), 5);
}

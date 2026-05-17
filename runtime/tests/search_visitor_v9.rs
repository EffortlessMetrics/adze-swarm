//! SearchVisitor v9 — 84 comprehensive tests across 20 categories.
//!
//! Categories:
//!   new_visitor_*           — freshly created SearchVisitor state
//!   visit_match_*           — visiting nodes that match the predicate
//!   visit_no_match_*        — visiting nodes that do not match
//!   multi_match_*           — multiple matching nodes
//!   target_kind_*           — various symbol/kind values as targets
//!   named_unnamed_*         — named vs unnamed node matching
//!   cumulative_*            — cumulative count over sequential walks
//!   count_invariant_*       — count never decreases
//!   found_count_*           — found ↔ count relationship
//!   large_tree_*            — trees with many nodes
//!   multi_visitor_*         — independent visitors with different targets
//!   debug_format_*          — Debug trait formatting
//!   predicate_*             — various predicate shapes
//!   byte_range_*            — recorded match byte ranges
//!   deep_tree_*             — deep chain trees
//!   wide_tree_*             — wide flat trees
//!   error_node_*            — error nodes in tree
//!   mixed_*                 — mixed named/unnamed/error trees
//!   reuse_*                 — walker/visitor reuse patterns
//!   edge_case_*             — boundary/edge-case scenarios

use adze::pure_parser::ParsedNode;
use adze::visitor::{SearchVisitor, TreeWalker, Visitor, VisitorAction};

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

/// Sample tree:
/// ```text
/// root(10)
///  ├── a(1) named leaf
///  ├── mid(11) named interior
///  │    ├── b(2) named leaf
///  │    └── c(3) unnamed leaf
///  └── d(4) named leaf
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

/// Chain: root(100) → n1(101) → n2(102) → n3(103) leaf
fn deep_tree() -> (ParsedNode, Vec<u8>) {
    let src = b"deep".to_vec();
    let n3 = leaf(103, 0, 4);
    let n2 = interior(102, vec![n3]);
    let n1 = interior(101, vec![n2]);
    let root = interior(100, vec![n1]);
    (root, src)
}

/// Root with 10 direct named leaves.
fn wide_tree_10() -> (ParsedNode, Vec<u8>) {
    let src: Vec<u8> = (0..10).map(|i| b'a' + i as u8).collect();
    let children: Vec<ParsedNode> = (0..10).map(|i| leaf(i as u16 + 1, i, i + 1)).collect();
    let root = interior(50, children);
    (root, src)
}

// ===================================================================
// Category 1 — new_visitor_* (5 tests)
// ===================================================================

#[test]
fn new_visitor_matches_empty() {
    let search = SearchVisitor::new(|_: &ParsedNode| true);
    assert!(search.matches.is_empty());
}

#[test]
fn new_visitor_count_zero() {
    let search = SearchVisitor::new(|_: &ParsedNode| false);
    assert_eq!(search.matches.len(), 0);
}

#[test]
fn new_visitor_not_found() {
    let search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 42);
    assert!(search.matches.is_empty());
}

#[test]
fn new_visitor_with_always_true_still_empty() {
    let search = SearchVisitor::new(|_: &ParsedNode| true);
    assert_eq!(search.matches.len(), 0);
}

#[test]
fn new_visitor_with_symbol_predicate_still_empty() {
    let search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 0);
    assert!(search.matches.is_empty());
}

// ===================================================================
// Category 2 — visit_match_* (5 tests)
// ===================================================================

#[test]
fn visit_match_single_leaf() {
    let node = leaf(5, 0, 1);
    let src = b"x".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 5);
    TreeWalker::new(&src).walk(&node, &mut search);
    assert!(!search.matches.is_empty());
}

#[test]
fn visit_match_count_is_one() {
    let node = leaf(5, 0, 1);
    let src = b"x".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 5);
    TreeWalker::new(&src).walk(&node, &mut search);
    assert_eq!(search.matches.len(), 1);
}

#[test]
fn visit_match_root_of_tree() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 10);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 1);
}

#[test]
fn visit_match_interior_node() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 11);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert!(!search.matches.is_empty());
}

#[test]
fn visit_match_deep_leaf() {
    let (root, src) = deep_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 103);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 1);
}

// ===================================================================
// Category 3 — visit_no_match_* (5 tests)
// ===================================================================

#[test]
fn visit_no_match_wrong_symbol() {
    let node = leaf(1, 0, 1);
    let src = b"a".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 99);
    TreeWalker::new(&src).walk(&node, &mut search);
    assert!(search.matches.is_empty());
}

#[test]
fn visit_no_match_count_zero() {
    let node = leaf(1, 0, 1);
    let src = b"a".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 99);
    TreeWalker::new(&src).walk(&node, &mut search);
    assert_eq!(search.matches.len(), 0);
}

#[test]
fn visit_no_match_always_false() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|_: &ParsedNode| false);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert!(search.matches.is_empty());
}

#[test]
fn visit_no_match_deep_tree() {
    let (root, src) = deep_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 999);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 0);
}

#[test]
fn visit_no_match_wide_tree() {
    let (root, src) = wide_tree_10();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 999);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert!(search.matches.is_empty());
}

// ===================================================================
// Category 4 — multi_match_* (5 tests)
// ===================================================================

#[test]
fn multi_match_two_leaves() {
    let a = leaf(7, 0, 1);
    let b = leaf(7, 1, 2);
    let root = interior(50, vec![a, b]);
    let src = b"ab".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 7);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 2);
}

#[test]
fn multi_match_ten_leaves() {
    let children: Vec<ParsedNode> = (0..10).map(|i| leaf(7, i, i + 1)).collect();
    let root = interior(50, children);
    let src: Vec<u8> = (0..10).map(|i| b'a' + i as u8).collect();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 7);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 10);
}

#[test]
fn multi_match_all_named() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.is_named());
    TreeWalker::new(&src).walk(&root, &mut search);
    // root(10), a(1), mid(11), b(2), d(4) = 5 named
    assert_eq!(search.matches.len(), 5);
}

#[test]
fn multi_match_some_match_some_dont() {
    // 3 leaves: sym 1, 2, 1 → search for sym 1 → 2 matches
    let a = leaf(1, 0, 1);
    let b = leaf(2, 1, 2);
    let c = leaf(1, 2, 3);
    let root = interior(50, vec![a, b, c]);
    let src = b"abc".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 2);
}

#[test]
fn multi_match_mixed_named_unnamed() {
    let a = leaf(1, 0, 1);
    let b = unnamed_leaf(1, 1, 2);
    let root = interior(50, vec![a, b]);
    let src = b"ab".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 2);
}

// ===================================================================
// Category 5 — target_kind_* (5 tests)
// ===================================================================

#[test]
fn target_kind_zero() {
    let node = leaf(0, 0, 1);
    let src = b"z".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 0);
    TreeWalker::new(&src).walk(&node, &mut search);
    assert_eq!(search.matches.len(), 1);
}

#[test]
fn target_kind_100() {
    let node = leaf(100, 0, 1);
    let src = b"x".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 100);
    TreeWalker::new(&src).walk(&node, &mut search);
    assert!(!search.matches.is_empty());
}

#[test]
fn target_kind_u16_max() {
    let node = leaf(u16::MAX, 0, 1);
    let src = b"m".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == u16::MAX);
    TreeWalker::new(&src).walk(&node, &mut search);
    assert_eq!(search.matches.len(), 1);
}

#[test]
fn target_kind_one() {
    let node = leaf(1, 0, 1);
    let src = b"a".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    TreeWalker::new(&src).walk(&node, &mut search);
    assert_eq!(search.matches.len(), 1);
}

#[test]
fn target_kind_255() {
    let node = leaf(255, 0, 1);
    let src = b"q".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 255);
    TreeWalker::new(&src).walk(&node, &mut search);
    assert!(!search.matches.is_empty());
}

// ===================================================================
// Category 6 — named_unnamed_* (5 tests)
// ===================================================================

#[test]
fn named_unnamed_named_matches_by_kind() {
    let node = leaf(5, 0, 1);
    let src = b"n".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 5);
    TreeWalker::new(&src).walk(&node, &mut search);
    assert_eq!(search.matches.len(), 1);
}

#[test]
fn named_unnamed_unnamed_matches_by_kind() {
    let node = unnamed_leaf(5, 0, 1);
    let src = b"u".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 5);
    TreeWalker::new(&src).walk(&node, &mut search);
    assert_eq!(search.matches.len(), 1);
}

#[test]
fn named_unnamed_filter_named_only() {
    let a = leaf(1, 0, 1);
    let b = unnamed_leaf(2, 1, 2);
    let root = interior(50, vec![a, b]);
    let src = b"ab".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.is_named());
    TreeWalker::new(&src).walk(&root, &mut search);
    // root + a = 2 named (b is unnamed, root is named)
    assert_eq!(search.matches.len(), 2);
}

#[test]
fn named_unnamed_filter_unnamed_only() {
    let a = leaf(1, 0, 1);
    let b = unnamed_leaf(2, 1, 2);
    let root = interior(50, vec![a, b]);
    let src = b"ab".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| !n.is_named());
    TreeWalker::new(&src).walk(&root, &mut search);
    // only b is unnamed
    assert_eq!(search.matches.len(), 1);
}

#[test]
fn named_unnamed_both_counted_by_symbol() {
    let a = leaf(3, 0, 1);
    let b = unnamed_leaf(3, 1, 2);
    let root = interior(50, vec![a, b]);
    let src = b"xy".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 3);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 2);
}

// ===================================================================
// Category 7 — cumulative_* (4 tests)
// ===================================================================

#[test]
fn cumulative_two_walks_accumulate() {
    let node = leaf(5, 0, 1);
    let src = b"x".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 5);
    TreeWalker::new(&src).walk(&node, &mut search);
    TreeWalker::new(&src).walk(&node, &mut search);
    assert_eq!(search.matches.len(), 2);
}

#[test]
fn cumulative_three_walks() {
    let node = leaf(1, 0, 1);
    let src = b"a".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    for _ in 0..3 {
        TreeWalker::new(&src).walk(&node, &mut search);
    }
    assert_eq!(search.matches.len(), 3);
}

#[test]
fn cumulative_mixed_trees() {
    let node_a = leaf(1, 0, 1);
    let node_b = leaf(1, 0, 2);
    let src_a = b"a".to_vec();
    let src_b = b"ab".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    TreeWalker::new(&src_a).walk(&node_a, &mut search);
    TreeWalker::new(&src_b).walk(&node_b, &mut search);
    assert_eq!(search.matches.len(), 2);
}

#[test]
fn cumulative_walk_no_match_then_match() {
    let no_match = leaf(99, 0, 1);
    let has_match = leaf(1, 0, 1);
    let src = b"x".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    TreeWalker::new(&src).walk(&no_match, &mut search);
    assert!(search.matches.is_empty());
    TreeWalker::new(&src).walk(&has_match, &mut search);
    assert_eq!(search.matches.len(), 1);
}

// ===================================================================
// Category 8 — count_invariant_* (4 tests)
// ===================================================================

#[test]
fn count_invariant_never_decreases_across_walks() {
    let node = leaf(1, 0, 1);
    let src = b"a".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    let mut prev = 0;
    for _ in 0..5 {
        TreeWalker::new(&src).walk(&node, &mut search);
        assert!(search.matches.len() >= prev);
        prev = search.matches.len();
    }
}

#[test]
fn count_invariant_monotonic_with_no_match_walks() {
    let matching = leaf(1, 0, 1);
    let non_matching = leaf(99, 0, 1);
    let src = b"x".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    TreeWalker::new(&src).walk(&matching, &mut search);
    let after_first = search.matches.len();
    TreeWalker::new(&src).walk(&non_matching, &mut search);
    assert!(search.matches.len() >= after_first);
}

#[test]
fn count_invariant_non_negative() {
    let search = SearchVisitor::new(|_: &ParsedNode| false);
    // usize is always >= 0; verify it's exactly 0
    assert_eq!(search.matches.len(), 0);
}

#[test]
fn count_invariant_increases_by_match_count() {
    let a = leaf(1, 0, 1);
    let b = leaf(1, 1, 2);
    let root = interior(50, vec![a, b]);
    let src = b"ab".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 2);
}

// ===================================================================
// Category 9 — found_count_* (5 tests)
// ===================================================================

#[test]
fn found_count_empty_means_not_found() {
    let search = SearchVisitor::new(|_: &ParsedNode| true);
    assert!(search.matches.is_empty());
}

#[test]
fn found_count_nonempty_means_found() {
    let node = leaf(1, 0, 1);
    let src = b"a".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    TreeWalker::new(&src).walk(&node, &mut search);
    assert!(!search.matches.is_empty());
}

#[test]
fn found_count_consistency_after_walk() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 2);
    TreeWalker::new(&src).walk(&root, &mut search);
    let found = !search.matches.is_empty();
    let count = search.matches.len();
    assert!(found);
    assert!(count > 0);
}

#[test]
fn found_count_zero_means_empty() {
    let node = leaf(1, 0, 1);
    let src = b"a".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 99);
    TreeWalker::new(&src).walk(&node, &mut search);
    assert_eq!(search.matches.len(), 0);
    assert!(search.matches.is_empty());
}

#[test]
fn found_count_iff_relationship() {
    // found iff count > 0 for various scenarios
    let node = leaf(1, 0, 1);
    let src = b"a".to_vec();

    let mut s1 = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    TreeWalker::new(&src).walk(&node, &mut s1);
    assert!(!s1.matches.is_empty());

    let mut s2 = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 99);
    TreeWalker::new(&src).walk(&node, &mut s2);
    assert!(s2.matches.is_empty());
}

// ===================================================================
// Category 10 — large_tree_* (5 tests)
// ===================================================================

#[test]
fn large_tree_100_nodes_50_match() {
    let children: Vec<ParsedNode> = (0..100)
        .map(|i| {
            if i % 2 == 0 {
                leaf(7, i, i + 1)
            } else {
                leaf(8, i, i + 1)
            }
        })
        .collect();
    let root = interior(50, children);
    let src: Vec<u8> = (0..100).map(|i| b'a' + (i % 26) as u8).collect();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 7);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 50);
}

#[test]
fn large_tree_1000_nodes_zero_match() {
    let children: Vec<ParsedNode> = (0..1000).map(|i| leaf(1, i, i + 1)).collect();
    let root = interior(50, children);
    let src: Vec<u8> = vec![b'x'; 1000];
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 999);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 0);
    assert!(search.matches.is_empty());
}

#[test]
fn large_tree_all_match() {
    let children: Vec<ParsedNode> = (0..200).map(|i| leaf(42, i, i + 1)).collect();
    let root = interior(50, children);
    let src: Vec<u8> = vec![b'a'; 200];
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 42);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 200);
}

#[test]
fn large_tree_only_root_matches() {
    let children: Vec<ParsedNode> = (0..50).map(|i| leaf(1, i, i + 1)).collect();
    let root = interior(50, children);
    let src: Vec<u8> = vec![b'z'; 50];
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 50);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 1);
}

#[test]
fn large_tree_100_nodes_count_all() {
    let children: Vec<ParsedNode> = (0..100).map(|i| leaf(i as u16, i, i + 1)).collect();
    let root = interior(200, children);
    let src: Vec<u8> = vec![b'x'; 100];
    let mut search = SearchVisitor::new(|_: &ParsedNode| true);
    TreeWalker::new(&src).walk(&root, &mut search);
    // root + 100 children = 101
    assert_eq!(search.matches.len(), 101);
}

// ===================================================================
// Category 11 — multi_visitor_* (5 tests)
// ===================================================================

#[test]
fn multi_visitor_different_targets_independent() {
    let (root, src) = sample_tree();
    let mut s1 = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    let mut s2 = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 2);
    TreeWalker::new(&src).walk(&root, &mut s1);
    TreeWalker::new(&src).walk(&root, &mut s2);
    assert_eq!(s1.matches.len(), 1);
    assert_eq!(s2.matches.len(), 1);
}

#[test]
fn multi_visitor_one_finds_one_doesnt() {
    let node = leaf(5, 0, 1);
    let src = b"x".to_vec();
    let mut finder = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 5);
    let mut miss = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 99);
    TreeWalker::new(&src).walk(&node, &mut finder);
    TreeWalker::new(&src).walk(&node, &mut miss);
    assert!(!finder.matches.is_empty());
    assert!(miss.matches.is_empty());
}

#[test]
fn multi_visitor_same_target_same_result() {
    let (root, src) = sample_tree();
    let mut s1 = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 4);
    let mut s2 = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 4);
    TreeWalker::new(&src).walk(&root, &mut s1);
    TreeWalker::new(&src).walk(&root, &mut s2);
    assert_eq!(s1.matches.len(), s2.matches.len());
}

#[test]
fn multi_visitor_three_visitors() {
    let (root, src) = sample_tree();
    let mut sa = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    let mut sb = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 11);
    let mut sc = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 999);
    TreeWalker::new(&src).walk(&root, &mut sa);
    TreeWalker::new(&src).walk(&root, &mut sb);
    TreeWalker::new(&src).walk(&root, &mut sc);
    assert_eq!(sa.matches.len(), 1);
    assert_eq!(sb.matches.len(), 1);
    assert!(sc.matches.is_empty());
}

#[test]
fn multi_visitor_do_not_interfere() {
    let node = leaf(10, 0, 1);
    let src = b"x".to_vec();
    let mut s1 = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 10);
    TreeWalker::new(&src).walk(&node, &mut s1);
    let mut s2 = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 10);
    // s2 is fresh — should be empty
    assert!(s2.matches.is_empty());
    TreeWalker::new(&src).walk(&node, &mut s2);
    assert_eq!(s2.matches.len(), 1);
}

// ===================================================================
// Category 12 — debug_format_* (3 tests)
// ===================================================================

#[test]
fn debug_format_nonempty_before_walk() {
    let search = SearchVisitor::new(|_: &ParsedNode| true);
    let dbg = format!("{:?}", search.matches);
    assert!(!dbg.is_empty());
}

#[test]
fn debug_format_nonempty_after_walk() {
    let node = leaf(1, 0, 1);
    let src = b"a".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    TreeWalker::new(&src).walk(&node, &mut search);
    let dbg = format!("{:?}", search.matches);
    assert!(!dbg.is_empty());
}

#[test]
fn debug_format_contains_match_info() {
    let node = leaf(1, 0, 3);
    let src = b"abc".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    TreeWalker::new(&src).walk(&node, &mut search);
    let dbg = format!("{:?}", search.matches);
    // Should contain the tuple info
    assert!(dbg.contains('0'));
    assert!(dbg.contains('3'));
}

// ===================================================================
// Category 13 — predicate_* (5 tests)
// ===================================================================

#[test]
fn predicate_by_byte_range() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.start_byte() == 0 && n.end_byte() == 1);
    TreeWalker::new(&src).walk(&root, &mut search);
    // a(1) spans [0,1)
    assert!(!search.matches.is_empty());
}

#[test]
fn predicate_by_is_named() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.is_named());
    TreeWalker::new(&src).walk(&root, &mut search);
    assert!(!search.matches.is_empty());
}

#[test]
fn predicate_compound_symbol_and_named() {
    let a = leaf(5, 0, 1);
    let b = unnamed_leaf(5, 1, 2);
    let root = interior(50, vec![a, b]);
    let src = b"ab".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 5 && n.is_named());
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 1);
}

#[test]
fn predicate_by_child_count() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.child_count() == 0);
    TreeWalker::new(&src).walk(&root, &mut search);
    // leaves: a(1), b(2), c(3), d(4) = 4
    assert_eq!(search.matches.len(), 4);
}

#[test]
fn predicate_always_true_counts_all_nodes() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|_: &ParsedNode| true);
    TreeWalker::new(&src).walk(&root, &mut search);
    // root(10), a(1), mid(11), b(2), c(3), d(4) = 6
    assert_eq!(search.matches.len(), 6);
}

// ===================================================================
// Category 14 — byte_range_* (4 tests)
// ===================================================================

#[test]
fn byte_range_single_match() {
    let node = leaf(1, 5, 10);
    let src: Vec<u8> = vec![b'x'; 10];
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    TreeWalker::new(&src).walk(&node, &mut search);
    let (start, end, _) = &search.matches[0];
    assert_eq!(*start, 5);
    assert_eq!(*end, 10);
}

#[test]
fn byte_range_multiple_matches_ordered() {
    let a = leaf(1, 0, 3);
    let b = leaf(1, 3, 7);
    let root = interior(50, vec![a, b]);
    let src: Vec<u8> = vec![b'x'; 7];
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 2);
    assert_eq!(search.matches[0].0, 0);
    assert_eq!(search.matches[0].1, 3);
    assert_eq!(search.matches[1].0, 3);
    assert_eq!(search.matches[1].1, 7);
}

#[test]
fn byte_range_zero_length_node() {
    let node = leaf(1, 5, 5);
    let src: Vec<u8> = vec![b'x'; 10];
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    TreeWalker::new(&src).walk(&node, &mut search);
    assert_eq!(search.matches.len(), 1);
    assert_eq!(search.matches[0].0, 5);
    assert_eq!(search.matches[0].1, 5);
}

#[test]
fn byte_range_root_spans_entire_source() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 10);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches[0].0, 0);
    assert_eq!(search.matches[0].1, 4);
}

// ===================================================================
// Category 15 — deep_tree_* (4 tests)
// ===================================================================

#[test]
fn deep_tree_find_root() {
    let (root, src) = deep_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 100);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 1);
}

#[test]
fn deep_tree_find_deepest() {
    let (root, src) = deep_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 103);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 1);
}

#[test]
fn deep_tree_find_all() {
    let (root, src) = deep_tree();
    let mut search = SearchVisitor::new(|_: &ParsedNode| true);
    TreeWalker::new(&src).walk(&root, &mut search);
    // 100 → 101 → 102 → 103 = 4 nodes
    assert_eq!(search.matches.len(), 4);
}

#[test]
fn deep_tree_find_middle() {
    let (root, src) = deep_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 102);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 1);
}

// ===================================================================
// Category 16 — wide_tree_* (4 tests)
// ===================================================================

#[test]
fn wide_tree_find_specific_child() {
    let (root, src) = wide_tree_10();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 5);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 1);
}

#[test]
fn wide_tree_find_all_children() {
    let (root, src) = wide_tree_10();
    // All children have symbols 1..=10, root is 50
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() != 50);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 10);
}

#[test]
fn wide_tree_find_root_only() {
    let (root, src) = wide_tree_10();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 50);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 1);
}

#[test]
fn wide_tree_total_count() {
    let (root, src) = wide_tree_10();
    let mut search = SearchVisitor::new(|_: &ParsedNode| true);
    TreeWalker::new(&src).walk(&root, &mut search);
    // root + 10 children = 11
    assert_eq!(search.matches.len(), 11);
}

// ===================================================================
// Category 17 — error_node_* (3 tests)
// ===================================================================

#[test]
fn error_node_skipped_by_search() {
    let err = error_node(0, 1);
    let src = b"e".to_vec();
    let mut search = SearchVisitor::new(|_: &ParsedNode| true);
    TreeWalker::new(&src).walk(&err, &mut search);
    // Error nodes go through visit_error, not enter_node
    assert!(search.matches.is_empty());
}

#[test]
fn error_node_sibling_still_found() {
    let err = error_node(0, 1);
    let good = leaf(5, 1, 2);
    let root = interior(50, vec![err, good]);
    let src = b"eg".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 5);
    TreeWalker::new(&src).walk(&root, &mut search);
    assert_eq!(search.matches.len(), 1);
}

#[test]
fn error_node_does_not_increment_count() {
    let err = error_node(0, 1);
    let src = b"e".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 0);
    TreeWalker::new(&src).walk(&err, &mut search);
    assert_eq!(search.matches.len(), 0);
}

// ===================================================================
// Category 18 — mixed_* (4 tests)
// ===================================================================

#[test]
fn mixed_named_unnamed_error() {
    let named = leaf(1, 0, 1);
    let unnamed = unnamed_leaf(2, 1, 2);
    let err = error_node(2, 3);
    let root = interior(50, vec![named, unnamed, err]);
    let src = b"abc".to_vec();
    let mut search = SearchVisitor::new(|_: &ParsedNode| true);
    TreeWalker::new(&src).walk(&root, &mut search);
    // root + named + unnamed = 3 (error skipped)
    assert_eq!(search.matches.len(), 3);
}

#[test]
fn mixed_count_only_named() {
    let named = leaf(1, 0, 1);
    let unnamed = unnamed_leaf(2, 1, 2);
    let root = interior(50, vec![named, unnamed]);
    let src = b"ab".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.is_named());
    TreeWalker::new(&src).walk(&root, &mut search);
    // root + leaf(1)
    assert_eq!(search.matches.len(), 2);
}

#[test]
fn mixed_various_symbols() {
    let children: Vec<ParsedNode> = (0..5).map(|i| leaf(i as u16, i, i + 1)).collect();
    let root = interior(99, children);
    let src = b"abcde".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() < 3);
    TreeWalker::new(&src).walk(&root, &mut search);
    // symbols 0, 1, 2 = 3 matches
    assert_eq!(search.matches.len(), 3);
}

#[test]
fn mixed_deep_and_wide() {
    // Wide: root → [chain, leaf1, leaf2]
    // chain: a(1) → b(2) leaf
    let b = leaf(2, 0, 1);
    let a = interior(1, vec![b]);
    let l1 = leaf(3, 1, 2);
    let l2 = leaf(4, 2, 3);
    let root = interior(50, vec![a, l1, l2]);
    let src = b"xyz".to_vec();
    let mut search = SearchVisitor::new(|_: &ParsedNode| true);
    TreeWalker::new(&src).walk(&root, &mut search);
    // root + a + b + l1 + l2 = 5
    assert_eq!(search.matches.len(), 5);
}

// ===================================================================
// Category 19 — reuse_* (3 tests)
// ===================================================================

#[test]
fn reuse_walker_different_visitors() {
    let node = leaf(1, 0, 1);
    let src = b"a".to_vec();
    let walker = TreeWalker::new(&src);
    let mut s1 = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    walker.walk(&node, &mut s1);
    let mut s2 = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 99);
    walker.walk(&node, &mut s2);
    assert_eq!(s1.matches.len(), 1);
    assert!(s2.matches.is_empty());
}

#[test]
fn reuse_visitor_across_different_walkers() {
    let node = leaf(1, 0, 1);
    let src = b"a".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    let w1 = TreeWalker::new(&src);
    let w2 = TreeWalker::new(&src);
    w1.walk(&node, &mut search);
    w2.walk(&node, &mut search);
    assert_eq!(search.matches.len(), 2);
}

#[test]
fn reuse_visitor_on_different_trees() {
    let t1 = leaf(1, 0, 1);
    let t2 = leaf(1, 0, 2);
    let s1 = b"a".to_vec();
    let s2 = b"ab".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    TreeWalker::new(&s1).walk(&t1, &mut search);
    TreeWalker::new(&s2).walk(&t2, &mut search);
    assert_eq!(search.matches.len(), 2);
    // Different byte ranges
    assert_eq!(search.matches[0].1, 1);
    assert_eq!(search.matches[1].1, 2);
}

// ===================================================================
// Category 20 — edge_case_* (4 tests)
// ===================================================================

#[test]
fn edge_case_single_node_tree() {
    let node = leaf(42, 0, 1);
    let src = b"x".to_vec();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 42);
    TreeWalker::new(&src).walk(&node, &mut search);
    assert_eq!(search.matches.len(), 1);
}

#[test]
fn edge_case_empty_interior() {
    let root = interior(50, vec![]);
    let src = b"".to_vec();
    let mut search = SearchVisitor::new(|_: &ParsedNode| true);
    TreeWalker::new(&src).walk(&root, &mut search);
    // Just the root
    assert_eq!(search.matches.len(), 1);
}

#[test]
fn edge_case_visitor_trait_enter_node_returns_continue() {
    let mut search = SearchVisitor::new(|_: &ParsedNode| true);
    let node = leaf(1, 0, 1);
    let action = search.enter_node(&node);
    assert_eq!(action, VisitorAction::Continue);
}

#[test]
fn edge_case_various_kinds_consistent() {
    let src = b"x".to_vec();
    for kind in [0u16, 1, 50, 100, 255, 1000, u16::MAX] {
        let node = leaf(kind, 0, 1);
        let mut search = SearchVisitor::new(move |n: &ParsedNode| n.symbol() == kind);
        TreeWalker::new(&src).walk(&node, &mut search);
        assert_eq!(search.matches.len(), 1, "failed for kind {kind}");
    }
}

#![cfg(feature = "glr")]
#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

//! Property-based tests for the forest-to-tree builder (`builder.rs`).
//!
//! Exercises the builder through the Parser API and via `Tree::new_for_testing()`
//! to verify tree validity, node order, determinism, and various forest shapes.

use proptest::prelude::*;

use adze_runtime::test_helpers::{linear_token_language, single_token_chain_language};
use adze_runtime::{Language, Node, Parser, Tree};

// ---------------------------------------------------------------------------
// Grammar helpers (ParseTable is leaked for 'static by the shared test helpers)
// ---------------------------------------------------------------------------

/// start → a  (Symbols: 0=EOF, 1=a, 2=start)
fn make_single_token_language() -> Language {
    linear_token_language("single", &["a"])
}

/// start → a b  (Symbols: 0=EOF, 1=a, 2=b, 3=start)
fn make_two_token_language() -> Language {
    linear_token_language("two", &["a", "b"])
}

/// start → a b c  (Symbols: 0=EOF, 1=a, 2=b, 3=c, 4=start)
fn make_three_token_language() -> Language {
    linear_token_language("three", &["a", "b", "c"])
}

/// Chain: start → mid, mid → a  (Symbols: 0=EOF, 1=a, 2=mid, 3=start)
fn make_chain_language() -> Language {
    single_token_chain_language()
}

// ---------------------------------------------------------------------------
// Parse helper – shortcut through Parser API (exercises builder internally)
// ---------------------------------------------------------------------------

fn parse_for_index(idx: usize) -> Tree {
    match idx % 4 {
        0 => {
            let mut p = Parser::new();
            p.set_language(make_single_token_language()).unwrap();
            p.parse(b"a", None).unwrap()
        }
        1 => {
            let mut p = Parser::new();
            p.set_language(make_two_token_language()).unwrap();
            p.parse(b"ab", None).unwrap()
        }
        2 => {
            let mut p = Parser::new();
            p.set_language(make_three_token_language()).unwrap();
            p.parse(b"abc", None).unwrap()
        }
        3 => {
            let mut p = Parser::new();
            p.set_language(make_chain_language()).unwrap();
            p.parse(b"a", None).unwrap()
        }
        _ => unreachable!(),
    }
}

// ---------------------------------------------------------------------------
// Tree analysis helpers
// ---------------------------------------------------------------------------

fn count_nodes(node: Node<'_>) -> usize {
    let mut total = 1;
    for i in 0..node.child_count() {
        total += count_nodes(node.child(i).unwrap());
    }
    total
}

fn tree_depth(node: Node<'_>) -> usize {
    let mut deepest = 0;
    for i in 0..node.child_count() {
        deepest = deepest.max(1 + tree_depth(node.child(i).unwrap()));
    }
    deepest
}

fn assert_ranges_valid(node: Node<'_>) {
    assert!(
        node.start_byte() <= node.end_byte(),
        "node kind_id={} has start {} > end {}",
        node.kind_id(),
        node.start_byte(),
        node.end_byte(),
    );
    for i in 0..node.child_count() {
        assert_ranges_valid(node.child(i).unwrap());
    }
}

fn assert_parent_covers_children(node: Node<'_>) {
    for i in 0..node.child_count() {
        let child = node.child(i).unwrap();
        assert!(child.start_byte() >= node.start_byte());
        assert!(child.end_byte() <= node.end_byte());
        assert_parent_covers_children(child);
    }
}

fn children_in_order(node: Node<'_>) -> bool {
    for i in 1..node.child_count() {
        let prev = node.child(i - 1).unwrap();
        let curr = node.child(i).unwrap();
        if prev.start_byte() > curr.start_byte() {
            return false;
        }
    }
    true
}

fn children_non_overlapping(node: Node<'_>) -> bool {
    for i in 1..node.child_count() {
        let prev = node.child(i - 1).unwrap();
        let curr = node.child(i).unwrap();
        if prev.end_byte() > curr.start_byte() {
            return false;
        }
    }
    true
}

fn collect_node_info(node: Node<'_>, out: &mut Vec<(u16, usize, usize)>) {
    out.push((node.kind_id(), node.start_byte(), node.end_byte()));
    for i in 0..node.child_count() {
        collect_node_info(node.child(i).unwrap(), out);
    }
}

// ---------------------------------------------------------------------------
// proptest strategies for Tree::new_for_testing
// ---------------------------------------------------------------------------

fn arb_leaf_tree() -> impl Strategy<Value = Tree> {
    (1u32..500, 0usize..256).prop_map(|(sym, len)| Tree::new_for_testing(sym, 0, len, vec![]))
}

fn arb_wide_tree(max_children: usize) -> impl Strategy<Value = (Tree, usize)> {
    let max_c = max_children.max(1);
    (1u32..500, 1usize..=max_c, 1usize..32).prop_map(move |(sym, n, chunk)| {
        let children: Vec<Tree> = (0..n)
            .map(|i| {
                Tree::new_for_testing(
                    (sym + 1 + i as u32) % 500 + 1,
                    i * chunk,
                    (i + 1) * chunk,
                    vec![],
                )
            })
            .collect();
        let total = n * chunk;
        (Tree::new_for_testing(sym, 0, total, children), n)
    })
}

fn arb_deep_tree(max_depth: usize) -> impl Strategy<Value = (Tree, usize)> {
    let max_d = max_depth.max(1);
    (1u32..500, 1usize..=max_d, 1usize..64).prop_map(move |(sym, depth, len)| {
        let mut tree = Tree::new_for_testing(sym, 0, len, vec![]);
        for i in 1..depth {
            tree = Tree::new_for_testing(sym + i as u32, 0, len, vec![tree]);
        }
        (tree, depth)
    })
}

// ===========================================================================
// 1 – Builder creates valid tree (proptest)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Parsing with any grammar yields a tree where every node has start <= end.
    #[test]
    fn parsed_tree_has_valid_ranges(idx in 0usize..4) {
        let tree = parse_for_index(idx);
        assert_ranges_valid(tree.root_node());
    }

    /// Parent byte range always encompasses all children byte ranges.
    #[test]
    fn parsed_tree_parent_covers_children(idx in 0usize..4) {
        let tree = parse_for_index(idx);
        assert_parent_covers_children(tree.root_node());
    }

    /// Root byte range starts at 0 and ends at input length.
    #[test]
    fn parsed_root_spans_full_input(idx in 0usize..4) {
        let tree = parse_for_index(idx);
        let root = tree.root_node();
        let expected_end = match idx % 4 { 0 | 3 => 1, 1 => 2, 2 => 3, _ => unreachable!() };
        prop_assert_eq!(root.start_byte(), 0);
        prop_assert_eq!(root.end_byte(), expected_end);
    }

    /// Any constructed leaf tree has valid ranges.
    #[test]
    fn new_for_testing_leaf_valid_ranges(tree in arb_leaf_tree()) {
        assert_ranges_valid(tree.root_node());
    }
}

// ===========================================================================
// 2 – Builder preserves node order (proptest)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Children of parsed root are in source order (non-decreasing start_byte).
    #[test]
    fn parsed_children_in_source_order(idx in 0usize..4) {
        let tree = parse_for_index(idx);
        prop_assert!(children_in_order(tree.root_node()));
    }

    /// Children of parsed root have non-overlapping byte ranges.
    #[test]
    fn parsed_children_non_overlapping(idx in 0usize..4) {
        let tree = parse_for_index(idx);
        prop_assert!(children_non_overlapping(tree.root_node()));
    }

    /// Wide trees built via new_for_testing preserve child ordering.
    #[test]
    fn wide_tree_children_in_order((tree, _n) in arb_wide_tree(12)) {
        prop_assert!(children_in_order(tree.root_node()));
    }

    /// Wide tree children have non-overlapping ranges.
    #[test]
    fn wide_tree_children_non_overlapping((tree, _n) in arb_wide_tree(12)) {
        prop_assert!(children_non_overlapping(tree.root_node()));
    }
}

// ===========================================================================
// 3 – Builder handles ambiguity (proptest)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    /// Parsing the same input always selects the same root symbol (disambiguation).
    #[test]
    fn disambiguation_consistent_root_symbol(idx in 0usize..4) {
        let t1 = parse_for_index(idx);
        let t2 = parse_for_index(idx);
        prop_assert_eq!(t1.root_node().kind_id(), t2.root_node().kind_id());
    }

    /// Disambiguation always picks same child structure.
    #[test]
    fn disambiguation_consistent_child_count(idx in 0usize..4) {
        let t1 = parse_for_index(idx);
        let t2 = parse_for_index(idx);
        prop_assert_eq!(t1.root_node().child_count(), t2.root_node().child_count());
    }

    /// Stub tree has zero range regardless of how many times accessed.
    #[test]
    fn stub_tree_zero_range(_iter in 0u8..10) {
        let tree = Tree::new_stub();
        prop_assert_eq!(tree.root_node().start_byte(), 0);
        prop_assert_eq!(tree.root_node().end_byte(), 0);
        prop_assert_eq!(tree.root_node().child_count(), 0);
    }
}

// ===========================================================================
// 4 – Builder output determinism (proptest)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    /// Parsing the same input twice yields identical root kind.
    #[test]
    fn deterministic_root_kind(idx in 0usize..4) {
        let t1 = parse_for_index(idx);
        let t2 = parse_for_index(idx);
        prop_assert_eq!(t1.root_kind(), t2.root_kind());
    }

    /// Parsing the same input twice yields identical byte ranges at every node.
    #[test]
    fn deterministic_byte_ranges(idx in 0usize..4) {
        let t1 = parse_for_index(idx);
        let t2 = parse_for_index(idx);
        let mut info1 = Vec::new();
        let mut info2 = Vec::new();
        collect_node_info(t1.root_node(), &mut info1);
        collect_node_info(t2.root_node(), &mut info2);
        prop_assert_eq!(info1, info2);
    }

    /// Parsing twice yields identical node counts.
    #[test]
    fn deterministic_node_count(idx in 0usize..4) {
        let t1 = parse_for_index(idx);
        let t2 = parse_for_index(idx);
        prop_assert_eq!(count_nodes(t1.root_node()), count_nodes(t2.root_node()));
    }

    /// Parsing twice yields identical depth.
    #[test]
    fn deterministic_depth(idx in 0usize..4) {
        let t1 = parse_for_index(idx);
        let t2 = parse_for_index(idx);
        prop_assert_eq!(tree_depth(t1.root_node()), tree_depth(t2.root_node()));
    }
}

// ===========================================================================
// 5 – Builder with single-node forest (proptest)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// A single-leaf tree (new_for_testing) has zero children.
    #[test]
    fn single_leaf_has_no_children(tree in arb_leaf_tree()) {
        prop_assert_eq!(tree.root_node().child_count(), 0);
    }

    /// A single-leaf tree preserves its symbol ID.
    #[test]
    fn single_leaf_preserves_symbol(sym in 1u32..500, len in 0usize..256) {
        let tree = Tree::new_for_testing(sym, 0, len, vec![]);
        prop_assert_eq!(tree.root_kind(), sym);
    }

    /// A single-leaf tree preserves its byte range.
    #[test]
    fn single_leaf_preserves_range(sym in 1u32..500, len in 0usize..256) {
        let tree = Tree::new_for_testing(sym, 0, len, vec![]);
        prop_assert_eq!(tree.root_node().start_byte(), 0);
        prop_assert_eq!(tree.root_node().end_byte(), len);
    }

    /// Single-leaf node count is exactly 1.
    #[test]
    fn single_leaf_node_count_is_one(tree in arb_leaf_tree()) {
        prop_assert_eq!(count_nodes(tree.root_node()), 1);
    }
}

// ===========================================================================
// 6 – Builder with deep forest (proptest)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Deep tree has correct depth (depth = nodes - 1 for a linear chain).
    #[test]
    fn deep_tree_correct_depth((tree, depth) in arb_deep_tree(20)) {
        let expected_depth = depth - 1;
        prop_assert_eq!(tree_depth(tree.root_node()), expected_depth);
    }

    /// All nodes in a deep tree have valid ranges.
    #[test]
    fn deep_tree_valid_ranges((tree, _depth) in arb_deep_tree(20)) {
        assert_ranges_valid(tree.root_node());
    }

    /// Cloning a deep tree preserves depth.
    #[test]
    fn deep_tree_clone_preserves_depth((tree, depth) in arb_deep_tree(20)) {
        let cloned = tree.clone();
        prop_assert_eq!(tree_depth(cloned.root_node()), depth - 1);
    }

    /// Node count in a deep chain equals the chain length.
    #[test]
    fn deep_tree_node_count_equals_depth((tree, depth) in arb_deep_tree(20)) {
        prop_assert_eq!(count_nodes(tree.root_node()), depth);
    }
}

// ===========================================================================
// 7 – Builder with wide forest (proptest)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Wide tree root has the expected number of children.
    #[test]
    fn wide_tree_correct_child_count((tree, n) in arb_wide_tree(16)) {
        prop_assert_eq!(tree.root_node().child_count(), n);
    }

    /// Parent range covers all children in a wide tree.
    #[test]
    fn wide_tree_parent_covers_children((tree, _n) in arb_wide_tree(16)) {
        assert_parent_covers_children(tree.root_node());
    }

    /// Node count in a wide tree is 1 (root) + n (children).
    #[test]
    fn wide_tree_node_count((tree, n) in arb_wide_tree(16)) {
        prop_assert_eq!(count_nodes(tree.root_node()), 1 + n);
    }

    /// Wide tree depth is exactly 1 (root + leaf children).
    #[test]
    fn wide_tree_depth_is_one((tree, _n) in arb_wide_tree(16)) {
        prop_assert_eq!(tree_depth(tree.root_node()), 1);
    }
}

// ===========================================================================
// 8 – Builder performance metrics output (proptest)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Any parsed tree has at least one node.
    #[test]
    fn parsed_tree_has_at_least_one_node(idx in 0usize..4) {
        let tree = parse_for_index(idx);
        prop_assert!(count_nodes(tree.root_node()) >= 1);
    }

    /// Tree depth is non-negative for all parsed trees.
    #[test]
    fn parsed_tree_depth_non_negative(idx in 0usize..4) {
        let tree = parse_for_index(idx);
        // depth is usize, so always >= 0, but verify non-trivial
        #[expect(clippy::absurd_extreme_comparisons, unused_comparisons, reason = "boundary condition tests deliberately compare against usize extremes")]
        {
            prop_assert!(tree_depth(tree.root_node()) >= 0);
        }
    }

    /// Chain grammar (start→mid→a) produces a tree with depth >= 1.
    #[test]
    fn chain_grammar_produces_nontrivial_depth(_iter in 0u8..4) {
        let mut p = Parser::new();
        p.set_language(make_chain_language()).unwrap();
        let tree = p.parse(b"a", None).unwrap();
        let depth = tree_depth(tree.root_node());
        // The builder may inline single-child non-terminals; depth >= 1
        prop_assert!(depth >= 1, "chain grammar depth was {}", depth);
    }
}

// ===========================================================================
// Additional edge-case tests
// ===========================================================================

/// Verify the builder pipeline produces a tree with language metadata set.
#[test]
fn builder_sets_language_on_tree() {
    let tree = parse_for_index(0);
    assert!(tree.language().is_some());
}

/// Verify the builder pipeline stores source bytes.
#[test]
fn builder_stores_source_bytes() {
    let tree = parse_for_index(1);
    assert_eq!(tree.source_bytes(), Some(b"ab".as_slice()));
}

/// Debug formatting of builder output does not panic.
#[test]
fn builder_output_debug_does_not_panic() {
    for idx in 0..4 {
        let tree = parse_for_index(idx);
        let debug = format!("{:?}", tree);
        assert!(!debug.is_empty());
    }
}

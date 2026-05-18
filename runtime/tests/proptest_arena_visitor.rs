//! Property-based tests for arena allocator and visitor modules.
//!
//! Tests properties of `TreeArena`, `NodeHandle`, `TreeNode`,
//! `VisitorAction`, `StatsVisitor`, `SearchVisitor`, `TreeWalker`,
//! and `BreadthFirstWalker`.

use adze::arena_allocator::{NodeHandle, TreeArena, TreeNode};
use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, SearchVisitor, StatsVisitor, TreeWalker, Visitor, VisitorAction,
};
use proptest::prelude::*;
use proptest::strategy::ValueTree;
use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Helpers for ParsedNode construction (language field is pub(crate))
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

fn pn_leaf(symbol: u16, start: usize, end: usize) -> ParsedNode {
    make_node(symbol, vec![], start, end, false, true)
}

fn pn_interior(symbol: u16, children: Vec<ParsedNode>) -> ParsedNode {
    let start = children.first().map_or(0, |c| c.start_byte());
    let end = children.last().map_or(0, |c| c.end_byte());
    make_node(symbol, children, start, end, false, true)
}

fn pn_error(start: usize, end: usize) -> ParsedNode {
    make_node(0, vec![], start, end, true, false)
}

fn count_non_error(node: &ParsedNode) -> usize {
    if node.is_error() {
        0
    } else {
        1 + node.children().iter().map(count_non_error).sum::<usize>()
    }
}

fn collect_dfs_symbols(node: &ParsedNode) -> Vec<u16> {
    let mut out = vec![node.symbol()];
    for child in node.children() {
        out.extend(collect_dfs_symbols(child));
    }
    out
}

fn collect_bfs_symbols(node: &ParsedNode) -> Vec<u16> {
    let mut out = Vec::new();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(node);
    while let Some(n) = queue.pop_front() {
        out.push(n.symbol());
        for child in n.children() {
            queue.push_back(child);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Proptest strategies
// ---------------------------------------------------------------------------

const SOURCE_LEN: usize = 64;

fn arb_capacity() -> impl Strategy<Value = usize> {
    1usize..=1000
}

fn arb_node_count() -> impl Strategy<Value = usize> {
    1usize..=100
}

fn arb_leaf_value() -> impl Strategy<Value = i32> {
    -1000i32..=1000
}

fn arb_parsed_leaf() -> impl Strategy<Value = ParsedNode> {
    (1u16..=10, 0..SOURCE_LEN - 1).prop_map(|(sym, start)| pn_leaf(sym, start, start + 1))
}

fn arb_parsed_tree(max_depth: u32, max_width: usize) -> impl Strategy<Value = ParsedNode> {
    arb_parsed_leaf().prop_recursive(max_depth, 64, max_width as u32, move |inner| {
        (1u16..=10, proptest::collection::vec(inner, 1..=max_width))
            .prop_map(|(sym, children)| pn_interior(sym, children))
    })
}

fn arb_source() -> impl Strategy<Value = String> {
    proptest::string::string_regex(&format!("[a-z0-9 ]{{{SOURCE_LEN},{SOURCE_LEN}}}")).unwrap()
}

// ===========================================================================
// Arena property tests
// ===========================================================================

// 1. Arena with capacity N can hold at least N nodes
proptest! {
    #[test]
    fn arena_capacity_holds_n_nodes(cap in arb_capacity()) {
        let mut arena = TreeArena::with_capacity(cap);
        for i in 0..cap {
            arena.alloc(TreeNode::leaf(i as i32));
        }
        prop_assert!(arena.len() >= cap);
    }
}

// 2. Every node added to arena can be retrieved
proptest! {
    #[test]
    fn arena_every_node_retrievable(values in proptest::collection::vec(arb_leaf_value(), 1..100)) {
        let mut arena = TreeArena::new();
        let handles: Vec<_> = values.iter().map(|&v| arena.alloc(TreeNode::leaf(v))).collect();
        for (handle, &expected) in handles.iter().zip(values.iter()) {
            prop_assert_eq!(arena.get(*handle).value(), expected);
        }
    }
}

// 3. Arena node count is monotonically increasing
proptest! {
    #[test]
    fn arena_len_monotonically_increasing(n in arb_node_count()) {
        let mut arena = TreeArena::new();
        let mut prev = arena.len();
        for i in 0..n {
            arena.alloc(TreeNode::leaf(i as i32));
            let cur = arena.len();
            prop_assert!(cur > prev || (prev == 0 && cur == 1),
                "len must increase: prev={}, cur={}", prev, cur);
            prev = cur;
        }
    }
}

// 10. Arena with capacity 1 works (minimal case)
proptest! {
    #[test]
    fn arena_capacity_one_works(n in 1usize..=50) {
        let mut arena = TreeArena::with_capacity(1);
        let mut handles = Vec::new();
        for i in 0..n {
            handles.push(arena.alloc(TreeNode::leaf(i as i32)));
        }
        for (i, h) in handles.iter().enumerate() {
            prop_assert_eq!(arena.get(*h).value(), i as i32);
        }
    }
}

// Arena reset makes it empty
proptest! {
    #[test]
    fn arena_reset_clears(n in arb_node_count()) {
        let mut arena = TreeArena::new();
        for i in 0..n {
            arena.alloc(TreeNode::leaf(i as i32));
        }
        arena.reset();
        prop_assert_eq!(arena.len(), 0);
        prop_assert!(arena.is_empty());
    }
}

// Arena capacity >= len always
proptest! {
    #[test]
    fn arena_capacity_ge_len(n in arb_node_count()) {
        let mut arena = TreeArena::new();
        for i in 0..n {
            arena.alloc(TreeNode::leaf(i as i32));
        }
        prop_assert!(arena.capacity() >= arena.len());
    }
}

// Arena clear makes it empty and reduces chunks
proptest! {
    #[test]
    fn arena_clear_reduces_chunks(cap in 1usize..=10, n in 1usize..=100) {
        let mut arena = TreeArena::with_capacity(cap);
        for i in 0..n {
            arena.alloc(TreeNode::leaf(i as i32));
        }
        arena.clear();
        prop_assert_eq!(arena.len(), 0);
        prop_assert_eq!(arena.num_chunks(), 1);
    }
}

// Arena memory usage is positive after allocation
proptest! {
    #[test]
    fn arena_memory_usage_positive(n in arb_node_count()) {
        let mut arena = TreeArena::new();
        for i in 0..n {
            arena.alloc(TreeNode::leaf(i as i32));
        }
        prop_assert!(arena.memory_usage() > 0);
    }
}

// Arena metrics match direct accessors
proptest! {
    #[test]
    fn arena_metrics_consistent(n in arb_node_count()) {
        let mut arena = TreeArena::new();
        for i in 0..n {
            arena.alloc(TreeNode::leaf(i as i32));
        }
        let m = arena.metrics();
        prop_assert_eq!(m.len(), arena.len());
        prop_assert_eq!(m.capacity(), arena.capacity());
        prop_assert_eq!(m.num_chunks(), arena.num_chunks());
        prop_assert_eq!(m.memory_usage(), arena.memory_usage());
        prop_assert_eq!(m.is_empty(), arena.is_empty());
    }
}

// Arena branch nodes store children correctly
proptest! {
    #[test]
    fn arena_branch_children_preserved(
        child_count in 0usize..=10,
        values in proptest::collection::vec(arb_leaf_value(), 10)
    ) {
        let mut arena = TreeArena::new();
        let child_handles: Vec<_> = values.iter()
            .take(child_count)
            .map(|&v| arena.alloc(TreeNode::leaf(v)))
            .collect();
        let parent = arena.alloc(TreeNode::branch(child_handles.clone()));
        let parent_ref = arena.get(parent);
        let stored = parent_ref.children();
        prop_assert_eq!(stored.len(), child_count);
        for (i, &h) in stored.iter().enumerate() {
            prop_assert_eq!(h, child_handles[i]);
        }
    }
}

// Arena leaf vs branch distinction
proptest! {
    #[test]
    fn arena_leaf_branch_distinction(v in arb_leaf_value()) {
        let mut arena = TreeArena::new();
        let lh = arena.alloc(TreeNode::leaf(v));
        let bh = arena.alloc(TreeNode::branch(vec![lh]));
        prop_assert!(arena.get(lh).is_leaf());
        prop_assert!(!arena.get(lh).is_branch());
        prop_assert!(arena.get(bh).is_branch());
        prop_assert!(!arena.get(bh).is_leaf());
    }
}

// Arena branch_with_symbol preserves symbol
proptest! {
    #[test]
    fn arena_branch_symbol(sym in -100i32..=100, v in arb_leaf_value()) {
        let mut arena = TreeArena::new();
        let child = arena.alloc(TreeNode::leaf(v));
        let parent = arena.alloc(TreeNode::branch_with_symbol(sym, vec![child]));
        prop_assert_eq!(arena.get(parent).symbol(), sym);
    }
}

// Arena mutable access works
proptest! {
    #[test]
    fn arena_mut_set_value(old in arb_leaf_value(), new in arb_leaf_value()) {
        let mut arena = TreeArena::new();
        let h = arena.alloc(TreeNode::leaf(old));
        prop_assert_eq!(arena.get(h).value(), old);
        arena.get_mut(h).set_value(new);
        prop_assert_eq!(arena.get(h).value(), new);
    }
}

// Arena reuse after reset
proptest! {
    #[test]
    fn arena_reuse_after_reset(n in 1usize..=50) {
        let mut arena = TreeArena::with_capacity(4);
        for i in 0..n {
            arena.alloc(TreeNode::leaf(i as i32));
        }
        arena.reset();
        let h = arena.alloc(TreeNode::leaf(999));
        prop_assert_eq!(arena.get(h).value(), 999);
        prop_assert_eq!(arena.len(), 1);
    }
}

// Arena handles are unique
proptest! {
    #[test]
    fn arena_handles_unique(n in 2usize..=100) {
        let mut arena = TreeArena::new();
        let handles: Vec<_> = (0..n).map(|i| arena.alloc(TreeNode::leaf(i as i32))).collect();
        let set: HashSet<_> = handles.iter().copied().collect();
        prop_assert_eq!(set.len(), handles.len());
    }
}

// Arena num_chunks grows with overflow
proptest! {
    #[test]
    fn arena_chunks_grow(cap in 1usize..=5, extra in 1usize..=20) {
        let mut arena = TreeArena::with_capacity(cap);
        for i in 0..(cap + extra) {
            arena.alloc(TreeNode::leaf(i as i32));
        }
        prop_assert!(arena.num_chunks() >= 2);
    }
}

// Arena is_empty only when zero nodes
proptest! {
    #[test]
    fn arena_empty_iff_zero(n in arb_node_count()) {
        let mut arena = TreeArena::new();
        prop_assert!(arena.is_empty());
        for i in 0..n {
            arena.alloc(TreeNode::leaf(i as i32));
        }
        prop_assert!(!arena.is_empty());
    }
}

// Arena default equals new
#[test]
fn arena_default_is_new() {
    let a = TreeArena::default();
    let b = TreeArena::new();
    assert_eq!(a.len(), b.len());
    assert_eq!(a.capacity(), b.capacity());
}

// Arena capacity 0 panics
#[test]
#[should_panic(expected = "Capacity must be > 0")]
fn arena_zero_capacity_panics() {
    let _ = TreeArena::with_capacity(0);
}

// NodeHandle equality
proptest! {
    #[test]
    fn node_handle_equality(ci in 0u32..100, ni in 0u32..100) {
        let h1 = NodeHandle::new(ci, ni);
        let h2 = NodeHandle::new(ci, ni);
        prop_assert_eq!(h1, h2);
    }
}

// NodeHandle inequality
proptest! {
    #[test]
    fn node_handle_inequality(ci1 in 0u32..100, ni1 in 0u32..100, ci2 in 0u32..100, ni2 in 0u32..100) {
        prop_assume!(ci1 != ci2 || ni1 != ni2);
        let h1 = NodeHandle::new(ci1, ni1);
        let h2 = NodeHandle::new(ci2, ni2);
        prop_assert_ne!(h1, h2);
    }
}

// TreeNode leaf value round-trips
proptest! {
    #[test]
    fn tree_node_leaf_value(v in arb_leaf_value()) {
        let n = TreeNode::leaf(v);
        prop_assert_eq!(n.value(), v);
        prop_assert_eq!(n.symbol(), v);
        prop_assert!(n.is_leaf());
        prop_assert!(!n.is_branch());
        prop_assert!(n.children().is_empty());
    }
}

// TreeNode branch stores children
proptest! {
    #[test]
    fn tree_node_branch_children(count in 0usize..=10) {
        let handles: Vec<_> = (0..count).map(|i| NodeHandle::new(0, i as u32)).collect();
        let n = TreeNode::branch(handles.clone());
        prop_assert!(n.is_branch());
        prop_assert!(!n.is_leaf());
        prop_assert_eq!(n.children().len(), count);
    }
}

// ===========================================================================
// Visitor property tests (using ParsedNode-based tree)
// ===========================================================================

// 4. TreeWalker visits all reachable non-error nodes exactly once
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn tree_walker_visits_all_reachable(
        tree in arb_parsed_tree(5, 3),
        src in arb_source(),
    ) {
        let source = src.as_bytes();
        let walker = TreeWalker::new(source);

        struct Counter(usize);
        impl Visitor for Counter {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.0 += 1;
                VisitorAction::Continue
            }
        }

        let mut counter = Counter(0);
        walker.walk(&tree, &mut counter);
        let expected = count_non_error(&tree);
        prop_assert_eq!(counter.0, expected,
            "walker visited {} but expected {}", counter.0, expected);
    }
}

// 5. BreadthFirstWalker visits all reachable nodes
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn bfs_walker_visits_all_reachable(
        tree in arb_parsed_tree(5, 3),
        src in arb_source(),
    ) {
        let source = src.as_bytes();
        let walker = BreadthFirstWalker::new(source);

        struct Counter(usize);
        impl Visitor for Counter {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.0 += 1;
                VisitorAction::Continue
            }
        }

        let mut counter = Counter(0);
        walker.walk(&tree, &mut counter);
        let expected = count_non_error(&tree);
        prop_assert_eq!(counter.0, expected);
    }
}

// DFS and BFS visit the same set of symbols
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn dfs_bfs_same_symbol_set(
        tree in arb_parsed_tree(4, 3),
        src in arb_source(),
    ) {
        let source = src.as_bytes();

        struct SymCollector(Vec<u16>);
        impl Visitor for SymCollector {
            fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
                self.0.push(node.symbol());
                VisitorAction::Continue
            }
        }

        let mut dfs_syms = SymCollector(vec![]);
        TreeWalker::new(source).walk(&tree, &mut dfs_syms);

        let mut bfs_syms = SymCollector(vec![]);
        BreadthFirstWalker::new(source).walk(&tree, &mut bfs_syms);

        let mut dfs_sorted = dfs_syms.0.clone();
        dfs_sorted.sort();
        let mut bfs_sorted = bfs_syms.0.clone();
        bfs_sorted.sort();
        prop_assert_eq!(dfs_sorted, bfs_sorted);
    }
}

// 6. StatsVisitor node count matches expected
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn stats_visitor_count_matches(
        tree in arb_parsed_tree(5, 3),
        src in arb_source(),
    ) {
        let source = src.as_bytes();
        let walker = TreeWalker::new(source);
        let mut stats = StatsVisitor::default();
        walker.walk(&tree, &mut stats);
        let expected = count_non_error(&tree);
        prop_assert_eq!(stats.total_nodes, expected);
    }
}

// 7. SearchVisitor finds nodes matching predicate
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn search_visitor_finds_matching(
        tree in arb_parsed_tree(4, 3),
        target_sym in 1u16..=10,
        src in arb_source(),
    ) {
        let source = src.as_bytes();
        let walker = TreeWalker::new(source);
        let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == target_sym);
        walker.walk(&tree, &mut search);

        // Count manually
        fn count_sym(node: &ParsedNode, sym: u16) -> usize {
            let me = if !node.is_error() && node.symbol() == sym { 1 } else { 0 };
            me + node.children().iter().map(|c| count_sym(c, sym)).sum::<usize>()
        }
        let expected = count_sym(&tree, target_sym);
        prop_assert_eq!(search.matches.len(), expected);
    }
}

// SearchVisitor with always-false predicate finds nothing
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn search_visitor_none_on_false_pred(
        tree in arb_parsed_tree(4, 3),
        src in arb_source(),
    ) {
        let source = src.as_bytes();
        let walker = TreeWalker::new(source);
        let mut search = SearchVisitor::new(|_: &ParsedNode| false);
        walker.walk(&tree, &mut search);
        prop_assert!(search.matches.is_empty());
    }
}

// SearchVisitor with always-true predicate finds all non-error nodes
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn search_visitor_all_on_true_pred(
        tree in arb_parsed_tree(4, 3),
        src in arb_source(),
    ) {
        let source = src.as_bytes();
        let walker = TreeWalker::new(source);
        let mut search = SearchVisitor::new(|_: &ParsedNode| true);
        walker.walk(&tree, &mut search);
        let expected = count_non_error(&tree);
        prop_assert_eq!(search.matches.len(), expected);
    }
}

// 8. VisitorAction::Stop terminates early
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn stop_terminates_early(
        tree in arb_parsed_tree(5, 3),
        src in arb_source(),
    ) {
        let total = count_non_error(&tree);
        prop_assume!(total > 1);

        let source = src.as_bytes();
        let walker = TreeWalker::new(source);

        struct StopAfterOne(usize);
        impl Visitor for StopAfterOne {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.0 += 1;
                if self.0 >= 1 {
                    VisitorAction::Stop
                } else {
                    VisitorAction::Continue
                }
            }
        }

        let mut v = StopAfterOne(0);
        walker.walk(&tree, &mut v);
        prop_assert!(v.0 <= total, "stopped visitor visited {} out of {}", v.0, total);
        prop_assert_eq!(v.0, 1);
    }
}

// VisitorAction::Stop terminates BFS early
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn bfs_stop_terminates_early(
        tree in arb_parsed_tree(5, 3),
        src in arb_source(),
    ) {
        let total = count_non_error(&tree);
        prop_assume!(total > 1);

        let source = src.as_bytes();
        let walker = BreadthFirstWalker::new(source);

        struct StopAfterOne(usize);
        impl Visitor for StopAfterOne {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.0 += 1;
                if self.0 >= 1 {
                    VisitorAction::Stop
                } else {
                    VisitorAction::Continue
                }
            }
        }

        let mut v = StopAfterOne(0);
        walker.walk(&tree, &mut v);
        prop_assert_eq!(v.0, 1);
    }
}

// SkipChildren skips subtrees in DFS
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn skip_children_reduces_visits(
        tree in arb_parsed_tree(4, 3),
        src in arb_source(),
    ) {
        let total = count_non_error(&tree);
        prop_assume!(total > 1);

        let source = src.as_bytes();

        struct SkipAll(usize);
        impl Visitor for SkipAll {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.0 += 1;
                VisitorAction::SkipChildren
            }
        }

        let mut v = SkipAll(0);
        TreeWalker::new(source).walk(&tree, &mut v);
        // Only the root should be visited since we skip all children
        prop_assert_eq!(v.0, 1);
    }
}

// SkipChildren in BFS skips subtrees
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn bfs_skip_children_reduces_visits(
        tree in arb_parsed_tree(4, 3),
        src in arb_source(),
    ) {
        let total = count_non_error(&tree);
        prop_assume!(total > 1);

        let source = src.as_bytes();

        struct SkipAll(usize);
        impl Visitor for SkipAll {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.0 += 1;
                VisitorAction::SkipChildren
            }
        }

        let mut v = SkipAll(0);
        BreadthFirstWalker::new(source).walk(&tree, &mut v);
        prop_assert_eq!(v.0, 1);
    }
}

// 9. Random tree shapes don't crash traversal (DFS)
proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn random_tree_dfs_no_crash(
        depth in 0u32..=20,
        width in 1usize..=5,
        src in arb_source(),
    ) {
        let tree_strat = arb_parsed_tree(depth.min(6), width.min(4));
        let runner = proptest::test_runner::TestRunner::default();
        let tree = tree_strat.new_tree(&mut proptest::test_runner::TestRunner::default())
            .unwrap()
            .current();
        let source = src.as_bytes();
        let walker = TreeWalker::new(source);
        let mut stats = StatsVisitor::default();
        walker.walk(&tree, &mut stats);
        // If we got here, no crash
        prop_assert!(stats.total_nodes >= 1);
        let _ = runner;
    }
}

// 9b. Random tree shapes don't crash traversal (BFS)
proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn random_tree_bfs_no_crash(
        depth in 0u32..=20,
        width in 1usize..=5,
        src in arb_source(),
    ) {
        let tree_strat = arb_parsed_tree(depth.min(6), width.min(4));
        let tree = tree_strat.new_tree(&mut proptest::test_runner::TestRunner::default())
            .unwrap()
            .current();
        let source = src.as_bytes();
        let walker = BreadthFirstWalker::new(source);
        let mut stats = StatsVisitor::default();
        walker.walk(&tree, &mut stats);
        prop_assert!(stats.total_nodes >= 1);
    }
}

// DFS visit order matches manual DFS preorder
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn dfs_visit_order_is_preorder(
        tree in arb_parsed_tree(4, 3),
        src in arb_source(),
    ) {
        let source = src.as_bytes();

        struct SymCollector(Vec<u16>);
        impl Visitor for SymCollector {
            fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
                self.0.push(node.symbol());
                VisitorAction::Continue
            }
        }

        let mut v = SymCollector(vec![]);
        TreeWalker::new(source).walk(&tree, &mut v);
        let expected = collect_dfs_symbols(&tree);
        prop_assert_eq!(v.0, expected);
    }
}

// BFS visit order matches manual BFS level-order
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn bfs_visit_order_is_levelorder(
        tree in arb_parsed_tree(4, 3),
        src in arb_source(),
    ) {
        let source = src.as_bytes();

        struct SymCollector(Vec<u16>);
        impl Visitor for SymCollector {
            fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
                self.0.push(node.symbol());
                VisitorAction::Continue
            }
        }

        let mut v = SymCollector(vec![]);
        BreadthFirstWalker::new(source).walk(&tree, &mut v);
        let expected = collect_bfs_symbols(&tree);
        prop_assert_eq!(v.0, expected);
    }
}

// StatsVisitor max_depth >= 1 for any non-empty tree
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn stats_visitor_max_depth_ge_one(
        tree in arb_parsed_tree(5, 3),
        src in arb_source(),
    ) {
        let source = src.as_bytes();
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source).walk(&tree, &mut stats);
        prop_assert!(stats.max_depth >= 1);
    }
}

// StatsVisitor leaf_nodes <= total_nodes
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn stats_visitor_leaves_le_total(
        tree in arb_parsed_tree(5, 3),
        src in arb_source(),
    ) {
        let source = src.as_bytes();
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source).walk(&tree, &mut stats);
        prop_assert!(stats.leaf_nodes <= stats.total_nodes);
    }
}

// StatsVisitor error_nodes counts errors
#[test]
fn stats_visitor_counts_errors() {
    let tree = pn_interior(1, vec![pn_error(0, 1), pn_leaf(2, 1, 2), pn_error(2, 3)]);
    let source = b"abcdefghijklmnop";
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut stats);
    assert_eq!(stats.error_nodes, 2);
}

// TreeWalker is reusable
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn tree_walker_reusable(
        tree in arb_parsed_tree(3, 3),
        src in arb_source(),
    ) {
        let source = src.as_bytes();
        let walker = TreeWalker::new(source);

        struct Counter(usize);
        impl Visitor for Counter {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.0 += 1;
                VisitorAction::Continue
            }
        }

        let mut c1 = Counter(0);
        walker.walk(&tree, &mut c1);
        let mut c2 = Counter(0);
        walker.walk(&tree, &mut c2);
        prop_assert_eq!(c1.0, c2.0);
    }
}

// BreadthFirstWalker is reusable
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn bfs_walker_reusable(
        tree in arb_parsed_tree(3, 3),
        src in arb_source(),
    ) {
        let source = src.as_bytes();
        let walker = BreadthFirstWalker::new(source);

        struct Counter(usize);
        impl Visitor for Counter {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.0 += 1;
                VisitorAction::Continue
            }
        }

        let mut c1 = Counter(0);
        walker.walk(&tree, &mut c1);
        let mut c2 = Counter(0);
        walker.walk(&tree, &mut c2);
        prop_assert_eq!(c1.0, c2.0);
    }
}

// DFS traversal is deterministic
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn dfs_deterministic(
        tree in arb_parsed_tree(4, 3),
        src in arb_source(),
    ) {
        let source = src.as_bytes();

        struct SymCollector(Vec<u16>);
        impl Visitor for SymCollector {
            fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
                self.0.push(node.symbol());
                VisitorAction::Continue
            }
        }

        let mut v1 = SymCollector(vec![]);
        TreeWalker::new(source).walk(&tree, &mut v1);
        let mut v2 = SymCollector(vec![]);
        TreeWalker::new(source).walk(&tree, &mut v2);
        prop_assert_eq!(v1.0, v2.0);
    }
}

// BFS traversal is deterministic
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn bfs_deterministic(
        tree in arb_parsed_tree(4, 3),
        src in arb_source(),
    ) {
        let source = src.as_bytes();

        struct SymCollector(Vec<u16>);
        impl Visitor for SymCollector {
            fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
                self.0.push(node.symbol());
                VisitorAction::Continue
            }
        }

        let mut v1 = SymCollector(vec![]);
        BreadthFirstWalker::new(source).walk(&tree, &mut v1);
        let mut v2 = SymCollector(vec![]);
        BreadthFirstWalker::new(source).walk(&tree, &mut v2);
        prop_assert_eq!(v1.0, v2.0);
    }
}

// VisitorAction variants are distinct
#[test]
fn visitor_action_variants_distinct() {
    assert_ne!(VisitorAction::Continue, VisitorAction::SkipChildren);
    assert_ne!(VisitorAction::Continue, VisitorAction::Stop);
    assert_ne!(VisitorAction::SkipChildren, VisitorAction::Stop);
}

// VisitorAction is Copy
#[test]
fn visitor_action_is_copy() {
    let a = VisitorAction::Continue;
    let b = a;
    assert_eq!(a, b);
}

// Single-leaf tree visit
#[test]
fn single_leaf_dfs() {
    let tree = pn_leaf(1, 0, 5);
    let source = b"hello world test!";
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
    assert_eq!(stats.max_depth, 1);
}

#[test]
fn single_leaf_bfs() {
    let tree = pn_leaf(1, 0, 5);
    let source = b"hello world test!";
    let mut stats = StatsVisitor::default();
    BreadthFirstWalker::new(source).walk(&tree, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
}

// Leave_node is called for every entered non-error node in DFS
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn dfs_leave_called_for_every_enter(
        tree in arb_parsed_tree(4, 3),
        src in arb_source(),
    ) {
        let source = src.as_bytes();

        struct EnterLeave { enters: usize, leaves: usize }
        impl Visitor for EnterLeave {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.enters += 1;
                VisitorAction::Continue
            }
            fn leave_node(&mut self, _: &ParsedNode) {
                self.leaves += 1;
            }
        }

        let mut v = EnterLeave { enters: 0, leaves: 0 };
        TreeWalker::new(source).walk(&tree, &mut v);
        prop_assert_eq!(v.enters, v.leaves);
    }
}

// Large arena stress test
proptest! {
    #![proptest_config(ProptestConfig::with_cases(5))]
    #[test]
    fn arena_large_allocation(cap in arb_capacity()) {
        let count = cap * 2;
        let mut arena = TreeArena::with_capacity(cap);
        let mut handles = Vec::with_capacity(count);
        for i in 0..count {
            handles.push(arena.alloc(TreeNode::leaf(i as i32)));
        }
        prop_assert_eq!(arena.len(), count);
        // Spot check first and last
        prop_assert_eq!(arena.get(handles[0]).value(), 0);
        prop_assert_eq!(arena.get(handles[count - 1]).value(), (count - 1) as i32);
    }
}

// Arena with nested branch tree
proptest! {
    #[test]
    fn arena_nested_branches(depth in 1usize..=10) {
        let mut arena = TreeArena::new();
        let leaf = arena.alloc(TreeNode::leaf(0));
        let mut current = leaf;
        for i in 1..=depth {
            current = arena.alloc(TreeNode::branch_with_symbol(i as i32, vec![current]));
        }
        // Walk back from root
        let mut node_ref = arena.get(current);
        for i in (1..=depth).rev() {
            prop_assert_eq!(node_ref.symbol(), i as i32);
            prop_assert!(node_ref.is_branch());
            let children = node_ref.children();
            prop_assert_eq!(children.len(), 1);
            node_ref = arena.get(children[0]);
        }
        prop_assert!(node_ref.is_leaf());
        prop_assert_eq!(node_ref.value(), 0);
    }
}

// Stop at N-th node in DFS — Stop prevents visiting current node's children
// but siblings may still be visited by the parent loop
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn stop_at_nth_node(
        tree in arb_parsed_tree(4, 3),
        src in arb_source(),
    ) {
        let total = count_non_error(&tree);
        prop_assume!(total >= 3);
        // Stop at root: only 1 node should be visited
        let source = src.as_bytes();

        struct StopAtRoot(usize);
        impl Visitor for StopAtRoot {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.0 += 1;
                VisitorAction::Stop
            }
        }

        let mut v = StopAtRoot(0);
        TreeWalker::new(source).walk(&tree, &mut v);
        // Stop at root means exactly 1 node visited
        prop_assert_eq!(v.0, 1);
    }
}

// Arena TreeNode clone preserves value
proptest! {
    #[test]
    fn tree_node_clone_eq(v in arb_leaf_value()) {
        let n = TreeNode::leaf(v);
        let cloned = n.clone();
        prop_assert_eq!(n, cloned);
    }
}

// Arena works with wide trees
proptest! {
    #[test]
    fn arena_wide_tree(width in 1usize..=50) {
        let mut arena = TreeArena::new();
        let children: Vec<_> = (0..width)
            .map(|i| arena.alloc(TreeNode::leaf(i as i32)))
            .collect();
        let root = arena.alloc(TreeNode::branch(children.clone()));
        prop_assert_eq!(arena.get(root).children().len(), width);
    }
}

// NodeHandle is hashable (used in sets)
proptest! {
    #[test]
    fn node_handle_hashable(ci in 0u32..50, ni in 0u32..50) {
        let h = NodeHandle::new(ci, ni);
        let mut set = HashSet::new();
        set.insert(h);
        prop_assert!(set.contains(&h));
    }
}

// Verify DFS enter/leave nesting is balanced (stack discipline)
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn dfs_enter_leave_balanced(
        tree in arb_parsed_tree(4, 3),
        src in arb_source(),
    ) {
        let source = src.as_bytes();

        struct StackCheck { depth: i32, max_depth: i32 }
        impl Visitor for StackCheck {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.depth += 1;
                self.max_depth = self.max_depth.max(self.depth);
                VisitorAction::Continue
            }
            fn leave_node(&mut self, _: &ParsedNode) {
                self.depth -= 1;
            }
        }

        let mut v = StackCheck { depth: 0, max_depth: 0 };
        TreeWalker::new(source).walk(&tree, &mut v);
        prop_assert_eq!(v.depth, 0, "stack not balanced");
        prop_assert!(v.max_depth >= 1);
    }
}

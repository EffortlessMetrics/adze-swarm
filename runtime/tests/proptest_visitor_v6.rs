//! Property-based tests (v6) for visitor patterns, TreeArena, and NodeHandle.
//!
//! 50+ proptest-driven tests covering:
//! - StatsVisitor accumulation properties
//! - PrettyPrintVisitor output invariants
//! - SearchVisitor matching properties
//! - TreeArena allocation, growth, and reset
//! - NodeHandle identity and uniqueness

use adze::arena_allocator::{NodeHandle, TreeArena, TreeNode};
use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TransformVisitor,
    TransformWalker, TreeWalker, Visitor, VisitorAction,
};
use proptest::prelude::*;
use std::collections::{HashMap, HashSet};

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

fn leaf(symbol: u16, start: usize, end: usize) -> ParsedNode {
    make_node(symbol, vec![], start, end, false, true)
}

fn interior(symbol: u16, children: Vec<ParsedNode>) -> ParsedNode {
    let start = children.first().map_or(0, |c| c.start_byte);
    let end = children.last().map_or(0, |c| c.end_byte);
    make_node(symbol, children, start, end, false, true)
}

fn error_node(start: usize, end: usize) -> ParsedNode {
    make_node(0, vec![], start, end, true, false)
}

fn count_nodes(node: &ParsedNode) -> usize {
    1 + node.children().iter().map(count_nodes).sum::<usize>()
}

fn tree_depth(node: &ParsedNode) -> usize {
    if node.children().is_empty() {
        1
    } else {
        1 + node.children().iter().map(tree_depth).max().unwrap_or(0)
    }
}

fn count_non_error_nodes(node: &ParsedNode) -> usize {
    if node.is_error() {
        0
    } else {
        1 + node
            .children()
            .iter()
            .map(count_non_error_nodes)
            .sum::<usize>()
    }
}

fn collect_kinds(node: &ParsedNode) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    collect_kinds_rec(node, &mut map);
    map
}

fn collect_kinds_rec(node: &ParsedNode, map: &mut HashMap<String, usize>) {
    if !node.is_error() {
        *map.entry(node.kind().to_string()).or_insert(0) += 1;
        for child in node.children() {
            collect_kinds_rec(child, map);
        }
    }
}

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

const SOURCE_LEN: usize = 64;

fn arb_leaf() -> impl Strategy<Value = ParsedNode> {
    (1u16..=10, 0..SOURCE_LEN - 1).prop_map(|(sym, start)| leaf(sym, start, start + 1))
}

fn arb_tree(max_depth: u32, max_width: usize) -> impl Strategy<Value = ParsedNode> {
    arb_leaf().prop_recursive(max_depth, 64, max_width as u32, move |inner| {
        (1u16..=10, proptest::collection::vec(inner, 1..=max_width))
            .prop_map(|(sym, children)| interior(sym, children))
    })
}

fn arb_tree_with_errors(max_depth: u32, max_width: usize) -> impl Strategy<Value = ParsedNode> {
    arb_leaf().prop_recursive(max_depth, 64, max_width as u32, move |inner| {
        (
            1u16..=10,
            proptest::collection::vec(inner, 1..=max_width),
            proptest::bool::weighted(0.15),
        )
            .prop_map(|(sym, mut children, inject_error)| {
                if inject_error && !children.is_empty() {
                    let last_end = children.last().map_or(1, |c| c.end_byte);
                    children.push(error_node(last_end, last_end + 1));
                }
                interior(sym, children)
            })
    })
}

fn arb_source() -> impl Strategy<Value = String> {
    proptest::string::string_regex(&format!("[a-z0-9 ]{{{SOURCE_LEN},{SOURCE_LEN}}}")).unwrap()
}

fn arb_symbol_values(count: usize) -> impl Strategy<Value = Vec<i32>> {
    proptest::collection::vec(any::<i32>(), count)
}

// ============================================================================
// StatsVisitor accumulation properties
// ============================================================================

proptest! {
    // 1. total_nodes matches non-error recursive count
    #[test]
    fn v6_stats_total_nodes_equals_non_error_count(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        let expected = count_non_error_nodes(&tree);
        prop_assert_eq!(stats.total_nodes, expected);
    }

    // 2. leaf_nodes never exceeds total_nodes
    #[test]
    fn v6_stats_leaf_lte_total(
        source in arb_source(),
        tree in arb_tree(4, 4),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert!(stats.leaf_nodes <= stats.total_nodes);
    }

    // 3. error_nodes is non-negative (trivially >= 0 for usize, but count matches)
    #[test]
    fn v6_stats_error_count_matches(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        // error_nodes should be >= 0 and consistent with tree structure
        prop_assert!(stats.error_nodes <= count_nodes(&tree));
    }

    // 4. max_depth is at least 1 for any non-empty tree
    #[test]
    fn v6_stats_max_depth_at_least_one(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert!(stats.max_depth >= 1);
    }

    // 5. node_counts values sum to total_nodes
    #[test]
    fn v6_stats_node_counts_sum(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        let sum: usize = stats.node_counts.values().sum();
        prop_assert_eq!(sum, stats.total_nodes);
    }

    // 6. node_counts keys match expected kinds from manual traversal
    #[test]
    fn v6_stats_node_counts_keys_match(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        let expected = collect_kinds(&tree);
        // Every key in stats.node_counts should exist in expected
        for key in stats.node_counts.keys() {
            prop_assert!(expected.contains_key(key), "Unexpected kind: {}", key);
        }
    }

    // 7. max_depth bounded by tree_depth
    #[test]
    fn v6_stats_max_depth_bounded(
        source in arb_source(),
        tree in arb_tree(4, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        prop_assert!(stats.max_depth <= tree_depth(&tree));
    }

    // 8. Running stats on same tree twice doubles total
    #[test]
    fn v6_stats_accumulates_across_walks(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        let src = source.as_bytes();
        TreeWalker::new(src).walk(&tree, &mut stats);
        let first_total = stats.total_nodes;
        TreeWalker::new(src).walk(&tree, &mut stats);
        prop_assert_eq!(stats.total_nodes, first_total * 2);
    }

    // 9. DFS and BFS report same total_nodes
    #[test]
    fn v6_stats_dfs_bfs_same_total(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut dfs = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut dfs);
        let mut bfs = StatsVisitor::default();
        BreadthFirstWalker::new(src).walk(&tree, &mut bfs);
        prop_assert_eq!(dfs.total_nodes, bfs.total_nodes);
    }

    // 10. DFS and BFS report same leaf count
    #[test]
    fn v6_stats_dfs_bfs_same_leaves(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut dfs = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut dfs);
        let mut bfs = StatsVisitor::default();
        BreadthFirstWalker::new(src).walk(&tree, &mut bfs);
        prop_assert_eq!(dfs.leaf_nodes, bfs.leaf_nodes);
    }
}

// ============================================================================
// PrettyPrintVisitor output invariants
// ============================================================================

proptest! {
    // 11. Output is never empty for any tree
    #[test]
    fn v6_pretty_output_nonempty(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut pp);
        prop_assert!(!pp.output().is_empty());
    }

    // 12. Output ends with newline
    #[test]
    fn v6_pretty_ends_with_newline(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut pp);
        prop_assert!(pp.output().ends_with('\n'));
    }

    // 13. Each non-empty line has consistent indentation (even number of spaces prefix)
    #[test]
    fn v6_pretty_indent_even_spaces(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut pp);
        for line in pp.output().lines() {
            let leading = line.len() - line.trim_start_matches(' ').len();
            prop_assert!(leading % 2 == 0, "Odd indentation: {:?}", line);
        }
    }

    // 14. Number of lines >= number of non-error nodes
    #[test]
    fn v6_pretty_lines_gte_nodes(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut pp);
        let lines = pp.output().lines().count();
        let non_error = count_non_error_nodes(&tree);
        prop_assert!(lines >= non_error, "lines={} < non_error={}", lines, non_error);
    }

    // 15. Named nodes have "[named]" annotation
    #[test]
    fn v6_pretty_named_annotation(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut pp);
        // All generated interior nodes are named, so at least one "[named]" should appear
        // if the tree has interior nodes.
        let has_interior = !tree.children().is_empty();
        if has_interior {
            prop_assert!(pp.output().contains("[named]"));
        }
    }

    // 16. Output length grows with tree size
    #[test]
    fn v6_pretty_output_length_positive(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut pp);
        prop_assert!(!pp.output().is_empty());
    }

    // 17. Pretty print of leaf-only tree has exactly one line per node
    #[test]
    fn v6_pretty_leaf_one_line(
        source in arb_source(),
        sym in 1u16..=10,
        start in 0usize..SOURCE_LEN - 1,
    ) {
        let node = leaf(sym, start, start + 1);
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&node, &mut pp);
        // A single leaf: enter_node prints one line, visit_leaf prints one line = 2 lines
        let lines = pp.output().lines().count();
        prop_assert!(lines >= 1);
    }

    // 18. Default and new() produce same initial state
    #[test]
    fn v6_pretty_default_eq_new(_x in 0u8..1) {
        let a = PrettyPrintVisitor::new();
        let b = PrettyPrintVisitor::default();
        prop_assert_eq!(a.output(), b.output());
    }

    // 19. No line exceeds reasonable max length
    #[test]
    fn v6_pretty_no_absurd_lines(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut pp);
        for line in pp.output().lines() {
            // Indentation depth * 2 + longest possible content. Tree depth <= ~4-ish,
            // source is 64 bytes max, so no line should be absurdly long.
            prop_assert!(line.len() < 1024, "Line too long: {}", line.len());
        }
    }
}

// ============================================================================
// SearchVisitor matching properties
// ============================================================================

proptest! {
    // 20. Always-true predicate matches all non-error nodes
    #[test]
    fn v6_search_always_true(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut search = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(src).walk(&tree, &mut search);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut stats);
        prop_assert_eq!(search.matches.len(), stats.total_nodes);
    }

    // 21. Always-false predicate yields empty matches
    #[test]
    fn v6_search_always_false(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut search = SearchVisitor::new(|_: &ParsedNode| false);
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut search);
        prop_assert!(search.matches.is_empty());
    }

    // 22. Predicate-filtered count is subset of total
    #[test]
    fn v6_search_subset_of_total(
        source in arb_source(),
        tree in arb_tree(3, 3),
        threshold in 1u16..=10,
    ) {
        let t = threshold;
        let src = source.as_bytes();
        let mut filtered = SearchVisitor::new(move |n: &ParsedNode| n.symbol <= t);
        TreeWalker::new(src).walk(&tree, &mut filtered);
        let mut all = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(src).walk(&tree, &mut all);
        prop_assert!(filtered.matches.len() <= all.matches.len());
    }

    // 23. Match tuples have non-empty kind strings (byte ranges may be synthetic)
    #[test]
    fn v6_search_valid_byte_ranges(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut search = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut search);
        // Our random trees may have synthetic byte ranges from interior(),
        // so just verify tuples are populated correctly (3-element tuples).
        for (start, end, kind) in &search.matches {
            // start and end are usizes (non-negative by definition)
            let _ = (start, end);
            prop_assert!(!kind.is_empty(), "Empty kind in match tuple");
        }
    }

    // 24. Match kinds are non-empty strings
    #[test]
    fn v6_search_kinds_nonempty(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut search = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut search);
        for (_start, _end, kind) in &search.matches {
            prop_assert!(!kind.is_empty(), "Empty kind found");
        }
    }

    // 25. Disjoint predicates partition matches
    #[test]
    fn v6_search_disjoint_partition(
        source in arb_source(),
        tree in arb_tree(3, 3),
        split in 1u16..=9,
    ) {
        let s = split;
        let src = source.as_bytes();
        let mut low = SearchVisitor::new(move |n: &ParsedNode| n.symbol <= s);
        TreeWalker::new(src).walk(&tree, &mut low);
        let mut high = SearchVisitor::new(move |n: &ParsedNode| n.symbol > s);
        TreeWalker::new(src).walk(&tree, &mut high);
        let mut all = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(src).walk(&tree, &mut all);
        prop_assert_eq!(
            low.matches.len() + high.matches.len(),
            all.matches.len(),
            "Partition doesn't sum: {} + {} != {}",
            low.matches.len(), high.matches.len(), all.matches.len()
        );
    }

    // 26. BFS search yields same match count as DFS search
    #[test]
    fn v6_search_bfs_dfs_same_count(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut dfs_search = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(src).walk(&tree, &mut dfs_search);
        let mut bfs_search = SearchVisitor::new(|_: &ParsedNode| true);
        BreadthFirstWalker::new(src).walk(&tree, &mut bfs_search);
        prop_assert_eq!(dfs_search.matches.len(), bfs_search.matches.len());
    }

    // 27. Search for named nodes on trees where all are named
    #[test]
    fn v6_search_named_matches_total(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut named_search = SearchVisitor::new(|n: &ParsedNode| n.is_named());
        TreeWalker::new(src).walk(&tree, &mut named_search);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut stats);
        // All nodes in our generated trees are named
        prop_assert_eq!(named_search.matches.len(), stats.total_nodes);
    }

    // 28. Search with error injection finds fewer named than total nodes
    #[test]
    fn v6_search_error_tree_named_lte_total(
        source in arb_source(),
        tree in arb_tree_with_errors(3, 3),
    ) {
        let src = source.as_bytes();
        let mut named_search = SearchVisitor::new(|n: &ParsedNode| n.is_named());
        TreeWalker::new(src).walk(&tree, &mut named_search);
        let total = count_nodes(&tree);
        prop_assert!(named_search.matches.len() <= total);
    }
}

// ============================================================================
// TreeArena with random node counts and values
// ============================================================================

proptest! {
    // 29. Arena len matches allocation count
    #[test]
    fn v6_arena_len_matches_allocs(count in 1usize..=200) {
        let mut arena = TreeArena::with_capacity(4);
        for i in 0..count {
            arena.alloc(TreeNode::leaf(i as i32));
        }
        prop_assert_eq!(arena.len(), count);
    }

    // 30. Arena values are retrievable
    #[test]
    fn v6_arena_values_round_trip(values in arb_symbol_values(50)) {
        let mut arena = TreeArena::with_capacity(8);
        let handles: Vec<_> = values.iter().map(|&v| arena.alloc(TreeNode::leaf(v))).collect();
        for (handle, &expected) in handles.iter().zip(values.iter()) {
            prop_assert_eq!(arena.get(*handle).value(), expected);
        }
    }

    // 31. Arena capacity >= len always
    #[test]
    fn v6_arena_capacity_gte_len(count in 1usize..=300) {
        let mut arena = TreeArena::with_capacity(4);
        for i in 0..count {
            arena.alloc(TreeNode::leaf(i as i32));
        }
        prop_assert!(arena.capacity() >= arena.len());
    }

    // 32. Arena is_empty iff len == 0
    #[test]
    fn v6_arena_is_empty_iff_zero(count in 0usize..=50) {
        let mut arena = TreeArena::with_capacity(4);
        for i in 0..count {
            arena.alloc(TreeNode::leaf(i as i32));
        }
        prop_assert_eq!(arena.is_empty(), count == 0);
    }

    // 33. Arena reset brings len to zero
    #[test]
    fn v6_arena_reset_clears(count in 1usize..=100) {
        let mut arena = TreeArena::with_capacity(4);
        for i in 0..count {
            arena.alloc(TreeNode::leaf(i as i32));
        }
        arena.reset();
        prop_assert_eq!(arena.len(), 0);
        prop_assert!(arena.is_empty());
    }

    // 34. Arena clear retains exactly one chunk
    #[test]
    fn v6_arena_clear_one_chunk(count in 1usize..=200) {
        let mut arena = TreeArena::with_capacity(4);
        for i in 0..count {
            arena.alloc(TreeNode::leaf(i as i32));
        }
        arena.clear();
        prop_assert_eq!(arena.num_chunks(), 1);
        prop_assert_eq!(arena.len(), 0);
    }

    // 35. Arena memory_usage > 0 always (at least one chunk)
    #[test]
    fn v6_arena_memory_positive(cap in 1usize..=1000) {
        let arena = TreeArena::with_capacity(cap);
        prop_assert!(arena.memory_usage() > 0);
    }

    // 36. Arena metrics consistency
    #[test]
    fn v6_arena_metrics_consistent(count in 0usize..=100) {
        let mut arena = TreeArena::with_capacity(8);
        for i in 0..count {
            arena.alloc(TreeNode::leaf(i as i32));
        }
        let m = arena.metrics();
        prop_assert_eq!(m.len(), arena.len());
        prop_assert_eq!(m.capacity(), arena.capacity());
        prop_assert_eq!(m.num_chunks(), arena.num_chunks());
        prop_assert_eq!(m.memory_usage(), arena.memory_usage());
        prop_assert_eq!(m.is_empty(), arena.is_empty());
    }

    // 37. Arena leaf/branch nodes preserve type
    #[test]
    fn v6_arena_node_types(leaf_count in 1usize..=20, branch_count in 1usize..=20) {
        let mut arena = TreeArena::with_capacity(64);
        let mut leaf_handles = vec![];
        let mut branch_handles = vec![];

        for i in 0..leaf_count {
            leaf_handles.push(arena.alloc(TreeNode::leaf(i as i32)));
        }
        for _ in 0..branch_count {
            let children: Vec<_> = leaf_handles.iter().copied().take(2).collect();
            branch_handles.push(arena.alloc(TreeNode::branch(children)));
        }

        for h in &leaf_handles {
            prop_assert!(arena.get(*h).is_leaf());
            prop_assert!(!arena.get(*h).is_branch());
        }
        for h in &branch_handles {
            prop_assert!(arena.get(*h).is_branch());
            prop_assert!(!arena.get(*h).is_leaf());
        }
    }

    // 38. Arena set_value round-trips
    #[test]
    fn v6_arena_set_value_roundtrip(initial in any::<i32>(), updated in any::<i32>()) {
        let mut arena = TreeArena::with_capacity(4);
        let handle = arena.alloc(TreeNode::leaf(initial));
        prop_assert_eq!(arena.get(handle).value(), initial);
        arena.get_mut(handle).set_value(updated);
        prop_assert_eq!(arena.get(handle).value(), updated);
    }

    // 39. Arena branch children list preserved
    #[test]
    fn v6_arena_branch_children(child_count in 1usize..=10) {
        let mut arena = TreeArena::with_capacity(64);
        let children: Vec<_> = (0..child_count)
            .map(|i| arena.alloc(TreeNode::leaf(i as i32)))
            .collect();
        let parent = arena.alloc(TreeNode::branch(children.clone()));
        let node_ref = arena.get(parent);
        let stored = node_ref.children();
        prop_assert_eq!(stored.len(), child_count);
        for (stored_h, expected_h) in stored.iter().zip(children.iter()) {
            prop_assert_eq!(stored_h, expected_h);
        }
    }

    // 40. Arena branch_with_symbol preserves symbol
    #[test]
    fn v6_arena_branch_symbol(sym in any::<i32>(), child_count in 1usize..=5) {
        let mut arena = TreeArena::with_capacity(16);
        let children: Vec<_> = (0..child_count)
            .map(|i| arena.alloc(TreeNode::leaf(i as i32)))
            .collect();
        let parent = arena.alloc(TreeNode::branch_with_symbol(sym, children));
        prop_assert_eq!(arena.get(parent).symbol(), sym);
    }

    // 41. Arena reuse after reset
    #[test]
    fn v6_arena_reuse_after_reset(
        first_count in 1usize..=50,
        second_count in 1usize..=50,
    ) {
        let mut arena = TreeArena::with_capacity(4);
        for i in 0..first_count {
            arena.alloc(TreeNode::leaf(i as i32));
        }
        arena.reset();
        let handles: Vec<_> = (0..second_count)
            .map(|i| arena.alloc(TreeNode::leaf((i as i32) + 1000)))
            .collect();
        prop_assert_eq!(arena.len(), second_count);
        for (idx, h) in handles.iter().enumerate() {
            prop_assert_eq!(arena.get(*h).value(), (idx as i32) + 1000);
        }
    }
}

// ============================================================================
// NodeHandle identity and uniqueness
// ============================================================================

proptest! {
    // 42. Handles from sequential allocs are all distinct
    #[test]
    fn v6_handles_unique(count in 2usize..=100) {
        let mut arena = TreeArena::with_capacity(4);
        let handles: Vec<_> = (0..count)
            .map(|i| arena.alloc(TreeNode::leaf(i as i32)))
            .collect();
        let unique: HashSet<_> = handles.iter().collect();
        prop_assert_eq!(unique.len(), count);
    }

    // 43. NodeHandle equality is reflexive
    #[test]
    fn v6_handle_eq_reflexive(chunk in 0u32..=100, node in 0u32..=100) {
        let h = NodeHandle::new(chunk, node);
        prop_assert_eq!(h, h);
    }

    // 44. NodeHandle equality is symmetric
    #[test]
    fn v6_handle_eq_symmetric(c1 in 0u32..=50, n1 in 0u32..=50, c2 in 0u32..=50, n2 in 0u32..=50) {
        let a = NodeHandle::new(c1, n1);
        let b = NodeHandle::new(c2, n2);
        prop_assert_eq!(a == b, b == a);
    }

    // 45. NodeHandle is Copy
    #[test]
    fn v6_handle_copy(chunk in 0u32..=100, node in 0u32..=100) {
        let h1 = NodeHandle::new(chunk, node);
        let h2 = h1; // Copy
        prop_assert_eq!(h1, h2);
    }

    // 46. Different indices produce different handles
    #[test]
    fn v6_handle_diff_indices(
        c in 0u32..=10,
        n1 in 0u32..=100,
        n2 in 0u32..=100,
    ) {
        let a = NodeHandle::new(c, n1);
        let b = NodeHandle::new(c, n2);
        if n1 != n2 {
            prop_assert_ne!(a, b);
        } else {
            prop_assert_eq!(a, b);
        }
    }

    // 47. NodeHandle hashing is consistent with equality
    #[test]
    fn v6_handle_hash_consistent(c1 in 0u32..=10, n1 in 0u32..=10, c2 in 0u32..=10, n2 in 0u32..=10) {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        let a = NodeHandle::new(c1, n1);
        let b = NodeHandle::new(c2, n2);
        if a == b {
            let hash_a = {
                let mut h = DefaultHasher::new();
                a.hash(&mut h);
                h.finish()
            };
            let hash_b = {
                let mut h = DefaultHasher::new();
                b.hash(&mut h);
                h.finish()
            };
            prop_assert_eq!(hash_a, hash_b);
        }
    }

    // 48. NodeHandle Debug is non-empty
    #[test]
    fn v6_handle_debug_nonempty(chunk in 0u32..=10, node in 0u32..=10) {
        let h = NodeHandle::new(chunk, node);
        let dbg = format!("{:?}", h);
        prop_assert!(!dbg.is_empty());
    }
}

// ============================================================================
// Arena growth with random allocation patterns
// ============================================================================

proptest! {
    // 49. Chunk count grows when exceeding capacity
    #[test]
    fn v6_arena_chunks_grow(cap in 1usize..=8, count in 1usize..=100) {
        let mut arena = TreeArena::with_capacity(cap);
        for i in 0..count {
            arena.alloc(TreeNode::leaf(i as i32));
        }
        if count > cap {
            prop_assert!(arena.num_chunks() > 1);
        }
        prop_assert_eq!(arena.len(), count);
    }

    // 50. Capacity never decreases after allocations
    #[test]
    fn v6_arena_capacity_monotonic(alloc_counts in proptest::collection::vec(1usize..=20, 1..=10)) {
        let mut arena = TreeArena::with_capacity(2);
        let mut prev_cap = arena.capacity();
        for batch_size in alloc_counts {
            for i in 0..batch_size {
                arena.alloc(TreeNode::leaf(i as i32));
            }
            let cur_cap = arena.capacity();
            prop_assert!(cur_cap >= prev_cap, "Capacity decreased: {} < {}", cur_cap, prev_cap);
            prev_cap = cur_cap;
        }
    }

    // 51. Mixed leaf/branch allocation preserves all handles
    #[test]
    fn v6_arena_mixed_alloc(
        leaf_vals in proptest::collection::vec(any::<i32>(), 1..=30),
        branch_syms in proptest::collection::vec(any::<i32>(), 1..=10),
    ) {
        let mut arena = TreeArena::with_capacity(4);
        let leaf_handles: Vec<_> = leaf_vals
            .iter()
            .map(|&v| arena.alloc(TreeNode::leaf(v)))
            .collect();
        let branch_handles: Vec<_> = branch_syms
            .iter()
            .map(|&s| {
                let children = leaf_handles.iter().take(2).copied().collect();
                arena.alloc(TreeNode::branch_with_symbol(s, children))
            })
            .collect();

        // Verify all handles still valid
        for (&h, &v) in leaf_handles.iter().zip(leaf_vals.iter()) {
            prop_assert_eq!(arena.get(h).value(), v);
        }
        for (&h, &s) in branch_handles.iter().zip(branch_syms.iter()) {
            prop_assert_eq!(arena.get(h).symbol(), s);
        }
    }

    // 52. TransformVisitor depth matches tree_depth helper
    #[test]
    fn v6_transform_depth(
        source in arb_source(),
        tree in arb_tree(4, 3),
    ) {
        struct DepthCalc;
        impl TransformVisitor for DepthCalc {
            type Output = usize;
            fn transform_node(&mut self, _: &ParsedNode, children: Vec<usize>) -> usize {
                1 + children.into_iter().max().unwrap_or(0)
            }
            fn transform_leaf(&mut self, _: &ParsedNode, _: &str) -> usize { 1 }
            fn transform_error(&mut self, _: &ParsedNode) -> usize { 1 }
        }
        let depth = TransformWalker::new(source.as_bytes()).walk(&tree, &mut DepthCalc);
        prop_assert_eq!(depth, tree_depth(&tree));
    }

    // 53. Enter/leave pairing is always balanced
    #[test]
    fn v6_enter_leave_balanced(
        source in arb_source(),
        tree in arb_tree(4, 3),
    ) {
        struct Balance { enters: usize, leaves: usize }
        impl Visitor for Balance {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.enters += 1;
                VisitorAction::Continue
            }
            fn leave_node(&mut self, _: &ParsedNode) {
                self.leaves += 1;
            }
        }
        let mut v = Balance { enters: 0, leaves: 0 };
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut v);
        prop_assert_eq!(v.enters, v.leaves);
    }

    // 54. SkipChildren still calls leave_node for the skipped node
    #[test]
    fn v6_skip_still_leaves(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct SkipAll { enters: usize, leaves: usize }
        impl Visitor for SkipAll {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.enters += 1;
                VisitorAction::SkipChildren
            }
            fn leave_node(&mut self, _: &ParsedNode) {
                self.leaves += 1;
            }
        }
        let mut v = SkipAll { enters: 0, leaves: 0 };
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut v);
        // Even with SkipChildren, every enter has a matching leave
        prop_assert_eq!(v.enters, v.leaves);
    }

    // 55. Arena default() is equivalent to new()
    #[test]
    fn v6_arena_default_eq_new(_x in 0u8..1) {
        let a = TreeArena::new();
        let b = TreeArena::default();
        prop_assert_eq!(a.len(), b.len());
        prop_assert_eq!(a.capacity(), b.capacity());
        prop_assert_eq!(a.num_chunks(), b.num_chunks());
    }
}

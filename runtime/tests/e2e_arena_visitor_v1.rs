//! End-to-end integration tests: TreeArena construction + TreeVisitor walking.
//!
//! Pipeline: TreeArena::new() → alloc leaf/branch nodes → build tree
//!         → walk with DFS/BFS visitors → verify results.
//!
//! Categories:
//!   1. StatsVisitor counts
//!   2. PrettyPrintVisitor format
//!   3. SearchVisitor found nodes
//!   4. DFS vs BFS ordering
//!   5. Error node handling
//!   6. Large trees (100+ nodes)
//!   7. Reset arena → rebuild → re-walk
//!   8. Edge cases

use std::collections::VecDeque;

use adze::arena_allocator::{NodeHandle, TreeArena, TreeNode};
use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TreeWalker, Visitor,
    VisitorAction,
};

// ---------------------------------------------------------------------------
// Helpers — ParsedNode construction (language field is pub(crate))
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

fn interior(sym: u16, children: Vec<ParsedNode>) -> ParsedNode {
    let start = children.first().map_or(0, |c| c.start_byte());
    let end = children.last().map_or(0, |c| c.end_byte());
    make_node(sym, children, start, end, false, true)
}

fn error_node(start: usize, end: usize) -> ParsedNode {
    make_node(0, vec![], start, end, true, false)
}

// ---------------------------------------------------------------------------
// Arena → ParsedNode bridge: walk arena tree to build a ParsedNode tree
// ---------------------------------------------------------------------------

fn arena_to_parsed(arena: &TreeArena, handle: NodeHandle, source_len: usize) -> ParsedNode {
    let node_ref = arena.get(handle);
    let children_handles = node_ref.children().to_vec();
    let symbol = node_ref.symbol();

    if children_handles.is_empty() {
        let sym = symbol as u16;
        let start = sym as usize % source_len;
        let end = (start + 1).min(source_len);
        leaf(sym, start, end)
    } else {
        let children: Vec<ParsedNode> = children_handles
            .iter()
            .map(|&h| arena_to_parsed(arena, h, source_len))
            .collect();
        interior(symbol as u16, children)
    }
}

// ---------------------------------------------------------------------------
// Custom visitor: records DFS enter-order of symbol IDs
// ---------------------------------------------------------------------------

struct OrderVisitor {
    symbols: Vec<u16>,
}

impl OrderVisitor {
    fn new() -> Self {
        Self {
            symbols: Vec::new(),
        }
    }
}

impl Visitor for OrderVisitor {
    fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
        self.symbols.push(node.symbol);
        VisitorAction::Continue
    }

    fn visit_leaf(&mut self, _node: &ParsedNode, _text: &str) {}

    fn visit_error(&mut self, node: &ParsedNode) {
        self.symbols.push(node.symbol);
    }
}

// ---------------------------------------------------------------------------
// Custom visitor: counts errors only
// ---------------------------------------------------------------------------

struct ErrorCountVisitor {
    count: usize,
}

impl ErrorCountVisitor {
    fn new() -> Self {
        Self { count: 0 }
    }
}

impl Visitor for ErrorCountVisitor {
    fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
        VisitorAction::Continue
    }

    fn visit_error(&mut self, _node: &ParsedNode) {
        self.count += 1;
    }
}

// ---------------------------------------------------------------------------
// Helper: manually collect DFS and BFS symbol orders from a ParsedNode tree
// ---------------------------------------------------------------------------

fn collect_dfs_symbols(node: &ParsedNode) -> Vec<u16> {
    if node.is_error() {
        return vec![node.symbol];
    }
    let mut out = vec![node.symbol];
    for child in node.children() {
        out.extend(collect_dfs_symbols(child));
    }
    out
}

fn collect_bfs_symbols(node: &ParsedNode) -> Vec<u16> {
    let mut out = Vec::new();
    let mut queue = VecDeque::new();
    queue.push_back(node);
    while let Some(n) = queue.pop_front() {
        out.push(n.symbol);
        if !n.is_error() {
            for child in n.children() {
                queue.push_back(child);
            }
        }
    }
    out
}

fn count_all_nodes(node: &ParsedNode) -> usize {
    if node.is_error() {
        return 1;
    }
    1 + node.children().iter().map(count_all_nodes).sum::<usize>()
}

fn count_leaves(node: &ParsedNode) -> usize {
    if node.is_error() {
        return 0;
    }
    if node.child_count() == 0 {
        1
    } else {
        node.children().iter().map(count_leaves).sum()
    }
}

// ---------------------------------------------------------------------------
// Helper: build common source buffer for visitor text extraction
// ---------------------------------------------------------------------------

fn source_buf(len: usize) -> Vec<u8> {
    let mut buf = Vec::with_capacity(len);
    for i in 0..len {
        buf.push(b'a' + (i % 26) as u8);
    }
    buf
}

// =========================================================================
// Category 1: StatsVisitor counts (8 tests)
// =========================================================================

#[test]
fn test_stats_single_leaf() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(5));

    let src = source_buf(16);
    let parsed = arena_to_parsed(&arena, h, src.len());

    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&parsed, &mut stats);

    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
    assert_eq!(stats.error_nodes, 0);
}

#[test]
fn test_stats_binary_tree() {
    let mut arena = TreeArena::new();
    let l = arena.alloc(TreeNode::leaf(1));
    let r = arena.alloc(TreeNode::leaf(2));
    let root = arena.alloc(TreeNode::branch_with_symbol(10, vec![l, r]));

    let src = source_buf(16);
    let parsed = arena_to_parsed(&arena, root, src.len());

    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&parsed, &mut stats);

    assert_eq!(stats.total_nodes, 3);
    assert_eq!(stats.leaf_nodes, 2);
}

#[test]
fn test_stats_three_level_tree() {
    let mut arena = TreeArena::new();
    let a = arena.alloc(TreeNode::leaf(1));
    let b = arena.alloc(TreeNode::leaf(2));
    let mid = arena.alloc(TreeNode::branch_with_symbol(20, vec![a, b]));
    let c = arena.alloc(TreeNode::leaf(3));
    let root = arena.alloc(TreeNode::branch_with_symbol(30, vec![mid, c]));

    let src = source_buf(16);
    let parsed = arena_to_parsed(&arena, root, src.len());

    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&parsed, &mut stats);

    assert_eq!(stats.total_nodes, 5);
    assert_eq!(stats.leaf_nodes, 3);
    assert!(stats.max_depth >= 3);
}

#[test]
fn test_stats_wide_tree_four_children() {
    let mut arena = TreeArena::new();
    let leaves: Vec<NodeHandle> = (1..=4).map(|i| arena.alloc(TreeNode::leaf(i))).collect();
    let root = arena.alloc(TreeNode::branch_with_symbol(50, leaves));

    let src = source_buf(16);
    let parsed = arena_to_parsed(&arena, root, src.len());

    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&parsed, &mut stats);

    assert_eq!(stats.total_nodes, 5);
    assert_eq!(stats.leaf_nodes, 4);
}

#[test]
fn test_stats_left_skewed_chain() {
    let mut arena = TreeArena::new();
    let mut current = arena.alloc(TreeNode::leaf(1));
    for sym in 2..=5 {
        current = arena.alloc(TreeNode::branch_with_symbol(sym, vec![current]));
    }

    let src = source_buf(16);
    let parsed = arena_to_parsed(&arena, current, src.len());

    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&parsed, &mut stats);

    assert_eq!(stats.total_nodes, 5);
    assert_eq!(stats.leaf_nodes, 1);
    assert!(stats.max_depth >= 5);
}

#[test]
fn test_stats_branch_no_leaves_only_branches() {
    let mut arena = TreeArena::new();
    let leaf = arena.alloc(TreeNode::leaf(1));
    let inner = arena.alloc(TreeNode::branch_with_symbol(10, vec![leaf]));
    let root = arena.alloc(TreeNode::branch_with_symbol(20, vec![inner]));

    let src = source_buf(16);
    let parsed = arena_to_parsed(&arena, root, src.len());

    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&parsed, &mut stats);

    assert_eq!(stats.total_nodes, 3);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn test_stats_mixed_tree_with_error() {
    let src = source_buf(16);
    let tree = interior(10, vec![leaf(1, 0, 1), error_node(1, 2), leaf(2, 2, 3)]);

    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&tree, &mut stats);

    assert_eq!(stats.error_nodes, 1);
    // StatsVisitor counts non-error nodes in enter_node and error nodes in visit_error
    assert_eq!(
        stats.total_nodes + stats.error_nodes,
        count_all_nodes(&tree)
    );
}

#[test]
fn test_stats_bfs_matches_dfs_counts() {
    let mut arena = TreeArena::new();
    let a = arena.alloc(TreeNode::leaf(1));
    let b = arena.alloc(TreeNode::leaf(2));
    let mid = arena.alloc(TreeNode::branch_with_symbol(10, vec![a, b]));
    let c = arena.alloc(TreeNode::leaf(3));
    let root = arena.alloc(TreeNode::branch_with_symbol(20, vec![mid, c]));

    let src = source_buf(16);
    let parsed = arena_to_parsed(&arena, root, src.len());

    let dfs_walker = TreeWalker::new(&src);
    let mut dfs_stats = StatsVisitor::default();
    dfs_walker.walk(&parsed, &mut dfs_stats);

    let bfs_walker = BreadthFirstWalker::new(&src);
    let mut bfs_stats = StatsVisitor::default();
    bfs_walker.walk(&parsed, &mut bfs_stats);

    assert_eq!(dfs_stats.total_nodes, bfs_stats.total_nodes);
    assert_eq!(dfs_stats.leaf_nodes, bfs_stats.leaf_nodes);
}

// =========================================================================
// Category 2: PrettyPrintVisitor format (8 tests)
// =========================================================================

#[test]
fn test_pretty_single_leaf() {
    let src = source_buf(16);
    let tree = leaf(5, 0, 1);

    let walker = TreeWalker::new(&src);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&tree, &mut pp);

    let out = pp.output();
    assert!(!out.is_empty());
    // Leaf with symbol 5 → kind "Expression", should have text
    assert!(out.contains('"'));
}

#[test]
fn test_pretty_two_level_tree() {
    let src = source_buf(16);
    let tree = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);

    let walker = TreeWalker::new(&src);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&tree, &mut pp);

    let out = pp.output();
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines.len() >= 3);
}

#[test]
fn test_pretty_indentation_increases() {
    let src = source_buf(16);
    let tree = interior(5, vec![leaf(1, 0, 1)]);

    let walker = TreeWalker::new(&src);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&tree, &mut pp);

    let lines: Vec<&str> = pp.output().lines().collect();
    // First line is root (no indent), children are indented
    assert!(lines.len() >= 2);
    let first_indent = lines[0].len() - lines[0].trim_start().len();
    let second_indent = lines[1].len() - lines[1].trim_start().len();
    assert!(second_indent > first_indent);
}

#[test]
fn test_pretty_error_node_prints_error() {
    let src = source_buf(16);
    let tree = interior(5, vec![error_node(0, 1)]);

    let walker = TreeWalker::new(&src);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&tree, &mut pp);

    assert!(pp.output().contains("ERROR"));
}

#[test]
fn test_pretty_wide_tree_all_leaves_printed() {
    let src = source_buf(32);
    let leaves: Vec<ParsedNode> = (0..4).map(|i| leaf(1, i, i + 1)).collect();
    let tree = interior(5, leaves);

    let walker = TreeWalker::new(&src);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&tree, &mut pp);

    // Root line + 4 leaf text lines
    let lines: Vec<&str> = pp.output().lines().collect();
    assert!(lines.len() >= 5);
}

#[test]
fn test_pretty_nested_three_levels() {
    let src = source_buf(16);
    let inner = interior(5, vec![leaf(1, 0, 1)]);
    let mid = interior(5, vec![inner]);
    let tree = interior(5, vec![mid]);

    let walker = TreeWalker::new(&src);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&tree, &mut pp);

    let lines: Vec<&str> = pp.output().lines().collect();
    // At least root + mid + inner + leaf
    assert!(lines.len() >= 4);
}

#[test]
fn test_pretty_named_node_annotation() {
    let src = source_buf(16);
    // Named node should show "[named]"
    let tree = make_node(5, vec![], 0, 1, false, true);

    let walker = TreeWalker::new(&src);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&tree, &mut pp);

    assert!(pp.output().contains("[named]"));
}

#[test]
fn test_pretty_bfs_produces_output() {
    let src = source_buf(16);
    let tree = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);

    let walker = BreadthFirstWalker::new(&src);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&tree, &mut pp);

    assert!(!pp.output().is_empty());
}

// =========================================================================
// Category 3: SearchVisitor found nodes (8 tests)
// =========================================================================

#[test]
fn test_search_finds_matching_symbol() {
    let src = source_buf(16);
    let tree = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(1, 2, 3)]);

    let walker = TreeWalker::new(&src);
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol == 1);
    walker.walk(&tree, &mut search);

    assert_eq!(search.matches.len(), 2);
}

#[test]
fn test_search_no_match_returns_empty() {
    let src = source_buf(16);
    let tree = interior(5, vec![leaf(1, 0, 1)]);

    let walker = TreeWalker::new(&src);
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol == 99);
    walker.walk(&tree, &mut search);

    assert!(search.matches.is_empty());
}

#[test]
fn test_search_finds_root() {
    let src = source_buf(16);
    let tree = interior(5, vec![leaf(1, 0, 1)]);

    let walker = TreeWalker::new(&src);
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol == 5);
    walker.walk(&tree, &mut search);

    assert_eq!(search.matches.len(), 1);
}

#[test]
fn test_search_finds_named_nodes_only() {
    let src = source_buf(16);
    let named = make_node(1, vec![], 0, 1, false, true);
    let unnamed = make_node(2, vec![], 1, 2, false, false);
    let tree = interior(5, vec![named, unnamed]);

    let walker = TreeWalker::new(&src);
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.is_named());
    walker.walk(&tree, &mut search);

    // Root (named) + named leaf
    assert_eq!(search.matches.len(), 2);
}

#[test]
fn test_search_deep_tree_finds_all() {
    let src = source_buf(32);
    let l3 = leaf(1, 0, 1);
    let l2 = interior(1, vec![l3]);
    let l1 = interior(1, vec![l2]);
    let tree = interior(1, vec![l1]);

    let walker = TreeWalker::new(&src);
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol == 1);
    walker.walk(&tree, &mut search);

    assert_eq!(search.matches.len(), 4);
}

#[test]
fn test_search_bfs_finds_same_count() {
    let src = source_buf(16);
    let tree = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);

    let dfs_walker = TreeWalker::new(&src);
    let mut dfs_search = SearchVisitor::new(|n: &ParsedNode| n.symbol == 1);
    dfs_walker.walk(&tree, &mut dfs_search);

    let bfs_walker = BreadthFirstWalker::new(&src);
    let mut bfs_search = SearchVisitor::new(|n: &ParsedNode| n.symbol == 1);
    bfs_walker.walk(&tree, &mut bfs_search);

    assert_eq!(dfs_search.matches.len(), bfs_search.matches.len());
}

#[test]
fn test_search_captures_byte_ranges() {
    let src = source_buf(16);
    let tree = leaf(5, 3, 7);

    let walker = TreeWalker::new(&src);
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol == 5);
    walker.walk(&tree, &mut search);

    assert_eq!(search.matches.len(), 1);
    assert_eq!(search.matches[0].0, 3);
    assert_eq!(search.matches[0].1, 7);
}

#[test]
fn test_search_in_tree_with_errors() {
    let src = source_buf(16);
    let tree = interior(5, vec![leaf(1, 0, 1), error_node(1, 2), leaf(1, 2, 3)]);

    let walker = TreeWalker::new(&src);
    // Error nodes are visited via visit_error, not enter_node; SearchVisitor
    // only records in enter_node, so errors should not match.
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol == 1);
    walker.walk(&tree, &mut search);

    assert_eq!(search.matches.len(), 2);
}

// =========================================================================
// Category 4: DFS vs BFS produce different orders (8 tests)
// =========================================================================

#[test]
fn test_dfs_bfs_order_differs_asymmetric_tree() {
    //       5
    //      / \
    //    10    3
    //   / \
    //  1   2
    let tree = interior(
        5,
        vec![
            interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]),
            leaf(3, 2, 3),
        ],
    );

    let dfs_syms = collect_dfs_symbols(&tree);
    let bfs_syms = collect_bfs_symbols(&tree);

    // DFS: 5, 10, 1, 2, 3
    // BFS: 5, 10, 3, 1, 2
    assert_ne!(dfs_syms, bfs_syms);
    assert_eq!(dfs_syms, vec![5, 10, 1, 2, 3]);
    assert_eq!(bfs_syms, vec![5, 10, 3, 1, 2]);
}

#[test]
fn test_dfs_bfs_same_for_linear_chain() {
    let tree = interior(5, vec![interior(3, vec![leaf(1, 0, 1)])]);

    let dfs_syms = collect_dfs_symbols(&tree);
    let bfs_syms = collect_bfs_symbols(&tree);
    // Linear: DFS and BFS visit same order
    assert_eq!(dfs_syms, bfs_syms);
}

#[test]
fn test_dfs_bfs_walker_order_differs() {
    let src = source_buf(16);
    let tree = interior(5, vec![interior(10, vec![leaf(1, 0, 1)]), leaf(2, 1, 2)]);

    let dfs_walker = TreeWalker::new(&src);
    let mut dfs_order = OrderVisitor::new();
    dfs_walker.walk(&tree, &mut dfs_order);

    let bfs_walker = BreadthFirstWalker::new(&src);
    let mut bfs_order = OrderVisitor::new();
    bfs_walker.walk(&tree, &mut bfs_order);

    // DFS: 5, 10, 1, 2 — BFS: 5, 10, 2, 1
    assert_ne!(dfs_order.symbols, bfs_order.symbols);
}

#[test]
fn test_dfs_preorder_root_first() {
    let src = source_buf(16);
    let tree = interior(99, vec![leaf(1, 0, 1)]);

    let walker = TreeWalker::new(&src);
    let mut order = OrderVisitor::new();
    walker.walk(&tree, &mut order);

    assert_eq!(order.symbols[0], 99);
}

#[test]
fn test_bfs_level_order() {
    //       5
    //      / \
    //     3   4
    //    /
    //   1
    let tree = interior(5, vec![interior(3, vec![leaf(1, 0, 1)]), leaf(4, 1, 2)]);

    let bfs_syms = collect_bfs_symbols(&tree);
    // Level 0: 5, Level 1: 3, 4, Level 2: 1
    assert_eq!(bfs_syms, vec![5, 3, 4, 1]);
}

#[test]
fn test_dfs_bfs_same_set_of_symbols() {
    let tree = interior(
        5,
        vec![
            interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]),
            leaf(3, 2, 3),
        ],
    );

    let mut dfs_syms = collect_dfs_symbols(&tree);
    let mut bfs_syms = collect_bfs_symbols(&tree);
    dfs_syms.sort();
    bfs_syms.sort();
    assert_eq!(dfs_syms, bfs_syms);
}

#[test]
fn test_dfs_bfs_three_level_wide_tree() {
    //         100
    //       /     \
    //     10       20
    //    / \      / \
    //   1   2   3   4
    let tree = interior(
        100,
        vec![
            interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]),
            interior(20, vec![leaf(3, 2, 3), leaf(4, 3, 4)]),
        ],
    );

    let dfs = collect_dfs_symbols(&tree);
    let bfs = collect_bfs_symbols(&tree);

    assert_eq!(dfs, vec![100, 10, 1, 2, 20, 3, 4]);
    assert_eq!(bfs, vec![100, 10, 20, 1, 2, 3, 4]);
    assert_ne!(dfs, bfs);
}

#[test]
fn test_dfs_bfs_single_node_same() {
    let tree = leaf(42, 0, 1);

    let dfs = collect_dfs_symbols(&tree);
    let bfs = collect_bfs_symbols(&tree);

    assert_eq!(dfs, bfs);
    assert_eq!(dfs, vec![42]);
}

// =========================================================================
// Category 5: Error node handling (8 tests)
// =========================================================================

#[test]
fn test_error_node_counted_by_stats() {
    let src = source_buf(16);
    let tree = interior(5, vec![error_node(0, 1)]);

    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&tree, &mut stats);

    assert_eq!(stats.error_nodes, 1);
}

#[test]
fn test_multiple_errors_counted() {
    let src = source_buf(16);
    let tree = interior(
        5,
        vec![error_node(0, 1), error_node(1, 2), error_node(2, 3)],
    );

    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&tree, &mut stats);

    assert_eq!(stats.error_nodes, 3);
}

#[test]
fn test_error_custom_visitor() {
    let src = source_buf(16);
    let tree = interior(5, vec![leaf(1, 0, 1), error_node(1, 2), leaf(2, 2, 3)]);

    let walker = TreeWalker::new(&src);
    let mut ev = ErrorCountVisitor::new();
    walker.walk(&tree, &mut ev);

    assert_eq!(ev.count, 1);
}

#[test]
fn test_error_bfs_counted() {
    let src = source_buf(16);
    let tree = interior(5, vec![error_node(0, 1), leaf(1, 1, 2)]);

    let walker = BreadthFirstWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&tree, &mut stats);

    assert_eq!(stats.error_nodes, 1);
}

#[test]
fn test_error_pretty_prints_error_tag() {
    let src = source_buf(16);
    let tree = interior(5, vec![error_node(0, 1)]);

    let walker = TreeWalker::new(&src);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&tree, &mut pp);

    assert!(pp.output().contains("ERROR"));
}

#[test]
fn test_error_search_does_not_match_errors() {
    let src = source_buf(16);
    let tree = interior(5, vec![error_node(0, 1)]);

    let walker = TreeWalker::new(&src);
    let mut search = SearchVisitor::new(|_n: &ParsedNode| true);
    walker.walk(&tree, &mut search);

    // SearchVisitor only fires enter_node which is skipped for errors
    // Root (sym 5) should match, error_node should not via enter_node
    assert_eq!(search.matches.len(), 1);
}

#[test]
fn test_error_among_siblings_other_siblings_visited() {
    let src = source_buf(16);
    let tree = interior(5, vec![leaf(1, 0, 1), error_node(1, 2), leaf(2, 2, 3)]);

    let walker = TreeWalker::new(&src);
    let mut order = OrderVisitor::new();
    walker.walk(&tree, &mut order);

    // Root + leaf(1) + error(0) + leaf(2) — error is still pushed via visit_error
    assert!(order.symbols.contains(&1));
    assert!(order.symbols.contains(&2));
}

#[test]
fn test_error_only_tree() {
    let src = source_buf(16);
    let tree = error_node(0, 1);

    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&tree, &mut stats);

    assert_eq!(stats.error_nodes, 1);
    assert_eq!(stats.total_nodes, 0);
}

// =========================================================================
// Category 6: Large trees (100+ nodes) → visitors complete correctly (8 tests)
// =========================================================================

fn build_wide_arena(n: usize) -> (TreeArena, NodeHandle) {
    let mut arena = TreeArena::new();
    let leaves: Vec<NodeHandle> = (0..n as i32)
        .map(|i| arena.alloc(TreeNode::leaf(i + 1)))
        .collect();
    let root = arena.alloc(TreeNode::branch_with_symbol(0, leaves));
    (arena, root)
}

fn build_deep_arena(depth: usize) -> (TreeArena, NodeHandle) {
    let mut arena = TreeArena::new();
    let mut current = arena.alloc(TreeNode::leaf(1));
    for sym in 2..=depth as i32 {
        current = arena.alloc(TreeNode::branch_with_symbol(sym, vec![current]));
    }
    (arena, current)
}

#[test]
fn test_large_wide_tree_stats() {
    let (arena, root) = build_wide_arena(200);
    let src = source_buf(256);
    let parsed = arena_to_parsed(&arena, root, src.len());

    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&parsed, &mut stats);

    assert_eq!(stats.total_nodes, 201); // root + 200 leaves
    assert_eq!(stats.leaf_nodes, 200);
}

#[test]
fn test_large_deep_tree_stats() {
    let (arena, root) = build_deep_arena(150);
    let src = source_buf(256);
    let parsed = arena_to_parsed(&arena, root, src.len());

    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&parsed, &mut stats);

    assert_eq!(stats.total_nodes, 150);
    assert_eq!(stats.leaf_nodes, 1);
    assert!(stats.max_depth >= 150);
}

#[test]
fn test_large_tree_search_finds_all_leaves() {
    let (arena, root) = build_wide_arena(120);
    let src = source_buf(256);
    let parsed = arena_to_parsed(&arena, root, src.len());

    let walker = TreeWalker::new(&src);
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.child_count() == 0);
    walker.walk(&parsed, &mut search);

    assert_eq!(search.matches.len(), 120);
    assert_eq!(count_leaves(&parsed), 120);
}

#[test]
fn test_large_tree_pretty_print_not_empty() {
    let (arena, root) = build_wide_arena(100);
    let src = source_buf(256);
    let parsed = arena_to_parsed(&arena, root, src.len());

    let walker = TreeWalker::new(&src);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&parsed, &mut pp);

    assert!(!pp.output().is_empty());
    let lines: Vec<&str> = pp.output().lines().collect();
    assert!(lines.len() >= 101); // root + 100 leaf text lines
}

#[test]
fn test_large_tree_bfs_stats() {
    let (arena, root) = build_wide_arena(150);
    let src = source_buf(256);
    let parsed = arena_to_parsed(&arena, root, src.len());

    let walker = BreadthFirstWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&parsed, &mut stats);

    assert_eq!(stats.total_nodes, 151);
}

#[test]
fn test_large_tree_arena_metrics() {
    let (arena, _root) = build_wide_arena(200);
    let metrics = arena.metrics();

    assert_eq!(metrics.len(), 201);
    assert!(metrics.capacity() >= 201);
    assert!(metrics.memory_usage() > 0);
}

#[test]
fn test_large_balanced_binary_tree() {
    // Build balanced binary tree of ~127 nodes (depth 7)
    fn build_balanced(arena: &mut TreeArena, depth: usize, sym: &mut i32) -> NodeHandle {
        if depth == 0 {
            let h = arena.alloc(TreeNode::leaf(*sym));
            *sym += 1;
            return h;
        }
        let left = build_balanced(arena, depth - 1, sym);
        let right = build_balanced(arena, depth - 1, sym);
        let h = arena.alloc(TreeNode::branch_with_symbol(*sym, vec![left, right]));
        *sym += 1;
        h
    }

    let mut arena = TreeArena::new();
    let mut sym = 1i32;
    let root = build_balanced(&mut arena, 6, &mut sym);
    let src = source_buf(256);
    let parsed = arena_to_parsed(&arena, root, src.len());

    let total = count_all_nodes(&parsed);
    assert!(total >= 127);

    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&parsed, &mut stats);

    assert_eq!(stats.total_nodes, total);
}

#[test]
fn test_large_tree_dfs_bfs_same_count() {
    let (arena, root) = build_wide_arena(100);
    let src = source_buf(256);
    let parsed = arena_to_parsed(&arena, root, src.len());

    let dfs_walker = TreeWalker::new(&src);
    let mut dfs_stats = StatsVisitor::default();
    dfs_walker.walk(&parsed, &mut dfs_stats);

    let bfs_walker = BreadthFirstWalker::new(&src);
    let mut bfs_stats = StatsVisitor::default();
    bfs_walker.walk(&parsed, &mut bfs_stats);

    assert_eq!(dfs_stats.total_nodes, bfs_stats.total_nodes);
}

// =========================================================================
// Category 7: Reset arena → rebuild → re-walk (8 tests)
// =========================================================================

#[test]
fn test_reset_and_rebuild_single() {
    let mut arena = TreeArena::new();
    arena.alloc(TreeNode::leaf(1));
    arena.alloc(TreeNode::leaf(2));
    assert_eq!(arena.len(), 2);

    arena.reset();
    assert!(arena.is_empty());

    let h = arena.alloc(TreeNode::leaf(42));
    assert_eq!(arena.get(h).value(), 42);
    assert_eq!(arena.len(), 1);
}

#[test]
fn test_reset_rebuild_tree_and_walk() {
    let mut arena = TreeArena::new();
    let src = source_buf(16);

    // First tree
    let a = arena.alloc(TreeNode::leaf(1));
    let root1 = arena.alloc(TreeNode::branch_with_symbol(10, vec![a]));
    let parsed1 = arena_to_parsed(&arena, root1, src.len());

    let walker = TreeWalker::new(&src);
    let mut stats1 = StatsVisitor::default();
    walker.walk(&parsed1, &mut stats1);
    assert_eq!(stats1.total_nodes, 2);

    // Reset and rebuild
    arena.reset();
    let b = arena.alloc(TreeNode::leaf(2));
    let c = arena.alloc(TreeNode::leaf(3));
    let root2 = arena.alloc(TreeNode::branch_with_symbol(20, vec![b, c]));
    let parsed2 = arena_to_parsed(&arena, root2, src.len());

    let walker2 = TreeWalker::new(&src);
    let mut stats2 = StatsVisitor::default();
    walker2.walk(&parsed2, &mut stats2);
    assert_eq!(stats2.total_nodes, 3);
}

#[test]
fn test_reset_preserves_chunks() {
    let mut arena = TreeArena::with_capacity(2);
    for i in 0..10 {
        arena.alloc(TreeNode::leaf(i));
    }
    let chunks_before = arena.num_chunks();
    arena.reset();
    assert_eq!(arena.num_chunks(), chunks_before);
}

#[test]
fn test_clear_releases_excess_chunks() {
    let mut arena = TreeArena::with_capacity(2);
    for i in 0..10 {
        arena.alloc(TreeNode::leaf(i));
    }
    assert!(arena.num_chunks() > 1);

    arena.clear();
    assert_eq!(arena.num_chunks(), 1);
    assert!(arena.is_empty());
}

#[test]
fn test_reset_multiple_times() {
    let mut arena = TreeArena::new();

    for round in 0..5 {
        let leaf_val = round * 10;
        let h = arena.alloc(TreeNode::leaf(leaf_val));
        assert_eq!(arena.get(h).value(), leaf_val);
        arena.reset();
        assert!(arena.is_empty());
    }
}

#[test]
fn test_reset_then_large_allocation() {
    let mut arena = TreeArena::with_capacity(4);
    for i in 0..4 {
        arena.alloc(TreeNode::leaf(i));
    }
    arena.reset();

    // Allocate more than original capacity
    let handles: Vec<NodeHandle> = (0..20).map(|i| arena.alloc(TreeNode::leaf(i))).collect();
    assert_eq!(arena.len(), 20);
    for (i, h) in handles.iter().enumerate() {
        assert_eq!(arena.get(*h).value(), i as i32);
    }
}

#[test]
fn test_reset_rebuild_walk_with_bfs() {
    let mut arena = TreeArena::new();
    let src = source_buf(16);

    let a = arena.alloc(TreeNode::leaf(1));
    arena.alloc(TreeNode::branch_with_symbol(10, vec![a]));

    arena.reset();

    let b = arena.alloc(TreeNode::leaf(2));
    let c = arena.alloc(TreeNode::leaf(3));
    let root = arena.alloc(TreeNode::branch_with_symbol(20, vec![b, c]));
    let parsed = arena_to_parsed(&arena, root, src.len());

    let walker = BreadthFirstWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&parsed, &mut stats);

    assert_eq!(stats.total_nodes, 3);
    assert_eq!(stats.leaf_nodes, 2);
}

#[test]
fn test_clear_rebuild_and_walk() {
    let mut arena = TreeArena::new();
    let src = source_buf(16);

    for i in 0..50 {
        arena.alloc(TreeNode::leaf(i));
    }

    arena.clear();
    assert!(arena.is_empty());
    assert_eq!(arena.num_chunks(), 1);

    let leaf = arena.alloc(TreeNode::leaf(99));
    let root = arena.alloc(TreeNode::branch_with_symbol(10, vec![leaf]));
    let parsed = arena_to_parsed(&arena, root, src.len());

    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&parsed, &mut stats);

    assert_eq!(stats.total_nodes, 2);
}

// =========================================================================
// Category 8: Edge cases (8 tests)
// =========================================================================

#[test]
fn test_edge_empty_arena() {
    let arena = TreeArena::new();
    assert!(arena.is_empty());
    assert_eq!(arena.len(), 0);
    assert_eq!(arena.num_chunks(), 1);
    assert!(arena.capacity() > 0);
}

#[test]
fn test_edge_single_leaf_dfs_and_bfs_same() {
    let src = source_buf(16);
    let tree = leaf(7, 0, 1);

    let dfs_walker = TreeWalker::new(&src);
    let mut dfs_stats = StatsVisitor::default();
    dfs_walker.walk(&tree, &mut dfs_stats);

    let bfs_walker = BreadthFirstWalker::new(&src);
    let mut bfs_stats = StatsVisitor::default();
    bfs_walker.walk(&tree, &mut bfs_stats);

    assert_eq!(dfs_stats.total_nodes, bfs_stats.total_nodes);
    assert_eq!(dfs_stats.leaf_nodes, bfs_stats.leaf_nodes);
    assert_eq!(dfs_stats.total_nodes, 1);
}

#[test]
fn test_edge_deeply_nested_50_levels() {
    let mut arena = TreeArena::new();
    let mut current = arena.alloc(TreeNode::leaf(1));
    for sym in 2..=50 {
        current = arena.alloc(TreeNode::branch_with_symbol(sym, vec![current]));
    }

    let src = source_buf(64);
    let parsed = arena_to_parsed(&arena, current, src.len());

    let walker = TreeWalker::new(&src);
    let mut stats = StatsVisitor::default();
    walker.walk(&parsed, &mut stats);

    assert_eq!(stats.total_nodes, 50);
    assert!(stats.max_depth >= 50);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn test_edge_arena_capacity_one() {
    let mut arena = TreeArena::with_capacity(1);
    let h1 = arena.alloc(TreeNode::leaf(1));
    assert_eq!(arena.num_chunks(), 1);

    let h2 = arena.alloc(TreeNode::leaf(2));
    assert_eq!(arena.num_chunks(), 2);

    assert_eq!(arena.get(h1).value(), 1);
    assert_eq!(arena.get(h2).value(), 2);
}

#[test]
fn test_edge_branch_empty_children() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::branch(vec![]));

    assert!(arena.get(h).is_branch());
    assert!(arena.get(h).children().is_empty());
}

#[test]
fn test_edge_leaf_value_zero() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(0));
    assert_eq!(arena.get(h).value(), 0);
}

#[test]
fn test_edge_leaf_value_negative() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(-1));
    assert_eq!(arena.get(h).value(), -1);
}

#[test]
fn test_edge_default_arena() {
    let arena = TreeArena::default();
    assert!(arena.is_empty());
    assert_eq!(arena.len(), 0);
}

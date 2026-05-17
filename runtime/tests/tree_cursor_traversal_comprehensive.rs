//! Comprehensive tests for tree cursor and traversal patterns in the adze runtime.
//!
//! Covers TreeArena, NodeHandle, TreeNode, TreeWalker, BreadthFirstWalker,
//! StatsVisitor, PrettyPrintVisitor, SearchVisitor, and VisitorAction.

use adze::arena_allocator::{NodeHandle, TreeArena, TreeNode};
use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TreeWalker, Visitor,
    VisitorAction,
};

// ===== Helpers =====

/// Create a synthetic ParsedNode with standard defaults.
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

/// root(10)( a(1), mid(11)( b(2), c(3) ), d(4) )  — source "abcd"
fn sample_tree() -> (ParsedNode, Vec<u8>) {
    let source = b"abcd".to_vec();
    let a = leaf(1, 0, 1);
    let b = leaf(2, 1, 2);
    let c = unnamed_leaf(3, 2, 3);
    let mid = interior(11, vec![b, c]);
    let d = leaf(4, 3, 4);
    let root = interior(10, vec![a, mid, d]);
    (root, source)
}

fn single_leaf_tree() -> (ParsedNode, Vec<u8>) {
    (leaf(1, 0, 1), b"x".to_vec())
}

/// Visitor that records enter/leave order via symbol ids.
struct OrderVisitor {
    enter_order: Vec<u16>,
    leave_order: Vec<u16>,
}

impl OrderVisitor {
    fn new() -> Self {
        Self {
            enter_order: vec![],
            leave_order: vec![],
        }
    }
}

impl Visitor for OrderVisitor {
    fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
        self.enter_order.push(node.symbol());
        VisitorAction::Continue
    }
    fn leave_node(&mut self, node: &ParsedNode) {
        self.leave_order.push(node.symbol());
    }
}

/// Visitor that stops after N enter calls.
struct StopAfterN {
    limit: usize,
    count: usize,
    seen: Vec<u16>,
}

impl StopAfterN {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            count: 0,
            seen: vec![],
        }
    }
}

impl Visitor for StopAfterN {
    fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
        self.count += 1;
        self.seen.push(node.symbol());
        if self.count >= self.limit {
            VisitorAction::Stop
        } else {
            VisitorAction::Continue
        }
    }
}

/// Visitor that skips children when the predicate matches.
struct SkipWhen<F> {
    predicate: F,
    entered: Vec<u16>,
}

impl<F: Fn(&ParsedNode) -> bool> Visitor for SkipWhen<F> {
    fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
        self.entered.push(node.symbol());
        if (self.predicate)(node) {
            VisitorAction::SkipChildren
        } else {
            VisitorAction::Continue
        }
    }
}

// ===========================
// 1. TreeArena creation tests
// ===========================

#[test]
fn arena_new_default() {
    let arena = TreeArena::new();
    assert!(arena.is_empty());
    assert_eq!(arena.len(), 0);
    assert_eq!(arena.num_chunks(), 1);
}

#[test]
fn arena_with_capacity_1() {
    let arena = TreeArena::with_capacity(1);
    assert_eq!(arena.capacity(), 1);
}

#[test]
fn arena_with_capacity_large() {
    let arena = TreeArena::with_capacity(10_000);
    assert!(arena.capacity() >= 10_000);
}

#[test]
#[should_panic(expected = "Capacity must be > 0")]
fn arena_with_capacity_zero_panics() {
    let _ = TreeArena::with_capacity(0);
}

#[test]
fn arena_default_trait() {
    let arena = TreeArena::default();
    assert!(arena.is_empty());
}

// ================================
// 2. Adding/retrieving arena nodes
// ================================

#[test]
fn arena_alloc_leaf() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(42));
    assert_eq!(arena.get(h).value(), 42);
    assert_eq!(arena.len(), 1);
}

#[test]
fn arena_alloc_multiple_leaves() {
    let mut arena = TreeArena::new();
    let handles: Vec<_> = (0..10).map(|i| arena.alloc(TreeNode::leaf(i))).collect();
    for (i, h) in handles.iter().enumerate() {
        assert_eq!(arena.get(*h).value(), i as i32);
    }
    assert_eq!(arena.len(), 10);
}

#[test]
fn arena_alloc_branch_no_children() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::branch(vec![]));
    assert!(arena.get(h).is_branch());
    assert_eq!(arena.get(h).children().len(), 0);
}

#[test]
fn arena_leaf_is_leaf() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(1));
    assert!(arena.get(h).is_leaf());
    assert!(!arena.get(h).is_branch());
}

#[test]
fn arena_branch_is_branch() {
    let mut arena = TreeArena::new();
    let c = arena.alloc(TreeNode::leaf(1));
    let h = arena.alloc(TreeNode::branch(vec![c]));
    assert!(arena.get(h).is_branch());
    assert!(!arena.get(h).is_leaf());
}

#[test]
fn arena_get_mut_set_value() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(10));
    arena.get_mut(h).set_value(20);
    assert_eq!(arena.get(h).value(), 20);
}

#[test]
fn arena_symbol_accessor() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(99));
    assert_eq!(arena.get(h).symbol(), 99);
}

// =================================
// 3. Tree structures (parent-child)
// =================================

#[test]
fn arena_parent_child_single() {
    let mut arena = TreeArena::new();
    let child = arena.alloc(TreeNode::leaf(1));
    let parent = arena.alloc(TreeNode::branch(vec![child]));
    assert_eq!(arena.get(parent).children().len(), 1);
    assert_eq!(arena.get(parent).children()[0], child);
}

#[test]
fn arena_two_level_tree() {
    let mut arena = TreeArena::new();
    let c1 = arena.alloc(TreeNode::leaf(1));
    let c2 = arena.alloc(TreeNode::leaf(2));
    let mid = arena.alloc(TreeNode::branch(vec![c1, c2]));
    let c3 = arena.alloc(TreeNode::leaf(3));
    let root = arena.alloc(TreeNode::branch(vec![mid, c3]));
    assert_eq!(arena.get(root).children().len(), 2);
    let mid_h = arena.get(root).children()[0];
    assert_eq!(arena.get(mid_h).children().len(), 2);
}

#[test]
fn arena_branch_with_symbol() {
    let mut arena = TreeArena::new();
    let c = arena.alloc(TreeNode::leaf(1));
    let h = arena.alloc(TreeNode::branch_with_symbol(42, vec![c]));
    assert_eq!(arena.get(h).symbol(), 42);
    assert!(arena.get(h).is_branch());
}

#[test]
fn arena_deep_nesting() {
    let mut arena = TreeArena::new();
    let mut current = arena.alloc(TreeNode::leaf(0));
    for i in 1..20 {
        current = arena.alloc(TreeNode::branch(vec![current]));
        assert_eq!(arena.get(current).children().len(), 1);
        let _ = i;
    }
    assert_eq!(arena.len(), 20);
}

#[test]
fn arena_wide_tree() {
    let mut arena = TreeArena::new();
    let children: Vec<_> = (0..50).map(|i| arena.alloc(TreeNode::leaf(i))).collect();
    let root = arena.alloc(TreeNode::branch(children.clone()));
    assert_eq!(arena.get(root).children().len(), 50);
}

#[test]
fn arena_reset_empties() {
    let mut arena = TreeArena::with_capacity(4);
    for i in 0..10 {
        arena.alloc(TreeNode::leaf(i));
    }
    assert!(!arena.is_empty());
    arena.reset();
    assert!(arena.is_empty());
    assert_eq!(arena.len(), 0);
}

#[test]
fn arena_clear_frees_chunks() {
    let mut arena = TreeArena::with_capacity(2);
    for i in 0..20 {
        arena.alloc(TreeNode::leaf(i));
    }
    let chunks_before = arena.num_chunks();
    arena.clear();
    assert_eq!(arena.num_chunks(), 1);
    assert!(chunks_before > 1);
}

#[test]
fn arena_reuse_after_reset() {
    let mut arena = TreeArena::new();
    let h1 = arena.alloc(TreeNode::leaf(1));
    assert_eq!(arena.get(h1).value(), 1);
    arena.reset();
    let h2 = arena.alloc(TreeNode::leaf(2));
    assert_eq!(arena.get(h2).value(), 2);
}

#[test]
fn arena_metrics_snapshot() {
    let mut arena = TreeArena::new();
    let m = arena.metrics();
    assert!(m.is_empty());
    assert_eq!(m.len(), 0);
    arena.alloc(TreeNode::leaf(1));
    let m = arena.metrics();
    assert_eq!(m.len(), 1);
    assert!(!m.is_empty());
    assert!(m.memory_usage() > 0);
}

#[test]
fn arena_memory_usage_grows() {
    let mut arena = TreeArena::with_capacity(2);
    let m1 = arena.memory_usage();
    for i in 0..10 {
        arena.alloc(TreeNode::leaf(i));
    }
    assert!(arena.memory_usage() >= m1);
}

#[test]
fn arena_chunk_growth_exponential() {
    let mut arena = TreeArena::with_capacity(2);
    for i in 0..100 {
        arena.alloc(TreeNode::leaf(i));
    }
    // should have grown into multiple chunks
    assert!(arena.num_chunks() > 1);
}

#[test]
fn arena_node_handle_equality() {
    let h1 = NodeHandle::new(0, 0);
    let h2 = NodeHandle::new(0, 0);
    let h3 = NodeHandle::new(0, 1);
    assert_eq!(h1, h2);
    assert_ne!(h1, h3);
}

#[test]
fn arena_node_handle_copy() {
    let h = NodeHandle::new(1, 2);
    let h2 = h;
    assert_eq!(h, h2);
}

#[test]
fn arena_node_handle_debug() {
    let h = NodeHandle::new(0, 5);
    let dbg = format!("{:?}", h);
    assert!(dbg.contains("NodeHandle"));
}

#[test]
fn arena_leaf_children_empty() {
    let n = TreeNode::leaf(1);
    assert!(n.children().is_empty());
}

#[test]
fn arena_tree_node_value() {
    let n = TreeNode::leaf(42);
    assert_eq!(n.value(), 42);
    assert_eq!(n.symbol(), 42);
}

#[test]
fn arena_tree_node_clone() {
    let n = TreeNode::leaf(5);
    let n2 = n.clone();
    assert_eq!(n, n2);
}

// ==================================================
// 4. TreeWalker depth-first traversal order
// ==================================================

#[test]
fn dfs_order_sample_tree() {
    let (root, source) = sample_tree();
    let walker = TreeWalker::new(&source);
    let mut v = OrderVisitor::new();
    walker.walk(&root, &mut v);
    // pre-order: root(10), a(1), mid(11), b(2), c(3), d(4)
    assert_eq!(v.enter_order, vec![10, 1, 11, 2, 3, 4]);
}

#[test]
fn dfs_leave_order_sample_tree() {
    let (root, source) = sample_tree();
    let walker = TreeWalker::new(&source);
    let mut v = OrderVisitor::new();
    walker.walk(&root, &mut v);
    // post-order leaves: a(1), b(2), c(3), mid(11), d(4), root(10)
    assert_eq!(v.leave_order, vec![1, 2, 3, 11, 4, 10]);
}

#[test]
fn dfs_single_leaf() {
    let (root, source) = single_leaf_tree();
    let walker = TreeWalker::new(&source);
    let mut v = OrderVisitor::new();
    walker.walk(&root, &mut v);
    assert_eq!(v.enter_order, vec![1]);
    assert_eq!(v.leave_order, vec![1]);
}

#[test]
fn dfs_leaf_text_visited() {
    let (root, source) = single_leaf_tree();
    let walker = TreeWalker::new(&source);
    struct LeafTextCollector(Vec<String>);
    impl Visitor for LeafTextCollector {
        fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn visit_leaf(&mut self, _node: &ParsedNode, text: &str) {
            self.0.push(text.to_string());
        }
    }
    let mut v = LeafTextCollector(vec![]);
    walker.walk(&root, &mut v);
    assert_eq!(v.0, vec!["x"]);
}

#[test]
fn dfs_error_node_visited() {
    let err = error_node(0, 1);
    let source = b"x";
    let walker = TreeWalker::new(source);
    struct ErrCounter(usize);
    impl Visitor for ErrCounter {
        fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn visit_error(&mut self, _node: &ParsedNode) {
            self.0 += 1;
        }
    }
    let mut v = ErrCounter(0);
    walker.walk(&err, &mut v);
    assert_eq!(v.0, 1);
}

#[test]
fn dfs_flat_children() {
    let source = b"abc".to_vec();
    let root = interior(100, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let walker = TreeWalker::new(&source);
    let mut v = OrderVisitor::new();
    walker.walk(&root, &mut v);
    assert_eq!(v.enter_order, vec![100, 1, 2, 3]);
}

// ==================================================
// 5. BreadthFirstWalker traversal order
// ==================================================

#[test]
fn bfs_order_sample_tree() {
    let (root, source) = sample_tree();
    let walker = BreadthFirstWalker::new(&source);
    let mut v = OrderVisitor::new();
    walker.walk(&root, &mut v);
    // BFS: root(10), a(1), mid(11), d(4), b(2), c(3)
    assert_eq!(v.enter_order, vec![10, 1, 11, 4, 2, 3]);
}

#[test]
fn bfs_single_leaf() {
    let (root, source) = single_leaf_tree();
    let walker = BreadthFirstWalker::new(&source);
    let mut v = OrderVisitor::new();
    walker.walk(&root, &mut v);
    assert_eq!(v.enter_order, vec![1]);
}

#[test]
fn bfs_flat_children() {
    let source = b"abc".to_vec();
    let root = interior(100, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let walker = BreadthFirstWalker::new(&source);
    let mut v = OrderVisitor::new();
    walker.walk(&root, &mut v);
    assert_eq!(v.enter_order, vec![100, 1, 2, 3]);
}

#[test]
fn bfs_three_level_tree() {
    // root -> [mid1 -> [leaf_a], mid2 -> [leaf_b]]
    let source = b"ab".to_vec();
    let leaf_a = leaf(1, 0, 1);
    let leaf_b = leaf(2, 1, 2);
    let mid1 = interior(10, vec![leaf_a]);
    let mid2 = interior(11, vec![leaf_b]);
    let root = interior(100, vec![mid1, mid2]);
    let walker = BreadthFirstWalker::new(&source);
    let mut v = OrderVisitor::new();
    walker.walk(&root, &mut v);
    // level 0: root(100), level 1: mid1(10), mid2(11), level 2: leaf_a(1), leaf_b(2)
    assert_eq!(v.enter_order, vec![100, 10, 11, 1, 2]);
}

#[test]
fn bfs_error_node_skipped_in_enter() {
    let err = error_node(0, 1);
    let root = interior(100, vec![err, leaf(1, 1, 2)]);
    let source = b"xx".to_vec();
    let walker = BreadthFirstWalker::new(&source);
    struct ErrCounter {
        errors: usize,
        entered: Vec<u16>,
    }
    impl Visitor for ErrCounter {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.entered.push(node.symbol());
            VisitorAction::Continue
        }
        fn visit_error(&mut self, _node: &ParsedNode) {
            self.errors += 1;
        }
    }
    let mut v = ErrCounter {
        errors: 0,
        entered: vec![],
    };
    walker.walk(&root, &mut v);
    assert_eq!(v.errors, 1);
    // error node is not entered (visit_error is called instead)
    assert_eq!(v.entered, vec![100, 1]);
}

// ==================================================
// 6. StatsVisitor
// ==================================================

#[test]
fn stats_sample_tree() {
    let (root, source) = sample_tree();
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 6);
    assert_eq!(stats.leaf_nodes, 4); // a, b, c, d all have 0 children
    assert_eq!(stats.error_nodes, 0);
}

#[test]
fn stats_max_depth_sample() {
    let (root, source) = sample_tree();
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    // root -> mid -> leaf = depth 3
    assert_eq!(stats.max_depth, 3);
}

#[test]
fn stats_single_leaf() {
    let (root, source) = single_leaf_tree();
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
    assert_eq!(stats.max_depth, 1);
}

#[test]
fn stats_node_counts_per_kind() {
    let (root, source) = sample_tree();
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    // All nodes have no language, so kind() uses fallback
    // symbol 10 => "rule_10", symbol 11 => "unknown", etc.
    assert!(!stats.node_counts.is_empty());
}

#[test]
fn stats_with_error_nodes() {
    let err = error_node(1, 2);
    let root = interior(10, vec![leaf(1, 0, 1), err, leaf(2, 2, 3)]);
    let source = b"xxx".to_vec();
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
    // error node is not entered, so total_nodes is less
    assert_eq!(stats.total_nodes, 3); // root + 2 leaves
}

#[test]
fn stats_bfs_total_nodes() {
    let (root, source) = sample_tree();
    let walker = BreadthFirstWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 6);
}

// ==================================================
// 7. PrettyPrintVisitor
// ==================================================

#[test]
fn pretty_print_leaf() {
    let (root, source) = single_leaf_tree();
    let walker = TreeWalker::new(&source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let output = pp.output();
    assert!(output.contains("\"x\""));
}

#[test]
fn pretty_print_sample_tree() {
    let (root, source) = sample_tree();
    let walker = TreeWalker::new(&source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let output = pp.output();
    assert!(!output.is_empty());
    // contains indentation
    assert!(output.contains("  "));
}

#[test]
fn pretty_print_contains_named_marker() {
    let (root, source) = sample_tree();
    let walker = TreeWalker::new(&source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    // named nodes get "[named]" suffix
    assert!(pp.output().contains("[named]"));
}

#[test]
fn pretty_print_default() {
    let pp = PrettyPrintVisitor::default();
    assert!(pp.output().is_empty());
}

#[test]
fn pretty_print_multiline() {
    let (root, source) = sample_tree();
    let walker = TreeWalker::new(&source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let lines: Vec<_> = pp.output().lines().collect();
    assert!(lines.len() > 1);
}

// ==================================================
// 8. SearchVisitor
// ==================================================

#[test]
fn search_find_by_symbol() {
    let (root, source) = sample_tree();
    let walker = TreeWalker::new(&source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 2);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
    assert_eq!(sv.matches[0].0, 1); // start_byte
    assert_eq!(sv.matches[0].1, 2); // end_byte
}

#[test]
fn search_no_match() {
    let (root, source) = sample_tree();
    let walker = TreeWalker::new(&source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 999);
    walker.walk(&root, &mut sv);
    assert!(sv.matches.is_empty());
}

#[test]
fn search_all_named() {
    let (root, source) = sample_tree();
    let walker = TreeWalker::new(&source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.is_named());
    walker.walk(&root, &mut sv);
    // root(named), a(named), mid(named), b(named), d(named) = 5
    // c is unnamed
    assert_eq!(sv.matches.len(), 5);
}

#[test]
fn search_multiple_matches() {
    let source = b"abc".to_vec();
    let root = interior(100, vec![leaf(5, 0, 1), leaf(5, 1, 2), leaf(5, 2, 3)]);
    let walker = TreeWalker::new(&source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 5);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 3);
}

#[test]
fn search_with_bfs() {
    let (root, source) = sample_tree();
    let walker = BreadthFirstWalker::new(&source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 4);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
}

// ==================================================
// 9. VisitorAction::Skip
// ==================================================

#[test]
fn skip_children_dfs() {
    let (root, source) = sample_tree();
    let walker = TreeWalker::new(&source);
    let mut v = SkipWhen {
        predicate: |n: &ParsedNode| n.symbol() == 11, // skip mid's children
        entered: vec![],
    };
    walker.walk(&root, &mut v);
    // root(10), a(1), mid(11) [skip], d(4) — b(2) and c(3) skipped
    assert_eq!(v.entered, vec![10, 1, 11, 4]);
}

#[test]
fn skip_children_bfs() {
    let (root, source) = sample_tree();
    let walker = BreadthFirstWalker::new(&source);
    let mut v = SkipWhen {
        predicate: |n: &ParsedNode| n.symbol() == 11,
        entered: vec![],
    };
    walker.walk(&root, &mut v);
    // BFS: root(10), a(1), mid(11) [skip children], d(4)
    assert_eq!(v.entered, vec![10, 1, 11, 4]);
}

#[test]
fn skip_root_children_dfs() {
    let (root, source) = sample_tree();
    let walker = TreeWalker::new(&source);
    let mut v = SkipWhen {
        predicate: |n: &ParsedNode| n.symbol() == 10, // skip root's children
        entered: vec![],
    };
    walker.walk(&root, &mut v);
    assert_eq!(v.entered, vec![10]); // only root entered
}

#[test]
fn skip_leaf_is_noop() {
    let (root, source) = single_leaf_tree();
    let walker = TreeWalker::new(&source);
    let mut v = SkipWhen {
        predicate: |_: &ParsedNode| true,
        entered: vec![],
    };
    walker.walk(&root, &mut v);
    assert_eq!(v.entered, vec![1]); // leaf still entered
}

// ==================================================
// 10. VisitorAction::Stop
// ==================================================

#[test]
fn stop_after_first_dfs() {
    let (root, source) = sample_tree();
    let walker = TreeWalker::new(&source);
    let mut v = StopAfterN::new(1);
    walker.walk(&root, &mut v);
    assert_eq!(v.seen, vec![10]);
}

#[test]
fn stop_after_two_dfs() {
    // In DFS, Stop only returns from current walk_node; parent loop continues siblings
    let (root, source) = sample_tree();
    let walker = TreeWalker::new(&source);
    let mut v = StopAfterN::new(2);
    walker.walk(&root, &mut v);
    // root(10)->Continue, a(1)->Stop, mid(11)->Stop, d(4)->Stop
    assert_eq!(v.seen, vec![10, 1, 11, 4]);
}

#[test]
fn stop_after_three_dfs() {
    // Stop at count=3 prevents mid's children but siblings still visited
    let (root, source) = sample_tree();
    let walker = TreeWalker::new(&source);
    let mut v = StopAfterN::new(3);
    walker.walk(&root, &mut v);
    // root(10)->Continue, a(1)->Continue, mid(11)->Stop, d(4)->Stop
    assert_eq!(v.seen, vec![10, 1, 11, 4]);
}

#[test]
fn stop_after_first_bfs() {
    let (root, source) = sample_tree();
    let walker = BreadthFirstWalker::new(&source);
    let mut v = StopAfterN::new(1);
    walker.walk(&root, &mut v);
    assert_eq!(v.seen, vec![10]);
}

#[test]
fn stop_after_two_bfs() {
    let (root, source) = sample_tree();
    let walker = BreadthFirstWalker::new(&source);
    let mut v = StopAfterN::new(2);
    walker.walk(&root, &mut v);
    assert_eq!(v.seen, vec![10, 1]);
}

#[test]
fn stop_never_reached_dfs() {
    let (root, source) = sample_tree();
    let walker = TreeWalker::new(&source);
    let mut v = StopAfterN::new(100);
    walker.walk(&root, &mut v);
    assert_eq!(v.seen.len(), 6); // all visited
}

// ==================================================
// 11. Large tree traversal (100+ nodes)
// ==================================================

fn wide_flat_tree(n: usize) -> (ParsedNode, Vec<u8>) {
    let source: Vec<u8> = (0..n).map(|_| b'x').collect();
    let children: Vec<_> = (0..n).map(|i| leaf(i as u16, i, i + 1)).collect();
    let root = interior(0, children);
    (root, source)
}

#[test]
fn large_tree_dfs_count() {
    let (root, source) = wide_flat_tree(200);
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 201); // root + 200 leaves
    assert_eq!(stats.leaf_nodes, 200);
}

#[test]
fn large_tree_bfs_count() {
    let (root, source) = wide_flat_tree(150);
    let walker = BreadthFirstWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 151);
}

#[test]
fn large_tree_search_finds_all() {
    let (root, source) = wide_flat_tree(100);
    let walker = TreeWalker::new(&source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.is_named());
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 101); // root + 100 leaves all named
}

#[test]
fn large_tree_stop_early_bfs() {
    // BFS Stop truly halts the entire walk
    let (root, source) = wide_flat_tree(500);
    let walker = BreadthFirstWalker::new(&source);
    let mut v = StopAfterN::new(10);
    walker.walk(&root, &mut v);
    assert_eq!(v.seen.len(), 10);
}

#[test]
fn large_tree_pretty_print() {
    let (root, source) = wide_flat_tree(100);
    let walker = TreeWalker::new(&source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(pp.output().len() > 100);
}

// ==================================================
// 12. Deep tree traversal (50+ depth)
// ==================================================

fn deep_chain_tree(depth: usize) -> (ParsedNode, Vec<u8>) {
    let source = b"x".to_vec();
    let mut current = leaf(0, 0, 1);
    for i in 1..depth {
        current = interior(i as u16, vec![current]);
    }
    (current, source)
}

#[test]
fn deep_tree_dfs_depth() {
    let (root, source) = deep_chain_tree(55);
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 55);
    assert_eq!(stats.max_depth, 55);
}

#[test]
fn deep_tree_bfs_count() {
    let (root, source) = deep_chain_tree(60);
    let walker = BreadthFirstWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 60);
}

#[test]
fn deep_tree_search() {
    let (root, source) = deep_chain_tree(50);
    let walker = TreeWalker::new(&source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 0);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1); // only the deepest leaf has symbol 0
}

#[test]
fn deep_tree_stop_midway() {
    let (root, source) = deep_chain_tree(100);
    let walker = TreeWalker::new(&source);
    let mut v = StopAfterN::new(25);
    walker.walk(&root, &mut v);
    assert_eq!(v.seen.len(), 25);
}

#[test]
fn deep_tree_pretty_print() {
    let (root, source) = deep_chain_tree(50);
    let walker = TreeWalker::new(&source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    // Each level adds indentation
    let max_indent = pp
        .output()
        .lines()
        .map(|l| l.len() - l.trim_start().len())
        .max()
        .unwrap_or(0);
    assert!(max_indent >= 90); // 50 levels * 2 spaces each ~= 98
}

// ==================================================
// 13. Empty tree edge cases
// ==================================================

#[test]
fn empty_interior_node_dfs() {
    let root = interior(42, vec![]);
    let source = b"".to_vec();
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    // interior with no children acts like a leaf for visit_leaf
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn empty_interior_node_bfs() {
    let root = interior(42, vec![]);
    let source = b"".to_vec();
    let walker = BreadthFirstWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 1);
}

#[test]
fn empty_source_pretty_print() {
    let root = interior(42, vec![]);
    let source = b"".to_vec();
    let walker = TreeWalker::new(&source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(!pp.output().is_empty());
}

#[test]
fn empty_source_search() {
    let root = interior(42, vec![]);
    let source = b"".to_vec();
    let walker = TreeWalker::new(&source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 42);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
}

// ==================================================
// 14. Single-node trees
// ==================================================

#[test]
fn single_leaf_stats() {
    let (root, source) = single_leaf_tree();
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
    assert_eq!(stats.max_depth, 1);
    assert_eq!(stats.error_nodes, 0);
}

#[test]
fn single_leaf_search_hit() {
    let (root, source) = single_leaf_tree();
    let walker = TreeWalker::new(&source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
}

#[test]
fn single_leaf_search_miss() {
    let (root, source) = single_leaf_tree();
    let walker = TreeWalker::new(&source);
    let mut sv = SearchVisitor::new(|_: &ParsedNode| false);
    walker.walk(&root, &mut sv);
    assert!(sv.matches.is_empty());
}

#[test]
fn single_leaf_pretty_print() {
    let (root, source) = single_leaf_tree();
    let walker = TreeWalker::new(&source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(pp.output().contains("\"x\""));
}

#[test]
fn single_leaf_bfs_stats() {
    let (root, source) = single_leaf_tree();
    let walker = BreadthFirstWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 1);
}

// ==================================================
// Additional combination / edge case tests
// ==================================================

#[test]
fn arena_node_handle_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(NodeHandle::new(0, 0));
    set.insert(NodeHandle::new(0, 1));
    set.insert(NodeHandle::new(0, 0)); // duplicate
    assert_eq!(set.len(), 2);
}

#[test]
fn arena_capacity_after_alloc() {
    let mut arena = TreeArena::with_capacity(4);
    let initial_cap = arena.capacity();
    for i in 0..4 {
        arena.alloc(TreeNode::leaf(i));
    }
    assert!(arena.capacity() >= initial_cap);
}

#[test]
fn arena_leaf_value_negative() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(-42));
    assert_eq!(arena.get(h).value(), -42);
}

#[test]
fn arena_leaf_value_zero() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(0));
    assert_eq!(arena.get(h).value(), 0);
}

#[test]
fn arena_branch_children_order() {
    let mut arena = TreeArena::new();
    let c1 = arena.alloc(TreeNode::leaf(10));
    let c2 = arena.alloc(TreeNode::leaf(20));
    let c3 = arena.alloc(TreeNode::leaf(30));
    let parent = arena.alloc(TreeNode::branch(vec![c1, c2, c3]));
    let binding = arena.get(parent);
    let children = binding.children();
    assert_eq!(children[0], c1);
    assert_eq!(children[1], c2);
    assert_eq!(children[2], c3);
}

#[test]
fn visitor_action_variants_distinct() {
    assert_ne!(VisitorAction::Continue, VisitorAction::SkipChildren);
    assert_ne!(VisitorAction::Continue, VisitorAction::Stop);
    assert_ne!(VisitorAction::SkipChildren, VisitorAction::Stop);
}

#[test]
fn visitor_action_clone() {
    let a = VisitorAction::Continue;
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn visitor_action_debug() {
    let dbg = format!("{:?}", VisitorAction::Stop);
    assert!(dbg.contains("Stop"));
}

#[test]
fn dfs_walker_creation() {
    let source = b"hello";
    let _walker = TreeWalker::new(source);
}

#[test]
fn bfs_walker_creation() {
    let source = b"hello";
    let _walker = BreadthFirstWalker::new(source);
}

#[test]
fn stats_visitor_default() {
    let s = StatsVisitor::default();
    assert_eq!(s.total_nodes, 0);
    assert_eq!(s.leaf_nodes, 0);
    assert_eq!(s.error_nodes, 0);
    assert_eq!(s.max_depth, 0);
    assert!(s.node_counts.is_empty());
}

#[test]
fn dfs_tree_with_errors_and_leaves() {
    let source = b"abcd".to_vec();
    let root = interior(
        100,
        vec![
            leaf(1, 0, 1),
            error_node(1, 2),
            leaf(2, 2, 3),
            error_node(3, 4),
        ],
    );
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 2);
    assert_eq!(stats.total_nodes, 3); // root + 2 non-error leaves
}

#[test]
fn bfs_skip_and_stop_combined() {
    // Use skip on mid, ensuring stop isn't triggered
    let (root, source) = sample_tree();
    let walker = BreadthFirstWalker::new(&source);
    struct SkipAndCount {
        count: usize,
    }
    impl Visitor for SkipAndCount {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.count += 1;
            if node.symbol() == 11 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }
    let mut v = SkipAndCount { count: 0 };
    walker.walk(&root, &mut v);
    // BFS: root(10), a(1), mid(11)[skip], d(4) = 4 nodes
    assert_eq!(v.count, 4);
}

#[test]
fn search_match_records_kind_string() {
    let (root, source) = sample_tree();
    let walker = TreeWalker::new(&source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 10);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
    // kind string from fallback for symbol 10
    assert_eq!(sv.matches[0].2, "rule_10");
}

#[test]
fn pretty_print_error_node() {
    let root = interior(100, vec![error_node(0, 1), leaf(1, 1, 2)]);
    let source = b"xx".to_vec();
    let walker = TreeWalker::new(&source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    // Error node is handled by visit_error, not enter_node
    // PrettyPrintVisitor doesn't normally output error nodes through enter_node
    // but we just verify no panic and output is non-empty
    assert!(!pp.output().is_empty());
}

#[test]
fn arena_multiple_resets() {
    let mut arena = TreeArena::new();
    for _ in 0..5 {
        for i in 0..10 {
            arena.alloc(TreeNode::leaf(i));
        }
        assert_eq!(arena.len(), 10);
        arena.reset();
        assert_eq!(arena.len(), 0);
    }
}

#[test]
fn arena_node_ref_deref() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(42));
    let node_ref = arena.get(h);
    // Deref gives access to TreeNode methods
    assert_eq!(node_ref.value(), 42);
    assert!(node_ref.is_leaf());
}

#[test]
fn dfs_unnamed_nodes_not_named() {
    let source = b"x".to_vec();
    let root = unnamed_leaf(1, 0, 1);
    let walker = TreeWalker::new(&source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.is_named());
    walker.walk(&root, &mut sv);
    assert!(sv.matches.is_empty());
}

#[test]
fn deep_tree_leaf_has_zero_children() {
    let (root, source) = deep_chain_tree(10);
    let walker = TreeWalker::new(&source);
    struct LeafCounter(usize);
    impl Visitor for LeafCounter {
        fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn visit_leaf(&mut self, node: &ParsedNode, _text: &str) {
            assert_eq!(node.child_count(), 0);
            self.0 += 1;
        }
    }
    let mut v = LeafCounter(0);
    walker.walk(&root, &mut v);
    assert_eq!(v.0, 1); // only deepest node is a leaf
}

#[test]
fn arena_tree_node_debug() {
    let n = TreeNode::leaf(1);
    let dbg = format!("{:?}", n);
    assert!(dbg.contains("Leaf"));
}

#[test]
fn arena_tree_node_branch_debug() {
    let n = TreeNode::branch(vec![]);
    let dbg = format!("{:?}", n);
    assert!(dbg.contains("Branch"));
}

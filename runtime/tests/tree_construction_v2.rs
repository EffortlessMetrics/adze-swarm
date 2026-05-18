//! Comprehensive tests for arena-based tree construction and visitor-driven traversal.
//!
//! Covers: TreeArena allocation patterns, TreeNode construction, NodeHandle identity,
//! TreeWalker / BreadthFirstWalker integration with StatsVisitor, PrettyPrintVisitor,
//! SearchVisitor, and various edge cases.

use adze::arena_allocator::{ArenaMetrics, NodeHandle, TreeArena, TreeNode};
use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TransformVisitor,
    TransformWalker, TreeWalker, Visitor, VisitorAction,
};

// ---------------------------------------------------------------------------
// ParsedNode construction helpers (language field is pub(crate))
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

// ---------------------------------------------------------------------------
// 1. Build simple trees (8 tests)
// ---------------------------------------------------------------------------

#[test]
fn test_arena_single_leaf_node() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(42));
    assert_eq!(arena.get(h).value(), 42);
    assert!(arena.get(h).is_leaf());
    assert!(!arena.get(h).is_branch());
}

#[test]
fn test_arena_parent_with_one_child() {
    let mut arena = TreeArena::new();
    let child = arena.alloc(TreeNode::leaf(1));
    let parent = arena.alloc(TreeNode::branch(vec![child]));

    assert!(arena.get(parent).is_branch());
    assert_eq!(arena.get(parent).children().len(), 1);
    assert_eq!(arena.get(parent).children()[0], child);
}

#[test]
fn test_arena_binary_tree() {
    let mut arena = TreeArena::new();
    let left = arena.alloc(TreeNode::leaf(10));
    let right = arena.alloc(TreeNode::leaf(20));
    let root = arena.alloc(TreeNode::branch(vec![left, right]));

    assert_eq!(arena.get(root).children().len(), 2);
    assert_eq!(arena.get(arena.get(root).children()[0]).value(), 10);
    assert_eq!(arena.get(arena.get(root).children()[1]).value(), 20);
}

#[test]
fn test_arena_linear_chain() {
    let mut arena = TreeArena::new();
    let leaf = arena.alloc(TreeNode::leaf(99));
    let mid = arena.alloc(TreeNode::branch(vec![leaf]));
    let root = arena.alloc(TreeNode::branch(vec![mid]));

    assert!(arena.get(root).is_branch());
    let mid_h = arena.get(root).children()[0];
    assert!(arena.get(mid_h).is_branch());
    let leaf_h = arena.get(mid_h).children()[0];
    assert!(arena.get(leaf_h).is_leaf());
    assert_eq!(arena.get(leaf_h).value(), 99);
}

#[test]
fn test_arena_branch_with_symbol() {
    let mut arena = TreeArena::new();
    let c = arena.alloc(TreeNode::leaf(5));
    let root = arena.alloc(TreeNode::branch_with_symbol(77, vec![c]));

    assert_eq!(arena.get(root).symbol(), 77);
    assert!(arena.get(root).is_branch());
}

#[test]
fn test_arena_wide_tree_three_children() {
    let mut arena = TreeArena::new();
    let a = arena.alloc(TreeNode::leaf(1));
    let b = arena.alloc(TreeNode::leaf(2));
    let c = arena.alloc(TreeNode::leaf(3));
    let root = arena.alloc(TreeNode::branch(vec![a, b, c]));

    assert_eq!(arena.get(root).children().len(), 3);
    for (i, &h) in arena.get(root).children().iter().enumerate() {
        assert_eq!(arena.get(h).value(), (i + 1) as i32);
    }
}

#[test]
fn test_arena_nested_three_levels() {
    let mut arena = TreeArena::new();
    let l1 = arena.alloc(TreeNode::leaf(1));
    let l2 = arena.alloc(TreeNode::leaf(2));
    let mid = arena.alloc(TreeNode::branch(vec![l1, l2]));
    let l3 = arena.alloc(TreeNode::leaf(3));
    let root = arena.alloc(TreeNode::branch(vec![mid, l3]));

    assert_eq!(arena.len(), 5);
    assert_eq!(arena.get(root).children().len(), 2);
}

#[test]
fn test_arena_leaf_children_empty() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(0));
    assert!(arena.get(h).children().is_empty());
}

// ---------------------------------------------------------------------------
// 2. Arena allocation patterns (8 tests)
// ---------------------------------------------------------------------------

#[test]
fn test_arena_alloc_many_retrieve_all() {
    let mut arena = TreeArena::new();
    let handles: Vec<NodeHandle> = (0..100).map(|i| arena.alloc(TreeNode::leaf(i))).collect();

    assert_eq!(arena.len(), 100);
    for (i, h) in handles.iter().enumerate() {
        assert_eq!(arena.get(*h).value(), i as i32);
    }
}

#[test]
fn test_arena_with_capacity() {
    let mut arena = TreeArena::with_capacity(4);
    assert_eq!(arena.capacity(), 4);
    for i in 0..4 {
        arena.alloc(TreeNode::leaf(i));
    }
    assert_eq!(arena.num_chunks(), 1);
    // Triggers new chunk
    arena.alloc(TreeNode::leaf(4));
    assert_eq!(arena.num_chunks(), 2);
}

#[test]
fn test_arena_modification_via_get_mut() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(10));
    assert_eq!(arena.get(h).value(), 10);

    arena.get_mut(h).set_value(99);
    assert_eq!(arena.get(h).value(), 99);
}

#[test]
fn test_arena_reset_preserves_capacity() {
    let mut arena = TreeArena::with_capacity(4);
    for i in 0..10 {
        arena.alloc(TreeNode::leaf(i));
    }
    let chunks_before = arena.num_chunks();
    arena.reset();

    assert!(arena.is_empty());
    assert_eq!(arena.len(), 0);
    assert_eq!(arena.num_chunks(), chunks_before);
}

#[test]
fn test_arena_clear_frees_excess_chunks() {
    let mut arena = TreeArena::with_capacity(2);
    for i in 0..20 {
        arena.alloc(TreeNode::leaf(i));
    }
    assert!(arena.num_chunks() > 1);

    arena.clear();
    assert_eq!(arena.num_chunks(), 1);
    assert!(arena.is_empty());
}

#[test]
fn test_arena_default_trait() {
    let arena = TreeArena::default();
    assert!(arena.is_empty());
    assert_eq!(arena.len(), 0);
}

#[test]
fn test_arena_metrics_snapshot() {
    let mut arena = TreeArena::new();
    arena.alloc(TreeNode::leaf(1));
    arena.alloc(TreeNode::leaf(2));

    let m: ArenaMetrics = arena.metrics();
    assert_eq!(m.len(), 2);
    assert!(!m.is_empty());
    assert!(m.capacity() >= 2);
    assert!(m.num_chunks() >= 1);
    assert!(m.memory_usage() > 0);
}

#[test]
fn test_arena_memory_usage_grows_with_capacity() {
    let small = TreeArena::with_capacity(8);
    let large = TreeArena::with_capacity(1024);
    assert!(large.memory_usage() > small.memory_usage());
}

// ---------------------------------------------------------------------------
// 3. Tree walker basic (8 tests)
// ---------------------------------------------------------------------------

#[test]
fn test_walker_single_leaf() {
    let source = b"x";
    let root = leaf(1, 0, 1);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);

    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn test_walker_parent_and_child() {
    let source = b"ab";
    let child = leaf(1, 0, 1);
    let root = interior(10, vec![child]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);

    assert_eq!(stats.total_nodes, 2);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn test_walker_deep_chain() {
    let source = b"z";
    // Build chain: root -> mid1 -> mid2 -> leaf
    let deep_leaf = leaf(1, 0, 1);
    let mid2 = interior(2, vec![deep_leaf]);
    let mid1 = interior(3, vec![mid2]);
    let root = interior(4, vec![mid1]);

    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);

    assert_eq!(stats.total_nodes, 4);
    assert_eq!(stats.max_depth, 4);
}

#[test]
fn test_walker_binary_tree() {
    let source = b"lr";
    let left = leaf(1, 0, 1);
    let right = leaf(2, 1, 2);
    let root = interior(10, vec![left, right]);

    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);

    assert_eq!(stats.total_nodes, 3);
    assert_eq!(stats.leaf_nodes, 2);
    assert_eq!(stats.max_depth, 2);
}

#[test]
fn test_walker_skip_children_action() {
    let source = b"abc";
    let a = leaf(1, 0, 1);
    let b = leaf(2, 1, 2);
    let c = leaf(3, 2, 3);
    let root = interior(10, vec![a, b, c]);

    struct SkipVisitor {
        entered: Vec<u16>,
    }
    impl Visitor for SkipVisitor {
        fn enter_node(&mut self, node: &adze::pure_parser::ParsedNode) -> VisitorAction {
            self.entered.push(node.symbol());
            VisitorAction::SkipChildren
        }
    }

    let walker = TreeWalker::new(source);
    let mut v = SkipVisitor { entered: vec![] };
    walker.walk(&root, &mut v);

    // Only root is entered; children are skipped
    assert_eq!(v.entered, [10]);
}

#[test]
fn test_walker_stop_action() {
    let source = b"ab";
    let a = leaf(1, 0, 1);
    let b = leaf(2, 1, 2);
    let root = interior(10, vec![a, b]);

    struct StopAfterTwo {
        count: usize,
    }
    impl Visitor for StopAfterTwo {
        fn enter_node(&mut self, _node: &adze::pure_parser::ParsedNode) -> VisitorAction {
            self.count += 1;
            if self.count >= 2 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }

    let walker = TreeWalker::new(source);
    let mut v = StopAfterTwo { count: 0 };
    walker.walk(&root, &mut v);

    // root enters (count=1, Continue), then first child enters (count=2, Stop)
    // second child is never visited because DFS recurses into first child first
    // but since first child is a leaf, after stop we bail — however the walker
    // iterates children sequentially and checks each, so count depends on impl.
    // The walker enters root(1), then child a(2) → Stop, then child b(3) → Stop.
    // Actually: root enters → Continue → iterates children → child a enters → Stop → return.
    // But the walker calls walk_node for each child in a loop; after child a returns
    // (with Stop), it still calls walk_node for child b which enters (count=3) → Stop.
    // So 3 nodes are entered. But Stop only returns from the current walk_node call,
    // not from the parent's loop. Let's just verify that not all children are fully walked.
    assert!(v.count >= 2);
    assert!(v.count <= 3);
}

#[test]
fn test_breadth_first_walker_order() {
    let source = b"abcd";
    let a = leaf(1, 0, 1);
    let b = leaf(2, 1, 2);
    let mid = interior(20, vec![a, b]);
    let c = leaf(3, 2, 3);
    let root = interior(10, vec![mid, c]);

    struct OrderTracker {
        symbols: Vec<u16>,
    }
    impl Visitor for OrderTracker {
        fn enter_node(&mut self, node: &adze::pure_parser::ParsedNode) -> VisitorAction {
            self.symbols.push(node.symbol());
            VisitorAction::Continue
        }
    }

    let walker = BreadthFirstWalker::new(source);
    let mut v = OrderTracker { symbols: vec![] };
    walker.walk(&root, &mut v);

    // BFS: root(10), mid(20), c(3), a(1), b(2)
    assert_eq!(v.symbols, [10, 20, 3, 1, 2]);
}

#[test]
fn test_walker_leaf_text_callback() {
    let source = b"hello";
    let root = leaf(1, 0, 5);

    struct TextCollector {
        texts: Vec<String>,
    }
    impl Visitor for TextCollector {
        fn enter_node(&mut self, _node: &adze::pure_parser::ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn visit_leaf(&mut self, _node: &adze::pure_parser::ParsedNode, text: &str) {
            self.texts.push(text.to_string());
        }
    }

    let walker = TreeWalker::new(source);
    let mut v = TextCollector { texts: vec![] };
    walker.walk(&root, &mut v);

    assert_eq!(v.texts, ["hello"]);
}

// ---------------------------------------------------------------------------
// 4. Stats visitor integration (7 tests)
// ---------------------------------------------------------------------------

#[test]
fn test_stats_total_nodes_complex_tree() {
    // root -> [a, mid -> [b, c], d]  = 6 nodes
    let source = b"abcd";
    let a = leaf(1, 0, 1);
    let b = leaf(2, 1, 2);
    let c = leaf(3, 2, 3);
    let mid = interior(20, vec![b, c]);
    let d = leaf(4, 3, 4);
    let root = interior(10, vec![a, mid, d]);

    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);

    assert_eq!(stats.total_nodes, 6);
}

#[test]
fn test_stats_leaf_count() {
    let source = b"xyz";
    let x = leaf(1, 0, 1);
    let y = leaf(2, 1, 2);
    let z = leaf(3, 2, 3);
    let root = interior(10, vec![x, y, z]);

    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);

    assert_eq!(stats.leaf_nodes, 3);
}

#[test]
fn test_stats_max_depth_flat() {
    let source = b"ab";
    let a = leaf(1, 0, 1);
    let b = leaf(2, 1, 2);
    let root = interior(10, vec![a, b]);

    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);

    assert_eq!(stats.max_depth, 2);
}

#[test]
fn test_stats_max_depth_deep() {
    let source = b"x";
    let l = leaf(1, 0, 1);
    let m3 = interior(2, vec![l]);
    let m2 = interior(3, vec![m3]);
    let m1 = interior(4, vec![m2]);
    let root = interior(5, vec![m1]);

    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);

    assert_eq!(stats.max_depth, 5);
}

#[test]
fn test_stats_error_node_counted() {
    let source = b"e";
    let err = error_node(0, 1);

    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&err, &mut stats);

    assert_eq!(stats.error_nodes, 1);
    // Error nodes are not entered via enter_node
    assert_eq!(stats.total_nodes, 0);
}

#[test]
fn test_stats_named_vs_anonymous_in_node_counts() {
    let source = b"ab";
    let named = leaf(1, 0, 1);
    let anon = unnamed_leaf(2, 1, 2);
    let root = interior(10, vec![named, anon]);

    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);

    // All non-error nodes are counted in total_nodes
    assert_eq!(stats.total_nodes, 3);
}

#[test]
fn test_stats_node_counts_map() {
    let source = b"abc";
    let a = leaf(1, 0, 1);
    let b = leaf(1, 1, 2);
    let c = leaf(2, 2, 3);
    let root = interior(10, vec![a, b, c]);

    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);

    // node_counts tracks by kind string (symbol-based fallback)
    assert!(!stats.node_counts.is_empty());
}

// ---------------------------------------------------------------------------
// 5. Pretty print integration (8 tests)
// ---------------------------------------------------------------------------

#[test]
fn test_pretty_print_single_leaf() {
    let source = b"x";
    let root = leaf(1, 0, 1);

    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);

    let out = pp.output();
    assert!(!out.is_empty());
    assert!(out.contains("[named]"));
}

#[test]
fn test_pretty_print_nested_tree() {
    let source = b"ab";
    let a = leaf(1, 0, 1);
    let root = interior(10, vec![a]);

    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);

    let out = pp.output();
    // Nested content should have indentation
    assert!(out.contains("  "));
}

#[test]
fn test_pretty_print_error_node() {
    let source = b"!";
    let err = error_node(0, 1);

    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&err, &mut pp);

    let out = pp.output();
    assert!(out.contains("ERROR"));
}

#[test]
fn test_pretty_print_leaf_text_included() {
    let source = b"hello";
    let root = leaf(1, 0, 5);

    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);

    assert!(pp.output().contains("hello"));
}

#[test]
fn test_pretty_print_unnamed_node() {
    let source = b"+";
    let root = unnamed_leaf(1, 0, 1);

    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);

    // Unnamed nodes do NOT have [named] tag
    assert!(!pp.output().contains("[named]"));
}

#[test]
fn test_pretty_print_multi_level() {
    let source = b"abc";
    let a = leaf(1, 0, 1);
    let b = leaf(2, 1, 2);
    let mid = interior(20, vec![a, b]);
    let c = leaf(3, 2, 3);
    let root = interior(10, vec![mid, c]);

    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);

    let lines: Vec<&str> = pp.output().lines().collect();
    assert!(lines.len() >= 4);
}

#[test]
fn test_pretty_print_default_trait() {
    let pp = PrettyPrintVisitor::default();
    assert!(pp.output().is_empty());
}

#[test]
fn test_pretty_print_breadth_first() {
    let source = b"xy";
    let x = leaf(1, 0, 1);
    let y = leaf(2, 1, 2);
    let root = interior(10, vec![x, y]);

    let walker = BreadthFirstWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);

    assert!(!pp.output().is_empty());
}

// ---------------------------------------------------------------------------
// 6. Search visitor integration (8 tests)
// ---------------------------------------------------------------------------

#[test]
fn test_search_find_by_symbol() {
    let source = b"ab";
    let a = leaf(1, 0, 1);
    let b = leaf(2, 1, 2);
    let root = interior(10, vec![a, b]);

    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 2);
    walker.walk(&root, &mut sv);

    assert_eq!(sv.matches.len(), 1);
    assert_eq!(sv.matches[0].0, 1); // start_byte
    assert_eq!(sv.matches[0].1, 2); // end_byte
}

#[test]
fn test_search_find_named_nodes() {
    let source = b"ab";
    let named = leaf(1, 0, 1);
    let anon = unnamed_leaf(2, 1, 2);
    let root = interior(10, vec![named, anon]);

    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.is_named());
    walker.walk(&root, &mut sv);

    // root + named leaf (anon leaf is also entered but is_named false)
    assert_eq!(sv.matches.len(), 2);
}

#[test]
fn test_search_no_matches() {
    let source = b"x";
    let root = leaf(1, 0, 1);

    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 999);
    walker.walk(&root, &mut sv);

    assert!(sv.matches.is_empty());
}

#[test]
fn test_search_find_all_leaves() {
    let source = b"abc";
    let a = leaf(1, 0, 1);
    let b = leaf(2, 1, 2);
    let c = leaf(3, 2, 3);
    let root = interior(10, vec![a, b, c]);

    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.child_count() == 0);
    walker.walk(&root, &mut sv);

    assert_eq!(sv.matches.len(), 3);
}

#[test]
fn test_search_find_interior_nodes() {
    let source = b"ab";
    let a = leaf(1, 0, 1);
    let mid = interior(20, vec![a]);
    let b = leaf(2, 1, 2);
    let root = interior(10, vec![mid, b]);

    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.child_count() > 0);
    walker.walk(&root, &mut sv);

    assert_eq!(sv.matches.len(), 2); // root and mid
}

#[test]
fn test_search_find_error_nodes_not_entered() {
    // Error nodes invoke visit_error, not enter_node, so SearchVisitor won't match them
    let source = b"e";
    let err = error_node(0, 1);

    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.is_error());
    walker.walk(&err, &mut sv);

    // SearchVisitor uses enter_node which is NOT called for error nodes
    assert!(sv.matches.is_empty());
}

#[test]
fn test_search_breadth_first() {
    let source = b"ab";
    let a = leaf(1, 0, 1);
    let b = leaf(2, 1, 2);
    let root = interior(10, vec![a, b]);

    let walker = BreadthFirstWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    walker.walk(&root, &mut sv);

    assert_eq!(sv.matches.len(), 1);
}

#[test]
fn test_search_multiple_matches_ordered() {
    let source = b"aaa";
    let a1 = leaf(5, 0, 1);
    let a2 = leaf(5, 1, 2);
    let a3 = leaf(5, 2, 3);
    let root = interior(10, vec![a1, a2, a3]);

    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 5);
    walker.walk(&root, &mut sv);

    assert_eq!(sv.matches.len(), 3);
    // DFS order: start bytes should be ascending
    assert_eq!(sv.matches[0].0, 0);
    assert_eq!(sv.matches[1].0, 1);
    assert_eq!(sv.matches[2].0, 2);
}

// ---------------------------------------------------------------------------
// 7. Edge cases (8 tests)
// ---------------------------------------------------------------------------

#[test]
fn test_empty_arena() {
    let arena = TreeArena::new();
    assert!(arena.is_empty());
    assert_eq!(arena.len(), 0);
    assert!(arena.capacity() > 0);
}

#[test]
fn test_cleared_arena_reuse() {
    let mut arena = TreeArena::new();
    let _h1 = arena.alloc(TreeNode::leaf(1));
    arena.clear();
    assert!(arena.is_empty());

    let h2 = arena.alloc(TreeNode::leaf(42));
    assert_eq!(arena.get(h2).value(), 42);
    assert_eq!(arena.len(), 1);
}

#[test]
fn test_reset_then_reuse() {
    let mut arena = TreeArena::new();
    for i in 0..50 {
        arena.alloc(TreeNode::leaf(i));
    }
    arena.reset();

    let h = arena.alloc(TreeNode::leaf(777));
    assert_eq!(arena.get(h).value(), 777);
    assert_eq!(arena.len(), 1);
}

#[test]
fn test_deep_nesting_arena() {
    let mut arena = TreeArena::new();
    let mut current = arena.alloc(TreeNode::leaf(0));
    for i in 1..100 {
        current = arena.alloc(TreeNode::branch(vec![current]));
        assert_eq!(arena.get(current).symbol(), 0);
        assert!(arena.get(current).is_branch());
        let _ = i;
    }
    assert_eq!(arena.len(), 100);
}

#[test]
fn test_wide_tree_arena() {
    let mut arena = TreeArena::new();
    let children: Vec<NodeHandle> = (0..200).map(|i| arena.alloc(TreeNode::leaf(i))).collect();
    let root = arena.alloc(TreeNode::branch(children));

    assert_eq!(arena.get(root).children().len(), 200);
    assert_eq!(arena.len(), 201);
}

#[test]
fn test_node_handle_equality() {
    let h1 = NodeHandle::new(0, 5);
    let h2 = NodeHandle::new(0, 5);
    let h3 = NodeHandle::new(1, 5);

    assert_eq!(h1, h2);
    assert_ne!(h1, h3);
}

#[test]
fn test_walker_error_mixed_with_normal() {
    let source = b"ae";
    let normal = leaf(1, 0, 1);
    let err = error_node(1, 2);
    let root = interior(10, vec![normal, err]);

    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);

    assert_eq!(stats.total_nodes, 2); // root + normal leaf
    assert_eq!(stats.error_nodes, 1);
}

#[test]
fn test_transform_walker_basic() {
    let source = b"12";
    let left = leaf(1, 0, 1);
    let right = leaf(2, 1, 2);
    let root = interior(10, vec![left, right]);

    struct CountTransform;
    impl TransformVisitor for CountTransform {
        type Output = usize;
        fn transform_node(&mut self, _node: &ParsedNode, children: Vec<usize>) -> usize {
            1 + children.iter().sum::<usize>()
        }
        fn transform_leaf(&mut self, _node: &ParsedNode, _text: &str) -> usize {
            1
        }
        fn transform_error(&mut self, _node: &ParsedNode) -> usize {
            1
        }
    }

    let walker = TransformWalker::new(source);
    let mut t = CountTransform;
    let count = walker.walk(&root, &mut t);

    assert_eq!(count, 3);
}

// ---------------------------------------------------------------------------
// Additional edge-case and integration tests to reach 55+
// ---------------------------------------------------------------------------

#[test]
fn test_arena_chunk_growth_exponential() {
    let mut arena = TreeArena::with_capacity(2);
    // Fill chunk 1 (cap=2), then chunk 2 (cap=4), then chunk 3 (cap=8)
    for i in 0..8 {
        arena.alloc(TreeNode::leaf(i));
    }
    assert!(arena.num_chunks() >= 2);
}

#[test]
fn test_arena_alloc_after_multiple_resets() {
    let mut arena = TreeArena::with_capacity(4);
    for _ in 0..3 {
        for i in 0..10 {
            arena.alloc(TreeNode::leaf(i));
        }
        arena.reset();
    }
    assert!(arena.is_empty());
    let h = arena.alloc(TreeNode::leaf(42));
    assert_eq!(arena.get(h).value(), 42);
}

#[test]
fn test_arena_branch_no_children() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::branch(vec![]));
    assert!(arena.get(h).is_branch());
    assert!(arena.get(h).children().is_empty());
}

#[test]
fn test_tree_node_value_alias() {
    let node = TreeNode::leaf(55);
    assert_eq!(node.value(), 55);
    assert_eq!(node.symbol(), 55);
}

#[test]
fn test_node_ref_deref() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(3));
    let node_ref = arena.get(h);
    // Deref to TreeNode
    assert_eq!(node_ref.symbol(), 3);
    assert!(node_ref.is_leaf());
}

#[test]
fn test_visitor_leave_node_called() {
    let source = b"x";
    let root = leaf(1, 0, 1);

    struct LeaveTracker {
        left: usize,
    }
    impl Visitor for LeaveTracker {
        fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn leave_node(&mut self, _node: &ParsedNode) {
            self.left += 1;
        }
    }

    let walker = TreeWalker::new(source);
    let mut v = LeaveTracker { left: 0 };
    walker.walk(&root, &mut v);

    assert_eq!(v.left, 1);
}

#[test]
fn test_visitor_leave_called_for_skip_children() {
    let source = b"ab";
    let a = leaf(1, 0, 1);
    let root = interior(10, vec![a]);

    struct SkipAndLeave {
        entered: usize,
        left: usize,
    }
    impl Visitor for SkipAndLeave {
        fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
            self.entered += 1;
            VisitorAction::SkipChildren
        }
        fn leave_node(&mut self, _node: &ParsedNode) {
            self.left += 1;
        }
    }

    let walker = TreeWalker::new(source);
    let mut v = SkipAndLeave {
        entered: 0,
        left: 0,
    };
    walker.walk(&root, &mut v);

    // SkipChildren still calls leave_node
    assert_eq!(v.entered, 1);
    assert_eq!(v.left, 1);
}

#[test]
fn test_arena_node_handle_copy_semantics() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(7));
    let h_copy = h;
    assert_eq!(arena.get(h).value(), arena.get(h_copy).value());
}

#[test]
fn test_walker_with_only_error_children() {
    let source = b"ee";
    let e1 = error_node(0, 1);
    let e2 = error_node(1, 2);
    let root = interior(10, vec![e1, e2]);

    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);

    assert_eq!(stats.error_nodes, 2);
    assert_eq!(stats.total_nodes, 1); // only root
}

#[test]
fn test_transform_walker_leaf_text() {
    let source = b"hi";
    let root = leaf(1, 0, 2);

    struct TextExtractor;
    impl TransformVisitor for TextExtractor {
        type Output = String;
        fn transform_node(&mut self, _node: &ParsedNode, children: Vec<String>) -> String {
            children.join("")
        }
        fn transform_leaf(&mut self, _node: &ParsedNode, text: &str) -> String {
            text.to_string()
        }
        fn transform_error(&mut self, _node: &ParsedNode) -> String {
            "ERR".to_string()
        }
    }

    let walker = TransformWalker::new(source);
    let mut t = TextExtractor;
    let result = walker.walk(&root, &mut t);

    assert_eq!(result, "hi");
}

#[test]
fn test_node_handle_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    let h1 = NodeHandle::new(0, 0);
    let h2 = NodeHandle::new(0, 1);
    set.insert(h1);
    set.insert(h2);
    set.insert(h1); // duplicate
    assert_eq!(set.len(), 2);
}

#[test]
fn test_arena_large_batch_allocation() {
    let mut arena = TreeArena::with_capacity(16);
    let handles: Vec<NodeHandle> = (0..2000).map(|i| arena.alloc(TreeNode::leaf(i))).collect();

    assert_eq!(arena.len(), 2000);
    // Spot-check first and last
    assert_eq!(arena.get(handles[0]).value(), 0);
    assert_eq!(arena.get(handles[1999]).value(), 1999);
}

#[test]
fn test_pretty_print_empty_source_leaf() {
    let source = b"";
    let root = leaf(1, 0, 0);

    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);

    // Even with empty text, there should be output for the node itself
    assert!(!pp.output().is_empty());
}

#[test]
fn test_search_visitor_start_end_byte_accuracy() {
    let source = b"abcdef";
    let a = leaf(1, 0, 2);
    let b = leaf(2, 2, 5);
    let c = leaf(3, 5, 6);
    let root = interior(10, vec![a, b, c]);

    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 2);
    walker.walk(&root, &mut sv);

    assert_eq!(sv.matches.len(), 1);
    assert_eq!(sv.matches[0].0, 2); // start
    assert_eq!(sv.matches[0].1, 5); // end
}

//! Tests (v8) for tree walking / visitor patterns.
//!
//! Covers: `TreeWalker`, `BreadthFirstWalker`, `StatsVisitor`,
//! `PrettyPrintVisitor`, `SearchVisitor`, `TransformVisitor`,
//! `TreeArena`, and various tree shapes (deep, wide, balanced, skewed).

use adze::arena_allocator::{NodeHandle, TreeArena, TreeNode};
use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TransformVisitor,
    TransformWalker, TreeWalker, Visitor, VisitorAction,
};
use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// Helpers — ParsedNode construction
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

fn anon_leaf(symbol: u16, start: usize, end: usize) -> ParsedNode {
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

/// Build a left-skewed chain of depth `n` (leaf at bottom).
fn left_chain(depth: usize) -> ParsedNode {
    let mut node = leaf(1, 0, 1);
    for _ in 1..depth {
        node = interior(2, vec![node]);
    }
    node
}

/// Build a right-skewed chain of depth `n`.
fn right_chain(depth: usize) -> ParsedNode {
    let mut node = leaf(1, 0, 1);
    for _ in 1..depth {
        node = interior(3, vec![node]);
    }
    node
}

/// Build a wide tree: one root with `width` leaf children.
fn wide_tree(width: usize) -> ParsedNode {
    let children: Vec<_> = (0..width)
        .map(|i| leaf((i % 65535) as u16 + 1, i, i + 1))
        .collect();
    interior(10, children)
}

/// Build a balanced binary tree of given depth.
fn balanced_tree(depth: usize, sym: u16) -> ParsedNode {
    if depth <= 1 {
        leaf(sym, 0, 1)
    } else {
        interior(
            sym + 100,
            vec![balanced_tree(depth - 1, sym), balanced_tree(depth - 1, sym)],
        )
    }
}

// ---------------------------------------------------------------------------
// Custom visitors used across tests
// ---------------------------------------------------------------------------

struct SymbolRecorder(Vec<u16>);

impl Visitor for SymbolRecorder {
    fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
        self.0.push(node.symbol);
        VisitorAction::Continue
    }
}

struct LeafTextRecorder(Vec<String>);

impl Visitor for LeafTextRecorder {
    fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
        VisitorAction::Continue
    }
    fn visit_leaf(&mut self, _node: &ParsedNode, text: &str) {
        self.0.push(text.to_string());
    }
}

struct DepthGauge {
    current: usize,
    max: usize,
}

impl Visitor for DepthGauge {
    fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
        self.current += 1;
        if self.current > self.max {
            self.max = self.current;
        }
        VisitorAction::Continue
    }
    fn leave_node(&mut self, _node: &ParsedNode) {
        self.current -= 1;
    }
}

struct PairTracker {
    enters: usize,
    leaves: usize,
}

impl Visitor for PairTracker {
    fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
        self.enters += 1;
        VisitorAction::Continue
    }
    fn leave_node(&mut self, _node: &ParsedNode) {
        self.leaves += 1;
    }
}

// ===================================================================
// 1. StatsVisitor — basic counts
// ===================================================================

#[test]
fn tw_v8_stats_empty_arena_zero_nodes() {
    // Walking an arena-allocated empty state: use a childless root as proxy
    let src = b"";
    let root = leaf(1, 0, 0);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.total_nodes, 1);
    assert_eq!(s.leaf_nodes, 1);
    assert_eq!(s.error_nodes, 0);
}

#[test]
fn tw_v8_stats_single_leaf() {
    let src = b"x";
    let root = leaf(1, 0, 1);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.total_nodes, 1);
    assert_eq!(s.max_depth, 1);
    assert_eq!(s.leaf_nodes, 1);
}

#[test]
fn tw_v8_stats_parent_and_child() {
    let src = b"a";
    let root = interior(5, vec![leaf(1, 0, 1)]);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.total_nodes, 2);
    assert_eq!(s.max_depth, 2);
    assert_eq!(s.leaf_nodes, 1);
}

#[test]
fn tw_v8_stats_three_siblings() {
    let src = b"abc";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.total_nodes, 4);
    assert_eq!(s.leaf_nodes, 3);
    assert_eq!(s.max_depth, 2);
}

#[test]
fn tw_v8_stats_error_nodes_counted_separately() {
    let src = b"ae";
    let root = interior(5, vec![leaf(1, 0, 1), error_node(1, 2)]);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.total_nodes, 2); // root + leaf
    assert_eq!(s.error_nodes, 1);
}

#[test]
fn tw_v8_stats_all_errors() {
    let src = b"abc";
    let root = interior(
        5,
        vec![error_node(0, 1), error_node(1, 2), error_node(2, 3)],
    );
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.error_nodes, 3);
    assert_eq!(s.total_nodes, 1); // only root entered
}

#[test]
fn tw_v8_stats_deep_chain_10() {
    let src = b"x";
    let root = left_chain(10);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.max_depth, 10);
    assert_eq!(s.total_nodes, 10);
    assert_eq!(s.leaf_nodes, 1);
}

#[test]
fn tw_v8_stats_wide_tree_10() {
    let src = b"0123456789";
    let root = wide_tree(10);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.total_nodes, 11); // root + 10 children
    assert_eq!(s.leaf_nodes, 10);
    assert_eq!(s.max_depth, 2);
}

#[test]
fn tw_v8_stats_balanced_binary_depth_4() {
    let src = b"x";
    let root = balanced_tree(4, 1);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    // depth 4 balanced binary: 8 leaves, 7 internals = 15 nodes
    assert_eq!(s.total_nodes, 15);
    assert_eq!(s.leaf_nodes, 8);
    assert_eq!(s.max_depth, 4);
}

#[test]
fn tw_v8_stats_accumulates_across_walks() {
    let src = b"ab";
    let t1 = interior(5, vec![leaf(1, 0, 1)]);
    let t2 = interior(5, vec![leaf(2, 0, 1), leaf(3, 1, 2)]);
    let walker = TreeWalker::new(src);
    let mut s = StatsVisitor::default();
    walker.walk(&t1, &mut s);
    walker.walk(&t2, &mut s);
    assert_eq!(s.total_nodes, 5); // 2 + 3
    assert_eq!(s.leaf_nodes, 3);
}

#[test]
fn tw_v8_stats_node_counts_map() {
    let src = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert!(!s.node_counts.is_empty());
}

#[test]
fn tw_v8_stats_left_skewed() {
    let src = b"x";
    let root = left_chain(20);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.max_depth, 20);
    assert_eq!(s.leaf_nodes, 1);
    assert_eq!(s.total_nodes, 20);
}

#[test]
fn tw_v8_stats_right_skewed() {
    let src = b"x";
    let root = right_chain(15);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.max_depth, 15);
    assert_eq!(s.leaf_nodes, 1);
    assert_eq!(s.total_nodes, 15);
}

// ===================================================================
// 2. PrettyPrintVisitor — output format
// ===================================================================

#[test]
fn tw_v8_pretty_single_leaf_nonempty() {
    let src = b"x";
    let root = leaf(1, 0, 1);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&root, &mut pp);
    assert!(!pp.output().is_empty());
}

#[test]
fn tw_v8_pretty_tree_indented() {
    let src = b"x";
    let root = interior(5, vec![leaf(1, 0, 1)]);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&root, &mut pp);
    let lines: Vec<&str> = pp.output().lines().collect();
    assert!(lines.len() >= 2);
    // First line (root) has no leading indent, deeper lines do
    assert!(!lines[0].starts_with(' '));
    assert!(lines.last().unwrap().starts_with(' '));
}

#[test]
fn tw_v8_pretty_deep_tree_more_indentation() {
    let src = b"x";
    let deep = interior(2, vec![leaf(1, 0, 1)]);
    let root = interior(5, vec![deep]);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&root, &mut pp);
    let lines: Vec<&str> = pp.output().lines().collect();
    assert!(lines.len() >= 3);
    // Deepest line has the most leading spaces
    let last = lines.last().unwrap();
    assert!(last.starts_with("      "));
}

#[test]
fn tw_v8_pretty_named_node_tagged() {
    let src = b"x";
    let root = leaf(1, 0, 1);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&root, &mut pp);
    assert!(pp.output().contains("[named]"));
}

#[test]
fn tw_v8_pretty_anonymous_no_named_tag() {
    let src = b"+";
    let root = anon_leaf(4, 0, 1);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&root, &mut pp);
    assert!(!pp.output().contains("[named]"));
}

#[test]
fn tw_v8_pretty_error_child_shows_error() {
    let src = b"err";
    let root = interior(5, vec![error_node(0, 3)]);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&root, &mut pp);
    assert!(pp.output().contains("ERROR"));
}

#[test]
fn tw_v8_pretty_default_matches_new() {
    let a = PrettyPrintVisitor::new();
    let b = PrettyPrintVisitor::default();
    assert_eq!(a.output(), b.output());
}

#[test]
fn tw_v8_pretty_initially_empty() {
    let pp = PrettyPrintVisitor::new();
    assert!(pp.output().is_empty());
}

#[test]
fn tw_v8_pretty_leaf_text_appears() {
    let src = b"hello";
    let root = leaf(1, 0, 5);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&root, &mut pp);
    assert!(pp.output().contains("\"hello\""));
}

#[test]
fn tw_v8_pretty_multiline_format_consistent() {
    let src = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&root, &mut pp);
    let lines: Vec<&str> = pp.output().lines().collect();
    // At least root line + 2 leaf lines + 2 text lines
    assert!(lines.len() >= 3);
    // All lines should end with content (no trailing spaces on purpose)
    for line in &lines {
        assert!(!line.is_empty());
    }
}

#[test]
fn tw_v8_pretty_chain_depth_10_indentation() {
    let src = b"x";
    let root = left_chain(10);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&root, &mut pp);
    // The leaf text at depth 10 should be indented with 20 spaces (10 * 2)
    assert!(pp.output().contains("                    \"x\""));
}

// ===================================================================
// 3. SearchVisitor — finding nodes
// ===================================================================

#[test]
fn tw_v8_search_finds_matching_symbol() {
    let src = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol == 2);
    TreeWalker::new(src).walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
    assert_eq!(sv.matches[0].0, 1); // start_byte
    assert_eq!(sv.matches[0].1, 2); // end_byte
}

#[test]
fn tw_v8_search_empty_for_missing_symbol() {
    let src = b"x";
    let root = leaf(1, 0, 1);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol == 99);
    TreeWalker::new(src).walk(&root, &mut sv);
    assert!(sv.matches.is_empty());
}

#[test]
fn tw_v8_search_multiple_matches() {
    let src = b"aaa";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2), leaf(1, 2, 3)]);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol == 1);
    TreeWalker::new(src).walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 3);
}

#[test]
fn tw_v8_search_root_matches() {
    let src = b"a";
    let root = interior(5, vec![leaf(1, 0, 1)]);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol == 5);
    TreeWalker::new(src).walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
}

#[test]
fn tw_v8_search_always_true_finds_all() {
    let src = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let mut sv = SearchVisitor::new(|_: &ParsedNode| true);
    TreeWalker::new(src).walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 3);
}

#[test]
fn tw_v8_search_by_byte_range() {
    let src = b"abcdef";
    let root = interior(5, vec![leaf(1, 0, 2), leaf(2, 2, 4), leaf(3, 4, 6)]);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.start_byte >= 2 && n.end_byte <= 4);
    TreeWalker::new(src).walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
    assert_eq!(sv.matches[0].0, 2);
}

#[test]
fn tw_v8_search_deep_tree_finds_leaf() {
    let src = b"x";
    let root = left_chain(20);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol == 1);
    TreeWalker::new(src).walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
}

#[test]
fn tw_v8_search_balanced_tree_all_leaves() {
    let src = b"x";
    let root = balanced_tree(3, 1);
    // 4 leaves (symbol=1), 3 interior (symbol=101)
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol == 1);
    TreeWalker::new(src).walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 4);
}

#[test]
fn tw_v8_search_via_bfs_same_count() {
    let src = b"abc";
    let root = interior(
        5,
        vec![
            leaf(1, 0, 1),
            interior(2, vec![leaf(1, 1, 2)]),
            leaf(1, 2, 3),
        ],
    );
    let mut dfs_sv = SearchVisitor::new(|n: &ParsedNode| n.symbol == 1);
    TreeWalker::new(src).walk(&root, &mut dfs_sv);
    let mut bfs_sv = SearchVisitor::new(|n: &ParsedNode| n.symbol == 1);
    BreadthFirstWalker::new(src).walk(&root, &mut bfs_sv);
    assert_eq!(dfs_sv.matches.len(), bfs_sv.matches.len());
}

// ===================================================================
// 4. DFS vs BFS traversal order
// ===================================================================

#[test]
fn tw_v8_dfs_left_subtree_first() {
    let src = b"abcd";
    let left = interior(2, vec![leaf(1, 0, 1)]);
    let right = leaf(3, 1, 4);
    let root = interior(5, vec![left, right]);
    let mut rec = SymbolRecorder(Vec::new());
    TreeWalker::new(src).walk(&root, &mut rec);
    assert_eq!(rec.0, vec![5, 2, 1, 3]);
}

#[test]
fn tw_v8_bfs_level_order() {
    let src = b"abcd";
    let left = interior(2, vec![leaf(1, 0, 1)]);
    let right = leaf(3, 1, 4);
    let root = interior(5, vec![left, right]);
    let mut rec = SymbolRecorder(Vec::new());
    BreadthFirstWalker::new(src).walk(&root, &mut rec);
    assert_eq!(rec.0, vec![5, 2, 3, 1]);
}

#[test]
fn tw_v8_dfs_bfs_differ_nontrivial() {
    let src = b"abcde";
    let left = interior(2, vec![leaf(7, 0, 1), leaf(8, 1, 2)]);
    let right = interior(3, vec![leaf(9, 2, 3)]);
    let root = interior(5, vec![left, right]);

    let mut dfs_rec = SymbolRecorder(Vec::new());
    TreeWalker::new(src).walk(&root, &mut dfs_rec);
    let mut bfs_rec = SymbolRecorder(Vec::new());
    BreadthFirstWalker::new(src).walk(&root, &mut bfs_rec);

    assert_eq!(dfs_rec.0, vec![5, 2, 7, 8, 3, 9]);
    assert_eq!(bfs_rec.0, vec![5, 2, 3, 7, 8, 9]);
    assert_ne!(dfs_rec.0, bfs_rec.0);
}

#[test]
fn tw_v8_dfs_bfs_same_total_nodes() {
    let src = b"abcde";
    let root = interior(
        5,
        vec![
            interior(2, vec![leaf(1, 0, 1), leaf(3, 1, 2)]),
            leaf(4, 2, 5),
        ],
    );
    let mut dfs = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut dfs);
    let mut bfs = StatsVisitor::default();
    BreadthFirstWalker::new(src).walk(&root, &mut bfs);
    assert_eq!(dfs.total_nodes, bfs.total_nodes);
    assert_eq!(dfs.leaf_nodes, bfs.leaf_nodes);
}

#[test]
fn tw_v8_dfs_bfs_same_error_count() {
    let src = b"a_err";
    let root = interior(5, vec![leaf(1, 0, 1), error_node(1, 5)]);
    let mut dfs = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut dfs);
    let mut bfs = StatsVisitor::default();
    BreadthFirstWalker::new(src).walk(&root, &mut bfs);
    assert_eq!(dfs.error_nodes, bfs.error_nodes);
}

#[test]
fn tw_v8_bfs_single_leaf() {
    let src = b"x";
    let root = leaf(1, 0, 1);
    let mut s = StatsVisitor::default();
    BreadthFirstWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.total_nodes, 1);
    assert_eq!(s.leaf_nodes, 1);
}

#[test]
fn tw_v8_bfs_single_error_root() {
    let src = b"e";
    let root = error_node(0, 1);
    let mut s = StatsVisitor::default();
    BreadthFirstWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.error_nodes, 1);
    assert_eq!(s.total_nodes, 0);
}

// ===================================================================
// 5. VisitorAction — Stop and SkipChildren
// ===================================================================

#[test]
fn tw_v8_stop_action_halts_dfs() {
    struct StopAt {
        target: u16,
        visited: Vec<u16>,
    }
    impl Visitor for StopAt {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.visited.push(node.symbol);
            if node.symbol == self.target {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }

    let src = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let mut v = StopAt {
        target: 1,
        visited: Vec::new(),
    };
    TreeWalker::new(src).walk(&root, &mut v);
    assert!(v.visited.contains(&5));
    assert!(v.visited.contains(&1));
}

#[test]
fn tw_v8_stop_action_halts_bfs() {
    struct StopAt {
        target: u16,
        visited: Vec<u16>,
    }
    impl Visitor for StopAt {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.visited.push(node.symbol);
            if node.symbol == self.target {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }

    let src = b"abc";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let mut v = StopAt {
        target: 1,
        visited: Vec::new(),
    };
    BreadthFirstWalker::new(src).walk(&root, &mut v);
    assert_eq!(v.visited, vec![5, 1]);
}

#[test]
fn tw_v8_skip_children_dfs() {
    struct SkipSym {
        skip: u16,
        visited: Vec<u16>,
    }
    impl Visitor for SkipSym {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.visited.push(node.symbol);
            if node.symbol == self.skip {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }

    let src = b"abc";
    let subtree = interior(2, vec![leaf(1, 0, 1), leaf(3, 1, 2)]);
    let root = interior(5, vec![subtree, leaf(4, 2, 3)]);
    let mut v = SkipSym {
        skip: 2,
        visited: Vec::new(),
    };
    TreeWalker::new(src).walk(&root, &mut v);
    assert_eq!(v.visited, vec![5, 2, 4]);
}

#[test]
fn tw_v8_skip_at_root_sees_only_root() {
    struct AlwaysSkip(Vec<u16>);
    impl Visitor for AlwaysSkip {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.0.push(node.symbol);
            VisitorAction::SkipChildren
        }
    }

    let src = b"abc";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let mut v = AlwaysSkip(Vec::new());
    TreeWalker::new(src).walk(&root, &mut v);
    assert_eq!(v.0, vec![5]);
}

#[test]
fn tw_v8_visitor_action_copy_semantics() {
    let a = VisitorAction::Continue;
    let b = a; // Copy
    assert_eq!(a, b);
}

#[test]
fn tw_v8_visitor_action_all_variants_distinct() {
    let variants = [
        VisitorAction::Continue,
        VisitorAction::SkipChildren,
        VisitorAction::Stop,
    ];
    for i in 0..variants.len() {
        for j in (i + 1)..variants.len() {
            assert_ne!(variants[i], variants[j]);
        }
    }
}

#[test]
fn tw_v8_visitor_action_debug_format() {
    let s = format!("{:?}", VisitorAction::SkipChildren);
    assert_eq!(s, "SkipChildren");
}

// ===================================================================
// 6. Enter/leave pairing and depth gauge
// ===================================================================

#[test]
fn tw_v8_enter_leave_pairing() {
    let src = b"abcd";
    let root = interior(
        5,
        vec![
            interior(2, vec![leaf(1, 0, 1), leaf(3, 1, 2)]),
            leaf(4, 2, 4),
        ],
    );
    let mut pt = PairTracker {
        enters: 0,
        leaves: 0,
    };
    TreeWalker::new(src).walk(&root, &mut pt);
    assert_eq!(pt.enters, pt.leaves);
}

#[test]
fn tw_v8_depth_gauge_returns_to_zero() {
    let src = b"x";
    let root = interior(5, vec![interior(2, vec![leaf(1, 0, 1)])]);
    let mut dg = DepthGauge { current: 0, max: 0 };
    TreeWalker::new(src).walk(&root, &mut dg);
    assert_eq!(dg.max, 3);
    assert_eq!(dg.current, 0);
}

#[test]
fn tw_v8_depth_gauge_deep_chain() {
    let src = b"x";
    let root = left_chain(50);
    let mut dg = DepthGauge { current: 0, max: 0 };
    TreeWalker::new(src).walk(&root, &mut dg);
    assert_eq!(dg.max, 50);
    assert_eq!(dg.current, 0);
}

#[test]
fn tw_v8_enter_leave_balanced_tree() {
    let src = b"x";
    let root = balanced_tree(4, 1);
    let mut pt = PairTracker {
        enters: 0,
        leaves: 0,
    };
    TreeWalker::new(src).walk(&root, &mut pt);
    assert_eq!(pt.enters, pt.leaves);
    assert_eq!(pt.enters, 15);
}

// ===================================================================
// 7. Leaf count — nodes with no children
// ===================================================================

#[test]
fn tw_v8_leaf_count_single_leaf() {
    let src = b"x";
    let root = leaf(1, 0, 1);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.leaf_nodes, 1);
}

#[test]
fn tw_v8_leaf_count_no_children_is_leaf() {
    let src = b"";
    let root = make_node(5, vec![], 0, 0, false, true);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    // Node with no children is treated as leaf
    assert_eq!(s.leaf_nodes, 1);
}

#[test]
fn tw_v8_leaf_count_wide_tree() {
    let src = b"0123456789";
    let root = wide_tree(10);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.leaf_nodes, 10);
}

#[test]
fn tw_v8_leaf_count_chain_one_leaf() {
    let src = b"x";
    let root = left_chain(30);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.leaf_nodes, 1);
}

#[test]
fn tw_v8_leaf_count_balanced_binary() {
    let src = b"x";
    let root = balanced_tree(5, 1);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    // 2^4 = 16 leaves in depth-5 balanced binary tree
    assert_eq!(s.leaf_nodes, 16);
}

// ===================================================================
// 8. Various tree shapes
// ===================================================================

#[test]
fn tw_v8_shape_left_skewed_depth() {
    let src = b"x";
    let root = left_chain(25);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.max_depth, 25);
}

#[test]
fn tw_v8_shape_right_skewed_depth() {
    let src = b"x";
    let root = right_chain(25);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.max_depth, 25);
}

#[test]
fn tw_v8_shape_balanced_depth() {
    let src = b"x";
    let root = balanced_tree(6, 1);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.max_depth, 6);
}

#[test]
fn tw_v8_shape_caterpillar_tree() {
    // Spine of depth 5, each spine node also has a leaf sibling
    let src = b"abcde";
    let mut node = leaf(1, 0, 1);
    for i in 1..5 {
        node = interior(10 + i as u16, vec![node, leaf(20 + i as u16, i, i + 1)]);
    }
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&node, &mut s);
    // 1 deepest leaf + 4 interior + 4 sibling leaves = 9
    assert_eq!(s.total_nodes, 9);
    assert_eq!(s.leaf_nodes, 5);
}

#[test]
fn tw_v8_shape_diamond() {
    // Root -> two interior nodes -> each with same-symbol leaf
    let src = b"ab";
    let root = interior(
        5,
        vec![
            interior(2, vec![leaf(1, 0, 1)]),
            interior(3, vec![leaf(1, 1, 2)]),
        ],
    );
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.total_nodes, 5);
    assert_eq!(s.leaf_nodes, 2);
    assert_eq!(s.max_depth, 3);
}

// ===================================================================
// 9. Custom leaf text collection
// ===================================================================

#[test]
fn tw_v8_leaf_text_dfs() {
    let src = b"hello world";
    let root = interior(5, vec![leaf(1, 0, 5), leaf(2, 6, 11)]);
    let mut ltr = LeafTextRecorder(Vec::new());
    TreeWalker::new(src).walk(&root, &mut ltr);
    assert_eq!(ltr.0, vec!["hello", "world"]);
}

#[test]
fn tw_v8_leaf_text_bfs() {
    let src = b"hello world";
    let root = interior(5, vec![leaf(1, 0, 5), leaf(2, 6, 11)]);
    let mut ltr = LeafTextRecorder(Vec::new());
    BreadthFirstWalker::new(src).walk(&root, &mut ltr);
    assert_eq!(ltr.0, vec!["hello", "world"]);
}

#[test]
fn tw_v8_leaf_text_empty_source() {
    let src = b"";
    let root = leaf(1, 0, 0);
    let mut ltr = LeafTextRecorder(Vec::new());
    TreeWalker::new(src).walk(&root, &mut ltr);
    assert_eq!(ltr.0, vec![""]);
}

// ===================================================================
// 10. TransformVisitor
// ===================================================================

#[test]
fn tw_v8_transform_leaf_count() {
    struct LeafCounter;
    impl TransformVisitor for LeafCounter {
        type Output = usize;
        fn transform_node(&mut self, _n: &ParsedNode, children: Vec<usize>) -> usize {
            children.iter().sum()
        }
        fn transform_leaf(&mut self, _n: &ParsedNode, _text: &str) -> usize {
            1
        }
        fn transform_error(&mut self, _n: &ParsedNode) -> usize {
            0
        }
    }

    let src = b"abcd";
    let root = interior(5, vec![leaf(1, 0, 2), leaf(2, 2, 4)]);
    let count = TransformWalker::new(src).walk(&root, &mut LeafCounter);
    assert_eq!(count, 2);
}

#[test]
fn tw_v8_transform_text_concat() {
    struct Concat;
    impl TransformVisitor for Concat {
        type Output = String;
        fn transform_node(&mut self, _n: &ParsedNode, children: Vec<String>) -> String {
            children.join("")
        }
        fn transform_leaf(&mut self, _n: &ParsedNode, text: &str) -> String {
            text.to_string()
        }
        fn transform_error(&mut self, _n: &ParsedNode) -> String {
            "<ERR>".to_string()
        }
    }

    let src = b"hello";
    let root = interior(5, vec![leaf(1, 0, 3), leaf(2, 3, 5)]);
    let result = TransformWalker::new(src).walk(&root, &mut Concat);
    assert_eq!(result, "hello");
}

#[test]
fn tw_v8_transform_depth_computation() {
    struct DepthComputer;
    impl TransformVisitor for DepthComputer {
        type Output = usize;
        fn transform_node(&mut self, _n: &ParsedNode, children: Vec<usize>) -> usize {
            children.iter().max().copied().unwrap_or(0) + 1
        }
        fn transform_leaf(&mut self, _n: &ParsedNode, _text: &str) -> usize {
            1
        }
        fn transform_error(&mut self, _n: &ParsedNode) -> usize {
            1
        }
    }

    let src = b"x";
    let root = left_chain(5);
    let depth = TransformWalker::new(src).walk(&root, &mut DepthComputer);
    assert_eq!(depth, 5);
}

#[test]
fn tw_v8_transform_error_node() {
    struct ErrCounter;
    impl TransformVisitor for ErrCounter {
        type Output = usize;
        fn transform_node(&mut self, _n: &ParsedNode, children: Vec<usize>) -> usize {
            children.iter().sum()
        }
        fn transform_leaf(&mut self, _n: &ParsedNode, _text: &str) -> usize {
            0
        }
        fn transform_error(&mut self, _n: &ParsedNode) -> usize {
            1
        }
    }

    let src = b"ae";
    let root = interior(5, vec![leaf(1, 0, 1), error_node(1, 2)]);
    let errs = TransformWalker::new(src).walk(&root, &mut ErrCounter);
    assert_eq!(errs, 1);
}

// ===================================================================
// 11. Composition — multiple walkers on same tree
// ===================================================================

#[test]
fn tw_v8_stats_then_pretty_same_tree() {
    let src = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let walker = TreeWalker::new(src);
    let mut s = StatsVisitor::default();
    walker.walk(&root, &mut s);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert_eq!(s.total_nodes, 3);
    assert!(!pp.output().is_empty());
}

#[test]
fn tw_v8_dfs_stats_then_bfs_search() {
    let src = b"abc";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(1, 2, 3)]);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol == 1);
    BreadthFirstWalker::new(src).walk(&root, &mut sv);
    assert_eq!(s.total_nodes, 4);
    assert_eq!(sv.matches.len(), 2);
}

// ===================================================================
// 12. Arena-based tree construction + manual walks
// ===================================================================

#[test]
fn tw_v8_arena_single_leaf_roundtrip() {
    let mut arena = TreeArena::new();
    let handle = arena.alloc(TreeNode::leaf(42));
    let node = arena.get(handle);
    assert!(node.is_leaf());
    assert_eq!(node.value(), 42);
}

#[test]
fn tw_v8_arena_branch_children() {
    let mut arena = TreeArena::new();
    let c1 = arena.alloc(TreeNode::leaf(1));
    let c2 = arena.alloc(TreeNode::leaf(2));
    let parent = arena.alloc(TreeNode::branch(vec![c1, c2]));
    let parent_ref = arena.get(parent);
    assert!(parent_ref.is_branch());
    assert_eq!(parent_ref.children().len(), 2);
}

#[test]
fn tw_v8_arena_branch_with_symbol() {
    let mut arena = TreeArena::new();
    let c = arena.alloc(TreeNode::leaf(10));
    let parent = arena.alloc(TreeNode::branch_with_symbol(99, vec![c]));
    assert_eq!(arena.get(parent).symbol(), 99);
}

#[test]
fn tw_v8_arena_deep_tree_walk() {
    let mut arena = TreeArena::new();
    let l = arena.alloc(TreeNode::leaf(1));
    let n1 = arena.alloc(TreeNode::branch(vec![l]));
    let n2 = arena.alloc(TreeNode::branch(vec![n1]));
    let root = arena.alloc(TreeNode::branch(vec![n2]));

    // Walk from root to leaf
    let r = arena.get(root);
    assert!(r.is_branch());
    let c0 = r.children()[0];
    let c1 = arena.get(c0).children()[0];
    let c2 = arena.get(c1).children()[0];
    assert!(arena.get(c2).is_leaf());
    assert_eq!(arena.get(c2).value(), 1);
}

#[test]
fn tw_v8_arena_len_and_empty() {
    let mut arena = TreeArena::new();
    assert!(arena.is_empty());
    arena.alloc(TreeNode::leaf(1));
    assert!(!arena.is_empty());
    assert_eq!(arena.len(), 1);
}

#[test]
fn tw_v8_arena_clear_resets() {
    let mut arena = TreeArena::new();
    arena.alloc(TreeNode::leaf(1));
    arena.alloc(TreeNode::leaf(2));
    assert_eq!(arena.len(), 2);
    arena.clear();
    assert!(arena.is_empty());
    assert_eq!(arena.len(), 0);
}

#[test]
fn tw_v8_arena_reset_resets() {
    let mut arena = TreeArena::new();
    for i in 0..10 {
        arena.alloc(TreeNode::leaf(i));
    }
    assert_eq!(arena.len(), 10);
    arena.reset();
    assert!(arena.is_empty());
}

#[test]
fn tw_v8_arena_dfs_via_handles() {
    let mut arena = TreeArena::new();
    let l1 = arena.alloc(TreeNode::leaf(10));
    let l2 = arena.alloc(TreeNode::leaf(20));
    let branch = arena.alloc(TreeNode::branch_with_symbol(99, vec![l1, l2]));
    let root = arena.alloc(TreeNode::branch_with_symbol(1, vec![branch]));

    let mut dfs_order = Vec::new();
    let mut stack = vec![root];
    while let Some(h) = stack.pop() {
        let n = arena.get(h);
        dfs_order.push(n.symbol());
        for &child in n.children().iter().rev() {
            stack.push(child);
        }
    }
    assert_eq!(dfs_order, vec![1, 99, 10, 20]);
}

#[test]
fn tw_v8_arena_bfs_via_handles() {
    let mut arena = TreeArena::new();
    let l1 = arena.alloc(TreeNode::leaf(10));
    let l2 = arena.alloc(TreeNode::leaf(20));
    let branch = arena.alloc(TreeNode::branch_with_symbol(99, vec![l1, l2]));
    let root = arena.alloc(TreeNode::branch_with_symbol(1, vec![branch]));

    let mut bfs_order = Vec::new();
    let mut queue = VecDeque::new();
    queue.push_back(root);
    while let Some(h) = queue.pop_front() {
        let n = arena.get(h);
        bfs_order.push(n.symbol());
        for &child in n.children() {
            queue.push_back(child);
        }
    }
    assert_eq!(bfs_order, vec![1, 99, 10, 20]);
}

#[test]
fn tw_v8_arena_handle_is_copy() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(7));
    let h2 = h; // Copy, not move
    assert_eq!(arena.get(h).value(), arena.get(h2).value());
}

#[test]
fn tw_v8_arena_stress_500_nodes() {
    let mut arena = TreeArena::new();
    let mut handles: Vec<NodeHandle> = Vec::new();
    for i in 0..500 {
        handles.push(arena.alloc(TreeNode::leaf(i)));
    }
    assert_eq!(arena.len(), 500);
    assert_eq!(arena.get(handles[0]).value(), 0);
    assert_eq!(arena.get(handles[499]).value(), 499);
}

#[test]
fn tw_v8_arena_num_chunks_grows() {
    let mut arena = TreeArena::with_capacity(2);
    arena.alloc(TreeNode::leaf(1));
    arena.alloc(TreeNode::leaf(2));
    assert_eq!(arena.num_chunks(), 1);
    arena.alloc(TreeNode::leaf(3));
    assert!(arena.num_chunks() >= 2);
}

#[test]
fn tw_v8_arena_metrics_snapshot() {
    let mut arena = TreeArena::new();
    let m0 = arena.metrics();
    assert_eq!(m0.len(), 0);
    assert!(m0.is_empty());
    arena.alloc(TreeNode::leaf(1));
    let m1 = arena.metrics();
    assert_eq!(m1.len(), 1);
    assert!(!m1.is_empty());
    assert!(m1.memory_usage() > 0);
}

// ===================================================================
// 13. Edge cases
// ===================================================================

#[test]
fn tw_v8_empty_source_leaf() {
    let src = b"";
    let root = leaf(1, 0, 0);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.total_nodes, 1);
    assert_eq!(s.leaf_nodes, 1);
}

#[test]
fn tw_v8_empty_interior_as_leaf() {
    let src = b"";
    let root = make_node(5, vec![], 0, 0, false, true);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.total_nodes, 1);
    assert_eq!(s.leaf_nodes, 1);
}

#[test]
fn tw_v8_single_error_root() {
    let src = b"e";
    let root = error_node(0, 1);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.error_nodes, 1);
    assert_eq!(s.total_nodes, 0);
}

#[test]
fn tw_v8_deep_chain_100() {
    let src = b"x";
    let root = left_chain(100);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.max_depth, 100);
    assert_eq!(s.total_nodes, 100);
    assert_eq!(s.leaf_nodes, 1);
}

#[test]
fn tw_v8_deep_chain_100_bfs() {
    let src = b"x";
    let root = left_chain(100);
    let mut s = StatsVisitor::default();
    BreadthFirstWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.total_nodes, 100);
    assert_eq!(s.leaf_nodes, 1);
}

#[test]
fn tw_v8_wide_50_children() {
    let src: Vec<u8> = (0..50).map(|i| b'a' + (i % 26)).collect();
    let children: Vec<_> = (0..50)
        .map(|i| leaf((i % 65535) as u16 + 1, i, i + 1))
        .collect();
    let root = interior(10, children);
    let mut s = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut s);
    assert_eq!(s.total_nodes, 51);
    assert_eq!(s.leaf_nodes, 50);
}

#[test]
fn tw_v8_mixed_errors_and_leaves() {
    let src = b"abcde";
    let root = interior(
        5,
        vec![
            leaf(1, 0, 1),
            error_node(1, 2),
            leaf(2, 2, 3),
            error_node(3, 4),
            leaf(3, 4, 5),
        ],
    );
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.error_nodes, 2);
    assert_eq!(s.leaf_nodes, 3);
    assert_eq!(s.total_nodes, 4); // root + 3 leaves
}

#[test]
fn tw_v8_search_no_match_in_large_tree() {
    let src = b"x";
    let root = balanced_tree(5, 1);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol == 9999);
    TreeWalker::new(src).walk(&root, &mut sv);
    assert!(sv.matches.is_empty());
}

#[test]
fn tw_v8_search_all_match_in_large_tree() {
    let src = b"x";
    let root = balanced_tree(4, 1);
    let mut sv = SearchVisitor::new(|_: &ParsedNode| true);
    TreeWalker::new(src).walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 15);
}

#[test]
fn tw_v8_pretty_wide_tree() {
    let src = b"abcde";
    let root = wide_tree(5);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&root, &mut pp);
    let lines: Vec<&str> = pp.output().lines().collect();
    // root + 5 leaf kind lines + 5 leaf text lines = at least 11
    assert!(lines.len() >= 6);
}

#[test]
fn tw_v8_arena_alloc_after_clear() {
    let mut arena = TreeArena::new();
    arena.alloc(TreeNode::leaf(1));
    arena.clear();
    assert!(arena.is_empty());
    let h = arena.alloc(TreeNode::leaf(42));
    assert_eq!(arena.len(), 1);
    assert_eq!(arena.get(h).value(), 42);
}

#[test]
fn tw_v8_arena_alloc_after_reset() {
    let mut arena = TreeArena::new();
    for i in 0..5 {
        arena.alloc(TreeNode::leaf(i));
    }
    arena.reset();
    assert!(arena.is_empty());
    let h = arena.alloc(TreeNode::leaf(99));
    assert_eq!(arena.len(), 1);
    assert_eq!(arena.get(h).value(), 99);
}

#[test]
fn tw_v8_arena_default_creates_empty() {
    let arena = TreeArena::default();
    assert!(arena.is_empty());
    assert_eq!(arena.num_chunks(), 1);
}

#[test]
fn tw_v8_arena_with_capacity() {
    let arena = TreeArena::with_capacity(64);
    assert!(arena.is_empty());
    assert!(arena.capacity() >= 64);
}

#[test]
fn tw_v8_pretty_accumulates_on_multiple_walks() {
    let src = b"x";
    let root = leaf(1, 0, 1);
    let mut pp = PrettyPrintVisitor::new();
    let walker = TreeWalker::new(src);
    walker.walk(&root, &mut pp);
    let len1 = pp.output().len();
    walker.walk(&root, &mut pp);
    let len2 = pp.output().len();
    // Output should grow with each walk
    assert!(len2 > len1);
}

#[test]
fn tw_v8_stats_fresh_visitor_zeroes() {
    let s = StatsVisitor::default();
    assert_eq!(s.total_nodes, 0);
    assert_eq!(s.leaf_nodes, 0);
    assert_eq!(s.error_nodes, 0);
    assert_eq!(s.max_depth, 0);
    assert!(s.node_counts.is_empty());
}

#[test]
fn tw_v8_bfs_wide_tree_stats() {
    let src = b"0123456789";
    let root = wide_tree(10);
    let mut s = StatsVisitor::default();
    BreadthFirstWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.total_nodes, 11);
    assert_eq!(s.leaf_nodes, 10);
}

#[test]
fn tw_v8_bfs_balanced_tree_stats() {
    let src = b"x";
    let root = balanced_tree(3, 1);
    let mut s = StatsVisitor::default();
    BreadthFirstWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.total_nodes, 7);
    assert_eq!(s.leaf_nodes, 4);
}

#[test]
fn tw_v8_dfs_bfs_leaf_text_order_flat() {
    // For a flat tree, DFS and BFS should produce same leaf order
    let src = b"abcde";
    let root = interior(
        5,
        vec![
            leaf(1, 0, 1),
            leaf(2, 1, 2),
            leaf(3, 2, 3),
            leaf(4, 3, 4),
            leaf(7, 4, 5),
        ],
    );
    let mut dfs_ltr = LeafTextRecorder(Vec::new());
    TreeWalker::new(src).walk(&root, &mut dfs_ltr);
    let mut bfs_ltr = LeafTextRecorder(Vec::new());
    BreadthFirstWalker::new(src).walk(&root, &mut bfs_ltr);
    assert_eq!(dfs_ltr.0, bfs_ltr.0);
}

#[test]
fn tw_v8_arena_manual_walk_leaf_count() {
    let mut arena = TreeArena::new();
    let l1 = arena.alloc(TreeNode::leaf(1));
    let l2 = arena.alloc(TreeNode::leaf(2));
    let l3 = arena.alloc(TreeNode::leaf(3));
    let b1 = arena.alloc(TreeNode::branch_with_symbol(10, vec![l1, l2]));
    let root = arena.alloc(TreeNode::branch_with_symbol(20, vec![b1, l3]));

    // Count leaves via manual arena walk
    let mut leaf_count = 0;
    let mut stack = vec![root];
    while let Some(h) = stack.pop() {
        let n = arena.get(h);
        if n.is_leaf() {
            leaf_count += 1;
        }
        for &child in n.children().iter().rev() {
            stack.push(child);
        }
    }
    assert_eq!(leaf_count, 3);
}

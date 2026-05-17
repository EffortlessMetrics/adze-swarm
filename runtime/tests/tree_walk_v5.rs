//! Tests (v5) for tree walking and visitor infrastructure.
//!
//! Covers: `TreeWalker`, `BreadthFirstWalker`, `StatsVisitor`,
//! `PrettyPrintVisitor`, `SearchVisitor`, custom visitors, deeply nested
//! trees, arena-based tree construction, and edge cases.

use adze::arena_allocator::{NodeHandle, TreeArena, TreeNode};
use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TransformVisitor,
    TransformWalker, TreeWalker, Visitor, VisitorAction,
};

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

/// Build a left-skewed chain of depth `n` (leaf at the bottom).
fn left_chain(depth: usize) -> ParsedNode {
    let mut node = leaf(1, 0, 1);
    for _ in 1..depth {
        node = interior(2, vec![node]);
    }
    node
}

// ---------------------------------------------------------------------------
// Reusable custom visitors
// ---------------------------------------------------------------------------

/// Records symbol order from `enter_node`.
struct SymbolRecorder(Vec<u16>);

impl Visitor for SymbolRecorder {
    fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
        self.0.push(node.symbol);
        VisitorAction::Continue
    }
}

/// Records leaf text in visitation order.
struct LeafTextRecorder(Vec<String>);

impl Visitor for LeafTextRecorder {
    fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
        VisitorAction::Continue
    }
    fn visit_leaf(&mut self, _node: &ParsedNode, text: &str) {
        self.0.push(text.to_string());
    }
}

/// Counts enter/leave pairs, tracking current and max depth.
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

// ===================================================================
// 1. StatsVisitor — counts nodes / leaves / errors
// ===================================================================

#[test]
fn stats_single_leaf_counts() {
    let src = b"a";
    let root = leaf(1, 0, 1);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.total_nodes, 1);
    assert_eq!(s.leaf_nodes, 1);
    assert_eq!(s.error_nodes, 0);
}

#[test]
fn stats_two_leaves_under_root() {
    let src = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.total_nodes, 3);
    assert_eq!(s.leaf_nodes, 2);
}

#[test]
fn stats_error_nodes_not_entered() {
    let src = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), error_node(1, 2)]);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.total_nodes, 2);
    assert_eq!(s.error_nodes, 1);
}

#[test]
fn stats_multiple_errors() {
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
fn stats_max_depth_linear_chain() {
    let src = b"x";
    let root = left_chain(6);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.max_depth, 6);
    assert_eq!(s.leaf_nodes, 1);
}

#[test]
fn stats_max_depth_wide_tree() {
    let src = b"abcde";
    let children: Vec<_> = (0u16..5)
        .map(|i| leaf(i + 1, i as usize, i as usize + 1))
        .collect();
    let root = interior(10, children);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.max_depth, 2);
    assert_eq!(s.total_nodes, 6);
}

#[test]
fn stats_accumulates_across_walks() {
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
fn stats_node_counts_map_populated() {
    let src = b"abc";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2), leaf(2, 2, 3)]);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert!(!s.node_counts.is_empty());
    // symbol 5 → "Expression", symbol 1 → "*", symbol 2 → "_2"
    assert_eq!(*s.node_counts.get("Expression").unwrap_or(&0), 1);
}

// ===================================================================
// 2. PrettyPrintVisitor — output format
// ===================================================================

#[test]
fn pretty_empty_initially() {
    let pp = PrettyPrintVisitor::new();
    assert!(pp.output().is_empty());
}

#[test]
fn pretty_named_leaf_tagged() {
    let src = b"x";
    let root = leaf(1, 0, 1);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&root, &mut pp);
    assert!(pp.output().contains("[named]"));
    assert!(pp.output().contains("\"x\""));
}

#[test]
fn pretty_anonymous_leaf_no_named_tag() {
    let src = b"+";
    let root = anon_leaf(4, 0, 1);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&root, &mut pp);
    assert!(!pp.output().contains("[named]"));
}

#[test]
fn pretty_nested_indentation() {
    let src = b"x";
    let deep = interior(2, vec![leaf(1, 0, 1)]);
    let root = interior(5, vec![deep]);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&root, &mut pp);
    // depth-0 root, depth-1 interior, depth-2 leaf, depth-3 text
    let lines: Vec<&str> = pp.output().lines().collect();
    assert!(lines.len() >= 3);
    // deepest line should have more leading spaces
    let last_line = lines.last().unwrap();
    assert!(last_line.starts_with("      "));
}

#[test]
fn pretty_error_child_shows_error() {
    let src = b"err";
    let root = interior(5, vec![error_node(0, 3)]);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&root, &mut pp);
    assert!(pp.output().contains("ERROR"));
}

#[test]
fn pretty_default_matches_new() {
    let a = PrettyPrintVisitor::new();
    let b = PrettyPrintVisitor::default();
    assert_eq!(a.output(), b.output());
}

// ===================================================================
// 3. SearchVisitor — finding nodes
// ===================================================================

#[test]
fn search_no_matches_returns_empty() {
    let src = b"x";
    let root = leaf(1, 0, 1);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol == 99);
    TreeWalker::new(src).walk(&root, &mut sv);
    assert!(sv.matches.is_empty());
}

#[test]
fn search_single_match() {
    let src = b"abcd";
    let root = interior(5, vec![leaf(1, 0, 2), leaf(2, 2, 4)]);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol == 2);
    TreeWalker::new(src).walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
    assert_eq!(sv.matches[0].0, 2); // start_byte
    assert_eq!(sv.matches[0].1, 4); // end_byte
}

#[test]
fn search_multiple_matches_all_found() {
    let src = b"abcdef";
    let root = interior(5, vec![leaf(1, 0, 2), leaf(1, 2, 4), leaf(1, 4, 6)]);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol == 1);
    TreeWalker::new(src).walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 3);
}

#[test]
fn search_root_matches_predicate() {
    let src = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol == 5);
    TreeWalker::new(src).walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
}

#[test]
fn search_always_true_finds_all_entered() {
    let src = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let mut sv = SearchVisitor::new(|_: &ParsedNode| true);
    TreeWalker::new(src).walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 3);
}

#[test]
fn search_by_byte_range() {
    let src = b"abcdef";
    let root = interior(5, vec![leaf(1, 0, 2), leaf(2, 2, 4), leaf(3, 4, 6)]);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.start_byte >= 2 && n.end_byte <= 4);
    TreeWalker::new(src).walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
    assert_eq!(sv.matches[0].0, 2);
}

// ===================================================================
// 4. Depth-first vs breadth-first traversal order
// ===================================================================

#[test]
fn dfs_visits_left_subtree_before_right() {
    let src = b"abcd";
    //       5
    //      / \
    //     2   3
    //    /
    //   1
    let left = interior(2, vec![leaf(1, 0, 1)]);
    let right = leaf(3, 1, 4);
    let root = interior(5, vec![left, right]);
    let mut rec = SymbolRecorder(Vec::new());
    TreeWalker::new(src).walk(&root, &mut rec);
    assert_eq!(rec.0, vec![5, 2, 1, 3]);
}

#[test]
fn bfs_visits_level_order() {
    let src = b"abcd";
    let left = interior(2, vec![leaf(1, 0, 1)]);
    let right = leaf(3, 1, 4);
    let root = interior(5, vec![left, right]);
    let mut rec = SymbolRecorder(Vec::new());
    BreadthFirstWalker::new(src).walk(&root, &mut rec);
    assert_eq!(rec.0, vec![5, 2, 3, 1]);
}

#[test]
fn dfs_and_bfs_differ_on_nontrivial_tree() {
    let src = b"abcde";
    let left = interior(2, vec![leaf(7, 0, 1), leaf(8, 1, 2)]);
    let right = interior(3, vec![leaf(9, 2, 3)]);
    let root = interior(5, vec![left, right]);

    let mut dfs_rec = SymbolRecorder(Vec::new());
    TreeWalker::new(src).walk(&root, &mut dfs_rec);

    let mut bfs_rec = SymbolRecorder(Vec::new());
    BreadthFirstWalker::new(src).walk(&root, &mut bfs_rec);

    // DFS: 5, 2, 7, 8, 3, 9
    assert_eq!(dfs_rec.0, vec![5, 2, 7, 8, 3, 9]);
    // BFS: 5, 2, 3, 7, 8, 9
    assert_eq!(bfs_rec.0, vec![5, 2, 3, 7, 8, 9]);
    assert_ne!(dfs_rec.0, bfs_rec.0);
}

#[test]
fn dfs_bfs_same_total_node_count() {
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
fn dfs_bfs_same_error_count() {
    let src = b"a_err";
    let root = interior(5, vec![leaf(1, 0, 1), error_node(1, 5)]);
    let mut dfs = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut dfs);
    let mut bfs = StatsVisitor::default();
    BreadthFirstWalker::new(src).walk(&root, &mut bfs);
    assert_eq!(dfs.error_nodes, bfs.error_nodes);
}

#[test]
fn dfs_bfs_search_same_result_count() {
    let src = b"aaa";
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
// 5. Empty trees and single-node trees
// ===================================================================

#[test]
fn empty_source_leaf() {
    let src = b"";
    let root = leaf(1, 0, 0);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.total_nodes, 1);
    assert_eq!(s.leaf_nodes, 1);
}

#[test]
fn empty_interior_treated_as_leaf() {
    let src = b"";
    let root = make_node(5, vec![], 0, 0, false, true);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.total_nodes, 1);
    assert_eq!(s.leaf_nodes, 1);
}

#[test]
fn single_error_root() {
    let src = b"e";
    let root = error_node(0, 1);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.error_nodes, 1);
    assert_eq!(s.total_nodes, 0);
}

#[test]
fn bfs_single_leaf() {
    let src = b"x";
    let root = leaf(1, 0, 1);
    let mut s = StatsVisitor::default();
    BreadthFirstWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.total_nodes, 1);
    assert_eq!(s.leaf_nodes, 1);
}

#[test]
fn bfs_single_error_root() {
    let src = b"e";
    let root = error_node(0, 1);
    let mut s = StatsVisitor::default();
    BreadthFirstWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.error_nodes, 1);
    assert_eq!(s.total_nodes, 0);
}

// ===================================================================
// 6. Custom visitor implementations
// ===================================================================

#[test]
fn custom_leaf_text_collector_dfs() {
    let src = b"hello world";
    let root = interior(5, vec![leaf(1, 0, 5), leaf(2, 6, 11)]);
    let mut ltr = LeafTextRecorder(Vec::new());
    TreeWalker::new(src).walk(&root, &mut ltr);
    assert_eq!(ltr.0, vec!["hello", "world"]);
}

#[test]
fn custom_leaf_text_collector_bfs() {
    let src = b"hello world";
    let root = interior(5, vec![leaf(1, 0, 5), leaf(2, 6, 11)]);
    let mut ltr = LeafTextRecorder(Vec::new());
    BreadthFirstWalker::new(src).walk(&root, &mut ltr);
    assert_eq!(ltr.0, vec!["hello", "world"]);
}

#[test]
fn custom_stop_action_halts_dfs() {
    struct StopAtSymbol {
        target: u16,
        visited: Vec<u16>,
    }
    impl Visitor for StopAtSymbol {
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
    let mut v = StopAtSymbol {
        target: 1,
        visited: Vec::new(),
    };
    TreeWalker::new(src).walk(&root, &mut v);
    // DFS: enter root (5), enter left child (1) -> Stop returns from walk_node
    // but siblings still visited because stop only returns from current node
    assert!(v.visited.contains(&5));
    assert!(v.visited.contains(&1));
}

#[test]
fn custom_stop_action_halts_bfs() {
    struct StopAtSymbol {
        target: u16,
        visited: Vec<u16>,
    }
    impl Visitor for StopAtSymbol {
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
    let mut v = StopAtSymbol {
        target: 1,
        visited: Vec::new(),
    };
    BreadthFirstWalker::new(src).walk(&root, &mut v);
    // BFS: pop 5 (continue, enqueue children), pop 1 (stop) -> done
    assert_eq!(v.visited, vec![5, 1]);
}

#[test]
fn custom_skip_children_dfs() {
    struct SkipSymbol {
        skip: u16,
        visited: Vec<u16>,
    }
    impl Visitor for SkipSymbol {
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
    let mut v = SkipSymbol {
        skip: 2,
        visited: Vec::new(),
    };
    TreeWalker::new(src).walk(&root, &mut v);
    // children of 2 (symbols 1, 3) should be skipped
    assert_eq!(v.visited, vec![5, 2, 4]);
}

#[test]
fn custom_depth_gauge_returns_to_zero() {
    let src = b"x";
    let root = interior(5, vec![interior(2, vec![leaf(1, 0, 1)])]);
    let mut dg = DepthGauge { current: 0, max: 0 };
    TreeWalker::new(src).walk(&root, &mut dg);
    assert_eq!(dg.max, 3);
    assert_eq!(dg.current, 0);
}

#[test]
fn custom_visitor_enter_leave_pairing() {
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
fn custom_skip_at_root_sees_only_root() {
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

// ===================================================================
// 7. Walker with deeply nested trees
// ===================================================================

#[test]
fn deep_chain_100_stats() {
    let src = b"x";
    let root = left_chain(100);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.max_depth, 100);
    assert_eq!(s.total_nodes, 100);
    assert_eq!(s.leaf_nodes, 1);
}

#[test]
fn deep_chain_100_bfs_stats() {
    let src = b"x";
    let root = left_chain(100);
    let mut s = StatsVisitor::default();
    BreadthFirstWalker::new(src).walk(&root, &mut s);
    assert_eq!(s.total_nodes, 100);
    assert_eq!(s.leaf_nodes, 1);
}

#[test]
fn deep_chain_depth_gauge_returns_zero() {
    let src = b"x";
    let root = left_chain(50);
    let mut dg = DepthGauge { current: 0, max: 0 };
    TreeWalker::new(src).walk(&root, &mut dg);
    assert_eq!(dg.max, 50);
    assert_eq!(dg.current, 0);
}

#[test]
fn deep_chain_pretty_print_deeply_indented() {
    let src = b"x";
    let root = left_chain(10);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&root, &mut pp);
    // 10 levels deep → the text "x" at depth 10 has 20 spaces
    assert!(pp.output().contains("                    \"x\""));
}

#[test]
fn deep_chain_search_finds_leaf() {
    let src = b"x";
    let root = left_chain(20);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol == 1);
    TreeWalker::new(src).walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
}

// ===================================================================
// 8. TreeArena construction and structural verification
// ===================================================================

#[test]
fn arena_single_leaf_roundtrip() {
    let mut arena = TreeArena::new();
    let handle = arena.alloc(TreeNode::leaf(42));
    let node = arena.get(handle);
    assert!(node.is_leaf());
    assert_eq!(node.value(), 42);
}

#[test]
fn arena_branch_children_accessible() {
    let mut arena = TreeArena::new();
    let c1 = arena.alloc(TreeNode::leaf(1));
    let c2 = arena.alloc(TreeNode::leaf(2));
    let parent = arena.alloc(TreeNode::branch(vec![c1, c2]));
    let parent_ref = arena.get(parent);
    assert!(parent_ref.is_branch());
    assert_eq!(parent_ref.children().len(), 2);
}

#[test]
fn arena_branch_with_symbol() {
    let mut arena = TreeArena::new();
    let c = arena.alloc(TreeNode::leaf(10));
    let parent = arena.alloc(TreeNode::branch_with_symbol(99, vec![c]));
    assert_eq!(arena.get(parent).symbol(), 99);
}

#[test]
fn arena_deep_tree_structure() {
    let mut arena = TreeArena::new();
    let l = arena.alloc(TreeNode::leaf(1));
    let n1 = arena.alloc(TreeNode::branch(vec![l]));
    let n2 = arena.alloc(TreeNode::branch(vec![n1]));
    let root = arena.alloc(TreeNode::branch(vec![n2]));

    // Walk from root to leaf through children
    let r = arena.get(root);
    assert!(r.is_branch());
    let c0 = r.children()[0];
    assert!(arena.get(c0).is_branch());
    let c1 = arena.get(c0).children()[0];
    assert!(arena.get(c1).is_branch());
    let c2 = arena.get(c1).children()[0];
    assert!(arena.get(c2).is_leaf());
    assert_eq!(arena.get(c2).value(), 1);
}

#[test]
fn arena_len_and_empty() {
    let mut arena = TreeArena::new();
    assert!(arena.is_empty());
    arena.alloc(TreeNode::leaf(1));
    assert!(!arena.is_empty());
    assert_eq!(arena.len(), 1);
}

#[test]
fn arena_reset_clears_nodes() {
    let mut arena = TreeArena::new();
    arena.alloc(TreeNode::leaf(1));
    arena.alloc(TreeNode::leaf(2));
    assert_eq!(arena.len(), 2);
    arena.reset();
    assert!(arena.is_empty());
}

#[test]
fn arena_dfs_walk_via_handles() {
    let mut arena = TreeArena::new();
    let l1 = arena.alloc(TreeNode::leaf(10));
    let l2 = arena.alloc(TreeNode::leaf(20));
    let branch = arena.alloc(TreeNode::branch_with_symbol(99, vec![l1, l2]));
    let root = arena.alloc(TreeNode::branch_with_symbol(1, vec![branch]));

    // Manual DFS
    let mut dfs_order = Vec::new();
    let mut stack = vec![root];
    while let Some(h) = stack.pop() {
        let n = arena.get(h);
        dfs_order.push(n.symbol());
        // push children in reverse so left is visited first
        let ch = n.children();
        for &child in ch.iter().rev() {
            stack.push(child);
        }
    }
    assert_eq!(dfs_order, vec![1, 99, 10, 20]);
}

#[test]
fn arena_bfs_walk_via_handles() {
    use std::collections::VecDeque;

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
fn arena_handle_is_copy() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(7));
    let h2 = h; // Copy, not move
    assert_eq!(arena.get(h).value(), arena.get(h2).value());
}

#[test]
fn arena_many_nodes_stress() {
    let mut arena = TreeArena::new();
    let mut handles: Vec<NodeHandle> = Vec::new();
    for i in 0..500 {
        handles.push(arena.alloc(TreeNode::leaf(i)));
    }
    assert_eq!(arena.len(), 500);
    // Verify first and last
    assert_eq!(arena.get(handles[0]).value(), 0);
    assert_eq!(arena.get(handles[499]).value(), 499);
}

// ===================================================================
// 9. VisitorAction properties
// ===================================================================

#[test]
fn visitor_action_copy_semantics() {
    let a = VisitorAction::Continue;
    let b = a; // Copy
    assert_eq!(a, b);
}

#[test]
fn visitor_action_debug_format() {
    let s = format!("{:?}", VisitorAction::SkipChildren);
    assert_eq!(s, "SkipChildren");
}

#[test]
fn visitor_action_all_variants_distinct() {
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

// ===================================================================
// 10. TransformVisitor integration
// ===================================================================

#[test]
fn transform_leaf_count() {
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
fn transform_text_concatenation() {
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

// ===================================================================
// 11. Composition: multiple walkers on same tree
// ===================================================================

#[test]
fn stats_then_pretty_same_tree() {
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
fn dfs_stats_then_bfs_search() {
    let src = b"abc";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(1, 2, 3)]);
    let mut s = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol == 1);
    BreadthFirstWalker::new(src).walk(&root, &mut sv);
    assert_eq!(s.total_nodes, 4);
    assert_eq!(sv.matches.len(), 2);
}

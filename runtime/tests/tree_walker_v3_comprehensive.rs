//! Comprehensive v3 tests for tree walker (DFS/BFS) APIs:
//! TreeWalker, BreadthFirstWalker, StatsVisitor, PrettyPrintVisitor,
//! SearchVisitor, and VisitorAction control flow.

use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TreeWalker, Visitor,
    VisitorAction,
};

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

/// root(10)( a(1), mid(20)( b(2), c(3) ), d(4) )
/// source: "abcd"
fn sample_tree() -> (ParsedNode, Vec<u8>) {
    let src = b"abcd".to_vec();
    let a = leaf(1, 0, 1);
    let b = leaf(2, 1, 2);
    let c = unnamed_leaf(3, 2, 3);
    let mid = interior(20, vec![b, c]);
    let d = leaf(4, 3, 4);
    let root = interior(10, vec![a, mid, d]);
    (root, src)
}

/// Deep linear chain: root -> child -> ... -> leaf (depth levels)
fn deep_chain(depth: usize) -> (ParsedNode, Vec<u8>) {
    let src = b"x".to_vec();
    let mut node = leaf(1, 0, 1);
    for i in 2..=depth as u16 {
        node = make_node(i, vec![node], 0, 1, false, true);
    }
    (node, src)
}

/// Wide tree: root with `width` leaf children
fn wide_tree(width: usize) -> (ParsedNode, Vec<u8>) {
    let src: Vec<u8> = (0..width).map(|_| b'x').collect();
    let children: Vec<ParsedNode> = (0..width).map(|i| leaf((i + 1) as u16, i, i + 1)).collect();
    let root = make_node(100, children, 0, width, false, true);
    (root, src)
}

// Visitor that records enter_node symbols
struct SymbolRecorder(Vec<u16>);
impl Visitor for SymbolRecorder {
    fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
        self.0.push(n.symbol());
        VisitorAction::Continue
    }
}

// Visitor that records both enter and leave symbols
struct EnterLeaveRecorder {
    enters: Vec<u16>,
    leaves: Vec<u16>,
}
impl EnterLeaveRecorder {
    fn new() -> Self {
        Self {
            enters: vec![],
            leaves: vec![],
        }
    }
}
impl Visitor for EnterLeaveRecorder {
    fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
        self.enters.push(n.symbol());
        VisitorAction::Continue
    }
    fn leave_node(&mut self, n: &ParsedNode) {
        self.leaves.push(n.symbol());
    }
}

// Visitor that records leaf text
struct LeafTextRecorder(Vec<String>);
impl Visitor for LeafTextRecorder {
    fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
        VisitorAction::Continue
    }
    fn visit_leaf(&mut self, _: &ParsedNode, text: &str) {
        self.0.push(text.to_string());
    }
}

fn dfs_stats(root: &ParsedNode, src: &[u8]) -> StatsVisitor {
    let walker = TreeWalker::new(src);
    let mut stats = StatsVisitor::default();
    walker.walk(root, &mut stats);
    stats
}

fn bfs_stats(root: &ParsedNode, src: &[u8]) -> StatsVisitor {
    let walker = BreadthFirstWalker::new(src);
    let mut stats = StatsVisitor::default();
    walker.walk(root, &mut stats);
    stats
}

// ===================================================================
// 1. DFS visit order (10 tests)
// ===================================================================

#[test]
fn test_dfs_single_leaf_enters_and_leaves() {
    let src = b"x";
    let walker = TreeWalker::new(src);
    let mut v = EnterLeaveRecorder::new();
    walker.walk(&leaf(1, 0, 1), &mut v);
    assert_eq!(v.enters, vec![1]);
    assert_eq!(v.leaves, vec![1]);
}

#[test]
fn test_dfs_preorder_enter_symbols() {
    let (root, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut v = SymbolRecorder(vec![]);
    walker.walk(&root, &mut v);
    // DFS pre-order: root(10), a(1), mid(20), b(2), c(3), d(4)
    assert_eq!(v.0, vec![10, 1, 20, 2, 3, 4]);
}

#[test]
fn test_dfs_postorder_leave_symbols() {
    let (root, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut v = EnterLeaveRecorder::new();
    walker.walk(&root, &mut v);
    // Post-order leaves: a(1), b(2), c(3), mid(20), d(4), root(10)
    assert_eq!(v.leaves, vec![1, 2, 3, 20, 4, 10]);
}

#[test]
fn test_dfs_enter_leave_count_equal() {
    let (root, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut v = EnterLeaveRecorder::new();
    walker.walk(&root, &mut v);
    assert_eq!(v.enters.len(), v.leaves.len());
}

#[test]
fn test_dfs_leaf_text_values() {
    let (root, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut v = LeafTextRecorder(vec![]);
    walker.walk(&root, &mut v);
    // DFS leaf order: a, b, c, d
    assert_eq!(v.0, vec!["a", "b", "c", "d"]);
}

#[test]
fn test_dfs_deep_chain_preorder() {
    // Chain of 5: sym5 -> sym4 -> sym3 -> sym2 -> sym1(leaf)
    let (root, src) = deep_chain(5);
    let walker = TreeWalker::new(&src);
    let mut v = SymbolRecorder(vec![]);
    walker.walk(&root, &mut v);
    assert_eq!(v.0, vec![5, 4, 3, 2, 1]);
}

#[test]
fn test_dfs_wide_tree_preorder() {
    let (root, src) = wide_tree(4);
    let walker = TreeWalker::new(&src);
    let mut v = SymbolRecorder(vec![]);
    walker.walk(&root, &mut v);
    // root(100), leaf(1), leaf(2), leaf(3), leaf(4)
    assert_eq!(v.0, vec![100, 1, 2, 3, 4]);
}

#[test]
fn test_dfs_root_entered_first() {
    let (root, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut v = SymbolRecorder(vec![]);
    walker.walk(&root, &mut v);
    assert_eq!(v.0[0], 10);
}

#[test]
fn test_dfs_root_left_last() {
    let (root, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut v = EnterLeaveRecorder::new();
    walker.walk(&root, &mut v);
    assert_eq!(*v.leaves.last().unwrap(), 10);
}

#[test]
fn test_dfs_children_between_enter_leave_of_parent() {
    let (root, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    struct Events(Vec<(char, u16)>);
    impl Visitor for Events {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(('E', n.symbol()));
            VisitorAction::Continue
        }
        fn leave_node(&mut self, n: &ParsedNode) {
            self.0.push(('L', n.symbol()));
        }
    }
    let mut v = Events(vec![]);
    walker.walk(&root, &mut v);
    // For mid(20): enter before its children, leave after
    let enter_mid = v.0.iter().position(|e| *e == ('E', 20)).unwrap();
    let leave_mid = v.0.iter().position(|e| *e == ('L', 20)).unwrap();
    let enter_b = v.0.iter().position(|e| *e == ('E', 2)).unwrap();
    let enter_c = v.0.iter().position(|e| *e == ('E', 3)).unwrap();
    assert!(enter_mid < enter_b);
    assert!(enter_mid < enter_c);
    assert!(leave_mid > enter_b);
    assert!(leave_mid > enter_c);
}

// ===================================================================
// 2. BFS visit order (8 tests)
// ===================================================================

#[test]
fn test_bfs_single_leaf_enters() {
    let src = b"x";
    let walker = BreadthFirstWalker::new(src);
    let mut v = SymbolRecorder(vec![]);
    walker.walk(&leaf(1, 0, 1), &mut v);
    assert_eq!(v.0, vec![1]);
}

#[test]
fn test_bfs_level_order_symbols() {
    let (root, src) = sample_tree();
    let walker = BreadthFirstWalker::new(&src);
    let mut v = SymbolRecorder(vec![]);
    walker.walk(&root, &mut v);
    // BFS level-order: root(10), a(1), mid(20), d(4), b(2), c(3)
    assert_eq!(v.0, vec![10, 1, 20, 4, 2, 3]);
}

#[test]
fn test_bfs_wide_tree_level_order() {
    let (root, src) = wide_tree(4);
    let walker = BreadthFirstWalker::new(&src);
    let mut v = SymbolRecorder(vec![]);
    walker.walk(&root, &mut v);
    // root then all leaves at level 1
    assert_eq!(v.0, vec![100, 1, 2, 3, 4]);
}

#[test]
fn test_bfs_deep_chain_level_order() {
    // Each level has one node, so BFS == DFS for a linear chain
    let (root, src) = deep_chain(5);
    let walker = BreadthFirstWalker::new(&src);
    let mut v = SymbolRecorder(vec![]);
    walker.walk(&root, &mut v);
    assert_eq!(v.0, vec![5, 4, 3, 2, 1]);
}

#[test]
fn test_bfs_leaf_text_values() {
    let (root, src) = sample_tree();
    let walker = BreadthFirstWalker::new(&src);
    let mut v = LeafTextRecorder(vec![]);
    walker.walk(&root, &mut v);
    // BFS order: a (level 1), d (level 1), b (level 2), c (level 2)
    assert_eq!(v.0, vec!["a", "d", "b", "c"]);
}

#[test]
fn test_bfs_processes_all_non_error_nodes() {
    let (root, src) = sample_tree();
    let walker = BreadthFirstWalker::new(&src);
    let mut v = SymbolRecorder(vec![]);
    walker.walk(&root, &mut v);
    assert_eq!(v.0.len(), 6);
}

#[test]
fn test_bfs_error_node_visited() {
    let src = b"ae".to_vec();
    let a = leaf(1, 0, 1);
    let err = error_node(1, 2);
    let root = interior(10, vec![a, err]);

    struct ErrorCounter {
        errors: usize,
        enters: usize,
    }
    impl Visitor for ErrorCounter {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.enters += 1;
            VisitorAction::Continue
        }
        fn visit_error(&mut self, _: &ParsedNode) {
            self.errors += 1;
        }
    }
    let walker = BreadthFirstWalker::new(&src);
    let mut v = ErrorCounter {
        errors: 0,
        enters: 0,
    };
    walker.walk(&root, &mut v);
    assert_eq!(v.errors, 1);
    // Error nodes bypass enter_node: root + a = 2
    assert_eq!(v.enters, 2);
}

#[test]
fn test_bfs_no_leave_node_called() {
    let (root, src) = sample_tree();
    let walker = BreadthFirstWalker::new(&src);
    let mut v = EnterLeaveRecorder::new();
    walker.walk(&root, &mut v);
    // BFS never calls leave_node
    assert!(v.leaves.is_empty());
}

// ===================================================================
// 3. DFS vs BFS count consistency (8 tests)
// ===================================================================

#[test]
fn test_consistency_total_nodes_sample_tree() {
    let (root, src) = sample_tree();
    assert_eq!(
        dfs_stats(&root, &src).total_nodes,
        bfs_stats(&root, &src).total_nodes
    );
}

#[test]
fn test_consistency_leaf_count_sample_tree() {
    let (root, src) = sample_tree();
    assert_eq!(
        dfs_stats(&root, &src).leaf_nodes,
        bfs_stats(&root, &src).leaf_nodes
    );
}

#[test]
fn test_consistency_error_count_with_errors() {
    let src = b"ae".to_vec();
    let a = leaf(1, 0, 1);
    let err = error_node(1, 2);
    let root = interior(10, vec![a, err]);
    assert_eq!(
        dfs_stats(&root, &src).error_nodes,
        bfs_stats(&root, &src).error_nodes
    );
}

#[test]
fn test_consistency_total_on_deep_chain() {
    let (root, src) = deep_chain(10);
    assert_eq!(
        dfs_stats(&root, &src).total_nodes,
        bfs_stats(&root, &src).total_nodes
    );
}

#[test]
fn test_consistency_total_on_wide_tree() {
    let (root, src) = wide_tree(8);
    assert_eq!(
        dfs_stats(&root, &src).total_nodes,
        bfs_stats(&root, &src).total_nodes
    );
}

#[test]
fn test_consistency_leaf_on_single_leaf() {
    let src = b"x";
    let node = leaf(1, 0, 1);
    assert_eq!(
        dfs_stats(&node, src).leaf_nodes,
        bfs_stats(&node, src).leaf_nodes
    );
}

#[test]
fn test_consistency_all_leaf_texts() {
    let (root, src) = sample_tree();
    let dfs_walker = TreeWalker::new(&src);
    let mut dfs_texts = LeafTextRecorder(vec![]);
    dfs_walker.walk(&root, &mut dfs_texts);

    let bfs_walker = BreadthFirstWalker::new(&src);
    let mut bfs_texts = LeafTextRecorder(vec![]);
    bfs_walker.walk(&root, &mut bfs_texts);

    // Same set of leaf texts, possibly in different order
    let mut dfs_sorted = dfs_texts.0.clone();
    dfs_sorted.sort();
    let mut bfs_sorted = bfs_texts.0.clone();
    bfs_sorted.sort();
    assert_eq!(dfs_sorted, bfs_sorted);
}

#[test]
fn test_consistency_same_node_kinds_visited() {
    let (root, src) = sample_tree();
    let dfs_walker = TreeWalker::new(&src);
    let mut dfs_syms = SymbolRecorder(vec![]);
    dfs_walker.walk(&root, &mut dfs_syms);

    let bfs_walker = BreadthFirstWalker::new(&src);
    let mut bfs_syms = SymbolRecorder(vec![]);
    bfs_walker.walk(&root, &mut bfs_syms);

    let mut dfs_sorted = dfs_syms.0.clone();
    dfs_sorted.sort();
    let mut bfs_sorted = bfs_syms.0.clone();
    bfs_sorted.sort();
    assert_eq!(dfs_sorted, bfs_sorted);
}

// ===================================================================
// 4. SkipChildren behavior (5 tests)
// ===================================================================

#[test]
fn test_skip_children_dfs_omits_subtree() {
    let (root, src) = sample_tree();
    struct SkipMid(Vec<u16>);
    impl Visitor for SkipMid {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            if n.symbol() == 20 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }
    let walker = TreeWalker::new(&src);
    let mut v = SkipMid(vec![]);
    walker.walk(&root, &mut v);
    // b(2) and c(3) skipped
    assert_eq!(v.0, vec![10, 1, 20, 4]);
}

#[test]
fn test_skip_children_dfs_calls_leave_node() {
    let (root, src) = sample_tree();
    struct SkipMidTracker {
        leaves: Vec<u16>,
    }
    impl Visitor for SkipMidTracker {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            if n.symbol() == 20 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
        fn leave_node(&mut self, n: &ParsedNode) {
            self.leaves.push(n.symbol());
        }
    }
    let walker = TreeWalker::new(&src);
    let mut v = SkipMidTracker { leaves: vec![] };
    walker.walk(&root, &mut v);
    // leave_node IS called for the skipped node itself
    assert!(v.leaves.contains(&20));
}

#[test]
fn test_skip_children_dfs_continues_siblings() {
    let (root, src) = sample_tree();
    struct SkipFirst(Vec<u16>);
    impl Visitor for SkipFirst {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            if n.symbol() == 1 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }
    let walker = TreeWalker::new(&src);
    let mut v = SkipFirst(vec![]);
    walker.walk(&root, &mut v);
    // Siblings mid(20) and d(4) still visited
    assert!(v.0.contains(&20));
    assert!(v.0.contains(&4));
}

#[test]
fn test_skip_children_bfs_omits_children() {
    let (root, src) = sample_tree();
    struct SkipMid(Vec<u16>);
    impl Visitor for SkipMid {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            if n.symbol() == 20 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }
    let walker = BreadthFirstWalker::new(&src);
    let mut v = SkipMid(vec![]);
    walker.walk(&root, &mut v);
    // Children b(2), c(3) not queued
    assert!(!v.0.contains(&2));
    assert!(!v.0.contains(&3));
}

#[test]
fn test_skip_children_bfs_continues_siblings() {
    let (root, src) = sample_tree();
    struct SkipMid(Vec<u16>);
    impl Visitor for SkipMid {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            if n.symbol() == 20 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }
    let walker = BreadthFirstWalker::new(&src);
    let mut v = SkipMid(vec![]);
    walker.walk(&root, &mut v);
    // Siblings still processed
    assert_eq!(v.0, vec![10, 1, 20, 4]);
}

// ===================================================================
// 5. Stop behavior (5 tests)
// ===================================================================

#[test]
fn test_stop_dfs_skips_subtree_of_stopped_node() {
    let (root, src) = sample_tree();
    struct StopMid(Vec<u16>);
    impl Visitor for StopMid {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            if n.symbol() == 20 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }
    let walker = TreeWalker::new(&src);
    let mut v = StopMid(vec![]);
    walker.walk(&root, &mut v);
    // b(2) and c(3) are not entered
    assert!(!v.0.contains(&2));
    assert!(!v.0.contains(&3));
}

#[test]
fn test_stop_dfs_no_leave_on_stopped_node() {
    let (root, src) = sample_tree();
    struct StopMidTracker {
        leaves: Vec<u16>,
    }
    impl Visitor for StopMidTracker {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            if n.symbol() == 20 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
        fn leave_node(&mut self, n: &ParsedNode) {
            self.leaves.push(n.symbol());
        }
    }
    let walker = TreeWalker::new(&src);
    let mut v = StopMidTracker { leaves: vec![] };
    walker.walk(&root, &mut v);
    // leave_node NOT called for the stopped node
    assert!(!v.leaves.contains(&20));
}

#[test]
fn test_stop_dfs_parent_loop_continues() {
    // In DFS, Stop only returns from walk_node; parent's child loop continues
    let (root, src) = sample_tree();
    struct StopA(Vec<u16>);
    impl Visitor for StopA {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            if n.symbol() == 1 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }
    let walker = TreeWalker::new(&src);
    let mut v = StopA(vec![]);
    walker.walk(&root, &mut v);
    // After Stop on a(1), siblings mid(20) and d(4) are still visited
    assert!(v.0.contains(&20));
    assert!(v.0.contains(&4));
}

#[test]
fn test_stop_bfs_halts_entire_walk() {
    let (root, src) = sample_tree();
    struct StopA(Vec<u16>);
    impl Visitor for StopA {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            if n.symbol() == 1 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }
    let walker = BreadthFirstWalker::new(&src);
    let mut v = StopA(vec![]);
    walker.walk(&root, &mut v);
    // BFS: root(10) entered, then a(1) → Stop → walk exits
    assert_eq!(v.0, vec![10, 1]);
}

#[test]
fn test_stop_bfs_stops_immediately() {
    let (root, src) = sample_tree();
    struct StopRoot(Vec<u16>);
    impl Visitor for StopRoot {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            VisitorAction::Stop
        }
    }
    let walker = BreadthFirstWalker::new(&src);
    let mut v = StopRoot(vec![]);
    walker.walk(&root, &mut v);
    // Only root entered before Stop halts the walk
    assert_eq!(v.0, vec![10]);
}

// ===================================================================
// 6. StatsVisitor aggregation (5 tests)
// ===================================================================

#[test]
fn test_stats_dfs_total_nodes() {
    let (root, src) = sample_tree();
    let stats = dfs_stats(&root, &src);
    // 6 non-error nodes: root(10), a(1), mid(20), b(2), c(3), d(4)
    assert_eq!(stats.total_nodes, 6);
}

#[test]
fn test_stats_dfs_leaf_count() {
    let (root, src) = sample_tree();
    let stats = dfs_stats(&root, &src);
    // 4 leaves: a, b, c, d
    assert_eq!(stats.leaf_nodes, 4);
}

#[test]
fn test_stats_dfs_error_count() {
    let src = b"ae".to_vec();
    let a = leaf(1, 0, 1);
    let err = error_node(1, 2);
    let root = interior(10, vec![a, err]);
    let stats = dfs_stats(&root, &src);
    assert_eq!(stats.error_nodes, 1);
    // Error node does not trigger enter_node
    assert_eq!(stats.total_nodes, 2);
}

#[test]
fn test_stats_dfs_max_depth() {
    let (root, src) = sample_tree();
    let stats = dfs_stats(&root, &src);
    // Deepest path: root(10) -> mid(20) -> b(2) = depth 3
    assert_eq!(stats.max_depth, 3);
}

#[test]
fn test_stats_dfs_node_counts_by_kind() {
    let (root, src) = sample_tree();
    let stats = dfs_stats(&root, &src);
    // Fallback kind names (no language):
    //   sym 10 → "rule_10", sym 1 → "*", sym 20 → "unknown",
    //   sym 2 → "_2", sym 3 → "_6", sym 4 → "-"
    assert_eq!(stats.node_counts.get("rule_10"), Some(&1));
    assert_eq!(stats.node_counts.get("*"), Some(&1));
    assert_eq!(stats.node_counts.get("unknown"), Some(&1));
    assert_eq!(stats.node_counts.get("_2"), Some(&1));
    assert_eq!(stats.node_counts.get("_6"), Some(&1));
    assert_eq!(stats.node_counts.get("-"), Some(&1));
}

// ===================================================================
// 7. PrettyPrint output (5 tests)
// ===================================================================

#[test]
fn test_prettyprint_single_leaf_output() {
    let src = b"x";
    let walker = TreeWalker::new(src);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&leaf(1, 0, 1), &mut pp);
    let out = pp.output();
    assert!(out.contains("*"));
    assert!(out.contains("[named]"));
    assert!(out.contains("\"x\""));
}

#[test]
fn test_prettyprint_nested_indentation() {
    let (root, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    // Root at indent 0
    assert!(out.starts_with("rule_10"));
    // Mid's children at indent 2 (4 spaces)
    assert!(out.contains("    _2"));
}

#[test]
fn test_prettyprint_named_annotation() {
    let (root, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    // Named nodes get [named]
    assert!(out.contains("rule_10 [named]"));
    // Unnamed c(3, "_6") should NOT have [named]
    for line in out.lines() {
        if line.trim().starts_with("_6") {
            assert!(
                !line.contains("[named]"),
                "unnamed node should not have [named]"
            );
        }
    }
}

#[test]
fn test_prettyprint_error_node_output() {
    let src = b"ae".to_vec();
    let a = leaf(1, 0, 1);
    let err = error_node(1, 2);
    let root = interior(10, vec![a, err]);
    let walker = TreeWalker::new(&src);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    assert!(
        out.contains("ERROR:"),
        "error node should produce ERROR: line"
    );
}

#[test]
fn test_prettyprint_output_method_returns_str() {
    let pp = PrettyPrintVisitor::new();
    assert_eq!(pp.output(), "");
}

// ===================================================================
// 8. SearchVisitor matching (5 tests)
// ===================================================================

#[test]
fn test_search_finds_matching_by_predicate() {
    let (root, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() < 5);
    walker.walk(&root, &mut sv);
    // Symbols < 5: 1, 2, 3, 4
    assert_eq!(sv.matches.len(), 4);
}

#[test]
fn test_search_no_matches() {
    let (root, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 999);
    walker.walk(&root, &mut sv);
    assert!(sv.matches.is_empty());
}

#[test]
fn test_search_all_nodes_match() {
    let (root, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut sv = SearchVisitor::new(|_: &ParsedNode| true);
    walker.walk(&root, &mut sv);
    // All 6 non-error nodes matched
    assert_eq!(sv.matches.len(), 6);
}

#[test]
fn test_search_records_byte_ranges() {
    let (root, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 2);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
    let (start, end, _kind) = &sv.matches[0];
    assert_eq!(*start, 1);
    assert_eq!(*end, 2);
}

#[test]
fn test_search_by_named_filter() {
    let (root, src) = sample_tree();
    let walker = TreeWalker::new(&src);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.is_named());
    walker.walk(&root, &mut sv);
    // 5 named nodes (c is unnamed): root(10), a(1), mid(20), b(2), d(4)
    assert_eq!(sv.matches.len(), 5);
}

// ===================================================================
// 9. Edge cases (4 tests)
// ===================================================================

#[test]
fn test_edge_empty_source_leaf_text() {
    let src: &[u8] = b"";
    let node = leaf(1, 0, 0);
    let walker = TreeWalker::new(src);
    let mut v = LeafTextRecorder(vec![]);
    walker.walk(&node, &mut v);
    assert_eq!(v.0, vec![""]);
}

#[test]
fn test_edge_zero_width_node() {
    let src = b"abc";
    let node = leaf(1, 1, 1);
    let walker = TreeWalker::new(src);
    let mut v = LeafTextRecorder(vec![]);
    walker.walk(&node, &mut v);
    assert_eq!(v.0, vec![""]);
}

#[test]
fn test_edge_single_child_tree() {
    let src = b"x";
    let child = leaf(1, 0, 1);
    let root = interior(10, vec![child]);
    let walker = TreeWalker::new(src);
    let mut v = SymbolRecorder(vec![]);
    walker.walk(&root, &mut v);
    assert_eq!(v.0, vec![10, 1]);
}

#[test]
fn test_edge_reuse_visitor_across_walks() {
    let src = b"ab";
    let tree1 = interior(10, vec![leaf(1, 0, 1)]);
    let tree2 = interior(20, vec![leaf(2, 1, 2)]);

    let walker = TreeWalker::new(src);
    let mut stats = StatsVisitor::default();
    walker.walk(&tree1, &mut stats);
    walker.walk(&tree2, &mut stats);

    // Stats accumulate across walks
    assert_eq!(stats.total_nodes, 4);
    assert_eq!(stats.leaf_nodes, 2);
}

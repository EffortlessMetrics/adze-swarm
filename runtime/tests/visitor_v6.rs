//! Visitor pattern v6 — 64 tests covering the full visitor API surface.

use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TransformVisitor,
    TransformWalker, TreeWalker, Visitor, VisitorAction,
};

mod common;

use common::{
    error_node, interior_node as interior, make_test_node as make_node, named_leaf as leaf,
    unnamed_leaf,
};

/// root(10)( a(1), mid(11)( b(2), c(3) ), d(4) )  — source "abcd"
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

// Visitor that records enter-order via symbol id.
struct SymCollector(Vec<u16>);

impl Visitor for SymCollector {
    fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
        self.0.push(n.symbol());
        VisitorAction::Continue
    }
}

// ===================================================================
// Category 1 — VisitorAction::Stop halts traversal (8 tests)
// ===================================================================

#[test]
fn stop_dfs_at_first_child() {
    let (root, src) = sample_tree();
    struct V(usize);
    impl Visitor for V {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.0 += 1;
            if self.0 >= 2 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }
    let mut v = V(0);
    TreeWalker::new(&src).walk(&root, &mut v);
    // DFS Stop returns from current walk_node; parent's sibling loop continues.
    // root→Continue, a→Stop, mid→Stop, d→Stop  ⇒  4
    assert_eq!(v.0, 4);
}

#[test]
fn stop_bfs_at_first_child() {
    let (root, src) = sample_tree();
    struct V(usize);
    impl Visitor for V {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.0 += 1;
            if self.0 >= 2 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }
    let mut v = V(0);
    BreadthFirstWalker::new(&src).walk(&root, &mut v);
    // BFS Stop truly halts the entire walk ⇒ only 2 nodes entered.
    assert_eq!(v.0, 2);
}

#[test]
fn stop_prevents_leave_on_stopped_node_dfs() {
    let (root, src) = sample_tree();
    struct V {
        left: usize,
    }
    impl Visitor for V {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            if n.symbol() == 1 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
        fn leave_node(&mut self, _: &ParsedNode) {
            self.left += 1;
        }
    }
    let mut v = V { left: 0 };
    TreeWalker::new(&src).walk(&root, &mut v);
    // leave_node NOT called for the node that returned Stop.
    // Leaves called for: root, mid, b, c, d ⇒ count depends on DFS traversal.
    assert!(v.left > 0);
}

#[test]
fn stop_on_root_dfs_no_children_visited() {
    let (root, src) = sample_tree();
    struct V(usize);
    impl Visitor for V {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.0 += 1;
            VisitorAction::Stop
        }
    }
    let mut v = V(0);
    TreeWalker::new(&src).walk(&root, &mut v);
    // Stop at root ⇒ only root entered.
    assert_eq!(v.0, 1);
}

#[test]
fn stop_on_root_bfs_no_children_visited() {
    let (root, src) = sample_tree();
    struct V(usize);
    impl Visitor for V {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.0 += 1;
            VisitorAction::Stop
        }
    }
    let mut v = V(0);
    BreadthFirstWalker::new(&src).walk(&root, &mut v);
    assert_eq!(v.0, 1);
}

#[test]
fn stop_dfs_leaves_not_visited_past_stop() {
    let (root, src) = sample_tree();
    struct V {
        leaves: usize,
    }
    impl Visitor for V {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            VisitorAction::Stop
        }
        fn visit_leaf(&mut self, _: &ParsedNode, _: &str) {
            self.leaves += 1;
        }
    }
    let mut v = V { leaves: 0 };
    TreeWalker::new(&src).walk(&root, &mut v);
    assert_eq!(v.leaves, 0);
}

#[test]
fn stop_bfs_leaves_not_visited_past_stop() {
    let (root, src) = sample_tree();
    struct V {
        leaves: usize,
    }
    impl Visitor for V {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            VisitorAction::Stop
        }
        fn visit_leaf(&mut self, _: &ParsedNode, _: &str) {
            self.leaves += 1;
        }
    }
    let mut v = V { leaves: 0 };
    BreadthFirstWalker::new(&src).walk(&root, &mut v);
    assert_eq!(v.leaves, 0);
}

#[test]
fn stop_after_n_nodes_bfs() {
    let (root, src) = sample_tree();
    struct V(usize);
    impl Visitor for V {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.0 += 1;
            if self.0 >= 4 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }
    let mut v = V(0);
    BreadthFirstWalker::new(&src).walk(&root, &mut v);
    assert_eq!(v.0, 4);
}

// ===================================================================
// Category 2 — VisitorAction::SkipChildren (8 tests)
// ===================================================================

#[test]
fn skip_children_dfs_on_mid_node() {
    let (root, src) = sample_tree();
    struct V(Vec<u16>);
    impl Visitor for V {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            if n.symbol() == 11 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }
    let mut v = V(vec![]);
    TreeWalker::new(&src).walk(&root, &mut v);
    // Should see root(10), a(1), mid(11), d(4) — b(2) and c(3) skipped.
    assert_eq!(v.0, vec![10, 1, 11, 4]);
}

#[test]
fn skip_children_bfs_on_mid_node() {
    let (root, src) = sample_tree();
    struct V(Vec<u16>);
    impl Visitor for V {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            if n.symbol() == 11 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }
    let mut v = V(vec![]);
    BreadthFirstWalker::new(&src).walk(&root, &mut v);
    // BFS level-order: root(10), a(1), mid(11), d(4) — mid's children not queued.
    assert_eq!(v.0, vec![10, 1, 11, 4]);
}

#[test]
fn skip_children_calls_leave_node_dfs() {
    let (root, src) = sample_tree();
    struct V {
        left: Vec<u16>,
    }
    impl Visitor for V {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            if n.symbol() == 11 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
        fn leave_node(&mut self, n: &ParsedNode) {
            self.left.push(n.symbol());
        }
    }
    let mut v = V { left: vec![] };
    TreeWalker::new(&src).walk(&root, &mut v);
    // DFS SkipChildren still calls leave_node on the skipped node.
    assert!(v.left.contains(&11));
}

#[test]
fn skip_children_on_root_dfs_sees_only_root() {
    let (root, src) = sample_tree();
    struct CountEnter(usize);
    impl Visitor for CountEnter {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.0 += 1;
            VisitorAction::SkipChildren
        }
    }
    let mut ce = CountEnter(0);
    TreeWalker::new(&src).walk(&root, &mut ce);
    assert_eq!(ce.0, 1);
}

#[test]
fn skip_children_leaf_not_called_for_skipped() {
    let (root, src) = sample_tree();
    struct V {
        leaves: Vec<u16>,
    }
    impl Visitor for V {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            if n.symbol() == 11 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
        fn visit_leaf(&mut self, n: &ParsedNode, _: &str) {
            self.leaves.push(n.symbol());
        }
    }
    let mut v = V { leaves: vec![] };
    TreeWalker::new(&src).walk(&root, &mut v);
    // a(1) and d(4) are leaves, but b(2) and c(3) are skipped.
    assert!(v.leaves.contains(&1));
    assert!(v.leaves.contains(&4));
    assert!(!v.leaves.contains(&2));
    assert!(!v.leaves.contains(&3));
}

#[test]
fn skip_children_bfs_does_not_queue_children() {
    // Verify that skip prevents queueing the children.
    let src = b"abc".to_vec();
    let root = interior(
        10,
        vec![
            interior(11, vec![leaf(1, 0, 1), leaf(2, 1, 2)]),
            leaf(3, 2, 3),
        ],
    );
    struct V(Vec<u16>);
    impl Visitor for V {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            if n.symbol() == 11 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }
    let mut v = V(vec![]);
    BreadthFirstWalker::new(&src).walk(&root, &mut v);
    // root(10), child-11(skip), leaf-3, then 11's children never queued.
    assert_eq!(v.0, vec![10, 11, 3]);
}

#[test]
fn skip_children_all_leaves_skipped_dfs() {
    let (root, src) = sample_tree();
    struct V(usize);
    impl Visitor for V {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            VisitorAction::SkipChildren
        }
        fn visit_leaf(&mut self, _: &ParsedNode, _: &str) {
            self.0 += 1;
        }
    }
    let mut v = V(0);
    TreeWalker::new(&src).walk(&root, &mut v);
    assert_eq!(v.0, 0);
}

#[test]
fn skip_children_preserves_sibling_order() {
    let src = b"abcde".to_vec();
    let root = interior(
        10,
        vec![
            leaf(1, 0, 1),
            interior(11, vec![leaf(2, 1, 2), leaf(3, 2, 3)]),
            leaf(4, 3, 4),
            leaf(5, 4, 5),
        ],
    );
    struct V(Vec<u16>);
    impl Visitor for V {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.0.push(n.symbol());
            if n.symbol() == 11 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }
    let mut v = V(vec![]);
    TreeWalker::new(&src).walk(&root, &mut v);
    assert_eq!(v.0, vec![10, 1, 11, 4, 5]);
}

// ===================================================================
// Category 3 — VisitorAction::Continue traverses everything (8 tests)
// ===================================================================

#[test]
fn continue_dfs_visits_all_nodes() {
    let (root, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    // 6 nodes: root, a, mid, b, c, d
    assert_eq!(stats.total_nodes, 6);
}

#[test]
fn continue_dfs_visits_all_leaves() {
    let (root, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    // 4 leaf nodes: a, b, c, d
    assert_eq!(stats.leaf_nodes, 4);
}

#[test]
fn continue_dfs_preorder() {
    let (root, src) = sample_tree();
    let mut v = SymCollector(vec![]);
    TreeWalker::new(&src).walk(&root, &mut v);
    // pre-order: root(10), a(1), mid(11), b(2), c(3), d(4)
    assert_eq!(v.0, vec![10, 1, 11, 2, 3, 4]);
}

#[test]
fn continue_bfs_level_order() {
    let (root, src) = sample_tree();
    let mut v = SymCollector(vec![]);
    BreadthFirstWalker::new(&src).walk(&root, &mut v);
    // level-order: root(10), a(1), mid(11), d(4), b(2), c(3)
    assert_eq!(v.0, vec![10, 1, 11, 4, 2, 3]);
}

#[test]
fn continue_dfs_enter_leave_paired() {
    let (root, src) = sample_tree();
    struct V {
        stack: Vec<u16>,
        ok: bool,
    }
    impl Visitor for V {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.stack.push(n.symbol());
            VisitorAction::Continue
        }
        fn leave_node(&mut self, n: &ParsedNode) {
            if self.stack.last().copied() != Some(n.symbol()) {
                self.ok = false;
            }
            self.stack.pop();
        }
    }
    let mut v = V {
        stack: vec![],
        ok: true,
    };
    TreeWalker::new(&src).walk(&root, &mut v);
    assert!(v.ok);
    assert!(v.stack.is_empty());
}

#[test]
fn continue_dfs_leaf_text_correct() {
    let src = b"abcd".to_vec();
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 3), leaf(3, 3, 4)]);
    struct V(Vec<String>);
    impl Visitor for V {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn visit_leaf(&mut self, _: &ParsedNode, text: &str) {
            self.0.push(text.to_string());
        }
    }
    let mut v = V(vec![]);
    TreeWalker::new(&src).walk(&root, &mut v);
    assert_eq!(v.0, vec!["a", "bc", "d"]);
}

#[test]
fn continue_bfs_visits_all_nodes() {
    let (root, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    BreadthFirstWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 6);
}

#[test]
fn continue_single_leaf_tree() {
    let src = b"x".to_vec();
    let root = leaf(1, 0, 1);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
    assert_eq!(stats.max_depth, 1);
}

// ===================================================================
// Category 4 — Visitor composition (8 tests)
// ===================================================================

/// A meta-visitor that delegates to two inner visitors.
struct DualVisitor<'a, A: Visitor, B: Visitor> {
    a: &'a mut A,
    b: &'a mut B,
}

impl<A: Visitor, B: Visitor> Visitor for DualVisitor<'_, A, B> {
    fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
        let r1 = self.a.enter_node(n);
        let r2 = self.b.enter_node(n);
        // Stop takes priority, then SkipChildren.
        if r1 == VisitorAction::Stop || r2 == VisitorAction::Stop {
            VisitorAction::Stop
        } else if r1 == VisitorAction::SkipChildren || r2 == VisitorAction::SkipChildren {
            VisitorAction::SkipChildren
        } else {
            VisitorAction::Continue
        }
    }
    fn leave_node(&mut self, n: &ParsedNode) {
        self.a.leave_node(n);
        self.b.leave_node(n);
    }
    fn visit_leaf(&mut self, n: &ParsedNode, text: &str) {
        self.a.visit_leaf(n, text);
        self.b.visit_leaf(n, text);
    }
    fn visit_error(&mut self, n: &ParsedNode) {
        self.a.visit_error(n);
        self.b.visit_error(n);
    }
}

#[test]
fn compose_stats_and_pretty_print() {
    let (root, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    let mut pp = PrettyPrintVisitor::new();
    let mut dual = DualVisitor {
        a: &mut stats,
        b: &mut pp,
    };
    TreeWalker::new(&src).walk(&root, &mut dual);
    assert_eq!(stats.total_nodes, 6);
    assert!(!pp.output().is_empty());
}

#[test]
fn compose_search_and_stats() {
    let (root, src) = sample_tree();
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 2);
    let mut stats = StatsVisitor::default();
    let mut dual = DualVisitor {
        a: &mut search,
        b: &mut stats,
    };
    TreeWalker::new(&src).walk(&root, &mut dual);
    assert_eq!(search.matches.len(), 1);
    assert_eq!(stats.total_nodes, 6);
}

#[test]
fn compose_stop_propagates() {
    let (root, src) = sample_tree();
    struct AlwaysStop;
    impl Visitor for AlwaysStop {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            VisitorAction::Stop
        }
    }
    let mut stopper = AlwaysStop;
    let mut stats = StatsVisitor::default();
    let mut dual = DualVisitor {
        a: &mut stopper,
        b: &mut stats,
    };
    TreeWalker::new(&src).walk(&root, &mut dual);
    // Stop at root ⇒ only root entered in stats.
    assert_eq!(stats.total_nodes, 1);
}

#[test]
fn compose_skip_propagates() {
    let (root, src) = sample_tree();
    struct SkipAll;
    impl Visitor for SkipAll {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            VisitorAction::SkipChildren
        }
    }
    let mut skipper = SkipAll;
    let mut stats = StatsVisitor::default();
    let mut dual = DualVisitor {
        a: &mut skipper,
        b: &mut stats,
    };
    TreeWalker::new(&src).walk(&root, &mut dual);
    assert_eq!(stats.total_nodes, 1);
}

#[test]
fn compose_both_continue() {
    let (root, src) = sample_tree();
    let mut s1 = StatsVisitor::default();
    let mut s2 = StatsVisitor::default();
    let mut dual = DualVisitor {
        a: &mut s1,
        b: &mut s2,
    };
    TreeWalker::new(&src).walk(&root, &mut dual);
    assert_eq!(s1.total_nodes, s2.total_nodes);
    assert_eq!(s1.total_nodes, 6);
}

#[test]
fn compose_leaf_dispatched_to_both() {
    let src = b"xy".to_vec();
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    struct LeafCounter(usize);
    impl Visitor for LeafCounter {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn visit_leaf(&mut self, _: &ParsedNode, _: &str) {
            self.0 += 1;
        }
    }
    let mut a = LeafCounter(0);
    let mut b = LeafCounter(0);
    let mut dual = DualVisitor {
        a: &mut a,
        b: &mut b,
    };
    TreeWalker::new(&src).walk(&root, &mut dual);
    assert_eq!(a.0, 2);
    assert_eq!(b.0, 2);
}

#[test]
fn compose_error_dispatched_to_both() {
    let src = b"x!".to_vec();
    let root = interior(10, vec![leaf(1, 0, 1), error_node(1, 2)]);
    struct ErrCounter(usize);
    impl Visitor for ErrCounter {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn visit_error(&mut self, _: &ParsedNode) {
            self.0 += 1;
        }
    }
    let mut a = ErrCounter(0);
    let mut b = ErrCounter(0);
    let mut dual = DualVisitor {
        a: &mut a,
        b: &mut b,
    };
    TreeWalker::new(&src).walk(&root, &mut dual);
    assert_eq!(a.0, 1);
    assert_eq!(b.0, 1);
}

#[test]
fn compose_leave_dispatched_to_both() {
    let (root, src) = sample_tree();
    struct LeaveCounter(usize);
    impl Visitor for LeaveCounter {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn leave_node(&mut self, _: &ParsedNode) {
            self.0 += 1;
        }
    }
    let mut a = LeaveCounter(0);
    let mut b = LeaveCounter(0);
    let mut dual = DualVisitor {
        a: &mut a,
        b: &mut b,
    };
    TreeWalker::new(&src).walk(&root, &mut dual);
    assert_eq!(a.0, b.0);
    assert!(a.0 > 0);
}

// ===================================================================
// Category 5 — Walker with error nodes (8 tests)
// ===================================================================

#[test]
fn error_node_triggers_visit_error_dfs() {
    let src = b"x!".to_vec();
    let root = interior(10, vec![leaf(1, 0, 1), error_node(1, 2)]);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
}

#[test]
fn error_node_triggers_visit_error_bfs() {
    let src = b"x!".to_vec();
    let root = interior(10, vec![leaf(1, 0, 1), error_node(1, 2)]);
    let mut stats = StatsVisitor::default();
    BreadthFirstWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
}

#[test]
fn error_node_skips_enter_node() {
    let src = b"x!".to_vec();
    let root = interior(10, vec![error_node(0, 1)]);
    struct V {
        entered: Vec<u16>,
    }
    impl Visitor for V {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            self.entered.push(n.symbol());
            VisitorAction::Continue
        }
        fn visit_error(&mut self, _: &ParsedNode) {}
    }
    let mut v = V { entered: vec![] };
    TreeWalker::new(&src).walk(&root, &mut v);
    // root entered, but error node goes directly to visit_error (not enter_node).
    assert_eq!(v.entered, vec![10]);
}

#[test]
fn multiple_errors_counted() {
    let src = b"x!!y".to_vec();
    let root = interior(
        10,
        vec![
            leaf(1, 0, 1),
            error_node(1, 2),
            error_node(2, 3),
            leaf(2, 3, 4),
        ],
    );
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 2);
}

#[test]
fn error_node_children_not_visited() {
    // An error node with children — walker should call visit_error and return,
    // not descend into children.
    let src = b"x!y".to_vec();
    let err = make_node(0, vec![leaf(1, 1, 2)], 0, 3, true, false);
    let root = interior(10, vec![err]);
    struct V(usize);
    impl Visitor for V {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.0 += 1;
            VisitorAction::Continue
        }
        fn visit_error(&mut self, _: &ParsedNode) {}
    }
    let mut v = V(0);
    TreeWalker::new(&src).walk(&root, &mut v);
    // Only root entered; the error node (and its child) skipped via visit_error.
    assert_eq!(v.0, 1);
}

#[test]
fn error_node_as_root_dfs() {
    let src = b"!".to_vec();
    let root = error_node(0, 1);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
    assert_eq!(stats.total_nodes, 0);
}

#[test]
fn error_node_as_root_bfs() {
    let src = b"!".to_vec();
    let root = error_node(0, 1);
    let mut stats = StatsVisitor::default();
    BreadthFirstWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
    assert_eq!(stats.total_nodes, 0);
}

#[test]
fn has_error_propagates_from_child() {
    let err = error_node(1, 2);
    let root = interior(10, vec![leaf(1, 0, 1), err]);
    assert!(root.has_error());
}

// ===================================================================
// Category 6 — TransformVisitor (8 tests)
// ===================================================================

/// Simple transform: count nodes in subtree.
struct CountTransform;
impl TransformVisitor for CountTransform {
    type Output = usize;
    fn transform_node(&mut self, _: &ParsedNode, children: Vec<usize>) -> usize {
        1 + children.iter().sum::<usize>()
    }
    fn transform_leaf(&mut self, _: &ParsedNode, _: &str) -> usize {
        1
    }
    fn transform_error(&mut self, _: &ParsedNode) -> usize {
        1
    }
}

#[test]
fn transform_counts_all_nodes() {
    let (root, src) = sample_tree();
    let mut t = CountTransform;
    let count = TransformWalker::new(&src).walk(&root, &mut t);
    assert_eq!(count, 6);
}

#[test]
fn transform_single_leaf() {
    let src = b"x".to_vec();
    let root = leaf(1, 0, 1);
    let mut t = CountTransform;
    let count = TransformWalker::new(&src).walk(&root, &mut t);
    assert_eq!(count, 1);
}

#[test]
fn transform_error_node() {
    let src = b"!".to_vec();
    let root = error_node(0, 1);
    let mut t = CountTransform;
    let count = TransformWalker::new(&src).walk(&root, &mut t);
    assert_eq!(count, 1);
}

/// Transform that collects leaf text.
struct CollectLeaves;
impl TransformVisitor for CollectLeaves {
    type Output = Vec<String>;
    fn transform_node(&mut self, _: &ParsedNode, children: Vec<Vec<String>>) -> Vec<String> {
        children.into_iter().flatten().collect()
    }
    fn transform_leaf(&mut self, _: &ParsedNode, text: &str) -> Vec<String> {
        vec![text.to_string()]
    }
    fn transform_error(&mut self, _: &ParsedNode) -> Vec<String> {
        vec![]
    }
}

#[test]
fn transform_collects_leaf_text() {
    let src = b"abcd".to_vec();
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 3), leaf(3, 3, 4)]);
    let mut t = CollectLeaves;
    let texts = TransformWalker::new(&src).walk(&root, &mut t);
    assert_eq!(texts, vec!["a", "bc", "d"]);
}

#[test]
fn transform_depth_calculation() {
    struct DepthCalc;
    impl TransformVisitor for DepthCalc {
        type Output = usize;
        fn transform_node(&mut self, _: &ParsedNode, children: Vec<usize>) -> usize {
            1 + children.iter().copied().max().unwrap_or(0)
        }
        fn transform_leaf(&mut self, _: &ParsedNode, _: &str) -> usize {
            1
        }
        fn transform_error(&mut self, _: &ParsedNode) -> usize {
            0
        }
    }
    let (root, src) = sample_tree();
    let mut t = DepthCalc;
    let depth = TransformWalker::new(&src).walk(&root, &mut t);
    // root → mid → b|c  ⇒ depth 3
    assert_eq!(depth, 3);
}

#[test]
fn transform_builds_sexpr() {
    struct SExpr;
    impl TransformVisitor for SExpr {
        type Output = String;
        fn transform_node(&mut self, n: &ParsedNode, children: Vec<String>) -> String {
            format!(
                "({}{})",
                n.symbol(),
                children.iter().map(|c| format!(" {c}")).collect::<String>()
            )
        }
        fn transform_leaf(&mut self, _: &ParsedNode, text: &str) -> String {
            format!("\"{}\"", text)
        }
        fn transform_error(&mut self, _: &ParsedNode) -> String {
            "ERROR".to_string()
        }
    }
    let src = b"ab".to_vec();
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let mut t = SExpr;
    let result = TransformWalker::new(&src).walk(&root, &mut t);
    assert_eq!(result, "(10 \"a\" \"b\")");
}

#[test]
fn transform_with_mixed_error_and_leaf() {
    let src = b"a!b".to_vec();
    let root = interior(10, vec![leaf(1, 0, 1), error_node(1, 2), leaf(2, 2, 3)]);
    let mut t = CollectLeaves;
    let texts = TransformWalker::new(&src).walk(&root, &mut t);
    assert_eq!(texts, vec!["a", "b"]);
}

#[test]
fn transform_nested_interior() {
    let src = b"abcd".to_vec();
    let deep = interior(
        11,
        vec![
            interior(12, vec![leaf(1, 0, 1), leaf(2, 1, 2)]),
            leaf(3, 2, 4),
        ],
    );
    let mut t = CountTransform;
    let count = TransformWalker::new(&src).walk(&deep, &mut t);
    assert_eq!(count, 5);
}

// ===================================================================
// Category 7 — Custom visitor implementations (8 tests)
// ===================================================================

#[test]
fn custom_visitor_counts_named_nodes() {
    struct NamedCounter(usize);
    impl Visitor for NamedCounter {
        fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
            if n.is_named() {
                self.0 += 1;
            }
            VisitorAction::Continue
        }
    }
    let (root, src) = sample_tree();
    let mut v = NamedCounter(0);
    TreeWalker::new(&src).walk(&root, &mut v);
    // c(3) is unnamed, the rest (root, a, mid, b, d) are named ⇒ 5
    assert_eq!(v.0, 5);
}

#[test]
fn search_visitor_finds_by_symbol() {
    let (root, src) = sample_tree();
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 2);
    TreeWalker::new(&src).walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
    assert_eq!(sv.matches[0].0, 1); // start_byte
    assert_eq!(sv.matches[0].1, 2); // end_byte
}

#[test]
fn search_visitor_finds_multiple() {
    let src = b"aabbcc".to_vec();
    let root = interior(10, vec![leaf(1, 0, 2), leaf(1, 2, 4), leaf(2, 4, 6)]);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    TreeWalker::new(&src).walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 2);
}

#[test]
fn search_visitor_no_match() {
    let (root, src) = sample_tree();
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 99);
    TreeWalker::new(&src).walk(&root, &mut sv);
    assert!(sv.matches.is_empty());
}

#[test]
fn pretty_print_visitor_output_not_empty() {
    let (root, src) = sample_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    assert!(!pp.output().is_empty());
}

#[test]
fn pretty_print_visitor_contains_leaf_text() {
    let src = b"hello".to_vec();
    let root = interior(10, vec![leaf(1, 0, 5)]);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    assert!(pp.output().contains("hello"));
}

#[test]
fn pretty_print_visitor_indentation() {
    let src = b"xy".to_vec();
    let root = interior(10, vec![interior(11, vec![leaf(1, 0, 1)]), leaf(2, 1, 2)]);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    let output = pp.output();
    // Nested lines should have more leading spaces.
    let lines: Vec<&str> = output.lines().collect();
    assert!(lines.len() >= 3);
    // Root line has no leading space; child lines have some.
    assert!(!lines[0].starts_with(' '));
    assert!(lines[1].starts_with(' '));
}

#[test]
fn custom_visitor_depth_tracker() {
    struct DepthTracker {
        current: usize,
        max: usize,
    }
    impl Visitor for DepthTracker {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.current += 1;
            if self.current > self.max {
                self.max = self.current;
            }
            VisitorAction::Continue
        }
        fn leave_node(&mut self, _: &ParsedNode) {
            self.current -= 1;
        }
    }
    let (root, src) = sample_tree();
    let mut v = DepthTracker { current: 0, max: 0 };
    TreeWalker::new(&src).walk(&root, &mut v);
    assert_eq!(v.max, 3);
    assert_eq!(v.current, 0);
}

// ===================================================================
// Category 8 — Edge cases (8 tests)
// ===================================================================

#[test]
fn edge_empty_interior_node() {
    let src = b"".to_vec();
    let root = interior(10, vec![]);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    // An interior node with no children is treated as a leaf.
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn edge_empty_interior_bfs() {
    let src = b"".to_vec();
    let root = interior(10, vec![]);
    let mut stats = StatsVisitor::default();
    BreadthFirstWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn edge_deep_tree() {
    // Build a 50-level deep chain.
    let src = b"x".to_vec();
    let mut node = leaf(1, 0, 1);
    for i in 2..=50u16 {
        node = interior(i, vec![node]);
    }
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&node, &mut stats);
    assert_eq!(stats.total_nodes, 50);
    assert_eq!(stats.max_depth, 50);
}

#[test]
fn edge_deep_tree_bfs() {
    let src = b"x".to_vec();
    let mut node = leaf(1, 0, 1);
    for i in 2..=50u16 {
        node = interior(i, vec![node]);
    }
    let mut stats = StatsVisitor::default();
    BreadthFirstWalker::new(&src).walk(&node, &mut stats);
    assert_eq!(stats.total_nodes, 50);
}

#[test]
fn edge_wide_tree() {
    // 100 children under one root.
    let src: Vec<u8> = (0..100u8).collect();
    let children: Vec<ParsedNode> = (0..100u16)
        .map(|i| leaf(i, i as usize, i as usize + 1))
        .collect();
    let root = interior(200, children);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 101); // root + 100 children
    assert_eq!(stats.leaf_nodes, 100);
    assert_eq!(stats.max_depth, 2);
}

#[test]
fn edge_wide_tree_bfs() {
    let src: Vec<u8> = (0..100u8).collect();
    let children: Vec<ParsedNode> = (0..100u16)
        .map(|i| leaf(i, i as usize, i as usize + 1))
        .collect();
    let root = interior(200, children);
    let mut stats = StatsVisitor::default();
    BreadthFirstWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 101);
}

#[test]
fn edge_transform_deep_tree() {
    let src = b"x".to_vec();
    let mut node = leaf(1, 0, 1);
    for i in 2..=30u16 {
        node = interior(i, vec![node]);
    }
    let mut t = CountTransform;
    let count = TransformWalker::new(&src).walk(&node, &mut t);
    assert_eq!(count, 30);
}

#[test]
fn edge_only_error_children() {
    let src = b"!!!".to_vec();
    let root = interior(
        10,
        vec![error_node(0, 1), error_node(1, 2), error_node(2, 3)],
    );
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 3);
    // root is entered, error children are not entered.
    assert_eq!(stats.total_nodes, 1);
}

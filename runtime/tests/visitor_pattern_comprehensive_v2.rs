//! Comprehensive v2 tests for the visitor/walker pattern in the adze runtime.
//!
//! Covers: VisitorAction variants, StatsVisitor, PrettyPrintVisitor,
//! SearchVisitor, TreeWalker (DFS), BreadthFirstWalker (BFS),
//! TransformVisitor/TransformWalker, empty/single/deep/wide trees,
//! error nodes, visitor composition, and custom visitor implementations.

use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TransformVisitor,
    TransformWalker, TreeWalker, Visitor, VisitorAction,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Construct a `ParsedNode`. The `language` field is `pub(crate)` so we
/// Construct a synthetic `ParsedNode` with standard defaults.
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

// Symbol-to-kind mapping (when language is None):
// 0 => "end", 1 => "*", 2 => "_2", 3 => "_6", 4 => "-",
// 5 => "Expression", 6 => "Whitespace__whitespace", 7 => "Whitespace",
// 8 => "Expression_Sub_1", 9 => "Expression_Sub", 10 => "rule_10"

/// Sample tree:
///   root(10)
///   ├── a(1)          "a"
///   ├── mid(5)
///   │   ├── b(2)      "b"
///   │   └── c(3)      "c"  (unnamed)
///   └── d(4)          "d"
/// Source: "abcd"
fn sample_tree() -> (ParsedNode, Vec<u8>) {
    let src = b"abcd".to_vec();
    let a = leaf(1, 0, 1);
    let b = leaf(2, 1, 2);
    let c = unnamed_leaf(3, 2, 3);
    let mid = interior(5, vec![b, c]);
    let d = leaf(4, 3, 4);
    let root = interior(10, vec![a, mid, d]);
    (root, src)
}

/// Deep linear tree with `depth` interior nodes wrapping a single leaf.
fn deep_tree(depth: usize) -> (ParsedNode, Vec<u8>) {
    let src = b"x".to_vec();
    let mut node = leaf(1, 0, 1);
    for _ in 0..depth {
        node = interior(10, vec![node]);
    }
    (node, src)
}

/// Wide tree: root with `width` leaf children.
fn wide_tree(width: usize) -> (ParsedNode, Vec<u8>) {
    let src: Vec<u8> = (0..width).map(|_| b'a').collect();
    let children: Vec<ParsedNode> = (0..width).map(|i| leaf(1, i, i + 1)).collect();
    let root = interior(10, children);
    (root, src)
}

/// Build a balanced binary tree of given depth. Leaf text comes from `src`.
fn balanced_tree(depth: usize, src: &[u8], offset: &mut usize) -> ParsedNode {
    if depth == 0 {
        let start = *offset;
        let end = (*offset + 1).min(src.len());
        *offset = end;
        return leaf(1, start, end);
    }
    let left = balanced_tree(depth - 1, src, offset);
    let right = balanced_tree(depth - 1, src, offset);
    interior(10, vec![left, right])
}

// ---------------------------------------------------------------------------
// Reusable visitors
// ---------------------------------------------------------------------------

/// Records enter-order of symbol IDs.
#[derive(Default)]
struct OrderVisitor {
    entered: Vec<u16>,
}

impl Visitor for OrderVisitor {
    fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
        self.entered.push(node.symbol());
        VisitorAction::Continue
    }
}

/// Records leaf texts in visit order.
#[derive(Default)]
struct LeafCollector {
    leaves: Vec<String>,
}

impl Visitor for LeafCollector {
    fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
        VisitorAction::Continue
    }
    fn visit_leaf(&mut self, _: &ParsedNode, text: &str) {
        self.leaves.push(text.to_string());
    }
}

/// Records leave-order of symbol IDs.
#[derive(Default)]
struct PostOrderVisitor {
    left: Vec<u16>,
}

impl Visitor for PostOrderVisitor {
    fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
        VisitorAction::Continue
    }
    fn leave_node(&mut self, node: &ParsedNode) {
        self.left.push(node.symbol());
    }
}

/// Counts error nodes encountered.
#[derive(Default)]
struct ErrorCounter {
    errors: usize,
}

impl Visitor for ErrorCounter {
    fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
        VisitorAction::Continue
    }
    fn visit_error(&mut self, _: &ParsedNode) {
        self.errors += 1;
    }
}

// ===================================================================
// 1. VisitorAction variants and behavior
// ===================================================================

#[test]
fn visitor_action_continue_eq() {
    assert_eq!(VisitorAction::Continue, VisitorAction::Continue);
}

#[test]
fn visitor_action_skip_eq() {
    assert_eq!(VisitorAction::SkipChildren, VisitorAction::SkipChildren);
}

#[test]
fn visitor_action_stop_eq() {
    assert_eq!(VisitorAction::Stop, VisitorAction::Stop);
}

#[test]
fn visitor_action_ne_continue_skip() {
    assert_ne!(VisitorAction::Continue, VisitorAction::SkipChildren);
}

#[test]
fn visitor_action_ne_continue_stop() {
    assert_ne!(VisitorAction::Continue, VisitorAction::Stop);
}

#[test]
fn visitor_action_ne_skip_stop() {
    assert_ne!(VisitorAction::SkipChildren, VisitorAction::Stop);
}

#[test]
fn visitor_action_debug_format() {
    let dbg = format!("{:?}", VisitorAction::Continue);
    assert_eq!(dbg, "Continue");
    assert_eq!(format!("{:?}", VisitorAction::Stop), "Stop");
    assert_eq!(format!("{:?}", VisitorAction::SkipChildren), "SkipChildren");
}

#[test]
fn visitor_action_clone() {
    let a = VisitorAction::SkipChildren;
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn visitor_action_copy() {
    let a = VisitorAction::Stop;
    let b = a;
    // `a` still usable — `Copy`
    assert_eq!(a, b);
}

// ===================================================================
// 2. StatsVisitor
// ===================================================================

#[test]
fn stats_default_is_zeroed() {
    let stats = StatsVisitor::default();
    assert_eq!(stats.total_nodes, 0);
    assert_eq!(stats.leaf_nodes, 0);
    assert_eq!(stats.error_nodes, 0);
    assert_eq!(stats.max_depth, 0);
    assert!(stats.node_counts.is_empty());
}

#[test]
fn stats_sample_total_nodes() {
    let (tree, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&tree, &mut stats);
    assert_eq!(stats.total_nodes, 6);
}

#[test]
fn stats_sample_leaf_count() {
    let (tree, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&tree, &mut stats);
    assert_eq!(stats.leaf_nodes, 4);
}

#[test]
fn stats_sample_max_depth() {
    let (tree, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&tree, &mut stats);
    assert_eq!(stats.max_depth, 3);
}

#[test]
fn stats_sample_node_counts_per_kind() {
    let (tree, src) = sample_tree();
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&tree, &mut stats);
    assert_eq!(stats.node_counts.get("rule_10"), Some(&1));
    assert_eq!(stats.node_counts.get("Expression"), Some(&1));
    assert_eq!(stats.node_counts.get("*"), Some(&1)); // symbol 1
}

#[test]
fn stats_error_node_counted() {
    let src = b"e".to_vec();
    let err = error_node(0, 1);
    let root = interior(10, vec![err]);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
}

#[test]
fn stats_deep_tree_depth_matches() {
    let (tree, src) = deep_tree(25);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&tree, &mut stats);
    assert_eq!(stats.max_depth, 26);
    assert_eq!(stats.total_nodes, 26);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn stats_wide_tree_depth_is_two() {
    let (tree, src) = wide_tree(40);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&tree, &mut stats);
    assert_eq!(stats.max_depth, 2);
    assert_eq!(stats.leaf_nodes, 40);
    assert_eq!(stats.total_nodes, 41);
}

#[test]
fn stats_single_leaf_tree() {
    let src = b"z".to_vec();
    let node = leaf(1, 0, 1);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&node, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
    assert_eq!(stats.max_depth, 1);
}

#[test]
fn stats_balanced_tree() {
    let src = b"abcdefgh".to_vec();
    let mut off = 0;
    let tree = balanced_tree(3, &src, &mut off);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&tree, &mut stats);
    // 8 leaves + 4 + 2 + 1 interior = 15 nodes
    assert_eq!(stats.total_nodes, 15);
    assert_eq!(stats.leaf_nodes, 8);
    assert_eq!(stats.max_depth, 4);
}

#[test]
fn stats_multiple_error_nodes() {
    let src = b"ee".to_vec();
    let e1 = error_node(0, 1);
    let e2 = error_node(1, 2);
    let root = interior(10, vec![e1, e2]);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 2);
}

// ===================================================================
// 3. PrettyPrintVisitor
// ===================================================================

#[test]
fn pretty_print_default_empty() {
    let pp = PrettyPrintVisitor::default();
    assert!(pp.output().is_empty());
}

#[test]
fn pretty_print_new_empty() {
    let pp = PrettyPrintVisitor::new();
    assert_eq!(pp.output(), "");
}

#[test]
fn pretty_print_contains_root_kind() {
    let (tree, src) = sample_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&tree, &mut pp);
    assert!(pp.output().contains("rule_10"));
}

#[test]
fn pretty_print_contains_child_kind() {
    let (tree, src) = sample_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&tree, &mut pp);
    assert!(pp.output().contains("Expression"));
}

#[test]
fn pretty_print_contains_leaf_text_quoted() {
    let (tree, src) = sample_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&tree, &mut pp);
    assert!(pp.output().contains("\"a\""));
    assert!(pp.output().contains("\"b\""));
    assert!(pp.output().contains("\"d\""));
}

#[test]
fn pretty_print_named_annotation_present() {
    let (tree, src) = sample_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&tree, &mut pp);
    assert!(pp.output().contains("[named]"));
}

#[test]
fn pretty_print_unnamed_node_no_named_tag() {
    // c(3) is unnamed — its line should NOT contain [named]
    let (tree, src) = sample_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&tree, &mut pp);
    let lines: Vec<&str> = pp.output().lines().collect();
    let c_line = lines.iter().find(|l| l.contains("_6")).unwrap();
    assert!(!c_line.contains("[named]"));
}

#[test]
fn pretty_print_root_no_indent() {
    let (tree, src) = sample_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&tree, &mut pp);
    let first_line = pp.output().lines().next().unwrap();
    assert!(!first_line.starts_with(' '));
}

#[test]
fn pretty_print_children_indented() {
    let (tree, src) = sample_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&tree, &mut pp);
    // Children of root should be indented by 2 spaces
    let lines: Vec<&str> = pp.output().lines().collect();
    let child_line = lines
        .iter()
        .find(|l| l.trim_start().starts_with('*'))
        .unwrap();
    assert!(child_line.starts_with("  "));
}

#[test]
fn pretty_print_grandchildren_double_indented() {
    let (tree, src) = sample_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&tree, &mut pp);
    // b(2) = "_2" is a grandchild under mid(5)
    let lines: Vec<&str> = pp.output().lines().collect();
    let b_line = lines.iter().find(|l| l.contains("_2")).unwrap();
    assert!(b_line.starts_with("    ")); // 4 spaces = depth 2
}

#[test]
fn pretty_print_error_node_output() {
    let src = b"e".to_vec();
    let err = error_node(0, 1);
    let root = interior(10, vec![err]);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    assert!(pp.output().contains("ERROR"));
}

#[test]
fn pretty_print_single_leaf() {
    let src = b"z".to_vec();
    let node = leaf(1, 0, 1);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&node, &mut pp);
    assert!(pp.output().contains("\"z\""));
    assert!(pp.output().contains("[named]"));
}

#[test]
fn pretty_print_deep_tree_has_many_lines() {
    let (tree, src) = deep_tree(5);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&tree, &mut pp);
    let line_count = pp.output().lines().count();
    // 5 interior enter lines + 1 leaf kind line + 1 leaf text line = at least 7
    assert!(line_count >= 6);
}

#[test]
fn pretty_print_ends_with_newline() {
    let (tree, src) = sample_tree();
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&tree, &mut pp);
    assert!(pp.output().ends_with('\n'));
}

// ===================================================================
// 4. SearchVisitor
// ===================================================================

#[test]
fn search_finds_by_symbol() {
    let (tree, src) = sample_tree();
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 2);
    TreeWalker::new(&src).walk(&tree, &mut sv);
    assert_eq!(sv.matches.len(), 1);
    assert_eq!(sv.matches[0], (1, 2, "_2".to_string()));
}

#[test]
fn search_returns_empty_when_no_match() {
    let (tree, src) = sample_tree();
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 99);
    TreeWalker::new(&src).walk(&tree, &mut sv);
    assert!(sv.matches.is_empty());
}

#[test]
fn search_finds_multiple_matches() {
    let (tree, src) = sample_tree();
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.is_named() && n.child_count() == 0);
    TreeWalker::new(&src).walk(&tree, &mut sv);
    // Named leaves: a(1), b(2), d(4)  —  c(3) is unnamed
    assert_eq!(sv.matches.len(), 3);
}

#[test]
fn search_by_byte_range() {
    let (tree, src) = sample_tree();
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.start_byte() >= 2 && n.end_byte() <= 4);
    TreeWalker::new(&src).walk(&tree, &mut sv);
    assert!(sv.matches.len() >= 2);
}

#[test]
fn search_all_nodes() {
    let (tree, src) = sample_tree();
    let mut sv = SearchVisitor::new(|_: &ParsedNode| true);
    TreeWalker::new(&src).walk(&tree, &mut sv);
    assert_eq!(sv.matches.len(), 6);
}

#[test]
fn search_no_nodes() {
    let (tree, src) = sample_tree();
    let mut sv = SearchVisitor::new(|_: &ParsedNode| false);
    TreeWalker::new(&src).walk(&tree, &mut sv);
    assert!(sv.matches.is_empty());
}

#[test]
fn search_with_bfs_walker() {
    let (tree, src) = sample_tree();
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 4);
    BreadthFirstWalker::new(&src).walk(&tree, &mut sv);
    assert_eq!(sv.matches.len(), 1);
    assert_eq!(sv.matches[0].2, "-");
}

#[test]
fn search_match_tuple_fields() {
    let (tree, src) = sample_tree();
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    TreeWalker::new(&src).walk(&tree, &mut sv);
    let (start, end, kind) = &sv.matches[0];
    assert_eq!(*start, 0);
    assert_eq!(*end, 1);
    assert_eq!(kind, "*");
}

#[test]
fn search_interior_node() {
    let (tree, src) = sample_tree();
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.child_count() > 0);
    TreeWalker::new(&src).walk(&tree, &mut sv);
    // root(10) and mid(5) have children
    assert_eq!(sv.matches.len(), 2);
}

#[test]
fn search_in_wide_tree() {
    let (tree, src) = wide_tree(20);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.child_count() == 0);
    TreeWalker::new(&src).walk(&tree, &mut sv);
    assert_eq!(sv.matches.len(), 20);
}

// ===================================================================
// 5. TreeWalker (DFS) traversal
// ===================================================================

#[test]
fn dfs_preorder_sequence() {
    let (tree, src) = sample_tree();
    let mut v = OrderVisitor::default();
    TreeWalker::new(&src).walk(&tree, &mut v);
    assert_eq!(v.entered, vec![10, 1, 5, 2, 3, 4]);
}

#[test]
fn dfs_postorder_via_leave() {
    let (tree, src) = sample_tree();
    let mut v = PostOrderVisitor::default();
    TreeWalker::new(&src).walk(&tree, &mut v);
    assert_eq!(v.left, vec![1, 2, 3, 5, 4, 10]);
}

#[test]
fn dfs_leaf_text_order() {
    let (tree, src) = sample_tree();
    let mut v = LeafCollector::default();
    TreeWalker::new(&src).walk(&tree, &mut v);
    assert_eq!(v.leaves, vec!["a", "b", "c", "d"]);
}

#[test]
fn dfs_stop_at_second_node() {
    struct StopAfterTwo {
        count: usize,
        entered: Vec<u16>,
    }
    impl Visitor for StopAfterTwo {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.count += 1;
            self.entered.push(node.symbol());
            if self.count >= 2 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }

    let (tree, src) = sample_tree();
    let mut v = StopAfterTwo {
        count: 0,
        entered: vec![],
    };
    TreeWalker::new(&src).walk(&tree, &mut v);
    // Stop on each child of root after root entered; siblings still visited
    assert_eq!(v.entered, vec![10, 1, 5, 4]);
}

#[test]
fn dfs_skip_children_skips_subtree() {
    struct SkipMid {
        entered: Vec<u16>,
    }
    impl Visitor for SkipMid {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.entered.push(node.symbol());
            if node.symbol() == 5 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }

    let (tree, src) = sample_tree();
    let mut v = SkipMid { entered: vec![] };
    TreeWalker::new(&src).walk(&tree, &mut v);
    assert_eq!(v.entered, vec![10, 1, 5, 4]);
}

#[test]
fn dfs_skip_calls_leave_node() {
    struct SkipLeaveChecker {
        left: Vec<u16>,
    }
    impl Visitor for SkipLeaveChecker {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            if node.symbol() == 5 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
        fn leave_node(&mut self, node: &ParsedNode) {
            self.left.push(node.symbol());
        }
    }

    let (tree, src) = sample_tree();
    let mut v = SkipLeaveChecker { left: vec![] };
    TreeWalker::new(&src).walk(&tree, &mut v);
    // leave_node is called even when SkipChildren is returned
    assert!(v.left.contains(&5));
}

#[test]
fn dfs_error_node_calls_visit_error() {
    let src = b"e".to_vec();
    let err = error_node(0, 1);
    let root = interior(10, vec![err]);
    let mut v = ErrorCounter::default();
    TreeWalker::new(&src).walk(&root, &mut v);
    assert_eq!(v.errors, 1);
}

#[test]
fn dfs_error_node_not_entered() {
    let src = b"e".to_vec();
    let err = error_node(0, 1);
    let root = interior(10, vec![err]);
    let mut v = OrderVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut v);
    // Error nodes trigger visit_error, not enter_node
    // root is entered, error is not
    assert_eq!(v.entered, vec![10]);
}

#[test]
fn dfs_walker_creation() {
    let src = b"test";
    let _walker = TreeWalker::new(src);
}

// ===================================================================
// 6. BreadthFirstWalker traversal
// ===================================================================

#[test]
fn bfs_level_order_sequence() {
    let (tree, src) = sample_tree();
    let mut v = OrderVisitor::default();
    BreadthFirstWalker::new(&src).walk(&tree, &mut v);
    assert_eq!(v.entered, vec![10, 1, 5, 4, 2, 3]);
}

#[test]
fn bfs_leaf_text_level_order() {
    let (tree, src) = sample_tree();
    let mut v = LeafCollector::default();
    BreadthFirstWalker::new(&src).walk(&tree, &mut v);
    // a and d are direct children (level 1), b and c are grandchildren
    assert_eq!(v.leaves, vec!["a", "d", "b", "c"]);
}

#[test]
fn bfs_stop_halts_immediately() {
    struct StopAfterThree {
        count: usize,
        entered: Vec<u16>,
    }
    impl Visitor for StopAfterThree {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.count += 1;
            self.entered.push(node.symbol());
            if self.count >= 3 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }

    let (tree, src) = sample_tree();
    let mut v = StopAfterThree {
        count: 0,
        entered: vec![],
    };
    BreadthFirstWalker::new(&src).walk(&tree, &mut v);
    assert_eq!(v.entered, vec![10, 1, 5]);
}

#[test]
fn bfs_skip_children_does_not_queue_subtree() {
    struct SkipMid {
        entered: Vec<u16>,
    }
    impl Visitor for SkipMid {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.entered.push(node.symbol());
            if node.symbol() == 5 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }

    let (tree, src) = sample_tree();
    let mut v = SkipMid { entered: vec![] };
    BreadthFirstWalker::new(&src).walk(&tree, &mut v);
    assert_eq!(v.entered, vec![10, 1, 5, 4]);
}

#[test]
fn bfs_error_node_triggers_visit_error() {
    let src = b"e".to_vec();
    let err = error_node(0, 1);
    let root = interior(10, vec![err]);
    let mut v = ErrorCounter::default();
    BreadthFirstWalker::new(&src).walk(&root, &mut v);
    assert_eq!(v.errors, 1);
}

#[test]
fn bfs_error_node_not_entered() {
    let src = b"e".to_vec();
    let err = error_node(0, 1);
    let root = interior(10, vec![err]);
    let mut v = OrderVisitor::default();
    BreadthFirstWalker::new(&src).walk(&root, &mut v);
    assert_eq!(v.entered, vec![10]);
}

#[test]
fn bfs_walker_creation() {
    let src = b"test";
    let _walker = BreadthFirstWalker::new(src);
}

// ===================================================================
// 7. Empty / single-node trees
// ===================================================================

#[test]
fn single_leaf_dfs_leaf_collected() {
    let src = b"z".to_vec();
    let node = leaf(1, 0, 1);
    let mut v = LeafCollector::default();
    TreeWalker::new(&src).walk(&node, &mut v);
    assert_eq!(v.leaves, vec!["z"]);
}

#[test]
fn single_leaf_bfs_leaf_collected() {
    let src = b"z".to_vec();
    let node = leaf(1, 0, 1);
    let mut v = LeafCollector::default();
    BreadthFirstWalker::new(&src).walk(&node, &mut v);
    assert_eq!(v.leaves, vec!["z"]);
}

#[test]
fn single_leaf_stats() {
    let src = b"z".to_vec();
    let node = leaf(1, 0, 1);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&node, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
    assert_eq!(stats.max_depth, 1);
}

#[test]
fn single_leaf_pretty_print() {
    let src = b"z".to_vec();
    let node = leaf(1, 0, 1);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&node, &mut pp);
    let out = pp.output();
    assert!(out.contains("\"z\""));
}

#[test]
fn single_error_node() {
    let src = b"e".to_vec();
    let err = error_node(0, 1);
    let mut v = ErrorCounter::default();
    TreeWalker::new(&src).walk(&err, &mut v);
    assert_eq!(v.errors, 1);
}

#[test]
fn single_unnamed_leaf() {
    let src = b"u".to_vec();
    let node = unnamed_leaf(3, 0, 1);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&node, &mut pp);
    let out = pp.output();
    assert!(!out.contains("[named]"));
    assert!(out.contains("\"u\""));
}

#[test]
fn interior_no_children_is_leaf() {
    let src = b"".to_vec();
    let node = make_node(10, vec![], 0, 0, false, true);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&node, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
}

// ===================================================================
// 8. Deep trees
// ===================================================================

#[test]
fn deep_tree_dfs_visits_all() {
    let (tree, src) = deep_tree(20);
    let mut v = OrderVisitor::default();
    TreeWalker::new(&src).walk(&tree, &mut v);
    assert_eq!(v.entered.len(), 21);
}

#[test]
fn deep_tree_bfs_visits_all() {
    let (tree, src) = deep_tree(20);
    let mut v = OrderVisitor::default();
    BreadthFirstWalker::new(&src).walk(&tree, &mut v);
    assert_eq!(v.entered.len(), 21);
}

#[test]
fn deep_tree_stats_depth() {
    let (tree, src) = deep_tree(30);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&tree, &mut stats);
    assert_eq!(stats.max_depth, 31);
}

#[test]
fn deep_tree_single_leaf_text() {
    let (tree, src) = deep_tree(10);
    let mut v = LeafCollector::default();
    TreeWalker::new(&src).walk(&tree, &mut v);
    assert_eq!(v.leaves, vec!["x"]);
}

// ===================================================================
// 9. Wide trees
// ===================================================================

#[test]
fn wide_tree_dfs_visits_all() {
    let (tree, src) = wide_tree(100);
    let mut v = OrderVisitor::default();
    TreeWalker::new(&src).walk(&tree, &mut v);
    assert_eq!(v.entered.len(), 101);
}

#[test]
fn wide_tree_bfs_visits_all() {
    let (tree, src) = wide_tree(100);
    let mut v = OrderVisitor::default();
    BreadthFirstWalker::new(&src).walk(&tree, &mut v);
    assert_eq!(v.entered.len(), 101);
}

#[test]
fn wide_tree_stats() {
    let (tree, src) = wide_tree(50);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&tree, &mut stats);
    assert_eq!(stats.total_nodes, 51);
    assert_eq!(stats.leaf_nodes, 50);
    assert_eq!(stats.max_depth, 2);
}

#[test]
fn wide_tree_leaf_texts() {
    let (tree, src) = wide_tree(5);
    let mut v = LeafCollector::default();
    TreeWalker::new(&src).walk(&tree, &mut v);
    assert_eq!(v.leaves, vec!["a"; 5]);
}

#[test]
fn wide_tree_search_all_leaves() {
    let (tree, src) = wide_tree(10);
    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.child_count() == 0);
    TreeWalker::new(&src).walk(&tree, &mut sv);
    assert_eq!(sv.matches.len(), 10);
}

// ===================================================================
// 10. TransformVisitor / TransformWalker
// ===================================================================

struct CharCountTransform;

impl TransformVisitor for CharCountTransform {
    type Output = usize;

    fn transform_node(&mut self, _: &ParsedNode, children: Vec<usize>) -> usize {
        children.iter().sum()
    }

    fn transform_leaf(&mut self, _: &ParsedNode, text: &str) -> usize {
        text.len()
    }

    fn transform_error(&mut self, _: &ParsedNode) -> usize {
        0
    }
}

struct SexpTransform;

impl TransformVisitor for SexpTransform {
    type Output = String;

    fn transform_node(&mut self, node: &ParsedNode, children: Vec<String>) -> String {
        format!("({} {})", node.kind(), children.join(" "))
    }

    fn transform_leaf(&mut self, _: &ParsedNode, text: &str) -> String {
        format!("\"{}\"", text)
    }

    fn transform_error(&mut self, _: &ParsedNode) -> String {
        "ERROR".to_string()
    }
}

struct DepthTransform;

impl TransformVisitor for DepthTransform {
    type Output = usize;

    fn transform_node(&mut self, _: &ParsedNode, children: Vec<usize>) -> usize {
        children.iter().copied().max().unwrap_or(0) + 1
    }

    fn transform_leaf(&mut self, _: &ParsedNode, _text: &str) -> usize {
        1
    }

    fn transform_error(&mut self, _: &ParsedNode) -> usize {
        1
    }
}

#[test]
fn transform_char_count_sample() {
    let (tree, src) = sample_tree();
    let count = TransformWalker::new(&src).walk(&tree, &mut CharCountTransform);
    assert_eq!(count, 4);
}

#[test]
fn transform_sexp_contains_root() {
    let (tree, src) = sample_tree();
    let result = TransformWalker::new(&src).walk(&tree, &mut SexpTransform);
    assert!(result.starts_with("(rule_10 "));
}

#[test]
fn transform_sexp_contains_leaves() {
    let (tree, src) = sample_tree();
    let result = TransformWalker::new(&src).walk(&tree, &mut SexpTransform);
    assert!(result.contains("\"a\""));
    assert!(result.contains("\"b\""));
    assert!(result.contains("\"d\""));
}

#[test]
fn transform_error_node() {
    let src = b"e".to_vec();
    let err = error_node(0, 1);
    let result = TransformWalker::new(&src).walk(&err, &mut SexpTransform);
    assert_eq!(result, "ERROR");
}

#[test]
fn transform_depth_sample() {
    let (tree, src) = sample_tree();
    let depth = TransformWalker::new(&src).walk(&tree, &mut DepthTransform);
    assert_eq!(depth, 3);
}

#[test]
fn transform_depth_deep_tree() {
    let (tree, src) = deep_tree(10);
    let depth = TransformWalker::new(&src).walk(&tree, &mut DepthTransform);
    assert_eq!(depth, 11);
}

#[test]
fn transform_char_count_single_leaf() {
    let src = b"z".to_vec();
    let node = leaf(1, 0, 1);
    let count = TransformWalker::new(&src).walk(&node, &mut CharCountTransform);
    assert_eq!(count, 1);
}

#[test]
fn transform_char_count_wide() {
    let (tree, src) = wide_tree(25);
    let count = TransformWalker::new(&src).walk(&tree, &mut CharCountTransform);
    assert_eq!(count, 25);
}

#[test]
fn transform_sexp_single_leaf() {
    let src = b"z".to_vec();
    let node = leaf(1, 0, 1);
    let result = TransformWalker::new(&src).walk(&node, &mut SexpTransform);
    assert_eq!(result, "\"z\"");
}

// ===================================================================
// 11. Visitor composition / chaining
// ===================================================================

#[test]
fn stats_then_pretty_on_same_tree() {
    let (tree, src) = sample_tree();
    let walker = TreeWalker::new(&src);

    let mut stats = StatsVisitor::default();
    walker.walk(&tree, &mut stats);

    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&tree, &mut pp);

    assert_eq!(stats.total_nodes, 6);
    assert!(pp.output().contains("rule_10"));
}

#[test]
fn dfs_then_bfs_on_same_tree() {
    let (tree, src) = sample_tree();

    let mut dfs_order = OrderVisitor::default();
    TreeWalker::new(&src).walk(&tree, &mut dfs_order);

    let mut bfs_order = OrderVisitor::default();
    BreadthFirstWalker::new(&src).walk(&tree, &mut bfs_order);

    // DFS pre-order and BFS level-order should differ
    assert_ne!(dfs_order.entered, bfs_order.entered);
    // But both visit same count of nodes
    assert_eq!(dfs_order.entered.len(), bfs_order.entered.len());
}

#[test]
fn search_then_transform() {
    let (tree, src) = sample_tree();

    let mut sv = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 5);
    TreeWalker::new(&src).walk(&tree, &mut sv);
    assert_eq!(sv.matches.len(), 1);

    let count = TransformWalker::new(&src).walk(&tree, &mut CharCountTransform);
    assert_eq!(count, 4);
}

#[test]
fn multiple_searches_independent() {
    let (tree, src) = sample_tree();
    let walker = TreeWalker::new(&src);

    let mut sv1 = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 1);
    walker.walk(&tree, &mut sv1);

    let mut sv2 = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 4);
    walker.walk(&tree, &mut sv2);

    assert_eq!(sv1.matches.len(), 1);
    assert_eq!(sv2.matches.len(), 1);
    assert_ne!(sv1.matches[0].2, sv2.matches[0].2);
}

// ===================================================================
// 12. Custom visitor implementations
// ===================================================================

#[test]
fn custom_visitor_default_enter_continues() {
    struct EmptyVisitor;
    impl Visitor for EmptyVisitor {}

    let node = leaf(1, 0, 1);
    let mut v = EmptyVisitor;
    assert_eq!(v.enter_node(&node), VisitorAction::Continue);
}

#[test]
fn custom_visitor_default_leave_noop() {
    struct EmptyVisitor;
    impl Visitor for EmptyVisitor {}

    let node = leaf(1, 0, 1);
    let mut v = EmptyVisitor;
    v.leave_node(&node); // should not panic
}

#[test]
fn custom_visitor_default_visit_leaf_noop() {
    struct EmptyVisitor;
    impl Visitor for EmptyVisitor {}

    let node = leaf(1, 0, 1);
    let mut v = EmptyVisitor;
    v.visit_leaf(&node, "x"); // should not panic
}

#[test]
fn custom_visitor_default_visit_error_noop() {
    struct EmptyVisitor;
    impl Visitor for EmptyVisitor {}

    let node = error_node(0, 1);
    let mut v = EmptyVisitor;
    v.visit_error(&node); // should not panic
}

#[test]
fn custom_depth_tracking_visitor() {
    struct DepthTracker {
        depths: Vec<usize>,
        current: usize,
    }
    impl Visitor for DepthTracker {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            self.current += 1;
            self.depths.push(self.current);
            VisitorAction::Continue
        }
        fn leave_node(&mut self, _: &ParsedNode) {
            self.current -= 1;
        }
    }

    let (tree, src) = sample_tree();
    let mut v = DepthTracker {
        depths: vec![],
        current: 0,
    };
    TreeWalker::new(&src).walk(&tree, &mut v);
    // root=1, a=2, mid=2, b=3, c=3, d=2
    assert_eq!(v.depths, vec![1, 2, 2, 3, 3, 2]);
    assert_eq!(v.current, 0);
}

#[test]
fn custom_kind_collector() {
    struct KindCollector {
        kinds: Vec<String>,
    }
    impl Visitor for KindCollector {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.kinds.push(node.kind().to_string());
            VisitorAction::Continue
        }
    }

    let (tree, src) = sample_tree();
    let mut v = KindCollector { kinds: vec![] };
    TreeWalker::new(&src).walk(&tree, &mut v);
    assert_eq!(v.kinds[0], "rule_10");
    assert!(v.kinds.contains(&"Expression".to_string()));
}

#[test]
fn custom_conditional_stop_visitor() {
    struct StopOnExpression {
        visited: Vec<u16>,
    }
    impl Visitor for StopOnExpression {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.visited.push(node.symbol());
            if node.kind() == "Expression" {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }

    let (tree, src) = sample_tree();
    let mut v = StopOnExpression { visited: vec![] };
    TreeWalker::new(&src).walk(&tree, &mut v);
    // root(10) -> a(1) -> mid(5) stops; then sibling d(4) is visited
    assert!(v.visited.contains(&5));
    assert!(!v.visited.contains(&2)); // children of mid skipped
}

#[test]
fn custom_transform_node_count() {
    struct NodeCountTransform;
    impl TransformVisitor for NodeCountTransform {
        type Output = usize;
        fn transform_node(&mut self, _: &ParsedNode, children: Vec<usize>) -> usize {
            1 + children.iter().sum::<usize>()
        }
        fn transform_leaf(&mut self, _: &ParsedNode, _text: &str) -> usize {
            1
        }
        fn transform_error(&mut self, _: &ParsedNode) -> usize {
            1
        }
    }

    let (tree, src) = sample_tree();
    let count = TransformWalker::new(&src).walk(&tree, &mut NodeCountTransform);
    assert_eq!(count, 6);
}

#[test]
fn custom_transform_leaf_list() {
    struct LeafListTransform;
    impl TransformVisitor for LeafListTransform {
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

    let (tree, src) = sample_tree();
    let leaves = TransformWalker::new(&src).walk(&tree, &mut LeafListTransform);
    assert_eq!(leaves, vec!["a", "b", "c", "d"]);
}

#[test]
fn custom_named_only_visitor() {
    struct NamedOnly {
        named_count: usize,
        anonymous_count: usize,
    }
    impl Visitor for NamedOnly {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            if node.is_named() {
                self.named_count += 1;
            } else {
                self.anonymous_count += 1;
            }
            VisitorAction::Continue
        }
    }

    let (tree, src) = sample_tree();
    let mut v = NamedOnly {
        named_count: 0,
        anonymous_count: 0,
    };
    TreeWalker::new(&src).walk(&tree, &mut v);
    // Named: root(10), a(1), mid(5), b(2), d(4) = 5; Anonymous: c(3) = 1
    assert_eq!(v.named_count, 5);
    assert_eq!(v.anonymous_count, 1);
}

// ===================================================================
// Balanced tree tests
// ===================================================================

#[test]
fn balanced_tree_dfs_visits_all() {
    let src = b"abcdefgh".to_vec();
    let mut off = 0;
    let tree = balanced_tree(3, &src, &mut off);
    let mut v = OrderVisitor::default();
    TreeWalker::new(&src).walk(&tree, &mut v);
    assert_eq!(v.entered.len(), 15);
}

#[test]
fn balanced_tree_bfs_visits_all() {
    let src = b"abcdefgh".to_vec();
    let mut off = 0;
    let tree = balanced_tree(3, &src, &mut off);
    let mut v = OrderVisitor::default();
    BreadthFirstWalker::new(&src).walk(&tree, &mut v);
    assert_eq!(v.entered.len(), 15);
}

#[test]
fn balanced_tree_dfs_bfs_same_count() {
    let src = b"abcdefgh".to_vec();
    let mut off = 0;
    let tree = balanced_tree(3, &src, &mut off);

    let mut dfs = OrderVisitor::default();
    TreeWalker::new(&src).walk(&tree, &mut dfs);

    off = 0;
    let tree2 = balanced_tree(3, &src, &mut off);
    let mut bfs = OrderVisitor::default();
    BreadthFirstWalker::new(&src).walk(&tree2, &mut bfs);

    assert_eq!(dfs.entered.len(), bfs.entered.len());
}

// ===================================================================
// Mixed error / normal tree
// ===================================================================

#[test]
fn tree_with_mixed_errors_and_normal() {
    let src = b"abce".to_vec();
    let a = leaf(1, 0, 1);
    let err = error_node(1, 2);
    let c = leaf(2, 2, 3);
    let root = interior(10, vec![a, err, c]);

    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 3); // root + a + c (error not entered)
    assert_eq!(stats.error_nodes, 1);
    assert_eq!(stats.leaf_nodes, 2);
}

#[test]
fn bfs_mixed_errors_order() {
    let src = b"abce".to_vec();
    let a = leaf(1, 0, 1);
    let err = error_node(1, 2);
    let c = leaf(2, 2, 3);
    let root = interior(10, vec![a, err, c]);

    let mut v = OrderVisitor::default();
    BreadthFirstWalker::new(&src).walk(&root, &mut v);
    // root entered, a entered, error skipped (visit_error), c entered
    assert_eq!(v.entered, vec![10, 1, 2]);
}

#[test]
fn pretty_print_mixed_errors() {
    let src = b"ae".to_vec();
    let a = leaf(1, 0, 1);
    let err = error_node(1, 2);
    let root = interior(10, vec![a, err]);

    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(&src).walk(&root, &mut pp);
    let out = pp.output();
    assert!(out.contains("ERROR"));
    assert!(out.contains("\"a\""));
}

// ===================================================================
// Edge: zero-length source
// ===================================================================

#[test]
fn zero_length_leaf() {
    let src = b"".to_vec();
    let node = make_node(1, vec![], 0, 0, false, true);
    let mut v = LeafCollector::default();
    TreeWalker::new(&src).walk(&node, &mut v);
    assert_eq!(v.leaves, vec![""]);
}

#[test]
fn zero_length_leaf_stats() {
    let src = b"".to_vec();
    let node = make_node(1, vec![], 0, 0, false, true);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(&src).walk(&node, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
}

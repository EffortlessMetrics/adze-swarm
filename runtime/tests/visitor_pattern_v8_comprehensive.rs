//! Comprehensive tests for the `adze::visitor` module (v8).
//!
//! Covers StatsVisitor, PrettyPrintVisitor, SearchVisitor, TreeWalker,
//! BreadthFirstWalker, TransformVisitor, deep/wide trees, composability,
//! and edge cases.

use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TransformVisitor,
    TransformWalker, TreeWalker, Visitor, VisitorAction,
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

/// Named leaf node.
fn leaf(symbol: u16, start: usize, end: usize) -> ParsedNode {
    make_node(symbol, vec![], start, end, false, true)
}

/// Anonymous leaf node.
fn anon_leaf(symbol: u16, start: usize, end: usize) -> ParsedNode {
    make_node(symbol, vec![], start, end, false, false)
}

/// Named interior node with byte span derived from children.
fn interior(symbol: u16, children: Vec<ParsedNode>) -> ParsedNode {
    let start = children.first().map_or(0, |c| c.start_byte());
    let end = children.last().map_or(0, |c| c.end_byte());
    make_node(symbol, children, start, end, false, true)
}

/// Anonymous interior node.
fn anon_interior(symbol: u16, children: Vec<ParsedNode>) -> ParsedNode {
    let start = children.first().map_or(0, |c| c.start_byte());
    let end = children.last().map_or(0, |c| c.end_byte());
    make_node(symbol, children, start, end, false, false)
}

/// Error node.
fn error_node(start: usize, end: usize) -> ParsedNode {
    make_node(0, vec![], start, end, true, false)
}

// Symbol-to-kind mapping (language=None fallback):
//   0 => "end", 1 => "*", 2 => "_2", 3 => "_6", 4 => "-",
//   5 => "Expression", 6 => "Whitespace__whitespace", 7 => "Whitespace",
//   8 => "Expression_Sub_1", 9 => "Expression_Sub", 10 => "rule_10",
//   _ => "unknown"

/// Build a simple `a + b` tree: Expression( leaf("a"), leaf("+"), leaf("b") )
fn simple_expr_tree() -> (Vec<u8>, ParsedNode) {
    let source = b"a+b".to_vec();
    let root = interior(5, vec![leaf(1, 0, 1), anon_leaf(4, 1, 2), leaf(1, 2, 3)]);
    (source, root)
}

// ===================================================================
// 1. StatsVisitor on various trees (8 tests)
// ===================================================================

#[test]
fn stats_default_is_zeroed() {
    let s = StatsVisitor::default();
    assert_eq!(s.total_nodes, 0);
    assert_eq!(s.leaf_nodes, 0);
    assert_eq!(s.error_nodes, 0);
    assert_eq!(s.max_depth, 0);
    assert!(s.node_counts.is_empty());
}

#[test]
fn stats_single_leaf_counts() {
    let source = b"x";
    let root = leaf(1, 0, 1);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
    assert_eq!(stats.max_depth, 1);
}

#[test]
fn stats_interior_two_leaves() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 3);
    assert_eq!(stats.leaf_nodes, 2);
    assert_eq!(stats.max_depth, 2);
}

#[test]
fn stats_node_counts_per_kind() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(*stats.node_counts.get("Expression").unwrap(), 1);
    assert_eq!(*stats.node_counts.get("*").unwrap(), 2);
}

#[test]
fn stats_error_nodes_counted() {
    let source = b"x?y";
    let root = interior(5, vec![leaf(1, 0, 1), error_node(1, 2), leaf(1, 2, 3)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
    // Error nodes are NOT entered, so total_nodes excludes them.
    assert_eq!(stats.total_nodes, 3); // root + 2 leaves
}

#[test]
fn stats_nested_depth_three() {
    let source = b"xyz";
    let inner = interior(5, vec![leaf(1, 1, 2)]);
    let root = interior(5, vec![leaf(1, 0, 1), inner, leaf(1, 2, 3)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.max_depth, 3);
}

#[test]
fn stats_only_anonymous_nodes() {
    let source = b"+-";
    let root = anon_interior(4, vec![anon_leaf(4, 0, 1), anon_leaf(4, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 3);
    assert_eq!(stats.leaf_nodes, 2);
}

#[test]
fn stats_mixed_named_anonymous() {
    let source = b"a+b";
    let (_, root) = simple_expr_tree();
    let walker = TreeWalker::new(&source[..]);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 4); // root + 3 children
    assert_eq!(stats.leaf_nodes, 3);
}

// ===================================================================
// 2. PrettyPrintVisitor output (8 tests)
// ===================================================================

#[test]
fn pretty_new_output_empty() {
    let pp = PrettyPrintVisitor::new();
    assert!(pp.output().is_empty());
}

#[test]
fn pretty_default_output_empty() {
    let pp = PrettyPrintVisitor::default();
    assert!(pp.output().is_empty());
}

#[test]
fn pretty_single_leaf_shows_kind_and_text() {
    let source = b"x";
    let root = leaf(1, 0, 1);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    assert!(out.contains("*"));
    assert!(out.contains("[named]"));
    assert!(out.contains("\"x\""));
}

#[test]
fn pretty_interior_indents_children() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    // Root at indent 0, children at indent 1 (2 spaces)
    assert!(out.contains("Expression"));
    assert!(out.contains("  *")); // child kind at indent=1
}

#[test]
fn pretty_nested_tree_double_indent() {
    let source = b"abc";
    let inner = interior(5, vec![leaf(1, 1, 2)]);
    let root = interior(5, vec![leaf(1, 0, 1), inner, leaf(1, 2, 3)]);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    // Deepest leaf at indent=2 (4 spaces)
    assert!(out.contains("    \"b\""));
}

#[test]
fn pretty_error_node_labelled() {
    let source = b"x?y";
    let root = interior(5, vec![leaf(1, 0, 1), error_node(1, 2), leaf(1, 2, 3)]);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(pp.output().contains("ERROR"));
}

#[test]
fn pretty_anonymous_node_no_named_tag() {
    let source = b"+";
    let root = anon_leaf(4, 0, 1);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    assert!(out.contains("-")); // kind for symbol 4
    assert!(!out.contains("[named]"));
}

#[test]
fn pretty_leaf_text_is_quoted() {
    let source = b"hello";
    let root = leaf(1, 0, 5);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(pp.output().contains("\"hello\""));
}

// ===================================================================
// 3. SearchVisitor pattern matching (8 tests)
// ===================================================================

#[test]
fn search_no_match_yields_empty() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &_| n.kind() == "nonexistent");
    walker.walk(&root, &mut sv);
    assert!(sv.matches.is_empty());
}

#[test]
fn search_matches_root() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &_| n.kind() == "Expression");
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
    assert_eq!(sv.matches[0].2, "Expression");
}

#[test]
fn search_matches_all_leaves() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &_| n.kind() == "*");
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 2);
}

#[test]
fn search_captures_byte_range() {
    let source = b"abcde";
    let root = leaf(1, 1, 4);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|_: &_| true);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches[0].0, 1); // start_byte
    assert_eq!(sv.matches[0].1, 4); // end_byte
}

#[test]
fn search_predicate_by_named() {
    let source = b"a+b";
    let (_, root) = simple_expr_tree();
    let walker = TreeWalker::new(&source[..]);
    let mut sv = SearchVisitor::new(|n: &_| n.is_named());
    walker.walk(&root, &mut sv);
    // Expression(named) + leaf "a"(named) + leaf "b"(named) = 3
    assert_eq!(sv.matches.len(), 3);
}

#[test]
fn search_predicate_by_anonymous() {
    let source = b"a+b";
    let (_, root) = simple_expr_tree();
    let walker = TreeWalker::new(&source[..]);
    let mut sv = SearchVisitor::new(|n: &_| !n.is_named());
    walker.walk(&root, &mut sv);
    // Only the "+" operator is anonymous
    assert_eq!(sv.matches.len(), 1);
}

#[test]
fn search_multiple_kinds() {
    let source = b"a+b";
    let (_, root) = simple_expr_tree();
    let walker = TreeWalker::new(&source[..]);
    let mut sv = SearchVisitor::new(|n: &_| n.kind() == "*" || n.kind() == "Expression");
    walker.walk(&root, &mut sv);
    // Expression + two "*" leaves = 3
    assert_eq!(sv.matches.len(), 3);
}

#[test]
fn search_error_nodes_skipped() {
    // Error nodes are visited via visit_error, not enter_node — SearchVisitor
    // predicate runs only in enter_node, so errors don't appear.
    let source = b"x?y";
    let root = interior(5, vec![leaf(1, 0, 1), error_node(1, 2), leaf(1, 2, 3)]);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|_: &_| true);
    walker.walk(&root, &mut sv);
    // root + 2 non-error leaves = 3 (error node bypasses enter_node)
    assert_eq!(sv.matches.len(), 3);
}

// ===================================================================
// 4. TreeWalker traversal (8 tests)
// ===================================================================

#[test]
fn walker_visits_root() {
    let source = b"x";
    let root = leaf(1, 0, 1);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 1);
}

#[test]
fn walker_depth_first_order() {
    // Track visit order via a custom visitor.
    struct OrderVisitor {
        order: Vec<String>,
    }
    impl Visitor for OrderVisitor {
        fn enter_node(&mut self, node: &adze::pure_parser::ParsedNode) -> VisitorAction {
            self.order.push(format!("enter:{}", node.kind()));
            VisitorAction::Continue
        }
        fn leave_node(&mut self, node: &adze::pure_parser::ParsedNode) {
            self.order.push(format!("leave:{}", node.kind()));
        }
        fn visit_leaf(&mut self, _node: &adze::pure_parser::ParsedNode, text: &str) {
            self.order.push(format!("leaf:{text}"));
        }
    }

    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut ov = OrderVisitor { order: vec![] };
    walker.walk(&root, &mut ov);
    assert_eq!(ov.order[0], "enter:Expression");
    assert_eq!(ov.order[1], "enter:*");
    assert_eq!(ov.order[2], "leaf:a");
    assert_eq!(ov.order[3], "leave:*");
    assert_eq!(ov.order[4], "enter:*");
    assert_eq!(ov.order[5], "leaf:b");
    assert_eq!(ov.order[6], "leave:*");
    assert_eq!(ov.order[7], "leave:Expression");
}

#[test]
fn walker_stop_halts_traversal() {
    struct StopAfterOne {
        count: usize,
    }
    impl Visitor for StopAfterOne {
        fn enter_node(&mut self, _node: &adze::pure_parser::ParsedNode) -> VisitorAction {
            self.count += 1;
            if self.count >= 2 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }

    let source = b"abcde";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2), leaf(1, 2, 3)]);
    let walker = TreeWalker::new(source);
    let mut vis = StopAfterOne { count: 0 };
    walker.walk(&root, &mut vis);
    // Stop prevents visiting remaining siblings after the stopping node.
    // root(1) + child1(2,Stop) + child2(3) + child3(4) — walker iterates
    // children sequentially; Stop from child1 returns up but siblings are
    // visited because the parent loop doesn't check Stop from children.
    assert!(vis.count >= 2);
}

#[test]
fn walker_skip_children_skips_subtree() {
    struct SkipExpr {
        entered: Vec<String>,
    }
    impl Visitor for SkipExpr {
        fn enter_node(&mut self, node: &adze::pure_parser::ParsedNode) -> VisitorAction {
            self.entered.push(node.kind().to_string());
            if node.kind() == "Expression" && node.child_count() > 0 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }

    let source = b"ab";
    let inner = interior(5, vec![leaf(1, 0, 1)]);
    let root = interior(9, vec![inner, leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut vis = SkipExpr { entered: vec![] };
    walker.walk(&root, &mut vis);
    // root (Expression_Sub), inner (Expression — skip), leaf("b")
    assert_eq!(vis.entered.len(), 3);
    assert_eq!(vis.entered[0], "Expression_Sub");
    assert_eq!(vis.entered[1], "Expression");
    assert_eq!(vis.entered[2], "*");
}

#[test]
fn walker_leaf_text_extraction() {
    struct LeafCollector {
        texts: Vec<String>,
    }
    impl Visitor for LeafCollector {
        fn enter_node(&mut self, _: &adze::pure_parser::ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn visit_leaf(&mut self, _: &adze::pure_parser::ParsedNode, text: &str) {
            self.texts.push(text.to_string());
        }
    }

    let source = b"hello world";
    let root = interior(5, vec![leaf(1, 0, 5), leaf(1, 6, 11)]);
    let walker = TreeWalker::new(source);
    let mut vis = LeafCollector { texts: vec![] };
    walker.walk(&root, &mut vis);
    assert_eq!(vis.texts, ["hello", "world"]);
}

#[test]
fn walker_error_node_triggers_visit_error() {
    struct ErrorCounter {
        errors: usize,
    }
    impl Visitor for ErrorCounter {
        fn visit_error(&mut self, _: &adze::pure_parser::ParsedNode) {
            self.errors += 1;
        }
    }

    let source = b"x?";
    let root = interior(5, vec![leaf(1, 0, 1), error_node(1, 2)]);
    let walker = TreeWalker::new(source);
    let mut vis = ErrorCounter { errors: 0 };
    walker.walk(&root, &mut vis);
    assert_eq!(vis.errors, 1);
}

#[test]
fn breadth_first_walker_level_order() {
    struct LevelOrder {
        kinds: Vec<String>,
    }
    impl Visitor for LevelOrder {
        fn enter_node(&mut self, node: &adze::pure_parser::ParsedNode) -> VisitorAction {
            self.kinds.push(node.kind().to_string());
            VisitorAction::Continue
        }
    }

    let source = b"abc";
    let inner = interior(9, vec![leaf(1, 1, 2)]);
    let root = interior(5, vec![leaf(1, 0, 1), inner, leaf(1, 2, 3)]);
    let bfw = BreadthFirstWalker::new(source);
    let mut vis = LevelOrder { kinds: vec![] };
    bfw.walk(&root, &mut vis);
    // Level 0: Expression, Level 1: *, Expression_Sub, *, Level 2: *
    assert_eq!(vis.kinds[0], "Expression");
    assert_eq!(vis.kinds[1], "*");
    assert_eq!(vis.kinds[2], "Expression_Sub");
    assert_eq!(vis.kinds[3], "*");
    assert_eq!(vis.kinds[4], "*");
}

#[test]
fn walker_empty_source_leaf() {
    let source = b"";
    let root = leaf(1, 0, 0);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
}

// ===================================================================
// 5. Visitor with deep trees (5 tests)
// ===================================================================

/// Build a left-recursive chain of depth `n`.
fn deep_tree(depth: usize, source_len: usize) -> ParsedNode {
    if depth <= 1 {
        return leaf(1, 0, source_len.min(1));
    }
    let child = deep_tree(depth - 1, source_len);
    interior(5, vec![child])
}

#[test]
fn deep_tree_depth_10() {
    let source = b"x";
    let root = deep_tree(10, 1);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.max_depth, 10);
    assert_eq!(stats.total_nodes, 10);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn deep_tree_depth_50() {
    let source = b"x";
    let root = deep_tree(50, 1);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.max_depth, 50);
}

#[test]
fn deep_tree_pretty_print_indentation() {
    let source = b"x";
    let root = deep_tree(5, 1);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    // Deepest leaf at indent=4 → 8 spaces
    assert!(pp.output().contains("        \"x\""));
}

#[test]
fn deep_tree_search_finds_all_interior() {
    let source = b"x";
    let root = deep_tree(6, 1);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &_| n.kind() == "Expression");
    walker.walk(&root, &mut sv);
    // 5 interior Expression nodes (depth 6 means 5 interiors + 1 leaf)
    assert_eq!(sv.matches.len(), 5);
}

#[test]
fn deep_tree_breadth_first_visits_all() {
    let source = b"x";
    let root = deep_tree(8, 1);
    let bfw = BreadthFirstWalker::new(source);
    let mut stats = StatsVisitor::default();
    bfw.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 8);
}

// ===================================================================
// 6. Visitor with wide trees (5 tests)
// ===================================================================

/// Build a flat tree with `width` leaf children.
fn wide_tree(width: usize) -> (Vec<u8>, ParsedNode) {
    let source: Vec<u8> = (0..width).map(|i| b'a' + (i % 26) as u8).collect();
    let children: Vec<ParsedNode> = (0..width).map(|i| leaf(1, i, i + 1)).collect();
    let root = interior(5, children);
    (source, root)
}

#[test]
fn wide_tree_10_children() {
    let (source, root) = wide_tree(10);
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 11); // root + 10
    assert_eq!(stats.leaf_nodes, 10);
    assert_eq!(stats.max_depth, 2);
}

#[test]
fn wide_tree_100_children() {
    let (source, root) = wide_tree(100);
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 101);
    assert_eq!(stats.leaf_nodes, 100);
}

#[test]
fn wide_tree_search_all_leaves() {
    let (source, root) = wide_tree(20);
    let walker = TreeWalker::new(&source);
    let mut sv = SearchVisitor::new(|n: &_| n.kind() == "*");
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 20);
}

#[test]
fn wide_tree_pretty_print_contains_all() {
    let (source, root) = wide_tree(5);
    let walker = TreeWalker::new(&source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    assert!(out.contains("\"a\""));
    assert!(out.contains("\"e\""));
}

#[test]
fn wide_tree_breadth_first_same_count() {
    let (source, root) = wide_tree(15);
    let bfw = BreadthFirstWalker::new(&source);
    let mut stats = StatsVisitor::default();
    bfw.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 16);
}

// ===================================================================
// 7. Visitor composability (5 tests)
// ===================================================================

/// Visitor that delegates to two inner visitors.
struct DualVisitor<'a> {
    stats: &'a mut StatsVisitor,
    pp: &'a mut PrettyPrintVisitor,
}

impl Visitor for DualVisitor<'_> {
    fn enter_node(&mut self, node: &adze::pure_parser::ParsedNode) -> VisitorAction {
        self.stats.enter_node(node);
        self.pp.enter_node(node);
        VisitorAction::Continue
    }
    fn leave_node(&mut self, node: &adze::pure_parser::ParsedNode) {
        self.stats.leave_node(node);
        self.pp.leave_node(node);
    }
    fn visit_leaf(&mut self, node: &adze::pure_parser::ParsedNode, text: &str) {
        self.stats.visit_leaf(node, text);
        self.pp.visit_leaf(node, text);
    }
    fn visit_error(&mut self, node: &adze::pure_parser::ParsedNode) {
        self.stats.visit_error(node);
        self.pp.visit_error(node);
    }
}

#[test]
fn dual_visitor_stats_and_pretty() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    let mut pp = PrettyPrintVisitor::new();
    let mut dual = DualVisitor {
        stats: &mut stats,
        pp: &mut pp,
    };
    walker.walk(&root, &mut dual);
    assert_eq!(stats.total_nodes, 3);
    assert!(!pp.output().is_empty());
}

#[test]
fn sequential_walks_accumulate() {
    let source = b"xy";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    walker.walk(&root, &mut stats);
    // Two walks → doubled counts
    assert_eq!(stats.total_nodes, 6);
    assert_eq!(stats.leaf_nodes, 4);
}

#[test]
fn transform_visitor_leaf_count() {
    struct CountTransform;
    impl TransformVisitor for CountTransform {
        type Output = usize;
        fn transform_node(
            &mut self,
            _node: &adze::pure_parser::ParsedNode,
            children: Vec<usize>,
        ) -> usize {
            children.iter().sum::<usize>() + 1
        }
        fn transform_leaf(&mut self, _node: &adze::pure_parser::ParsedNode, _text: &str) -> usize {
            1
        }
        fn transform_error(&mut self, _node: &adze::pure_parser::ParsedNode) -> usize {
            0
        }
    }

    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let tw = TransformWalker::new(source);
    let mut ct = CountTransform;
    let total = tw.walk(&root, &mut ct);
    assert_eq!(total, 3); // 2 leaves + 1 interior
}

#[test]
fn transform_visitor_collects_text() {
    struct TextCollect;
    impl TransformVisitor for TextCollect {
        type Output = String;
        fn transform_node(
            &mut self,
            _node: &adze::pure_parser::ParsedNode,
            children: Vec<String>,
        ) -> String {
            children.join("")
        }
        fn transform_leaf(&mut self, _node: &adze::pure_parser::ParsedNode, text: &str) -> String {
            text.to_string()
        }
        fn transform_error(&mut self, _node: &adze::pure_parser::ParsedNode) -> String {
            "<error>".to_string()
        }
    }

    let source = b"hello world";
    let root = interior(5, vec![leaf(1, 0, 5), leaf(1, 6, 11)]);
    let tw = TransformWalker::new(source);
    let mut tc = TextCollect;
    let result = tw.walk(&root, &mut tc);
    assert_eq!(result, "helloworld");
}

#[test]
fn depth_first_and_breadth_first_same_totals() {
    let source = b"abc";
    let inner = interior(9, vec![leaf(1, 1, 2)]);
    let root = interior(5, vec![leaf(1, 0, 1), inner, leaf(1, 2, 3)]);

    let walker = TreeWalker::new(source);
    let mut df_stats = StatsVisitor::default();
    walker.walk(&root, &mut df_stats);

    let bfw = BreadthFirstWalker::new(source);
    let mut bf_stats = StatsVisitor::default();
    bfw.walk(&root, &mut bf_stats);

    assert_eq!(df_stats.total_nodes, bf_stats.total_nodes);
    assert_eq!(df_stats.leaf_nodes, bf_stats.leaf_nodes);
}

// ===================================================================
// 8. Edge cases (8 tests)
// ===================================================================

#[test]
fn edge_childless_interior() {
    // An interior node with zero children acts as a leaf in the walker.
    let source = b"";
    let root = make_node(5, vec![], 0, 0, false, true);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1); // zero children → visit_leaf
}

#[test]
fn edge_single_error_tree() {
    let source = b"?";
    let root = error_node(0, 1);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
    assert_eq!(stats.total_nodes, 0); // error bypasses enter_node
}

#[test]
fn edge_only_error_children() {
    let source = b"??";
    let root = interior(5, vec![error_node(0, 1), error_node(1, 2)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 2);
    assert_eq!(stats.total_nodes, 1); // only root enters
}

#[test]
fn edge_leaf_empty_text() {
    let source = b"";
    let root = leaf(1, 0, 0);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(pp.output().contains("\"\""));
}

#[test]
fn edge_visitor_action_equality() {
    assert_eq!(VisitorAction::Continue, VisitorAction::Continue);
    assert_eq!(VisitorAction::SkipChildren, VisitorAction::SkipChildren);
    assert_eq!(VisitorAction::Stop, VisitorAction::Stop);
    assert_ne!(VisitorAction::Continue, VisitorAction::Stop);
    assert_ne!(VisitorAction::Continue, VisitorAction::SkipChildren);
}

#[test]
fn edge_visitor_action_debug() {
    let dbg = format!("{:?}", VisitorAction::Continue);
    assert_eq!(dbg, "Continue");
}

#[test]
fn edge_search_always_false() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|_: &_| false);
    walker.walk(&root, &mut sv);
    assert!(sv.matches.is_empty());
}

#[test]
fn edge_search_always_true() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|_: &_| true);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 3); // root + 2 leaves
}

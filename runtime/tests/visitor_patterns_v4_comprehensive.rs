//! Comprehensive tests for the `adze::visitor` module (v4).
//!
//! Covers StatsVisitor, PrettyPrintVisitor, SearchVisitor, custom Visitor
//! implementations, composition patterns, edge cases, multiple traversals,
//! and cross-visitor consistency checks.

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

/// Named interior node whose byte span is derived from children.
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

// ===================================================================
// 1. StatsVisitor — Default construction
// ===================================================================

#[test]
fn stats_default_total_nodes_zero() {
    let s = StatsVisitor::default();
    assert_eq!(s.total_nodes, 0);
}

#[test]
fn stats_default_leaf_nodes_zero() {
    let s = StatsVisitor::default();
    assert_eq!(s.leaf_nodes, 0);
}

#[test]
fn stats_default_error_nodes_zero() {
    let s = StatsVisitor::default();
    assert_eq!(s.error_nodes, 0);
}

#[test]
fn stats_default_max_depth_zero() {
    let s = StatsVisitor::default();
    assert_eq!(s.max_depth, 0);
}

#[test]
fn stats_default_node_counts_empty() {
    let s = StatsVisitor::default();
    assert!(s.node_counts.is_empty());
}

// ===================================================================
// 1b. StatsVisitor — single leaf tree
// ===================================================================

#[test]
fn stats_single_leaf() {
    let source = b"x";
    let root = leaf(1, 0, 1); // kind = "*"
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
    assert_eq!(stats.max_depth, 1);
}

#[test]
fn stats_single_leaf_node_counts() {
    let source = b"x";
    let root = leaf(1, 0, 1);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(*stats.node_counts.get("*").unwrap(), 1);
}

// ===================================================================
// 1c. StatsVisitor — interior with children
// ===================================================================

#[test]
fn stats_interior_with_two_leaves() {
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
fn stats_interior_node_counts_keys() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(*stats.node_counts.get("Expression").unwrap(), 1);
    assert_eq!(*stats.node_counts.get("*").unwrap(), 1);
    assert_eq!(*stats.node_counts.get("_2").unwrap(), 1);
}

// ===================================================================
// 1d. StatsVisitor — deep tree (linear chain)
// ===================================================================

#[test]
fn stats_deep_chain_depth() {
    let source = b"x";
    // Build a chain: root -> child -> grandchild -> leaf
    let l = leaf(1, 0, 1);
    let n1 = interior(2, vec![l]);
    let n2 = interior(3, vec![n1]);
    let root = interior(5, vec![n2]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.max_depth, 4);
    assert_eq!(stats.total_nodes, 4);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn stats_very_deep_chain() {
    let source = b"x";
    let mut node = leaf(1, 0, 1);
    for i in 0..20 {
        node = interior((i % 5 + 2) as u16, vec![node]);
    }
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&node, &mut stats);
    assert_eq!(stats.max_depth, 21);
    assert_eq!(stats.total_nodes, 21);
    assert_eq!(stats.leaf_nodes, 1);
}

// ===================================================================
// 1e. StatsVisitor — wide tree
// ===================================================================

#[test]
fn stats_wide_tree() {
    let source = b"abcdefghij";
    let children: Vec<_> = (0..10).map(|i| leaf(1, i, i + 1)).collect();
    let root = interior(5, children);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 11);
    assert_eq!(stats.leaf_nodes, 10);
    assert_eq!(stats.max_depth, 2);
}

// ===================================================================
// 1f. StatsVisitor — mixed tree
// ===================================================================

#[test]
fn stats_mixed_tree() {
    // root(left(a, b), c)
    let source = b"abc";
    let a = leaf(1, 0, 1);
    let b = leaf(2, 1, 2);
    let left = interior(8, vec![a, b]);
    let c = leaf(3, 2, 3);
    let root = interior(5, vec![left, c]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 5);
    assert_eq!(stats.leaf_nodes, 3);
    assert_eq!(stats.max_depth, 3);
}

// ===================================================================
// 1g. StatsVisitor — tree with error nodes
// ===================================================================

#[test]
fn stats_error_node_counted() {
    let source = b"x!y";
    let a = leaf(1, 0, 1);
    let err = error_node(1, 2);
    let b = leaf(1, 2, 3);
    let root = interior(5, vec![a, err, b]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
    // Error nodes are not entered, so only root + 2 leaves visited
    assert_eq!(stats.total_nodes, 3);
    assert_eq!(stats.leaf_nodes, 2);
}

#[test]
fn stats_multiple_error_nodes() {
    let source = b"x!!y";
    let a = leaf(1, 0, 1);
    let e1 = error_node(1, 2);
    let e2 = error_node(2, 3);
    let b = leaf(1, 3, 4);
    let root = interior(5, vec![a, e1, e2, b]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 2);
}

// ===================================================================
// 1h. StatsVisitor — empty interior (no children)
// ===================================================================

#[test]
fn stats_empty_interior() {
    let source = b"";
    let root = interior(5, vec![]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    // An interior with 0 children is treated as a leaf by the walker
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
    assert_eq!(stats.max_depth, 1);
}

// ===================================================================
// 2. PrettyPrintVisitor — output formatting
// ===================================================================

#[test]
fn pp_new_is_empty() {
    let pp = PrettyPrintVisitor::new();
    assert_eq!(pp.output(), "");
}

#[test]
fn pp_default_is_empty() {
    let pp = PrettyPrintVisitor::default();
    assert_eq!(pp.output(), "");
}

#[test]
fn pp_single_leaf() {
    let source = b"x";
    let root = leaf(1, 0, 1);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    // Should contain the kind and the leaf text
    assert!(out.contains("*"), "Expected kind '*' in output: {out}");
    assert!(out.contains("\"x\""), "Expected leaf text in output: {out}");
}

#[test]
fn pp_named_annotation() {
    let source = b"x";
    let root = leaf(1, 0, 1); // is_named = true
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(pp.output().contains("[named]"));
}

#[test]
fn pp_unnamed_no_annotation() {
    let source = b"x";
    let root = anon_leaf(1, 0, 1); // is_named = false
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(!pp.output().contains("[named]"));
}

#[test]
fn pp_indentation_depth_one() {
    let source = b"x";
    let root = interior(5, vec![leaf(1, 0, 1)]);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let lines: Vec<&str> = pp.output().lines().collect();
    // Root at indent 0, child at indent 1, leaf text at indent 2
    assert!(
        lines[0].starts_with("Expression"),
        "Root line: {}",
        lines[0]
    );
    assert!(lines[1].starts_with("  *"), "Child line: {}", lines[1]);
}

#[test]
fn pp_indentation_depth_two() {
    let source = b"x";
    let inner = interior(8, vec![leaf(1, 0, 1)]);
    let root = interior(5, vec![inner]);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let lines: Vec<&str> = pp.output().lines().collect();
    assert!(lines[0].starts_with("Expression"));
    assert!(lines[1].starts_with("  Expression_Sub_1"));
    assert!(lines[2].starts_with("    *"));
}

#[test]
fn pp_error_node_format() {
    let source = b"x!y";
    let a = leaf(1, 0, 1);
    let err = error_node(1, 2);
    let b = leaf(1, 2, 3);
    let root = interior(5, vec![a, err, b]);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(
        pp.output().contains("ERROR:"),
        "Expected ERROR in: {}",
        pp.output()
    );
}

#[test]
fn pp_wide_tree_has_all_children() {
    let source = b"abcde";
    let children: Vec<_> = (0..5).map(|i| leaf(1, i, i + 1)).collect();
    let root = interior(5, children);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    // Each child produces a kind line + a leaf text line
    assert_eq!(out.matches("[named]").count(), 6); // root + 5 children
}

#[test]
fn pp_leaf_text_quoted() {
    let source = b"hello";
    let root = leaf(5, 0, 5);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(pp.output().contains("\"hello\""));
}

// ===================================================================
// 3. SearchVisitor — matching
// ===================================================================

#[test]
fn search_no_matches() {
    let source = b"x";
    let root = leaf(1, 0, 1); // kind = "*"
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &adze::pure_parser::ParsedNode| n.kind() == "Expression");
    walker.walk(&root, &mut sv);
    assert!(sv.matches.is_empty());
}

#[test]
fn search_single_match() {
    let source = b"x";
    let root = leaf(5, 0, 1); // kind = "Expression"
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &adze::pure_parser::ParsedNode| n.kind() == "Expression");
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
    assert_eq!(sv.matches[0].2, "Expression");
}

#[test]
fn search_match_byte_range() {
    let source = b"hello";
    let root = leaf(5, 0, 5);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &adze::pure_parser::ParsedNode| n.kind() == "Expression");
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches[0].0, 0); // start_byte
    assert_eq!(sv.matches[0].1, 5); // end_byte
}

#[test]
fn search_multiple_matches() {
    let source = b"abc";
    let a = leaf(1, 0, 1);
    let b = leaf(1, 1, 2);
    let c = leaf(1, 2, 3);
    let root = interior(5, vec![a, b, c]);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &adze::pure_parser::ParsedNode| n.kind() == "*");
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 3);
}

#[test]
fn search_nested_match() {
    let source = b"x";
    let inner = interior(5, vec![leaf(1, 0, 1)]);
    let root = interior(5, vec![inner]);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &adze::pure_parser::ParsedNode| n.kind() == "Expression");
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 2); // root + inner both match
}

#[test]
fn search_always_true() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|_: &adze::pure_parser::ParsedNode| true);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 3);
}

#[test]
fn search_always_false() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|_: &adze::pure_parser::ParsedNode| false);
    walker.walk(&root, &mut sv);
    assert!(sv.matches.is_empty());
}

#[test]
fn search_by_is_named() {
    let source = b"ab";
    let a = leaf(1, 0, 1); // named
    let b = anon_leaf(2, 1, 2); // anonymous
    let root = interior(5, vec![a, b]);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &adze::pure_parser::ParsedNode| !n.is_named());
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
}

#[test]
fn search_by_byte_range() {
    let source = b"abcd";
    let a = leaf(1, 0, 2);
    let b = leaf(2, 2, 4);
    let root = interior(5, vec![a, b]);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &adze::pure_parser::ParsedNode| n.start_byte() >= 2);
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
    assert_eq!(sv.matches[0].2, "_2");
}

#[test]
fn search_does_not_match_error_nodes() {
    // Error nodes call visit_error, not enter_node, so SearchVisitor won't see them
    let source = b"x!y";
    let a = leaf(1, 0, 1);
    let err = error_node(1, 2);
    let b = leaf(1, 2, 3);
    let root = interior(5, vec![a, err, b]);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|_: &adze::pure_parser::ParsedNode| true);
    walker.walk(&root, &mut sv);
    // root + a + b = 3 (error node skipped)
    assert_eq!(sv.matches.len(), 3);
}

// ===================================================================
// 4. Custom Visitor implementations
// ===================================================================

#[test]
fn custom_visitor_enter_leave_order() {
    struct OrderVisitor {
        log: Vec<String>,
    }
    impl Visitor for OrderVisitor {
        fn enter_node(&mut self, node: &adze::pure_parser::ParsedNode) -> VisitorAction {
            self.log.push(format!("enter:{}", node.kind()));
            VisitorAction::Continue
        }
        fn leave_node(&mut self, node: &adze::pure_parser::ParsedNode) {
            self.log.push(format!("leave:{}", node.kind()));
        }
        fn visit_leaf(&mut self, _node: &adze::pure_parser::ParsedNode, text: &str) {
            self.log.push(format!("leaf:{text}"));
        }
    }
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut v = OrderVisitor { log: vec![] };
    walker.walk(&root, &mut v);
    assert_eq!(v.log[0], "enter:Expression");
    assert_eq!(v.log[1], "enter:*");
    assert_eq!(v.log[2], "leaf:a");
    assert_eq!(v.log[3], "leave:*");
    assert_eq!(v.log[4], "enter:_2");
    assert_eq!(v.log[5], "leaf:b");
    assert_eq!(v.log[6], "leave:_2");
    assert_eq!(v.log[7], "leave:Expression");
}

#[test]
fn custom_visitor_stop_action() {
    struct CountThenStop {
        count: usize,
        limit: usize,
    }
    impl Visitor for CountThenStop {
        fn enter_node(&mut self, _node: &adze::pure_parser::ParsedNode) -> VisitorAction {
            self.count += 1;
            if self.count >= self.limit {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }
    // Tree: root -> inner -> leaf. Stop at limit=2 means root + inner entered,
    // then Stop returned so leaf is never reached.
    let source = b"x";
    let inner = interior(8, vec![leaf(1, 0, 1)]);
    let root = interior(5, vec![inner]);
    let walker = TreeWalker::new(source);
    let mut v = CountThenStop { count: 0, limit: 2 };
    walker.walk(&root, &mut v);
    assert_eq!(v.count, 2);
}

#[test]
fn custom_visitor_skip_children() {
    struct SkipDeep {
        total: usize,
    }
    impl Visitor for SkipDeep {
        fn enter_node(&mut self, _node: &adze::pure_parser::ParsedNode) -> VisitorAction {
            self.total += 1;
            if self.total == 1 {
                // Skip the root's children
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }
    let source = b"abc";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let walker = TreeWalker::new(source);
    let mut v = SkipDeep { total: 0 };
    walker.walk(&root, &mut v);
    assert_eq!(v.total, 1); // Only root was entered
}

#[test]
fn custom_visitor_text_collector() {
    struct TextCollector {
        texts: Vec<String>,
    }
    impl Visitor for TextCollector {
        fn enter_node(&mut self, _: &adze::pure_parser::ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn visit_leaf(&mut self, _: &adze::pure_parser::ParsedNode, text: &str) {
            self.texts.push(text.to_string());
        }
    }
    let source = b"hello world";
    let a = leaf(1, 0, 5);
    let b = leaf(2, 6, 11);
    let root = interior(5, vec![a, b]);
    let walker = TreeWalker::new(source);
    let mut v = TextCollector { texts: vec![] };
    walker.walk(&root, &mut v);
    assert_eq!(v.texts, vec!["hello", "world"]);
}

#[test]
fn custom_visitor_error_handler() {
    struct ErrorLogger {
        errors: Vec<(usize, usize)>,
    }
    impl Visitor for ErrorLogger {
        fn enter_node(&mut self, _: &adze::pure_parser::ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn visit_error(&mut self, node: &adze::pure_parser::ParsedNode) {
            self.errors.push((node.start_byte(), node.end_byte()));
        }
    }
    let source = b"a!b";
    let root = interior(5, vec![leaf(1, 0, 1), error_node(1, 2), leaf(1, 2, 3)]);
    let walker = TreeWalker::new(source);
    let mut v = ErrorLogger { errors: vec![] };
    walker.walk(&root, &mut v);
    assert_eq!(v.errors, vec![(1, 2)]);
}

// ===================================================================
// 5. Visitor composition patterns
// ===================================================================

#[test]
fn compose_stats_and_pretty_same_tree() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert_eq!(stats.total_nodes, 3);
    assert!(!pp.output().is_empty());
}

#[test]
fn compose_stats_and_search_same_tree() {
    let source = b"abc";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2), leaf(1, 2, 3)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    let mut search = SearchVisitor::new(|n: &adze::pure_parser::ParsedNode| n.kind() == "*");
    walker.walk(&root, &mut search);
    assert_eq!(search.matches.len(), 3);
    assert_eq!(*stats.node_counts.get("*").unwrap(), 3);
}

#[test]
fn compose_depth_first_and_breadth_first() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let dfs = TreeWalker::new(source);
    let bfs = BreadthFirstWalker::new(source);
    let mut stats_dfs = StatsVisitor::default();
    let mut stats_bfs = StatsVisitor::default();
    dfs.walk(&root, &mut stats_dfs);
    bfs.walk(&root, &mut stats_bfs);
    assert_eq!(stats_dfs.total_nodes, stats_bfs.total_nodes);
    assert_eq!(stats_dfs.leaf_nodes, stats_bfs.leaf_nodes);
}

// ===================================================================
// 6. Edge cases
// ===================================================================

#[test]
fn edge_deeply_nested_20_levels() {
    let source = b"x";
    let mut node = leaf(1, 0, 1);
    for _ in 0..19 {
        node = interior(5, vec![node]);
    }
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&node, &mut stats);
    assert_eq!(stats.max_depth, 20);
}

#[test]
fn edge_wide_50_children() {
    let source: Vec<u8> = (0..50).map(|_| b'x').collect();
    let children: Vec<_> = (0..50).map(|i| leaf(1, i, i + 1)).collect();
    let root = interior(5, children);
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 51);
    assert_eq!(stats.leaf_nodes, 50);
}

#[test]
fn edge_error_only_tree() {
    let source = b"!";
    let root = error_node(0, 1);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
    assert_eq!(stats.total_nodes, 0); // error nodes are not entered
}

#[test]
fn edge_zero_length_leaf() {
    let source = b"abc";
    let root = leaf(1, 2, 2); // zero-length span
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn edge_anonymous_interior() {
    let source = b"x";
    let root = anon_interior(5, vec![anon_leaf(1, 0, 1)]);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(!pp.output().contains("[named]"));
}

#[test]
fn edge_mixed_named_anonymous() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), anon_leaf(2, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    // Root and first child are named, second child is not
    let named_count = out.matches("[named]").count();
    assert_eq!(named_count, 2); // root + first child
}

#[test]
fn edge_interior_no_children_is_leaf() {
    let source = b"";
    let root = interior(5, vec![]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn edge_error_between_interior_nodes() {
    let source = b"a!b";
    let left = interior(8, vec![leaf(1, 0, 1)]);
    let err = error_node(1, 2);
    let right = interior(9, vec![leaf(1, 2, 3)]);
    let root = interior(5, vec![left, err, right]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
    assert_eq!(stats.total_nodes, 5); // root + left + a + right + b
    assert_eq!(stats.leaf_nodes, 2);
}

// ===================================================================
// 7. Visitor state after multiple traversals
// ===================================================================

#[test]
fn stats_accumulates_across_walks() {
    let source = b"xy";
    let tree1 = interior(5, vec![leaf(1, 0, 1)]);
    let tree2 = interior(5, vec![leaf(2, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&tree1, &mut stats);
    walker.walk(&tree2, &mut stats);
    assert_eq!(stats.total_nodes, 4); // 2 roots + 2 leaves
    assert_eq!(stats.leaf_nodes, 2);
}

#[test]
fn stats_max_depth_tracks_deepest_across_walks() {
    let source = b"x";
    let shallow = leaf(1, 0, 1);
    let deep = interior(5, vec![interior(8, vec![leaf(1, 0, 1)])]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&shallow, &mut stats);
    assert_eq!(stats.max_depth, 1);
    walker.walk(&deep, &mut stats);
    assert_eq!(stats.max_depth, 3);
}

#[test]
fn pp_accumulates_across_walks() {
    let source = b"ab";
    let t1 = leaf(1, 0, 1);
    let t2 = leaf(2, 1, 2);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&t1, &mut pp);
    let len1 = pp.output().len();
    walker.walk(&t2, &mut pp);
    assert!(pp.output().len() > len1);
}

#[test]
fn search_accumulates_across_walks() {
    let source = b"ab";
    let t1 = leaf(1, 0, 1);
    let t2 = leaf(1, 1, 2);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &adze::pure_parser::ParsedNode| n.kind() == "*");
    walker.walk(&t1, &mut sv);
    assert_eq!(sv.matches.len(), 1);
    walker.walk(&t2, &mut sv);
    assert_eq!(sv.matches.len(), 2);
}

// ===================================================================
// 8. Cross-visitor consistency
// ===================================================================

#[test]
fn cross_stats_total_matches_search_all() {
    let source = b"abc";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    let mut search = SearchVisitor::new(|_: &adze::pure_parser::ParsedNode| true);
    walker.walk(&root, &mut search);
    assert_eq!(stats.total_nodes, search.matches.len());
}

#[test]
fn cross_stats_with_errors_consistency() {
    let source = b"a!b";
    let root = interior(5, vec![leaf(1, 0, 1), error_node(1, 2), leaf(1, 2, 3)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    let mut search_all = SearchVisitor::new(|_: &adze::pure_parser::ParsedNode| true);
    walker.walk(&root, &mut search_all);
    // search only sees entered nodes (not errors)
    assert_eq!(stats.total_nodes, search_all.matches.len());
    assert_eq!(stats.error_nodes, 1);
}

#[test]
fn cross_leaf_count_matches_search_for_childless() {
    let source = b"abcde";
    let children: Vec<_> = (0..5).map(|i| leaf(1, i, i + 1)).collect();
    let root = interior(5, children);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    let mut search_leaves =
        SearchVisitor::new(|n: &adze::pure_parser::ParsedNode| n.child_count() == 0);
    walker.walk(&root, &mut search_leaves);
    assert_eq!(stats.leaf_nodes, search_leaves.matches.len());
}

#[test]
fn cross_node_counts_sum_equals_total() {
    let source = b"abc";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    let sum: usize = stats.node_counts.values().sum();
    assert_eq!(sum, stats.total_nodes);
}

// ===================================================================
// 9. BreadthFirstWalker tests
// ===================================================================

#[test]
fn bfs_single_leaf() {
    let source = b"x";
    let root = leaf(1, 0, 1);
    let walker = BreadthFirstWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn bfs_interior_with_children() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let walker = BreadthFirstWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 3);
}

#[test]
fn bfs_error_node() {
    let source = b"!";
    let root = error_node(0, 1);
    let walker = BreadthFirstWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
    assert_eq!(stats.total_nodes, 0);
}

#[test]
fn bfs_stop_action_respected() {
    struct StopAfter1;
    impl Visitor for StopAfter1 {
        fn enter_node(&mut self, _: &adze::pure_parser::ParsedNode) -> VisitorAction {
            VisitorAction::Stop
        }
    }
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let walker = BreadthFirstWalker::new(source);
    let mut v = StopAfter1;
    walker.walk(&root, &mut v);
    // Should not panic — just stops
}

#[test]
fn bfs_skip_children_respected() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let walker = BreadthFirstWalker::new(source);
    let stats = StatsVisitor::default();
    // Override stats behavior: we'll use a custom visitor
    struct SkipRoot {
        count: usize,
    }
    impl Visitor for SkipRoot {
        fn enter_node(&mut self, _: &adze::pure_parser::ParsedNode) -> VisitorAction {
            self.count += 1;
            if self.count == 1 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }
    let mut v = SkipRoot { count: 0 };
    walker.walk(&root, &mut v);
    assert_eq!(v.count, 1); // Only root entered, children skipped
    let _ = stats; // suppress unused warning
}

// ===================================================================
// 10. TransformVisitor / TransformWalker tests
// ===================================================================

#[test]
fn transform_leaf_to_string() {
    struct ToStr;
    impl TransformVisitor for ToStr {
        type Output = String;
        fn transform_node(
            &mut self,
            _: &adze::pure_parser::ParsedNode,
            children: Vec<String>,
        ) -> String {
            format!("({})", children.join(" "))
        }
        fn transform_leaf(&mut self, _: &adze::pure_parser::ParsedNode, text: &str) -> String {
            text.to_string()
        }
        fn transform_error(&mut self, _: &adze::pure_parser::ParsedNode) -> String {
            "ERROR".to_string()
        }
    }
    let source = b"x";
    let root = leaf(1, 0, 1);
    let walker = TransformWalker::new(source);
    let result = walker.walk(&root, &mut ToStr);
    assert_eq!(result, "x");
}

#[test]
fn transform_interior_parenthesized() {
    struct ToStr;
    impl TransformVisitor for ToStr {
        type Output = String;
        fn transform_node(
            &mut self,
            _: &adze::pure_parser::ParsedNode,
            children: Vec<String>,
        ) -> String {
            format!("({})", children.join(" "))
        }
        fn transform_leaf(&mut self, _: &adze::pure_parser::ParsedNode, text: &str) -> String {
            text.to_string()
        }
        fn transform_error(&mut self, _: &adze::pure_parser::ParsedNode) -> String {
            "ERROR".to_string()
        }
    }
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let walker = TransformWalker::new(source);
    let result = walker.walk(&root, &mut ToStr);
    assert_eq!(result, "(a b)");
}

#[test]
fn transform_error_node() {
    struct ToStr;
    impl TransformVisitor for ToStr {
        type Output = String;
        fn transform_node(
            &mut self,
            _: &adze::pure_parser::ParsedNode,
            children: Vec<String>,
        ) -> String {
            children.join("")
        }
        fn transform_leaf(&mut self, _: &adze::pure_parser::ParsedNode, text: &str) -> String {
            text.to_string()
        }
        fn transform_error(&mut self, _: &adze::pure_parser::ParsedNode) -> String {
            "ERR".to_string()
        }
    }
    let source = b"!";
    let root = error_node(0, 1);
    let walker = TransformWalker::new(source);
    let result = walker.walk(&root, &mut ToStr);
    assert_eq!(result, "ERR");
}

#[test]
fn transform_count_nodes() {
    struct Counter;
    impl TransformVisitor for Counter {
        type Output = usize;
        fn transform_node(
            &mut self,
            _: &adze::pure_parser::ParsedNode,
            children: Vec<usize>,
        ) -> usize {
            1 + children.iter().sum::<usize>()
        }
        fn transform_leaf(&mut self, _: &adze::pure_parser::ParsedNode, _: &str) -> usize {
            1
        }
        fn transform_error(&mut self, _: &adze::pure_parser::ParsedNode) -> usize {
            1
        }
    }
    let source = b"abc";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let walker = TransformWalker::new(source);
    let count = walker.walk(&root, &mut Counter);
    assert_eq!(count, 4);
}

// ===================================================================
// 11. VisitorAction enum tests
// ===================================================================

#[test]
fn visitor_action_debug() {
    let s = format!("{:?}", VisitorAction::Continue);
    assert_eq!(s, "Continue");
}

#[test]
fn visitor_action_clone() {
    let a = VisitorAction::Stop;
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn visitor_action_all_variants_distinct() {
    assert_ne!(VisitorAction::Continue, VisitorAction::SkipChildren);
    assert_ne!(VisitorAction::Continue, VisitorAction::Stop);
    assert_ne!(VisitorAction::SkipChildren, VisitorAction::Stop);
}

// ===================================================================
// 12. Additional coverage
// ===================================================================

#[test]
fn pp_deep_indentation() {
    let source = b"x";
    let mut node = leaf(1, 0, 1);
    for _ in 0..5 {
        node = interior(5, vec![node]);
    }
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&node, &mut pp);
    // The leaf text should be indented 6 levels (5 wrappers + 1 leaf itself)
    assert!(pp.output().contains("            \"x\"")); // 12 spaces = 6 * 2
}

#[test]
fn stats_different_kinds_counted_separately() {
    let source = b"abc";
    let root = interior(
        5,
        vec![
            leaf(1, 0, 1), // kind = "*"
            leaf(2, 1, 2), // kind = "_2"
            leaf(3, 2, 3), // kind = "_6"
        ],
    );
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.node_counts.len(), 4); // Expression, *, _2, _6
}

#[test]
fn search_preserves_order() {
    let source = b"abcde";
    let children: Vec<_> = (0..5).map(|i| leaf(1, i, i + 1)).collect();
    let root = interior(5, children);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &adze::pure_parser::ParsedNode| n.kind() == "*");
    walker.walk(&root, &mut sv);
    for i in 0..5 {
        assert_eq!(sv.matches[i].0, i); // start_byte
        assert_eq!(sv.matches[i].1, i + 1); // end_byte
    }
}

#[test]
fn bfs_pretty_print() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let walker = BreadthFirstWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    // BFS visits root first, then children — all produce output
    assert!(!pp.output().is_empty());
}

#[test]
fn stats_node_counts_accumulate_duplicates() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(*stats.node_counts.get("*").unwrap(), 2);
}

#[test]
fn search_in_deep_tree_finds_all_matching() {
    let source = b"x";
    let l = leaf(1, 0, 1);
    let n1 = interior(5, vec![l]);
    let n2 = interior(5, vec![n1]);
    let root = interior(5, vec![n2]);
    let walker = TreeWalker::new(source);
    let mut sv = SearchVisitor::new(|n: &adze::pure_parser::ParsedNode| n.kind() == "Expression");
    walker.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 3); // root, n2, n1
}

#[test]
fn transform_nested_tree() {
    struct Depth;
    impl TransformVisitor for Depth {
        type Output = usize;
        fn transform_node(
            &mut self,
            _: &adze::pure_parser::ParsedNode,
            children: Vec<usize>,
        ) -> usize {
            1 + children.iter().copied().max().unwrap_or(0)
        }
        fn transform_leaf(&mut self, _: &adze::pure_parser::ParsedNode, _: &str) -> usize {
            1
        }
        fn transform_error(&mut self, _: &adze::pure_parser::ParsedNode) -> usize {
            0
        }
    }
    let source = b"x";
    let root = interior(5, vec![interior(8, vec![leaf(1, 0, 1)])]);
    let walker = TransformWalker::new(source);
    let depth = walker.walk(&root, &mut Depth);
    assert_eq!(depth, 3);
}

#[test]
fn bfs_and_dfs_stats_match() {
    let source = b"abcd";
    let root = interior(
        5,
        vec![
            interior(8, vec![leaf(1, 0, 1), leaf(2, 1, 2)]),
            interior(9, vec![leaf(3, 2, 3), leaf(4, 3, 4)]),
        ],
    );
    let mut dfs_stats = StatsVisitor::default();
    let mut bfs_stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&root, &mut dfs_stats);
    BreadthFirstWalker::new(source).walk(&root, &mut bfs_stats);
    assert_eq!(dfs_stats.total_nodes, bfs_stats.total_nodes);
    assert_eq!(dfs_stats.leaf_nodes, bfs_stats.leaf_nodes);
    assert_eq!(dfs_stats.error_nodes, bfs_stats.error_nodes);
}

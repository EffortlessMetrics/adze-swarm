//! Comprehensive tests for TreeWalker with various visitor combinations (v4).
//!
//! Covers StatsVisitor, PrettyPrintVisitor, SearchVisitor, sequential walks,
//! multiple walks with the same visitor, complex tree structures, into_visitor
//! equivalents, and edge cases.

use adze::pure_parser::ParsedNode;
use adze::visitor::{
    PrettyPrintVisitor, SearchVisitor, StatsVisitor, TreeWalker, Visitor, VisitorAction,
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
//   8 => "Expression_Sub_1", 9 => "Expression_Sub", 10 => "rule_10"

/// Build a simple `a+b` tree: Expression( leaf("a"), leaf("+"), leaf("b") )
fn simple_expr_tree() -> (Vec<u8>, ParsedNode) {
    let source = b"a+b".to_vec();
    let root = interior(5, vec![leaf(1, 0, 1), anon_leaf(4, 1, 2), leaf(1, 2, 3)]);
    (source, root)
}

/// Build a nested tree: Expr( Expr( leaf, leaf ), leaf )
fn nested_expr_tree() -> (Vec<u8>, ParsedNode) {
    let source = b"abcde".to_vec();
    let inner = interior(5, vec![leaf(1, 0, 2), leaf(1, 2, 3)]);
    let root = interior(5, vec![inner, leaf(1, 3, 5)]);
    (source, root)
}

/// Build a deep chain: Expr( Expr( Expr( leaf ) ) )
fn deep_chain_tree() -> (Vec<u8>, ParsedNode) {
    let source = b"x".to_vec();
    let l = leaf(1, 0, 1);
    let d1 = interior(5, vec![l]);
    let d2 = interior(5, vec![d1]);
    let root = interior(5, vec![d2]);
    (source, root)
}

/// Build a wide tree with many children.
fn wide_tree(width: usize) -> (Vec<u8>, ParsedNode) {
    let source: Vec<u8> = (0..width).map(|i| b'a' + (i % 26) as u8).collect();
    let children: Vec<ParsedNode> = (0..width).map(|i| leaf(1, i, i + 1)).collect();
    let root = interior(5, children);
    (source, root)
}

// ===================================================================
// 1. StatsVisitor + TreeWalker (8 tests)
// ===================================================================

#[test]
fn stats_walker_single_leaf() {
    let source = b"x";
    let root = leaf(1, 0, 1);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
    assert_eq!(stats.max_depth, 1);
    assert_eq!(stats.error_nodes, 0);
}

#[test]
fn stats_walker_simple_expr() {
    let (source, root) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 4);
    assert_eq!(stats.leaf_nodes, 3);
    assert_eq!(stats.max_depth, 2);
}

#[test]
fn stats_walker_nested_depth() {
    let (source, root) = nested_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.max_depth, 3);
    assert_eq!(stats.total_nodes, 5);
    assert_eq!(stats.leaf_nodes, 3);
}

#[test]
fn stats_walker_error_nodes_not_entered() {
    let source = b"a?b";
    let root = interior(5, vec![leaf(1, 0, 1), error_node(1, 2), leaf(1, 2, 3)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
    // Error nodes are not entered, so total_nodes excludes them.
    assert_eq!(stats.total_nodes, 3);
}

#[test]
fn stats_walker_deep_chain_depth() {
    let (source, root) = deep_chain_tree();
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.max_depth, 4);
    assert_eq!(stats.total_nodes, 4);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn stats_walker_per_kind_counts() {
    let (source, root) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(*stats.node_counts.get("Expression").unwrap(), 1);
    assert_eq!(*stats.node_counts.get("*").unwrap(), 2);
    assert_eq!(*stats.node_counts.get("-").unwrap(), 1);
}

#[test]
fn stats_walker_wide_tree_counts() {
    let (source, root) = wide_tree(10);
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 11);
    assert_eq!(stats.leaf_nodes, 10);
    assert_eq!(stats.max_depth, 2);
}

#[test]
fn stats_walker_all_anonymous_nodes() {
    let source = b"--";
    let root = anon_interior(4, vec![anon_leaf(4, 0, 1), anon_leaf(4, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 3);
    assert_eq!(stats.leaf_nodes, 2);
    assert_eq!(*stats.node_counts.get("-").unwrap(), 3);
}

// ===================================================================
// 2. PrettyPrintVisitor + TreeWalker (8 tests)
// ===================================================================

#[test]
fn pretty_walker_single_leaf() {
    let source = b"x";
    let root = leaf(1, 0, 1);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(pp.output().contains("* [named]"));
    assert!(pp.output().contains("\"x\""));
}

#[test]
fn pretty_walker_simple_expr_structure() {
    let (source, root) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    assert!(out.contains("Expression [named]"));
    assert!(out.contains("\"a\""));
    assert!(out.contains("\"+\""));
    assert!(out.contains("\"b\""));
}

#[test]
fn pretty_walker_indentation_increases() {
    let (source, root) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let lines: Vec<&str> = pp.output().lines().collect();
    // Root at indent 0, children at indent 1
    assert!(!lines[0].starts_with("  "));
    assert!(lines[1].starts_with("  "));
}

#[test]
fn pretty_walker_anonymous_no_named_marker() {
    let source = b"-";
    let root = anon_leaf(4, 0, 1);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    assert!(!out.contains("[named]"));
}

#[test]
fn pretty_walker_nested_indentation() {
    let (source, root) = nested_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    // Depth-3 leaf should have 4-space indent
    assert!(out.contains("    \""));
}

#[test]
fn pretty_walker_error_node_label() {
    let source = b"a?b";
    let root = interior(5, vec![leaf(1, 0, 1), error_node(1, 2), leaf(1, 2, 3)]);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(pp.output().contains("ERROR"));
}

#[test]
fn pretty_walker_empty_output_before_walk() {
    let pp = PrettyPrintVisitor::default();
    assert!(pp.output().is_empty());
}

#[test]
fn pretty_walker_deep_chain_many_indents() {
    let (source, root) = deep_chain_tree();
    let walker = TreeWalker::new(&source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    // Deepest leaf text should be at 6 spaces (3 levels of nesting)
    assert!(out.contains("      \"x\""));
}

// ===================================================================
// 3. SearchVisitor + TreeWalker (8 tests)
// ===================================================================

#[test]
fn search_walker_find_expression_nodes() {
    let (source, root) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut search = SearchVisitor::new(|node| node.kind() == "Expression");
    walker.walk(&root, &mut search);
    assert_eq!(search.matches.len(), 1);
    assert_eq!(search.matches[0].2, "Expression");
}

#[test]
fn search_walker_find_all_star_leaves() {
    let (source, root) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut search = SearchVisitor::new(|node| node.kind() == "*");
    walker.walk(&root, &mut search);
    assert_eq!(search.matches.len(), 2);
}

#[test]
fn search_walker_no_matches() {
    let (source, root) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut search = SearchVisitor::new(|node| node.kind() == "nonexistent");
    walker.walk(&root, &mut search);
    assert!(search.matches.is_empty());
}

#[test]
fn search_walker_match_named_only() {
    let (source, root) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut search = SearchVisitor::new(|node| node.is_named());
    walker.walk(&root, &mut search);
    // Expression + 2 named leaves ("a" and "b"); "-" is anonymous
    assert_eq!(search.matches.len(), 3);
}

#[test]
fn search_walker_byte_range_correctness() {
    let (source, root) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut search = SearchVisitor::new(|node| node.kind() == "Expression");
    walker.walk(&root, &mut search);
    assert_eq!(search.matches[0].0, 0); // start_byte
    assert_eq!(search.matches[0].1, 3); // end_byte
}

#[test]
fn search_walker_nested_finds_all() {
    let (source, root) = nested_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut search = SearchVisitor::new(|node| node.kind() == "Expression");
    walker.walk(&root, &mut search);
    assert_eq!(search.matches.len(), 2);
}

#[test]
fn search_walker_predicate_by_byte_range() {
    let source = b"abcde";
    let root = interior(5, vec![leaf(1, 0, 2), leaf(1, 2, 3), leaf(1, 3, 5)]);
    let walker = TreeWalker::new(source);
    let mut search = SearchVisitor::new(|node| node.start_byte() >= 2);
    walker.walk(&root, &mut search);
    assert_eq!(search.matches.len(), 2);
}

#[test]
fn search_walker_error_nodes_skipped() {
    let source = b"a?b";
    let root = interior(5, vec![leaf(1, 0, 1), error_node(1, 2), leaf(1, 2, 3)]);
    let walker = TreeWalker::new(source);
    let mut search = SearchVisitor::new(|_| true);
    walker.walk(&root, &mut search);
    // Error nodes trigger visit_error, not enter_node — so not matched.
    assert_eq!(search.matches.len(), 3); // root + 2 non-error leaves
}

// ===================================================================
// 4. Sequential walks: same tree, different visitors (5 tests)
// ===================================================================

#[test]
fn sequential_stats_then_pretty() {
    let (source, root) = simple_expr_tree();
    let walker = TreeWalker::new(&source);

    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 4);

    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(pp.output().contains("Expression"));
}

#[test]
fn sequential_pretty_then_search() {
    let (source, root) = simple_expr_tree();
    let walker = TreeWalker::new(&source);

    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(!pp.output().is_empty());

    let mut search = SearchVisitor::new(|node| node.kind() == "*");
    walker.walk(&root, &mut search);
    assert_eq!(search.matches.len(), 2);
}

#[test]
fn sequential_search_then_stats() {
    let (source, root) = nested_expr_tree();
    let walker = TreeWalker::new(&source);

    let mut search = SearchVisitor::new(|node| node.kind() == "Expression");
    walker.walk(&root, &mut search);
    assert_eq!(search.matches.len(), 2);

    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.max_depth, 3);
}

#[test]
fn sequential_all_three_visitors() {
    let (source, root) = simple_expr_tree();
    let walker = TreeWalker::new(&source);

    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);

    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);

    let mut search = SearchVisitor::new(|node| node.is_named());
    walker.walk(&root, &mut search);

    assert_eq!(stats.total_nodes, 4);
    assert!(pp.output().contains("Expression"));
    assert_eq!(search.matches.len(), 3);
}

#[test]
fn sequential_two_different_searches() {
    let (source, root) = simple_expr_tree();
    let walker = TreeWalker::new(&source);

    let mut s1 = SearchVisitor::new(|node| node.kind() == "Expression");
    walker.walk(&root, &mut s1);

    let mut s2 = SearchVisitor::new(|node| node.kind() == "-");
    walker.walk(&root, &mut s2);

    assert_eq!(s1.matches.len(), 1);
    assert_eq!(s2.matches.len(), 1);
}

// ===================================================================
// 5. Multiple walks with the same visitor (5 tests)
// ===================================================================

#[test]
fn multi_walk_stats_accumulates() {
    let source = b"xy";
    let tree1 = interior(5, vec![leaf(1, 0, 1)]);
    let tree2 = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&tree1, &mut stats);
    walker.walk(&tree2, &mut stats);
    assert_eq!(stats.total_nodes, 5); // 2 + 3
    assert_eq!(stats.leaf_nodes, 3); // 1 + 2
}

#[test]
fn multi_walk_pretty_appends() {
    let source = b"ab";
    let tree1 = leaf(1, 0, 1);
    let tree2 = leaf(1, 1, 2);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&tree1, &mut pp);
    walker.walk(&tree2, &mut pp);
    let lines: Vec<&str> = pp.output().lines().collect();
    assert!(lines.len() >= 4); // two walks produce at least 4 lines
}

#[test]
fn multi_walk_search_accumulates_matches() {
    let source = b"abcd";
    let tree1 = interior(5, vec![leaf(1, 0, 2)]);
    let tree2 = interior(5, vec![leaf(1, 2, 4)]);
    let walker = TreeWalker::new(source);
    let mut search = SearchVisitor::new(|node| node.kind() == "*");
    walker.walk(&tree1, &mut search);
    walker.walk(&tree2, &mut search);
    assert_eq!(search.matches.len(), 2);
}

#[test]
fn multi_walk_stats_max_depth_tracks_maximum() {
    let source = b"xyz";
    let shallow = interior(5, vec![leaf(1, 0, 1)]);
    let deep = interior(5, vec![interior(5, vec![interior(5, vec![leaf(1, 0, 1)])])]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&shallow, &mut stats);
    assert_eq!(stats.max_depth, 2);
    walker.walk(&deep, &mut stats);
    assert_eq!(stats.max_depth, 4);
}

#[test]
fn multi_walk_error_counts_accumulate() {
    let source = b"a?b!";
    let tree1 = interior(5, vec![error_node(0, 1)]);
    let tree2 = interior(5, vec![error_node(2, 3), error_node(3, 4)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&tree1, &mut stats);
    walker.walk(&tree2, &mut stats);
    assert_eq!(stats.error_nodes, 3);
}

// ===================================================================
// 6. Complex tree structures (8 tests)
// ===================================================================

#[test]
fn complex_balanced_binary_tree() {
    // Build a balanced binary tree of depth 3
    let source = b"abcd";
    let ll = leaf(1, 0, 1);
    let lr = leaf(1, 1, 2);
    let rl = leaf(1, 2, 3);
    let rr = leaf(1, 3, 4);
    let left = interior(5, vec![ll, lr]);
    let right = interior(5, vec![rl, rr]);
    let root = interior(5, vec![left, right]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 7);
    assert_eq!(stats.leaf_nodes, 4);
    assert_eq!(stats.max_depth, 3);
}

#[test]
fn complex_mixed_error_and_normal() {
    let source = b"a?b+c";
    let root = interior(
        5,
        vec![
            leaf(1, 0, 1),
            error_node(1, 2),
            leaf(1, 2, 3),
            anon_leaf(4, 3, 4),
            leaf(1, 4, 5),
        ],
    );
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
    assert_eq!(stats.total_nodes, 4 + 1); // root + 3 normal leaves + 1 anon leaf
    assert_eq!(stats.leaf_nodes, 4);
}

#[test]
fn complex_deeply_nested_left_spine() {
    let source = b"x";
    let mut current = leaf(1, 0, 1);
    for _ in 0..10 {
        current = interior(5, vec![current]);
    }
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&current, &mut stats);
    assert_eq!(stats.max_depth, 11);
    assert_eq!(stats.total_nodes, 11);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn complex_wide_50_children() {
    let (source, root) = wide_tree(50);
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 51);
    assert_eq!(stats.leaf_nodes, 50);
    assert_eq!(stats.max_depth, 2);
}

#[test]
fn complex_heterogeneous_symbols() {
    let source = b"abcdef";
    let root = interior(
        5,
        vec![
            leaf(1, 0, 1),  // "*"
            leaf(2, 1, 2),  // "_2"
            leaf(3, 2, 3),  // "_6"
            leaf(4, 3, 4),  // "-"
            leaf(7, 4, 5),  // "Whitespace"
            leaf(10, 5, 6), // "rule_10"
        ],
    );
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.node_counts.len(), 7); // 6 distinct leaf kinds + 1 root
}

#[test]
fn complex_search_nested_only_leaves() {
    let (source, root) = nested_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut search = SearchVisitor::new(|node| node.child_count() == 0);
    walker.walk(&root, &mut search);
    assert_eq!(search.matches.len(), 3);
}

#[test]
fn complex_pretty_nested_with_mixed_named() {
    let source = b"a-b";
    let root = interior(
        5,
        vec![
            leaf(1, 0, 1),
            anon_interior(4, vec![anon_leaf(4, 1, 2)]),
            leaf(1, 2, 3),
        ],
    );
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    // Root is named, inner anon_interior is not
    assert!(out.contains("Expression [named]"));
    assert!(out.contains("  -\n")); // anonymous child without [named]
}

#[test]
fn complex_search_and_stats_agree_on_node_count() {
    let (source, root) = nested_expr_tree();
    let walker = TreeWalker::new(&source);

    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);

    let mut search = SearchVisitor::new(|_| true);
    walker.walk(&root, &mut search);

    assert_eq!(stats.total_nodes, search.matches.len());
}

// ===================================================================
// 7. Walk + into_visitor equivalent (visitor consumption) (5 tests)
// ===================================================================

#[test]
fn consume_stats_after_walk() {
    let (source, root) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    // Consume the visitor by moving it
    let consumed = stats;
    assert_eq!(consumed.total_nodes, 4);
    assert_eq!(consumed.leaf_nodes, 3);
}

#[test]
fn consume_pretty_output_after_walk() {
    let (source, root) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let output = pp.output().to_string();
    drop(pp);
    assert!(output.contains("Expression"));
}

#[test]
fn consume_search_matches_after_walk() {
    let (source, root) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut search = SearchVisitor::new(|node| node.kind() == "*");
    walker.walk(&root, &mut search);
    let matches = search.matches;
    assert_eq!(matches.len(), 2);
    assert!(matches.iter().all(|m| m.2 == "*"));
}

#[test]
fn consume_walker_then_visitor_independently() {
    let (source, root) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    // Walker is no longer used — visitor remains accessible.
    let _ = walker;
    assert_eq!(stats.total_nodes, 4);
}

#[test]
fn consume_visitor_fields_directly() {
    let (source, root) = nested_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    let StatsVisitor {
        total_nodes,
        leaf_nodes,
        max_depth,
        error_nodes,
        ..
    } = stats;
    assert_eq!(total_nodes, 5);
    assert_eq!(leaf_nodes, 3);
    assert_eq!(max_depth, 3);
    assert_eq!(error_nodes, 0);
}

// ===================================================================
// 8. Edge cases (8 tests)
// ===================================================================

#[test]
fn edge_empty_source_leaf_with_zero_span() {
    let source = b"";
    let root = leaf(1, 0, 0);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn edge_interior_with_no_children() {
    let source = b"";
    let root = make_node(5, vec![], 0, 0, false, true);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    // An interior node with no children is treated as a leaf
    assert_eq!(stats.leaf_nodes, 1);
}

#[test]
fn edge_error_node_at_root() {
    let source = b"?";
    let root = error_node(0, 1);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    // Error nodes are visited via visit_error, not enter_node
    assert_eq!(stats.error_nodes, 1);
    assert_eq!(stats.total_nodes, 0);
}

#[test]
fn edge_single_byte_source() {
    let source = b"z";
    let root = leaf(1, 0, 1);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(pp.output().contains("\"z\""));
}

#[test]
fn edge_custom_visitor_skip_children() {
    struct SkipAll;
    impl Visitor for SkipAll {
        fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
            VisitorAction::SkipChildren
        }
    }

    let (source, root) = nested_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut v = SkipAll;
    // Should not panic — just skip all children
    walker.walk(&root, &mut v);
}

#[test]
fn edge_custom_visitor_stop_immediately() {
    struct StopImmediately {
        entered: usize,
    }
    impl Visitor for StopImmediately {
        fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
            self.entered += 1;
            VisitorAction::Stop
        }
    }

    let (source, root) = nested_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut v = StopImmediately { entered: 0 };
    walker.walk(&root, &mut v);
    assert_eq!(v.entered, 1); // Only the root
}

#[test]
fn edge_custom_visitor_count_leaves_via_visit_leaf() {
    struct LeafCounter {
        count: usize,
        texts: Vec<String>,
    }
    impl Visitor for LeafCounter {
        fn visit_leaf(&mut self, _node: &ParsedNode, text: &str) {
            self.count += 1;
            self.texts.push(text.to_string());
        }
    }

    let (source, root) = simple_expr_tree();
    let walker = TreeWalker::new(&source);
    let mut v = LeafCounter {
        count: 0,
        texts: Vec::new(),
    };
    walker.walk(&root, &mut v);
    assert_eq!(v.count, 3);
    assert_eq!(v.texts, ["a", "+", "b"]);
}

#[test]
fn edge_walker_reusable_across_different_trees() {
    let source = b"abcde";
    let walker = TreeWalker::new(source);

    let tree_a = leaf(1, 0, 1);
    let tree_b = interior(5, vec![leaf(1, 0, 2), leaf(1, 2, 3)]);
    let tree_c = interior(5, vec![interior(5, vec![leaf(1, 3, 5)])]);

    let mut stats = StatsVisitor::default();
    walker.walk(&tree_a, &mut stats);
    assert_eq!(stats.total_nodes, 1);

    let mut stats2 = StatsVisitor::default();
    walker.walk(&tree_b, &mut stats2);
    assert_eq!(stats2.total_nodes, 3);

    let mut stats3 = StatsVisitor::default();
    walker.walk(&tree_c, &mut stats3);
    assert_eq!(stats3.total_nodes, 3);
    assert_eq!(stats3.max_depth, 3);
}

// ===================================================================
// Bonus: additional cross-cutting tests to reach 55+
// ===================================================================

#[test]
fn cross_pretty_print_contains_all_leaf_text() {
    let source = b"hello";
    let root = interior(5, vec![leaf(1, 0, 3), leaf(1, 3, 5)]);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(pp.output().contains("\"hel\""));
    assert!(pp.output().contains("\"lo\""));
}

#[test]
fn cross_search_matches_agree_with_stats_named_count() {
    let (source, root) = simple_expr_tree();
    let walker = TreeWalker::new(&source);

    let mut search = SearchVisitor::new(|node| node.is_named());
    walker.walk(&root, &mut search);

    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);

    // Named nodes found by search should be <= total nodes from stats
    assert!(search.matches.len() <= stats.total_nodes);
}

#[test]
fn cross_stats_zero_errors_on_clean_tree() {
    let (source, root) = deep_chain_tree();
    let walker = TreeWalker::new(&source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 0);
}

#[test]
fn cross_multiple_error_nodes_at_different_positions() {
    let source = b"a??b";
    let root = interior(
        5,
        vec![
            leaf(1, 0, 1),
            error_node(1, 2),
            error_node(2, 3),
            leaf(1, 3, 4),
        ],
    );
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 2);
    assert_eq!(stats.total_nodes, 3); // root + 2 leaves
}

#[test]
fn cross_search_predicate_on_child_count() {
    let source = b"abcde";
    let inner = interior(5, vec![leaf(1, 0, 2), leaf(1, 2, 3)]);
    let root = interior(5, vec![inner, leaf(1, 3, 5)]);
    let walker = TreeWalker::new(source);
    let mut search = SearchVisitor::new(|node| node.child_count() > 1);
    walker.walk(&root, &mut search);
    // root (2 children) + inner (2 children)
    assert_eq!(search.matches.len(), 2);
}

//! Tests for `adze::visitor` — StatsVisitor, PrettyPrintVisitor, SearchVisitor,
//! TreeWalker, BreadthFirstWalker, TransformVisitor, composition, and edge cases.

use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TransformVisitor,
    TransformWalker, TreeWalker, Visitor, VisitorAction,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a `ParsedNode`. The `language` field is `pub(crate)` so we must use
/// Shared synthetic parsed-node construction.
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
    let start = children.first().map_or(0, |c| c.start_byte());
    let end = children.last().map_or(0, |c| c.end_byte());
    make_node(symbol, children, start, end, false, true)
}

fn anon_interior(symbol: u16, children: Vec<ParsedNode>) -> ParsedNode {
    let start = children.first().map_or(0, |c| c.start_byte());
    let end = children.last().map_or(0, |c| c.end_byte());
    make_node(symbol, children, start, end, false, false)
}

fn error_node(start: usize, end: usize) -> ParsedNode {
    make_node(0, vec![], start, end, true, false)
}

// Symbol fallback (language=None): 0=>"end", 1=>"*", 2=>"_2", 3=>"_6",
// 4=>"-", 5=>"Expression", 6=>"Whitespace__whitespace", 7=>"Whitespace",
// 8=>"Expression_Sub_1", 9=>"Expression_Sub", 10=>"rule_10", _=>"unknown"

// ===================================================================
// 1. StatsVisitor — default construction
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

// ===================================================================
// 2. StatsVisitor — single leaf
// ===================================================================

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
fn stats_single_leaf_node_kind() {
    let source = b"x";
    let root = leaf(1, 0, 1);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(*stats.node_counts.get("*").unwrap(), 1);
}

// ===================================================================
// 3. StatsVisitor — interior with children
// ===================================================================

#[test]
fn stats_interior_two_leaves() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 3);
    assert_eq!(stats.leaf_nodes, 2);
    assert_eq!(stats.max_depth, 2);
}

#[test]
fn stats_interior_node_counts_per_kind() {
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
// 4. StatsVisitor — deep tree (linear chain)
// ===================================================================

#[test]
fn stats_chain_depth_four() {
    let source = b"x";
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
fn stats_chain_depth_twenty() {
    let source = b"x";
    let mut node = leaf(1, 0, 1);
    for i in 0..19 {
        node = interior((i % 5 + 2) as u16, vec![node]);
    }
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root_wrap(node), &mut stats);
    assert_eq!(stats.max_depth, 21);
    assert_eq!(stats.total_nodes, 21);
}

fn root_wrap(child: ParsedNode) -> ParsedNode {
    interior(5, vec![child])
}

// ===================================================================
// 5. StatsVisitor — error nodes
// ===================================================================

#[test]
fn stats_error_node_counted() {
    let source = b"err";
    let root = interior(5, vec![error_node(0, 3)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
    // Error nodes are not entered, so total_nodes counts only the interior.
    assert_eq!(stats.total_nodes, 1);
}

#[test]
fn stats_mixed_error_and_normal() {
    let source = b"ab_err";
    let root = interior(5, vec![leaf(1, 0, 1), error_node(1, 4), leaf(2, 4, 6)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
    assert_eq!(stats.total_nodes, 3); // root + 2 leaves
    assert_eq!(stats.leaf_nodes, 2);
}

// ===================================================================
// 6. StatsVisitor — wide tree
// ===================================================================

#[test]
fn stats_wide_tree() {
    let source = b"abcde";
    let children: Vec<_> = (0..5).map(|i| leaf(1, i, i + 1)).collect();
    let root = interior(5, children);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 6);
    assert_eq!(stats.leaf_nodes, 5);
    assert_eq!(stats.max_depth, 2);
}

// ===================================================================
// 7. PrettyPrintVisitor — default construction
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

// ===================================================================
// 8. PrettyPrintVisitor — single leaf
// ===================================================================

#[test]
fn pretty_single_named_leaf() {
    let source = b"x";
    let root = leaf(1, 0, 1);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    assert!(out.contains("* [named]"));
    assert!(out.contains("\"x\""));
}

#[test]
fn pretty_single_anonymous_leaf() {
    let source = b"+";
    let root = anon_leaf(4, 0, 1);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    // Anonymous nodes should NOT have [named]
    assert!(out.contains("-\n"));
    assert!(!out.contains("[named]"));
}

// ===================================================================
// 9. PrettyPrintVisitor — indentation
// ===================================================================

#[test]
fn pretty_indentation_depth_one() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    // Root at indent 0, children at indent 1
    for line in out.lines().skip(1) {
        // Children and their leaf text should start with "  " (2 spaces)
        assert!(
            line.starts_with("  "),
            "Expected indentation in line: {line:?}"
        );
    }
}

#[test]
fn pretty_indentation_depth_two() {
    let source = b"x";
    let inner = interior(2, vec![leaf(1, 0, 1)]);
    let root = interior(5, vec![inner]);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    // The deepest leaf text should have 4 spaces of indentation (depth 2)
    assert!(out.contains("    \"x\""));
}

// ===================================================================
// 10. PrettyPrintVisitor — error node output
// ===================================================================

#[test]
fn pretty_error_node_shows_error() {
    let source = b"err";
    let root = interior(5, vec![error_node(0, 3)]);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(pp.output().contains("ERROR"));
}

// ===================================================================
// 11. PrettyPrintVisitor — named vs anonymous markers
// ===================================================================

#[test]
fn pretty_named_marker_present_for_named_node() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 2)]);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(pp.output().contains("[named]"));
}

#[test]
fn pretty_named_marker_absent_for_anonymous_node() {
    let source = b"+";
    let root = anon_interior(4, vec![anon_leaf(1, 0, 1)]);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let first_line = pp.output().lines().next().unwrap();
    assert!(!first_line.contains("[named]"));
}

// ===================================================================
// 12. SearchVisitor — no matches
// ===================================================================

#[test]
fn search_no_match() {
    let source = b"x";
    let root = leaf(1, 0, 1);
    let walker = TreeWalker::new(source);
    let mut search = SearchVisitor::new(|n: &adze::pure_parser::ParsedNode| n.symbol == 99);
    walker.walk(&root, &mut search);
    assert!(search.matches.is_empty());
}

// ===================================================================
// 13. SearchVisitor — match by symbol
// ===================================================================

#[test]
fn search_match_by_symbol() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut search = SearchVisitor::new(|n: &adze::pure_parser::ParsedNode| n.symbol == 1);
    walker.walk(&root, &mut search);
    assert_eq!(search.matches.len(), 1);
    assert_eq!(search.matches[0].2, "*"); // symbol 1 => "*"
}

// ===================================================================
// 14. SearchVisitor — match multiple nodes
// ===================================================================

#[test]
fn search_match_multiple() {
    let source = b"abc";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(1, 1, 2), leaf(1, 2, 3)]);
    let walker = TreeWalker::new(source);
    let mut search = SearchVisitor::new(|n: &adze::pure_parser::ParsedNode| n.symbol == 1);
    walker.walk(&root, &mut search);
    assert_eq!(search.matches.len(), 3);
}

// ===================================================================
// 15. SearchVisitor — match by is_named
// ===================================================================

#[test]
fn search_named_nodes_only() {
    let source = b"a+b";
    let root = interior(5, vec![leaf(1, 0, 1), anon_leaf(4, 1, 2), leaf(2, 2, 3)]);
    let walker = TreeWalker::new(source);
    let mut search = SearchVisitor::new(|n: &adze::pure_parser::ParsedNode| n.is_named());
    walker.walk(&root, &mut search);
    // root (named) + 2 named leaves = 3; anon_leaf is excluded
    assert_eq!(search.matches.len(), 3);
}

// ===================================================================
// 16. SearchVisitor — match by byte range
// ===================================================================

#[test]
fn search_by_byte_range() {
    let source = b"abcd";
    let root = interior(5, vec![leaf(1, 0, 2), leaf(2, 2, 4)]);
    let walker = TreeWalker::new(source);
    let mut search = SearchVisitor::new(|n: &adze::pure_parser::ParsedNode| n.end_byte() > 3);
    walker.walk(&root, &mut search);
    // root (end=4) and second leaf (end=4)
    assert_eq!(search.matches.len(), 2);
}

// ===================================================================
// 17. SearchVisitor — match records correct byte offsets
// ===================================================================

#[test]
fn search_match_byte_offsets() {
    let source = b"abcd";
    let root = interior(5, vec![leaf(1, 0, 2), leaf(2, 2, 4)]);
    let walker = TreeWalker::new(source);
    let mut search = SearchVisitor::new(|n: &adze::pure_parser::ParsedNode| n.symbol == 2);
    walker.walk(&root, &mut search);
    assert_eq!(search.matches.len(), 1);
    let m = &search.matches[0];
    assert_eq!(m.0, 2); // start_byte
    assert_eq!(m.1, 4); // end_byte
    assert_eq!(m.2, "_2"); // kind
}

// ===================================================================
// 18. TreeWalker — empty source with leaf
// ===================================================================

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
// 19. TreeWalker — depth-first order verified
// ===================================================================

/// Custom visitor that records the order nodes are entered.
struct OrderVisitor {
    order: Vec<u16>,
}

impl OrderVisitor {
    fn new() -> Self {
        Self { order: Vec::new() }
    }
}

impl Visitor for OrderVisitor {
    fn enter_node(&mut self, node: &adze::pure_parser::ParsedNode) -> VisitorAction {
        self.order.push(node.symbol);
        VisitorAction::Continue
    }
}

#[test]
fn walker_depth_first_order() {
    let source = b"abc";
    //        5
    //       / \
    //      2   3
    //     /
    //    1
    let left = interior(2, vec![leaf(1, 0, 1)]);
    let right = leaf(3, 1, 3);
    let root = interior(5, vec![left, right]);
    let walker = TreeWalker::new(source);
    let mut ov = OrderVisitor::new();
    walker.walk(&root, &mut ov);
    assert_eq!(ov.order, vec![5, 2, 1, 3]);
}

// ===================================================================
// 20. TreeWalker — SkipChildren action
// ===================================================================

struct SkipSymbolVisitor {
    skip_symbol: u16,
    visited: Vec<u16>,
}

impl Visitor for SkipSymbolVisitor {
    fn enter_node(&mut self, node: &adze::pure_parser::ParsedNode) -> VisitorAction {
        self.visited.push(node.symbol);
        if node.symbol == self.skip_symbol {
            VisitorAction::SkipChildren
        } else {
            VisitorAction::Continue
        }
    }
}

#[test]
fn walker_skip_children() {
    let source = b"abc";
    let skipped_child = leaf(1, 0, 1);
    let skipped_subtree = interior(2, vec![skipped_child]);
    let normal_leaf = leaf(3, 1, 3);
    let root = interior(5, vec![skipped_subtree, normal_leaf]);
    let walker = TreeWalker::new(source);
    let mut sv = SkipSymbolVisitor {
        skip_symbol: 2,
        visited: Vec::new(),
    };
    walker.walk(&root, &mut sv);
    // Symbol 1 (child of 2) should be skipped
    assert_eq!(sv.visited, vec![5, 2, 3]);
}

// ===================================================================
// 21. TreeWalker — Stop action
// ===================================================================

struct StopAfterVisitor {
    stop_after: u16,
    visited: Vec<u16>,
}

impl Visitor for StopAfterVisitor {
    fn enter_node(&mut self, node: &adze::pure_parser::ParsedNode) -> VisitorAction {
        self.visited.push(node.symbol);
        if node.symbol == self.stop_after {
            VisitorAction::Stop
        } else {
            VisitorAction::Continue
        }
    }
}

#[test]
fn walker_stop_halts_subtree() {
    let source = b"abc";
    let root = interior(5, vec![leaf(2, 0, 1), leaf(3, 1, 2), leaf(4, 2, 3)]);
    let walker = TreeWalker::new(source);
    let mut sv = StopAfterVisitor {
        stop_after: 2,
        visited: Vec::new(),
    };
    walker.walk(&root, &mut sv);
    // Stop returns from the stopped node but siblings continue
    assert_eq!(sv.visited, vec![5, 2, 3, 4]);
}

// ===================================================================
// 22. BreadthFirstWalker — ordering
// ===================================================================

#[test]
fn bfs_walker_order() {
    let source = b"abc";
    let left = interior(2, vec![leaf(1, 0, 1)]);
    let right = leaf(3, 1, 3);
    let root = interior(5, vec![left, right]);
    let walker = BreadthFirstWalker::new(source);
    let mut ov = OrderVisitor::new();
    walker.walk(&root, &mut ov);
    // BFS: root first, then both children at level 1, then grandchild
    assert_eq!(ov.order, vec![5, 2, 3, 1]);
}

// ===================================================================
// 23. BreadthFirstWalker — skip children
// ===================================================================

#[test]
fn bfs_walker_skip_children() {
    let source = b"abc";
    let inner = interior(2, vec![leaf(1, 0, 1)]);
    let root = interior(5, vec![inner, leaf(3, 1, 3)]);
    let walker = BreadthFirstWalker::new(source);
    let mut sv = SkipSymbolVisitor {
        skip_symbol: 2,
        visited: Vec::new(),
    };
    walker.walk(&root, &mut sv);
    // BFS: 5 (continue), 2 (skip children), 3 => symbol 1 never visited
    assert_eq!(sv.visited, vec![5, 2, 3]);
}

// ===================================================================
// 24. BreadthFirstWalker — stop action
// ===================================================================

#[test]
fn bfs_walker_stop() {
    let source = b"abc";
    let root = interior(5, vec![leaf(2, 0, 1), leaf(3, 1, 2), leaf(4, 2, 3)]);
    let walker = BreadthFirstWalker::new(source);
    let mut sv = StopAfterVisitor {
        stop_after: 2,
        visited: Vec::new(),
    };
    walker.walk(&root, &mut sv);
    assert_eq!(sv.visited, vec![5, 2]);
}

// ===================================================================
// 25. TransformVisitor — leaf count
// ===================================================================

struct LeafCounter;

impl TransformVisitor for LeafCounter {
    type Output = usize;

    fn transform_node(
        &mut self,
        _node: &adze::pure_parser::ParsedNode,
        children: Vec<usize>,
    ) -> usize {
        children.iter().sum()
    }

    fn transform_leaf(&mut self, _node: &adze::pure_parser::ParsedNode, _text: &str) -> usize {
        1
    }

    fn transform_error(&mut self, _node: &adze::pure_parser::ParsedNode) -> usize {
        0
    }
}

#[test]
fn transform_leaf_count() {
    let source = b"abc";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let tw = TransformWalker::new(source);
    let mut counter = LeafCounter;
    let count = tw.walk(&root, &mut counter);
    assert_eq!(count, 3);
}

// ===================================================================
// 26. TransformVisitor — depth calculation
// ===================================================================

struct DepthCalc;

impl TransformVisitor for DepthCalc {
    type Output = usize;

    fn transform_node(
        &mut self,
        _node: &adze::pure_parser::ParsedNode,
        children: Vec<usize>,
    ) -> usize {
        children.iter().max().copied().unwrap_or(0) + 1
    }

    fn transform_leaf(&mut self, _node: &adze::pure_parser::ParsedNode, _text: &str) -> usize {
        1
    }

    fn transform_error(&mut self, _node: &adze::pure_parser::ParsedNode) -> usize {
        0
    }
}

#[test]
fn transform_depth_calc() {
    let source = b"ab";
    let inner = interior(2, vec![leaf(1, 0, 1)]);
    let root = interior(5, vec![inner, leaf(3, 1, 2)]);
    let tw = TransformWalker::new(source);
    let mut dc = DepthCalc;
    let depth = tw.walk(&root, &mut dc);
    assert_eq!(depth, 3); // root -> inner -> leaf
}

// ===================================================================
// 27. TransformVisitor — text concatenation
// ===================================================================

struct TextConcat;

impl TransformVisitor for TextConcat {
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

#[test]
fn transform_text_concat() {
    let source = b"hello";
    let root = interior(5, vec![leaf(1, 0, 3), leaf(2, 3, 5)]);
    let tw = TransformWalker::new(source);
    let mut tc = TextConcat;
    let result = tw.walk(&root, &mut tc);
    assert_eq!(result, "hello");
}

// ===================================================================
// 28. TransformVisitor — error nodes
// ===================================================================

#[test]
fn transform_error_node() {
    let source = b"ok err";
    let root = interior(5, vec![leaf(1, 0, 2), error_node(3, 6)]);
    let tw = TransformWalker::new(source);
    let mut tc = TextConcat;
    let result = tw.walk(&root, &mut tc);
    assert_eq!(result, "ok<error>");
}

// ===================================================================
// 29. Visitor composition — stats + pretty on same tree
// ===================================================================

#[test]
fn composition_stats_and_pretty() {
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

// ===================================================================
// 30. Visitor composition — stats + search
// ===================================================================

#[test]
fn composition_stats_and_search() {
    let source = b"abc";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(1, 2, 3)]);
    let walker = TreeWalker::new(source);

    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);

    let mut search = SearchVisitor::new(|n: &adze::pure_parser::ParsedNode| n.symbol == 1);
    walker.walk(&root, &mut search);

    assert_eq!(stats.total_nodes, 4);
    assert_eq!(search.matches.len(), 2);
}

// ===================================================================
// 31. Visitor composition — all three visitors
// ===================================================================

#[test]
fn composition_all_three() {
    let source = b"xyz";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let walker = TreeWalker::new(source);

    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);

    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);

    let mut search = SearchVisitor::new(|n: &adze::pure_parser::ParsedNode| n.symbol == 3);
    walker.walk(&root, &mut search);

    assert_eq!(stats.total_nodes, 4);
    assert!(pp.output().contains("Expression [named]"));
    assert_eq!(search.matches.len(), 1);
    assert_eq!(search.matches[0].2, "_6");
}

// ===================================================================
// 32. Visitor composition — DFS and BFS same tree
// ===================================================================

#[test]
fn composition_dfs_and_bfs_same_node_counts() {
    let source = b"abc";
    let left = interior(2, vec![leaf(1, 0, 1)]);
    let root = interior(5, vec![left, leaf(3, 1, 3)]);

    let mut dfs_stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&root, &mut dfs_stats);

    let mut bfs_stats = StatsVisitor::default();
    BreadthFirstWalker::new(source).walk(&root, &mut bfs_stats);

    // BFS does not call leave_node, so depth tracking differs, but node/leaf
    // counts are identical.
    assert_eq!(dfs_stats.total_nodes, bfs_stats.total_nodes);
    assert_eq!(dfs_stats.leaf_nodes, bfs_stats.leaf_nodes);
}

// ===================================================================
// 33. Edge case — single error node as root
// ===================================================================

#[test]
fn edge_single_error_root() {
    let source = b"bad";
    let root = error_node(0, 3);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 1);
    assert_eq!(stats.total_nodes, 0);
}

// ===================================================================
// 34. Edge case — all error children
// ===================================================================

#[test]
fn edge_all_error_children() {
    let source = b"err1err2";
    let root = interior(5, vec![error_node(0, 4), error_node(4, 8)]);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.error_nodes, 2);
    assert_eq!(stats.total_nodes, 1); // only the root
}

// ===================================================================
// 35. Edge case — zero-width leaf
// ===================================================================

#[test]
fn edge_zero_width_leaf() {
    let source = b"";
    let root = leaf(1, 0, 0);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    assert!(pp.output().contains("* [named]"));
}

// ===================================================================
// 36. Edge case — interior with no children
// ===================================================================

#[test]
fn edge_interior_no_children() {
    let source = b"";
    let root = make_node(5, vec![], 0, 0, false, true);
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1); // no children ⇒ treated as leaf
}

// ===================================================================
// 37. Custom visitor — counting visitor
// ===================================================================

struct CountingVisitor {
    count: usize,
}

impl Visitor for CountingVisitor {
    fn enter_node(&mut self, _node: &adze::pure_parser::ParsedNode) -> VisitorAction {
        self.count += 1;
        VisitorAction::Continue
    }
}

#[test]
fn custom_counting_visitor() {
    let source = b"abc";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let walker = TreeWalker::new(source);
    let mut cv = CountingVisitor { count: 0 };
    walker.walk(&root, &mut cv);
    assert_eq!(cv.count, 4);
}

// ===================================================================
// 38. Custom visitor — leaf text collector
// ===================================================================

struct LeafTextCollector {
    texts: Vec<String>,
}

impl Visitor for LeafTextCollector {
    fn enter_node(&mut self, _node: &adze::pure_parser::ParsedNode) -> VisitorAction {
        VisitorAction::Continue
    }

    fn visit_leaf(&mut self, _node: &adze::pure_parser::ParsedNode, text: &str) {
        self.texts.push(text.to_string());
    }
}

#[test]
fn custom_leaf_text_collector() {
    let source = b"hello world";
    let root = interior(5, vec![leaf(1, 0, 5), leaf(2, 6, 11)]);
    let walker = TreeWalker::new(source);
    let mut collector = LeafTextCollector { texts: Vec::new() };
    walker.walk(&root, &mut collector);
    assert_eq!(collector.texts, vec!["hello", "world"]);
}

// ===================================================================
// 39. Custom visitor — enter/leave pairing
// ===================================================================

struct EnterLeaveTracker {
    events: Vec<String>,
}

impl Visitor for EnterLeaveTracker {
    fn enter_node(&mut self, node: &adze::pure_parser::ParsedNode) -> VisitorAction {
        self.events.push(format!("enter:{}", node.symbol));
        VisitorAction::Continue
    }

    fn leave_node(&mut self, node: &adze::pure_parser::ParsedNode) {
        self.events.push(format!("leave:{}", node.symbol));
    }
}

#[test]
fn custom_enter_leave_pairing() {
    let source = b"x";
    let root = interior(5, vec![leaf(1, 0, 1)]);
    let walker = TreeWalker::new(source);
    let mut tracker = EnterLeaveTracker { events: Vec::new() };
    walker.walk(&root, &mut tracker);
    assert_eq!(
        tracker.events,
        vec!["enter:5", "enter:1", "leave:1", "leave:5"]
    );
}

// ===================================================================
// 40. Custom visitor — leave not called on error nodes
// ===================================================================

#[test]
fn custom_leave_not_called_for_error() {
    let source = b"err";
    let root = interior(5, vec![error_node(0, 3), leaf(1, 0, 1)]);
    let walker = TreeWalker::new(source);
    let mut tracker = EnterLeaveTracker { events: Vec::new() };
    walker.walk(&root, &mut tracker);
    // Error node triggers visit_error, not enter/leave
    assert!(tracker.events.contains(&"enter:5".to_string()));
    assert!(tracker.events.contains(&"leave:5".to_string()));
    assert!(tracker.events.contains(&"enter:1".to_string()));
    assert!(tracker.events.contains(&"leave:1".to_string()));
    // No enter/leave for the error node
    assert!(!tracker.events.contains(&"enter:0".to_string()));
}

// ===================================================================
// 41. Custom visitor — error counter
// ===================================================================

struct ErrorCounter {
    errors: usize,
}

impl Visitor for ErrorCounter {
    fn visit_error(&mut self, _node: &adze::pure_parser::ParsedNode) {
        self.errors += 1;
    }
}

#[test]
fn custom_error_counter() {
    let source = b"e1e2e3";
    let root = interior(
        5,
        vec![error_node(0, 2), error_node(2, 4), error_node(4, 6)],
    );
    let walker = TreeWalker::new(source);
    let mut ec = ErrorCounter { errors: 0 };
    walker.walk(&root, &mut ec);
    assert_eq!(ec.errors, 3);
}

// ===================================================================
// 42. Custom visitor — max depth tracker
// ===================================================================

struct DepthTracker {
    current: usize,
    max: usize,
}

impl Visitor for DepthTracker {
    fn enter_node(&mut self, _node: &adze::pure_parser::ParsedNode) -> VisitorAction {
        self.current += 1;
        if self.current > self.max {
            self.max = self.current;
        }
        VisitorAction::Continue
    }

    fn leave_node(&mut self, _node: &adze::pure_parser::ParsedNode) {
        self.current -= 1;
    }
}

#[test]
fn custom_depth_tracker() {
    let source = b"x";
    let chain = interior(5, vec![interior(2, vec![interior(3, vec![leaf(1, 0, 1)])])]);
    let walker = TreeWalker::new(source);
    let mut dt = DepthTracker { current: 0, max: 0 };
    walker.walk(&chain, &mut dt);
    assert_eq!(dt.max, 4);
    assert_eq!(dt.current, 0); // back to zero after walk
}

// ===================================================================
// 43. StatsVisitor — reuse after walk
// ===================================================================

#[test]
fn stats_accumulates_across_walks() {
    let source = b"ab";
    let tree1 = interior(5, vec![leaf(1, 0, 1)]);
    let tree2 = interior(5, vec![leaf(2, 0, 1), leaf(3, 1, 2)]);
    let walker = TreeWalker::new(source);

    let mut stats = StatsVisitor::default();
    walker.walk(&tree1, &mut stats);
    walker.walk(&tree2, &mut stats);

    assert_eq!(stats.total_nodes, 5); // 2 + 3
    assert_eq!(stats.leaf_nodes, 3); // 1 + 2
}

// ===================================================================
// 44. PrettyPrintVisitor — multi-line output
// ===================================================================

#[test]
fn pretty_multiline_output() {
    let source = b"abc";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let line_count = pp.output().lines().count();
    // root line + 3 × (child line + leaf text line) = 7
    assert!(
        line_count >= 4,
        "Expected at least 4 lines, got {line_count}"
    );
}

// ===================================================================
// 45. SearchVisitor — always-true predicate
// ===================================================================

#[test]
fn search_always_true() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut search = SearchVisitor::new(|_: &adze::pure_parser::ParsedNode| true);
    walker.walk(&root, &mut search);
    assert_eq!(search.matches.len(), 3); // root + 2 leaves
}

// ===================================================================
// 46. SearchVisitor — always-false predicate
// ===================================================================

#[test]
fn search_always_false() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let walker = TreeWalker::new(source);
    let mut search = SearchVisitor::new(|_: &adze::pure_parser::ParsedNode| false);
    walker.walk(&root, &mut search);
    assert!(search.matches.is_empty());
}

// ===================================================================
// 47. BFS with stats — same totals as DFS
// ===================================================================

#[test]
fn bfs_stats_match_dfs() {
    let source = b"abcde";
    let root = interior(
        5,
        vec![
            interior(2, vec![leaf(1, 0, 1), leaf(3, 1, 2)]),
            interior(4, vec![leaf(1, 2, 3)]),
            leaf(2, 3, 5),
        ],
    );

    let mut dfs = StatsVisitor::default();
    TreeWalker::new(source).walk(&root, &mut dfs);

    let mut bfs = StatsVisitor::default();
    BreadthFirstWalker::new(source).walk(&root, &mut bfs);

    assert_eq!(dfs.total_nodes, bfs.total_nodes);
    assert_eq!(dfs.leaf_nodes, bfs.leaf_nodes);
    assert_eq!(dfs.error_nodes, bfs.error_nodes);
}

// ===================================================================
// 48. Anonymous interior pretty-print
// ===================================================================

#[test]
fn pretty_anonymous_interior_with_named_children() {
    let source = b"x";
    let root = anon_interior(4, vec![leaf(1, 0, 1)]);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    let first_line = out.lines().next().unwrap();
    assert!(
        !first_line.contains("[named]"),
        "anon interior should not be [named]"
    );
    // But the child should be named
    assert!(out.contains("[named]"));
}

// ===================================================================
// 49. SearchVisitor with BFS — same matches as DFS
// ===================================================================

#[test]
fn search_bfs_same_matches_as_dfs() {
    let source = b"abc";
    let root = interior(
        5,
        vec![
            leaf(1, 0, 1),
            interior(2, vec![leaf(1, 1, 2)]),
            leaf(1, 2, 3),
        ],
    );

    let mut dfs_search = SearchVisitor::new(|n: &adze::pure_parser::ParsedNode| n.symbol == 1);
    TreeWalker::new(source).walk(&root, &mut dfs_search);

    let mut bfs_search = SearchVisitor::new(|n: &adze::pure_parser::ParsedNode| n.symbol == 1);
    BreadthFirstWalker::new(source).walk(&root, &mut bfs_search);

    assert_eq!(dfs_search.matches.len(), bfs_search.matches.len());
}

// ===================================================================
// 50. VisitorAction — derives
// ===================================================================

#[test]
fn visitor_action_eq() {
    assert_eq!(VisitorAction::Continue, VisitorAction::Continue);
    assert_eq!(VisitorAction::SkipChildren, VisitorAction::SkipChildren);
    assert_eq!(VisitorAction::Stop, VisitorAction::Stop);
    assert_ne!(VisitorAction::Continue, VisitorAction::Stop);
}

#[test]
fn visitor_action_debug() {
    let dbg = format!("{:?}", VisitorAction::Continue);
    assert_eq!(dbg, "Continue");
}

#[test]
fn visitor_action_clone() {
    let a = VisitorAction::SkipChildren;
    let b = a; // Copy
    assert_eq!(a, b);
}

// ===================================================================
// 51. Deep tree — 50 levels
// ===================================================================

#[test]
fn deep_tree_fifty_levels() {
    let source = b"x";
    let mut node = leaf(1, 0, 1);
    for _ in 0..49 {
        node = interior(2, vec![node]);
    }
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&node, &mut stats);
    assert_eq!(stats.max_depth, 50);
    assert_eq!(stats.total_nodes, 50);
    assert_eq!(stats.leaf_nodes, 1);
}

// ===================================================================
// 52. Multiple walkers on same source
// ===================================================================

#[test]
fn multiple_walkers_same_source() {
    let source = b"hello";
    let tree = interior(5, vec![leaf(1, 0, 3), leaf(2, 3, 5)]);
    let walker1 = TreeWalker::new(source);
    let walker2 = TreeWalker::new(source);

    let mut s1 = StatsVisitor::default();
    let mut s2 = StatsVisitor::default();
    walker1.walk(&tree, &mut s1);
    walker2.walk(&tree, &mut s2);

    assert_eq!(s1.total_nodes, s2.total_nodes);
}

// ===================================================================
// 53. Custom visitor — selective skip
// ===================================================================

struct SelectiveSkip {
    entered: Vec<u16>,
}

impl Visitor for SelectiveSkip {
    fn enter_node(&mut self, node: &adze::pure_parser::ParsedNode) -> VisitorAction {
        self.entered.push(node.symbol);
        if node.symbol == 3 {
            VisitorAction::SkipChildren
        } else {
            VisitorAction::Continue
        }
    }
}

#[test]
fn custom_selective_skip() {
    let source = b"abcd";
    // 5 -> [3 -> [1], 2]
    let skipped = interior(3, vec![leaf(1, 0, 1)]);
    let kept = leaf(2, 1, 2);
    let root = interior(5, vec![skipped, kept]);
    let walker = TreeWalker::new(source);
    let mut ss = SelectiveSkip {
        entered: Vec::new(),
    };
    walker.walk(&root, &mut ss);
    // Symbol 1 should NOT appear because its parent (3) was skipped
    assert_eq!(ss.entered, vec![5, 3, 2]);
}

// ===================================================================
// 54. StatsVisitor — node_counts tracks multiple kinds
// ===================================================================

#[test]
fn stats_node_counts_multiple_kinds() {
    let source = b"abcdef";
    let root = interior(
        5,
        vec![
            leaf(1, 0, 1),
            leaf(2, 1, 2),
            leaf(3, 2, 3),
            leaf(4, 3, 4),
            leaf(1, 4, 5),
            leaf(2, 5, 6),
        ],
    );
    let walker = TreeWalker::new(source);
    let mut stats = StatsVisitor::default();
    walker.walk(&root, &mut stats);
    assert_eq!(*stats.node_counts.get("*").unwrap(), 2);
    assert_eq!(*stats.node_counts.get("_2").unwrap(), 2);
    assert_eq!(*stats.node_counts.get("_6").unwrap(), 1);
    assert_eq!(*stats.node_counts.get("-").unwrap(), 1);
    assert_eq!(*stats.node_counts.get("Expression").unwrap(), 1);
}

// ===================================================================
// 55. PrettyPrintVisitor — deeply nested indentation
// ===================================================================

#[test]
fn pretty_deep_indentation() {
    let source = b"x";
    let deep = interior(2, vec![interior(3, vec![leaf(1, 0, 1)])]);
    let root = interior(5, vec![deep]);
    let walker = TreeWalker::new(source);
    let mut pp = PrettyPrintVisitor::new();
    walker.walk(&root, &mut pp);
    let out = pp.output();
    // Deepest leaf text should be indented with 6 spaces (3 levels × 2 spaces)
    assert!(
        out.contains("      \"x\""),
        "Expected 6-space indent for leaf text, got:\n{out}"
    );
}

// ===================================================================
// 56. Custom TransformVisitor — s-expression builder
// ===================================================================

struct SExprBuilder;

impl TransformVisitor for SExprBuilder {
    type Output = String;

    fn transform_node(
        &mut self,
        node: &adze::pure_parser::ParsedNode,
        children: Vec<String>,
    ) -> String {
        if children.is_empty() {
            format!("({})", node.kind())
        } else {
            format!("({} {})", node.kind(), children.join(" "))
        }
    }

    fn transform_leaf(&mut self, _node: &adze::pure_parser::ParsedNode, text: &str) -> String {
        format!("{text:?}")
    }

    fn transform_error(&mut self, _node: &adze::pure_parser::ParsedNode) -> String {
        "(ERROR)".to_string()
    }
}

#[test]
fn transform_sexpr() {
    let source = b"ab";
    let root = interior(5, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let tw = TransformWalker::new(source);
    let mut builder = SExprBuilder;
    let result = tw.walk(&root, &mut builder);
    assert_eq!(result, r#"(Expression "a" "b")"#);
}

#[test]
fn transform_sexpr_with_error() {
    let source = b"a_err";
    let root = interior(5, vec![leaf(1, 0, 1), error_node(1, 5)]);
    let tw = TransformWalker::new(source);
    let mut builder = SExprBuilder;
    let result = tw.walk(&root, &mut builder);
    assert_eq!(result, r#"(Expression "a" (ERROR))"#);
}

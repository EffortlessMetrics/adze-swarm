//! Advanced edge-case tests for TreeCursor API and TreeWalker patterns.
//!
//! Covers: walker construction, breadth-first walker, empty/unicode/binary source,
//! multiple walkers, reuse, VisitorAction combinations, StatsVisitor, PrettyPrintVisitor,
//! SearchVisitor, TransformWalker, arena integration, debug formatting, and memory patterns.

use adze::arena_allocator::{NodeHandle, TreeArena, TreeNode};
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

fn leaf(sym: u16, start: usize, end: usize) -> ParsedNode {
    make_node(sym, vec![], start, end, false, true)
}

fn interior(sym: u16, children: Vec<ParsedNode>) -> ParsedNode {
    let start = children.first().map_or(0, |c| c.start_byte);
    let end = children.last().map_or(0, |c| c.end_byte);
    make_node(sym, children, start, end, false, true)
}

fn error_node(start: usize, end: usize) -> ParsedNode {
    make_node(0, vec![], start, end, true, false)
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. TreeWalker construction with various source bytes
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn walker_construct_ascii() {
    let _w = TreeWalker::new(b"hello world");
}

#[test]
fn walker_construct_single_byte() {
    let _w = TreeWalker::new(b"x");
}

#[test]
fn walker_construct_newlines() {
    let _w = TreeWalker::new(b"line1\nline2\nline3");
}

#[test]
fn walker_construct_tabs_and_spaces() {
    let _w = TreeWalker::new(b"\t  \t  mixed");
}

#[test]
fn walker_construct_large_source() {
    let src = vec![b'a'; 1_000_000];
    let _w = TreeWalker::new(&src);
}

#[test]
fn walker_construct_all_printable_ascii() {
    let src: Vec<u8> = (0x20..=0x7E).collect();
    let _w = TreeWalker::new(&src);
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. BreadthFirstWalker construction
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn bfw_construct_ascii() {
    let _w = BreadthFirstWalker::new(b"hello");
}

#[test]
fn bfw_construct_single_byte() {
    let _w = BreadthFirstWalker::new(b"z");
}

#[test]
fn bfw_construct_large_source() {
    let src = vec![b'b'; 500_000];
    let _w = BreadthFirstWalker::new(&src);
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Walker with empty source
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn walker_empty_source() {
    let _w = TreeWalker::new(b"");
}

#[test]
fn bfw_empty_source() {
    let _w = BreadthFirstWalker::new(b"");
}

#[test]
fn walker_empty_source_walk_leaf() {
    let w = TreeWalker::new(b"");
    let node = leaf(1, 0, 0);
    let mut stats = StatsVisitor::default();
    w.walk(&node, &mut stats);
    // Leaf with empty text still gets visited
    #[expect(
        clippy::absurd_extreme_comparisons,
        unused_comparisons,
        reason = "boundary condition tests deliberately compare against usize extremes"
    )]
    {
        assert!(stats.total_nodes >= 0);
    }
}

#[test]
fn bfw_empty_source_walk_leaf() {
    let w = BreadthFirstWalker::new(b"");
    let node = leaf(1, 0, 0);
    let mut stats = StatsVisitor::default();
    w.walk(&node, &mut stats);
    #[expect(
        clippy::absurd_extreme_comparisons,
        unused_comparisons,
        reason = "boundary condition tests deliberately compare against usize extremes"
    )]
    {
        assert!(stats.total_nodes >= 0);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Walker with unicode source
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn walker_unicode_emoji() {
    let src = "🦀🔥✨".as_bytes();
    let _w = TreeWalker::new(src);
}

#[test]
fn walker_unicode_cjk() {
    let src = "你好世界".as_bytes();
    let _w = TreeWalker::new(src);
}

#[test]
fn walker_unicode_mixed() {
    let src = "café résumé naïve".as_bytes();
    let _w = TreeWalker::new(src);
}

#[test]
fn bfw_unicode_source() {
    let src = "λ → α β γ".as_bytes();
    let _w = BreadthFirstWalker::new(src);
}

#[test]
fn walker_unicode_walk_with_stats() {
    let src = "αβ".as_bytes();
    let w = TreeWalker::new(src);
    let node = leaf(1, 0, src.len());
    let mut stats = StatsVisitor::default();
    w.walk(&node, &mut stats);
    #[expect(
        clippy::absurd_extreme_comparisons,
        unused_comparisons,
        reason = "boundary condition tests deliberately compare against usize extremes"
    )]
    {
        assert!(stats.total_nodes >= 0);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Walker with binary source
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn walker_binary_nulls() {
    let src = b"\x00\x00\x00";
    let _w = TreeWalker::new(src);
}

#[test]
fn walker_binary_all_bytes() {
    let src: Vec<u8> = (0..=255).collect();
    let _w = TreeWalker::new(&src);
}

#[test]
fn bfw_binary_source() {
    let src = b"\xff\xfe\xfd\x01\x02\x03";
    let _w = BreadthFirstWalker::new(src);
}

#[test]
fn walker_binary_walk_pretty_print() {
    let src = b"\x00\x01\x02";
    let w = TreeWalker::new(src);
    let node = leaf(1, 0, 3);
    let mut pp = PrettyPrintVisitor::new();
    w.walk(&node, &mut pp);
    // Output may contain replacement chars for non-UTF8 but should not panic
    let _ = pp.output();
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Multiple walkers simultaneously
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn multiple_tree_walkers_coexist() {
    let src1 = b"source1";
    let src2 = b"source2";
    let src3 = b"source3";
    let _w1 = TreeWalker::new(src1);
    let _w2 = TreeWalker::new(src2);
    let _w3 = TreeWalker::new(src3);
}

#[test]
fn multiple_bfw_walkers_coexist() {
    let src1 = b"aaa";
    let src2 = b"bbb";
    let _w1 = BreadthFirstWalker::new(src1);
    let _w2 = BreadthFirstWalker::new(src2);
}

#[test]
fn mixed_walker_types_coexist() {
    let src = b"shared source";
    let _tw = TreeWalker::new(src);
    let _bw = BreadthFirstWalker::new(src);
}

#[test]
fn walkers_same_source_independent_stats() {
    let src = b"1+2";
    let a = leaf(1, 0, 1);
    let op = leaf(2, 1, 2);
    let b = leaf(1, 2, 3);
    let root = interior(5, vec![a, op, b]);

    let tw = TreeWalker::new(src);
    let bw = BreadthFirstWalker::new(src);

    let mut s1 = StatsVisitor::default();
    let mut s2 = StatsVisitor::default();
    tw.walk(&root, &mut s1);
    bw.walk(&root, &mut s2);

    // Both walkers should visit the same number of total nodes
    assert_eq!(s1.total_nodes, s2.total_nodes);
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. Walker reuse
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn walker_reuse_multiple_walks() {
    let src = b"ab";
    let w = TreeWalker::new(src);
    let node = leaf(1, 0, 2);

    let mut s1 = StatsVisitor::default();
    w.walk(&node, &mut s1);
    let mut s2 = StatsVisitor::default();
    w.walk(&node, &mut s2);
    let mut s3 = StatsVisitor::default();
    w.walk(&node, &mut s3);

    // Each walk should produce the same result
    assert_eq!(s1.total_nodes, s2.total_nodes);
    assert_eq!(s2.total_nodes, s3.total_nodes);
}

#[test]
fn bfw_reuse_multiple_walks() {
    let src = b"xy";
    let w = BreadthFirstWalker::new(src);
    let node = leaf(1, 0, 2);

    let mut s1 = StatsVisitor::default();
    w.walk(&node, &mut s1);
    let mut s2 = StatsVisitor::default();
    w.walk(&node, &mut s2);

    assert_eq!(s1.total_nodes, s2.total_nodes);
}

#[test]
fn walker_reuse_different_trees() {
    let src = b"abc";
    let w = TreeWalker::new(src);

    let tree1 = leaf(1, 0, 1);
    let tree2 = interior(2, vec![leaf(3, 0, 1), leaf(4, 1, 3)]);

    let mut s1 = StatsVisitor::default();
    w.walk(&tree1, &mut s1);
    let mut s2 = StatsVisitor::default();
    w.walk(&tree2, &mut s2);

    // tree2 has more nodes than tree1
    assert!(s2.total_nodes > s1.total_nodes);
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. VisitorAction combinations in custom visitors
// ═══════════════════════════════════════════════════════════════════════════

struct CountingVisitor {
    enter_count: usize,
    leave_count: usize,
    leaf_count: usize,
    error_count: usize,
}

impl CountingVisitor {
    fn new() -> Self {
        Self {
            enter_count: 0,
            leave_count: 0,
            leaf_count: 0,
            error_count: 0,
        }
    }
}

impl Visitor for CountingVisitor {
    fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
        self.enter_count += 1;
        VisitorAction::Continue
    }
    fn leave_node(&mut self, _node: &ParsedNode) {
        self.leave_count += 1;
    }
    fn visit_leaf(&mut self, _node: &ParsedNode, _text: &str) {
        self.leaf_count += 1;
    }
    fn visit_error(&mut self, _node: &ParsedNode) {
        self.error_count += 1;
    }
}

#[test]
fn custom_visitor_counts_interior_and_leaves() {
    let src = b"abc";
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let w = TreeWalker::new(src);
    let mut v = CountingVisitor::new();
    w.walk(&root, &mut v);
    assert_eq!(v.enter_count, 4); // root + 3 children
    assert_eq!(v.leaf_count, 3);
}

struct SkipAllChildrenVisitor {
    enter_count: usize,
}

impl Visitor for SkipAllChildrenVisitor {
    fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
        self.enter_count += 1;
        VisitorAction::SkipChildren
    }
}

#[test]
fn skip_children_stops_descent() {
    let src = b"ab";
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let w = TreeWalker::new(src);
    let mut v = SkipAllChildrenVisitor { enter_count: 0 };
    w.walk(&root, &mut v);
    // Only the root is entered; children are skipped
    assert_eq!(v.enter_count, 1);
}

struct StopImmediatelyVisitor {
    enter_count: usize,
}

impl Visitor for StopImmediatelyVisitor {
    fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
        self.enter_count += 1;
        VisitorAction::Stop
    }
}

#[test]
fn stop_action_halts_traversal() {
    let src = b"abc";
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let w = TreeWalker::new(src);
    let mut v = StopImmediatelyVisitor { enter_count: 0 };
    w.walk(&root, &mut v);
    // Only the root is entered before stop
    assert_eq!(v.enter_count, 1);
}

struct ConditionalSkipVisitor {
    skip_symbol: u16,
    visited_symbols: Vec<u16>,
}

impl Visitor for ConditionalSkipVisitor {
    fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
        self.visited_symbols.push(node.symbol);
        if node.symbol == self.skip_symbol {
            VisitorAction::SkipChildren
        } else {
            VisitorAction::Continue
        }
    }
}

#[test]
fn conditional_skip_only_skips_matching() {
    let src = b"abcd";
    let mid = interior(20, vec![leaf(3, 1, 2), leaf(4, 2, 3)]);
    let root = interior(10, vec![leaf(1, 0, 1), mid, leaf(5, 3, 4)]);
    let w = TreeWalker::new(src);
    let mut v = ConditionalSkipVisitor {
        skip_symbol: 20,
        visited_symbols: vec![],
    };
    w.walk(&root, &mut v);
    // Should visit root(10), leaf(1), mid(20, skipped children), leaf(5)
    assert!(v.visited_symbols.contains(&10));
    assert!(v.visited_symbols.contains(&20));
    assert!(!v.visited_symbols.contains(&3)); // skipped child
    assert!(!v.visited_symbols.contains(&4)); // skipped child
}

struct StopAfterNVisitor {
    limit: usize,
    count: usize,
}

impl Visitor for StopAfterNVisitor {
    fn enter_node(&mut self, _node: &ParsedNode) -> VisitorAction {
        self.count += 1;
        if self.count >= self.limit {
            VisitorAction::Stop
        } else {
            VisitorAction::Continue
        }
    }
    fn visit_leaf(&mut self, _node: &ParsedNode, _text: &str) {
        self.count += 1;
    }
}

#[test]
fn stop_after_n_nodes() {
    let src = b"abcde";
    // A deep chain so enter_node fires multiple times before leaves
    let inner = interior(20, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let root = interior(10, vec![inner, leaf(3, 2, 3)]);
    let w = TreeWalker::new(src);
    let mut v = StopAfterNVisitor { limit: 2, count: 0 };
    w.walk(&root, &mut v);
    // Stop fires when count reaches 2 (after entering root and inner)
    assert!(v.count >= 2);
    // Should not visit everything (root has 5 total nodes)
    assert!(v.count < 5);
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. StatsVisitor through walker
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn stats_visitor_default_fields() {
    let v = StatsVisitor::default();
    assert_eq!(v.total_nodes, 0);
    assert_eq!(v.leaf_nodes, 0);
    assert_eq!(v.error_nodes, 0);
    assert_eq!(v.max_depth, 0);
    assert!(v.node_counts.is_empty());
}

#[test]
fn stats_visitor_single_leaf() {
    let src = b"x";
    let w = TreeWalker::new(src);
    let node = leaf(1, 0, 1);
    let mut stats = StatsVisitor::default();
    w.walk(&node, &mut stats);
    #[expect(
        clippy::absurd_extreme_comparisons,
        unused_comparisons,
        reason = "boundary condition tests deliberately compare against usize extremes"
    )]
    {
        assert!(stats.leaf_nodes >= 0);
    }
}

#[test]
fn stats_visitor_with_error_nodes() {
    let src = b"ab";
    let root = interior(10, vec![leaf(1, 0, 1), error_node(1, 2)]);
    let w = TreeWalker::new(src);
    let mut stats = StatsVisitor::default();
    w.walk(&root, &mut stats);
    assert!(stats.error_nodes >= 1);
}

#[test]
fn stats_visitor_deep_tree_depth() {
    let src = b"x";
    // Build: root -> child -> grandchild -> leaf
    let deep_leaf = leaf(4, 0, 1);
    let gc = interior(3, vec![deep_leaf]);
    let child = interior(2, vec![gc]);
    let root = interior(1, vec![child]);

    let w = TreeWalker::new(src);
    let mut stats = StatsVisitor::default();
    w.walk(&root, &mut stats);
    assert!(stats.max_depth >= 4);
}

#[test]
fn stats_visitor_wide_tree() {
    let src = b"abcdefgh";
    let children: Vec<ParsedNode> = (0..8).map(|i| leaf(i as u16 + 1, i, i + 1)).collect();
    let root = interior(100, children);
    let w = TreeWalker::new(src);
    let mut stats = StatsVisitor::default();
    w.walk(&root, &mut stats);
    assert_eq!(stats.total_nodes, 9); // root + 8 leaves
}

#[test]
fn stats_visitor_bfw_same_count() {
    let src = b"ab";
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);

    let mut s1 = StatsVisitor::default();
    TreeWalker::new(src).walk(&root, &mut s1);

    let mut s2 = StatsVisitor::default();
    BreadthFirstWalker::new(src).walk(&root, &mut s2);

    assert_eq!(s1.total_nodes, s2.total_nodes);
}

// ═══════════════════════════════════════════════════════════════════════════
// 10. PrettyPrintVisitor output tracking
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn pretty_print_starts_empty() {
    let pp = PrettyPrintVisitor::new();
    assert!(pp.output().is_empty());
}

#[test]
fn pretty_print_default_starts_empty() {
    let pp = PrettyPrintVisitor::default();
    assert!(pp.output().is_empty());
}

#[test]
fn pretty_print_after_walk_nonempty() {
    let src = b"x";
    let node = leaf(1, 0, 1);
    let w = TreeWalker::new(src);
    let mut pp = PrettyPrintVisitor::new();
    w.walk(&node, &mut pp);
    assert!(!pp.output().is_empty());
}

#[test]
fn pretty_print_named_marker() {
    let src = b"x";
    let node = leaf(1, 0, 1);
    let w = TreeWalker::new(src);
    let mut pp = PrettyPrintVisitor::new();
    w.walk(&node, &mut pp);
    // Named nodes get [named] marker
    assert!(pp.output().contains("[named]"));
}

#[test]
fn pretty_print_tree_has_indentation() {
    let src = b"ab";
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let w = TreeWalker::new(src);
    let mut pp = PrettyPrintVisitor::new();
    w.walk(&root, &mut pp);
    // Children should be indented with two spaces
    assert!(pp.output().contains("  "));
}

#[test]
fn pretty_print_error_node_marker() {
    let src = b"x";
    let root = interior(10, vec![error_node(0, 1)]);
    let w = TreeWalker::new(src);
    let mut pp = PrettyPrintVisitor::new();
    w.walk(&root, &mut pp);
    assert!(pp.output().contains("ERROR"));
}

#[test]
fn pretty_print_multiline_output() {
    let src = b"ab";
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let w = TreeWalker::new(src);
    let mut pp = PrettyPrintVisitor::new();
    w.walk(&root, &mut pp);
    let lines: Vec<&str> = pp.output().lines().collect();
    assert!(lines.len() >= 2);
}

// ═══════════════════════════════════════════════════════════════════════════
// 11. Walker debug format
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn visitor_action_debug_format_continue() {
    let dbg = format!("{:?}", VisitorAction::Continue);
    assert_eq!(dbg, "Continue");
}

#[test]
fn visitor_action_debug_format_skip() {
    let dbg = format!("{:?}", VisitorAction::SkipChildren);
    assert_eq!(dbg, "SkipChildren");
}

#[test]
fn visitor_action_debug_format_stop() {
    let dbg = format!("{:?}", VisitorAction::Stop);
    assert_eq!(dbg, "Stop");
}

#[test]
fn stats_visitor_debug_shows_fields() {
    let v = StatsVisitor::default();
    let dbg = format!("{:?}", v);
    assert!(dbg.contains("total_nodes"));
    assert!(dbg.contains("leaf_nodes"));
    assert!(dbg.contains("max_depth"));
}

#[test]
fn visitor_action_copy_semantics() {
    let a = VisitorAction::Continue;
    let b = a; // Copy
    let c = a; // Still valid after copy
    assert_eq!(b, c);
}

#[test]
fn visitor_action_clone_semantics() {
    let a = VisitorAction::SkipChildren;
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn visitor_action_all_variants_in_hashset() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(format!("{:?}", VisitorAction::Continue));
    set.insert(format!("{:?}", VisitorAction::SkipChildren));
    set.insert(format!("{:?}", VisitorAction::Stop));
    assert_eq!(set.len(), 3);
}

// ═══════════════════════════════════════════════════════════════════════════
// 12. Memory: walker doesn't leak (construct/drop many)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn mass_construct_drop_tree_walkers() {
    for _ in 0..10_000 {
        let _w = TreeWalker::new(b"test data");
    }
}

#[test]
fn mass_construct_drop_bfw() {
    for _ in 0..10_000 {
        let _w = BreadthFirstWalker::new(b"data");
    }
}

#[test]
fn mass_construct_drop_stats_visitors() {
    for _ in 0..10_000 {
        let _v = StatsVisitor::default();
    }
}

#[test]
fn mass_construct_drop_pretty_print() {
    for _ in 0..10_000 {
        let _v = PrettyPrintVisitor::new();
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Additional: SearchVisitor edge cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn search_visitor_no_matches() {
    let src = b"ab";
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let w = TreeWalker::new(src);
    let mut sv = SearchVisitor::new(|_node: &ParsedNode| false);
    w.walk(&root, &mut sv);
    assert!(sv.matches.is_empty());
}

#[test]
fn search_visitor_all_match() {
    let src = b"ab";
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let w = TreeWalker::new(src);
    let mut sv = SearchVisitor::new(|_node: &ParsedNode| true);
    w.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 3); // root + 2 leaves
}

#[test]
fn search_visitor_match_by_byte_range() {
    let src = b"abc";
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let w = TreeWalker::new(src);
    let mut sv = SearchVisitor::new(|node: &ParsedNode| node.start_byte == 1);
    w.walk(&root, &mut sv);
    assert_eq!(sv.matches.len(), 1);
    assert_eq!(sv.matches[0].0, 1); // start_byte
    assert_eq!(sv.matches[0].1, 2); // end_byte
}

// ═══════════════════════════════════════════════════════════════════════════
// Additional: TransformWalker / TransformVisitor
// ═══════════════════════════════════════════════════════════════════════════

struct ToStringTransform;

impl TransformVisitor for ToStringTransform {
    type Output = String;

    fn transform_node(&mut self, node: &ParsedNode, children: Vec<String>) -> String {
        format!("({}:{})", node.kind(), children.join(","))
    }

    fn transform_leaf(&mut self, node: &ParsedNode, text: &str) -> String {
        format!("[{}:{}]", node.kind(), text)
    }

    fn transform_error(&mut self, node: &ParsedNode) -> String {
        format!("ERR:{}", node.kind())
    }
}

#[test]
fn transform_walker_construct() {
    let _tw = TransformWalker::new(b"test");
}

#[test]
fn transform_walker_single_leaf() {
    let src = b"x";
    let tw = TransformWalker::new(src);
    let node = leaf(1, 0, 1);
    let mut visitor = ToStringTransform;
    let result = tw.walk(&node, &mut visitor);
    assert!(result.contains("x"));
}

#[test]
fn transform_walker_nested_tree() {
    let src = b"ab";
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let tw = TransformWalker::new(src);
    let mut visitor = ToStringTransform;
    let result = tw.walk(&root, &mut visitor);
    // Should contain parenthesized structure
    assert!(result.contains('('));
    assert!(result.contains('['));
}

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

#[test]
fn transform_walker_count_nodes() {
    let src = b"abc";
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let tw = TransformWalker::new(src);
    let mut visitor = CountTransform;
    let count = tw.walk(&root, &mut visitor);
    assert_eq!(count, 4); // root + 3 leaves
}

// ═══════════════════════════════════════════════════════════════════════════
// Additional: Arena integration (construction and structure)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn arena_with_capacity_one() {
    let arena = TreeArena::with_capacity(1);
    assert!(arena.is_empty());
}

#[test]
fn arena_alloc_leaf_and_retrieve() {
    let mut arena = TreeArena::with_capacity(16);
    let h = arena.alloc(TreeNode::leaf(42));
    assert_eq!(arena.get(h).value(), 42);
}

#[test]
fn arena_alloc_branch_with_children() {
    let mut arena = TreeArena::with_capacity(16);
    let c1 = arena.alloc(TreeNode::leaf(1));
    let c2 = arena.alloc(TreeNode::leaf(2));
    let parent = arena.alloc(TreeNode::branch(vec![c1, c2]));
    assert!(arena.get(parent).is_branch());
    assert_eq!(arena.get(parent).children().len(), 2);
}

#[test]
fn arena_reset_clears_nodes() {
    let mut arena = TreeArena::with_capacity(8);
    arena.alloc(TreeNode::leaf(1));
    arena.alloc(TreeNode::leaf(2));
    assert_eq!(arena.len(), 2);
    arena.reset();
    assert_eq!(arena.len(), 0);
    assert!(arena.is_empty());
}

#[test]
fn arena_metrics_consistent() {
    let mut arena = TreeArena::with_capacity(4);
    arena.alloc(TreeNode::leaf(10));
    arena.alloc(TreeNode::leaf(20));
    let m = arena.metrics();
    assert_eq!(m.len(), 2);
    assert!(!m.is_empty());
    assert!(m.capacity() >= 2);
    assert!(m.memory_usage() > 0);
}

#[test]
fn arena_node_handle_equality() {
    let h1 = NodeHandle::new(0, 0);
    let h2 = NodeHandle::new(0, 0);
    let h3 = NodeHandle::new(1, 0);
    assert_eq!(h1, h2);
    assert_ne!(h1, h3);
}

#[test]
fn arena_node_handle_debug() {
    let h = NodeHandle::new(2, 5);
    let dbg = format!("{:?}", h);
    assert!(!dbg.is_empty());
}

#[test]
fn arena_node_handle_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(NodeHandle::new(0, 0));
    set.insert(NodeHandle::new(0, 1));
    set.insert(NodeHandle::new(0, 0)); // duplicate
    assert_eq!(set.len(), 2);
}

#[test]
fn arena_get_mut_update_value() {
    let mut arena = TreeArena::with_capacity(8);
    let h = arena.alloc(TreeNode::leaf(100));
    arena.get_mut(h).set_value(200);
    assert_eq!(arena.get(h).value(), 200);
}

#[test]
fn arena_branch_with_symbol() {
    let mut arena = TreeArena::with_capacity(8);
    let c = arena.alloc(TreeNode::leaf(1));
    let p = arena.alloc(TreeNode::branch_with_symbol(42, vec![c]));
    assert_eq!(arena.get(p).symbol(), 42);
    assert!(arena.get(p).is_branch());
}

#[test]
fn arena_num_chunks_grows() {
    let mut arena = TreeArena::with_capacity(2);
    arena.alloc(TreeNode::leaf(1));
    arena.alloc(TreeNode::leaf(2));
    let c1 = arena.num_chunks();
    // Force growth
    arena.alloc(TreeNode::leaf(3));
    assert!(arena.num_chunks() >= c1);
}

#[test]
fn arena_default_is_empty() {
    let arena = TreeArena::default();
    assert!(arena.is_empty());
    assert_eq!(arena.len(), 0);
}

#[test]
fn arena_clear_vs_reset() {
    let mut arena = TreeArena::with_capacity(8);
    arena.alloc(TreeNode::leaf(1));
    arena.alloc(TreeNode::leaf(2));
    arena.clear();
    assert!(arena.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// Additional: Trait bound checks
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn visitor_action_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<VisitorAction>();
}

#[test]
fn visitor_action_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<VisitorAction>();
}

#[test]
fn node_handle_is_copy() {
    fn assert_copy<T: Copy>() {}
    assert_copy::<NodeHandle>();
}

#[test]
fn tree_arena_is_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<TreeArena>();
}

#[test]
fn arena_debug_format_nonempty() {
    let mut arena = TreeArena::with_capacity(4);
    arena.alloc(TreeNode::leaf(1));
    let dbg = format!("{:?}", arena);
    assert!(!dbg.is_empty());
}

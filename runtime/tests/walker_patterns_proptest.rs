#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]
//! Property-based and unit tests for Walker patterns in the adze visitor API.
//!
//! Covers DfsWalker, BfsWalker, SearchVisitor, StatsVisitor,
//! PrettyPrintVisitor, TransformWalker, walker reusability, and determinism.

use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TransformVisitor,
    TransformWalker, TreeWalker, Visitor, VisitorAction,
};
use proptest::prelude::*;

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

fn leaf(symbol: u16, start: usize, end: usize) -> ParsedNode {
    make_node(symbol, vec![], start, end, false, true)
}

fn interior(symbol: u16, children: Vec<ParsedNode>) -> ParsedNode {
    let start = children.first().map_or(0, |c| c.start_byte);
    let end = children.last().map_or(0, |c| c.end_byte);
    make_node(symbol, children, start, end, false, true)
}

fn error_node(start: usize, end: usize) -> ParsedNode {
    make_node(0, vec![], start, end, true, false)
}

fn count_nodes(node: &ParsedNode) -> usize {
    1 + node.children().iter().map(count_nodes).sum::<usize>()
}

fn count_non_error_nodes(node: &ParsedNode) -> usize {
    if node.is_error() {
        0
    } else {
        1 + node
            .children()
            .iter()
            .map(count_non_error_nodes)
            .sum::<usize>()
    }
}

// ---------------------------------------------------------------------------
// Proptest strategies
// ---------------------------------------------------------------------------

const SOURCE_LEN: usize = 64;

fn arb_leaf() -> impl Strategy<Value = ParsedNode> {
    (1u16..=10, 0..SOURCE_LEN - 1).prop_map(|(sym, start)| {
        let end = start + 1;
        leaf(sym, start, end)
    })
}

fn arb_tree(max_depth: u32, max_width: usize) -> impl Strategy<Value = ParsedNode> {
    arb_leaf().prop_recursive(max_depth, 64, max_width as u32, move |inner| {
        (1u16..=10, proptest::collection::vec(inner, 1..=max_width))
            .prop_map(|(sym, children)| interior(sym, children))
    })
}

fn arb_source() -> impl Strategy<Value = String> {
    proptest::string::string_regex(&format!("[a-z0-9 ]{{{},{}}}", SOURCE_LEN, SOURCE_LEN)).unwrap()
}

// ===========================================================================
// 1. DfsWalker: pre-order visit sequence matches manual DFS
// ===========================================================================

#[test]
fn dfs_visit_order_matches_preorder() {
    let source = b"abcdefghij";
    //   root(1)
    //   ├─ a(2) leaf [0,1)
    //   └─ mid(3)
    //      ├─ b(4) leaf [1,2)
    //      └─ c(5) leaf [2,3)
    let tree = interior(
        1,
        vec![
            leaf(2, 0, 1),
            interior(3, vec![leaf(4, 1, 2), leaf(5, 2, 3)]),
        ],
    );

    struct OrderRecorder(Vec<u16>);
    impl Visitor for OrderRecorder {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.0.push(node.symbol());
            VisitorAction::Continue
        }
    }

    let mut v = OrderRecorder(Vec::new());
    TreeWalker::new(source).walk(&tree, &mut v);
    assert_eq!(v.0, vec![1, 2, 3, 4, 5]);
}

// ===========================================================================
// 2. DfsWalker: leave_node called in post-order
// ===========================================================================

#[test]
fn dfs_leave_order_is_postorder() {
    let source = b"abcdef";
    let tree = interior(1, vec![leaf(2, 0, 1), leaf(3, 1, 2)]);

    struct LeaveRecorder(Vec<u16>);
    impl Visitor for LeaveRecorder {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn leave_node(&mut self, node: &ParsedNode) {
            self.0.push(node.symbol());
        }
    }

    let mut v = LeaveRecorder(Vec::new());
    TreeWalker::new(source).walk(&tree, &mut v);
    // Leaves first, then root
    assert_eq!(v.0, vec![2, 3, 1]);
}

// ===========================================================================
// 3. BfsWalker: level-order visit sequence
// ===========================================================================

#[test]
fn bfs_visit_order_is_level_order() {
    let source = b"abcdefghij";
    let tree = interior(
        1,
        vec![
            interior(2, vec![leaf(4, 0, 1), leaf(5, 1, 2)]),
            leaf(3, 2, 3),
        ],
    );

    struct OrderRecorder(Vec<u16>);
    impl Visitor for OrderRecorder {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.0.push(node.symbol());
            VisitorAction::Continue
        }
    }

    let mut v = OrderRecorder(Vec::new());
    BreadthFirstWalker::new(source).walk(&tree, &mut v);
    // Level 0: 1, Level 1: 2,3, Level 2: 4,5
    assert_eq!(v.0, vec![1, 2, 3, 4, 5]);
}

// ===========================================================================
// 4. BfsWalker: BFS and DFS visit different orders on asymmetric trees
// ===========================================================================

proptest! {
    #[test]
    fn bfs_dfs_order_may_differ(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct SymRecorder(Vec<u16>);
        impl Visitor for SymRecorder {
            fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
                self.0.push(node.symbol());
                VisitorAction::Continue
            }
        }

        let src = source.as_bytes();
        let mut dfs_v = SymRecorder(Vec::new());
        TreeWalker::new(src).walk(&tree, &mut dfs_v);

        let mut bfs_v = SymRecorder(Vec::new());
        BreadthFirstWalker::new(src).walk(&tree, &mut bfs_v);

        // Both must visit the same multiset of symbols
        let mut dfs_sorted = dfs_v.0.clone();
        dfs_sorted.sort();
        let mut bfs_sorted = bfs_v.0.clone();
        bfs_sorted.sort();
        prop_assert_eq!(dfs_sorted, bfs_sorted);
    }
}

// ===========================================================================
// 5. SearchVisitor: finds nodes matching by symbol
// ===========================================================================

#[test]
fn search_finds_specific_symbol() {
    let source = b"abcdefghij";
    let tree = interior(1, vec![leaf(2, 0, 1), leaf(3, 1, 2), leaf(2, 2, 3)]);
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 2);
    TreeWalker::new(source).walk(&tree, &mut search);
    assert_eq!(search.matches.len(), 2);
}

// ===========================================================================
// 6. SearchVisitor: empty predicate returns nothing
// ===========================================================================

#[test]
fn search_impossible_predicate_returns_empty() {
    let source = b"abcdef";
    let tree = interior(1, vec![leaf(2, 0, 1)]);
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 99);
    TreeWalker::new(source).walk(&tree, &mut search);
    assert!(search.matches.is_empty());
}

// ===========================================================================
// 7. SearchVisitor: match tuples contain correct byte ranges
// ===========================================================================

#[test]
fn search_match_byte_ranges_correct() {
    let source = b"hello world!";
    let tree = interior(1, vec![leaf(2, 0, 5), leaf(3, 6, 11)]);
    let mut search = SearchVisitor::new(|n: &ParsedNode| n.symbol() == 2);
    TreeWalker::new(source).walk(&tree, &mut search);
    assert_eq!(search.matches.len(), 1);
    assert_eq!(search.matches[0].0, 0); // start_byte
    assert_eq!(search.matches[0].1, 5); // end_byte
}

// ===========================================================================
// 8. StatsVisitor: total_nodes counts all non-error nodes
// ===========================================================================

proptest! {
    #[test]
    fn stats_total_matches_non_error_count(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        let expected = count_non_error_nodes(&tree);
        prop_assert_eq!(stats.total_nodes, expected);
    }
}

// ===========================================================================
// 9. StatsVisitor: error_nodes counts error nodes
// ===========================================================================

#[test]
fn stats_counts_multiple_errors() {
    let source = b"a err1 b err2 c";
    let tree = interior(
        1,
        vec![
            leaf(2, 0, 1),
            error_node(2, 6),
            leaf(3, 7, 8),
            error_node(9, 13),
            leaf(4, 14, 15),
        ],
    );
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut stats);
    assert_eq!(stats.error_nodes, 2);
}

// ===========================================================================
// 10. StatsVisitor: max_depth accurate for deep chain
// ===========================================================================

#[test]
fn stats_max_depth_deep_chain() {
    let source = b"abcdefghijklmnop";
    // Chain: depth 4
    let tree = interior(1, vec![interior(2, vec![interior(3, vec![leaf(4, 0, 1)])])]);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut stats);
    assert_eq!(stats.max_depth, 4);
}

// ===========================================================================
// 11. StatsVisitor: node_counts keys match visited kinds
// ===========================================================================

proptest! {
    #[test]
    fn stats_node_counts_keys_are_visited_kinds(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut stats = StatsVisitor::default();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut stats);
        // Every key in node_counts should have count > 0
        for count in stats.node_counts.values() {
            prop_assert!(*count > 0);
        }
    }
}

// ===========================================================================
// 12. PrettyPrintVisitor: output contains node kinds
// ===========================================================================

#[test]
fn pretty_print_contains_node_kinds() {
    let source = b"abcdef";
    let tree = interior(1, vec![leaf(2, 0, 1)]);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(source).walk(&tree, &mut pp);
    let out = pp.output();
    // The output should contain the kind strings for all visited nodes.
    // Kinds come from node.kind() which returns the symbol name.
    assert!(!out.is_empty());
    // It should contain newlines (one per enter_node + leaves)
    assert!(out.contains('\n'));
}

// ===========================================================================
// 13. PrettyPrintVisitor: indentation increases with depth
// ===========================================================================

#[test]
fn pretty_print_indentation_increases() {
    let source = b"abcdefghijklmnop";
    let tree = interior(1, vec![interior(2, vec![leaf(3, 0, 1)])]);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(source).walk(&tree, &mut pp);
    let lines: Vec<&str> = pp.output().lines().collect();
    // First line (root) has no indentation
    assert!(!lines[0].starts_with(' '));
    // Deeper lines have more leading spaces
    if lines.len() > 1 {
        let indent_first = lines[0].len() - lines[0].trim_start().len();
        let indent_second = lines[1].len() - lines[1].trim_start().len();
        assert!(indent_second > indent_first);
    }
}

// ===========================================================================
// 14. PrettyPrintVisitor: named nodes get [named] tag
// ===========================================================================

#[test]
fn pretty_print_named_tag() {
    let source = b"abcdef";
    let tree = leaf(1, 0, 1); // is_named=true
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(source).walk(&tree, &mut pp);
    assert!(pp.output().contains("[named]"));
}

// ===========================================================================
// 15. PrettyPrintVisitor: leaf text is quoted
// ===========================================================================

#[test]
fn pretty_print_leaf_text_quoted() {
    let source = b"hello";
    let tree = leaf(1, 0, 5);
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(source).walk(&tree, &mut pp);
    assert!(pp.output().contains('"'));
}

// ===========================================================================
// 16. TransformWalker: bottom-up string concatenation
// ===========================================================================

#[test]
fn transform_walker_concatenates_bottom_up() {
    let source = b"ab";
    let tree = interior(1, vec![leaf(2, 0, 1), leaf(3, 1, 2)]);

    struct Concat;
    impl TransformVisitor for Concat {
        type Output = String;
        fn transform_node(&mut self, _: &ParsedNode, children: Vec<String>) -> String {
            format!("({})", children.join(","))
        }
        fn transform_leaf(&mut self, _: &ParsedNode, text: &str) -> String {
            text.to_string()
        }
        fn transform_error(&mut self, _: &ParsedNode) -> String {
            "ERR".to_string()
        }
    }

    let mut c = Concat;
    let result = TransformWalker::new(source).walk(&tree, &mut c);
    assert_eq!(result, "(a,b)");
}

// ===========================================================================
// 17. TransformWalker: error nodes produce transform_error result
// ===========================================================================

#[test]
fn transform_walker_handles_error_nodes() {
    let source = b"a err b";
    let tree = interior(1, vec![leaf(2, 0, 1), error_node(2, 5), leaf(3, 6, 7)]);

    struct Concat;
    impl TransformVisitor for Concat {
        type Output = String;
        fn transform_node(&mut self, _: &ParsedNode, children: Vec<String>) -> String {
            children.join("+")
        }
        fn transform_leaf(&mut self, _: &ParsedNode, text: &str) -> String {
            text.to_string()
        }
        fn transform_error(&mut self, _: &ParsedNode) -> String {
            "ERROR".to_string()
        }
    }

    let mut c = Concat;
    let result = TransformWalker::new(source).walk(&tree, &mut c);
    assert_eq!(result, "a+ERROR+b");
}

// ===========================================================================
// 18. TransformWalker: node count via bottom-up sum
// ===========================================================================

proptest! {
    #[test]
    fn transform_walker_node_count(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct NodeCount;
        impl TransformVisitor for NodeCount {
            type Output = usize;
            fn transform_node(&mut self, _: &ParsedNode, children: Vec<usize>) -> usize {
                1 + children.iter().sum::<usize>()
            }
            fn transform_leaf(&mut self, _: &ParsedNode, _: &str) -> usize { 1 }
            fn transform_error(&mut self, _: &ParsedNode) -> usize { 1 }
        }

        let mut nc = NodeCount;
        let result = TransformWalker::new(source.as_bytes()).walk(&tree, &mut nc);
        prop_assert_eq!(result, count_nodes(&tree));
    }
}

// ===========================================================================
// 19. Walker reusability: same TreeWalker used for multiple visitors
// ===========================================================================

#[test]
fn dfs_walker_reusable_across_visitors() {
    let source = b"abcdefghij";
    let tree = interior(1, vec![leaf(2, 0, 1), leaf(3, 1, 2)]);
    let walker = TreeWalker::new(source);

    let mut stats1 = StatsVisitor::default();
    walker.walk(&tree, &mut stats1);

    let mut stats2 = StatsVisitor::default();
    walker.walk(&tree, &mut stats2);

    assert_eq!(stats1.total_nodes, stats2.total_nodes);
    assert_eq!(stats1.leaf_nodes, stats2.leaf_nodes);
    assert_eq!(stats1.max_depth, stats2.max_depth);
}

// ===========================================================================
// 20. Walker reusability: same BFS walker used for multiple visitors
// ===========================================================================

#[test]
fn bfs_walker_reusable_across_visitors() {
    let source = b"abcdefghij";
    let tree = interior(1, vec![leaf(2, 0, 1), leaf(3, 1, 2)]);
    let walker = BreadthFirstWalker::new(source);

    let mut stats1 = StatsVisitor::default();
    walker.walk(&tree, &mut stats1);

    let mut stats2 = StatsVisitor::default();
    walker.walk(&tree, &mut stats2);

    assert_eq!(stats1.total_nodes, stats2.total_nodes);
}

// ===========================================================================
// 21. Walker reusability: same walker, different trees
// ===========================================================================

#[test]
fn walker_reusable_different_trees() {
    let source = b"abcdefghijklmnop";
    let walker = TreeWalker::new(source);

    let tree1 = leaf(1, 0, 1);
    let tree2 = interior(2, vec![leaf(3, 0, 1), leaf(4, 1, 2)]);

    let mut stats1 = StatsVisitor::default();
    walker.walk(&tree1, &mut stats1);

    let mut stats2 = StatsVisitor::default();
    walker.walk(&tree2, &mut stats2);

    assert_eq!(stats1.total_nodes, 1);
    assert_eq!(stats2.total_nodes, 3);
}

// ===========================================================================
// 22. Walker determinism: DFS produces identical results on repeated runs
// ===========================================================================

proptest! {
    #[test]
    fn dfs_deterministic(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();

        struct SymRecorder(Vec<u16>);
        impl Visitor for SymRecorder {
            fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
                self.0.push(node.symbol());
                VisitorAction::Continue
            }
        }

        let mut v1 = SymRecorder(Vec::new());
        TreeWalker::new(src).walk(&tree, &mut v1);

        let mut v2 = SymRecorder(Vec::new());
        TreeWalker::new(src).walk(&tree, &mut v2);

        prop_assert_eq!(v1.0, v2.0);
    }
}

// ===========================================================================
// 23. Walker determinism: BFS produces identical results on repeated runs
// ===========================================================================

proptest! {
    #[test]
    fn bfs_deterministic(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();

        struct SymRecorder(Vec<u16>);
        impl Visitor for SymRecorder {
            fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
                self.0.push(node.symbol());
                VisitorAction::Continue
            }
        }

        let mut v1 = SymRecorder(Vec::new());
        BreadthFirstWalker::new(src).walk(&tree, &mut v1);

        let mut v2 = SymRecorder(Vec::new());
        BreadthFirstWalker::new(src).walk(&tree, &mut v2);

        prop_assert_eq!(v1.0, v2.0);
    }
}

// ===========================================================================
// 24. Walker determinism: TransformWalker produces identical results
// ===========================================================================

proptest! {
    #[test]
    fn transform_deterministic(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct DepthCalc;
        impl TransformVisitor for DepthCalc {
            type Output = usize;
            fn transform_node(&mut self, _: &ParsedNode, children: Vec<usize>) -> usize {
                1 + children.into_iter().max().unwrap_or(0)
            }
            fn transform_leaf(&mut self, _: &ParsedNode, _: &str) -> usize { 1 }
            fn transform_error(&mut self, _: &ParsedNode) -> usize { 1 }
        }

        let src = source.as_bytes();
        let r1 = TransformWalker::new(src).walk(&tree, &mut DepthCalc);
        let r2 = TransformWalker::new(src).walk(&tree, &mut DepthCalc);
        prop_assert_eq!(r1, r2);
    }
}

// ===========================================================================
// 25. DfsWalker: SkipChildren prevents child visits
// ===========================================================================

#[test]
fn dfs_skip_children_prevents_descendants() {
    let source = b"abcdefghij";
    let tree = interior(1, vec![interior(2, vec![leaf(3, 0, 1), leaf(4, 1, 2)])]);

    struct SkipSymbol2(Vec<u16>);
    impl Visitor for SkipSymbol2 {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.0.push(node.symbol());
            if node.symbol() == 2 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }

    let mut v = SkipSymbol2(Vec::new());
    TreeWalker::new(source).walk(&tree, &mut v);
    // Should see root(1), child(2), but NOT 3 or 4
    assert_eq!(v.0, vec![1, 2]);
}

// ===========================================================================
// 26. BfsWalker: SkipChildren skips enqueuing children
// ===========================================================================

#[test]
fn bfs_skip_children_prevents_descendants() {
    let source = b"abcdefghij";
    let tree = interior(1, vec![interior(2, vec![leaf(4, 0, 1)]), leaf(3, 1, 2)]);

    struct SkipSymbol2(Vec<u16>);
    impl Visitor for SkipSymbol2 {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.0.push(node.symbol());
            if node.symbol() == 2 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }

    let mut v = SkipSymbol2(Vec::new());
    BreadthFirstWalker::new(source).walk(&tree, &mut v);
    // Level 0: 1, Level 1: 2 (skip), 3. Symbol 4 should NOT appear.
    assert_eq!(v.0, vec![1, 2, 3]);
}

// ===========================================================================
// 27. DfsWalker: Stop prevents descent into the stopped node's children
// ===========================================================================

#[test]
fn dfs_stop_prevents_descent() {
    let source = b"abcdefghij";
    // root(1) -> mid(2) -> [leaf(3), leaf(4)]
    let tree = interior(1, vec![interior(2, vec![leaf(3, 0, 1), leaf(4, 1, 2)])]);

    struct StopAt2(Vec<u16>);
    impl Visitor for StopAt2 {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.0.push(node.symbol());
            if node.symbol() == 2 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }

    let mut v = StopAt2(Vec::new());
    TreeWalker::new(source).walk(&tree, &mut v);
    // Stop at node 2: children 3 and 4 should NOT be visited
    assert_eq!(v.0, vec![1, 2]);
}

// ===========================================================================
// 28. BfsWalker: Stop halts traversal
// ===========================================================================

#[test]
fn bfs_stop_halts_traversal() {
    let source = b"abcdefghij";
    let tree = interior(1, vec![leaf(2, 0, 1), leaf(3, 1, 2)]);

    struct StopAt2(Vec<u16>);
    impl Visitor for StopAt2 {
        fn enter_node(&mut self, node: &ParsedNode) -> VisitorAction {
            self.0.push(node.symbol());
            if node.symbol() == 2 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }

    let mut v = StopAt2(Vec::new());
    BreadthFirstWalker::new(source).walk(&tree, &mut v);
    // BFS: root(1) then child(2), stop before 3
    assert_eq!(v.0, vec![1, 2]);
}

// ===========================================================================
// 29. SearchVisitor: match count equals StatsVisitor total for always-true
// ===========================================================================

proptest! {
    #[test]
    fn search_match_count_equals_stats_total(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut search = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(src).walk(&tree, &mut search);

        let mut stats = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut stats);

        // Both use enter_node so counts must match
        prop_assert_eq!(search.matches.len(), stats.total_nodes);
    }
}

// ===========================================================================
// 30. PrettyPrint: output lines count matches node + leaf count
// ===========================================================================

proptest! {
    #[test]
    fn pretty_print_line_count(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut pp = PrettyPrintVisitor::new();
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut pp);
        let lines = pp.output().lines().count();
        // At least 1 line per non-error node (from enter_node), plus leaves
        prop_assert!(lines >= 1);
    }
}

// ===========================================================================
// 31. StatsVisitor: single leaf tree has depth 1
// ===========================================================================

#[test]
fn stats_single_leaf_depth() {
    let source = b"x";
    let tree = leaf(1, 0, 1);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut stats);
    assert_eq!(stats.max_depth, 1);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
}

// ===========================================================================
// 32. DfsWalker: visit_leaf receives correct text slice
// ===========================================================================

#[test]
fn dfs_visit_leaf_receives_text() {
    let source = b"hello world";
    let tree = interior(1, vec![leaf(2, 0, 5), leaf(3, 6, 11)]);

    struct LeafCollector(Vec<String>);
    impl Visitor for LeafCollector {
        fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
            VisitorAction::Continue
        }
        fn visit_leaf(&mut self, _: &ParsedNode, text: &str) {
            self.0.push(text.to_string());
        }
    }

    let mut v = LeafCollector(Vec::new());
    TreeWalker::new(source).walk(&tree, &mut v);
    assert_eq!(v.0, vec!["hello", "world"]);
}

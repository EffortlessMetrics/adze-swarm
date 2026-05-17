#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]
//! Property-based tests for tree operations (traversal, searching, counting)
//! in the adze runtime.

use adze::pure_parser::ParsedNode;
use adze::visitor::{
    BreadthFirstWalker, PrettyPrintVisitor, SearchVisitor, StatsVisitor, TransformVisitor,
    TransformWalker, TreeWalker, Visitor, VisitorAction,
};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

/// Count every node in a tree recursively (including the root).
fn count_nodes(node: &ParsedNode) -> usize {
    1 + node.children().iter().map(count_nodes).sum::<usize>()
}

/// Compute tree depth (root-only = 1).
fn tree_depth(node: &ParsedNode) -> usize {
    if node.children().is_empty() {
        1
    } else {
        1 + node.children().iter().map(tree_depth).max().unwrap_or(0)
    }
}

/// Count leaf nodes in a tree.
fn count_leaves(node: &ParsedNode) -> usize {
    if node.children().is_empty() {
        1
    } else {
        node.children().iter().map(count_leaves).sum()
    }
}

/// Count non-error nodes reachable (error nodes stop traversal).
fn count_non_error_reachable(node: &ParsedNode) -> usize {
    if node.is_error() {
        0
    } else {
        1 + node
            .children()
            .iter()
            .map(count_non_error_reachable)
            .sum::<usize>()
    }
}

/// Collect all kinds in DFS pre-order (skipping error subtrees).
fn collect_kinds_dfs(node: &ParsedNode, out: &mut Vec<String>) {
    if node.is_error() {
        return;
    }
    out.push(node.kind().to_string());
    for child in node.children() {
        collect_kinds_dfs(child, out);
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

/// Strategy that produces trees with a mix of named and anonymous leaves.
fn arb_mixed_tree(max_depth: u32, max_width: usize) -> impl Strategy<Value = ParsedNode> {
    let base =
        (1u16..=10, 0..SOURCE_LEN - 1, proptest::bool::ANY).prop_map(|(sym, start, named)| {
            if named {
                leaf(sym, start, start + 1)
            } else {
                anon_leaf(sym, start, start + 1)
            }
        });
    base.prop_recursive(max_depth, 64, max_width as u32, move |inner| {
        (1u16..=10, proptest::collection::vec(inner, 1..=max_width))
            .prop_map(|(sym, children)| interior(sym, children))
    })
}

// =========================================================================
// Tests
// =========================================================================

// ---------------------------------------------------------------------------
// 1. Tree depth of a single leaf is 1
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn depth_of_leaf_is_one(sym in 1u16..=10, start in 0usize..SOURCE_LEN - 1) {
        let node = leaf(sym, start, start + 1);
        prop_assert_eq!(tree_depth(&node), 1);
    }
}

// ---------------------------------------------------------------------------
// 2. Tree depth matches TransformWalker computation
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn depth_via_transform_matches_recursive(
        source in arb_source(),
        tree in arb_tree(4, 3),
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

        let tw = TransformWalker::new(source.as_bytes());
        let mut dc = DepthCalc;
        let computed = tw.walk(&tree, &mut dc);
        prop_assert_eq!(computed, tree_depth(&tree));
    }
}

// ---------------------------------------------------------------------------
// 3. Node count of single leaf is 1
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn node_count_of_leaf_is_one(sym in 1u16..=10, start in 0usize..SOURCE_LEN - 1) {
        let node = leaf(sym, start, start + 1);
        prop_assert_eq!(count_nodes(&node), 1);
    }
}

// ---------------------------------------------------------------------------
// 4. StatsVisitor total_nodes equals non-error reachable count
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn stats_total_equals_non_error_reachable(
        source in arb_source(),
        tree in arb_tree(4, 3),
    ) {
        let walker = TreeWalker::new(source.as_bytes());
        let mut stats = StatsVisitor::default();
        walker.walk(&tree, &mut stats);
        let expected = count_non_error_reachable(&tree);
        prop_assert_eq!(stats.total_nodes, expected);
    }
}

// ---------------------------------------------------------------------------
// 5. Leaf count via TransformWalker matches recursive count
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn leaf_count_via_transform(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct CountLeaves;
        impl TransformVisitor for CountLeaves {
            type Output = usize;
            fn transform_node(&mut self, _: &ParsedNode, children: Vec<usize>) -> usize {
                children.iter().sum()
            }
            fn transform_leaf(&mut self, _: &ParsedNode, _: &str) -> usize { 1 }
            fn transform_error(&mut self, _: &ParsedNode) -> usize { 0 }
        }

        let tw = TransformWalker::new(source.as_bytes());
        let mut cl = CountLeaves;
        let computed = tw.walk(&tree, &mut cl);
        prop_assert_eq!(computed, count_leaves(&tree));
    }
}

// ---------------------------------------------------------------------------
// 6. Leaf count <= total node count
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn leaf_count_le_total(tree in arb_tree(4, 4)) {
        prop_assert!(count_leaves(&tree) <= count_nodes(&tree));
    }
}

// ---------------------------------------------------------------------------
// 7. Child iteration order preserved by ChildWalker
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn child_walker_order_matches_children_slice(
        tree in arb_tree(2, 5),
    ) {
        let expected_syms: Vec<u16> = tree.children().iter().map(|c| c.symbol()).collect();
        let mut walker = tree.walk();
        let mut walker_syms = vec![];
        if walker.goto_first_child() {
            loop {
                walker_syms.push(walker.node().symbol());
                if !walker.goto_next_sibling() {
                    break;
                }
            }
        }
        prop_assert_eq!(walker_syms, expected_syms);
    }
}

// ---------------------------------------------------------------------------
// 8. Child iteration via index matches slice
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn child_index_access_matches_slice(
        tree in arb_tree(2, 5),
    ) {
        let children = tree.children();
        for i in 0..children.len() {
            let by_index = tree.child(i).unwrap();
            prop_assert_eq!(by_index.symbol(), children[i].symbol());
            prop_assert_eq!(by_index.start_byte(), children[i].start_byte());
        }
        prop_assert!(tree.child(children.len()).is_none());
    }
}

// ---------------------------------------------------------------------------
// 9. SearchVisitor find by kind finds correct nodes
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn search_by_kind_finds_correct(
        source in arb_source(),
        tree in arb_tree(3, 3),
        target_sym in 1u16..=10,
    ) {
        let target_kind = match target_sym {
            0 => "end",
            1 => "*",
            2 => "_2",
            3 => "_6",
            4 => "-",
            5 => "Expression",
            6 => "Whitespace__whitespace",
            7 => "Whitespace",
            8 => "Expression_Sub_1",
            9 => "Expression_Sub",
            10 => "rule_10",
            _ => "unknown",
        };

        let walker = TreeWalker::new(source.as_bytes());
        let tk = target_kind.to_string();
        let mut search = SearchVisitor::new(move |n: &ParsedNode| n.kind() == tk);
        walker.walk(&tree, &mut search);

        for m in &search.matches {
            prop_assert_eq!(&m.2, target_kind);
        }
    }
}

// ---------------------------------------------------------------------------
// 10. SearchVisitor find by position range
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn search_by_position_range(
        source in arb_source(),
        tree in arb_tree(3, 3),
        range_start in 0usize..SOURCE_LEN / 2,
    ) {
        let range_end = range_start + SOURCE_LEN / 4;
        let rs = range_start;
        let re = range_end;
        let walker = TreeWalker::new(source.as_bytes());
        let mut search = SearchVisitor::new(move |n: &ParsedNode| {
            n.start_byte() >= rs && n.end_byte() <= re
        });
        walker.walk(&tree, &mut search);

        for m in &search.matches {
            prop_assert!(m.0 >= range_start);
            prop_assert!(m.1 <= range_end);
        }
    }
}

// ---------------------------------------------------------------------------
// 11. Root access: root is the node itself
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn root_symbol_preserved(tree in arb_tree(3, 3)) {
        prop_assert_eq!(tree.symbol(), tree.symbol);
        prop_assert_eq!(tree.start_byte(), tree.start_byte);
        prop_assert_eq!(tree.end_byte(), tree.end_byte);
    }
}

// ---------------------------------------------------------------------------
// 12. Root child_count matches children slice length
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn root_child_count_matches_slice(tree in arb_tree(3, 4)) {
        prop_assert_eq!(tree.child_count(), tree.children().len());
    }
}

// ---------------------------------------------------------------------------
// 13. Tree string representation via PrettyPrintVisitor is non-empty
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn string_repr_non_empty(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let walker = TreeWalker::new(source.as_bytes());
        let mut pp = PrettyPrintVisitor::new();
        walker.walk(&tree, &mut pp);
        prop_assert!(!pp.output().is_empty());
    }
}

// ---------------------------------------------------------------------------
// 14. PrettyPrint contains the root kind
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn string_repr_contains_root_kind(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let walker = TreeWalker::new(source.as_bytes());
        let mut pp = PrettyPrintVisitor::new();
        walker.walk(&tree, &mut pp);
        let root_kind = tree.kind();
        prop_assert!(
            pp.output().contains(root_kind),
            "output should contain root kind '{}', got:\n{}",
            root_kind,
            pp.output()
        );
    }
}

// ---------------------------------------------------------------------------
// 15. PrettyPrint line count >= node count (each node emits at least one line)
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn string_repr_lines_ge_nodes(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let walker = TreeWalker::new(source.as_bytes());
        let mut pp = PrettyPrintVisitor::new();
        walker.walk(&tree, &mut pp);
        let line_count = pp.output().lines().count();
        let reachable = count_non_error_reachable(&tree);
        // Each non-error node produces at least one line (enter_node writes kind,
        // and leaves additionally get a text line). So lines >= reachable.
        prop_assert!(
            line_count >= reachable,
            "lines {} < reachable {}",
            line_count,
            reachable,
        );
    }
}

// ---------------------------------------------------------------------------
// 16. StatsVisitor max_depth <= recursive tree_depth
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn stats_depth_bounded(
        source in arb_source(),
        tree in arb_tree(4, 4),
    ) {
        let walker = TreeWalker::new(source.as_bytes());
        let mut stats = StatsVisitor::default();
        walker.walk(&tree, &mut stats);
        prop_assert!(stats.max_depth <= tree_depth(&tree));
    }
}

// ---------------------------------------------------------------------------
// 17. StatsVisitor node_counts values sum to total_nodes
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn stats_counts_sum(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let walker = TreeWalker::new(source.as_bytes());
        let mut stats = StatsVisitor::default();
        walker.walk(&tree, &mut stats);
        let sum: usize = stats.node_counts.values().sum();
        prop_assert_eq!(sum, stats.total_nodes);
    }
}

// ---------------------------------------------------------------------------
// 18. DFS and BFS visit same total node count
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn dfs_bfs_same_total(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut dfs = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut dfs);
        let mut bfs = StatsVisitor::default();
        BreadthFirstWalker::new(src).walk(&tree, &mut bfs);
        prop_assert_eq!(dfs.total_nodes, bfs.total_nodes);
    }
}

// ---------------------------------------------------------------------------
// 19. SearchVisitor always-true matches equal total_nodes
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn search_all_matches_total(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let src = source.as_bytes();
        let mut search = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(src).walk(&tree, &mut search);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut stats);
        prop_assert_eq!(search.matches.len(), stats.total_nodes);
    }
}

// ---------------------------------------------------------------------------
// 20. SearchVisitor always-false finds nothing
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn search_none_finds_nothing(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut search = SearchVisitor::new(|_: &ParsedNode| false);
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut search);
        prop_assert!(search.matches.is_empty());
    }
}

// ---------------------------------------------------------------------------
// 21. DFS enter/leave are balanced
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn dfs_enter_leave_balanced(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct Balance { enters: usize, leaves: usize }
        impl Visitor for Balance {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.enters += 1;
                VisitorAction::Continue
            }
            fn leave_node(&mut self, _: &ParsedNode) {
                self.leaves += 1;
            }
        }
        let mut v = Balance { enters: 0, leaves: 0 };
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut v);
        prop_assert_eq!(v.enters, v.leaves);
    }
}

// ---------------------------------------------------------------------------
// 22. SkipChildren reduces visited count
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn skip_children_reduces_count(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        struct SkipFirst { count: usize, skipped: bool }
        impl Visitor for SkipFirst {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.count += 1;
                if !self.skipped {
                    self.skipped = true;
                    VisitorAction::SkipChildren
                } else {
                    VisitorAction::Continue
                }
            }
        }
        let src = source.as_bytes();
        let mut sv = SkipFirst { count: 0, skipped: false };
        TreeWalker::new(src).walk(&tree, &mut sv);
        let mut stats = StatsVisitor::default();
        TreeWalker::new(src).walk(&tree, &mut stats);
        prop_assert!(sv.count <= stats.total_nodes);
    }
}

// ---------------------------------------------------------------------------
// 23. Stop limits BFS traversal
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn stop_limits_bfs(
        source in arb_source(),
        tree in arb_tree(3, 4),
        limit in 1usize..=5,
    ) {
        struct StopAfter { count: usize, limit: usize }
        impl Visitor for StopAfter {
            fn enter_node(&mut self, _: &ParsedNode) -> VisitorAction {
                self.count += 1;
                if self.count >= self.limit {
                    VisitorAction::Stop
                } else {
                    VisitorAction::Continue
                }
            }
        }
        let mut v = StopAfter { count: 0, limit };
        BreadthFirstWalker::new(source.as_bytes()).walk(&tree, &mut v);
        prop_assert!(v.count <= limit);
    }
}

// ---------------------------------------------------------------------------
// 24. Search by symbol value finds subset
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn search_by_symbol_subset(
        source in arb_source(),
        tree in arb_tree(3, 3),
        threshold in 1u16..=10,
    ) {
        let t = threshold;
        let src = source.as_bytes();
        let mut filtered = SearchVisitor::new(move |n: &ParsedNode| n.symbol() <= t);
        TreeWalker::new(src).walk(&tree, &mut filtered);
        let mut all = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(src).walk(&tree, &mut all);
        prop_assert!(filtered.matches.len() <= all.matches.len());
    }
}

// ---------------------------------------------------------------------------
// 25. Interior node always has depth > 1
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn interior_depth_gt_one(
        tree in arb_tree(3, 3),
    ) {
        if !tree.children().is_empty() {
            prop_assert!(tree_depth(&tree) > 1);
        }
    }
}

// ---------------------------------------------------------------------------
// 26. is_named reflects construction
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn is_named_reflects_construction(
        source in arb_source(),
        tree in arb_mixed_tree(3, 3),
    ) {
        // PrettyPrint tags named nodes with "[named]"
        let walker = TreeWalker::new(source.as_bytes());
        let mut pp = PrettyPrintVisitor::new();
        walker.walk(&tree, &mut pp);
        // If root is named, the first line should contain "[named]"
        if tree.is_named() {
            let first_line = pp.output().lines().next().unwrap_or("");
            prop_assert!(first_line.contains("[named]"));
        }
    }
}

// ---------------------------------------------------------------------------
// 27. Error node increments error_nodes and does not count in total_nodes
// ---------------------------------------------------------------------------

#[test]
fn error_node_counted_separately() {
    let source = b"x err y  ";
    let tree = interior(1, vec![leaf(2, 0, 1), error_node(2, 5), leaf(3, 6, 7)]);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut stats);
    assert_eq!(stats.error_nodes, 1);
    // root + 2 non-error leaves = 3 entered via enter_node
    assert_eq!(stats.total_nodes, 3);
}

// ---------------------------------------------------------------------------
// 28. utf8_text returns correct slice
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn utf8_text_returns_correct_slice(
        source in arb_source(),
    ) {
        let bytes = source.as_bytes();
        let len = bytes.len();
        if len >= 2 {
            let start = 0;
            let end = 2.min(len);
            let node = leaf(1, start, end);
            let text = node.utf8_text(bytes).unwrap();
            prop_assert_eq!(text, &source[start..end]);
        }
    }
}

// ---------------------------------------------------------------------------
// 29. has_error is true when any descendant has error
// ---------------------------------------------------------------------------

#[test]
fn has_error_detects_descendant_error() {
    let tree = interior(
        1,
        vec![
            leaf(2, 0, 1),
            interior(3, vec![error_node(2, 3), leaf(4, 4, 5)]),
        ],
    );
    assert!(tree.has_error());
}

#[test]
fn has_error_false_when_no_errors() {
    let tree = interior(1, vec![leaf(2, 0, 1), leaf(3, 2, 3)]);
    assert!(!tree.has_error());
}

// ---------------------------------------------------------------------------
// 30. kind() returns hardcoded names for known symbols
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn kind_returns_known_or_unknown(sym in 0u16..=15) {
        let node = leaf(sym, 0, 1);
        let k = node.kind();
        if sym <= 10 {
            prop_assert_ne!(k, "unknown");
        } else {
            prop_assert_eq!(k, "unknown");
        }
    }
}

// ---------------------------------------------------------------------------
// 31. DFS visit order matches collect_kinds_dfs
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn dfs_order_matches_manual(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut expected = Vec::new();
        collect_kinds_dfs(&tree, &mut expected);

        struct KindCollector { kinds: Vec<String> }
        impl Visitor for KindCollector {
            fn enter_node(&mut self, n: &ParsedNode) -> VisitorAction {
                self.kinds.push(n.kind().to_string());
                VisitorAction::Continue
            }
        }
        let mut kc = KindCollector { kinds: vec![] };
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut kc);
        prop_assert_eq!(kc.kinds, expected);
    }
}

// ---------------------------------------------------------------------------
// 32. child_count of leaf is 0
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn leaf_child_count_zero(sym in 1u16..=10, start in 0usize..SOURCE_LEN - 1) {
        let node = leaf(sym, start, start + 1);
        prop_assert_eq!(node.child_count(), 0);
        prop_assert!(node.children().is_empty());
    }
}

// ---------------------------------------------------------------------------
// 33. ChildWalker on leaf yields no children
// ---------------------------------------------------------------------------

#[test]
fn child_walker_empty_on_leaf() {
    let node = leaf(1, 0, 1);
    let mut cw = node.walk();
    assert!(!cw.goto_first_child());
}

// ---------------------------------------------------------------------------
// 34. Search matches have valid byte ranges
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn search_matches_have_nonempty_kind(
        source in arb_source(),
        tree in arb_tree(3, 3),
    ) {
        let mut search = SearchVisitor::new(|_: &ParsedNode| true);
        TreeWalker::new(source.as_bytes()).walk(&tree, &mut search);
        for m in &search.matches {
            prop_assert!(!m.2.is_empty(), "kind should be non-empty");
        }
    }
}

// ---------------------------------------------------------------------------
// 35. Single-leaf tree: depth=1, nodes=1, leaves=1
// ---------------------------------------------------------------------------

#[test]
fn single_leaf_invariants() {
    let source = b"hello";
    let tree = leaf(1, 0, 5);
    let mut stats = StatsVisitor::default();
    TreeWalker::new(source).walk(&tree, &mut stats);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.leaf_nodes, 1);
    assert_eq!(stats.max_depth, 1);
    assert_eq!(stats.error_nodes, 0);
}

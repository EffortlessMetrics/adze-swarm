//! Comprehensive edge case tests for Tree and Node structures.
//!
//! This test suite covers:
//! - Stub tree creation and validation
//! - Tree construction with various parameter combinations
//! - TreeCursor navigation on different tree structures
//! - Node metadata access on different tree types
//! - Tree structure integrity (byte ranges, parent-child relationships)
//! - Edge cases: empty trees, single child, many children, deeply nested
//! - Memory and cloning semantics

use adze_runtime::{Tree, tree::TreeCursor};

fn leaf(symbol: u32, start_byte: usize, end_byte: usize) -> Tree {
    Tree::new_for_testing(symbol, start_byte, end_byte, vec![])
}

fn branch(
    symbol: u32,
    start_byte: usize,
    end_byte: usize,
    children: impl IntoIterator<Item = Tree>,
) -> Tree {
    Tree::new_for_testing(symbol, start_byte, end_byte, children.into_iter().collect())
}

fn sibling_leaves(specs: &[(u32, usize, usize)]) -> Vec<Tree> {
    specs
        .iter()
        .map(|&(symbol, start, end)| leaf(symbol, start, end))
        .collect()
}

fn evenly_spaced_leaves(count: usize, width: usize) -> Vec<Tree> {
    (0..count)
        .map(|i| leaf((i as u32) + 2, i * width, (i + 1) * width))
        .collect()
}

fn nested_tree(levels: u32) -> Tree {
    let mut tree = leaf(levels, 0, 1);
    for symbol in (1..levels).rev() {
        tree = branch(symbol, 0, (levels + 1 - symbol) as usize, [tree]);
    }
    tree
}

// ============================================================================
// Test 1: Tree::new_stub() creates valid stub tree
// ============================================================================

#[test]
fn test_stub_tree_creation() {
    let tree = Tree::new_stub();
    let root = tree.root_node();
    assert_eq!(root.kind(), "unknown");
    assert_eq!(root.kind_id(), 0);
    assert_eq!(root.child_count(), 0);
    assert_eq!(root.start_byte(), 0);
    assert_eq!(root.end_byte(), 0);
}

// ============================================================================
// Test 2: Tree::new_for_testing() with various parameters
// ============================================================================

#[test]
fn test_tree_new_for_testing_simple() {
    let tree = leaf(1, 0, 10);
    let root = tree.root_node();
    assert_eq!(root.kind_id(), 1);
    assert_eq!(root.start_byte(), 0);
    assert_eq!(root.end_byte(), 10);
    assert_eq!(root.child_count(), 0);
}

#[test]
fn test_tree_new_for_testing_with_single_child() {
    let child = leaf(2, 0, 5);
    let tree = branch(1, 0, 10, [child]);
    let root = tree.root_node();
    assert_eq!(root.child_count(), 1);
    let child_node = root.child(0).unwrap();
    assert_eq!(child_node.kind_id(), 2);
    assert_eq!(child_node.start_byte(), 0);
    assert_eq!(child_node.end_byte(), 5);
}

#[test]
fn test_tree_new_for_testing_with_multiple_children() {
    let tree = branch(
        1,
        0,
        10,
        sibling_leaves(&[(2, 0, 3), (3, 3, 7), (4, 7, 10)]),
    );
    let root = tree.root_node();
    assert_eq!(root.child_count(), 3);
    assert_eq!(root.child(0).unwrap().kind_id(), 2);
    assert_eq!(root.child(1).unwrap().kind_id(), 3);
    assert_eq!(root.child(2).unwrap().kind_id(), 4);
}

#[test]
fn test_tree_new_for_testing_zero_width_range() {
    let tree = leaf(1, 100, 100);
    let root = tree.root_node();
    assert_eq!(root.start_byte(), 100);
    assert_eq!(root.end_byte(), 100);
    assert_eq!(root.byte_range().len(), 0);
}

#[test]
fn test_tree_new_for_testing_large_byte_range() {
    let tree = leaf(1, 0, 1_000_000);
    let root = tree.root_node();
    assert_eq!(root.start_byte(), 0);
    assert_eq!(root.end_byte(), 1_000_000);
}

// ============================================================================
// Test 3: Tree root_node() returns correct root
// ============================================================================

#[test]
fn test_root_node_identity() {
    let tree = leaf(5, 10, 20);
    let root = tree.root_node();
    assert_eq!(root.kind_id(), 5);
    assert_eq!(root.start_byte(), 10);
    assert_eq!(root.end_byte(), 20);
}

#[test]
fn test_root_node_consistency() {
    let tree = Tree::new_stub();
    let root1 = tree.root_node();
    let root2 = tree.root_node();
    assert_eq!(root1.kind_id(), root2.kind_id());
    assert_eq!(root1.start_byte(), root2.start_byte());
    assert_eq!(root1.end_byte(), root2.end_byte());
}

// ============================================================================
// Test 4: TreeCursor::new() on stub tree
// ============================================================================

#[test]
fn test_cursor_on_stub_tree_new() {
    let tree = Tree::new_stub();
    let cursor = TreeCursor::new(&tree);
    assert_eq!(cursor.depth(), 0);
    assert_eq!(cursor.node().kind_id(), 0);
}

// ============================================================================
// Test 5: TreeCursor traversal on empty tree (stub)
// ============================================================================

#[test]
fn test_cursor_stub_tree_no_children() {
    let tree = Tree::new_stub();
    let mut cursor = TreeCursor::new(&tree);
    assert!(!cursor.goto_first_child());
    assert!(!cursor.goto_next_sibling());
}

#[test]
fn test_cursor_stub_tree_goto_parent_from_root() {
    let tree = Tree::new_stub();
    let mut cursor = TreeCursor::new(&tree);
    assert!(!cursor.goto_parent());
}

#[test]
fn test_cursor_stub_tree_node_still_valid_after_failed_traversal() {
    let tree = Tree::new_stub();
    let mut cursor = TreeCursor::new(&tree);
    let _ = cursor.goto_first_child();
    let node = cursor.node();
    assert_eq!(node.kind_id(), 0);
}

// ============================================================================
// Test 6: TreeCursor depth() at root is 0
// ============================================================================

#[test]
fn test_cursor_depth_at_root() {
    let tree = Tree::new_stub();
    let cursor = TreeCursor::new(&tree);
    assert_eq!(cursor.depth(), 0);
}

#[test]
fn test_cursor_depth_after_goto_first_child() {
    let child = leaf(2, 0, 5);
    let tree = branch(1, 0, 10, [child]);
    let mut cursor = TreeCursor::new(&tree);
    assert_eq!(cursor.depth(), 0);
    cursor.goto_first_child();
    assert_eq!(cursor.depth(), 1);
}

#[test]
fn test_cursor_depth_after_goto_parent() {
    let child = leaf(2, 0, 5);
    let tree = branch(1, 0, 10, [child]);
    let mut cursor = TreeCursor::new(&tree);
    cursor.goto_first_child();
    cursor.goto_parent();
    assert_eq!(cursor.depth(), 0);
}

#[test]
fn test_cursor_depth_deeply_nested() {
    // Create a 4-level deep tree: level 4 <- 3 <- 2 <- 1
    let tree = nested_tree(4);

    let mut cursor = TreeCursor::new(&tree);
    for expected_depth in 1..=3 {
        assert!(cursor.goto_first_child());
        assert_eq!(cursor.depth(), expected_depth);
    }
}

// ============================================================================
// Test 7: TreeCursor node() returns correct node
// ============================================================================

#[test]
fn test_cursor_node_at_root() {
    let tree = leaf(7, 5, 15);
    let cursor = TreeCursor::new(&tree);
    let node = cursor.node();
    assert_eq!(node.kind_id(), 7);
    assert_eq!(node.start_byte(), 5);
    assert_eq!(node.end_byte(), 15);
}

#[test]
fn test_cursor_node_after_traversal() {
    let tree = branch(1, 0, 6, sibling_leaves(&[(2, 0, 3), (3, 3, 6)]));
    let mut cursor = TreeCursor::new(&tree);
    cursor.goto_first_child();
    let node = cursor.node();
    assert_eq!(node.kind_id(), 2);
}

// ============================================================================
// Test 8: TreeCursor reset() works correctly
// ============================================================================

#[test]
fn test_cursor_reset_from_child() {
    let child = leaf(2, 0, 5);
    let tree = branch(1, 0, 10, [child]);
    let mut cursor = TreeCursor::new(&tree);
    cursor.goto_first_child();
    assert_eq!(cursor.depth(), 1);
    cursor.reset(&tree);
    assert_eq!(cursor.depth(), 0);
    assert_eq!(cursor.node().kind_id(), 1);
}

#[test]
fn test_cursor_reset_to_different_tree() {
    let tree1 = leaf(1, 0, 10);
    let tree2 = leaf(2, 0, 20);
    let mut cursor = TreeCursor::new(&tree1);
    assert_eq!(cursor.node().kind_id(), 1);
    cursor.reset(&tree2);
    assert_eq!(cursor.node().kind_id(), 2);
}

// ============================================================================
// Test 9: Node start_byte/end_byte for stub tree
// ============================================================================

#[test]
fn test_node_stub_byte_range() {
    let tree = Tree::new_stub();
    let root = tree.root_node();
    assert_eq!(root.start_byte(), 0);
    assert_eq!(root.end_byte(), 0);
    assert_eq!(root.byte_range(), 0..0);
}

#[test]
fn test_node_byte_range_non_zero() {
    let tree = leaf(1, 100, 200);
    let root = tree.root_node();
    assert_eq!(root.start_byte(), 100);
    assert_eq!(root.end_byte(), 200);
    assert_eq!(root.byte_range(), 100..200);
}

// ============================================================================
// Test 10: Node children count for leaf vs internal
// ============================================================================

#[test]
fn test_node_leaf_has_no_children() {
    let tree = leaf(1, 0, 10);
    let root = tree.root_node();
    assert_eq!(root.child_count(), 0);
    assert_eq!(root.named_child_count(), 0);
}

#[test]
fn test_node_internal_has_children() {
    let tree = branch(1, 0, 10, sibling_leaves(&[(2, 0, 5), (3, 5, 10)]));
    let root = tree.root_node();
    assert_eq!(root.child_count(), 2);
    assert_eq!(root.named_child_count(), 2);
}

// ============================================================================
// Test 11: Tree with single child
// ============================================================================

#[test]
fn test_single_child_traversal() {
    let child = leaf(2, 0, 10);
    let tree = branch(1, 0, 10, [child]);
    let root = tree.root_node();
    assert_eq!(root.child_count(), 1);
    let child_node = root.child(0).unwrap();
    assert_eq!(child_node.kind_id(), 2);
    assert!(root.child(1).is_none());
}

#[test]
fn test_single_child_cursor_traversal() {
    let child = leaf(2, 0, 10);
    let tree = branch(1, 0, 10, [child]);
    let mut cursor = TreeCursor::new(&tree);
    assert!(cursor.goto_first_child());
    assert_eq!(cursor.node().kind_id(), 2);
    assert!(!cursor.goto_next_sibling());
}

// ============================================================================
// Test 12: Tree with many children (50+)
// ============================================================================

#[test]
fn test_many_children() {
    let tree = branch(1, 0, 100, evenly_spaced_leaves(50, 2));
    let root = tree.root_node();
    assert_eq!(root.child_count(), 50);
    for i in 0..50 {
        let child = root.child(i).unwrap();
        assert_eq!(child.kind_id(), (i as u16) + 2);
    }
    assert!(root.child(50).is_none());
}

#[test]
fn test_many_children_cursor_traversal() {
    let tree = branch(1, 0, 90, evenly_spaced_leaves(30, 3));
    let mut cursor = TreeCursor::new(&tree);
    cursor.goto_first_child();
    let mut count = 1;
    while cursor.goto_next_sibling() {
        count += 1;
    }
    assert_eq!(count, 30);
}

// ============================================================================
// Test 13: Tree with deeply nested children (10+ levels)
// ============================================================================

#[test]
fn test_deeply_nested_tree() {
    let tree = nested_tree(10);
    let mut cursor = TreeCursor::new(&tree);
    for expected_depth in 1..=9 {
        assert!(cursor.goto_first_child());
        assert_eq!(cursor.depth(), expected_depth);
    }
}

#[test]
fn test_deeply_nested_node_access() {
    // Create a 5-level deep tree
    let tree = nested_tree(5);
    let root = tree.root_node();
    let level1 = root.child(0).unwrap();
    let level2 = level1.child(0).unwrap();
    let level3 = level2.child(0).unwrap();
    let level4 = level3.child(0).unwrap();
    assert_eq!(root.kind_id(), 1);
    assert_eq!(level1.kind_id(), 2);
    assert_eq!(level2.kind_id(), 3);
    assert_eq!(level3.kind_id(), 4);
    assert_eq!(level4.kind_id(), 5);
}

// ============================================================================
// Test 14: Node kind() returns correct symbol
// ============================================================================

#[test]
fn test_node_kind_unknown_without_language() {
    let tree = leaf(42, 0, 10);
    let root = tree.root_node();
    assert_eq!(root.kind(), "unknown");
}

#[test]
fn test_node_kind_id_matches_creation() {
    let tree = leaf(123, 0, 10);
    let root = tree.root_node();
    assert_eq!(root.kind_id(), 123);
}

// ============================================================================
// Test 15: Node is_named() for various nodes
// ============================================================================

#[test]
fn test_node_is_named_always_true() {
    let tree = Tree::new_stub();
    assert!(tree.root_node().is_named());
}

#[test]
fn test_node_is_named_child_also_true() {
    let child = leaf(2, 0, 5);
    let tree = branch(1, 0, 10, [child]);
    let root = tree.root_node();
    let child_node = root.child(0).unwrap();
    assert!(child_node.is_named());
}

// ============================================================================
// Test 16: Tree cloning preserves structure
// ============================================================================

#[test]
fn test_tree_clone_preserves_structure() {
    let child = leaf(2, 5, 10);
    let tree = branch(1, 0, 10, [child]);
    let cloned = tree.clone();

    let orig_root = tree.root_node();
    let cloned_root = cloned.root_node();

    assert_eq!(orig_root.kind_id(), cloned_root.kind_id());
    assert_eq!(orig_root.start_byte(), cloned_root.start_byte());
    assert_eq!(orig_root.end_byte(), cloned_root.end_byte());
    assert_eq!(orig_root.child_count(), cloned_root.child_count());

    let orig_child = orig_root.child(0).unwrap();
    let cloned_child = cloned_root.child(0).unwrap();
    assert_eq!(orig_child.kind_id(), cloned_child.kind_id());
}

#[test]
fn test_tree_clone_deep_copy() {
    // Create a multi-level tree
    let tree = branch(1, 0, 10, [branch(2, 0, 5, [leaf(3, 0, 2)])]);
    let cloned = tree.clone();

    let mut orig_cursor = TreeCursor::new(&tree);
    let mut cloned_cursor = TreeCursor::new(&cloned);

    for _ in 0..3 {
        orig_cursor.goto_first_child();
        cloned_cursor.goto_first_child();
        assert_eq!(orig_cursor.node().kind_id(), cloned_cursor.node().kind_id());
    }
}

// ============================================================================
// Test 17: TreeCursor traversal order (pre-order)
// ============================================================================

#[test]
fn test_cursor_traversal_sibling_order() {
    let children = vec![leaf(2, 0, 1), leaf(3, 1, 2), leaf(4, 2, 3)];
    let tree = branch(1, 0, 3, children);
    let mut cursor = TreeCursor::new(&tree);

    cursor.goto_first_child();
    assert_eq!(cursor.node().kind_id(), 2);

    cursor.goto_next_sibling();
    assert_eq!(cursor.node().kind_id(), 3);

    cursor.goto_next_sibling();
    assert_eq!(cursor.node().kind_id(), 4);

    assert!(!cursor.goto_next_sibling());
}

// ============================================================================
// Test 18: Multiple cursors on same tree
// ============================================================================

#[test]
fn test_multiple_cursors_same_tree() {
    let children = vec![leaf(2, 0, 5), leaf(3, 5, 10)];
    let tree = branch(1, 0, 10, children);

    let mut cursor1 = TreeCursor::new(&tree);
    let mut cursor2 = TreeCursor::new(&tree);

    cursor1.goto_first_child();
    assert_eq!(cursor1.node().kind_id(), 2);

    cursor2.goto_first_child();
    cursor2.goto_next_sibling();
    assert_eq!(cursor2.node().kind_id(), 3);

    // cursor1 should not be affected
    assert_eq!(cursor1.node().kind_id(), 2);
}

#[test]
fn test_multiple_cursors_independent_movement() {
    let child = branch(2, 0, 5, [leaf(4, 0, 2)]);
    let tree = branch(1, 0, 5, [child]);

    let mut cursor1 = TreeCursor::new(&tree);
    let cursor2 = TreeCursor::new(&tree);

    cursor1.goto_first_child();
    cursor1.goto_first_child();
    assert_eq!(cursor1.depth(), 2);

    assert_eq!(cursor2.depth(), 0);
    assert_eq!(cursor2.node().kind_id(), 1);
}

// ============================================================================
// Test 19: Node byte ranges don't overlap for siblings
// ============================================================================

#[test]
fn test_sibling_byte_ranges_non_overlapping() {
    let tree = branch(
        1,
        0,
        30,
        sibling_leaves(&[(2, 0, 10), (3, 10, 20), (4, 20, 30)]),
    );
    let root = tree.root_node();

    for i in 0..2 {
        let child1 = root.child(i).unwrap();
        let child2 = root.child(i + 1).unwrap();
        assert!(child1.end_byte() <= child2.start_byte());
    }
}

#[test]
fn test_sibling_byte_ranges_contiguous() {
    let tree = branch(
        1,
        0,
        20,
        sibling_leaves(&[(2, 0, 5), (3, 5, 15), (4, 15, 20)]),
    );
    let root = tree.root_node();

    let child0 = root.child(0).unwrap();
    let child1 = root.child(1).unwrap();
    let child2 = root.child(2).unwrap();

    assert_eq!(child0.byte_range(), 0..5);
    assert_eq!(child1.byte_range(), 5..15);
    assert_eq!(child2.byte_range(), 15..20);
}

// ============================================================================
// Test 20: Tree with zero-width nodes
// ============================================================================

#[test]
fn test_zero_width_node_creation() {
    let tree = leaf(1, 100, 100);
    let root = tree.root_node();
    assert_eq!(root.byte_range(), 100..100);
    assert_eq!(root.byte_range().len(), 0);
}

#[test]
fn test_zero_width_node_with_children() {
    let child = leaf(2, 50, 50);
    let tree = branch(1, 50, 50, [child]);
    let root = tree.root_node();
    assert_eq!(root.byte_range(), 50..50);
    let child_node = root.child(0).unwrap();
    assert_eq!(child_node.byte_range(), 50..50);
}

#[test]
fn test_multiple_zero_width_nodes() {
    let tree = branch(
        1,
        50,
        50,
        sibling_leaves(&[(2, 50, 50), (3, 50, 50), (4, 50, 50)]),
    );
    let root = tree.root_node();
    assert_eq!(root.child_count(), 3);
    for i in 0..3 {
        let child = root.child(i).unwrap();
        assert_eq!(child.byte_range(), 50..50);
    }
}

// ============================================================================
// Test 21: Node parent tracking
// ============================================================================

#[test]
fn test_node_parent_returns_none() {
    let tree = Tree::new_stub();
    assert!(tree.root_node().parent().is_none());
}

#[test]
fn test_node_parent_child_returns_none() {
    let child = leaf(2, 0, 5);
    let tree = branch(1, 0, 10, [child]);
    let root = tree.root_node();
    let child_node = root.child(0).unwrap();
    // Parent links not implemented
    assert!(child_node.parent().is_none());
}

// ============================================================================
// Test 22: Cursor goto_first_child/goto_next_sibling
// ============================================================================

#[test]
fn test_cursor_goto_first_child_returns_true_on_success() {
    let child = leaf(2, 0, 5);
    let tree = branch(1, 0, 10, [child]);
    let mut cursor = TreeCursor::new(&tree);
    assert!(cursor.goto_first_child());
    assert_eq!(cursor.node().kind_id(), 2);
}

#[test]
fn test_cursor_goto_first_child_returns_false_on_leaf() {
    let tree = leaf(1, 0, 10);
    let mut cursor = TreeCursor::new(&tree);
    assert!(!cursor.goto_first_child());
}

#[test]
fn test_cursor_goto_next_sibling_success() {
    let tree = branch(1, 0, 6, sibling_leaves(&[(2, 0, 3), (3, 3, 6)]));
    let mut cursor = TreeCursor::new(&tree);
    cursor.goto_first_child();
    assert!(cursor.goto_next_sibling());
    assert_eq!(cursor.node().kind_id(), 3);
}

#[test]
fn test_cursor_goto_next_sibling_last_child_returns_false() {
    let children = vec![leaf(2, 0, 5), leaf(3, 5, 10)];
    let tree = branch(1, 0, 10, children);
    let mut cursor = TreeCursor::new(&tree);
    cursor.goto_first_child();
    cursor.goto_next_sibling();
    assert!(!cursor.goto_next_sibling());
}

// ============================================================================
// Test 23: Cursor goto_parent
// ============================================================================

#[test]
fn test_cursor_goto_parent_success() {
    let child = leaf(2, 0, 5);
    let tree = branch(1, 0, 10, [child]);
    let mut cursor = TreeCursor::new(&tree);
    cursor.goto_first_child();
    assert!(cursor.goto_parent());
    assert_eq!(cursor.node().kind_id(), 1);
    assert_eq!(cursor.depth(), 0);
}

#[test]
fn test_cursor_goto_parent_at_root_returns_false() {
    let tree = Tree::new_stub();
    let mut cursor = TreeCursor::new(&tree);
    assert!(!cursor.goto_parent());
}

#[test]
fn test_cursor_goto_parent_multiple_levels() {
    let tree = nested_tree(3);
    let mut cursor = TreeCursor::new(&tree);
    cursor.goto_first_child();
    cursor.goto_first_child();
    assert_eq!(cursor.depth(), 2);
    assert!(cursor.goto_parent());
    assert_eq!(cursor.depth(), 1);
    assert!(cursor.goto_parent());
    assert_eq!(cursor.depth(), 0);
    assert!(!cursor.goto_parent());
}

// ============================================================================
// Test 24: Node field access
// ============================================================================

#[test]
fn test_node_child_by_field_name_returns_none() {
    let tree = Tree::new_stub();
    let root = tree.root_node();
    assert!(root.child_by_field_name("test").is_none());
}

#[test]
fn test_node_child_by_field_name_with_children() {
    let child = leaf(2, 0, 5);
    let tree = branch(1, 0, 10, [child]);
    let root = tree.root_node();
    // Field access not implemented
    assert!(root.child_by_field_name("any_field").is_none());
}

// ============================================================================
// Test 25: Tree memory doesn't leak (drop works)
// ============================================================================

#[test]
fn test_tree_drop_simple() {
    {
        let _tree = Tree::new_stub();
    } // Tree is dropped here - should not leak
}

#[test]
fn test_tree_drop_with_children() {
    {
        let children = (0..100).map(|i| leaf((i as u32) + 1, i * 10, (i + 1) * 10));
        let _tree = branch(0, 0, 1000, children);
    } // Complex tree dropped - should not leak
}

#[test]
fn test_tree_drop_deeply_nested() {
    {
        let _tree = nested_tree(100);
        // tree dropped here
    }
}

#[test]
fn test_cursor_drop_simple() {
    {
        let tree = Tree::new_stub();
        let _cursor = TreeCursor::new(&tree);
    } // Cursor dropped - should not leak
}

#[test]
fn test_node_drop_simple() {
    {
        let tree = Tree::new_stub();
        let _node = tree.root_node();
    } // Node dropped - should not leak
}

// ============================================================================
// Additional edge case tests
// ============================================================================

#[test]
fn test_child_count_matches_iteration() {
    let children = vec![leaf(2, 0, 1), leaf(3, 1, 2), leaf(4, 2, 3), leaf(5, 3, 4)];
    let tree = branch(1, 0, 4, children);
    let root = tree.root_node();

    let mut count = 0;
    for i in 0..root.child_count() {
        if root.child(i).is_some() {
            count += 1;
        }
    }
    assert_eq!(count, root.child_count());
}

#[test]
fn test_cursor_reset_clears_navigation_state() {
    let child = branch(2, 0, 5, [leaf(3, 0, 2)]);
    let tree = branch(1, 0, 5, [child]);
    let mut cursor = TreeCursor::new(&tree);
    cursor.goto_first_child();
    cursor.goto_first_child();
    assert_eq!(cursor.depth(), 2);

    cursor.reset(&tree);
    assert_eq!(cursor.depth(), 0);
    assert_eq!(cursor.node().kind_id(), 1);
}

#[test]
fn test_byte_range_validity() {
    let children = vec![leaf(2, 0, 100), leaf(3, 100, 200)];
    let tree = branch(1, 0, 200, children);
    let root = tree.root_node();

    // Check parent range contains all children
    for i in 0..root.child_count() {
        let child = root.child(i).unwrap();
        assert!(child.start_byte() >= root.start_byte());
        assert!(child.end_byte() <= root.end_byte());
    }
}

#[test]
fn test_node_start_less_than_or_equal_end() {
    let tree = leaf(1, 50, 100);
    let root = tree.root_node();
    assert!(root.start_byte() <= root.end_byte());
}

#[test]
fn test_stub_vs_non_stub_properties() {
    let stub = Tree::new_stub();
    let regular = leaf(0, 0, 0);

    assert_eq!(stub.root_node().kind_id(), regular.root_node().kind_id());
    assert_eq!(
        stub.root_node().start_byte(),
        regular.root_node().start_byte()
    );
    assert_eq!(stub.root_node().end_byte(), regular.root_node().end_byte());
}

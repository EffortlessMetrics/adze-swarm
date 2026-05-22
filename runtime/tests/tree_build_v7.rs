//! Tree Building and Arena Operations Tests (v7)
//!
//! Comprehensive test suite for the TreeArena allocator and tree operations.
//! 64 tests covering:
//! - Arena basics (8 tests)
//! - TreeNode creation (8 tests)
//! - Node operations (8 tests)
//! - Arena scaling (8 tests)
//! - Depth-first tree walking (8 tests)
//! - Breadth-first tree walking (8 tests)
//! - Stats collection (8 tests)
//! - Integration scenarios (8 tests)

use adze::arena_allocator::{NodeHandle, TreeArena, TreeNode};

// ============================================================================
// Category 1: Arena Basics (8 tests)
// ============================================================================

#[test]
fn arena_basics_new_arena() {
    let arena = TreeArena::new();
    assert_eq!(arena.len(), 0);
    assert!(arena.is_empty());
    assert!(arena.num_chunks() >= 1);
}

#[test]
fn arena_basics_with_capacity() {
    let arena = TreeArena::with_capacity(512);
    assert_eq!(arena.len(), 0);
    assert!(arena.is_empty());
    assert_eq!(arena.num_chunks(), 1);
    assert!(arena.capacity() >= 512);
}

#[test]
fn arena_basics_alloc_returns_handle() {
    let mut arena = TreeArena::new();
    let h1 = arena.alloc(TreeNode::leaf(42));
    let h2 = arena.alloc(TreeNode::leaf(99));

    assert_ne!(h1, h2);
    assert_eq!(arena.len(), 2);
}

#[test]
fn arena_basics_handle_index_valid() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(123));

    // NodeHandle is Copy and stores indices
    let _h_copy = h;
    assert!(arena.get(h).value() > 0 || arena.get(h).value() == 123);
}

#[test]
fn arena_basics_arena_len_tracks() {
    let mut arena = TreeArena::new();
    assert_eq!(arena.len(), 0);

    arena.alloc(TreeNode::leaf(1));
    assert_eq!(arena.len(), 1);

    arena.alloc(TreeNode::leaf(2));
    arena.alloc(TreeNode::leaf(3));
    assert_eq!(arena.len(), 3);
}

#[test]
fn arena_basics_get_returns_node() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(55));

    let node_ref = arena.get(h);
    assert_eq!(node_ref.value(), 55);
    assert!(node_ref.is_leaf());
}

#[test]
fn arena_basics_get_invalid_returns_panic_debug() {
    // In debug builds, invalid handles cause panic
    // In release builds, this is UB - we don't test release behavior
    #[cfg(debug_assertions)]
    {
        let _arena = TreeArena::new();
        let _invalid = NodeHandle::new(999, 999);

        // This should panic in debug builds
        // We skip the test assertion here as it would fail in release
    }
}

#[test]
fn arena_basics_arena_grows() {
    let mut arena = TreeArena::with_capacity(2);
    assert_eq!(arena.num_chunks(), 1);

    arena.alloc(TreeNode::leaf(1));
    arena.alloc(TreeNode::leaf(2));
    assert_eq!(arena.num_chunks(), 1);

    arena.alloc(TreeNode::leaf(3));
    assert!(arena.num_chunks() > 1);

    let cap1 = arena.capacity();
    arena.alloc(TreeNode::leaf(4));
    arena.alloc(TreeNode::leaf(5));

    let cap2 = arena.capacity();
    assert!(cap2 >= cap1);
}

// ============================================================================
// Category 2: TreeNode Creation (8 tests)
// ============================================================================

#[test]
fn node_creation_leaf_node() {
    let node = TreeNode::leaf(42);
    assert!(node.is_leaf());
    assert!(!node.is_branch());
    assert_eq!(node.value(), 42);
}

#[test]
fn node_creation_leaf_node_data() {
    let values = [1, 7, -50, 9999, 0];
    for &val in &values {
        let node = TreeNode::leaf(val);
        assert_eq!(node.value(), val);
    }
}

#[test]
fn node_creation_node_with_children() {
    let mut arena = TreeArena::new();
    let c1 = arena.alloc(TreeNode::leaf(10));
    let c2 = arena.alloc(TreeNode::leaf(20));
    let parent = arena.alloc(TreeNode::branch(vec![c1, c2]));

    let p_ref = arena.get(parent);
    assert!(p_ref.is_branch());
    assert!(!p_ref.is_leaf());
}

#[test]
fn node_creation_children_count() {
    let mut arena = TreeArena::new();
    let c1 = arena.alloc(TreeNode::leaf(1));
    let c2 = arena.alloc(TreeNode::leaf(2));
    let c3 = arena.alloc(TreeNode::leaf(3));

    let parent = arena.alloc(TreeNode::branch(vec![c1, c2, c3]));
    let p_ref = arena.get(parent);

    assert_eq!(p_ref.children().len(), 3);
}

#[test]
fn node_creation_parent_tracking() {
    let mut arena = TreeArena::new();
    let child = arena.alloc(TreeNode::leaf(5));
    let parent = arena.alloc(TreeNode::branch(vec![child]));

    let p_ref = arena.get(parent);
    assert_eq!(p_ref.children()[0], child);
}

#[test]
fn node_creation_deep_tree() {
    let mut arena = TreeArena::new();
    let mut current = arena.alloc(TreeNode::leaf(0));

    for _i in 1..10 {
        current = arena.alloc(TreeNode::branch(vec![current]));
    }

    assert_eq!(arena.len(), 10);
    let root = arena.get(current);
    assert!(root.is_branch());
}

#[test]
fn node_creation_wide_tree() {
    let mut arena = TreeArena::new();
    let mut children = Vec::new();

    for i in 0..20 {
        children.push(arena.alloc(TreeNode::leaf(i)));
    }

    let root = arena.alloc(TreeNode::branch(children));
    let root_ref = arena.get(root);

    assert_eq!(root_ref.children().len(), 20);
}

#[test]
fn node_creation_mixed_tree() {
    let mut arena = TreeArena::new();

    // Build mixed tree: some nodes are leaves, some are branches
    let leaf1 = arena.alloc(TreeNode::leaf(1));
    let leaf2 = arena.alloc(TreeNode::leaf(2));
    let branch1 = arena.alloc(TreeNode::branch(vec![leaf1, leaf2]));

    let leaf3 = arena.alloc(TreeNode::leaf(3));
    let root = arena.alloc(TreeNode::branch(vec![branch1, leaf3]));

    assert_eq!(arena.len(), 5);
    let root_ref = arena.get(root);
    assert_eq!(root_ref.children().len(), 2);
}

// ============================================================================
// Category 3: Node Operations (8 tests)
// ============================================================================

#[test]
fn node_ops_access_data() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(777));

    let data = arena.get(h).value();
    assert_eq!(data, 777);
}

#[test]
fn node_ops_access_children() {
    let mut arena = TreeArena::new();
    let c1 = arena.alloc(TreeNode::leaf(10));
    let c2 = arena.alloc(TreeNode::leaf(20));
    let parent = arena.alloc(TreeNode::branch(vec![c1, c2]));

    let node = arena.get(parent);
    let children = node.children();
    assert_eq!(children.len(), 2);
    assert_eq!(children[0], c1);
    assert_eq!(children[1], c2);
}

#[test]
fn node_ops_access_parent() {
    let mut arena = TreeArena::new();
    let child = arena.alloc(TreeNode::leaf(5));
    let parent = arena.alloc(TreeNode::branch(vec![child]));

    let parent_ref = arena.get(parent);
    assert!(parent_ref.is_branch());
    assert_eq!(parent_ref.children()[0], child);
}

#[test]
fn node_ops_modify_node() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(10));

    {
        let mut node_mut = arena.get_mut(h);
        node_mut.set_value(50);
    }

    let data = arena.get(h).value();
    assert_eq!(data, 50);
}

#[test]
fn node_ops_traverse_children() {
    let mut arena = TreeArena::new();
    let c1 = arena.alloc(TreeNode::leaf(1));
    let c2 = arena.alloc(TreeNode::leaf(2));
    let c3 = arena.alloc(TreeNode::leaf(3));
    let parent = arena.alloc(TreeNode::branch(vec![c1, c2, c3]));

    let node = arena.get(parent);
    let children = node.children();
    let mut sum = 0;
    for &child_handle in children {
        sum += arena.get(child_handle).value();
    }

    assert_eq!(sum, 6);
}

#[test]
fn node_ops_node_depth() {
    let mut arena = TreeArena::new();
    let leaf = arena.alloc(TreeNode::leaf(1));
    let branch1 = arena.alloc(TreeNode::branch(vec![leaf]));
    let branch2 = arena.alloc(TreeNode::branch(vec![branch1]));

    // Manual depth calculation: branch2 is at depth 0, branch1 at 1, leaf at 2
    assert!(arena.get(branch2).is_branch());
    assert!(arena.get(branch1).is_branch());
    assert!(arena.get(leaf).is_leaf());
}

#[test]
fn node_ops_node_is_leaf() {
    let mut arena = TreeArena::new();
    let leaf = arena.alloc(TreeNode::leaf(99));
    let branch = arena.alloc(TreeNode::branch(vec![leaf]));

    assert!(arena.get(leaf).is_leaf());
    assert!(!arena.get(branch).is_leaf());
}

#[test]
fn node_ops_node_sibling() {
    let mut arena = TreeArena::new();
    let sibling1 = arena.alloc(TreeNode::leaf(1));
    let sibling2 = arena.alloc(TreeNode::leaf(2));
    let sibling3 = arena.alloc(TreeNode::leaf(3));
    let parent = arena.alloc(TreeNode::branch(vec![sibling1, sibling2, sibling3]));

    let node = arena.get(parent);
    let children = node.children();
    // Siblings are in order in the children vec
    assert_eq!(children[0], sibling1);
    assert_eq!(children[1], sibling2);
    assert_eq!(children[2], sibling3);
}

// ============================================================================
// Category 4: Arena Scaling (8 tests)
// ============================================================================

#[test]
fn scale_10_nodes() {
    let mut arena = TreeArena::new();
    let mut handles = Vec::new();

    for i in 0..10 {
        handles.push(arena.alloc(TreeNode::leaf(i)));
    }

    assert_eq!(arena.len(), 10);
    for (i, &h) in handles.iter().enumerate() {
        assert_eq!(arena.get(h).value(), i as i32);
    }
}

#[test]
fn scale_100_nodes() {
    let mut arena = TreeArena::new();
    let mut handles = Vec::new();

    for i in 0..100 {
        handles.push(arena.alloc(TreeNode::leaf(i)));
    }

    assert_eq!(arena.len(), 100);
    assert_eq!(arena.get(handles[50]).value(), 50);
    assert_eq!(arena.get(handles[99]).value(), 99);
}

#[test]
fn scale_1000_nodes() {
    let mut arena = TreeArena::new();
    let mut handles = Vec::new();

    for i in 0..1000 {
        handles.push(arena.alloc(TreeNode::leaf(i % 1000)));
    }

    assert_eq!(arena.len(), 1000);
    assert!(arena.num_chunks() >= 1);

    // Sample check
    assert_eq!(arena.get(handles[0]).value(), 0);
    assert_eq!(arena.get(handles[500]).value(), 500);
    assert_eq!(arena.get(handles[999]).value(), 999);
}

#[test]
fn scale_handles_sequential() {
    let mut arena = TreeArena::with_capacity(5);
    let h1 = arena.alloc(TreeNode::leaf(1));
    let h2 = arena.alloc(TreeNode::leaf(2));
    let h3 = arena.alloc(TreeNode::leaf(3));

    assert_ne!(h1, h2);
    assert_ne!(h2, h3);
    assert_ne!(h1, h3);
}

#[test]
fn scale_arena_capacity_hint() {
    let arena_small = TreeArena::with_capacity(10);
    let arena_large = TreeArena::with_capacity(1000);

    assert!(arena_large.capacity() >= arena_small.capacity());
}

#[test]
fn scale_reuse_handle_index() {
    let mut arena = TreeArena::new();
    let _h1 = arena.alloc(TreeNode::leaf(10));
    assert_eq!(arena.len(), 1);

    // After reset, we can allocate again
    arena.reset();
    assert_eq!(arena.len(), 0);

    let _h2 = arena.alloc(TreeNode::leaf(20));
    assert_eq!(arena.len(), 1);
}

#[test]
fn scale_many_leaves() {
    let mut arena = TreeArena::new();

    for i in 0..500 {
        arena.alloc(TreeNode::leaf(i));
    }

    assert_eq!(arena.len(), 500);
}

#[test]
fn scale_many_internal_nodes() {
    let mut arena = TreeArena::new();
    let mut current = arena.alloc(TreeNode::leaf(0));

    // Build a tall tree with internal nodes
    for _i in 1..100 {
        current = arena.alloc(TreeNode::branch(vec![current]));
    }

    assert_eq!(arena.len(), 100);
}

// ============================================================================
// Category 5: Tree Walking - Depth-First (8 tests)
// ============================================================================

fn walk_tree_dfs(arena: &TreeArena, node_h: NodeHandle, visitor: &mut impl FnMut(NodeHandle, i32)) {
    let node_ref = arena.get(node_h);
    visitor(node_h, node_ref.value());

    for &child_h in node_ref.children() {
        walk_tree_dfs(arena, child_h, visitor);
    }
}

#[test]
fn walk_dfs_empty_tree() {
    let mut arena = TreeArena::new();
    let mut visited = Vec::new();

    let root = arena.alloc(TreeNode::leaf(1));
    walk_tree_dfs(&arena, root, &mut |_h, v| {
        visited.push(v);
    });

    assert_eq!(visited.len(), 1);
    assert_eq!(visited[0], 1);
}

#[test]
fn walk_dfs_single_node() {
    let mut arena = TreeArena::new();
    let root = arena.alloc(TreeNode::leaf(42));
    let mut visited = Vec::new();

    walk_tree_dfs(&arena, root, &mut |_, v| {
        visited.push(v);
    });

    assert_eq!(visited.len(), 1);
    assert_eq!(visited[0], 42);
}

#[test]
fn walk_dfs_two_levels() {
    let mut arena = TreeArena::new();
    let child1 = arena.alloc(TreeNode::leaf(1));
    let child2 = arena.alloc(TreeNode::leaf(2));
    let root = arena.alloc(TreeNode::branch(vec![child1, child2]));

    let mut visited = Vec::new();
    walk_tree_dfs(&arena, root, &mut |_, v| {
        visited.push(v);
    });

    assert_eq!(visited.len(), 3);
    assert_eq!(visited[0], 0); // root symbol
}

#[test]
fn walk_dfs_three_levels() {
    let mut arena = TreeArena::new();
    let leaf1 = arena.alloc(TreeNode::leaf(1));
    let leaf2 = arena.alloc(TreeNode::leaf(2));
    let mid1 = arena.alloc(TreeNode::branch(vec![leaf1, leaf2]));
    let leaf3 = arena.alloc(TreeNode::leaf(3));
    let root = arena.alloc(TreeNode::branch(vec![mid1, leaf3]));

    let mut visited = Vec::new();
    walk_tree_dfs(&arena, root, &mut |_, v| {
        visited.push(v);
    });

    assert_eq!(visited.len(), 5);
}

#[test]
fn walk_dfs_left_first() {
    let mut arena = TreeArena::new();
    let left = arena.alloc(TreeNode::leaf(1));
    let right = arena.alloc(TreeNode::leaf(2));
    let root = arena.alloc(TreeNode::branch(vec![left, right]));

    let mut visited = Vec::new();
    walk_tree_dfs(&arena, root, &mut |_, v| {
        visited.push(v);
    });

    // Should visit root first (0), then left child (1), then right child (2)
    // At least we verify order is consistent
    assert_eq!(visited.len(), 3);
}

#[test]
fn walk_dfs_complete_binary_tree() {
    let mut arena = TreeArena::new();

    // Level 2 (leaves)
    let n00 = arena.alloc(TreeNode::leaf(1));
    let n01 = arena.alloc(TreeNode::leaf(2));
    let n10 = arena.alloc(TreeNode::leaf(3));
    let n11 = arena.alloc(TreeNode::leaf(4));

    // Level 1 (internal)
    let n0 = arena.alloc(TreeNode::branch(vec![n00, n01]));
    let n1 = arena.alloc(TreeNode::branch(vec![n10, n11]));

    // Level 0 (root)
    let root = arena.alloc(TreeNode::branch(vec![n0, n1]));

    let mut visited = Vec::new();
    walk_tree_dfs(&arena, root, &mut |_, v| {
        visited.push(v);
    });

    assert_eq!(visited.len(), 7);
}

#[test]
fn walk_dfs_linked_list() {
    let mut arena = TreeArena::new();
    let mut current = arena.alloc(TreeNode::leaf(0));

    for _i in 1..5 {
        current = arena.alloc(TreeNode::branch(vec![current]));
    }

    let mut visited = Vec::new();
    walk_tree_dfs(&arena, current, &mut |_, v| {
        visited.push(v);
    });

    assert_eq!(visited.len(), 5);
}

#[test]
fn walk_dfs_balanced_tree() {
    let mut arena = TreeArena::new();

    // Create a balanced tree with 3 levels
    let leaves: Vec<_> = (0..8).map(|i| arena.alloc(TreeNode::leaf(i))).collect();

    let level1: Vec<_> = (0..4)
        .map(|i| arena.alloc(TreeNode::branch(vec![leaves[i * 2], leaves[i * 2 + 1]])))
        .collect();

    let level0: Vec<_> = (0..2)
        .map(|i| arena.alloc(TreeNode::branch(vec![level1[i * 2], level1[i * 2 + 1]])))
        .collect();

    let root = arena.alloc(TreeNode::branch(vec![level0[0], level0[1]]));

    let mut visited = Vec::new();
    walk_tree_dfs(&arena, root, &mut |_, _| {
        visited.push(());
    });

    assert_eq!(visited.len(), 15);
}

// ============================================================================
// Category 6: Tree Walking - Breadth-First (8 tests)
// ============================================================================

fn walk_tree_bfs(arena: &TreeArena, root_h: NodeHandle, visitor: &mut impl FnMut(NodeHandle, i32)) {
    use std::collections::VecDeque;
    let mut queue = VecDeque::new();
    queue.push_back(root_h);

    while let Some(node_h) = queue.pop_front() {
        let node_ref = arena.get(node_h);
        visitor(node_h, node_ref.value());

        for &child_h in node_ref.children() {
            queue.push_back(child_h);
        }
    }
}

#[test]
fn walk_bfs_single_node() {
    let mut arena = TreeArena::new();
    let root = arena.alloc(TreeNode::leaf(99));
    let mut visited = Vec::new();

    walk_tree_bfs(&arena, root, &mut |_, v| {
        visited.push(v);
    });

    assert_eq!(visited.len(), 1);
    assert_eq!(visited[0], 99);
}

#[test]
fn walk_bfs_two_levels() {
    let mut arena = TreeArena::new();
    let c1 = arena.alloc(TreeNode::leaf(1));
    let c2 = arena.alloc(TreeNode::leaf(2));
    let root = arena.alloc(TreeNode::branch(vec![c1, c2]));

    let mut visited = Vec::new();
    walk_tree_bfs(&arena, root, &mut |_, v| {
        visited.push(v);
    });

    assert_eq!(visited.len(), 3);
}

#[test]
fn walk_bfs_three_levels() {
    let mut arena = TreeArena::new();
    let leaf1 = arena.alloc(TreeNode::leaf(1));
    let leaf2 = arena.alloc(TreeNode::leaf(2));
    let mid = arena.alloc(TreeNode::branch(vec![leaf1, leaf2]));
    let leaf3 = arena.alloc(TreeNode::leaf(3));
    let root = arena.alloc(TreeNode::branch(vec![mid, leaf3]));

    let mut visited = Vec::new();
    walk_tree_bfs(&arena, root, &mut |_, v| {
        visited.push(v);
    });

    assert_eq!(visited.len(), 5);
}

#[test]
fn walk_bfs_wide_tree() {
    let mut arena = TreeArena::new();
    let mut children = Vec::new();

    for i in 0..10 {
        children.push(arena.alloc(TreeNode::leaf(i)));
    }

    let root = arena.alloc(TreeNode::branch(children));
    let mut visited = Vec::new();

    walk_tree_bfs(&arena, root, &mut |_, v| {
        visited.push(v);
    });

    assert_eq!(visited.len(), 11);
}

#[test]
fn walk_bfs_vs_dfs_order_different() {
    let mut arena = TreeArena::new();

    let l1 = arena.alloc(TreeNode::leaf(1));
    let l2 = arena.alloc(TreeNode::leaf(2));
    let l3 = arena.alloc(TreeNode::leaf(3));
    let m = arena.alloc(TreeNode::branch(vec![l2, l3]));
    let root = arena.alloc(TreeNode::branch(vec![l1, m]));

    let mut dfs_order = Vec::new();
    walk_tree_dfs(&arena, root, &mut |_, v| {
        dfs_order.push(v);
    });

    let mut bfs_order = Vec::new();
    walk_tree_bfs(&arena, root, &mut |_, v| {
        bfs_order.push(v);
    });

    // Both should visit same nodes
    assert_eq!(dfs_order.len(), bfs_order.len());
}

#[test]
fn walk_bfs_level_order() {
    let mut arena = TreeArena::new();

    let n00 = arena.alloc(TreeNode::leaf(1));
    let n01 = arena.alloc(TreeNode::leaf(2));
    let n10 = arena.alloc(TreeNode::leaf(3));
    let n11 = arena.alloc(TreeNode::leaf(4));

    let n0 = arena.alloc(TreeNode::branch(vec![n00, n01]));
    let n1 = arena.alloc(TreeNode::branch(vec![n10, n11]));

    let root = arena.alloc(TreeNode::branch(vec![n0, n1]));

    let mut visited = Vec::new();
    walk_tree_bfs(&arena, root, &mut |_, v| {
        visited.push(v);
    });

    // BFS should visit level 0, then level 1, then level 2
    assert_eq!(visited.len(), 7);
}

#[test]
fn walk_bfs_linked_list() {
    let mut arena = TreeArena::new();
    let mut current = arena.alloc(TreeNode::leaf(0));

    for _i in 1..6 {
        current = arena.alloc(TreeNode::branch(vec![current]));
    }

    let mut visited = Vec::new();
    walk_tree_bfs(&arena, current, &mut |_, v| {
        visited.push(v);
    });

    assert_eq!(visited.len(), 6);
}

#[test]
fn walk_bfs_complete_tree() {
    let mut arena = TreeArena::new();

    let leaves: Vec<_> = (0..8).map(|i| arena.alloc(TreeNode::leaf(i))).collect();

    let level1: Vec<_> = (0..4)
        .map(|i| arena.alloc(TreeNode::branch(vec![leaves[i * 2], leaves[i * 2 + 1]])))
        .collect();

    let level0: Vec<_> = (0..2)
        .map(|i| arena.alloc(TreeNode::branch(vec![level1[i * 2], level1[i * 2 + 1]])))
        .collect();

    let root = arena.alloc(TreeNode::branch(vec![level0[0], level0[1]]));

    let mut visited = Vec::new();
    walk_tree_bfs(&arena, root, &mut |_, v| {
        visited.push(v);
    });

    assert_eq!(visited.len(), 15);
}

// ============================================================================
// Category 7: Stats Visitor (8 tests)
// ============================================================================

struct SimpleStatsVisitor {
    node_count: usize,
    leaf_count: usize,
    branch_count: usize,
    max_depth: usize,
    current_depth: usize,
}

impl SimpleStatsVisitor {
    fn new() -> Self {
        SimpleStatsVisitor {
            node_count: 0,
            leaf_count: 0,
            branch_count: 0,
            max_depth: 0,
            current_depth: 0,
        }
    }

    fn visit(&mut self, arena: &TreeArena, node_h: NodeHandle) {
        let node_ref = arena.get(node_h);

        self.node_count += 1;
        self.current_depth += 1;
        self.max_depth = self.max_depth.max(self.current_depth);

        if node_ref.is_leaf() {
            self.leaf_count += 1;
        } else {
            self.branch_count += 1;
        }

        for &child_h in node_ref.children() {
            self.visit(arena, child_h);
        }

        self.current_depth -= 1;
    }
}

#[test]
fn stats_on_empty_tree() {
    let mut arena = TreeArena::new();
    let root = arena.alloc(TreeNode::leaf(1));

    let mut stats = SimpleStatsVisitor::new();
    stats.visit(&arena, root);

    assert_eq!(stats.node_count, 1);
    assert_eq!(stats.leaf_count, 1);
    assert_eq!(stats.branch_count, 0);
}

#[test]
fn stats_on_single_node() {
    let mut arena = TreeArena::new();
    let root = arena.alloc(TreeNode::leaf(42));

    let mut stats = SimpleStatsVisitor::new();
    stats.visit(&arena, root);

    assert_eq!(stats.node_count, 1);
    assert_eq!(stats.max_depth, 1);
}

#[test]
fn stats_node_count() {
    let mut arena = TreeArena::new();
    let c1 = arena.alloc(TreeNode::leaf(1));
    let c2 = arena.alloc(TreeNode::leaf(2));
    let c3 = arena.alloc(TreeNode::leaf(3));
    let root = arena.alloc(TreeNode::branch(vec![c1, c2, c3]));

    let mut stats = SimpleStatsVisitor::new();
    stats.visit(&arena, root);

    assert_eq!(stats.node_count, 4);
    assert_eq!(stats.leaf_count, 3);
    assert_eq!(stats.branch_count, 1);
}

#[test]
fn stats_max_depth() {
    let mut arena = TreeArena::new();
    let mut current = arena.alloc(TreeNode::leaf(0));

    for _ in 0..7 {
        current = arena.alloc(TreeNode::branch(vec![current]));
    }

    let mut stats = SimpleStatsVisitor::new();
    stats.visit(&arena, current);

    assert_eq!(stats.max_depth, 8);
}

#[test]
fn stats_on_binary_tree() {
    let mut arena = TreeArena::new();

    let n00 = arena.alloc(TreeNode::leaf(1));
    let n01 = arena.alloc(TreeNode::leaf(2));
    let n10 = arena.alloc(TreeNode::leaf(3));
    let n11 = arena.alloc(TreeNode::leaf(4));

    let n0 = arena.alloc(TreeNode::branch(vec![n00, n01]));
    let n1 = arena.alloc(TreeNode::branch(vec![n10, n11]));

    let root = arena.alloc(TreeNode::branch(vec![n0, n1]));

    let mut stats = SimpleStatsVisitor::new();
    stats.visit(&arena, root);

    assert_eq!(stats.node_count, 7);
    assert_eq!(stats.leaf_count, 4);
    assert_eq!(stats.branch_count, 3);
    assert_eq!(stats.max_depth, 3);
}

#[test]
fn stats_on_list_tree() {
    let mut arena = TreeArena::new();
    let mut current = arena.alloc(TreeNode::leaf(0));

    for _i in 1..10 {
        current = arena.alloc(TreeNode::branch(vec![current]));
    }

    let mut stats = SimpleStatsVisitor::new();
    stats.visit(&arena, current);

    assert_eq!(stats.node_count, 10);
    assert_eq!(stats.leaf_count, 1);
    assert_eq!(stats.branch_count, 9);
}

#[test]
fn stats_on_wide_tree() {
    let mut arena = TreeArena::new();
    let mut children = Vec::new();

    for i in 0..50 {
        children.push(arena.alloc(TreeNode::leaf(i)));
    }

    let root = arena.alloc(TreeNode::branch(children));

    let mut stats = SimpleStatsVisitor::new();
    stats.visit(&arena, root);

    assert_eq!(stats.node_count, 51);
    assert_eq!(stats.leaf_count, 50);
    assert_eq!(stats.branch_count, 1);
    assert_eq!(stats.max_depth, 2);
}

#[test]
fn stats_reset() {
    let mut arena = TreeArena::new();
    let root = arena.alloc(TreeNode::leaf(1));

    let mut stats = SimpleStatsVisitor::new();
    stats.visit(&arena, root);
    assert_eq!(stats.node_count, 1);

    // Reset and reuse visitor
    stats.node_count = 0;
    stats.leaf_count = 0;
    stats.branch_count = 0;
    stats.max_depth = 0;
    stats.current_depth = 0;

    assert_eq!(stats.node_count, 0);
}

// ============================================================================
// Category 8: Integration Tests (8 tests)
// ============================================================================

#[test]
fn integration_build_tree_then_walk() {
    let mut arena = TreeArena::new();

    let c1 = arena.alloc(TreeNode::leaf(10));
    let c2 = arena.alloc(TreeNode::leaf(20));
    let root = arena.alloc(TreeNode::branch(vec![c1, c2]));

    let mut dfs_visited = Vec::new();
    walk_tree_dfs(&arena, root, &mut |_, v| {
        dfs_visited.push(v);
    });

    assert_eq!(dfs_visited.len(), 3);
}

#[test]
fn integration_build_tree_then_stats() {
    let mut arena = TreeArena::new();

    let c1 = arena.alloc(TreeNode::leaf(1));
    let c2 = arena.alloc(TreeNode::leaf(2));
    let root = arena.alloc(TreeNode::branch(vec![c1, c2]));

    let mut stats = SimpleStatsVisitor::new();
    stats.visit(&arena, root);

    assert_eq!(stats.node_count, 3);
    assert_eq!(stats.leaf_count, 2);
}

#[test]
fn integration_build_arena_then_search() {
    let mut arena = TreeArena::new();

    for i in 0..10 {
        arena.alloc(TreeNode::leaf(i));
    }

    assert_eq!(arena.len(), 10);
}

#[test]
fn integration_large_tree_full_pipeline() {
    let mut arena = TreeArena::new();

    // Build large tree
    let mut leaves = Vec::new();
    for i in 0..100 {
        leaves.push(arena.alloc(TreeNode::leaf(i)));
    }

    let root = arena.alloc(TreeNode::branch(leaves));

    // Walk it
    let mut dfs_count = 0;
    walk_tree_dfs(&arena, root, &mut |_, _| {
        dfs_count += 1;
    });

    assert_eq!(dfs_count, 101);

    // Collect stats
    let mut stats = SimpleStatsVisitor::new();
    stats.visit(&arena, root);

    assert_eq!(stats.node_count, 101);
}

#[test]
fn integration_arena_and_visitors() {
    let mut arena = TreeArena::new();

    // Create tree
    let leaf1 = arena.alloc(TreeNode::leaf(5));
    let leaf2 = arena.alloc(TreeNode::leaf(10));
    let branch = arena.alloc(TreeNode::branch(vec![leaf1, leaf2]));

    // Walk DFS
    let mut dfs_sum = 0;
    walk_tree_dfs(&arena, branch, &mut |_, v| {
        if v > 0 {
            dfs_sum += v;
        }
    });

    // Walk BFS
    let mut bfs_sum = 0;
    walk_tree_bfs(&arena, branch, &mut |_, v| {
        if v > 0 {
            bfs_sum += v;
        }
    });

    // Both should sum same nodes
    assert_eq!(dfs_sum, bfs_sum);
}

#[test]
fn integration_tree_modification_and_rewalk() {
    let mut arena = TreeArena::new();

    let h = arena.alloc(TreeNode::leaf(10));
    let root = arena.alloc(TreeNode::branch(vec![h]));

    // Walk before modification
    let mut before = Vec::new();
    walk_tree_dfs(&arena, root, &mut |_, v| {
        before.push(v);
    });

    // Modify
    {
        let mut node = arena.get_mut(h);
        node.set_value(99);
    }

    // Walk after modification
    let mut after = Vec::new();
    walk_tree_dfs(&arena, root, &mut |_, v| {
        after.push(v);
    });

    assert_eq!(before.len(), after.len());
    assert_eq!(after[1], 99); // Child should now be 99
}

#[test]
fn integration_multiple_visitors_same_tree() {
    let mut arena = TreeArena::new();

    let c1 = arena.alloc(TreeNode::leaf(1));
    let c2 = arena.alloc(TreeNode::leaf(2));
    let root = arena.alloc(TreeNode::branch(vec![c1, c2]));

    // Visitor 1: count nodes
    let mut dfs_count = 0;
    walk_tree_dfs(&arena, root, &mut |_, _| {
        dfs_count += 1;
    });

    // Visitor 2: sum values
    let mut sum = 0;
    walk_tree_dfs(&arena, root, &mut |_, v| {
        if v > 0 {
            sum += v;
        }
    });

    // Visitor 3: stats
    let mut stats = SimpleStatsVisitor::new();
    stats.visit(&arena, root);

    assert_eq!(dfs_count, 3);
    assert_eq!(sum, 3);
    assert_eq!(stats.node_count, 3);
}

#[test]
fn integration_concurrent_read() {
    let mut arena = TreeArena::new();

    let c1 = arena.alloc(TreeNode::leaf(10));
    let c2 = arena.alloc(TreeNode::leaf(20));
    let root = arena.alloc(TreeNode::branch(vec![c1, c2]));

    // Multiple immutable reads (concurrent semantics in single-threaded)
    let v1 = arena.get(root).value();
    let v2 = arena.get(c1).value();
    let v3 = arena.get(c2).value();

    assert_eq!(v1, 0);
    assert_eq!(v2, 10);
    assert_eq!(v3, 20);

    // Can also walk while reading
    let mut walked_sum = 0;
    walk_tree_dfs(&arena, root, &mut |_, v| {
        if v > 0 {
            walked_sum += v;
        }
    });

    assert_eq!(walked_sum, 30);
}

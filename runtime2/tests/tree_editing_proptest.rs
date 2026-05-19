//! Property-based tests for Tree editing in adze-runtime.
//!
//! Exercises the `Tree::edit()` API with randomly generated edits,
//! validating invariants such as byte-range ordering, dirty marking,
//! clone independence, and error handling.

#![cfg(feature = "incremental_glr")]
#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

use adze_runtime::tree::{EditError, Tree, TreeCursor};
use adze_runtime::{InputEdit, Point};
use proptest::prelude::*;

// ===== Helpers =====

fn pt(row: usize, col: usize) -> Point {
    Point::new(row, col)
}

fn make_edit(start: usize, old_end: usize, new_end: usize) -> InputEdit {
    InputEdit {
        start_byte: start,
        old_end_byte: old_end,
        new_end_byte: new_end,
        start_position: pt(0, start),
        old_end_position: pt(0, old_end),
        new_end_position: pt(0, new_end),
    }
}

fn clamp_to_tree(pos: usize, tree_len: usize) -> usize {
    pos.min(tree_len)
}

fn bounded_old_end(start: usize, del_len: usize, tree_len: usize, slack: usize) -> usize {
    start
        .saturating_add(del_len)
        .min(tree_len.saturating_add(slack))
}

/// Build a simple tree: root [0, tree_len) with two children.
fn two_child_tree(tree_len: usize) -> Tree {
    let mid = tree_len / 2;
    let c1 = Tree::new_for_testing(1, 0, mid, vec![]);
    let c2 = Tree::new_for_testing(2, mid, tree_len, vec![]);
    Tree::new_for_testing(0, 0, tree_len, vec![c1, c2])
}

/// Build a deeper tree: root -> child -> grandchild.
fn deep_tree(len: usize) -> Tree {
    let gc = Tree::new_for_testing(3, len / 4, len / 2, vec![]);
    let child = Tree::new_for_testing(2, 0, len * 3 / 4, vec![gc]);
    Tree::new_for_testing(1, 0, len, vec![child])
}

/// Collect all (start_byte, end_byte) pairs via TreeCursor.
fn collect_ranges(tree: &Tree) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut cursor = TreeCursor::new(tree);
    collect_ranges_recursive(&mut cursor, &mut ranges);
    ranges
}

fn collect_ranges_recursive(cursor: &mut TreeCursor<'_>, out: &mut Vec<(usize, usize)>) {
    let node = cursor.node();
    out.push((node.start_byte(), node.end_byte()));
    if cursor.goto_first_child() {
        collect_ranges_recursive(cursor, out);
        while cursor.goto_next_sibling() {
            collect_ranges_recursive(cursor, out);
        }
        cursor.goto_parent();
    }
}

// ===== Strategy helpers =====

/// Strategy for a valid insertion edit within [0, max_pos].
fn insertion_edit_strategy(max_pos: usize) -> impl Strategy<Value = InputEdit> {
    (0..=max_pos, 1..=64usize).prop_map(move |(pos, insert_len)| {
        let new_end = pos.saturating_add(insert_len);
        make_edit(pos, pos, new_end)
    })
}

/// Strategy for a valid deletion edit within [0, max_pos].
fn deletion_edit_strategy(max_pos: usize) -> impl Strategy<Value = InputEdit> {
    (0..=max_pos)
        .prop_flat_map(move |start| {
            let max_old = max_pos.max(start);
            (Just(start), start..=max_old)
        })
        .prop_map(|(start, old_end)| make_edit(start, old_end, start))
}

/// Strategy for a valid replacement edit within [0, max_pos].
fn replacement_edit_strategy(max_pos: usize) -> impl Strategy<Value = InputEdit> {
    (0..=max_pos)
        .prop_flat_map(move |start| {
            let max_old = max_pos.max(start);
            (Just(start), start..=max_old, 1..=64usize)
        })
        .prop_map(|(start, old_end, new_len)| {
            let new_end = start.saturating_add(new_len);
            make_edit(start, old_end, new_end)
        })
}

// =====================================================================
// Property-based tests
// =====================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    // ----- 1. Insert preserves root start_byte -----
    #[test]
    fn insert_preserves_root_start(
        tree_len in 10..500usize,
        pos in 0..500usize,
        insert_len in 1..100usize,
    ) {
        let pos = clamp_to_tree(pos, tree_len);
        let mut tree = two_child_tree(tree_len);
        let orig_start = tree.root_node().start_byte();
        let edit = make_edit(pos, pos, pos + insert_len);
        if tree.edit(&edit).is_ok() {
            // Root start should stay at 0 unless edit is at byte 0
            // (intersecting the root start).
            prop_assert!(tree.root_node().start_byte() <= orig_start || pos == 0);
        }
    }

    // ----- 2. Insert grows root end_byte -----
    #[test]
    fn insert_grows_root_end(
        tree_len in 10..500usize,
        pos in 0..500usize,
        insert_len in 1..100usize,
    ) {
        let pos = clamp_to_tree(pos, tree_len);
        let mut tree = two_child_tree(tree_len);
        let orig_end = tree.root_node().end_byte();
        let edit = make_edit(pos, pos, pos + insert_len);
        if tree.edit(&edit).is_ok() {
            prop_assert!(tree.root_node().end_byte() >= orig_end);
        }
    }

    // ----- 3. Delete shrinks or keeps root end_byte -----
    #[test]
    fn delete_shrinks_root_end(
        tree_len in 10..500usize,
        start in 0..500usize,
        del_len in 1..100usize,
    ) {
        let start = clamp_to_tree(start, tree_len);
        let old_end = bounded_old_end(start, del_len, tree_len, 100);
        let mut tree = two_child_tree(tree_len);
        let orig_end = tree.root_node().end_byte();
        let edit = make_edit(start, old_end, start);
        if tree.edit(&edit).is_ok() {
            prop_assert!(tree.root_node().end_byte() <= orig_end);
        }
    }

    // ----- 4. Replace adjusts root end_byte -----
    #[test]
    fn replace_adjusts_root_end(
        tree_len in 10..500usize,
        start in 0..500usize,
        old_span in 1..50usize,
        new_span in 1..50usize,
    ) {
        let start = clamp_to_tree(start, tree_len);
        let old_end = start + old_span;
        let new_end = start + new_span;
        let mut tree = two_child_tree(tree_len);
        let orig_end = tree.root_node().end_byte();
        let edit = make_edit(start, old_end, new_end);
        if tree.edit(&edit).is_ok() {
            // Root end should have been adjusted; just verify it's non-negative
            // and the tree is still structurally valid.
            let new_root_end = tree.root_node().end_byte();
            prop_assert!(new_root_end <= orig_end.saturating_add(new_span) + old_span);
        }
    }

    // ----- 5. All nodes keep start <= end after insert -----
    #[test]
    fn insert_all_ranges_valid(
        tree_len in 10..200usize,
        pos in 0..200usize,
        insert_len in 1..50usize,
    ) {
        let pos = clamp_to_tree(pos, tree_len);
        let mut tree = two_child_tree(tree_len);
        let edit = make_edit(pos, pos, pos + insert_len);
        if tree.edit(&edit).is_ok() {
            for (s, e) in collect_ranges(&tree) {
                prop_assert!(s <= e, "start {} > end {}", s, e);
            }
        }
    }

    // ----- 6. All nodes keep start <= end after delete -----
    #[test]
    fn delete_all_ranges_valid(
        tree_len in 10..200usize,
        start in 0..200usize,
        del_len in 1..50usize,
    ) {
        let start = clamp_to_tree(start, tree_len);
        let old_end = bounded_old_end(start, del_len, tree_len, 50);
        let mut tree = two_child_tree(tree_len);
        let edit = make_edit(start, old_end, start);
        if tree.edit(&edit).is_ok() {
            for (s, e) in collect_ranges(&tree) {
                prop_assert!(s <= e, "start {} > end {}", s, e);
            }
        }
    }

    // ----- 7. All nodes keep start <= end after replace -----
    #[test]
    fn replace_all_ranges_valid(
        tree_len in 10..200usize,
        start in 0..200usize,
        old_span in 1..30usize,
        new_span in 1..30usize,
    ) {
        let start = clamp_to_tree(start, tree_len);
        let old_end = start + old_span;
        let new_end = start + new_span;
        let mut tree = two_child_tree(tree_len);
        let edit = make_edit(start, old_end, new_end);
        if tree.edit(&edit).is_ok() {
            for (s, e) in collect_ranges(&tree) {
                prop_assert!(s <= e, "start {} > end {}", s, e);
            }
        }
    }

    // ----- 8. Invalid range: old_end < start yields error -----
    #[test]
    fn invalid_range_old_end_before_start(
        start in 1..1000usize,
        deficit in 1..100usize,
    ) {
        let old_end = start.saturating_sub(deficit);
        prop_assume!(old_end < start);
        let mut tree = two_child_tree(100);
        let edit = make_edit(start, old_end, start + 5);
        let result = tree.edit(&edit);
        let is_invalid = matches!(result, Err(EditError::InvalidRange { .. }));
        prop_assert!(is_invalid);
    }

    // ----- 9. Invalid range: new_end < start yields error -----
    #[test]
    fn invalid_range_new_end_before_start(
        start in 1..1000usize,
        deficit in 1..100usize,
    ) {
        let new_end = start.saturating_sub(deficit);
        prop_assume!(new_end < start);
        let mut tree = two_child_tree(100);
        let edit = make_edit(start, start + 5, new_end);
        let result = tree.edit(&edit);
        let is_invalid = matches!(result, Err(EditError::InvalidRange { .. }));
        prop_assert!(is_invalid);
    }

    // ----- 10. Deep clone after insert is independent -----
    #[test]
    fn clone_independent_after_insert(
        tree_len in 10..200usize,
        pos in 0..200usize,
        insert_len in 1..50usize,
    ) {
        let pos = clamp_to_tree(pos, tree_len);
        let mut tree = two_child_tree(tree_len);
        let edit = make_edit(pos, pos, pos + insert_len);
        if tree.edit(&edit).is_ok() {
            let cloned = tree.clone();
            let orig_end = tree.root_node().end_byte();
            let clone_end = cloned.root_node().end_byte();
            prop_assert_eq!(orig_end, clone_end);
        }
    }

    // ----- 11. Clone before edit stays unchanged -----
    #[test]
    fn clone_before_edit_unchanged(
        tree_len in 10..200usize,
        pos in 0..200usize,
        insert_len in 1..50usize,
    ) {
        let pos = clamp_to_tree(pos, tree_len);
        let original = two_child_tree(tree_len);
        let mut edited = original.clone();
        let edit = make_edit(pos, pos, pos + insert_len);
        if edited.edit(&edit).is_ok() {
            // Original should be unmodified
            prop_assert_eq!(original.root_node().end_byte(), tree_len);
        }
    }

    // ----- 12. Deep tree insert ranges valid -----
    #[test]
    fn deep_tree_insert_ranges_valid(
        tree_len in 20..300usize,
        pos in 0..300usize,
        insert_len in 1..50usize,
    ) {
        let pos = clamp_to_tree(pos, tree_len);
        let mut tree = deep_tree(tree_len);
        let edit = make_edit(pos, pos, pos + insert_len);
        if tree.edit(&edit).is_ok() {
            for (s, e) in collect_ranges(&tree) {
                prop_assert!(s <= e, "start {} > end {}", s, e);
            }
        }
    }

    // ----- 13. Deep tree delete ranges valid -----
    #[test]
    fn deep_tree_delete_ranges_valid(
        tree_len in 20..300usize,
        start in 0..300usize,
        del_len in 1..50usize,
    ) {
        let start = clamp_to_tree(start, tree_len);
        let old_end = bounded_old_end(start, del_len, tree_len, 50);
        let mut tree = deep_tree(tree_len);
        let edit = make_edit(start, old_end, start);
        if tree.edit(&edit).is_ok() {
            for (s, e) in collect_ranges(&tree) {
                prop_assert!(s <= e, "start {} > end {}", s, e);
            }
        }
    }

    // ----- 14. Zero-length edit (no-op) is accepted -----
    #[test]
    fn zero_length_edit_noop(
        tree_len in 10..200usize,
        pos in 0..200usize,
    ) {
        let pos = clamp_to_tree(pos, tree_len);
        let mut tree = two_child_tree(tree_len);
        let orig_end = tree.root_node().end_byte();
        let edit = make_edit(pos, pos, pos);
        let result = tree.edit(&edit);
        prop_assert!(result.is_ok());
        prop_assert_eq!(tree.root_node().end_byte(), orig_end);
    }

    // ----- 15. Sequential inserts accumulate size -----
    #[test]
    fn sequential_inserts_accumulate(
        tree_len in 50..200usize,
        insert_a in 1..20usize,
        insert_b in 1..20usize,
    ) {
        let mut tree = two_child_tree(tree_len);
        let e1 = make_edit(0, 0, insert_a);
        let ok1 = tree.edit(&e1).is_ok();
        if ok1 {
            let after_first = tree.root_node().end_byte();
            let e2 = make_edit(0, 0, insert_b);
            if tree.edit(&e2).is_ok() {
                prop_assert!(tree.root_node().end_byte() >= after_first);
            }
        }
    }

    // ----- 16. Insert at end: root end_byte == start means node is before edit -----
    #[test]
    fn insert_at_end_root_unaffected(
        tree_len in 10..200usize,
        insert_len in 1..50usize,
    ) {
        let mut tree = two_child_tree(tree_len);
        // When start == root.end_byte the node is "before" the edit
        let edit = make_edit(tree_len, tree_len, tree_len + insert_len);
        if tree.edit(&edit).is_ok() {
            // Root end stays the same because end_byte <= start_byte
            prop_assert_eq!(tree.root_node().end_byte(), tree_len);
        }
    }

    // ----- 17. Insert before tree shifts nodes after -----
    #[test]
    fn insert_before_tree_shifts_children(
        tree_len in 20..200usize,
        insert_len in 1..50usize,
    ) {
        let mut tree = two_child_tree(tree_len);
        let mid = tree_len / 2;
        let orig_c2_start = tree.root_node().child(1).map(|n| n.start_byte());
        // Insert at the start
        let edit = make_edit(0, 0, insert_len);
        if tree.edit(&edit).is_ok() {
            // Second child should have shifted
            if let Some(c2) = tree.root_node().child(1) {
                if let Some(orig) = orig_c2_start {
                    // Child 2 starts at mid originally; after insert it should be >= mid
                    prop_assert!(c2.start_byte() >= mid);
                }
            }
        }
    }

    // ----- 18. Deletion that spans entire tree -----
    #[test]
    fn delete_entire_tree(tree_len in 10..200usize) {
        let mut tree = two_child_tree(tree_len);
        let edit = make_edit(0, tree_len, 0);
        if tree.edit(&edit).is_ok() {
            // Root end should be 0 after deleting everything
            prop_assert_eq!(tree.root_node().end_byte(), 0);
        }
    }

    // ----- 19. Replacement preserves node count -----
    #[test]
    fn replace_preserves_node_count(
        tree_len in 10..200usize,
        start in 0..200usize,
        old_span in 1..30usize,
        new_span in 1..30usize,
    ) {
        let start = clamp_to_tree(start, tree_len);
        let old_end = start + old_span;
        let new_end = start + new_span;
        let orig_count = {
            let tree = two_child_tree(tree_len);
            collect_ranges(&tree).len()
        };
        let mut tree = two_child_tree(tree_len);
        let edit = make_edit(start, old_end, new_end);
        if tree.edit(&edit).is_ok() {
            let new_count = collect_ranges(&tree).len();
            // Edit only adjusts ranges, not structure
            prop_assert_eq!(orig_count, new_count);
        }
    }

    // ----- 20. Clone after delete is independent -----
    #[test]
    fn clone_independent_after_delete(
        tree_len in 10..200usize,
        start in 0..200usize,
        del_len in 1..50usize,
    ) {
        let start = clamp_to_tree(start, tree_len);
        let old_end = bounded_old_end(start, del_len, tree_len, 50);
        let mut tree = two_child_tree(tree_len);
        let edit = make_edit(start, old_end, start);
        if tree.edit(&edit).is_ok() {
            let cloned = tree.clone();
            let snapshot = tree.root_node().end_byte();
            prop_assert_eq!(cloned.root_node().end_byte(), snapshot);
        }
    }

    // ----- 21. Edit on stub tree: root [0,0] is "before" an edit at byte 0 -----
    #[test]
    fn edit_stub_tree_insert(insert_len in 1..100usize) {
        let mut tree = Tree::new_stub();
        let edit = make_edit(0, 0, insert_len);
        let result = tree.edit(&edit);
        prop_assert!(result.is_ok());
        // Stub root has end_byte == 0 which satisfies end_byte <= start_byte,
        // so the node is treated as "completely before" the edit and unchanged.
        prop_assert_eq!(tree.root_node().end_byte(), 0);
    }

    // ----- 22. Insert then delete restores original end -----
    #[test]
    fn insert_then_delete_roundtrip(
        tree_len in 10..200usize,
        pos in 0..200usize,
        span in 1..50usize,
    ) {
        let pos = clamp_to_tree(pos, tree_len);
        let mut tree = two_child_tree(tree_len);
        let orig_end = tree.root_node().end_byte();
        // Insert
        let ins = make_edit(pos, pos, pos + span);
        if tree.edit(&ins).is_ok() {
            // Delete the same region
            let del = make_edit(pos, pos + span, pos);
            if tree.edit(&del).is_ok() {
                prop_assert_eq!(tree.root_node().end_byte(), orig_end);
            }
        }
    }

    // ----- 23. Byte range strategy: insertion never returns InvalidRange -----
    #[test]
    fn insertion_strategy_never_invalid(edit in insertion_edit_strategy(500)) {
        let mut tree = two_child_tree(200);
        let result = tree.edit(&edit);
        let is_invalid = matches!(result, Err(EditError::InvalidRange { .. }));
        prop_assert!(!is_invalid);
    }

    // ----- 24. Byte range strategy: deletion never returns InvalidRange -----
    #[test]
    fn deletion_strategy_never_invalid(edit in deletion_edit_strategy(500)) {
        let mut tree = two_child_tree(200);
        let result = tree.edit(&edit);
        let is_invalid = matches!(result, Err(EditError::InvalidRange { .. }));
        prop_assert!(!is_invalid);
    }

    // ----- 25. Byte range strategy: replacement never returns InvalidRange -----
    #[test]
    fn replacement_strategy_never_invalid(edit in replacement_edit_strategy(500)) {
        let mut tree = two_child_tree(200);
        let result = tree.edit(&edit);
        let is_invalid = matches!(result, Err(EditError::InvalidRange { .. }));
        prop_assert!(!is_invalid);
    }

    // ----- 26. Deep clone after replace preserves all ranges -----
    #[test]
    fn deep_clone_preserves_all_ranges_after_replace(
        tree_len in 20..200usize,
        start in 0..200usize,
        old_span in 1..20usize,
        new_span in 1..20usize,
    ) {
        let start = clamp_to_tree(start, tree_len);
        let old_end = start + old_span;
        let new_end = start + new_span;
        let mut tree = deep_tree(tree_len);
        let edit = make_edit(start, old_end, new_end);
        if tree.edit(&edit).is_ok() {
            let cloned = tree.clone();
            let orig_ranges = collect_ranges(&tree);
            let clone_ranges = collect_ranges(&cloned);
            prop_assert_eq!(orig_ranges, clone_ranges);
        }
    }

    // ----- 27. Multiple sequential edits keep ranges valid -----
    #[test]
    fn multiple_edits_keep_ranges_valid(
        tree_len in 50..200usize,
        a_pos in 0..50usize,
        a_len in 1..10usize,
        b_pos in 0..50usize,
        b_len in 1..10usize,
    ) {
        let mut tree = two_child_tree(tree_len);
        let e1 = make_edit(a_pos, a_pos, a_pos + a_len);
        let _ = tree.edit(&e1);
        let e2 = make_edit(b_pos, b_pos, b_pos + b_len);
        let _ = tree.edit(&e2);
        for (s, e) in collect_ranges(&tree) {
            prop_assert!(s <= e, "start {} > end {}", s, e);
        }
    }

    // ----- 28. Edit at exact boundary of child -----
    #[test]
    fn edit_at_child_boundary(
        tree_len in 20..200usize,
        insert_len in 1..30usize,
    ) {
        let mid = tree_len / 2;
        let mut tree = two_child_tree(tree_len);
        let edit = make_edit(mid, mid, mid + insert_len);
        if tree.edit(&edit).is_ok() {
            for (s, e) in collect_ranges(&tree) {
                prop_assert!(s <= e, "start {} > end {}", s, e);
            }
        }
    }

    // ----- 29. Root kind unchanged by edit -----
    #[test]
    fn root_kind_unchanged_by_edit(
        tree_len in 10..200usize,
        pos in 0..200usize,
        new_len in 1..50usize,
    ) {
        let pos = clamp_to_tree(pos, tree_len);
        let mut tree = two_child_tree(tree_len);
        let orig_kind = tree.root_kind();
        let edit = make_edit(pos, pos, pos + new_len);
        let _ = tree.edit(&edit);
        prop_assert_eq!(tree.root_kind(), orig_kind);
    }

    // ----- 30. Child count unchanged by edit -----
    #[test]
    fn child_count_unchanged_by_edit(
        tree_len in 10..200usize,
        pos in 0..200usize,
        new_len in 1..50usize,
    ) {
        let pos = clamp_to_tree(pos, tree_len);
        let mut tree = two_child_tree(tree_len);
        let orig_count = tree.root_node().child_count();
        let edit = make_edit(pos, pos, pos + new_len);
        let _ = tree.edit(&edit);
        prop_assert_eq!(tree.root_node().child_count(), orig_count);
    }

    // ----- 31. Edit with usize::MAX triggers overflow/underflow or succeeds -----
    #[test]
    fn overflow_detection(
        start in 0..100usize,
    ) {
        let mut tree = two_child_tree(100);
        let edit = make_edit(start, start, usize::MAX);
        let result = tree.edit(&edit);
        // Must either succeed or return an arithmetic error
        match result {
            Ok(()) => { /* acceptable */ }
            Err(EditError::ArithmeticOverflow) => { /* expected */ }
            Err(EditError::ArithmeticUnderflow) => { /* also possible */ }
            Err(other) => prop_assert!(false, "unexpected error: {:?}", other),
        }
    }

    // ----- 32. Cursor traversal works after edit -----
    #[test]
    fn cursor_traversal_after_edit(
        tree_len in 20..200usize,
        pos in 0..200usize,
        insert_len in 1..50usize,
    ) {
        let pos = clamp_to_tree(pos, tree_len);
        let mut tree = two_child_tree(tree_len);
        let edit = make_edit(pos, pos, pos + insert_len);
        if tree.edit(&edit).is_ok() {
            let mut cursor = TreeCursor::new(&tree);
            // Should be able to traverse without panics
            let root_node = cursor.node();
            prop_assert!(root_node.start_byte() <= root_node.end_byte());
            if cursor.goto_first_child() {
                let child = cursor.node();
                prop_assert!(child.start_byte() <= child.end_byte());
                cursor.goto_parent();
            }
        }
    }
}

// =====================================================================
// Non-proptest parametric tests (counted toward 25-35 total)
// =====================================================================

#[test]
fn edit_error_display_invalid_range() {
    let err = EditError::InvalidRange {
        start: 10,
        old_end: 5,
    };
    let msg = format!("{err}");
    assert!(msg.contains("10"));
    assert!(msg.contains("5"));
}

#[test]
fn edit_error_display_overflow() {
    let err = EditError::ArithmeticOverflow;
    let msg = format!("{err}");
    assert!(msg.contains("overflow"));
}

#[test]
fn edit_error_display_underflow() {
    let err = EditError::ArithmeticUnderflow;
    let msg = format!("{err}");
    assert!(msg.contains("underflow"));
}

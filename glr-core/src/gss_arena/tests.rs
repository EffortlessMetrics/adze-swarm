use super::*;
use crate::{StateId, SymbolId};
use typed_arena::Arena;

#[test]
fn test_arena_gss_basic() {
    let arena = Arena::new();
    let mut gss = ArenaGSS::new(&arena, StateId(0));

    gss.push(0, StateId(1), Some(SymbolId(10)));
    gss.push(0, StateId(2), Some(SymbolId(20)));

    assert_eq!(gss.top_state(0), StateId(2));

    let fork_idx = gss.fork_head(0);
    assert_eq!(gss.active_heads.len(), 2);

    gss.push(0, StateId(3), None);
    gss.push(fork_idx, StateId(4), None);

    assert_ne!(gss.top_state(0), gss.top_state(fork_idx));
}

#[test]
fn test_arena_gss_shared_memory() {
    let arena = Arena::new();
    let mut gss = ArenaGSS::new(&arena, StateId(0));

    gss.push(0, StateId(1), None);
    gss.push(0, StateId(2), None);

    let fork1 = gss.fork_head(0);
    let fork2 = gss.fork_head(0);

    // All heads should share the same parent
    assert!(gss.active_heads[0].shares_prefix_with(gss.active_heads[fork1]));
    assert!(gss.active_heads[0].shares_prefix_with(gss.active_heads[fork2]));

    // Parent pointers should be identical (same memory location)
    assert!(std::ptr::eq(
        gss.active_heads[0].parent.unwrap(),
        gss.active_heads[fork1].parent.unwrap()
    ));
}

#[test]
fn test_arena_manager() {
    let manager = ArenaGSSManager::new();

    {
        let mut gss = manager.new_session(StateId(0));
        gss.push(0, StateId(1), None);
        gss.push(0, StateId(2), None);

        assert_eq!(gss.top_state(0), StateId(2));
        assert_eq!(gss.stats.total_nodes_created, 3);
    }

    // Session ends, but arena memory is still allocated
    // In production, we'd clear the arena between parsing sessions
}

#[test]
fn arena_stack_node_get_states_returns_root_first() {
    let arena = Arena::new();
    let mut gss = ArenaGSS::new(&arena, StateId(0));
    gss.push(0, StateId(1), None);
    gss.push(0, StateId(2), None);

    let states = gss.active_heads[0].get_states();
    assert_eq!(states, vec![StateId(0), StateId(1), StateId(2)]);
}

#[test]
fn arena_stack_node_get_states_single_root() {
    let arena = Arena::new();
    let gss = ArenaGSS::new(&arena, StateId(11));
    assert_eq!(gss.active_heads[0].get_states(), vec![StateId(11)]);
}

#[test]
fn arena_stack_node_shares_prefix_with_both_roots() {
    let arena = Arena::new();
    let gss_a = ArenaGSS::new(&arena, StateId(0));
    let gss_b = ArenaGSS::new(&arena, StateId(0));
    assert!(gss_a.active_heads[0].shares_prefix_with(gss_b.active_heads[0]));
}

#[test]
fn arena_stack_node_shares_prefix_with_distinct_parents() {
    let arena = Arena::new();
    let mut gss_a = ArenaGSS::new(&arena, StateId(0));
    let mut gss_b = ArenaGSS::new(&arena, StateId(0));
    gss_a.push(0, StateId(1), None);
    gss_b.push(0, StateId(1), None);
    // Different root parents (separate allocations) => no shared prefix.
    assert!(!gss_a.active_heads[0].shares_prefix_with(gss_b.active_heads[0]));
}

#[test]
fn arena_stack_node_shares_prefix_with_root_vs_child() {
    let arena = Arena::new();
    let gss_root = ArenaGSS::new(&arena, StateId(0));
    let mut gss_child = ArenaGSS::new(&arena, StateId(0));
    gss_child.push(0, StateId(1), None);
    // A root (no parent) vs a child (has parent) should never share prefix.
    assert!(!gss_root.active_heads[0].shares_prefix_with(gss_child.active_heads[0]));
    assert!(!gss_child.active_heads[0].shares_prefix_with(gss_root.active_heads[0]));
}

#[test]
fn arena_stack_node_shares_prefix_with_after_fork_push() {
    let arena = Arena::new();
    let mut gss = ArenaGSS::new(&arena, StateId(0));
    gss.push(0, StateId(1), None);
    let fork = gss.fork_head(0);
    // Forked heads share the same parent (the original root).
    assert!(gss.active_heads[0].shares_prefix_with(gss.active_heads[fork]));
    gss.push(fork, StateId(2), None);
    // After pushing onto fork, parents differ — fork's new parent is the
    // pre-push fork head (state=1), while head 0's parent is the initial root.
    assert!(!gss.active_heads[0].shares_prefix_with(gss.active_heads[fork]));
}

#[test]
fn arena_gss_can_merge_rejects_same_index() {
    let arena = Arena::new();
    let mut gss = ArenaGSS::new(&arena, StateId(0));
    gss.push(0, StateId(1), None);
    assert!(!gss.can_merge(0, 0));
}

#[test]
fn arena_gss_manager_clear_resets_arena() {
    let mut manager = ArenaGSSManager::new();
    {
        let mut gss = manager.new_session(StateId(0));
        for s in 1..=4 {
            gss.push(0, StateId(s), None);
        }
        assert_eq!(gss.stats.total_nodes_created, 5);
    }
    manager.clear();
    // After clear, a fresh session starts with a single node.
    let gss = manager.new_session(StateId(99));
    assert_eq!(gss.stats.total_nodes_created, 1);
    assert_eq!(gss.top_state(0), StateId(99));
}

//! Tests for the Graph Structured Stack (GSS) module.

use adze_glr_core::StateId;
use adze_glr_core::gss::{GSSStats, GraphStructuredStack, StackNode};

#[test]
fn gss_new_has_single_head() {
    let gss = GraphStructuredStack::new(StateId(0));
    assert_eq!(gss.top_state(0), StateId(0));
}

#[test]
fn gss_push_updates_top_state() {
    let mut gss = GraphStructuredStack::new(StateId(0));
    gss.push(0, StateId(5), None);
    assert_eq!(gss.top_state(0), StateId(5));
}

#[test]
fn gss_fork_creates_new_head() {
    let mut gss = GraphStructuredStack::new(StateId(0));
    let new_idx = gss.fork_head(0);
    assert_ne!(new_idx, 0);
    assert_eq!(gss.top_state(new_idx), StateId(0));
}

#[test]
fn gss_pop_returns_states() {
    let mut gss = GraphStructuredStack::new(StateId(0));
    gss.push(0, StateId(1), None);
    gss.push(0, StateId(2), None);
    let popped = gss.pop(0, 2);
    assert!(popped.is_some());
}

#[test]
fn gss_pop_zero_succeeds() {
    let mut gss = GraphStructuredStack::new(StateId(0));
    let popped = gss.pop(0, 0);
    assert!(popped.is_some());
}

#[test]
fn gss_can_merge_same_state() {
    let mut gss = GraphStructuredStack::new(StateId(0));
    let fork = gss.fork_head(0);
    assert!(gss.can_merge(0, fork));
}

#[test]
fn gss_can_merge_different_states() {
    let mut gss = GraphStructuredStack::new(StateId(0));
    let fork = gss.fork_head(0);
    gss.push(fork, StateId(1), None);
    assert!(!gss.can_merge(0, fork));
}

#[test]
fn gss_merge_heads() {
    let mut gss = GraphStructuredStack::new(StateId(0));
    let fork = gss.fork_head(0);
    gss.merge_heads(0, fork);
}

#[test]
fn gss_mark_completed() {
    let mut gss = GraphStructuredStack::new(StateId(0));
    gss.mark_completed(0);
}

#[test]
fn gss_try_fork_head_invalid_index_returns_none() {
    let mut gss = GraphStructuredStack::new(StateId(0));
    assert_eq!(gss.try_fork_head(3), None);
    assert_eq!(gss.active_heads.len(), 1);
}

#[test]
fn gss_try_mark_completed_invalid_index_returns_false() {
    let mut gss = GraphStructuredStack::new(StateId(0));
    assert!(!gss.try_mark_completed(3));
    assert_eq!(gss.active_heads.len(), 1);
    assert!(gss.completed_heads.is_empty());
}

#[test]
fn gss_stats_initial() {
    let gss = GraphStructuredStack::new(StateId(0));
    let stats = gss.get_stats();
    assert_eq!(stats.total_forks, 0);
    assert_eq!(stats.total_merges, 0);
}

#[test]
fn gss_stats_after_fork() {
    let mut gss = GraphStructuredStack::new(StateId(0));
    let _fork = gss.fork_head(0);
    let stats = gss.get_stats();
    assert_eq!(stats.total_forks, 1);
}

#[test]
fn gss_deduplicate() {
    let mut gss = GraphStructuredStack::new(StateId(0));
    gss.deduplicate();
    // Should not panic on single head
}

#[test]
fn stack_node_new() {
    let node = StackNode::new(StateId(3), None, None);
    assert_eq!(node.state, StateId(3));
}

#[test]
fn stack_node_get_states() {
    let node = StackNode::new(StateId(5), None, None);
    let states = node.get_states();
    assert!(states.contains(&StateId(5)));
}

#[test]
fn stack_node_shares_prefix_self() {
    let node = StackNode::new(StateId(0), None, None);
    assert!(node.shares_prefix_with(&node));
}

#[test]
fn gss_stats_debug() {
    let stats = GSSStats {
        total_nodes_created: 20,
        max_active_heads: 10,
        total_forks: 5,
        total_merges: 3,
        shared_segments: 2,
    };
    let debug = format!("{stats:?}");
    assert!(debug.contains("GSSStats"));
}

// Graph-Structured Stack (GSS) for efficient GLR parsing
//! Graph-Structured Stack (GSS) for efficient GLR fork/merge.

// This implementation uses shared stack segments to avoid unnecessary copying during forking

use crate::{StateId, SymbolId};
use std::rc::Rc;

/// A node in the graph-structured stack
/// Uses Rc for shared ownership, allowing multiple heads to share the same tail
#[derive(Debug, Clone)]
pub struct StackNode {
    pub state: StateId,
    pub symbol: Option<SymbolId>,
    pub parent: Option<Rc<StackNode>>,
    pub depth: usize,
}

impl StackNode {
    /// Create a new stack node
    pub fn new(state: StateId, symbol: Option<SymbolId>, parent: Option<Rc<StackNode>>) -> Self {
        let depth = parent.as_ref().map_or(0, |p| p.depth + 1);
        Self {
            state,
            symbol,
            parent,
            depth,
        }
    }

    /// Get the states from this node back to the root
    pub fn get_states(&self) -> Vec<StateId> {
        let mut states = Vec::with_capacity(self.depth + 1);
        let mut current = Some(self);

        while let Some(node) = current {
            states.push(node.state);
            current = node.parent.as_deref();
        }

        states.reverse();
        states
    }

    /// Check if this stack shares a common prefix with another
    pub fn shares_prefix_with(&self, other: &StackNode) -> bool {
        // Use Rc pointer equality for fast comparison
        if let (Some(p1), Some(p2)) = (&self.parent, &other.parent) {
            Rc::ptr_eq(p1, p2)
        } else {
            self.parent.is_none() && other.parent.is_none()
        }
    }
}

/// Graph-Structured Stack for GLR parsing
pub struct GraphStructuredStack {
    /// Active stack heads that are being processed
    pub active_heads: Vec<Rc<StackNode>>,
    /// Completed stack heads (for accepted parses)
    pub completed_heads: Vec<Rc<StackNode>>,
    /// Statistics for performance monitoring
    pub stats: GSSStats,
}

#[derive(Debug, Default)]
pub struct GSSStats {
    pub total_nodes_created: usize,
    pub max_active_heads: usize,
    pub total_forks: usize,
    pub total_merges: usize,
    pub shared_segments: usize,
}

impl GraphStructuredStack {
    /// Create a new GSS with initial state
    pub fn new(initial_state: StateId) -> Self {
        let initial_node = Rc::new(StackNode::new(initial_state, None, None));

        Self {
            active_heads: vec![initial_node],
            completed_heads: Vec::new(),
            stats: GSSStats {
                total_nodes_created: 1,
                max_active_heads: 1,
                ..Default::default()
            },
        }
    }

    /// Fork a stack head, creating a new head that shares the same parent
    pub fn fork_head(&mut self, head_idx: usize) -> usize {
        let head = self.active_heads[head_idx].clone();
        self.active_heads.push(head);

        self.stats.total_forks += 1;
        self.stats.max_active_heads = self.stats.max_active_heads.max(self.active_heads.len());

        self.active_heads.len() - 1
    }

    /// Push a new state onto a stack head
    pub fn push(&mut self, head_idx: usize, state: StateId, symbol: Option<SymbolId>) {
        let parent = Some(self.active_heads[head_idx].clone());
        let new_node = Rc::new(StackNode::new(state, symbol, parent));

        self.active_heads[head_idx] = new_node;
        self.stats.total_nodes_created += 1;
    }

    /// Pop states from a stack head
    pub fn pop(&mut self, head_idx: usize, count: usize) -> Option<Vec<StateId>> {
        let mut current = Some(self.active_heads[head_idx].as_ref());
        let mut popped_states = Vec::with_capacity(count);

        // Collect the states being popped
        for _ in 0..count {
            match current {
                Some(node) => {
                    popped_states.push(node.state);
                    current = node.parent.as_deref();
                }
                None => return None, // Not enough states to pop
            }
        }

        // Update the head to point to the new top
        if let Some(node) = current {
            // The current node becomes the new head
            // We need to clone it into an Rc
            self.active_heads[head_idx] =
                Rc::new(StackNode::new(node.state, node.symbol, node.parent.clone()));
        }

        popped_states.reverse();
        Some(popped_states)
    }

    /// Get the top state of a stack head
    pub fn top_state(&self, head_idx: usize) -> StateId {
        self.active_heads[head_idx].state
    }

    /// Check if two heads can be merged (same state and same parent)
    pub fn can_merge(&self, idx1: usize, idx2: usize) -> bool {
        if idx1 == idx2 {
            return false;
        }

        let head1 = &self.active_heads[idx1];
        let head2 = &self.active_heads[idx2];

        head1.state == head2.state && head1.shares_prefix_with(head2)
    }

    /// Merge two stack heads that share the same configuration
    pub fn merge_heads(&mut self, keep_idx: usize, remove_idx: usize) {
        if keep_idx != remove_idx && self.can_merge(keep_idx, remove_idx) {
            // Remove the duplicate head
            self.active_heads.remove(remove_idx);
            self.stats.total_merges += 1;

            // Count shared segments
            let head = &self.active_heads[keep_idx.min(self.active_heads.len() - 1)];
            if head.parent.is_some() {
                self.stats.shared_segments += 1;
            }
        }
    }

    /// Mark a head as completed (accepted)
    pub fn mark_completed(&mut self, head_idx: usize) {
        let head = self.active_heads.remove(head_idx);
        self.completed_heads.push(head);
    }

    /// Get statistics about the GSS
    pub fn get_stats(&self) -> &GSSStats {
        &self.stats
    }

    /// Check for and merge any duplicate heads
    pub fn deduplicate(&mut self) {
        let mut i = 0;
        while i < self.active_heads.len() {
            let mut j = i + 1;
            while j < self.active_heads.len() {
                if self.can_merge(i, j) {
                    self.merge_heads(i, j);
                    // Don't increment j since we removed an element
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gss_basic_operations() {
        let mut gss = GraphStructuredStack::new(StateId(0));

        // Push some states
        gss.push(0, StateId(1), Some(SymbolId(10)));
        gss.push(0, StateId(2), Some(SymbolId(20)));

        assert_eq!(gss.top_state(0), StateId(2));

        // Fork the stack
        let fork_idx = gss.fork_head(0);
        assert_eq!(gss.active_heads.len(), 2);

        // Both heads should have the same state initially
        assert_eq!(gss.top_state(0), gss.top_state(fork_idx));

        // Push different states to each fork
        gss.push(0, StateId(3), Some(SymbolId(30)));
        gss.push(fork_idx, StateId(4), Some(SymbolId(40)));

        // Now they should have different top states
        assert_ne!(gss.top_state(0), gss.top_state(fork_idx));
    }

    #[test]
    fn test_gss_shared_segments() {
        let mut gss = GraphStructuredStack::new(StateId(0));

        // Build up a stack
        gss.push(0, StateId(1), None);
        gss.push(0, StateId(2), None);

        // Fork it
        let fork1 = gss.fork_head(0);
        let fork2 = gss.fork_head(0);

        // All three heads share the same parent chain
        assert!(gss.active_heads[0].shares_prefix_with(&gss.active_heads[fork1]));
        assert!(gss.active_heads[0].shares_prefix_with(&gss.active_heads[fork2]));

        // Verify shared memory - parent pointers should be identical
        assert!(Rc::ptr_eq(
            gss.active_heads[0].parent.as_ref().unwrap(),
            gss.active_heads[fork1].parent.as_ref().unwrap()
        ));
    }

    #[test]
    fn test_gss_merge() {
        let mut gss = GraphStructuredStack::new(StateId(0));

        // Create two forks that end up in the same state
        gss.push(0, StateId(1), None);
        let fork = gss.fork_head(0);

        // Both push to the same state
        gss.push(0, StateId(2), None);
        gss.push(fork, StateId(2), None);

        // They should be mergeable
        assert!(gss.can_merge(0, fork));

        // Merge them
        gss.deduplicate();
        assert_eq!(gss.active_heads.len(), 1);
        assert_eq!(gss.stats.total_merges, 1);
    }

    #[test]
    fn test_gss_pop() {
        let mut gss = GraphStructuredStack::new(StateId(0));

        gss.push(0, StateId(1), None);
        gss.push(0, StateId(2), None);
        gss.push(0, StateId(3), None);

        let mut popped = gss.pop(0, 2).unwrap();
        popped.sort_by_key(|s| s.0);
        assert_eq!(popped, vec![StateId(2), StateId(3)]);
        assert_eq!(gss.top_state(0), StateId(1));
    }

    #[test]
    fn stack_node_new_root_has_depth_zero() {
        let node = StackNode::new(StateId(7), None, None);
        assert_eq!(node.state, StateId(7));
        assert_eq!(node.depth, 0);
        assert!(node.parent.is_none());
        assert!(node.symbol.is_none());
    }

    #[test]
    fn stack_node_new_with_parent_increments_depth() {
        let root = Rc::new(StackNode::new(StateId(0), None, None));
        let child = StackNode::new(StateId(1), Some(SymbolId(99)), Some(root.clone()));
        assert_eq!(child.depth, 1);
        assert_eq!(child.state, StateId(1));
        assert_eq!(child.symbol, Some(SymbolId(99)));

        let grandchild = StackNode::new(StateId(2), None, Some(Rc::new(child)));
        assert_eq!(grandchild.depth, 2);
    }

    #[test]
    fn stack_node_get_states_returns_root_first() {
        let root = Rc::new(StackNode::new(StateId(0), None, None));
        let mid = Rc::new(StackNode::new(StateId(1), None, Some(root.clone())));
        let top = StackNode::new(StateId(2), None, Some(mid));

        assert_eq!(top.get_states(), vec![StateId(0), StateId(1), StateId(2)]);
    }

    #[test]
    fn stack_node_get_states_single_root() {
        let only = StackNode::new(StateId(5), None, None);
        assert_eq!(only.get_states(), vec![StateId(5)]);
    }

    #[test]
    fn stack_node_shares_prefix_with_both_roots() {
        let a = StackNode::new(StateId(0), None, None);
        let b = StackNode::new(StateId(0), None, None);
        assert!(a.shares_prefix_with(&b));
    }

    #[test]
    fn stack_node_shares_prefix_with_same_parent_rc() {
        let root = Rc::new(StackNode::new(StateId(0), None, None));
        let a = StackNode::new(StateId(1), None, Some(root.clone()));
        let b = StackNode::new(StateId(2), None, Some(root));
        assert!(a.shares_prefix_with(&b));
    }

    #[test]
    fn stack_node_shares_prefix_with_distinct_parents() {
        let root_a = Rc::new(StackNode::new(StateId(0), None, None));
        let root_b = Rc::new(StackNode::new(StateId(0), None, None));
        let a = StackNode::new(StateId(1), None, Some(root_a));
        let b = StackNode::new(StateId(1), None, Some(root_b));
        assert!(!a.shares_prefix_with(&b));
    }

    #[test]
    fn stack_node_shares_prefix_with_one_root_one_child() {
        let root = Rc::new(StackNode::new(StateId(0), None, None));
        let child = StackNode::new(StateId(1), None, Some(root));
        let other_root = StackNode::new(StateId(0), None, None);
        assert!(!child.shares_prefix_with(&other_root));
        assert!(!other_root.shares_prefix_with(&child));
    }

    #[test]
    fn mark_completed_moves_head_off_active_list() {
        let mut gss = GraphStructuredStack::new(StateId(0));
        gss.push(0, StateId(1), None);
        let fork = gss.fork_head(0);
        gss.push(fork, StateId(7), None);

        let before_active = gss.active_heads.len();
        let before_completed = gss.completed_heads.len();

        gss.mark_completed(0);

        assert_eq!(gss.active_heads.len(), before_active - 1);
        assert_eq!(gss.completed_heads.len(), before_completed + 1);
        // The head moved to completed should retain its terminal state.
        assert_eq!(
            gss.completed_heads
                .last()
                .expect("completed_heads.last() should contain the moved head")
                .state,
            StateId(1)
        );
    }

    #[test]
    fn can_merge_returns_false_for_same_index() {
        let mut gss = GraphStructuredStack::new(StateId(0));
        gss.push(0, StateId(1), None);
        assert!(!gss.can_merge(0, 0));
    }
}

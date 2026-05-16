//! Parse stack state and structural comparison helpers.

use crate::subtree::Subtree;
use adze_glr_core::VersionInfo;
use adze_ir::StateId;
use std::sync::Arc;

/// A parse stack version (fork) in GLR parsing
#[derive(Debug, Clone)]
pub struct ParseStack {
    /// Stack of states
    pub(crate) states: Vec<StateId>,

    /// Stack of subtrees
    pub(crate) nodes: Vec<Arc<Subtree>>,

    /// Version tracking info for conflict resolution
    pub(crate) version: VersionInfo,

    /// Unique ID for this fork
    #[allow(dead_code)]
    pub(crate) id: usize,
}

impl ParseStack {
    pub(crate) fn new(initial_state: StateId, id: usize) -> Self {
        Self {
            states: vec![initial_state],
            nodes: vec![],
            version: VersionInfo::new(),
            id,
        }
    }

    /// Get the current state
    pub(crate) fn current_state(&self) -> StateId {
        self.states.last().copied().unwrap_or(StateId(0))
    }

    /// Push a new state and node
    pub(crate) fn push(&mut self, state: StateId, node: Arc<Subtree>) {
        // Update version info with dynamic precedence
        self.version.add_dynamic_prec(node.dynamic_prec);

        self.states.push(state);
        self.nodes.push(node);
    }

    /// Pop n states and nodes for a reduction
    pub(crate) fn pop(&mut self, n: usize) -> Vec<Arc<Subtree>> {
        if n >= self.states.len() {
            // Should not happen in valid LR parsing, but protect against overflow
            self.states.truncate(1); // Keep initial state
            return self.nodes.split_off(0);
        }
        self.states.truncate(self.states.len() - n);
        self.nodes.split_off(self.nodes.len() - n)
    }

    /// Clone this stack for forking
    pub(crate) fn fork(&self, new_id: usize) -> Self {
        Self {
            states: self.states.clone(),
            nodes: self.nodes.clone(),
            version: self.version.clone(),
            id: new_id,
        }
    }

    /// Print tree structure for debugging
    #[allow(dead_code)]
    pub(crate) fn print_tree_structure(node: &Arc<Subtree>, indent: usize) {
        let _prefix = "  ".repeat(indent);
        debug_glr!(
            "{}Symbol {}, range {:?}",
            _prefix,
            node.node.symbol_id.0,
            node.node.byte_range
        );
        for edge in &node.children {
            Self::print_tree_structure(&edge.subtree, indent + 1);
        }
    }

    /// Check if two stacks have structurally equivalent parse trees
    #[allow(dead_code)]
    pub(crate) fn has_equivalent_parse_tree(&self, other: &ParseStack) -> bool {
        // First check if they have the same number of nodes
        if self.nodes.len() != other.nodes.len() {
            return false;
        }

        // Check each node for structural equivalence
        for (node1, node2) in self.nodes.iter().zip(other.nodes.iter()) {
            if !Self::nodes_structurally_equivalent(node1, node2) {
                return false;
            }
        }

        true
    }

    /// Check if two subtree nodes are structurally equivalent
    #[allow(dead_code)]
    fn nodes_structurally_equivalent(node1: &Arc<Subtree>, node2: &Arc<Subtree>) -> bool {
        // Check symbol and span
        if node1.node.symbol_id != node2.node.symbol_id {
            return false;
        }

        if node1.node.byte_range != node2.node.byte_range {
            return false;
        }

        // Check if both are error nodes
        if node1.node.is_error != node2.node.is_error {
            return false;
        }

        // Check children structure
        if node1.children.len() != node2.children.len() {
            return false;
        }

        // Recursively check all children
        for (edge1, edge2) in node1.children.iter().zip(node2.children.iter()) {
            // Check field IDs match
            if edge1.field_id != edge2.field_id {
                return false;
            }
            // Check subtrees match
            if !Self::nodes_structurally_equivalent(&edge1.subtree, &edge2.subtree) {
                return false;
            }
        }

        true
    }
}

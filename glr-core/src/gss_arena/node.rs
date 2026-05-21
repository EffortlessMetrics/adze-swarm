use crate::{StateId, SymbolId};

/// A node in the arena-allocated graph-structured stack.
#[derive(Debug)]
pub struct ArenaStackNode<'a> {
    pub state: StateId,
    pub symbol: Option<SymbolId>,
    pub parent: Option<&'a ArenaStackNode<'a>>,
    pub depth: usize,
}

impl<'a> ArenaStackNode<'a> {
    /// Get the states from this node back to the root.
    pub fn get_states(&self) -> Vec<StateId> {
        let mut states = Vec::with_capacity(self.depth + 1);
        let mut current = Some(self);

        while let Some(node) = current {
            states.push(node.state);
            current = node.parent;
        }

        states.reverse();
        states
    }

    /// Check if this stack shares a common prefix with another.
    pub fn shares_prefix_with(&self, other: &ArenaStackNode<'a>) -> bool {
        match (self.parent, other.parent) {
            (Some(p1), Some(p2)) => std::ptr::eq(p1, p2),
            (None, None) => true,
            _ => false,
        }
    }
}

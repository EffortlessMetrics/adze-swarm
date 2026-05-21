use crate::{StateId, SymbolId};
use typed_arena::Arena;

use super::{ArenaGSSStats, ArenaStackNode};

/// Arena-allocated Graph-Structured Stack.
pub struct ArenaGSS<'a> {
    /// Arena for allocating stack nodes.
    pub(super) arena: &'a Arena<ArenaStackNode<'a>>,
    /// Active stack heads.
    pub active_heads: Vec<&'a ArenaStackNode<'a>>,
    /// Completed stack heads.
    pub completed_heads: Vec<&'a ArenaStackNode<'a>>,
    /// Statistics.
    pub stats: ArenaGSSStats,
}

impl<'a> ArenaGSS<'a> {
    /// Create a new arena-based GSS.
    pub fn new(arena: &'a Arena<ArenaStackNode<'a>>, initial_state: StateId) -> Self {
        let initial_node = arena.alloc(ArenaStackNode {
            state: initial_state,
            symbol: None,
            parent: None,
            depth: 0,
        });

        Self {
            arena,
            active_heads: vec![initial_node],
            completed_heads: Vec::new(),
            stats: ArenaGSSStats {
                total_nodes_created: 1,
                max_active_heads: 1,
                ..Default::default()
            },
        }
    }

    pub fn fork_head(&mut self, head_idx: usize) -> usize {
        let head = self.active_heads[head_idx];
        self.active_heads.push(head);

        self.stats.total_forks += 1;
        self.stats.max_active_heads = self.stats.max_active_heads.max(self.active_heads.len());

        self.active_heads.len() - 1
    }

    pub fn push(&mut self, head_idx: usize, state: StateId, symbol: Option<SymbolId>) {
        let parent = Some(self.active_heads[head_idx]);
        let depth = parent.map_or(0, |p| p.depth + 1);

        let new_node = self.arena.alloc(ArenaStackNode {
            state,
            symbol,
            parent,
            depth,
        });

        self.active_heads[head_idx] = new_node;
        self.stats.total_nodes_created += 1;
    }

    pub fn pop(&mut self, head_idx: usize, count: usize) -> Option<Vec<StateId>> {
        let mut current = Some(self.active_heads[head_idx]);
        let mut popped_states = Vec::with_capacity(count);

        for _ in 0..count {
            match current {
                Some(node) => {
                    popped_states.push(node.state);
                    current = node.parent;
                }
                None => return None,
            }
        }

        if let Some(node) = current {
            self.active_heads[head_idx] = node;
        }

        popped_states.reverse();
        Some(popped_states)
    }

    pub fn top_state(&self, head_idx: usize) -> StateId {
        self.active_heads[head_idx].state
    }

    pub fn can_merge(&self, idx1: usize, idx2: usize) -> bool {
        if idx1 == idx2 {
            return false;
        }

        let head1 = self.active_heads[idx1];
        let head2 = self.active_heads[idx2];

        head1.state == head2.state && head1.shares_prefix_with(head2)
    }

    pub fn merge_heads(&mut self, keep_idx: usize, remove_idx: usize) {
        if self.can_merge(keep_idx, remove_idx) {
            self.active_heads.remove(remove_idx);
            self.stats.total_merges += 1;
        }
    }

    pub fn deduplicate(&mut self) {
        let mut i = 0;
        while i < self.active_heads.len() {
            let mut j = i + 1;
            while j < self.active_heads.len() {
                if self.can_merge(i, j) {
                    self.merge_heads(i, j);
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
    }

    pub fn get_stats(&self) -> &ArenaGSSStats {
        &self.stats
    }
}

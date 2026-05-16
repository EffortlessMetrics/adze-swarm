// Arena-based GSS implementation for high-performance GLR parsing
//! Arena-allocated GSS implementation for high-performance parsing.

// This version uses an arena allocator to minimize allocation overhead

use crate::{StateId, SymbolId};
use typed_arena::Arena;

/// A node in the arena-allocated graph-structured stack
#[derive(Debug)]
pub struct ArenaStackNode<'a> {
    pub state: StateId,
    pub symbol: Option<SymbolId>,
    pub parent: Option<&'a ArenaStackNode<'a>>,
    pub depth: usize,
}

impl<'a> ArenaStackNode<'a> {
    /// Get the states from this node back to the root
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

    /// Check if this stack shares a common prefix with another
    pub fn shares_prefix_with(&self, other: &ArenaStackNode<'a>) -> bool {
        // Use pointer equality for fast comparison
        match (self.parent, other.parent) {
            (Some(p1), Some(p2)) => std::ptr::eq(p1, p2),
            (None, None) => true,
            _ => false,
        }
    }
}

/// Arena-allocated Graph-Structured Stack
pub struct ArenaGSS<'a> {
    /// Arena for allocating stack nodes
    arena: &'a Arena<ArenaStackNode<'a>>,
    /// Active stack heads
    pub active_heads: Vec<&'a ArenaStackNode<'a>>,
    /// Completed stack heads
    pub completed_heads: Vec<&'a ArenaStackNode<'a>>,
    /// Statistics
    pub stats: ArenaGSSStats,
}

#[derive(Debug, Default)]
pub struct ArenaGSSStats {
    pub total_nodes_created: usize,
    pub max_active_heads: usize,
    pub total_forks: usize,
    pub total_merges: usize,
    pub arena_bytes_allocated: usize,
}

impl<'a> ArenaGSS<'a> {
    /// Create a new arena-based GSS
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

    /// Fork a stack head
    pub fn fork_head(&mut self, head_idx: usize) -> usize {
        let head = self.active_heads[head_idx];
        self.active_heads.push(head);

        self.stats.total_forks += 1;
        self.stats.max_active_heads = self.stats.max_active_heads.max(self.active_heads.len());

        self.active_heads.len() - 1
    }

    /// Push a new state onto a stack head
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

    /// Pop states from a stack head
    pub fn pop(&mut self, head_idx: usize, count: usize) -> Option<Vec<StateId>> {
        let mut current = Some(self.active_heads[head_idx]);
        let mut popped_states = Vec::with_capacity(count);

        // Collect the states being popped
        for _ in 0..count {
            match current {
                Some(node) => {
                    popped_states.push(node.state);
                    current = node.parent;
                }
                None => return None,
            }
        }

        // Update the head
        if let Some(node) = current {
            self.active_heads[head_idx] = node;
        }

        popped_states.reverse();
        Some(popped_states)
    }

    /// Get the top state of a stack head
    pub fn top_state(&self, head_idx: usize) -> StateId {
        self.active_heads[head_idx].state
    }

    /// Check if two heads can be merged
    pub fn can_merge(&self, idx1: usize, idx2: usize) -> bool {
        if idx1 == idx2 {
            return false;
        }

        let head1 = self.active_heads[idx1];
        let head2 = self.active_heads[idx2];

        head1.state == head2.state && head1.shares_prefix_with(head2)
    }

    /// Merge duplicate heads
    pub fn merge_heads(&mut self, keep_idx: usize, remove_idx: usize) {
        if self.can_merge(keep_idx, remove_idx) {
            self.active_heads.remove(remove_idx);
            self.stats.total_merges += 1;
        }
    }

    /// Deduplicate all heads
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

    /// Get a reference to the GSS statistics
    pub fn get_stats(&self) -> &ArenaGSSStats {
        &self.stats
    }
}

/// Manager for arena-based GSS parsing sessions
pub struct ArenaGSSManager {
    arena: Arena<ArenaStackNode<'static>>,
}

impl Default for ArenaGSSManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ArenaGSSManager {
    /// Creates a new arena-based GSS manager.
    pub fn new() -> Self {
        Self {
            arena: Arena::new(),
        }
    }

    /// Create a new parsing session
    /// The lifetime of the GSS is tied to the arena
    pub fn new_session<'a>(&'a self, initial_state: StateId) -> ArenaGSS<'a> {
        // SAFETY: We transmute the arena's lifetime from 'static to 'a. This is sound
        // because: (1) the arena is owned by `self` and `&'a self` guarantees it lives
        // at least as long as 'a, (2) ArenaGSS<'a> cannot outlive 'a, so all arena-
        // allocated references are valid for their use.
        // TODO(safety): The 'static → 'a transmute relies on typed_arena's covariance
        // over the lifetime parameter of stored nodes. If Arena ever stores a
        // PhantomData<fn(&'a ())> (contravariant), this becomes unsound. Pin to a
        // specific typed_arena version or audit on upgrades.
        unsafe {
            let arena_ref = &*(&self.arena as *const Arena<ArenaStackNode<'static>>);
            let arena_transmuted = std::mem::transmute::<
                &Arena<ArenaStackNode<'static>>,
                &'a Arena<ArenaStackNode<'a>>,
            >(arena_ref);
            ArenaGSS::new(arena_transmuted, initial_state)
        }
    }

    /// Clear the arena for reuse
    pub fn clear(&mut self) {
        // Arena doesn't have a clear method, so we need to replace it
        self.arena = Arena::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

use typed_arena::Arena;

use crate::StateId;

use super::{ArenaGSS, ArenaStackNode};

/// Manager for arena-based GSS parsing sessions.
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

    /// Create a new parsing session.
    /// The lifetime of the GSS is tied to the arena.
    pub fn new_session<'a>(&'a self, initial_state: StateId) -> ArenaGSS<'a> {
        unsafe {
            let arena_ref = &*(&self.arena as *const Arena<ArenaStackNode<'static>>);
            let arena_transmuted = std::mem::transmute::<
                &Arena<ArenaStackNode<'static>>,
                &'a Arena<ArenaStackNode<'a>>,
            >(arena_ref);
            ArenaGSS::new(arena_transmuted, initial_state)
        }
    }

    /// Clear the arena for reuse.
    pub fn clear(&mut self) {
        self.arena = Arena::new();
    }
}

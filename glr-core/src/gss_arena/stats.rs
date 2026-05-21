#[derive(Debug, Default)]
pub struct ArenaGSSStats {
    pub total_nodes_created: usize,
    pub max_active_heads: usize,
    pub total_forks: usize,
    pub total_merges: usize,
    pub arena_bytes_allocated: usize,
}

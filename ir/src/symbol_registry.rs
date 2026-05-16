//! Deterministic symbol ID assignment via an ordered registry.

use crate::{SymbolId, SymbolMetadata};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A centralized registry for symbol ID assignment and metadata.
/// Ensures consistent, deterministic symbol ordering across all components.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolRegistry {
    /// Ordered map of symbol names to IDs (maintains insertion order)
    symbols: IndexMap<String, SymbolId>,
    /// Reverse lookup: ID to name
    ids: HashMap<SymbolId, String>,
    /// Metadata for each symbol
    metadata: HashMap<SymbolId, SymbolMetadata>,
    /// Next available symbol ID
    next_id: u16,
}

/// Metadata about a symbol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymbolInfo {
    /// Symbol ID
    pub id: SymbolId,
    /// Symbol metadata
    pub metadata: SymbolMetadata,
}

impl SymbolRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        let mut registry = Self {
            symbols: IndexMap::new(),
            ids: HashMap::new(),
            metadata: HashMap::new(),
            next_id: 0,
        };

        // EOF is always symbol 0
        registry.register(
            "end",
            SymbolMetadata {
                visible: true,
                named: false,
                hidden: false,
                terminal: true,
            },
        );

        registry
    }

    /// Register a symbol with automatic ID assignment
    pub fn register(&mut self, name: &str, metadata: SymbolMetadata) -> SymbolId {
        if let Some(&id) = self.symbols.get(name) {
            // Update metadata if symbol already exists
            self.metadata.insert(id, metadata);
            return id;
        }

        let id = SymbolId(self.next_id);
        self.next_id += 1;

        self.symbols.insert(name.to_string(), id);
        self.ids.insert(id, name.to_string());
        self.metadata.insert(id, metadata);

        id
    }

    /// Get symbol ID by name
    pub fn get_id(&self, name: &str) -> Option<SymbolId> {
        self.symbols.get(name).copied()
    }

    /// Get symbol name by ID
    pub fn get_name(&self, id: SymbolId) -> Option<&str> {
        self.ids.get(&id).map(String::as_str)
    }

    /// Get metadata for a symbol
    pub fn get_metadata(&self, id: SymbolId) -> Option<SymbolMetadata> {
        self.metadata.get(&id).copied()
    }

    /// Check if a symbol ID exists
    pub fn contains_id(&self, id: SymbolId) -> bool {
        self.ids.contains_key(&id)
    }

    /// Get total number of symbols
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// Iterate over all symbols in order
    pub fn iter(&self) -> impl Iterator<Item = (&str, SymbolInfo)> {
        self.symbols.iter().map(move |(name, &id)| {
            let metadata = self.metadata[&id];
            (name.as_str(), SymbolInfo { id, metadata })
        })
    }

    /// Create a symbol-to-index mapping for parse table generation
    pub fn to_index_map(&self) -> HashMap<SymbolId, usize> {
        self.symbols
            .values()
            .enumerate()
            .map(|(idx, &id)| (id, idx))
            .collect()
    }

    /// Create an index-to-symbol mapping for parse table decompression
    pub fn to_symbol_map(&self) -> HashMap<usize, SymbolId> {
        self.symbols
            .values()
            .enumerate()
            .map(|(idx, &id)| (idx, id))
            .collect()
    }
}

impl Default for SymbolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_deterministic() {
        let mut reg1 = SymbolRegistry::new();
        let mut reg2 = SymbolRegistry::new();

        // Register symbols in same order
        for name in ["number", "plus", "minus", "expr"] {
            let meta = SymbolMetadata {
                visible: true,
                named: name == "expr",
                hidden: false,
                terminal: name != "expr",
            };
            reg1.register(name, meta);
            reg2.register(name, meta);
        }

        // Should have same IDs
        for name in ["number", "plus", "minus", "expr"] {
            assert_eq!(reg1.get_id(name), reg2.get_id(name));
        }
    }

    #[test]
    fn test_eof_is_zero() {
        let registry = SymbolRegistry::new();
        assert_eq!(registry.get_id("end"), Some(SymbolId(0)));
    }

    /// Helper: a terminal symbol metadata.
    fn term_meta() -> SymbolMetadata {
        SymbolMetadata {
            visible: true,
            named: false,
            hidden: false,
            terminal: true,
        }
    }

    /// Helper: a nonterminal/named symbol metadata.
    fn nt_meta() -> SymbolMetadata {
        SymbolMetadata {
            visible: true,
            named: true,
            hidden: false,
            terminal: false,
        }
    }

    #[test]
    fn new_registry_has_eof_only_and_is_non_empty() {
        let registry = SymbolRegistry::new();
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.get_id("end"), Some(SymbolId(0)));
        assert_eq!(registry.get_name(SymbolId(0)), Some("end"));
        assert!(registry.contains_id(SymbolId(0)));
    }

    #[test]
    fn default_matches_new() {
        let a = SymbolRegistry::default();
        let b = SymbolRegistry::new();
        // Same structural state: same length, same EOF mapping.
        assert_eq!(a.len(), b.len());
        assert_eq!(a.get_id("end"), b.get_id("end"));
        assert_eq!(a, b);
    }

    #[test]
    fn register_assigns_incrementing_ids() {
        let mut reg = SymbolRegistry::new();
        let a = reg.register("a", term_meta());
        let b = reg.register("b", term_meta());
        let c = reg.register("c", term_meta());
        assert_eq!(a, SymbolId(1));
        assert_eq!(b, SymbolId(2));
        assert_eq!(c, SymbolId(3));
        assert_eq!(reg.len(), 4);
    }

    #[test]
    fn register_duplicate_name_returns_same_id_and_updates_metadata() {
        let mut reg = SymbolRegistry::new();
        let first = reg.register("foo", term_meta());
        let len_before = reg.len();

        // Re-register with different metadata.
        let updated_meta = SymbolMetadata {
            visible: false,
            named: true,
            hidden: true,
            terminal: false,
        };
        let second = reg.register("foo", updated_meta);

        assert_eq!(first, second, "duplicate name must reuse the same id");
        assert_eq!(reg.len(), len_before, "duplicate must not grow registry");
        assert_eq!(reg.get_metadata(first), Some(updated_meta));
    }

    #[test]
    fn get_id_returns_none_for_unknown_name() {
        let reg = SymbolRegistry::new();
        assert_eq!(reg.get_id("does_not_exist"), None);
    }

    #[test]
    fn get_name_returns_none_for_unknown_id() {
        let reg = SymbolRegistry::new();
        assert_eq!(reg.get_name(SymbolId(999)), None);
    }

    #[test]
    fn get_metadata_returns_none_for_unknown_id() {
        let reg = SymbolRegistry::new();
        assert_eq!(reg.get_metadata(SymbolId(999)), None);
    }

    #[test]
    fn contains_id_distinguishes_known_and_unknown() {
        let mut reg = SymbolRegistry::new();
        let id = reg.register("alpha", term_meta());
        assert!(reg.contains_id(id));
        assert!(reg.contains_id(SymbolId(0))); // EOF
        assert!(!reg.contains_id(SymbolId(42)));
    }

    #[test]
    fn name_and_id_round_trip() {
        let mut reg = SymbolRegistry::new();
        let id = reg.register("round", nt_meta());
        let name = reg.get_name(id).expect("name should resolve");
        let back = reg.get_id(name).expect("id should resolve");
        assert_eq!(back, id);
    }

    #[test]
    fn metadata_is_preserved_per_symbol() {
        let mut reg = SymbolRegistry::new();
        let t = reg.register("term", term_meta());
        let n = reg.register("nt", nt_meta());
        assert_eq!(reg.get_metadata(t), Some(term_meta()));
        assert_eq!(reg.get_metadata(n), Some(nt_meta()));
        // EOF metadata is also stored (set inside `new`).
        let eof_meta = reg.get_metadata(SymbolId(0)).expect("eof metadata");
        assert!(eof_meta.terminal);
        assert!(!eof_meta.named);
    }

    #[test]
    fn iter_yields_insertion_order() {
        let mut reg = SymbolRegistry::new();
        for name in ["alpha", "beta", "gamma"] {
            reg.register(name, term_meta());
        }
        let collected: Vec<&str> = reg.iter().map(|(n, _)| n).collect();
        assert_eq!(collected, vec!["end", "alpha", "beta", "gamma"]);
    }

    #[test]
    fn iter_yields_matching_info_payload() {
        let mut reg = SymbolRegistry::new();
        let id = reg.register("only", nt_meta());
        let info: Vec<SymbolInfo> = reg.iter().map(|(_, i)| i).collect();
        // EOF + one inserted symbol.
        assert_eq!(info.len(), 2);
        assert_eq!(info[0].id, SymbolId(0));
        assert_eq!(info[1].id, id);
        assert_eq!(info[1].metadata, nt_meta());
    }

    #[test]
    fn to_index_map_assigns_sequential_indices() {
        let mut reg = SymbolRegistry::new();
        let a = reg.register("a", term_meta());
        let b = reg.register("b", term_meta());
        let map = reg.to_index_map();
        assert_eq!(map.len(), 3);
        assert_eq!(map.get(&SymbolId(0)).copied(), Some(0));
        assert_eq!(map.get(&a).copied(), Some(1));
        assert_eq!(map.get(&b).copied(), Some(2));
    }

    #[test]
    fn to_symbol_map_is_inverse_of_to_index_map() {
        let mut reg = SymbolRegistry::new();
        reg.register("x", term_meta());
        reg.register("y", term_meta());
        reg.register("z", nt_meta());

        let idx = reg.to_index_map();
        let sym = reg.to_symbol_map();
        assert_eq!(idx.len(), sym.len());
        for (id, i) in &idx {
            assert_eq!(sym.get(i).copied(), Some(*id));
        }
    }

    #[test]
    fn re_registration_does_not_perturb_other_ids() {
        let mut reg = SymbolRegistry::new();
        let a = reg.register("a", term_meta());
        let b = reg.register("b", term_meta());
        let c = reg.register("c", term_meta());

        // Re-register middle symbol with new metadata.
        let b2 = reg.register(
            "b",
            SymbolMetadata {
                visible: false,
                named: false,
                hidden: true,
                terminal: false,
            },
        );

        assert_eq!(b, b2);
        // Other IDs untouched.
        assert_eq!(reg.get_id("a"), Some(a));
        assert_eq!(reg.get_id("c"), Some(c));
        assert_eq!(reg.len(), 4); // end, a, b, c
    }

    #[test]
    fn symbol_info_is_copy_and_equatable() {
        let info = SymbolInfo {
            id: SymbolId(7),
            metadata: term_meta(),
        };
        let info_copy = info; // Copy semantics
        assert_eq!(info, info_copy);
    }

    #[test]
    fn registry_equality_reflects_state() {
        let mut a = SymbolRegistry::new();
        let mut b = SymbolRegistry::new();
        assert_eq!(a, b);
        a.register("x", term_meta());
        assert_ne!(a, b);
        b.register("x", term_meta());
        assert_eq!(a, b);
    }

    #[test]
    fn empty_name_is_a_valid_symbol_name() {
        let mut reg = SymbolRegistry::new();
        let id = reg.register("", term_meta());
        assert_eq!(reg.get_id(""), Some(id));
        assert_eq!(reg.get_name(id), Some(""));
        // Re-registering empty name returns the same id.
        let id2 = reg.register("", nt_meta());
        assert_eq!(id, id2);
    }

    #[test]
    fn serde_round_trip_preserves_state() {
        let mut reg = SymbolRegistry::new();
        reg.register("alpha", term_meta());
        reg.register("beta", nt_meta());

        let json = serde_json::to_string(&reg).expect("serialize");
        let restored: SymbolRegistry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, reg);
        // Verify lookups survive the round trip.
        assert_eq!(restored.get_id("alpha"), reg.get_id("alpha"));
        assert_eq!(restored.get_id("beta"), reg.get_id("beta"));
        assert_eq!(
            restored.get_metadata(restored.get_id("beta").unwrap()),
            Some(nt_meta())
        );
    }
}

//! Deterministic symbol ID assignment via an ordered registry.

use crate::{GrammarError, SymbolId, SymbolMetadata};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A centralized registry for symbol ID assignment and metadata.
/// Ensures consistent, deterministic symbol ordering across all components.
///
/// Deserialization validates exact forward/inverse maps, metadata keys, EOF
/// identity and the monotonic allocation counter without renumbering IDs.
/// Coherent sparse imports allocate above their high-water ID; a zero counter
/// is admitted only when that high-water ID is `u16::MAX` (exhausted).
#[derive(Debug, Clone, PartialEq, Serialize)]
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

// Preserve the serialized representation while admitting only coherent imported
// identity graphs. Validating once here keeps normal registration constant-time.
impl<'de> Deserialize<'de> for SymbolRegistry {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename = "SymbolRegistry")]
        struct Representation {
            symbols: IndexMap<String, SymbolId>,
            ids: HashMap<SymbolId, String>,
            metadata: HashMap<SymbolId, SymbolMetadata>,
            next_id: u16,
        }
        let raw = Representation::deserialize(deserializer)?;
        let registry = Self {
            symbols: raw.symbols,
            ids: raw.ids,
            metadata: raw.metadata,
            next_id: raw.next_id,
        };
        registry
            .validate_import()
            .map_err(serde::de::Error::custom)?;
        Ok(registry)
    }
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
    /// Validate the imported forward/inverse identity graph and allocator state.
    /// Sparse IDs retain their identities: new allocation starts above the
    /// high-water ID and never fills old gaps. Zero is an exhausted counter only
    /// after the high-water ID reaches u16::MAX; it is never a free allocation.
    fn validate_import(&self) -> Result<(), &'static str> {
        if self.symbols.get("end") != Some(&SymbolId(0)) {
            return Err("invalid symbol registry: EOF forward mapping must be end -> 0");
        }
        if self.ids.get(&SymbolId(0)).map(String::as_str) != Some("end") {
            return Err("invalid symbol registry: EOF inverse mapping must be 0 -> end");
        }
        let mut used = std::collections::HashSet::with_capacity(self.symbols.len());
        for id in self.symbols.values() {
            if !used.insert(*id) {
                return Err("invalid symbol registry: duplicate forward symbol ID");
            }
        }
        if self.ids.len() != self.symbols.len()
            || self
                .symbols
                .iter()
                .any(|(name, id)| self.ids.get(id) != Some(name))
        {
            return Err("invalid symbol registry: forward and inverse maps must be exact inverses");
        }
        if self.metadata.len() != used.len()
            || used.iter().any(|id| !self.metadata.contains_key(id))
        {
            return Err("invalid symbol registry: metadata keys must exactly match symbol IDs");
        }
        // EOF was checked above, so the used domain is nonempty.
        let high_water = self.symbols.values().map(|id| id.0).max().unwrap_or(0);
        if self.next_id == 0 {
            if high_water != u16::MAX {
                return Err(
                    "invalid symbol registry: zero counter requires exhausted high-water ID",
                );
            }
        } else if self.next_id <= high_water {
            return Err("invalid symbol registry: next ID must exceed the high-water ID");
        }
        Ok(())
    }

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
    ///
    /// # Panics
    /// Panics when monotonic `SymbolId` allocation is exhausted, including a
    /// sparse imported high-water state at `u16::MAX`; use
    /// [`try_register`](Self::try_register) for the typed-error form.
    pub fn register(&mut self, name: &str, metadata: SymbolMetadata) -> SymbolId {
        self.try_register(name, metadata).unwrap_or_else(|error| {
            panic!("symbol registry exhausted: cannot register {name:?}: {error}")
        })
    }

    /// Register a symbol with automatic ID assignment, returning a typed
    /// capacity error instead of panicking, wrapping, or aliasing EOF (#985).
    ///
    /// The reserved EOF id `SymbolId(0)` is handed out exactly once (to
    /// `"end"` at construction); each distinct new name receives a fresh
    /// non-zero ID. An existing name updates metadata without consuming an ID.
    /// Validated sparse imports allocate above their high-water ID, preserving
    /// existing identities and leaving gaps unused; an exhausted high-water
    /// counter refuses new names even when such gaps remain.
    ///
    /// # Errors
    /// Returns [`GrammarError::SymbolSpaceExhausted`] without mutating the
    /// registry when no fresh `SymbolId` remains (including a wrapped or
    /// otherwise inconsistent counter pointing at an occupied id, which
    /// fails closed rather than aliasing EOF or an occupied symbol).
    pub fn try_register(
        &mut self,
        name: &str,
        metadata: SymbolMetadata,
    ) -> Result<SymbolId, GrammarError> {
        if let Some(&id) = self.symbols.get(name) {
            // Update metadata if symbol already exists
            self.metadata.insert(id, metadata);
            return Ok(id);
        }
        // One fresh slot per distinct name: EOF plus at most u16::MAX
        // further symbols fit the SymbolId domain.
        if self.symbols.len() > usize::from(u16::MAX) {
            return Err(GrammarError::SymbolSpaceExhausted);
        }
        let candidate = SymbolId(self.next_id);
        if self.ids.contains_key(&candidate) {
            // The counter wrapped past the domain edge (or the registry
            // was deserialized into an inconsistent state): the candidate
            // identity is already taken. Fail closed.
            return Err(GrammarError::SymbolSpaceExhausted);
        }
        self.next_id = self.next_id.wrapping_add(1);

        self.symbols.insert(name.to_string(), candidate);
        self.ids.insert(candidate, name.to_string());
        self.metadata.insert(candidate, metadata);

        Ok(candidate)
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

    #[test]
    fn test_try_register_returns_distinct_nonzero_ids() {
        let mut reg = SymbolRegistry::new();
        let a = reg.try_register("a", term_meta()).unwrap();
        let b = reg.try_register("b", term_meta()).unwrap();
        assert_ne!(a, SymbolId(0), "fresh ids must never alias EOF");
        assert_ne!(b, SymbolId(0), "fresh ids must never alias EOF");
        assert_ne!(a, b);
        assert_eq!(reg.len(), 3); // end + a + b
    }

    #[test]
    fn test_try_register_duplicate_name_reuses_id_without_consuming_capacity() {
        let mut reg = SymbolRegistry::new();
        reg.next_id = u16::MAX;
        let last = reg.try_register("last", term_meta()).unwrap();
        assert_eq!(last, SymbolId(u16::MAX));
        // Re-registering an existing name succeeds even at the edge.
        let again = reg.try_register("last", nt_meta()).unwrap();
        assert_eq!(again, last);
        assert_eq!(reg.len(), 2); // end + last
    }

    #[test]
    fn test_try_register_hands_out_last_slot_then_fails_typed() {
        // Falsifier 1 (#985): last valid SymbolId followed by one
        // additional symbol; falsifier 10: deterministic result at the
        // exact highest supported count.
        let mut reg = SymbolRegistry::new();
        reg.next_id = u16::MAX - 1;
        assert_eq!(
            reg.try_register("second-to-last", term_meta()).unwrap(),
            SymbolId(u16::MAX - 1)
        );
        assert_eq!(
            reg.try_register("last", term_meta()).unwrap(),
            SymbolId(u16::MAX)
        );
        let before = reg.clone();
        let err = reg.try_register("overflow", term_meta()).unwrap_err();
        assert!(
            matches!(err, crate::GrammarError::SymbolSpaceExhausted),
            "expected SymbolSpaceExhausted, got {err:?}"
        );
        // Atomicity: the failed registration mutates nothing.
        assert_eq!(reg, before);
        assert_eq!(reg.get_id("overflow"), None);
    }

    #[test]
    fn test_try_register_is_deterministic_at_the_domain_edge() {
        let mut first = SymbolRegistry::new();
        first.next_id = u16::MAX - 1;
        let mut second = SymbolRegistry::new();
        second.next_id = u16::MAX - 1;
        for name in ["x", "y"] {
            let a = first.try_register(name, term_meta()).unwrap();
            let b = second.try_register(name, term_meta()).unwrap();
            assert_eq!(a, b);
        }
        assert_eq!(first, second);
    }

    #[test]
    #[should_panic(expected = "symbol registry exhausted")]
    fn test_register_panics_explicitly_when_exhausted() {
        // The infallible default path fails closed with an actionable
        // message instead of wrapping into EOF; try_register is the
        // typed-error form.
        let mut reg = SymbolRegistry::new();
        reg.next_id = u16::MAX;
        let _ = reg.register("last", term_meta());
        let _ = reg.register("overflow", term_meta());
    }
}

//! Compiler-significant identity metadata for the 0.10 generated path (#862).
//!
//! Start-symbol and pattern-wrapper relations are authoritative facts captured at
//! grammar-definition or import time. Downstream lowering must consume these
//! fields instead of name/order heuristics.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{Grammar, SymbolId};

/// Explicit relation between a pattern-wrapper nonterminal and its backing token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WrapperTokenRelation {
    /// Wrapper nonterminal symbol.
    pub wrapper: SymbolId,
    /// Backing terminal token symbol.
    pub token: SymbolId,
}

impl Grammar {
    /// Returns the explicitly declared start symbol, if set.
    #[must_use]
    pub fn explicit_start_symbol(&self) -> Option<SymbolId> {
        self.start_symbol
    }

    /// Set the authoritative start symbol for this grammar.
    pub fn set_start_symbol(&mut self, symbol: SymbolId) {
        self.start_symbol = Some(symbol);
    }

    /// Returns the backing token for a pattern-wrapper nonterminal, if declared.
    #[must_use]
    pub fn wrapper_token_for(&self, wrapper: SymbolId) -> Option<SymbolId> {
        self.wrapper_token_relations.get(&wrapper).copied()
    }

    /// Record an explicit wrapper-to-token relation.
    pub fn set_wrapper_token_relation(&mut self, wrapper: SymbolId, token: SymbolId) {
        self.wrapper_token_relations.insert(wrapper, token);
    }

    /// Iterate wrapper-to-token relations in deterministic symbol-id order.
    #[must_use]
    pub fn wrapper_token_relations_sorted(&self) -> Vec<WrapperTokenRelation> {
        let mut relations: Vec<_> = self
            .wrapper_token_relations
            .iter()
            .map(|(wrapper, token)| WrapperTokenRelation {
                wrapper: *wrapper,
                token: *token,
            })
            .collect();
        relations.sort_by_key(|relation| relation.wrapper.0);
        relations
    }
}

/// Deterministic ordering for wrapper-token relations.
#[must_use]
pub fn sorted_wrapper_token_relations(
    relations: &IndexMap<SymbolId, SymbolId>,
) -> Vec<WrapperTokenRelation> {
    let mut entries: Vec<_> = relations
        .iter()
        .map(|(wrapper, token)| WrapperTokenRelation {
            wrapper: *wrapper,
            token: *token,
        })
        .collect();
    entries.sort_by_key(|relation| relation.wrapper.0);
    entries
}

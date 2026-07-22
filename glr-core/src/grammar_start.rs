//! Internal start-symbol resolution for GLR table generation.

use adze_ir::{Grammar, SymbolId};

/// Start symbol for GLR table generation.
///
/// Uses explicit compiler-identity metadata when present; otherwise pins the first
/// rule LHS, matching [`GrammarBuilder`](adze_ir::builder::GrammarBuilder) defaults.
/// Legacy name/order heuristics remain removed (#862 PR6).
pub(crate) fn analysis_start_symbol(grammar: &Grammar) -> Option<SymbolId> {
    if let Some(explicit) = grammar.explicit_start_symbol()
        && grammar.rules.contains_key(&explicit)
    {
        return Some(explicit);
    }
    grammar.rules.keys().next().copied()
}

//! Allocation-time coordinates for direct explicit-IR `field(Repeat(atom))` sites.

use crate::{FieldId, SourceRuleId, Symbol, SymbolId};

/// Read-only facts recorded by one normalization invocation. All IDs and slots
/// are local to that invocation, not a grammar identity or a selected reduction.
/// No runtime projection or serialization capability is implied.
///
/// External callers can inspect a source coordinate but cannot change it:
/// ```compile_fail
/// fn rewrite(site: &mut adze_ir::RepeatFieldOrigin) {
///     site.source_rhs_position = 99;
/// }
/// ```
/// These coordinates cannot certify a selected branch in any grammar:
/// ```compile_fail
/// fn select(site: &adze_ir::RepeatFieldOrigin, grammar: &adze_ir::Grammar) {
///     site.branch_for(grammar, adze_ir::RuleId(2), adze_ir::RuleId(0));
/// }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct RepeatFieldOrigin {
    pub(super) source_rule: SourceRuleId,
    pub(super) source_rhs_position: usize,
    pub(super) lowered_rhs_position: usize,
    pub(super) field: FieldId,
    pub(super) helper: SymbolId,
    pub(super) item: Symbol,
    pub(super) recursive_alternative: usize,
    pub(super) empty_alternative: usize,
}

impl RepeatFieldOrigin {
    /// Original owner and alternative ordinal, not a shared production slot.
    pub fn source_rule(&self) -> SourceRuleId {
        self.source_rule
    }
    /// Original RHS slot, before sequence flattening.
    pub fn source_rhs_position(&self) -> usize {
        self.source_rhs_position
    }
    /// Slot in the normalized owner's RHS.
    pub fn lowered_rhs_position(&self) -> usize {
        self.lowered_rhs_position
    }
    /// Source field bound at this site.
    pub fn field(&self) -> FieldId {
        self.field
    }
    /// Helper actually allocated at this site.
    pub fn helper(&self) -> SymbolId {
        self.helper
    }
    /// Opaque item, preserving terminal/nonterminal/external identity.
    pub fn item(&self) -> &Symbol {
        &self.item
    }
    /// Local ordinal of the emitted recursive helper alternative.
    pub fn recursive_alternative(&self) -> usize {
        self.recursive_alternative
    }
    /// Local ordinal of the emitted zero-occurrence helper alternative.
    pub fn empty_alternative(&self) -> usize {
        self.empty_alternative
    }
}

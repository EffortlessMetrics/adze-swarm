use adze_ir::SymbolId;
use std::ops::Range;

/// Summary of retained complete alternatives for an ambiguous GLR parse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AmbiguitySummary {
    /// Byte span covered by the complete alternatives.
    pub span: Range<usize>,
    /// Retained complete alternatives in runtime order.
    pub alternatives: Vec<AlternativeSummary>,
    /// Index of the selected alternative within [`Self::alternatives`].
    pub selected: Option<usize>,
    /// Reason the selected alternative won.
    pub selection_reason: SelectionReason,
}

/// Public metadata for one retained complete GLR parse alternative.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlternativeSummary {
    /// Index within the ambiguity summary.
    pub index: usize,
    /// Root symbol of this complete alternative.
    pub root_symbol: SymbolId,
    /// Byte span covered by this complete alternative.
    pub span: Range<usize>,
    /// Dynamic-precedence score accumulated for this parse version.
    pub dynamic_precedence: i32,
    /// Whether this parse version entered error recovery.
    pub in_error: bool,
    /// Error/recovery cost for this parse version.
    pub cost: usize,
    /// Structural node count for this retained alternative tree.
    pub node_count: usize,
}

/// Reason the GLR runtime selected one complete alternative.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionReason {
    /// Only one complete parse was retained.
    SingleParse,
    /// Parse-version comparison selected a lower-cost non-error path.
    ErrorCost,
    /// Parse-version comparison selected higher dynamic precedence.
    DynamicPrecedence,
    /// Parse versions tied and the stable structural key selected a tree.
    StableStructuralTieBreak,
}

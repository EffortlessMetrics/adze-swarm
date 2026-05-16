//! Ambiguous-parse summaries and deterministic selection helpers.

use super::ParseStack;
use crate::subtree::Subtree;
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

pub(super) type SubtreeSelectionKey = Vec<(usize, usize, u16, u16, usize)>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SelectedCompleteStack {
    pub(super) stack_index: usize,
    pub(super) reason: SelectionReason,
}

pub(super) fn subtree_selection_key(node: &Subtree) -> SubtreeSelectionKey {
    let mut key = Vec::new();
    append_subtree_selection_key(node, &mut key);
    key
}

fn append_subtree_selection_key(node: &Subtree, key: &mut SubtreeSelectionKey) {
    key.push((
        node.node.byte_range.start,
        node.node.byte_range.end,
        node.node.symbol_id.0,
        u16::from(node.node.is_error),
        node.children.len(),
    ));

    for edge in &node.children {
        key.push((usize::MAX, usize::MAX, edge.field_id, 0, 0));
        append_subtree_selection_key(&edge.subtree, key);
    }
}

pub(super) fn subtree_node_count(node: &Subtree) -> usize {
    1 + node
        .children
        .iter()
        .map(|edge| subtree_node_count(&edge.subtree))
        .sum::<usize>()
}

pub(super) fn version_selection_reason(left: &ParseStack, right: &ParseStack) -> SelectionReason {
    if left.version.in_error != right.version.in_error || left.version.cost != right.version.cost {
        return SelectionReason::ErrorCost;
    }

    if left.version.dynamic_prec != right.version.dynamic_prec {
        return SelectionReason::DynamicPrecedence;
    }

    SelectionReason::StableStructuralTieBreak
}

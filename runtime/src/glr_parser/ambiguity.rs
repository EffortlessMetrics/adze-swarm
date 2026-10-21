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
    /// Retained complete alternatives in canonical deterministic order.
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

pub(crate) fn subtree_selection_key(node: &Subtree) -> SubtreeSelectionKey {
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

type AlternativeCanonicalKey = (
    SubtreeSelectionKey,
    i32,
    usize,
    bool,
    u16,
    usize,
    usize,
    usize,
);

fn alternative_canonical_key(
    alternative: &AlternativeSummary,
    structural_key: &SubtreeSelectionKey,
) -> AlternativeCanonicalKey {
    (
        structural_key.clone(),
        alternative.dynamic_precedence,
        alternative.cost,
        alternative.in_error,
        alternative.root_symbol.0,
        alternative.node_count,
        alternative.span.start,
        alternative.span.end,
    )
}

/// Sort retained alternatives into a canonical deterministic order and drop
/// exact duplicates.
///
/// Two alternatives are duplicates only when their structural keys and every
/// serialized summary field agree; alternatives that share a summary but have
/// different tree structures are retained. `selected` indexes into
/// `alternatives` on input and is remapped to the canonical position of the
/// same alternative on output.
pub(crate) fn canonicalize_alternatives(
    alternatives: Vec<AlternativeSummary>,
    structural_keys: Vec<SubtreeSelectionKey>,
    selected: Option<usize>,
) -> (Vec<AlternativeSummary>, Option<usize>) {
    debug_assert_eq!(alternatives.len(), structural_keys.len());

    let selected_key = selected.and_then(|index| {
        alternatives
            .get(index)
            .zip(structural_keys.get(index))
            .map(|(alternative, key)| alternative_canonical_key(alternative, key))
    });

    let mut entries: Vec<(AlternativeCanonicalKey, AlternativeSummary)> = alternatives
        .into_iter()
        .zip(structural_keys.iter())
        .map(|(alternative, key)| (alternative_canonical_key(&alternative, key), alternative))
        .collect();
    entries.sort_by(|left, right| left.0.cmp(&right.0));

    let mut seen = std::collections::HashSet::new();
    entries.retain(|(key, _)| seen.insert(key.clone()));

    let selected = selected_key.and_then(|key| entries.iter().position(|(candidate, _)| *candidate == key));

    let alternatives = entries
        .into_iter()
        .enumerate()
        .map(|(index, (_, mut alternative))| {
            alternative.index = index;
            alternative
        })
        .collect();

    (alternatives, selected)
}

pub(crate) fn subtree_node_count(node: &Subtree) -> usize {
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

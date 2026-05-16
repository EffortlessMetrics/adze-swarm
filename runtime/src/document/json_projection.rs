//! JSON projection helpers for the native parse document view.

use super::*;

pub(super) fn diagnostic_to_json(diagnostic: &ParseDiagnostic) -> serde_json::Value {
    serde_json::json!({
        "start_byte": diagnostic.start_byte,
        "end_byte": diagnostic.end_byte,
        "point_range": point_range_to_json(diagnostic.point_range),
        "found": diagnostic.found.as_deref(),
        "expected": &diagnostic.expected,
        "related_nodes": diagnostic
            .related_nodes
            .iter()
            .map(|node_id| node_id.as_usize())
            .collect::<Vec<_>>(),
        "message": diagnostic.message.as_str(),
    })
}

#[cfg(feature = "serialization")]
pub(super) fn ambiguity_to_json(
    ambiguity: &crate::glr_parser::AmbiguitySummary,
) -> serde_json::Value {
    serde_json::json!({
        "span": {
            "start_byte": ambiguity.span.start,
            "end_byte": ambiguity.span.end,
        },
        "selected": ambiguity.selected,
        "selection_reason": format!("{:?}", ambiguity.selection_reason),
        "alternatives": ambiguity
            .alternatives
            .iter()
            .map(alternative_to_json)
            .collect::<Vec<_>>(),
    })
}

#[cfg(feature = "serialization")]
fn alternative_to_json(alternative: &crate::glr_parser::AlternativeSummary) -> serde_json::Value {
    serde_json::json!({
        "index": alternative.index,
        "root_symbol": alternative.root_symbol.0,
        "span": {
            "start_byte": alternative.span.start,
            "end_byte": alternative.span.end,
        },
        "dynamic_precedence": alternative.dynamic_precedence,
        "in_error": alternative.in_error,
        "cost": alternative.cost,
        "node_count": alternative.node_count,
    })
}

#[cfg(feature = "serialization")]
pub(super) fn node_to_document_json(node: AdzeNode<'_>) -> serde_json::Value {
    let identity = node.identity();
    let flags = node.flags();
    let text = if node.child_count() == 0 {
        node.utf8_text().ok()
    } else {
        None
    };
    let children = node
        .child_edges()
        .filter_map(|edge| {
            edge.child().map(|child| {
                serde_json::json!({
                    "child_index": edge.child_index(),
                    "field_name": edge.field_name(),
                    "field_id": edge.field_id().map(|field_id| field_id.get()),
                    "node": node_to_document_json(child),
                })
            })
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "id": node.node_id().as_usize(),
        "kind": identity.visible_name(),
        "kind_id": identity.visible_id().0,
        "grammar_kind": identity.grammar_name(),
        "grammar_id": identity.grammar_id().0,
        "alias_symbol_id": identity.alias_symbol_id().map(|symbol_id| symbol_id.0),
        "has_alias": identity.has_alias(),
        "range": {
            "start_byte": node.start_byte(),
            "end_byte": node.end_byte(),
            "start_point": point_to_json(node.point_range().start),
            "end_point": point_to_json(node.point_range().end),
        },
        "flags": {
            "named": flags.is_named(),
            "visible": flags.is_visible(),
            "extra": flags.is_extra(),
            "terminal": flags.is_terminal(),
            "supertype": flags.is_supertype(),
            "error": flags.is_error(),
            "missing": flags.is_missing(),
            "has_error": flags.has_error(),
        },
        "text": text,
        "children": children,
    })
}

#[cfg(feature = "serialization")]
fn point_range_to_json(range: PointRange) -> serde_json::Value {
    serde_json::json!({
        "start": point_to_json(range.start),
        "end": point_to_json(range.end),
    })
}

#[cfg(feature = "serialization")]
fn point_to_json(point: DocumentPoint) -> serde_json::Value {
    serde_json::json!({
        "row": point.row,
        "column": point.column,
    })
}

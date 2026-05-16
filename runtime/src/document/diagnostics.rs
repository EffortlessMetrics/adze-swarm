//! Parse-diagnostic construction and document-node correlation helpers.

use super::*;

pub(super) fn build_diagnostics(
    root: &ParseNode,
    error_count: usize,
    source: &str,
) -> Vec<ParseDiagnostic> {
    if error_count == 0 {
        return Vec::new();
    }

    let span = first_error_span(root).unwrap_or(root.start_byte..root.end_byte);
    let start_byte = span.start.min(source.len());
    let end_byte = span.end.min(source.len()).max(start_byte);
    let point_range = PointRange::from_byte_range(source, start_byte..end_byte);

    vec![ParseDiagnostic {
        start_byte,
        end_byte,
        point_range,
        found: None,
        expected: Vec::new(),
        related_nodes: Vec::new(),
        message: format!("parser recorded {error_count} recovery/error event(s)"),
    }]
}

pub(super) fn attach_related_nodes(root: &ParseNode, diagnostics: &mut [ParseDiagnostic]) {
    for diagnostic in diagnostics {
        diagnostic.related_nodes = related_nodes_for_diagnostic(root, diagnostic);
    }
}

fn related_nodes_for_diagnostic(root: &ParseNode, diagnostic: &ParseDiagnostic) -> Vec<NodeId> {
    let mut related_errors = Vec::new();
    let mut next_id = 0;
    collect_related_error_nodes(root, diagnostic, &mut next_id, &mut related_errors);
    if !related_errors.is_empty() {
        return related_errors;
    }

    let mut best = None;
    let mut next_id = 0;
    collect_smallest_covering_node(root, diagnostic, &mut next_id, &mut best);
    best.map(|(node_id, _)| vec![node_id]).unwrap_or_default()
}

fn collect_related_error_nodes(
    node: &ParseNode,
    diagnostic: &ParseDiagnostic,
    next_id: &mut usize,
    related: &mut Vec<NodeId>,
) {
    let node_id = NodeId::new(*next_id);
    *next_id += 1;

    if is_error_parse_node(node) && node_range_touches_diagnostic(node, diagnostic) {
        related.push(node_id);
    }

    for child in &node.children {
        collect_related_error_nodes(child, diagnostic, next_id, related);
    }
}

fn collect_smallest_covering_node(
    node: &ParseNode,
    diagnostic: &ParseDiagnostic,
    next_id: &mut usize,
    best: &mut Option<(NodeId, usize)>,
) {
    let node_id = NodeId::new(*next_id);
    *next_id += 1;

    if node_covers_diagnostic(node, diagnostic) {
        let width = node.end_byte.saturating_sub(node.start_byte);
        if best
            .map(|(_, best_width)| width < best_width)
            .unwrap_or(true)
        {
            *best = Some((node_id, width));
        }
    }

    for child in &node.children {
        collect_smallest_covering_node(child, diagnostic, next_id, best);
    }
}

pub(in crate::document) fn is_error_parse_node(node: &ParseNode) -> bool {
    node.symbol.0 == 0 && node.children.is_empty()
}

fn node_range_touches_diagnostic(node: &ParseNode, diagnostic: &ParseDiagnostic) -> bool {
    if diagnostic.start_byte == diagnostic.end_byte {
        node.start_byte <= diagnostic.start_byte && diagnostic.start_byte <= node.end_byte
    } else {
        node.start_byte < diagnostic.end_byte && diagnostic.start_byte < node.end_byte
    }
}

fn node_covers_diagnostic(node: &ParseNode, diagnostic: &ParseDiagnostic) -> bool {
    if diagnostic.start_byte == diagnostic.end_byte {
        node.start_byte <= diagnostic.start_byte && diagnostic.start_byte <= node.end_byte
    } else {
        node.start_byte <= diagnostic.start_byte && diagnostic.end_byte <= node.end_byte
    }
}

fn first_error_span(node: &ParseNode) -> Option<Range<usize>> {
    if node.symbol.0 == 0 && node.children.is_empty() {
        return Some(node.start_byte..node.end_byte);
    }

    node.children.iter().find_map(first_error_span)
}

pub(in crate::document) fn source_line(source: &str, byte_offset: usize) -> Option<&str> {
    if source.is_empty() {
        return None;
    }

    let bytes = source.as_bytes();
    let offset = byte_offset.min(bytes.len());
    let mut start = offset;
    while start > 0 && bytes[start - 1] != b'\n' && bytes[start - 1] != b'\r' {
        start -= 1;
    }

    let mut end = offset;
    while end < bytes.len() && bytes[end] != b'\n' && bytes[end] != b'\r' {
        end += 1;
    }

    source.get(start..end)
}

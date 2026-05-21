//! Parse-diagnostic construction and document-node correlation helpers.

use super::*;

mod node_matching;
mod spans;

pub(in crate::document) use node_matching::is_error_parse_node;
use node_matching::related_nodes_for_diagnostic;
use spans::first_error_span;

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

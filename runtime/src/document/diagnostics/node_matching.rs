use super::*;

pub(super) fn related_nodes_for_diagnostic(
    root: &ParseNode,
    diagnostic: &ParseDiagnostic,
) -> Vec<NodeId> {
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

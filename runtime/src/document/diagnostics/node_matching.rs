use super::*;

/// Document-local ids related to one diagnostic, derived from canonical
/// flat records.
///
/// Error-flagged nodes whose range touches the diagnostic win; otherwise the
/// smallest covering node is returned. Flat records are numbered in preorder,
/// so the selected ids match the historical recursive numbering exactly.
pub(super) fn related_nodes_for_diagnostic(
    tree: &SyntaxTree,
    diagnostic: &ParseDiagnostic,
) -> Vec<NodeId> {
    let mut related_errors = Vec::new();
    for node_id in node_ids(tree) {
        let record = tree.node_record(node_id).expect("flat ids are dense");
        if record.flags().is_error() && record_range_touches_diagnostic(record, diagnostic) {
            related_errors.push(node_id);
        }
    }
    if !related_errors.is_empty() {
        return related_errors;
    }

    let mut best: Option<(NodeId, usize)> = None;
    for node_id in node_ids(tree) {
        let record = tree.node_record(node_id).expect("flat ids are dense");
        if record_covers_diagnostic(record, diagnostic) {
            let range = record.byte_range();
            let width = range.end.saturating_sub(range.start);
            if best
                .map(|(_, best_width)| width < best_width)
                .unwrap_or(true)
            {
                best = Some((node_id, width));
            }
        }
    }
    best.map(|(node_id, _)| vec![node_id]).unwrap_or_default()
}

fn node_ids(tree: &SyntaxTree) -> impl Iterator<Item = NodeId> {
    (0..tree.node_count()).map(NodeId::new)
}

pub(in crate::document) fn is_error_parse_node(node: &ParseNode) -> bool {
    // Symbol 0 is the document error identity, for leaves and for error
    // nodes that still carry recovered children (#875/#898).
    node.symbol.0 == 0
}

fn record_range_touches_diagnostic(record: &NodeRecord, diagnostic: &ParseDiagnostic) -> bool {
    let range = record.byte_range();
    if diagnostic.start_byte == diagnostic.end_byte {
        range.start <= diagnostic.start_byte && diagnostic.start_byte <= range.end
    } else {
        range.start < diagnostic.end_byte && diagnostic.start_byte < range.end
    }
}

fn record_covers_diagnostic(record: &NodeRecord, diagnostic: &ParseDiagnostic) -> bool {
    let range = record.byte_range();
    if diagnostic.start_byte == diagnostic.end_byte {
        range.start <= diagnostic.start_byte && diagnostic.start_byte <= range.end
    } else {
        range.start <= diagnostic.start_byte && diagnostic.end_byte <= range.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::tree_builder::build_flat_tree;

    fn flat_language() -> LanguageMetadata {
        LanguageMetadata {
            name: "related-nodes-test".to_string(),
            symbols: Vec::new(),
            fields: Vec::new(),
            field_semantics: Default::default(),
        }
    }

    fn leaf(symbol: u16, start: usize, end: usize) -> ParseNode {
        ParseNode {
            symbol: SymbolId(symbol),
            symbol_id: SymbolId(symbol),
            start_byte: start,
            end_byte: end,
            field_name: None,
            alias_symbol_id: None,
            children: Vec::new(),
        }
    }

    fn diagnostic_for(start: usize, end: usize) -> ParseDiagnostic {
        ParseDiagnostic {
            start_byte: start,
            end_byte: end,
            point_range: PointRange::default(),
            found: None,
            expected: Vec::new(),
            related_nodes: Vec::new(),
            message: String::new(),
        }
    }

    #[test]
    fn test_related_nodes_prefers_error_records_touching_the_diagnostic() {
        // root(0..8) > clean(0..3), errors(3..5, 5..8); diagnostic touches
        // only the second error span.
        let root = ParseNode {
            symbol: SymbolId(1),
            symbol_id: SymbolId(1),
            start_byte: 0,
            end_byte: 8,
            field_name: None,
            alias_symbol_id: None,
            children: vec![leaf(1, 0, 3), leaf(0, 3, 5), leaf(0, 5, 8)],
        };
        let language = flat_language();
        let built = build_flat_tree(&root, "12345678", &language, 1);
        let tree = SyntaxTree {
            root_id: NodeId::new(0),
            nodes: built.nodes,
            edges: built.edges,
            parents: built.parents,
        };

        let diagnostic = diagnostic_for(6, 7);
        assert_eq!(
            related_nodes_for_diagnostic(&tree, &diagnostic),
            vec![NodeId::new(3)],
            "only the touching error record is related"
        );
    }

    #[test]
    fn test_related_nodes_falls_back_to_smallest_covering_record() {
        let root = ParseNode {
            symbol: SymbolId(1),
            symbol_id: SymbolId(1),
            start_byte: 0,
            end_byte: 8,
            field_name: None,
            alias_symbol_id: None,
            children: vec![leaf(1, 0, 3), leaf(1, 3, 8)],
        };
        let language = flat_language();
        let built = build_flat_tree(&root, "12345678", &language, 0);
        let tree = SyntaxTree {
            root_id: NodeId::new(0),
            nodes: built.nodes,
            edges: built.edges,
            parents: built.parents,
        };

        let diagnostic = diagnostic_for(4, 5);
        assert_eq!(
            related_nodes_for_diagnostic(&tree, &diagnostic),
            vec![NodeId::new(2)],
            "smallest covering record wins when nothing is an error"
        );
    }
}

//! Node-record point ranges retain the public byte-position contract (#1068).

#![cfg(feature = "pure-rust")]

use adze::document::{AdzeDocument, DocumentPoint, PointRange};

// An independent linear oracle; do not use the production point converter.
fn expected_point(source: &str, offset: usize) -> DocumentPoint {
    let mut point = DocumentPoint::default();
    for byte in source.bytes().take(offset) {
        if byte == b'\n' {
            point.row = point.row.saturating_add(1);
            point.column = 0;
        } else {
            point.column = point.column.saturating_add(1);
        }
    }
    point
}

fn assert_node_points(document: &AdzeDocument) {
    let source = document.source_text();
    let tree = document.tree();
    assert_eq!(tree.root().kind_name(), Some("source_file"));
    let mut pending = vec![tree.root()];
    let mut visited = 0;
    while let Some(node) = pending.pop() {
        visited += 1;
        let bytes = node.byte_range();
        assert!(bytes.start <= bytes.end && bytes.end <= source.len());
        let expected = PointRange {
            start: expected_point(source, bytes.start),
            end: expected_point(source, bytes.end),
        };
        assert_eq!(node.point_range(), expected);
        assert_eq!(node.record().point_range(), expected);
        for child_index in 0..node.child_count() {
            let child = node.child(child_index).expect("declared child must exist");
            assert_eq!(child.parent_id(), Some(node.node_id()));
            pending.push(child);
        }
    }
    assert!(visited > 0, "a root record must be exercised");
    assert_eq!(visited, tree.node_count());
}

#[test]
fn test_document_points_clean_multiline_input_match_source_bytes() {
    let source = "1 +\n 2";
    let document = adze_example::typed_ast_contract::grammar::parse_document(source)
        .expect("valid expression should produce a document");
    assert!(document.diagnostics().is_empty());
    assert_node_points(&document);
    assert_eq!(
        document.tree().root().point_range().end,
        DocumentPoint { row: 1, column: 2 }
    );
}

#[test]
fn test_document_points_invalid_utf8_adjacent_syntax_match_source_bytes() {
    for source in ["1 +\n", "1 +\né"] {
        let document = adze_example::typed_ast_contract::grammar::parse_document(source)
            .expect("invalid syntax should preserve a diagnostic document");
        assert!(!document.diagnostics().is_empty());
        assert_node_points(&document);
    }
}

#[cfg(feature = "glr")]
#[test]
fn test_document_points_ambiguous_grammar_match_source_bytes() {
    let source = "1 + 2 + 3";
    let document = adze_example::ambiguous_expr::grammar::parse_document(source)
        .expect("ambiguous expression should produce a selected document");
    assert!(document.diagnostics().is_empty());
    assert_node_points(&document);
}

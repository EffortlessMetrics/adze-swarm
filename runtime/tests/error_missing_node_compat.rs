//! Document-backed Tree-sitter compatibility canaries for ERROR and MISSING nodes.

#![cfg(all(test, feature = "pure-rust", feature = "glr", feature = "ts-compat"))]

use adze::{
    document::{AdzeDocument, AdzeNode},
    parser::Parser as CoreParser,
    ts_compat::{Language, Node, Tree},
};
use std::{ops::Range, sync::Arc};

fn parse_document(source: &str) -> (Arc<Language>, AdzeDocument) {
    let lang = adze_example::ts_langs::arithmetic();
    let mut parser = CoreParser::new(lang.grammar.clone(), lang.table.clone(), lang.name.clone());
    let document = parser
        .parse_document(source)
        .expect("arithmetic parser should return partial document facts for bad input");
    (lang, document)
}

fn collect_native_nodes<'doc>(node: AdzeNode<'doc>, nodes: &mut Vec<AdzeNode<'doc>>) {
    nodes.push(node);
    for index in 0..node.child_count() {
        let child = node.child(index).expect("child index should be valid");
        collect_native_nodes(child, nodes);
    }
}

fn collect_ts_nodes<'tree>(node: Node<'tree>, nodes: &mut Vec<Node<'tree>>) {
    nodes.push(node.clone());
    for index in 0..node.child_count() {
        let child = node.child(index).expect("child index should be valid");
        collect_ts_nodes(child, nodes);
    }
}

fn native_missing_ranges(document: &AdzeDocument) -> Vec<Range<usize>> {
    let mut nodes = Vec::new();
    collect_native_nodes(document.tree().root(), &mut nodes);

    nodes
        .into_iter()
        .filter(|node| node.is_missing())
        .map(|node| {
            assert!(node.is_error());
            assert!(node.has_error());
            node.byte_range()
        })
        .collect()
}

fn ts_missing_ranges(tree: &Tree) -> Vec<Range<usize>> {
    let mut nodes = Vec::new();
    collect_ts_nodes(tree.root_node(), &mut nodes);

    nodes
        .into_iter()
        .filter(|node| node.is_missing())
        .map(|node| {
            assert!(node.is_error());
            assert!(node.has_error());
            node.byte_range()
        })
        .collect()
}

#[test]
fn error_missing_node_compat_projects_zero_width_missing_root() {
    let (lang, document) = parse_document("");
    let tree = Tree::from_document(Arc::clone(&lang), &document);
    let native_root = document.tree().root();
    let ts_root = tree.root_node();
    let diagnostic = document
        .diagnostics()
        .first()
        .expect("empty input should produce a native diagnostic");

    assert!(document.metadata().error_count > 0);
    assert!(document.tree().has_errors());
    assert!(tree.has_errors());
    assert_eq!(tree.error_count(), document.metadata().error_count);

    assert!(native_root.is_error());
    assert!(native_root.is_missing());
    assert!(native_root.has_error());
    assert_eq!(native_root.byte_range(), 0..0);

    assert!(ts_root.is_error());
    assert!(ts_root.is_missing());
    assert!(ts_root.has_error());
    assert_eq!(ts_root.byte_range(), native_root.byte_range());
    assert_eq!(
        (
            ts_root.start_position().row,
            ts_root.start_position().column
        ),
        (
            native_root.point_range().start.row,
            native_root.point_range().start.column
        )
    );
    assert_eq!(
        (ts_root.end_position().row, ts_root.end_position().column),
        (
            native_root.point_range().end.row,
            native_root.point_range().end.column
        )
    );

    assert_eq!(diagnostic.byte_span(), 0..0);
    assert!(
        diagnostic.related_nodes.contains(&native_root.node_id()),
        "native diagnostic should link to the zero-width root error node"
    );
}

#[test]
fn error_missing_node_compat_projects_recovered_missing_children() {
    let source = "1-";
    let (lang, document) = parse_document(source);
    let tree = Tree::from_document(Arc::clone(&lang), &document);
    let native_root = document.tree().root();
    let ts_root = tree.root_node();
    let native_missing = native_missing_ranges(&document);
    let ts_missing = ts_missing_ranges(&tree);

    assert!(document.metadata().error_count > 0);
    assert!(native_root.has_error());
    assert!(ts_root.has_error());
    assert!(!native_root.is_error());
    assert!(!ts_root.is_error());
    assert!(!native_root.is_missing());
    assert!(!ts_root.is_missing());

    assert!(
        !native_missing.is_empty(),
        "truncated input should expose recovered native missing nodes"
    );
    assert_eq!(
        ts_missing, native_missing,
        "document-backed ts_compat projection should preserve missing-node ranges"
    );
    assert!(
        ts_missing
            .iter()
            .all(|range| range.start == source.len() && range.end == source.len()),
        "recovered missing nodes should be zero-width at EOF: {ts_missing:?}"
    );
}

#[test]
fn error_missing_node_compat_links_invalid_tail_diagnostics_to_error_state() {
    let source = "1-@";
    let (lang, document) = parse_document(source);
    let tree = Tree::from_document(Arc::clone(&lang), &document);
    let diagnostic = document
        .diagnostics()
        .first()
        .expect("invalid token should produce a native diagnostic");
    let related_nodes: Vec<_> = diagnostic
        .related_nodes
        .iter()
        .filter_map(|node_id| document.tree().node(*node_id))
        .collect();

    assert_eq!(
        diagnostic.byte_span(),
        source.len()..source.len(),
        "core arithmetic recovery reports this invalid tail at the EOF insertion point"
    );
    assert!(document.metadata().error_count > 0);
    assert!(document.tree().root().has_error());
    assert!(tree.has_errors());
    assert_eq!(tree.error_count(), document.metadata().error_count);
    assert_eq!(
        tree.root_node().has_error(),
        document.tree().root().has_error()
    );
    assert!(
        !related_nodes.is_empty(),
        "bad-token diagnostic should link to at least one native document node"
    );
    assert!(
        related_nodes.iter().any(|node| node.has_error()),
        "at least one related native node should carry aggregate error state"
    );
    assert!(
        related_nodes.iter().any(|node| {
            let range = node.byte_range();
            range.start <= diagnostic.start_byte && range.end >= diagnostic.end_byte
        }),
        "a related native node should cover the bad-token diagnostic span"
    );
}

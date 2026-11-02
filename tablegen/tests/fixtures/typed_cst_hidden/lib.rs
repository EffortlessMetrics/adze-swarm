const GRAMMAR_NAME: &str = "schema_hidden_consumer";
include!(concat!(env!("OUT_DIR"), "/generated.rs"));

#[test]
fn test_normalized_hidden_target_accessor_returns_exact_document_child() {
    use adze::document::{ParseOutcome, SyntaxNode};
    let document = parse_document("x ").expect("present normalized optional parses");
    assert_eq!(
        document.outcome(),
        ParseOutcome::Complete,
        "diagnostics: {:?}",
        document.diagnostics()
    );
    assert!(document.diagnostics().is_empty());
    let root = syntax::source_file(&document).expect("cast actual generated root");
    let items = root.value_item();
    let separators = root.value_sep();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].text(), Some("x"));
    assert_eq!(items[0].byte_range(), Some(0..1));
    assert_eq!(separators.len(), 1, "document: {document:?}");
    assert_eq!(separators[0].text(), Some(" "));
    assert_eq!(separators[0].byte_range(), Some(1..2));
    let generic = root
        .node()
        .unwrap()
        .child_edges()
        .filter(|edge| edge.field_name() == Some("value"))
        .filter_map(|edge| edge.child())
        .find(|node| node.kind_name() == Some("_sep"))
        .expect("generic field edge reaches the hidden target");
    assert_eq!(separators[0].node_id(), generic.node_id());
}

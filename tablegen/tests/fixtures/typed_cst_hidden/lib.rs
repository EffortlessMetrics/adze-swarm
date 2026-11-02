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
    // Independently identify the actual selected native separator. The
    // physical carrier edge is intentionally preserved by #1187.
    let native: Vec<_> = (0..document.tree().node_count())
        .map(|i| {
            document
                .tree()
                .node(adze::document::NodeId::new(i))
                .unwrap()
        })
        .filter(|node| node.grammar_id().0 == 2 && node.byte_range() == (1..2))
        .collect();
    assert_eq!(native.len(), 1);
    assert_eq!(separators[0].node_id(), native[0].node_id());
    let fact = document
        .language()
        .checked_field_semantics(root.node().unwrap().grammar_id(), "value")
        .unwrap()
        .expect("canonical root field fact");
    assert!(fact.required);
    assert!(fact.multiple);
    assert_eq!(
        fact.target_kinds.iter().map(|id| id.0).collect::<Vec<_>>(),
        vec![2, 11]
    );
}

#[test]
fn test_absent_optional_keeps_required_item_without_synthetic_separator() {
    use adze::document::{ParseOutcome, SyntaxNode};
    let document = parse_document("x").expect("absent normalized optional parses");
    assert_eq!(document.outcome(), ParseOutcome::Complete);
    assert!(document.diagnostics().is_empty());
    assert_eq!(document.tree().root().byte_range(), 0..1);
    let root = syntax::source_file(&document).expect("cast actual generated root");
    let items = root.value_item();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].text(), Some("x"));
    assert_eq!(items[0].byte_range(), Some(0..1));
    assert!(root.value_sep().is_empty());
    assert!((0..document.tree().node_count()).all(|i| {
        document
            .tree()
            .node(adze::document::NodeId::new(i))
            .unwrap()
            .grammar_id()
            .0
            != 2
    }));
    let fact = document
        .language()
        .checked_field_semantics(root.node().unwrap().grammar_id(), "value")
        .unwrap()
        .expect("canonical root field fact");
    assert!(fact.required);
    assert!(fact.multiple);
    assert_eq!(
        fact.target_kinds.iter().map(|id| id.0).collect::<Vec<_>>(),
        vec![2, 11]
    );
}

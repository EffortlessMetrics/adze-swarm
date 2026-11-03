#![deny(dead_code, unused_imports)]

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
    assert_eq!(document.validate_fields_strict_at(root.node_id()), Ok(()));
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
    let logical: Vec<_> = document
        .field_projection_view(root.node_id(), &["value"])
        .unwrap()
        .nodes("value")
        .map(|target| target.node_id())
        .collect();
    assert_eq!(logical, vec![items[0].node_id(), separators[0].node_id()]);
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
    assert_eq!(document.validate_fields_strict_at(root.node_id()), Ok(()));
    let items = root.value_item();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].text(), Some("x"));
    assert_eq!(items[0].byte_range(), Some(0..1));
    assert!(root.value_sep().is_empty());
    let logical: Vec<_> = document
        .field_projection_view(root.node_id(), &["value"])
        .unwrap()
        .nodes("value")
        .map(|target| target.node_id())
        .collect();
    assert_eq!(logical, vec![items[0].node_id()]);
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

#[test]
fn test_fact_only_descriptor_does_not_grant_projection_or_fake_empty_getter() {
    use adze::document::{FieldProjectionError, ParseOutcome};
    let document =
        adze::__private::parse_document_with_descriptor("x ", &LANGUAGE_DESCRIPTOR, GRAMMAR_NAME)
            .unwrap();
    assert_eq!(document.outcome(), ParseOutcome::Complete);
    assert!(
        document
            .language()
            .checked_field_semantics(document.tree().root().grammar_id(), "value")
            .unwrap()
            .is_some()
    );
    let id = document.tree().root_id();
    assert!(
        matches!(syntax::SourceFile::try_cast(&document, id), Err(FieldProjectionError::Unavailable { node }) if node == id)
    );
    assert!(syntax::SourceFile::cast(&document, id).is_none());
    let separator = (0..document.tree().node_count())
        .map(adze::document::NodeId::new)
        .find(|id| document.tree().node(*id).unwrap().grammar_id().0 == 2)
        .unwrap();
    assert!(
        syntax::SepToken::try_cast(&document, separator)
            .unwrap()
            .is_some(),
        "leaf needs no unrelated field capability"
    );
}

#[test]
fn test_recovered_selected_field_path_reports_incomplete() {
    use adze::document::{FieldProjectionError, ParseOutcome};
    let document = parse_document("x?").expect("native recovered document retained");
    assert!(matches!(
        document.validate_fields_strict_at(document.tree().root().node_id()),
        Err(adze::document::FieldProjectionError::Incomplete { .. })
    ));
    assert_ne!(document.outcome(), ParseOutcome::Complete);
    let id = document.tree().root_id();
    assert!(
        matches!(
            syntax::SourceFile::try_cast(&document, id),
            Err(FieldProjectionError::Incomplete { .. })
        ),
        "{document:?}"
    );
    assert!(syntax::SourceFile::cast(&document, id).is_none());
}

#[test]
fn test_projection_transport_rejects_bad_version_slot_and_carrier_alternative() {
    use adze::generated_schema::{GeneratedLanguageDescriptorV2, GeneratedSchemaError};
    let projection = LANGUAGE_DESCRIPTOR_V2.field_projection.unwrap();
    let validate = |program| {
        GeneratedLanguageDescriptorV2 {
            field_projection: Some(program),
            ..LANGUAGE_DESCRIPTOR_V2
        }
        .validate()
    };
    let mut bad = projection;
    bad.format_version = 2;
    assert_eq!(validate(bad), Err(GeneratedSchemaError::InvalidProjection));
    let mut bad = projection;
    bad.schema_revision += 1;
    assert_eq!(validate(bad), Err(GeneratedSchemaError::InvalidProjection));
    for bad_slot in [false, true] {
        let mut owners = projection.owners.to_vec();
        let owner = owners.iter_mut().find(|owner| owner.owner == 10).unwrap();
        let mut fields = owner.fields.to_vec();
        let field = fields
            .iter_mut()
            .find(|field| field.optional.is_some())
            .unwrap();
        if bad_slot {
            field.position = usize::MAX;
        } else {
            field.optional.as_mut().unwrap().present_alternative = usize::MAX;
        }
        owner.fields = Box::leak(fields.into_boxed_slice());
        let bad = adze::generated_schema::GeneratedFieldProjectionV1 {
            owners: Box::leak(owners.into_boxed_slice()),
            ..projection
        };
        assert_eq!(validate(bad), Err(GeneratedSchemaError::InvalidProjection));
    }
}

pub mod foreign {
    const GRAMMAR_NAME: &str = "schema_hidden_consumer";
    include!(concat!(env!("OUT_DIR"), "/foreign_generated.rs"));
}
#[test]
fn test_foreign_grammar_id_collision_does_not_grant_generated_leaf_wrapper() {
    use adze::document::{NodeId, ParseOutcome};
    let document = foreign::parse_document("z").unwrap();
    assert_eq!(
        document.language().name(),
        GRAMMAR_NAME,
        "grammar names can collide too"
    );
    assert_eq!(document.outcome(), ParseOutcome::Complete);
    assert!(document.diagnostics().is_empty());
    // This entire generated module has no field accessors. It must compile
    // under the same strict lint policy and still admit its own checked root.
    let own_root = foreign::syntax::SourceFile::try_cast(&document, document.tree().root_id())
        .expect("fieldless checked cast")
        .expect("own grammar root");
    assert_eq!(
        adze::document::SyntaxNode::node_id(&own_root),
        document.tree().root_id()
    );
    let id = (0..document.tree().node_count())
        .map(NodeId::new)
        .find(|id| document.tree().node(*id).unwrap().grammar_name() == Some("foreign_token"))
        .unwrap();
    assert_eq!(document.tree().node(id).unwrap().grammar_id().0, 2);
    assert_eq!(document.tree().node(id).unwrap().utf8_text().unwrap(), "z");
    assert!(!std::ptr::eq(&LANGUAGE, &foreign::LANGUAGE));
    assert!(
        syntax::SepToken::try_cast(&document, id).unwrap().is_none(),
        "grammar-local numeric IDs cannot identify another grammar's typed wrapper"
    );
}

#[test]
fn test_revision_one_keeps_positional_optional_presence() {
    use adze::document::SyntaxNode;
    for (source, separators) in [("x ", 1), ("x", 0)] {
        let mut descriptor = LANGUAGE_DESCRIPTOR_V2;
        descriptor
            .field_projection
            .as_mut()
            .unwrap()
            .projection_revision = 1;
        assert_eq!(descriptor.validate(), Ok(()));
        let descriptor = Box::leak(Box::new(descriptor));
        let document = adze::__private::parse_document_with_descriptor_v2_and_identity(
            source,
            descriptor,
            Some(&START_IDENTITY),
            GRAMMAR_NAME,
        )
        .unwrap();
        let root = syntax::source_file(&document).unwrap();
        assert_eq!(root.value_item()[0].text(), Some("x"));
        assert_eq!(root.value_sep().len(), separators);
        assert_eq!(document.validate_fields_strict_at(root.node_id()), Ok(()));
    }
}

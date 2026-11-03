//! Co-generated canonical facts, identity rejection, and ABI-only availability.
#![cfg(feature = "pure-rust")]

use adze::document::{FieldSchemaAvailability, FieldValidationError};
use adze::generated_schema::{
    GeneratedFieldFact, GeneratedLanguageDescriptor, GeneratedSchemaError,
};
use adze_example::fielded_typed_cst_contract::grammar as pair;
use adze_ir::SymbolId;

fn generated_status() -> FieldSchemaAvailability {
    FieldSchemaAvailability::Generated {
        format_version: 1,
        schema_revision: 1,
    }
}

#[test]
fn test_pair_descriptor_carries_canonical_owner_targets_and_cardinality() {
    let descriptor = &pair::LANGUAGE_DESCRIPTOR;
    assert_eq!(descriptor.validate(), Ok(()));
    assert_eq!(
        descriptor.field_schema.public_symbols,
        &[0, 5, 6, 1, 2, 3, 4]
    );
    let document = pair::parse_document("123+").unwrap();
    assert_eq!(
        document.language().field_schema_availability(),
        generated_status()
    );
    for (name, target) in [("left", 5), ("right", 6)] {
        let fact = document
            .language()
            .checked_field_semantics(SymbolId(4), name)
            .unwrap()
            .unwrap();
        assert_eq!(fact.target_kinds, vec![SymbolId(target)]);
        assert!(fact.required);
        assert!(!fact.multiple);
    }
    assert_eq!(
        document
            .language()
            .checked_field_semantics(SymbolId(4), "missing"),
        Ok(None)
    );
    assert_eq!(document.validate_fields(), Ok(()));
    let pair_owner = (0..document.tree().node_count())
        .map(adze::document::NodeId::new)
        .find(|id| document.tree().node(*id).unwrap().grammar_id() == SymbolId(4))
        .unwrap();
    assert_eq!(document.validate_fields_strict_at(pair_owner), Ok(()));
}

#[test]
fn test_descriptor_rejects_same_tables_attached_to_different_native_object() {
    // Copy only Copy fields; the second native object has precisely the same
    // counts and pointers, but a different language identity.
    let language = pair::LANGUAGE_DESCRIPTOR.language;
    macro_rules! copy_language {
        ($($field:ident),* $(,)?) => { adze::pure_parser::TSLanguage { $($field: language.$field),* } };
    }
    let other = Box::leak(Box::new(copy_language!(
        version,
        symbol_count,
        alias_count,
        token_count,
        external_token_count,
        state_count,
        large_state_count,
        production_id_count,
        field_count,
        max_alias_sequence_length,
        production_id_map,
        parse_table,
        small_parse_table,
        small_parse_table_map,
        parse_actions,
        symbol_names,
        field_names,
        field_map_slices,
        field_map_entries,
        symbol_metadata,
        public_symbol_map,
        alias_map,
        alias_sequences,
        lex_modes,
        lex_fn,
        keyword_lex_fn,
        keyword_capture_token,
        external_scanner,
        primary_state_ids,
        production_lhs_index,
        production_count,
        eof_symbol,
        rules,
        rule_count,
    )));
    let descriptor = GeneratedLanguageDescriptor {
        language: other,
        ..pair::LANGUAGE_DESCRIPTOR
    };
    assert_eq!(
        descriptor.validate(),
        Err(GeneratedSchemaError::LanguageIdentity)
    );
    let descriptor = Box::leak(Box::new(descriptor));
    assert!(adze::__private::parse_document_with_descriptor("123+", descriptor, "wrong").is_err());
}

#[test]
fn test_descriptor_rejects_changed_public_identity_snapshot() {
    let mut descriptor = pair::LANGUAGE_DESCRIPTOR;
    descriptor.field_schema.public_symbols = &[0, 6, 5, 1, 2, 3, 4];
    assert_eq!(
        descriptor.validate(),
        Err(GeneratedSchemaError::PublicSymbols)
    );
    descriptor.field_schema.public_symbols = &[0, 5, 6];
    assert_eq!(
        descriptor.validate(),
        Err(GeneratedSchemaError::PublicSymbols)
    );
}

#[test]
fn test_descriptor_rejects_changed_field_name_snapshot() {
    let mut descriptor = pair::LANGUAGE_DESCRIPTOR;
    descriptor.field_schema.field_names = &["right", "left"];
    assert_eq!(descriptor.validate(), Err(GeneratedSchemaError::FieldNames));
    descriptor.field_schema.field_names = &["left"];
    assert_eq!(descriptor.validate(), Err(GeneratedSchemaError::FieldNames));
}

#[test]
fn test_descriptor_rejects_unsupported_format_and_semantic_revision() {
    let mut descriptor = pair::LANGUAGE_DESCRIPTOR;
    descriptor.field_schema.format_version = 999;
    assert_eq!(
        descriptor.validate(),
        Err(GeneratedSchemaError::UnsupportedFormat(999))
    );
    descriptor = pair::LANGUAGE_DESCRIPTOR;
    descriptor.field_schema.schema_revision = 999;
    assert_eq!(
        descriptor.validate(),
        Err(GeneratedSchemaError::UnsupportedRevision(999))
    );
}

#[test]
fn test_descriptor_rejects_unknown_fact_references_and_duplicate_facts() {
    let invalid: &[&[GeneratedFieldFact]] = &[
        &[GeneratedFieldFact {
            owner: 999,
            name: "left",
            required: true,
            multiple: false,
            targets: &[5],
        }],
        &[GeneratedFieldFact {
            owner: 5,
            name: "left",
            required: true,
            multiple: false,
            targets: &[5],
        }],
        &[GeneratedFieldFact {
            owner: 4,
            name: "missing",
            required: true,
            multiple: false,
            targets: &[5],
        }],
        &[GeneratedFieldFact {
            owner: 4,
            name: "left",
            required: true,
            multiple: false,
            targets: &[999],
        }],
        &[GeneratedFieldFact {
            owner: 4,
            name: "left",
            required: true,
            multiple: false,
            targets: &[5, 5],
        }],
        &[
            GeneratedFieldFact {
                owner: 4,
                name: "left",
                required: true,
                multiple: false,
                targets: &[5],
            },
            GeneratedFieldFact {
                owner: 4,
                name: "left",
                required: true,
                multiple: false,
                targets: &[5],
            },
        ],
    ];
    for facts in invalid {
        let mut descriptor = pair::LANGUAGE_DESCRIPTOR;
        descriptor.field_schema.facts = facts;
        assert!(matches!(
            descriptor.validate(),
            Err(GeneratedSchemaError::InvalidFact { .. })
        ));
    }
}

#[test]
fn test_abi_only_documents_explicitly_lack_schema_even_when_they_have_field_names() {
    for source in ["123+", "?"] {
        let document =
            adze::__private::parse_document(source, pair::language, "pair ABI only").unwrap();
        assert_eq!(
            document.language().field_schema_availability(),
            FieldSchemaAvailability::Unavailable
        );
        assert!(
            document
                .language()
                .checked_field_semantics(SymbolId(4), "left")
                .is_err()
        );
        assert!(
            document
                .language()
                .field_semantics(SymbolId(4), "left")
                .is_none()
        );
        assert_eq!(
            document.validate_fields(),
            Err(FieldValidationError::SchemaUnavailable)
        );
        if source == "?" {
            assert!(document.metadata().error_count > 0);
        }
    }
}

#[test]
fn test_generated_lr_error_documents_keep_canonical_schema() {
    for source in ["123", "?"] {
        let document = pair::parse_document(source).unwrap();
        assert!(document.metadata().error_count > 0);
        assert_eq!(
            document.language().field_schema_availability(),
            generated_status()
        );
        assert_eq!(
            document
                .language()
                .checked_field_semantics(SymbolId(4), "right")
                .unwrap()
                .unwrap()
                .target_kinds,
            vec![SymbolId(6)]
        );
    }
}

#[cfg(feature = "glr")]
#[test]
fn test_real_streaming_glr_success_and_error_documents_keep_generated_facts() {
    use adze_example::fielded_ambiguous_schema_contract::grammar as conflicted;
    let descriptor = &conflicted::LANGUAGE_DESCRIPTOR;
    // The Number alternative omits all three Binary fields, so each is
    // optional at Expr owner 3; recursion targets Expr and the operator is 4.
    let facts: Vec<_> = descriptor
        .field_schema
        .facts
        .iter()
        .map(|fact| {
            (
                fact.owner,
                fact.name,
                fact.required,
                fact.multiple,
                fact.targets,
            )
        })
        .collect();
    assert_eq!(
        facts,
        vec![
            (3, "left", false, false, &[3][..]),
            (3, "operator", false, false, &[4][..]),
            (3, "right", false, false, &[3][..]),
        ]
    );
    let table = adze::decoder::decode_parse_table(descriptor.language);
    assert_eq!(
        adze::__private::engine_dispatch_for_table(descriptor.language, &table),
        adze::__private::EngineDispatch::StreamingGlr
    );
    for source in ["1+2+3", "1+", "?"] {
        let document = conflicted::parse_document(source).unwrap();
        assert_eq!(
            document.language().field_schema_availability(),
            generated_status()
        );
        for fact in descriptor.field_schema.facts {
            let actual = document
                .language()
                .checked_field_semantics(SymbolId(fact.owner), fact.name)
                .unwrap()
                .unwrap();
            assert_eq!(
                actual.target_kinds,
                fact.targets
                    .iter()
                    .copied()
                    .map(SymbolId)
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                (actual.required, actual.multiple),
                (fact.required, fact.multiple)
            );
        }
        assert!(!descriptor.field_schema.facts.is_empty());
        if source != "1+2+3" {
            assert!(document.metadata().error_count > 0);
        } else {
            assert_eq!(document.metadata().error_count, 0);
            assert_eq!(document.validate_fields(), Ok(()));
        }
        let raw = adze::__private::parse_document(source, conflicted::language, "raw conflicted")
            .unwrap();
        assert_eq!(
            raw.language().field_schema_availability(),
            FieldSchemaAvailability::Unavailable
        );
        assert_eq!(
            raw.validate_fields(),
            Err(FieldValidationError::SchemaUnavailable)
        );
    }
}

#[test]
fn test_projection_revisions_preserve_legacy_positional_and_reject_unknown() {
    assert_eq!(
        pair::LANGUAGE_DESCRIPTOR_V2
            .field_projection
            .unwrap()
            .projection_revision,
        2
    );
    for revision in [1, 2] {
        let mut descriptor = pair::LANGUAGE_DESCRIPTOR_V2;
        descriptor
            .field_projection
            .as_mut()
            .unwrap()
            .projection_revision = revision;
        assert_eq!(descriptor.validate(), Ok(()));
        let descriptor = Box::leak(Box::new(descriptor));
        let document = adze::__private::parse_document_with_descriptor_v2_and_identity(
            "123+",
            descriptor,
            Some(&pair::START_IDENTITY),
            "pair",
        )
        .unwrap();
        let owner = (0..document.tree().node_count())
            .map(adze::document::NodeId::new)
            .find(|id| document.tree().node(*id).unwrap().grammar_id() == SymbolId(4))
            .unwrap();
        let view = document
            .field_projection_view(owner, &["left", "right"])
            .unwrap();
        let left = view.nodes("left").next().unwrap().node_id();
        assert_eq!(
            document.tree().node(left).unwrap().utf8_text().unwrap(),
            "123"
        );
        assert_eq!(document.validate_fields_strict_at(owner), Ok(()));
    }
    for revision in [0, 3, u32::MAX] {
        let mut descriptor = pair::LANGUAGE_DESCRIPTOR_V2;
        descriptor
            .field_projection
            .as_mut()
            .unwrap()
            .projection_revision = revision;
        assert!(matches!(
            descriptor.validate(),
            Err(GeneratedSchemaError::InvalidProjection)
        ));
    }
}

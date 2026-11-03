#[adze::grammar("occurrence_nullable_baseline")]
pub mod nullable {
    use adze::Spanned;

    #[adze::language]
    #[derive(Debug)]
    pub struct NumberList {
        #[adze::delimited(
            #[adze::leaf(text = ",")]
            ()
        )]
        #[adze::leaf(pattern = r"\d+", try_transform = |v| v.parse::<i32>())]
        pub numbers: Spanned<Vec<Spanned<Option<i32>>>>,
        #[adze::skip(123)]
        pub metadata: u32,
    }

    #[adze::extra]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s")]
        _whitespace: (),
    }
}

pub mod ir_repeat {
    const GRAMMAR_NAME: &str = "occurrence_ir_repeat_baseline";
    include!(concat!(env!("OUT_DIR"), "/ir_repeat.rs"));
}

#[cfg(test)]
mod tests {
    use super::{ir_repeat, nullable};
    use adze::document::{AdzeDocument, NodeId, ParseOutcome};
    use adze::generated_identity::NativeLanguageIdentityV1;
    use adze::generated_schema::GeneratedLanguageDescriptorV2;
    use serde_json::{Value, json};

    type ExpectedItem = (Option<i32>, (usize, usize));

    fn assert_complete(document: &AdzeDocument, source: &str) {
        assert_eq!(document.outcome(), ParseOutcome::Complete);
        assert!(document.diagnostics().is_empty());
        assert_eq!(document.metadata().error_count, 0);
        assert_eq!(document.tree().root().byte_range(), 0..source.len());
    }

    fn number_values(value: &nullable::NumberList) -> Vec<ExpectedItem> {
        value
            .numbers
            .value
            .iter()
            .map(|item| (item.value, item.span))
            .collect()
    }

    #[test]
    fn test_explicit_ir_zero_one_several_values_and_spans() {
        let cases: &[(&str, &[(&str, (usize, usize))])] = &[
            ("[]", &[]),
            ("[a]", &[("a", (1, 2))]),
            ("[abc]", &[("a", (1, 2)), ("b", (2, 3)), ("c", (3, 4))]),
        ];
        for (source, expected) in cases {
            let document = ir_repeat::parse_document(source).expect("valid explicit IR input");
            assert_complete(&document, source);
            // The declared letter identity is the independent lexical oracle.
            // This neither calls field projection nor assigns target NodeIds.
            let actual: Vec<_> = (0..document.tree().node_count())
                .map(|index| document.tree().node(NodeId::new(index)).unwrap())
                .filter(|node| node.grammar_id().0 == 1)
                .map(|node| {
                    let bytes = node.byte_range();
                    let points = node.point_range();
                    assert_eq!((points.start.row, points.end.row), (0, 0));
                    assert_eq!(points.start.column as usize, bytes.start);
                    assert_eq!(points.end.column as usize, bytes.end);
                    (node.utf8_text().unwrap(), (bytes.start, bytes.end))
                })
                .collect();
            assert_eq!(actual.as_slice(), *expected, "{source:?}");
        }
    }

    #[test]
    fn test_nullable_values_and_spans_match_retained_independent_cases() {
        let cases: &[(&str, &[ExpectedItem])] = &[
            ("", &[]),
            ("1", &[(Some(1), (0, 1))]),
            ("1, 2", &[(Some(1), (0, 1)), (Some(2), (3, 4))]),
            ("1,", &[(Some(1), (0, 1)), (None, (2, 2))]),
            (
                "1,, 2",
                &[(Some(1), (0, 1)), (None, (2, 2)), (Some(2), (4, 5))],
            ),
            (
                "1,, 2,",
                &[
                    (Some(1), (0, 1)),
                    (None, (2, 2)),
                    (Some(2), (4, 5)),
                    (None, (6, 6)),
                ],
            ),
        ];
        for (source, expected) in cases {
            let direct = nullable::parse(source).expect("retained nullable typed input");
            assert_eq!(number_values(&direct).as_slice(), *expected, "{source:?}");
            assert_eq!(direct.numbers.span, (0, source.len()));
            assert_eq!(direct.metadata, 123);

            let document = nullable::parse_document(source).expect("nullable document input");
            assert_complete(&document, source);
            let projected: nullable::NumberList = document.ast().expect("nullable AST projection");
            assert_eq!(
                number_values(&projected).as_slice(),
                *expected,
                "{source:?}"
            );
            assert_eq!(projected.numbers.span, (0, source.len()));
            assert_eq!(projected.metadata, 123);
        }
    }

    fn physical_record(case: &str, source: &str, document: &AdzeDocument) -> Value {
        let tree = document.tree();
        let nodes: Vec<_> = (0..tree.node_count())
            .map(|index| {
                let node = tree.node(NodeId::new(index)).unwrap();
                let bytes = node.byte_range();
                let points = node.point_range();
                let edges: Vec<_> = node
                    .child_edges()
                    .map(|edge| {
                        json!({
                            "parent": edge.parent_id().as_usize(),
                            "child": edge.child_id().as_usize(),
                            "child_index": edge.child_index(),
                            "field_id": edge.field_id().map(|id| id.get()),
                            "field_name": edge.field_name(),
                        })
                    })
                    .collect();
                json!({
                    "id": node.node_id().as_usize(),
                    "parent": node.parent_id().map(|id| id.as_usize()),
                    "child_index": node.child_index(),
                    "grammar_id": node.grammar_id().0,
                    "visible_id": node.kind_id().0,
                    "grammar_name": node.grammar_name(),
                    "visible_name": node.kind_name(),
                    "alias_id": node.record().alias_symbol_id().map(|id| id.0),
                    "text": node.utf8_text().unwrap(),
                    "bytes": [bytes.start, bytes.end],
                    "points": [
                        [points.start.row, points.start.column],
                        [points.end.row, points.end.column],
                    ],
                    "flags": {
                        "named": node.is_named(), "visible": node.is_visible(),
                        "terminal": node.is_terminal(), "extra": node.is_extra(),
                        "error": node.is_error(), "missing": node.is_missing(),
                        "has_error": node.has_error(),
                    },
                    "edges": edges,
                })
            })
            .collect();
        json!({
            "case": case, "source": source,
            "root": tree.root().node_id().as_usize(),
            "outcome": document.outcome().as_str(),
            "error_count": document.metadata().error_count,
            "diagnostic_count": document.diagnostics().len(),
            "nodes": nodes,
        })
    }

    fn descriptor_record(
        descriptor: &'static GeneratedLanguageDescriptorV2,
        identity: &NativeLanguageIdentityV1,
    ) -> Value {
        let table = identity.decode_for(descriptor.language).unwrap();
        let dispatch = adze::__private::engine_dispatch_for_table(descriptor.language, &table);
        assert_eq!(dispatch, adze::__private::EngineDispatch::DeterministicLr);
        let facts: Vec<_> = descriptor
            .field_schema
            .facts
            .iter()
            .map(|fact| {
                json!({
                    "owner": fact.owner, "name": fact.name,
                    "required": fact.required, "multiple": fact.multiple,
                    "targets": fact.targets,
                })
            })
            .collect();
        let projection = descriptor.field_projection.map(|program| {
            let owners: Vec<_> = program
                .owners
                .iter()
                .map(|owner| {
                    let fields: Vec<_> = owner
                        .fields
                        .iter()
                        .map(|field| {
                            json!({
                                "name": field.name, "position": field.position,
                                "target": field.target,
                                "optional": field.optional.map(|optional| json!({
                                    "helper": optional.helper,
                                    "present": optional.present_alternative,
                                    "empty": optional.empty_alternative,
                                })),
                            })
                        })
                        .collect();
                    json!({"owner": owner.owner, "rhs": owner.rhs, "fields": fields})
                })
                .collect();
            json!({
                "format": program.format_version,
                "revision": program.projection_revision,
                "owners": owners,
            })
        });
        let rules: Vec<_> = table
            .rules
            .iter()
            .enumerate()
            .map(|(rule_id, rule)| {
                json!({"rule_id": rule_id, "lhs": rule.lhs.0, "rhs_len": rule.rhs_len})
            })
            .collect();
        let metadata: Vec<_> = table
            .grammar
            .production_ids
            .iter()
            .map(|(rule, production)| [rule.0, production.0])
            .collect();
        json!({
            "route": format!("{dispatch:?}"),
            "public_symbols": descriptor.field_schema.public_symbols,
            "field_names": descriptor.field_schema.field_names,
            "schema_revision": descriptor.field_schema.schema_revision,
            "facts": facts, "projection": projection,
            "decoded_rules": rules, "rule_to_production": metadata,
        })
    }

    #[test]
    fn test_record_physical_origins_without_assigning_expected_node_ids() {
        // Recording is separate from the literal value/range assertions above.
        // These are actual records for later review, not an expected-ID oracle.
        let mut cases = Vec::new();
        for source in ["[]", "[a]", "[abc]"] {
            let document = ir_repeat::parse_document(source).unwrap();
            cases.push(physical_record("explicit_ir_repeat", source, &document));
        }
        for source in ["", "1", "1, 2", "1,", "1,, 2", "1,, 2,"] {
            let document = nullable::parse_document(source).unwrap();
            cases.push(physical_record("annotated_nullable", source, &document));
        }
        let origin_inputs: Value = serde_json::from_str(include_str!(concat!(
            env!("OUT_DIR"),
            "/origin-inputs.json"
        )))
        .unwrap();
        let manifest = json!({
            "schema": "adze.test.physical-origin.v1",
            "recording_only": true,
            "origin_inputs": origin_inputs,
            "explicit_ir_descriptor": descriptor_record(
                &ir_repeat::LANGUAGE_DESCRIPTOR_V2, &ir_repeat::START_IDENTITY,
            ),
            "annotated_descriptor": descriptor_record(
                &nullable::LANGUAGE_DESCRIPTOR_V2, &nullable::START_IDENTITY,
            ),
            "cases": cases,
        });
        let path = std::env::var_os("ADZE_OCCURRENCE_MANIFEST")
            .expect("the controller provides an isolated recording path");
        let mut bytes = serde_json::to_vec_pretty(&manifest).unwrap();
        bytes.push(b'\n');
        std::fs::write(path, bytes).unwrap();
    }
}

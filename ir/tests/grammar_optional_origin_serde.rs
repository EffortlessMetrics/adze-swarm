//! Optional provenance occupies fixed sequence slots even when empty.
use adze_ir::{
    FieldId, Grammar, OptionalFieldOrigin, OptionalOrigin, SourceRuleId, Symbol, SymbolId,
};

fn grammar(with_origins: bool, with_fields: bool) -> Grammar {
    let mut grammar = Grammar::new("optional_origin_serde".into());
    grammar.start_symbol = Some(SymbolId(10));
    grammar.document_start_symbol = Some(SymbolId(20));
    grammar
        .wrapper_token_relations
        .insert(SymbolId(30), SymbolId(1));
    if with_origins {
        grammar.optional_origins.insert(
            SymbolId(40),
            OptionalOrigin {
                present: Symbol::Terminal(SymbolId(1)),
                present_alternative: 0,
                empty_alternative: 1,
            },
        );
    }
    if with_fields {
        grammar.optional_field_origins.push(OptionalFieldOrigin {
            source_rule: SourceRuleId {
                owner: SymbolId(10),
                alternative: 2,
            },
            source_rhs_position: 3,
            lowered_rhs_position: 4,
            field: FieldId(5),
            helper: SymbolId(40),
        });
    }
    grammar
}

#[test]
fn test_postcard_preserves_all_empty_and_nonempty_provenance_slot_combinations() {
    for origins in [false, true] {
        for fields in [false, true] {
            let grammar = grammar(origins, fields);
            let bytes = postcard::to_allocvec(&grammar).unwrap();
            let restored: Grammar = postcard::from_bytes(&bytes).unwrap();
            assert_eq!(restored, grammar, "origins={origins}, fields={fields}");
            assert_eq!(restored.explicit_start_symbol(), Some(SymbolId(10)));
            assert_eq!(restored.document_start_symbol, Some(SymbolId(20)));
        }
    }
}

#[test]
fn test_json_serializes_both_empty_provenance_slots_explicitly() {
    let grammar = grammar(false, false);
    let encoded = serde_json::to_value(&grammar).unwrap();
    assert_eq!(
        encoded.get("optional_origins"),
        Some(&serde_json::json!({}))
    );
    assert_eq!(
        encoded.get("optional_field_origins"),
        Some(&serde_json::json!([]))
    );
    let restored: Grammar = serde_json::from_value(encoded).unwrap();
    assert_eq!(restored, grammar);
}

#[test]
fn test_json_nonempty_and_legacy_missing_provenance_preserve_independent_identity() {
    let grammar = grammar(true, true);
    let encoded = serde_json::to_value(&grammar).unwrap();
    assert_eq!(encoded["optional_origins"]["40"]["present_alternative"], 0);
    assert_eq!(
        encoded["optional_field_origins"][0]["source_rhs_position"],
        3
    );
    let restored: Grammar = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(restored, grammar);
    for remove_origins in [false, true] {
        for remove_fields in [false, true] {
            let mut legacy = encoded.clone();
            let object = legacy.as_object_mut().unwrap();
            if remove_origins {
                assert!(object.remove("optional_origins").is_some());
            }
            if remove_fields {
                assert!(object.remove("optional_field_origins").is_some());
            }
            let restored: Grammar = serde_json::from_value(legacy).unwrap();
            let expected = self::grammar(!remove_origins, !remove_fields);
            assert_eq!(restored, expected);
        }
    }
}

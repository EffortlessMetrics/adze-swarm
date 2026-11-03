//! The document role has a fixed sequence slot, separate from analysis identity.
use adze_ir::builder::GrammarBuilder;
use adze_ir::{Grammar, SymbolId};

fn grammar(document_role: Option<SymbolId>) -> Grammar {
    let mut grammar = GrammarBuilder::new("document_role_serde")
        .token("x", "x")
        .rule("analysis", vec!["x"])
        .rule("document", vec!["analysis"])
        .start("analysis")
        .build();
    assert_eq!(grammar.explicit_start_symbol(), Some(SymbolId(2)));
    assert_eq!(grammar.find_symbol_by_name("document"), Some(SymbolId(3)));
    grammar.document_start_symbol = document_role;
    // This map immediately follows document_start_symbol in the sequence wire
    // representation. Keep it nonempty to catch slot shifting, not just EOF.
    grammar
        .wrapper_token_relations
        .insert(SymbolId(2), SymbolId(1));
    grammar
}

#[test]
fn test_postcard_none_document_role_preserves_following_map_and_analysis_identity() {
    let grammar = grammar(None);
    let bytes = postcard::to_allocvec(&grammar).unwrap();
    let restored: Grammar = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(restored, grammar);
    assert_eq!(restored.document_start_symbol, None);
    assert_eq!(restored.explicit_start_symbol(), Some(SymbolId(2)));
    assert_eq!(
        restored.wrapper_token_relations.get(&SymbolId(2)),
        Some(&SymbolId(1))
    );
}

#[test]
fn test_postcard_some_document_role_preserves_distinct_identity_and_following_map() {
    let grammar = grammar(Some(SymbolId(3)));
    let bytes = postcard::to_allocvec(&grammar).unwrap();
    let restored: Grammar = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(restored, grammar);
    assert_eq!(restored.document_start_symbol, Some(SymbolId(3)));
    assert_eq!(restored.explicit_start_symbol(), Some(SymbolId(2)));
    assert_eq!(
        restored.wrapper_token_relations.get(&SymbolId(2)),
        Some(&SymbolId(1))
    );
}

#[test]
fn test_json_none_document_role_is_explicit_null_without_losing_following_map() {
    let grammar = grammar(None);
    let encoded = serde_json::to_value(&grammar).unwrap();
    assert_eq!(
        encoded.get("document_start_symbol"),
        Some(&serde_json::Value::Null)
    );
    let restored: Grammar = serde_json::from_value(encoded).unwrap();
    assert_eq!(restored, grammar);
}

#[test]
fn test_json_some_and_legacy_missing_document_role_preserve_analysis_without_invention() {
    let grammar = grammar(Some(SymbolId(3)));
    let mut encoded = serde_json::to_value(&grammar).unwrap();
    assert_eq!(encoded["document_start_symbol"], serde_json::json!(3));
    let restored: Grammar = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(restored, grammar);
    assert_eq!(
        encoded
            .as_object_mut()
            .unwrap()
            .remove("document_start_symbol"),
        Some(serde_json::json!(3))
    );
    let legacy: Grammar = serde_json::from_value(encoded).unwrap();
    assert_eq!(legacy.document_start_symbol, None);
    assert_eq!(legacy.explicit_start_symbol(), Some(SymbolId(2)));
    assert_eq!(
        legacy.wrapper_token_relations.get(&SymbolId(2)),
        Some(&SymbolId(1))
    );
}

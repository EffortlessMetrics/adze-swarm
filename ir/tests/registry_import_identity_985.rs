//! Imported identity/counter admission is separate from ordinary registration.
use adze_ir::{Grammar, GrammarError, SymbolId, SymbolMetadata, SymbolRegistry};
use serde_json::{Value, json};

fn metadata() -> SymbolMetadata {
    SymbolMetadata {
        visible: true,
        named: true,
        hidden: false,
        terminal: false,
    }
}

fn sparse(id: u16, next: u16) -> Value {
    let mut value = serde_json::to_value(SymbolRegistry::new()).unwrap();
    value["symbols"]["far"] = json!(id);
    value["ids"][id.to_string()] = json!("far");
    value["metadata"][id.to_string()] = serde_json::to_value(metadata()).unwrap();
    value["next_id"] = json!(next);
    value
}

fn refuse(value: Value, cause: &str) {
    let error = serde_json::from_value::<SymbolRegistry>(value).expect_err(cause);
    assert!(
        error.to_string().contains(cause),
        "expected {cause:?}, got {error}"
    );
}

#[test]
fn test_import_rejects_each_forward_inverse_and_metadata_inconsistency() {
    let base = sparse(7, 8);
    let mut value = base.clone();
    value["symbols"].as_object_mut().unwrap().remove("end");
    refuse(value, "EOF forward mapping must be end -> 0");
    let mut value = base.clone();
    value["symbols"]["end"] = json!(1);
    refuse(value, "EOF forward mapping must be end -> 0");
    let mut value = base.clone();
    value["ids"]["0"] = json!("far");
    refuse(value, "EOF inverse mapping must be 0 -> end");
    let mut value = base.clone();
    value["symbols"]["alias"] = json!(7);
    refuse(value, "duplicate forward symbol ID");
    let mut value = base.clone();
    value["ids"].as_object_mut().unwrap().remove("7");
    refuse(value, "forward and inverse maps must be exact inverses");
    let mut value = base.clone();
    value["ids"]["9"] = json!("orphan");
    refuse(value, "forward and inverse maps must be exact inverses");
    let mut value = base.clone();
    value["ids"]["7"] = json!("wrong_name");
    refuse(value, "forward and inverse maps must be exact inverses");
    let mut value = base.clone();
    value["metadata"].as_object_mut().unwrap().remove("7");
    refuse(value, "metadata keys must exactly match symbol IDs");
    let mut value = base;
    value["metadata"]["9"] = serde_json::to_value(metadata()).unwrap();
    refuse(value, "metadata keys must exactly match symbol IDs");
}

#[test]
fn test_import_rejects_free_looking_zero_occupied_and_below_high_water_counters() {
    refuse(
        sparse(7, 0),
        "zero counter requires exhausted high-water ID",
    );
    refuse(sparse(7, 7), "next ID must exceed the high-water ID");
    refuse(sparse(7, 1), "next ID must exceed the high-water ID");
}

#[test]
fn test_sparse_import_preserves_identities_allocates_above_high_water_and_roundtrips() {
    let encoded = sparse(7, 8);
    let mut registry: SymbolRegistry = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(serde_json::to_value(&registry).unwrap(), encoded);
    assert_eq!(registry.get_id("end"), Some(SymbolId(0)));
    assert_eq!(registry.get_id("far"), Some(SymbolId(7)));
    assert_eq!(
        registry.try_register("next", metadata()).unwrap(),
        SymbolId(8)
    );
    assert!(
        !registry.contains_id(SymbolId(1)),
        "old gaps must remain unused"
    );
    let restored: SymbolRegistry =
        serde_json::from_str(&serde_json::to_string(&registry).unwrap()).unwrap();
    assert_eq!(restored, registry);
    let bytes = postcard::to_stdvec(&registry).unwrap();
    let restored: SymbolRegistry = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(restored, registry);
}

#[test]
fn test_sparse_exhausted_import_preserves_metadata_updates_and_atomic_new_name_refusal() {
    let mut registry: SymbolRegistry = serde_json::from_value(sparse(u16::MAX, 0)).unwrap();
    let replacement = SymbolMetadata {
        visible: false,
        ..metadata()
    };
    assert_eq!(
        registry.try_register("far", replacement).unwrap(),
        SymbolId(u16::MAX)
    );
    // Existing public EOF metadata updates remain compatible as well.
    assert_eq!(
        registry.try_register("end", replacement).unwrap(),
        SymbolId(0)
    );
    let registry: SymbolRegistry =
        serde_json::from_value(serde_json::to_value(registry).unwrap()).unwrap();
    let mut actual = registry.clone();
    assert!(matches!(
        actual.try_register("new", metadata()),
        Err(GrammarError::SymbolSpaceExhausted)
    ));
    assert_eq!(actual, registry);
}

#[test]
fn test_full_domain_import_roundtrips_last_slot_and_atomic_capacity_refusal() {
    let mut registry = SymbolRegistry::new();
    for index in 1..=u16::MAX {
        assert_eq!(
            registry
                .try_register(&format!("symbol_{index}"), metadata())
                .unwrap(),
            SymbolId(index)
        );
    }
    let encoded = serde_json::to_string(&registry).unwrap();
    let mut restored: SymbolRegistry = serde_json::from_str(&encoded).unwrap();
    assert_eq!(restored, registry);
    assert_eq!(restored.len(), 65536);
    assert_eq!(
        restored.try_register("symbol_65535", metadata()).unwrap(),
        SymbolId(u16::MAX)
    );
    let before = restored.clone();
    assert!(matches!(
        restored.try_register("overflow", metadata()),
        Err(GrammarError::SymbolSpaceExhausted)
    ));
    assert_eq!(restored, before);
}

#[test]
fn test_fallible_cache_accessor_preserves_cached_import_and_caches_only_success() {
    let registry: SymbolRegistry = serde_json::from_value(sparse(7, 8)).unwrap();
    let mut grammar = Grammar::new("cached_import".into());
    grammar.symbol_registry = Some(registry.clone());
    assert_eq!(grammar.try_get_or_build_registry().unwrap(), &registry);
    let mut fresh = Grammar::new("fresh".into());
    assert_eq!(fresh.try_get_or_build_registry().unwrap().len(), 1);
    assert_eq!(
        fresh.symbol_registry.as_ref().unwrap().get_id("end"),
        Some(SymbolId(0))
    );
    let mut oversized = Grammar::new("capacity".into());
    oversized.externals = (0..=u16::MAX)
        .map(|index| adze_ir::ExternalToken {
            name: format!("external_{index}"),
            symbol_id: SymbolId(index),
        })
        .collect();
    assert!(matches!(
        oversized.try_get_or_build_registry(),
        Err(GrammarError::SymbolSpaceExhausted)
    ));
    assert!(oversized.symbol_registry.is_none());
}

//! Original EOF-collision input now refuses at the earlier import boundary.
use adze_ir::SymbolRegistry;

#[test]
fn test_imported_missing_reverse_eof_entry_cannot_allocate_eof_to_a_fresh_name() {
    let mut encoded = serde_json::to_value(SymbolRegistry::new()).unwrap();
    assert!(
        encoded["ids"]
            .as_object_mut()
            .unwrap()
            .remove("0")
            .is_some()
    );
    encoded["next_id"] = serde_json::json!(0);
    assert_eq!(encoded["symbols"]["end"], serde_json::json!(0));
    let error = serde_json::from_value::<SymbolRegistry>(encoded)
        .expect_err("the identical imported EOF collision input must be refused before allocation");
    assert!(
        error
            .to_string()
            .contains("invalid symbol registry: EOF inverse mapping must be 0 -> end"),
        "expected the specific EOF/inverse-map admission failure, got {error}"
    );
}

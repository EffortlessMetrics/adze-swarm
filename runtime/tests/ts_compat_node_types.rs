//! Tree-sitter compatibility node-types metadata canaries.

#![cfg(all(test, feature = "ts-compat", feature = "pure-rust"))]

use adze::{
    adze_glr_core::SymbolMetadata,
    adze_ir::{FieldId, RuleId, SymbolId},
    ts_compat::Language,
};
use serde_json::Value;

fn node_types(lang: &Language) -> Vec<Value> {
    let json = lang.node_types_json();
    serde_json::from_str(&json).expect("node-types projection should be valid JSON")
}

fn entry<'a>(entries: &'a [Value], type_name: &str) -> &'a Value {
    entries
        .iter()
        .find(|entry| entry["type"] == type_name)
        .unwrap_or_else(|| panic!("node-types should contain {type_name:?}"))
}

fn field<'a>(node_type: &'a Value, field_name: &str) -> &'a Value {
    node_type["fields"]
        .as_object()
        .and_then(|fields| fields.get(field_name))
        .unwrap_or_else(|| panic!("node-types field {field_name:?} should be present"))
}

fn field_type_names(field: &Value) -> Vec<(String, bool)> {
    field["types"]
        .as_array()
        .expect("field should expose a types array")
        .iter()
        .map(|entry| {
            (
                entry["type"]
                    .as_str()
                    .expect("field type should expose type")
                    .to_string(),
                entry["named"]
                    .as_bool()
                    .expect("field type should expose named"),
            )
        })
        .collect()
}

fn symbol_named(lang: &Language, name: &str) -> SymbolId {
    let index = lang
        .table
        .symbol_metadata
        .iter()
        .position(|metadata| metadata.name == name)
        .unwrap_or_else(|| panic!("arithmetic fixture should expose '{name}' symbol metadata"));
    let symbol = lang.table.symbol_metadata[index].symbol_id;
    assert_eq!(
        symbol.0 as usize, index,
        "arithmetic fixture metadata should be indexed by symbol id"
    );
    symbol
}

fn arithmetic_with_fields() -> Language {
    let mut lang = (*adze_example::ts_langs::arithmetic()).clone();

    for (index, name) in ["left", "operator", "right"].into_iter().enumerate() {
        let index = index as u16;
        let field_id = FieldId(index);
        lang.grammar.fields.insert(field_id, name.to_string());
        lang.table.field_names.push(name.to_string());
        lang.table.field_map.insert((RuleId(2), index), index);
    }

    let expression = symbol_named(&lang, "expression");
    let subtraction_rule = lang
        .grammar
        .rules
        .get_mut(&expression)
        .and_then(|rules| {
            rules
                .iter_mut()
                .find(|rule| rule.production_id == adze::adze_ir::ProductionId(2))
        })
        .expect("arithmetic fixture should expose subtraction expression rule");
    subtraction_rule.fields = vec![(FieldId(0), 0), (FieldId(1), 1), (FieldId(2), 2)];

    lang
}

fn arithmetic_with_expression_child_alias(alias_name: &str) -> Language {
    let mut lang = (*adze_example::ts_langs::arithmetic()).clone();
    let source_file = symbol_named(&lang, "source_file");
    let alias_symbol = SymbolId(lang.table.symbol_metadata.len() as u16);

    lang.table.symbol_metadata.push(SymbolMetadata {
        name: alias_name.to_string(),
        is_visible: true,
        is_named: true,
        is_supertype: false,
        is_terminal: false,
        is_extra: false,
        is_fragile: false,
        symbol_id: alias_symbol,
    });
    lang.table.symbol_count = lang
        .table
        .symbol_count
        .max(lang.table.symbol_metadata.len());
    lang.table
        .index_to_symbol
        .resize(lang.table.symbol_metadata.len(), SymbolId(0));
    lang.table.index_to_symbol[alias_symbol.0 as usize] = alias_symbol;

    let source_file_rule = lang
        .table
        .rules
        .iter()
        .position(|rule| rule.lhs == source_file && rule.rhs_len == 1)
        .expect("arithmetic fixture should reduce source_file from expression");
    lang.table
        .alias_sequences
        .resize_with(source_file_rule + 1, Vec::new);
    lang.table.alias_sequences[source_file_rule] = vec![Some(alias_symbol)];

    lang
}

#[test]
fn node_types_json_includes_generated_arithmetic_node_kinds() {
    let lang = adze_example::ts_langs::arithmetic();
    let entries = node_types(&lang);

    assert_eq!(entry(&entries, "source_file")["named"], true);
    assert_eq!(entry(&entries, "expression")["named"], true);
    assert_eq!(entry(&entries, "number")["named"], true);
}

#[test]
fn node_types_json_is_deterministic_for_generated_language() {
    let lang = adze_example::ts_langs::arithmetic();
    let first_json = lang.node_types_json();
    let second_json = lang.node_types_json();
    assert_eq!(first_json, second_json);

    let entries: Vec<Value> =
        serde_json::from_str(&first_json).expect("node-types projection should be valid JSON");
    let names: Vec<&str> = entries
        .iter()
        .map(|entry| {
            entry["type"]
                .as_str()
                .expect("node-types entry should expose type")
        })
        .collect();
    assert_eq!(names, vec!["expression", "source_file", "number"]);
}

#[test]
fn node_types_json_projects_field_metadata_from_language_grammar() {
    let lang = arithmetic_with_fields();
    let entries = node_types(&lang);
    let expression = entry(&entries, "expression");
    let fields = expression["fields"]
        .as_object()
        .expect("expression should expose field metadata");

    for field_name in ["left", "operator", "right"] {
        let field = fields
            .get(field_name)
            .unwrap_or_else(|| panic!("field {field_name:?} should be present"));
        assert_eq!(field["multiple"], false);
        assert_eq!(field["required"], true);
        assert!(field["types"].is_array());
    }

    // The runtime projection currently preserves field names and field shape.
    // Field target type refs remain an advisory gap covered separately by
    // tablegen node-types tests.
    assert_eq!(
        field_type_names(field(expression, "left")),
        Vec::<(String, bool)>::new()
    );
    assert_eq!(
        field_type_names(field(expression, "operator")),
        Vec::<(String, bool)>::new()
    );
    assert_eq!(
        field_type_names(field(expression, "right")),
        Vec::<(String, bool)>::new()
    );
}

#[test]
fn node_types_json_keeps_alias_projection_gap_explicit() {
    let lang = arithmetic_with_expression_child_alias("binary_expression");
    let entries = node_types(&lang);

    assert!(
        entries
            .iter()
            .all(|entry| entry["type"] != "binary_expression")
    );
    assert_eq!(entry(&entries, "expression")["named"], true);
}

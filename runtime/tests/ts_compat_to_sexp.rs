//! Tree-sitter compatibility S-expression canaries.

#![cfg(all(test, feature = "ts-compat", feature = "pure-rust"))]

use adze::{
    adze_glr_core::SymbolMetadata,
    adze_ir::{RuleId, SymbolId},
    ts_compat::{Language, Parser},
};
use std::sync::Arc;

fn arithmetic_with_fields() -> Language {
    let mut lang = (*adze_example::ts_langs::arithmetic()).clone();
    lang.table.field_names = vec![
        "left".to_string(),
        "operator".to_string(),
        "right".to_string(),
    ];
    lang.table.field_map.insert((RuleId(2), 0), 0);
    lang.table.field_map.insert((RuleId(2), 1), 1);
    lang.table.field_map.insert((RuleId(2), 2), 2);
    lang
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

fn push_alias_symbol(lang: &mut Language, alias_name: &str, alias_is_named: bool) -> SymbolId {
    let alias_symbol = SymbolId(lang.table.symbol_metadata.len() as u16);

    lang.table.symbol_metadata.push(SymbolMetadata {
        name: alias_name.to_string(),
        is_visible: true,
        is_named: alias_is_named,
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

    alias_symbol
}

fn arithmetic_with_expression_child_alias(alias_name: &str) -> Language {
    let mut lang = (*adze_example::ts_langs::arithmetic()).clone();
    let source_file = symbol_named(&lang, "source_file");
    let alias_symbol = push_alias_symbol(&mut lang, alias_name, true);

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

fn arithmetic_with_nested_expression_aliases() -> Language {
    let mut lang = (*adze_example::ts_langs::arithmetic()).clone();
    let source_file = symbol_named(&lang, "source_file");
    let expression = symbol_named(&lang, "expression");
    let outer_alias = push_alias_symbol(&mut lang, "outer_expression", true);
    let inner_alias = push_alias_symbol(&mut lang, "inner_expression", true);

    let source_file_rule = lang
        .table
        .rules
        .iter()
        .position(|rule| rule.lhs == source_file && rule.rhs_len == 1)
        .expect("arithmetic fixture should reduce source_file from expression");
    let binary_expression_rule = RuleId(2).0 as usize;
    assert!(
        lang.table
            .rules
            .get(binary_expression_rule)
            .is_some_and(|rule| rule.lhs == expression && rule.rhs_len == 3),
        "arithmetic fixture should keep rule 2 as the subtraction expression rule"
    );

    lang.table
        .alias_sequences
        .resize_with(binary_expression_rule.max(source_file_rule) + 1, Vec::new);
    lang.table.alias_sequences[source_file_rule] = vec![Some(outer_alias)];
    lang.table.alias_sequences[binary_expression_rule] =
        vec![Some(inner_alias), None, Some(inner_alias)];

    lang
}

#[test]
fn to_sexp_serializes_generated_arithmetic_tree() {
    let mut parser = Parser::new();
    parser
        .set_language(adze_example::ts_langs::arithmetic())
        .expect("Failed to set language");

    let tree = parser.parse("1-2", None).expect("Parse failed");

    assert_eq!(
        tree.root_node().to_sexp(),
        "(source_file (expression (expression) (expression)))"
    );
}

#[test]
fn to_sexp_matches_root_and_subtree_contract_for_generated_arithmetic() {
    let mut parser = Parser::new();
    parser
        .set_language(adze_example::ts_langs::arithmetic())
        .expect("Failed to set language");

    let tree = parser.parse("1-2", None).expect("Parse failed");
    let root = tree.root_node();
    let expression = root.child(0).expect("root should expose expression child");

    assert_eq!(
        root.to_sexp(),
        "(source_file (expression (expression) (expression)))"
    );
    assert_eq!(
        expression.to_sexp(),
        "(expression (expression) (expression))"
    );
}

#[test]
fn alias_visible_identity_is_used_in_sexp() {
    let mut parser = Parser::new();
    parser
        .set_language(Arc::new(arithmetic_with_expression_child_alias(
            "binary_expression",
        )))
        .expect("Failed to set language");

    let tree = parser.parse("1-2", None).expect("Parse failed");
    let expression = tree
        .root_node()
        .child(0)
        .expect("root should expose aliased expression child");

    assert_eq!(
        tree.root_node().to_sexp(),
        "(source_file (binary_expression (expression) (expression)))"
    );
    assert_eq!(
        expression.to_sexp(),
        "(binary_expression (expression) (expression))"
    );
}

#[test]
fn nested_alias_visible_identity_is_used_in_sexp() {
    let mut parser = Parser::new();
    parser
        .set_language(Arc::new(arithmetic_with_nested_expression_aliases()))
        .expect("Failed to set language");

    let tree = parser.parse("1-2", None).expect("Parse failed");
    let expression = tree
        .root_node()
        .child(0)
        .expect("root should expose outer aliased expression child");

    assert_eq!(
        tree.root_node().to_sexp(),
        "(source_file (outer_expression (inner_expression) (inner_expression)))"
    );
    assert_eq!(
        expression.to_sexp(),
        "(outer_expression (inner_expression) (inner_expression))"
    );
}

#[test]
fn to_sexp_omits_anonymous_operator_children() {
    let mut parser = Parser::new();
    parser
        .set_language(adze_example::ts_langs::arithmetic())
        .expect("Failed to set language");

    let tree = parser.parse("1-2", None).expect("Parse failed");
    let expression = tree
        .root_node()
        .child(0)
        .expect("root should expose expression child");

    assert_eq!(expression.child_count(), 3);
    assert_eq!(expression.named_child_count(), 2);
    assert_eq!(
        expression.to_sexp(),
        "(expression (expression) (expression))"
    );
    assert!(!expression.to_sexp().contains("-"));
}

#[test]
fn to_sexp_includes_field_labels_for_named_children() {
    let mut parser = Parser::new();
    parser
        .set_language(Arc::new(arithmetic_with_fields()))
        .expect("Failed to set language");

    let tree = parser.parse("1-2", None).expect("Parse failed");

    assert_eq!(
        tree.root_node().to_sexp(),
        "(source_file (expression left: (expression) right: (expression)))"
    );
}

#[test]
fn to_sexp_field_labels_round_trip_through_language_field_ids() {
    let mut parser = Parser::new();
    parser
        .set_language(Arc::new(arithmetic_with_fields()))
        .expect("Failed to set language");

    let source = "1-2";
    let tree = parser.parse(source, None).expect("Parse failed");
    let expression = tree
        .root_node()
        .child(0)
        .expect("root should expose expression child");
    let left_id = tree
        .language()
        .field_id_for_name("left")
        .expect("left should have a public field id");
    let right_id = tree
        .language()
        .field_id_for_name("right")
        .expect("right should have a public field id");

    assert_eq!(
        tree.language().field_name_for_id(left_id.get()),
        Some("left")
    );
    assert_eq!(
        expression
            .child_by_field_id(left_id.get())
            .expect("left field id should resolve")
            .text(source.as_bytes()),
        "1"
    );
    assert_eq!(
        expression
            .child_by_field_id(right_id.get())
            .expect("right field id should resolve")
            .text(source.as_bytes()),
        "2"
    );
    assert_eq!(
        expression.to_sexp(),
        "(expression left: (expression) right: (expression))"
    );
}

#[test]
fn to_sexp_remains_named_only_when_anonymous_child_has_field_id() {
    let mut parser = Parser::new();
    parser
        .set_language(Arc::new(arithmetic_with_fields()))
        .expect("Failed to set language");

    let tree = parser.parse("1-2", None).expect("Parse failed");
    let expression = tree
        .root_node()
        .child(0)
        .expect("root should expose expression child");
    let operator = expression
        .child(1)
        .expect("expression should expose operator child");

    assert!(!operator.is_named());
    assert_eq!(expression.field_name_for_child(1), Some("operator"));
    assert_eq!(expression.field_id_for_child(1).map(|id| id.get()), Some(2));
    assert_eq!(
        expression.to_sexp(),
        "(expression left: (expression) right: (expression))"
    );
    assert!(!expression.to_sexp().contains("operator:"));
    assert!(!expression.to_sexp().contains("-"));
}

#[test]
fn to_sexp_includes_missing_nodes_for_recovered_input() {
    let mut parser = Parser::new();
    parser
        .set_language(adze_example::ts_langs::arithmetic())
        .expect("Failed to set language");

    let tree = parser
        .parse("1-", None)
        .expect("parser should return an inspectable recovered tree");
    let sexp = tree.root_node().to_sexp();

    assert!(
        sexp.contains("(MISSING)"),
        "S-expression should expose recovered missing nodes: {sexp}"
    );
    assert!(
        !sexp.contains("(ERROR)"),
        "zero-width recovery should render as MISSING rather than ERROR: {sexp}"
    );
}

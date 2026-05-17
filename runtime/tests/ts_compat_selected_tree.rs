//! Tree-sitter compatibility selected-tree parity matrix.

#![cfg(all(test, feature = "ts-compat", feature = "pure-rust"))]

use adze::{
    adze_glr_core::SymbolMetadata,
    adze_ir::{RuleId, SymbolId},
    ts_compat::{Language, Node, Parser, Point},
};
use std::sync::Arc;

fn point(row: u32, column: u32) -> Point {
    Point { row, column }
}

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

fn arithmetic_with_nested_expression_aliases() -> (Language, SymbolId, SymbolId, SymbolId) {
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

    (lang, expression, outer_alias, inner_alias)
}

fn collect_missing_nodes<'tree>(node: Node<'tree>, missing: &mut Vec<Node<'tree>>) {
    if node.is_missing() {
        missing.push(node.clone());
    }

    for index in 0..node.child_count() {
        let child = node.child(index).expect("child index should be valid");
        collect_missing_nodes(child, missing);
    }
}

#[test]
fn selected_tree_matrix_covers_traversal_fields_ranges_identity_and_sexp() {
    let lang = Arc::new(arithmetic_with_fields());
    let mut parser = Parser::new();
    parser
        .set_language(Arc::clone(&lang))
        .expect("Failed to set language");

    let source = "1-2";
    let tree = parser.parse(source, None).expect("Parse failed");
    let root = tree.root_node();
    let expression = root.child(0).expect("root should expose expression child");
    let left = expression
        .child(0)
        .expect("expression should expose left child");
    let operator = expression
        .child(1)
        .expect("expression should expose operator child");
    let right = expression
        .child(2)
        .expect("expression should expose right child");

    assert_eq!(tree.language().name, lang.name);
    assert_eq!(root.kind(), "source_file");
    assert_eq!(root.child_count(), 1);
    assert_eq!(root.named_child_count(), 1);
    assert_eq!(expression.child_count(), 3);
    assert_eq!(expression.named_child_count(), 2);
    assert_eq!(expression.text(source.as_bytes()), source);
    assert_eq!(
        expression.to_sexp(),
        "(expression left: (expression) right: (expression))"
    );

    assert_eq!(expression.start_byte(), 0);
    assert_eq!(expression.end_byte(), 3);
    assert_eq!(expression.byte_range(), 0..3);
    assert_eq!(expression.start_position(), point(0, 0));
    assert_eq!(expression.end_position(), point(0, 3));
    assert_eq!(expression.range().start_byte, 0);
    assert_eq!(expression.range().end_byte, 3);

    assert_eq!(left.text(source.as_bytes()), "1");
    assert_eq!(operator.text(source.as_bytes()), "-");
    assert_eq!(right.text(source.as_bytes()), "2");
    assert!(left.is_named());
    assert!(!operator.is_named());
    assert!(right.is_named());
    assert!(!expression.is_error());
    assert!(!expression.has_error());
    assert!(!expression.is_missing());
    assert!(!expression.is_extra());

    assert_eq!(expression.field_name_for_child(0), Some("left"));
    assert_eq!(expression.field_name_for_child(1), Some("operator"));
    assert_eq!(expression.field_name_for_child(2), Some("right"));
    assert_eq!(expression.field_id_for_child(0).map(|id| id.get()), Some(1));
    assert_eq!(expression.field_id_for_child(1).map(|id| id.get()), Some(2));
    assert_eq!(expression.field_id_for_child(2).map(|id| id.get()), Some(3));
    assert_eq!(
        expression
            .child_by_field_name("operator")
            .expect("operator field should resolve")
            .text(source.as_bytes()),
        "-"
    );
    assert_eq!(
        expression
            .child_by_field_id(3)
            .expect("right field id should resolve")
            .text(source.as_bytes()),
        "2"
    );
    assert!(expression.child_by_field_name("missing").is_none());
    assert!(expression.child_by_field_id(0).is_none());

    assert_eq!(lang.field_count(), 3);
    assert_eq!(lang.field_name_for_id(1), Some("left"));
    assert_eq!(lang.field_name_for_id(2), Some("operator"));
    assert_eq!(lang.field_name_for_id(3), Some("right"));
    assert_eq!(lang.field_id_for_name("left").map(|id| id.get()), Some(1));
    assert_eq!(
        lang.field_id_for_name("operator").map(|id| id.get()),
        Some(2)
    );
    assert_eq!(lang.field_id_for_name("right").map(|id| id.get()), Some(3));

    assert!(root.parent().is_none());
    assert_eq!(
        expression
            .parent()
            .expect("expression should have root parent")
            .kind(),
        "source_file"
    );
    assert_eq!(
        left.next_sibling()
            .expect("left should have operator sibling")
            .text(source.as_bytes()),
        "-"
    );
    assert_eq!(
        operator
            .prev_sibling()
            .expect("operator should have left sibling")
            .text(source.as_bytes()),
        "1"
    );
    assert_eq!(
        right
            .prev_sibling()
            .expect("right should have operator sibling")
            .text(source.as_bytes()),
        "-"
    );

    assert_eq!(
        root.descendant_for_byte_range(1, 2)
            .expect("operator byte range should resolve")
            .kind(),
        "-"
    );
    assert_eq!(
        root.named_descendant_for_byte_range(1, 2)
            .expect("anonymous operator range should resolve to nearest named parent")
            .kind(),
        "expression"
    );
    assert_eq!(
        root.descendant_for_point_range(point(0, 0), point(0, 1))
            .expect("left point range should resolve")
            .kind(),
        "number"
    );
    assert_eq!(
        root.named_descendant_for_point_range(point(0, 1), point(0, 2))
            .expect("operator point range should resolve to named parent")
            .kind(),
        "expression"
    );

    let source_file_symbol = symbol_named(&lang, "source_file");
    let expression_symbol = symbol_named(&lang, "expression");
    let minus_symbol = symbol_named(&lang, "-");
    assert_eq!(root.kind_id(), source_file_symbol.0);
    assert_eq!(root.grammar_id(), source_file_symbol.0);
    assert_eq!(expression.kind_id(), expression_symbol.0);
    assert_eq!(expression.grammar_id(), expression_symbol.0);
    assert_eq!(operator.kind_id(), minus_symbol.0);
    assert_eq!(operator.grammar_id(), minus_symbol.0);

    let mut cursor = root.walk();
    assert_eq!(cursor.node().kind(), "source_file");
    assert!(cursor.goto_first_child());
    assert_eq!(cursor.node().kind(), "expression");
    assert!(cursor.goto_first_child());
    assert_eq!(cursor.field_name(), Some("left"));
    assert_eq!(cursor.field_id().map(|id| id.get()), Some(1));
    assert_eq!(cursor.node().text(source.as_bytes()), "1");
    assert!(cursor.goto_next_sibling());
    assert_eq!(cursor.field_name(), Some("operator"));
    assert_eq!(cursor.node().text(source.as_bytes()), "-");
    assert!(cursor.goto_next_sibling());
    assert_eq!(cursor.field_name(), Some("right"));
    assert_eq!(cursor.node().text(source.as_bytes()), "2");
    assert!(!cursor.goto_next_sibling());
}

#[test]
fn selected_tree_matrix_covers_alias_visible_and_raw_grammar_identity() {
    let (lang, expression_symbol, outer_alias, inner_alias) =
        arithmetic_with_nested_expression_aliases();
    let mut parser = Parser::new();
    parser
        .set_language(Arc::new(lang))
        .expect("Failed to set language");

    let tree = parser.parse("1-2", None).expect("Parse failed");
    let root = tree.root_node();
    let expression = root
        .child(0)
        .expect("root should expose outer aliased expression child");
    let left = expression
        .named_child(0)
        .expect("binary expression should expose aliased left child");

    assert_eq!(expression.kind(), "outer_expression");
    assert_eq!(expression.kind_id(), outer_alias.0);
    assert_eq!(expression.grammar_name(), "expression");
    assert_eq!(expression.grammar_id(), expression_symbol.0);
    assert_eq!(left.kind(), "inner_expression");
    assert_eq!(left.kind_id(), inner_alias.0);
    assert_eq!(left.grammar_name(), "expression");
    assert_eq!(left.grammar_id(), expression_symbol.0);
    assert_eq!(
        root.to_sexp(),
        "(source_file (outer_expression (inner_expression) (inner_expression)))"
    );
}

#[test]
fn selected_tree_matrix_covers_error_and_missing_node_projection() {
    let mut parser = Parser::new();
    parser
        .set_language(adze_example::ts_langs::arithmetic())
        .expect("Failed to set language");

    let source = "1-";
    let tree = parser
        .parse(source, None)
        .expect("parser should return an inspectable truncated-input error tree");
    let root = tree.root_node();

    assert!(tree.error_count() > 0);
    assert!(tree.has_errors());
    assert!(!root.is_error());
    assert!(!root.is_missing());
    assert!(root.has_error());

    let mut missing_nodes = Vec::new();
    collect_missing_nodes(root, &mut missing_nodes);
    assert!(
        !missing_nodes.is_empty(),
        "truncated expression should expose at least one recovered missing node"
    );

    for missing in missing_nodes {
        assert!(missing.is_error());
        assert!(missing.is_missing());
        assert!(missing.has_error());
        assert_eq!(missing.start_byte(), missing.end_byte());
        assert_eq!(missing.start_byte(), source.len());
    }
}

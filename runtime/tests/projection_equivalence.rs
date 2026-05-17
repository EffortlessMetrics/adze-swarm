//! Projection equivalence canaries for the GLR toolkit productization lane.
//!
//! These tests keep the API-foundation rule concrete: `AdzeDocument` is the
//! parse truth, and typed AST, typed CST, JSON, diagnostics, and Tree-sitter
//! compatibility should agree with document facts instead of constructing
//! separate parse-shaped products.

#![cfg(all(
    test,
    feature = "pure-rust",
    feature = "ts-compat",
    feature = "serialization"
))]

use adze::{
    adze_ir::RuleId,
    document::{ADZE_DOCUMENT_JSON_SCHEMA, AdzeNode, SyntaxNode},
    parser_v4::Parser as CoreParser,
    ts_compat::{Language, Node as TsNode, Tree},
};
use serde_json::Value;
use std::sync::Arc;

#[test]
fn generated_document_ast_cst_and_json_projections_agree() {
    use adze_example::fielded_precedence_typed_cst_contract::grammar::{self, Expr};

    let source = "1+2*3";
    let parsed = grammar::parse(source).expect("typed AST parse should succeed");
    let document = grammar::parse_document(source)
        .expect("parse_document should return the canonical AdzeDocument");

    let document_ast: Expr = document
        .ast()
        .expect("document-backed typed AST projection should succeed");
    assert_eq!(
        document_ast, parsed,
        "parse() and parse_document().ast() should select the same typed AST"
    );

    let syntax_root = grammar::syntax::source_file(&document)
        .expect("generated typed CST root should cast from the document root");
    let root = document.tree().root();
    assert_eq!(syntax_root.node_id(), root.node_id());
    assert_eq!(syntax_root.text(), Some(source));
    assert_eq!(syntax_root.kind_name(), root.kind_name());

    let add_node =
        find_node(root, "Expr_Add", source).expect("generic CST should contain the add node");
    let add = grammar::syntax::ExprAdd::cast(&document, add_node.node_id())
        .expect("typed CST add wrapper should cast from the same document node");
    assert_same_node(add, add_node);

    let left = add.left().expect("typed CST should expose left field");
    let operator = add
        .operator()
        .expect("typed CST should expose operator field");
    let right = add.right().expect("typed CST should expose right field");
    let generic_left = add_node
        .edge_by_field_name("left")
        .and_then(|edge| edge.child())
        .expect("generic document should expose left field edge");
    let generic_operator = add_node
        .edge_by_field_name("operator")
        .and_then(|edge| edge.child())
        .expect("generic document should expose operator field edge");
    let generic_right = add_node
        .edge_by_field_name("right")
        .and_then(|edge| edge.child())
        .expect("generic document should expose right field edge");
    assert_same_node(left, generic_left);
    assert_same_node(operator, generic_operator);
    assert_same_node(right, generic_right);
    assert_eq!(left.text(), Some("1"));
    assert_eq!(operator.text(), Some("+"));
    assert_eq!(right.text(), Some("2*3"));

    let json = document.to_json_value();
    assert_eq!(json["schema"].as_str(), Some(ADZE_DOCUMENT_JSON_SCHEMA));
    assert_eq!(
        json["diagnostics"].as_array().map(Vec::len),
        Some(document.diagnostics().len())
    );
    assert_json_node_matches_document(root, &json["tree"]["root"]);
}

#[test]
fn tree_sitter_projection_matches_document_selected_tree() {
    let lang = fielded_arithmetic_language();
    let mut parser = CoreParser::new(lang.grammar.clone(), lang.table.clone(), lang.name.clone());

    let source = "1-2";
    let document = parser
        .parse_document(source)
        .expect("core parser should return an AdzeDocument");
    let ts_tree = Tree::from_document(Arc::clone(&lang), &document);

    assert_eq!(ts_tree.error_count(), document.metadata().error_count);
    assert_ts_node_matches_document(
        document.tree().root(),
        ts_tree.root_node(),
        source.as_bytes(),
    );
    assert_eq!(
        ts_tree.root_node().to_sexp(),
        "(source_file (expression left: (expression) right: (expression)))",
        "Tree-sitter S-expression should be a selected-tree projection over document edges"
    );
}

#[test]
fn diagnostic_document_and_json_projection_agree_for_recovered_input() {
    use adze_example::typed_ast_contract::grammar;

    let source = "1 +";
    let document = grammar::parse_document(source)
        .expect("parse_document should preserve partial facts for bad input");
    let parse_errors = grammar::parse(source).expect_err("strict typed parse should reject input");
    let json = document.to_json_value();

    assert!(document.tree().has_errors());
    assert!(!document.diagnostics().is_empty());
    assert_eq!(
        json["tree"]["root"]["flags"]["has_error"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["diagnostics"].as_array().map(Vec::len),
        Some(document.diagnostics().len())
    );

    for (error, diagnostic) in parse_errors.iter().zip(document.diagnostics()) {
        assert_eq!(error.byte_span(), diagnostic.byte_span());
        assert_eq!(error.expected, diagnostic.expected);
    }

    let json_diagnostic = json["diagnostics"]
        .as_array()
        .and_then(|diagnostics| diagnostics.first())
        .expect("diagnostic JSON should serialize the native diagnostic");
    let native_diagnostic = document
        .diagnostics()
        .first()
        .expect("recovered document should contain a diagnostic");
    assert_eq!(
        json_diagnostic["start_byte"].as_u64(),
        Some(native_diagnostic.byte_span().start as u64)
    );
    assert_eq!(
        json_diagnostic["end_byte"].as_u64(),
        Some(native_diagnostic.byte_span().end as u64)
    );
    assert_eq!(
        json_diagnostic["expected"].as_array().map(Vec::len),
        Some(native_diagnostic.expected.len())
    );
}

fn fielded_arithmetic_language() -> Arc<Language> {
    let mut lang = (*adze_example::ts_langs::arithmetic()).clone();
    lang.table.field_names = vec![
        "left".to_string(),
        "operator".to_string(),
        "right".to_string(),
    ];
    lang.table.field_map.insert((RuleId(2), 0), 0);
    lang.table.field_map.insert((RuleId(2), 1), 1);
    lang.table.field_map.insert((RuleId(2), 2), 2);
    Arc::new(lang)
}

fn assert_same_node<'doc>(wrapper: impl SyntaxNode<'doc>, node: AdzeNode<'doc>) {
    assert_eq!(wrapper.node_id(), node.node_id());
    assert_eq!(wrapper.kind_name(), node.kind_name());
    assert_eq!(wrapper.byte_range(), Some(node.byte_range()));
    assert_eq!(wrapper.point_range(), Some(node.point_range()));
    assert_eq!(wrapper.text(), node.utf8_text().ok());
}

fn assert_json_node_matches_document(node: AdzeNode<'_>, json: &Value) {
    assert_eq!(json["id"].as_u64(), Some(node.node_id().as_usize() as u64));
    assert_eq!(json["kind"].as_str(), node.kind_name());
    assert_eq!(json["kind_id"].as_u64(), Some(node.kind_id().0 as u64));
    assert_eq!(json["grammar_kind"].as_str(), node.grammar_name());
    assert_eq!(
        json["grammar_id"].as_u64(),
        Some(node.grammar_id().0 as u64)
    );
    assert_eq!(
        json["range"]["start_byte"].as_u64(),
        Some(node.byte_range().start as u64)
    );
    assert_eq!(
        json["range"]["end_byte"].as_u64(),
        Some(node.byte_range().end as u64)
    );
    assert_eq!(
        json["range"]["start_point"]["row"].as_u64(),
        Some(node.point_range().start.row as u64)
    );
    assert_eq!(
        json["range"]["start_point"]["column"].as_u64(),
        Some(node.point_range().start.column as u64)
    );
    assert_eq!(
        json["range"]["end_point"]["row"].as_u64(),
        Some(node.point_range().end.row as u64)
    );
    assert_eq!(
        json["range"]["end_point"]["column"].as_u64(),
        Some(node.point_range().end.column as u64)
    );
    assert_eq!(json["flags"]["named"].as_bool(), Some(node.is_named()));
    assert_eq!(json["flags"]["extra"].as_bool(), Some(node.is_extra()));
    assert_eq!(json["flags"]["error"].as_bool(), Some(node.is_error()));
    assert_eq!(json["flags"]["missing"].as_bool(), Some(node.is_missing()));
    assert_eq!(json["flags"]["has_error"].as_bool(), Some(node.has_error()));

    let json_children = json["children"]
        .as_array()
        .expect("document JSON nodes should serialize child edges");
    assert_eq!(json_children.len(), node.child_count());
    for (child_index, json_edge) in json_children.iter().enumerate() {
        let edge = node
            .child_edge(child_index)
            .expect("native document edge should resolve");
        assert_eq!(json_edge["child_index"].as_u64(), Some(child_index as u64));
        assert_eq!(json_edge["field_name"].as_str(), edge.field_name());
        assert_eq!(
            json_edge["field_id"].as_u64(),
            edge.field_id().map(|field_id| field_id.get() as u64)
        );
        assert_json_node_matches_document(
            edge.child().expect("native edge child should resolve"),
            &json_edge["node"],
        );
    }
}

fn assert_ts_node_matches_document(
    document_node: AdzeNode<'_>,
    ts_node: TsNode<'_>,
    source: &[u8],
) {
    assert_eq!(ts_node.kind(), document_node.kind_name().unwrap_or(""));
    assert_eq!(
        ts_node.grammar_name(),
        document_node.grammar_name().unwrap_or("")
    );
    assert_eq!(ts_node.kind_id(), document_node.kind_id().0);
    assert_eq!(ts_node.grammar_id(), document_node.grammar_id().0);
    assert_eq!(ts_node.byte_range(), document_node.byte_range());
    assert_eq!(
        ts_node
            .utf8_text(source)
            .expect("projection text should be UTF-8"),
        document_node
            .utf8_text()
            .expect("document text should be UTF-8")
    );
    assert_eq!(
        ts_node.start_position().row,
        document_node.point_range().start.row
    );
    assert_eq!(
        ts_node.start_position().column,
        document_node.point_range().start.column
    );
    assert_eq!(
        ts_node.end_position().row,
        document_node.point_range().end.row
    );
    assert_eq!(
        ts_node.end_position().column,
        document_node.point_range().end.column
    );
    assert_eq!(ts_node.is_named(), document_node.is_named());
    assert_eq!(ts_node.is_extra(), document_node.is_extra());
    assert_eq!(ts_node.is_error(), document_node.is_error());
    assert_eq!(ts_node.is_missing(), document_node.is_missing());
    assert_eq!(ts_node.has_error(), document_node.has_error());
    assert_eq!(ts_node.child_count(), document_node.child_count());
    assert_eq!(
        ts_node.named_child_count(),
        (0..document_node.child_count())
            .filter_map(|index| document_node.child(index))
            .filter(AdzeNode::is_named)
            .count()
    );

    for child_index in 0..document_node.child_count() {
        assert_eq!(
            ts_node.field_name_for_child(child_index),
            document_node.field_name_for_child(child_index)
        );
        assert_eq!(
            ts_node
                .field_id_for_child(child_index)
                .map(|field_id| field_id.get()),
            document_node
                .field_id_for_child(child_index)
                .map(|field_id| field_id.get())
        );
        let document_child = document_node
            .child(child_index)
            .expect("document child should resolve");
        let ts_child = ts_node
            .child(child_index)
            .expect("Tree-sitter projection child should resolve");
        assert_ts_node_matches_document(document_child, ts_child, source);
    }
}

fn find_node<'doc>(node: AdzeNode<'doc>, kind: &str, text: &str) -> Option<AdzeNode<'doc>> {
    if node.kind_name() == Some(kind) && node.utf8_text().ok() == Some(text) {
        return Some(node);
    }

    for child_index in 0..node.child_count() {
        let child = node.child(child_index)?;
        if let Some(found) = find_node(child, kind, text) {
            return Some(found);
        }
    }

    None
}

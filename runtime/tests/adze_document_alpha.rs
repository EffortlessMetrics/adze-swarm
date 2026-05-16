//! Native parse document alpha canaries.

#![cfg(all(test, feature = "pure-rust", feature = "ts-compat"))]

use adze::{
    adze_glr_core::SymbolMetadata,
    adze_ir::{RuleId, SymbolId},
    document::NodeId,
    parser_v4::Parser as CoreParser,
    ts_compat::{Language, Tree},
};
use std::sync::Arc;

fn symbol_named(lang: &Language, name: &str) -> SymbolId {
    let index = lang
        .table
        .symbol_metadata
        .iter()
        .position(|metadata| metadata.name == name)
        .unwrap_or_else(|| panic!("arithmetic fixture should expose '{name}' symbol metadata"));
    lang.table.symbol_metadata[index].symbol_id
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

fn empty_field_arithmetic_language() -> Arc<Language> {
    let mut lang = (*adze_example::ts_langs::arithmetic()).clone();
    lang.table.field_names.clear();
    lang.table.field_map.clear();
    Arc::new(lang)
}

fn repeated_field_arithmetic_language() -> Arc<Language> {
    let mut lang = (*adze_example::ts_langs::arithmetic()).clone();
    lang.table.field_names = vec!["part".to_string()];
    lang.table.field_map.clear();
    lang.table.field_map.insert((RuleId(2), 0), 0);
    lang.table.field_map.insert((RuleId(2), 1), 0);
    lang.table.field_map.insert((RuleId(2), 2), 0);
    Arc::new(lang)
}

fn arithmetic_with_expression_child_alias(alias_name: &str) -> Arc<Language> {
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

    Arc::new(lang)
}

#[test]
fn parse_document_exposes_generic_tree_and_ts_projection_from_same_parse() {
    let lang = fielded_arithmetic_language();
    let mut parser = CoreParser::new(lang.grammar.clone(), lang.table.clone(), lang.name.clone());

    let source = "1-2";
    let document = parser
        .parse_document(source)
        .expect("document parse should succeed");

    assert_eq!(document.source_text(), source);
    assert_eq!(document.source_bytes(), source.as_bytes());
    assert_eq!(document.language().name(), lang.name.as_str());
    assert_eq!(document.language().field_count(), 3);
    assert_eq!(document.language().field_name_for_id(0), None);
    assert_eq!(document.language().field_name_for_id(1), Some("left"));
    assert_eq!(document.language().field_name_for_id(2), Some("operator"));
    assert_eq!(document.language().field_name_for_id(3), Some("right"));
    assert_eq!(
        document
            .language()
            .field_id_for_name("left")
            .map(|id| id.get()),
        Some(1)
    );
    assert_eq!(
        document
            .language()
            .field_id_for_name("operator")
            .map(|id| id.get()),
        Some(2)
    );
    assert_eq!(
        document
            .language()
            .field_id_for_name(b"right")
            .map(|id| id.get()),
        Some(3)
    );
    assert_eq!(document.language().field_id_for_name("missing"), None);
    assert_eq!(document.metadata().error_count, 0);
    assert!(document.diagnostics().is_empty());

    let tree = document.tree();
    assert_eq!(tree.language().name(), lang.name.as_str());
    assert!(!tree.has_errors());
    assert_eq!(tree.error_count(), 0);
    assert_eq!(
        tree.edge_count(),
        (0..tree.node_count())
            .filter_map(|index| tree.node(NodeId::new(index)))
            .map(|node| node.child_count())
            .sum::<usize>(),
        "direct edge records should cover every parent-to-child relation"
    );
    assert!(
        tree.node_count() >= 5,
        "tree should index root, expression, and fielded arithmetic children"
    );

    let root = tree.root();
    assert_eq!(tree.root_id(), root.node_id());
    assert!(
        document
            .diagnostics_for_node(root.node_id())
            .next()
            .is_none(),
        "clean document root should have no related diagnostics"
    );
    assert!(
        root.diagnostics().next().is_none(),
        "clean node diagnostic iterator should be empty"
    );
    let root_record = root.record();
    assert_eq!(tree.node_record(root.node_id()), Some(root_record));
    assert_eq!(root_record.visible_id(), root.kind_id());
    assert_eq!(root_record.grammar_id(), root.grammar_id());
    assert_eq!(root_record.alias_symbol_id(), None);
    assert_eq!(root_record.byte_range(), root.byte_range());
    assert_eq!(root_record.point_range(), root.point_range());
    assert_eq!(root_record.edge_range().len(), root.child_count());
    assert_eq!(root_record.flags(), root.flags());
    assert_eq!(root.parent_id(), None);
    assert!(root.parent().is_none());
    assert!(root.parent_edge().is_none());
    assert_eq!(
        tree.node(root.node_id())
            .expect("root id should resolve")
            .kind_name(),
        root.kind_name()
    );
    assert!(tree.node(NodeId::new(tree.node_count())).is_none());
    assert_eq!(root.kind_id(), symbol_named(&lang, "source_file"));
    assert_eq!(root.grammar_id(), symbol_named(&lang, "source_file"));
    assert_eq!(root.kind_name(), Some("source_file"));
    assert_eq!(root.grammar_name(), Some("source_file"));
    let root_identity = root.identity();
    assert_eq!(root_identity.visible_id(), root.kind_id());
    assert_eq!(root_identity.grammar_id(), root.grammar_id());
    assert_eq!(root_identity.visible_name(), root.kind_name());
    assert_eq!(root_identity.grammar_name(), root.grammar_name());
    assert_eq!(root_identity.alias_symbol_id(), None);
    assert!(!root_identity.has_alias());
    assert!(root_identity.visible_is_named());
    assert!(root_identity.grammar_is_named());
    let root_flags = root.flags();
    assert!(root_flags.is_named());
    assert!(root_flags.is_visible());
    assert!(!root_flags.is_terminal());
    assert!(!root_flags.is_extra());
    assert!(!root_flags.is_error());
    assert!(!root_flags.is_missing());
    assert!(!root_flags.has_error());
    assert!(root.is_named());
    assert!(root.is_visible());
    assert!(!root.is_terminal());
    assert!(!root.is_extra());
    assert_eq!(root.symbol_id(), symbol_named(&lang, "source_file"));
    assert_eq!(root.child_count(), 1);
    assert_eq!(root.utf8_text().expect("root text should be UTF-8"), source);

    let root_expression_edge = root
        .child_edge(0)
        .expect("root should expose expression edge");
    let root_expression_edge_record = root_expression_edge.record();
    assert_eq!(
        tree.edge_record(root_record.edge_range().start()),
        Some(root_expression_edge_record)
    );
    assert_eq!(
        root_expression_edge_record.parent_id(),
        root_expression_edge.parent_id()
    );
    assert_eq!(
        root_expression_edge_record.child_index(),
        root_expression_edge.child_index()
    );
    assert_eq!(
        root_expression_edge_record.field_id(),
        root_expression_edge.field_id()
    );
    assert_eq!(root_expression_edge.parent_id(), root.node_id());
    assert_eq!(root_expression_edge.child_index(), 0);
    assert_eq!(root_expression_edge.field_name(), None);
    assert_eq!(root_expression_edge.field_id(), None);

    let expression = root.child(0).expect("root should expose expression child");
    assert_eq!(root_expression_edge.child_id(), expression.node_id());
    assert_eq!(root_expression_edge_record.child_id(), expression.node_id());
    assert_eq!(
        expression
            .parent_edge()
            .expect("expression should resolve parent edge")
            .record(),
        root_expression_edge_record
    );
    assert_eq!(
        root_expression_edge
            .child()
            .expect("root expression edge should resolve child")
            .node_id(),
        expression.node_id()
    );
    assert_eq!(expression.parent_id(), Some(root.node_id()));
    assert_eq!(
        expression
            .parent()
            .expect("expression should resolve parent")
            .node_id(),
        root.node_id()
    );
    assert_eq!(
        tree.node(expression.node_id())
            .expect("expression id should resolve")
            .byte_range(),
        expression.byte_range()
    );
    assert_eq!(expression.kind_name(), Some("expression"));
    assert_eq!(expression.grammar_name(), Some("expression"));
    let expression_identity = expression.identity();
    let expression_record = expression.record();
    assert_eq!(expression_record.visible_id(), expression.kind_id());
    assert_eq!(expression_record.grammar_id(), expression.grammar_id());
    assert_eq!(expression_record.alias_symbol_id(), None);
    assert_eq!(expression_record.byte_range(), expression.byte_range());
    assert_eq!(expression_record.point_range(), expression.point_range());
    assert_eq!(
        expression_record.edge_range().len(),
        expression.child_count()
    );
    assert_eq!(expression_identity.visible_id(), expression.kind_id());
    assert_eq!(expression_identity.grammar_id(), expression.grammar_id());
    assert_eq!(expression_identity.visible_name(), Some("expression"));
    assert_eq!(expression_identity.grammar_name(), Some("expression"));
    assert_eq!(expression_identity.alias_symbol_id(), None);
    assert!(!expression_identity.has_alias());
    assert!(expression_identity.visible_is_named());
    assert!(expression_identity.grammar_is_named());
    let expression_flags = expression.flags();
    assert!(expression_flags.is_named());
    assert!(expression_flags.is_visible());
    assert!(!expression_flags.is_terminal());
    assert!(!expression_flags.is_extra());
    assert!(!expression_flags.is_error());
    assert!(!expression_flags.is_missing());
    assert!(!expression_flags.has_error());
    assert!(expression.is_named());
    assert!(expression.is_visible());
    assert!(!expression.is_terminal());
    assert_eq!(expression.symbol_id(), symbol_named(&lang, "expression"));
    assert_eq!(expression.child_count(), 3);
    assert_eq!(expression.field_name_for_child(0), Some("left"));
    assert_eq!(expression.field_name_for_child(1), Some("operator"));
    assert_eq!(expression.field_name_for_child(2), Some("right"));
    let left_field_id = document
        .language()
        .field_id_for_name("left")
        .expect("left field should resolve");
    let operator_field_id = document
        .language()
        .field_id_for_name("operator")
        .expect("operator field should resolve");
    let right_field_id = document
        .language()
        .field_id_for_name("right")
        .expect("right field should resolve");
    assert_eq!(expression.field_id_for_child(0), Some(left_field_id));
    assert_eq!(expression.field_id_for_child(1), Some(operator_field_id));
    assert_eq!(expression.field_id_for_child(2), Some(right_field_id));
    assert_eq!(expression.field_id_for_child(3), None);
    assert!(expression.child_edge(3).is_none());
    assert!(expression.edge_by_field_name("missing").is_none());
    assert!(expression.child_by_field_name("missing").is_none());

    let left = expression.child(0).expect("left child should exist");
    let operator = expression.child(1).expect("operator child should exist");
    let right = expression.child(2).expect("right child should exist");
    let edges: Vec<_> = expression.child_edges().collect();

    assert_eq!(edges.len(), 3);
    assert_eq!(edges[0].parent_id(), expression.node_id());
    assert_eq!(edges[0].child_index(), 0);
    assert_eq!(edges[0].child_id(), left.node_id());
    assert_eq!(edges[0].field_name(), Some("left"));
    assert_eq!(edges[0].field_id().map(|id| id.get()), Some(1));
    assert_eq!(edges[0].record().parent_id(), expression.node_id());
    assert_eq!(edges[0].record().child_id(), left.node_id());
    assert_eq!(edges[0].record().child_index(), 0);
    assert_eq!(edges[0].record().field_id().map(|id| id.get()), Some(1));
    assert_eq!(edges[1].field_name(), Some("operator"));
    assert_eq!(edges[1].field_id().map(|id| id.get()), Some(2));
    assert_eq!(edges[1].record().child_id(), operator.node_id());
    assert_eq!(edges[1].record().field_id().map(|id| id.get()), Some(2));
    assert_eq!(edges[2].field_name(), Some("right"));
    assert_eq!(edges[2].field_id().map(|id| id.get()), Some(3));
    assert_eq!(edges[2].record().child_id(), right.node_id());
    assert_eq!(edges[2].record().field_id().map(|id| id.get()), Some(3));
    assert_eq!(
        expression
            .edge_by_field_name("left")
            .expect("left edge should resolve")
            .child_id(),
        left.node_id()
    );
    assert_eq!(
        expression
            .child_by_field_name("operator")
            .expect("operator field should resolve")
            .node_id(),
        operator.node_id()
    );
    assert_eq!(
        expression
            .child_by_field_id(left_field_id)
            .expect("left field id should resolve")
            .node_id(),
        left.node_id()
    );
    assert_eq!(
        expression
            .child_by_field_id(operator_field_id)
            .expect("operator field id should resolve")
            .node_id(),
        operator.node_id()
    );
    assert_eq!(
        expression
            .child_by_field_id(right_field_id)
            .expect("right field id should resolve")
            .node_id(),
        right.node_id()
    );
    assert_ne!(left.node_id(), operator.node_id());
    assert_ne!(operator.node_id(), right.node_id());
    assert_eq!(left.parent_id(), Some(expression.node_id()));
    assert_eq!(operator.parent_id(), Some(expression.node_id()));
    assert_eq!(right.parent_id(), Some(expression.node_id()));
    assert_eq!(
        tree.node(left.node_id())
            .expect("left id should resolve")
            .utf8_text()
            .expect("left text should be UTF-8"),
        "1"
    );
    assert_eq!(left.field_name(), Some("left"));
    assert_eq!(left.field_id().map(|id| id.get()), Some(1));
    assert_eq!(left.utf8_text().expect("left text should be UTF-8"), "1");
    assert_eq!(operator.field_name(), Some("operator"));
    assert_eq!(operator.field_id().map(|id| id.get()), Some(2));
    let operator_flags = operator.flags();
    assert!(!operator_flags.is_named());
    assert!(operator_flags.is_terminal());
    assert!(!operator_flags.is_error());
    assert!(!operator_flags.is_missing());
    assert!(!operator_flags.has_error());
    assert_eq!(
        operator.utf8_text().expect("operator text should be UTF-8"),
        "-"
    );
    assert_eq!(right.field_name(), Some("right"));
    assert_eq!(right.field_id().map(|id| id.get()), Some(3));
    assert_eq!(right.utf8_text().expect("right text should be UTF-8"), "2");

    let ts_tree = Tree::from_document(Arc::clone(&lang), &document);
    let ts_expression = ts_tree
        .root_node()
        .child(0)
        .expect("Tree-sitter projection should expose expression child");

    assert_eq!(ts_tree.error_count(), document.metadata().error_count);
    assert_eq!(ts_expression.kind(), expression.kind_name().unwrap());
    assert_eq!(
        ts_expression.grammar_name(),
        expression.grammar_name().unwrap()
    );
    assert_eq!(ts_expression.field_name_for_child(0), Some("left"));
    assert_eq!(ts_expression.field_name_for_child(1), Some("operator"));
    assert_eq!(ts_expression.field_name_for_child(2), Some("right"));
    assert_eq!(
        ts_expression
            .field_id_for_child(0)
            .map(|field_id| field_id.get()),
        expression
            .child_edge(0)
            .and_then(|edge| edge.field_id())
            .map(|field_id| field_id.get())
    );
    assert_eq!(
        ts_expression
            .field_id_for_child(1)
            .map(|field_id| field_id.get()),
        expression
            .child_edge(1)
            .and_then(|edge| edge.field_id())
            .map(|field_id| field_id.get())
    );
    assert_eq!(
        ts_expression
            .field_id_for_child(2)
            .map(|field_id| field_id.get()),
        expression
            .child_edge(2)
            .and_then(|edge| edge.field_id())
            .map(|field_id| field_id.get())
    );
    assert_eq!(
        ts_expression
            .child_by_field_name("left")
            .expect("left field should project")
            .text(source.as_bytes()),
        "1"
    );
}

#[test]
fn parse_document_empty_field_map_has_no_edge_fields() {
    let lang = empty_field_arithmetic_language();
    let mut parser = CoreParser::new(lang.grammar.clone(), lang.table.clone(), lang.name.clone());

    let source = "1-2";
    let document = parser
        .parse_document(source)
        .expect("document parse should succeed with empty field metadata");

    assert_eq!(document.language().field_count(), 0);
    assert!(document.language().fields().is_empty());
    assert_eq!(document.language().field_name_for_id(0), None);
    assert_eq!(document.language().field_name_for_id(1), None);
    assert_eq!(document.language().field_id_for_name("left"), None);
    assert_eq!(document.language().field_id_for_name("part"), None);

    let root = document.tree().root();
    let expression = root.child(0).expect("root should expose expression child");
    assert_eq!(expression.child_count(), 3);
    assert!(expression.edge_by_field_name("left").is_none());
    assert!(expression.child_by_field_name("left").is_none());

    for child_index in 0..expression.child_count() {
        assert_eq!(expression.field_name_for_child(child_index), None);
        assert_eq!(expression.field_id_for_child(child_index), None);
        let edge = expression
            .child_edge(child_index)
            .expect("expression child edge should resolve");
        assert_eq!(edge.field_name(), None);
        assert_eq!(edge.field_id(), None);
        assert_eq!(edge.record().field_id(), None);
        assert_eq!(
            edge.child()
                .expect("edge child should resolve")
                .field_name(),
            None
        );
    }

    let ts_tree = Tree::from_document(Arc::clone(&lang), &document);
    let ts_expression = ts_tree
        .root_node()
        .child(0)
        .expect("Tree-sitter projection should expose expression child");
    assert_eq!(ts_tree.language().field_count(), 0);
    assert_eq!(ts_expression.field_name_for_child(0), None);
    assert!(ts_expression.child_by_field_name("left").is_none());
}

#[test]
fn parse_document_repeated_field_edges_remain_iterable() {
    let lang = repeated_field_arithmetic_language();
    let mut parser = CoreParser::new(lang.grammar.clone(), lang.table.clone(), lang.name.clone());

    let source = "1-2";
    let document = parser
        .parse_document(source)
        .expect("document parse should succeed with repeated field metadata");
    let part_field = document
        .language()
        .field_id_for_name("part")
        .expect("repeated field should resolve");

    let expression = document
        .tree()
        .root()
        .child(0)
        .expect("root should expose expression child");
    let part_edges: Vec<_> = expression
        .child_edges()
        .filter(|edge| edge.field_name() == Some("part"))
        .collect();
    assert_eq!(
        part_edges.len(),
        3,
        "all expression children should remain iterable through the repeated field"
    );

    let part_texts: Vec<_> = part_edges
        .iter()
        .map(|edge| {
            edge.child()
                .expect("repeated field edge should resolve its child")
                .utf8_text()
                .expect("child text should be UTF-8")
        })
        .collect();
    assert_eq!(part_texts, ["1", "-", "2"]);

    for child_index in 0..expression.child_count() {
        assert_eq!(expression.field_name_for_child(child_index), Some("part"));
        assert_eq!(expression.field_id_for_child(child_index), Some(part_field));
    }
    assert_eq!(
        expression
            .child_by_field_name("part")
            .expect("first repeated field child should resolve")
            .utf8_text()
            .expect("child text should be UTF-8"),
        "1"
    );
    assert_eq!(
        expression
            .child_by_field_id(part_field)
            .expect("first repeated field id child should resolve")
            .utf8_text()
            .expect("child text should be UTF-8"),
        "1"
    );

    let ts_tree = Tree::from_document(Arc::clone(&lang), &document);
    let ts_expression = ts_tree
        .root_node()
        .child(0)
        .expect("Tree-sitter projection should expose expression child");
    assert_eq!(ts_tree.language().field_count(), 1);
    assert_eq!(ts_expression.field_name_for_child(0), Some("part"));
    assert_eq!(ts_expression.field_name_for_child(1), Some("part"));
    assert_eq!(ts_expression.field_name_for_child(2), Some("part"));
    assert_eq!(
        ts_expression
            .child_by_field_name("part")
            .expect("first repeated Tree-sitter field child should resolve")
            .text(source.as_bytes()),
        "1"
    );
}

#[test]
fn parse_document_source_slice_respects_utf8_boundaries() {
    let lang = adze_example::ts_langs::arithmetic();
    let mut parser = CoreParser::new(lang.grammar.clone(), lang.table.clone(), lang.name.clone());

    let source = "é+1";
    let document = parser
        .parse_document(source)
        .expect("document parse should retain source text even for partial parse facts");

    assert_eq!(document.source_slice(0..2), Some("é"));
    assert_eq!(document.source_slice(2..3), Some("+"));
    assert_eq!(document.source_slice(3..4), Some("1"));
    assert_eq!(document.source_slice(4..4), Some(""));

    assert_eq!(
        document.source_slice(0..1),
        None,
        "slice ending inside a UTF-8 codepoint should be rejected"
    );
    assert_eq!(
        document.source_slice(1..2),
        None,
        "slice starting inside a UTF-8 codepoint should be rejected"
    );
    assert_eq!(
        document.source_slice(0..5),
        None,
        "slice beyond source bounds should be rejected"
    );
}

#[test]
fn parse_document_projects_alias_visible_identity_from_native_node_data() {
    let lang = arithmetic_with_expression_child_alias("binary_expression");
    let expression_symbol = symbol_named(&lang, "expression");
    let alias_symbol = symbol_named(&lang, "binary_expression");
    let mut parser = CoreParser::new(lang.grammar.clone(), lang.table.clone(), lang.name.clone());

    let document = parser
        .parse_document("1-2")
        .expect("document parse should succeed");
    let root = document.tree().root();
    let expression = root
        .child(0)
        .expect("root should expose aliased expression child");
    let identity = expression.identity();

    assert_eq!(identity.visible_name(), Some("binary_expression"));
    assert_eq!(identity.visible_id(), alias_symbol);
    assert_eq!(identity.grammar_name(), Some("expression"));
    assert_eq!(identity.grammar_id(), expression_symbol);
    assert_eq!(identity.alias_symbol_id(), Some(alias_symbol));
    assert!(identity.has_alias());
    assert!(identity.visible_is_named());
    assert!(identity.grammar_is_named());
    assert_eq!(expression.kind_name(), Some("binary_expression"));
    assert_eq!(expression.kind_id(), alias_symbol);
    assert_eq!(expression.grammar_name(), Some("expression"));
    assert_eq!(expression.grammar_id(), expression_symbol);
    assert_eq!(expression.symbol_id(), expression_symbol);
    assert!(expression.flags().is_named());

    let ts_tree = Tree::from_document(Arc::clone(&lang), &document);
    let ts_expression = ts_tree
        .root_node()
        .child(0)
        .expect("Tree-sitter projection should expose aliased expression child");
    assert_eq!(ts_expression.kind(), "binary_expression");
    assert_eq!(ts_expression.kind_id(), alias_symbol.0);
    assert_eq!(ts_expression.grammar_name(), "expression");
    assert_eq!(ts_expression.grammar_id(), expression_symbol.0);
}

#[test]
fn parse_document_exposes_recovery_metadata_and_diagnostics() {
    let lang = adze_example::ts_langs::arithmetic();
    let mut parser = CoreParser::new(lang.grammar.clone(), lang.table.clone(), lang.name.clone());

    let source = "1-@";
    let document = parser
        .parse_document(source)
        .expect("document parse should return partial parse facts");

    assert!(document.metadata().error_count > 0);
    assert!(document.tree().has_errors());
    assert!(document.tree().root().has_error());
    assert!(document.tree().root().flags().has_error());
    assert!(!document.diagnostics().is_empty());

    let diagnostic = &document.diagnostics()[0];
    assert!(diagnostic.start_byte <= diagnostic.end_byte);
    assert!(diagnostic.end_byte <= document.source_bytes().len());
    assert!(
        diagnostic.message.contains("parser recorded"),
        "diagnostic should explain the recorded parser recovery count"
    );
    let related_node = diagnostic
        .related_nodes
        .first()
        .and_then(|node_id| document.tree().node(*node_id))
        .expect("diagnostic should resolve to a related document node");
    assert!(
        related_node.has_error(),
        "related diagnostic node should carry error state"
    );
    assert!(
        related_node.flags().has_error(),
        "native node flags should carry the same aggregate error state"
    );
    assert_eq!(related_node.flags().is_error(), related_node.is_error());
    assert_eq!(related_node.flags().is_missing(), related_node.is_missing());
    assert_eq!(related_node.byte_range(), diagnostic.byte_span());
    let document_node_diagnostics: Vec<_> = document
        .diagnostics_for_node(related_node.node_id())
        .collect();
    assert_eq!(document_node_diagnostics.len(), 1);
    assert_eq!(
        document_node_diagnostics[0].byte_span(),
        diagnostic.byte_span()
    );
    let node_diagnostics: Vec<_> = related_node.diagnostics().collect();
    assert_eq!(node_diagnostics.len(), 1);
    assert_eq!(node_diagnostics[0].byte_span(), diagnostic.byte_span());
    assert!(
        document
            .diagnostics_for_node(NodeId::new(document.tree().node_count()))
            .next()
            .is_none(),
        "diagnostic lookup for an unassigned node id should be empty"
    );

    let ts_tree = Tree::from_document(lang, &document);
    assert_eq!(ts_tree.error_count(), document.metadata().error_count);
    assert!(ts_tree.has_errors());
    assert_eq!(
        ts_tree.root_node().has_error(),
        document.tree().root().has_error()
    );
}

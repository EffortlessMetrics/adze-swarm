//! Generated typed CST document-helper canaries.

#![cfg(all(test, feature = "pure-rust"))]

use adze::document::SyntaxNode;

#[test]
fn generated_parse_document_helper_feeds_generated_syntax_module() {
    let source = "1+2+3";
    let document = adze_example::typed_ast_contract::grammar::parse_document(source)
        .expect("generated parse_document helper should return an AdzeDocument");

    assert_eq!(document.source_text(), source);
    assert_eq!(
        document.metadata().error_count,
        document.tree().error_count()
    );
    assert_eq!(
        document.tree().has_errors(),
        document.metadata().error_count > 0
    );
    assert_eq!(
        document.diagnostics().is_empty(),
        document.metadata().error_count == 0
    );
    assert_eq!(document.metadata().error_count, 0);

    let syntax = adze_example::typed_ast_contract::grammar::syntax::source_file(&document)
        .expect("generated syntax root should cast from document root");

    assert_eq!(syntax.node_id(), document.tree().root_id());
    assert_eq!(syntax.kind_name(), Some("source_file"));
    assert_eq!(syntax.text(), Some(source));
    assert!(
        syntax.child(0).is_some(),
        "source_file wrapper should expose the parsed expression child"
    );
}

#[test]
fn generated_parse_document_diagnostics_preserve_expected_tokens() {
    let source = "1 +";
    let document = adze_example::typed_ast_contract::grammar::parse_document(source)
        .expect("generated parse_document helper should return partial parse facts");
    let parse_error = adze_example::typed_ast_contract::grammar::parse(source)
        .expect_err("truncated expression should fail through the typed AST parser")
        .into_iter()
        .next()
        .expect("typed AST parser should report at least one error");

    assert!(document.metadata().error_count > 0);
    assert!(document.tree().has_errors());

    let diagnostic = document
        .diagnostics()
        .first()
        .expect("parse_document should expose a structured diagnostic");

    assert_eq!(diagnostic.byte_span(), parse_error.byte_span());
    let source_span = parse_error.source_span(source.as_bytes());
    assert_eq!(
        diagnostic.point_range.start.row as usize + 1,
        source_span.start.line
    );
    assert_eq!(
        diagnostic.point_range.start.column as usize + 1,
        source_span.start.column
    );
    assert_eq!(
        diagnostic.point_range.end.row as usize + 1,
        source_span.end.line
    );
    assert_eq!(
        diagnostic.point_range.end.column as usize + 1,
        source_span.end.column
    );
    assert_eq!(diagnostic.expected, parse_error.expected);
    let related_node = diagnostic
        .related_nodes
        .first()
        .and_then(|node_id| document.tree().node(*node_id))
        .expect("document diagnostic should resolve to a related node");
    assert!(
        related_node.has_error(),
        "document diagnostic related node should carry error state"
    );
    assert_eq!(related_node.byte_range(), diagnostic.byte_span());
    assert!(
        diagnostic.expected.iter().any(|token| token == r"/\d+/"),
        "document diagnostic should preserve generated expected token names: {:?}",
        diagnostic.expected
    );
    assert!(
        diagnostic.message.contains("expected one of:"),
        "document diagnostic should retain expected-token context: {}",
        diagnostic.message
    );
    assert!(
        !diagnostic.message.contains("SymbolId")
            && !diagnostic.message.contains("symbol ")
            && !diagnostic.message.contains("_4"),
        "document diagnostic should not expose raw symbol internals: {}",
        diagnostic.message
    );

    let rendered = diagnostic.display_with_source(source).to_string();
    assert!(
        rendered.contains("at 1:4 (bytes 3..3)"),
        "document diagnostic display should include one-indexed location and byte span: {rendered}"
    );
    assert!(
        rendered.contains("1 +\n   ^"),
        "document diagnostic display should include source context and EOF marker: {rendered}"
    );
}

#[test]
fn generated_parse_document_diagnostics_byte_and_point_ranges_agree() {
    let cases = ["1 +", "1 + \u{03bb}", "1 +\n@"];

    for source in cases {
        let document = adze_example::typed_ast_contract::grammar::parse_document(source)
            .expect("generated parse_document helper should return partial parse facts");
        let diagnostic = document
            .diagnostics()
            .first()
            .unwrap_or_else(|| panic!("source {source:?} should produce a diagnostic"));
        let expected_point_range =
            adze::document::PointRange::from_byte_range(source, diagnostic.byte_span());

        assert_eq!(
            diagnostic.point_range, expected_point_range,
            "diagnostic point range should describe the same source span as its byte range for {source:?}"
        );
    }
}

#[test]
fn generated_parse_document_diagnostics_include_multibyte_byte_span() {
    let source = "1 + \u{03bb}";
    let document = adze_example::typed_ast_contract::grammar::parse_document(source)
        .expect("generated parse_document helper should return multibyte partial parse facts");
    let parse_error = adze_example::typed_ast_contract::grammar::parse(source)
        .expect_err("multibyte invalid token should fail through the typed AST parser")
        .into_iter()
        .next()
        .expect("typed AST parser should report at least one multibyte error");

    let diagnostic = document
        .diagnostics()
        .first()
        .expect("parse_document should expose a structured multibyte diagnostic");

    assert_eq!(diagnostic.byte_span(), parse_error.byte_span());
    assert_eq!(diagnostic.byte_span(), 4..6);
    assert_eq!(diagnostic.point_range.start.row, 0);
    assert_eq!(diagnostic.point_range.start.column, 4);
    assert_eq!(diagnostic.point_range.end.row, 0);
    assert_eq!(diagnostic.point_range.end.column, 6);

    let source_span = parse_error.source_span(source.as_bytes());
    assert_eq!(
        diagnostic.point_range.start.row as usize + 1,
        source_span.start.line
    );
    assert_eq!(
        diagnostic.point_range.start.column as usize + 1,
        source_span.start.column
    );
    assert_eq!(
        diagnostic.point_range.end.row as usize + 1,
        source_span.end.line
    );
    assert_eq!(
        diagnostic.point_range.end.column as usize + 1,
        source_span.end.column
    );
    assert_eq!(diagnostic.expected, parse_error.expected);
    assert!(
        diagnostic.expected.iter().any(|token| token == r"/\d+/"),
        "document diagnostic should preserve generated expected token names: {:?}",
        diagnostic.expected
    );

    let rendered = diagnostic.display_with_source(source).to_string();
    assert!(
        rendered.contains("at 1:5 (bytes 4..6)"),
        "document diagnostic display should include byte-oriented multibyte location: {rendered}"
    );
    assert!(
        rendered.contains("    ^^"),
        "document diagnostic display should mark the full UTF-8 byte width: {rendered}"
    );
}

#[test]
fn generated_parse_document_diagnostics_include_multiline_point_range() {
    let source = "1 +\n@";
    let document = adze_example::typed_ast_contract::grammar::parse_document(source)
        .expect("generated parse_document helper should return multiline partial parse facts");
    let parse_error = adze_example::typed_ast_contract::grammar::parse(source)
        .expect_err("multiline invalid token should fail through the typed AST parser")
        .into_iter()
        .next()
        .expect("typed AST parser should report at least one multiline error");

    let diagnostic = document
        .diagnostics()
        .first()
        .expect("parse_document should expose a structured multiline diagnostic");

    assert_eq!(diagnostic.byte_span(), parse_error.byte_span());
    assert_eq!(diagnostic.point_range.start.row, 1);
    assert_eq!(diagnostic.point_range.start.column, 0);
    assert_eq!(diagnostic.point_range.end.row, 1);
    assert_eq!(diagnostic.point_range.end.column, 1);

    let source_span = parse_error.source_span(source.as_bytes());
    assert_eq!(
        diagnostic.point_range.start.row as usize + 1,
        source_span.start.line
    );
    assert_eq!(
        diagnostic.point_range.start.column as usize + 1,
        source_span.start.column
    );
    assert_eq!(
        diagnostic.point_range.end.row as usize + 1,
        source_span.end.line
    );
    assert_eq!(
        diagnostic.point_range.end.column as usize + 1,
        source_span.end.column
    );

    let rendered = diagnostic.display_with_source(source).to_string();
    assert!(
        rendered.contains("at 2:1 (bytes 4..5)"),
        "document diagnostic display should include multiline location and byte span: {rendered}"
    );
    assert!(
        rendered.contains("@\n^"),
        "document diagnostic display should include the offending source line and marker: {rendered}"
    );
}

#[test]
fn generated_typed_cst_wrappers_cast_generic_document_nodes() {
    let source = "1+2+3";
    let document = adze_example::typed_ast_contract::grammar::parse_document(source)
        .expect("generated parse_document helper should return an AdzeDocument");
    let root = document.tree().root();

    let syntax = adze_example::typed_ast_contract::grammar::syntax::source_file(&document)
        .expect("generated source_file wrapper should cast the generic root");
    assert_same_node(syntax, root);

    let add_node = find_node(root, "Expr_Add", source)
        .expect("generic CST should contain the root addition expression");
    let add = adze_example::typed_ast_contract::grammar::syntax::ExprAdd::cast(
        &document,
        add_node.node_id(),
    )
    .expect("generated Expr_Add wrapper should cast the matching generic node");
    assert_same_node(add, add_node);

    let number_node =
        find_node(root, "/\\d+/", "1").expect("generic CST should contain a number token");
    let number = adze_example::typed_ast_contract::grammar::syntax::DToken::cast(
        &document,
        number_node.node_id(),
    )
    .expect("generated number token wrapper should cast the matching generic node");
    assert_same_node(number, number_node);
}

#[test]
fn generated_typed_cst_wrappers_share_document_point_ranges() {
    let source = "1+\n2+3";
    let document = adze_example::typed_ast_contract::grammar::parse_document(source)
        .expect("generated parse_document helper should return a multiline AdzeDocument");
    let root = document.tree().root();
    let syntax = adze_example::typed_ast_contract::grammar::syntax::source_file(&document)
        .expect("generated source_file wrapper should cast the generic root");

    assert_eq!(syntax.point_range(), Some(root.point_range()));
    assert_eq!(root.point_range().start.row, 0);
    assert_eq!(root.point_range().start.column, 0);
    assert_eq!(root.point_range().end.row, 1);
    assert_eq!(root.point_range().end.column, 3);

    let second_number =
        find_node(root, "/\\d+/", "2").expect("generic CST should contain the second-line token");
    let wrapper = adze_example::typed_ast_contract::grammar::syntax::DToken::cast(
        &document,
        second_number.node_id(),
    )
    .expect("generated number token wrapper should cast the matching generic node");
    let point_range = wrapper
        .point_range()
        .expect("typed CST wrapper should expose the generic node point range");

    assert_same_node(wrapper, second_number);
    assert_eq!(point_range, second_number.point_range());
    assert_eq!(point_range.start.row, 1);
    assert_eq!(point_range.start.column, 0);
    assert_eq!(point_range.end.row, 1);
    assert_eq!(point_range.end.column, 1);
}

#[test]
fn generated_typed_cst_field_accessors_project_native_edges() {
    let source = "123+";
    let document = adze_example::fielded_typed_cst_contract::grammar::parse_document(source)
        .expect("generated parse_document helper should return an AdzeDocument");
    let root = document.tree().root();
    let pair_node =
        find_node(root, "Pair", source).expect("generic CST should contain the pair node");
    let pair = adze_example::fielded_typed_cst_contract::grammar::syntax::Pair::cast(
        &document,
        pair_node.node_id(),
    )
    .expect("generated Pair wrapper should cast the matching generic node");

    let generic_left = pair_node
        .edge_by_field_name("left")
        .and_then(|edge| edge.child())
        .expect("generic CST should expose the left field edge");
    let generic_right = pair_node
        .edge_by_field_name("right")
        .and_then(|edge| edge.child())
        .expect("generic CST should expose the right field edge");

    let left = pair
        .left()
        .expect("generated Pair wrapper should expose the left field");
    let right = pair
        .right()
        .expect("generated Pair wrapper should expose the right field");

    assert_same_node(left, generic_left);
    assert_same_node(right, generic_right);
    assert_eq!(left.text(), Some("123"));
    assert_eq!(right.text(), Some("+"));
}

#[test]
fn generated_parse_document_bridge_populates_direct_node_edge_records() {
    let source = "123+";
    let document = adze_example::fielded_typed_cst_contract::grammar::parse_document(source)
        .expect("generated parse_document helper should return an AdzeDocument");
    let tree = document.tree();
    let root = tree.root();
    let pair = find_node(root, "Pair", source).expect("generic CST should contain the pair node");
    let pair_record = pair.record();

    assert_eq!(tree.node_record(pair.node_id()), Some(pair_record));
    assert_eq!(pair_record.visible_id(), pair.kind_id());
    assert_eq!(pair_record.grammar_id(), pair.grammar_id());
    assert_eq!(pair_record.byte_range(), pair.byte_range());
    assert_eq!(pair_record.point_range(), pair.point_range());
    assert_eq!(pair_record.edge_range().len(), pair.child_count());
    assert_eq!(pair_record.flags(), pair.flags());
    assert_eq!(
        tree.edge_count(),
        (0..tree.node_count())
            .filter_map(|index| tree.node(adze::document::NodeId::new(index)))
            .map(|node| node.child_count())
            .sum::<usize>(),
        "direct edge records should cover every generated parse_document edge"
    );

    let left_edge = pair
        .edge_by_field_name("left")
        .expect("generic CST should expose direct left edge metadata");
    let left = left_edge
        .child()
        .expect("direct left edge record should resolve its child");
    let left_record = left_edge.record();
    let left_field_id = document
        .language()
        .field_id_for_name("left")
        .expect("generated language metadata should expose left field id");

    assert_eq!(left_edge.field_id(), Some(left_field_id));
    assert_eq!(left_record.parent_id(), pair.node_id());
    assert_eq!(left_record.child_id(), left.node_id());
    assert_eq!(left_record.child_index(), 0);
    assert_eq!(left_record.field_id(), Some(left_field_id));
    assert_eq!(
        left.parent_edge()
            .expect("left node should resolve direct parent edge")
            .record(),
        left_record
    );
    assert_eq!(left.field_name(), Some("left"));
    assert_eq!(left.field_id(), Some(left_field_id));
    assert_eq!(left.record().byte_range(), left.byte_range());
    assert_eq!(left.utf8_text().ok(), Some("123"));
}

#[test]
fn generated_typed_cst_field_accessors_survive_precedence_enum_variants() {
    use adze_example::fielded_precedence_typed_cst_contract::grammar::{self, Expr};

    let source = "1+2*3";
    let parsed = grammar::parse(source).expect("fielded precedence grammar should parse");
    assert_eq!(
        parsed,
        Expr::Add {
            left: Box::new(Expr::Number(1)),
            operator: (),
            right: Box::new(Expr::Mul {
                left: Box::new(Expr::Number(2)),
                operator: (),
                right: Box::new(Expr::Number(3)),
            }),
        }
    );

    let document = grammar::parse_document(source)
        .expect("generated parse_document helper should return an AdzeDocument");
    let root = document.tree().root();
    let add_node =
        find_node(root, "Expr_Add", source).expect("generic CST should contain the add node");
    let add = grammar::syntax::ExprAdd::cast(&document, add_node.node_id())
        .expect("generated Expr_Add wrapper should cast the matching generic node");

    let generic_left = add_node
        .edge_by_field_name("left")
        .and_then(|edge| edge.child())
        .expect("generic CST should expose the left field edge");
    let generic_operator = add_node
        .edge_by_field_name("operator")
        .and_then(|edge| edge.child())
        .expect("generic CST should expose the operator field edge");
    let generic_right = add_node
        .edge_by_field_name("right")
        .and_then(|edge| edge.child())
        .expect("generic CST should expose the right field edge");

    let left = add
        .left()
        .expect("generated Expr_Add wrapper should expose the left field");
    let operator = add
        .operator()
        .unwrap_or_else(|| {
            panic!(
                "generated Expr_Add wrapper should expose the operator field; generic edge child kind was {:?}",
                generic_operator.kind_name()
            )
        });
    let right = add
        .right()
        .expect("generated Expr_Add wrapper should expose the right field");

    assert_same_node(left, generic_left);
    assert_same_node(operator, generic_operator);
    assert_same_node(right, generic_right);
    assert_eq!(left.text(), Some("1"));
    assert_eq!(operator.text(), Some("+"));
    assert_eq!(right.text(), Some("2*3"));
}

#[test]
fn generated_typed_cst_wrapper_extracts_typed_ast_from_its_node() {
    use adze::document::Provenance;
    use adze_example::fielded_precedence_typed_cst_contract::grammar::{self, Expr};

    let source = "1+2*3";
    let expected = grammar::parse(source).expect("fielded precedence grammar should parse");
    let document = grammar::parse_document(source)
        .expect("generated parse_document helper should return an AdzeDocument");
    let root = document.tree().root();
    let add_node =
        find_node(root, "Expr_Add", source).expect("generic CST should contain the add node");
    let add = grammar::syntax::ExprAdd::cast(&document, add_node.node_id())
        .expect("generated Expr_Add wrapper should cast the matching generic node");

    let typed_ast = add
        .ast::<Expr>()
        .expect("typed CST wrapper should extract typed AST from its own node");

    assert_eq!(typed_ast.value(), &expected);
    assert_eq!(typed_ast.provenance(), &Provenance::Node(add.node_id()));
}

fn assert_same_node<'doc>(wrapper: impl SyntaxNode<'doc>, node: adze::document::AdzeNode<'doc>) {
    assert_eq!(wrapper.node_id(), node.node_id());
    assert_eq!(wrapper.kind_name(), node.kind_name());
    assert_eq!(wrapper.byte_range(), Some(node.byte_range()));
    assert_eq!(wrapper.point_range(), Some(node.point_range()));
    assert_eq!(wrapper.text(), node.utf8_text().ok());
}

fn find_node<'doc>(
    node: adze::document::AdzeNode<'doc>,
    kind: &str,
    text: &str,
) -> Option<adze::document::AdzeNode<'doc>> {
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

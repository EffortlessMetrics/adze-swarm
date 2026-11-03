//! Independent source facts for native non-RHS attachments (#1191).
use adze::document::{NodeId, ParseOutcome};
use adze::pure_parser::{ParsedNode, Parser, Point};
use adze_example::typed_ast_contract::grammar as numbers;

fn walk<'a>(node: &'a ParsedNode, out: &mut Vec<&'a ParsedNode>) {
    out.push(node);
    for child in node.children() {
        assert!(node.start_byte() <= child.start_byte());
        assert!(child.end_byte() <= node.end_byte());
        walk(child, out);
    }
    for pair in node.children().windows(2) {
        assert!(
            pair[0].end_byte() <= pair[1].start_byte(),
            "children follow source order"
        );
    }
}

#[test]
fn test_trailing_extras_preserve_exact_native_and_document_numeric_lexemes() {
    for (source, span, start, end, root_end, extra_spans) in [
        (
            "  17  ",
            2..4,
            Point { row: 0, column: 2 },
            Point { row: 0, column: 4 },
            Point { row: 0, column: 6 },
            vec![0..2, 4..6],
        ),
        (
            "\n17\n",
            1..3,
            Point { row: 1, column: 0 },
            Point { row: 1, column: 2 },
            Point { row: 2, column: 0 },
            vec![0..1, 3..4],
        ),
    ] {
        let mut parser = Parser::new();
        parser.set_language(numbers::language()).unwrap();
        let parsed = parser.parse_string(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let root = parsed.root.unwrap();
        assert_eq!(root.kind(), "Expr");
        assert_eq!(root.start_byte()..root.end_byte(), 0..source.len());
        assert_eq!(root.start_point(), Point { row: 0, column: 0 });
        assert_eq!(root.end_point(), root_end);
        let mut nodes = vec![];
        walk(&root, &mut nodes);
        let leaf = nodes.iter().find(|node| node.kind() == r"/\d+/").unwrap();
        assert_eq!(leaf.start_byte()..leaf.end_byte(), span);
        assert_eq!(leaf.start_point(), start);
        assert_eq!(leaf.end_point(), end);
        assert_eq!(leaf.utf8_text(source.as_bytes()).unwrap(), "17");
        assert!(leaf.children().is_empty());
        let mut extras: Vec<_> = nodes
            .iter()
            .filter(|node| node.is_extra())
            .map(|node| {
                assert!(
                    !node.is_named(),
                    "declared anonymous whitespace retains its ABI named flag"
                );
                assert!(node.field_id.is_none());
                assert!(!node.is_error() && !node.is_missing());
                node.start_byte()..node.end_byte()
            })
            .collect();
        extras.sort_by_key(|range| range.start);
        assert_eq!(extras, extra_spans);
        let document = numbers::parse_document(source).unwrap();
        assert_eq!(document.outcome(), ParseOutcome::Complete);
        assert!(document.diagnostics().is_empty());
        assert_eq!(document.tree().root().byte_range(), 0..source.len());
        let leaves: Vec<_> = (0..document.tree().node_count())
            .filter_map(|index| {
                let node = document.tree().node(NodeId::new(index)).unwrap();
                (node.grammar_name() == Some(r"/\d+/")).then_some(node)
            })
            .collect();
        assert_eq!(leaves.len(), 1);
        assert_eq!(leaves[0].byte_range(), span);
        assert_eq!(leaves[0].utf8_text().unwrap(), "17");
        assert_eq!(leaves[0].child_count(), 0);
        assert_eq!(numbers::parse(source).unwrap(), numbers::Expr::Number(17));
        assert_eq!(
            document.ast::<numbers::Expr>().unwrap(),
            numbers::Expr::Number(17)
        );
    }
}

#[test]
fn test_interleaved_extras_do_not_shift_fielded_semantic_children() {
    use adze_example::fielded_precedence_typed_cst_contract::grammar as fielded;
    let source = " 1 + 2 * 3 ";
    let mut parser = Parser::new();
    parser.set_language(fielded::language()).unwrap();
    let result = parser.parse_string(source);
    assert!(result.errors.is_empty());
    let root = result.root.unwrap();
    let mut nodes = vec![];
    walk(&root, &mut nodes);
    let lexical: Vec<_> = nodes
        .iter()
        .filter(|node| node.kind() == r"/\d+/")
        .map(|node| {
            assert!(node.children().is_empty());
            node.start_byte()..node.end_byte()
        })
        .collect();
    assert_eq!(lexical, vec![1..2, 5..6, 9..10]);
    let operators: Vec<_> = nodes
        .iter()
        .filter(|node| ["\"+\"", "\"*\""].contains(&node.kind()))
        .map(|node| {
            assert!(node.children().is_empty());
            node.start_byte()..node.end_byte()
        })
        .collect();
    assert_eq!(operators, vec![3..4, 7..8]);
    let document = fielded::parse_document(source).unwrap();
    assert_eq!(document.outcome(), ParseOutcome::Complete);
    assert!(document.diagnostics().is_empty());
    let mut operators = vec![];
    for index in 0..document.tree().node_count() {
        let node = document.tree().node(NodeId::new(index)).unwrap();
        if node.is_extra() {
            assert!(node.field_name().is_none());
        }
        if let Some(operator) = node.child_by_field_name("operator") {
            operators.push(operator.byte_range());
            assert!(node.child_by_field_name("left").is_some());
            assert!(node.child_by_field_name("right").is_some());
        }
    }
    assert_eq!(operators, vec![3..4, 7..8]);
    let expected = fielded::Expr::Add {
        left: Box::new(fielded::Expr::Number(1)),
        operator: (),
        right: Box::new(fielded::Expr::Mul {
            left: Box::new(fielded::Expr::Number(2)),
            operator: (),
            right: Box::new(fielded::Expr::Number(3)),
        }),
    };
    assert_eq!(fielded::parse(source).unwrap(), expected);
    assert_eq!(document.ast::<fielded::Expr>().unwrap(), expected);
}

#[test]
fn test_utf8_eof_and_nul_comments_preserve_exact_numeric_and_extra_ranges() {
    use adze_example::native_extra_provenance_1191::grammar as comments;
    for (source, number_span, comment_spans, start, end, root_end) in [
        (
            "#λ\n17 #β\n",
            4..6,
            vec![0..3, 7..10],
            Point { row: 1, column: 0 },
            Point { row: 1, column: 2 },
            Point { row: 2, column: 0 },
        ),
        (
            "17#尾",
            0..2,
            vec![2..6],
            Point { row: 0, column: 0 },
            Point { row: 0, column: 2 },
            Point { row: 0, column: 6 },
        ),
        (
            "17# a\0b",
            0..2,
            vec![2..7],
            Point { row: 0, column: 0 },
            Point { row: 0, column: 2 },
            Point { row: 0, column: 7 },
        ),
    ] {
        let mut parser = Parser::new();
        parser.set_language(comments::language()).unwrap();
        let parsed = parser.parse_string(source);
        assert!(parsed.errors.is_empty());
        let root = parsed.root.unwrap();
        assert_eq!(root.kind(), "Number");
        assert_eq!(root.start_byte()..root.end_byte(), 0..source.len());
        assert_eq!(root.end_point(), root_end);
        let mut nodes = vec![];
        walk(&root, &mut nodes);
        let leaf = nodes.iter().find(|node| node.kind() == r"_/\d+/").unwrap();
        assert_eq!(leaf.start_byte()..leaf.end_byte(), number_span);
        assert_eq!(leaf.start_point(), start);
        assert_eq!(leaf.end_point(), end);
        assert!(leaf.children().is_empty());
        let actual: Vec<_> = nodes
            .iter()
            .filter(|node| {
                node.is_extra() && node.utf8_text(source.as_bytes()).unwrap().starts_with('#')
            })
            .map(|node| {
                assert!(node.field_id.is_none());
                node.start_byte()..node.end_byte()
            })
            .collect();
        assert_eq!(actual, comment_spans);
        let document = comments::parse_document(source).unwrap();
        assert_eq!(document.outcome(), ParseOutcome::Complete);
        assert!(document.diagnostics().is_empty());
        assert_eq!(
            comments::parse(source).unwrap(),
            comments::Number { value: 17 }
        );
        assert_eq!(
            document.ast::<comments::Number>().unwrap(),
            comments::Number { value: 17 }
        );
    }
}

#[test]
fn test_whitespace_adjacent_nullable_occurrence_keeps_consumed_boundary_and_no_children() {
    use adze_example::repetitions::grammar3;
    for (source, end_point) in [
        ("1, ", Point { row: 0, column: 3 }),
        ("1,\n", Point { row: 1, column: 0 }),
    ] {
        let value = grammar3::parse(source).unwrap();
        assert_eq!(value.numbers.value.len(), 2);
        assert_eq!(value.numbers.value[0].value, Some(1));
        assert_eq!(value.numbers.value[0].span, (0, 1));
        assert_eq!(value.numbers.value[1].value, None);
        assert_eq!(value.numbers.value[1].span, (3, 3));
        let mut parser = Parser::new();
        parser.set_language(grammar3::language()).unwrap();
        let parsed = parser.parse_string(source);
        assert!(parsed.errors.is_empty());
        let root = parsed.root.unwrap();
        let mut nodes = vec![];
        walk(&root, &mut nodes);
        let blank: Vec<_> = nodes
            .iter()
            .filter(|node| {
                !node.is_named()
                    && node.start_byte() == 3
                    && node.end_byte() == 3
                    && node.field_id.is_some()
            })
            .collect();
        assert_eq!(blank.len(), 1);
        assert!(blank[0].children().is_empty());
        assert_eq!(blank[0].start_point(), end_point);
        assert_eq!(blank[0].end_point(), end_point);
    }
}

#[test]
fn test_recovery_attachments_retain_diagnostics_and_parser_reuse_clears_pending_nodes() {
    let mut parser = Parser::new();
    parser.set_language(numbers::language()).unwrap();
    for source in ["17x ", " x17 "] {
        let parsed = parser.parse_string(source);
        assert_eq!(parsed.errors.len(), 1);
        let error_start = source.find('x').unwrap();
        assert_eq!(parsed.errors[0].position, error_start);
        let root = parsed.root.unwrap();
        let mut nodes = vec![];
        walk(&root, &mut nodes);
        let error: Vec<_> = nodes.iter().filter(|node| node.is_error()).collect();
        assert_eq!(error.len(), 1);
        assert_eq!(
            error[0].start_byte()..error[0].end_byte(),
            error_start..error_start + 1
        );
        assert!(error[0].children().is_empty());
        let leaf = nodes.iter().find(|node| node.kind() == r"/\d+/").unwrap();
        assert_eq!(leaf.utf8_text(source.as_bytes()).unwrap(), "17");
        assert!(leaf.children().is_empty());
        let document = numbers::parse_document(source).unwrap();
        assert_ne!(document.outcome(), ParseOutcome::Complete);
        assert_eq!(document.diagnostics().len(), 1);
        assert_eq!(
            document.diagnostics()[0].byte_span(),
            error_start..error_start + 1
        );
        assert_eq!(document.diagnostics()[0].found.as_deref(), Some("x"));
        assert!(document.ast::<numbers::Expr>().is_err());
        assert!(numbers::parse(source).is_err());
    }
    let clean = parser.parse_string("17");
    assert!(clean.errors.is_empty());
    let root = clean.root.unwrap();
    let mut nodes = vec![];
    walk(&root, &mut nodes);
    assert_eq!(root.start_byte()..root.end_byte(), 0..2);
    assert!(
        nodes
            .iter()
            .all(|node| !node.is_extra() && !node.is_error())
    );
    let overflow = numbers::parse("2147483648 ").unwrap_err();
    assert_eq!(overflow.len(), 1);
    assert_eq!(overflow[0].byte_span(), 0..10);
    assert!(matches!(
        overflow[0].reason,
        adze::errors::ParseErrorReason::ConversionFailed { .. }
    ));
}

fn repeat_with_interleaved_extras() -> ParsedNode {
    let first = ParsedNode::builder(1, vec![], 0, 1).build();
    let base = ParsedNode::builder(10, vec![first], 0, 1).build();
    let gap = ParsedNode::builder(2, vec![], 1, 2).is_extra(true).build();
    let second = ParsedNode::builder(1, vec![], 2, 3).build();
    let recursive = ParsedNode::builder(10, vec![base, gap, second], 0, 3).build();
    let gap = ParsedNode::builder(2, vec![], 3, 4).is_extra(true).build();
    let third = ParsedNode::builder(1, vec![], 4, 5).build();
    ParsedNode::builder(10, vec![recursive, gap, third], 0, 5).build()
}

#[test]
fn test_fallible_repeat_binary_shape_ignores_direct_extra_children() {
    use adze::Extract;
    let root = repeat_with_interleaved_extras();
    let leaf = |text: &str| text.parse::<i32>().map_err(|error| error.to_string());
    let values = <Vec<adze::WithTryLeaf<i32>> as Extract<Vec<i32>>>::try_extract(
        Some(&root),
        b"1 2 3",
        0,
        Some(&leaf),
    )
    .unwrap();
    assert_eq!(values, vec![1, 2, 3]);
}

#[test]
fn test_infallible_repeat_binary_shape_ignores_direct_extra_children() {
    use adze::Extract;
    let root = repeat_with_interleaved_extras();
    let leaf = |text: &str| text.parse::<i32>().map_err(|error| error.to_string());
    let values = <Vec<adze::WithTryLeaf<i32>> as Extract<Vec<i32>>>::extract(
        Some(&root),
        b"1 2 3",
        0,
        Some(&leaf),
    );
    assert_eq!(values, vec![1, 2, 3]);
}

fn tuple_with_interleaved_extras() -> ParsedNode {
    let gap = |start, end| {
        ParsedNode::builder(2, vec![], start, end)
            .is_extra(true)
            .build()
    };
    let left = ParsedNode::builder(1, vec![], 1, 2).build();
    let operator = ParsedNode::builder(3, vec![], 3, 4).build();
    let right = ParsedNode::builder(1, vec![], 5, 6).build();
    ParsedNode::builder(
        10,
        vec![
            gap(0, 1),
            left,
            gap(2, 3),
            operator,
            gap(4, 5),
            right,
            gap(6, 7),
        ],
        0,
        7,
    )
    .build()
}

#[test]
fn test_fallible_tuple_slots_ignore_prefix_and_interleaved_extras() {
    use adze::{
        __private::{extract_field_try, extract_struct_or_variant_try},
        WithTryLeaf,
    };
    let root = tuple_with_interleaved_extras();
    let leaf = |text: &str| text.parse::<i32>().map_err(|error| error.to_string());
    let value = extract_struct_or_variant_try(&root, |cursor, last_idx| {
        let left = extract_field_try::<WithTryLeaf<i32>, i32>(
            cursor,
            b" 1 + 2 ",
            last_idx,
            "0",
            Some(&leaf),
        )?;
        extract_field_try::<(), ()>(cursor, b" 1 + 2 ", last_idx, "1", None)?;
        let right = extract_field_try::<WithTryLeaf<i32>, i32>(
            cursor,
            b" 1 + 2 ",
            last_idx,
            "2",
            Some(&leaf),
        )?;
        Ok((left, right))
    })
    .unwrap();
    assert_eq!(value, (1, 2));
}

#[test]
fn test_infallible_tuple_slots_ignore_prefix_and_interleaved_extras() {
    use adze::{
        __private::{extract_field, extract_struct_or_variant},
        WithLeaf,
    };
    let root = tuple_with_interleaved_extras();
    let leaf = |text: &str| text.parse::<i32>().unwrap();
    let value = extract_struct_or_variant(&root, |cursor, last_idx| {
        let left =
            extract_field::<WithLeaf<i32>, i32>(cursor, b" 1 + 2 ", last_idx, "0", Some(&leaf));
        extract_field::<(), ()>(cursor, b" 1 + 2 ", last_idx, "1", None);
        let right =
            extract_field::<WithLeaf<i32>, i32>(cursor, b" 1 + 2 ", last_idx, "2", Some(&leaf));
        (left, right)
    });
    assert_eq!(value, (1, 2));
}

//! Generated lexical failures cannot masquerade as physical EOF (#1174).

#![cfg(feature = "pure-rust")]

use adze::document::ParseOutcome;
use adze_example::typed_ast_contract::grammar as deterministic;

fn assert_native_containment(node: &adze::pure_parser::ParsedNode, source_len: usize) {
    assert!(node.start_byte() <= node.end_byte());
    assert!(node.end_byte() <= source_len);
    for child in node.children() {
        assert!(
            node.start_byte() <= child.start_byte() && child.end_byte() <= node.end_byte(),
            "child {:?} {}..{} outside parent {:?} {}..{}",
            child.kind(),
            child.start_byte(),
            child.end_byte(),
            node.kind(),
            node.start_byte(),
            node.end_byte(),
        );
        assert!(
            (node.start_point().row, node.start_point().column)
                <= (child.start_point().row, child.start_point().column)
        );
        assert!(
            (child.end_point().row, child.end_point().column)
                <= (node.end_point().row, node.end_point().column)
        );
        assert_native_containment(child, source_len);
    }
}

fn assert_document_containment(document: &adze::document::AdzeDocument, source_len: usize) {
    for index in 0..document.tree().node_count() {
        let node = document
            .tree()
            .node(adze::document::NodeId::new(index))
            .unwrap();
        assert!(node.start_byte() <= node.end_byte());
        assert!(node.end_byte() <= source_len);
        if let Some(parent) = node.parent() {
            assert!(parent.start_byte() <= node.start_byte());
            assert!(node.end_byte() <= parent.end_byte());
        }
    }
}

fn assert_number_leaf_span(
    root: &adze::pure_parser::ParsedNode,
    document: &adze::document::AdzeDocument,
    expected: std::ops::Range<usize>,
) {
    let mut pending = vec![root];
    let mut native_spans = Vec::new();
    while let Some(node) = pending.pop() {
        if node.kind() == r"/\d+/" {
            native_spans.push(node.start_byte()..node.end_byte());
        }
        pending.extend(node.children());
    }
    assert_eq!(native_spans, vec![expected.clone()]);
    let document_spans: Vec<_> = (0..document.tree().node_count())
        .map(|index| {
            document
                .tree()
                .node(adze::document::NodeId::new(index))
                .unwrap()
        })
        .filter(|node| node.grammar_name() == Some(r"/\d+/"))
        .map(|node| node.byte_range())
        .collect();
    assert_eq!(document_spans, vec![expected]);
}

#[test]
fn test_leading_extras_are_contained_without_widening_semantic_leaves() {
    use adze::pure_parser::{Parser, Point};
    for (source, end_point) in [
        ("  17", Point { row: 0, column: 4 }),
        ("\n 17", Point { row: 1, column: 3 }),
    ] {
        let mut parser = Parser::new();
        parser.set_language(deterministic::language()).unwrap();
        let parsed = parser.parse_string(source);
        assert!(parsed.errors.is_empty(), "{source:?}");
        let root = parsed.root.unwrap();
        let document = deterministic::parse_document(source).unwrap();
        assert_eq!(document.outcome(), ParseOutcome::Complete);
        assert!(document.diagnostics().is_empty());
        assert_eq!(
            deterministic::parse(source).unwrap(),
            deterministic::Expr::Number(17)
        );
        assert_eq!(
            document.ast::<deterministic::Expr>().unwrap(),
            deterministic::Expr::Number(17)
        );
        assert_number_leaf_span(&root, &document, 2..4);
        assert!(
            root.children()
                .iter()
                .any(|node| node.is_extra() && node.start_byte() == 0)
        );
        assert_eq!(root.start_byte()..root.end_byte(), 0..source.len());
        assert_eq!(root.start_point(), Point { row: 0, column: 0 });
        assert_eq!(root.end_point(), end_point);
        assert_eq!(document.tree().root().byte_range(), 0..source.len());
        assert_native_containment(&root, source.len());
        assert_document_containment(&document, source.len());
    }
}

#[test]
fn test_leading_and_trailing_extras_remain_inside_native_and_document_parents() {
    use adze::pure_parser::{Parser, Point};
    for (source, end_point) in [
        ("  17  ", Point { row: 0, column: 6 }),
        ("\n17\n", Point { row: 2, column: 0 }),
    ] {
        let mut parser = Parser::new();
        parser.set_language(deterministic::language()).unwrap();
        let parsed = parser.parse_string(source);
        assert!(parsed.errors.is_empty());
        let root = parsed.root.unwrap();
        let document = deterministic::parse_document(source).unwrap();
        assert_eq!(document.outcome(), ParseOutcome::Complete);
        assert!(document.diagnostics().is_empty());
        assert_eq!(
            document.ast::<deterministic::Expr>().unwrap(),
            deterministic::Expr::Number(17)
        );
        assert_eq!(root.start_byte()..root.end_byte(), 0..source.len());
        assert_eq!(root.start_point(), Point { row: 0, column: 0 });
        assert_eq!(root.end_point(), end_point);
        assert_eq!(document.tree().root().byte_range(), 0..source.len());
        assert_native_containment(&root, source.len());
        assert_document_containment(&document, source.len());
        // Exact trailing-extra placement and leaf spans remain a separate
        // #898 defect; this control establishes parent containment only.
    }
}

#[test]
fn test_leading_errors_are_contained_with_exact_diagnostics_and_leaf_spans() {
    use adze::pure_parser::{Parser, Point};
    for (source, error_span, found, number_span, end_point) in [
        ("x17", 0..1, "x", 1..3, Point { row: 0, column: 3 }),
        ("\nλ17", 1..3, "λ", 3..5, Point { row: 1, column: 4 }),
    ] {
        let mut parser = Parser::new();
        parser.set_language(deterministic::language()).unwrap();
        let parsed = parser.parse_string(source);
        assert_eq!(parsed.errors.len(), 1, "{source:?}");
        assert_eq!(parsed.errors[0].position, error_span.start);
        let root = parsed.root.expect("retain the selected recovery tree");
        let document = deterministic::parse_document(source).unwrap();
        assert_ne!(document.outcome(), ParseOutcome::Complete);
        assert_eq!(document.diagnostics().len(), 1);
        assert_eq!(document.diagnostics()[0].byte_span(), error_span);
        assert_eq!(document.diagnostics()[0].found.as_deref(), Some(found));
        assert!(deterministic::parse(source).is_err());
        assert!(document.ast::<deterministic::Expr>().is_err());
        assert_number_leaf_span(&root, &document, number_span);
        assert!(root.has_error());
        assert_eq!(root.start_byte()..root.end_byte(), 0..source.len());
        assert_eq!(root.start_point(), Point { row: 0, column: 0 });
        assert_eq!(root.end_point(), end_point);
        assert_eq!(document.tree().root().byte_range(), 0..source.len());
        assert_native_containment(&root, source.len());
        assert_document_containment(&document, source.len());
    }
}

#[test]
fn test_deterministic_complete_input_retains_exact_values_and_true_eof() {
    assert_eq!(
        deterministic::parse("17").unwrap(),
        deterministic::Expr::Number(17)
    );
    assert_eq!(
        deterministic::parse("17+2").unwrap(),
        deterministic::Expr::Add(
            Box::new(deterministic::Expr::Number(17)),
            (),
            Box::new(deterministic::Expr::Number(2)),
        )
    );
    let document = deterministic::parse_document("17").unwrap();
    assert_eq!(document.outcome(), ParseOutcome::Complete);
    assert_eq!(document.tree().root().byte_range(), 0..2);
    assert!(document.diagnostics().is_empty());
    let errors = deterministic::parse("17+").expect_err("actual EOF is still incomplete");
    assert_eq!(errors[0].byte_span(), 3..3);
    assert!(matches!(
        errors[0].reason,
        adze::errors::ParseErrorReason::UnexpectedEof
    ));
}

#[test]
fn test_deterministic_unknown_suffix_has_source_bounded_failure_facts() {
    for (source, span, found) in [
        ("17x", 2..3, "x"),
        ("17λ", 2..4, "λ"),
        ("17\0", 2..3, "\0"),
        ("17 x", 3..4, "x"),
    ] {
        let errors =
            deterministic::parse(source).expect_err("unknown suffix cannot accept a prefix");
        assert_eq!(errors[0].byte_span(), span, "{source:?}");
        assert!(matches!(
            errors[0].reason,
            adze::errors::ParseErrorReason::UnexpectedToken(_)
        ));
        let document =
            deterministic::parse_document(source).expect("retain invalid-input document");
        assert_ne!(document.outcome(), ParseOutcome::Complete, "{source:?}");
        assert_eq!(
            document.diagnostics().len(),
            1,
            "one invalid scalar: {source:?}"
        );
        let diagnostic = &document.diagnostics()[0];
        assert_eq!(diagnostic.byte_span(), span, "{source:?}");
        assert_eq!(diagnostic.found.as_deref(), Some(found), "{source:?}");
        assert_eq!(diagnostic.expected, errors[0].expected, "{source:?}");
        assert!(document.tree().has_errors());
        let node = diagnostic
            .related_nodes
            .iter()
            .find_map(|id| {
                document
                    .tree()
                    .node(*id)
                    .filter(|node| node.is_error() && node.byte_range() == span)
            })
            .expect("diagnostic must retain the offending error node, not missing EOF");
        assert!(!node.is_missing(), "non-EOF source is not a missing token");
        assert!(document.ast::<deterministic::Expr>().is_err());
        assert_eq!(
            deterministic::parse("17").unwrap(),
            deterministic::Expr::Number(17)
        );
    }
}

#[cfg(feature = "glr")]
#[test]
fn test_streaming_unknown_suffix_uses_native_failure_without_conversion() {
    use adze::__private::{EngineDispatch, engine_dispatch_for_language};
    use adze_example::native_failure_facts_1030::{
        NATIVE_FAILURE_FACTS_1030_CALLS as calls, grammar as conflicted,
    };
    use std::sync::atomic::Ordering;
    assert_eq!(
        engine_dispatch_for_language(conflicted::language()),
        EngineDispatch::StreamingGlr
    );
    assert_eq!(
        conflicted::parse("17").unwrap(),
        conflicted::Expr::Number(17)
    );
    for (source, span, found) in [("17x", 2..3, "x"), ("17 λ", 3..5, "λ")] {
        calls.store(0, Ordering::SeqCst);
        let errors = conflicted::parse(source).expect_err("streaming route must reject suffix");
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "syntax failure cannot invoke conversion"
        );
        assert_eq!(errors[0].byte_span(), span);
        let document = conflicted::parse_document(source).expect("native failure document");
        assert_ne!(document.outcome(), ParseOutcome::Complete);
        assert_eq!(document.diagnostics()[0].byte_span(), span);
        assert_eq!(document.diagnostics()[0].found.as_deref(), Some(found));
        assert_eq!(document.diagnostics()[0].expected, errors[0].expected);
        assert!(document.ast::<conflicted::Expr>().is_err());
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "no observer reparse or converter"
        );
    }
}

#[test]
fn test_deterministic_reduction_lookahead_preserves_complete_repetitions() {
    use adze_example::repetitions::grammar2;
    for (source, expected) in [
        ("1", vec![(1, (0, 1))]),
        ("1 2", vec![(1, (0, 1)), (2, (2, 3))]),
        ("1 2 3", vec![(1, (0, 1)), (2, (2, 3)), (3, (4, 5))]),
    ] {
        let parsed = grammar2::parse(source).expect("valid numbers after reductions");
        let values: Vec<_> = parsed
            .numbers
            .value
            .iter()
            .map(|item| (item.value, item.span))
            .collect();
        assert_eq!(values, expected, "{source}");
        let document = grammar2::parse_document(source).unwrap();
        assert_eq!(document.outcome(), ParseOutcome::Complete);
        let via_document: grammar2::NumberList = document.ast().unwrap();
        let values: Vec<_> = via_document
            .numbers
            .value
            .iter()
            .map(|item| (item.value, item.span))
            .collect();
        assert_eq!(values, expected, "document: {source}");
    }
    for source in ["abc", "1x", "1, 2"] {
        assert!(
            grammar2::parse(source).is_err(),
            "invalid suffix cannot become a prefix: {source}"
        );
    }
    // Keep conversion coverage on a declared fallible numeric leaf.
    let errors =
        deterministic::parse("1+2147483648").expect_err("second number still converts fallibly");
    assert!(matches!(
        errors[0].reason,
        adze::errors::ParseErrorReason::ConversionFailed(_)
    ));
    assert_eq!(errors[0].byte_span(), 2..12);
}

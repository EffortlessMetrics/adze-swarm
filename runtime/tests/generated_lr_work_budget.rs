//! Productive large-input regression for #1211, with an independent value/span oracle.
//!
//! The issue publishes this exact generator and the old 263/264 boundary.
//! Its earlier raw receipt/archive is unavailable in this workspace; this
//! target reconstructs the published input openly and makes no timing claim.
//! It qualifies the selected deterministic profile, not conflicted GLR parsing.

#![cfg(all(feature = "pure-rust", not(feature = "glr")))]

use adze::__private::{EngineDispatch, engine_dispatch_for_language};
use adze::document::{AdzeDocument, NodeId, ParseOutcome};
use adze::pure_parser::{ParsedNode, Parser, Point};
use adze_python_simple::grammar::{self, Expression, Module, PrimaryExpression, Statement};
use std::ops::Range;

struct ExpectedToken {
    span: Range<usize>,
    text: String,
    row: u32,
    column: u32,
}

fn assignment_tokens(count: usize) -> Vec<ExpectedToken> {
    let mut expected = Vec::new();
    let mut offset = 0;
    for index in 0..count {
        // Independent lexical layout: seven one-space-separated tokens and
        // one trailing newline. No parser tree or generated table is consulted.
        let mut column = 0;
        for text in [
            format!("var{index}"),
            "=".to_string(),
            index.to_string(),
            "*".to_string(),
            "2".to_string(),
            "+".to_string(),
            (index + 1).to_string(),
        ] {
            let width = text.len();
            expected.push(ExpectedToken {
                span: offset + column..offset + column + width,
                text,
                row: index as u32,
                column: column as u32,
            });
            column += width + 1;
        }
        offset += column;
    }
    expected
}

fn assert_native_tokens(root: &ParsedNode, source: &str, expected: &[ExpectedToken]) {
    assert_eq!(root.start_byte()..root.end_byte(), 0..source.len());
    assert!(!root.has_error());
    let mut pending = vec![root];
    let mut actual = Vec::new();
    while let Some(node) = pending.pop() {
        if u32::from(node.symbol) < grammar::language().token_count && !node.is_extra() {
            assert_eq!(node.child_count(), 0, "semantic tokens remain exact leaves");
            actual.push(node);
        }
        pending.extend(node.children().iter().rev());
    }
    assert_eq!(actual.len(), expected.len());
    for (node, token) in actual.into_iter().zip(expected) {
        assert_eq!(node.start_byte()..node.end_byte(), token.span);
        assert_eq!(node.utf8_text(source.as_bytes()).unwrap(), token.text);
        assert_eq!(node.start_point(), Point { row: token.row, column: token.column });
        assert_eq!(
            node.end_point(),
            Point {
                row: token.row,
                column: token.column + token.text.len() as u32,
            }
        );
    }
}

fn assert_document_tokens(document: &AdzeDocument, source: &str, expected: &[ExpectedToken]) {
    assert_eq!(document.outcome(), ParseOutcome::Complete);
    assert!(document.diagnostics().is_empty());
    assert_eq!(document.tree().root().byte_range(), 0..source.len());
    let actual: Vec<_> = (0..document.tree().node_count())
        .map(|index| document.tree().node(NodeId::new(index)).unwrap())
        .filter(|node| node.is_terminal() && !node.is_extra())
        .collect();
    assert_eq!(actual.len(), expected.len());
    for (node, token) in actual.into_iter().zip(expected) {
        assert_eq!(node.child_count(), 0);
        assert_eq!(node.byte_range(), token.span);
        assert_eq!(node.utf8_text().unwrap(), token.text);
        let points = node.point_range();
        assert_eq!((points.start.row, points.start.column), (token.row, token.column));
        assert_eq!(
            (points.end.row, points.end.column),
            (token.row, token.column + token.text.len() as u32)
        );
    }
}

fn number(expression: &Expression) -> i32 {
    let Expression::Primary(PrimaryExpression::Number(number)) = expression else {
        panic!("expected one numeric primary");
    };
    number.value
}

fn assert_assignments(module: &Module, count: usize) {
    assert_eq!(module.body.len(), count);
    for (index, statement) in module.body.iter().enumerate() {
        let Statement::Assignment(assignment) = statement else {
            panic!("statement {index} is not the required assignment");
        };
        assert_eq!(assignment.target.name, format!("var{index}"));
        let Expression::Add(product, (), addend) = &assignment.value else {
            panic!("statement {index} must add after multiplying");
        };
        let Expression::Multiply(left, (), right) = product.as_ref() else {
            panic!("statement {index} must multiply its left operand");
        };
        assert_eq!(number(left), index as i32);
        assert_eq!(number(right), 2);
        assert_eq!(number(addend), index as i32 + 1);
        assert_eq!(number(left) * number(right) + number(addend), 3 * index as i32 + 1);
    }
}

#[test]
fn test_generated_263_264_and_1000_assignments_have_exact_values_and_spans() {
    assert_eq!(
        engine_dispatch_for_language(grammar::language()),
        EngineDispatch::DeterministicLr
    );
    let mut parser = Parser::new();
    parser.set_language(grammar::language()).unwrap();
    for (count, byte_len) in [(263, 5721), (264, 5744), (1000, 22673)] {
        let input: String = (0..count)
            .map(|i| format!("var{i} = {i} * 2 + {}\n", i + 1))
            .collect();
        assert_eq!(input.len(), byte_len);
        let expected = assignment_tokens(count);
        assert_eq!(expected.len(), count * 7);
        let native = parser.parse_string(&input);
        assert!(native.errors.is_empty(), "count={count}: {:?}", native.errors);
        assert_native_tokens(
            native.root.as_ref().expect("complete native root"),
            &input,
            &expected,
        );
        assert_assignments(&grammar::parse(&input).expect("typed parse"), count);
        let document = grammar::parse_document(&input).expect("generated document");
        assert_document_tokens(&document, &input, &expected);
        assert_assignments(&document.ast::<Module>().expect("document typed projection"), count);
        // Normal drops and reuse are part of the regression, with no large
        // thread stack or forgotten tree to hide lifecycle failure.
        drop(document);
        drop(native);
        parser.reset();
    }
    let clean = parser.parse_string("x = 42");
    assert!(clean.errors.is_empty());
    assert!(clean.root.is_some());
}

#[test]
fn test_long_single_token_and_small_parse_remain_correct_after_recovery() {
    let mut parser = Parser::new();
    parser.set_language(grammar::language()).unwrap();
    let invalid = parser.parse_string("@");
    assert!(!invalid.errors.is_empty());
    drop(invalid);
    for identifier_length in [12_000, 1] {
        let identifier = "x".repeat(identifier_length);
        let input = format!("{identifier} = 42");
        let expected = vec![
            ExpectedToken {
                span: 0..identifier_length,
                text: identifier.clone(),
                row: 0,
                column: 0,
            },
            ExpectedToken {
                span: identifier_length + 1..identifier_length + 2,
                text: "=".to_string(),
                row: 0,
                column: identifier_length as u32 + 1,
            },
            ExpectedToken {
                span: identifier_length + 3..identifier_length + 5,
                text: "42".to_string(),
                row: 0,
                column: identifier_length as u32 + 3,
            },
        ];
        let native = parser.parse_string(&input);
        assert!(native.errors.is_empty());
        assert_native_tokens(
            native.root.as_ref().expect("complete native root"),
            &input,
            &expected,
        );
        let document = grammar::parse_document(&input).expect("generated document");
        assert_document_tokens(&document, &input, &expected);
        for module in [
            grammar::parse(&input).expect("typed parse"),
            document.ast::<Module>().expect("document typed projection"),
        ] {
            assert_eq!(module.body.len(), 1);
            let Statement::Assignment(assignment) = &module.body[0] else {
                panic!("expected assignment");
            };
            assert_eq!(assignment.target.name, identifier);
            assert_eq!(number(&assignment.value), 42);
        }
    }
}

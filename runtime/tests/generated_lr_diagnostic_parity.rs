//! #1240: every native LR failure has the same typed/document interpretation.
//!
//! Keep the original `", ,"` full-tail counterexample from commit 4987bbdc.
//! The streaming route keeps its first-stall contract (#1240/#1030);
//! SPEC0005 permits different diagnostic richness between paths.

#![cfg(feature = "pure-rust")]

use adze::__private::EngineDispatch;
use adze::document::{AdzeDocument, ParseOutcome};
use adze::errors::{ParseError, ParseErrorReason};
use std::ops::Range;

struct ExpectedDiagnostic {
    span: Range<usize>,
    start_point: (u32, u32),
    end_point: (u32, u32),
    found: &'static str,
    expected: &'static [&'static str],
}

fn assert_diagnostic_views<T: adze::Extract<T>>(
    source: &str,
    route: EngineDispatch,
    errors: &[ParseError],
    document: &AdzeDocument,
    expected: &[ExpectedDiagnostic],
) {
    assert_ne!(document.outcome(), ParseOutcome::Complete, "{source:?}");
    assert_eq!(errors.len(), expected.len(), "typed errors for {source:?}");
    assert_eq!(document.diagnostics().len(), expected.len(), "{source:?}");
    let projected = document
        .ast::<T>()
        .err()
        .expect("a diagnostic document must reject typed extraction");
    assert_eq!(projected.len(), expected.len(), "{source:?}");

    for (index, facts) in expected.iter().enumerate() {
        let error = &errors[index];
        let diagnostic = &document.diagnostics()[index];
        let projected_error = &projected[index];
        for actual in [error, projected_error] {
            assert_eq!(actual.byte_span(), facts.span, "{source:?}, error {index}");
            assert_eq!(actual.expected, facts.expected, "{source:?}, error {index}");
            assert!(actual.start <= actual.end && actual.end <= source.len());
            assert!(source.is_char_boundary(actual.start));
            assert!(source.is_char_boundary(actual.end));
            if facts.found == "end of input" {
                assert!(matches!(actual.reason, ParseErrorReason::UnexpectedEof));
            } else {
                assert!(matches!(
                    actual.reason,
                    ParseErrorReason::UnexpectedToken(_)
                ));
            }
            let points = actual.source_span(source.as_bytes());
            assert_eq!(
                (points.start.line, points.start.column),
                (
                    facts.start_point.0 as usize + 1,
                    facts.start_point.1 as usize + 1
                )
            );
            assert_eq!(
                (points.end.line, points.end.column),
                (
                    facts.end_point.0 as usize + 1,
                    facts.end_point.1 as usize + 1
                )
            );
        }
        assert_eq!(
            diagnostic.byte_span(),
            facts.span,
            "{source:?}, error {index}"
        );
        assert_eq!(diagnostic.found.as_deref(), Some(facts.found), "{source:?}");
        assert_eq!(
            diagnostic.expected, facts.expected,
            "{source:?}, error {index}"
        );
        assert_eq!(
            (
                diagnostic.point_range.start.row,
                diagnostic.point_range.start.column
            ),
            facts.start_point
        );
        assert_eq!(
            (
                diagnostic.point_range.end.row,
                diagnostic.point_range.end.column
            ),
            facts.end_point
        );
        if route == EngineDispatch::DeterministicLr
            && let ParseErrorReason::UnexpectedToken(message) = &error.reason
        {
            // Compare the two presentations without parsing facts from prose or
            // freezing a particular wording. Independent facts are above.
            assert_eq!(message, &diagnostic.message);
        }
    }
}

fn assert_clean_csv_after_failure() {
    use adze_example::csv_list::grammar;

    let source = "alpha, beta";
    let typed = grammar::parse(source).expect("clean typed parse after failure");
    let document = grammar::parse_document(source).expect("clean document after failure");
    assert_eq!(document.outcome(), ParseOutcome::Complete);
    assert!(document.diagnostics().is_empty());
    let projected: grammar::CsvList = document.ast().expect("clean typed projection");
    for list in [typed, projected] {
        let items: Vec<_> = list
            .items
            .value
            .iter()
            .map(|item| (item.value.as_str(), item.span))
            .collect();
        assert_eq!(items, [("alpha", (0, 5)), ("beta", (7, 11))]);
    }
}

#[cfg(not(feature = "glr"))]
#[test]
fn generated_lr_csv_trailing_delimiter_preserves_full_diagnostic_tail() {
    use adze_example::csv_list::grammar;

    let route = adze::__private::engine_dispatch_for_language(grammar::language());
    assert_eq!(route, EngineDispatch::DeterministicLr);

    // Original independent counterexample: retain the source and compare every
    // diagnostic, including the final delimiter (not just the first cause).
    let source = ", ,";
    let errors = grammar::parse(source).expect_err("repeated separators must fail");
    let document = grammar::parse_document(source).expect("bad syntax yields a document");
    assert_diagnostic_views::<grammar::CsvList>(
        source,
        route,
        &errors,
        &document,
        &[
            ExpectedDiagnostic {
                span: 0..1,
                start_point: (0, 0),
                end_point: (0, 1),
                found: ",",
                expected: &["identifier"],
            },
            ExpectedDiagnostic {
                span: 2..3,
                start_point: (0, 2),
                end_point: (0, 3),
                found: ",",
                expected: &["identifier"],
            },
            ExpectedDiagnostic {
                span: 2..3,
                start_point: (0, 2),
                end_point: (0, 3),
                found: ",",
                expected: &["end"],
            },
        ],
    );
    assert_clean_csv_after_failure();

    // EOF after whitespace must still point back to the delimiter, including
    // its original line/column rather than the final input position.
    for source in ["alpha,", "alpha,\n \t"] {
        let errors = grammar::parse(source).expect_err("trailing separator must fail");
        let document = grammar::parse_document(source).expect("diagnostic document");
        assert_diagnostic_views::<grammar::CsvList>(
            source,
            route,
            &errors,
            &document,
            &[ExpectedDiagnostic {
                span: 5..6,
                start_point: (0, 5),
                end_point: (0, 6),
                found: ",",
                expected: &["end"],
            }],
        );
        assert_clean_csv_after_failure();
    }
}

#[test]
fn generated_lr_genuine_eof_and_multiline_utf8_facts_remain_distinct() {
    use adze_example::typed_ast_contract::grammar::{self, Expr};

    let route = adze::__private::engine_dispatch_for_language(grammar::language());
    assert_eq!(route, EngineDispatch::DeterministicLr);
    let cases = [
        (
            "1 +",
            vec![ExpectedDiagnostic {
                span: 3..3,
                start_point: (0, 3),
                end_point: (0, 3),
                found: "end of input",
                expected: &["number"],
            }],
        ),
        (
            "1 +\n",
            vec![ExpectedDiagnostic {
                span: 4..4,
                start_point: (1, 0),
                end_point: (1, 0),
                found: "end of input",
                expected: &["number"],
            }],
        ),
        (
            "1 +\nλ",
            vec![
                ExpectedDiagnostic {
                    span: 4..6,
                    start_point: (1, 0),
                    end_point: (1, 2),
                    found: "λ",
                    expected: &["number"],
                },
                ExpectedDiagnostic {
                    span: 6..6,
                    start_point: (1, 2),
                    end_point: (1, 2),
                    found: "end of input",
                    expected: &["number"],
                },
            ],
        ),
    ];
    for (source, expected) in cases {
        let errors = grammar::parse(source).expect_err("incomplete expression must fail");
        let document = grammar::parse_document(source).expect("diagnostic document");
        assert_diagnostic_views::<Expr>(source, route, &errors, &document, &expected);

        // Successive generated calls, not a claim of persistent-parser reuse.
        let expected = Expr::Add(Box::new(Expr::Number(1)), (), Box::new(Expr::Number(2)));
        assert_eq!(
            grammar::parse("1 + 2").expect("clean typed parse"),
            expected
        );
        let clean = grammar::parse_document("1 + 2").expect("clean document");
        assert_eq!(clean.outcome(), ParseOutcome::Complete);
        assert!(clean.diagnostics().is_empty());
        assert_eq!(
            clean.ast::<Expr>().expect("clean typed projection"),
            expected
        );
    }
}

#[cfg(feature = "glr")]
#[test]
fn generated_streaming_csv_keeps_its_one_first_stall_diagnostic() {
    use adze_example::csv_list::grammar;

    let route = adze::__private::engine_dispatch_for_language(grammar::language());
    assert_eq!(route, EngineDispatch::StreamingGlr);
    let source = ", ,";
    let errors = grammar::parse(source).expect_err("streaming lexer must stop");
    let document = grammar::parse_document(source).expect("diagnostic document");
    assert_diagnostic_views::<grammar::CsvList>(
        source,
        route,
        &errors,
        &document,
        &[ExpectedDiagnostic {
            span: 0..1,
            start_point: (0, 0),
            end_point: (0, 1),
            found: ",",
            expected: &["identifier"],
        }],
    );
    assert_clean_csv_after_failure();
}

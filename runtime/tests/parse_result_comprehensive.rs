#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

//! Comprehensive tests for parse result types in the adze runtime.
//!
//! Covers `ParseResult`, `ParseError` (both `pure_parser` and `errors` module),
//! `ParseErrorReason`, `SpanError`, and `SpanErrorReason`.

use adze::errors::{ParseError as ErrorsParseError, ParseErrorReason};
use adze::pure_parser::{ParseError, ParseResult, ParsedNode, Point};
use adze::{SpanError, SpanErrorReason};
use std::mem::MaybeUninit;

// ============================================================================
// Helpers
// ============================================================================

/// Construct a `ParsedNode` working around the `pub(crate)` `language` field.
fn make_node(start: usize, end: usize, children: Vec<ParsedNode>) -> ParsedNode {
    let mut uninit = MaybeUninit::<ParsedNode>::uninit();
    let ptr = uninit.as_mut_ptr();
    unsafe {
        std::ptr::write_bytes(ptr, 0, 1);
        std::ptr::addr_of_mut!((*ptr).symbol).write(1);
        std::ptr::addr_of_mut!((*ptr).children).write(children);
        std::ptr::addr_of_mut!((*ptr).start_byte).write(start);
        std::ptr::addr_of_mut!((*ptr).end_byte).write(end);
        std::ptr::addr_of_mut!((*ptr).start_point).write(Point {
            row: 0,
            column: start as u32,
        });
        std::ptr::addr_of_mut!((*ptr).end_point).write(Point {
            row: 0,
            column: end as u32,
        });
        std::ptr::addr_of_mut!((*ptr).is_extra).write(false);
        std::ptr::addr_of_mut!((*ptr).is_error).write(false);
        std::ptr::addr_of_mut!((*ptr).is_missing).write(false);
        std::ptr::addr_of_mut!((*ptr).is_named).write(true);
        std::ptr::addr_of_mut!((*ptr).field_id).write(None);
        uninit.assume_init()
    }
}

fn leaf(start: usize, end: usize) -> ParsedNode {
    make_node(start, end, vec![])
}

fn make_parse_error(position: usize) -> ParseError {
    ParseError {
        work_limit: None,
        position,
        point: Point {
            row: 0,
            column: position as u32,
        },
        expected: vec![1, 2],
        found: 3,
    }
}

// ============================================================================
// 1. Successful parse results
// ============================================================================

#[test]
fn successful_parse_result_has_root() {
    let result = ParseResult {
        root: Some(leaf(0, 5)),
        errors: vec![],
    };
    assert!(result.root.is_some());
    assert!(result.errors.is_empty());
}

#[test]
fn successful_parse_result_root_spans() {
    let result = ParseResult {
        root: Some(leaf(0, 42)),
        errors: vec![],
    };
    let root = result.root.unwrap();
    assert_eq!(root.start_byte(), 0);
    assert_eq!(root.end_byte(), 42);
}

#[test]
fn successful_parse_result_with_children() {
    let parent = make_node(0, 10, vec![leaf(0, 3), leaf(4, 10)]);
    let result = ParseResult {
        root: Some(parent),
        errors: vec![],
    };
    let root = result.root.unwrap();
    assert_eq!(root.child_count(), 2);
    assert_eq!(root.child(0).unwrap().end_byte(), 3);
    assert_eq!(root.child(1).unwrap().start_byte(), 4);
}

// ============================================================================
// 2. Failed parse results
// ============================================================================

#[test]
fn failed_parse_result_no_root() {
    let result = ParseResult {
        root: None,
        errors: vec![make_parse_error(0)],
    };
    assert!(result.root.is_none());
    assert_eq!(result.errors.len(), 1);
}

#[test]
fn failed_parse_result_error_position() {
    let result = ParseResult {
        root: None,
        errors: vec![make_parse_error(7)],
    };
    assert_eq!(result.errors[0].position, 7);
    assert_eq!(result.errors[0].point, Point { row: 0, column: 7 });
}

#[test]
fn failed_parse_result_error_expected_found() {
    let err = make_parse_error(0);
    assert_eq!(err.expected, vec![1, 2]);
    assert_eq!(err.found, 3);
}

#[test]
fn failed_parse_result_empty_expected() {
    let err = ParseError {
        work_limit: None,
        position: 0,
        point: Point { row: 0, column: 0 },
        expected: vec![],
        found: 0,
    };
    assert!(err.expected.is_empty());
}

// ============================================================================
// 3. Parse result with warnings (root present but errors non-empty)
// ============================================================================

#[test]
fn parse_result_with_root_and_errors() {
    let result = ParseResult {
        root: Some(leaf(0, 10)),
        errors: vec![make_parse_error(5)],
    };
    assert!(result.root.is_some());
    assert!(!result.errors.is_empty());
}

#[test]
fn parse_result_root_accessible_despite_errors() {
    let result = ParseResult {
        root: Some(leaf(0, 20)),
        errors: vec![make_parse_error(3), make_parse_error(8)],
    };
    let root = result.root.unwrap();
    assert_eq!(root.end_byte(), 20);
    assert_eq!(result.errors.len(), 2);
}

// ============================================================================
// 4. ParseErrorReason variants
// ============================================================================

#[test]
fn parse_error_reason_unexpected_token() {
    let reason = ParseErrorReason::UnexpectedToken("foo".to_string());
    match &reason {
        ParseErrorReason::UnexpectedToken(tok) => assert_eq!(tok, "foo"),
        _ => panic!("expected UnexpectedToken"),
    }
}

#[test]
fn parse_error_reason_failed_node_empty() {
    let reason = ParseErrorReason::FailedNode(vec![]);
    match &reason {
        ParseErrorReason::FailedNode(inner) => assert!(inner.is_empty()),
        _ => panic!("expected FailedNode"),
    }
}

#[test]
fn parse_error_reason_failed_node_with_children() {
    let inner = ErrorsParseError {
        reason: ParseErrorReason::UnexpectedToken("bar".into()),
        start: 1,
        end: 4,
        expected: vec![],
    };
    let reason = ParseErrorReason::FailedNode(vec![inner]);
    match &reason {
        ParseErrorReason::FailedNode(errs) => {
            assert_eq!(errs.len(), 1);
            assert_eq!(errs[0].start, 1);
            assert_eq!(errs[0].end, 4);
        }
        _ => panic!("expected FailedNode"),
    }
}

#[test]
fn parse_error_reason_missing_token() {
    let reason = ParseErrorReason::MissingToken("SEMICOLON".to_string());
    match &reason {
        ParseErrorReason::MissingToken(tok) => assert_eq!(tok, "SEMICOLON"),
        _ => panic!("expected MissingToken"),
    }
}

// ============================================================================
// 5. SpanError type
// ============================================================================

#[test]
fn span_error_start_greater_than_end() {
    let err = SpanError {
        span: (10, 5),
        source_len: 20,
        reason: SpanErrorReason::StartGreaterThanEnd,
    };
    assert_eq!(err.span, (10, 5));
    assert_eq!(err.source_len, 20);
    assert_eq!(err.reason, SpanErrorReason::StartGreaterThanEnd);
}

#[test]
fn span_error_start_out_of_bounds() {
    let err = SpanError {
        span: (25, 30),
        source_len: 20,
        reason: SpanErrorReason::StartOutOfBounds,
    };
    assert_eq!(err.reason, SpanErrorReason::StartOutOfBounds);
}

#[test]
fn span_error_end_out_of_bounds() {
    let err = SpanError {
        span: (5, 25),
        source_len: 20,
        reason: SpanErrorReason::EndOutOfBounds,
    };
    assert_eq!(err.reason, SpanErrorReason::EndOutOfBounds);
}

#[test]
fn span_error_clone_and_eq() {
    let err = SpanError {
        span: (3, 1),
        source_len: 10,
        reason: SpanErrorReason::StartGreaterThanEnd,
    };
    let cloned = err.clone();
    assert_eq!(err, cloned);
}

#[test]
fn span_error_ne_different_reason() {
    let a = SpanError {
        span: (5, 25),
        source_len: 20,
        reason: SpanErrorReason::EndOutOfBounds,
    };
    let b = SpanError {
        span: (5, 25),
        source_len: 20,
        reason: SpanErrorReason::StartGreaterThanEnd,
    };
    assert_ne!(a, b);
}

// ============================================================================
// 6. Error propagation
// ============================================================================

#[test]
fn errors_parse_error_nested_propagation() {
    let leaf_err = ErrorsParseError {
        reason: ParseErrorReason::UnexpectedToken("+".into()),
        start: 3,
        end: 4,
        expected: vec![],
    };
    let mid = ErrorsParseError {
        reason: ParseErrorReason::FailedNode(vec![leaf_err]),
        start: 0,
        end: 10,
        expected: vec![],
    };
    let root = ErrorsParseError {
        reason: ParseErrorReason::FailedNode(vec![mid]),
        start: 0,
        end: 20,
        expected: vec![],
    };
    match &root.reason {
        ParseErrorReason::FailedNode(level1) => {
            assert_eq!(level1.len(), 1);
            match &level1[0].reason {
                ParseErrorReason::FailedNode(level2) => {
                    assert_eq!(level2.len(), 1);
                    match &level2[0].reason {
                        ParseErrorReason::UnexpectedToken(tok) => assert_eq!(tok, "+"),
                        _ => panic!("expected UnexpectedToken at leaf"),
                    }
                }
                _ => panic!("expected FailedNode at level 1"),
            }
        }
        _ => panic!("expected FailedNode at root"),
    }
}

#[test]
fn span_error_is_std_error() {
    let err = SpanError {
        span: (5, 2),
        source_len: 10,
        reason: SpanErrorReason::StartGreaterThanEnd,
    };
    let dyn_err: &dyn std::error::Error = &err;
    assert!(dyn_err.source().is_none());
}

#[test]
fn span_error_can_be_boxed() {
    let err = SpanError {
        span: (5, 2),
        source_len: 10,
        reason: SpanErrorReason::StartGreaterThanEnd,
    };
    let boxed: Box<dyn std::error::Error> = Box::new(err);
    assert!(!boxed.to_string().is_empty());
}

#[test]
fn errors_parse_error_sibling_propagation() {
    let a = ErrorsParseError {
        reason: ParseErrorReason::UnexpectedToken("x".into()),
        start: 0,
        end: 1,
        expected: vec![],
    };
    let b = ErrorsParseError {
        reason: ParseErrorReason::MissingToken(";".into()),
        start: 5,
        end: 5,
        expected: vec![],
    };
    let parent = ErrorsParseError {
        reason: ParseErrorReason::FailedNode(vec![a, b]),
        start: 0,
        end: 10,
        expected: vec![],
    };
    match &parent.reason {
        ParseErrorReason::FailedNode(children) => assert_eq!(children.len(), 2),
        _ => panic!("expected FailedNode"),
    }
}

// ============================================================================
// 7. Result display/debug
// ============================================================================

#[test]
fn span_error_display_start_greater_than_end() {
    let err = SpanError {
        span: (8, 3),
        source_len: 20,
        reason: SpanErrorReason::StartGreaterThanEnd,
    };
    let s = err.to_string();
    assert!(s.contains("8"));
    assert!(s.contains("3"));
    assert!(s.contains("start"));
}

#[test]
fn span_error_display_start_out_of_bounds() {
    let err = SpanError {
        span: (50, 60),
        source_len: 10,
        reason: SpanErrorReason::StartOutOfBounds,
    };
    let s = err.to_string();
    assert!(s.contains("50"));
    assert!(s.contains("10"));
}

#[test]
fn span_error_display_end_out_of_bounds() {
    let err = SpanError {
        span: (0, 100),
        source_len: 50,
        reason: SpanErrorReason::EndOutOfBounds,
    };
    let s = err.to_string();
    assert!(s.contains("100"));
    assert!(s.contains("50"));
}

#[test]
fn span_error_debug_format() {
    let err = SpanError {
        span: (1, 0),
        source_len: 5,
        reason: SpanErrorReason::StartGreaterThanEnd,
    };
    let dbg = format!("{err:?}");
    assert!(dbg.contains("SpanError"));
    assert!(dbg.contains("StartGreaterThanEnd"));
}

#[test]
fn parse_error_debug_format() {
    let err = make_parse_error(42);
    let dbg = format!("{err:?}");
    assert!(dbg.contains("ParseError"));
    assert!(dbg.contains("42"));
}

#[test]
fn errors_parse_error_debug_shows_reason() {
    let err = ErrorsParseError {
        reason: ParseErrorReason::MissingToken("EOF".into()),
        start: 0,
        end: 0,
        expected: vec![],
    };
    let dbg = format!("{err:?}");
    assert!(dbg.contains("MissingToken"));
    assert!(dbg.contains("EOF"));
}

// ============================================================================
// 8. Result conversion
// ============================================================================

#[test]
fn parse_result_into_option_some() {
    let result = ParseResult {
        root: Some(leaf(0, 5)),
        errors: vec![],
    };
    let opt: Option<ParsedNode> = result.root;
    assert!(opt.is_some());
}

#[test]
fn parse_result_into_option_none() {
    let result = ParseResult {
        root: None,
        errors: vec![make_parse_error(0)],
    };
    let opt: Option<ParsedNode> = result.root;
    assert!(opt.is_none());
}

#[test]
fn span_error_reason_all_variants_are_distinct() {
    let a = SpanErrorReason::StartGreaterThanEnd;
    let b = SpanErrorReason::StartOutOfBounds;
    let c = SpanErrorReason::EndOutOfBounds;
    assert_ne!(a, b);
    assert_ne!(b, c);
    assert_ne!(a, c);
}

#[test]
fn parse_error_clone() {
    let err = make_parse_error(10);
    let cloned = err.clone();
    assert_eq!(cloned.position, 10);
    assert_eq!(cloned.expected, vec![1, 2]);
    assert_eq!(cloned.found, 3);
}

// ============================================================================
// 9. Multiple errors in result
// ============================================================================

#[test]
fn multiple_pure_parser_errors() {
    let result = ParseResult {
        root: None,
        errors: vec![
            make_parse_error(0),
            make_parse_error(5),
            make_parse_error(12),
        ],
    };
    assert_eq!(result.errors.len(), 3);
    for i in 0..result.errors.len() {
        assert_eq!(result.errors[i].expected.len(), 2);
    }
}

#[test]
fn multiple_errors_parse_errors_different_reasons() {
    let errors = [
        ErrorsParseError {
            reason: ParseErrorReason::UnexpectedToken("x".into()),
            start: 0,
            end: 1,
            expected: vec![],
        },
        ErrorsParseError {
            reason: ParseErrorReason::MissingToken(";".into()),
            start: 5,
            end: 5,
            expected: vec![],
        },
        ErrorsParseError {
            reason: ParseErrorReason::FailedNode(vec![]),
            start: 10,
            end: 15,
            expected: vec![],
        },
    ];
    assert_eq!(errors.len(), 3);
    assert!(matches!(
        &errors[0].reason,
        ParseErrorReason::UnexpectedToken(_)
    ));
    assert!(matches!(
        &errors[1].reason,
        ParseErrorReason::MissingToken(_)
    ));
    assert!(matches!(&errors[2].reason, ParseErrorReason::FailedNode(_)));
}

#[test]
fn multiple_errors_positions_are_ordered() {
    let result = ParseResult {
        root: Some(leaf(0, 100)),
        errors: vec![
            make_parse_error(10),
            make_parse_error(20),
            make_parse_error(30),
        ],
    };
    for i in 0..result.errors.len() - 1 {
        assert!(result.errors[i].position < result.errors[i + 1].position);
    }
}

#[test]
fn errors_collect_into_vec() {
    let all_errors: Vec<ErrorsParseError> = vec![
        ErrorsParseError {
            reason: ParseErrorReason::UnexpectedToken("a".into()),
            start: 0,
            end: 1,
            expected: vec![],
        },
        ErrorsParseError {
            reason: ParseErrorReason::UnexpectedToken("b".into()),
            start: 2,
            end: 3,
            expected: vec![],
        },
    ];
    assert_eq!(all_errors.len(), 2);
    for i in 0..all_errors.len() {
        match &all_errors[i].reason {
            ParseErrorReason::UnexpectedToken(_) => {}
            _ => panic!("expected UnexpectedToken"),
        }
    }
}

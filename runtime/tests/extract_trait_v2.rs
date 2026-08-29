//! Comprehensive tests for the Extract trait and parsing API.
//!
//! Covers: Extract trait properties, ParsedNode operations, tree traversal,
//! type extraction, error handling, helper functions, and edge cases.

use std::mem::MaybeUninit;

use adze::errors::{ParseError, ParseErrorReason};
use adze::pure_parser::{ParsedNode, Point};
use adze::{Extract, SpanError, SpanErrorReason, Spanned, WithLeaf};

// ---------------------------------------------------------------------------
// Helpers — uses MaybeUninit to skip the pub(crate) `language` field.
// ---------------------------------------------------------------------------

/// Builds a `ParsedNode` from individual field values, zeroing `language`.
///
/// # Safety
///
/// All public fields are written; the private `language` field is zeroed,
/// which is equivalent to `None` for `Option<*const _>`.
#[expect(
    clippy::too_many_arguments,
    reason = "test helper accepts all fields as explicit parameters to avoid builder indirection"
)]
fn make_node(
    symbol: u16,
    children: Vec<ParsedNode>,
    start: usize,
    end: usize,
    start_point: Point,
    end_point: Point,
    is_extra: bool,
    is_error: bool,
    is_missing: bool,
    is_named: bool,
    field_id: Option<u16>,
) -> ParsedNode {
    let mut uninit = MaybeUninit::<ParsedNode>::uninit();
    let ptr = uninit.as_mut_ptr();
    // SAFETY: We write every field of `ParsedNode`. The private `language`
    // field is covered by the initial zero-fill (Option<*const _> is
    // all-zeros == None).
    unsafe {
        std::ptr::write_bytes(ptr, 0, 1);
        std::ptr::addr_of_mut!((*ptr).symbol).write(symbol);
        std::ptr::addr_of_mut!((*ptr).children).write(children);
        std::ptr::addr_of_mut!((*ptr).start_byte).write(start);
        std::ptr::addr_of_mut!((*ptr).end_byte).write(end);
        std::ptr::addr_of_mut!((*ptr).start_point).write(start_point);
        std::ptr::addr_of_mut!((*ptr).end_point).write(end_point);
        std::ptr::addr_of_mut!((*ptr).is_extra).write(is_extra);
        std::ptr::addr_of_mut!((*ptr).is_error).write(is_error);
        std::ptr::addr_of_mut!((*ptr).is_missing).write(is_missing);
        std::ptr::addr_of_mut!((*ptr).is_named).write(is_named);
        std::ptr::addr_of_mut!((*ptr).field_id).write(field_id);
        uninit.assume_init()
    }
}

fn pt(row: u32, col: u32) -> Point {
    Point { row, column: col }
}

/// Named leaf on row 0.
fn leaf_node(symbol: u16, start: usize, end: usize) -> ParsedNode {
    make_node(
        symbol,
        vec![],
        start,
        end,
        pt(0, start as u32),
        pt(0, end as u32),
        false,
        false,
        false,
        true,
        None,
    )
}

/// Parent node whose span covers its children.
fn parent_node(symbol: u16, children: Vec<ParsedNode>) -> ParsedNode {
    let start = children.first().map_or(0, |c| c.start_byte);
    let end = children.last().map_or(0, |c| c.end_byte);
    make_node(
        symbol,
        children,
        start,
        end,
        pt(0, start as u32),
        pt(0, end as u32),
        false,
        false,
        false,
        true,
        None,
    )
}

// ===========================================================================
// 1. Extract trait properties (8 tests)
// ===========================================================================

#[test]
fn test_extract_is_open_to_downstream_types() {
    // `Extract` names no marker supertrait, so a type declared outside `adze`
    // needs nothing extra to satisfy the bound. See ADZE-ADR-0008 and
    // `extract_open_contract.rs` for the full downstream proof.
    struct Dummy;
    impl Extract<u32> for Dummy {
        type LeafFn = ();

        fn extract(
            _node: Option<&ParsedNode>,
            _source: &[u8],
            _last_idx: usize,
            _leaf_fn: Option<&Self::LeafFn>,
        ) -> u32 {
            0
        }
    }
    fn assert_extract<T: Extract<u32>>() {}
    assert_extract::<Dummy>();
}

#[test]
fn test_extract_has_conflicts_default_is_false() {
    const { assert!(!<String as Extract<String>>::HAS_CONFLICTS) };
}

#[test]
fn test_extract_unit_has_no_conflicts() {
    const { assert!(!<() as Extract<()>>::HAS_CONFLICTS) };
}

#[test]
fn test_extract_option_has_no_conflicts() {
    const { assert!(!<Option<String> as Extract<Option<String>>>::HAS_CONFLICTS) };
}

#[test]
fn test_extract_box_has_no_conflicts() {
    const { assert!(!<Box<String> as Extract<Box<String>>>::HAS_CONFLICTS) };
}

#[test]
fn test_extract_vec_has_no_conflicts() {
    const { assert!(!<Vec<String> as Extract<Vec<String>>>::HAS_CONFLICTS) };
}

#[test]
fn test_extract_string_leaf_fn_is_unit() {
    // String's LeafFn associated type should be ().
    fn assert_leaf_fn_is_unit<T: Extract<String, LeafFn = ()>>() {}
    assert_leaf_fn_is_unit::<String>();
}

#[test]
fn test_extract_trait_has_no_sealing_supertrait() {
    // `Extract` alone is a sufficient bound. If a sealing supertrait is ever
    // reintroduced, this signature stops compiling.
    fn _check<T: Extract<U>, U>() {}
}

// ===========================================================================
// 2. ParsedNode type operations (8 tests)
// ===========================================================================

#[test]
fn test_parsed_node_accessors_basic() {
    let node = leaf_node(5, 10, 20);
    assert_eq!(node.symbol(), 5);
    assert_eq!(node.start_byte(), 10);
    assert_eq!(node.end_byte(), 20);
}

#[test]
fn test_parsed_node_point_accessors() {
    let node = make_node(
        0,
        vec![],
        4,
        9,
        pt(1, 4),
        pt(1, 9),
        false,
        false,
        false,
        true,
        None,
    );
    assert_eq!(node.start_point(), Point { row: 1, column: 4 });
    assert_eq!(node.end_point(), Point { row: 1, column: 9 });
}

#[test]
fn test_parsed_node_flags_default_false() {
    let node = leaf_node(1, 0, 3);
    assert!(!node.is_extra());
    assert!(!node.is_error());
    assert!(!node.is_missing());
    assert!(node.is_named());
}

#[test]
fn test_parsed_node_error_flag() {
    let node = make_node(
        1,
        vec![],
        0,
        3,
        pt(0, 0),
        pt(0, 3),
        false,
        true,
        false,
        true,
        None,
    );
    assert!(node.is_error());
    assert!(node.has_error());
}

#[test]
fn test_parsed_node_missing_flag() {
    let node = make_node(
        1,
        vec![],
        0,
        0,
        pt(0, 0),
        pt(0, 0),
        false,
        false,
        true,
        true,
        None,
    );
    assert!(node.is_missing());
}

#[test]
fn test_parsed_node_extra_flag() {
    let node = make_node(
        6,
        vec![],
        0,
        1,
        pt(0, 0),
        pt(0, 1),
        true,
        false,
        false,
        true,
        None,
    );
    assert!(node.is_extra());
}

#[test]
fn test_parsed_node_utf8_text() {
    let source = b"hello world";
    let node = leaf_node(1, 0, 5);
    assert_eq!(node.utf8_text(source).unwrap(), "hello");
}

#[test]
fn test_parsed_node_utf8_text_mid_range() {
    let source = b"abc xyz";
    let node = leaf_node(1, 4, 7);
    assert_eq!(node.utf8_text(source).unwrap(), "xyz");
}

// ===========================================================================
// 3. Parse tree traversal (8 tests)
// ===========================================================================

#[test]
fn test_child_count_leaf() {
    let node = leaf_node(1, 0, 3);
    assert_eq!(node.child_count(), 0);
}

#[test]
fn test_child_count_parent() {
    let node = parent_node(0, vec![leaf_node(1, 0, 2), leaf_node(2, 3, 5)]);
    assert_eq!(node.child_count(), 2);
}

#[test]
fn test_child_by_index() {
    let node = parent_node(0, vec![leaf_node(1, 0, 2), leaf_node(2, 3, 5)]);
    assert_eq!(node.child(0).unwrap().symbol(), 1);
    assert_eq!(node.child(1).unwrap().symbol(), 2);
    assert!(node.child(2).is_none());
}

#[test]
fn test_children_slice() {
    let node = parent_node(
        0,
        vec![leaf_node(1, 0, 1), leaf_node(2, 2, 3), leaf_node(3, 4, 5)],
    );
    let children = node.children();
    assert_eq!(children.len(), 3);
    assert_eq!(children[2].symbol(), 3);
}

#[test]
fn test_walker_traversal() {
    let node = parent_node(0, vec![leaf_node(1, 0, 1), leaf_node(2, 2, 3)]);
    let mut walker = node.walk();
    assert!(walker.goto_first_child());
    assert_eq!(walker.node().symbol(), 1);
    assert!(walker.goto_next_sibling());
    assert_eq!(walker.node().symbol(), 2);
    assert!(!walker.goto_next_sibling());
}

#[test]
fn test_walker_empty_children() {
    let node = leaf_node(0, 0, 0);
    let mut walker = node.walk();
    assert!(!walker.goto_first_child());
}

#[test]
fn test_has_error_propagates() {
    let error_child = make_node(
        1,
        vec![],
        0,
        1,
        pt(0, 0),
        pt(0, 1),
        false,
        true,
        false,
        true,
        None,
    );
    let parent = parent_node(0, vec![leaf_node(2, 0, 1), error_child]);
    assert!(parent.has_error());
}

#[test]
fn test_has_error_deep_propagation() {
    let deep_error = make_node(
        3,
        vec![],
        0,
        1,
        pt(0, 0),
        pt(0, 1),
        false,
        true,
        false,
        true,
        None,
    );
    let mid = parent_node(2, vec![deep_error]);
    let root = parent_node(1, vec![mid]);
    assert!(root.has_error());
}

// ===========================================================================
// 4. Type extraction patterns (7 tests)
// ===========================================================================

#[test]
fn test_extract_string_from_node() {
    let source = b"hello";
    let node = leaf_node(1, 0, 5);
    let result = <String as Extract<String>>::extract(Some(&node), source, 0, None);
    assert_eq!(result, "hello");
}

#[test]
fn test_extract_string_from_none() {
    let result = <String as Extract<String>>::extract(None, b"ignored", 0, None);
    assert_eq!(result, "");
}

#[test]
fn test_extract_unit() {
    // Unit extraction always succeeds and returns ().
    let node = leaf_node(0, 0, 0);
    <() as Extract<()>>::extract(Some(&node), b"", 0, None);
}

#[test]
fn test_extract_option_some() {
    let source = b"42";
    let node = leaf_node(1, 0, 2);
    let result = <Option<String> as Extract<Option<String>>>::extract(Some(&node), source, 0, None);
    assert_eq!(result, Some("42".to_string()));
}

#[test]
fn test_extract_option_none() {
    let result = <Option<String> as Extract<Option<String>>>::extract(None, b"ignored", 0, None);
    assert!(result.is_none());
}

#[test]
fn test_extract_box() {
    let source = b"boxed";
    let node = leaf_node(1, 0, 5);
    let result = <Box<String> as Extract<Box<String>>>::extract(Some(&node), source, 0, None);
    assert_eq!(*result, "boxed");
}

#[test]
fn test_extract_i32_from_node() {
    let source = b"42";
    let node = leaf_node(1, 0, 2);
    let result = <i32 as Extract<i32>>::extract(Some(&node), source, 0, None);
    assert_eq!(result, 42);
}

// ===========================================================================
// 5. Error handling (8 tests)
// ===========================================================================

#[test]
fn test_parse_error_reason_unexpected_token() {
    let err = ParseError {
        reason: ParseErrorReason::UnexpectedToken("$".to_string()),
        start: 0,
        end: 1,
        expected: vec![],
    };
    assert_eq!(err.start, 0);
    assert_eq!(err.end, 1);
    match &err.reason {
        ParseErrorReason::UnexpectedToken(tok) => assert_eq!(tok, "$"),
        _ => panic!("expected UnexpectedToken"),
    }
}

#[test]
fn test_parse_error_reason_missing_token() {
    let err = ParseError {
        reason: ParseErrorReason::MissingToken("semicolon".to_string()),
        start: 5,
        end: 5,
        expected: vec![],
    };
    match &err.reason {
        ParseErrorReason::MissingToken(tok) => assert_eq!(tok, "semicolon"),
        _ => panic!("expected MissingToken"),
    }
}

#[test]
fn test_parse_error_reason_failed_node_empty() {
    let err = ParseError {
        reason: ParseErrorReason::FailedNode(vec![]),
        start: 0,
        end: 10,
        expected: vec![],
    };
    match &err.reason {
        ParseErrorReason::FailedNode(inner) => assert!(inner.is_empty()),
        _ => panic!("expected FailedNode"),
    }
}

#[test]
fn test_parse_error_reason_failed_node_nested() {
    let inner = ParseError {
        reason: ParseErrorReason::UnexpectedToken("x".to_string()),
        start: 3,
        end: 4,
        expected: vec![],
    };
    let outer = ParseError {
        reason: ParseErrorReason::FailedNode(vec![inner]),
        start: 0,
        end: 10,
        expected: vec![],
    };
    match &outer.reason {
        ParseErrorReason::FailedNode(v) => assert_eq!(v.len(), 1),
        _ => panic!("expected FailedNode"),
    }
}

#[test]
fn test_span_error_start_greater_than_end() {
    let err = SpanError {
        span: (5, 3),
        source_len: 10,
        reason: SpanErrorReason::StartGreaterThanEnd,
    };
    assert_eq!(err.reason, SpanErrorReason::StartGreaterThanEnd);
    let msg = err.to_string();
    assert!(msg.contains("start (5) > end (3)"), "got: {msg}");
}

#[test]
fn test_span_error_start_out_of_bounds() {
    let err = SpanError {
        span: (20, 25),
        source_len: 10,
        reason: SpanErrorReason::StartOutOfBounds,
    };
    assert_eq!(err.reason, SpanErrorReason::StartOutOfBounds);
    let msg = err.to_string();
    assert!(
        msg.contains("start (20) > source length (10)"),
        "got: {msg}"
    );
}

#[test]
fn test_span_error_end_out_of_bounds() {
    let err = SpanError {
        span: (0, 99),
        source_len: 10,
        reason: SpanErrorReason::EndOutOfBounds,
    };
    assert_eq!(err.reason, SpanErrorReason::EndOutOfBounds);
    let msg = err.to_string();
    assert!(msg.contains("end (99) > source length (10)"), "got: {msg}");
}

#[test]
fn test_span_error_implements_std_error() {
    let err = SpanError {
        span: (5, 3),
        source_len: 10,
        reason: SpanErrorReason::StartGreaterThanEnd,
    };
    // SpanError implements std::error::Error.
    let _: &dyn std::error::Error = &err;
}

// ===========================================================================
// 6. Helper functions / Spanned utilities (8 tests)
// ===========================================================================

#[test]
fn test_spanned_deref() {
    let s = Spanned {
        value: 42,
        span: (0, 2),
    };
    assert_eq!(*s, 42);
}

#[test]
fn test_spanned_index_str() {
    let source = "hello world";
    let s = Spanned {
        value: (),
        span: (6, 11),
    };
    assert_eq!(&source[s], "world");
}

#[test]
fn test_spanned_index_empty_span() {
    let source = "hello";
    let s = Spanned {
        value: (),
        span: (3, 3),
    };
    assert_eq!(&source[s], "");
}

#[test]
fn test_spanned_index_full_string() {
    let source = "abc";
    let s = Spanned {
        value: (),
        span: (0, 3),
    };
    assert_eq!(&source[s], "abc");
}

#[test]
fn test_spanned_clone() {
    let s = Spanned {
        value: "hi".to_string(),
        span: (0, 2),
    };
    let s2 = s.clone();
    assert_eq!(*s2, "hi");
    assert_eq!(s2.span, (0, 2));
}

#[test]
fn test_spanned_debug() {
    let s = Spanned {
        value: 7,
        span: (0, 1),
    };
    let dbg = format!("{s:?}");
    assert!(dbg.contains("7"), "got: {dbg}");
    assert!(dbg.contains("span"), "got: {dbg}");
}

#[test]
fn test_spanned_extract_from_node() {
    let source = b"hello";
    let node = leaf_node(1, 0, 5);
    let result =
        <Spanned<String> as Extract<Spanned<String>>>::extract(Some(&node), source, 0, None);
    assert_eq!(*result, "hello");
    assert_eq!(result.span, (0, 5));
}

#[test]
fn test_spanned_extract_from_none_uses_last_idx() {
    let result = <Spanned<String> as Extract<Spanned<String>>>::extract(None, b"x", 7, None);
    assert_eq!(*result, "");
    assert_eq!(result.span, (7, 7));
}

// ===========================================================================
// 7. Edge cases (8 tests)
// ===========================================================================

#[test]
fn test_extract_string_empty_source() {
    let node = leaf_node(1, 0, 0);
    let result = <String as Extract<String>>::extract(Some(&node), b"", 0, None);
    assert_eq!(result, "");
}

#[test]
fn test_extract_primitive_invalid_text_returns_default() {
    // "abc" cannot parse as i32 — should return 0.
    let source = b"abc";
    let node = leaf_node(1, 0, 3);
    let result = <i32 as Extract<i32>>::extract(Some(&node), source, 0, None);
    assert_eq!(result, 0);
}

#[test]
fn test_extract_bool_from_true_text() {
    let source = b"true";
    let node = leaf_node(1, 0, 4);
    let result = <bool as Extract<bool>>::extract(Some(&node), source, 0, None);
    assert!(result);
}

#[test]
fn test_extract_bool_invalid_returns_false() {
    let source = b"maybe";
    let node = leaf_node(1, 0, 5);
    let result = <bool as Extract<bool>>::extract(Some(&node), source, 0, None);
    assert!(!result);
}

#[test]
fn test_extract_f64_from_node() {
    let source = b"1.25";
    let node = leaf_node(1, 0, 4);
    let result = <f64 as Extract<f64>>::extract(Some(&node), source, 0, None);
    assert!((result - 1.25).abs() < f64::EPSILON);
}

#[test]
fn test_extract_u8_overflow_returns_default() {
    let source = b"999";
    let node = leaf_node(1, 0, 3);
    let result = <u8 as Extract<u8>>::extract(Some(&node), source, 0, None);
    // parse::<u8>("999") fails → default 0.
    assert_eq!(result, 0);
}

#[test]
fn test_extract_vec_from_none_returns_empty() {
    let result = <Vec<String> as Extract<Vec<String>>>::extract(None, b"", 0, None);
    assert!(result.is_empty());
}

#[test]
#[should_panic(expected = "Leaf extraction failed")]
fn test_with_leaf_panics_without_transform() {
    let source = b"hello";
    let node = leaf_node(1, 0, 5);
    let _: String = <WithLeaf<String> as Extract<String>>::extract(Some(&node), source, 0, None);
}

#[test]
fn test_with_leaf_uses_transform() {
    let source = b"hello";
    let node = leaf_node(1, 0, 5);
    let transform: &dyn Fn(&str) -> String = &|s: &str| s.to_uppercase();
    let result =
        <WithLeaf<String> as Extract<String>>::extract(Some(&node), source, 0, Some(transform));
    assert_eq!(result, "HELLO");
}

#[test]
fn test_extract_negative_i64() {
    let source = b"-999";
    let node = leaf_node(1, 0, 4);
    let result = <i64 as Extract<i64>>::extract(Some(&node), source, 0, None);
    assert_eq!(result, -999);
}

#[test]
fn test_extract_i32_from_none_returns_default() {
    let result = <i32 as Extract<i32>>::extract(None, b"", 0, None);
    assert_eq!(result, 0);
}

#[test]
#[should_panic(expected = "Invalid span")]
fn test_spanned_index_out_of_bounds_panics() {
    let source = "hi";
    let s = Spanned {
        value: (),
        span: (0, 10),
    };
    let _ = &source[s];
}

#[test]
#[should_panic(expected = "Invalid span")]
fn test_spanned_index_reversed_span_panics() {
    let source = "hello";
    let s = Spanned {
        value: (),
        span: (4, 2),
    };
    let _ = &source[s];
}

#[test]
fn test_parsed_node_utf8_text_out_of_bounds_is_err() {
    let source = b"hi";
    let node = leaf_node(1, 0, 100);
    assert!(node.utf8_text(source).is_err());
}

#[test]
fn test_walker_single_child() {
    let node = parent_node(0, vec![leaf_node(9, 0, 1)]);
    let mut walker = node.walk();
    assert!(walker.goto_first_child());
    assert_eq!(walker.node().symbol(), 9);
    assert!(!walker.goto_next_sibling());
}

#[test]
fn test_parsed_node_kind_fallback_without_language() {
    // Without a language pointer, kind() uses hardcoded fallback names.
    let node = leaf_node(0, 0, 0);
    assert_eq!(node.kind(), "end");
}

#[test]
fn test_collect_parsing_errors_no_errors() {
    let node = leaf_node(1, 0, 3);
    let mut errors = vec![];
    adze::errors::collect_parsing_errors(&node, b"abc", &mut errors);
    assert!(errors.is_empty());
}

#[test]
fn test_symbol_id_is_u16() {
    assert_eq!(
        std::mem::size_of::<adze::SymbolId>(),
        std::mem::size_of::<u16>()
    );
}

#[test]
fn test_point_default() {
    let p = Point::default();
    assert_eq!(p.row, 0);
    assert_eq!(p.column, 0);
}

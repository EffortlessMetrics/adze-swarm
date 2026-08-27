//! Comprehensive tests for the Extract trait, ParsedNode, Point, Spanned,
//! error types, arena Node, and trait-bound checks.

use std::mem::MaybeUninit;

use adze::errors::{ParseError, ParseErrorReason};
use adze::pure_parser::{ParsedNode, Point};
use adze::{Extract, SpanError, SpanErrorReason, Spanned, WithLeaf};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn pt(row: u32, col: u32) -> Point {
    Point { row, column: col }
}

/// Build a `ParsedNode` via `MaybeUninit` to bypass the `pub(crate)` `language` field.
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
    // SAFETY: All public fields are written; the private `language` field is
    // zeroed (Option<*const _> all-zeros == None).
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

fn anon_leaf(symbol: u16, start: usize, end: usize) -> ParsedNode {
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
        false,
        None,
    )
}

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

fn error_node(start: usize, end: usize) -> ParsedNode {
    make_node(
        0,
        vec![],
        start,
        end,
        pt(0, start as u32),
        pt(0, end as u32),
        false,
        true,
        false,
        false,
        None,
    )
}

fn missing_node(symbol: u16, pos: usize) -> ParsedNode {
    make_node(
        symbol,
        vec![],
        pos,
        pos,
        pt(0, pos as u32),
        pt(0, pos as u32),
        false,
        false,
        true,
        true,
        None,
    )
}

fn extra_node(start: usize, end: usize) -> ParsedNode {
    make_node(
        0,
        vec![],
        start,
        end,
        pt(0, start as u32),
        pt(0, end as u32),
        true,
        false,
        false,
        false,
        None,
    )
}

// ===========================================================================
// 1. Node kind returns expected string (fallback, no language)
// ===========================================================================

#[test]
fn test_kind_symbol_0_returns_end() {
    let node = leaf_node(0, 0, 1);
    assert_eq!(node.kind(), "end");
}

#[test]
fn test_kind_symbol_5_returns_expression() {
    let node = leaf_node(5, 0, 1);
    assert_eq!(node.kind(), "Expression");
}

#[test]
fn test_kind_unknown_symbol_returns_unknown() {
    let node = leaf_node(999, 0, 1);
    assert_eq!(node.kind(), "unknown");
}

// ===========================================================================
// 2. Node text returns expected content
// ===========================================================================

#[test]
fn test_utf8_text_basic() {
    let source = b"hello world";
    let node = leaf_node(1, 0, 5);
    assert_eq!(node.utf8_text(source).unwrap(), "hello");
}

#[test]
fn test_utf8_text_mid_source() {
    let source = b"hello world";
    let node = leaf_node(1, 6, 11);
    assert_eq!(node.utf8_text(source).unwrap(), "world");
}

#[test]
fn test_utf8_text_empty_range() {
    let source = b"abc";
    let node = leaf_node(1, 2, 2);
    assert_eq!(node.utf8_text(source).unwrap(), "");
}

#[test]
fn test_utf8_text_entire_source() {
    let source = b"foobar";
    let node = leaf_node(1, 0, 6);
    assert_eq!(node.utf8_text(source).unwrap(), "foobar");
}

#[test]
fn test_utf8_text_out_of_bounds_returns_error() {
    let source = b"hi";
    let node = leaf_node(1, 0, 100);
    assert!(node.utf8_text(source).is_err());
}

// ===========================================================================
// 3. Root node is named
// ===========================================================================

#[test]
fn test_named_leaf_is_named() {
    let node = leaf_node(1, 0, 3);
    assert!(node.is_named());
}

#[test]
fn test_named_parent_is_named() {
    let node = parent_node(10, vec![leaf_node(1, 0, 3)]);
    assert!(node.is_named());
}

// ===========================================================================
// 4. Leaf node detection
// ===========================================================================

#[test]
fn test_leaf_has_zero_children() {
    let node = leaf_node(1, 0, 3);
    assert_eq!(node.child_count(), 0);
}

// ===========================================================================
// 5. Branch node has children
// ===========================================================================

#[test]
fn test_branch_has_children() {
    let node = parent_node(10, vec![leaf_node(1, 0, 3)]);
    assert_eq!(node.child_count(), 1);
}

// ===========================================================================
// 6. Child count matches children
// ===========================================================================

#[test]
fn test_child_count_two() {
    let node = parent_node(10, vec![leaf_node(1, 0, 2), leaf_node(2, 3, 5)]);
    assert_eq!(node.child_count(), 2);
}

#[test]
fn test_child_count_zero_for_leaf() {
    let node = leaf_node(1, 0, 1);
    assert_eq!(node.child_count(), 0);
}

// ===========================================================================
// 7. Named child count ≤ child_count (via children filtering)
// ===========================================================================

#[test]
fn test_named_children_subset_of_all() {
    let node = parent_node(
        10,
        vec![leaf_node(1, 0, 2), anon_leaf(2, 2, 3), leaf_node(3, 3, 5)],
    );
    let named_count = node.children().iter().filter(|c| c.is_named()).count();
    assert!(named_count <= node.child_count());
    assert_eq!(named_count, 2);
}

#[test]
fn test_all_anon_children_named_count_zero() {
    let node = parent_node(10, vec![anon_leaf(1, 0, 1), anon_leaf(2, 1, 2)]);
    let named_count = node.children().iter().filter(|c| c.is_named()).count();
    assert_eq!(named_count, 0);
}

// ===========================================================================
// 8. child(out_of_bounds) → None
// ===========================================================================

#[test]
fn test_child_out_of_bounds_returns_none() {
    let node = leaf_node(1, 0, 3);
    assert!(node.child(0).is_none());
}

#[test]
fn test_child_index_past_end_returns_none() {
    let node = parent_node(10, vec![leaf_node(1, 0, 2)]);
    assert!(node.child(1).is_none());
}

#[test]
fn test_child_large_index_returns_none() {
    let node = parent_node(10, vec![leaf_node(1, 0, 2)]);
    assert!(node.child(usize::MAX).is_none());
}

// ===========================================================================
// 9. named_child(out_of_bounds) → None (via manual filtering)
// ===========================================================================

#[test]
fn test_named_child_manual_filter_oob() {
    let node = parent_node(10, vec![leaf_node(1, 0, 2)]);
    let named: Vec<_> = node.children().iter().filter(|c| c.is_named()).collect();
    assert!(named.get(5).is_none());
}

// ===========================================================================
// 10. Node Debug format
// ===========================================================================

#[test]
fn test_parsed_node_debug_contains_symbol() {
    let node = leaf_node(42, 0, 5);
    let dbg = format!("{node:?}");
    assert!(dbg.contains("42"), "Debug should contain symbol: {dbg}");
}

#[test]
fn test_parsed_node_debug_contains_start_byte() {
    let node = leaf_node(1, 10, 20);
    let dbg = format!("{node:?}");
    assert!(dbg.contains("10"), "Debug should contain start_byte: {dbg}");
}

#[test]
fn test_point_debug() {
    let p = pt(3, 7);
    let dbg = format!("{p:?}");
    assert!(dbg.contains("3"));
    assert!(dbg.contains("7"));
}

// ===========================================================================
// 11. Node Clone
// ===========================================================================

#[test]
fn test_parsed_node_clone_is_equal() {
    let node = leaf_node(5, 0, 10);
    let cloned = node.clone();
    assert_eq!(cloned.symbol, node.symbol);
    assert_eq!(cloned.start_byte, node.start_byte);
    assert_eq!(cloned.end_byte, node.end_byte);
    assert_eq!(cloned.is_named, node.is_named);
}

#[test]
fn test_parsed_node_clone_with_children() {
    let node = parent_node(10, vec![leaf_node(1, 0, 2), leaf_node(2, 3, 5)]);
    let cloned = node.clone();
    assert_eq!(cloned.child_count(), 2);
    assert_eq!(cloned.child(0).unwrap().symbol, 1);
}

// ===========================================================================
// 12. Point PartialEq and Default
// ===========================================================================

#[test]
fn test_point_partial_eq() {
    assert_eq!(pt(0, 0), pt(0, 0));
    assert_ne!(pt(0, 0), pt(0, 1));
    assert_ne!(pt(1, 0), pt(0, 0));
}

#[test]
fn test_point_default_is_zero() {
    let p = Point::default();
    assert_eq!(p.row, 0);
    assert_eq!(p.column, 0);
}

#[test]
fn test_point_copy() {
    let p = pt(5, 10);
    let q = p; // Copy
    assert_eq!(p, q);
}

// ===========================================================================
// 13. ParseErrorReason variants
// ===========================================================================

#[test]
fn test_parse_error_reason_unexpected_token() {
    let reason = ParseErrorReason::UnexpectedToken("foo".to_string());
    let dbg = format!("{reason:?}");
    assert!(dbg.contains("UnexpectedToken"));
    assert!(dbg.contains("foo"));
}

#[test]
fn test_parse_error_reason_failed_node() {
    let reason = ParseErrorReason::FailedNode(vec![]);
    let dbg = format!("{reason:?}");
    assert!(dbg.contains("FailedNode"));
}

#[test]
fn test_parse_error_reason_missing_token() {
    let reason = ParseErrorReason::MissingToken("semicolon".to_string());
    let dbg = format!("{reason:?}");
    assert!(dbg.contains("MissingToken"));
    assert!(dbg.contains("semicolon"));
}

// ===========================================================================
// 14. ParseError Debug format
// ===========================================================================

#[test]
fn test_parse_error_debug() {
    let err = ParseError {
        reason: ParseErrorReason::UnexpectedToken("!".to_string()),
        start: 0,
        end: 1,
        expected: vec![],
    };
    let dbg = format!("{err:?}");
    assert!(dbg.contains("ParseError"));
    assert!(dbg.contains("start"));
}

#[test]
fn test_parse_error_fields_accessible() {
    let err = ParseError {
        reason: ParseErrorReason::MissingToken(";".to_string()),
        start: 10,
        end: 10,
        expected: vec![],
    };
    assert_eq!(err.start, 10);
    assert_eq!(err.end, 10);
}

// ===========================================================================
// 15. Pure parser ParseError
// ===========================================================================

#[test]
fn test_pure_parse_error_debug() {
    let err = adze::pure_parser::ParseError {
        position: 5,
        point: pt(0, 5),
        expected: vec![1, 2],
        found: 3,
    };
    let dbg = format!("{err:?}");
    assert!(dbg.contains("position"));
    assert!(dbg.contains("5"));
}

#[test]
fn test_pure_parse_error_clone() {
    let err = adze::pure_parser::ParseError {
        position: 0,
        point: pt(0, 0),
        expected: vec![1],
        found: 2,
    };
    let cloned = err.clone();
    assert_eq!(cloned.position, 0);
    assert_eq!(cloned.found, 2);
}

// ===========================================================================
// 16. Multiple children accessible
// ===========================================================================

#[test]
fn test_multiple_children_indexed() {
    let node = parent_node(
        10,
        vec![leaf_node(1, 0, 2), leaf_node(2, 3, 5), leaf_node(3, 6, 8)],
    );
    assert_eq!(node.child(0).unwrap().symbol, 1);
    assert_eq!(node.child(1).unwrap().symbol, 2);
    assert_eq!(node.child(2).unwrap().symbol, 3);
}

#[test]
fn test_children_slice() {
    let node = parent_node(10, vec![leaf_node(1, 0, 1), leaf_node(2, 1, 2)]);
    let children = node.children();
    assert_eq!(children.len(), 2);
    assert_eq!(children[0].symbol, 1);
    assert_eq!(children[1].symbol, 2);
}

// ===========================================================================
// 17. Deep tree navigation
// ===========================================================================

#[test]
fn test_deep_tree_navigation() {
    let inner = parent_node(2, vec![leaf_node(3, 0, 1)]);
    let outer = parent_node(1, vec![inner]);
    let deep = outer.child(0).unwrap().child(0).unwrap();
    assert_eq!(deep.symbol, 3);
}

#[test]
fn test_three_level_deep_navigation() {
    let l3 = leaf_node(4, 0, 1);
    let l2 = parent_node(3, vec![l3]);
    let l1 = parent_node(2, vec![l2]);
    let root = parent_node(1, vec![l1]);
    let leaf = root.child(0).unwrap().child(0).unwrap().child(0).unwrap();
    assert_eq!(leaf.symbol, 4);
    assert_eq!(leaf.child_count(), 0);
}

// ===========================================================================
// 18. Error node detection
// ===========================================================================

#[test]
fn test_error_node_is_error() {
    let node = error_node(0, 3);
    assert!(node.is_error());
}

#[test]
fn test_error_node_has_error() {
    let node = error_node(0, 3);
    assert!(node.has_error());
}

#[test]
fn test_parent_has_error_if_child_error() {
    let child = error_node(0, 2);
    let node = parent_node(10, vec![child]);
    assert!(node.has_error());
}

#[test]
fn test_no_error_returns_false() {
    let node = leaf_node(1, 0, 3);
    assert!(!node.is_error());
    assert!(!node.has_error());
}

// ===========================================================================
// 19. Missing node detection
// ===========================================================================

#[test]
fn test_missing_node_is_missing() {
    let node = missing_node(1, 5);
    assert!(node.is_missing());
}

#[test]
fn test_non_missing_node() {
    let node = leaf_node(1, 0, 3);
    assert!(!node.is_missing());
}

// ===========================================================================
// 20. Extra node detection
// ===========================================================================

#[test]
fn test_extra_node_is_extra() {
    let node = extra_node(0, 1);
    assert!(node.is_extra());
}

#[test]
fn test_non_extra_node() {
    let node = leaf_node(1, 0, 3);
    assert!(!node.is_extra());
}

// ===========================================================================
// 21. Node iteration via children slice
// ===========================================================================

#[test]
fn test_children_iteration_count() {
    let node = parent_node(
        10,
        vec![leaf_node(1, 0, 1), leaf_node(2, 1, 2), leaf_node(3, 2, 3)],
    );
    assert_eq!(node.children().iter().count(), 3);
}

#[test]
fn test_children_symbols_collected() {
    let node = parent_node(10, vec![leaf_node(7, 0, 1), leaf_node(8, 1, 2)]);
    let symbols: Vec<u16> = node.children().iter().map(|c| c.symbol).collect();
    assert_eq!(symbols, vec![7, 8]);
}

// ===========================================================================
// 22. ChildWalker traversal
// ===========================================================================

#[test]
fn test_child_walker_basic() {
    let node = parent_node(10, vec![leaf_node(1, 0, 2), leaf_node(2, 3, 5)]);
    let mut walker = node.walk();
    assert!(walker.goto_first_child());
    assert_eq!(walker.node().symbol, 1);
    assert!(walker.goto_next_sibling());
    assert_eq!(walker.node().symbol, 2);
    assert!(!walker.goto_next_sibling());
}

#[test]
fn test_child_walker_empty() {
    let node = leaf_node(1, 0, 1);
    let mut walker = node.walk();
    assert!(!walker.goto_first_child());
}

// ===========================================================================
// 23–30. Extract trait properties
// ===========================================================================

#[test]
fn test_extract_is_open_to_downstream_types() {
    // A type declared outside `adze` satisfies `Extract` with no marker impl.
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
fn test_extract_has_conflicts_default_false() {
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
    fn check<T: Extract<String, LeafFn = ()>>() {}
    check::<String>();
}

#[test]
fn test_extract_has_no_sealing_supertrait() {
    fn _check<T: Extract<U>, U>() {}
}

// ===========================================================================
// 31–40. Extract<String> behaviour
// ===========================================================================

#[test]
fn test_extract_string_from_node() {
    let source = b"hello world";
    let node = leaf_node(1, 0, 5);
    let result = String::extract(Some(&node), source, 0, None);
    assert_eq!(result, "hello");
}

#[test]
fn test_extract_string_from_none_returns_empty() {
    let result = String::extract(None, b"hello", 0, None);
    assert_eq!(result, "");
}

#[test]
fn test_extract_string_entire_source() {
    let source = b"foobar";
    let node = leaf_node(1, 0, 6);
    let result = String::extract(Some(&node), source, 0, None);
    assert_eq!(result, "foobar");
}

#[test]
fn test_extract_string_empty_range() {
    let source = b"abc";
    let node = leaf_node(1, 1, 1);
    let result = String::extract(Some(&node), source, 0, None);
    assert_eq!(result, "");
}

#[test]
fn test_extract_string_unicode() {
    let source = "héllo".as_bytes();
    let len = source.len();
    let node = leaf_node(1, 0, len);
    let result = String::extract(Some(&node), source, 0, None);
    assert_eq!(result, "héllo");
}

// ===========================================================================
// 41–45. Extract<()> (unit)
// ===========================================================================

#[test]
fn test_extract_unit_from_some() {
    let node = leaf_node(1, 0, 3);
    <() as Extract<()>>::extract(Some(&node), b"abc", 0, None);
}

#[test]
fn test_extract_unit_from_none() {
    <() as Extract<()>>::extract(None, b"", 0, None);
}

// ===========================================================================
// 46–50. Extract<Option<T>>
// ===========================================================================

#[test]
fn test_extract_option_some() {
    let source = b"yes";
    let node = leaf_node(1, 0, 3);
    let result = <Option<String> as Extract<Option<String>>>::extract(Some(&node), source, 0, None);
    assert_eq!(result, Some("yes".to_string()));
}

#[test]
fn test_extract_option_none() {
    let result = <Option<String> as Extract<Option<String>>>::extract(None, b"", 0, None);
    assert!(result.is_none());
}

// ===========================================================================
// 51–55. Extract<Box<T>>
// ===========================================================================

#[test]
fn test_extract_box_from_node() {
    let source = b"boxed";
    let node = leaf_node(1, 0, 5);
    let result = <Box<String> as Extract<Box<String>>>::extract(Some(&node), source, 0, None);
    assert_eq!(*result, "boxed");
}

#[test]
fn test_extract_box_from_none() {
    let result = <Box<String> as Extract<Box<String>>>::extract(None, b"", 0, None);
    assert_eq!(*result, "");
}

// ===========================================================================
// 56–60. Extract<Vec<T>>
// ===========================================================================

#[test]
fn test_extract_vec_from_none_returns_empty() {
    let result = <Vec<String> as Extract<Vec<String>>>::extract(None, b"", 0, None);
    assert!(result.is_empty());
}

#[test]
fn test_extract_vec_with_single_child() {
    let child = leaf_node(1, 0, 3);
    let node = parent_node(10, vec![child]);
    let source = b"abc";
    let result = <Vec<String> as Extract<Vec<String>>>::extract(Some(&node), source, 0, None);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], "abc");
}

// ===========================================================================
// 61–65. Primitive extraction
// ===========================================================================

#[test]
fn test_extract_i32_from_node() {
    let source = b"42";
    let node = leaf_node(1, 0, 2);
    let result = i32::extract(Some(&node), source, 0, None);
    assert_eq!(result, 42);
}

#[test]
fn test_extract_i32_from_none_returns_default() {
    let result = i32::extract(None, b"", 0, None);
    assert_eq!(result, 0);
}

#[test]
fn test_extract_f64_from_node() {
    let source = b"4.25";
    let node = leaf_node(1, 0, 4);
    let result = f64::extract(Some(&node), source, 0, None);
    assert!((result - 4.25).abs() < f64::EPSILON * 100.0);
}

#[test]
fn test_extract_bool_from_node() {
    let source = b"true";
    let node = leaf_node(1, 0, 4);
    let result = bool::extract(Some(&node), source, 0, None);
    assert!(result);
}

#[test]
fn test_extract_u64_from_none() {
    let result = u64::extract(None, b"", 0, None);
    assert_eq!(result, 0);
}

// ===========================================================================
// 66–70. Spanned type
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
fn test_spanned_clone() {
    let s = Spanned {
        value: "hi".to_string(),
        span: (0, 2),
    };
    let c = s.clone();
    assert_eq!(c.value, "hi");
    assert_eq!(c.span, (0, 2));
}

#[test]
fn test_spanned_debug() {
    let s = Spanned {
        value: 1,
        span: (0, 1),
    };
    let dbg = format!("{s:?}");
    assert!(dbg.contains("Spanned"));
    assert!(dbg.contains("value"));
    assert!(dbg.contains("span"));
}

#[test]
fn test_spanned_extract_from_node() {
    let source = b"test";
    let node = leaf_node(1, 0, 4);
    let result =
        <Spanned<String> as Extract<Spanned<String>>>::extract(Some(&node), source, 0, None);
    assert_eq!(result.value, "test");
    assert_eq!(result.span, (0, 4));
}

// ===========================================================================
// 71–75. SpanError and SpanErrorReason
// ===========================================================================

#[test]
fn test_span_error_start_greater_than_end() {
    let err = SpanError {
        span: (5, 3),
        source_len: 10,
        reason: SpanErrorReason::StartGreaterThanEnd,
    };
    assert_eq!(err.reason, SpanErrorReason::StartGreaterThanEnd);
    let msg = err.to_string();
    assert!(msg.contains("start"));
    assert!(msg.contains("end"));
}

#[test]
fn test_span_error_start_out_of_bounds() {
    let err = SpanError {
        span: (11, 12),
        source_len: 10,
        reason: SpanErrorReason::StartOutOfBounds,
    };
    let msg = err.to_string();
    assert!(msg.contains("source length"));
}

#[test]
fn test_span_error_end_out_of_bounds() {
    let err = SpanError {
        span: (0, 20),
        source_len: 10,
        reason: SpanErrorReason::EndOutOfBounds,
    };
    let msg = err.to_string();
    assert!(msg.contains("end"));
    assert!(msg.contains("source length"));
}

#[test]
fn test_span_error_partial_eq() {
    let a = SpanErrorReason::StartGreaterThanEnd;
    let b = SpanErrorReason::StartGreaterThanEnd;
    assert_eq!(a, b);
    assert_ne!(
        SpanErrorReason::StartOutOfBounds,
        SpanErrorReason::EndOutOfBounds
    );
}

#[test]
fn test_span_error_is_std_error() {
    fn assert_error<T: std::error::Error>() {}
    assert_error::<SpanError>();
}

// ===========================================================================
// 76–78. WithLeaf extraction
// ===========================================================================

#[test]
fn test_with_leaf_extract_with_transform() {
    let source = b"42";
    let node = leaf_node(1, 0, 2);
    let transform: &dyn Fn(&str) -> i64 = &|s: &str| s.parse::<i64>().unwrap_or(0);
    let result = <WithLeaf<i64> as Extract<i64>>::extract(Some(&node), source, 0, Some(transform));
    assert_eq!(result, 42);
}

#[test]
#[should_panic(expected = "Leaf extraction failed")]
fn test_with_leaf_extract_no_transform_panics() {
    let source = b"x";
    let node = leaf_node(1, 0, 1);
    let _: i64 = <WithLeaf<i64> as Extract<i64>>::extract(Some(&node), source, 0, None);
}

#[test]
fn test_with_leaf_extract_from_none_with_transform() {
    let transform: &dyn Fn(&str) -> String = &|s: &str| format!("got:{s}");
    let result = <WithLeaf<String> as Extract<String>>::extract(None, b"", 0, Some(transform));
    assert_eq!(result, "got:");
}

// ===========================================================================
// 79–82. Trait bound compile-time checks
// ===========================================================================

#[test]
fn test_parsed_node_is_clone() {
    fn assert_clone<T: Clone>() {}
    assert_clone::<ParsedNode>();
}

#[test]
fn test_parsed_node_is_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<ParsedNode>();
}

#[test]
fn test_point_is_copy_clone_eq() {
    fn assert_bounds<T: Copy + Clone + PartialEq + Eq + Default + std::fmt::Debug>() {}
    assert_bounds::<Point>();
}

#[test]
fn test_span_error_reason_is_clone_debug_eq() {
    fn assert_bounds<T: Clone + std::fmt::Debug + PartialEq + Eq>() {}
    assert_bounds::<SpanErrorReason>();
}

// ===========================================================================
// 83–86. Byte range and point accessors
// ===========================================================================

#[test]
fn test_start_byte_accessor() {
    let node = leaf_node(1, 10, 20);
    assert_eq!(node.start_byte(), 10);
}

#[test]
fn test_end_byte_accessor() {
    let node = leaf_node(1, 10, 20);
    assert_eq!(node.end_byte(), 20);
}

#[test]
fn test_start_point_accessor() {
    let node = make_node(
        1,
        vec![],
        0,
        5,
        pt(2, 3),
        pt(2, 8),
        false,
        false,
        false,
        true,
        None,
    );
    assert_eq!(node.start_point(), pt(2, 3));
}

#[test]
fn test_end_point_accessor() {
    let node = make_node(
        1,
        vec![],
        0,
        5,
        pt(2, 3),
        pt(2, 8),
        false,
        false,
        false,
        true,
        None,
    );
    assert_eq!(node.end_point(), pt(2, 8));
}

// ===========================================================================
// 87–90. Field ID
// ===========================================================================

#[test]
fn test_field_id_none_by_default() {
    let node = leaf_node(1, 0, 3);
    assert!(node.field_id.is_none());
}

#[test]
fn test_field_id_some_when_set() {
    let node = make_node(
        1,
        vec![],
        0,
        3,
        pt(0, 0),
        pt(0, 3),
        false,
        false,
        false,
        true,
        Some(5),
    );
    assert_eq!(node.field_id, Some(5));
}

// ===========================================================================
// 91–93. Anonymous vs Named nodes
// ===========================================================================

#[test]
fn test_anonymous_node_is_not_named() {
    let node = anon_leaf(1, 0, 1);
    assert!(!node.is_named());
}

#[test]
fn test_named_node_is_named() {
    let node = leaf_node(1, 0, 1);
    assert!(node.is_named());
}

#[test]
fn test_mixed_named_anonymous_children() {
    let node = parent_node(
        10,
        vec![
            leaf_node(1, 0, 1), // named
            anon_leaf(2, 1, 2), // anonymous
            leaf_node(3, 2, 3), // named
            anon_leaf(4, 3, 4), // anonymous
        ],
    );
    let named: Vec<_> = node.children().iter().filter(|c| c.is_named()).collect();
    let anon: Vec<_> = node.children().iter().filter(|c| !c.is_named()).collect();
    assert_eq!(named.len(), 2);
    assert_eq!(anon.len(), 2);
}

// ===========================================================================
// 94–96. Extract trait associated constants
// ===========================================================================

#[test]
fn test_grammar_name_default_is_unknown() {
    assert_eq!(<String as Extract<String>>::GRAMMAR_NAME, "unknown");
}

#[test]
fn test_grammar_json_default_is_empty_object() {
    assert_eq!(<String as Extract<String>>::GRAMMAR_JSON, "{}");
}

#[test]
fn test_unit_grammar_name_default() {
    assert_eq!(<() as Extract<()>>::GRAMMAR_NAME, "unknown");
}

// ===========================================================================
// 97. ParseResult
// ===========================================================================

#[test]
fn test_parse_result_root_none() {
    let result = adze::pure_parser::ParseResult {
        root: None,
        errors: vec![],
    };
    assert!(result.root.is_none());
    assert!(result.errors.is_empty());
}

#[test]
fn test_parse_result_root_some() {
    let node = leaf_node(1, 0, 3);
    let result = adze::pure_parser::ParseResult {
        root: Some(node),
        errors: vec![],
    };
    assert!(result.root.is_some());
    assert_eq!(result.root.unwrap().symbol, 1);
}

// ===========================================================================
// 98–100. Edge cases
// ===========================================================================

#[test]
fn test_extract_string_non_utf8_returns_empty() {
    let source: &[u8] = &[0xFF, 0xFE, 0xFD];
    let node = leaf_node(1, 0, 3);
    let result = String::extract(Some(&node), source, 0, None);
    assert_eq!(result, "");
}

#[test]
fn test_node_with_empty_children_vec() {
    let node = parent_node(10, vec![]);
    assert_eq!(node.child_count(), 0);
    assert!(node.child(0).is_none());
}

#[test]
fn test_spanned_extract_from_none_uses_last_idx() {
    let result = <Spanned<String> as Extract<Spanned<String>>>::extract(None, b"", 42, None);
    assert_eq!(result.span, (42, 42));
    assert_eq!(result.value, "");
}

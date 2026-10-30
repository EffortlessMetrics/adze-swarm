//! Exact container selection for repetition extraction (#1062).
//!
//! A delimited single-element `vec_contents` node carries two children: the
//! fielded element token and an anonymous, field-less `_repeat_aux_*`
//! sentinel for the (empty) repetition tail. Pure-Rust `Vec` extraction must
//! select only the fielded/named children — mirroring the tree-sitter route's
//! `field_name().is_some() || n.is_named()` filter — instead of feeding the
//! aux sentinel's empty text (or a delimiter/extra sibling) to the element
//! transform.

use adze::pure_parser::{ParsedNode, Point};
use adze::{Extract, Spanned, WithLeaf};
use std::mem::MaybeUninit;

fn pt(row: u32, col: u32) -> Point {
    Point { row, column: col }
}

fn make_node_full(
    symbol: u16,
    children: Vec<ParsedNode>,
    start: usize,
    end: usize,
    is_named: bool,
    is_extra: bool,
    field_id: Option<u16>,
) -> ParsedNode {
    let mut uninit = MaybeUninit::<ParsedNode>::uninit();
    let ptr = uninit.as_mut_ptr();
    unsafe {
        std::ptr::write_bytes(ptr, 0, 1);
        std::ptr::addr_of_mut!((*ptr).symbol).write(symbol);
        std::ptr::addr_of_mut!((*ptr).children).write(children);
        std::ptr::addr_of_mut!((*ptr).start_byte).write(start);
        std::ptr::addr_of_mut!((*ptr).end_byte).write(end);
        std::ptr::addr_of_mut!((*ptr).start_point).write(pt(0, start as u32));
        std::ptr::addr_of_mut!((*ptr).end_point).write(pt(0, end as u32));
        std::ptr::addr_of_mut!((*ptr).is_extra).write(is_extra);
        std::ptr::addr_of_mut!((*ptr).is_error).write(false);
        std::ptr::addr_of_mut!((*ptr).is_missing).write(false);
        std::ptr::addr_of_mut!((*ptr).is_named).write(is_named);
        std::ptr::addr_of_mut!((*ptr).field_id).write(field_id);
        uninit.assume_init()
    }
}

/// Anonymous element token carrying the `..._vec_element` field, as emitted
/// for `#[adze::leaf]` items inside a `Vec` field.
fn element(symbol: u16, start: usize, end: usize, field: u16) -> ParsedNode {
    make_node_full(symbol, vec![], start, end, false, false, Some(field))
}

/// Anonymous, field-less `_repeat_aux_*` sentinel with an empty span.
fn aux(symbol: u16, pos: usize) -> ParsedNode {
    make_node_full(symbol, vec![], pos, pos, false, false, None)
}

/// `vec_contents` branch node.
fn contents(symbol: u16, children: Vec<ParsedNode>, start: usize, end: usize) -> ParsedNode {
    make_node_full(symbol, children, start, end, true, false, None)
}

#[test]
fn vec_extract_selects_fielded_element_skips_aux() {
    // Shape of a delimited singleton: `vec_contents("1")` with children
    // `[element(0..1), _repeat_aux(1..1)]`.
    let source = b"1";
    let node = contents(4, vec![element(8, 0, 1, 1), aux(10, 1)], 0, 1);
    let val = <Vec<String> as Extract<Vec<String>>>::extract(Some(&node), source, 0, None);
    assert_eq!(val, vec!["1".to_string()]);
}

#[test]
fn vec_try_extract_selects_fielded_element_skips_aux() {
    let source = b"1";
    let node = contents(4, vec![element(8, 0, 1, 1), aux(10, 1)], 0, 1);
    let val = <Vec<String> as Extract<Vec<String>>>::try_extract(Some(&node), source, 0, None)
        .expect("fallible repeat extraction succeeds");
    assert_eq!(val, vec!["1".to_string()]);
}

#[test]
fn vec_leaf_transform_receives_element_text_not_aux_empty() {
    // Unit-level repro of the `snapshot_repetitions_single` panic class:
    // the aux sentinel's empty text must never reach the element transform.
    let source = b"1";
    let node = contents(4, vec![element(8, 0, 1, 1), aux(10, 1)], 0, 1);
    let transform: Box<dyn Fn(&str) -> i32> =
        Box::new(|s: &str| s.parse::<i32>().expect("element text is numeric"));
    let val = <Vec<WithLeaf<i32>> as Extract<Vec<i32>>>::extract(
        Some(&node),
        source,
        0,
        Some(transform.as_ref()),
    );
    assert_eq!(val, vec![1]);
}

#[test]
fn vec_option_extract_has_no_phantom_some_from_aux() {
    let source = b"1";
    let node = contents(4, vec![element(8, 0, 1, 1), aux(10, 1)], 0, 1);
    let val = <Vec<Option<String>> as Extract<Vec<Option<String>>>>::extract(
        Some(&node),
        source,
        0,
        None,
    );
    assert_eq!(val, vec![Some("1".to_string())]);
}

#[test]
fn vec_extract_skips_fieldless_extra_sibling() {
    // A trailing-whitespace extra attached as a sibling of the element is
    // not an element either. Whitespace tokens are named, so the extra
    // check must come before the named check.
    let source = b"1 ";
    let ws = make_node_full(12, vec![], 1, 2, true, true, None);
    let node = contents(4, vec![element(8, 0, 1, 1), ws], 0, 2);
    let val = <Vec<String> as Extract<Vec<String>>>::extract(Some(&node), source, 0, None);
    assert_eq!(val, vec!["1".to_string()]);
}

#[test]
fn vec_extract_untagged_element_with_extra_child() {
    // Exact middle-element shape of undelimited `"1 2 3"`: the element slot
    // is untagged but the token carries only a trailing-whitespace extra, so
    // it extracts whole (leaf text trims extras).
    let source = b"2 ";
    let ws = make_node_full(8, vec![], 1, 2, true, true, None);
    let elem = make_node_full(7, vec![ws], 0, 2, false, false, None);
    let node = contents(3, vec![elem], 0, 2);
    let val = <Vec<String> as Extract<Vec<String>>>::extract(Some(&node), source, 0, None);
    assert_eq!(val, vec!["2".to_string()]);
}

#[test]
fn vec_extract_named_extra_inside_tail_is_skipped() {
    // The delimiter's trailing whitespace is named AND extra: strict tail
    // selection must skip it rather than feed `" "` to the element
    // transform (regression: `ParseIntError { kind: InvalidDigit }` panic on
    // `"1 2 3 4 5"` and `"1, 2"`).
    let source = b"1, 2";
    let elem1 = element(8, 0, 1, 0);
    let ws = make_node_full(12, vec![], 2, 3, true, true, None);
    let comma = make_node_full(9, vec![ws], 1, 3, false, false, None);
    let elem2 = element(8, 3, 4, 0);
    let tail = make_node_full(11, vec![comma, elem2], 1, 4, false, false, None);
    let node = make_node_full(4, vec![elem1, tail], 0, 4, true, false, Some(2));
    let transform: Box<dyn Fn(&str) -> i32> =
        Box::new(|s: &str| s.parse::<i32>().expect("element text is numeric"));
    let val = <Vec<WithLeaf<i32>> as Extract<Vec<i32>>>::extract(
        Some(&node),
        source,
        0,
        Some(transform.as_ref()),
    );
    assert_eq!(val, vec![1, 2]);
}

#[test]
fn vec_extract_keeps_named_fieldless_child() {
    // Parity with the tree-sitter route: named children without a field
    // are still selected.
    let source = b"ab";
    let named_a = make_node_full(2, vec![], 0, 1, true, false, None);
    let named_b = make_node_full(2, vec![], 1, 2, true, false, None);
    let node = contents(4, vec![named_a, named_b], 0, 2);
    let val = <Vec<String> as Extract<Vec<String>>>::extract(Some(&node), source, 0, None);
    assert_eq!(val, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn vec_extract_aux_only_yields_empty() {
    // A contents node holding only the empty aux sentinel selects nothing.
    let source = b"";
    let node = contents(4, vec![aux(10, 0)], 0, 0);
    let val = <Vec<String> as Extract<Vec<String>>>::extract(Some(&node), source, 0, None);
    assert!(val.is_empty());
}

#[test]
fn vec_extract_recursive_repeat_shape_unaffected() {
    // Undelimited right-recursive shape `[prev_contents, element]` still
    // flattens to every element in order.
    let source = b"12";
    let prev = contents(3, vec![element(7, 0, 1, 1)], 0, 1);
    let node = contents(3, vec![prev, element(7, 1, 2, 1)], 0, 2);
    let val = <Vec<String> as Extract<Vec<String>>>::extract(Some(&node), source, 0, None);
    assert_eq!(val, vec!["1".to_string(), "2".to_string()]);
}

#[test]
fn vec_extract_recursive_repeat_untagged_outer_element() {
    // Exact raw shape of undelimited `"1 2"`: the nested base element is
    // fielded but the outer element slot is untagged. Positional selection
    // keeps both, in order.
    let source = b"1 2";
    let ws = make_node_full(8, vec![], 1, 2, true, true, None);
    let inner_elem = make_node_full(7, vec![ws], 0, 2, false, false, Some(0));
    let prev = make_node_full(3, vec![inner_elem], 0, 2, true, false, None);
    let outer_elem = make_node_full(7, vec![], 2, 3, false, false, None);
    let node = make_node_full(3, vec![prev, outer_elem], 0, 3, true, false, Some(2));
    let val = <Vec<String> as Extract<Vec<String>>>::extract(Some(&node), source, 0, None);
    assert_eq!(val, vec!["1".to_string(), "2".to_string()]);
}

#[test]
fn vec_extract_delimited_tail_descends_past_delimiter() {
    // Exact raw shape of delimited `"1, 2"`: the tail container holds the
    // delimiter plus the second fielded element. Strict descent selects the
    // element without feeding `","` to the element transform.
    let source = b"1, 2";
    let elem1 = element(8, 0, 1, 0);
    let comma = make_node_full(9, vec![], 1, 2, false, false, None);
    let elem2 = element(8, 3, 4, 0);
    let tail = make_node_full(11, vec![comma, elem2], 1, 4, false, false, None);
    let node = make_node_full(4, vec![elem1, tail], 0, 4, true, false, Some(2));
    let val = <Vec<String> as Extract<Vec<String>>>::extract(Some(&node), source, 0, None);
    assert_eq!(val, vec!["1".to_string(), "2".to_string()]);
}

#[test]
fn vec_try_extract_delimited_tail_descends_past_delimiter() {
    let source = b"1, 2";
    let elem1 = element(8, 0, 1, 0);
    let comma = make_node_full(9, vec![], 1, 2, false, false, None);
    let elem2 = element(8, 3, 4, 0);
    let tail = make_node_full(11, vec![comma, elem2], 1, 4, false, false, None);
    let node = make_node_full(4, vec![elem1, tail], 0, 4, true, false, Some(2));
    let val = <Vec<String> as Extract<Vec<String>>>::try_extract(Some(&node), source, 0, None)
        .expect("fallible delimited-tail extraction succeeds");
    assert_eq!(val, vec!["1".to_string(), "2".to_string()]);
}

#[test]
fn vec_extract_delimited_tail_with_whitespace_delimiter() {
    // Delimiter carrying a trailing-whitespace extra child still descends to
    // nothing; only the fielded elements are selected.
    let source = b"1, 2";
    let elem1 = element(8, 0, 1, 0);
    let ws = make_node_full(12, vec![], 2, 3, true, true, None);
    let comma = make_node_full(9, vec![ws], 1, 3, false, false, None);
    let elem2 = element(8, 3, 4, 0);
    let tail = make_node_full(11, vec![comma, elem2], 1, 4, false, false, None);
    let node = make_node_full(4, vec![elem1, tail], 0, 4, true, false, Some(2));
    let val = <Vec<String> as Extract<Vec<String>>>::extract(Some(&node), source, 0, None);
    assert_eq!(val, vec!["1".to_string(), "2".to_string()]);
}

#[test]
fn spanned_container_with_only_extra_child_keeps_full_span() {
    // A container whose only child is an extra spans that extra. Trimming
    // it like a token would collapse the reported span to empty.
    let source = b" ";
    let ws = make_node_full(12, vec![], 0, 1, true, true, None);
    let node = contents(4, vec![ws], 0, 1);
    let val = <Spanned<Vec<String>> as Extract<Spanned<Vec<String>>>>::extract(
        Some(&node),
        source,
        0,
        None,
    );
    assert!(val.value.is_empty());
    assert_eq!(val.span, (0, 1));
}

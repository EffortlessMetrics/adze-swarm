//! Independent semantic controls for newly admitted nullable repeat tails.
use adze_example::repetitions::grammar3;

fn values(source: &str) -> Vec<(Option<i32>, (usize, usize))> {
    let parsed = grammar3::parse(source).expect("nullable-tail grammar admits this input");
    assert_eq!(parsed.metadata, 123);
    assert_eq!(parsed.numbers.span, (0, source.len()));
    parsed
        .numbers
        .value
        .into_iter()
        .map(|item| (item.value, item.span))
        .collect()
}

#[test]
fn test_trailing_nullable_element_keeps_none_and_zero_width_span() {
    assert_eq!(values("1,"), vec![(Some(1), (0, 1)), (None, (2, 2))]);
}

#[test]
fn test_middle_nullable_element_keeps_none_and_zero_width_span() {
    assert_eq!(
        values("1,, 2"),
        vec![(Some(1), (0, 1)), (None, (2, 2)), (Some(2), (4, 5))]
    );
}

#[test]
fn test_nullable_tail_tree_preserves_blank_field_edge() {
    let mut parser = adze::pure_parser::Parser::new();
    parser.set_language(grammar3::language()).unwrap();
    let parsed = parser.parse_string("1,, 2");
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let root = parsed.root.as_ref().unwrap();
    fn empty_occurrences(node: &adze::pure_parser::ParsedNode, out: &mut Vec<Option<u16>>) {
        if !node.is_named && node.start_byte == 2 && node.end_byte == 2 {
            out.push(node.field_id);
        }
        for child in &node.children {
            empty_occurrences(child, out);
        }
    }
    let mut occurrences = vec![];
    empty_occurrences(root, &mut occurrences);
    assert_eq!(occurrences.len(), 1);
    assert!(
        occurrences[0].is_some(),
        "blank element retains its field edge"
    );
}

#[test]
fn test_singleton_retains_its_exact_present_value() {
    assert_eq!(values("1"), vec![(Some(1), (0, 1))]);
}

#[test]
fn test_empty_whole_repeat_is_an_empty_vector() {
    assert!(values("").is_empty());
}

#[test]
fn test_named_empty_value_stays_some_and_unfielded_sentinel_stays_empty() {
    use adze::pure_parser::ParsedNode;
    use adze::{Extract, Spanned};
    type Item = Spanned<Option<()>>;
    let present = ParsedNode::builder(2, vec![], 2, 2)
        .is_named(true)
        .field_id(Some(0))
        .build();
    let root = ParsedNode::builder(1, vec![present], 0, 2).build();
    let fallible =
        <Vec<Item> as Extract<Vec<Item>>>::try_extract(Some(&root), b"1,", 0, None).unwrap();
    assert_eq!(fallible.len(), 1);
    assert_eq!(fallible[0].value, Some(()));
    assert_eq!(fallible[0].span, (2, 2));
    let infallible = <Vec<Item> as Extract<Vec<Item>>>::extract(Some(&root), b"1,", 0, None);
    assert_eq!(infallible.len(), 1);
    assert_eq!(infallible[0].value, Some(()));
    assert_eq!(infallible[0].span, (2, 2));

    let sentinel = ParsedNode::builder(3, vec![], 2, 2).is_named(false).build();
    let root = ParsedNode::builder(1, vec![sentinel], 0, 2).build();
    assert!(
        <Vec<Item> as Extract<Vec<Item>>>::try_extract(Some(&root), b"1,", 0, None)
            .unwrap()
            .is_empty()
    );
    assert!(<Vec<Item> as Extract<Vec<Item>>>::extract(Some(&root), b"1,", 0, None).is_empty());

    let absent = ParsedNode::builder(4, vec![], 2, 2)
        .is_named(false)
        .field_id(Some(0))
        .build();
    let delimiter = ParsedNode::builder(5, vec![], 1, 2).is_named(false).build();
    let tail = ParsedNode::builder(6, vec![delimiter, absent], 1, 2)
        .is_named(false)
        .build();
    let root = ParsedNode::builder(1, vec![tail], 0, 2).build();
    let fallible =
        <Vec<Item> as Extract<Vec<Item>>>::try_extract(Some(&root), b"1,", 0, None).unwrap();
    let infallible = <Vec<Item> as Extract<Vec<Item>>>::extract(Some(&root), b"1,", 0, None);
    for result in [fallible, infallible] {
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, None);
        assert_eq!(result[0].span, (2, 2));
    }
}

#[test]
fn test_document_projection_retains_nullable_values_and_spans() {
    for (source, expected) in [
        ("1", vec![(Some(1), (0, 1))]),
        ("1,", vec![(Some(1), (0, 1)), (None, (2, 2))]),
        (
            "1,, 2",
            vec![(Some(1), (0, 1)), (None, (2, 2)), (Some(2), (4, 5))],
        ),
    ] {
        let document = grammar3::parse_document(source).expect("document syntax");
        let parsed: grammar3::NumberList = document.ast().expect("document AST");
        assert_eq!(parsed.metadata, 123);
        assert_eq!(parsed.numbers.span, (0, source.len()));
        let actual: Vec<_> = parsed
            .numbers
            .value
            .into_iter()
            .map(|item| (item.value, item.span))
            .collect();
        assert_eq!(actual, expected, "{source:?}");
    }
}

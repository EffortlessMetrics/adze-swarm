//! Public compatibility entry points share occurrence semantics without source text.
#![cfg(feature = "query")]

use adze::parser_v4::ParseNode;
use adze::query::ast::{Pattern, PatternChild, PatternNode, Predicate, Quantifier, Query};
use adze::query::{QueryCursor, QueryMatch};
use adze_ir::SymbolId;
use std::collections::HashMap;
use std::ops::Range;

fn node(symbol: u16, range: Range<usize>, children: Vec<ParseNode>) -> ParseNode {
    ParseNode {
        symbol: SymbolId(symbol),
        symbol_id: SymbolId(symbol),
        start_byte: range.start,
        end_byte: range.end,
        field_name: None,
        alias_symbol_id: None,
        children,
    }
}

fn query(children: Vec<PatternNode>, predicates: Vec<Predicate>) -> Query {
    let mut root = PatternNode::new(SymbolId(0), true);
    root.children = children.into_iter().map(PatternChild::Node).collect();
    Query {
        source: String::new(),
        patterns: vec![Pattern {
            root,
            predicates,
            start_byte: 0,
        }],
        capture_names: HashMap::new(),
        property_settings: vec![],
        property_predicates: vec![],
    }
}

fn capture(symbol: u16, index: u32) -> PatternNode {
    PatternNode::new(SymbolId(symbol), true).with_capture(index)
}

fn facts(matches: &[QueryMatch]) -> Vec<Vec<(u32, Range<usize>)>> {
    matches
        .iter()
        .map(|m| {
            m.captures
                .iter()
                .map(|c| (c.index, c.node.start_byte..c.node.end_byte))
                .collect()
        })
        .collect()
}

#[test]
fn test_public_cursor_and_legacy_matcher_keep_distinct_equal_text_occurrences() {
    let source = "é é";
    let tree = node(
        0,
        0..source.len(),
        vec![node(1, 0..2, vec![]), node(1, 3..5, vec![])],
    );
    let query = query(vec![capture(1, 0), capture(1, 0)], vec![]);
    let expected = vec![vec![(0, 0..2), (0, 3..5)]];
    let matches = QueryCursor::new().collect_matches(&query, &tree);
    assert_eq!(facts(&matches), expected);
    let texts: Vec<_> = matches[0]
        .captures
        .iter()
        .map(|c| &source[c.node.start_byte..c.node.end_byte])
        .collect();
    assert_eq!(texts, vec!["é", "é"]);
    let direct = adze::query::matcher::QueryMatcher::new(&query).matches(&tree);
    assert_eq!(facts(&direct), expected);
}

#[test]
fn test_public_cursor_orders_capture_ids_then_preserves_occurrence_order() {
    let tree = node(
        0,
        0..5,
        vec![
            node(1, 0..1, vec![]),
            node(1, 2..3, vec![]),
            node(1, 4..5, vec![]),
        ],
    );
    let query = query(vec![capture(1, 7), capture(1, 1), capture(1, 7)], vec![]);
    assert_eq!(
        facts(&QueryCursor::new().collect_matches(&query, &tree)),
        vec![vec![(1, 2..3), (7, 0..1), (7, 4..5)]]
    );
}

#[test]
fn test_public_cursor_plus_backtracks_without_losing_or_leaking_captures() {
    let tree = node(
        0,
        0..3,
        vec![
            node(1, 0..1, vec![]),
            node(1, 1..2, vec![]),
            node(1, 2..3, vec![]),
        ],
    );
    let query = query(
        vec![
            capture(1, 0).with_quantifier(Quantifier::Plus),
            capture(1, 1),
        ],
        vec![],
    );
    assert_eq!(
        facts(&QueryCursor::new().collect_matches(&query, &tree)),
        vec![vec![(0, 0..1), (0, 1..2), (1, 2..3)]]
    );
}

#[test]
fn test_public_cursor_keeps_separate_matches_and_range_root_options() {
    let tree = node(0, 0..5, vec![node(1, 0..1, vec![]), node(1, 4..5, vec![])]);
    let mut query = query(vec![], vec![]);
    query.patterns[0].root = capture(1, 0);
    let mut cursor = QueryCursor::new();
    assert_eq!(
        facts(&cursor.collect_matches(&query, &tree)),
        vec![vec![(0, 0..1)], vec![(0, 4..5)]]
    );
    cursor.set_byte_range(3..5);
    assert_eq!(
        facts(&cursor.collect_matches(&query, &tree)),
        vec![vec![(0, 4..5)]]
    );
    cursor.clear_byte_range();
    cursor.set_match_root(true);
    assert!(cursor.collect_matches(&query, &tree).is_empty());
}

#[test]
fn test_source_free_empty_ranges_do_not_fabricate_empty_text() {
    let tree = node(0, 0..0, vec![]);
    let mut available = query(
        vec![],
        vec![Predicate::Eq {
            capture1: 0,
            capture2: None,
            value: Some(String::new()),
        }],
    );
    available.patterns[0].root.capture = Some(0);
    assert_eq!(
        adze::query::matcher_v2::QueryMatcher::new(&available, "", &[])
            .matches(&tree)
            .len(),
        1,
        "an explicitly available empty source remains valid"
    );
    for predicate in [
        Predicate::Eq {
            capture1: 0,
            capture2: None,
            value: Some(String::new()),
        },
        Predicate::NotEq {
            capture1: 0,
            capture2: None,
            value: Some("x".into()),
        },
        Predicate::Match {
            capture: 0,
            regex: ".*".into(),
        },
        Predicate::NotMatch {
            capture: 0,
            regex: "x".into(),
        },
        Predicate::AnyOf {
            capture: 0,
            values: vec![String::new()],
        },
    ] {
        let mut query = query(vec![], vec![predicate]);
        query.patterns[0].root.capture = Some(0);
        assert!(QueryCursor::new().collect_matches(&query, &tree).is_empty());
    }
}

#[test]
fn test_source_free_capture_text_comparisons_do_not_compare_spans() {
    for (range, predicate) in [
        (
            0..3,
            Predicate::Eq {
                capture1: 0,
                capture2: Some(1),
                value: None,
            },
        ),
        (
            1..2,
            Predicate::NotEq {
                capture1: 0,
                capture2: Some(1),
                value: None,
            },
        ),
    ] {
        let tree = node(0, 0..3, vec![node(1, range, vec![])]);
        let mut query = query(vec![capture(1, 1)], vec![predicate]);
        query.patterns[0].root.capture = Some(0);
        assert!(
            QueryCursor::new().collect_matches(&query, &tree).is_empty(),
            "capture equality requires unavailable text; equal/different spans do not prove text equality"
        );
    }
}

#[test]
fn test_source_free_symbol_matching_does_not_invent_named_metadata() {
    let tree = node(0, 0..1, vec![node(1, 0..1, vec![])]);
    let mut child = capture(1, 0);
    child.is_named = false;
    let query = query(vec![child], vec![]);
    assert_eq!(
        facts(&QueryCursor::new().collect_matches(&query, &tree)),
        vec![vec![(0, 0..1)]]
    );
}

#[test]
fn test_public_cursor_range_considers_every_capture_and_preserves_capture_filter() {
    let tree = node(0, 0..5, vec![node(1, 0..1, vec![]), node(1, 4..5, vec![])]);
    let query = query(vec![capture(1, 0), capture(1, 0)], vec![]);
    let mut cursor = QueryCursor::new();
    cursor.set_byte_range(0..2);
    assert_eq!(
        facts(&cursor.collect_matches(&query, &tree)),
        vec![vec![(0, 0..1), (0, 4..5)]],
        "a nonfinal occurrence can admit a match"
    );
    cursor.set_byte_range(2..3);
    assert!(
        cursor.collect_matches(&query, &tree).is_empty(),
        "existing capture-based range filtering excludes matches whose captures all miss"
    );
}

#[test]
fn test_source_free_property_capabilities_remain_explicit() {
    let tree = node(0, 0..1, vec![]);
    for (predicate, count) in [
        (
            Predicate::Set {
                property: "x".into(),
                capture: None,
                value: None,
            },
            1,
        ),
        (
            Predicate::Is {
                property: "x".into(),
                capture: None,
                value: None,
            },
            0,
        ),
        (
            Predicate::IsNot {
                property: "x".into(),
                capture: None,
                value: None,
            },
            0,
        ),
        (
            Predicate::Custom {
                name: "unknown".into(),
                args: vec![],
            },
            0,
        ),
    ] {
        let mut query = query(vec![], vec![predicate]);
        query.patterns[0].root.capture = Some(0);
        assert_eq!(
            QueryCursor::new().collect_matches(&query, &tree).len(),
            count
        );
    }
}

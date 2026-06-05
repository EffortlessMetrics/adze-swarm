#![cfg(feature = "serialization")]

//! Property-based and unit tests for `SExpr` in the adze runtime.
//!
//! Covers atom construction, list construction, predicates, accessors,
//! Display determinism, Clone/Eq, nested structures, and edge cases.

use adze::serialization::*;
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

/// Arbitrary non-empty string for atoms (avoids empty-string edge cases in
/// the main proptest battery; empty strings are tested separately).
fn arb_atom_string() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_]{1,30}"
}

/// Arbitrary SExpr of bounded depth.
fn arb_sexpr(max_depth: u32) -> impl Strategy<Value = SExpr> {
    let leaf = arb_atom_string().prop_map(SExpr::Atom);
    leaf.prop_recursive(max_depth, 64, 8, |inner| {
        prop::collection::vec(inner, 0..8).prop_map(SExpr::List)
    })
}

// ---------------------------------------------------------------------------
// 1. Atom proptest (8 tests)
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn test_atom_from_random_string(s in "[a-zA-Z0-9_]{1,50}") {
        let expr = SExpr::atom(&s);
        prop_assert!(expr.is_atom());
        prop_assert!(!expr.is_list());
    }

    #[test]
    fn test_atom_as_atom_returns_some(s in "[a-zA-Z0-9]{1,30}") {
        let expr = SExpr::atom(&s);
        prop_assert_eq!(expr.as_atom(), Some(s.as_str()));
    }

    #[test]
    fn test_atom_as_list_returns_none(s in "\\PC{1,20}") {
        let expr = SExpr::atom(&s);
        prop_assert!(expr.as_list().is_none());
    }

    #[test]
    fn test_atom_variant_matches(s in "[a-z]{1,10}") {
        let expr = SExpr::Atom(s.clone());
        match &expr {
            SExpr::Atom(inner) => prop_assert_eq!(inner, &s),
            SExpr::List(_) => prop_assert!(false, "expected Atom"),
        }
    }

    #[test]
    fn test_atom_display_equals_content(s in "[a-zA-Z0-9]{1,20}") {
        let expr = SExpr::atom(&s);
        prop_assert_eq!(format!("{expr}"), s);
    }

    #[test]
    fn test_atom_debug_contains_atom(s in "[a-z]{1,10}") {
        let expr = SExpr::atom(&s);
        let debug_str = format!("{expr:?}");
        prop_assert!(debug_str.contains("Atom"));
    }

    #[test]
    fn test_atom_eq_same_string(s in "[a-zA-Z]{1,20}") {
        let a = SExpr::atom(&s);
        let b = SExpr::atom(&s);
        prop_assert_eq!(a, b);
    }

    #[test]
    fn test_atom_ne_different_string(a_str in "[a-z]{1,10}", b_str in "[A-Z]{1,10}") {
        let a = SExpr::atom(&a_str);
        let b = SExpr::atom(&b_str);
        // Lowercase vs uppercase — always different.
        prop_assert_ne!(a, b);
    }
}

// ---------------------------------------------------------------------------
// 2. List proptest (8 tests)
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn test_list_is_list(items in prop::collection::vec(arb_atom_string(), 0..10)) {
        let children: Vec<SExpr> = items.iter().map(|s| SExpr::atom(s)).collect();
        let expr = SExpr::list(children);
        prop_assert!(expr.is_list());
        prop_assert!(!expr.is_atom());
    }

    #[test]
    fn test_list_as_list_len(items in prop::collection::vec(arb_atom_string(), 0..10)) {
        let n = items.len();
        let children: Vec<SExpr> = items.iter().map(|s| SExpr::atom(s)).collect();
        let expr = SExpr::list(children);
        prop_assert_eq!(expr.as_list().unwrap().len(), n);
    }

    #[test]
    fn test_list_as_atom_returns_none(items in prop::collection::vec(arb_atom_string(), 0..5)) {
        let children: Vec<SExpr> = items.iter().map(|s| SExpr::atom(s)).collect();
        let expr = SExpr::list(children);
        prop_assert!(expr.as_atom().is_none());
    }

    #[test]
    fn test_list_preserves_order(items in prop::collection::vec("[a-z]{1,5}", 1..8)) {
        let children: Vec<SExpr> = items.iter().map(|s| SExpr::atom(s)).collect();
        let expr = SExpr::list(children);
        let list = expr.as_list().unwrap();
        for (i, item) in items.iter().enumerate() {
            prop_assert_eq!(list[i].as_atom().unwrap(), item.as_str());
        }
    }

    #[test]
    fn test_list_nested_depth_2(
        outer in prop::collection::vec("[a-z]{1,3}", 1..4),
        inner in prop::collection::vec("[A-Z]{1,3}", 1..4),
    ) {
        let inner_list = SExpr::list(inner.iter().map(|s| SExpr::atom(s)).collect());
        let mut children: Vec<SExpr> = outer.iter().map(|s| SExpr::atom(s)).collect();
        children.push(inner_list);
        let expr = SExpr::list(children);
        prop_assert!(expr.is_list());
        let items = expr.as_list().unwrap();
        prop_assert!(items.last().unwrap().is_list());
    }

    #[test]
    fn test_list_empty_is_valid(_dummy in Just(())) {
        let expr = SExpr::list(vec![]);
        prop_assert!(expr.is_list());
        prop_assert_eq!(expr.as_list().unwrap().len(), 0);
    }

    #[test]
    fn test_list_singleton(s in "[a-z]{1,5}") {
        let expr = SExpr::list(vec![SExpr::atom(&s)]);
        let items = expr.as_list().unwrap();
        prop_assert_eq!(items.len(), 1);
        prop_assert_eq!(items[0].as_atom().unwrap(), s.as_str());
    }

    #[test]
    fn test_list_debug_contains_list(items in prop::collection::vec("[a-z]{1,3}", 0..3)) {
        let children: Vec<SExpr> = items.iter().map(|s| SExpr::atom(s)).collect();
        let expr = SExpr::list(children);
        let debug_str = format!("{expr:?}");
        prop_assert!(debug_str.contains("List"));
    }
}

// ---------------------------------------------------------------------------
// 3. Display proptest (5 tests)
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn test_display_deterministic(expr in arb_sexpr(3)) {
        let s1 = format!("{expr}");
        let s2 = format!("{expr}");
        prop_assert_eq!(s1, s2);
    }

    #[test]
    fn test_display_atom_no_parens(s in "[a-zA-Z0-9]{1,20}") {
        let expr = SExpr::atom(&s);
        let displayed = format!("{expr}");
        prop_assert!(!displayed.starts_with('('));
        prop_assert!(!displayed.ends_with(')'));
    }

    #[test]
    fn test_display_list_has_parens(items in prop::collection::vec("[a-z]{1,5}", 0..5)) {
        let children: Vec<SExpr> = items.iter().map(|s| SExpr::atom(s)).collect();
        let expr = SExpr::list(children);
        let displayed = format!("{expr}");
        prop_assert!(displayed.starts_with('('));
        prop_assert!(displayed.ends_with(')'));
    }

    #[test]
    fn test_display_list_spaces_between_items(items in prop::collection::vec("[a-z]{2,5}", 2..6)) {
        let children: Vec<SExpr> = items.iter().map(|s| SExpr::atom(s)).collect();
        let expr = SExpr::list(children);
        let displayed = format!("{expr}");
        // Between items there should be exactly one space (not at start/end).
        let inner = &displayed[1..displayed.len() - 1];
        prop_assert!(inner.contains(' '));
    }

    #[test]
    fn test_display_non_empty(expr in arb_sexpr(2)) {
        let displayed = format!("{expr}");
        prop_assert!(!displayed.is_empty());
    }
}

// ---------------------------------------------------------------------------
// 4. Clone/Eq proptest (5 tests)
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn test_clone_equals_original(expr in arb_sexpr(3)) {
        let cloned = expr.clone();
        prop_assert_eq!(&expr, &cloned);
    }

    #[test]
    fn test_clone_display_matches(expr in arb_sexpr(3)) {
        let cloned = expr.clone();
        prop_assert_eq!(format!("{expr}"), format!("{cloned}"));
    }

    #[test]
    fn test_eq_reflexive(expr in arb_sexpr(3)) {
        prop_assert_eq!(&expr, &expr);
    }

    #[test]
    fn test_eq_symmetric(a in arb_sexpr(2), b in arb_sexpr(2)) {
        prop_assert_eq!(a == b, b == a);
    }

    #[test]
    fn test_ne_atom_vs_list(s in "[a-z]{1,10}") {
        let atom = SExpr::atom(&s);
        let list = SExpr::list(vec![SExpr::atom(&s)]);
        prop_assert_ne!(atom, list);
    }
}

// ---------------------------------------------------------------------------
// 5. Nested structure proptest (5 tests)
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn test_nested_recursive_is_valid(expr in arb_sexpr(4)) {
        // Should not panic when accessing predicates.
        let _ = expr.is_atom();
        let _ = expr.is_list();
    }

    #[test]
    fn test_nested_display_balanced_parens(expr in arb_sexpr(3)) {
        let displayed = format!("{expr}");
        let opens = displayed.chars().filter(|&c| c == '(').count();
        let closes = displayed.chars().filter(|&c| c == ')').count();
        prop_assert_eq!(opens, closes);
    }

    #[test]
    fn test_nested_clone_deep(expr in arb_sexpr(4)) {
        let cloned = expr.clone();
        prop_assert_eq!(format!("{expr}"), format!("{cloned}"));
    }

    #[test]
    fn test_nested_list_of_lists(
        items_a in prop::collection::vec("[a-z]{1,3}", 1..4),
        items_b in prop::collection::vec("[A-Z]{1,3}", 1..4),
    ) {
        let list_a = SExpr::list(items_a.iter().map(|s| SExpr::atom(s)).collect());
        let list_b = SExpr::list(items_b.iter().map(|s| SExpr::atom(s)).collect());
        let outer = SExpr::list(vec![list_a, list_b]);
        let children = outer.as_list().unwrap();
        prop_assert_eq!(children.len(), 2);
        prop_assert!(children[0].is_list());
        prop_assert!(children[1].is_list());
    }

    #[test]
    fn test_nested_atom_at_every_leaf(expr in arb_sexpr(3)) {
        fn check_leaves(e: &SExpr) -> bool {
            match e {
                SExpr::Atom(_) => true,
                SExpr::List(items) => items.iter().all(check_leaves),
            }
        }
        prop_assert!(check_leaves(&expr));
    }
}

// ---------------------------------------------------------------------------
// 6. Regular atom tests (8 tests)
// ---------------------------------------------------------------------------

#[test]
fn test_atom_simple() {
    let expr = SExpr::atom("hello");
    assert_eq!(expr.as_atom(), Some("hello"));
}

#[test]
fn test_atom_empty_string() {
    let expr = SExpr::atom("");
    assert!(expr.is_atom());
    assert_eq!(expr.as_atom(), Some(""));
}

#[test]
fn test_atom_with_spaces() {
    let expr = SExpr::atom("hello world");
    assert_eq!(expr.as_atom(), Some("hello world"));
}

#[test]
fn test_atom_numeric_string() {
    let expr = SExpr::atom("42");
    assert!(expr.is_atom());
    assert_eq!(expr.as_atom(), Some("42"));
}

#[test]
fn test_atom_special_chars() {
    let expr = SExpr::atom("!@#$%");
    assert_eq!(expr.as_atom(), Some("!@#$%"));
}

#[test]
fn test_atom_unicode() {
    let expr = SExpr::atom("日本語");
    assert!(expr.is_atom());
    assert_eq!(expr.as_atom(), Some("日本語"));
}

#[test]
fn test_atom_display_simple() {
    let expr = SExpr::atom("foo");
    assert_eq!(format!("{expr}"), "foo");
}

#[test]
fn test_atom_variant_constructor_matches_enum() {
    let via_fn = SExpr::atom("x");
    let via_enum = SExpr::Atom("x".to_string());
    assert_eq!(via_fn, via_enum);
}

// ---------------------------------------------------------------------------
// 7. Regular list tests (5 tests)
// ---------------------------------------------------------------------------

#[test]
fn test_list_empty() {
    let expr = SExpr::list(vec![]);
    assert!(expr.is_list());
    assert_eq!(expr.as_list().unwrap().len(), 0);
    assert_eq!(format!("{expr}"), "()");
}

#[test]
fn test_list_single_atom() {
    let expr = SExpr::list(vec![SExpr::atom("a")]);
    assert_eq!(format!("{expr}"), "(a)");
}

#[test]
fn test_list_multiple_atoms() {
    let expr = SExpr::list(vec![SExpr::atom("a"), SExpr::atom("b"), SExpr::atom("c")]);
    assert_eq!(format!("{expr}"), "(a b c)");
}

#[test]
fn test_list_nested() {
    let inner = SExpr::list(vec![SExpr::atom("x"), SExpr::atom("y")]);
    let outer = SExpr::list(vec![SExpr::atom("a"), inner]);
    assert_eq!(format!("{outer}"), "(a (x y))");
}

#[test]
fn test_list_constructor_matches_enum() {
    let via_fn = SExpr::list(vec![SExpr::atom("z")]);
    let via_enum = SExpr::List(vec![SExpr::Atom("z".to_string())]);
    assert_eq!(via_fn, via_enum);
}

// ---------------------------------------------------------------------------
// 8. Edge cases (6 tests)
// ---------------------------------------------------------------------------

#[test]
fn test_parse_sexpr_atom_returns_ok() {
    let result = parse_sexpr("anything");
    assert_eq!(result, Ok(SExpr::Atom("anything".to_string())));
}

#[test]
fn test_parse_sexpr_list_returns_items() {
    let result = parse_sexpr("(a b c)").unwrap();
    assert!(result.is_list());
    assert_eq!(result.as_list().unwrap().len(), 3);
}

#[test]
fn test_deeply_nested_display() {
    let mut expr = SExpr::atom("leaf");
    for _ in 0..20 {
        expr = SExpr::list(vec![expr]);
    }
    let displayed = format!("{expr}");
    let opens = displayed.chars().filter(|&c| c == '(').count();
    let closes = displayed.chars().filter(|&c| c == ')').count();
    assert_eq!(opens, 20);
    assert_eq!(closes, 20);
}

#[test]
fn test_display_empty_atom() {
    let expr = SExpr::atom("");
    assert_eq!(format!("{expr}"), "");
}

#[test]
fn test_eq_empty_list_vs_empty_list() {
    let a = SExpr::list(vec![]);
    let b = SExpr::list(vec![]);
    assert_eq!(a, b);
}

#[test]
fn test_ne_empty_atom_vs_empty_list() {
    let atom = SExpr::atom("");
    let list = SExpr::list(vec![]);
    assert_ne!(atom, list);
}

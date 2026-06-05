#![cfg(feature = "serialization")]

//! Comprehensive tests for the `SExpr` enum and `parse_sexpr` in
//! `adze::serialization`.
//!
//! Categories:
//! 1. SExpr construction (8)
//! 2. SExpr Display/Debug (5)
//! 3. SExpr Clone/PartialEq (5)
//! 4. SExpr nesting patterns (8)
//! 5. SExpr Atom values (5)
//! 6. SExpr List operations (5)
//! 7. SExpr traversal patterns (8)
//! 8. parse_sexpr behavior (5)
//! 9. Edge cases (6)

use adze::serialization::{SExpr, parse_sexpr};

// ---- helpers ----------------------------------------------------------------

fn atom(s: &str) -> SExpr {
    SExpr::Atom(s.to_string())
}

fn list(items: Vec<SExpr>) -> SExpr {
    SExpr::List(items)
}

fn empty_list() -> SExpr {
    SExpr::List(vec![])
}

/// Recursively count every node (atoms + lists) in an SExpr tree.
fn count_nodes(expr: &SExpr) -> usize {
    match expr {
        SExpr::Atom(_) => 1,
        SExpr::List(items) => 1 + items.iter().map(count_nodes).sum::<usize>(),
    }
}

/// Return the maximum nesting depth (an Atom has depth 0, an empty list has
/// depth 0, a list with children has depth 1 + max child depth).
fn max_depth(expr: &SExpr) -> usize {
    match expr {
        SExpr::Atom(_) => 0,
        SExpr::List(items) if items.is_empty() => 0,
        SExpr::List(items) => 1 + items.iter().map(max_depth).max().unwrap_or(0),
    }
}

/// Collect all Atom values in pre-order.
fn collect_atoms(expr: &SExpr) -> Vec<String> {
    match expr {
        SExpr::Atom(s) => vec![s.clone()],
        SExpr::List(items) => items.iter().flat_map(collect_atoms).collect(),
    }
}

// =============================================================================
// 1. SExpr construction (8 tests)
// =============================================================================

#[test]
fn test_construct_atom_simple() {
    let a = atom("hello");
    assert!(matches!(a, SExpr::Atom(ref s) if s == "hello"));
}

#[test]
fn test_construct_atom_empty_string() {
    let a = atom("");
    assert!(matches!(a, SExpr::Atom(ref s) if s.is_empty()));
}

#[test]
fn test_construct_list_empty() {
    let l = empty_list();
    assert!(matches!(l, SExpr::List(ref v) if v.is_empty()));
}

#[test]
fn test_construct_list_single_atom() {
    let l = list(vec![atom("x")]);
    if let SExpr::List(ref items) = l {
        assert_eq!(items.len(), 1);
    } else {
        panic!("expected List");
    }
}

#[test]
fn test_construct_list_multiple_atoms() {
    let l = list(vec![atom("a"), atom("b"), atom("c")]);
    if let SExpr::List(ref items) = l {
        assert_eq!(items.len(), 3);
    } else {
        panic!("expected List");
    }
}

#[test]
fn test_construct_nested_lists() {
    let inner = list(vec![atom("inner")]);
    let outer = list(vec![inner]);
    if let SExpr::List(ref items) = outer {
        assert!(matches!(items[0], SExpr::List(_)));
    } else {
        panic!("expected outer List");
    }
}

#[test]
fn test_construct_mixed_atoms_and_lists() {
    let expr = list(vec![atom("head"), list(vec![atom("sub")]), atom("tail")]);
    if let SExpr::List(ref items) = expr {
        assert_eq!(items.len(), 3);
        assert!(matches!(items[0], SExpr::Atom(_)));
        assert!(matches!(items[1], SExpr::List(_)));
        assert!(matches!(items[2], SExpr::Atom(_)));
    } else {
        panic!("expected List");
    }
}

#[test]
fn test_construct_atom_with_special_chars() {
    let a = atom("hello world\t\n\"quotes\"");
    if let SExpr::Atom(ref s) = a {
        assert!(s.contains("hello world"));
        assert!(s.contains('"'));
    } else {
        panic!("expected Atom");
    }
}

// =============================================================================
// 2. SExpr Display/Debug (5 tests)
// =============================================================================

#[test]
fn test_debug_atom() {
    let a = atom("test");
    let dbg = format!("{:?}", a);
    assert!(dbg.contains("Atom"));
    assert!(dbg.contains("test"));
}

#[test]
fn test_debug_empty_list() {
    let l = empty_list();
    let dbg = format!("{:?}", l);
    assert!(dbg.contains("List"));
}

#[test]
fn test_debug_nested() {
    let expr = list(vec![atom("a"), list(vec![atom("b")])]);
    let dbg = format!("{:?}", expr);
    assert!(dbg.contains("Atom"));
    assert!(dbg.contains("List"));
}

#[test]
fn test_debug_atom_roundtrip_contains_value() {
    let a = atom("my-value-123");
    let dbg = format!("{:?}", a);
    assert!(dbg.contains("my-value-123"));
}

#[test]
fn test_debug_list_shows_children_count_implicitly() {
    let l = list(vec![atom("x"), atom("y"), atom("z")]);
    let dbg = format!("{:?}", l);
    // Debug should show all three children in some form
    assert!(dbg.contains("x"));
    assert!(dbg.contains("y"));
    assert!(dbg.contains("z"));
}

// =============================================================================
// 3. SExpr Clone/PartialEq (5 tests)
// =============================================================================

#[test]
fn test_clone_atom() {
    let a = atom("hello");
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn test_clone_list() {
    let l = list(vec![atom("a"), atom("b")]);
    let l2 = l.clone();
    assert_eq!(l, l2);
}

#[test]
fn test_eq_different_atoms() {
    assert_ne!(atom("a"), atom("b"));
}

#[test]
fn test_eq_atom_vs_list() {
    assert_ne!(atom("a"), empty_list());
}

#[test]
fn test_eq_nested_deep() {
    let expr1 = list(vec![atom("x"), list(vec![atom("y")])]);
    let expr2 = list(vec![atom("x"), list(vec![atom("y")])]);
    let expr3 = list(vec![atom("x"), list(vec![atom("z")])]);
    assert_eq!(expr1, expr2);
    assert_ne!(expr1, expr3);
}

// =============================================================================
// 4. SExpr nesting patterns (8 tests)
// =============================================================================

#[test]
fn test_nesting_depth_0_atom() {
    assert_eq!(max_depth(&atom("x")), 0);
}

#[test]
fn test_nesting_depth_0_empty_list() {
    assert_eq!(max_depth(&empty_list()), 0);
}

#[test]
fn test_nesting_depth_1() {
    let expr = list(vec![atom("a")]);
    assert_eq!(max_depth(&expr), 1);
}

#[test]
fn test_nesting_depth_2() {
    let expr = list(vec![list(vec![atom("a")])]);
    assert_eq!(max_depth(&expr), 2);
}

#[test]
fn test_nesting_depth_5() {
    let mut expr = atom("leaf");
    for _ in 0..5 {
        expr = list(vec![expr]);
    }
    assert_eq!(max_depth(&expr), 5);
}

#[test]
fn test_nesting_wide_tree() {
    let wide = list(vec![atom("a"), atom("b"), atom("c"), atom("d"), atom("e")]);
    assert_eq!(max_depth(&wide), 1);
    assert_eq!(count_nodes(&wide), 6); // 1 list + 5 atoms
}

#[test]
fn test_nesting_unbalanced() {
    // left-heavy
    let expr = list(vec![list(vec![list(vec![atom("deep")])]), atom("shallow")]);
    assert_eq!(max_depth(&expr), 3);
}

#[test]
fn test_nesting_sibling_lists() {
    let expr = list(vec![
        list(vec![atom("a")]),
        list(vec![atom("b")]),
        list(vec![atom("c")]),
    ]);
    assert_eq!(max_depth(&expr), 2);
    assert_eq!(count_nodes(&expr), 7); // 1 outer + 3 inner lists + 3 atoms
}

// =============================================================================
// 5. SExpr Atom values (5 tests)
// =============================================================================

#[test]
fn test_atom_numeric_string() {
    let a = atom("42");
    assert_eq!(a, SExpr::Atom("42".to_string()));
}

#[test]
fn test_atom_unicode() {
    let a = atom("日本語");
    if let SExpr::Atom(ref s) = a {
        assert_eq!(s, "日本語");
    } else {
        panic!("expected Atom");
    }
}

#[test]
fn test_atom_whitespace_only() {
    let a = atom("   ");
    if let SExpr::Atom(ref s) = a {
        assert_eq!(s.len(), 3);
    } else {
        panic!("expected Atom");
    }
}

#[test]
fn test_atom_with_parentheses() {
    let a = atom("(hello)");
    if let SExpr::Atom(ref s) = a {
        assert!(s.starts_with('('));
        assert!(s.ends_with(')'));
    } else {
        panic!("expected Atom");
    }
}

#[test]
fn test_atom_long_string() {
    let long = "a".repeat(10_000);
    let a = atom(&long);
    if let SExpr::Atom(ref s) = a {
        assert_eq!(s.len(), 10_000);
    } else {
        panic!("expected Atom");
    }
}

// =============================================================================
// 6. SExpr List operations (5 tests)
// =============================================================================

#[test]
fn test_list_push_via_vec() {
    let mut items = vec![atom("a")];
    items.push(atom("b"));
    let l = list(items);
    if let SExpr::List(ref v) = l {
        assert_eq!(v.len(), 2);
    } else {
        panic!("expected List");
    }
}

#[test]
fn test_list_iteration() {
    let l = list(vec![atom("x"), atom("y"), atom("z")]);
    if let SExpr::List(ref items) = l {
        let names: Vec<&str> = items
            .iter()
            .map(|item| match item {
                SExpr::Atom(s) => s.as_str(),
                _ => "",
            })
            .collect();
        assert_eq!(names, vec!["x", "y", "z"]);
    } else {
        panic!("expected List");
    }
}

#[test]
fn test_list_contains_check() {
    let l = list(vec![atom("needle"), atom("hay")]);
    if let SExpr::List(ref items) = l {
        assert!(items.contains(&atom("needle")));
        assert!(!items.contains(&atom("missing")));
    } else {
        panic!("expected List");
    }
}

#[test]
fn test_list_len_consistency() {
    for n in 0..8 {
        let items: Vec<SExpr> = (0..n).map(|i| atom(&i.to_string())).collect();
        let l = list(items);
        if let SExpr::List(ref v) = l {
            assert_eq!(v.len(), n);
        }
    }
}

#[test]
fn test_list_index_access() {
    let l = list(vec![atom("first"), atom("second"), atom("third")]);
    if let SExpr::List(ref items) = l {
        assert_eq!(items[0], atom("first"));
        assert_eq!(items[2], atom("third"));
    } else {
        panic!("expected List");
    }
}

// =============================================================================
// 7. SExpr traversal patterns (8 tests)
// =============================================================================

#[test]
fn test_collect_atoms_flat() {
    let expr = list(vec![atom("a"), atom("b"), atom("c")]);
    assert_eq!(collect_atoms(&expr), vec!["a", "b", "c"]);
}

#[test]
fn test_collect_atoms_nested() {
    let expr = list(vec![atom("a"), list(vec![atom("b"), atom("c")]), atom("d")]);
    assert_eq!(collect_atoms(&expr), vec!["a", "b", "c", "d"]);
}

#[test]
fn test_collect_atoms_empty_list() {
    let expr = empty_list();
    assert!(collect_atoms(&expr).is_empty());
}

#[test]
fn test_collect_atoms_single() {
    assert_eq!(collect_atoms(&atom("solo")), vec!["solo"]);
}

#[test]
fn test_count_nodes_single_atom() {
    assert_eq!(count_nodes(&atom("x")), 1);
}

#[test]
fn test_count_nodes_complex_tree() {
    // (a (b c) d) => 1 outer list + atom a + 1 inner list + atom b + atom c + atom d = 6
    let expr = list(vec![atom("a"), list(vec![atom("b"), atom("c")]), atom("d")]);
    assert_eq!(count_nodes(&expr), 6);
}

#[test]
fn test_traversal_preserves_order() {
    let expr = list(vec![
        atom("1"),
        list(vec![atom("2"), atom("3")]),
        atom("4"),
        list(vec![list(vec![atom("5")])]),
    ]);
    assert_eq!(collect_atoms(&expr), vec!["1", "2", "3", "4", "5"]);
}

#[test]
fn test_traversal_nested_empty_lists() {
    let expr = list(vec![
        empty_list(),
        list(vec![empty_list(), atom("found")]),
        empty_list(),
    ]);
    assert_eq!(collect_atoms(&expr), vec!["found"]);
    // 1 outer + 3 children (empty, inner list, empty) + 1 inner empty + 1 atom = 6
    assert_eq!(count_nodes(&expr), 6);
}

// =============================================================================
// 8. parse_sexpr behavior (5 tests)
// =============================================================================

#[test]
fn test_parse_sexpr_returns_ok() {
    let result = parse_sexpr("(hello world)");
    assert!(result.is_ok());
}

#[test]
fn test_parse_sexpr_returns_atom() {
    let result = parse_sexpr("anything").unwrap();
    assert_eq!(result, atom("anything"));
}

#[test]
fn test_parse_sexpr_respects_input() {
    let r1 = parse_sexpr("(a b c)").unwrap();
    let r2 = parse_sexpr("completely_different").unwrap();
    assert_ne!(r1, r2);
}

#[test]
fn test_parse_sexpr_empty_string() {
    assert!(parse_sexpr("").is_err());
}

#[test]
fn test_parse_sexpr_complex_input() {
    let result =
        parse_sexpr("(define (factorial n) (if (= n 0) 1 (* n (factorial (- n 1)))))").unwrap();
    assert!(result.is_list());
    assert_ne!(result, empty_list());
}

// =============================================================================
// 9. Edge cases (6 tests)
// =============================================================================

#[test]
fn test_sexpr_serde_atom_roundtrip() {
    let original = atom("round-trip");
    let json = serde_json::to_string(&original).unwrap();
    let deserialized: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(original, deserialized);
}

#[test]
fn test_sexpr_serde_list_roundtrip() {
    let original = list(vec![atom("a"), list(vec![atom("b")]), atom("c")]);
    let json = serde_json::to_string(&original).unwrap();
    let deserialized: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(original, deserialized);
}

#[test]
fn test_sexpr_serde_empty_list_roundtrip() {
    let original = empty_list();
    let json = serde_json::to_string(&original).unwrap();
    let deserialized: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(original, deserialized);
}

#[test]
fn test_sexpr_eq_is_reflexive() {
    let expr = list(vec![atom("x"), list(vec![atom("y")])]);
    assert_eq!(expr, expr);
}

#[test]
fn test_sexpr_clone_independence() {
    let original = list(vec![atom("mutable?")]);
    let cloned = original.clone();
    // Cloned value equals original
    assert_eq!(original, cloned);
    // They are separate allocations (modify via reconstruction)
    let modified = list(vec![atom("changed")]);
    assert_ne!(original, modified);
    assert_eq!(cloned, list(vec![atom("mutable?")]));
}

#[test]
fn test_sexpr_large_flat_list() {
    let items: Vec<SExpr> = (0..1000).map(|i| atom(&i.to_string())).collect();
    let big = list(items);
    if let SExpr::List(ref v) = big {
        assert_eq!(v.len(), 1000);
        assert_eq!(v[0], atom("0"));
        assert_eq!(v[999], atom("999"));
    } else {
        panic!("expected List");
    }
    assert_eq!(count_nodes(&big), 1001);
}

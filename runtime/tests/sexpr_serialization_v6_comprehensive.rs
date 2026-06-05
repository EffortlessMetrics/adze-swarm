#![cfg(feature = "serialization")]
//! Comprehensive tests for SExpr serialization API.

use adze::serialization::*;

// ── 1. SExpr::atom construction (8 tests) ──────────────────────────────────

#[test]
fn test_atom_simple_word() {
    let a = SExpr::atom("hello");
    assert_eq!(a, SExpr::Atom("hello".to_string()));
}

#[test]
fn test_atom_empty_string() {
    let a = SExpr::atom("");
    assert_eq!(a, SExpr::Atom(String::new()));
}

#[test]
fn test_atom_with_spaces() {
    let a = SExpr::atom("hello world");
    assert_eq!(a, SExpr::Atom("hello world".to_string()));
}

#[test]
fn test_atom_with_special_chars() {
    let a = SExpr::atom("foo-bar_baz");
    assert_eq!(a, SExpr::Atom("foo-bar_baz".to_string()));
}

#[test]
fn test_atom_numeric_string() {
    let a = SExpr::atom("42");
    assert_eq!(a, SExpr::Atom("42".to_string()));
}

#[test]
fn test_atom_unicode() {
    let a = SExpr::atom("café");
    assert_eq!(a, SExpr::Atom("café".to_string()));
}

#[test]
fn test_atom_parentheses_in_value() {
    let a = SExpr::atom("(foo)");
    assert_eq!(a, SExpr::Atom("(foo)".to_string()));
}

#[test]
fn test_atom_newline_in_value() {
    let a = SExpr::atom("line1\nline2");
    assert_eq!(a, SExpr::Atom("line1\nline2".to_string()));
}

// ── 2. SExpr::list construction (8 tests) ──────────────────────────────────

#[test]
fn test_list_empty() {
    let l = SExpr::list(vec![]);
    assert_eq!(l, SExpr::List(vec![]));
}

#[test]
fn test_list_single_atom() {
    let l = SExpr::list(vec![SExpr::atom("x")]);
    assert_eq!(l, SExpr::List(vec![SExpr::Atom("x".to_string())]));
}

#[test]
fn test_list_multiple_atoms() {
    let l = SExpr::list(vec![SExpr::atom("a"), SExpr::atom("b"), SExpr::atom("c")]);
    assert_eq!(
        l,
        SExpr::List(vec![
            SExpr::Atom("a".to_string()),
            SExpr::Atom("b".to_string()),
            SExpr::Atom("c".to_string()),
        ])
    );
}

#[test]
fn test_list_nested_list() {
    let inner = SExpr::list(vec![SExpr::atom("inner")]);
    let outer = SExpr::list(vec![inner]);
    assert_eq!(
        outer,
        SExpr::List(vec![SExpr::List(vec![SExpr::Atom("inner".to_string())])])
    );
}

#[test]
fn test_list_mixed_atoms_and_lists() {
    let l = SExpr::list(vec![
        SExpr::atom("head"),
        SExpr::list(vec![SExpr::atom("nested")]),
    ]);
    assert!(matches!(l, SExpr::List(_)));
    let items = l.as_list().unwrap();
    assert_eq!(items.len(), 2);
}

#[test]
fn test_list_deeply_nested() {
    let mut expr = SExpr::atom("leaf");
    for _ in 0..5 {
        expr = SExpr::list(vec![expr]);
    }
    assert!(expr.is_list());
}

#[test]
fn test_list_from_vec_literal() {
    let items = vec![SExpr::atom("1"), SExpr::atom("2")];
    let l = SExpr::list(items);
    assert_eq!(l.as_list().unwrap().len(), 2);
}

#[test]
fn test_list_preserves_order() {
    let l = SExpr::list(vec![
        SExpr::atom("first"),
        SExpr::atom("second"),
        SExpr::atom("third"),
    ]);
    let items = l.as_list().unwrap();
    assert_eq!(items[0].as_atom().unwrap(), "first");
    assert_eq!(items[1].as_atom().unwrap(), "second");
    assert_eq!(items[2].as_atom().unwrap(), "third");
}

// ── 3. SExpr predicates (is_atom, is_list) (5 tests) ───────────────────────

#[test]
fn test_is_atom_true_for_atom() {
    assert!(SExpr::atom("x").is_atom());
}

#[test]
fn test_is_atom_false_for_list() {
    assert!(!SExpr::list(vec![]).is_atom());
}

#[test]
fn test_is_list_true_for_list() {
    assert!(SExpr::list(vec![]).is_list());
}

#[test]
fn test_is_list_false_for_atom() {
    assert!(!SExpr::atom("x").is_list());
}

#[test]
fn test_predicates_mutually_exclusive() {
    let atom = SExpr::atom("a");
    let list = SExpr::list(vec![]);
    assert!(atom.is_atom() && !atom.is_list());
    assert!(list.is_list() && !list.is_atom());
}

// ── 4. SExpr accessors (as_atom, as_list) (5 tests) ────────────────────────

#[test]
fn test_as_atom_returns_some_for_atom() {
    let a = SExpr::atom("hello");
    assert_eq!(a.as_atom(), Some("hello"));
}

#[test]
fn test_as_atom_returns_none_for_list() {
    let l = SExpr::list(vec![]);
    assert_eq!(l.as_atom(), None);
}

#[test]
fn test_as_list_returns_some_for_list() {
    let l = SExpr::list(vec![SExpr::atom("x")]);
    let items = l.as_list().unwrap();
    assert_eq!(items.len(), 1);
}

#[test]
fn test_as_list_returns_none_for_atom() {
    let a = SExpr::atom("x");
    assert_eq!(a.as_list(), None);
}

#[test]
fn test_as_list_empty_list_returns_empty_slice() {
    let l = SExpr::list(vec![]);
    let items = l.as_list().unwrap();
    assert!(items.is_empty());
}

// ── 5. Display formatting (8 tests) ────────────────────────────────────────

#[test]
fn test_display_atom() {
    let a = SExpr::atom("hello");
    assert_eq!(format!("{a}"), "hello");
}

#[test]
fn test_display_empty_list() {
    let l = SExpr::list(vec![]);
    assert_eq!(format!("{l}"), "()");
}

#[test]
fn test_display_single_element_list() {
    let l = SExpr::list(vec![SExpr::atom("x")]);
    assert_eq!(format!("{l}"), "(x)");
}

#[test]
fn test_display_multi_element_list() {
    let l = SExpr::list(vec![SExpr::atom("a"), SExpr::atom("b"), SExpr::atom("c")]);
    assert_eq!(format!("{l}"), "(a b c)");
}

#[test]
fn test_display_nested_list() {
    let l = SExpr::list(vec![
        SExpr::atom("define"),
        SExpr::list(vec![SExpr::atom("x"), SExpr::atom("y")]),
        SExpr::atom("body"),
    ]);
    assert_eq!(format!("{l}"), "(define (x y) body)");
}

#[test]
fn test_display_deeply_nested() {
    let inner = SExpr::list(vec![SExpr::atom("deep")]);
    let mid = SExpr::list(vec![inner]);
    let outer = SExpr::list(vec![mid]);
    assert_eq!(format!("{outer}"), "(((deep)))");
}

#[test]
fn test_display_atom_with_spaces() {
    let a = SExpr::atom("hello world");
    assert_eq!(format!("{a}"), "hello world");
}

#[test]
fn test_display_list_of_lists() {
    let l = SExpr::list(vec![
        SExpr::list(vec![SExpr::atom("a")]),
        SExpr::list(vec![SExpr::atom("b")]),
    ]);
    assert_eq!(format!("{l}"), "((a) (b))");
}

// ── 6. Nested S-expressions (8 tests) ──────────────────────────────────────

#[test]
fn test_nested_atom_in_list() {
    let expr = SExpr::list(vec![SExpr::atom("only")]);
    let items = expr.as_list().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].as_atom().unwrap(), "only");
}

#[test]
fn test_nested_list_in_list() {
    let inner = SExpr::list(vec![SExpr::atom("a"), SExpr::atom("b")]);
    let outer = SExpr::list(vec![inner.clone(), SExpr::atom("c")]);
    let items = outer.as_list().unwrap();
    assert_eq!(items.len(), 2);
    assert!(items[0].is_list());
    assert!(items[1].is_atom());
}

#[test]
fn test_nested_three_levels() {
    let level3 = SExpr::atom("leaf");
    let level2 = SExpr::list(vec![level3]);
    let level1 = SExpr::list(vec![level2]);
    let items1 = level1.as_list().unwrap();
    let items2 = items1[0].as_list().unwrap();
    assert_eq!(items2[0].as_atom().unwrap(), "leaf");
}

#[test]
fn test_nested_mixed_structure() {
    let expr = SExpr::list(vec![
        SExpr::atom("+"),
        SExpr::list(vec![SExpr::atom("*"), SExpr::atom("2"), SExpr::atom("3")]),
        SExpr::atom("4"),
    ]);
    assert_eq!(format!("{expr}"), "(+ (* 2 3) 4)");
}

#[test]
fn test_nested_sibling_lists() {
    let expr = SExpr::list(vec![
        SExpr::list(vec![SExpr::atom("a")]),
        SExpr::list(vec![SExpr::atom("b")]),
        SExpr::list(vec![SExpr::atom("c")]),
    ]);
    let items = expr.as_list().unwrap();
    assert_eq!(items.len(), 3);
    for item in items {
        assert!(item.is_list(), "expected list variant");
    }
}

#[test]
fn test_nested_empty_lists() {
    let expr = SExpr::list(vec![SExpr::list(vec![]), SExpr::list(vec![])]);
    assert_eq!(format!("{expr}"), "(() ())");
}

#[test]
fn test_nested_lisp_style_expression() {
    let expr = SExpr::list(vec![
        SExpr::atom("defun"),
        SExpr::atom("factorial"),
        SExpr::list(vec![SExpr::atom("n")]),
        SExpr::list(vec![
            SExpr::atom("if"),
            SExpr::list(vec![SExpr::atom("="), SExpr::atom("n"), SExpr::atom("0")]),
            SExpr::atom("1"),
            SExpr::list(vec![
                SExpr::atom("*"),
                SExpr::atom("n"),
                SExpr::list(vec![
                    SExpr::atom("factorial"),
                    SExpr::list(vec![SExpr::atom("-"), SExpr::atom("n"), SExpr::atom("1")]),
                ]),
            ]),
        ]),
    ]);
    assert_eq!(
        format!("{expr}"),
        "(defun factorial (n) (if (= n 0) 1 (* n (factorial (- n 1)))))"
    );
}

#[test]
fn test_nested_access_chain() {
    let expr = SExpr::list(vec![
        SExpr::atom("root"),
        SExpr::list(vec![SExpr::atom("child"), SExpr::atom("value")]),
    ]);
    let child_list = expr.as_list().unwrap()[1].as_list().unwrap();
    assert_eq!(child_list[0].as_atom().unwrap(), "child");
    assert_eq!(child_list[1].as_atom().unwrap(), "value");
}

// ── 7. Clone/Debug/PartialEq (5 tests) ─────────────────────────────────────

#[test]
fn test_clone_atom() {
    let a = SExpr::atom("x");
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn test_clone_list() {
    let l = SExpr::list(vec![SExpr::atom("a"), SExpr::atom("b")]);
    let l2 = l.clone();
    assert_eq!(l, l2);
}

#[test]
fn test_debug_atom() {
    let a = SExpr::atom("test");
    let debug_str = format!("{a:?}");
    assert!(debug_str.contains("Atom"));
    assert!(debug_str.contains("test"));
}

#[test]
fn test_debug_list() {
    let l = SExpr::list(vec![SExpr::atom("x")]);
    let debug_str = format!("{l:?}");
    assert!(debug_str.contains("List"));
}

#[test]
fn test_partial_eq_different_variants() {
    let a = SExpr::atom("x");
    let l = SExpr::list(vec![SExpr::atom("x")]);
    assert_ne!(a, l);
}

// ── 8. Edge cases (8 tests) ────────────────────────────────────────────────

#[test]
fn test_edge_empty_list_display() {
    assert_eq!(format!("{}", SExpr::list(vec![])), "()");
}

#[test]
fn test_edge_deeply_nested_10_levels() {
    let mut expr = SExpr::atom("bottom");
    for _ in 0..10 {
        expr = SExpr::list(vec![expr]);
    }
    let display = format!("{expr}");
    assert!(display.starts_with("(((((((((("));
    assert!(display.ends_with("bottom))))))))))"));
}

#[test]
fn test_edge_single_atom_is_not_list() {
    let a = SExpr::atom("solo");
    assert!(a.is_atom());
    assert!(!a.is_list());
    assert!(a.as_list().is_none());
}

#[test]
fn test_edge_empty_atom() {
    let a = SExpr::atom("");
    assert_eq!(a.as_atom().unwrap(), "");
    assert_eq!(format!("{a}"), "");
}

#[test]
fn test_edge_list_with_empty_atom() {
    let l = SExpr::list(vec![SExpr::atom("")]);
    assert_eq!(format!("{l}"), "()");
}

#[test]
fn test_edge_many_children() {
    let children: Vec<SExpr> = (0..100).map(|i| SExpr::atom(&i.to_string())).collect();
    let l = SExpr::list(children);
    let items = l.as_list().unwrap();
    assert_eq!(items.len(), 100);
    assert_eq!(items[0].as_atom().unwrap(), "0");
    assert_eq!(items[99].as_atom().unwrap(), "99");
}

#[test]
fn test_edge_parse_sexpr_returns_operator_list() {
    let result = parse_sexpr("(+ 1 2)");
    assert!(result.is_ok());
    let expr = result.unwrap();
    assert_eq!(
        expr,
        SExpr::List(vec![
            SExpr::Atom("+".to_string()),
            SExpr::Atom("1".to_string()),
            SExpr::Atom("2".to_string())
        ])
    );
}

#[test]
fn test_edge_parse_sexpr_distinguishes_input_and_rejects_empty() {
    let r1 = parse_sexpr("anything").unwrap();
    let r2 = parse_sexpr("");
    let r3 = parse_sexpr("(deeply (nested))").unwrap();
    assert_eq!(r1, SExpr::Atom("anything".to_string()));
    assert!(r2.is_err());
    assert_ne!(r1, r3);
}

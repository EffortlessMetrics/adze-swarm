#![cfg(feature = "serialization")]

//! Serialization v4 tests for `adze::serialization`.
//!
//! 55+ tests covering SExpr constructors, type predicates, accessors,
//! Display formatting, equality, cloning, parse_sexpr behavior, and edge cases.

use adze::serialization::{SExpr, parse_sexpr};

// ===========================================================================
// SExpr::atom constructor
// ===========================================================================

#[test]
fn test_atom_from_simple_string() {
    let a = SExpr::atom("hello");
    assert!(a.is_atom());
    assert_eq!(a.as_atom(), Some("hello"));
}

#[test]
fn test_atom_from_empty_string() {
    let a = SExpr::atom("");
    assert!(a.is_atom());
    assert_eq!(a.as_atom(), Some(""));
}

#[test]
fn test_atom_from_numeric_string() {
    let a = SExpr::atom("42");
    assert_eq!(a.as_atom(), Some("42"));
}

#[test]
fn test_atom_from_whitespace_string() {
    let a = SExpr::atom("  spaces  ");
    assert_eq!(a.as_atom(), Some("  spaces  "));
}

#[test]
fn test_atom_from_unicode() {
    let a = SExpr::atom("日本語");
    assert_eq!(a.as_atom(), Some("日本語"));
}

#[test]
fn test_atom_from_emoji() {
    let a = SExpr::atom("🦀🔥");
    assert_eq!(a.as_atom(), Some("🦀🔥"));
}

#[test]
fn test_atom_from_special_chars() {
    let a = SExpr::atom("(parens)");
    assert_eq!(a.as_atom(), Some("(parens)"));
}

#[test]
fn test_atom_from_newline() {
    let a = SExpr::atom("line\nbreak");
    assert_eq!(a.as_atom(), Some("line\nbreak"));
}

// ===========================================================================
// SExpr::list constructor
// ===========================================================================

#[test]
fn test_list_empty() {
    let l = SExpr::list(vec![]);
    assert!(l.is_list());
    assert_eq!(l.as_list(), Some([].as_slice()));
}

#[test]
fn test_list_single_atom() {
    let l = SExpr::list(vec![SExpr::atom("x")]);
    let items = l.as_list().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].as_atom(), Some("x"));
}

#[test]
fn test_list_multiple_atoms() {
    let l = SExpr::list(vec![SExpr::atom("a"), SExpr::atom("b"), SExpr::atom("c")]);
    let items = l.as_list().unwrap();
    assert_eq!(items.len(), 3);
}

#[test]
fn test_list_nested() {
    let inner = SExpr::list(vec![SExpr::atom("inner")]);
    let outer = SExpr::list(vec![inner]);
    let items = outer.as_list().unwrap();
    assert_eq!(items.len(), 1);
    assert!(items[0].is_list());
}

#[test]
fn test_list_deeply_nested() {
    let mut expr = SExpr::atom("leaf");
    for _ in 0..10 {
        expr = SExpr::list(vec![expr]);
    }
    assert!(expr.is_list());
}

#[test]
fn test_list_mixed_atoms_and_lists() {
    let l = SExpr::list(vec![
        SExpr::atom("fn"),
        SExpr::list(vec![SExpr::atom("x"), SExpr::atom("y")]),
        SExpr::atom("body"),
    ]);
    let items = l.as_list().unwrap();
    assert_eq!(items.len(), 3);
    assert!(items[0].is_atom());
    assert!(items[1].is_list());
    assert!(items[2].is_atom());
}

// ===========================================================================
// Type predicates: is_atom / is_list
// ===========================================================================

#[test]
fn test_atom_is_atom_true() {
    assert!(SExpr::atom("x").is_atom());
}

#[test]
fn test_atom_is_list_false() {
    assert!(!SExpr::atom("x").is_list());
}

#[test]
fn test_list_is_list_true() {
    assert!(SExpr::list(vec![]).is_list());
}

#[test]
fn test_list_is_atom_false() {
    assert!(!SExpr::list(vec![]).is_atom());
}

#[test]
fn test_nonempty_list_is_list() {
    assert!(SExpr::list(vec![SExpr::atom("a")]).is_list());
}

#[test]
fn test_nonempty_list_is_not_atom() {
    assert!(!SExpr::list(vec![SExpr::atom("a")]).is_atom());
}

// ===========================================================================
// Accessors: as_atom / as_list
// ===========================================================================

#[test]
fn test_as_atom_on_atom_returns_some() {
    assert_eq!(SExpr::atom("val").as_atom(), Some("val"));
}

#[test]
fn test_as_atom_on_list_returns_none() {
    assert_eq!(SExpr::list(vec![]).as_atom(), None);
}

#[test]
fn test_as_list_on_list_returns_some() {
    let l = SExpr::list(vec![SExpr::atom("a")]);
    assert!(l.as_list().is_some());
}

#[test]
fn test_as_list_on_atom_returns_none() {
    assert!(SExpr::atom("x").as_list().is_none());
}

#[test]
fn test_as_list_empty_slice() {
    let l = SExpr::list(vec![]);
    let items = l.as_list().unwrap();
    assert!(items.is_empty());
}

#[test]
fn test_as_list_preserves_order() {
    let l = SExpr::list(vec![
        SExpr::atom("first"),
        SExpr::atom("second"),
        SExpr::atom("third"),
    ]);
    let items = l.as_list().unwrap();
    assert_eq!(items[0].as_atom(), Some("first"));
    assert_eq!(items[1].as_atom(), Some("second"));
    assert_eq!(items[2].as_atom(), Some("third"));
}

// ===========================================================================
// Display formatting
// ===========================================================================

#[test]
fn test_display_atom_simple() {
    assert_eq!(format!("{}", SExpr::atom("hello")), "hello");
}

#[test]
fn test_display_atom_empty() {
    assert_eq!(format!("{}", SExpr::atom("")), "");
}

#[test]
fn test_display_atom_with_spaces() {
    assert_eq!(format!("{}", SExpr::atom("a b")), "a b");
}

#[test]
fn test_display_list_empty() {
    assert_eq!(format!("{}", SExpr::list(vec![])), "()");
}

#[test]
fn test_display_list_single() {
    let l = SExpr::list(vec![SExpr::atom("x")]);
    assert_eq!(format!("{l}"), "(x)");
}

#[test]
fn test_display_list_multiple() {
    let l = SExpr::list(vec![SExpr::atom("a"), SExpr::atom("b"), SExpr::atom("c")]);
    assert_eq!(format!("{l}"), "(a b c)");
}

#[test]
fn test_display_nested_list() {
    let inner = SExpr::list(vec![SExpr::atom("1"), SExpr::atom("2")]);
    let outer = SExpr::list(vec![SExpr::atom("fn"), inner]);
    assert_eq!(format!("{outer}"), "(fn (1 2))");
}

#[test]
fn test_display_deeply_nested() {
    let l = SExpr::list(vec![SExpr::list(vec![SExpr::list(vec![SExpr::atom(
        "deep",
    )])])]);
    assert_eq!(format!("{l}"), "(((deep)))");
}

#[test]
fn test_display_mixed_nesting() {
    let expr = SExpr::list(vec![
        SExpr::atom("+"),
        SExpr::list(vec![SExpr::atom("*"), SExpr::atom("2"), SExpr::atom("3")]),
        SExpr::atom("4"),
    ]);
    assert_eq!(format!("{expr}"), "(+ (* 2 3) 4)");
}

#[test]
fn test_display_unicode_atom() {
    assert_eq!(format!("{}", SExpr::atom("λ")), "λ");
}

#[test]
fn test_display_unicode_in_list() {
    let l = SExpr::list(vec![SExpr::atom("α"), SExpr::atom("β")]);
    assert_eq!(format!("{l}"), "(α β)");
}

// ===========================================================================
// Equality (PartialEq, Eq)
// ===========================================================================

#[test]
fn test_atom_equality_same() {
    assert_eq!(SExpr::atom("x"), SExpr::atom("x"));
}

#[test]
fn test_atom_inequality_different() {
    assert_ne!(SExpr::atom("x"), SExpr::atom("y"));
}

#[test]
fn test_list_equality_empty() {
    assert_eq!(SExpr::list(vec![]), SExpr::list(vec![]));
}

#[test]
fn test_list_equality_same_contents() {
    let a = SExpr::list(vec![SExpr::atom("x")]);
    let b = SExpr::list(vec![SExpr::atom("x")]);
    assert_eq!(a, b);
}

#[test]
fn test_list_inequality_different_contents() {
    let a = SExpr::list(vec![SExpr::atom("x")]);
    let b = SExpr::list(vec![SExpr::atom("y")]);
    assert_ne!(a, b);
}

#[test]
fn test_atom_not_equal_to_list() {
    assert_ne!(SExpr::atom("x"), SExpr::list(vec![SExpr::atom("x")]));
}

#[test]
fn test_list_not_equal_to_atom() {
    assert_ne!(SExpr::list(vec![]), SExpr::atom(""));
}

#[test]
fn test_nested_equality() {
    let a = SExpr::list(vec![SExpr::list(vec![SExpr::atom("a")])]);
    let b = SExpr::list(vec![SExpr::list(vec![SExpr::atom("a")])]);
    assert_eq!(a, b);
}

#[test]
fn test_nested_inequality_depth() {
    let a = SExpr::list(vec![SExpr::atom("a")]);
    let b = SExpr::list(vec![SExpr::list(vec![SExpr::atom("a")])]);
    assert_ne!(a, b);
}

// ===========================================================================
// Clone
// ===========================================================================

#[test]
fn test_clone_atom() {
    let a = SExpr::atom("hello");
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn test_clone_list() {
    let l = SExpr::list(vec![SExpr::atom("a"), SExpr::atom("b")]);
    let c = l.clone();
    assert_eq!(l, c);
}

#[test]
fn test_clone_independence() {
    let original = SExpr::list(vec![SExpr::atom("a")]);
    let cloned = original.clone();
    // They are equal but distinct allocations
    assert_eq!(original, cloned);
    assert!(original.is_list());
    assert!(cloned.is_list());
}

// ===========================================================================
// Debug
// ===========================================================================

#[test]
fn test_debug_atom() {
    let dbg = format!("{:?}", SExpr::atom("x"));
    assert!(dbg.contains("Atom"));
    assert!(dbg.contains("x"));
}

#[test]
fn test_debug_list() {
    let dbg = format!("{:?}", SExpr::list(vec![]));
    assert!(dbg.contains("List"));
}

// ===========================================================================
// parse_sexpr behavior
// ===========================================================================

#[test]
fn test_parse_sexpr_returns_ok() {
    let result = parse_sexpr("(a b c)");
    assert!(result.is_ok());
}

#[test]
fn test_parse_sexpr_returns_list() {
    let result = parse_sexpr("(anything)").unwrap();
    assert!(result.is_list());
}

#[test]
fn test_parse_sexpr_list_keeps_items() {
    let result = parse_sexpr("(+ 1 2)").unwrap();
    let items = result.as_list().unwrap();
    assert_eq!(items.len(), 3);
}

#[test]
fn test_parse_sexpr_empty_input() {
    let result = parse_sexpr("");
    assert!(result.is_err());
}

#[test]
fn test_parse_sexpr_whitespace_input() {
    let result = parse_sexpr("   ");
    assert!(result.is_err());
}

#[test]
fn test_parse_sexpr_nested_input() {
    let result = parse_sexpr("((nested))");
    assert!(result.is_ok());
}

// ===========================================================================
// Pattern matching on SExpr
// ===========================================================================

#[test]
fn test_match_atom_variant() {
    let expr = SExpr::atom("val");
    match expr {
        SExpr::Atom(ref s) => assert_eq!(s, "val"),
        SExpr::List(_) => panic!("expected Atom"),
    }
}

#[test]
fn test_match_list_variant() {
    let expr = SExpr::list(vec![SExpr::atom("a")]);
    match expr {
        SExpr::Atom(_) => panic!("expected List"),
        SExpr::List(ref items) => assert_eq!(items.len(), 1),
    }
}

// ===========================================================================
// Edge cases and stress
// ===========================================================================

#[test]
fn test_atom_with_null_byte() {
    let a = SExpr::atom("a\0b");
    assert_eq!(a.as_atom(), Some("a\0b"));
}

#[test]
fn test_large_atom() {
    let big = "x".repeat(10_000);
    let a = SExpr::atom(&big);
    assert_eq!(a.as_atom().unwrap().len(), 10_000);
}

#[test]
fn test_wide_list() {
    let items: Vec<SExpr> = (0..100).map(|i| SExpr::atom(&i.to_string())).collect();
    let l = SExpr::list(items);
    assert_eq!(l.as_list().unwrap().len(), 100);
}

#[test]
fn test_display_wide_list() {
    let items: Vec<SExpr> = (0..5).map(|i| SExpr::atom(&i.to_string())).collect();
    let l = SExpr::list(items);
    assert_eq!(format!("{l}"), "(0 1 2 3 4)");
}

#[test]
fn test_list_of_empty_lists() {
    let l = SExpr::list(vec![SExpr::list(vec![]), SExpr::list(vec![])]);
    assert_eq!(format!("{l}"), "(() ())");
}

#[test]
fn test_display_to_string() {
    let expr = SExpr::atom("test");
    assert_eq!(expr.to_string(), "test");
}

#[test]
fn test_list_to_string() {
    let expr = SExpr::list(vec![SExpr::atom("a"), SExpr::atom("b")]);
    assert_eq!(expr.to_string(), "(a b)");
}

#[test]
fn test_atom_preserves_backslash() {
    let a = SExpr::atom(r"path\to\file");
    assert_eq!(a.as_atom(), Some(r"path\to\file"));
}

#[test]
fn test_atom_preserves_quotes() {
    let a = SExpr::atom(r#""quoted""#);
    assert_eq!(a.as_atom(), Some(r#""quoted""#));
}

#[test]
fn test_list_single_empty_list() {
    let l = SExpr::list(vec![SExpr::list(vec![])]);
    assert_eq!(format!("{l}"), "(())");
}

#[test]
fn test_sexpr_lisp_like_expression() {
    // (defun add (x y) (+ x y))
    let expr = SExpr::list(vec![
        SExpr::atom("defun"),
        SExpr::atom("add"),
        SExpr::list(vec![SExpr::atom("x"), SExpr::atom("y")]),
        SExpr::list(vec![SExpr::atom("+"), SExpr::atom("x"), SExpr::atom("y")]),
    ]);
    assert_eq!(format!("{expr}"), "(defun add (x y) (+ x y))");
}

#[test]
fn test_sexpr_tree_sitter_style() {
    // (source_file (function_definition name: (identifier)))
    let expr = SExpr::list(vec![
        SExpr::atom("source_file"),
        SExpr::list(vec![
            SExpr::atom("function_definition"),
            SExpr::atom("name:"),
            SExpr::list(vec![SExpr::atom("identifier")]),
        ]),
    ]);
    assert_eq!(
        format!("{expr}"),
        "(source_file (function_definition name: (identifier)))"
    );
}

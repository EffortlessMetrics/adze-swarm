#![cfg(feature = "serialization")]

//! Serialization v5 tests for `adze::serialization`.
//!
//! 64 tests covering SExpr constructors, accessors, Display formatting,
//! nested structures, equality/clone, empty values, complex trees, and edge cases.

use adze::serialization::{SExpr, parse_sexpr};

// ===========================================================================
// 1. SExpr::atom construction and accessors (8 tests)
// ===========================================================================

#[test]
fn test_atom_simple_word() {
    let a = SExpr::atom("identifier");
    assert!(a.is_atom());
    assert!(!a.is_list());
    assert_eq!(a.as_atom(), Some("identifier"));
}

#[test]
fn test_atom_as_list_returns_none() {
    let a = SExpr::atom("value");
    assert_eq!(a.as_list(), None);
}

#[test]
fn test_atom_numeric_string() {
    let a = SExpr::atom("3.14");
    assert_eq!(a.as_atom(), Some("3.14"));
}

#[test]
fn test_atom_hyphenated() {
    let a = SExpr::atom("my-node-kind");
    assert_eq!(a.as_atom(), Some("my-node-kind"));
}

#[test]
fn test_atom_with_underscores() {
    let a = SExpr::atom("function_definition");
    assert!(a.is_atom());
    assert_eq!(a.as_atom(), Some("function_definition"));
}

#[test]
fn test_atom_single_char() {
    let a = SExpr::atom("x");
    assert_eq!(a.as_atom(), Some("x"));
}

#[test]
fn test_atom_with_dots() {
    let a = SExpr::atom("std.io.Read");
    assert_eq!(a.as_atom(), Some("std.io.Read"));
}

#[test]
fn test_atom_preserves_case() {
    let a = SExpr::atom("CamelCase");
    assert_eq!(a.as_atom(), Some("CamelCase"));
}

// ===========================================================================
// 2. SExpr::list construction and accessors (8 tests)
// ===========================================================================

#[test]
fn test_list_single_item() {
    let l = SExpr::list(vec![SExpr::atom("a")]);
    assert!(l.is_list());
    assert!(!l.is_atom());
    assert_eq!(l.as_list().unwrap().len(), 1);
}

#[test]
fn test_list_as_atom_returns_none() {
    let l = SExpr::list(vec![]);
    assert_eq!(l.as_atom(), None);
}

#[test]
fn test_list_multiple_atoms() {
    let l = SExpr::list(vec![SExpr::atom("a"), SExpr::atom("b"), SExpr::atom("c")]);
    let items = l.as_list().unwrap();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].as_atom(), Some("a"));
    assert_eq!(items[2].as_atom(), Some("c"));
}

#[test]
fn test_list_two_items() {
    let l = SExpr::list(vec![SExpr::atom("head"), SExpr::atom("tail")]);
    let items = l.as_list().unwrap();
    assert_eq!(items.len(), 2);
}

#[test]
fn test_list_containing_list() {
    let inner = SExpr::list(vec![SExpr::atom("inner")]);
    let outer = SExpr::list(vec![inner]);
    assert!(outer.is_list());
    let items = outer.as_list().unwrap();
    assert!(items[0].is_list());
}

#[test]
fn test_list_mixed_atom_and_list() {
    let l = SExpr::list(vec![
        SExpr::atom("fn"),
        SExpr::list(vec![SExpr::atom("args")]),
    ]);
    let items = l.as_list().unwrap();
    assert!(items[0].is_atom());
    assert!(items[1].is_list());
}

#[test]
fn test_list_accessor_returns_slice() {
    let l = SExpr::list(vec![SExpr::atom("x"), SExpr::atom("y")]);
    let slice = l.as_list().unwrap();
    assert_eq!(slice.len(), 2);
}

#[test]
fn test_list_five_atoms() {
    let items: Vec<SExpr> = (0..5).map(|i| SExpr::atom(&i.to_string())).collect();
    let l = SExpr::list(items);
    assert_eq!(l.as_list().unwrap().len(), 5);
}

// ===========================================================================
// 3. Display formatting for atoms and lists (8 tests)
// ===========================================================================

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
fn test_display_single_item_list() {
    let l = SExpr::list(vec![SExpr::atom("node")]);
    assert_eq!(format!("{l}"), "(node)");
}

#[test]
fn test_display_two_item_list() {
    let l = SExpr::list(vec![SExpr::atom("a"), SExpr::atom("b")]);
    assert_eq!(format!("{l}"), "(a b)");
}

#[test]
fn test_display_three_item_list() {
    let l = SExpr::list(vec![SExpr::atom("add"), SExpr::atom("1"), SExpr::atom("2")]);
    assert_eq!(format!("{l}"), "(add 1 2)");
}

#[test]
fn test_display_nested_list() {
    let inner = SExpr::list(vec![SExpr::atom("x"), SExpr::atom("y")]);
    let outer = SExpr::list(vec![SExpr::atom("pair"), inner]);
    assert_eq!(format!("{outer}"), "(pair (x y))");
}

#[test]
fn test_display_deeply_nested() {
    let d3 = SExpr::list(vec![SExpr::atom("c")]);
    let d2 = SExpr::list(vec![SExpr::atom("b"), d3]);
    let d1 = SExpr::list(vec![SExpr::atom("a"), d2]);
    assert_eq!(format!("{d1}"), "(a (b (c)))");
}

#[test]
fn test_display_atom_with_spaces() {
    let a = SExpr::atom("has space");
    assert_eq!(format!("{a}"), "has space");
}

// ===========================================================================
// 4. Nested list construction (8 tests)
// ===========================================================================

#[test]
fn test_nested_two_levels() {
    let inner = SExpr::list(vec![SExpr::atom("leaf")]);
    let outer = SExpr::list(vec![inner]);
    let items = outer.as_list().unwrap();
    assert_eq!(items[0].as_list().unwrap()[0].as_atom(), Some("leaf"));
}

#[test]
fn test_nested_three_levels() {
    let l3 = SExpr::list(vec![SExpr::atom("deep")]);
    let l2 = SExpr::list(vec![l3]);
    let l1 = SExpr::list(vec![l2]);
    let inner = l1.as_list().unwrap()[0].as_list().unwrap()[0]
        .as_list()
        .unwrap();
    assert_eq!(inner[0].as_atom(), Some("deep"));
}

#[test]
fn test_nested_siblings_at_same_level() {
    let l = SExpr::list(vec![
        SExpr::list(vec![SExpr::atom("a")]),
        SExpr::list(vec![SExpr::atom("b")]),
    ]);
    let items = l.as_list().unwrap();
    assert_eq!(items[0].as_list().unwrap()[0].as_atom(), Some("a"));
    assert_eq!(items[1].as_list().unwrap()[0].as_atom(), Some("b"));
}

#[test]
fn test_nested_binary_tree_shape() {
    let leaf_a = SExpr::atom("a");
    let leaf_b = SExpr::atom("b");
    let left = SExpr::list(vec![leaf_a]);
    let right = SExpr::list(vec![leaf_b]);
    let root = SExpr::list(vec![SExpr::atom("tree"), left, right]);
    assert_eq!(format!("{root}"), "(tree (a) (b))");
}

#[test]
fn test_nested_with_head_atom() {
    let l = SExpr::list(vec![
        SExpr::atom("define"),
        SExpr::atom("x"),
        SExpr::list(vec![SExpr::atom("+"), SExpr::atom("1"), SExpr::atom("2")]),
    ]);
    assert_eq!(format!("{l}"), "(define x (+ 1 2))");
}

#[test]
fn test_nested_empty_inner_list() {
    let l = SExpr::list(vec![SExpr::atom("wrap"), SExpr::list(vec![])]);
    assert_eq!(format!("{l}"), "(wrap ())");
}

#[test]
fn test_nested_list_of_empty_lists() {
    let l = SExpr::list(vec![SExpr::list(vec![]), SExpr::list(vec![])]);
    assert_eq!(format!("{l}"), "(() ())");
}

#[test]
fn test_nested_mixed_depths() {
    let l = SExpr::list(vec![
        SExpr::atom("flat"),
        SExpr::list(vec![SExpr::list(vec![SExpr::atom("deep")])]),
    ]);
    let items = l.as_list().unwrap();
    assert!(items[0].is_atom());
    assert!(items[1].is_list());
}

// ===========================================================================
// 5. Equality and Clone (8 tests)
// ===========================================================================

#[test]
fn test_atom_equality() {
    let a1 = SExpr::atom("same");
    let a2 = SExpr::atom("same");
    assert_eq!(a1, a2);
}

#[test]
fn test_atom_inequality() {
    let a1 = SExpr::atom("foo");
    let a2 = SExpr::atom("bar");
    assert_ne!(a1, a2);
}

#[test]
fn test_list_equality() {
    let l1 = SExpr::list(vec![SExpr::atom("a"), SExpr::atom("b")]);
    let l2 = SExpr::list(vec![SExpr::atom("a"), SExpr::atom("b")]);
    assert_eq!(l1, l2);
}

#[test]
fn test_list_inequality_different_length() {
    let l1 = SExpr::list(vec![SExpr::atom("a")]);
    let l2 = SExpr::list(vec![SExpr::atom("a"), SExpr::atom("b")]);
    assert_ne!(l1, l2);
}

#[test]
fn test_atom_vs_list_not_equal() {
    let a = SExpr::atom("a");
    let l = SExpr::list(vec![SExpr::atom("a")]);
    assert_ne!(a, l);
}

#[test]
fn test_clone_atom_equals_original() {
    let a = SExpr::atom("cloned");
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn test_clone_list_equals_original() {
    let l = SExpr::list(vec![SExpr::atom("x"), SExpr::atom("y")]);
    let c = l.clone();
    assert_eq!(l, c);
}

#[test]
fn test_clone_nested_list_equals_original() {
    let nested = SExpr::list(vec![
        SExpr::atom("root"),
        SExpr::list(vec![SExpr::atom("child")]),
    ]);
    let c = nested.clone();
    assert_eq!(nested, c);
}

// ===========================================================================
// 6. Empty list, empty atom (8 tests)
// ===========================================================================

#[test]
fn test_empty_atom_is_atom() {
    let a = SExpr::atom("");
    assert!(a.is_atom());
}

#[test]
fn test_empty_atom_as_atom() {
    let a = SExpr::atom("");
    assert_eq!(a.as_atom(), Some(""));
}

#[test]
fn test_empty_atom_display() {
    let a = SExpr::atom("");
    assert_eq!(format!("{a}"), "");
}

#[test]
fn test_empty_list_is_list() {
    let l = SExpr::list(vec![]);
    assert!(l.is_list());
}

#[test]
fn test_empty_list_as_list_len() {
    let l = SExpr::list(vec![]);
    assert!(l.as_list().unwrap().is_empty());
}

#[test]
fn test_empty_list_display_parens() {
    let l = SExpr::list(vec![]);
    assert_eq!(format!("{l}"), "()");
}

#[test]
fn test_empty_atom_not_equal_to_empty_list() {
    let a = SExpr::atom("");
    let l = SExpr::list(vec![]);
    assert_ne!(a, l);
}

#[test]
fn test_empty_list_clone_equals() {
    let l = SExpr::list(vec![]);
    let c = l.clone();
    assert_eq!(l, c);
}

// ===========================================================================
// 7. Complex nested structures (8 tests)
// ===========================================================================

#[test]
fn test_complex_function_call() {
    // (call "print" (args "hello" "world"))
    let expr = SExpr::list(vec![
        SExpr::atom("call"),
        SExpr::atom("print"),
        SExpr::list(vec![
            SExpr::atom("args"),
            SExpr::atom("hello"),
            SExpr::atom("world"),
        ]),
    ]);
    assert_eq!(format!("{expr}"), "(call print (args hello world))");
}

#[test]
fn test_complex_if_then_else() {
    // (if cond (then a) (else b))
    let expr = SExpr::list(vec![
        SExpr::atom("if"),
        SExpr::atom("cond"),
        SExpr::list(vec![SExpr::atom("then"), SExpr::atom("a")]),
        SExpr::list(vec![SExpr::atom("else"), SExpr::atom("b")]),
    ]);
    assert_eq!(format!("{expr}"), "(if cond (then a) (else b))");
}

#[test]
fn test_complex_nested_arithmetic() {
    // (+ (* 2 3) (- 10 4))
    let expr = SExpr::list(vec![
        SExpr::atom("+"),
        SExpr::list(vec![SExpr::atom("*"), SExpr::atom("2"), SExpr::atom("3")]),
        SExpr::list(vec![SExpr::atom("-"), SExpr::atom("10"), SExpr::atom("4")]),
    ]);
    assert_eq!(format!("{expr}"), "(+ (* 2 3) (- 10 4))");
}

#[test]
fn test_complex_triple_nesting() {
    // (a (b (c (d))))
    let expr = SExpr::list(vec![
        SExpr::atom("a"),
        SExpr::list(vec![
            SExpr::atom("b"),
            SExpr::list(vec![SExpr::atom("c"), SExpr::list(vec![SExpr::atom("d")])]),
        ]),
    ]);
    assert_eq!(format!("{expr}"), "(a (b (c (d))))");
}

#[test]
fn test_complex_program_structure() {
    // (program (fn main (block (return 0))))
    let expr = SExpr::list(vec![
        SExpr::atom("program"),
        SExpr::list(vec![
            SExpr::atom("fn"),
            SExpr::atom("main"),
            SExpr::list(vec![
                SExpr::atom("block"),
                SExpr::list(vec![SExpr::atom("return"), SExpr::atom("0")]),
            ]),
        ]),
    ]);
    assert_eq!(format!("{expr}"), "(program (fn main (block (return 0))))");
}

#[test]
fn test_complex_wide_tree() {
    let children: Vec<SExpr> = (0..6).map(|i| SExpr::atom(&format!("n{i}"))).collect();
    let tree = SExpr::list(children);
    assert_eq!(format!("{tree}"), "(n0 n1 n2 n3 n4 n5)");
}

#[test]
fn test_complex_equality_deep() {
    let make_tree = || {
        SExpr::list(vec![
            SExpr::atom("root"),
            SExpr::list(vec![
                SExpr::atom("left"),
                SExpr::list(vec![SExpr::atom("ll")]),
            ]),
            SExpr::list(vec![SExpr::atom("right")]),
        ])
    };
    assert_eq!(make_tree(), make_tree());
}

#[test]
fn test_complex_inequality_different_structure() {
    let a = SExpr::list(vec![SExpr::atom("x"), SExpr::list(vec![SExpr::atom("y")])]);
    let b = SExpr::list(vec![SExpr::atom("x"), SExpr::atom("y")]);
    assert_ne!(a, b);
}

// ===========================================================================
// 8. Edge cases: deeply nested, many items, special characters (8 tests)
// ===========================================================================

#[test]
fn test_edge_deeply_nested_10_levels() {
    let mut current = SExpr::atom("bottom");
    for _ in 0..10 {
        current = SExpr::list(vec![current]);
    }
    assert!(current.is_list());
    // Traverse 10 levels
    let mut node = &current;
    for _ in 0..10 {
        node = &node.as_list().unwrap()[0];
    }
    assert_eq!(node.as_atom(), Some("bottom"));
}

#[test]
fn test_edge_many_items_100() {
    let items: Vec<SExpr> = (0..100).map(|i| SExpr::atom(&i.to_string())).collect();
    let l = SExpr::list(items);
    let slice = l.as_list().unwrap();
    assert_eq!(slice.len(), 100);
    assert_eq!(slice[0].as_atom(), Some("0"));
    assert_eq!(slice[99].as_atom(), Some("99"));
}

#[test]
fn test_edge_special_char_backslash() {
    let a = SExpr::atom("path\\to\\file");
    assert_eq!(a.as_atom(), Some("path\\to\\file"));
}

#[test]
fn test_edge_special_char_quotes() {
    let a = SExpr::atom(r#""quoted""#);
    assert_eq!(a.as_atom(), Some(r#""quoted""#));
}

#[test]
fn test_edge_special_char_tab_newline() {
    let a = SExpr::atom("tab\there\nnewline");
    assert_eq!(a.as_atom(), Some("tab\there\nnewline"));
}

#[test]
fn test_edge_special_char_null_byte() {
    let a = SExpr::atom("before\0after");
    assert_eq!(a.as_atom(), Some("before\0after"));
}

#[test]
fn test_edge_unicode_multibyte() {
    let a = SExpr::atom("αβγδ");
    assert_eq!(a.as_atom(), Some("αβγδ"));
    let l = SExpr::list(vec![SExpr::atom("λ"), SExpr::atom("→")]);
    assert_eq!(format!("{l}"), "(λ →)");
}

#[test]
fn test_edge_parse_sexpr_returns_result() {
    let result = parse_sexpr("(anything)");
    assert!(result.is_ok());
}

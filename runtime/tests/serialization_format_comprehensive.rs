//! Comprehensive tests for the serialization module's public API types.
//!
//! Covers: SerializedNode, SExpr, parse_sexpr, TreeSerializer, CompactNode,
//! SExpressionSerializer, BinarySerializer, BinaryFormat — construction,
//! traits, builder patterns, edge cases, and collection usage.

#![cfg(feature = "serialization")]

use adze::serialization::{
    BinaryFormat, BinarySerializer, CompactNode, SExpr, SExpressionSerializer, SerializedNode,
    TreeSerializer, parse_sexpr,
};
use std::collections::HashMap;

// ===== Helper =====

fn leaf_node(kind: &str, text: &str, start: usize, end: usize) -> SerializedNode {
    SerializedNode {
        kind: kind.to_string(),
        is_named: true,
        field_name: None,
        start_position: (0, start),
        end_position: (0, end),
        start_byte: start,
        end_byte: end,
        text: Some(text.to_string()),
        children: vec![],
        is_error: false,
        is_missing: false,
    }
}

// ===========================================================================
// 1. SerializedNode construction
// ===========================================================================

#[test]
fn serialized_node_basic_leaf() {
    let n = leaf_node("number", "42", 0, 2);
    assert_eq!(n.kind, "number");
    assert_eq!(n.text.as_deref(), Some("42"));
    assert!(n.children.is_empty());
}

#[test]
fn serialized_node_all_fields_set() {
    let n = SerializedNode {
        kind: "string_literal".to_string(),
        is_named: true,
        field_name: Some("value".to_string()),
        start_position: (3, 7),
        end_position: (3, 20),
        start_byte: 55,
        end_byte: 68,
        text: Some("hello world".to_string()),
        children: vec![],
        is_error: false,
        is_missing: false,
    };
    assert_eq!(n.start_position, (3, 7));
    assert_eq!(n.end_position, (3, 20));
    assert_eq!(n.start_byte, 55);
    assert_eq!(n.end_byte, 68);
    assert_eq!(n.field_name.as_deref(), Some("value"));
}

#[test]
fn serialized_node_error_flag() {
    let n = SerializedNode {
        kind: "ERROR".to_string(),
        is_named: false,
        field_name: None,
        start_position: (0, 0),
        end_position: (0, 1),
        start_byte: 0,
        end_byte: 1,
        text: Some("@".to_string()),
        children: vec![],
        is_error: true,
        is_missing: false,
    };
    assert!(n.is_error);
    assert!(!n.is_missing);
}

#[test]
fn serialized_node_missing_flag() {
    let n = SerializedNode {
        kind: "semicolon".to_string(),
        is_named: false,
        field_name: None,
        start_position: (1, 0),
        end_position: (1, 0),
        start_byte: 10,
        end_byte: 10,
        text: None,
        children: vec![],
        is_error: false,
        is_missing: true,
    };
    assert!(n.is_missing);
    assert_eq!(n.start_byte, n.end_byte);
}

#[test]
fn serialized_node_unnamed() {
    let n = SerializedNode {
        kind: "+".to_string(),
        is_named: false,
        field_name: None,
        start_position: (0, 2),
        end_position: (0, 3),
        start_byte: 2,
        end_byte: 3,
        text: Some("+".to_string()),
        children: vec![],
        is_error: false,
        is_missing: false,
    };
    assert!(!n.is_named);
}

#[test]
fn serialized_node_with_children() {
    let left = leaf_node("number", "1", 0, 1);
    let right = leaf_node("number", "2", 4, 5);
    let parent = SerializedNode {
        kind: "binary_expression".to_string(),
        is_named: true,
        field_name: None,
        start_position: (0, 0),
        end_position: (0, 5),
        start_byte: 0,
        end_byte: 5,
        text: None,
        children: vec![left, right],
        is_error: false,
        is_missing: false,
    };
    assert_eq!(parent.children.len(), 2);
    assert_eq!(parent.children[0].kind, "number");
    assert_eq!(parent.children[1].text.as_deref(), Some("2"));
}

#[test]
fn serialized_node_deeply_nested() {
    let inner = leaf_node("id", "x", 0, 1);
    let mid = SerializedNode {
        kind: "paren".to_string(),
        is_named: true,
        field_name: None,
        start_position: (0, 0),
        end_position: (0, 3),
        start_byte: 0,
        end_byte: 3,
        text: None,
        children: vec![inner],
        is_error: false,
        is_missing: false,
    };
    let outer = SerializedNode {
        kind: "expr".to_string(),
        is_named: true,
        field_name: None,
        start_position: (0, 0),
        end_position: (0, 3),
        start_byte: 0,
        end_byte: 3,
        text: None,
        children: vec![mid],
        is_error: false,
        is_missing: false,
    };
    assert_eq!(outer.children[0].children[0].kind, "id");
}

#[test]
fn serialized_node_no_text_for_parent() {
    let parent = SerializedNode {
        kind: "program".to_string(),
        is_named: true,
        field_name: None,
        start_position: (0, 0),
        end_position: (5, 0),
        start_byte: 0,
        end_byte: 100,
        text: None,
        children: vec![leaf_node("id", "main", 0, 4)],
        is_error: false,
        is_missing: false,
    };
    assert!(parent.text.is_none());
}

// ===========================================================================
// 2. SerializedNode clone/debug
// ===========================================================================

#[test]
fn serialized_node_clone() {
    let orig = leaf_node("id", "abc", 0, 3);
    let cloned = orig.clone();
    assert_eq!(cloned.kind, "id");
    assert_eq!(cloned.text.as_deref(), Some("abc"));
}

#[test]
fn serialized_node_clone_with_children() {
    let parent = SerializedNode {
        kind: "block".to_string(),
        is_named: true,
        field_name: None,
        start_position: (0, 0),
        end_position: (2, 1),
        start_byte: 0,
        end_byte: 20,
        text: None,
        children: vec![leaf_node("stmt", "x", 0, 1), leaf_node("stmt", "y", 3, 4)],
        is_error: false,
        is_missing: false,
    };
    let cloned = parent.clone();
    assert_eq!(cloned.children.len(), 2);
    assert_eq!(cloned.children[1].kind, "stmt");
}

#[test]
fn serialized_node_debug_contains_kind() {
    let n = leaf_node("keyword", "fn", 0, 2);
    let dbg = format!("{:?}", n);
    assert!(dbg.contains("keyword"));
    assert!(dbg.contains("fn"));
}

#[test]
fn serialized_node_debug_shows_flags() {
    let n = SerializedNode {
        kind: "ERR".to_string(),
        is_named: false,
        field_name: None,
        start_position: (0, 0),
        end_position: (0, 0),
        start_byte: 0,
        end_byte: 0,
        text: None,
        children: vec![],
        is_error: true,
        is_missing: true,
    };
    let dbg = format!("{:?}", n);
    assert!(dbg.contains("is_error: true"));
    assert!(dbg.contains("is_missing: true"));
}

// ===========================================================================
// 3. SExpr variants
// ===========================================================================

#[test]
fn sexpr_atom_creation() {
    let a = SExpr::Atom("hello".to_string());
    assert_eq!(a, SExpr::Atom("hello".to_string()));
}

#[test]
fn sexpr_atom_empty_string() {
    let a = SExpr::Atom(String::new());
    assert_eq!(a, SExpr::Atom("".to_string()));
}

#[test]
fn sexpr_list_empty() {
    let l = SExpr::List(vec![]);
    assert_eq!(l, SExpr::List(vec![]));
}

#[test]
fn sexpr_list_with_atoms() {
    let l = SExpr::List(vec![
        SExpr::Atom("a".to_string()),
        SExpr::Atom("b".to_string()),
    ]);
    if let SExpr::List(items) = &l {
        assert_eq!(items.len(), 2);
    } else {
        panic!("expected list");
    }
}

#[test]
fn sexpr_nested_lists() {
    let inner = SExpr::List(vec![SExpr::Atom("x".to_string())]);
    let outer = SExpr::List(vec![inner.clone()]);
    if let SExpr::List(items) = &outer {
        assert_eq!(items.len(), 1);
        assert_eq!(items[0], inner);
    }
}

#[test]
fn sexpr_mixed_atoms_and_lists() {
    let s = SExpr::List(vec![
        SExpr::Atom("define".to_string()),
        SExpr::List(vec![SExpr::Atom("f".to_string())]),
        SExpr::Atom("body".to_string()),
    ]);
    if let SExpr::List(items) = &s {
        assert_eq!(items.len(), 3);
    }
}

// ===========================================================================
// 4. SExpr clone/debug/eq
// ===========================================================================

#[test]
fn sexpr_clone_atom() {
    let a = SExpr::Atom("test".to_string());
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn sexpr_clone_nested() {
    let s = SExpr::List(vec![
        SExpr::Atom("a".to_string()),
        SExpr::List(vec![SExpr::Atom("b".to_string())]),
    ]);
    let cloned = s.clone();
    assert_eq!(s, cloned);
}

#[test]
fn sexpr_debug_atom() {
    let a = SExpr::Atom("hello".to_string());
    let dbg = format!("{:?}", a);
    assert!(dbg.contains("Atom"));
    assert!(dbg.contains("hello"));
}

#[test]
fn sexpr_debug_list() {
    let l = SExpr::List(vec![SExpr::Atom("item".to_string())]);
    let dbg = format!("{:?}", l);
    assert!(dbg.contains("List"));
    assert!(dbg.contains("item"));
}

#[test]
fn sexpr_equality_same() {
    let a = SExpr::Atom("x".to_string());
    let b = SExpr::Atom("x".to_string());
    assert_eq!(a, b);
}

#[test]
fn sexpr_inequality_different_atoms() {
    let a = SExpr::Atom("x".to_string());
    let b = SExpr::Atom("y".to_string());
    assert_ne!(a, b);
}

#[test]
fn sexpr_inequality_atom_vs_list() {
    let a = SExpr::Atom("x".to_string());
    let l = SExpr::List(vec![]);
    assert_ne!(a, l);
}

#[test]
fn sexpr_equality_nested_lists() {
    let mk = || {
        SExpr::List(vec![
            SExpr::Atom("a".to_string()),
            SExpr::List(vec![SExpr::Atom("b".to_string())]),
        ])
    };
    assert_eq!(mk(), mk());
}

// ===========================================================================
// 5. parse_sexpr
// ===========================================================================

#[test]
fn parse_sexpr_empty_string() {
    let r = parse_sexpr("");
    assert!(r.is_err());
}

#[test]
fn parse_sexpr_atom_input() {
    let r = parse_sexpr("hello");
    assert_eq!(r, Ok(SExpr::Atom("hello".to_string())));
}

#[test]
fn parse_sexpr_simple_list() {
    let r = parse_sexpr("(a b c)");
    assert_eq!(
        r,
        Ok(SExpr::List(vec![
            SExpr::Atom("a".to_string()),
            SExpr::Atom("b".to_string()),
            SExpr::Atom("c".to_string()),
        ]))
    );
}

#[test]
fn parse_sexpr_nested_list() {
    let r = parse_sexpr("(define (f x) (+ x 1))");
    assert_eq!(
        r,
        Ok(SExpr::List(vec![
            SExpr::Atom("define".to_string()),
            SExpr::List(vec![
                SExpr::Atom("f".to_string()),
                SExpr::Atom("x".to_string()),
            ]),
            SExpr::List(vec![
                SExpr::Atom("+".to_string()),
                SExpr::Atom("x".to_string()),
                SExpr::Atom("1".to_string()),
            ]),
        ]))
    );
}

#[test]
fn parse_sexpr_numbers() {
    let r = parse_sexpr("42");
    assert_eq!(r, Ok(SExpr::Atom("42".to_string())));
}

#[test]
fn parse_sexpr_special_chars() {
    let r = parse_sexpr("(!@#$%^&*)");
    assert_eq!(
        r,
        Ok(SExpr::List(vec![SExpr::Atom("!@#$%^&*".to_string())]))
    );
}

#[test]
fn parse_sexpr_whitespace_only() {
    let r = parse_sexpr("   \t\n  ");
    assert!(r.is_err());
}

#[test]
fn parse_sexpr_unicode() {
    let r = parse_sexpr("(日本語 テスト)");
    assert_eq!(
        r,
        Ok(SExpr::List(vec![
            SExpr::Atom("日本語".to_string()),
            SExpr::Atom("テスト".to_string()),
        ]))
    );
}

#[test]
fn parse_sexpr_emoji() {
    let r = parse_sexpr("🎉🚀");
    assert_eq!(r, Ok(SExpr::Atom("🎉🚀".to_string())));
}

#[test]
fn parse_sexpr_unbalanced_parens() {
    let r = parse_sexpr("((())");
    assert!(r.is_err());
}

#[test]
fn parse_sexpr_result_is_ok() {
    assert_eq!(
        parse_sexpr("anything"),
        Ok(SExpr::Atom("anything".to_string()))
    );
}

#[test]
fn parse_sexpr_long_input() {
    let long = "a".repeat(10_000);
    let r = parse_sexpr(&long);
    assert_eq!(r, Ok(SExpr::Atom(long)));
}

// ===========================================================================
// 6. TreeSerializer construction
// ===========================================================================

#[test]
fn tree_serializer_default_config() {
    let src = b"source";
    let ts = TreeSerializer::new(src);
    assert!(!ts.include_unnamed);
    assert_eq!(ts.max_text_length, Some(100));
    assert_eq!(ts.source, b"source");
}

#[test]
fn tree_serializer_with_unnamed() {
    let ts = TreeSerializer::new(b"x").with_unnamed_nodes();
    assert!(ts.include_unnamed);
}

#[test]
fn tree_serializer_with_max_text_none() {
    let ts = TreeSerializer::new(b"x").with_max_text_length(None);
    assert_eq!(ts.max_text_length, None);
}

#[test]
fn tree_serializer_with_max_text_custom() {
    let ts = TreeSerializer::new(b"x").with_max_text_length(Some(50));
    assert_eq!(ts.max_text_length, Some(50));
}

#[test]
fn tree_serializer_chained_builders() {
    let ts = TreeSerializer::new(b"code")
        .with_unnamed_nodes()
        .with_max_text_length(Some(200));
    assert!(ts.include_unnamed);
    assert_eq!(ts.max_text_length, Some(200));
}

#[test]
fn tree_serializer_source_preserved() {
    let src = b"fn main() {}";
    let ts = TreeSerializer::new(src);
    assert_eq!(ts.source, b"fn main() {}");
}

#[test]
fn tree_serializer_empty_source() {
    let ts = TreeSerializer::new(b"");
    assert_eq!(ts.source.len(), 0);
}

#[test]
fn tree_serializer_max_text_zero() {
    let ts = TreeSerializer::new(b"x").with_max_text_length(Some(0));
    assert_eq!(ts.max_text_length, Some(0));
}

// ===========================================================================
// 7. TreeSerializer methods — we test config; serialize_tree/serialize_node
//    require a Tree/Node which needs a real parser, so we test config only.
// ===========================================================================

#[test]
fn tree_serializer_override_max_text_twice() {
    let ts = TreeSerializer::new(b"x")
        .with_max_text_length(Some(10))
        .with_max_text_length(Some(999));
    assert_eq!(ts.max_text_length, Some(999));
}

#[test]
fn tree_serializer_unnamed_idempotent() {
    let ts = TreeSerializer::new(b"x")
        .with_unnamed_nodes()
        .with_unnamed_nodes();
    assert!(ts.include_unnamed);
}

// ===========================================================================
// 8. Multiple serialization calls / SExpressionSerializer / CompactNode
// ===========================================================================

#[test]
fn sexpr_serializer_default_config() {
    // include_positions is private; just verify construction succeeds.
    let _s = SExpressionSerializer::new(b"test");
}

#[test]
fn sexpr_serializer_with_positions() {
    // with_positions() builder returns Self; verify it chains.
    let _s = SExpressionSerializer::new(b"test").with_positions();
}

#[test]
fn compact_node_minimal() {
    let cn = CompactNode {
        kind: "id".to_string(),
        start: None,
        end: None,
        field: None,
        children: vec![],
        text: Some("x".to_string()),
    };
    assert_eq!(cn.kind, "id");
    assert!(cn.start.is_none());
}

#[test]
fn compact_node_with_positions() {
    let cn = CompactNode {
        kind: "block".to_string(),
        start: Some(0),
        end: Some(50),
        field: Some("body".to_string()),
        children: vec![],
        text: None,
    };
    assert_eq!(cn.start, Some(0));
    assert_eq!(cn.end, Some(50));
    assert_eq!(cn.field.as_deref(), Some("body"));
}

#[test]
fn compact_node_json_roundtrip() {
    let cn = CompactNode {
        kind: "num".to_string(),
        start: None,
        end: None,
        field: None,
        children: vec![],
        text: Some("7".to_string()),
    };
    let json = serde_json::to_string(&cn).unwrap();
    let back: CompactNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.kind, "num");
    assert_eq!(back.text.as_deref(), Some("7"));
}

#[test]
fn compact_node_clone() {
    let cn = CompactNode {
        kind: "a".to_string(),
        start: Some(1),
        end: Some(2),
        field: None,
        children: vec![CompactNode {
            kind: "b".to_string(),
            start: None,
            end: None,
            field: None,
            children: vec![],
            text: Some("v".to_string()),
        }],
        text: None,
    };
    let cloned = cn.clone();
    assert_eq!(cloned.children.len(), 1);
    assert_eq!(cloned.children[0].kind, "b");
}

#[test]
fn compact_node_debug() {
    let cn = CompactNode {
        kind: "tok".to_string(),
        start: None,
        end: None,
        field: None,
        children: vec![],
        text: None,
    };
    let dbg = format!("{:?}", cn);
    assert!(dbg.contains("tok"));
}

// ===========================================================================
// 8b. BinarySerializer / BinaryFormat
// ===========================================================================

#[test]
fn binary_serializer_new() {
    let _bs = BinarySerializer::new();
}

#[test]
fn binary_serializer_default() {
    let _bs = BinarySerializer::default();
}

#[test]
fn binary_format_debug() {
    let bf = BinaryFormat {
        node_types: vec!["program".to_string()],
        field_names: vec!["name".to_string()],
        tree_data: vec![0u8; 10],
    };
    let dbg = format!("{:?}", bf);
    assert!(dbg.contains("program"));
    assert!(dbg.contains("name"));
}

#[test]
fn binary_format_clone() {
    let bf = BinaryFormat {
        node_types: vec!["id".to_string()],
        field_names: vec![],
        tree_data: vec![1, 2, 3],
    };
    let cloned = bf.clone();
    assert_eq!(cloned.node_types, bf.node_types);
    assert_eq!(cloned.tree_data, bf.tree_data);
}

#[test]
fn binary_format_empty() {
    let bf = BinaryFormat {
        node_types: vec![],
        field_names: vec![],
        tree_data: vec![],
    };
    assert!(bf.node_types.is_empty());
    assert!(bf.tree_data.is_empty());
}

// ===========================================================================
// 9. Edge cases
// ===========================================================================

#[test]
fn serialized_node_empty_kind() {
    let n = SerializedNode {
        kind: String::new(),
        is_named: false,
        field_name: None,
        start_position: (0, 0),
        end_position: (0, 0),
        start_byte: 0,
        end_byte: 0,
        text: None,
        children: vec![],
        is_error: false,
        is_missing: false,
    };
    assert!(n.kind.is_empty());
}

#[test]
fn serialized_node_unicode_kind() {
    let n = leaf_node("識別子", "変数", 0, 6);
    assert_eq!(n.kind, "識別子");
}

#[test]
fn serialized_node_unicode_text() {
    let n = leaf_node("string", "Ünïcödé", 0, 10);
    assert_eq!(n.text.as_deref(), Some("Ünïcödé"));
}

#[test]
fn serialized_node_large_positions() {
    let n = SerializedNode {
        kind: "eof".to_string(),
        is_named: false,
        field_name: None,
        start_position: (999_999, 999_999),
        end_position: (999_999, 999_999),
        start_byte: usize::MAX - 1,
        end_byte: usize::MAX,
        text: None,
        children: vec![],
        is_error: false,
        is_missing: false,
    };
    assert_eq!(n.start_byte, usize::MAX - 1);
}

#[test]
fn serialized_node_many_children() {
    let children: Vec<_> = (0..100)
        .map(|i| leaf_node("item", &i.to_string(), i, i + 1))
        .collect();
    let parent = SerializedNode {
        kind: "list".to_string(),
        is_named: true,
        field_name: None,
        start_position: (0, 0),
        end_position: (0, 100),
        start_byte: 0,
        end_byte: 100,
        text: None,
        children,
        is_error: false,
        is_missing: false,
    };
    assert_eq!(parent.children.len(), 100);
}

#[test]
fn tree_serializer_unicode_source() {
    let src = "日本語プログラム".as_bytes();
    let ts = TreeSerializer::new(src);
    assert_eq!(ts.source.len(), "日本語プログラム".len());
}

#[test]
fn tree_serializer_binary_source() {
    let src: &[u8] = &[0xFF, 0xFE, 0x00, 0x01];
    let ts = TreeSerializer::new(src);
    assert_eq!(ts.source.len(), 4);
}

#[test]
fn parse_sexpr_newlines() {
    assert!(parse_sexpr("\n\n\n").is_err());
}

#[test]
fn parse_sexpr_null_bytes_in_str() {
    assert_eq!(
        parse_sexpr("abc\0def"),
        Ok(SExpr::Atom("abc\0def".to_string()))
    );
}

#[test]
fn sexpr_deeply_nested() {
    // Build 50-deep nesting
    let mut s = SExpr::Atom("leaf".to_string());
    for _ in 0..50 {
        s = SExpr::List(vec![s]);
    }
    // Verify we can clone and debug without stack overflow
    let cloned = s.clone();
    let _ = format!("{:?}", cloned);
}

// ===========================================================================
// 10. SerializedNode in collections
// ===========================================================================

#[test]
fn serialized_node_in_vec() {
    let nodes: Vec<SerializedNode> = vec![
        leaf_node("a", "1", 0, 1),
        leaf_node("b", "2", 2, 3),
        leaf_node("c", "3", 4, 5),
    ];
    assert_eq!(nodes.len(), 3);
    assert_eq!(nodes[1].kind, "b");
}

#[test]
fn serialized_node_in_hashmap() {
    let mut map: HashMap<String, SerializedNode> = HashMap::new();
    map.insert("first".to_string(), leaf_node("id", "x", 0, 1));
    map.insert("second".to_string(), leaf_node("id", "y", 2, 3));
    assert_eq!(map.len(), 2);
    assert_eq!(map["first"].text.as_deref(), Some("x"));
}

#[test]
fn sexpr_in_vec() {
    let items: Vec<SExpr> = vec![
        SExpr::Atom("a".to_string()),
        SExpr::List(vec![]),
        SExpr::Atom("b".to_string()),
    ];
    assert_eq!(items.len(), 3);
}

#[test]
fn sexpr_in_hashmap() {
    let mut map: HashMap<String, SExpr> = HashMap::new();
    map.insert("key".to_string(), SExpr::Atom("val".to_string()));
    assert_eq!(map["key"], SExpr::Atom("val".to_string()));
}

#[test]
fn compact_node_in_vec() {
    let nodes: Vec<CompactNode> = (0..5)
        .map(|i| CompactNode {
            kind: format!("t{}", i),
            start: None,
            end: None,
            field: None,
            children: vec![],
            text: None,
        })
        .collect();
    assert_eq!(nodes.len(), 5);
    assert_eq!(nodes[3].kind, "t3");
}

// ===========================================================================
// Additional coverage: serde serialization of SerializedNode
// ===========================================================================

#[test]
fn serialized_node_json_roundtrip() {
    let n = leaf_node("lit", "42", 0, 2);
    let json = serde_json::to_string(&n).unwrap();
    let back: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.kind, "lit");
    assert_eq!(back.text.as_deref(), Some("42"));
    assert_eq!(back.start_byte, 0);
    assert_eq!(back.end_byte, 2);
}

#[test]
fn serialized_node_json_with_children_roundtrip() {
    let parent = SerializedNode {
        kind: "expr".to_string(),
        is_named: true,
        field_name: None,
        start_position: (0, 0),
        end_position: (0, 5),
        start_byte: 0,
        end_byte: 5,
        text: None,
        children: vec![leaf_node("num", "1", 0, 1), leaf_node("num", "2", 4, 5)],
        is_error: false,
        is_missing: false,
    };
    let json = serde_json::to_string_pretty(&parent).unwrap();
    let back: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.children.len(), 2);
    assert_eq!(back.children[0].kind, "num");
}

#[test]
fn sexpr_json_atom_roundtrip() {
    let a = SExpr::Atom("test".to_string());
    let json = serde_json::to_string(&a).unwrap();
    let back: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(back, a);
}

#[test]
fn sexpr_json_list_roundtrip() {
    let l = SExpr::List(vec![
        SExpr::Atom("a".to_string()),
        SExpr::List(vec![SExpr::Atom("b".to_string())]),
    ]);
    let json = serde_json::to_string(&l).unwrap();
    let back: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(back, l);
}

#[test]
fn compact_node_skips_empty_children_in_json() {
    let cn = CompactNode {
        kind: "x".to_string(),
        start: None,
        end: None,
        field: None,
        children: vec![],
        text: None,
    };
    let json = serde_json::to_string(&cn).unwrap();
    // children field is skipped when empty due to skip_serializing_if
    assert!(!json.contains("\"c\""));
}

#[test]
fn compact_node_skips_none_fields_in_json() {
    let cn = CompactNode {
        kind: "y".to_string(),
        start: None,
        end: None,
        field: None,
        children: vec![],
        text: None,
    };
    let json = serde_json::to_string(&cn).unwrap();
    assert!(!json.contains("\"s\""));
    assert!(!json.contains("\"e\""));
    assert!(!json.contains("\"f\""));
    assert!(!json.contains("\"x\""));
}

#[test]
fn compact_node_includes_present_fields() {
    let cn = CompactNode {
        kind: "z".to_string(),
        start: Some(10),
        end: Some(20),
        field: Some("name".to_string()),
        children: vec![CompactNode {
            kind: "child".to_string(),
            start: None,
            end: None,
            field: None,
            children: vec![],
            text: Some("val".to_string()),
        }],
        text: None,
    };
    let json = serde_json::to_string(&cn).unwrap();
    assert!(json.contains("\"s\":10"));
    assert!(json.contains("\"e\":20"));
    assert!(json.contains("\"f\":\"name\""));
    assert!(json.contains("\"c\""));
}

// ===========================================================================
// Additional: parse_sexpr consistency
// ===========================================================================

#[test]
fn parse_sexpr_returns_list_variant() {
    match parse_sexpr("(+ 1 2)").unwrap() {
        SExpr::List(_) => {} // ok
        SExpr::Atom(_) => panic!("expected List"),
    }
}

#[test]
fn parse_sexpr_result_inner_has_items() {
    if let SExpr::List(items) = parse_sexpr("(+ 1 2)").unwrap() {
        assert_eq!(
            items,
            vec![
                SExpr::Atom("+".to_string()),
                SExpr::Atom("1".to_string()),
                SExpr::Atom("2".to_string()),
            ]
        );
    }
}

#[test]
fn parse_sexpr_multiple_calls_consistent() {
    let r1 = parse_sexpr("(a)");
    let r2 = parse_sexpr("(b c d)");
    let r3 = parse_sexpr("");
    assert_eq!(r1, parse_sexpr("(a)"));
    assert_eq!(r2, parse_sexpr("(b c d)"));
    assert!(r3.is_err());
}

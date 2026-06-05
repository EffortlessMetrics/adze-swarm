// Advanced comprehensive tests for serialization data structures.
// Exercises SerializedNode, SExpr, CompactNode, BinaryFormat, parse_sexpr,
// and serializer construction without requiring tree-sitter parsing.
#![cfg(feature = "serialization")]

use adze::serialization::*;

// ── helpers ──────────────────────────────────────────────────────────────────

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

fn parent_node(kind: &str, children: Vec<SerializedNode>) -> SerializedNode {
    let start = children.first().map_or(0, |c| c.start_byte);
    let end = children.last().map_or(0, |c| c.end_byte);
    SerializedNode {
        kind: kind.to_string(),
        is_named: true,
        field_name: None,
        start_position: (0, start),
        end_position: (0, end),
        start_byte: start,
        end_byte: end,
        text: None,
        children,
        is_error: false,
        is_missing: false,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. SerializedNode – construction and field access
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn serialized_node_basic_fields() {
    let n = leaf_node("identifier", "foo", 0, 3);
    assert_eq!(n.kind, "identifier");
    assert!(n.is_named);
    assert_eq!(n.text, Some("foo".to_string()));
    assert_eq!(n.start_byte, 0);
    assert_eq!(n.end_byte, 3);
}

#[test]
fn serialized_node_field_name_none() {
    let n = leaf_node("number", "42", 0, 2);
    assert!(n.field_name.is_none());
}

#[test]
fn serialized_node_field_name_some() {
    let mut n = leaf_node("identifier", "x", 0, 1);
    n.field_name = Some("name".to_string());
    assert_eq!(n.field_name, Some("name".to_string()));
}

#[test]
fn serialized_node_positions() {
    let n = SerializedNode {
        kind: "string".to_string(),
        is_named: true,
        field_name: None,
        start_position: (3, 10),
        end_position: (3, 20),
        start_byte: 50,
        end_byte: 60,
        text: Some("hello".to_string()),
        children: vec![],
        is_error: false,
        is_missing: false,
    };
    assert_eq!(n.start_position, (3, 10));
    assert_eq!(n.end_position, (3, 20));
}

#[test]
fn serialized_node_unnamed() {
    let n = SerializedNode {
        kind: "+".to_string(),
        is_named: false,
        field_name: None,
        start_position: (0, 0),
        end_position: (0, 1),
        start_byte: 0,
        end_byte: 1,
        text: Some("+".to_string()),
        children: vec![],
        is_error: false,
        is_missing: false,
    };
    assert!(!n.is_named);
}

#[test]
fn serialized_node_error_flag() {
    let mut n = leaf_node("ERROR", "??", 0, 2);
    n.is_error = true;
    n.is_named = false;
    assert!(n.is_error);
    assert!(!n.is_missing);
}

#[test]
fn serialized_node_missing_flag() {
    let mut n = leaf_node("identifier", "", 5, 5);
    n.is_missing = true;
    assert!(n.is_missing);
    assert!(!n.is_error);
}

#[test]
fn serialized_node_both_error_and_missing() {
    let n = SerializedNode {
        kind: "ERROR".to_string(),
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
    assert!(n.is_error && n.is_missing);
}

#[test]
fn serialized_node_text_none() {
    let n = parent_node("program", vec![]);
    assert!(n.text.is_none());
}

#[test]
fn serialized_node_empty_text() {
    let mut n = leaf_node("string", "", 0, 0);
    n.text = Some(String::new());
    assert_eq!(n.text, Some(String::new()));
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. SerializedNode – children
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn serialized_node_single_child() {
    let child = leaf_node("number", "1", 0, 1);
    let p = parent_node("expression", vec![child]);
    assert_eq!(p.children.len(), 1);
    assert_eq!(p.children[0].kind, "number");
}

#[test]
fn serialized_node_multiple_children() {
    let a = leaf_node("id", "a", 0, 1);
    let b = leaf_node("id", "b", 2, 3);
    let c = leaf_node("id", "c", 4, 5);
    let p = parent_node("list", vec![a, b, c]);
    assert_eq!(p.children.len(), 3);
}

#[test]
fn serialized_node_nested_children() {
    let inner = parent_node("inner", vec![leaf_node("x", "x", 0, 1)]);
    let outer = parent_node("outer", vec![inner]);
    assert_eq!(outer.children.len(), 1);
    assert_eq!(outer.children[0].children.len(), 1);
}

#[test]
fn serialized_node_deep_nesting() {
    let mut node = leaf_node("leaf", "v", 0, 1);
    for i in 0..5 {
        node = parent_node(&format!("level_{}", i), vec![node]);
    }
    // Walk 5 levels deep
    let mut cur = &node;
    for _ in 0..5 {
        assert_eq!(cur.children.len(), 1);
        cur = &cur.children[0];
    }
    assert_eq!(cur.kind, "leaf");
}

#[test]
fn serialized_node_wide_tree() {
    let children: Vec<_> = (0..20)
        .map(|i| leaf_node("item", &i.to_string(), i, i + 1))
        .collect();
    let p = parent_node("wide", children);
    assert_eq!(p.children.len(), 20);
}

#[test]
fn serialized_node_children_preserve_order() {
    let children: Vec<_> = ["alpha", "beta", "gamma"]
        .iter()
        .enumerate()
        .map(|(i, &s)| leaf_node("word", s, i, i + s.len()))
        .collect();
    let p = parent_node("words", children);
    assert_eq!(p.children[0].text.as_deref(), Some("alpha"));
    assert_eq!(p.children[1].text.as_deref(), Some("beta"));
    assert_eq!(p.children[2].text.as_deref(), Some("gamma"));
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. SerializedNode – Clone
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn serialized_node_clone_leaf() {
    let n = leaf_node("id", "abc", 0, 3);
    let c = n.clone();
    assert_eq!(format!("{:?}", n), format!("{:?}", c));
}

#[test]
fn serialized_node_clone_with_children() {
    let p = parent_node("p", vec![leaf_node("a", "a", 0, 1)]);
    let c = p.clone();
    assert_eq!(c.children.len(), 1);
    assert_eq!(c.children[0].kind, "a");
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. SerializedNode – Debug format
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn serialized_node_debug_contains_kind() {
    let n = leaf_node("identifier", "x", 0, 1);
    let dbg = format!("{:?}", n);
    assert!(dbg.contains("identifier"));
}

#[test]
fn serialized_node_debug_contains_text() {
    let n = leaf_node("string", "hello world", 0, 11);
    let dbg = format!("{:?}", n);
    assert!(dbg.contains("hello world"));
}

#[test]
fn serialized_node_debug_contains_positions() {
    let n = leaf_node("num", "7", 5, 6);
    let dbg = format!("{:?}", n);
    assert!(dbg.contains("5"));
    assert!(dbg.contains("6"));
}

#[test]
fn serialized_node_debug_contains_field_name() {
    let mut n = leaf_node("id", "x", 0, 1);
    n.field_name = Some("lhs".to_string());
    let dbg = format!("{:?}", n);
    assert!(dbg.contains("lhs"));
}

#[test]
fn serialized_node_debug_shows_error() {
    let mut n = leaf_node("ERR", "?", 0, 1);
    n.is_error = true;
    let dbg = format!("{:?}", n);
    assert!(dbg.contains("is_error: true"));
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. SerializedNode – JSON round-trip
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn serialized_node_json_roundtrip_leaf() {
    let n = leaf_node("number", "42", 0, 2);
    let json = serde_json::to_string(&n).unwrap();
    let back: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.kind, "number");
    assert_eq!(back.text, Some("42".to_string()));
}

#[test]
fn serialized_node_json_roundtrip_parent() {
    let p = parent_node("expr", vec![leaf_node("a", "a", 0, 1)]);
    let json = serde_json::to_string(&p).unwrap();
    let back: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.children.len(), 1);
}

#[test]
fn serialized_node_json_pretty() {
    let n = leaf_node("id", "x", 0, 1);
    let pretty = serde_json::to_string_pretty(&n).unwrap();
    assert!(pretty.contains('\n'));
}

#[test]
fn serialized_node_json_contains_all_fields() {
    let mut n = leaf_node("id", "x", 0, 1);
    n.field_name = Some("name".to_string());
    let json = serde_json::to_string(&n).unwrap();
    assert!(json.contains("\"kind\""));
    assert!(json.contains("\"is_named\""));
    assert!(json.contains("\"field_name\""));
    assert!(json.contains("\"start_position\""));
    assert!(json.contains("\"end_position\""));
    assert!(json.contains("\"start_byte\""));
    assert!(json.contains("\"end_byte\""));
    assert!(json.contains("\"text\""));
    assert!(json.contains("\"children\""));
    assert!(json.contains("\"is_error\""));
    assert!(json.contains("\"is_missing\""));
}

#[test]
fn serialized_node_json_field_name_null_when_none() {
    let n = leaf_node("id", "x", 0, 1);
    let json = serde_json::to_string(&n).unwrap();
    assert!(json.contains("\"field_name\":null"));
}

#[test]
fn serialized_node_json_nested_children_roundtrip() {
    let inner = parent_node("inner", vec![leaf_node("v", "1", 0, 1)]);
    let outer = parent_node("outer", vec![inner]);
    let json = serde_json::to_string(&outer).unwrap();
    let back: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.children[0].children[0].kind, "v");
}

#[test]
fn serialized_node_json_error_and_missing_flags() {
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
    let json = serde_json::to_string(&n).unwrap();
    let back: SerializedNode = serde_json::from_str(&json).unwrap();
    assert!(back.is_error);
    assert!(back.is_missing);
}

#[test]
fn serialized_node_json_unicode_text() {
    let n = leaf_node("string", "日本語テスト🚀", 0, 22);
    let json = serde_json::to_string(&n).unwrap();
    let back: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.text.as_deref(), Some("日本語テスト🚀"));
}

#[test]
fn serialized_node_json_special_chars_in_text() {
    let n = leaf_node("string", r#"he said "hi" \ tab	end"#, 0, 25);
    let json = serde_json::to_string(&n).unwrap();
    let back: SerializedNode = serde_json::from_str(&json).unwrap();
    assert!(back.text.as_deref().unwrap().contains("\"hi\""));
}

#[test]
fn serialized_node_deserialize_from_value() {
    let val = serde_json::json!({
        "kind": "root",
        "is_named": true,
        "field_name": null,
        "start_position": [0, 0],
        "end_position": [0, 5],
        "start_byte": 0,
        "end_byte": 5,
        "text": "hello",
        "children": [],
        "is_error": false,
        "is_missing": false
    });
    let n: SerializedNode = serde_json::from_value(val).unwrap();
    assert_eq!(n.kind, "root");
    assert_eq!(n.text.as_deref(), Some("hello"));
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. SExpr – Atom
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn sexpr_atom_simple() {
    let a = SExpr::Atom("hello".to_string());
    assert_eq!(a, SExpr::Atom("hello".to_string()));
}

#[test]
fn sexpr_atom_empty_string() {
    let a = SExpr::Atom(String::new());
    assert_eq!(a, SExpr::Atom(String::new()));
}

#[test]
fn sexpr_atom_with_spaces() {
    let a = SExpr::Atom("hello world".to_string());
    if let SExpr::Atom(s) = &a {
        assert!(s.contains(' '));
    } else {
        panic!("expected Atom");
    }
}

#[test]
fn sexpr_atom_unicode() {
    let a = SExpr::Atom("αβγ".to_string());
    assert_eq!(a, SExpr::Atom("αβγ".to_string()));
}

#[test]
fn sexpr_atom_inequality() {
    let a = SExpr::Atom("a".to_string());
    let b = SExpr::Atom("b".to_string());
    assert_ne!(a, b);
}

#[test]
fn sexpr_atom_vs_list_inequality() {
    let atom = SExpr::Atom("x".to_string());
    let list = SExpr::List(vec![SExpr::Atom("x".to_string())]);
    assert_ne!(atom, list);
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. SExpr – List
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn sexpr_empty_list() {
    let l = SExpr::List(vec![]);
    assert_eq!(l, SExpr::List(vec![]));
}

#[test]
fn sexpr_singleton_list() {
    let l = SExpr::List(vec![SExpr::Atom("only".to_string())]);
    if let SExpr::List(items) = &l {
        assert_eq!(items.len(), 1);
    }
}

#[test]
fn sexpr_list_multiple_atoms() {
    let l = SExpr::List(vec![
        SExpr::Atom("a".to_string()),
        SExpr::Atom("b".to_string()),
        SExpr::Atom("c".to_string()),
    ]);
    if let SExpr::List(items) = &l {
        assert_eq!(items.len(), 3);
    }
}

#[test]
fn sexpr_list_equality() {
    let a = SExpr::List(vec![SExpr::Atom("x".to_string())]);
    let b = SExpr::List(vec![SExpr::Atom("x".to_string())]);
    assert_eq!(a, b);
}

#[test]
fn sexpr_list_inequality_different_length() {
    let a = SExpr::List(vec![SExpr::Atom("x".to_string())]);
    let b = SExpr::List(vec![
        SExpr::Atom("x".to_string()),
        SExpr::Atom("y".to_string()),
    ]);
    assert_ne!(a, b);
}

#[test]
fn sexpr_list_inequality_different_content() {
    let a = SExpr::List(vec![SExpr::Atom("x".to_string())]);
    let b = SExpr::List(vec![SExpr::Atom("y".to_string())]);
    assert_ne!(a, b);
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. SExpr – nesting
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn sexpr_nested_list() {
    let inner = SExpr::List(vec![SExpr::Atom("inner".to_string())]);
    let outer = SExpr::List(vec![inner]);
    if let SExpr::List(items) = &outer {
        assert!(matches!(&items[0], SExpr::List(_)));
    }
}

#[test]
fn sexpr_double_nested() {
    let l = SExpr::List(vec![SExpr::List(vec![SExpr::List(vec![SExpr::Atom(
        "deep".to_string(),
    )])])]);
    // Three levels deep
    if let SExpr::List(a) = &l
        && let SExpr::List(b) = &a[0]
        && let SExpr::List(c) = &b[0]
    {
        assert_eq!(c[0], SExpr::Atom("deep".to_string()));
        return;
    }
    panic!("nesting mismatch");
}

#[test]
fn sexpr_mixed_list() {
    let l = SExpr::List(vec![
        SExpr::Atom("a".to_string()),
        SExpr::List(vec![SExpr::Atom("b".to_string())]),
        SExpr::Atom("c".to_string()),
    ]);
    if let SExpr::List(items) = &l {
        assert!(matches!(&items[0], SExpr::Atom(_)));
        assert!(matches!(&items[1], SExpr::List(_)));
        assert!(matches!(&items[2], SExpr::Atom(_)));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. SExpr – Clone
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn sexpr_clone_atom() {
    let a = SExpr::Atom("cloned".to_string());
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn sexpr_clone_list() {
    let l = SExpr::List(vec![
        SExpr::Atom("1".to_string()),
        SExpr::Atom("2".to_string()),
    ]);
    let c = l.clone();
    assert_eq!(l, c);
}

#[test]
fn sexpr_clone_nested() {
    let nested = SExpr::List(vec![SExpr::List(vec![SExpr::Atom("x".to_string())])]);
    let c = nested.clone();
    assert_eq!(nested, c);
}

// ═══════════════════════════════════════════════════════════════════════════
// 10. SExpr – Debug format
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn sexpr_debug_atom() {
    let a = SExpr::Atom("test".to_string());
    let dbg = format!("{:?}", a);
    assert!(dbg.contains("Atom"));
    assert!(dbg.contains("test"));
}

#[test]
fn sexpr_debug_list() {
    let l = SExpr::List(vec![SExpr::Atom("a".to_string())]);
    let dbg = format!("{:?}", l);
    assert!(dbg.contains("List"));
}

// ═══════════════════════════════════════════════════════════════════════════
// 11. SExpr – JSON serialization
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn sexpr_json_roundtrip_atom() {
    let a = SExpr::Atom("test".to_string());
    let json = serde_json::to_string(&a).unwrap();
    let back: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(a, back);
}

#[test]
fn sexpr_json_roundtrip_list() {
    let l = SExpr::List(vec![SExpr::Atom("x".to_string()), SExpr::List(vec![])]);
    let json = serde_json::to_string(&l).unwrap();
    let back: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(l, back);
}

// ═══════════════════════════════════════════════════════════════════════════
// 12. parse_sexpr
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn parse_sexpr_empty_input() {
    let r = parse_sexpr("");
    assert!(r.is_err());
}

#[test]
fn parse_sexpr_atom_input() {
    let r = parse_sexpr("hello");
    assert_eq!(r, Ok(SExpr::Atom("hello".to_string())));
}

#[test]
fn parse_sexpr_list_input() {
    let r = parse_sexpr("(a b c)");
    assert_eq!(
        r,
        Ok(SExpr::List(vec![
            SExpr::Atom("a".to_string()),
            SExpr::Atom("b".to_string()),
            SExpr::Atom("c".to_string())
        ]))
    );
}

#[test]
fn parse_sexpr_nested_input() {
    let r = parse_sexpr("(a (b c))");
    assert!(matches!(r, Ok(SExpr::List(items)) if items.len() == 2));
}

#[test]
fn parse_sexpr_whitespace_input() {
    let r = parse_sexpr("   \t\n  ");
    assert!(r.is_err());
}

#[test]
fn parse_sexpr_numeric_input() {
    let r = parse_sexpr("12345");
    assert_eq!(r, Ok(SExpr::Atom("12345".to_string())));
}

#[test]
fn parse_sexpr_special_chars() {
    let r = parse_sexpr("!@#$%^&*");
    assert_eq!(r, Ok(SExpr::Atom("!@#$%^&*".to_string())));
}

#[test]
fn parse_sexpr_unicode_input() {
    let r = parse_sexpr("日本語");
    assert_eq!(r, Ok(SExpr::Atom("日本語".to_string())));
}

// ═══════════════════════════════════════════════════════════════════════════
// 13. TreeSerializer – construction and configuration
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn tree_serializer_defaults() {
    let s = TreeSerializer::new(b"src");
    assert!(!s.include_unnamed);
    assert_eq!(s.max_text_length, Some(100));
}

#[test]
fn tree_serializer_with_unnamed() {
    let s = TreeSerializer::new(b"src").with_unnamed_nodes();
    assert!(s.include_unnamed);
}

#[test]
fn tree_serializer_max_text_none() {
    let s = TreeSerializer::new(b"src").with_max_text_length(None);
    assert_eq!(s.max_text_length, None);
}

#[test]
fn tree_serializer_max_text_custom() {
    let s = TreeSerializer::new(b"x").with_max_text_length(Some(50));
    assert_eq!(s.max_text_length, Some(50));
}

#[test]
fn tree_serializer_chained_config() {
    let s = TreeSerializer::new(b"code")
        .with_unnamed_nodes()
        .with_max_text_length(Some(200));
    assert!(s.include_unnamed);
    assert_eq!(s.max_text_length, Some(200));
}

#[test]
fn tree_serializer_source_preserved() {
    let src = b"let x = 1;";
    let s = TreeSerializer::new(src);
    assert_eq!(s.source, src);
}

// ═══════════════════════════════════════════════════════════════════════════
// 14. CompactSerializer – construction
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn compact_serializer_new() {
    let _s = CompactSerializer::new(b"code");
    // construction succeeds
}

// ═══════════════════════════════════════════════════════════════════════════
// 15. SExpressionSerializer – construction and configuration
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn sexpr_serializer_new() {
    let _s = SExpressionSerializer::new(b"code");
    // construction succeeds
}

#[test]
fn sexpr_serializer_with_positions() {
    // with_positions() returns Self, so chaining succeeds
    let _s = SExpressionSerializer::new(b"code").with_positions();
}

// ═══════════════════════════════════════════════════════════════════════════
// 16. CompactNode – construction and serde
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn compact_node_leaf() {
    let cn = CompactNode {
        kind: "id".to_string(),
        start: None,
        end: None,
        field: None,
        children: vec![],
        text: Some("x".to_string()),
    };
    let json = serde_json::to_string(&cn).unwrap();
    assert!(json.contains("\"t\":\"id\""));
    assert!(json.contains("\"x\":\"x\""));
    assert!(!json.contains("\"s\"")); // skipped when None
}

#[test]
fn compact_node_internal() {
    let cn = CompactNode {
        kind: "expr".to_string(),
        start: Some(0),
        end: Some(10),
        field: Some("body".to_string()),
        children: vec![CompactNode {
            kind: "num".to_string(),
            start: None,
            end: None,
            field: None,
            children: vec![],
            text: Some("5".to_string()),
        }],
        text: None,
    };
    let json = serde_json::to_string(&cn).unwrap();
    assert!(json.contains("\"f\":\"body\""));
    assert!(json.contains("\"c\":["));
}

#[test]
fn compact_node_empty_children_skipped() {
    let cn = CompactNode {
        kind: "x".to_string(),
        start: None,
        end: None,
        field: None,
        children: vec![],
        text: None,
    };
    let json = serde_json::to_string(&cn).unwrap();
    // "c" key omitted when children empty due to skip_serializing_if
    assert!(!json.contains("\"c\""));
}

#[test]
fn compact_node_json_roundtrip() {
    let cn = CompactNode {
        kind: "fn".to_string(),
        start: Some(10),
        end: Some(50),
        field: Some("definition".to_string()),
        children: vec![],
        text: None,
    };
    let json = serde_json::to_string(&cn).unwrap();
    let back: CompactNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.kind, "fn");
    assert_eq!(back.start, Some(10));
    assert_eq!(back.end, Some(50));
    assert_eq!(back.field, Some("definition".to_string()));
}

// ═══════════════════════════════════════════════════════════════════════════
// 17. BinaryFormat – construction
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn binary_format_empty() {
    let bf = BinaryFormat {
        node_types: vec![],
        field_names: vec![],
        tree_data: vec![],
    };
    assert!(bf.node_types.is_empty());
    assert!(bf.field_names.is_empty());
    assert!(bf.tree_data.is_empty());
}

#[test]
fn binary_format_with_data() {
    let bf = BinaryFormat {
        node_types: vec!["program".to_string(), "identifier".to_string()],
        field_names: vec!["name".to_string()],
        tree_data: vec![0, 1, 2, 3],
    };
    assert_eq!(bf.node_types.len(), 2);
    assert_eq!(bf.field_names.len(), 1);
    assert_eq!(bf.tree_data.len(), 4);
}

#[test]
fn binary_format_clone() {
    let bf = BinaryFormat {
        node_types: vec!["a".to_string()],
        field_names: vec![],
        tree_data: vec![42],
    };
    let c = bf.clone();
    assert_eq!(c.node_types, bf.node_types);
    assert_eq!(c.tree_data, bf.tree_data);
}

#[test]
fn binary_format_debug() {
    let bf = BinaryFormat {
        node_types: vec!["root".to_string()],
        field_names: vec![],
        tree_data: vec![],
    };
    let dbg = format!("{:?}", bf);
    assert!(dbg.contains("BinaryFormat"));
    assert!(dbg.contains("root"));
}

// ═══════════════════════════════════════════════════════════════════════════
// 18. BinarySerializer – construction
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn binary_serializer_new() {
    let _bs = BinarySerializer::new();
}

#[test]
fn binary_serializer_default() {
    let _bs = BinarySerializer::default();
}

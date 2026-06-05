#![cfg(feature = "serialization")]
#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

//! Comprehensive tests for `adze::serialization` module (v3).
//!
//! Covers SerializedNode, CompactNode, SExpr, parse_sexpr, TreeSerializer
//! configuration, BinarySerializer, JSON roundtrip, edge cases, unicode,
//! and format detection.

use adze::serialization::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_leaf(kind: &str, text: &str, start: usize, end: usize) -> SerializedNode {
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

fn make_parent(
    kind: &str,
    children: Vec<SerializedNode>,
    start: usize,
    end: usize,
) -> SerializedNode {
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

fn make_compact_leaf(kind: &str, text: &str) -> CompactNode {
    CompactNode {
        kind: kind.to_string(),
        start: None,
        end: None,
        field: None,
        children: vec![],
        text: Some(text.to_string()),
    }
}

// ===========================================================================
// SerializedNode – construction and field access
// ===========================================================================

#[test]
fn serialized_node_leaf_defaults() {
    let n = make_leaf("number", "42", 0, 2);
    assert_eq!(n.kind, "number");
    assert!(n.is_named);
    assert_eq!(n.text.as_deref(), Some("42"));
    assert!(n.children.is_empty());
    assert!(!n.is_error);
    assert!(!n.is_missing);
}

#[test]
fn serialized_node_with_field_name() {
    let mut n = make_leaf("ident", "x", 0, 1);
    n.field_name = Some("name".into());
    assert_eq!(n.field_name.as_deref(), Some("name"));
}

#[test]
fn serialized_node_error_flag() {
    let mut n = make_leaf("ERROR", "??", 0, 2);
    n.is_error = true;
    n.is_named = false;
    assert!(n.is_error);
    assert!(!n.is_named);
}

#[test]
fn serialized_node_missing_flag() {
    let mut n = make_leaf("identifier", "", 5, 5);
    n.is_missing = true;
    assert!(n.is_missing);
    assert_eq!(n.start_byte, n.end_byte);
}

#[test]
fn serialized_node_positions() {
    let n = SerializedNode {
        kind: "stmt".into(),
        is_named: true,
        field_name: None,
        start_position: (3, 4),
        end_position: (3, 10),
        start_byte: 40,
        end_byte: 46,
        text: None,
        children: vec![],
        is_error: false,
        is_missing: false,
    };
    assert_eq!(n.start_position, (3, 4));
    assert_eq!(n.end_position, (3, 10));
}

#[test]
fn serialized_node_with_children() {
    let parent = make_parent(
        "binary_expr",
        vec![make_leaf("num", "1", 0, 1), make_leaf("num", "2", 4, 5)],
        0,
        5,
    );
    assert_eq!(parent.children.len(), 2);
    assert_eq!(parent.children[0].text.as_deref(), Some("1"));
    assert_eq!(parent.children[1].text.as_deref(), Some("2"));
    assert!(parent.text.is_none());
}

#[test]
fn serialized_node_deeply_nested() {
    let deep = make_parent(
        "a",
        vec![make_parent(
            "b",
            vec![make_parent("c", vec![make_leaf("d", "val", 0, 3)], 0, 3)],
            0,
            3,
        )],
        0,
        3,
    );
    assert_eq!(deep.children[0].children[0].children[0].kind, "d");
}

// ===========================================================================
// SerializedNode – JSON serialization roundtrip
// ===========================================================================

#[test]
fn serialized_node_json_roundtrip_leaf() {
    let original = make_leaf("ident", "foo", 0, 3);
    let json = serde_json::to_string(&original).unwrap();
    let restored: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.kind, original.kind);
    assert_eq!(restored.text, original.text);
    assert_eq!(restored.start_byte, original.start_byte);
    assert_eq!(restored.end_byte, original.end_byte);
}

#[test]
fn serialized_node_json_roundtrip_parent() {
    let original = make_parent(
        "program",
        vec![make_leaf("kw", "let", 0, 3), make_leaf("ident", "x", 4, 5)],
        0,
        5,
    );
    let json = serde_json::to_string_pretty(&original).unwrap();
    let restored: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.children.len(), 2);
    assert_eq!(restored.children[1].text.as_deref(), Some("x"));
}

#[test]
fn serialized_node_json_contains_kind() {
    let n = make_leaf("string_literal", "\"hi\"", 0, 4);
    let json = serde_json::to_string(&n).unwrap();
    assert!(json.contains("string_literal"));
}

#[test]
fn serialized_node_json_error_node_roundtrip() {
    let mut n = make_leaf("ERROR", "bad", 0, 3);
    n.is_error = true;
    n.is_named = false;
    let json = serde_json::to_string(&n).unwrap();
    let restored: SerializedNode = serde_json::from_str(&json).unwrap();
    assert!(restored.is_error);
    assert!(!restored.is_named);
}

#[test]
fn serialized_node_json_missing_node_roundtrip() {
    let mut n = make_leaf("semicolon", "", 10, 10);
    n.is_missing = true;
    let json = serde_json::to_string(&n).unwrap();
    let restored: SerializedNode = serde_json::from_str(&json).unwrap();
    assert!(restored.is_missing);
}

#[test]
fn serialized_node_json_field_name_roundtrip() {
    let mut n = make_leaf("ident", "x", 0, 1);
    n.field_name = Some("name".into());
    let json = serde_json::to_string(&n).unwrap();
    let restored: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.field_name.as_deref(), Some("name"));
}

#[test]
fn serialized_node_json_no_text_for_parent() {
    let parent = make_parent("program", vec![make_leaf("id", "a", 0, 1)], 0, 1);
    let json = serde_json::to_string(&parent).unwrap();
    // text should be null in JSON
    assert!(json.contains("\"text\":null"));
}

// ===========================================================================
// CompactNode – construction and serde
// ===========================================================================

#[test]
fn compact_node_leaf_serde_field_names() {
    let c = make_compact_leaf("identifier", "test");
    let json = serde_json::to_string(&c).unwrap();
    // Serde renames: kind->t, text->x
    assert!(json.contains("\"t\":\"identifier\""));
    assert!(json.contains("\"x\":\"test\""));
    // start/end are None so they should be absent (skip_serializing_if)
    assert!(!json.contains("\"s\""));
    assert!(!json.contains("\"e\""));
}

#[test]
fn compact_node_with_positions() {
    let c = CompactNode {
        kind: "func".into(),
        start: Some(10),
        end: Some(50),
        field: None,
        children: vec![],
        text: None,
    };
    let json = serde_json::to_string(&c).unwrap();
    assert!(json.contains("\"s\":10"));
    assert!(json.contains("\"e\":50"));
}

#[test]
fn compact_node_with_field() {
    let c = CompactNode {
        kind: "id".into(),
        start: None,
        end: None,
        field: Some("name".into()),
        children: vec![],
        text: Some("x".into()),
    };
    let json = serde_json::to_string(&c).unwrap();
    assert!(json.contains("\"f\":\"name\""));
}

#[test]
fn compact_node_children_present() {
    let parent = CompactNode {
        kind: "call".into(),
        start: Some(0),
        end: Some(10),
        field: None,
        children: vec![make_compact_leaf("id", "f"), make_compact_leaf("num", "1")],
        text: None,
    };
    let json = serde_json::to_string(&parent).unwrap();
    assert!(json.contains("\"c\":["));
}

#[test]
fn compact_node_empty_children_omitted() {
    let c = make_compact_leaf("x", "y");
    let json = serde_json::to_string(&c).unwrap();
    // Empty children vec should be skipped
    assert!(!json.contains("\"c\""));
}

#[test]
fn compact_node_json_roundtrip() {
    let original = CompactNode {
        kind: "binary".into(),
        start: Some(0),
        end: Some(5),
        field: Some("expr".into()),
        children: vec![make_compact_leaf("num", "1"), make_compact_leaf("num", "2")],
        text: None,
    };
    let json = serde_json::to_string(&original).unwrap();
    let restored: CompactNode = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.kind, "binary");
    assert_eq!(restored.start, Some(0));
    assert_eq!(restored.end, Some(5));
    assert_eq!(restored.field.as_deref(), Some("expr"));
    assert_eq!(restored.children.len(), 2);
}

// ===========================================================================
// SExpr – construction, equality, serialization
// ===========================================================================

#[test]
fn sexpr_atom_creation() {
    let a = SExpr::Atom("hello".into());
    assert_eq!(a, SExpr::Atom("hello".into()));
}

#[test]
fn sexpr_list_creation() {
    let l = SExpr::List(vec![SExpr::Atom("a".into()), SExpr::Atom("b".into())]);
    if let SExpr::List(items) = &l {
        assert_eq!(items.len(), 2);
    } else {
        panic!("expected list");
    }
}

#[test]
fn sexpr_empty_list() {
    let l = SExpr::List(vec![]);
    assert_eq!(l, SExpr::List(vec![]));
}

#[test]
fn sexpr_nested_list() {
    let inner = SExpr::List(vec![SExpr::Atom("inner".into())]);
    let outer = SExpr::List(vec![SExpr::Atom("outer".into()), inner]);
    if let SExpr::List(items) = &outer {
        assert_eq!(items.len(), 2);
        assert!(matches!(&items[1], SExpr::List(_)));
    } else {
        panic!("expected list");
    }
}

#[test]
fn sexpr_equality_atoms() {
    assert_eq!(SExpr::Atom("x".into()), SExpr::Atom("x".into()));
    assert_ne!(SExpr::Atom("x".into()), SExpr::Atom("y".into()));
}

#[test]
fn sexpr_equality_lists() {
    let a = SExpr::List(vec![SExpr::Atom("1".into())]);
    let b = SExpr::List(vec![SExpr::Atom("1".into())]);
    let c = SExpr::List(vec![SExpr::Atom("2".into())]);
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn sexpr_atom_not_equal_to_list() {
    assert_ne!(SExpr::Atom("x".into()), SExpr::List(vec![]));
}

#[test]
fn sexpr_clone() {
    let original = SExpr::List(vec![SExpr::Atom("cloned".into())]);
    let cloned = original.clone();
    assert_eq!(original, cloned);
}

#[test]
fn sexpr_debug_format() {
    let a = SExpr::Atom("test".into());
    let dbg = format!("{:?}", a);
    assert!(dbg.contains("Atom"));
    assert!(dbg.contains("test"));
}

#[test]
fn sexpr_json_roundtrip_atom() {
    let original = SExpr::Atom("hello".into());
    let json = serde_json::to_string(&original).unwrap();
    let restored: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn sexpr_json_roundtrip_list() {
    let original = SExpr::List(vec![
        SExpr::Atom("program".into()),
        SExpr::List(vec![SExpr::Atom("stmt".into()), SExpr::Atom("expr".into())]),
    ]);
    let json = serde_json::to_string(&original).unwrap();
    let restored: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn sexpr_json_roundtrip_empty_list() {
    let original = SExpr::List(vec![]);
    let json = serde_json::to_string(&original).unwrap();
    let restored: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}

// ===========================================================================
// parse_sexpr behavior
// ===========================================================================

#[test]
fn parse_sexpr_returns_ok() {
    let result = parse_sexpr("(program)");
    assert!(result.is_ok());
}

#[test]
fn parse_sexpr_returns_program_list() {
    let result = parse_sexpr("(program (stmt))").unwrap();
    assert!(matches!(result, SExpr::List(items) if items.len() == 2));
}

#[test]
fn parse_sexpr_empty_input() {
    let result = parse_sexpr("");
    assert!(result.is_err());
}

#[test]
fn parse_sexpr_atom_input() {
    let result = parse_sexpr("atom").unwrap();
    assert_eq!(result, SExpr::Atom("atom".to_string()));
}

#[test]
fn parse_sexpr_nested_input() {
    let result = parse_sexpr("(a (b (c)))").unwrap();
    assert!(matches!(result, SExpr::List(items) if items.len() == 2));
}

#[test]
fn parse_sexpr_whitespace_only() {
    let result = parse_sexpr("   \t\n  ");
    assert!(result.is_err());
}

#[test]
fn parse_sexpr_special_chars() {
    let result = parse_sexpr("(+ 1 2)").unwrap();
    assert!(matches!(result, SExpr::List(items) if items.len() == 3));
}

// ===========================================================================
// TreeSerializer – configuration
// ===========================================================================

#[test]
fn tree_serializer_default_config() {
    let src = b"source";
    let ser = TreeSerializer::new(src);
    assert!(!ser.include_unnamed);
    assert_eq!(ser.max_text_length, Some(100));
    assert_eq!(ser.source, src);
}

#[test]
fn tree_serializer_with_unnamed() {
    let ser = TreeSerializer::new(b"x").with_unnamed_nodes();
    assert!(ser.include_unnamed);
}

#[test]
fn tree_serializer_with_max_text() {
    let ser = TreeSerializer::new(b"x").with_max_text_length(Some(50));
    assert_eq!(ser.max_text_length, Some(50));
}

#[test]
fn tree_serializer_unlimited_text() {
    let ser = TreeSerializer::new(b"x").with_max_text_length(None);
    assert_eq!(ser.max_text_length, None);
}

#[test]
fn tree_serializer_chained_config() {
    let ser = TreeSerializer::new(b"hello")
        .with_unnamed_nodes()
        .with_max_text_length(Some(25));
    assert!(ser.include_unnamed);
    assert_eq!(ser.max_text_length, Some(25));
}

// ===========================================================================
// SExpressionSerializer – configuration
// ===========================================================================

#[test]
fn sexpr_serializer_default_construction() {
    // Verify construction doesn't panic
    let _ser = SExpressionSerializer::new(b"src");
}

#[test]
fn sexpr_serializer_with_positions_chain() {
    // Verify with_positions builder doesn't panic
    let _ser = SExpressionSerializer::new(b"src").with_positions();
}

// ===========================================================================
// BinarySerializer – construction
// ===========================================================================

#[test]
fn binary_serializer_new_construction() {
    // BinarySerializer::new() should not panic
    let _bs = BinarySerializer::new();
}

#[test]
fn binary_serializer_default_construction() {
    // Default impl should not panic
    let _bs = BinarySerializer::default();
}

// ===========================================================================
// BinaryFormat – field access
// ===========================================================================

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

#[test]
fn binary_format_with_data() {
    let bf = BinaryFormat {
        node_types: vec!["program".into(), "ident".into()],
        field_names: vec!["name".into()],
        tree_data: vec![0, 1, 2, 3],
    };
    assert_eq!(bf.node_types.len(), 2);
    assert_eq!(bf.field_names.len(), 1);
    assert_eq!(bf.tree_data.len(), 4);
}

#[test]
fn binary_format_clone() {
    let bf = BinaryFormat {
        node_types: vec!["a".into()],
        field_names: vec![],
        tree_data: vec![42],
    };
    let cloned = bf.clone();
    assert_eq!(cloned.node_types, bf.node_types);
    assert_eq!(cloned.tree_data, bf.tree_data);
}

// ===========================================================================
// Edge cases – unicode
// ===========================================================================

#[test]
fn serialized_node_unicode_text() {
    let n = make_leaf("string", "こんにちは", 0, 15);
    let json = serde_json::to_string(&n).unwrap();
    let restored: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.text.as_deref(), Some("こんにちは"));
}

#[test]
fn serialized_node_emoji_text() {
    let n = make_leaf("string", "🦀🔥", 0, 8);
    let json = serde_json::to_string(&n).unwrap();
    let restored: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.text.as_deref(), Some("🦀🔥"));
}

#[test]
fn compact_node_unicode_roundtrip() {
    let c = make_compact_leaf("str", "αβγδ");
    let json = serde_json::to_string(&c).unwrap();
    let restored: CompactNode = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.text.as_deref(), Some("αβγδ"));
}

#[test]
fn sexpr_unicode_atom() {
    let a = SExpr::Atom("日本語".into());
    let json = serde_json::to_string(&a).unwrap();
    let restored: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, SExpr::Atom("日本語".into()));
}

// ===========================================================================
// Edge cases – special characters
// ===========================================================================

#[test]
fn serialized_node_backslash_in_text() {
    let n = make_leaf("string", r#"a\b"#, 0, 3);
    let json = serde_json::to_string(&n).unwrap();
    let restored: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.text.as_deref(), Some(r#"a\b"#));
}

#[test]
fn serialized_node_quotes_in_text() {
    let n = make_leaf("string", r#"say "hello""#, 0, 11);
    let json = serde_json::to_string(&n).unwrap();
    let restored: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.text.as_deref(), Some(r#"say "hello""#));
}

#[test]
fn serialized_node_newlines_in_text() {
    let n = make_leaf("string", "line1\nline2\n", 0, 12);
    let json = serde_json::to_string(&n).unwrap();
    let restored: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.text.as_deref(), Some("line1\nline2\n"));
}

#[test]
fn serialized_node_null_char_in_kind() {
    // JSON can represent \u0000
    let n = make_leaf("a\0b", "text", 0, 4);
    let json = serde_json::to_string(&n).unwrap();
    let restored: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.kind, "a\0b");
}

// ===========================================================================
// Edge cases – empty / boundary
// ===========================================================================

#[test]
fn serialized_node_empty_kind() {
    let n = make_leaf("", "text", 0, 4);
    assert_eq!(n.kind, "");
}

#[test]
fn serialized_node_empty_text() {
    let n = make_leaf("tok", "", 5, 5);
    assert_eq!(n.text.as_deref(), Some(""));
}

#[test]
fn serialized_node_no_text() {
    let mut n = make_leaf("tok", "x", 0, 1);
    n.text = None;
    let json = serde_json::to_string(&n).unwrap();
    let restored: SerializedNode = serde_json::from_str(&json).unwrap();
    assert!(restored.text.is_none());
}

#[test]
fn compact_node_all_none_optional_fields() {
    let c = CompactNode {
        kind: "x".into(),
        start: None,
        end: None,
        field: None,
        children: vec![],
        text: None,
    };
    let json = serde_json::to_string(&c).unwrap();
    // Only "t" should be present
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let obj = parsed.as_object().unwrap();
    assert!(obj.contains_key("t"));
    assert!(!obj.contains_key("s"));
    assert!(!obj.contains_key("e"));
    assert!(!obj.contains_key("f"));
    assert!(!obj.contains_key("c"));
    assert!(!obj.contains_key("x"));
}

#[test]
fn serialized_node_zero_byte_range() {
    let n = make_leaf("missing", "", 0, 0);
    assert_eq!(n.start_byte, 0);
    assert_eq!(n.end_byte, 0);
}

// ===========================================================================
// Roundtrip – deterministic re-serialization
// ===========================================================================

#[test]
fn serialized_node_double_roundtrip() {
    let original = make_parent(
        "root",
        vec![
            make_leaf("a", "hello", 0, 5),
            make_leaf("b", "world", 6, 11),
        ],
        0,
        11,
    );
    let json1 = serde_json::to_string(&original).unwrap();
    let mid: SerializedNode = serde_json::from_str(&json1).unwrap();
    let json2 = serde_json::to_string(&mid).unwrap();
    assert_eq!(json1, json2);
}

#[test]
fn compact_node_double_roundtrip() {
    let original = CompactNode {
        kind: "root".into(),
        start: Some(0),
        end: Some(20),
        field: None,
        children: vec![make_compact_leaf("a", "1"), make_compact_leaf("b", "2")],
        text: None,
    };
    let json1 = serde_json::to_string(&original).unwrap();
    let mid: CompactNode = serde_json::from_str(&json1).unwrap();
    let json2 = serde_json::to_string(&mid).unwrap();
    assert_eq!(json1, json2);
}

#[test]
fn sexpr_double_roundtrip() {
    let original = SExpr::List(vec![
        SExpr::Atom("root".into()),
        SExpr::List(vec![SExpr::Atom("child".into())]),
    ]);
    let json1 = serde_json::to_string(&original).unwrap();
    let mid: SExpr = serde_json::from_str(&json1).unwrap();
    let json2 = serde_json::to_string(&mid).unwrap();
    assert_eq!(json1, json2);
}

// ===========================================================================
// Format detection / selection heuristics
// ===========================================================================

#[test]
fn json_output_is_valid_json() {
    let n = make_parent("prog", vec![make_leaf("id", "x", 0, 1)], 0, 1);
    let json = serde_json::to_string_pretty(&n).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.is_object());
}

#[test]
fn compact_json_is_valid_json() {
    let c = CompactNode {
        kind: "prog".into(),
        start: Some(0),
        end: Some(5),
        field: None,
        children: vec![make_compact_leaf("id", "x")],
        text: None,
    };
    let json = serde_json::to_string(&c).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.is_object());
}

#[test]
fn compact_json_smaller_than_full_json() {
    let full = make_parent(
        "program",
        vec![make_leaf("identifier", "variable_name", 0, 13)],
        0,
        13,
    );
    let compact = CompactNode {
        kind: "program".into(),
        start: Some(0),
        end: Some(13),
        field: None,
        children: vec![make_compact_leaf("identifier", "variable_name")],
        text: None,
    };
    let full_json = serde_json::to_string(&full).unwrap();
    let compact_json = serde_json::to_string(&compact).unwrap();
    assert!(compact_json.len() < full_json.len());
}

// ===========================================================================
// Large tree / stress
// ===========================================================================

#[test]
fn serialized_node_many_children() {
    let children: Vec<SerializedNode> = (0..100)
        .map(|i| make_leaf("item", &format!("v{}", i), i, i + 1))
        .collect();
    let parent = make_parent("list", children, 0, 100);
    assert_eq!(parent.children.len(), 100);
    let json = serde_json::to_string(&parent).unwrap();
    let restored: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.children.len(), 100);
    assert_eq!(restored.children[99].text.as_deref(), Some("v99"));
}

#[test]
fn compact_node_many_children_roundtrip() {
    let children: Vec<CompactNode> = (0..50)
        .map(|i| make_compact_leaf("n", &format!("{}", i)))
        .collect();
    let parent = CompactNode {
        kind: "seq".into(),
        start: Some(0),
        end: Some(50),
        field: None,
        children,
        text: None,
    };
    let json = serde_json::to_string(&parent).unwrap();
    let restored: CompactNode = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.children.len(), 50);
}

// ===========================================================================
// Max text length truncation logic (unit-level)
// ===========================================================================

#[test]
fn truncation_logic_short_text_unchanged() {
    let text = "hi";
    let max_len = 10;
    let result = if text.len() > max_len {
        format!("{}...", &text[..max_len])
    } else {
        text.to_string()
    };
    assert_eq!(result, "hi");
}

#[test]
fn truncation_logic_exact_boundary() {
    let text = "abcde";
    let max_len = 5;
    let result = if text.len() > max_len {
        format!("{}...", &text[..max_len])
    } else {
        text.to_string()
    };
    assert_eq!(result, "abcde");
}

#[test]
fn truncation_logic_one_over() {
    let text = "abcdef";
    let max_len = 5;
    let result = if text.len() > max_len {
        format!("{}...", &text[..max_len])
    } else {
        text.to_string()
    };
    assert_eq!(result, "abcde...");
}

#[test]
fn truncation_logic_long_text() {
    let text = "a".repeat(200);
    let max_len = 100;
    let result = if text.len() > max_len {
        format!("{}...", &text[..max_len])
    } else {
        text.to_string()
    };
    assert_eq!(result.len(), 103); // 100 + "..."
    assert!(result.ends_with("..."));
}

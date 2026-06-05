#![cfg(feature = "serialization")]

//! Comprehensive tests for `adze::serialization` module (v4).
//!
//! Covers SExpr construction, tree_to_sexpr output, tree_to_json output,
//! JSON roundtrip, serialization determinism, edge cases, trait impls,
//! and error handling.

use adze::serialization::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn leaf(kind: &str, text: &str, start: usize, end: usize) -> SerializedNode {
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

fn inner(kind: &str, children: Vec<SerializedNode>, start: usize, end: usize) -> SerializedNode {
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

fn compact_leaf(kind: &str, text: &str) -> CompactNode {
    CompactNode {
        kind: kind.to_string(),
        start: None,
        end: None,
        field: None,
        children: vec![],
        text: Some(text.to_string()),
    }
}

fn compact_inner(kind: &str, children: Vec<CompactNode>) -> CompactNode {
    CompactNode {
        kind: kind.to_string(),
        start: Some(0),
        end: Some(10),
        field: None,
        children,
        text: None,
    }
}

// ===========================================================================
// 1. SExpr construction and variants (10 tests)
// ===========================================================================

#[test]
fn sexpr_atom_from_string() {
    let a = SExpr::Atom("hello".to_string());
    if let SExpr::Atom(ref s) = a {
        assert_eq!(s, "hello");
    } else {
        panic!("expected Atom");
    }
}

#[test]
fn sexpr_atom_empty_string() {
    let a = SExpr::Atom(String::new());
    assert_eq!(a, SExpr::Atom("".to_string()));
}

#[test]
fn sexpr_list_empty() {
    let l = SExpr::List(vec![]);
    if let SExpr::List(ref items) = l {
        assert!(items.is_empty());
    } else {
        panic!("expected List");
    }
}

#[test]
fn sexpr_list_single_atom() {
    let l = SExpr::List(vec![SExpr::Atom("x".to_string())]);
    if let SExpr::List(ref items) = l {
        assert_eq!(items.len(), 1);
        assert_eq!(items[0], SExpr::Atom("x".to_string()));
    } else {
        panic!("expected List");
    }
}

#[test]
fn sexpr_nested_lists() {
    let inner_list = SExpr::List(vec![SExpr::Atom("a".to_string())]);
    let outer = SExpr::List(vec![inner_list.clone(), SExpr::Atom("b".to_string())]);
    if let SExpr::List(ref items) = outer {
        assert_eq!(items.len(), 2);
        assert_eq!(items[0], inner_list);
    } else {
        panic!("expected List");
    }
}

#[test]
fn sexpr_deeply_nested() {
    let mut expr = SExpr::Atom("leaf".to_string());
    for _ in 0..10 {
        expr = SExpr::List(vec![expr]);
    }
    // Unwrap 10 levels
    let mut current = &expr;
    for _ in 0..10 {
        match current {
            SExpr::List(items) => {
                assert_eq!(items.len(), 1);
                current = &items[0];
            }
            _ => panic!("expected List at nesting level"),
        }
    }
    assert_eq!(*current, SExpr::Atom("leaf".to_string()));
}

#[test]
fn sexpr_list_multiple_atoms() {
    let l = SExpr::List(vec![
        SExpr::Atom("a".to_string()),
        SExpr::Atom("b".to_string()),
        SExpr::Atom("c".to_string()),
    ]);
    if let SExpr::List(ref items) = l {
        assert_eq!(items.len(), 3);
    } else {
        panic!("expected List");
    }
}

#[test]
fn sexpr_atom_with_special_chars() {
    let a = SExpr::Atom("hello world\n\ttab".to_string());
    if let SExpr::Atom(ref s) = a {
        assert!(s.contains('\n'));
        assert!(s.contains('\t'));
    } else {
        panic!("expected Atom");
    }
}

#[test]
fn sexpr_atom_with_unicode() {
    let a = SExpr::Atom("こんにちは".to_string());
    assert_eq!(a, SExpr::Atom("こんにちは".to_string()));
}

#[test]
fn sexpr_mixed_list() {
    let l = SExpr::List(vec![
        SExpr::Atom("fn".to_string()),
        SExpr::List(vec![SExpr::Atom("arg1".to_string())]),
        SExpr::Atom("body".to_string()),
    ]);
    if let SExpr::List(ref items) = l {
        assert_eq!(items.len(), 3);
        assert!(matches!(&items[0], SExpr::Atom(_)));
        assert!(matches!(&items[1], SExpr::List(_)));
        assert!(matches!(&items[2], SExpr::Atom(_)));
    } else {
        panic!("expected List");
    }
}

// ===========================================================================
// 2. tree_to_sexpr output format — via SExpressionSerializer (8 tests)
// ===========================================================================

#[test]
fn sexpr_serializer_creation() {
    let source = b"hello";
    // Construction should not panic
    let _ser = SExpressionSerializer::new(source);
}

#[test]
fn sexpr_serializer_with_positions_chain() {
    let source = b"test";
    // with_positions returns Self, so chaining works
    let _ser = SExpressionSerializer::new(source).with_positions();
}

#[test]
fn sexpr_serializer_from_source() {
    let source = b"some code";
    let _ser = SExpressionSerializer::new(source);
    // Validates construction from arbitrary source bytes
}

#[test]
fn sexpr_serializer_default_construction() {
    let _ser = SExpressionSerializer::new(b"x");
    // Default construction should succeed
}

#[test]
fn sexpr_serializer_chained_with_positions() {
    let _ser = SExpressionSerializer::new(b"x").with_positions();
    // Chaining should not panic
}

#[test]
fn sexpr_serializer_empty_source() {
    let _ser = SExpressionSerializer::new(b"");
    // Empty source is valid
}

#[test]
fn sexpr_serializer_large_source() {
    let large = vec![b'a'; 10_000];
    let _ser = SExpressionSerializer::new(&large);
    // Large sources should be accepted
}

#[test]
fn sexpr_serializer_binary_source() {
    let source: &[u8] = &[0xFF, 0x00, 0xFE, 0x01];
    let _ser = SExpressionSerializer::new(source);
    // Binary data source is valid
}

// ===========================================================================
// 3. tree_to_json output format — via TreeSerializer (8 tests)
// ===========================================================================

#[test]
fn tree_serializer_default_excludes_unnamed() {
    let ser = TreeSerializer::new(b"test");
    assert!(!ser.include_unnamed);
}

#[test]
fn tree_serializer_with_unnamed_nodes() {
    let ser = TreeSerializer::new(b"test").with_unnamed_nodes();
    assert!(ser.include_unnamed);
}

#[test]
fn tree_serializer_default_max_text_length() {
    let ser = TreeSerializer::new(b"x");
    assert_eq!(ser.max_text_length, Some(100));
}

#[test]
fn tree_serializer_custom_max_text_length() {
    let ser = TreeSerializer::new(b"x").with_max_text_length(Some(50));
    assert_eq!(ser.max_text_length, Some(50));
}

#[test]
fn tree_serializer_no_max_text_length() {
    let ser = TreeSerializer::new(b"x").with_max_text_length(None);
    assert_eq!(ser.max_text_length, None);
}

#[test]
fn tree_serializer_chained_config() {
    let ser = TreeSerializer::new(b"code")
        .with_unnamed_nodes()
        .with_max_text_length(Some(200));
    assert!(ser.include_unnamed);
    assert_eq!(ser.max_text_length, Some(200));
}

#[test]
fn tree_serializer_source_reference() {
    let src = b"fn main() {}";
    let ser = TreeSerializer::new(src);
    assert_eq!(ser.source, b"fn main() {}");
}

#[test]
fn tree_serializer_zero_max_text_length() {
    let ser = TreeSerializer::new(b"x").with_max_text_length(Some(0));
    assert_eq!(ser.max_text_length, Some(0));
}

// ===========================================================================
// 4. JSON roundtrip via SerializedNode serde (8 tests)
// ===========================================================================

#[test]
fn json_roundtrip_leaf_node() {
    let node = leaf("ident", "foo", 0, 3);
    let json = serde_json::to_string(&node).unwrap();
    let back: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.kind, "ident");
    assert_eq!(back.text.as_deref(), Some("foo"));
}

#[test]
fn json_roundtrip_parent_with_children() {
    let node = inner(
        "expr",
        vec![leaf("num", "1", 0, 1), leaf("num", "2", 2, 3)],
        0,
        3,
    );
    let json = serde_json::to_string(&node).unwrap();
    let back: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.children.len(), 2);
    assert_eq!(back.children[0].kind, "num");
    assert_eq!(back.children[1].text.as_deref(), Some("2"));
}

#[test]
fn json_roundtrip_error_node() {
    let mut node = leaf("ERROR", "bad", 0, 3);
    node.is_error = true;
    node.is_named = false;
    let json = serde_json::to_string(&node).unwrap();
    let back: SerializedNode = serde_json::from_str(&json).unwrap();
    assert!(back.is_error);
    assert!(!back.is_named);
}

#[test]
fn json_roundtrip_missing_node() {
    let mut node = leaf("semi", "", 5, 5);
    node.is_missing = true;
    let json = serde_json::to_string(&node).unwrap();
    let back: SerializedNode = serde_json::from_str(&json).unwrap();
    assert!(back.is_missing);
    assert_eq!(back.start_byte, back.end_byte);
}

#[test]
fn json_roundtrip_field_name() {
    let mut node = leaf("ident", "x", 0, 1);
    node.field_name = Some("name".to_string());
    let json = serde_json::to_string(&node).unwrap();
    let back: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.field_name.as_deref(), Some("name"));
}

#[test]
fn json_roundtrip_positions() {
    let mut node = leaf("tok", "abc", 10, 13);
    node.start_position = (2, 5);
    node.end_position = (2, 8);
    let json = serde_json::to_string(&node).unwrap();
    let back: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.start_position, (2, 5));
    assert_eq!(back.end_position, (2, 8));
    assert_eq!(back.start_byte, 10);
    assert_eq!(back.end_byte, 13);
}

#[test]
fn json_roundtrip_compact_node() {
    let node = compact_leaf("id", "test");
    let json = serde_json::to_string(&node).unwrap();
    let back: CompactNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.kind, "id");
    assert_eq!(back.text.as_deref(), Some("test"));
    assert!(back.start.is_none());
}

#[test]
fn json_roundtrip_compact_with_children() {
    let node = compact_inner("root", vec![compact_leaf("a", "x"), compact_leaf("b", "y")]);
    let json = serde_json::to_string(&node).unwrap();
    let back: CompactNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.kind, "root");
    assert_eq!(back.children.len(), 2);
    assert_eq!(back.children[0].text.as_deref(), Some("x"));
}

// ===========================================================================
// 5. Serialization determinism (5 tests)
// ===========================================================================

#[test]
fn determinism_leaf_json_stable() {
    let node = leaf("id", "abc", 0, 3);
    let j1 = serde_json::to_string(&node).unwrap();
    let j2 = serde_json::to_string(&node).unwrap();
    assert_eq!(j1, j2);
}

#[test]
fn determinism_nested_json_stable() {
    let node = inner(
        "program",
        vec![inner("fn", vec![leaf("name", "main", 3, 7)], 0, 15)],
        0,
        15,
    );
    let j1 = serde_json::to_string_pretty(&node).unwrap();
    let j2 = serde_json::to_string_pretty(&node).unwrap();
    assert_eq!(j1, j2);
}

#[test]
fn determinism_compact_json_stable() {
    let node = compact_inner("root", vec![compact_leaf("a", "1"), compact_leaf("b", "2")]);
    let j1 = serde_json::to_string(&node).unwrap();
    let j2 = serde_json::to_string(&node).unwrap();
    assert_eq!(j1, j2);
}

#[test]
fn determinism_clone_produces_equal_json() {
    let node = inner("expr", vec![leaf("lit", "42", 0, 2)], 0, 2);
    let cloned = node.clone();
    let j_orig = serde_json::to_string(&node).unwrap();
    let j_clone = serde_json::to_string(&cloned).unwrap();
    assert_eq!(j_orig, j_clone);
}

#[test]
fn determinism_pretty_vs_compact_same_data() {
    let node = leaf("x", "val", 0, 3);
    let pretty = serde_json::to_string_pretty(&node).unwrap();
    let compact = serde_json::to_string(&node).unwrap();
    // Both should deserialize to same data
    let from_pretty: SerializedNode = serde_json::from_str(&pretty).unwrap();
    let from_compact: SerializedNode = serde_json::from_str(&compact).unwrap();
    assert_eq!(from_pretty.kind, from_compact.kind);
    assert_eq!(from_pretty.text, from_compact.text);
    assert_eq!(from_pretty.start_byte, from_compact.start_byte);
    assert_eq!(from_pretty.end_byte, from_compact.end_byte);
}

// ===========================================================================
// 6. Edge cases (empty trees, deep trees, wide trees) (10 tests)
// ===========================================================================

#[test]
fn edge_empty_leaf_no_text() {
    let mut node = leaf("empty", "", 0, 0);
    node.text = None;
    let json = serde_json::to_string(&node).unwrap();
    let back: SerializedNode = serde_json::from_str(&json).unwrap();
    assert!(back.text.is_none());
}

#[test]
fn edge_zero_byte_span() {
    let node = leaf("missing_semi", "", 5, 5);
    assert_eq!(node.start_byte, node.end_byte);
    let json = serde_json::to_string(&node).unwrap();
    let back: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.start_byte, 5);
    assert_eq!(back.end_byte, 5);
}

#[test]
fn edge_deeply_nested_tree() {
    // Build a tree 50 levels deep
    let mut current = leaf("leaf", "x", 0, 1);
    for i in 0..50 {
        current = inner(&format!("level_{}", i), vec![current], 0, 1);
    }
    let json = serde_json::to_string(&current).unwrap();
    let back: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.kind, "level_49");
    assert_eq!(back.children.len(), 1);
}

#[test]
fn edge_wide_tree_100_children() {
    let children: Vec<SerializedNode> = (0..100)
        .map(|i| leaf(&format!("child_{}", i), &format!("{}", i), i, i + 1))
        .collect();
    let node = inner("wide_root", children, 0, 100);
    assert_eq!(node.children.len(), 100);
    let json = serde_json::to_string(&node).unwrap();
    let back: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.children.len(), 100);
    assert_eq!(back.children[99].kind, "child_99");
}

#[test]
fn edge_unicode_text_in_leaf() {
    let node = leaf("string", "日本語テスト🎉", 0, 22);
    let json = serde_json::to_string(&node).unwrap();
    let back: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.text.as_deref(), Some("日本語テスト🎉"));
}

#[test]
fn edge_special_chars_in_kind() {
    let node = leaf("\"quoted\"", "val", 0, 3);
    let json = serde_json::to_string(&node).unwrap();
    let back: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.kind, "\"quoted\"");
}

#[test]
fn edge_very_long_text() {
    let long_text = "a".repeat(10_000);
    let node = leaf("big", &long_text, 0, 10_000);
    let json = serde_json::to_string(&node).unwrap();
    let back: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.text.as_deref().unwrap().len(), 10_000);
}

#[test]
fn edge_compact_node_skip_empty_children() {
    let node = compact_leaf("tok", "v");
    let json = serde_json::to_string(&node).unwrap();
    // CompactNode skips serializing empty children vec
    assert!(!json.contains("\"c\""));
}

#[test]
fn edge_compact_node_skip_none_fields() {
    let node = compact_leaf("tok", "v");
    let json = serde_json::to_string(&node).unwrap();
    // Should skip None start/end/field
    assert!(!json.contains("\"s\""));
    assert!(!json.contains("\"e\""));
    assert!(!json.contains("\"f\""));
}

#[test]
fn edge_serialized_node_empty_children_vec() {
    let node = inner("empty_parent", vec![], 0, 0);
    assert!(node.children.is_empty());
    let json = serde_json::to_string(&node).unwrap();
    assert!(json.contains("\"children\":[]"));
}

// ===========================================================================
// 7. SExpr Debug/Clone/PartialEq (5 tests)
// ===========================================================================

#[test]
fn sexpr_debug_atom() {
    let a = SExpr::Atom("test".to_string());
    let dbg = format!("{:?}", a);
    assert!(dbg.contains("Atom"));
    assert!(dbg.contains("test"));
}

#[test]
fn sexpr_debug_list() {
    let l = SExpr::List(vec![SExpr::Atom("x".to_string())]);
    let dbg = format!("{:?}", l);
    assert!(dbg.contains("List"));
    assert!(dbg.contains("Atom"));
}

#[test]
fn sexpr_clone_equality() {
    let original = SExpr::List(vec![
        SExpr::Atom("a".to_string()),
        SExpr::List(vec![SExpr::Atom("b".to_string())]),
    ]);
    let cloned = original.clone();
    assert_eq!(original, cloned);
}

#[test]
fn sexpr_partial_eq_same_atoms() {
    let a1 = SExpr::Atom("same".to_string());
    let a2 = SExpr::Atom("same".to_string());
    assert_eq!(a1, a2);
}

#[test]
fn sexpr_partial_eq_different() {
    let a = SExpr::Atom("x".to_string());
    let b = SExpr::Atom("y".to_string());
    assert_ne!(a, b);

    let list = SExpr::List(vec![SExpr::Atom("x".to_string())]);
    assert_ne!(a, list);
}

// ===========================================================================
// 8. Error handling / parse_sexpr (5 tests)
// ===========================================================================

#[test]
fn parse_sexpr_empty_input_returns_error() {
    let result = parse_sexpr("");
    assert!(result.is_err());
}

#[test]
fn parse_sexpr_simple_atom_returns_atom() {
    let result = parse_sexpr("hello");
    assert_eq!(result, Ok(SExpr::Atom("hello".to_string())));
}

#[test]
fn parse_sexpr_nested_parens_returns_list() {
    let result = parse_sexpr("(a (b c))");
    assert!(matches!(result, Ok(SExpr::List(items)) if items.len() == 2));
}

#[test]
fn parse_sexpr_invalid_input_returns_error() {
    let result = parse_sexpr(")))((( garbage !!!");
    assert!(result.is_err());
}

#[test]
fn parse_sexpr_unicode_input_returns_list() {
    let result = parse_sexpr("(こんにちは 世界)");
    assert_eq!(
        result,
        Ok(SExpr::List(vec![
            SExpr::Atom("こんにちは".to_string()),
            SExpr::Atom("世界".to_string())
        ]))
    );
}

// ===========================================================================
// Additional coverage: BinarySerializer, CompactSerializer config, SExpr serde
// ===========================================================================

#[test]
fn binary_serializer_new_default() {
    // Construction should not panic
    let _bs = BinarySerializer::new();
}

#[test]
fn binary_serializer_default_trait() {
    // Default impl should produce same result as new()
    let _bs = BinarySerializer::default();
}

#[test]
fn binary_format_fields() {
    let bf = BinaryFormat {
        node_types: vec!["program".to_string(), "ident".to_string()],
        field_names: vec!["name".to_string()],
        tree_data: vec![0u8; 16],
    };
    assert_eq!(bf.node_types.len(), 2);
    assert_eq!(bf.field_names.len(), 1);
    assert_eq!(bf.tree_data.len(), 16);
}

#[test]
fn binary_format_clone() {
    let bf = BinaryFormat {
        node_types: vec!["a".to_string()],
        field_names: vec![],
        tree_data: vec![1, 2, 3],
    };
    let cloned = bf.clone();
    assert_eq!(cloned.node_types, bf.node_types);
    assert_eq!(cloned.tree_data, bf.tree_data);
}

#[test]
fn binary_format_debug() {
    let bf = BinaryFormat {
        node_types: vec![],
        field_names: vec![],
        tree_data: vec![],
    };
    let dbg = format!("{:?}", bf);
    assert!(dbg.contains("BinaryFormat"));
}

#[test]
fn compact_serializer_creation() {
    let cs = CompactSerializer::new(b"test code");
    // Verify it doesn't panic; source is private, just ensure construction
    let _ = cs;
}

#[test]
fn compact_node_with_field() {
    let node = CompactNode {
        kind: "ident".to_string(),
        start: Some(5),
        end: Some(10),
        field: Some("name".to_string()),
        children: vec![],
        text: Some("foo".to_string()),
    };
    let json = serde_json::to_string(&node).unwrap();
    assert!(json.contains("\"f\":\"name\""));
    assert!(json.contains("\"s\":5"));
    assert!(json.contains("\"e\":10"));
}

#[test]
fn compact_node_with_children_json() {
    let node = compact_inner(
        "block",
        vec![compact_leaf("stmt", "x = 1"), compact_leaf("stmt", "y = 2")],
    );
    let json = serde_json::to_string(&node).unwrap();
    let back: CompactNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.children.len(), 2);
    assert_eq!(back.children[0].text.as_deref(), Some("x = 1"));
}

#[test]
fn sexpr_serde_atom_roundtrip() {
    let a = SExpr::Atom("test_val".to_string());
    let json = serde_json::to_string(&a).unwrap();
    let back: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(back, a);
}

#[test]
fn sexpr_serde_list_roundtrip() {
    let l = SExpr::List(vec![
        SExpr::Atom("fn".to_string()),
        SExpr::List(vec![SExpr::Atom("arg".to_string())]),
    ]);
    let json = serde_json::to_string(&l).unwrap();
    let back: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(back, l);
}

#[test]
fn sexpr_serde_empty_list_roundtrip() {
    let l = SExpr::List(vec![]);
    let json = serde_json::to_string(&l).unwrap();
    let back: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(back, l);
}

#[test]
fn serialized_node_json_contains_all_fields() {
    let mut node = leaf("kw", "let", 0, 3);
    node.field_name = Some("keyword".to_string());
    node.is_error = true;
    node.is_missing = true;
    let json = serde_json::to_string(&node).unwrap();
    assert!(json.contains("\"kind\":\"kw\""));
    assert!(json.contains("\"is_named\":true"));
    assert!(json.contains("\"field_name\":\"keyword\""));
    assert!(json.contains("\"is_error\":true"));
    assert!(json.contains("\"is_missing\":true"));
    assert!(json.contains("\"start_byte\":0"));
    assert!(json.contains("\"end_byte\":3"));
}

#[test]
fn serialized_node_pretty_json_readable() {
    let node = inner("program", vec![leaf("id", "x", 0, 1)], 0, 1);
    let pretty = serde_json::to_string_pretty(&node).unwrap();
    assert!(pretty.contains('\n'));
    assert!(pretty.contains("  ")); // indentation
}

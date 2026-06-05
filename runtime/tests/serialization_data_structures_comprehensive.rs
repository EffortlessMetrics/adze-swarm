// Comprehensive tests for serialization data structures (SerializedNode, SExpr, CompactNode, BinaryFormat)
// These tests exercise the data structures without needing tree-sitter parsing.
// Requires the "serialization" feature.
#![cfg(feature = "serialization")]

use adze::serialization::*;

// ---------------------------------------------------------------------------
// SExpr tests
// ---------------------------------------------------------------------------

#[test]
fn sexpr_atom_creation() {
    let atom = SExpr::Atom("hello".to_string());
    assert!(matches!(atom, SExpr::Atom(ref s) if s == "hello"));
}

#[test]
fn sexpr_list_creation() {
    let list = SExpr::List(vec![
        SExpr::Atom("a".to_string()),
        SExpr::Atom("b".to_string()),
    ]);
    if let SExpr::List(items) = &list {
        assert_eq!(items.len(), 2);
    } else {
        panic!("expected list");
    }
}

#[test]
fn sexpr_empty_list() {
    let list = SExpr::List(vec![]);
    if let SExpr::List(items) = &list {
        assert!(items.is_empty());
    } else {
        panic!("expected list");
    }
}

#[test]
fn sexpr_nested_list() {
    let inner = SExpr::List(vec![SExpr::Atom("x".to_string())]);
    let outer = SExpr::List(vec![inner.clone()]);
    if let SExpr::List(items) = &outer {
        assert_eq!(items.len(), 1);
        assert!(matches!(&items[0], SExpr::List(_)));
    } else {
        panic!("expected list");
    }
}

#[test]
fn sexpr_equality_atoms() {
    let a = SExpr::Atom("test".to_string());
    let b = SExpr::Atom("test".to_string());
    assert_eq!(a, b);
}

#[test]
fn sexpr_inequality_atoms() {
    let a = SExpr::Atom("foo".to_string());
    let b = SExpr::Atom("bar".to_string());
    assert_ne!(a, b);
}

#[test]
fn sexpr_equality_lists() {
    let a = SExpr::List(vec![SExpr::Atom("x".to_string())]);
    let b = SExpr::List(vec![SExpr::Atom("x".to_string())]);
    assert_eq!(a, b);
}

#[test]
fn sexpr_inequality_atom_vs_list() {
    let atom = SExpr::Atom("x".to_string());
    let list = SExpr::List(vec![SExpr::Atom("x".to_string())]);
    assert_ne!(atom, list);
}

#[test]
fn sexpr_clone() {
    let original = SExpr::List(vec![
        SExpr::Atom("a".to_string()),
        SExpr::List(vec![SExpr::Atom("b".to_string())]),
    ]);
    let cloned = original.clone();
    assert_eq!(original, cloned);
}

#[test]
fn sexpr_debug_format() {
    let atom = SExpr::Atom("test".to_string());
    let debug = format!("{:?}", atom);
    assert!(debug.contains("Atom"));
    assert!(debug.contains("test"));
}

#[test]
fn sexpr_debug_list() {
    let list = SExpr::List(vec![SExpr::Atom("item".to_string())]);
    let debug = format!("{:?}", list);
    assert!(debug.contains("List"));
    assert!(debug.contains("item"));
}

#[test]
fn sexpr_deeply_nested() {
    let mut current = SExpr::Atom("leaf".to_string());
    for _ in 0..10 {
        current = SExpr::List(vec![current]);
    }
    // Just verify it doesn't panic
    let _ = format!("{:?}", current);
}

#[test]
fn sexpr_many_siblings() {
    let items: Vec<SExpr> = (0..100)
        .map(|i| SExpr::Atom(format!("item_{}", i)))
        .collect();
    let list = SExpr::List(items);
    if let SExpr::List(items) = &list {
        assert_eq!(items.len(), 100);
    } else {
        panic!("expected list");
    }
}

#[test]
fn sexpr_serde_roundtrip_atom() {
    let atom = SExpr::Atom("hello".to_string());
    let json = serde_json::to_string(&atom).unwrap();
    let back: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(atom, back);
}

#[test]
fn sexpr_serde_roundtrip_list() {
    let list = SExpr::List(vec![
        SExpr::Atom("a".to_string()),
        SExpr::Atom("b".to_string()),
    ]);
    let json = serde_json::to_string(&list).unwrap();
    let back: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(list, back);
}

#[test]
fn sexpr_serde_roundtrip_nested() {
    let nested = SExpr::List(vec![
        SExpr::Atom("root".to_string()),
        SExpr::List(vec![SExpr::Atom("child".to_string())]),
    ]);
    let json = serde_json::to_string(&nested).unwrap();
    let back: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(nested, back);
}

#[test]
fn sexpr_serde_roundtrip_empty_list() {
    let empty = SExpr::List(vec![]);
    let json = serde_json::to_string(&empty).unwrap();
    let back: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(empty, back);
}

// ---------------------------------------------------------------------------
// parse_sexpr tests
// ---------------------------------------------------------------------------

#[test]
fn parse_sexpr_returns_ok() {
    let result = parse_sexpr("(a b c)");
    assert!(result.is_ok());
}

#[test]
fn parse_sexpr_empty_input() {
    let result = parse_sexpr("");
    assert!(result.is_err());
}

#[test]
fn parse_sexpr_returns_atom() {
    let result = parse_sexpr("anything").unwrap();
    assert_eq!(result, SExpr::Atom("anything".to_string()));
}

// ---------------------------------------------------------------------------
// SerializedNode tests
// ---------------------------------------------------------------------------

fn make_leaf(kind: &str, start: usize, end: usize) -> SerializedNode {
    SerializedNode {
        kind: kind.to_string(),
        is_named: true,
        field_name: None,
        start_position: (0, start),
        end_position: (0, end),
        start_byte: start,
        end_byte: end,
        text: Some("text".to_string()),
        children: vec![],
        is_error: false,
        is_missing: false,
    }
}

fn make_node(kind: &str, children: Vec<SerializedNode>) -> SerializedNode {
    let start = children.first().map(|c| c.start_byte).unwrap_or(0);
    let end = children.last().map(|c| c.end_byte).unwrap_or(0);
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

#[test]
fn serialized_node_leaf() {
    let node = make_leaf("number", 0, 3);
    assert_eq!(node.kind, "number");
    assert_eq!(node.start_byte, 0);
    assert_eq!(node.end_byte, 3);
    assert!(node.children.is_empty());
    assert!(node.text.is_some());
}

#[test]
fn serialized_node_with_children() {
    let left = make_leaf("num", 0, 1);
    let right = make_leaf("num", 2, 3);
    let parent = make_node("expr", vec![left, right]);
    assert_eq!(parent.children.len(), 2);
    assert!(parent.text.is_none());
}

#[test]
fn serialized_node_field_name() {
    let mut node = make_leaf("ident", 0, 5);
    node.field_name = Some("name".to_string());
    assert_eq!(node.field_name.as_deref(), Some("name"));
}

#[test]
fn serialized_node_error() {
    let mut node = make_leaf("ERROR", 0, 1);
    node.is_error = true;
    assert!(node.is_error);
}

#[test]
fn serialized_node_missing() {
    let mut node = make_leaf("MISSING", 0, 0);
    node.is_missing = true;
    assert!(node.is_missing);
}

#[test]
fn serialized_node_unnamed() {
    let mut node = make_leaf("+", 0, 1);
    node.is_named = false;
    assert!(!node.is_named);
}

#[test]
fn serialized_node_serde_roundtrip() {
    let node = make_leaf("test", 0, 4);
    let json = serde_json::to_string(&node).unwrap();
    let back: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.kind, "test");
    assert_eq!(back.start_byte, 0);
    assert_eq!(back.end_byte, 4);
}

#[test]
fn serialized_node_nested_serde_roundtrip() {
    let child = make_leaf("child", 0, 3);
    let parent = make_node("parent", vec![child]);
    let json = serde_json::to_string_pretty(&parent).unwrap();
    let back: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.kind, "parent");
    assert_eq!(back.children.len(), 1);
    assert_eq!(back.children[0].kind, "child");
}

#[test]
fn serialized_node_clone() {
    let node = make_leaf("clone_me", 10, 20);
    let cloned = node.clone();
    assert_eq!(cloned.kind, "clone_me");
    assert_eq!(cloned.start_byte, 10);
}

#[test]
fn serialized_node_debug() {
    let node = make_leaf("debug_test", 0, 1);
    let debug = format!("{:?}", node);
    assert!(debug.contains("debug_test"));
}

#[test]
fn serialized_node_deep_tree() {
    let leaf = make_leaf("deep", 0, 1);
    let mut current = leaf;
    for i in 0..10 {
        current = make_node(&format!("level_{}", i), vec![current]);
    }
    assert_eq!(current.kind, "level_9");
}

#[test]
fn serialized_node_wide_tree() {
    let children: Vec<SerializedNode> = (0..50)
        .map(|i| make_leaf(&format!("child_{}", i), i * 2, i * 2 + 1))
        .collect();
    let parent = make_node("wide", children);
    assert_eq!(parent.children.len(), 50);
}

// ---------------------------------------------------------------------------
// CompactNode tests
// ---------------------------------------------------------------------------

#[test]
fn compact_node_basic() {
    let node = CompactNode {
        kind: "expr".to_string(),
        start: Some(0),
        end: Some(10),
        field: None,
        children: vec![],
        text: Some("hello".to_string()),
    };
    assert_eq!(node.kind, "expr");
    assert_eq!(node.text.as_deref(), Some("hello"));
}

#[test]
fn compact_node_serde_roundtrip() {
    let node = CompactNode {
        kind: "num".to_string(),
        start: Some(5),
        end: Some(8),
        field: Some("value".to_string()),
        children: vec![],
        text: Some("42".to_string()),
    };
    let json = serde_json::to_string(&node).unwrap();
    let back: CompactNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.kind, "num");
    assert_eq!(back.text.as_deref(), Some("42"));
    assert_eq!(back.field.as_deref(), Some("value"));
}

#[test]
fn compact_node_skip_empty_children() {
    let node = CompactNode {
        kind: "leaf".to_string(),
        start: None,
        end: None,
        field: None,
        children: vec![],
        text: Some("x".to_string()),
    };
    let json = serde_json::to_string(&node).unwrap();
    // "c" should be skipped when empty
    assert!(!json.contains("\"c\""));
}

#[test]
fn compact_node_skip_none_fields() {
    let node = CompactNode {
        kind: "test".to_string(),
        start: None,
        end: None,
        field: None,
        children: vec![],
        text: None,
    };
    let json = serde_json::to_string(&node).unwrap();
    assert!(!json.contains("\"s\""));
    assert!(!json.contains("\"e\""));
    assert!(!json.contains("\"f\""));
    assert!(!json.contains("\"x\""));
}

#[test]
fn compact_node_with_children() {
    let child = CompactNode {
        kind: "lit".to_string(),
        start: None,
        end: None,
        field: None,
        children: vec![],
        text: Some("1".to_string()),
    };
    let parent = CompactNode {
        kind: "expr".to_string(),
        start: Some(0),
        end: Some(5),
        field: None,
        children: vec![child],
        text: None,
    };
    let json = serde_json::to_string(&parent).unwrap();
    assert!(json.contains("\"c\""));
}

#[test]
fn compact_node_debug() {
    let node = CompactNode {
        kind: "dbg".to_string(),
        start: Some(0),
        end: Some(1),
        field: None,
        children: vec![],
        text: None,
    };
    let debug = format!("{:?}", node);
    assert!(debug.contains("dbg"));
}

#[test]
fn compact_node_clone() {
    let node = CompactNode {
        kind: "original".to_string(),
        start: Some(0),
        end: Some(5),
        field: Some("field".to_string()),
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
    let cloned = node.clone();
    assert_eq!(cloned.kind, "original");
    assert_eq!(cloned.children.len(), 1);
}

// ---------------------------------------------------------------------------
// BinaryFormat tests
// ---------------------------------------------------------------------------

#[test]
fn binary_format_empty() {
    let fmt = BinaryFormat {
        node_types: vec![],
        field_names: vec![],
        tree_data: vec![],
    };
    assert!(fmt.node_types.is_empty());
    assert!(fmt.field_names.is_empty());
    assert!(fmt.tree_data.is_empty());
}

#[test]
fn binary_format_with_types() {
    let fmt = BinaryFormat {
        node_types: vec!["expr".to_string(), "num".to_string()],
        field_names: vec!["left".to_string(), "right".to_string()],
        tree_data: vec![0, 1, 2, 3],
    };
    assert_eq!(fmt.node_types.len(), 2);
    assert_eq!(fmt.field_names.len(), 2);
    assert_eq!(fmt.tree_data.len(), 4);
}

#[test]
fn binary_format_debug() {
    let fmt = BinaryFormat {
        node_types: vec!["test".to_string()],
        field_names: vec![],
        tree_data: vec![42],
    };
    let debug = format!("{:?}", fmt);
    assert!(debug.contains("test"));
}

#[test]
fn binary_format_clone() {
    let fmt = BinaryFormat {
        node_types: vec!["a".to_string()],
        field_names: vec!["f".to_string()],
        tree_data: vec![1, 2, 3],
    };
    let cloned = fmt.clone();
    assert_eq!(cloned.node_types, fmt.node_types);
    assert_eq!(cloned.tree_data, fmt.tree_data);
}

// ---------------------------------------------------------------------------
// BinarySerializer unit tests (just the type/field ID allocation)
// ---------------------------------------------------------------------------

#[test]
fn binary_serializer_new() {
    let _bs = BinarySerializer::new();
    // Just verify it doesn't panic
}

#[test]
fn binary_serializer_default() {
    let _bs = BinarySerializer::default();
    // Default should work via Default impl
}

// ---------------------------------------------------------------------------
// TreeSerializer builder tests
// ---------------------------------------------------------------------------

#[test]
fn tree_serializer_new() {
    let source = b"hello world";
    let ts = TreeSerializer::new(source);
    assert!(!ts.include_unnamed);
    assert_eq!(ts.max_text_length, Some(100));
}

#[test]
fn tree_serializer_with_unnamed() {
    let source = b"test";
    let ts = TreeSerializer::new(source).with_unnamed_nodes();
    assert!(ts.include_unnamed);
}

#[test]
fn tree_serializer_with_max_length() {
    let source = b"test";
    let ts = TreeSerializer::new(source).with_max_text_length(Some(50));
    assert_eq!(ts.max_text_length, Some(50));
}

#[test]
fn tree_serializer_unlimited_text() {
    let source = b"test";
    let ts = TreeSerializer::new(source).with_max_text_length(None);
    assert_eq!(ts.max_text_length, None);
}

#[test]
fn tree_serializer_builder_chain() {
    let source = b"chain";
    let ts = TreeSerializer::new(source)
        .with_unnamed_nodes()
        .with_max_text_length(Some(200));
    assert!(ts.include_unnamed);
    assert_eq!(ts.max_text_length, Some(200));
}

// ---------------------------------------------------------------------------
// SExpressionSerializer builder tests
// ---------------------------------------------------------------------------

#[test]
fn sexpr_serializer_new() {
    let source = b"test";
    let _ser = SExpressionSerializer::new(source);
}

#[test]
fn sexpr_serializer_with_positions() {
    let source = b"test";
    // with_positions() returns Self — just verify it doesn't panic and the builder chains
    let _ser = SExpressionSerializer::new(source).with_positions();
}

// ---------------------------------------------------------------------------
// CompactSerializer builder tests
// ---------------------------------------------------------------------------

#[test]
fn compact_serializer_new() {
    let source = b"test";
    let _ser = CompactSerializer::new(source);
}

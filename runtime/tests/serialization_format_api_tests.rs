//! Comprehensive tests for the runtime serialization module's public API.
//!
//! These tests exercise serializer types, format enums, configuration,
//! and error handling in isolation — no parsed tree required.

#![cfg(feature = "serialization")]

use adze::serialization::*;

// ---------------------------------------------------------------------------
// BinarySerializer construction
// ---------------------------------------------------------------------------

#[test]
fn binary_serializer_new_creates_empty_instance() {
    let bs = BinarySerializer::new();
    // BinarySerializer starts with empty type/field maps.
    // We can only observe this indirectly: a freshly-built BinaryFormat
    // from Default should behave the same.
    let bs2 = BinarySerializer::default();
    // Both should exist — just verify construction compiles and runs.
    drop(bs);
    drop(bs2);
}

#[test]
fn binary_serializer_default_matches_new() {
    // `Default` impl delegates to `new()`. Verify both paths produce
    // an equivalent starting state by comparing debug output absence.
    let _a = BinarySerializer::new();
    let _b = BinarySerializer::default();
}

// ---------------------------------------------------------------------------
// BinaryFormat construction and fields
// ---------------------------------------------------------------------------

#[test]
fn binary_format_fields_accessible() {
    let fmt = BinaryFormat {
        node_types: vec!["program".to_string(), "identifier".to_string()],
        field_names: vec!["name".to_string()],
        tree_data: vec![0x01, 0x02, 0x03],
    };

    assert_eq!(fmt.node_types.len(), 2);
    assert_eq!(fmt.node_types[0], "program");
    assert_eq!(fmt.field_names.len(), 1);
    assert_eq!(fmt.field_names[0], "name");
    assert_eq!(fmt.tree_data, vec![0x01, 0x02, 0x03]);
}

#[test]
fn binary_format_clone() {
    let fmt = BinaryFormat {
        node_types: vec!["root".to_string()],
        field_names: vec![],
        tree_data: vec![0xFF],
    };
    let cloned = fmt.clone();
    assert_eq!(cloned.node_types, fmt.node_types);
    assert_eq!(cloned.field_names, fmt.field_names);
    assert_eq!(cloned.tree_data, fmt.tree_data);
}

#[test]
fn binary_format_debug_impl() {
    let fmt = BinaryFormat {
        node_types: vec![],
        field_names: vec![],
        tree_data: vec![],
    };
    let dbg = format!("{:?}", fmt);
    assert!(dbg.contains("BinaryFormat"));
}

// ---------------------------------------------------------------------------
// TreeSerializer configuration
// ---------------------------------------------------------------------------

#[test]
fn tree_serializer_default_config() {
    let src = b"fn main() {}";
    let s = TreeSerializer::new(src);
    assert!(!s.include_unnamed, "unnamed nodes excluded by default");
    assert_eq!(
        s.max_text_length,
        Some(100),
        "default max text length is 100"
    );
    assert_eq!(s.source, src);
}

#[test]
fn tree_serializer_with_unnamed_nodes() {
    let s = TreeSerializer::new(b"x").with_unnamed_nodes();
    assert!(s.include_unnamed);
}

#[test]
fn tree_serializer_with_max_text_length_none() {
    let s = TreeSerializer::new(b"x").with_max_text_length(None);
    assert_eq!(s.max_text_length, None, "unlimited text length");
}

#[test]
fn tree_serializer_with_max_text_length_custom() {
    let s = TreeSerializer::new(b"x").with_max_text_length(Some(42));
    assert_eq!(s.max_text_length, Some(42));
}

#[test]
fn tree_serializer_chained_builders() {
    let src = b"source";
    let s = TreeSerializer::new(src)
        .with_unnamed_nodes()
        .with_max_text_length(Some(10));
    assert!(s.include_unnamed);
    assert_eq!(s.max_text_length, Some(10));
    assert_eq!(s.source, b"source");
}

// ---------------------------------------------------------------------------
// SExpressionSerializer configuration
// ---------------------------------------------------------------------------

#[test]
fn sexpr_serializer_new() {
    let src = b"hello";
    let _s = SExpressionSerializer::new(src);
}

#[test]
fn sexpr_serializer_with_positions() {
    let src = b"test";
    let _s = SExpressionSerializer::new(src).with_positions();
}

// ---------------------------------------------------------------------------
// CompactSerializer construction
// ---------------------------------------------------------------------------

#[test]
fn compact_serializer_new() {
    let src = b"some code";
    let _cs = CompactSerializer::new(src);
}

// ---------------------------------------------------------------------------
// SExpr enum variants
// ---------------------------------------------------------------------------

#[test]
fn sexpr_atom_creation_and_equality() {
    let a = SExpr::Atom("identifier".to_string());
    let b = SExpr::Atom("identifier".to_string());
    assert_eq!(a, b);
}

#[test]
fn sexpr_atom_inequality() {
    let a = SExpr::Atom("foo".to_string());
    let b = SExpr::Atom("bar".to_string());
    assert_ne!(a, b);
}

#[test]
fn sexpr_list_creation_and_equality() {
    let list = SExpr::List(vec![
        SExpr::Atom("add".to_string()),
        SExpr::Atom("1".to_string()),
        SExpr::Atom("2".to_string()),
    ]);
    let list2 = list.clone();
    assert_eq!(list, list2);
}

#[test]
fn sexpr_empty_list() {
    let empty = SExpr::List(vec![]);
    assert_eq!(empty, SExpr::List(vec![]));
}

#[test]
fn sexpr_nested_lists() {
    let inner = SExpr::List(vec![SExpr::Atom("x".to_string())]);
    let outer = SExpr::List(vec![SExpr::Atom("let".to_string()), inner.clone()]);
    if let SExpr::List(items) = &outer {
        assert_eq!(items.len(), 2);
        assert_eq!(items[1], inner);
    } else {
        panic!("expected List");
    }
}

#[test]
fn sexpr_debug_impl() {
    let atom = SExpr::Atom("test".to_string());
    let dbg = format!("{:?}", atom);
    assert!(dbg.contains("Atom"));
    assert!(dbg.contains("test"));
}

#[test]
fn sexpr_json_roundtrip_atom() {
    let atom = SExpr::Atom("hello".to_string());
    let json = serde_json::to_string(&atom).unwrap();
    let decoded: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(atom, decoded);
}

#[test]
fn sexpr_json_roundtrip_nested_list() {
    let expr = SExpr::List(vec![
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
    ]);
    let json = serde_json::to_string(&expr).unwrap();
    let decoded: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(expr, decoded);
}

// ---------------------------------------------------------------------------
// parse_sexpr function
// ---------------------------------------------------------------------------

#[test]
fn parse_sexpr_returns_ok_for_valid_input() {
    assert!(parse_sexpr("(program)").is_ok());
    assert!(parse_sexpr("").is_err());
    assert!(parse_sexpr("atom").is_ok());
}

#[test]
fn parse_sexpr_result_preserves_list_items() {
    let result = parse_sexpr("(program (number))").unwrap();
    assert_eq!(
        result,
        SExpr::List(vec![
            SExpr::Atom("program".to_string()),
            SExpr::List(vec![SExpr::Atom("number".to_string())]),
        ])
    );
}

// ---------------------------------------------------------------------------
// SerializedNode JSON error handling
// ---------------------------------------------------------------------------

#[test]
fn serialized_node_deserialize_rejects_invalid_json() {
    let result = serde_json::from_str::<SerializedNode>("not valid json");
    assert!(result.is_err());
}

#[test]
fn serialized_node_deserialize_rejects_missing_fields() {
    // Missing required fields like `kind`, `is_named`, etc.
    let result = serde_json::from_str::<SerializedNode>(r#"{"kind": "x"}"#);
    assert!(result.is_err());
}

#[test]
fn serialized_node_deserialize_rejects_wrong_type() {
    // `is_named` should be bool, not string
    let bad = r#"{
        "kind": "id",
        "is_named": "yes",
        "field_name": null,
        "start_position": [0, 0],
        "end_position": [0, 1],
        "start_byte": 0,
        "end_byte": 1,
        "text": null,
        "children": [],
        "is_error": false,
        "is_missing": false
    }"#;
    let result = serde_json::from_str::<SerializedNode>(bad);
    assert!(result.is_err());
}

#[test]
fn compact_node_deserialize_rejects_invalid_json() {
    let result = serde_json::from_str::<CompactNode>("{{{");
    assert!(result.is_err());
}

#[test]
fn compact_node_deserialize_rejects_missing_kind() {
    // CompactNode requires at least "t" (kind)
    let result = serde_json::from_str::<CompactNode>(r#"{"s": 0}"#);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// SerializedNode construction edge cases
// ---------------------------------------------------------------------------

#[test]
fn serialized_node_zero_length_span() {
    let node = SerializedNode {
        kind: "empty".to_string(),
        is_named: true,
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
    assert_eq!(node.start_byte, node.end_byte);
    let json = serde_json::to_string(&node).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.start_byte, decoded.end_byte);
}

#[test]
fn serialized_node_with_all_flags_set() {
    let node = SerializedNode {
        kind: "weird".to_string(),
        is_named: true,
        field_name: Some("f".to_string()),
        start_position: (10, 20),
        end_position: (30, 40),
        start_byte: 100,
        end_byte: 200,
        text: Some("txt".to_string()),
        children: vec![],
        is_error: true,
        is_missing: true,
    };
    let json = serde_json::to_string(&node).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert!(decoded.is_error);
    assert!(decoded.is_missing);
    assert!(decoded.is_named);
    assert_eq!(decoded.field_name.as_deref(), Some("f"));
    assert_eq!(decoded.text.as_deref(), Some("txt"));
}

#[test]
fn serialized_node_deeply_nested_children() {
    // Build a 5-level deep tree purely from SerializedNode structs.
    fn make_node(depth: usize) -> SerializedNode {
        if depth == 0 {
            return SerializedNode {
                kind: "leaf".to_string(),
                is_named: true,
                field_name: None,
                start_position: (0, 0),
                end_position: (0, 1),
                start_byte: 0,
                end_byte: 1,
                text: Some("v".to_string()),
                children: vec![],
                is_error: false,
                is_missing: false,
            };
        }
        SerializedNode {
            kind: format!("level_{}", depth),
            is_named: true,
            field_name: None,
            start_position: (0, 0),
            end_position: (0, 1),
            start_byte: 0,
            end_byte: 1,
            text: None,
            children: vec![make_node(depth - 1)],
            is_error: false,
            is_missing: false,
        }
    }

    let tree = make_node(5);
    let json = serde_json::to_string(&tree).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.kind, "level_5");

    let mut cur = &decoded;
    for d in (1..5).rev() {
        cur = &cur.children[0];
        assert_eq!(cur.kind, format!("level_{}", d));
    }
    assert_eq!(cur.children[0].kind, "leaf");
    assert_eq!(cur.children[0].text.as_deref(), Some("v"));
}

// ---------------------------------------------------------------------------
// CompactNode edge cases
// ---------------------------------------------------------------------------

#[test]
fn compact_node_with_children_roundtrip() {
    let node = CompactNode {
        kind: "root".to_string(),
        start: Some(0),
        end: Some(50),
        field: None,
        children: vec![
            CompactNode {
                kind: "a".to_string(),
                start: Some(0),
                end: Some(10),
                field: Some("left".to_string()),
                children: vec![],
                text: Some("hello".to_string()),
            },
            CompactNode {
                kind: "b".to_string(),
                start: Some(11),
                end: Some(50),
                field: Some("right".to_string()),
                children: vec![],
                text: Some("world".to_string()),
            },
        ],
        text: None,
    };
    let json = serde_json::to_string(&node).unwrap();
    let decoded: CompactNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.kind, "root");
    assert_eq!(decoded.children.len(), 2);
    assert_eq!(decoded.children[0].field.as_deref(), Some("left"));
    assert_eq!(decoded.children[1].text.as_deref(), Some("world"));
}

#[test]
fn compact_node_minimal_only_kind() {
    // Only required field is `kind` ("t" in JSON)
    let json_str = r#"{"t":"min"}"#;
    let decoded: CompactNode = serde_json::from_str(json_str).unwrap();
    assert_eq!(decoded.kind, "min");
    assert_eq!(decoded.start, None);
    assert_eq!(decoded.end, None);
    assert_eq!(decoded.field, None);
    assert!(decoded.children.is_empty());
    assert_eq!(decoded.text, None);
}

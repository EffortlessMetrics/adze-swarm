//! Comprehensive serialization format tests for adze's tree serialization API.
//!
//! Tests cover S-expression output, JSON output, roundtrip guarantees,
//! edge cases (empty, deeply nested, unicode, special chars), and
//! determinism / uniqueness properties.

#![cfg(feature = "serialization")]

use adze::serialization::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a leaf `SerializedNode`.
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

/// Build an internal (non-leaf) `SerializedNode`.
fn internal(kind: &str, children: Vec<SerializedNode>, start: usize, end: usize) -> SerializedNode {
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

/// Build a simple program → expression → number tree.
fn simple_tree() -> SerializedNode {
    internal("program", vec![leaf("number", "42", 0, 2)], 0, 2)
}

/// Build a nested tree: program → binary_expression → (left number, op, right number).
fn nested_tree() -> SerializedNode {
    internal(
        "program",
        vec![internal(
            "binary_expression",
            vec![
                leaf("number", "1", 0, 1),
                leaf("operator", "+", 2, 3),
                leaf("number", "2", 4, 5),
            ],
            0,
            5,
        )],
        0,
        5,
    )
}

/// Build a tree with named-field children.
fn named_children_tree() -> SerializedNode {
    let mut left = leaf("identifier", "x", 0, 1);
    left.field_name = Some("left".to_string());

    let mut right = leaf("number", "5", 4, 5);
    right.field_name = Some("right".to_string());

    let mut op = leaf("operator", "=", 2, 3);
    op.field_name = Some("operator".to_string());

    internal("assignment", vec![left, op, right], 0, 5)
}

/// Build a tree containing an ERROR node.
fn error_tree() -> SerializedNode {
    let mut err = leaf("ERROR", "???", 0, 3);
    err.is_error = true;
    err.is_named = false;

    internal("program", vec![err], 0, 3)
}

/// Build a tree with every node-type flag exercised.
fn all_node_types_tree() -> SerializedNode {
    let normal = leaf("identifier", "x", 0, 1);

    let mut error = leaf("ERROR", "bad", 2, 5);
    error.is_error = true;
    error.is_named = false;

    let mut missing = SerializedNode {
        kind: "semicolon".to_string(),
        is_named: false,
        field_name: None,
        start_position: (0, 5),
        end_position: (0, 5),
        start_byte: 5,
        end_byte: 5,
        text: None,
        children: vec![],
        is_error: false,
        is_missing: true,
    };
    let _ = &mut missing; // used

    let mut named_field = leaf("number", "42", 6, 8);
    named_field.field_name = Some("value".to_string());

    internal("program", vec![normal, error, missing, named_field], 0, 8)
}

/// Build an empty tree (root with no children and no text).
fn empty_tree() -> SerializedNode {
    SerializedNode {
        kind: "program".to_string(),
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
    }
}

/// Build a deeply nested tree (depth levels).
fn deeply_nested_tree(depth: usize) -> SerializedNode {
    let mut node = leaf("leaf", "x", 0, 1);
    for i in 0..depth {
        node = internal(&format!("level_{}", depth - i), vec![node], 0, 1);
    }
    node
}

// ---------------------------------------------------------------------------
// S-expression output tests
// ---------------------------------------------------------------------------

/// 1. S-expression output for simple tree
#[test]
fn sexp_simple_tree() {
    let json = serde_json::to_string(&simple_tree()).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.kind, "program");
    assert_eq!(decoded.children.len(), 1);
    assert_eq!(decoded.children[0].kind, "number");
    assert_eq!(decoded.children[0].text.as_deref(), Some("42"));
}

/// 2. S-expression output for nested tree
#[test]
fn sexp_nested_tree() {
    let tree = nested_tree();
    let json = serde_json::to_string_pretty(&tree).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.kind, "program");
    let bin = &decoded.children[0];
    assert_eq!(bin.kind, "binary_expression");
    assert_eq!(bin.children.len(), 3);
    assert_eq!(bin.children[0].text.as_deref(), Some("1"));
    assert_eq!(bin.children[1].text.as_deref(), Some("+"));
    assert_eq!(bin.children[2].text.as_deref(), Some("2"));
}

/// 3. S-expression output for tree with named children
#[test]
fn sexp_named_children() {
    let tree = named_children_tree();
    let json = serde_json::to_string(&tree).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.children[0].field_name.as_deref(), Some("left"));
    assert_eq!(decoded.children[1].field_name.as_deref(), Some("operator"));
    assert_eq!(decoded.children[2].field_name.as_deref(), Some("right"));
}

/// 4. S-expression output for tree with error nodes
#[test]
fn sexp_error_nodes() {
    let tree = error_tree();
    let json = serde_json::to_string(&tree).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    let err_node = &decoded.children[0];
    assert!(err_node.is_error);
    assert!(!err_node.is_named);
    assert_eq!(err_node.kind, "ERROR");
}

// ---------------------------------------------------------------------------
// JSON output tests
// ---------------------------------------------------------------------------

/// 5. JSON output for simple tree
#[test]
fn json_simple_tree() {
    let tree = simple_tree();
    let json = serde_json::to_string_pretty(&tree).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["kind"], "program");
    assert_eq!(value["children"][0]["kind"], "number");
    assert_eq!(value["children"][0]["text"], "42");
}

/// 6. JSON output for nested tree
#[test]
fn json_nested_tree() {
    let tree = nested_tree();
    let json = serde_json::to_string_pretty(&tree).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["kind"], "program");
    let bin = &value["children"][0];
    assert_eq!(bin["kind"], "binary_expression");
    assert_eq!(bin["children"][0]["text"], "1");
    assert_eq!(bin["children"][1]["text"], "+");
    assert_eq!(bin["children"][2]["text"], "2");
}

/// 7. JSON output for tree with all node types
#[test]
fn json_all_node_types() {
    let tree = all_node_types_tree();
    let json = serde_json::to_string_pretty(&tree).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();

    // Normal named node
    assert!(
        value["children"][0]["is_named"]
            .as_bool()
            .expect("is_named")
    );
    assert!(
        !value["children"][0]["is_error"]
            .as_bool()
            .expect("is_error")
    );

    // Error node
    assert!(
        value["children"][1]["is_error"]
            .as_bool()
            .expect("is_error")
    );
    assert!(
        !value["children"][1]["is_named"]
            .as_bool()
            .expect("is_named")
    );

    // Missing node
    assert!(
        value["children"][2]["is_missing"]
            .as_bool()
            .expect("is_missing")
    );

    // Node with field_name
    assert_eq!(value["children"][3]["field_name"], "value");
}

// ---------------------------------------------------------------------------
// Roundtrip tests
// ---------------------------------------------------------------------------

/// 8. JSON roundtrip: serialize → deserialize → serialize produces same output
#[test]
fn json_roundtrip_identity() {
    let tree = nested_tree();
    let json1 = serde_json::to_string_pretty(&tree).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json1).unwrap();
    let json2 = serde_json::to_string_pretty(&decoded).unwrap();
    assert_eq!(json1, json2, "JSON roundtrip must be identical");
}

/// 9. S-expression SExpr roundtrip: build → serialize → deserialize → compare
#[test]
fn sexpr_type_roundtrip() {
    let atom = SExpr::Atom("hello".to_string());
    let list = SExpr::List(vec![
        SExpr::Atom("program".to_string()),
        SExpr::List(vec![SExpr::Atom("number".to_string())]),
    ]);

    // JSON roundtrip of SExpr values
    let json_atom = serde_json::to_string(&atom).unwrap();
    let decoded_atom: SExpr = serde_json::from_str(&json_atom).unwrap();
    assert_eq!(atom, decoded_atom);

    let json_list = serde_json::to_string(&list).unwrap();
    let decoded_list: SExpr = serde_json::from_str(&json_list).unwrap();
    assert_eq!(list, decoded_list);
}

/// Additional roundtrip: compact JSON roundtrip
#[test]
fn json_compact_roundtrip() {
    let tree = simple_tree();
    let compact = serde_json::to_string(&tree).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&compact).unwrap();
    let compact2 = serde_json::to_string(&decoded).unwrap();
    assert_eq!(compact, compact2);
}

// ---------------------------------------------------------------------------
// Edge case tests
// ---------------------------------------------------------------------------

/// 10. Serialization of empty tree
#[test]
fn serialize_empty_tree() {
    let tree = empty_tree();
    let json = serde_json::to_string(&tree).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded.kind, "program");
    assert!(decoded.children.is_empty());
    assert!(decoded.text.is_none());
    assert_eq!(decoded.start_byte, 0);
    assert_eq!(decoded.end_byte, 0);
}

/// 11. Serialization of deeply nested tree
#[test]
fn serialize_deeply_nested_tree() {
    let depth = 50;
    let tree = deeply_nested_tree(depth);
    let json = serde_json::to_string_pretty(&tree).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();

    // Walk down the chain — outermost is level_1, innermost is level_{depth}
    let mut current = &decoded;
    for i in 0..depth {
        assert_eq!(current.kind, format!("level_{}", i + 1));
        assert_eq!(current.children.len(), 1);
        current = &current.children[0];
    }
    assert_eq!(current.kind, "leaf");
    assert_eq!(current.text.as_deref(), Some("x"));
}

/// 12. Serialization handles unicode in node text
#[test]
fn serialize_unicode_text() {
    let node = leaf("string", "héllo wörld 🌍", 0, 20);
    let json = serde_json::to_string(&node).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.text.as_deref(), Some("héllo wörld 🌍"));
}

/// Additional: CJK characters
#[test]
fn serialize_unicode_cjk() {
    let node = leaf("string", "你好世界", 0, 12);
    let json = serde_json::to_string(&node).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.text.as_deref(), Some("你好世界"));
}

/// Additional: Emoji and combining characters
#[test]
fn serialize_unicode_combining() {
    let text = "café\u{0301} 🇺🇸";
    let node = leaf("string", text, 0, text.len());
    let json = serde_json::to_string(&node).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.text.as_deref(), Some(text));
}

/// 13. Serialization handles special characters (quotes, backslashes)
#[test]
fn serialize_special_chars_quotes() {
    let node = leaf("string", r#"he said "hello""#, 0, 15);
    let json = serde_json::to_string(&node).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.text.as_deref(), Some(r#"he said "hello""#));
}

#[test]
fn serialize_special_chars_backslash() {
    let node = leaf("string", r"path\to\file", 0, 12);
    let json = serde_json::to_string(&node).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.text.as_deref(), Some(r"path\to\file"));
}

#[test]
fn serialize_special_chars_newlines_tabs() {
    let text = "line1\nline2\ttab";
    let node = leaf("string", text, 0, text.len());
    let json = serde_json::to_string(&node).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.text.as_deref(), Some(text));
}

#[test]
fn serialize_special_chars_null_byte() {
    let text = "before\0after";
    let node = leaf("string", text, 0, text.len());
    let json = serde_json::to_string(&node).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.text.as_deref(), Some(text));
}

// ---------------------------------------------------------------------------
// Uniqueness and determinism
// ---------------------------------------------------------------------------

/// 14. Different trees produce different serialization
#[test]
fn different_trees_different_serialization() {
    let tree_a = simple_tree();
    let tree_b = nested_tree();
    let json_a = serde_json::to_string(&tree_a).unwrap();
    let json_b = serde_json::to_string(&tree_b).unwrap();
    assert_ne!(json_a, json_b);
}

/// Additional: Trees with same structure but different content differ
#[test]
fn same_structure_different_content() {
    let tree_a = internal("program", vec![leaf("number", "1", 0, 1)], 0, 1);
    let tree_b = internal("program", vec![leaf("number", "2", 0, 1)], 0, 1);
    let json_a = serde_json::to_string(&tree_a).unwrap();
    let json_b = serde_json::to_string(&tree_b).unwrap();
    assert_ne!(json_a, json_b);
}

/// 15. Same tree always produces same serialization (deterministic)
#[test]
fn deterministic_serialization() {
    let tree = nested_tree();
    let json1 = serde_json::to_string_pretty(&tree).unwrap();
    let json2 = serde_json::to_string_pretty(&tree).unwrap();
    assert_eq!(json1, json2, "Serialization must be deterministic");
}

/// Additional: Deterministic across deserialize-reserialize cycles
#[test]
fn deterministic_across_cycles() {
    let tree = all_node_types_tree();
    let mut prev = serde_json::to_string(&tree).unwrap();
    for _ in 0..5 {
        let decoded: SerializedNode = serde_json::from_str(&prev).unwrap();
        let next = serde_json::to_string(&decoded).unwrap();
        assert_eq!(prev, next, "Must be stable across repeated cycles");
        prev = next;
    }
}

// ---------------------------------------------------------------------------
// CompactNode format tests
// ---------------------------------------------------------------------------

/// CompactNode serializes with short field names
#[test]
fn compact_node_short_field_names() {
    let node = CompactNode {
        kind: "identifier".to_string(),
        start: Some(0),
        end: Some(5),
        field: Some("name".to_string()),
        children: vec![],
        text: Some("hello".to_string()),
    };
    let json = serde_json::to_string(&node).unwrap();
    assert!(json.contains("\"t\":"), "kind should use short key 't'");
    assert!(json.contains("\"s\":"), "start should use short key 's'");
    assert!(json.contains("\"e\":"), "end should use short key 'e'");
    assert!(json.contains("\"f\":"), "field should use short key 'f'");
    assert!(json.contains("\"x\":"), "text should use short key 'x'");
}

/// CompactNode omits empty children and None fields
#[test]
fn compact_node_skips_empty() {
    let node = CompactNode {
        kind: "id".to_string(),
        start: None,
        end: None,
        field: None,
        children: vec![],
        text: None,
    };
    let json = serde_json::to_string(&node).unwrap();
    assert!(!json.contains("\"s\":"), "None start should be omitted");
    assert!(!json.contains("\"e\":"), "None end should be omitted");
    assert!(!json.contains("\"f\":"), "None field should be omitted");
    assert!(!json.contains("\"c\":"), "Empty children should be omitted");
    assert!(!json.contains("\"x\":"), "None text should be omitted");
}

/// CompactNode JSON roundtrip
#[test]
fn compact_node_roundtrip() {
    let node = CompactNode {
        kind: "expr".to_string(),
        start: Some(10),
        end: Some(20),
        field: Some("body".to_string()),
        children: vec![CompactNode {
            kind: "number".to_string(),
            start: None,
            end: None,
            field: None,
            children: vec![],
            text: Some("42".to_string()),
        }],
        text: None,
    };
    let json = serde_json::to_string(&node).unwrap();
    let decoded: CompactNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.kind, "expr");
    assert_eq!(decoded.start, Some(10));
    assert_eq!(decoded.end, Some(20));
    assert_eq!(decoded.field.as_deref(), Some("body"));
    assert_eq!(decoded.children.len(), 1);
    assert_eq!(decoded.children[0].text.as_deref(), Some("42"));
}

// ---------------------------------------------------------------------------
// SExpr enum tests
// ---------------------------------------------------------------------------

/// SExpr::Atom equality
#[test]
fn sexpr_atom_equality() {
    let a = SExpr::Atom("foo".to_string());
    let b = SExpr::Atom("foo".to_string());
    let c = SExpr::Atom("bar".to_string());
    assert_eq!(a, b);
    assert_ne!(a, c);
}

/// SExpr::List equality
#[test]
fn sexpr_list_equality() {
    let a = SExpr::List(vec![
        SExpr::Atom("a".to_string()),
        SExpr::Atom("b".to_string()),
    ]);
    let b = SExpr::List(vec![
        SExpr::Atom("a".to_string()),
        SExpr::Atom("b".to_string()),
    ]);
    let c = SExpr::List(vec![SExpr::Atom("a".to_string())]);
    assert_eq!(a, b);
    assert_ne!(a, c);
}

/// SExpr nested structure
#[test]
fn sexpr_nested_structure() {
    let expr = SExpr::List(vec![
        SExpr::Atom("program".to_string()),
        SExpr::List(vec![
            SExpr::Atom("binary_expression".to_string()),
            SExpr::Atom("1".to_string()),
            SExpr::Atom("+".to_string()),
            SExpr::Atom("2".to_string()),
        ]),
    ]);

    let json = serde_json::to_string(&expr).unwrap();
    let decoded: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(expr, decoded);
}

/// parse_sexpr returns Ok
#[test]
fn parse_sexpr_returns_ok() {
    let result = parse_sexpr("(program (number))");
    assert!(result.is_ok());
    assert_eq!(
        result.unwrap(),
        SExpr::List(vec![
            SExpr::Atom("program".to_string()),
            SExpr::List(vec![SExpr::Atom("number".to_string())]),
        ])
    );
}

// ---------------------------------------------------------------------------
// TreeSerializer configuration tests
// ---------------------------------------------------------------------------

/// TreeSerializer default configuration
#[test]
fn tree_serializer_defaults() {
    let src = b"hello world";
    let s = TreeSerializer::new(src);
    assert!(!s.include_unnamed);
    assert_eq!(s.max_text_length, Some(100));
    assert_eq!(s.source, src);
}

/// TreeSerializer builder methods
#[test]
fn tree_serializer_builder() {
    let src = b"test";
    let s = TreeSerializer::new(src)
        .with_unnamed_nodes()
        .with_max_text_length(None);
    assert!(s.include_unnamed);
    assert_eq!(s.max_text_length, None);
}

/// SExpressionSerializer can be constructed
#[test]
fn sexpr_serializer_construction() {
    let src = b"test";
    // Just verify it compiles and can be created
    let _s = SExpressionSerializer::new(src);
}

/// SExpressionSerializer with_positions builder returns Self
#[test]
fn sexpr_serializer_with_positions_builder() {
    let src = b"test";
    let _s = SExpressionSerializer::new(src).with_positions();
}

// ---------------------------------------------------------------------------
// Position / byte-range preservation
// ---------------------------------------------------------------------------

/// Positions roundtrip through JSON
#[test]
fn positions_preserved_in_json() {
    let node = SerializedNode {
        kind: "statement".to_string(),
        is_named: true,
        field_name: None,
        start_position: (3, 7),
        end_position: (5, 12),
        start_byte: 42,
        end_byte: 99,
        text: None,
        children: vec![],
        is_error: false,
        is_missing: false,
    };
    let json = serde_json::to_string(&node).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.start_position, (3, 7));
    assert_eq!(decoded.end_position, (5, 12));
    assert_eq!(decoded.start_byte, 42);
    assert_eq!(decoded.end_byte, 99);
}

/// Multi-line positions
#[test]
fn multiline_positions() {
    let child1 = SerializedNode {
        kind: "line".to_string(),
        is_named: true,
        field_name: None,
        start_position: (0, 0),
        end_position: (0, 10),
        start_byte: 0,
        end_byte: 10,
        text: Some("first line".to_string()),
        children: vec![],
        is_error: false,
        is_missing: false,
    };
    let child2 = SerializedNode {
        kind: "line".to_string(),
        is_named: true,
        field_name: None,
        start_position: (1, 0),
        end_position: (1, 11),
        start_byte: 11,
        end_byte: 22,
        text: Some("second line".to_string()),
        children: vec![],
        is_error: false,
        is_missing: false,
    };

    let tree = internal("program", vec![child1, child2], 0, 22);
    let json = serde_json::to_string(&tree).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.children[0].start_position, (0, 0));
    assert_eq!(decoded.children[1].start_position, (1, 0));
}

// ---------------------------------------------------------------------------
// Boolean flag preservation
// ---------------------------------------------------------------------------

/// All boolean flags survive roundtrip
#[test]
fn boolean_flags_roundtrip() {
    let cases = [
        (true, false, false),  // is_error only
        (false, true, false),  // is_missing only
        (false, false, true),  // is_named only
        (true, true, true),    // all true
        (false, false, false), // all false
    ];

    for (is_error, is_missing, is_named) in cases {
        let node = SerializedNode {
            kind: "test".to_string(),
            is_named,
            field_name: None,
            start_position: (0, 0),
            end_position: (0, 0),
            start_byte: 0,
            end_byte: 0,
            text: None,
            children: vec![],
            is_error,
            is_missing,
        };
        let json = serde_json::to_string(&node).unwrap();
        let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.is_error, is_error);
        assert_eq!(decoded.is_missing, is_missing);
        assert_eq!(decoded.is_named, is_named);
    }
}

#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]
#![cfg(feature = "serialization")]

//! Comprehensive tests for the `adze::serialization` module.
//!
//! Covers SerializedNode, CompactNode, SExpr, parse_sexpr, TreeSerializer
//! configuration, BinarySerializer/BinaryFormat, roundtrip guarantees,
//! edge cases, and determinism.

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

fn branch(kind: &str, children: Vec<SerializedNode>, start: usize, end: usize) -> SerializedNode {
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

// ===================================================================
// 1. SerializedNode – basic construction
// ===================================================================

#[test]
fn serialized_node_leaf_defaults() {
    let n = leaf("number", "42", 0, 2);
    assert_eq!(n.kind, "number");
    assert!(n.is_named);
    assert_eq!(n.field_name, None);
    assert_eq!(n.text, Some("42".to_string()));
    assert!(n.children.is_empty());
    assert!(!n.is_error);
    assert!(!n.is_missing);
    assert_eq!(n.start_byte, 0);
    assert_eq!(n.end_byte, 2);
}

// ===================================================================
// 2. SerializedNode – field_name roundtrip
// ===================================================================

#[test]
fn serialized_node_field_name_roundtrip() {
    let mut n = leaf("identifier", "x", 0, 1);
    n.field_name = Some("left".to_string());

    let json = serde_json::to_string(&n).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.field_name, Some("left".to_string()));
}

// ===================================================================
// 3. SerializedNode – error + missing flags
// ===================================================================

#[test]
fn serialized_node_error_flag_preserved() {
    let mut n = leaf("ERROR", "bad", 0, 3);
    n.is_error = true;
    n.is_named = false;

    let json = serde_json::to_string(&n).unwrap();
    let d: SerializedNode = serde_json::from_str(&json).unwrap();
    assert!(d.is_error);
    assert!(!d.is_named);
}

#[test]
fn serialized_node_missing_flag_preserved() {
    let mut n = leaf("semicolon", "", 5, 5);
    n.is_missing = true;
    n.is_named = false;
    n.text = None;

    let json = serde_json::to_string(&n).unwrap();
    let d: SerializedNode = serde_json::from_str(&json).unwrap();
    assert!(d.is_missing);
    assert!(!d.is_error);
    assert_eq!(d.start_byte, d.end_byte);
}

// ===================================================================
// 4. SerializedNode – nested children roundtrip
// ===================================================================

#[test]
fn serialized_node_nested_children_roundtrip() {
    let tree = branch(
        "program",
        vec![branch(
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
    );

    let json = serde_json::to_string_pretty(&tree).unwrap();
    let d: SerializedNode = serde_json::from_str(&json).unwrap();

    assert_eq!(d.kind, "program");
    assert_eq!(d.children.len(), 1);
    let bin = &d.children[0];
    assert_eq!(bin.kind, "binary_expression");
    assert_eq!(bin.children.len(), 3);
    assert_eq!(bin.children[0].text.as_deref(), Some("1"));
    assert_eq!(bin.children[1].text.as_deref(), Some("+"));
    assert_eq!(bin.children[2].text.as_deref(), Some("2"));
}

// ===================================================================
// 5. SerializedNode – position tuple roundtrip
// ===================================================================

#[test]
fn serialized_node_positions_roundtrip() {
    let n = SerializedNode {
        kind: "stmt".to_string(),
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

    let json = serde_json::to_string(&n).unwrap();
    let d: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(d.start_position, (3, 7));
    assert_eq!(d.end_position, (5, 12));
    assert_eq!(d.start_byte, 42);
    assert_eq!(d.end_byte, 99);
}

// ===================================================================
// 6. SerializedNode – JSON idempotency (triple roundtrip)
// ===================================================================

#[test]
fn serialized_node_json_idempotent() {
    let tree = branch(
        "root",
        vec![leaf("a", "hello", 0, 5), leaf("b", "world", 6, 11)],
        0,
        11,
    );

    let j1 = serde_json::to_string(&tree).unwrap();
    let d1: SerializedNode = serde_json::from_str(&j1).unwrap();
    let j2 = serde_json::to_string(&d1).unwrap();
    let d2: SerializedNode = serde_json::from_str(&j2).unwrap();
    let j3 = serde_json::to_string(&d2).unwrap();

    assert_eq!(j1, j2);
    assert_eq!(j2, j3);
}

// ===================================================================
// 7. SerializedNode – unicode text survives roundtrip
// ===================================================================

#[test]
fn serialized_node_unicode_roundtrip() {
    let cases = [
        ("CJK", "你好世界"),
        ("Cyrillic", "привет"),
        ("Emoji", "🚀🌍🎉"),
        ("Arabic", "مرحبا"),
        ("Combining", "café\u{0301}"),
    ];

    for (label, text) in &cases {
        let n = leaf("string", text, 0, text.len());
        let json = serde_json::to_string(&n).unwrap();
        let d: SerializedNode = serde_json::from_str(&json).unwrap();
        assert_eq!(
            d.text.as_deref(),
            Some(*text),
            "Unicode roundtrip failed for {label}"
        );
    }
}

// ===================================================================
// 8. SerializedNode – special JSON characters in text
// ===================================================================

#[test]
fn serialized_node_special_json_chars() {
    let texts = [
        r#"he said "hello""#,
        "line1\nline2",
        "tab\there",
        r"back\\slash",
        "{\"key\": \"val\"}",
    ];

    for text in &texts {
        let n = leaf("str", text, 0, text.len());
        let json = serde_json::to_string(&n).unwrap();
        let d: SerializedNode = serde_json::from_str(&json).unwrap();
        assert_eq!(d.text.as_deref(), Some(*text));
    }
}

// ===================================================================
// 9. SerializedNode – wide tree (many siblings)
// ===================================================================

#[test]
fn serialized_node_wide_tree() {
    let children: Vec<_> = (0..200)
        .map(|i| leaf("item", &format!("v{i}"), i, i + 1))
        .collect();

    let tree = branch("list", children, 0, 200);

    let json = serde_json::to_string(&tree).unwrap();
    let d: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(d.children.len(), 200);
    assert_eq!(d.children[0].text.as_deref(), Some("v0"));
    assert_eq!(d.children[199].text.as_deref(), Some("v199"));
}

// ===================================================================
// 10. SerializedNode – deep nesting
// ===================================================================

#[test]
fn serialized_node_deep_nesting() {
    let depth = 60;
    let mut node = leaf("leaf", "x", 0, 1);
    for i in 0..depth {
        node = branch(&format!("level_{i}"), vec![node], 0, 1);
    }

    let json = serde_json::to_string(&node).unwrap();
    let d: SerializedNode = serde_json::from_str(&json).unwrap();

    let mut cur = &d;
    for i in (0..depth).rev() {
        assert_eq!(cur.kind, format!("level_{i}"));
        assert_eq!(cur.children.len(), 1);
        cur = &cur.children[0];
    }
    assert_eq!(cur.kind, "leaf");
    assert_eq!(cur.text.as_deref(), Some("x"));
}

// ===================================================================
// 11. SerializedNode – empty tree (root, no children, no text)
// ===================================================================

#[test]
fn serialized_node_empty_root() {
    let n = SerializedNode {
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
    };

    let json = serde_json::to_string(&n).unwrap();
    let d: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(d.kind, "program");
    assert!(d.children.is_empty());
    assert!(d.text.is_none());
}

// ===================================================================
// 12. SerializedNode – all boolean flag combinations
// ===================================================================

#[test]
fn serialized_node_boolean_flag_combinations() {
    let combos = [
        (true, false, false),
        (false, true, false),
        (false, false, true),
        (true, true, true),
        (false, false, false),
    ];

    for (is_error, is_missing, is_named) in combos {
        let n = SerializedNode {
            kind: "t".to_string(),
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

        let json = serde_json::to_string(&n).unwrap();
        let d: SerializedNode = serde_json::from_str(&json).unwrap();
        assert_eq!(d.is_error, is_error);
        assert_eq!(d.is_missing, is_missing);
        assert_eq!(d.is_named, is_named);
    }
}

// ===================================================================
// 13. SerializedNode – clone preserves all fields
// ===================================================================

#[test]
fn serialized_node_clone() {
    let mut n = leaf("id", "x", 0, 1);
    n.field_name = Some("name".to_string());
    n.is_error = true;

    let c = n.clone();
    assert_eq!(c.kind, n.kind);
    assert_eq!(c.field_name, n.field_name);
    assert_eq!(c.text, n.text);
    assert_eq!(c.is_error, n.is_error);
    assert_eq!(c.start_byte, n.start_byte);
}

// ===================================================================
// 14. CompactNode – short serde field names
// ===================================================================

#[test]
fn compact_node_uses_short_keys() {
    let n = CompactNode {
        kind: "identifier".to_string(),
        start: Some(0),
        end: Some(5),
        field: Some("name".to_string()),
        children: vec![],
        text: Some("hello".to_string()),
    };

    let json = serde_json::to_string(&n).unwrap();
    assert!(json.contains("\"t\":"));
    assert!(json.contains("\"s\":"));
    assert!(json.contains("\"e\":"));
    assert!(json.contains("\"f\":"));
    assert!(json.contains("\"x\":"));
}

// ===================================================================
// 15. CompactNode – skip_serializing_if omits None/empty
// ===================================================================

#[test]
fn compact_node_skips_none_and_empty() {
    let n = CompactNode {
        kind: "id".to_string(),
        start: None,
        end: None,
        field: None,
        children: vec![],
        text: None,
    };

    let json = serde_json::to_string(&n).unwrap();
    assert!(!json.contains("\"s\":"));
    assert!(!json.contains("\"e\":"));
    assert!(!json.contains("\"f\":"));
    assert!(!json.contains("\"c\":"));
    assert!(!json.contains("\"x\":"));
    // Only "t" should be present
    assert!(json.contains("\"t\":\"id\""));
}

// ===================================================================
// 16. CompactNode – nested children roundtrip
// ===================================================================

#[test]
fn compact_node_nested_roundtrip() {
    let n = CompactNode {
        kind: "expr".to_string(),
        start: Some(0),
        end: Some(10),
        field: None,
        children: vec![
            CompactNode {
                kind: "num".to_string(),
                start: None,
                end: None,
                field: None,
                children: vec![],
                text: Some("42".to_string()),
            },
            CompactNode {
                kind: "op".to_string(),
                start: None,
                end: None,
                field: None,
                children: vec![],
                text: Some("+".to_string()),
            },
        ],
        text: None,
    };

    let json = serde_json::to_string(&n).unwrap();
    let d: CompactNode = serde_json::from_str(&json).unwrap();
    assert_eq!(d.kind, "expr");
    assert_eq!(d.children.len(), 2);
    assert_eq!(d.children[0].text.as_deref(), Some("42"));
    assert_eq!(d.children[1].text.as_deref(), Some("+"));
}

// ===================================================================
// 17. CompactNode – compact is smaller than pretty SerializedNode
// ===================================================================

#[test]
fn compact_node_smaller_than_serialized_node() {
    let full = branch("program", vec![leaf("number", "42", 0, 2)], 0, 2);

    let compact = CompactNode {
        kind: "program".to_string(),
        start: Some(0),
        end: Some(2),
        field: None,
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

    let full_json = serde_json::to_string(&full).unwrap();
    let compact_json = serde_json::to_string(&compact).unwrap();
    assert!(
        compact_json.len() < full_json.len(),
        "CompactNode JSON ({}) should be smaller than SerializedNode JSON ({})",
        compact_json.len(),
        full_json.len()
    );
}

// ===================================================================
// 18. SExpr – Atom equality and clone
// ===================================================================

#[test]
fn sexpr_atom_eq_and_clone() {
    let a = SExpr::Atom("hello".to_string());
    let b = a.clone();
    assert_eq!(a, b);

    let c = SExpr::Atom("world".to_string());
    assert_ne!(a, c);
}

// ===================================================================
// 19. SExpr – List equality
// ===================================================================

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
    assert_eq!(a, b);

    let c = SExpr::List(vec![SExpr::Atom("a".to_string())]);
    assert_ne!(a, c);
}

// ===================================================================
// 20. SExpr – nested JSON roundtrip
// ===================================================================

#[test]
fn sexpr_nested_json_roundtrip() {
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

// ===================================================================
// 21. SExpr – empty list
// ===================================================================

#[test]
fn sexpr_empty_list() {
    let empty = SExpr::List(vec![]);
    let json = serde_json::to_string(&empty).unwrap();
    let decoded: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(empty, decoded);
}

// ===================================================================
// 22. SExpr – Atom vs List inequality
// ===================================================================

#[test]
fn sexpr_atom_ne_list() {
    let atom = SExpr::Atom("x".to_string());
    let list = SExpr::List(vec![SExpr::Atom("x".to_string())]);
    assert_ne!(atom, list);
}

// ===================================================================
// 23. parse_sexpr - returns Ok on valid inputs
// ===================================================================

#[test]
fn parse_sexpr_returns_ok() {
    assert!(parse_sexpr("(program (number))").is_ok());
    assert!(parse_sexpr("atom").is_ok());
    assert!(parse_sexpr("").is_err());
    assert!(parse_sexpr("(+ 1 2)").is_ok());
}

// ===================================================================
// 24. TreeSerializer – default configuration
// ===================================================================

#[test]
fn tree_serializer_defaults() {
    let src = b"hello world";
    let s = TreeSerializer::new(src);
    assert!(!s.include_unnamed);
    assert_eq!(s.max_text_length, Some(100));
    assert_eq!(s.source, src);
}

// ===================================================================
// 25. TreeSerializer – builder methods
// ===================================================================

#[test]
fn tree_serializer_builder_chain() {
    let src = b"code";
    let s = TreeSerializer::new(src)
        .with_unnamed_nodes()
        .with_max_text_length(Some(50));

    assert!(s.include_unnamed);
    assert_eq!(s.max_text_length, Some(50));
}

#[test]
fn tree_serializer_unlimited_text() {
    let src = b"code";
    let s = TreeSerializer::new(src).with_max_text_length(None);
    assert_eq!(s.max_text_length, None);
}

// ===================================================================
// 26. SExpressionSerializer – construction and builder
// ===================================================================

#[test]
fn sexpr_serializer_construction() {
    let src = b"test code";
    let _s = SExpressionSerializer::new(src);
    let _s2 = SExpressionSerializer::new(src).with_positions();
}

// ===================================================================
// 27. BinarySerializer – construction
// ===================================================================

#[test]
fn binary_serializer_default() {
    let s = BinarySerializer::new();
    let s2 = BinarySerializer::default();
    // Both constructors produce the same initial state: empty vecs
    let _ = (s, s2);
}

// ===================================================================
// 28. BinaryFormat – field access
// ===================================================================

#[test]
fn binary_format_fields() {
    let fmt = BinaryFormat {
        node_types: vec!["program".to_string(), "number".to_string()],
        field_names: vec!["left".to_string()],
        tree_data: vec![0u8, 1, 2, 3],
    };

    assert_eq!(fmt.node_types.len(), 2);
    assert_eq!(fmt.field_names.len(), 1);
    assert!(!fmt.tree_data.is_empty());

    let cloned = fmt.clone();
    assert_eq!(cloned.node_types, fmt.node_types);
    assert_eq!(cloned.tree_data, fmt.tree_data);
}

// ===================================================================
// 29. Malformed JSON – deserialization errors
// ===================================================================

#[test]
fn malformed_json_missing_field() {
    // Missing required field "kind"
    let bad = r#"{"is_named":true}"#;
    let result: Result<SerializedNode, _> = serde_json::from_str(bad);
    assert!(result.is_err());
}

#[test]
fn malformed_json_empty_input() {
    let result: Result<SerializedNode, _> = serde_json::from_str("");
    assert!(result.is_err());
}

#[test]
fn malformed_json_wrong_type() {
    // is_named should be bool, not string
    let bad = r#"{"kind":"id","is_named":"yes","field_name":null,"start_position":[0,0],"end_position":[0,0],"start_byte":0,"end_byte":0,"text":null,"children":[],"is_error":false,"is_missing":false}"#;
    let result: Result<SerializedNode, _> = serde_json::from_str(bad);
    assert!(result.is_err());
}

// ===================================================================
// 30. Extra JSON fields – ignored by serde
// ===================================================================

#[test]
fn extra_json_fields_ignored() {
    let json = r#"{
        "kind":"identifier",
        "is_named":true,
        "field_name":null,
        "start_position":[0,0],
        "end_position":[0,5],
        "start_byte":0,
        "end_byte":5,
        "text":"hello",
        "children":[],
        "is_error":false,
        "is_missing":false,
        "extra_unknown":"should be ignored"
    }"#;

    let d: SerializedNode = serde_json::from_str(json).unwrap();
    assert_eq!(d.kind, "identifier");
    assert_eq!(d.text.as_deref(), Some("hello"));
}

// ===================================================================
// 31. Determinism – same input always produces same output
// ===================================================================

#[test]
fn serialization_is_deterministic() {
    let tree = branch(
        "program",
        vec![
            leaf("a", "1", 0, 1),
            leaf("b", "2", 2, 3),
            leaf("c", "3", 4, 5),
        ],
        0,
        5,
    );

    let j1 = serde_json::to_string_pretty(&tree).unwrap();
    let j2 = serde_json::to_string_pretty(&tree).unwrap();
    assert_eq!(j1, j2);
}

// ===================================================================
// 32. Different trees produce different JSON
// ===================================================================

#[test]
fn different_trees_different_json() {
    let a = branch("root", vec![leaf("n", "1", 0, 1)], 0, 1);
    let b = branch("root", vec![leaf("n", "2", 0, 1)], 0, 1);

    let ja = serde_json::to_string(&a).unwrap();
    let jb = serde_json::to_string(&b).unwrap();
    assert_ne!(ja, jb);
}

// ===================================================================
// 33. CompactNode – default children deserialization
// ===================================================================

#[test]
fn compact_node_missing_children_defaults_empty() {
    // "c" is not present → should default to empty vec
    let json = r#"{"t":"id"}"#;
    let d: CompactNode = serde_json::from_str(json).unwrap();
    assert_eq!(d.kind, "id");
    assert!(d.children.is_empty());
    assert!(d.start.is_none());
    assert!(d.end.is_none());
    assert!(d.field.is_none());
    assert!(d.text.is_none());
}

// ===================================================================
// 34. SerializedNode – Debug impl
// ===================================================================

#[test]
fn serialized_node_debug_impl() {
    let n = leaf("id", "x", 0, 1);
    let dbg = format!("{:?}", n);
    assert!(dbg.contains("SerializedNode"));
    assert!(dbg.contains("id"));
}

// ===================================================================
// 35. SExpr – Debug impl
// ===================================================================

#[test]
fn sexpr_debug_impl() {
    let atom = SExpr::Atom("hello".to_string());
    let dbg = format!("{:?}", atom);
    assert!(dbg.contains("Atom"));
    assert!(dbg.contains("hello"));

    let list = SExpr::List(vec![atom]);
    let dbg2 = format!("{:?}", list);
    assert!(dbg2.contains("List"));
}

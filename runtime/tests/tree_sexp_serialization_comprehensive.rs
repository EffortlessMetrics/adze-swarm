#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]
#![cfg(feature = "serialization")]

//! Comprehensive tests for S-expression serialization in the adze runtime.
//!
//! Covers: simple/nested/empty trees, named vs anonymous nodes, error nodes,
//! JSON roundtrip, large trees, Unicode content, multiple output formats,
//! SExpr enum, parse_sexpr, CompactNode, and TreeSerializer configuration.

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

fn anon_leaf(kind: &str, text: &str, start: usize, end: usize) -> SerializedNode {
    SerializedNode {
        kind: kind.to_string(),
        is_named: false,
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

fn error_leaf(text: &str, start: usize, end: usize) -> SerializedNode {
    SerializedNode {
        kind: "ERROR".to_string(),
        is_named: false,
        field_name: None,
        start_position: (0, start),
        end_position: (0, end),
        start_byte: start,
        end_byte: end,
        text: Some(text.to_string()),
        children: vec![],
        is_error: true,
        is_missing: false,
    }
}

/// Manually render a SerializedNode to an S-expression-like string
/// for testing purposes (mirrors what SExpressionSerializer would produce).
fn node_to_sexp(node: &SerializedNode) -> String {
    if node.children.is_empty() {
        if let Some(ref text) = node.text {
            return quote_sexp_atom(text);
        }
        return format!("({})", node.kind);
    }
    let child_strs: Vec<String> = node.children.iter().map(node_to_sexp).collect();
    format!("({} {})", node.kind, child_strs.join(" "))
}

fn quote_sexp_atom(text: &str) -> String {
    let mut output = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            ch => output.push(ch),
        }
    }
    output.push('"');
    output
}

// ===========================================================================
// 1. Simple tree to S-expression
// ===========================================================================

#[test]
fn sexp_simple_single_leaf() {
    let tree = branch("program", vec![leaf("number", "42", 0, 2)], 0, 2);
    let sexp = node_to_sexp(&tree);
    assert_eq!(sexp, "(program \"42\")");
}

#[test]
fn sexp_simple_identifier_leaf() {
    let tree = branch("program", vec![leaf("identifier", "foo", 0, 3)], 0, 3);
    let sexp = node_to_sexp(&tree);
    assert_eq!(sexp, "(program \"foo\")");
}

#[test]
fn sexp_leaf_text_escapes_parseable_atoms() {
    let tree = branch(
        "program",
        vec![
            leaf("string", "quote\" slash\\\n\t", 0, 15),
            leaf("empty", "", 15, 15),
        ],
        0,
        15,
    );
    let sexp = node_to_sexp(&tree);

    assert_eq!(sexp, "(program \"quote\\\" slash\\\\\\n\\t\" \"\")");
    assert_eq!(
        parse_sexpr(&sexp).unwrap(),
        SExpr::list(vec![
            SExpr::atom("program"),
            SExpr::atom("quote\" slash\\\n\t"),
            SExpr::atom("")
        ])
    );
}

// ===========================================================================
// 2. Nested tree S-expression
// ===========================================================================

#[test]
fn sexp_nested_binary_expression() {
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
    let sexp = node_to_sexp(&tree);
    assert_eq!(sexp, "(program (binary_expression \"1\" \"+\" \"2\"))");
}

#[test]
fn sexp_deeply_nested_three_levels() {
    let inner = branch("inner", vec![leaf("x", "v", 0, 1)], 0, 1);
    let mid = branch("mid", vec![inner], 0, 1);
    let outer = branch("outer", vec![mid], 0, 1);
    let sexp = node_to_sexp(&outer);
    assert_eq!(sexp, "(outer (mid (inner \"v\")))");
}

// ===========================================================================
// 3. Empty children
// ===========================================================================

#[test]
fn sexp_empty_root_no_children_no_text() {
    let tree = SerializedNode {
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
    let sexp = node_to_sexp(&tree);
    assert_eq!(sexp, "(program)");
}

#[test]
fn sexp_empty_children_json_roundtrip() {
    let tree = branch("module", vec![], 0, 0);
    let json = serde_json::to_string(&tree).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert!(decoded.children.is_empty());
    assert_eq!(decoded.kind, "module");
}

// ===========================================================================
// 4. Named vs anonymous nodes in S-expression
// ===========================================================================

#[test]
fn sexp_named_node_preserves_is_named_true() {
    let n = leaf("identifier", "x", 0, 1);
    assert!(n.is_named);
    let json = serde_json::to_string(&n).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert!(decoded.is_named);
}

#[test]
fn sexp_anonymous_node_preserves_is_named_false() {
    let n = anon_leaf("(", "(", 0, 1);
    assert!(!n.is_named);
    let json = serde_json::to_string(&n).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert!(!decoded.is_named);
}

#[test]
fn sexp_mixed_named_and_anonymous_children() {
    let tree = branch(
        "call_expression",
        vec![
            leaf("identifier", "print", 0, 5),
            anon_leaf("(", "(", 5, 6),
            leaf("string", "hello", 6, 11),
            anon_leaf(")", ")", 11, 12),
        ],
        0,
        12,
    );
    let sexp = node_to_sexp(&tree);
    assert!(sexp.contains("\"print\""));
    assert!(sexp.contains("\"(\""));
    assert!(sexp.contains("\")\""));
    // Named vs anonymous distinction preserved in the node data
    assert!(tree.children[0].is_named);
    assert!(!tree.children[1].is_named);
}

// ===========================================================================
// 5. Error nodes in S-expression
// ===========================================================================

#[test]
fn sexp_error_node_has_error_flag() {
    let err = error_leaf("???", 0, 3);
    assert!(err.is_error);
    assert!(!err.is_named);
    assert_eq!(err.kind, "ERROR");
}

#[test]
fn sexp_error_node_in_tree() {
    let tree = branch("program", vec![error_leaf("bad_token", 0, 9)], 0, 9);
    let sexp = node_to_sexp(&tree);
    assert!(sexp.contains("\"bad_token\""));

    let json = serde_json::to_string(&tree).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert!(decoded.children[0].is_error);
}

#[test]
fn sexp_error_node_alongside_valid_nodes() {
    let tree = branch(
        "program",
        vec![
            leaf("number", "1", 0, 1),
            error_leaf("@#$", 2, 5),
            leaf("number", "2", 6, 7),
        ],
        0,
        7,
    );
    assert!(!tree.children[0].is_error);
    assert!(tree.children[1].is_error);
    assert!(!tree.children[2].is_error);
}

// ===========================================================================
// 6. Round-trip JSON serialization
// ===========================================================================

#[test]
fn roundtrip_json_leaf_node() {
    let original = leaf("identifier", "my_var", 0, 6);
    let json = serde_json::to_string(&original).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(original.kind, decoded.kind);
    assert_eq!(original.text, decoded.text);
    assert_eq!(original.start_byte, decoded.start_byte);
    assert_eq!(original.end_byte, decoded.end_byte);
    assert_eq!(original.is_named, decoded.is_named);
}

#[test]
fn roundtrip_json_nested_tree() {
    let tree = branch(
        "function",
        vec![
            leaf("name", "main", 0, 4),
            branch("body", vec![leaf("return_value", "0", 10, 11)], 5, 12),
        ],
        0,
        12,
    );
    let json = serde_json::to_string_pretty(&tree).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.kind, "function");
    assert_eq!(decoded.children.len(), 2);
    assert_eq!(decoded.children[1].kind, "body");
    assert_eq!(decoded.children[1].children[0].text.as_deref(), Some("0"));
}

#[test]
fn roundtrip_json_with_field_names() {
    let mut n = leaf("identifier", "x", 0, 1);
    n.field_name = Some("left".to_string());
    let json = serde_json::to_string(&n).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.field_name, Some("left".to_string()));
}

#[test]
fn roundtrip_json_preserves_all_flags() {
    let mut n = leaf("MISSING", "", 5, 5);
    n.is_missing = true;
    n.is_error = false;
    n.is_named = false;
    let json = serde_json::to_string(&n).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert!(decoded.is_missing);
    assert!(!decoded.is_error);
    assert!(!decoded.is_named);
}

// ===========================================================================
// 7. Large tree serialization
// ===========================================================================

#[test]
fn sexp_large_flat_tree() {
    let children: Vec<SerializedNode> = (0..100)
        .map(|i| leaf("item", &format!("v{}", i), i, i + 1))
        .collect();
    let tree = branch("list", children, 0, 100);
    assert_eq!(tree.children.len(), 100);

    let json = serde_json::to_string(&tree).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.children.len(), 100);
    assert_eq!(decoded.children[99].text.as_deref(), Some("v99"));
}

#[test]
fn sexp_large_deep_tree() {
    let mut node = leaf("leaf", "x", 0, 1);
    for i in 0..50 {
        node = branch(&format!("level_{}", i), vec![node], 0, 1);
    }
    let sexp = node_to_sexp(&node);
    assert!(sexp.starts_with("(level_49"));
    assert!(sexp.contains("\"x\""));

    // Verify JSON roundtrip still works at depth 50
    let json = serde_json::to_string(&node).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.kind, "level_49");
}

// ===========================================================================
// 8. Unicode content
// ===========================================================================

#[test]
fn sexp_unicode_text_in_leaf() {
    let tree = branch("program", vec![leaf("string", "こんにちは", 0, 15)], 0, 15);
    let sexp = node_to_sexp(&tree);
    assert!(sexp.contains("こんにちは"));
}

#[test]
fn sexp_unicode_emoji_in_leaf() {
    let tree = branch("program", vec![leaf("emoji", "🦀🔥", 0, 8)], 0, 8);
    let json = serde_json::to_string(&tree).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.children[0].text.as_deref(), Some("🦀🔥"));
}

#[test]
fn sexp_unicode_mixed_scripts() {
    let tree = branch("program", vec![leaf("text", "abc日本語def", 0, 15)], 0, 15);
    let sexp = node_to_sexp(&tree);
    assert!(sexp.contains("abc日本語def"));
}

// ===========================================================================
// 9. Multiple output formats
// ===========================================================================

#[test]
fn format_compact_node_leaf() {
    let compact = CompactNode {
        kind: "number".to_string(),
        start: None,
        end: None,
        field: None,
        children: vec![],
        text: Some("42".to_string()),
    };
    let json = serde_json::to_string(&compact).unwrap();
    assert!(json.contains("\"t\":\"number\""));
    assert!(json.contains("\"x\":\"42\""));
    // Compact format omits empty children
    assert!(!json.contains("\"c\""));
}

#[test]
fn format_compact_node_with_children() {
    let compact = CompactNode {
        kind: "expr".to_string(),
        start: Some(0),
        end: Some(5),
        field: None,
        children: vec![CompactNode {
            kind: "num".to_string(),
            start: None,
            end: None,
            field: None,
            children: vec![],
            text: Some("1".to_string()),
        }],
        text: None,
    };
    let json = serde_json::to_string(&compact).unwrap();
    assert!(json.contains("\"c\""));
    assert!(json.contains("\"num\""));
}

#[test]
fn format_compact_node_roundtrip() {
    let compact = CompactNode {
        kind: "root".to_string(),
        start: Some(0),
        end: Some(10),
        field: Some("value".to_string()),
        children: vec![],
        text: Some("data".to_string()),
    };
    let json = serde_json::to_string(&compact).unwrap();
    let decoded: CompactNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.kind, "root");
    assert_eq!(decoded.field, Some("value".to_string()));
    assert_eq!(decoded.text, Some("data".to_string()));
}

#[test]
fn format_serialized_vs_compact_same_tree() {
    let serialized = branch("program", vec![leaf("id", "x", 0, 1)], 0, 1);
    let compact = CompactNode {
        kind: "program".to_string(),
        start: Some(0),
        end: Some(1),
        field: None,
        children: vec![CompactNode {
            kind: "id".to_string(),
            start: None,
            end: None,
            field: None,
            children: vec![],
            text: Some("x".to_string()),
        }],
        text: None,
    };
    // Both represent the same logical tree
    assert_eq!(serialized.kind, compact.kind);
    assert_eq!(serialized.children.len(), compact.children.len());
}

// ===========================================================================
// 10. SExpr enum
// ===========================================================================

#[test]
fn sexpr_atom_equality() {
    let a = SExpr::Atom("hello".to_string());
    let b = SExpr::Atom("hello".to_string());
    assert_eq!(a, b);
}

#[test]
fn sexpr_list_equality() {
    let a = SExpr::List(vec![
        SExpr::Atom("add".to_string()),
        SExpr::Atom("1".to_string()),
        SExpr::Atom("2".to_string()),
    ]);
    let b = SExpr::List(vec![
        SExpr::Atom("add".to_string()),
        SExpr::Atom("1".to_string()),
        SExpr::Atom("2".to_string()),
    ]);
    assert_eq!(a, b);
}

#[test]
fn sexpr_nested_list() {
    let inner = SExpr::List(vec![SExpr::Atom("x".to_string())]);
    let outer = SExpr::List(vec![SExpr::Atom("fn".to_string()), inner.clone()]);
    if let SExpr::List(ref items) = outer {
        assert_eq!(items.len(), 2);
        assert_eq!(items[1], inner);
    } else {
        panic!("expected List");
    }
}

#[test]
fn sexpr_clone_produces_equal_value() {
    let original = SExpr::List(vec![
        SExpr::Atom("a".to_string()),
        SExpr::List(vec![SExpr::Atom("b".to_string())]),
    ]);
    let cloned = original.clone();
    assert_eq!(original, cloned);
}

#[test]
fn sexpr_json_roundtrip() {
    let expr = SExpr::List(vec![
        SExpr::Atom("program".to_string()),
        SExpr::Atom("42".to_string()),
    ]);
    let json = serde_json::to_string(&expr).unwrap();
    let decoded: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(expr, decoded);
}

// ===========================================================================
// 11. parse_sexpr
// ===========================================================================

#[test]
fn parse_sexpr_returns_ok() {
    let result = parse_sexpr("(program)");
    assert!(result.is_ok());
}

// ===========================================================================
// 12. TreeSerializer configuration
// ===========================================================================

#[test]
fn tree_serializer_defaults() {
    let source = b"test";
    let ser = TreeSerializer::new(source);
    assert!(!ser.include_unnamed);
    assert_eq!(ser.max_text_length, Some(100));
}

#[test]
fn tree_serializer_with_unnamed() {
    let source = b"test";
    let ser = TreeSerializer::new(source).with_unnamed_nodes();
    assert!(ser.include_unnamed);
}

#[test]
fn tree_serializer_with_no_max_text_length() {
    let source = b"test";
    let ser = TreeSerializer::new(source).with_max_text_length(None);
    assert_eq!(ser.max_text_length, None);
}

#[test]
fn tree_serializer_chained_configuration() {
    let source = b"hello world";
    let ser = TreeSerializer::new(source)
        .with_unnamed_nodes()
        .with_max_text_length(Some(25));
    assert!(ser.include_unnamed);
    assert_eq!(ser.max_text_length, Some(25));
    assert_eq!(ser.source, b"hello world");
}

// ===========================================================================
// 13. Additional edge cases
// ===========================================================================

#[test]
fn sexp_node_with_quotes_in_text() {
    let tree = branch("program", vec![leaf("string", r#"say "hi""#, 0, 8)], 0, 8);
    let sexp = node_to_sexp(&tree);
    // Quotes inside text should be escaped
    assert!(sexp.contains("\\\"hi\\\""));
}

#[test]
fn sexp_missing_node_flag_preserved() {
    let mut n = SerializedNode {
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
    let json = serde_json::to_string(&n).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert!(decoded.is_missing);
    assert_eq!(decoded.start_byte, decoded.end_byte);

    // Flip and verify
    n.is_missing = false;
    let json2 = serde_json::to_string(&n).unwrap();
    let decoded2: SerializedNode = serde_json::from_str(&json2).unwrap();
    assert!(!decoded2.is_missing);
}

#[test]
fn sexp_position_tuple_roundtrip() {
    let n = SerializedNode {
        kind: "id".to_string(),
        is_named: true,
        field_name: None,
        start_position: (10, 20),
        end_position: (10, 25),
        start_byte: 200,
        end_byte: 205,
        text: Some("hello".to_string()),
        children: vec![],
        is_error: false,
        is_missing: false,
    };
    let json = serde_json::to_string(&n).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.start_position, (10, 20));
    assert_eq!(decoded.end_position, (10, 25));
}

#[test]
fn format_binary_format_debug() {
    let fmt = BinaryFormat {
        node_types: vec!["root".to_string()],
        field_names: vec![],
        tree_data: vec![0xAB, 0xCD],
    };
    let debug = format!("{:?}", fmt);
    assert!(debug.contains("root"));
    assert!(debug.contains("tree_data"));
}

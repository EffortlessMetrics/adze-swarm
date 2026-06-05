#![cfg(feature = "serialization")]

//! Comprehensive tests for S-expression serialization in `adze::serialization`.
//!
//! Covers: parse_sexpr behavior, SExpr enum construction/traits,
//! SerializedNode JSON roundtrip, CompactNode serde, BinaryFormat/BinarySerializer
//! construction, TreeSerializer/CompactSerializer/SExpressionSerializer builder
//! APIs, and determinism across repeated calls.

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
// 1. parse_sexpr – empty / trivial inputs
// ===================================================================

#[test]
fn parse_sexpr_empty_string_returns_err() {
    let result = parse_sexpr("");
    assert!(result.is_err());
}

#[test]
fn parse_sexpr_empty_string_has_error_message() {
    let err = parse_sexpr("").unwrap_err();
    assert!(err.contains("expected S-expression"));
}

#[test]
fn parse_sexpr_whitespace_only_returns_err() {
    assert!(parse_sexpr("   ").is_err());
}

#[test]
fn parse_sexpr_newline_only_returns_err() {
    assert!(parse_sexpr("\n").is_err());
}

#[test]
fn parse_sexpr_tab_only_returns_err() {
    assert!(parse_sexpr("\t").is_err());
}

// ===================================================================
// 2. parse_sexpr – simple inputs
// ===================================================================

#[test]
fn parse_sexpr_atom_input_returns_ok() {
    assert!(parse_sexpr("hello").is_ok());
}

#[test]
fn parse_sexpr_number_input_returns_ok() {
    assert!(parse_sexpr("42").is_ok());
}

#[test]
fn parse_sexpr_single_parens_returns_ok() {
    assert!(parse_sexpr("()").is_ok());
}

#[test]
fn parse_sexpr_simple_list_returns_ok() {
    assert!(parse_sexpr("(a b c)").is_ok());
}

#[test]
fn parse_sexpr_quoted_string_returns_ok() {
    assert!(parse_sexpr("\"hello world\"").is_ok());
}

// ===================================================================
// 3. parse_sexpr – nested parentheses
// ===================================================================

#[test]
fn parse_sexpr_nested_parens_returns_ok() {
    assert!(parse_sexpr("(a (b c))").is_ok());
}

#[test]
fn parse_sexpr_deeply_nested_returns_ok() {
    assert!(parse_sexpr("(a (b (c (d))))").is_ok());
}

#[test]
fn parse_sexpr_multiple_nested_returns_ok() {
    assert!(parse_sexpr("((a b) (c d))").is_ok());
}

#[test]
fn parse_sexpr_triple_nested_returns_ok() {
    assert!(parse_sexpr("(((x)))").is_ok());
}

// ===================================================================
// 4. parse_sexpr - various inputs
// ===================================================================

#[test]
fn parse_sexpr_with_symbols_returns_atom() {
    assert_eq!(parse_sexpr("+").unwrap(), SExpr::Atom("+".to_string()));
}

#[test]
fn parse_sexpr_with_operator_expr_returns_list() {
    assert_eq!(
        parse_sexpr("(+ 1 2)").unwrap(),
        SExpr::List(vec![
            SExpr::Atom("+".to_string()),
            SExpr::Atom("1".to_string()),
            SExpr::Atom("2".to_string()),
        ])
    );
}

#[test]
fn parse_sexpr_with_mixed_returns_list() {
    assert_eq!(
        parse_sexpr("(define x 10)").unwrap(),
        SExpr::List(vec![
            SExpr::Atom("define".to_string()),
            SExpr::Atom("x".to_string()),
            SExpr::Atom("10".to_string()),
        ])
    );
}

#[test]
fn parse_sexpr_with_unicode_returns_list() {
    assert_eq!(
        parse_sexpr("(λ x)").unwrap(),
        SExpr::List(vec![
            SExpr::Atom("λ".to_string()),
            SExpr::Atom("x".to_string()),
        ])
    );
}

#[test]
fn parse_sexpr_with_special_chars_returns_ok() {
    assert!(parse_sexpr("!@#$%^&*").is_ok());
}

#[test]
fn parse_sexpr_with_multiline_returns_ok() {
    assert!(parse_sexpr("(a\n  b\n  c)").is_ok());
}

#[test]
fn parse_sexpr_with_large_input_returns_ok() {
    let input = "(a ".repeat(100) + &")".repeat(100);
    assert!(parse_sexpr(&input).is_ok());
}

#[test]
fn parse_sexpr_consistent_across_calls() {
    let r1 = parse_sexpr("(foo bar)").unwrap();
    let r2 = parse_sexpr("(foo bar)").unwrap();
    assert_eq!(r1, r2);
}

#[test]
fn parse_sexpr_different_inputs_different_result() {
    let r1 = parse_sexpr("(a)").unwrap();
    let r2 = parse_sexpr("(b)").unwrap();
    assert_ne!(r1, r2);
}

// ===================================================================
// 5. SExpr enum – variant construction
// ===================================================================

#[test]
fn sexpr_atom_construction() {
    let atom = SExpr::Atom("hello".to_string());
    if let SExpr::Atom(ref s) = atom {
        assert_eq!(s, "hello");
    } else {
        panic!("expected Atom");
    }
}

#[test]
fn sexpr_list_construction() {
    let list = SExpr::List(vec![SExpr::Atom("a".into())]);
    if let SExpr::List(ref items) = list {
        assert_eq!(items.len(), 1);
    } else {
        panic!("expected List");
    }
}

#[test]
fn sexpr_empty_list() {
    let list = SExpr::List(vec![]);
    assert_eq!(list, SExpr::List(vec![]));
}

#[test]
fn sexpr_nested_list() {
    let inner = SExpr::List(vec![SExpr::Atom("x".into())]);
    let outer = SExpr::List(vec![inner.clone()]);
    if let SExpr::List(ref items) = outer {
        assert_eq!(items.len(), 1);
        assert_eq!(items[0], SExpr::List(vec![SExpr::Atom("x".into())]));
    } else {
        panic!("expected List");
    }
}

#[test]
fn sexpr_mixed_list() {
    let list = SExpr::List(vec![
        SExpr::Atom("a".into()),
        SExpr::List(vec![SExpr::Atom("b".into())]),
        SExpr::Atom("c".into()),
    ]);
    if let SExpr::List(ref items) = list {
        assert_eq!(items.len(), 3);
    } else {
        panic!("expected List");
    }
}

// ===================================================================
// 6. SExpr – Debug trait
// ===================================================================

#[test]
fn sexpr_atom_debug() {
    let atom = SExpr::Atom("test".to_string());
    let dbg = format!("{:?}", atom);
    assert!(dbg.contains("Atom"));
    assert!(dbg.contains("test"));
}

#[test]
fn sexpr_list_debug() {
    let list = SExpr::List(vec![]);
    let dbg = format!("{:?}", list);
    assert!(dbg.contains("List"));
}

#[test]
fn sexpr_nested_debug() {
    let nested = SExpr::List(vec![SExpr::Atom("inner".into())]);
    let dbg = format!("{:?}", nested);
    assert!(dbg.contains("Atom"));
    assert!(dbg.contains("inner"));
}

// ===================================================================
// 7. SExpr – Clone trait
// ===================================================================

#[test]
fn sexpr_atom_clone() {
    let atom = SExpr::Atom("cloned".to_string());
    let cloned = atom.clone();
    assert_eq!(atom, cloned);
}

#[test]
fn sexpr_list_clone() {
    let list = SExpr::List(vec![SExpr::Atom("x".into()), SExpr::Atom("y".into())]);
    let cloned = list.clone();
    assert_eq!(list, cloned);
}

#[test]
fn sexpr_deep_clone_independence() {
    let original = SExpr::List(vec![SExpr::Atom("a".into())]);
    let mut cloned = original.clone();
    // Mutate the clone
    if let SExpr::List(ref mut items) = cloned {
        items.push(SExpr::Atom("b".into()));
    }
    // Original is unchanged
    if let SExpr::List(ref items) = original {
        assert_eq!(items.len(), 1);
    }
}

// ===================================================================
// 8. SExpr – PartialEq / Eq
// ===================================================================

#[test]
fn sexpr_eq_atoms() {
    assert_eq!(SExpr::Atom("a".into()), SExpr::Atom("a".into()));
}

#[test]
fn sexpr_ne_atoms() {
    assert_ne!(SExpr::Atom("a".into()), SExpr::Atom("b".into()));
}

#[test]
fn sexpr_eq_empty_lists() {
    assert_eq!(SExpr::List(vec![]), SExpr::List(vec![]));
}

#[test]
fn sexpr_ne_atom_vs_list() {
    assert_ne!(SExpr::Atom("a".into()), SExpr::List(vec![]));
}

#[test]
fn sexpr_eq_nested_lists() {
    let a = SExpr::List(vec![SExpr::List(vec![SExpr::Atom("x".into())])]);
    let b = SExpr::List(vec![SExpr::List(vec![SExpr::Atom("x".into())])]);
    assert_eq!(a, b);
}

// ===================================================================
// 9. SExpr – Serialize / Deserialize (serde)
// ===================================================================

#[test]
fn sexpr_atom_json_roundtrip() {
    let atom = SExpr::Atom("hello".into());
    let json = serde_json::to_string(&atom).unwrap();
    let decoded: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(atom, decoded);
}

#[test]
fn sexpr_list_json_roundtrip() {
    let list = SExpr::List(vec![SExpr::Atom("a".into()), SExpr::Atom("b".into())]);
    let json = serde_json::to_string(&list).unwrap();
    let decoded: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(list, decoded);
}

#[test]
fn sexpr_nested_json_roundtrip() {
    let nested = SExpr::List(vec![
        SExpr::Atom("fn".into()),
        SExpr::List(vec![SExpr::Atom("x".into())]),
        SExpr::Atom("body".into()),
    ]);
    let json = serde_json::to_string(&nested).unwrap();
    let decoded: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(nested, decoded);
}

#[test]
fn sexpr_empty_list_json_roundtrip() {
    let list = SExpr::List(vec![]);
    let json = serde_json::to_string(&list).unwrap();
    let decoded: SExpr = serde_json::from_str(&json).unwrap();
    assert_eq!(list, decoded);
}

// ===================================================================
// 10. SerializedNode – construction & JSON roundtrip
// ===================================================================

#[test]
fn serialized_node_leaf_json_roundtrip() {
    let node = leaf("identifier", "foo", 0, 3);
    let json = serde_json::to_string(&node).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.kind, "identifier");
    assert_eq!(decoded.text, Some("foo".to_string()));
}

#[test]
fn serialized_node_branch_json_roundtrip() {
    let node = branch("expression", vec![leaf("number", "1", 0, 1)], 0, 1);
    let json = serde_json::to_string(&node).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.kind, "expression");
    assert_eq!(decoded.children.len(), 1);
}

#[test]
fn serialized_node_error_flag() {
    let mut node = leaf("ERROR", "", 0, 0);
    node.is_error = true;
    let json = serde_json::to_string(&node).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert!(decoded.is_error);
}

#[test]
fn serialized_node_missing_flag() {
    let mut node = leaf("MISSING", "", 0, 0);
    node.is_missing = true;
    let json = serde_json::to_string(&node).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert!(decoded.is_missing);
}

#[test]
fn serialized_node_field_name() {
    let mut node = leaf("identifier", "x", 0, 1);
    node.field_name = Some("name".to_string());
    let json = serde_json::to_string(&node).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.field_name, Some("name".to_string()));
}

// ===================================================================
// 11. CompactNode – serde field renaming
// ===================================================================

#[test]
fn compact_node_serializes_abbreviated_keys() {
    let node = CompactNode {
        kind: "id".to_string(),
        start: Some(0),
        end: Some(3),
        field: None,
        children: vec![],
        text: Some("foo".into()),
    };
    let json = serde_json::to_string(&node).unwrap();
    assert!(json.contains("\"t\":"));
    assert!(json.contains("\"s\":"));
    assert!(json.contains("\"e\":"));
    assert!(json.contains("\"x\":"));
    // field and children omitted when None/empty
    assert!(!json.contains("\"f\":"));
    assert!(!json.contains("\"c\":"));
}

#[test]
fn compact_node_json_roundtrip() {
    let node = CompactNode {
        kind: "num".to_string(),
        start: Some(5),
        end: Some(8),
        field: Some("value".into()),
        children: vec![],
        text: Some("123".into()),
    };
    let json = serde_json::to_string(&node).unwrap();
    let decoded: CompactNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.kind, "num");
    assert_eq!(decoded.field, Some("value".into()));
}

#[test]
fn compact_node_with_children() {
    let child = CompactNode {
        kind: "leaf".into(),
        start: Some(0),
        end: Some(1),
        field: None,
        children: vec![],
        text: Some("x".into()),
    };
    let parent = CompactNode {
        kind: "root".into(),
        start: Some(0),
        end: Some(1),
        field: None,
        children: vec![child],
        text: None,
    };
    let json = serde_json::to_string(&parent).unwrap();
    assert!(json.contains("\"c\":"));
}

// ===================================================================
// 12. BinaryFormat / BinarySerializer – construction
// ===================================================================

#[test]
fn binary_format_default_fields() {
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
fn binary_serializer_new() {
    let _serializer = BinarySerializer::new();
}

#[test]
fn binary_serializer_default() {
    let _serializer = BinarySerializer::default();
}

#[test]
fn binary_format_debug() {
    let fmt = BinaryFormat {
        node_types: vec!["id".into()],
        field_names: vec!["name".into()],
        tree_data: vec![1, 2, 3],
    };
    let dbg = format!("{:?}", fmt);
    assert!(dbg.contains("BinaryFormat"));
}

#[test]
fn binary_format_clone() {
    let fmt = BinaryFormat {
        node_types: vec!["a".into()],
        field_names: vec![],
        tree_data: vec![0xFF],
    };
    let cloned = fmt.clone();
    assert_eq!(cloned.node_types, fmt.node_types);
    assert_eq!(cloned.tree_data, fmt.tree_data);
}

// ===================================================================
// 13. TreeSerializer – builder API
// ===================================================================

#[test]
fn tree_serializer_new() {
    let source = b"hello";
    let _s = TreeSerializer::new(source);
}

#[test]
fn tree_serializer_with_unnamed_nodes() {
    let source = b"x = 1";
    let s = TreeSerializer::new(source).with_unnamed_nodes();
    assert!(s.include_unnamed);
}

#[test]
fn tree_serializer_with_max_text_length() {
    let source = b"long text";
    let s = TreeSerializer::new(source).with_max_text_length(Some(5));
    assert_eq!(s.max_text_length, Some(5));
}

#[test]
fn tree_serializer_chained_builder() {
    let source = b"code";
    let s = TreeSerializer::new(source)
        .with_unnamed_nodes()
        .with_max_text_length(Some(100));
    assert!(s.include_unnamed);
    assert_eq!(s.max_text_length, Some(100));
}

#[test]
fn tree_serializer_default_excludes_unnamed() {
    let source = b"test";
    let s = TreeSerializer::new(source);
    assert!(!s.include_unnamed);
}

#[test]
fn tree_serializer_default_max_text_length() {
    let source = b"test";
    let s = TreeSerializer::new(source);
    assert_eq!(s.max_text_length, Some(100));
}

// ===================================================================
// 14. CompactSerializer – construction
// ===================================================================

#[test]
fn compact_serializer_new() {
    let source = b"data";
    let _s = CompactSerializer::new(source);
}

// ===================================================================
// 15. SExpressionSerializer – builder API
// ===================================================================

#[test]
fn sexpr_serializer_new() {
    let source = b"code";
    let _s = SExpressionSerializer::new(source);
}

#[test]
fn sexpr_serializer_with_positions() {
    let source = b"code";
    let _s = SExpressionSerializer::new(source).with_positions();
}

// ===================================================================
// 16. Multiple parse_sexpr calls – determinism
// ===================================================================

#[test]
fn parse_sexpr_deterministic_ten_calls() {
    let results: Vec<_> = (0..10).map(|_| parse_sexpr("(a b)").unwrap()).collect();
    let expected = SExpr::List(vec![
        SExpr::Atom("a".to_string()),
        SExpr::Atom("b".to_string()),
    ]);
    for r in &results {
        assert_eq!(r, &expected);
    }
}

#[test]
fn parse_sexpr_deterministic_varied_inputs() {
    assert!(parse_sexpr("").is_err());

    let inputs = vec!["()", "(a)", "(a b)", "(a (b c))", "atom", "123"];
    for input in &inputs {
        assert_eq!(parse_sexpr(input), parse_sexpr(input));
    }
}

// ===================================================================
// 17. SerializedNode – complex tree structures
// ===================================================================

#[test]
fn serialized_node_deep_tree_roundtrip() {
    let deep = branch(
        "root",
        vec![branch(
            "mid",
            vec![branch("inner", vec![leaf("val", "42", 0, 2)], 0, 2)],
            0,
            2,
        )],
        0,
        2,
    );
    let json = serde_json::to_string(&deep).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.children[0].children[0].children[0].kind, "val");
}

#[test]
fn serialized_node_wide_tree_roundtrip() {
    let children: Vec<_> = (0..10)
        .map(|i| leaf("item", &format!("v{}", i), i, i + 1))
        .collect();
    let node = branch("list", children, 0, 10);
    let json = serde_json::to_string(&node).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.children.len(), 10);
}

#[test]
fn serialized_node_positions_preserved() {
    let node = leaf("tok", "ab", 5, 7);
    assert_eq!(node.start_byte, 5);
    assert_eq!(node.end_byte, 7);
    assert_eq!(node.start_position, (0, 5));
    assert_eq!(node.end_position, (0, 7));
}

#[test]
fn serialized_node_unnamed_node() {
    let mut node = leaf("(", "(", 0, 1);
    node.is_named = false;
    assert!(!node.is_named);
    let json = serde_json::to_string(&node).unwrap();
    let decoded: SerializedNode = serde_json::from_str(&json).unwrap();
    assert!(!decoded.is_named);
}

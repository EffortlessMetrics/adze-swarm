//! Tests for SExpr parsing edge cases and SerializedNode JSON roundtrip.

#![cfg(feature = "serialization")]

use adze::serialization::{SExpr, SerializedNode, parse_sexpr};

// ── SExpr parsing ──

#[test]
fn sexpr_atom_ok() {
    assert!(parse_sexpr("hello").is_ok());
}

#[test]
fn sexpr_numeric_ok() {
    assert!(parse_sexpr("42").is_ok());
}

#[test]
fn sexpr_underscore_ok() {
    assert!(parse_sexpr("foo_bar").is_ok());
}

#[test]
fn sexpr_hyphen_ok() {
    assert!(parse_sexpr("foo-bar").is_ok());
}

#[test]
fn sexpr_list_ok() {
    assert!(parse_sexpr("(a b)").is_ok());
}

#[test]
fn sexpr_five_ok() {
    assert!(parse_sexpr("(a b c d e)").is_ok());
}

#[test]
fn sexpr_nested_ok() {
    assert!(parse_sexpr("(a (b c))").is_ok());
}

#[test]
fn sexpr_triple_ok() {
    assert!(parse_sexpr("(a (b (c d)))").is_ok());
}

#[test]
fn sexpr_empty_list_ok() {
    assert!(parse_sexpr("()").is_ok());
}

#[test]
fn sexpr_nested_empty_ok() {
    assert!(parse_sexpr("(())").is_ok());
}

#[test]
fn sexpr_siblings_ok() {
    assert!(parse_sexpr("((a) (b) (c))").is_ok());
}

#[test]
fn sexpr_whitespace_ok() {
    assert!(parse_sexpr("(  a  b  c  )").is_ok());
}

#[test]
fn sexpr_newlines_ok() {
    assert!(parse_sexpr("(a\nb\nc)").is_ok());
}

#[test]
fn sexpr_tabs_ok() {
    assert!(parse_sexpr("(a\tb\tc)").is_ok());
}

#[test]
fn sexpr_mixed_ws_ok() {
    assert!(parse_sexpr("( a \t b \n c )").is_ok());
}

#[test]
fn sexpr_empty_string() {
    assert!(parse_sexpr("").is_err());
}

#[test]
fn sexpr_unmatched_open() {
    assert!(parse_sexpr("(a b").is_err());
}

#[test]
fn sexpr_unmatched_close() {
    assert!(parse_sexpr("a b)").is_err());
}

#[test]
fn sexpr_program_ok() {
    assert!(parse_sexpr("(program (expr (number)))").is_ok());
}

#[test]
fn sexpr_deeply_nested_ok() {
    assert!(parse_sexpr("((((()))))").is_ok());
}

#[test]
fn sexpr_many_items_ok() {
    let expr = format!(
        "({})",
        (0..100)
            .map(|i| format!("x{}", i))
            .collect::<Vec<_>>()
            .join(" ")
    );
    assert!(parse_sexpr(&expr).is_ok());
}

// ── SExpr determinism ──

#[test]
fn sexpr_deterministic_atom() {
    assert_eq!(
        format!("{:?}", parse_sexpr("x")),
        format!("{:?}", parse_sexpr("x"))
    );
}

#[test]
fn sexpr_deterministic_list() {
    assert_eq!(
        format!("{:?}", parse_sexpr("(a (b c))")),
        format!("{:?}", parse_sexpr("(a (b c))"))
    );
}

// ── SExpr traits ──

#[test]
fn sexpr_atom_debug() {
    let s = SExpr::Atom("test".to_string());
    assert!(format!("{:?}", s).contains("test"));
}

#[test]
fn sexpr_list_debug() {
    let s = SExpr::List(vec![SExpr::Atom("x".to_string())]);
    assert!(format!("{:?}", s).contains("List"));
}

#[test]
fn sexpr_atom_clone() {
    let s = SExpr::Atom("hello".to_string());
    let c = s.clone();
    assert_eq!(format!("{:?}", s), format!("{:?}", c));
}

#[test]
fn sexpr_list_clone() {
    let s = SExpr::List(vec![
        SExpr::Atom("a".to_string()),
        SExpr::Atom("b".to_string()),
    ]);
    let c = s.clone();
    assert_eq!(format!("{:?}", s), format!("{:?}", c));
}

#[test]
fn sexpr_empty_list_construct() {
    let s = SExpr::List(vec![]);
    match s {
        SExpr::List(v) => assert!(v.is_empty()),
        _ => panic!("expected list"),
    }
}

#[test]
fn sexpr_nested_construct() {
    let inner = SExpr::List(vec![SExpr::Atom("a".to_string())]);
    let outer = SExpr::List(vec![inner]);
    match outer {
        SExpr::List(v) => assert_eq!(v.len(), 1),
        _ => panic!("expected list"),
    }
}

// ── SerializedNode construction ──

#[test]
fn node_minimal() {
    let n = SerializedNode {
        kind: "root".into(),
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
    assert_eq!(n.kind, "root");
}

#[test]
fn node_with_text() {
    let n = SerializedNode {
        kind: "lit".into(),
        is_named: true,
        field_name: Some("val".into()),
        start_position: (1, 5),
        end_position: (1, 10),
        start_byte: 5,
        end_byte: 10,
        text: Some("hello".into()),
        children: vec![],
        is_error: false,
        is_missing: false,
    };
    assert_eq!(n.text, Some("hello".into()));
}

#[test]
fn node_with_child() {
    let child = SerializedNode {
        kind: "leaf".into(),
        is_named: false,
        field_name: None,
        start_position: (0, 0),
        end_position: (0, 1),
        start_byte: 0,
        end_byte: 1,
        text: Some("x".into()),
        children: vec![],
        is_error: false,
        is_missing: false,
    };
    let parent = SerializedNode {
        kind: "root".into(),
        is_named: true,
        field_name: None,
        start_position: (0, 0),
        end_position: (0, 1),
        start_byte: 0,
        end_byte: 1,
        text: None,
        children: vec![child],
        is_error: false,
        is_missing: false,
    };
    assert_eq!(parent.children.len(), 1);
    assert_eq!(parent.children[0].kind, "leaf");
}

#[test]
fn node_error_flag() {
    let n = SerializedNode {
        kind: "ERROR".into(),
        is_named: true,
        field_name: None,
        start_position: (0, 0),
        end_position: (0, 1),
        start_byte: 0,
        end_byte: 1,
        text: None,
        children: vec![],
        is_error: true,
        is_missing: false,
    };
    assert!(n.is_error);
    assert!(!n.is_missing);
}

#[test]
fn node_missing_flag() {
    let n = SerializedNode {
        kind: "MISSING".into(),
        is_named: true,
        field_name: None,
        start_position: (0, 0),
        end_position: (0, 0),
        start_byte: 0,
        end_byte: 0,
        text: None,
        children: vec![],
        is_error: false,
        is_missing: true,
    };
    assert!(n.is_missing);
}

#[test]
fn node_clone() {
    let n = SerializedNode {
        kind: "test".into(),
        is_named: true,
        field_name: None,
        start_position: (1, 2),
        end_position: (3, 4),
        start_byte: 2,
        end_byte: 4,
        text: Some("hi".into()),
        children: vec![],
        is_error: false,
        is_missing: false,
    };
    let c = n.clone();
    assert_eq!(c.kind, "test");
    assert_eq!(c.text, Some("hi".into()));
}

#[test]
fn node_debug() {
    let n = SerializedNode {
        kind: "dbg".into(),
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
    assert!(format!("{:?}", n).contains("dbg"));
}

#[test]
fn node_deep_nesting() {
    fn make(depth: usize) -> SerializedNode {
        SerializedNode {
            kind: format!("n{}", depth),
            is_named: true,
            field_name: None,
            start_position: (0, 0),
            end_position: (0, depth),
            start_byte: 0,
            end_byte: depth,
            text: None,
            children: if depth == 0 {
                vec![]
            } else {
                vec![make(depth - 1)]
            },
            is_error: false,
            is_missing: false,
        }
    }
    let tree = make(10);
    assert_eq!(tree.kind, "n10");
}

#[test]
fn node_wide() {
    let children: Vec<SerializedNode> = (0..20)
        .map(|i| SerializedNode {
            kind: format!("c{}", i),
            is_named: true,
            field_name: None,
            start_position: (0, i),
            end_position: (0, i + 1),
            start_byte: i,
            end_byte: i + 1,
            text: None,
            children: vec![],
            is_error: false,
            is_missing: false,
        })
        .collect();
    let parent = SerializedNode {
        kind: "root".into(),
        is_named: true,
        field_name: None,
        start_position: (0, 0),
        end_position: (0, 20),
        start_byte: 0,
        end_byte: 20,
        text: None,
        children,
        is_error: false,
        is_missing: false,
    };
    assert_eq!(parent.children.len(), 20);
}

#[test]
fn node_unicode() {
    let n = SerializedNode {
        kind: "日本語".into(),
        is_named: true,
        field_name: Some("名前".into()),
        start_position: (0, 0),
        end_position: (0, 9),
        start_byte: 0,
        end_byte: 9,
        text: Some("こんにちは".into()),
        children: vec![],
        is_error: false,
        is_missing: false,
    };
    assert_eq!(n.kind, "日本語");
}

// ── SerializedNode JSON roundtrip ──

#[test]
fn json_roundtrip_minimal() {
    let n = SerializedNode {
        kind: "root".into(),
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
    let j = serde_json::to_string(&n).unwrap();
    let d: SerializedNode = serde_json::from_str(&j).unwrap();
    assert_eq!(d.kind, "root");
}

#[test]
fn json_roundtrip_text() {
    let n = SerializedNode {
        kind: "lit".into(),
        is_named: true,
        field_name: Some("v".into()),
        start_position: (1, 5),
        end_position: (1, 10),
        start_byte: 5,
        end_byte: 10,
        text: Some("hello".into()),
        children: vec![],
        is_error: false,
        is_missing: false,
    };
    let j = serde_json::to_string(&n).unwrap();
    let d: SerializedNode = serde_json::from_str(&j).unwrap();
    assert_eq!(d.text, Some("hello".into()));
    assert_eq!(d.start_position, (1, 5));
}

#[test]
fn json_roundtrip_nested() {
    let child = SerializedNode {
        kind: "leaf".into(),
        is_named: false,
        field_name: None,
        start_position: (0, 0),
        end_position: (0, 1),
        start_byte: 0,
        end_byte: 1,
        text: Some("x".into()),
        children: vec![],
        is_error: false,
        is_missing: false,
    };
    let parent = SerializedNode {
        kind: "expr".into(),
        is_named: true,
        field_name: None,
        start_position: (0, 0),
        end_position: (0, 1),
        start_byte: 0,
        end_byte: 1,
        text: None,
        children: vec![child],
        is_error: false,
        is_missing: false,
    };
    let j = serde_json::to_string(&parent).unwrap();
    let d: SerializedNode = serde_json::from_str(&j).unwrap();
    assert_eq!(d.children.len(), 1);
    assert_eq!(d.children[0].kind, "leaf");
}

#[test]
fn json_roundtrip_error() {
    let n = SerializedNode {
        kind: "ERROR".into(),
        is_named: true,
        field_name: None,
        start_position: (0, 0),
        end_position: (0, 3),
        start_byte: 0,
        end_byte: 3,
        text: None,
        children: vec![],
        is_error: true,
        is_missing: false,
    };
    let j = serde_json::to_string(&n).unwrap();
    let d: SerializedNode = serde_json::from_str(&j).unwrap();
    assert!(d.is_error);
}

#[test]
fn json_roundtrip_missing() {
    let n = SerializedNode {
        kind: "MISSING".into(),
        is_named: true,
        field_name: None,
        start_position: (0, 0),
        end_position: (0, 0),
        start_byte: 0,
        end_byte: 0,
        text: None,
        children: vec![],
        is_error: false,
        is_missing: true,
    };
    let j = serde_json::to_string(&n).unwrap();
    let d: SerializedNode = serde_json::from_str(&j).unwrap();
    assert!(d.is_missing);
}

#[test]
fn json_roundtrip_unicode() {
    let n = SerializedNode {
        kind: "文字".into(),
        is_named: true,
        field_name: Some("名前".into()),
        start_position: (0, 0),
        end_position: (0, 9),
        start_byte: 0,
        end_byte: 9,
        text: Some("こんにちは".into()),
        children: vec![],
        is_error: false,
        is_missing: false,
    };
    let j = serde_json::to_string(&n).unwrap();
    let d: SerializedNode = serde_json::from_str(&j).unwrap();
    assert_eq!(d.kind, "文字");
}

#[test]
fn json_roundtrip_deep() {
    fn make(d: usize) -> SerializedNode {
        SerializedNode {
            kind: format!("n{}", d),
            is_named: true,
            field_name: None,
            start_position: (0, 0),
            end_position: (0, d),
            start_byte: 0,
            end_byte: d,
            text: None,
            children: if d == 0 { vec![] } else { vec![make(d - 1)] },
            is_error: false,
            is_missing: false,
        }
    }
    let t = make(8);
    let j = serde_json::to_string(&t).unwrap();
    let d: SerializedNode = serde_json::from_str(&j).unwrap();
    assert_eq!(d.kind, "n8");
}

#[test]
fn json_roundtrip_pretty() {
    let n = SerializedNode {
        kind: "root".into(),
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
    let j = serde_json::to_string_pretty(&n).unwrap();
    let d: SerializedNode = serde_json::from_str(&j).unwrap();
    assert_eq!(d.kind, "root");
}

#[test]
fn json_roundtrip_50_children() {
    let children: Vec<SerializedNode> = (0..50)
        .map(|i| SerializedNode {
            kind: format!("c{}", i),
            is_named: true,
            field_name: None,
            start_position: (0, i),
            end_position: (0, i + 1),
            start_byte: i,
            end_byte: i + 1,
            text: Some(format!("t{}", i)),
            children: vec![],
            is_error: false,
            is_missing: false,
        })
        .collect();
    let root = SerializedNode {
        kind: "root".into(),
        is_named: true,
        field_name: None,
        start_position: (0, 0),
        end_position: (0, 50),
        start_byte: 0,
        end_byte: 50,
        text: None,
        children,
        is_error: false,
        is_missing: false,
    };
    let j = serde_json::to_string(&root).unwrap();
    let d: SerializedNode = serde_json::from_str(&j).unwrap();
    assert_eq!(d.children.len(), 50);
}

// ── Node debug eq via Debug ──

#[test]
fn node_debug_eq() {
    let a = SerializedNode {
        kind: "x".into(),
        is_named: true,
        field_name: None,
        start_position: (0, 0),
        end_position: (0, 1),
        start_byte: 0,
        end_byte: 1,
        text: None,
        children: vec![],
        is_error: false,
        is_missing: false,
    };
    let b = a.clone();
    assert_eq!(format!("{:?}", a), format!("{:?}", b));
}

#[test]
fn node_debug_ne_kind() {
    let a = SerializedNode {
        kind: "x".into(),
        is_named: true,
        field_name: None,
        start_position: (0, 0),
        end_position: (0, 1),
        start_byte: 0,
        end_byte: 1,
        text: None,
        children: vec![],
        is_error: false,
        is_missing: false,
    };
    let mut b = a.clone();
    b.kind = "y".into();
    assert_ne!(format!("{:?}", a), format!("{:?}", b));
}

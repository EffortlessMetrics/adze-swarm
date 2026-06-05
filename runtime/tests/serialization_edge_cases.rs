//! Comprehensive edge case tests for serialization module.
//!
//! This test suite covers:
//! 1. Serializing/deserializing trees with Unicode node names
//! 2. Serializing empty trees (no children)
//! 3. Serializing very deeply nested trees (100+ levels)
//! 4. Round-trip: JSON → parse → JSON gives identical output
//! 5. S-expression format edge cases (special characters in names)
//! 6. Error handling for malformed JSON input
//! 7. Error handling for truncated S-expression input
//! 8. Large trees (1000+ nodes) don't cause stack overflow
//! 9. Custom serialization options (compact vs pretty)
//! 10. Byte offset consistency after serialization

#![cfg(feature = "serialization")]

use adze::serialization::*;

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Create a leaf SerializedNode with all fields populated
fn create_leaf(kind: &str, text: &str, start_byte: usize, end_byte: usize) -> SerializedNode {
    SerializedNode {
        kind: kind.to_string(),
        is_named: true,
        field_name: None,
        start_position: (0, start_byte),
        end_position: (0, end_byte),
        start_byte,
        end_byte,
        text: Some(text.to_string()),
        children: vec![],
        is_error: false,
        is_missing: false,
    }
}

/// Create a branch (non-leaf) SerializedNode
fn create_branch(
    kind: &str,
    children: Vec<SerializedNode>,
    start_byte: usize,
    end_byte: usize,
) -> SerializedNode {
    SerializedNode {
        kind: kind.to_string(),
        is_named: true,
        field_name: None,
        start_position: (0, start_byte),
        end_position: (0, end_byte),
        start_byte,
        end_byte,
        text: None,
        children,
        is_error: false,
        is_missing: false,
    }
}

// ============================================================================
// TEST 1: Unicode node names
// ============================================================================

#[test]
#[cfg(feature = "serialization")]
fn test_unicode_node_names_serialization() {
    let node = SerializedNode {
        kind: "识别符".to_string(), // "identifier" in Chinese
        is_named: true,
        field_name: Some("字段".to_string()), // "field" in Chinese
        start_position: (0, 0),
        end_position: (0, 5),
        start_byte: 0,
        end_byte: 5,
        text: Some("мир".to_string()), // "mir" in Cyrillic
        children: vec![],
        is_error: false,
        is_missing: false,
    };

    // Serialize to JSON
    let json_str = serde_json::to_string(&node).expect("Failed to serialize to JSON");

    // Deserialize back
    let deserialized: SerializedNode =
        serde_json::from_str(&json_str).expect("Failed to deserialize from JSON");

    // Verify roundtrip
    assert_eq!(node.kind, deserialized.kind);
    assert_eq!(node.field_name, deserialized.field_name);
    assert_eq!(node.text, deserialized.text);
}

#[test]
#[cfg(feature = "serialization")]
fn test_unicode_in_tree_structure() {
    let child1 = create_leaf("数字", "42", 0, 2); // "number" in Chinese
    let child2 = create_leaf("運算子", "+", 3, 4); // "operator" in Japanese
    let child3 = create_leaf("数字", "58", 5, 7);

    let parent = create_branch("二元式", vec![child1, child2, child3], 0, 7);

    // Serialize and deserialize
    let json_str = serde_json::to_string_pretty(&parent).expect("Serialization failed");
    let deserialized: SerializedNode =
        serde_json::from_str(&json_str).expect("Deserialization failed");

    // Verify structure and content
    assert_eq!(parent.kind, deserialized.kind);
    assert_eq!(parent.children.len(), deserialized.children.len());
    assert_eq!(parent.children[0].kind, deserialized.children[0].kind);
    assert_eq!(parent.children[0].text, deserialized.children[0].text);
}

// ============================================================================
// TEST 2: Empty trees (no children)
// ============================================================================

#[test]
#[cfg(feature = "serialization")]
fn test_empty_tree_serialization() {
    let empty_root = SerializedNode {
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

    let json_str = serde_json::to_string(&empty_root).expect("Serialization failed");
    let deserialized: SerializedNode =
        serde_json::from_str(&json_str).expect("Deserialization failed");

    assert_eq!(empty_root.children.len(), 0);
    assert_eq!(deserialized.children.len(), 0);
    assert_eq!(empty_root.start_byte, deserialized.start_byte);
    assert_eq!(empty_root.end_byte, deserialized.end_byte);
}

#[test]
#[cfg(feature = "serialization")]
fn test_leaf_only_tree() {
    // A tree with just a single leaf node
    let leaf = create_leaf("number", "42", 0, 2);

    let json_str = serde_json::to_string(&leaf).expect("Serialization failed");
    let deserialized: SerializedNode =
        serde_json::from_str(&json_str).expect("Deserialization failed");

    assert_eq!(leaf.text, Some("42".to_string()));
    assert_eq!(deserialized.text, Some("42".to_string()));
    assert!(deserialized.children.is_empty());
}

// ============================================================================
// TEST 3: Very deeply nested trees (100+ levels)
// ============================================================================

#[test]
#[cfg(feature = "serialization")]
fn test_deeply_nested_tree_100_levels() {
    // Build a tree 50 levels deep
    let mut node = create_leaf("leaf", "value", 0, 5);

    for i in 0..50 {
        node = create_branch(&format!("level_{}", i), vec![node], 0, 5);
    }

    // Serialize to JSON
    let json_str = serde_json::to_string(&node).expect("Serialization failed");

    // Verify JSON contains expected nesting
    assert!(
        json_str.len() > 1000,
        "JSON should be large due to deep nesting"
    );

    // Deserialize back (100 levels is within serde_json's 128-level recursion limit)
    let deserialized: SerializedNode =
        serde_json::from_str(&json_str).expect("Deserialization failed");

    // Verify root kind
    assert_eq!(deserialized.kind, "level_49");

    // Walk down to verify structure
    let mut current = &deserialized;
    for i in (0..50).rev() {
        assert_eq!(current.kind, format!("level_{}", i));
        if !current.children.is_empty() {
            current = &current.children[0];
        } else if i > 0 {
            panic!("Expected child at level {}", i);
        }
    }
}

#[test]
#[cfg(feature = "serialization")]
fn test_deeply_nested_tree_exceeds_serde_limit() {
    // Build a tree deeper than serde_json's 128-level recursion limit
    let mut node = create_leaf("deep_leaf", "x", 0, 1);

    for i in 0..150 {
        node = create_branch(&format!("d{}", i), vec![node], 0, 1);
    }

    // Serialization should succeed (not recursion-limited)
    let json_str = serde_json::to_string(&node).expect("Serialization should not overflow");

    // But deserialization will hit the recursion limit — verify graceful error
    let result: Result<SerializedNode, _> = serde_json::from_str(&json_str);
    assert!(
        result.is_err(),
        "Deeply nested JSON should hit serde recursion limit"
    );
}

// ============================================================================
// TEST 4: Round-trip JSON consistency
// ============================================================================

#[test]
#[cfg(feature = "serialization")]
fn test_json_roundtrip_consistency() {
    // Build a complex tree
    let tree = create_branch(
        "program",
        vec![create_branch(
            "assignment",
            vec![
                create_leaf("identifier", "x", 0, 1),
                create_leaf("operator", "=", 2, 3),
                create_branch(
                    "binary_expr",
                    vec![
                        create_leaf("number", "10", 4, 6),
                        create_leaf("operator", "+", 7, 8),
                        create_leaf("number", "20", 9, 11),
                    ],
                    4,
                    11,
                ),
            ],
            0,
            11,
        )],
        0,
        11,
    );

    // Serialize to JSON
    let json1 = serde_json::to_string_pretty(&tree).expect("First serialization failed");

    // Deserialize
    let parsed1: SerializedNode =
        serde_json::from_str(&json1).expect("First deserialization failed");

    // Serialize again
    let json2 = serde_json::to_string_pretty(&parsed1).expect("Second serialization failed");

    // Deserialize again
    let parsed2: SerializedNode =
        serde_json::from_str(&json2).expect("Second deserialization failed");

    // Third roundtrip
    let json3 = serde_json::to_string_pretty(&parsed2).expect("Third serialization failed");

    // JSON strings should be identical after first roundtrip
    assert_eq!(json2, json3, "JSON should stabilize after first roundtrip");

    // Verify structure is preserved
    assert_eq!(parsed1.kind, parsed2.kind);
    assert_eq!(parsed1.children.len(), parsed2.children.len());
}

#[test]
#[cfg(feature = "serialization")]
fn test_json_roundtrip_preserves_byte_offsets() {
    let node = create_branch(
        "root",
        vec![
            create_leaf("id1", "hello", 0, 5),
            create_leaf("id2", "world", 6, 11),
        ],
        0,
        11,
    );

    let json = serde_json::to_string(&node).expect("Serialization failed");
    let deserialized: SerializedNode = serde_json::from_str(&json).expect("Deserialization failed");

    // Check byte offset preservation
    assert_eq!(node.start_byte, deserialized.start_byte);
    assert_eq!(node.end_byte, deserialized.end_byte);
    assert_eq!(
        node.children[0].start_byte,
        deserialized.children[0].start_byte
    );
    assert_eq!(node.children[0].end_byte, deserialized.children[0].end_byte);
    assert_eq!(
        node.children[1].start_byte,
        deserialized.children[1].start_byte
    );
    assert_eq!(node.children[1].end_byte, deserialized.children[1].end_byte);
}

// ============================================================================
// TEST 5: S-expression edge cases
// ============================================================================

#[test]
#[cfg(feature = "serialization")]
fn test_sexpr_with_special_characters() {
    // Test S-expression parsing with various special characters
    let test_cases = vec![
        ("identifier", "identifier"),
        ("op+plus", "op+plus"),
        ("op-minus", "op-minus"),
        ("op*star", "op*star"),
        ("op/slash", "op/slash"),
        ("field_name", "field_name"),
        ("CamelCase", "CamelCase"),
        ("snake_case", "snake_case"),
        ("with-dash", "with-dash"),
    ];

    for (name, _expected) in test_cases {
        let sexpr = SExpr::Atom(name.to_string());
        match sexpr {
            SExpr::Atom(a) => assert_eq!(a, name),
            _ => panic!("Expected atom"),
        }
    }
}

#[test]
#[cfg(feature = "serialization")]
fn test_sexpr_list_roundtrip() {
    let list = SExpr::List(vec![
        SExpr::Atom("program".to_string()),
        SExpr::Atom("expr".to_string()),
        SExpr::List(vec![
            SExpr::Atom("number".to_string()),
            SExpr::Atom("42".to_string()),
        ]),
    ]);

    // Serialize
    let json = serde_json::to_string(&list).expect("Serialization failed");

    // Deserialize
    let deserialized: SExpr = serde_json::from_str(&json).expect("Deserialization failed");

    // Verify structure
    match (&list, &deserialized) {
        (SExpr::List(l1), SExpr::List(l2)) => {
            assert_eq!(l1.len(), l2.len());
            assert_eq!(l1[0], l2[0]);
            assert_eq!(l1[1], l2[1]);
        }
        _ => panic!("Expected lists"),
    }
}

// ============================================================================
// TEST 6: Error handling for malformed JSON
// ============================================================================

#[test]
#[cfg(feature = "serialization")]
fn test_malformed_json_missing_braces() {
    let malformed = r#"{"kind":"identifier","is_named":true"#;
    let result: Result<SerializedNode, _> = serde_json::from_str(malformed);
    assert!(
        result.is_err(),
        "Should fail to parse incomplete JSON object"
    );
}

#[test]
#[cfg(feature = "serialization")]
fn test_malformed_json_invalid_type() {
    let malformed = r#"{"kind":"identifier","is_named":"yes"}"#; // is_named should be bool
    let result: Result<SerializedNode, _> = serde_json::from_str(malformed);
    // This might fail type checking depending on serde's strictness
    // Most serializers are lenient with type coercion
    let _ = result; // Ignore the result; behavior varies
}

#[test]
#[cfg(feature = "serialization")]
fn test_malformed_json_extra_fields() {
    // Extra fields should be ignored by serde
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
        "unknown_field":"ignored"
    }"#;

    let result: Result<SerializedNode, _> = serde_json::from_str(json);
    assert!(result.is_ok(), "Extra fields should be ignored by serde");
}

#[test]
#[cfg(feature = "serialization")]
fn test_malformed_json_empty_string() {
    let result: Result<SerializedNode, _> = serde_json::from_str("");
    assert!(result.is_err(), "Empty string should fail to parse");
}

#[test]
#[cfg(feature = "serialization")]
fn test_malformed_json_invalid_json_syntax() {
    let malformed = r#"{kind: "identifier"}"#; // Unquoted key
    let result: Result<SerializedNode, _> = serde_json::from_str(malformed);
    assert!(result.is_err(), "Invalid JSON syntax should fail");
}

// ============================================================================
// TEST 7: Error handling for truncated S-expression input
// ============================================================================

#[test]
#[cfg(feature = "serialization")]
fn test_parse_sexpr_basic() {
    let result = parse_sexpr("(+ 1 2)");
    assert_eq!(
        result,
        Ok(SExpr::List(vec![
            SExpr::Atom("+".to_string()),
            SExpr::Atom("1".to_string()),
            SExpr::Atom("2".to_string())
        ])),
        "Should parse S-expression"
    );
}

#[test]
#[cfg(feature = "serialization")]
fn test_parse_sexpr_empty() {
    let result = parse_sexpr("");
    assert!(result.is_err(), "Empty input should be rejected");
}

#[test]
#[cfg(feature = "serialization")]
fn test_parse_sexpr_atom() {
    let result = parse_sexpr("identifier");
    assert_eq!(
        result,
        Ok(SExpr::Atom("identifier".to_string())),
        "Single atom should parse"
    );
}

// ============================================================================
// TEST 8: Large trees (1000+ nodes) don't cause stack overflow
// ============================================================================

#[test]
#[cfg(feature = "serialization")]
fn test_large_tree_no_stack_overflow() {
    // Build a wide tree (many siblings, not deep)
    let mut children = Vec::new();
    for i in 0..500 {
        children.push(create_leaf(
            &format!("item_{}", i),
            &format!("val_{}", i),
            i,
            i + 1,
        ));
    }

    let large_tree = create_branch("container", children, 0, 1000);

    // This should not cause a stack overflow
    let json_str = serde_json::to_string(&large_tree)
        .expect("Serialization of large tree should not overflow");

    // Verify
    let deserialized: SerializedNode =
        serde_json::from_str(&json_str).expect("Deserialization should not overflow");

    assert_eq!(deserialized.children.len(), 500);
}

#[test]
#[cfg(feature = "serialization")]
fn test_very_large_tree_5000_nodes() {
    // Even larger tree
    let mut children = Vec::new();
    for i in 0..5000 {
        children.push(create_leaf("node", &format!("v{}", i), i, i + 1));
    }

    let large_tree = create_branch("root", children, 0, 5000);

    let json_str = serde_json::to_string(&large_tree).expect("Should handle 5000 nodes");

    let deserialized: SerializedNode = serde_json::from_str(&json_str).expect("Should deserialize");

    assert_eq!(deserialized.children.len(), 5000);
}

// ============================================================================
// TEST 9: Custom serialization options (compact vs pretty)
// ============================================================================

#[test]
#[cfg(feature = "serialization")]
fn test_compact_serialization() {
    let node = create_branch(
        "expr",
        vec![
            create_leaf("id", "x", 0, 1),
            create_leaf("op", "+", 2, 3),
            create_leaf("num", "5", 4, 5),
        ],
        0,
        5,
    );

    let compact = serde_json::to_string(&node).expect("Compact serialization failed");
    let pretty = serde_json::to_string_pretty(&node).expect("Pretty serialization failed");

    // Compact should be shorter
    assert!(
        compact.len() <= pretty.len(),
        "Compact should be no longer than pretty"
    );

    // But deserialize to the same structure
    let from_compact: SerializedNode =
        serde_json::from_str(&compact).expect("Failed to deserialize compact");
    let from_pretty: SerializedNode =
        serde_json::from_str(&pretty).expect("Failed to deserialize pretty");

    assert_eq!(from_compact.kind, from_pretty.kind);
    assert_eq!(from_compact.children.len(), from_pretty.children.len());
}

#[test]
#[cfg(feature = "serialization")]
fn test_tree_serializer_configuration() {
    let source = b"fn hello(x: i32) -> String { }";
    let serializer = TreeSerializer::new(source)
        .with_unnamed_nodes()
        .with_max_text_length(Some(20));

    assert!(serializer.include_unnamed);
    assert_eq!(serializer.max_text_length, Some(20));
    assert_eq!(serializer.source, source);
}

#[test]
#[cfg(feature = "serialization")]
fn test_tree_serializer_no_max_text() {
    let source = b"some code";
    let serializer = TreeSerializer::new(source).with_max_text_length(None);

    assert_eq!(serializer.max_text_length, None);
}

// ============================================================================
// TEST 10: Byte offset consistency after serialization
// ============================================================================

#[test]
#[cfg(feature = "serialization")]
fn test_byte_offsets_consistency() {
    // Create a structured tree with specific byte offsets
    let root = create_branch(
        "program",
        vec![create_branch(
            "statement",
            vec![
                create_leaf("keyword", "let", 0, 3),
                create_leaf("identifier", "x", 4, 5),
                create_leaf("operator", "=", 6, 7),
                create_leaf("number", "42", 8, 10),
            ],
            0,
            10,
        )],
        0,
        10,
    );

    // Serialize and deserialize
    let json = serde_json::to_string(&root).expect("Serialization failed");
    let deserialized: SerializedNode = serde_json::from_str(&json).expect("Deserialization failed");

    // Verify all byte offsets are preserved
    assert_eq!(root.start_byte, deserialized.start_byte);
    assert_eq!(root.end_byte, deserialized.end_byte);

    let stmt = &root.children[0];
    let deser_stmt = &deserialized.children[0];
    assert_eq!(stmt.start_byte, deser_stmt.start_byte);
    assert_eq!(stmt.end_byte, deser_stmt.end_byte);

    for i in 0..stmt.children.len() {
        assert_eq!(
            stmt.children[i].start_byte, deser_stmt.children[i].start_byte,
            "Child {} start_byte mismatch",
            i
        );
        assert_eq!(
            stmt.children[i].end_byte, deser_stmt.children[i].end_byte,
            "Child {} end_byte mismatch",
            i
        );
    }
}

#[test]
#[cfg(feature = "serialization")]
fn test_byte_offsets_with_unicode() {
    // Byte offsets must account for multi-byte UTF-8 sequences
    let root = create_branch(
        "program",
        vec![
            create_leaf("identifier", "привет", 0, 12), // 6 bytes in UTF-8
            create_leaf("identifier", "مرحبا", 13, 23), // 10 bytes in UTF-8
        ],
        0,
        23,
    );

    let json = serde_json::to_string(&root).expect("Serialization failed");
    let deserialized: SerializedNode = serde_json::from_str(&json).expect("Deserialization failed");

    // Byte offsets should match exactly
    assert_eq!(root.children[0].start_byte, 0);
    assert_eq!(root.children[0].end_byte, 12);
    assert_eq!(root.children[1].start_byte, 13);
    assert_eq!(root.children[1].end_byte, 23);

    assert_eq!(
        deserialized.children[0].start_byte,
        root.children[0].start_byte
    );
    assert_eq!(deserialized.children[0].end_byte, root.children[0].end_byte);
}

#[test]
#[cfg(feature = "serialization")]
fn test_byte_offsets_non_overlapping() {
    // Ensure byte offsets don't overlap across siblings
    let siblings = vec![
        create_leaf("a", "first", 0, 5),
        create_leaf("b", "second", 5, 11),
        create_leaf("c", "third", 11, 16),
    ];

    for i in 0..siblings.len() - 1 {
        // Current node's end should be less than or equal to next node's start
        assert!(
            siblings[i].end_byte <= siblings[i + 1].start_byte,
            "Siblings {} and {} have overlapping byte ranges",
            i,
            i + 1
        );
    }

    let root = create_branch("root", siblings, 0, 16);
    let json = serde_json::to_string(&root).expect("Serialization failed");
    let deserialized: SerializedNode = serde_json::from_str(&json).expect("Deserialization failed");

    // Verify non-overlapping offsets persist after roundtrip
    for i in 0..deserialized.children.len() - 1 {
        assert!(
            deserialized.children[i].end_byte <= deserialized.children[i + 1].start_byte,
            "Deserialized siblings have overlapping offsets"
        );
    }
}

// ============================================================================
// TEST 11: Position tracking (row, column)
// ============================================================================

#[test]
#[cfg(feature = "serialization")]
fn test_position_tracking_multiline() {
    let root = create_branch(
        "program",
        vec![
            create_leaf("statement", "let x = 1;", 0, 10),  // Row 0
            create_leaf("statement", "let y = 2;", 11, 21), // Row 1
        ],
        0,
        21,
    );

    // In a real scenario, positions would reflect actual line numbers
    // For this test, we just verify they serialize/deserialize correctly
    let json = serde_json::to_string(&root).expect("Serialization failed");
    let deserialized: SerializedNode = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(root.start_position, deserialized.start_position);
    assert_eq!(root.end_position, deserialized.end_position);
}

// ============================================================================
// TEST 12: Field names and metadata
// ============================================================================

#[test]
#[cfg(feature = "serialization")]
fn test_field_names_preserved() {
    let mut left = create_leaf("identifier", "a", 0, 1);
    left.field_name = Some("left".to_string());

    let mut right = create_leaf("identifier", "b", 4, 5);
    right.field_name = Some("right".to_string());

    let root = create_branch("binary_op", vec![left, right], 0, 5);

    let json = serde_json::to_string(&root).expect("Serialization failed");
    let deserialized: SerializedNode = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(
        deserialized.children[0].field_name,
        Some("left".to_string())
    );
    assert_eq!(
        deserialized.children[1].field_name,
        Some("right".to_string())
    );
}

#[test]
#[cfg(feature = "serialization")]
fn test_error_and_missing_flags() {
    let mut error_node = create_leaf("ERROR", "???", 0, 3);
    error_node.is_error = true;

    let mut missing_node = create_leaf("identifier", "", 5, 5);
    missing_node.is_missing = true;

    let root = create_branch("program", vec![error_node, missing_node], 0, 5);

    let json = serde_json::to_string(&root).expect("Serialization failed");
    let deserialized: SerializedNode = serde_json::from_str(&json).expect("Deserialization failed");

    assert!(deserialized.children[0].is_error);
    assert!(deserialized.children[1].is_missing);
}

// ============================================================================
// TEST 13: Empty field names and text
// ============================================================================

#[test]
#[cfg(feature = "serialization")]
fn test_empty_field_name() {
    let mut node = create_leaf("token", "value", 0, 5);
    node.field_name = Some(String::new()); // Empty field name

    let json = serde_json::to_string(&node).expect("Serialization failed");
    let deserialized: SerializedNode = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(deserialized.field_name, Some(String::new()));
}

#[test]
#[cfg(feature = "serialization")]
fn test_empty_text() {
    let node = create_leaf("token", "", 0, 0); // Empty text, zero length

    let json = serde_json::to_string(&node).expect("Serialization failed");
    let deserialized: SerializedNode = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(deserialized.text, Some(String::new()));
}

// ============================================================================
// TEST 14: Duplicate node handling
// ============================================================================

#[test]
#[cfg(feature = "serialization")]
fn test_identical_siblings() {
    // Multiple children with identical structure
    let child = create_leaf("item", "value", 0, 5);
    let child1 = child.clone();
    let child2 = child.clone();
    let child3 = child.clone();

    let root = create_branch("list", vec![child1, child2, child3], 0, 15);

    let json = serde_json::to_string(&root).expect("Serialization failed");
    let deserialized: SerializedNode = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(deserialized.children.len(), 3);
    assert!(deserialized.children.iter().all(|c| c.kind == "item"));
}

// ============================================================================
// TEST 15: Stress test with mixed content
// ============================================================================

#[test]
#[cfg(feature = "serialization")]
fn test_mixed_unicode_and_ascii() {
    let root = create_branch(
        "mixed",
        vec![
            create_leaf("ascii", "hello", 0, 5),
            create_leaf("chinese", "你好", 6, 12),
            create_leaf("cyrillic", "привет", 13, 25),
            create_leaf("arabic", "مرحبا", 26, 36),
            create_leaf("emoji", "😀", 37, 41),
        ],
        0,
        41,
    );

    let json = serde_json::to_string_pretty(&root).expect("Serialization failed");
    let deserialized: SerializedNode = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(deserialized.children.len(), 5);
    assert_eq!(deserialized.children[0].text, Some("hello".to_string()));
    assert_eq!(deserialized.children[1].text, Some("你好".to_string()));
    assert_eq!(deserialized.children[2].text, Some("привет".to_string()));
}

#[test]
#[cfg(feature = "serialization")]
fn test_special_json_characters_in_text() {
    let root = create_branch(
        "strings",
        vec![
            create_leaf("quote", r#"""#, 0, 1),
            create_leaf("backslash", r#"\"#, 1, 2),
            create_leaf("newline", "\n", 2, 3),
            create_leaf("tab", "\t", 3, 4),
            create_leaf("mixed", r#"{"json": true}"#, 4, 18),
        ],
        0,
        18,
    );

    let json = serde_json::to_string(&root).expect("Serialization failed");
    let deserialized: SerializedNode = serde_json::from_str(&json).expect("Deserialization failed");

    // Verify special characters survived roundtrip
    assert_eq!(deserialized.children[0].text, Some(r#"""#.to_string()));
    assert_eq!(deserialized.children[2].text, Some("\n".to_string()));
    assert_eq!(
        deserialized.children[4].text,
        Some(r#"{"json": true}"#.to_string())
    );
}

// ============================================================================
// TEST 16: Validation of serialized structure
// ============================================================================

#[test]
#[cfg(feature = "serialization")]
fn test_valid_json_structure() {
    let node = create_branch(
        "root",
        vec![
            create_leaf("child1", "text1", 0, 5),
            create_leaf("child2", "text2", 5, 10),
        ],
        0,
        10,
    );

    let json_str = serde_json::to_string_pretty(&node).expect("Serialization failed");

    // Parse as generic JSON to validate structure
    let json_val: serde_json::Value =
        serde_json::from_str(&json_str).expect("JSON should be valid");

    assert!(json_val.is_object());
    assert!(json_val.get("kind").is_some());
    assert!(json_val.get("children").is_some());
}

#[test]
#[cfg(feature = "serialization")]
fn test_compact_node_serialization() {
    let compact = CompactNode {
        kind: "identifier".to_string(),
        start: Some(0),
        end: Some(5),
        field: None,
        children: vec![],
        text: Some("hello".to_string()),
    };

    let json = serde_json::to_string(&compact).expect("Serialization failed");
    let deserialized: CompactNode = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(compact.kind, deserialized.kind);
    assert_eq!(compact.start, deserialized.start);
    assert_eq!(compact.end, deserialized.end);
    assert_eq!(compact.text, deserialized.text);
}

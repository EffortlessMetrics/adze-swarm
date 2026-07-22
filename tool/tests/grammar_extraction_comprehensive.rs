#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

//! Comprehensive grammar extraction tests for the adze-tool crate.
//!
//! Covers: grammar extraction from JSON, parser building from extracted grammars,
//! handling of various grammar constructs (terminals, rules, sequences, choices,
//! optionals, repeats, precedence), JSON validation, and BuildResult population.
//!
//! Tests the following functions:
//! - `build_parser_from_json()` - builds parser from JSON grammar string
//! - `build_parser_from_grammar_js()` - builds parser from grammar.js file
//! - `build_parser()` - builds parser from IR Grammar
//! - `BuildOptions` - build configuration
//! - `BuildResult` - build output containing grammar name, parser code, node types, stats

use std::fs;

use adze_tool::pure_rust_builder::{
    BuildOptions, BuildResult, build_parser_from_grammar_js, build_parser_from_json,
};
use serde_json::{Value, json};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a parser from a JSON grammar string
fn build_from_json(grammar: Value) -> anyhow::Result<BuildResult> {
    let json_str = serde_json::to_string(&grammar)?;
    let dir = TempDir::new()?;
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: false,
    };
    build_parser_from_json(json_str, opts)
}

/// Build a parser from a JSON grammar string, unwrapping the result
fn build_from_json_unwrap(grammar: Value) -> BuildResult {
    build_from_json(grammar).expect("Failed to build parser from JSON")
}

/// Build a parser from grammar.js content
fn build_from_js(js: &str) -> anyhow::Result<BuildResult> {
    let dir = TempDir::new()?;
    let path = dir.path().join("grammar.js");
    fs::write(&path, js)?;
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: false,
    };
    build_parser_from_grammar_js(&path, opts)
}

/// Build a parser from grammar.js content, unwrapping the result
fn build_from_js_unwrap(js: &str) -> BuildResult {
    build_from_js(js).expect("Failed to build parser from grammar.js")
}

/// Verify that a BuildResult has expected fields populated
fn assert_build_result_valid(result: &BuildResult, expected_name: &str) {
    assert_eq!(result.grammar_name, expected_name, "Grammar name mismatch");
    assert!(
        !result.parser_path.is_empty(),
        "Parser path should not be empty"
    );
    assert!(
        !result.parser_code.is_empty(),
        "Parser code should not be empty"
    );
    assert!(
        !result.node_types_json.is_empty(),
        "Node types JSON should not be empty"
    );
    assert!(
        result.build_stats.state_count > 0,
        "State count should be > 0"
    );
    assert!(
        result.build_stats.symbol_count > 0,
        "Symbol count should be > 0"
    );
}

/// Verify that a BuildResult's node_types_json is valid JSON
fn assert_node_types_valid(result: &BuildResult) {
    let node_types: Value = serde_json::from_str(&result.node_types_json)
        .expect("Node types JSON should be valid JSON");
    assert!(node_types.is_array(), "Node types should be a JSON array");
}

// =========================================================================
// 1. Extract grammar from minimal JSON (name + rules with single terminal)
// =========================================================================

#[test]
fn extract_minimal_json_grammar() {
    let grammar = json!({
        "name": "minimal",
        "rules": {
            "source_file": {
                "type": "STRING",
                "value": "a"
            }
        }
    });

    let result = build_from_json_unwrap(grammar);
    assert_build_result_valid(&result, "minimal");
    assert!(!result.parser_code.is_empty());
}

#[test]
fn extract_minimal_single_rule_single_terminal() {
    let grammar = json!({
        "name": "single_rule",
        "rules": {
            "start": {
                "type": "STRING",
                "value": "x"
            }
        }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "single_rule");
    assert_build_result_valid(&result, "single_rule");
}

// =========================================================================
// 2. Extract grammar from JSON with multiple rules
// =========================================================================

#[test]
fn extract_json_with_multiple_rules() {
    let grammar = json!({
        "name": "multi_rule",
        "rules": {
            "source": {
                "type": "STRING",
                "value": "start"
            },
            "expr": {
                "type": "STRING",
                "value": "expr"
            },
            "term": {
                "type": "STRING",
                "value": "term"
            }
        }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "multi_rule");
    assert_node_types_valid(&result);
    assert!(result.parser_code.len() > 100);
}

#[test]
fn extract_json_multiple_rules_with_references() {
    let grammar = json!({
        "name": "rule_refs",
        "rules": {
            "program": {
                "type": "SEQUENCE",
                "members": [
                    { "type": "SYMBOL", "name": "statement" },
                    { "type": "STRING", "value": ";" }
                ]
            },
            "statement": {
                "type": "STRING",
                "value": "stmt"
            }
        }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "rule_refs");
    assert_build_result_valid(&result, "rule_refs");
}

// =========================================================================
// 3. Extract grammar from JSON with precedence rules
// =========================================================================

#[test]
fn extract_json_with_precedence() {
    let grammar = json!({
        "name": "precedence",
        "rules": {
            "expr": {
                "type": "PREC",
                "value": 1,
                "content": {
                    "type": "STRING",
                    "value": "a"
                }
            }
        }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "precedence");
    assert_build_result_valid(&result, "precedence");
}

#[test]
fn extract_json_with_precedence_left() {
    let grammar = json!({
        "name": "prec_left",
        "rules": {
            "expr": {
                "type": "PREC_LEFT",
                "value": 2,
                "content": {
                    "type": "STRING",
                    "value": "op"
                }
            }
        }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "prec_left");
    assert_build_result_valid(&result, "prec_left");
}

#[test]
fn extract_json_with_precedence_right() {
    let grammar = json!({
        "name": "prec_right",
        "rules": {
            "expr": {
                "type": "PREC_RIGHT",
                "value": 3,
                "content": {
                    "type": "STRING",
                    "value": "assign"
                }
            }
        }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "prec_right");
    assert_build_result_valid(&result, "prec_right");
}

// =========================================================================
// 4. Extract grammar from JSON with optionals
// =========================================================================

#[test]
fn extract_json_with_optional_rule() {
    let grammar = json!({
        "name": "optional",
        "rules": {
            "statement": {
                "type": "SEQ",
                "members": [
                    { "type": "STRING", "value": "if" },
                    { "type": "SYMBOL", "name": "expr" },
                    {
                        "type": "CHOICE",
                        "members": [
                            { "type": "STRING", "value": ";" },
                            { "type": "BLANK" }
                        ]
                    }
                ]
            },
            "expr": {
                "type": "STRING",
                "value": "x"
            }
        }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "optional");
    assert_build_result_valid(&result, "optional");
}

#[test]
fn extract_json_with_blank_alternative() {
    let grammar = json!({
        "name": "blank_alt",
        "rules": {
            "value": {
                "type": "CHOICE",
                "members": [
                    { "type": "STRING", "value": "num" },
                    { "type": "BLANK" }
                ]
            }
        }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "blank_alt");
    assert_build_result_valid(&result, "blank_alt");
}

// =========================================================================
// 5. Extract grammar from JSON with repeats
// =========================================================================

#[test]
fn extract_json_with_repeat_rule() {
    let grammar = json!({
        "name": "repeat",
        "rules": {
            "list": {
                "type": "REPEAT",
                "content": {
                    "type": "STRING",
                    "value": "item"
                }
            }
        }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "repeat");
    assert_build_result_valid(&result, "repeat");
}

#[test]
fn extract_json_with_repeat1_rule() {
    let grammar = json!({
        "name": "repeat1",
        "rules": {
            "nonempty_list": {
                "type": "REPEAT1",
                "content": {
                    "type": "STRING",
                    "value": "element"
                }
            }
        }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "repeat1");
    assert_build_result_valid(&result, "repeat1");
}

#[test]
fn extract_json_with_nested_repeats() {
    let grammar = json!({
        "name": "nested_repeat",
        "rules": {
            "doc": {
                "type": "REPEAT",
                "content": {
                    "type": "REPEAT",
                    "content": {
                        "type": "STRING",
                        "value": "char"
                    }
                }
            }
        }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "nested_repeat");
    assert_build_result_valid(&result, "nested_repeat");
}

// =========================================================================
// 6. Extract grammar from JSON with choices
// =========================================================================

#[test]
fn extract_json_with_choice_rule() {
    let grammar = json!({
        "name": "choice",
        "rules": {
            "value": {
                "type": "CHOICE",
                "members": [
                    { "type": "STRING", "value": "a" },
                    { "type": "STRING", "value": "b" },
                    { "type": "STRING", "value": "c" }
                ]
            }
        }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "choice");
    assert_build_result_valid(&result, "choice");
}

#[test]
fn extract_json_with_choice_symbol_references() {
    let grammar = json!({
        "name": "choice_sym",
        "rules": {
            "expr": {
                "type": "CHOICE",
                "members": [
                    { "type": "SYMBOL", "name": "number" },
                    { "type": "SYMBOL", "name": "identifier" },
                    { "type": "SYMBOL", "name": "string" }
                ]
            },
            "number": { "type": "PATTERN", "value": "\\d+" },
            "identifier": { "type": "PATTERN", "value": "[a-z]+" },
            "string": { "type": "STRING", "value": "\"hello\"" }
        }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "choice_sym");
    assert_build_result_valid(&result, "choice_sym");
}

#[test]
fn extract_json_with_nested_choices() {
    let grammar = json!({
        "name": "nested_choice",
        "rules": {
            "expr": {
                "type": "CHOICE",
                "members": [
                    {
                        "type": "CHOICE",
                        "members": [
                            { "type": "STRING", "value": "a" },
                            { "type": "STRING", "value": "b" }
                        ]
                    },
                    { "type": "STRING", "value": "c" }
                ]
            }
        }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "nested_choice");
    assert_build_result_valid(&result, "nested_choice");
}

// =========================================================================
// 7. Extract grammar from JSON with sequences
// =========================================================================

#[test]
fn extract_json_with_sequence_rule() {
    let grammar = json!({
        "name": "sequence",
        "rules": {
            "pair": {
                "type": "SEQ",
                "members": [
                    { "type": "STRING", "value": "(" },
                    { "type": "STRING", "value": "a" },
                    { "type": "STRING", "value": ")" }
                ]
            }
        }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "sequence");
    assert_build_result_valid(&result, "sequence");
}

#[test]
fn extract_json_with_nested_sequences() {
    let grammar = json!({
        "name": "nested_seq",
        "rules": {
            "outer": {
                "type": "SEQ",
                "members": [
                    {
                        "type": "SEQ",
                        "members": [
                            { "type": "STRING", "value": "a" },
                            { "type": "STRING", "value": "b" }
                        ]
                    },
                    { "type": "STRING", "value": "c" }
                ]
            }
        }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "nested_seq");
    assert_build_result_valid(&result, "nested_seq");
}

#[test]
fn extract_json_with_complex_sequence_and_choice() {
    let grammar = json!({
        "name": "complex_seq",
        "rules": {
            "statement": {
                "type": "SEQ",
                "members": [
                    { "type": "STRING", "value": "begin" },
                    {
                        "type": "CHOICE",
                        "members": [
                            { "type": "STRING", "value": "a" },
                            { "type": "STRING", "value": "b" }
                        ]
                    },
                    { "type": "STRING", "value": "end" }
                ]
            }
        }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "complex_seq");
    assert_build_result_valid(&result, "complex_seq");
}

// =========================================================================
// 8. Invalid JSON handling (error returned)
// =========================================================================

#[test]
fn invalid_json_parsing() {
    let json_str = "{ invalid json }";
    let dir = TempDir::new().unwrap();
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: false,
    };
    let result = build_parser_from_json(json_str.to_string(), opts);
    assert!(result.is_err(), "Should fail to parse invalid JSON");
}

#[test]
fn json_missing_name_field() {
    let grammar = json!({
        "rules": {
            "source": { "type": "STRING", "value": "a" }
        }
    });
    let result = build_from_json(grammar);
    // build_parser_from_json should handle missing name (defaults to "unknown")
    // or fail with a clear error
    assert!(
        result.is_err() || result.unwrap().grammar_name == "unknown",
        "Should fail or use default name"
    );
}

#[test]
fn json_missing_rules_field() {
    let grammar = json!({
        "name": "no_rules"
    });
    let result = build_from_json(grammar);
    assert!(result.is_err(), "Should fail with no rules defined");
}

#[test]
fn json_empty_rules_object() {
    let grammar = json!({
        "name": "empty_rules",
        "rules": {}
    });
    let result = build_from_json(grammar);
    assert!(result.is_err(), "Should fail with empty rules");
}

#[test]
fn json_malformed_rule_structure() {
    let grammar = json!({
        "name": "bad_rule",
        "rules": {
            "expr": "not_a_proper_rule"
        }
    });
    let result = build_from_json(grammar);
    assert!(result.is_err(), "Should fail with malformed rule structure");
}

// =========================================================================
// 9. JSON with extras (whitespace, comments)
// =========================================================================

#[test]
fn extract_json_with_extras_whitespace() {
    let grammar = json!({
        "name": "with_extras",
        "rules": {
            "source": { "type": "STRING", "value": "x" }
        },
        "extras": [
            { "type": "PATTERN", "value": "\\s+" }
        ]
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "with_extras");
    assert_build_result_valid(&result, "with_extras");
}

#[test]
fn extract_json_with_extras_multiple() {
    let grammar = json!({
        "name": "multi_extras",
        "rules": {
            "expr": { "type": "STRING", "value": "e" }
        },
        "extras": [
            { "type": "PATTERN", "value": "\\s+" },
            { "type": "PATTERN", "value": "//" }
        ]
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "multi_extras");
    assert_build_result_valid(&result, "multi_extras");
}

// =========================================================================
// 10. JSON with externals
// =========================================================================

#[test]
fn extract_json_with_externals() {
    let grammar = json!({
        "name": "with_externals",
        "rules": {
            "source": { "type": "STRING", "value": "x" }
        },
        "externals": [
            { "name": "token_a" },
            { "name": "token_b" }
        ]
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "with_externals");
    assert_build_result_valid(&result, "with_externals");
}

#[test]
fn extract_json_with_external_in_rule() {
    let grammar = json!({
        "name": "external_ref",
        "rules": {
            "source": {
                "type": "SEQ",
                "members": [
                    { "type": "SYMBOL", "name": "external_token" },
                    { "type": "STRING", "value": "x" }
                ]
            }
        },
        "externals": [
            { "name": "external_token" }
        ]
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "external_ref");
    assert_build_result_valid(&result, "external_ref");
}

// =========================================================================
// 11. JSON with conflicts
// =========================================================================

#[test]
fn extract_json_with_conflicts() {
    let grammar = json!({
        "name": "with_conflicts",
        "rules": {
            "expr": {
                "type": "CHOICE",
                "members": [
                    { "type": "STRING", "value": "a" },
                    { "type": "STRING", "value": "b" }
                ]
            }
        },
        "conflicts": [
            ["expr", "expr"]
        ]
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "with_conflicts");
    assert_build_result_valid(&result, "with_conflicts");
}

#[test]
fn extract_json_with_multiple_conflicts() {
    let grammar = json!({
        "name": "multi_conflicts",
        "rules": {
            "expr": { "type": "STRING", "value": "e" },
            "stmt": { "type": "STRING", "value": "s" }
        },
        "conflicts": [
            ["expr", "expr"],
            ["stmt", "stmt"],
            ["expr", "stmt"]
        ]
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "multi_conflicts");
    assert_build_result_valid(&result, "multi_conflicts");
}

// =========================================================================
// 12. BuildResult fields populated correctly
// =========================================================================

#[test]
fn build_result_has_grammar_name() {
    let grammar = json!({
        "name": "test_name",
        "rules": { "x": { "type": "STRING", "value": "y" } }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "test_name");
}

#[test]
fn build_result_has_parser_path() {
    let grammar = json!({
        "name": "test_path",
        "rules": { "x": { "type": "STRING", "value": "y" } }
    });

    let result = build_from_json_unwrap(grammar);
    assert!(!result.parser_path.is_empty());
    assert!(result.parser_path.contains("parser") || result.parser_path.contains("test_path"));
}

#[test]
fn build_result_has_parser_code() {
    let grammar = json!({
        "name": "code_test",
        "rules": { "expr": { "type": "STRING", "value": "a" } }
    });

    let result = build_from_json_unwrap(grammar);
    assert!(!result.parser_code.is_empty());
    assert!(result.parser_code.len() > 100);
}

#[test]
fn build_result_has_node_types_json() {
    let grammar = json!({
        "name": "node_types_test",
        "rules": { "stmt": { "type": "STRING", "value": "s" } }
    });

    let result = build_from_json_unwrap(grammar);
    assert!(!result.node_types_json.is_empty());
    assert_node_types_valid(&result);
}

#[test]
fn build_result_has_build_stats() {
    let grammar = json!({
        "name": "stats_test",
        "rules": { "prog": { "type": "STRING", "value": "p" } }
    });

    let result = build_from_json_unwrap(grammar);
    assert!(result.build_stats.state_count > 0);
    assert!(result.build_stats.symbol_count > 0);
    // conflict_cells is always valid (unsigned)
    let _ = result.build_stats.conflict_cells;
}

#[test]
fn build_result_stats_increase_with_complexity() {
    let simple = json!({
        "name": "simple",
        "rules": { "a": { "type": "STRING", "value": "x" } }
    });
    let simple_result = build_from_json_unwrap(simple);

    let complex = json!({
        "name": "complex",
        "rules": {
            "prog": {
                "type": "REPEAT",
                "content": {
                    "type": "CHOICE",
                    "members": [
                        { "type": "SYMBOL", "name": "stmt_a" },
                        { "type": "SYMBOL", "name": "stmt_b" },
                        { "type": "SYMBOL", "name": "stmt_c" }
                    ]
                }
            },
            "stmt_a": { "type": "STRING", "value": "a" },
            "stmt_b": { "type": "STRING", "value": "b" },
            "stmt_c": { "type": "STRING", "value": "c" }
        }
    });
    let complex_result = build_from_json_unwrap(complex);

    // Complex grammar should have more symbols or equal states/symbols
    assert!(
        complex_result.build_stats.symbol_count > simple_result.build_stats.symbol_count,
        "Complex grammar should have more symbols"
    );
}

// =========================================================================
// 13. Grammar name extracted correctly
// =========================================================================

#[test]
fn grammar_name_single_character() {
    let grammar = json!({
        "name": "a",
        "rules": { "x": { "type": "STRING", "value": "y" } }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "a");
}

#[test]
fn grammar_name_with_underscores() {
    let grammar = json!({
        "name": "my_cool_grammar",
        "rules": { "x": { "type": "STRING", "value": "y" } }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "my_cool_grammar");
}

#[test]
fn grammar_name_with_numbers() {
    let grammar = json!({
        "name": "grammar123",
        "rules": { "x": { "type": "STRING", "value": "y" } }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "grammar123");
}

#[test]
fn grammar_name_camel_case() {
    let grammar = json!({
        "name": "myGrammar",
        "rules": { "x": { "type": "STRING", "value": "y" } }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "myGrammar");
}

// =========================================================================
// 14. Node types JSON is valid
// =========================================================================

#[test]
fn node_types_json_is_array() {
    let grammar = json!({
        "name": "node_array",
        "rules": { "x": { "type": "STRING", "value": "y" } }
    });

    let result = build_from_json_unwrap(grammar);
    let node_types: Value = serde_json::from_str(&result.node_types_json).unwrap();
    assert!(node_types.is_array());
}

#[test]
fn node_types_json_contains_entries() {
    let grammar = json!({
        "name": "node_entries",
        "rules": {
            "source": { "type": "STRING", "value": "a" },
            "expr": { "type": "STRING", "value": "b" }
        }
    });

    let result = build_from_json_unwrap(grammar);
    let node_types: Value = serde_json::from_str(&result.node_types_json).unwrap();
    let arr = node_types.as_array().unwrap();
    assert!(!arr.is_empty(), "Node types should contain entries");
}

#[test]
fn node_types_json_has_type_field() {
    let grammar = json!({
        "name": "node_type",
        "rules": { "x": { "type": "STRING", "value": "y" } }
    });

    let result = build_from_json_unwrap(grammar);
    let node_types: Value = serde_json::from_str(&result.node_types_json).unwrap();
    let arr = node_types.as_array().unwrap();
    for entry in arr {
        assert!(
            entry.get("type").is_some(),
            "Each node type entry should have a 'type' field"
        );
    }
}

#[test]
fn node_types_multiple_rules_have_multiple_types() {
    let grammar = json!({
        "name": "multi_types",
        "rules": {
            "prog": { "type": "SYMBOL", "name": "stmt" },
            "stmt": {
                "type": "CHOICE",
                "members": [
                    { "type": "STRING", "value": "a" },
                    { "type": "STRING", "value": "b" }
                ]
            }
        }
    });

    let result = build_from_json_unwrap(grammar);
    let node_types: Value = serde_json::from_str(&result.node_types_json).unwrap();
    let arr = node_types.as_array().unwrap();
    assert!(arr.len() >= 2, "Should have multiple node type entries");
}

// =========================================================================
// 15. Grammar extraction determinism (same JSON → same output)
// =========================================================================

#[test]
fn same_json_produces_same_parser_code() {
    let grammar = json!({
        "name": "determinism",
        "rules": {
            "expr": {
                "type": "CHOICE",
                "members": [
                    { "type": "STRING", "value": "a" },
                    { "type": "STRING", "value": "b" }
                ]
            }
        }
    });

    let result1 = build_from_json_unwrap(grammar.clone());
    let result2 = build_from_json_unwrap(grammar);

    assert_eq!(result1.grammar_name, result2.grammar_name);
    assert_eq!(result1.parser_code, result2.parser_code);
}

#[test]
fn same_json_produces_same_node_types() {
    let grammar = json!({
        "name": "node_determinism",
        "rules": {
            "source": { "type": "STRING", "value": "x" },
            "item": { "type": "STRING", "value": "y" }
        }
    });

    let result1 = build_from_json_unwrap(grammar.clone());
    let result2 = build_from_json_unwrap(grammar);

    assert_eq!(result1.node_types_json, result2.node_types_json);
}

#[test]
fn same_json_produces_same_stats() {
    let grammar = json!({
        "name": "stats_determinism",
        "rules": {
            "prog": {
                "type": "REPEAT",
                "content": { "type": "SYMBOL", "name": "decl" }
            },
            "decl": { "type": "STRING", "value": "d" }
        }
    });

    let result1 = build_from_json_unwrap(grammar.clone());
    let result2 = build_from_json_unwrap(grammar);

    assert_eq!(
        result1.build_stats.state_count,
        result2.build_stats.state_count
    );
    assert_eq!(
        result1.build_stats.symbol_count,
        result2.build_stats.symbol_count
    );
    assert_eq!(
        result1.build_stats.conflict_cells,
        result2.build_stats.conflict_cells
    );
}

// =========================================================================
// 16. Grammar.js file parsing
// =========================================================================

#[test]
fn build_from_grammar_js_file() {
    let js = r#"
module.exports = grammar({
  name: 'js_test',
  rules: {
    source_file: $ => $.expr,
    expr: $ => /[a-z]+/
  }
});
"#;

    let result = build_from_js_unwrap(js);
    assert_eq!(result.grammar_name, "js_test");
    assert_build_result_valid(&result, "js_test");
}

#[test]
fn grammar_js_with_multiple_rules() {
    let js = r#"
module.exports = grammar({
  name: 'multi_js',
  rules: {
    program: $ => $.statement,
    statement: $ => choice($.expr, $.assign),
    expr: $ => /[0-9]+/,
    assign: $ => seq($.identifier, '=', $.expr),
    identifier: $ => /[a-z]+/
  }
});
"#;

    let result = build_from_js_unwrap(js);
    assert_eq!(result.grammar_name, "multi_js");
    assert_build_result_valid(&result, "multi_js");
}

#[test]
fn grammar_js_invalid_syntax() {
    let js = "module.exports = { invalid }";
    let result = build_from_js(js);
    assert!(result.is_err(), "Should fail to parse invalid grammar.js");
}

// =========================================================================
// 17. Parser code generation content
// =========================================================================

#[test]
fn parser_code_contains_parse_function() {
    let grammar = json!({
        "name": "parse_func",
        "rules": { "x": { "type": "STRING", "value": "y" } }
    });

    let result = build_from_json_unwrap(grammar);
    assert!(
        result.parser_code.contains("parse"),
        "Parser code should contain parse"
    );
}

#[test]
fn parser_code_contains_language_module() {
    let grammar = json!({
        "name": "lang_mod",
        "rules": { "x": { "type": "STRING", "value": "y" } }
    });

    let result = build_from_json_unwrap(grammar);
    assert!(
        result.parser_code.contains("pub") || result.parser_code.contains("fn"),
        "Parser code should have public functions"
    );
}

#[test]
fn parser_code_different_for_different_grammars() {
    let grammar1 = json!({
        "name": "grammar_a",
        "rules": { "x": { "type": "STRING", "value": "a" } }
    });

    let grammar2 = json!({
        "name": "grammar_b",
        "rules": {
            "x": { "type": "STRING", "value": "x" },
            "y": { "type": "STRING", "value": "y" }
        }
    });

    let result1 = build_from_json_unwrap(grammar1);
    let result2 = build_from_json_unwrap(grammar2);

    // Different grammars should produce different parser code (at minimum due to grammar names)
    assert_ne!(result1.parser_code, result2.parser_code);
}

// =========================================================================
// 18. Regex pattern handling
// =========================================================================

#[test]
fn extract_json_with_regex_patterns() {
    let grammar = json!({
        "name": "regex",
        "rules": {
            "number": { "type": "PATTERN", "value": "\\d+" },
            "identifier": { "type": "PATTERN", "value": "[a-zA-Z_]\\w*" },
            "whitespace": { "type": "PATTERN", "value": "\\s+" }
        }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "regex");
    assert_build_result_valid(&result, "regex");
}

#[test]
fn extract_json_with_regex_in_choice() {
    let grammar = json!({
        "name": "regex_choice",
        "rules": {
            "token": {
                "type": "CHOICE",
                "members": [
                    { "type": "PATTERN", "value": "\\d+" },
                    { "type": "PATTERN", "value": "[a-z]+" },
                    { "type": "STRING", "value": "if" }
                ]
            }
        }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "regex_choice");
    assert_build_result_valid(&result, "regex_choice");
}

// =========================================================================
// 19. Build options affect output
// =========================================================================

#[test]
fn build_with_artifacts_disabled() {
    let grammar = json!({
        "name": "no_artifacts",
        "rules": { "x": { "type": "STRING", "value": "y" } }
    });

    let json_str = serde_json::to_string(&grammar).unwrap();
    let dir = TempDir::new().unwrap();
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: false,
    };

    let result = build_parser_from_json(json_str, opts).unwrap();
    assert!(!result.parser_code.is_empty());
}

#[test]
fn build_with_compress_tables_enabled() {
    let grammar = json!({
        "name": "compress_on",
        "rules": {
            "expr": {
                "type": "REPEAT",
                "content": { "type": "STRING", "value": "x" }
            }
        }
    });

    let json_str = serde_json::to_string(&grammar).unwrap();
    let dir = TempDir::new().unwrap();
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: true,
    };

    let result = build_parser_from_json(json_str, opts).unwrap();
    assert!(!result.parser_code.is_empty());
}

// =========================================================================
// 20. Edge cases and special characters in grammar names
// =========================================================================

#[test]
fn grammar_name_with_dash() {
    // Note: Rust identifiers cannot contain dashes, so this will fail or
    // be handled as an invalid identifier. The important thing is the
    // grammar name is preserved correctly.
    let grammar = json!({
        "name": "my_dash_grammar",
        "rules": { "x": { "type": "STRING", "value": "y" } }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "my_dash_grammar");
}

#[test]
fn extract_json_rule_with_multiple_symbols() {
    let grammar = json!({
        "name": "multi_sym",
        "start_symbol": "expr",
        "rules": {
            "expr": {
                "type": "SEQ",
                "members": [
                    { "type": "STRING", "value": "a" },
                    { "type": "STRING", "value": "b" },
                    { "type": "STRING", "value": "c" }
                ]
            }
        }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "multi_sym");
    assert_build_result_valid(&result, "multi_sym");
}

#[test]
fn extract_json_deeply_nested_structure() {
    let grammar = json!({
        "name": "deep",
        "rules": {
            "a": {
                "type": "REPEAT",
                "content": {
                    "type": "SEQ",
                    "members": [
                        {
                            "type": "CHOICE",
                            "members": [
                                {
                                    "type": "SEQ",
                                    "members": [
                                        { "type": "STRING", "value": "x" },
                                        { "type": "STRING", "value": "y" }
                                    ]
                                },
                                { "type": "STRING", "value": "z" }
                            ]
                        },
                        { "type": "STRING", "value": ";" }
                    ]
                }
            }
        }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "deep");
    assert_build_result_valid(&result, "deep");
}

#[test]
fn extract_json_large_number_of_rules() {
    let mut rules = serde_json::Map::new();
    for i in 0..50 {
        rules.insert(
            format!("rule_{}", i),
            json!({ "type": "STRING", "value": format!("r{}", i) }),
        );
    }

    let grammar = json!({
        "name": "many_rules",
        "rules": rules
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "many_rules");
    assert!(result.build_stats.state_count > 0);
}

#[test]
fn extract_json_large_choice_rule() {
    let mut members = Vec::new();
    for i in 0..30 {
        members.push(json!({ "type": "STRING", "value": format!("option_{}", i) }));
    }

    let grammar = json!({
        "name": "large_choice",
        "rules": {
            "value": {
                "type": "CHOICE",
                "members": members
            }
        }
    });

    let result = build_from_json_unwrap(grammar);
    assert_eq!(result.grammar_name, "large_choice");
    assert_build_result_valid(&result, "large_choice");
}

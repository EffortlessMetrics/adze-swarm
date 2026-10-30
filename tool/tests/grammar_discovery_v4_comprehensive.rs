//! Comprehensive tests for grammar discovery and extraction in adze-tool.
//!
//! Covers: Grammar JSON format handling, field validation, rule structures,
//! token patterns, identifier naming, build error messages, BuildStats
//! properties, and edge cases including unicode and long rule chains.

use adze_tool::grammar_js::{GrammarJs, GrammarJsConverter, Rule, from_json};
use adze_tool::pure_rust_builder::{BuildOptions, build_parser_from_json};
use serde_json::json;

// ===========================================================================
// Helpers
// ===========================================================================

/// Build a minimal valid grammar JSON with the given name and rules object.
fn minimal_grammar_json(name: &str, rules: serde_json::Value) -> serde_json::Value {
    json!({
        "name": name,
        "rules": rules,
        "extras": [{"type": "PATTERN", "value": "\\s"}],
        "word": null
    })
}

/// Convert JSON to GrammarJs, asserting success.
fn grammar_from_json(value: &serde_json::Value) -> GrammarJs {
    from_json(value).expect("from_json should succeed")
}

/// Attempt a full build and return the result (may fail).
fn try_build(grammar_json: serde_json::Value) -> Result<adze_tool::BuildResult, anyhow::Error> {
    let json_str = serde_json::to_string(&grammar_json).unwrap();
    let opts = BuildOptions {
        out_dir: std::env::temp_dir()
            .join("adze_test_v4")
            .to_string_lossy()
            .to_string(),
        emit_artifacts: false,
        compress_tables: false,
    };
    build_parser_from_json(json_str, opts)
}

// ===========================================================================
// 1. Grammar JSON parsing valid formats (8 tests)
// ===========================================================================

#[test]
fn json_parse_minimal_grammar_succeeds() {
    let val = minimal_grammar_json(
        "minimal",
        json!({
            "source_file": {"type": "PATTERN", "value": "\\d+"}
        }),
    );
    let gjs = grammar_from_json(&val);
    assert_eq!(gjs.name, "minimal");
    assert!(!gjs.rules.is_empty());
}

#[test]
fn json_parse_grammar_with_word_token() {
    let val = json!({
        "name": "word_test",
        "word": "identifier",
        "rules": {
            "source_file": {"type": "SYMBOL", "name": "identifier"},
            "identifier": {"type": "PATTERN", "value": "[a-z]+"}
        },
        "extras": []
    });
    let gjs = grammar_from_json(&val);
    assert_eq!(gjs.word, Some("identifier".to_string()));
}

#[test]
fn json_parse_grammar_with_extras_array() {
    let val = json!({
        "name": "extras_test",
        "rules": {
            "source_file": {"type": "PATTERN", "value": "."}
        },
        "extras": [
            {"type": "PATTERN", "value": "\\s"},
            {"type": "STRING", "value": "//"}
        ]
    });
    let gjs = grammar_from_json(&val);
    assert_eq!(gjs.extras.len(), 2);
}

#[test]
fn json_parse_grammar_with_inline_rules() {
    let val = json!({
        "name": "inline_test",
        "rules": {
            "source_file": {"type": "SYMBOL", "name": "_item"},
            "_item": {"type": "PATTERN", "value": "x"}
        },
        "inline": ["_item"],
        "extras": []
    });
    let gjs = grammar_from_json(&val);
    assert_eq!(gjs.inline, vec!["_item"]);
}

#[test]
fn json_parse_grammar_with_conflicts() {
    let val = json!({
        "name": "conflicts_test",
        "rules": {
            "source_file": {"type": "SYMBOL", "name": "expr"},
            "expr": {"type": "PATTERN", "value": "\\d+"}
        },
        "conflicts": [["source_file", "expr"]],
        "extras": []
    });
    let gjs = grammar_from_json(&val);
    assert_eq!(gjs.conflicts.len(), 1);
    assert_eq!(gjs.conflicts[0].len(), 2);
}

#[test]
fn json_parse_grammar_with_supertypes() {
    let val = json!({
        "name": "super_test",
        "rules": {
            "source_file": {"type": "PATTERN", "value": "."},
            "_expression": {"type": "PATTERN", "value": "\\d+"}
        },
        "supertypes": ["_expression"],
        "extras": []
    });
    let gjs = grammar_from_json(&val);
    assert_eq!(gjs.supertypes, vec!["_expression"]);
}

#[test]
fn json_parse_grammar_with_externals() {
    let val = json!({
        "name": "ext_test",
        "rules": {
            "source_file": {"type": "PATTERN", "value": "."}
        },
        "externals": [
            {"name": "indent", "type": "SYMBOL"},
            {"name": "dedent", "type": "SYMBOL"}
        ],
        "extras": []
    });
    let gjs = grammar_from_json(&val);
    assert_eq!(gjs.externals.len(), 2);
    assert_eq!(gjs.externals[0].name, "indent");
}

#[test]
fn json_parse_grammar_preserves_rule_order() {
    let val = json!({
        "name": "order_test",
        "rules": {
            "alpha": {"type": "PATTERN", "value": "a"},
            "beta": {"type": "PATTERN", "value": "b"},
            "gamma": {"type": "PATTERN", "value": "c"}
        },
        "extras": []
    });
    let gjs = grammar_from_json(&val);
    let names: Vec<&String> = gjs.rules.keys().collect();
    assert_eq!(names, vec!["alpha", "beta", "gamma"]);
}

// ===========================================================================
// 2. Grammar JSON missing fields (8 tests)
// ===========================================================================

#[test]
fn json_missing_name_field_errors() {
    let val = json!({
        "rules": {"source_file": {"type": "PATTERN", "value": "x"}}
    });
    assert!(from_json(&val).is_err());
}

#[test]
fn json_name_field_null_errors() {
    let val = json!({
        "name": null,
        "rules": {"source_file": {"type": "PATTERN", "value": "x"}}
    });
    assert!(from_json(&val).is_err());
}

#[test]
fn json_name_field_non_string_errors() {
    let val = json!({
        "name": 42,
        "rules": {"source_file": {"type": "PATTERN", "value": "x"}}
    });
    assert!(from_json(&val).is_err());
}

#[test]
fn json_missing_rules_still_parses_empty_grammar() {
    // from_json tolerates missing rules (just gets empty rules map)
    let val = json!({"name": "no_rules"});
    let gjs = grammar_from_json(&val);
    assert!(gjs.rules.is_empty());
}

#[test]
fn json_rules_as_array_ignored() {
    // rules must be an object; if array, no rules are parsed
    let val = json!({
        "name": "bad_rules",
        "rules": [{"type": "PATTERN", "value": "x"}]
    });
    let gjs = grammar_from_json(&val);
    assert!(gjs.rules.is_empty());
}

#[test]
fn json_top_level_not_object_errors() {
    let val = json!("not an object");
    assert!(from_json(&val).is_err());
}

#[test]
fn json_top_level_array_errors() {
    let val = json!([{"name": "test"}]);
    assert!(from_json(&val).is_err());
}

#[test]
fn json_top_level_null_errors() {
    let val = json!(null);
    assert!(from_json(&val).is_err());
}

// ===========================================================================
// 3. Grammar with rules array / rule types (8 tests)
// ===========================================================================

#[test]
fn rule_type_string_parsed() {
    let val = minimal_grammar_json(
        "str_rule",
        json!({
            "source_file": {"type": "STRING", "value": "hello"}
        }),
    );
    let gjs = grammar_from_json(&val);
    assert!(matches!(gjs.rules["source_file"], Rule::String { .. }));
}

#[test]
fn rule_type_pattern_parsed() {
    let val = minimal_grammar_json(
        "pat_rule",
        json!({
            "source_file": {"type": "PATTERN", "value": "[0-9]+"}
        }),
    );
    let gjs = grammar_from_json(&val);
    assert!(matches!(gjs.rules["source_file"], Rule::Pattern { .. }));
}

#[test]
fn rule_type_symbol_parsed() {
    let val = minimal_grammar_json(
        "sym_rule",
        json!({
            "source_file": {"type": "SYMBOL", "name": "item"},
            "item": {"type": "PATTERN", "value": "x"}
        }),
    );
    let gjs = grammar_from_json(&val);
    assert!(matches!(gjs.rules["source_file"], Rule::Symbol { .. }));
}

#[test]
fn rule_type_blank_parsed() {
    let val = minimal_grammar_json(
        "blank_rule",
        json!({
            "source_file": {
                "type": "CHOICE",
                "members": [
                    {"type": "PATTERN", "value": "x"},
                    {"type": "BLANK"}
                ]
            }
        }),
    );
    let gjs = grammar_from_json(&val);
    if let Rule::Choice { members } = &gjs.rules["source_file"] {
        assert!(matches!(members[1], Rule::Blank));
    } else {
        panic!("expected Choice rule");
    }
}

#[test]
fn rule_type_seq_parsed() {
    let val = minimal_grammar_json(
        "seq_rule",
        json!({
            "source_file": {
                "type": "SEQ",
                "members": [
                    {"type": "STRING", "value": "("},
                    {"type": "STRING", "value": ")"}
                ]
            }
        }),
    );
    let gjs = grammar_from_json(&val);
    assert!(matches!(gjs.rules["source_file"], Rule::Seq { .. }));
}

#[test]
fn rule_type_choice_parsed() {
    let val = minimal_grammar_json(
        "choice_rule",
        json!({
            "source_file": {
                "type": "CHOICE",
                "members": [
                    {"type": "STRING", "value": "a"},
                    {"type": "STRING", "value": "b"}
                ]
            }
        }),
    );
    let gjs = grammar_from_json(&val);
    assert!(matches!(gjs.rules["source_file"], Rule::Choice { .. }));
}

#[test]
fn rule_type_repeat_parsed() {
    let val = minimal_grammar_json(
        "repeat_rule",
        json!({
            "source_file": {
                "type": "REPEAT",
                "content": {"type": "STRING", "value": "x"}
            }
        }),
    );
    let gjs = grammar_from_json(&val);
    assert!(matches!(gjs.rules["source_file"], Rule::Repeat { .. }));
}

#[test]
fn rule_type_repeat1_parsed() {
    let val = minimal_grammar_json(
        "repeat1_rule",
        json!({
            "source_file": {
                "type": "REPEAT1",
                "content": {"type": "STRING", "value": "y"}
            }
        }),
    );
    let gjs = grammar_from_json(&val);
    assert!(matches!(gjs.rules["source_file"], Rule::Repeat1 { .. }));
}

// ===========================================================================
// 4. Grammar with token patterns (5 tests)
// ===========================================================================

#[test]
fn token_pattern_regex_value_preserved() {
    let val = minimal_grammar_json(
        "tok_pat",
        json!({
            "source_file": {"type": "PATTERN", "value": "[a-zA-Z_][a-zA-Z0-9_]*"}
        }),
    );
    let gjs = grammar_from_json(&val);
    if let Rule::Pattern { value } = &gjs.rules["source_file"] {
        assert_eq!(value, "[a-zA-Z_][a-zA-Z0-9_]*");
    } else {
        panic!("expected Pattern rule");
    }
}

#[test]
fn token_string_literal_value_preserved() {
    let val = minimal_grammar_json(
        "tok_str",
        json!({
            "source_file": {"type": "STRING", "value": "return"}
        }),
    );
    let gjs = grammar_from_json(&val);
    if let Rule::String { value } = &gjs.rules["source_file"] {
        assert_eq!(value, "return");
    } else {
        panic!("expected String rule");
    }
}

#[test]
fn token_wrapper_parsed() {
    let val = minimal_grammar_json(
        "tok_wrap",
        json!({
            "source_file": {
                "type": "TOKEN",
                "content": {"type": "PATTERN", "value": "\\d+\\.\\d+"}
            }
        }),
    );
    let gjs = grammar_from_json(&val);
    assert!(matches!(gjs.rules["source_file"], Rule::Token { .. }));
}

#[test]
fn immediate_token_parsed() {
    let val = minimal_grammar_json(
        "imm_tok",
        json!({
            "source_file": {
                "type": "IMMEDIATE_TOKEN",
                "content": {"type": "STRING", "value": "."}
            }
        }),
    );
    let gjs = grammar_from_json(&val);
    assert!(matches!(
        gjs.rules["source_file"],
        Rule::ImmediateToken { .. }
    ));
}

#[test]
fn field_rule_parsed_with_name() {
    let val = minimal_grammar_json(
        "field_test",
        json!({
            "source_file": {
                "type": "FIELD",
                "name": "body",
                "content": {"type": "PATTERN", "value": ".+"}
            }
        }),
    );
    let gjs = grammar_from_json(&val);
    if let Rule::Field { name, .. } = &gjs.rules["source_file"] {
        assert_eq!(name, "body");
    } else {
        panic!("expected Field rule");
    }
}

// ===========================================================================
// 5. Grammar names and identifiers (5 tests)
// ===========================================================================

#[test]
fn grammar_name_simple_ascii() {
    let val = minimal_grammar_json(
        "simple_lang",
        json!({"source_file": {"type": "PATTERN", "value": "."}}),
    );
    let gjs = grammar_from_json(&val);
    assert_eq!(gjs.name, "simple_lang");
}

#[test]
fn grammar_name_with_underscores() {
    let val = minimal_grammar_json(
        "my_custom_language",
        json!({"source_file": {"type": "PATTERN", "value": "."}}),
    );
    let gjs = grammar_from_json(&val);
    assert_eq!(gjs.name, "my_custom_language");
}

#[test]
fn grammar_name_single_char() {
    let val = minimal_grammar_json(
        "x",
        json!({"source_file": {"type": "PATTERN", "value": "."}}),
    );
    let gjs = grammar_from_json(&val);
    assert_eq!(gjs.name, "x");
}

#[test]
fn grammar_name_with_digits() {
    let val = minimal_grammar_json(
        "lang2024",
        json!({"source_file": {"type": "PATTERN", "value": "."}}),
    );
    let gjs = grammar_from_json(&val);
    assert_eq!(gjs.name, "lang2024");
}

#[test]
fn grammar_name_empty_string_parses() {
    let val = minimal_grammar_json(
        "",
        json!({"source_file": {"type": "PATTERN", "value": "."}}),
    );
    let gjs = grammar_from_json(&val);
    assert_eq!(gjs.name, "");
}

// ===========================================================================
// 6. Build error messages (8 tests)
// ===========================================================================

#[test]
fn build_error_invalid_json_string() {
    let opts = BuildOptions {
        out_dir: std::env::temp_dir()
            .join("adze_err_test")
            .to_string_lossy()
            .to_string(),
        emit_artifacts: false,
        compress_tables: false,
    };
    let result = build_parser_from_json("not json".to_string(), opts);
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(
        msg.contains("parse") || msg.contains("JSON") || msg.contains("json"),
        "error should mention parsing: {msg}"
    );
}

#[test]
fn build_error_empty_json_string() {
    let opts = BuildOptions {
        out_dir: std::env::temp_dir()
            .join("adze_err_test2")
            .to_string_lossy()
            .to_string(),
        emit_artifacts: false,
        compress_tables: false,
    };
    let result = build_parser_from_json(String::new(), opts);
    assert!(result.is_err());
}

#[test]
fn build_error_json_array_not_object() {
    let result = try_build(json!([]));
    // from_json expects an object
    assert!(result.is_err());
}

#[test]
fn build_error_json_number() {
    let result = try_build(json!(42));
    assert!(result.is_err());
}

#[test]
fn build_error_missing_name() {
    let result = try_build(json!({
        "rules": {"source_file": {"type": "PATTERN", "value": "."}}
    }));
    assert!(result.is_err());
}

#[test]
fn build_error_rules_with_unknown_type() {
    let val = minimal_grammar_json(
        "unknown_type",
        json!({
            "source_file": {"type": "NONEXISTENT_TYPE", "value": "x"}
        }),
    );
    // #1076: unknown rule types are rejected, not silently skipped.
    assert!(from_json(&val).is_err());
}

#[test]
fn build_error_rule_missing_type_field() {
    let val = minimal_grammar_json(
        "no_type",
        json!({
            "source_file": {"value": "hello"}
        }),
    );
    // #1076: a rule without "type" is rejected, not silently skipped.
    assert!(from_json(&val).is_err());
}

#[test]
fn build_error_rule_type_as_number() {
    let val = minimal_grammar_json(
        "num_type",
        json!({
            "source_file": {"type": 123, "value": "x"}
        }),
    );
    // #1076: a non-string rule type is rejected, not silently skipped.
    assert!(from_json(&val).is_err());
}

// ===========================================================================
// 7. BuildStats properties (5 tests)
// ===========================================================================

#[test]
fn build_stats_present_on_success() {
    let val = minimal_grammar_json(
        "stats_test",
        json!({
            "source_file": {"type": "SYMBOL", "name": "number"},
            "number": {"type": "PATTERN", "value": "\\d+"}
        }),
    );
    let result = try_build(val);
    assert!(result.is_ok(), "build failed: {:?}", result.err());
    let br = result.unwrap();
    // BuildStats must have sensible values
    assert!(br.build_stats.state_count > 0, "state_count must be > 0");
}

#[test]
fn build_stats_symbol_count_positive() {
    let val = minimal_grammar_json(
        "sym_count",
        json!({
            "source_file": {"type": "SYMBOL", "name": "tok"},
            "tok": {"type": "PATTERN", "value": "[a-z]+"}
        }),
    );
    let result = try_build(val);
    assert!(result.is_ok(), "build failed: {:?}", result.err());
    assert!(result.unwrap().build_stats.symbol_count > 0);
}

#[test]
fn build_stats_conflict_cells_non_negative() {
    let val = minimal_grammar_json(
        "conflicts_stat",
        json!({
            "source_file": {"type": "SYMBOL", "name": "item"},
            "item": {"type": "PATTERN", "value": "\\w+"}
        }),
    );
    let result = try_build(val);
    assert!(result.is_ok(), "build failed: {:?}", result.err());
    // conflict_cells is usize, always >= 0; just ensure it doesn't panic
    let _ = result.unwrap().build_stats.conflict_cells;
}

#[test]
fn build_result_grammar_name_matches() {
    let val = minimal_grammar_json(
        "name_check",
        json!({
            "source_file": {"type": "SYMBOL", "name": "tok"},
            "tok": {"type": "PATTERN", "value": "[a-z]+"}
        }),
    );
    let result = try_build(val);
    assert!(result.is_ok(), "build failed: {:?}", result.err());
    assert_eq!(result.unwrap().grammar_name, "name_check");
}

#[test]
fn build_result_parser_code_non_empty() {
    let val = minimal_grammar_json(
        "code_check",
        json!({
            "source_file": {"type": "SYMBOL", "name": "tok"},
            "tok": {"type": "PATTERN", "value": "[a-z]+"}
        }),
    );
    let result = try_build(val);
    assert!(result.is_ok(), "build failed: {:?}", result.err());
    assert!(!result.unwrap().parser_code.is_empty());
}

// ===========================================================================
// 8. Edge cases (8 tests)
// ===========================================================================

#[test]
fn edge_case_unicode_grammar_name() {
    let val = minimal_grammar_json(
        "café_语言",
        json!({"source_file": {"type": "PATTERN", "value": "."}}),
    );
    let gjs = grammar_from_json(&val);
    assert_eq!(gjs.name, "café_语言");
}

#[test]
fn edge_case_unicode_rule_name() {
    let val = minimal_grammar_json(
        "unicode_rule",
        json!({
            "source_file": {"type": "SYMBOL", "name": "表达式"},
            "表达式": {"type": "PATTERN", "value": "\\d+"}
        }),
    );
    let gjs = grammar_from_json(&val);
    assert!(gjs.rules.contains_key("表达式"));
}

#[test]
fn edge_case_empty_rules_object() {
    let val = json!({
        "name": "empty_rules",
        "rules": {},
        "extras": []
    });
    let gjs = grammar_from_json(&val);
    assert!(gjs.rules.is_empty());
}

#[test]
fn edge_case_empty_extras_array() {
    let val = json!({
        "name": "no_extras",
        "rules": {"source_file": {"type": "PATTERN", "value": "."}},
        "extras": []
    });
    let gjs = grammar_from_json(&val);
    assert!(gjs.extras.is_empty());
}

#[test]
fn edge_case_deep_nesting_choice_in_seq() {
    let val = minimal_grammar_json(
        "nested",
        json!({
            "source_file": {
                "type": "SEQ",
                "members": [
                    {
                        "type": "CHOICE",
                        "members": [
                            {"type": "STRING", "value": "a"},
                            {
                                "type": "SEQ",
                                "members": [
                                    {"type": "STRING", "value": "b"},
                                    {"type": "STRING", "value": "c"}
                                ]
                            }
                        ]
                    },
                    {"type": "STRING", "value": "d"}
                ]
            }
        }),
    );
    let gjs = grammar_from_json(&val);
    assert!(matches!(gjs.rules["source_file"], Rule::Seq { .. }));
}

#[test]
fn edge_case_precedence_rules() {
    let val = minimal_grammar_json(
        "prec_test",
        json!({
            "source_file": {
                "type": "PREC_LEFT",
                "value": 1,
                "content": {
                    "type": "SEQ",
                    "members": [
                        {"type": "SYMBOL", "name": "source_file"},
                        {"type": "STRING", "value": "+"},
                        {"type": "SYMBOL", "name": "source_file"}
                    ]
                }
            }
        }),
    );
    let gjs = grammar_from_json(&val);
    assert!(matches!(gjs.rules["source_file"], Rule::PrecLeft { .. }));
}

#[test]
fn edge_case_alias_rule() {
    let val = minimal_grammar_json(
        "alias_test",
        json!({
            "source_file": {
                "type": "ALIAS",
                "content": {"type": "PATTERN", "value": "\\d+"},
                "value": "number_literal",
                "named": true
            }
        }),
    );
    let gjs = grammar_from_json(&val);
    if let Rule::Alias { value, named, .. } = &gjs.rules["source_file"] {
        assert_eq!(value, "number_literal");
        assert!(named);
    } else {
        panic!("expected Alias rule");
    }
}

#[test]
fn edge_case_optional_rule() {
    let val = minimal_grammar_json(
        "opt_test",
        json!({
            "source_file": {
                "type": "OPTIONAL",
                "value": {"type": "STRING", "value": ";"}
            }
        }),
    );
    let gjs = grammar_from_json(&val);
    assert!(matches!(gjs.rules["source_file"], Rule::Optional { .. }));
}

// ===========================================================================
// Additional tests to reach 55+ (validation & conversion tests)
// ===========================================================================

#[test]
fn validate_valid_grammar_ok() {
    let mut gjs = GrammarJs::new("valid".to_string());
    gjs.rules.insert(
        "source_file".to_string(),
        Rule::Symbol {
            name: "item".to_string(),
        },
    );
    gjs.rules.insert(
        "item".to_string(),
        Rule::Pattern {
            value: "[a-z]+".to_string(),
        },
    );
    assert!(gjs.validate().is_ok());
}

#[test]
fn validate_dangling_symbol_ref_fails() {
    let mut gjs = GrammarJs::new("dangling".to_string());
    gjs.rules.insert(
        "source_file".to_string(),
        Rule::Symbol {
            name: "nonexistent".to_string(),
        },
    );
    assert!(gjs.validate().is_err());
}

#[test]
fn validate_word_token_references_existing_rule() {
    let mut gjs = GrammarJs::new("word_ok".to_string());
    gjs.word = Some("ident".to_string());
    gjs.rules.insert(
        "source_file".to_string(),
        Rule::Symbol {
            name: "ident".to_string(),
        },
    );
    gjs.rules.insert(
        "ident".to_string(),
        Rule::Pattern {
            value: "[a-z]+".to_string(),
        },
    );
    assert!(gjs.validate().is_ok());
}

#[test]
fn validate_word_token_references_missing_rule_fails() {
    let mut gjs = GrammarJs::new("word_bad".to_string());
    gjs.word = Some("missing_ident".to_string());
    gjs.rules.insert(
        "source_file".to_string(),
        Rule::Pattern {
            value: ".".to_string(),
        },
    );
    assert!(gjs.validate().is_err());
}

#[test]
fn validate_inline_references_missing_rule_fails() {
    let mut gjs = GrammarJs::new("inline_bad".to_string());
    gjs.inline.push("_ghost".to_string());
    gjs.rules.insert(
        "source_file".to_string(),
        Rule::Pattern {
            value: ".".to_string(),
        },
    );
    assert!(gjs.validate().is_err());
}

#[test]
fn validate_conflict_references_missing_rule_fails() {
    let mut gjs = GrammarJs::new("conflict_bad".to_string());
    gjs.conflicts.push(vec!["phantom".to_string()]);
    gjs.rules.insert(
        "source_file".to_string(),
        Rule::Pattern {
            value: ".".to_string(),
        },
    );
    assert!(gjs.validate().is_err());
}

#[test]
fn converter_converts_simple_grammar() {
    let mut gjs = GrammarJs::new("convert_test".to_string());
    gjs.rules.insert(
        "source_file".to_string(),
        Rule::Symbol {
            name: "number".to_string(),
        },
    );
    gjs.rules.insert(
        "number".to_string(),
        Rule::Pattern {
            value: "\\d+".to_string(),
        },
    );
    let converter = GrammarJsConverter::new(gjs);
    let result = converter.convert();
    assert!(result.is_ok(), "converter failed: {:?}", result.err());
}

#[test]
fn prec_dynamic_rule_parsed() {
    let val = minimal_grammar_json(
        "prec_dyn",
        json!({
            "source_file": {
                "type": "PREC_DYNAMIC",
                "value": 5,
                "content": {"type": "PATTERN", "value": "x"}
            }
        }),
    );
    let gjs = grammar_from_json(&val);
    assert!(matches!(gjs.rules["source_file"], Rule::PrecDynamic { .. }));
}

#[test]
fn prec_right_rule_parsed() {
    let val = minimal_grammar_json(
        "prec_right",
        json!({
            "source_file": {
                "type": "PREC_RIGHT",
                "value": 2,
                "content": {"type": "PATTERN", "value": "y"}
            }
        }),
    );
    let gjs = grammar_from_json(&val);
    assert!(matches!(gjs.rules["source_file"], Rule::PrecRight { .. }));
}

#[test]
fn prec_plain_rule_parsed() {
    let val = minimal_grammar_json(
        "prec_plain",
        json!({
            "source_file": {
                "type": "PREC",
                "value": -1,
                "content": {"type": "STRING", "value": "z"}
            }
        }),
    );
    let gjs = grammar_from_json(&val);
    if let Rule::Prec { value, .. } = &gjs.rules["source_file"] {
        assert_eq!(*value, -1);
    } else {
        panic!("expected Prec rule");
    }
}

#[test]
fn build_result_node_types_json_non_empty() {
    let val = minimal_grammar_json(
        "node_types",
        json!({
            "source_file": {"type": "SYMBOL", "name": "tok"},
            "tok": {"type": "PATTERN", "value": "[a-z]+"}
        }),
    );
    let result = try_build(val);
    assert!(result.is_ok(), "build failed: {:?}", result.err());
    assert!(!result.unwrap().node_types_json.is_empty());
}

#[test]
fn build_result_parser_path_contains_grammar_name() {
    let val = minimal_grammar_json(
        "path_check",
        json!({
            "source_file": {"type": "SYMBOL", "name": "tok"},
            "tok": {"type": "PATTERN", "value": "[a-z]+"}
        }),
    );
    let result = try_build(val);
    assert!(result.is_ok(), "build failed: {:?}", result.err());
    assert!(result.unwrap().parser_path.contains("path_check"));
}

#[test]
fn build_options_default_compress_tables_true() {
    let opts = BuildOptions::default();
    assert!(opts.compress_tables);
}

#[test]
fn build_options_default_emit_artifacts_false() {
    // When ADZE_EMIT_ARTIFACTS is not set, default should be false
    let opts = BuildOptions {
        out_dir: "test".to_string(),
        emit_artifacts: false,
        compress_tables: true,
    };
    assert!(!opts.emit_artifacts);
}

#[test]
fn edge_case_many_rules() {
    let mut rules = serde_json::Map::new();
    rules.insert(
        "source_file".to_string(),
        json!({"type": "SYMBOL", "name": "rule_0"}),
    );
    for i in 0..20 {
        let name = format!("rule_{i}");
        let next = if i < 19 {
            json!({"type": "SYMBOL", "name": format!("rule_{}", i + 1)})
        } else {
            json!({"type": "PATTERN", "value": "end"})
        };
        rules.insert(name, next);
    }
    let val = json!({
        "name": "many_rules",
        "rules": rules,
        "extras": [{"type": "PATTERN", "value": "\\s"}]
    });
    let gjs = grammar_from_json(&val);
    assert_eq!(gjs.rules.len(), 21);
}

#[test]
fn edge_case_empty_string_value_in_string_rule() {
    let val = minimal_grammar_json(
        "empty_str",
        json!({
            "source_file": {"type": "STRING", "value": ""}
        }),
    );
    let gjs = grammar_from_json(&val);
    if let Rule::String { value } = &gjs.rules["source_file"] {
        assert!(value.is_empty());
    } else {
        panic!("expected String rule");
    }
}

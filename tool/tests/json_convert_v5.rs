//! JSON grammar conversion tests for adze-tool (v5).
//!
//! 64 tests across 8 categories (8 tests each):
//! 1. json_to_grammar_*  — JSON → Grammar conversion
//! 2. json_from_grammar_* — Grammar → JSON conversion
//! 3. json_roundtrip_*    — JSON → Grammar → JSON
//! 4. json_format_*       — JSON format validation
//! 5. json_error_*        — error handling for invalid JSON
//! 6. json_complex_*      — complex grammar JSON
//! 7. json_tokens_*       — token representation in JSON
//! 8. json_rules_*        — rule representation in JSON

use adze_tool::grammar_js::json_converter::from_tree_sitter_json;
use adze_tool::grammar_js::{ExternalToken, GrammarJs, Rule};
use adze_tool::pure_rust_builder::{BuildOptions, build_parser_from_json};
use serde_json::json;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn opts() -> BuildOptions {
    BuildOptions {
        out_dir: "/tmp/adze-json-convert-v5".to_string(),
        emit_artifacts: false,
        compress_tables: false,
    }
}

fn make_grammar(name: &str, rules: serde_json::Value) -> serde_json::Value {
    json!({ "name": name, "rules": rules })
}

fn minimal_grammar_json(name: &str) -> serde_json::Value {
    json!({
        "name": name,
        "rules": {
            "source": { "type": "STRING", "value": "hello" }
        }
    })
}

fn grammar_js_to_json(grammar: &GrammarJs) -> serde_json::Value {
    serde_json::to_value(grammar).expect("GrammarJs should serialize to JSON")
}

// ===========================================================================
// Category 1: json_to_grammar — JSON → Grammar conversion (8 tests)
// ===========================================================================

#[test]
fn json_to_grammar_single_string_rule() {
    let g = from_tree_sitter_json(&make_grammar(
        "simple",
        json!({ "start": { "type": "STRING", "value": "x" } }),
    ))
    .unwrap();
    assert_eq!(g.name, "simple");
    assert_eq!(g.rules.len(), 1);
    assert!(matches!(&g.rules["start"], Rule::String { value } if value == "x"));
}

#[test]
fn json_to_grammar_pattern_rule() {
    let g = from_tree_sitter_json(&make_grammar(
        "pat_g",
        json!({ "num": { "type": "PATTERN", "value": r"\d+" } }),
    ))
    .unwrap();
    assert!(matches!(&g.rules["num"], Rule::Pattern { value } if value == r"\d+"));
}

#[test]
fn json_to_grammar_symbol_rule() {
    let g = from_tree_sitter_json(&make_grammar(
        "sym_g",
        json!({
            "root": { "type": "SYMBOL", "name": "item" },
            "item": { "type": "STRING", "value": "a" }
        }),
    ))
    .unwrap();
    assert!(matches!(&g.rules["root"], Rule::Symbol { name } if name == "item"));
}

#[test]
fn json_to_grammar_blank_rule() {
    let g = from_tree_sitter_json(&make_grammar(
        "blank_g",
        json!({ "empty": { "type": "BLANK" } }),
    ))
    .unwrap();
    assert!(matches!(&g.rules["empty"], Rule::Blank));
}

#[test]
fn json_to_grammar_seq_rule() {
    let g = from_tree_sitter_json(&make_grammar(
        "seq_g",
        json!({
            "pair": {
                "type": "SEQ",
                "members": [
                    { "type": "STRING", "value": "(" },
                    { "type": "STRING", "value": ")" }
                ]
            }
        }),
    ))
    .unwrap();
    match &g.rules["pair"] {
        Rule::Seq { members } => assert_eq!(members.len(), 2),
        other => panic!("Expected Seq, got {:?}", other),
    }
}

#[test]
fn json_to_grammar_choice_rule() {
    let g = from_tree_sitter_json(&make_grammar(
        "choice_g",
        json!({
            "alt": {
                "type": "CHOICE",
                "members": [
                    { "type": "STRING", "value": "a" },
                    { "type": "STRING", "value": "b" },
                    { "type": "STRING", "value": "c" }
                ]
            }
        }),
    ))
    .unwrap();
    match &g.rules["alt"] {
        Rule::Choice { members } => assert_eq!(members.len(), 3),
        other => panic!("Expected Choice, got {:?}", other),
    }
}

#[test]
fn json_to_grammar_word_token_parsed() {
    let g = from_tree_sitter_json(&json!({
        "name": "word_g",
        "word": "identifier",
        "rules": {
            "identifier": { "type": "PATTERN", "value": "[a-z]+" }
        }
    }))
    .unwrap();
    assert_eq!(g.word, Some("identifier".to_string()));
}

#[test]
fn json_to_grammar_extras_parsed() {
    let g = from_tree_sitter_json(&json!({
        "name": "extras_g",
        "rules": {
            "root": { "type": "STRING", "value": "x" }
        },
        "extras": [
            { "type": "PATTERN", "value": r"\s" }
        ]
    }))
    .unwrap();
    assert_eq!(g.extras.len(), 1);
    assert!(matches!(&g.extras[0], Rule::Pattern { value } if value == r"\s"));
}

// ===========================================================================
// Category 2: json_from_grammar — Grammar → JSON conversion (8 tests)
// ===========================================================================

#[test]
fn json_from_grammar_name_preserved() {
    let g = GrammarJs::new("my_lang".to_string());
    let j = grammar_js_to_json(&g);
    assert_eq!(j["name"].as_str().unwrap(), "my_lang");
}

#[test]
fn json_from_grammar_empty_rules_is_object() {
    let g = GrammarJs::new("empty_rules".to_string());
    let j = grammar_js_to_json(&g);
    assert!(j["rules"].is_object());
    assert_eq!(j["rules"].as_object().unwrap().len(), 0);
}

#[test]
fn json_from_grammar_string_rule_serialized() {
    let mut g = GrammarJs::new("str_ser".to_string());
    g.rules.insert(
        "kw".to_string(),
        Rule::String {
            value: "if".to_string(),
        },
    );
    let j = grammar_js_to_json(&g);
    assert_eq!(j["rules"]["kw"]["type"].as_str().unwrap(), "STRING");
    assert_eq!(j["rules"]["kw"]["value"].as_str().unwrap(), "if");
}

#[test]
fn json_from_grammar_pattern_rule_serialized() {
    let mut g = GrammarJs::new("pat_ser".to_string());
    g.rules.insert(
        "id".to_string(),
        Rule::Pattern {
            value: "[a-z]+".to_string(),
        },
    );
    let j = grammar_js_to_json(&g);
    assert_eq!(j["rules"]["id"]["type"].as_str().unwrap(), "PATTERN");
    assert_eq!(j["rules"]["id"]["value"].as_str().unwrap(), "[a-z]+");
}

#[test]
fn json_from_grammar_word_field_present() {
    let mut g = GrammarJs::new("word_ser".to_string());
    g.word = Some("ident".to_string());
    let j = grammar_js_to_json(&g);
    assert_eq!(j["word"].as_str().unwrap(), "ident");
}

#[test]
fn json_from_grammar_extras_serialized() {
    let mut g = GrammarJs::new("extras_ser".to_string());
    g.extras.push(Rule::Pattern {
        value: r"\s".to_string(),
    });
    let j = grammar_js_to_json(&g);
    let extras = j["extras"].as_array().unwrap();
    assert_eq!(extras.len(), 1);
    assert_eq!(extras[0]["type"].as_str().unwrap(), "PATTERN");
}

#[test]
fn json_from_grammar_conflicts_serialized() {
    let mut g = GrammarJs::new("conf_ser".to_string());
    g.conflicts
        .push(vec!["rule_a".to_string(), "rule_b".to_string()]);
    let j = grammar_js_to_json(&g);
    let conflicts = j["conflicts"].as_array().unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].as_array().unwrap().len(), 2);
}

#[test]
fn json_from_grammar_supertypes_serialized() {
    let mut g = GrammarJs::new("super_ser".to_string());
    g.supertypes.push("expression".to_string());
    let j = grammar_js_to_json(&g);
    let supertypes = j["supertypes"].as_array().unwrap();
    assert_eq!(supertypes.len(), 1);
    assert_eq!(supertypes[0].as_str().unwrap(), "expression");
}

// ===========================================================================
// Category 3: json_roundtrip — JSON → Grammar → JSON (8 tests)
// ===========================================================================

#[test]
fn json_roundtrip_name_preserved() {
    let original = minimal_grammar_json("roundtrip_name");
    let g = from_tree_sitter_json(&original).unwrap();
    let j = grammar_js_to_json(&g);
    assert_eq!(j["name"].as_str().unwrap(), "roundtrip_name");
}

#[test]
fn json_roundtrip_string_rule() {
    let original = make_grammar(
        "rt_str",
        json!({ "kw": { "type": "STRING", "value": "return" } }),
    );
    let g = from_tree_sitter_json(&original).unwrap();
    let j = grammar_js_to_json(&g);
    assert_eq!(j["rules"]["kw"]["type"].as_str().unwrap(), "STRING");
    assert_eq!(j["rules"]["kw"]["value"].as_str().unwrap(), "return");
}

#[test]
fn json_roundtrip_pattern_rule() {
    let original = make_grammar(
        "rt_pat",
        json!({ "num": { "type": "PATTERN", "value": "[0-9]+" } }),
    );
    let g = from_tree_sitter_json(&original).unwrap();
    let j = grammar_js_to_json(&g);
    assert_eq!(j["rules"]["num"]["value"].as_str().unwrap(), "[0-9]+");
}

#[test]
fn json_roundtrip_symbol_rule() {
    let original = make_grammar(
        "rt_sym",
        json!({
            "root": { "type": "SYMBOL", "name": "child" },
            "child": { "type": "STRING", "value": "c" }
        }),
    );
    let g = from_tree_sitter_json(&original).unwrap();
    let j = grammar_js_to_json(&g);
    assert_eq!(j["rules"]["root"]["type"].as_str().unwrap(), "SYMBOL");
    assert_eq!(j["rules"]["root"]["name"].as_str().unwrap(), "child");
}

#[test]
fn json_roundtrip_blank_rule() {
    let original = make_grammar("rt_blank", json!({ "empty": { "type": "BLANK" } }));
    let g = from_tree_sitter_json(&original).unwrap();
    let j = grammar_js_to_json(&g);
    assert_eq!(j["rules"]["empty"]["type"].as_str().unwrap(), "BLANK");
}

#[test]
fn json_roundtrip_seq_members_count() {
    let original = make_grammar(
        "rt_seq",
        json!({
            "pair": {
                "type": "SEQ",
                "members": [
                    { "type": "STRING", "value": "a" },
                    { "type": "STRING", "value": "b" }
                ]
            }
        }),
    );
    let g = from_tree_sitter_json(&original).unwrap();
    let j = grammar_js_to_json(&g);
    let members = j["rules"]["pair"]["members"].as_array().unwrap();
    assert_eq!(members.len(), 2);
}

#[test]
fn json_roundtrip_choice_members_count() {
    let original = make_grammar(
        "rt_choice",
        json!({
            "alt": {
                "type": "CHOICE",
                "members": [
                    { "type": "STRING", "value": "x" },
                    { "type": "STRING", "value": "y" }
                ]
            }
        }),
    );
    let g = from_tree_sitter_json(&original).unwrap();
    let j = grammar_js_to_json(&g);
    let members = j["rules"]["alt"]["members"].as_array().unwrap();
    assert_eq!(members.len(), 2);
}

#[test]
fn json_roundtrip_extras_preserved() {
    let original = json!({
        "name": "rt_extras",
        "rules": {
            "root": { "type": "STRING", "value": "x" }
        },
        "extras": [
            { "type": "PATTERN", "value": r"\s" }
        ]
    });
    let g = from_tree_sitter_json(&original).unwrap();
    let j = grammar_js_to_json(&g);
    let extras = j["extras"].as_array().unwrap();
    assert_eq!(extras.len(), 1);
}

// ===========================================================================
// Category 4: json_format — JSON format validation (8 tests)
// ===========================================================================

#[test]
fn json_format_top_level_is_object() {
    let g = GrammarJs::new("fmt_obj".to_string());
    let j = grammar_js_to_json(&g);
    assert!(j.is_object());
}

#[test]
fn json_format_name_is_string() {
    let g = GrammarJs::new("fmt_name".to_string());
    let j = grammar_js_to_json(&g);
    assert!(j["name"].is_string());
}

#[test]
fn json_format_rules_is_object() {
    let g = GrammarJs::new("fmt_rules".to_string());
    let j = grammar_js_to_json(&g);
    assert!(j["rules"].is_object());
}

#[test]
fn json_format_extras_is_array() {
    let g = GrammarJs::new("fmt_extras".to_string());
    let j = grammar_js_to_json(&g);
    assert!(j["extras"].is_array());
}

#[test]
fn json_format_conflicts_is_array() {
    let g = GrammarJs::new("fmt_conflicts".to_string());
    let j = grammar_js_to_json(&g);
    assert!(j["conflicts"].is_array());
}

#[test]
fn json_format_inline_is_array() {
    let g = GrammarJs::new("fmt_inline".to_string());
    let j = grammar_js_to_json(&g);
    assert!(j["inline"].is_array());
}

#[test]
fn json_format_supertypes_is_array() {
    let g = GrammarJs::new("fmt_supertypes".to_string());
    let j = grammar_js_to_json(&g);
    assert!(j["supertypes"].is_array());
}

#[test]
fn json_format_rule_type_field_present() {
    let mut g = GrammarJs::new("fmt_type".to_string());
    g.rules.insert(
        "root".to_string(),
        Rule::String {
            value: "ok".to_string(),
        },
    );
    let j = grammar_js_to_json(&g);
    assert!(j["rules"]["root"]["type"].is_string());
}

// ===========================================================================
// Category 5: json_error — error handling for invalid JSON (8 tests)
// ===========================================================================

#[test]
fn json_error_not_an_object() {
    let result = from_tree_sitter_json(&json!("not an object"));
    assert!(result.is_err());
}

#[test]
fn json_error_missing_name() {
    let result = from_tree_sitter_json(&json!({
        "rules": { "root": { "type": "BLANK" } }
    }));
    assert!(result.is_err());
}

#[test]
fn json_error_name_not_string() {
    let result = from_tree_sitter_json(&json!({
        "name": 42,
        "rules": { "root": { "type": "BLANK" } }
    }));
    assert!(result.is_err());
}

#[test]
fn json_error_array_input() {
    let result = from_tree_sitter_json(&json!([1, 2, 3]));
    assert!(result.is_err());
}

#[test]
fn json_error_null_input() {
    let result = from_tree_sitter_json(&json!(null));
    assert!(result.is_err());
}

#[test]
fn json_error_build_from_invalid_json_string() {
    let result = build_parser_from_json("not valid json".to_string(), opts());
    assert!(result.is_err());
}

#[test]
fn json_error_build_from_empty_json_object() {
    let result = build_parser_from_json("{}".to_string(), opts());
    assert!(result.is_err());
}

#[test]
fn json_error_unknown_rule_type_skipped() {
    // Unknown rule types are silently skipped during parsing, so the rule
    // won't appear in the resulting grammar.
    let g = from_tree_sitter_json(&json!({
        "name": "bad_type",
        "rules": {
            "root": { "type": "NONEXISTENT_TYPE", "value": "x" }
        }
    }))
    .unwrap();
    // The rule with an unknown type is not inserted
    assert!(!g.rules.contains_key("root"));
}

// ===========================================================================
// Category 6: json_complex — complex grammar JSON (8 tests)
// ===========================================================================

#[test]
fn json_complex_nested_seq_in_choice() {
    let g = from_tree_sitter_json(&make_grammar(
        "nested_sc",
        json!({
            "expr": {
                "type": "CHOICE",
                "members": [
                    {
                        "type": "SEQ",
                        "members": [
                            { "type": "STRING", "value": "(" },
                            { "type": "SYMBOL", "name": "expr" },
                            { "type": "STRING", "value": ")" }
                        ]
                    },
                    { "type": "PATTERN", "value": "[0-9]+" }
                ]
            }
        }),
    ))
    .unwrap();
    match &g.rules["expr"] {
        Rule::Choice { members } => {
            assert_eq!(members.len(), 2);
            assert!(matches!(&members[0], Rule::Seq { members: inner } if inner.len() == 3));
        }
        other => panic!("Expected Choice, got {:?}", other),
    }
}

#[test]
fn json_complex_repeat_of_symbol() {
    let g = from_tree_sitter_json(&make_grammar(
        "rep_g",
        json!({
            "list": {
                "type": "REPEAT",
                "content": { "type": "SYMBOL", "name": "item" }
            },
            "item": { "type": "PATTERN", "value": "[a-z]+" }
        }),
    ))
    .unwrap();
    assert!(matches!(&g.rules["list"], Rule::Repeat { .. }));
}

#[test]
fn json_complex_repeat1_of_string() {
    let g = from_tree_sitter_json(&make_grammar(
        "rep1_g",
        json!({
            "items": {
                "type": "REPEAT1",
                "content": { "type": "STRING", "value": "x" }
            }
        }),
    ))
    .unwrap();
    assert!(matches!(&g.rules["items"], Rule::Repeat1 { .. }));
}

#[test]
fn json_complex_optional_rule() {
    let g = from_tree_sitter_json(&make_grammar(
        "opt_g",
        json!({
            "maybe": {
                "type": "OPTIONAL",
                "value": { "type": "STRING", "value": ";" }
            }
        }),
    ))
    .unwrap();
    assert!(matches!(&g.rules["maybe"], Rule::Optional { .. }));
}

#[test]
fn json_complex_prec_rule() {
    let g = from_tree_sitter_json(&make_grammar(
        "prec_g",
        json!({
            "expr": {
                "type": "PREC",
                "value": 5,
                "content": { "type": "STRING", "value": "+" }
            }
        }),
    ))
    .unwrap();
    match &g.rules["expr"] {
        Rule::Prec { value, .. } => assert_eq!(*value, 5),
        other => panic!("Expected Prec, got {:?}", other),
    }
}

#[test]
fn json_complex_prec_left_rule() {
    let g = from_tree_sitter_json(&make_grammar(
        "prec_left_g",
        json!({
            "expr": {
                "type": "PREC_LEFT",
                "value": 3,
                "content": { "type": "STRING", "value": "-" }
            }
        }),
    ))
    .unwrap();
    match &g.rules["expr"] {
        Rule::PrecLeft { value, .. } => assert_eq!(*value, 3),
        other => panic!("Expected PrecLeft, got {:?}", other),
    }
}

#[test]
fn json_complex_prec_right_rule() {
    let g = from_tree_sitter_json(&make_grammar(
        "prec_right_g",
        json!({
            "expr": {
                "type": "PREC_RIGHT",
                "value": 7,
                "content": { "type": "STRING", "value": "**" }
            }
        }),
    ))
    .unwrap();
    match &g.rules["expr"] {
        Rule::PrecRight { value, .. } => assert_eq!(*value, 7),
        other => panic!("Expected PrecRight, got {:?}", other),
    }
}

#[test]
fn json_complex_prec_dynamic_rule() {
    let g = from_tree_sitter_json(&make_grammar(
        "prec_dyn_g",
        json!({
            "expr": {
                "type": "PREC_DYNAMIC",
                "value": -2,
                "content": { "type": "STRING", "value": "?" }
            }
        }),
    ))
    .unwrap();
    match &g.rules["expr"] {
        Rule::PrecDynamic { value, .. } => assert_eq!(*value, -2),
        other => panic!("Expected PrecDynamic, got {:?}", other),
    }
}

// ===========================================================================
// Category 7: json_tokens — token representation in JSON (8 tests)
// ===========================================================================

#[test]
fn json_tokens_string_type_tag() {
    let mut g = GrammarJs::new("tok_str".to_string());
    g.rules.insert(
        "kw".to_string(),
        Rule::String {
            value: "let".to_string(),
        },
    );
    let j = grammar_js_to_json(&g);
    assert_eq!(j["rules"]["kw"]["type"].as_str().unwrap(), "STRING");
}

#[test]
fn json_tokens_pattern_type_tag() {
    let mut g = GrammarJs::new("tok_pat".to_string());
    g.rules.insert(
        "num".to_string(),
        Rule::Pattern {
            value: r"\d+".to_string(),
        },
    );
    let j = grammar_js_to_json(&g);
    assert_eq!(j["rules"]["num"]["type"].as_str().unwrap(), "PATTERN");
}

#[test]
fn json_tokens_token_wrapper() {
    let g = from_tree_sitter_json(&make_grammar(
        "tok_wrap",
        json!({
            "t": {
                "type": "TOKEN",
                "content": { "type": "PATTERN", "value": "[a-z]+" }
            }
        }),
    ))
    .unwrap();
    assert!(matches!(&g.rules["t"], Rule::Token { .. }));
}

#[test]
fn json_tokens_immediate_token() {
    let g = from_tree_sitter_json(&make_grammar(
        "imm_tok",
        json!({
            "t": {
                "type": "IMMEDIATE_TOKEN",
                "content": { "type": "STRING", "value": "." }
            }
        }),
    ))
    .unwrap();
    assert!(matches!(&g.rules["t"], Rule::ImmediateToken { .. }));
}

#[test]
fn json_tokens_token_roundtrip_type() {
    let mut g = GrammarJs::new("tok_rt".to_string());
    g.rules.insert(
        "t".to_string(),
        Rule::Token {
            content: Box::new(Rule::Pattern {
                value: "[0-9]+".to_string(),
            }),
        },
    );
    let j = grammar_js_to_json(&g);
    assert_eq!(j["rules"]["t"]["type"].as_str().unwrap(), "TOKEN");
    assert_eq!(
        j["rules"]["t"]["content"]["type"].as_str().unwrap(),
        "PATTERN"
    );
}

#[test]
fn json_tokens_immediate_token_roundtrip() {
    let mut g = GrammarJs::new("imm_rt".to_string());
    g.rules.insert(
        "t".to_string(),
        Rule::ImmediateToken {
            content: Box::new(Rule::String {
                value: ".".to_string(),
            }),
        },
    );
    let j = grammar_js_to_json(&g);
    assert_eq!(j["rules"]["t"]["type"].as_str().unwrap(), "IMMEDIATE_TOKEN");
}

#[test]
fn json_tokens_external_token_parsed() {
    let g = from_tree_sitter_json(&json!({
        "name": "ext_tok",
        "rules": {
            "root": { "type": "BLANK" }
        },
        "externals": [
            { "name": "heredoc_start", "type": "SYMBOL" }
        ]
    }))
    .unwrap();
    assert_eq!(g.externals.len(), 1);
    assert_eq!(g.externals[0].name, "heredoc_start");
}

#[test]
fn json_tokens_external_serialized() {
    let mut g = GrammarJs::new("ext_ser".to_string());
    g.externals.push(ExternalToken {
        name: "indent".to_string(),
        symbol: "external_0".to_string(),
    });
    let j = grammar_js_to_json(&g);
    let externals = j["externals"].as_array().unwrap();
    assert_eq!(externals.len(), 1);
    assert_eq!(externals[0]["name"].as_str().unwrap(), "indent");
}

// ===========================================================================
// Category 8: json_rules — rule representation in JSON (8 tests)
// ===========================================================================

#[test]
fn json_rules_field_rule_parsed() {
    let g = from_tree_sitter_json(&make_grammar(
        "field_g",
        json!({
            "assign": {
                "type": "FIELD",
                "name": "left",
                "content": { "type": "SYMBOL", "name": "assign" }
            }
        }),
    ))
    .unwrap();
    match &g.rules["assign"] {
        Rule::Field { name, .. } => assert_eq!(name, "left"),
        other => panic!("Expected Field, got {:?}", other),
    }
}

#[test]
fn json_rules_alias_rule_parsed() {
    let g = from_tree_sitter_json(&make_grammar(
        "alias_g",
        json!({
            "item": {
                "type": "ALIAS",
                "value": "renamed",
                "named": true,
                "content": { "type": "SYMBOL", "name": "item" }
            }
        }),
    ))
    .unwrap();
    match &g.rules["item"] {
        Rule::Alias { value, named, .. } => {
            assert_eq!(value, "renamed");
            assert!(*named);
        }
        other => panic!("Expected Alias, got {:?}", other),
    }
}

#[test]
fn json_rules_alias_unnamed() {
    let g = from_tree_sitter_json(&make_grammar(
        "alias_unnamed",
        json!({
            "tok": {
                "type": "ALIAS",
                "value": "op",
                "named": false,
                "content": { "type": "STRING", "value": "+" }
            }
        }),
    ))
    .unwrap();
    match &g.rules["tok"] {
        Rule::Alias { named, .. } => assert!(!*named),
        other => panic!("Expected Alias, got {:?}", other),
    }
}

#[test]
fn json_rules_field_roundtrip() {
    let mut g = GrammarJs::new("field_rt".to_string());
    g.rules.insert(
        "assign".to_string(),
        Rule::Field {
            name: "lhs".to_string(),
            content: Box::new(Rule::Symbol {
                name: "expr".to_string(),
            }),
        },
    );
    let j = grammar_js_to_json(&g);
    assert_eq!(j["rules"]["assign"]["type"].as_str().unwrap(), "FIELD");
    assert_eq!(j["rules"]["assign"]["name"].as_str().unwrap(), "lhs");
}

#[test]
fn json_rules_alias_roundtrip() {
    let mut g = GrammarJs::new("alias_rt".to_string());
    g.rules.insert(
        "item".to_string(),
        Rule::Alias {
            content: Box::new(Rule::Symbol {
                name: "orig".to_string(),
            }),
            value: "alias_name".to_string(),
            named: true,
        },
    );
    let j = grammar_js_to_json(&g);
    assert_eq!(j["rules"]["item"]["type"].as_str().unwrap(), "ALIAS");
    assert_eq!(j["rules"]["item"]["value"].as_str().unwrap(), "alias_name");
    assert!(j["rules"]["item"]["named"].as_bool().unwrap());
}

#[test]
fn json_rules_inline_parsed() {
    let g = from_tree_sitter_json(&json!({
        "name": "inline_g",
        "rules": {
            "root": { "type": "SYMBOL", "name": "helper" },
            "helper": { "type": "STRING", "value": "h" }
        },
        "inline": ["helper"]
    }))
    .unwrap();
    assert_eq!(g.inline.len(), 1);
    assert_eq!(g.inline[0], "helper");
}

#[test]
fn json_rules_conflicts_parsed() {
    let g = from_tree_sitter_json(&json!({
        "name": "conflict_g",
        "rules": {
            "a": { "type": "BLANK" },
            "b": { "type": "BLANK" }
        },
        "conflicts": [["a", "b"]]
    }))
    .unwrap();
    assert_eq!(g.conflicts.len(), 1);
    assert_eq!(g.conflicts[0], vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn json_rules_multiple_rules_preserved() {
    let g = from_tree_sitter_json(&make_grammar(
        "multi",
        json!({
            "program": { "type": "SYMBOL", "name": "stmt" },
            "stmt": { "type": "SYMBOL", "name": "expr" },
            "expr": { "type": "PATTERN", "value": "[0-9]+" }
        }),
    ))
    .unwrap();
    assert_eq!(g.rules.len(), 3);
    assert!(g.rules.contains_key("program"));
    assert!(g.rules.contains_key("stmt"));
    assert!(g.rules.contains_key("expr"));
}

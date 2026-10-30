//! Tests for adze-tool JSON grammar emission via the pure-Rust builder.
//!
//! Covers:
//! 1. JSON grammar output — valid JSON produced by grammar pipeline
//! 2. JSON structure — has expected keys (name, rules, extras, conflicts, etc.)
//! 3. Token representation — token patterns in JSON
//! 4. Rule representation — rules with RHS sequences in JSON
//! 5. Deterministic output — same grammar → same JSON
//! 6. Grammar roundtrip — JSON → parse → compare
//! 7. Complex grammars — expressions, recursive, deep nesting
//! 8. Edge cases — minimal grammar, many rules, many tokens

use adze_ir::builder::GrammarBuilder;
use adze_tool::grammar_js::json_converter::from_tree_sitter_json;
use adze_tool::pure_rust_builder::{
    BuildOptions, BuildResult, build_parser, build_parser_from_json,
};
use serde_json::{Value, json};

// ===========================================================================
// Helpers
// ===========================================================================

fn test_opts() -> BuildOptions {
    BuildOptions {
        out_dir: "/tmp/json_emit_v3".to_string(),
        emit_artifacts: false,
        compress_tables: false,
    }
}

fn build_json_ok(grammar_json: &Value) -> BuildResult {
    build_parser_from_json(grammar_json.to_string(), test_opts())
        .expect("build_parser_from_json should succeed")
}

fn simple_grammar() -> adze_ir::Grammar {
    GrammarBuilder::new("simple_emit")
        .token("a", "a")
        .rule("s", vec!["a"])
        .start("s")
        .build()
}

fn arithmetic_grammar() -> adze_ir::Grammar {
    GrammarBuilder::new("arith")
        .token("NUMBER", r"[0-9]+")
        .token("+", "+")
        .token("*", "*")
        .token("(", "(")
        .token(")", ")")
        .rule("expr", vec!["expr", "+", "term"])
        .rule("expr", vec!["term"])
        .rule("term", vec!["term", "*", "factor"])
        .rule("term", vec!["factor"])
        .rule("factor", vec!["(", "expr", ")"])
        .rule("factor", vec!["NUMBER"])
        .start("expr")
        .build()
}

fn minimal_json() -> Value {
    json!({
        "name": "minimal_emit",
        "rules": {
            "source_file": { "type": "STRING", "value": "hello" }
        }
    })
}

// ===========================================================================
// 1. JSON grammar output — valid JSON produced by grammar pipeline (8 tests)
// ===========================================================================

#[test]
fn output_parser_code_is_nonempty() {
    let result = build_parser(simple_grammar(), test_opts()).unwrap();
    assert!(!result.parser_code.is_empty());
}

#[test]
fn output_node_types_is_valid_json() {
    let result = build_parser(simple_grammar(), test_opts()).unwrap();
    let parsed: Value = serde_json::from_str(&result.node_types_json).unwrap();
    assert!(parsed.is_array());
}

#[test]
fn output_grammar_name_matches() {
    let result = build_parser(simple_grammar(), test_opts()).unwrap();
    assert_eq!(result.grammar_name, "simple_emit");
}

#[test]
fn output_parser_path_contains_grammar_name() {
    let result = build_parser(simple_grammar(), test_opts()).unwrap();
    assert!(
        result.parser_path.contains("simple_emit"),
        "parser_path should contain grammar name: {}",
        result.parser_path
    );
}

#[test]
fn output_build_stats_state_count_positive() {
    let result = build_parser(simple_grammar(), test_opts()).unwrap();
    assert!(result.build_stats.state_count > 0);
}

#[test]
fn output_build_stats_symbol_count_positive() {
    let result = build_parser(simple_grammar(), test_opts()).unwrap();
    assert!(result.build_stats.symbol_count > 0);
}

#[test]
fn output_node_types_array_has_entries() {
    let result = build_parser(simple_grammar(), test_opts()).unwrap();
    let parsed: Value = serde_json::from_str(&result.node_types_json).unwrap();
    let arr = parsed.as_array().unwrap();
    assert!(!arr.is_empty(), "node_types should have at least one entry");
}

#[test]
fn output_json_from_json_pipeline_is_valid() {
    let result = build_json_ok(&minimal_json());
    let _: Value = serde_json::from_str(&result.node_types_json).unwrap();
}

// ===========================================================================
// 2. JSON structure — has expected keys (8 tests)
// ===========================================================================

#[test]
fn json_structure_from_tree_sitter_has_name() {
    let g = from_tree_sitter_json(&minimal_json()).unwrap();
    assert_eq!(g.name, "minimal_emit");
}

#[test]
fn json_structure_from_tree_sitter_has_rules() {
    let g = from_tree_sitter_json(&minimal_json()).unwrap();
    assert!(g.rules.contains_key("source_file"));
}

#[test]
fn json_structure_extras_defaults_to_empty() {
    let g = from_tree_sitter_json(&minimal_json()).unwrap();
    assert!(g.extras.is_empty());
}

#[test]
fn json_structure_conflicts_defaults_to_empty() {
    let g = from_tree_sitter_json(&minimal_json()).unwrap();
    assert!(g.conflicts.is_empty());
}

#[test]
fn json_structure_inline_defaults_to_empty() {
    let g = from_tree_sitter_json(&minimal_json()).unwrap();
    assert!(g.inline.is_empty());
}

#[test]
fn json_structure_word_defaults_to_none() {
    let g = from_tree_sitter_json(&minimal_json()).unwrap();
    assert!(g.word.is_none());
}

#[test]
fn json_structure_externals_defaults_to_empty() {
    let g = from_tree_sitter_json(&minimal_json()).unwrap();
    assert!(g.externals.is_empty());
}

#[test]
fn json_structure_with_extras_parsed() {
    let g_json = json!({
        "name": "extras_test",
        "extras": [{ "type": "PATTERN", "value": "\\s+" }],
        "rules": {
            "source_file": { "type": "STRING", "value": "x" }
        }
    });
    let g = from_tree_sitter_json(&g_json).unwrap();
    assert!(!g.extras.is_empty(), "extras should be parsed");
}

// ===========================================================================
// 3. Token representation — token patterns in JSON (8 tests)
// ===========================================================================

#[test]
fn token_string_literal_builds() {
    let g = json!({
        "name": "tok_string",
        "rules": {
            "source_file": { "type": "STRING", "value": "hello" }
        }
    });
    let r = build_json_ok(&g);
    assert!(!r.parser_code.is_empty());
}

#[test]
fn token_pattern_regex_builds() {
    let g = json!({
        "name": "tok_pattern",
        "rules": {
            "source_file": { "type": "PATTERN", "value": "[a-z]+" }
        }
    });
    let r = build_json_ok(&g);
    assert!(r.build_stats.symbol_count > 0);
}

#[test]
fn token_single_char_builds() {
    let g = json!({
        "name": "tok_single",
        "rules": {
            "source_file": { "type": "STRING", "value": "x" }
        }
    });
    let r = build_json_ok(&g);
    assert!(!r.parser_code.is_empty());
}

#[test]
fn token_numeric_pattern_builds() {
    let g = json!({
        "name": "tok_num",
        "rules": {
            "source_file": { "type": "PATTERN", "value": "[0-9]+" }
        }
    });
    let r = build_json_ok(&g);
    assert!(r.build_stats.state_count > 0);
}

#[test]
fn token_multichar_string_builds() {
    let g = json!({
        "name": "tok_multi",
        "rules": {
            "source_file": { "type": "STRING", "value": "return" }
        }
    });
    let r = build_json_ok(&g);
    assert!(!r.parser_code.is_empty());
}

#[test]
fn token_complex_regex_builds() {
    let g = json!({
        "name": "tok_complex",
        "rules": {
            "source_file": { "type": "PATTERN", "value": "[a-zA-Z_][a-zA-Z0-9_]*" }
        }
    });
    let r = build_json_ok(&g);
    assert!(r.build_stats.symbol_count > 0);
}

#[test]
fn token_in_seq_builds() {
    let g = json!({
        "name": "tok_seq",
        "rules": {
            "source_file": {
                "type": "SEQ",
                "members": [
                    { "type": "STRING", "value": "(" },
                    { "type": "PATTERN", "value": "[0-9]+" },
                    { "type": "STRING", "value": ")" }
                ]
            }
        }
    });
    let r = build_json_ok(&g);
    assert!(r.build_stats.state_count > 0);
}

#[test]
fn token_in_choice_builds() {
    let g = json!({
        "name": "tok_choice",
        "rules": {
            "source_file": {
                "type": "CHOICE",
                "members": [
                    { "type": "STRING", "value": "true" },
                    { "type": "STRING", "value": "false" }
                ]
            }
        }
    });
    let r = build_json_ok(&g);
    assert!(r.build_stats.symbol_count > 0);
}

// ===========================================================================
// 4. Rule representation — rules with RHS sequences in JSON (7 tests)
// ===========================================================================

#[test]
fn rule_simple_symbol_ref() {
    let g = json!({
        "name": "rule_sym",
        "rules": {
            "source_file": { "type": "SYMBOL", "name": "item" },
            "item": { "type": "STRING", "value": "tok" }
        }
    });
    let r = build_json_ok(&g);
    assert!(!r.parser_code.is_empty());
}

#[test]
fn rule_seq_of_symbols() {
    let g = json!({
        "name": "rule_seq_sym",
        "rules": {
            "source_file": {
                "type": "SEQ",
                "members": [
                    { "type": "SYMBOL", "name": "lhs" },
                    { "type": "STRING", "value": "=" },
                    { "type": "SYMBOL", "name": "rhs" }
                ]
            },
            "lhs": { "type": "PATTERN", "value": "[a-z]+" },
            "rhs": { "type": "PATTERN", "value": "[0-9]+" }
        }
    });
    let r = build_json_ok(&g);
    assert!(r.build_stats.state_count > 0);
}

#[test]
fn rule_choice_of_multiple_alts() {
    let g = json!({
        "name": "rule_choice_alts",
        "rules": {
            "source_file": {
                "type": "CHOICE",
                "members": [
                    { "type": "STRING", "value": "a" },
                    { "type": "STRING", "value": "b" },
                    { "type": "STRING", "value": "c" }
                ]
            }
        }
    });
    let r = build_json_ok(&g);
    assert!(r.build_stats.symbol_count > 0);
}

#[test]
fn rule_repeat_builds() {
    let g = json!({
        "name": "rule_repeat",
        "rules": {
            "source_file": {
                "type": "REPEAT",
                "content": { "type": "STRING", "value": "x" }
            }
        }
    });
    let r = build_json_ok(&g);
    assert!(!r.parser_code.is_empty());
}

#[test]
fn rule_repeat1_builds() {
    let g = json!({
        "name": "rule_repeat1",
        "rules": {
            "source_file": {
                "type": "REPEAT1",
                "content": { "type": "PATTERN", "value": "[a-z]" }
            }
        }
    });
    let r = build_json_ok(&g);
    assert!(r.build_stats.state_count > 0);
}

#[test]
fn rule_optional_builds() {
    let g = json!({
        "name": "rule_optional",
        "rules": {
            "source_file": {
                "type": "SEQ",
                "members": [
                    { "type": "STRING", "value": "start" },
                    {
                        "type": "CHOICE",
                        "members": [
                            { "type": "STRING", "value": "opt" },
                            { "type": "BLANK" }
                        ]
                    }
                ]
            }
        }
    });
    let r = build_json_ok(&g);
    assert!(r.build_stats.symbol_count > 0);
}

#[test]
fn rule_nested_seq_in_choice() {
    let g = json!({
        "name": "rule_nested",
        "rules": {
            "source_file": {
                "type": "CHOICE",
                "members": [
                    {
                        "type": "SEQ",
                        "members": [
                            { "type": "STRING", "value": "a" },
                            { "type": "STRING", "value": "b" }
                        ]
                    },
                    {
                        "type": "SEQ",
                        "members": [
                            { "type": "STRING", "value": "c" },
                            { "type": "STRING", "value": "d" }
                        ]
                    }
                ]
            }
        }
    });
    let r = build_json_ok(&g);
    assert!(r.build_stats.state_count > 0);
}

// ===========================================================================
// 5. Deterministic output — same grammar → same JSON (6 tests)
// ===========================================================================

#[test]
fn deterministic_parser_code_from_ir() {
    let r1 = build_parser(simple_grammar(), test_opts()).unwrap();
    let r2 = build_parser(simple_grammar(), test_opts()).unwrap();
    assert_eq!(r1.parser_code, r2.parser_code);
}

#[test]
fn deterministic_node_types_from_ir() {
    let r1 = build_parser(simple_grammar(), test_opts()).unwrap();
    let r2 = build_parser(simple_grammar(), test_opts()).unwrap();
    assert_eq!(r1.node_types_json, r2.node_types_json);
}

#[test]
fn deterministic_grammar_name_from_ir() {
    let r1 = build_parser(simple_grammar(), test_opts()).unwrap();
    let r2 = build_parser(simple_grammar(), test_opts()).unwrap();
    assert_eq!(r1.grammar_name, r2.grammar_name);
}

#[test]
fn deterministic_parser_code_from_json() {
    let g = minimal_json();
    let r1 = build_json_ok(&g);
    let r2 = build_json_ok(&g);
    assert_eq!(r1.parser_code, r2.parser_code);
}

#[test]
fn deterministic_node_types_from_json() {
    let g = minimal_json();
    let r1 = build_json_ok(&g);
    let r2 = build_json_ok(&g);
    assert_eq!(r1.node_types_json, r2.node_types_json);
}

#[test]
fn deterministic_build_stats_from_json() {
    let g = minimal_json();
    let r1 = build_json_ok(&g);
    let r2 = build_json_ok(&g);
    assert_eq!(r1.build_stats.state_count, r2.build_stats.state_count);
    assert_eq!(r1.build_stats.symbol_count, r2.build_stats.symbol_count);
}

// ===========================================================================
// 6. Grammar roundtrip — JSON → parse → compare (6 tests)
// ===========================================================================

#[test]
fn roundtrip_name_preserved() {
    let g_json = json!({
        "name": "roundtrip_name",
        "rules": {
            "source_file": { "type": "STRING", "value": "hi" }
        }
    });
    let parsed = from_tree_sitter_json(&g_json).unwrap();
    assert_eq!(parsed.name, "roundtrip_name");
}

#[test]
fn roundtrip_rules_count_preserved() {
    let g_json = json!({
        "name": "roundtrip_rules",
        "rules": {
            "source_file": { "type": "SYMBOL", "name": "item" },
            "item": { "type": "PATTERN", "value": "[a-z]+" }
        }
    });
    let parsed = from_tree_sitter_json(&g_json).unwrap();
    assert_eq!(parsed.rules.len(), 2);
}

#[test]
fn roundtrip_extras_preserved() {
    let g_json = json!({
        "name": "roundtrip_extras",
        "extras": [{ "type": "PATTERN", "value": "\\s+" }],
        "rules": {
            "source_file": { "type": "STRING", "value": "x" }
        }
    });
    let parsed = from_tree_sitter_json(&g_json).unwrap();
    assert_eq!(parsed.extras.len(), 1);
}

#[test]
fn roundtrip_conflicts_preserved() {
    let g_json = json!({
        "name": "roundtrip_conflicts",
        "conflicts": [["source_file", "item"]],
        "rules": {
            "source_file": { "type": "SYMBOL", "name": "item" },
            "item": { "type": "STRING", "value": "v" }
        }
    });
    let parsed = from_tree_sitter_json(&g_json).unwrap();
    assert_eq!(parsed.conflicts.len(), 1);
    assert_eq!(parsed.conflicts[0].len(), 2);
}

#[test]
fn roundtrip_word_preserved() {
    let g_json = json!({
        "name": "roundtrip_word",
        "word": "identifier",
        "rules": {
            "source_file": { "type": "SYMBOL", "name": "identifier" },
            "identifier": { "type": "PATTERN", "value": "[a-z]+" }
        }
    });
    let parsed = from_tree_sitter_json(&g_json).unwrap();
    assert_eq!(parsed.word.as_deref(), Some("identifier"));
}

#[test]
fn roundtrip_inline_preserved() {
    let g_json = json!({
        "name": "roundtrip_inline",
        "inline": ["helper"],
        "rules": {
            "source_file": { "type": "SYMBOL", "name": "helper" },
            "helper": { "type": "STRING", "value": "z" }
        }
    });
    let parsed = from_tree_sitter_json(&g_json).unwrap();
    assert_eq!(parsed.inline.len(), 1);
    assert_eq!(parsed.inline[0], "helper");
}

// ===========================================================================
// 7. Complex grammars — expressions, recursive, deep nesting (7 tests)
// ===========================================================================

#[test]
fn complex_arithmetic_grammar_builds() {
    let result = build_parser(arithmetic_grammar(), test_opts()).unwrap();
    assert_eq!(result.grammar_name, "arith");
    assert!(result.build_stats.state_count > 0);
}

#[test]
fn complex_arithmetic_node_types_valid_json() {
    let result = build_parser(arithmetic_grammar(), test_opts()).unwrap();
    let parsed: Value = serde_json::from_str(&result.node_types_json).unwrap();
    assert!(parsed.is_array());
}

#[test]
fn complex_recursive_via_json() {
    let g = json!({
        "name": "recursive_json",
        "rules": {
            "source_file": { "type": "SYMBOL", "name": "list" },
            "list": {
                "type": "CHOICE",
                "members": [
                    {
                        "type": "SEQ",
                        "members": [
                            { "type": "STRING", "value": "[" },
                            { "type": "SYMBOL", "name": "list" },
                            { "type": "STRING", "value": "]" }
                        ]
                    },
                    { "type": "PATTERN", "value": "[a-z]+" }
                ]
            }
        }
    });
    let r = build_json_ok(&g);
    assert!(r.build_stats.state_count > 0);
}

#[test]
fn complex_deep_nesting_seq_in_choice_in_seq() {
    let g = json!({
        "name": "deep_nest",
        "rules": {
            "source_file": {
                "type": "SEQ",
                "members": [
                    { "type": "STRING", "value": "begin" },
                    {
                        "type": "CHOICE",
                        "members": [
                            {
                                "type": "SEQ",
                                "members": [
                                    { "type": "STRING", "value": "a" },
                                    { "type": "STRING", "value": "b" }
                                ]
                            },
                            {
                                "type": "SEQ",
                                "members": [
                                    { "type": "STRING", "value": "c" },
                                    {
                                        "type": "CHOICE",
                                        "members": [
                                            { "type": "STRING", "value": "d" },
                                            { "type": "STRING", "value": "e" }
                                        ]
                                    }
                                ]
                            }
                        ]
                    },
                    { "type": "STRING", "value": "end" }
                ]
            }
        }
    });
    let r = build_json_ok(&g);
    assert!(r.build_stats.symbol_count > 0);
}

#[test]
fn complex_multi_rule_chain() {
    let g = GrammarBuilder::new("chain")
        .token("x", "x")
        .token("y", "y")
        .rule("inner", vec!["x"])
        .rule("middle", vec!["inner", "y"])
        .rule("outer", vec!["middle"])
        .start("outer")
        .build();
    let result = build_parser(g, test_opts()).unwrap();
    assert!(result.build_stats.state_count > 0);
}

#[test]
fn complex_prec_left_right_grammar() {
    let g = json!({
        "name": "prec_lr",
        "rules": {
            "source_file": { "type": "SYMBOL", "name": "expr" },
            "expr": {
                "type": "CHOICE",
                "members": [
                    {
                        "type": "PREC_LEFT",
                        "value": 1,
                        "content": {
                            "type": "SEQ",
                            "members": [
                                { "type": "SYMBOL", "name": "expr" },
                                { "type": "STRING", "value": "+" },
                                { "type": "SYMBOL", "name": "expr" }
                            ]
                        }
                    },
                    {
                        "type": "PREC_RIGHT",
                        "value": 2,
                        "content": {
                            "type": "SEQ",
                            "members": [
                                { "type": "SYMBOL", "name": "expr" },
                                { "type": "STRING", "value": "^" },
                                { "type": "SYMBOL", "name": "expr" }
                            ]
                        }
                    },
                    { "type": "PATTERN", "value": "[0-9]+" }
                ]
            }
        }
    });
    let r = build_json_ok(&g);
    assert!(r.build_stats.state_count > 0);
}

#[test]
fn complex_grammar_with_field_annotation() {
    let g = json!({
        "name": "field_grammar",
        "rules": {
            "source_file": { "type": "SYMBOL", "name": "assignment" },
            "assignment": {
                "type": "SEQ",
                "members": [
                    {
                        "type": "FIELD",
                        "name": "left",
                        "content": { "type": "PATTERN", "value": "[a-z]+" }
                    },
                    { "type": "STRING", "value": "=" },
                    {
                        "type": "FIELD",
                        "name": "right",
                        "content": { "type": "PATTERN", "value": "[0-9]+" }
                    }
                ]
            }
        }
    });
    let r = build_json_ok(&g);
    assert!(!r.parser_code.is_empty());
}

// ===========================================================================
// 8. Edge cases — minimal grammar, many rules, many tokens (10 tests)
// ===========================================================================

#[test]
fn edge_minimal_blank_rule() {
    let g = json!({
        "name": "blank_rule",
        "rules": {
            "source_file": { "type": "BLANK" }
        }
    });
    let r = build_json_ok(&g);
    assert!(!r.parser_code.is_empty());
}

#[test]
fn edge_grammar_with_many_tokens() {
    let mut g = GrammarBuilder::new("many_tokens");
    for i in 0..20 {
        let name = format!("t{i}");
        let pattern = format!("tok{i}");
        g = g.token(
            Box::leak(name.into_boxed_str()),
            Box::leak(pattern.into_boxed_str()),
        );
    }
    // Build a choice of all tokens
    let token_names: Vec<&str> = (0..20)
        .map(|i| {
            let s = format!("t{i}");
            &*Box::leak(s.into_boxed_str())
        })
        .collect();
    g = g.rule("s", vec![token_names[0]]);
    for &tn in &token_names[1..] {
        g = g.rule("s", vec![tn]);
    }
    g = g.start("s");
    let grammar = g.build();
    let result = build_parser(grammar, test_opts()).unwrap();
    assert!(result.build_stats.symbol_count >= 20);
}

#[test]
fn edge_grammar_with_many_rules_via_json() {
    let mut rules = serde_json::Map::new();
    let mut choice_members = Vec::new();
    for i in 0..15 {
        let rule_name = format!("rule_{i}");
        let val = format!("val{i}");
        rules.insert(rule_name.clone(), json!({ "type": "STRING", "value": val }));
        choice_members.push(json!({ "type": "SYMBOL", "name": rule_name }));
    }
    rules.insert(
        "source_file".to_string(),
        json!({ "type": "CHOICE", "members": choice_members }),
    );
    let g = json!({ "name": "many_rules", "rules": rules });
    let r = build_json_ok(&g);
    assert!(r.build_stats.state_count > 0);
}

#[test]
fn edge_single_token_grammar_from_ir() {
    let g = GrammarBuilder::new("single_tok")
        .token("EOF_MARKER", "EOF")
        .rule("s", vec!["EOF_MARKER"])
        .start("s")
        .build();
    let result = build_parser(g, test_opts()).unwrap();
    assert_eq!(result.grammar_name, "single_tok");
}

#[test]
fn edge_empty_extras_and_conflicts() {
    let g = json!({
        "name": "empty_meta",
        "extras": [],
        "conflicts": [],
        "rules": {
            "source_file": { "type": "STRING", "value": "z" }
        }
    });
    let r = build_json_ok(&g);
    assert!(!r.parser_code.is_empty());
}

#[test]
fn edge_grammar_name_with_numbers() {
    let g = json!({
        "name": "lang_v2_3",
        "rules": {
            "source_file": { "type": "STRING", "value": "v" }
        }
    });
    let r = build_json_ok(&g);
    assert_eq!(r.grammar_name, "lang_v2_3");
}

#[test]
fn edge_grammar_name_single_char() {
    let g = json!({
        "name": "z",
        "rules": {
            "source_file": { "type": "STRING", "value": "q" }
        }
    });
    let r = build_json_ok(&g);
    assert_eq!(r.grammar_name, "z");
}

#[test]
fn edge_repeat_inside_repeat() {
    let g = json!({
        "name": "nested_repeat",
        "rules": {
            "source_file": {
                "type": "REPEAT",
                "content": {
                    "type": "REPEAT1",
                    "content": { "type": "STRING", "value": "x" }
                }
            }
        }
    });
    let r = build_json_ok(&g);
    assert!(!r.parser_code.is_empty());
}

#[test]
fn edge_choice_with_blank_alt() {
    let g = json!({
        "name": "choice_blank",
        "rules": {
            "source_file": {
                "type": "CHOICE",
                "members": [
                    { "type": "STRING", "value": "present" },
                    { "type": "BLANK" }
                ]
            }
        }
    });
    let r = build_json_ok(&g);
    assert!(r.build_stats.state_count > 0);
}

#[test]
fn edge_invalid_json_string_fails() {
    let result = build_parser_from_json("not valid json".to_string(), test_opts());
    assert!(result.is_err());
}

// ===========================================================================
// Additional coverage — mixed scenarios (4 tests)
// ===========================================================================

#[test]
fn mixed_ir_and_json_produce_nonempty_code() {
    // IR path
    let ir_result = build_parser(simple_grammar(), test_opts()).unwrap();
    assert!(!ir_result.parser_code.is_empty());

    // JSON path
    let json_result = build_json_ok(&minimal_json());
    assert!(!json_result.parser_code.is_empty());
}

#[test]
fn different_grammars_produce_different_outputs() {
    let g1 = json!({
        "name": "diff_a",
        "rules": {
            "source_file": { "type": "STRING", "value": "aaa" }
        }
    });
    let g2 = json!({
        "name": "diff_b",
        "rules": {
            "source_file": { "type": "PATTERN", "value": "[0-9]+" }
        }
    });
    let r1 = build_json_ok(&g1);
    let r2 = build_json_ok(&g2);
    assert_ne!(r1.grammar_name, r2.grammar_name);
    assert_ne!(r1.parser_code, r2.parser_code);
}

#[test]
fn grammar_with_alias_builds() {
    let g = json!({
        "name": "alias_grammar",
        "rules": {
            "source_file": { "type": "SYMBOL", "name": "stmt" },
            "stmt": {
                "type": "ALIAS",
                "content": { "type": "PATTERN", "value": "[a-z]+" },
                "value": "identifier",
                "named": true
            }
        }
    });
    // #1076: ALIAS semantics are not admitted yet; reject loudly instead of
    // building a parser that dropped the aliased content.
    let result = build_parser_from_json(g.to_string(), test_opts());
    assert!(
        result.is_err(),
        "unsupported ALIAS must fail, got {result:?}"
    );
}

#[test]
fn grammar_with_token_wrapper_builds() {
    let g = json!({
        "name": "token_wrap",
        "rules": {
            "source_file": {
                "type": "TOKEN",
                "content": {
                    "type": "SEQ",
                    "members": [
                        { "type": "STRING", "value": "0" },
                        { "type": "STRING", "value": "x" },
                        { "type": "PATTERN", "value": "[0-9a-f]+" }
                    ]
                }
            }
        }
    });
    let r = build_json_ok(&g);
    assert!(r.build_stats.symbol_count > 0);
}

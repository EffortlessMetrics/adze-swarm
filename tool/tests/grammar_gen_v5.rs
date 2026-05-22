//! Grammar generation v5: 64 tests for grammar generation, JSON output,
//! and code generation in adze-tool.
//!
//! 8 categories × 8 tests = 64 tests:
//!   1. gen_basic_*           — basic grammar generation from IR and JSON
//!   2. gen_json_*            — JSON grammar output format and structure
//!   3. gen_code_*            — generated code structure
//!   4. gen_options_*         — build options variations
//!   5. gen_stats_*           — build statistics accuracy
//!   6. gen_errors_*          — error handling in generation
//!   7. gen_complex_*         — complex grammar generation
//!   8. gen_deterministic_*   — deterministic output verification

use adze_ir::builder::GrammarBuilder;
use adze_ir::{Associativity, Grammar};
use adze_tool::grammar_js::GrammarJsConverter;
use adze_tool::grammar_js::json_converter::from_tree_sitter_json;
use adze_tool::pure_rust_builder::{
    BuildOptions, BuildResult, build_parser, build_parser_from_json,
};
use serde_json::json;
use tempfile::TempDir;

// ===========================================================================
// Helpers
// ===========================================================================

fn tmp_opts() -> (TempDir, BuildOptions) {
    let dir = TempDir::new().unwrap();
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().to_string(),
        emit_artifacts: false,
        compress_tables: false,
    };
    (dir, opts)
}

fn tmp_opts_compressed() -> (TempDir, BuildOptions) {
    let dir = TempDir::new().unwrap();
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().to_string(),
        emit_artifacts: false,
        compress_tables: true,
    };
    (dir, opts)
}

fn tmp_opts_with_artifacts() -> (TempDir, BuildOptions) {
    let dir = TempDir::new().unwrap();
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().to_string(),
        emit_artifacts: true,
        compress_tables: false,
    };
    (dir, opts)
}

fn pattern_json(name: &str) -> String {
    json!({
        "name": name,
        "rules": {
            "source": {"type": "PATTERN", "value": "[a-z]+"}
        }
    })
    .to_string()
}

fn string_json(name: &str, value: &str) -> String {
    json!({
        "name": name,
        "rules": {
            "source": {"type": "STRING", "value": value}
        }
    })
    .to_string()
}

fn make_json(name: &str, rules: serde_json::Value) -> serde_json::Value {
    json!({ "name": name, "rules": rules })
}

/// Minimal IR grammar: one token, one rule, one start.
fn minimal_ir(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("a", "a")
        .rule("s", vec!["a"])
        .start("s")
        .build()
}

/// Two-alternative IR grammar.
fn two_alt_ir(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("x", "x")
        .token("y", "y")
        .rule("s", vec!["x"])
        .rule("s", vec!["y"])
        .start("s")
        .build()
}

/// Sequence IR grammar: s → a b c.
fn seq_ir(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .rule("s", vec!["a", "b", "c"])
        .start("s")
        .build()
}

/// Expression grammar with one binary operator.
fn expr_ir(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("num", "0")
        .token("plus", "+")
        .rule_with_precedence("expr", vec!["expr", "plus", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["num"])
        .start("expr")
        .build()
}

/// Chain grammar: s → inner → leaf.
fn chain_ir(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("leaf", "leaf")
        .rule("inner", vec!["leaf"])
        .rule("s", vec!["inner"])
        .start("s")
        .build()
}

fn do_build(grammar: Grammar) -> BuildResult {
    let (_dir, opts) = tmp_opts();
    build_parser(grammar, opts).expect("build should succeed")
}

// ===========================================================================
// 1. gen_basic_* — basic grammar generation (8 tests)
// ===========================================================================

#[test]
fn gen_basic_single_token_ir() {
    let result = do_build(minimal_ir("gg_basic1"));
    assert_eq!(result.grammar_name, "gg_basic1");
    assert!(!result.parser_code.is_empty());
}

#[test]
fn gen_basic_two_alternatives_ir() {
    let result = do_build(two_alt_ir("gg_basic2"));
    assert_eq!(result.grammar_name, "gg_basic2");
    assert!(result.build_stats.symbol_count >= 2);
}

#[test]
fn gen_basic_sequence_ir() {
    let result = do_build(seq_ir("gg_basic3"));
    assert_eq!(result.grammar_name, "gg_basic3");
    assert!(result.build_stats.state_count > 0);
}

#[test]
fn gen_basic_chain_ir() {
    let result = do_build(chain_ir("gg_basic4"));
    assert_eq!(result.grammar_name, "gg_basic4");
    assert!(!result.parser_code.is_empty());
}

#[test]
fn gen_basic_pattern_json_builds() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser_from_json(pattern_json("gg_basic5"), opts).unwrap();
    assert_eq!(result.grammar_name, "gg_basic5");
}

#[test]
fn gen_basic_string_json_builds() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser_from_json(string_json("gg_basic6", "hello"), opts).unwrap();
    assert_eq!(result.grammar_name, "gg_basic6");
}

#[test]
fn gen_basic_symbol_reference_json() {
    let (_dir, opts) = tmp_opts();
    let grammar_json = json!({
        "name": "gg_basic7",
        "rules": {
            "source": {
                "type": "CHOICE",
                "members": [
                    {"type": "SYMBOL", "name": "word"},
                    {"type": "SYMBOL", "name": "num"}
                ]
            },
            "word": {"type": "PATTERN", "value": "[a-z]+"},
            "num": {"type": "PATTERN", "value": "[0-9]+"}
        }
    })
    .to_string();
    let result = build_parser_from_json(grammar_json, opts).unwrap();
    assert_eq!(result.grammar_name, "gg_basic7");
}

#[test]
fn gen_basic_expr_ir_builds() {
    let result = do_build(expr_ir("gg_basic8"));
    assert_eq!(result.grammar_name, "gg_basic8");
    assert!(result.build_stats.symbol_count >= 2);
}

// ===========================================================================
// 2. gen_json_* — JSON grammar output format (8 tests)
// ===========================================================================

#[test]
fn gen_json_node_types_is_valid_json() {
    let result = do_build(minimal_ir("gg_json1"));
    let parsed: serde_json::Value = serde_json::from_str(&result.node_types_json).unwrap();
    assert!(parsed.is_array());
}

#[test]
fn gen_json_node_types_entries_are_objects() {
    let result = do_build(minimal_ir("gg_json2"));
    let parsed: serde_json::Value = serde_json::from_str(&result.node_types_json).unwrap();
    for entry in parsed.as_array().unwrap() {
        assert!(entry.is_object());
    }
}

#[test]
fn gen_json_node_types_entries_have_type_field() {
    let result = do_build(minimal_ir("gg_json3"));
    let parsed: serde_json::Value = serde_json::from_str(&result.node_types_json).unwrap();
    for entry in parsed.as_array().unwrap() {
        assert!(
            entry.get("type").is_some(),
            "node type entry missing 'type' field: {entry}"
        );
    }
}

#[test]
fn gen_json_node_types_type_is_string() {
    let result = do_build(minimal_ir("gg_json4"));
    let parsed: serde_json::Value = serde_json::from_str(&result.node_types_json).unwrap();
    for entry in parsed.as_array().unwrap() {
        if let Some(ty) = entry.get("type") {
            assert!(ty.is_string(), "type field must be a string: {ty}");
        }
    }
}

#[test]
fn gen_json_from_tree_sitter_preserves_name() {
    let value = make_json(
        "gg_json5",
        json!({ "source": {"type": "STRING", "value": "hi"} }),
    );
    let gjs = from_tree_sitter_json(&value).unwrap();
    assert_eq!(gjs.name, "gg_json5");
}

#[test]
fn gen_json_from_tree_sitter_preserves_rules() {
    let value = make_json(
        "gg_json6",
        json!({
            "alpha": {"type": "STRING", "value": "a"},
            "beta": {"type": "PATTERN", "value": "[b]+"}
        }),
    );
    let gjs = from_tree_sitter_json(&value).unwrap();
    assert_eq!(gjs.rules.len(), 2);
}

#[test]
fn gen_json_conversion_roundtrip_to_ir() {
    let value = make_json(
        "gg_json7",
        json!({ "source": {"type": "PATTERN", "value": "\\d+"} }),
    );
    let gjs = from_tree_sitter_json(&value).unwrap();
    let grammar = GrammarJsConverter::new(gjs).convert().unwrap();
    assert!(!grammar.tokens.is_empty());
}

#[test]
fn gen_json_seq_rule_produces_multiple_rhs_symbols() {
    let value = make_json(
        "gg_json8",
        json!({
            "source": {
                "type": "SEQ",
                "members": [
                    {"type": "STRING", "value": "a"},
                    {"type": "STRING", "value": "b"},
                    {"type": "STRING", "value": "c"}
                ]
            }
        }),
    );
    let gjs = from_tree_sitter_json(&value).unwrap();
    let grammar = GrammarJsConverter::new(gjs).convert().unwrap();
    // The source rule should have at least one production with 3 RHS symbols
    let has_seq_rule = grammar
        .rules
        .values()
        .any(|rules| rules.iter().any(|r| r.rhs.len() >= 3));
    assert!(has_seq_rule, "expected a rule with 3+ RHS symbols from SEQ");
}

// ===========================================================================
// 3. gen_code_* — generated code structure (8 tests)
// ===========================================================================

#[test]
fn gen_code_nonempty_for_minimal() {
    let result = do_build(minimal_ir("gg_code1"));
    assert!(
        !result.parser_code.is_empty(),
        "minimal grammar should produce non-empty parser code"
    );
}

#[test]
fn gen_code_contains_grammar_name_constant() {
    let result = do_build(minimal_ir("gg_code2"));
    assert!(
        result.parser_code.contains("GRAMMAR_NAME") || result.parser_code.contains("gg_code2"),
        "parser code should reference grammar name"
    );
}

#[test]
fn gen_code_nonempty_for_chain() {
    let result = do_build(chain_ir("gg_code3"));
    assert!(
        !result.parser_code.is_empty(),
        "chain grammar should produce non-empty code"
    );
}

#[test]
fn gen_code_nonempty_for_expr() {
    let result = do_build(expr_ir("gg_code4"));
    assert!(
        !result.parser_code.is_empty(),
        "expr grammar should produce non-empty code"
    );
}

#[test]
fn gen_code_length_scales_with_tokens() {
    let small = do_build(minimal_ir("gg_code5a"));
    let grammar_big = GrammarBuilder::new("gg_code5b")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .token("d", "d")
        .token("e", "e")
        .rule("s", vec!["a"])
        .rule("s", vec!["b"])
        .rule("s", vec!["c"])
        .rule("s", vec!["d"])
        .rule("s", vec!["e"])
        .start("s")
        .build();
    let big = do_build(grammar_big);
    assert!(
        big.parser_code.len() >= small.parser_code.len(),
        "larger grammar should produce at least as much code"
    );
}

#[test]
fn gen_code_parser_path_ends_with_rs() {
    let result = do_build(minimal_ir("gg_code6"));
    assert!(
        result.parser_path.ends_with(".rs"),
        "parser path should end with .rs: {}",
        result.parser_path
    );
}

#[test]
fn gen_code_parser_path_contains_grammar_name() {
    let result = do_build(minimal_ir("gg_code7"));
    assert!(
        result.parser_path.contains("gg_code7"),
        "parser path should contain grammar name: {}",
        result.parser_path
    );
}

#[test]
fn gen_code_language_code_from_compressed() {
    let (_dir, opts) = tmp_opts_compressed();
    let result = build_parser(minimal_ir("gg_code8"), opts).unwrap();
    assert!(
        !result.parser_code.is_empty(),
        "compressed build should also produce parser code"
    );
}

// ===========================================================================
// 4. gen_options_* — build options variations (8 tests)
// ===========================================================================

#[test]
fn gen_options_default_compress_is_true() {
    let opts = BuildOptions::default();
    assert!(opts.compress_tables);
}

#[test]
fn gen_options_default_emit_artifacts_is_false() {
    let opts = BuildOptions::default();
    assert!(!opts.emit_artifacts);
}

#[test]
fn gen_options_default_out_dir_nonempty() {
    let opts = BuildOptions::default();
    assert!(!opts.out_dir.is_empty());
}

#[test]
fn gen_options_custom_fields() {
    let opts = BuildOptions {
        out_dir: "/gg/custom".to_string(),
        emit_artifacts: true,
        compress_tables: false,
    };
    assert_eq!(opts.out_dir, "/gg/custom");
    assert!(opts.emit_artifacts);
    assert!(!opts.compress_tables);
}

#[test]
fn gen_options_clone_preserves() {
    let opts = BuildOptions {
        out_dir: "/gg/clonable".to_string(),
        emit_artifacts: true,
        compress_tables: true,
    };
    let cloned = opts.clone();
    assert_eq!(cloned.out_dir, "/gg/clonable");
    assert!(cloned.emit_artifacts);
    assert!(cloned.compress_tables);
}

#[test]
fn gen_options_compress_and_no_compress_both_succeed() {
    let (_d1, o1) = tmp_opts_compressed();
    let (_d2, o2) = tmp_opts();
    let r1 = build_parser(minimal_ir("gg_opt6a"), o1).unwrap();
    let r2 = build_parser(minimal_ir("gg_opt6b"), o2).unwrap();
    assert!(!r1.parser_code.is_empty());
    assert!(!r2.parser_code.is_empty());
}

#[test]
fn gen_options_emit_artifacts_writes_files() {
    let (_dir, opts) = tmp_opts_with_artifacts();
    let result = build_parser(minimal_ir("gg_opt7"), opts).unwrap();
    let parser_path = std::path::Path::new(&result.parser_path);
    assert!(
        parser_path.exists(),
        "emit_artifacts should write parser file"
    );
}

#[test]
fn gen_options_stats_consistent_across_compress_modes() {
    let (_d1, o1) = tmp_opts_compressed();
    let (_d2, o2) = tmp_opts();
    let r1 = build_parser(minimal_ir("gg_opt8a"), o1).unwrap();
    let r2 = build_parser(minimal_ir("gg_opt8b"), o2).unwrap();
    assert_eq!(r1.build_stats.state_count, r2.build_stats.state_count);
    assert_eq!(r1.build_stats.symbol_count, r2.build_stats.symbol_count);
}

// ===========================================================================
// 5. gen_stats_* — build statistics accuracy (8 tests)
// ===========================================================================

#[test]
fn gen_stats_state_count_positive_minimal() {
    let result = do_build(minimal_ir("gg_st1"));
    assert!(result.build_stats.state_count > 0);
}

#[test]
fn gen_stats_symbol_count_positive_minimal() {
    let result = do_build(minimal_ir("gg_st2"));
    assert!(result.build_stats.symbol_count > 0);
}

#[test]
fn gen_stats_symbol_count_at_least_token_count() {
    let grammar = two_alt_ir("gg_st3");
    let token_count = grammar.tokens.len();
    let result = do_build(grammar);
    assert!(
        result.build_stats.symbol_count >= token_count,
        "symbol_count {} < token_count {}",
        result.build_stats.symbol_count,
        token_count,
    );
}

#[test]
fn gen_stats_conflict_cells_bounded() {
    let result = do_build(minimal_ir("gg_st4"));
    let upper = result.build_stats.state_count * result.build_stats.symbol_count;
    assert!(
        result.build_stats.conflict_cells <= upper,
        "conflict_cells {} > state*symbol {}",
        result.build_stats.conflict_cells,
        upper,
    );
}

#[test]
fn gen_stats_more_alternatives_more_symbols() {
    let r1 = do_build(minimal_ir("gg_st5a"));
    let r2 = do_build(two_alt_ir("gg_st5b"));
    assert!(
        r2.build_stats.symbol_count >= r1.build_stats.symbol_count,
        "two-alt {} should have >= symbols than minimal {}",
        r2.build_stats.symbol_count,
        r1.build_stats.symbol_count,
    );
}

#[test]
fn gen_stats_expr_grammar_has_positive_counts() {
    let result = do_build(expr_ir("gg_st6"));
    assert!(result.build_stats.state_count > 0);
    assert!(result.build_stats.symbol_count > 0);
}

#[test]
fn gen_stats_debug_format_contains_all_fields() {
    let result = do_build(minimal_ir("gg_st7"));
    let debug_str = format!("{:?}", result.build_stats);
    assert!(debug_str.contains("state_count"));
    assert!(debug_str.contains("symbol_count"));
    assert!(debug_str.contains("conflict_cells"));
}

#[test]
fn gen_stats_json_build_has_positive_counts() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser_from_json(pattern_json("gg_st8"), opts).unwrap();
    assert!(result.build_stats.state_count > 0);
    assert!(result.build_stats.symbol_count > 0);
}

// ===========================================================================
// 6. gen_errors_* — error handling in generation (8 tests)
// ===========================================================================

#[test]
fn gen_errors_empty_string() {
    let (_dir, opts) = tmp_opts();
    assert!(build_parser_from_json(String::new(), opts).is_err());
}

#[test]
fn gen_errors_malformed_json() {
    let (_dir, opts) = tmp_opts();
    assert!(build_parser_from_json("{bad json".into(), opts).is_err());
}

#[test]
fn gen_errors_json_array_input() {
    let (_dir, opts) = tmp_opts();
    assert!(build_parser_from_json("[]".into(), opts).is_err());
}

#[test]
fn gen_errors_json_number_input() {
    let (_dir, opts) = tmp_opts();
    assert!(build_parser_from_json("42".into(), opts).is_err());
}

#[test]
fn gen_errors_json_null_input() {
    let (_dir, opts) = tmp_opts();
    assert!(build_parser_from_json("null".into(), opts).is_err());
}

#[test]
fn gen_errors_json_boolean_input() {
    let (_dir, opts) = tmp_opts();
    assert!(build_parser_from_json("true".into(), opts).is_err());
}

#[test]
fn gen_errors_missing_rules_key() {
    let (_dir, opts) = tmp_opts();
    let grammar_json = json!({"name": "gg_err7"}).to_string();
    assert!(build_parser_from_json(grammar_json, opts).is_err());
}

#[test]
fn gen_errors_rules_not_object() {
    let (_dir, opts) = tmp_opts();
    let grammar_json = json!({
        "name": "gg_err8",
        "rules": "not_an_object"
    })
    .to_string();
    assert!(build_parser_from_json(grammar_json, opts).is_err());
}

// ===========================================================================
// 7. gen_complex_* — complex grammar generation (8 tests)
// ===========================================================================

#[test]
fn gen_complex_arithmetic_ir() {
    let grammar = GrammarBuilder::new("gg_cx1")
        .token("num", "0")
        .token("plus", "+")
        .token("star", "*")
        .rule_with_precedence("expr", vec!["expr", "plus", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "star", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["num"])
        .start("expr")
        .build();
    let result = do_build(grammar);
    assert_eq!(result.grammar_name, "gg_cx1");
    assert!(result.build_stats.symbol_count >= 3);
}

#[test]
fn gen_complex_nested_seq_in_choice_json() {
    let (_dir, opts) = tmp_opts();
    let grammar_json = json!({
        "name": "gg_cx2",
        "rules": {
            "source": {
                "type": "CHOICE",
                "members": [
                    {
                        "type": "SEQ",
                        "members": [
                            {"type": "STRING", "value": "if"},
                            {"type": "SYMBOL", "name": "cond"}
                        ]
                    },
                    {"type": "STRING", "value": "else"}
                ]
            },
            "cond": {"type": "PATTERN", "value": "[a-z]+"}
        }
    })
    .to_string();
    let result = build_parser_from_json(grammar_json, opts).unwrap();
    assert_eq!(result.grammar_name, "gg_cx2");
}

#[test]
fn gen_complex_repeat_inside_seq_json() {
    let (_dir, opts) = tmp_opts();
    let grammar_json = json!({
        "name": "gg_cx3",
        "rules": {
            "source": {
                "type": "SEQ",
                "members": [
                    {"type": "STRING", "value": "["},
                    {"type": "REPEAT", "content": {"type": "SYMBOL", "name": "item"}},
                    {"type": "STRING", "value": "]"}
                ]
            },
            "item": {"type": "PATTERN", "value": "[a-z]+"}
        }
    })
    .to_string();
    let result = build_parser_from_json(grammar_json, opts).unwrap();
    assert_eq!(result.grammar_name, "gg_cx3");
}

#[test]
fn gen_complex_json_like_value_types() {
    let (_dir, opts) = tmp_opts();
    let grammar_json = json!({
        "name": "gg_cx4",
        "rules": {
            "source": {"type": "SYMBOL", "name": "value"},
            "value": {
                "type": "CHOICE",
                "members": [
                    {"type": "SYMBOL", "name": "object_lit"},
                    {"type": "SYMBOL", "name": "array_lit"},
                    {"type": "SYMBOL", "name": "atom"}
                ]
            },
            "object_lit": {
                "type": "SEQ",
                "members": [
                    {"type": "STRING", "value": "{"},
                    {"type": "STRING", "value": "}"}
                ]
            },
            "array_lit": {
                "type": "SEQ",
                "members": [
                    {"type": "STRING", "value": "["},
                    {"type": "STRING", "value": "]"}
                ]
            },
            "atom": {"type": "PATTERN", "value": "[a-z0-9]+"}
        }
    })
    .to_string();
    let result = build_parser_from_json(grammar_json, opts).unwrap();
    assert_eq!(result.grammar_name, "gg_cx4");
    assert!(result.build_stats.symbol_count >= 4);
}

#[test]
fn gen_complex_deep_chain_ir() {
    let mut builder = GrammarBuilder::new("gg_cx5");
    builder = builder.token("leaf", "leaf");
    let depth = 8;
    let names: Vec<String> = (0..depth).map(|i| format!("n{i}")).collect();
    builder = builder.rule(&names[0], vec!["leaf"]);
    for i in 1..depth {
        builder = builder.rule(&names[i], vec![&names[i - 1]]);
    }
    builder = builder.start(&names[depth - 1]);
    let grammar = builder.build();
    let result = do_build(grammar);
    assert_eq!(result.grammar_name, "gg_cx5");
    assert!(result.build_stats.state_count > 0);
}

#[test]
fn gen_complex_many_alternatives_ir() {
    let count = 10;
    let mut builder = GrammarBuilder::new("gg_cx6");
    let names: Vec<String> = (0..count).map(|i| format!("tok{i}")).collect();
    for name in &names {
        builder = builder.token(name, name);
    }
    for name in &names {
        builder = builder.rule("s", vec![name]);
    }
    builder = builder.start("s");
    let grammar = builder.build();
    let result = do_build(grammar);
    assert_eq!(result.grammar_name, "gg_cx6");
    assert!(result.build_stats.symbol_count >= count);
}

#[test]
fn gen_complex_mixed_precedence_ir() {
    let grammar = GrammarBuilder::new("gg_cx7")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .rule_with_precedence("s", vec!["a"], 1, Associativity::Left)
        .rule_with_precedence("s", vec!["b"], 2, Associativity::Right)
        .rule("s", vec!["c"])
        .start("s")
        .build();
    let result = do_build(grammar);
    assert!(result.build_stats.symbol_count >= 3);
}

#[test]
fn gen_complex_operator_pairs_ir() {
    let grammar = GrammarBuilder::new("gg_cx8")
        .token("plus", "+")
        .token("minus", "-")
        .token("n", "0")
        .rule("s", vec!["n", "plus", "n"])
        .rule("s", vec!["n", "minus", "n"])
        .start("s")
        .build();
    let result = do_build(grammar);
    assert_eq!(result.grammar_name, "gg_cx8");
    assert!(!result.parser_code.is_empty());
}

// ===========================================================================
// 8. gen_deterministic_* — deterministic output verification (8 tests)
// ===========================================================================

#[test]
fn gen_deterministic_parser_code_from_ir() {
    let (_d1, o1) = tmp_opts();
    let (_d2, o2) = tmp_opts();
    let r1 = build_parser(minimal_ir("gg_det1"), o1).unwrap();
    let r2 = build_parser(minimal_ir("gg_det1"), o2).unwrap();
    assert_eq!(r1.parser_code, r2.parser_code);
}

#[test]
fn gen_deterministic_node_types_from_ir() {
    let (_d1, o1) = tmp_opts();
    let (_d2, o2) = tmp_opts();
    let r1 = build_parser(minimal_ir("gg_det2"), o1).unwrap();
    let r2 = build_parser(minimal_ir("gg_det2"), o2).unwrap();
    assert_eq!(r1.node_types_json, r2.node_types_json);
}

#[test]
fn gen_deterministic_stats_from_ir() {
    let (_d1, o1) = tmp_opts();
    let (_d2, o2) = tmp_opts();
    let r1 = build_parser(minimal_ir("gg_det3"), o1).unwrap();
    let r2 = build_parser(minimal_ir("gg_det3"), o2).unwrap();
    assert_eq!(r1.build_stats.state_count, r2.build_stats.state_count);
    assert_eq!(r1.build_stats.symbol_count, r2.build_stats.symbol_count);
    assert_eq!(r1.build_stats.conflict_cells, r2.build_stats.conflict_cells);
}

#[test]
fn gen_deterministic_json_roundtrip() {
    let (_d1, o1) = tmp_opts();
    let (_d2, o2) = tmp_opts();
    let r1 = build_parser_from_json(pattern_json("gg_det4"), o1).unwrap();
    let r2 = build_parser_from_json(pattern_json("gg_det4"), o2).unwrap();
    assert_eq!(r1.parser_code, r2.parser_code);
    assert_eq!(r1.node_types_json, r2.node_types_json);
}

#[test]
fn gen_deterministic_chain_code() {
    let (_d1, o1) = tmp_opts();
    let (_d2, o2) = tmp_opts();
    let r1 = build_parser(chain_ir("gg_det5"), o1).unwrap();
    let r2 = build_parser(chain_ir("gg_det5"), o2).unwrap();
    assert_eq!(r1.parser_code, r2.parser_code);
}

#[test]
fn gen_deterministic_expr_code() {
    let (_d1, o1) = tmp_opts();
    let (_d2, o2) = tmp_opts();
    let r1 = build_parser(expr_ir("gg_det6"), o1).unwrap();
    let r2 = build_parser(expr_ir("gg_det6"), o2).unwrap();
    assert_eq!(r1.parser_code, r2.parser_code);
}

#[test]
fn gen_deterministic_two_alt_stats() {
    let (_d1, o1) = tmp_opts();
    let (_d2, o2) = tmp_opts();
    let r1 = build_parser(two_alt_ir("gg_det7"), o1).unwrap();
    let r2 = build_parser(two_alt_ir("gg_det7"), o2).unwrap();
    assert_eq!(r1.build_stats.state_count, r2.build_stats.state_count);
    assert_eq!(r1.build_stats.symbol_count, r2.build_stats.symbol_count);
}

#[test]
fn gen_deterministic_compressed_vs_uncompressed_stats() {
    let (_d1, o1) = tmp_opts_compressed();
    let (_d2, o2) = tmp_opts();
    let r1 = build_parser(seq_ir("gg_det8a"), o1).unwrap();
    let r2 = build_parser(seq_ir("gg_det8b"), o2).unwrap();
    // Stats (state_count, symbol_count) should match regardless of compression
    assert_eq!(r1.build_stats.state_count, r2.build_stats.state_count);
    assert_eq!(r1.build_stats.symbol_count, r2.build_stats.symbol_count);
}

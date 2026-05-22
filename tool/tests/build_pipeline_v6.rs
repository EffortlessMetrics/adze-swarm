//! Build-pipeline v6 integration tests for adze-tool.
//!
//! 64 tests covering the full build pipeline from Grammar to generated output:
//!   1. pipeline_basic_*          — basic pipeline execution (8 tests)
//!   2. pipeline_json_*           — JSON-based pipeline (8 tests)
//!   3. pipeline_stats_*          — statistics from pipeline (8 tests)
//!   4. pipeline_output_*         — output format verification (8 tests)
//!   5. pipeline_complex_*        — complex grammar pipelines (8 tests)
//!   6. pipeline_error_*          — error cases in pipeline (8 tests)
//!   7. pipeline_deterministic_*  — deterministic pipeline (8 tests)
//!   8. pipeline_integration_*    — full integration tests (8 tests)

use adze_ir::Associativity;
use adze_ir::builder::GrammarBuilder;
use adze_tool::pure_rust_builder::{BuildOptions, build_parser, build_parser_from_json};
use tempfile::TempDir;

// ── Helpers ──────────────────────────────────────────────────────────────

fn make_opts() -> (TempDir, BuildOptions) {
    let dir = TempDir::new().unwrap();
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().to_string(),
        emit_artifacts: false,
        compress_tables: true,
    };
    (dir, opts)
}

fn make_opts_no_compress() -> (TempDir, BuildOptions) {
    let dir = TempDir::new().unwrap();
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().to_string(),
        emit_artifacts: false,
        compress_tables: false,
    };
    (dir, opts)
}

fn make_opts_with_artifacts() -> (TempDir, BuildOptions) {
    let dir = TempDir::new().unwrap();
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().to_string(),
        emit_artifacts: true,
        compress_tables: true,
    };
    (dir, opts)
}

fn minimal_grammar(name: &str) -> adze_ir::Grammar {
    GrammarBuilder::new(name)
        .token("tok", "x")
        .rule("root", vec!["tok"])
        .start("root")
        .build()
}

fn two_token_grammar(name: &str) -> adze_ir::Grammar {
    GrammarBuilder::new(name)
        .token("a", "a")
        .token("b", "b")
        .rule("root", vec!["a", "b"])
        .start("root")
        .build()
}

fn alt_grammar(name: &str) -> adze_ir::Grammar {
    GrammarBuilder::new(name)
        .token("a", "a")
        .token("b", "b")
        .rule("root", vec!["a"])
        .rule("root", vec!["b"])
        .start("root")
        .build()
}

fn chain_grammar(name: &str) -> adze_ir::Grammar {
    GrammarBuilder::new(name)
        .token("leaf", "z")
        .rule("mid", vec!["leaf"])
        .rule("root", vec!["mid"])
        .start("root")
        .build()
}

fn pattern_json(name: &str) -> String {
    serde_json::json!({
        "name": name,
        "rules": {
            "source": {"type": "PATTERN", "value": "[a-z]+"}
        }
    })
    .to_string()
}

fn string_json(name: &str, value: &str) -> String {
    serde_json::json!({
        "name": name,
        "rules": {
            "source": {"type": "STRING", "value": value}
        }
    })
    .to_string()
}

fn seq_json(name: &str) -> String {
    serde_json::json!({
        "name": name,
        "rules": {
            "source": {
                "type": "SEQ",
                "members": [
                    {"type": "STRING", "value": "hello"},
                    {"type": "STRING", "value": "world"}
                ]
            }
        }
    })
    .to_string()
}

fn choice_json(name: &str) -> String {
    serde_json::json!({
        "name": name,
        "rules": {
            "source": {
                "type": "CHOICE",
                "members": [
                    {"type": "PATTERN", "value": "[a-z]+"},
                    {"type": "PATTERN", "value": "[0-9]+"}
                ]
            }
        }
    })
    .to_string()
}

// =========================================================================
// 1. pipeline_basic_* — basic pipeline execution (8 tests)
// =========================================================================

#[test]
fn pipeline_basic_single_token_builds() {
    let (_dir, opts) = make_opts();
    let result = build_parser(minimal_grammar("basic_single_v6"), opts).unwrap();
    assert_eq!(result.grammar_name, "basic_single_v6");
}

#[test]
fn pipeline_basic_two_token_sequence() {
    let (_dir, opts) = make_opts();
    let result = build_parser(two_token_grammar("basic_seq_v6"), opts).unwrap();
    assert_eq!(result.grammar_name, "basic_seq_v6");
    assert!(!result.parser_code.is_empty());
}

#[test]
fn pipeline_basic_alternation() {
    let (_dir, opts) = make_opts();
    let result = build_parser(alt_grammar("basic_alt_v6"), opts).unwrap();
    assert_eq!(result.grammar_name, "basic_alt_v6");
}

#[test]
fn pipeline_basic_chain_rule() {
    let (_dir, opts) = make_opts();
    let result = build_parser(chain_grammar("basic_chain_v6"), opts).unwrap();
    assert_eq!(result.grammar_name, "basic_chain_v6");
    assert!(result.build_stats.state_count > 0);
}

#[test]
fn pipeline_basic_parser_code_nonempty() {
    let (_dir, opts) = make_opts();
    let result = build_parser(minimal_grammar("basic_code_v6"), opts).unwrap();
    assert!(!result.parser_code.is_empty());
}

#[test]
fn pipeline_basic_node_types_nonempty() {
    let (_dir, opts) = make_opts();
    let result = build_parser(minimal_grammar("basic_nt_v6"), opts).unwrap();
    assert!(!result.node_types_json.is_empty());
}

#[test]
fn pipeline_basic_parser_path_nonempty() {
    let (_dir, opts) = make_opts();
    let result = build_parser(minimal_grammar("basic_path_v6"), opts).unwrap();
    assert!(!result.parser_path.is_empty());
}

#[test]
fn pipeline_basic_with_extra_whitespace_token() {
    let grammar = GrammarBuilder::new("basic_ws_v6")
        .token("word", "[a-z]+")
        .token("ws", "\\s+")
        .rule("root", vec!["word"])
        .start("root")
        .extra("ws")
        .build();
    let (_dir, opts) = make_opts();
    let result = build_parser(grammar, opts).unwrap();
    assert_eq!(result.grammar_name, "basic_ws_v6");
}

// =========================================================================
// 2. pipeline_json_* — JSON-based pipeline (8 tests)
// =========================================================================

#[test]
fn pipeline_json_pattern_rule() {
    let (_dir, opts) = make_opts();
    let result = build_parser_from_json(pattern_json("json_pat_v6"), opts).unwrap();
    assert_eq!(result.grammar_name, "json_pat_v6");
}

#[test]
fn pipeline_json_string_rule() {
    let (_dir, opts) = make_opts();
    let result = build_parser_from_json(string_json("json_str_v6", "hello"), opts).unwrap();
    assert_eq!(result.grammar_name, "json_str_v6");
}

#[test]
fn pipeline_json_seq_rule() {
    let (_dir, opts) = make_opts();
    let result = build_parser_from_json(seq_json("json_seq_v6"), opts).unwrap();
    assert_eq!(result.grammar_name, "json_seq_v6");
}

#[test]
fn pipeline_json_choice_rule() {
    let (_dir, opts) = make_opts();
    let result = build_parser_from_json(choice_json("json_choice_v6"), opts).unwrap();
    assert_eq!(result.grammar_name, "json_choice_v6");
}

#[test]
fn pipeline_json_repeat_rule() {
    let (_dir, opts) = make_opts();
    let json = serde_json::json!({
        "name": "json_rep_v6",
        "rules": {
            "source": {
                "type": "REPEAT",
                "content": {"type": "PATTERN", "value": "[a-z]+"}
            }
        }
    })
    .to_string();
    let result = build_parser_from_json(json, opts).unwrap();
    assert_eq!(result.grammar_name, "json_rep_v6");
}

#[test]
fn pipeline_json_repeat1_rule() {
    let (_dir, opts) = make_opts();
    let json = serde_json::json!({
        "name": "json_rep1_v6",
        "rules": {
            "source": {
                "type": "REPEAT1",
                "content": {"type": "STRING", "value": "x"}
            }
        }
    })
    .to_string();
    let result = build_parser_from_json(json, opts).unwrap();
    assert_eq!(result.grammar_name, "json_rep1_v6");
}

#[test]
fn pipeline_json_symbol_reference() {
    let (_dir, opts) = make_opts();
    let json = serde_json::json!({
        "name": "json_sym_v6",
        "rules": {
            "source": {"type": "SYMBOL", "name": "item"},
            "item": {"type": "PATTERN", "value": "[a-z]+"}
        }
    })
    .to_string();
    let result = build_parser_from_json(json, opts).unwrap();
    assert_eq!(result.grammar_name, "json_sym_v6");
}

#[test]
fn pipeline_json_optional_via_choice_blank() {
    let (_dir, opts) = make_opts();
    let json = serde_json::json!({
        "name": "json_opt_v6",
        "rules": {
            "source": {
                "type": "SEQ",
                "members": [
                    {"type": "STRING", "value": "start"},
                    {
                        "type": "CHOICE",
                        "members": [
                            {"type": "STRING", "value": "opt"},
                            {"type": "BLANK"}
                        ]
                    }
                ]
            }
        }
    })
    .to_string();
    let result = build_parser_from_json(json, opts).unwrap();
    assert_eq!(result.grammar_name, "json_opt_v6");
}

// =========================================================================
// 3. pipeline_stats_* — statistics from pipeline (8 tests)
// =========================================================================

#[test]
fn pipeline_stats_state_count_positive() {
    let (_dir, opts) = make_opts();
    let result = build_parser(minimal_grammar("stats_sc_v6"), opts).unwrap();
    assert!(result.build_stats.state_count > 0);
}

#[test]
fn pipeline_stats_symbol_count_positive() {
    let (_dir, opts) = make_opts();
    let result = build_parser(minimal_grammar("stats_sym_v6"), opts).unwrap();
    assert!(result.build_stats.symbol_count > 0);
}

#[test]
fn pipeline_stats_symbol_count_at_least_two() {
    let (_dir, opts) = make_opts();
    let result = build_parser(minimal_grammar("stats_min2_v6"), opts).unwrap();
    // token + EOF at minimum
    assert!(result.build_stats.symbol_count >= 2);
}

#[test]
fn pipeline_stats_alt_has_more_symbols_than_single() {
    let (_d1, o1) = make_opts();
    let (_d2, o2) = make_opts();
    let r_single = build_parser(minimal_grammar("stats_cmp1_v6"), o1).unwrap();
    let r_alt = build_parser(alt_grammar("stats_cmp2_v6"), o2).unwrap();
    assert!(r_alt.build_stats.symbol_count >= r_single.build_stats.symbol_count);
}

#[test]
fn pipeline_stats_seq_state_count_positive() {
    let (_dir, opts) = make_opts();
    let result = build_parser(two_token_grammar("stats_seqst_v6"), opts).unwrap();
    assert!(result.build_stats.state_count > 0);
}

#[test]
fn pipeline_stats_conflict_cells_accessible() {
    let (_dir, opts) = make_opts();
    let result = build_parser(minimal_grammar("stats_conf_v6"), opts).unwrap();
    // Just verify it is accessible (usize, always >= 0)
    let _cells = result.build_stats.conflict_cells;
}

#[test]
fn pipeline_stats_consistent_across_compress_modes() {
    let (_d1, o1) = make_opts();
    let (_d2, o2) = make_opts_no_compress();
    let r_comp = build_parser(minimal_grammar("stats_cc1_v6"), o1).unwrap();
    let r_nocomp = build_parser(minimal_grammar("stats_cc2_v6"), o2).unwrap();
    assert_eq!(
        r_comp.build_stats.state_count,
        r_nocomp.build_stats.state_count
    );
    assert_eq!(
        r_comp.build_stats.symbol_count,
        r_nocomp.build_stats.symbol_count
    );
}

#[test]
fn pipeline_stats_debug_format_contains_fields() {
    let (_dir, opts) = make_opts();
    let result = build_parser(minimal_grammar("stats_dbg_v6"), opts).unwrap();
    let dbg = format!("{:?}", result.build_stats);
    assert!(dbg.contains("state_count"));
    assert!(dbg.contains("symbol_count"));
    assert!(dbg.contains("conflict_cells"));
}

// =========================================================================
// 4. pipeline_output_* — output format verification (8 tests)
// =========================================================================

#[test]
fn pipeline_output_node_types_is_json_array() {
    let (_dir, opts) = make_opts();
    let result = build_parser(minimal_grammar("out_arr_v6"), opts).unwrap();
    let val: serde_json::Value = serde_json::from_str(&result.node_types_json).unwrap();
    assert!(val.is_array());
}

#[test]
fn pipeline_output_node_types_entries_are_objects() {
    let (_dir, opts) = make_opts();
    let result = build_parser(minimal_grammar("out_obj_v6"), opts).unwrap();
    let val: serde_json::Value = serde_json::from_str(&result.node_types_json).unwrap();
    for entry in val.as_array().unwrap() {
        assert!(entry.is_object());
    }
}

#[test]
fn pipeline_output_node_types_have_type_field() {
    let (_dir, opts) = make_opts();
    let result = build_parser(minimal_grammar("out_type_v6"), opts).unwrap();
    let val: serde_json::Value = serde_json::from_str(&result.node_types_json).unwrap();
    for entry in val.as_array().unwrap() {
        assert!(entry.get("type").is_some());
    }
}

#[test]
fn pipeline_output_parser_path_contains_grammar_name() {
    let (_dir, opts) = make_opts();
    let result = build_parser(minimal_grammar("out_name_v6"), opts).unwrap();
    assert!(result.parser_path.contains("out_name_v6"));
}

#[test]
fn pipeline_output_debug_format_includes_name() {
    let (_dir, opts) = make_opts();
    let result = build_parser(minimal_grammar("out_dbg_v6"), opts).unwrap();
    let dbg = format!("{result:?}");
    assert!(dbg.contains("out_dbg_v6"));
}

#[test]
fn pipeline_output_emit_artifacts_creates_file() {
    let (_dir, opts) = make_opts_with_artifacts();
    let result = build_parser(minimal_grammar("out_emit_v6"), opts).unwrap();
    let parser_path = std::path::Path::new(&result.parser_path);
    assert!(parser_path.exists());
}

#[test]
fn pipeline_output_json_pipeline_has_parser_code() {
    let (_dir, opts) = make_opts();
    let result = build_parser_from_json(pattern_json("out_jpc_v6"), opts).unwrap();
    assert!(!result.parser_code.is_empty());
}

#[test]
fn pipeline_output_json_pipeline_has_node_types() {
    let (_dir, opts) = make_opts();
    let result = build_parser_from_json(pattern_json("out_jnt_v6"), opts).unwrap();
    let val: serde_json::Value = serde_json::from_str(&result.node_types_json).unwrap();
    assert!(val.is_array());
}

// =========================================================================
// 5. pipeline_complex_* — complex grammar pipelines (8 tests)
// =========================================================================

#[test]
fn pipeline_complex_arithmetic_via_ir() {
    let grammar = GrammarBuilder::new("cx_arith_v6")
        .token("num", "0")
        .token("plus", "+")
        .token("star", "*")
        .rule_with_precedence("expr", vec!["expr", "plus", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "star", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["num"])
        .start("expr")
        .build();
    let (_dir, opts) = make_opts();
    let result = build_parser(grammar, opts).unwrap();
    assert_eq!(result.grammar_name, "cx_arith_v6");
    assert!(result.build_stats.symbol_count >= 3);
}

#[test]
fn pipeline_complex_deep_chain_via_ir() {
    let mut builder = GrammarBuilder::new("cx_deep_v6");
    builder = builder.token("leaf", "z");
    let depth = 10;
    let names: Vec<String> = (0..depth).map(|i| format!("lv{i}")).collect();
    builder = builder.rule(&names[0], vec!["leaf"]);
    for i in 1..depth {
        builder = builder.rule(&names[i], vec![&names[i - 1]]);
    }
    builder = builder.start(&names[depth - 1]);
    let grammar = builder.build();

    let (_dir, opts) = make_opts();
    let result = build_parser(grammar, opts).unwrap();
    assert_eq!(result.grammar_name, "cx_deep_v6");
    assert!(result.build_stats.state_count > 0);
}

#[test]
fn pipeline_complex_many_alternatives_via_ir() {
    let count = 10;
    let mut builder = GrammarBuilder::new("cx_manyalt_v6");
    let names: Vec<String> = (0..count).map(|i| format!("tok{i}")).collect();
    for name in &names {
        builder = builder.token(name, name);
    }
    for name in &names {
        builder = builder.rule("root", vec![name]);
    }
    builder = builder.start("root");
    let grammar = builder.build();

    let (_dir, opts) = make_opts();
    let result = build_parser(grammar, opts).unwrap();
    assert_eq!(result.grammar_name, "cx_manyalt_v6");
    assert!(result.build_stats.symbol_count >= count);
}

#[test]
fn pipeline_complex_nested_seq_in_choice_json() {
    let (_dir, opts) = make_opts();
    let json = serde_json::json!({
        "name": "cx_nestsc_v6",
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
    let result = build_parser_from_json(json, opts).unwrap();
    assert_eq!(result.grammar_name, "cx_nestsc_v6");
}

#[test]
fn pipeline_complex_repeat_inside_seq_json() {
    let (_dir, opts) = make_opts();
    let json = serde_json::json!({
        "name": "cx_repseq_v6",
        "rules": {
            "source": {
                "type": "SEQ",
                "members": [
                    {"type": "STRING", "value": "["},
                    {
                        "type": "REPEAT",
                        "content": {"type": "SYMBOL", "name": "item"}
                    },
                    {"type": "STRING", "value": "]"}
                ]
            },
            "item": {"type": "PATTERN", "value": "[a-z]+"}
        }
    })
    .to_string();
    let result = build_parser_from_json(json, opts).unwrap();
    assert_eq!(result.grammar_name, "cx_repseq_v6");
}

#[test]
fn pipeline_complex_mixed_precedence_via_ir() {
    let grammar = GrammarBuilder::new("cx_mixp_v6")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .rule_with_precedence("root", vec!["a"], 1, Associativity::Left)
        .rule_with_precedence("root", vec!["b"], 2, Associativity::Right)
        .rule("root", vec!["c"])
        .start("root")
        .build();
    let (_dir, opts) = make_opts();
    let result = build_parser(grammar, opts).unwrap();
    assert!(result.build_stats.symbol_count > 0);
}

#[test]
fn pipeline_complex_json_like_structure() {
    let (_dir, opts) = make_opts();
    let json = serde_json::json!({
        "name": "cx_jlike_v6",
        "rules": {
            "source": {"type": "SYMBOL", "name": "value"},
            "value": {
                "type": "CHOICE",
                "members": [
                    {"type": "SYMBOL", "name": "obj"},
                    {"type": "SYMBOL", "name": "arr"},
                    {"type": "SYMBOL", "name": "prim"}
                ]
            },
            "obj": {
                "type": "SEQ",
                "members": [
                    {"type": "STRING", "value": "{"},
                    {"type": "STRING", "value": "}"}
                ]
            },
            "arr": {
                "type": "SEQ",
                "members": [
                    {"type": "STRING", "value": "["},
                    {"type": "STRING", "value": "]"}
                ]
            },
            "prim": {"type": "PATTERN", "value": "[a-z0-9]+"}
        }
    })
    .to_string();
    let result = build_parser_from_json(json, opts).unwrap();
    assert_eq!(result.grammar_name, "cx_jlike_v6");
    assert!(result.build_stats.symbol_count >= 4);
}

#[test]
fn pipeline_complex_prec_left_json() {
    let (_dir, opts) = make_opts();
    let json = serde_json::json!({
        "name": "cx_pleft_v6",
        "rules": {
            "source": {
                "type": "PREC_LEFT",
                "value": 1,
                "content": {"type": "PATTERN", "value": "[a-z]+"}
            }
        }
    })
    .to_string();
    let result = build_parser_from_json(json, opts).unwrap();
    assert_eq!(result.grammar_name, "cx_pleft_v6");
}

// =========================================================================
// 6. pipeline_error_* — error cases in pipeline (8 tests)
// =========================================================================

#[test]
fn pipeline_error_empty_string() {
    let (_dir, opts) = make_opts();
    assert!(build_parser_from_json(String::new(), opts).is_err());
}

#[test]
fn pipeline_error_malformed_json() {
    let (_dir, opts) = make_opts();
    assert!(build_parser_from_json("{bad json".into(), opts).is_err());
}

#[test]
fn pipeline_error_json_array_input() {
    let (_dir, opts) = make_opts();
    assert!(build_parser_from_json("[]".into(), opts).is_err());
}

#[test]
fn pipeline_error_json_number_input() {
    let (_dir, opts) = make_opts();
    assert!(build_parser_from_json("42".into(), opts).is_err());
}

#[test]
fn pipeline_error_json_null_input() {
    let (_dir, opts) = make_opts();
    assert!(build_parser_from_json("null".into(), opts).is_err());
}

#[test]
fn pipeline_error_json_boolean_input() {
    let (_dir, opts) = make_opts();
    assert!(build_parser_from_json("true".into(), opts).is_err());
}

#[test]
fn pipeline_error_json_missing_rules_key() {
    let (_dir, opts) = make_opts();
    let json = serde_json::json!({"name": "norules_v6"}).to_string();
    assert!(build_parser_from_json(json, opts).is_err());
}

#[test]
fn pipeline_error_json_empty_rules_object() {
    let (_dir, opts) = make_opts();
    let json = serde_json::json!({
        "name": "emptyrules_v6",
        "rules": {}
    })
    .to_string();
    assert!(build_parser_from_json(json, opts).is_err());
}

// =========================================================================
// 7. pipeline_deterministic_* — deterministic pipeline (8 tests)
// =========================================================================

#[test]
fn pipeline_deterministic_parser_code_from_ir() {
    let (_d1, o1) = make_opts();
    let (_d2, o2) = make_opts();
    let r1 = build_parser(minimal_grammar("det_pc_v6"), o1).unwrap();
    let r2 = build_parser(minimal_grammar("det_pc_v6"), o2).unwrap();
    assert_eq!(r1.parser_code, r2.parser_code);
}

#[test]
fn pipeline_deterministic_node_types_from_ir() {
    let (_d1, o1) = make_opts();
    let (_d2, o2) = make_opts();
    let r1 = build_parser(minimal_grammar("det_nt_v6"), o1).unwrap();
    let r2 = build_parser(minimal_grammar("det_nt_v6"), o2).unwrap();
    assert_eq!(r1.node_types_json, r2.node_types_json);
}

#[test]
fn pipeline_deterministic_stats_from_ir() {
    let (_d1, o1) = make_opts();
    let (_d2, o2) = make_opts();
    let r1 = build_parser(minimal_grammar("det_st_v6"), o1).unwrap();
    let r2 = build_parser(minimal_grammar("det_st_v6"), o2).unwrap();
    assert_eq!(r1.build_stats.state_count, r2.build_stats.state_count);
    assert_eq!(r1.build_stats.symbol_count, r2.build_stats.symbol_count);
    assert_eq!(r1.build_stats.conflict_cells, r2.build_stats.conflict_cells);
}

#[test]
fn pipeline_deterministic_chain_grammar_code() {
    let (_d1, o1) = make_opts();
    let (_d2, o2) = make_opts();
    let r1 = build_parser(chain_grammar("det_ch_v6"), o1).unwrap();
    let r2 = build_parser(chain_grammar("det_ch_v6"), o2).unwrap();
    assert_eq!(r1.parser_code, r2.parser_code);
}

#[test]
fn pipeline_deterministic_alt_grammar_code() {
    let (_d1, o1) = make_opts();
    let (_d2, o2) = make_opts();
    let r1 = build_parser(alt_grammar("det_alt_v6"), o1).unwrap();
    let r2 = build_parser(alt_grammar("det_alt_v6"), o2).unwrap();
    assert_eq!(r1.parser_code, r2.parser_code);
}

#[test]
fn pipeline_deterministic_json_roundtrip_code() {
    let (_d1, o1) = make_opts();
    let (_d2, o2) = make_opts();
    let r1 = build_parser_from_json(pattern_json("det_jr_v6"), o1).unwrap();
    let r2 = build_parser_from_json(pattern_json("det_jr_v6"), o2).unwrap();
    assert_eq!(r1.parser_code, r2.parser_code);
}

#[test]
fn pipeline_deterministic_json_roundtrip_node_types() {
    let (_d1, o1) = make_opts();
    let (_d2, o2) = make_opts();
    let r1 = build_parser_from_json(pattern_json("det_jnt_v6"), o1).unwrap();
    let r2 = build_parser_from_json(pattern_json("det_jnt_v6"), o2).unwrap();
    assert_eq!(r1.node_types_json, r2.node_types_json);
}

#[test]
fn pipeline_deterministic_json_roundtrip_stats() {
    let (_d1, o1) = make_opts();
    let (_d2, o2) = make_opts();
    let r1 = build_parser_from_json(pattern_json("det_jst_v6"), o1).unwrap();
    let r2 = build_parser_from_json(pattern_json("det_jst_v6"), o2).unwrap();
    assert_eq!(r1.build_stats.state_count, r2.build_stats.state_count);
    assert_eq!(r1.build_stats.symbol_count, r2.build_stats.symbol_count);
}

// =========================================================================
// 8. pipeline_integration_* — full integration tests (8 tests)
// =========================================================================

#[test]
fn pipeline_integration_ir_to_parser_to_stats() {
    let (_dir, opts) = make_opts();
    let grammar = GrammarBuilder::new("int_full_v6")
        .token("num", "[0-9]+")
        .token("plus", "+")
        .rule("expr", vec!["num"])
        .rule("expr", vec!["expr", "plus", "num"])
        .start("expr")
        .build();
    let result = build_parser(grammar, opts).unwrap();
    assert_eq!(result.grammar_name, "int_full_v6");
    assert!(!result.parser_code.is_empty());
    assert!(result.build_stats.state_count > 0);
    assert!(result.build_stats.symbol_count >= 2);
}

#[test]
fn pipeline_integration_json_to_parser_to_stats() {
    let (_dir, opts) = make_opts();
    let json = serde_json::json!({
        "name": "int_json_v6",
        "rules": {
            "source": {
                "type": "SEQ",
                "members": [
                    {"type": "STRING", "value": "begin"},
                    {"type": "SYMBOL", "name": "body"},
                    {"type": "STRING", "value": "end"}
                ]
            },
            "body": {"type": "PATTERN", "value": "[a-z]+"}
        }
    })
    .to_string();
    let result = build_parser_from_json(json, opts).unwrap();
    assert_eq!(result.grammar_name, "int_json_v6");
    assert!(!result.parser_code.is_empty());
    assert!(result.build_stats.state_count > 0);
}

#[test]
fn pipeline_integration_no_compress_produces_valid_output() {
    let (_dir, opts) = make_opts_no_compress();
    let result = build_parser(minimal_grammar("int_nocomp_v6"), opts).unwrap();
    assert!(!result.parser_code.is_empty());
    let val: serde_json::Value = serde_json::from_str(&result.node_types_json).unwrap();
    assert!(val.is_array());
}

#[test]
fn pipeline_integration_artifacts_mode_creates_file() {
    let (_dir, opts) = make_opts_with_artifacts();
    let result = build_parser(minimal_grammar("int_artif_v6"), opts).unwrap();
    let path = std::path::Path::new(&result.parser_path);
    assert!(path.exists());
    let contents = std::fs::read_to_string(path).unwrap();
    assert!(!contents.is_empty());
}

#[test]
fn pipeline_integration_multiple_grammars_independent() {
    let (_d1, o1) = make_opts();
    let (_d2, o2) = make_opts();
    let r1 = build_parser(minimal_grammar("int_indep1_v6"), o1).unwrap();
    let r2 = build_parser(two_token_grammar("int_indep2_v6"), o2).unwrap();
    assert_ne!(r1.grammar_name, r2.grammar_name);
    assert_ne!(r1.parser_code, r2.parser_code);
}

#[test]
fn pipeline_integration_build_options_default_compress() {
    let opts = BuildOptions::default();
    assert!(opts.compress_tables);
    assert!(!opts.emit_artifacts);
    assert!(!opts.out_dir.is_empty());
}

#[test]
fn pipeline_integration_ir_and_json_both_produce_stats() {
    let (_d1, o1) = make_opts();
    let (_d2, o2) = make_opts();
    let r_ir = build_parser(minimal_grammar("int_both1_v6"), o1).unwrap();
    let r_json = build_parser_from_json(pattern_json("int_both2_v6"), o2).unwrap();
    assert!(r_ir.build_stats.state_count > 0);
    assert!(r_json.build_stats.state_count > 0);
}

#[test]
fn pipeline_integration_complex_grammar_full_roundtrip() {
    let grammar = GrammarBuilder::new("int_rt_v6")
        .token("id", "[a-z]+")
        .token("num", "[0-9]+")
        .token("eq", "=")
        .token("semi", ";")
        .rule("stmt", vec!["id", "eq", "num", "semi"])
        .rule("root", vec!["stmt"])
        .start("root")
        .build();
    let (_dir, opts) = make_opts();
    let result = build_parser(grammar, opts).unwrap();
    assert_eq!(result.grammar_name, "int_rt_v6");
    assert!(!result.parser_code.is_empty());
    assert!(!result.node_types_json.is_empty());
    assert!(result.build_stats.state_count > 0);
    assert!(result.build_stats.symbol_count >= 4);
}

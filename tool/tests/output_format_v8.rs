//! Comprehensive tests for output format validation of adze-tool build results.
//!
//! 80+ tests covering:
//!   1. parser_code is non-empty string
//!   2. parser_code starts with known prefix
//!   3. node_types_json starts with "["
//!   4. node_types_json ends with "]"
//!   5. node_types_json parseable by serde_json
//!   6. node_types_json::Value is Array
//!   7. Each entry in node_types has "type" key
//!   8. Each entry has "named" key (bool)
//!   9. node_types "type" values are strings
//!  10. node_types "named" values are booleans
//!  11. parser_code contains no null bytes
//!  12. parser_code is valid UTF-8 (guaranteed by String)
//!  13. node_types contains at least 1 entry
//!  14. BuildStats Debug format is readable
//!  15. parser_code length > 100 for any grammar
//!  16. Various grammar sizes → all produce valid format
//!  17. Format consistent across option combinations
//!  18. Compressed vs uncompressed → both valid format
//!  19. Emitted vs non-emitted → both valid format
//!  20. node_types entries have consistent structure

use adze_ir::builder::GrammarBuilder;
use adze_ir::{Associativity, Grammar};
use adze_tool::pure_rust_builder::{BuildOptions, BuildResult, build_parser};
use serde_json::Value;
use tempfile::TempDir;

// ── Helpers ──────────────────────────────────────────────────────────────

fn opts() -> (TempDir, BuildOptions) {
    let dir = TempDir::new().expect("tmpdir");
    let o = BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: true,
    };
    (dir, o)
}

fn opts_uncompressed() -> (TempDir, BuildOptions) {
    let dir = TempDir::new().expect("tmpdir");
    let o = BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: false,
    };
    (dir, o)
}

fn opts_emit() -> (TempDir, BuildOptions) {
    let dir = TempDir::new().expect("tmpdir");
    let o = BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: true,
        compress_tables: true,
    };
    (dir, o)
}

fn opts_emit_uncompressed() -> (TempDir, BuildOptions) {
    let dir = TempDir::new().expect("tmpdir");
    let o = BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: true,
        compress_tables: false,
    };
    (dir, o)
}

fn build(g: Grammar) -> BuildResult {
    let (_dir, o) = opts();
    build_parser(g, o).expect("build should succeed")
}

fn build_with(g: Grammar, dir_opts: (TempDir, BuildOptions)) -> BuildResult {
    let (_dir, o) = dir_opts;
    build_parser(g, o).expect("build should succeed")
}

fn single_token(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("x", "x")
        .rule("start", vec!["x"])
        .start("start")
        .build()
}

fn two_token(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("a", "a")
        .token("b", "b")
        .rule("start", vec!["a", "b"])
        .start("start")
        .build()
}

fn three_token(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .rule("start", vec!["a", "b", "c"])
        .start("start")
        .build()
}

fn alt_grammar(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("a", "a")
        .token("b", "b")
        .rule("start", vec!["a"])
        .rule("start", vec!["b"])
        .start("start")
        .build()
}

fn chain_grammar(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("x", "x")
        .rule("c", vec!["x"])
        .rule("b", vec!["c"])
        .rule("start", vec!["b"])
        .start("start")
        .build()
}

fn left_prec_grammar(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("num", r"\d+")
        .token("plus", r"\+")
        .rule_with_precedence("expr", vec!["expr", "plus", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["num"])
        .rule("start", vec!["expr"])
        .start("start")
        .build()
}

fn right_prec_grammar(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("num", r"\d+")
        .token("eq", "=")
        .rule_with_precedence("expr", vec!["expr", "eq", "expr"], 1, Associativity::Right)
        .rule("expr", vec!["num"])
        .rule("start", vec!["expr"])
        .start("start")
        .build()
}

fn none_prec_grammar(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("num", r"\d+")
        .token("lt", "<")
        .rule_with_precedence("expr", vec!["expr", "lt", "expr"], 1, Associativity::None)
        .rule("expr", vec!["num"])
        .rule("start", vec!["expr"])
        .start("start")
        .build()
}

fn extras_grammar(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("word", r"[a-z]+")
        .token("ws", r"[ \t]+")
        .rule("start", vec!["word"])
        .extra("ws")
        .start("start")
        .build()
}

fn five_token(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .token("d", "d")
        .token("e", "e")
        .rule("start", vec!["a", "b", "c", "d", "e"])
        .start("start")
        .build()
}

fn arithmetic_grammar(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("num", r"\d+")
        .token("plus", r"\+")
        .token("star", r"\*")
        .token("lparen", r"\(")
        .token("rparen", r"\)")
        .rule_with_precedence("expr", vec!["expr", "plus", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "star", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["lparen", "expr", "rparen"])
        .rule("expr", vec!["num"])
        .rule("start", vec!["expr"])
        .start("start")
        .build()
}

fn parse_node_types(json: &str) -> Vec<Value> {
    let v: Value = serde_json::from_str(json).expect("valid JSON");
    v.as_array().expect("should be array").clone()
}

fn assert_valid_node_types_format(json: &str) {
    assert!(json.starts_with('['), "node_types_json must start with '['");
    assert!(json.ends_with(']'), "node_types_json must end with ']'");
    let entries = parse_node_types(json);
    assert!(!entries.is_empty(), "node_types must have at least 1 entry");
    for entry in &entries {
        let obj = entry.as_object().expect("each entry should be an object");
        assert!(obj.contains_key("type"), "entry missing 'type' key");
        assert!(obj.contains_key("named"), "entry missing 'named' key");
        assert!(obj["type"].is_string(), "'type' must be a string");
        assert!(obj["named"].is_boolean(), "'named' must be a boolean");
    }
}

fn assert_valid_parser_code(code: &str) {
    assert!(!code.is_empty(), "parser_code must not be empty");
    assert!(
        !code.contains('\0'),
        "parser_code must not contain null bytes"
    );
    assert!(code.len() > 100, "parser_code must be >100 chars");
}

// ═════════════════════════════════════════════════════════════════════════
// 1. parser_code — non-empty
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_parser_code_nonempty_single_token() {
    let r = build(single_token("of_v8_pc_nonempty_1"));
    assert!(!r.parser_code.is_empty());
}

#[test]
fn test_parser_code_nonempty_two_token() {
    let r = build(two_token("of_v8_pc_nonempty_2"));
    assert!(!r.parser_code.is_empty());
}

#[test]
fn test_parser_code_nonempty_alt() {
    let r = build(alt_grammar("of_v8_pc_nonempty_alt"));
    assert!(!r.parser_code.is_empty());
}

#[test]
fn test_parser_code_nonempty_chain() {
    let r = build(chain_grammar("of_v8_pc_nonempty_chain"));
    assert!(!r.parser_code.is_empty());
}

#[test]
fn test_parser_code_nonempty_arithmetic() {
    let r = build(arithmetic_grammar("of_v8_pc_nonempty_arith"));
    assert!(!r.parser_code.is_empty());
}

// ═════════════════════════════════════════════════════════════════════════
// 2. parser_code — starts with known prefix
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_parser_code_starts_with_known_prefix_single() {
    let r = build(single_token("of_v8_prefix_1"));
    let trimmed = r.parser_code.trim_start();
    assert!(
        trimmed.starts_with("//")
            || trimmed.starts_with("use ")
            || trimmed.starts_with("pub ")
            || trimmed.starts_with("#[")
            || trimmed.starts_with("mod ")
            || trimmed.starts_with("const ")
            || trimmed.starts_with("static "),
        "parser_code should start with a known Rust construct, got: {:?}",
        &trimmed[..trimmed.len().min(60)]
    );
}

#[test]
fn test_parser_code_starts_with_known_prefix_arith() {
    let r = build(arithmetic_grammar("of_v8_prefix_arith"));
    let trimmed = r.parser_code.trim_start();
    assert!(
        trimmed.starts_with("//")
            || trimmed.starts_with("use ")
            || trimmed.starts_with("pub ")
            || trimmed.starts_with("#[")
            || trimmed.starts_with("mod ")
            || trimmed.starts_with("const ")
            || trimmed.starts_with("static "),
        "parser_code should start with a known Rust construct"
    );
}

// ═════════════════════════════════════════════════════════════════════════
// 3–4. node_types_json — starts with "[" and ends with "]"
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_node_types_starts_with_bracket_single() {
    let r = build(single_token("of_v8_bracket_s_1"));
    assert!(r.node_types_json.starts_with('['));
}

#[test]
fn test_node_types_starts_with_bracket_two() {
    let r = build(two_token("of_v8_bracket_s_2"));
    assert!(r.node_types_json.starts_with('['));
}

#[test]
fn test_node_types_ends_with_bracket_single() {
    let r = build(single_token("of_v8_bracket_e_1"));
    assert!(r.node_types_json.ends_with(']'));
}

#[test]
fn test_node_types_ends_with_bracket_two() {
    let r = build(two_token("of_v8_bracket_e_2"));
    assert!(r.node_types_json.ends_with(']'));
}

#[test]
fn test_node_types_brackets_chain() {
    let r = build(chain_grammar("of_v8_bracket_chain"));
    assert!(r.node_types_json.starts_with('['));
    assert!(r.node_types_json.ends_with(']'));
}

#[test]
fn test_node_types_brackets_arithmetic() {
    let r = build(arithmetic_grammar("of_v8_bracket_arith"));
    assert!(r.node_types_json.starts_with('['));
    assert!(r.node_types_json.ends_with(']'));
}

// ═════════════════════════════════════════════════════════════════════════
// 5–6. node_types_json — parseable and is Array
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_node_types_parseable_single() {
    let r = build(single_token("of_v8_parse_1"));
    let v: Value = serde_json::from_str(&r.node_types_json).expect("valid JSON");
    assert!(v.is_array());
}

#[test]
fn test_node_types_parseable_two() {
    let r = build(two_token("of_v8_parse_2"));
    let v: Value = serde_json::from_str(&r.node_types_json).expect("valid JSON");
    assert!(v.is_array());
}

#[test]
fn test_node_types_parseable_alt() {
    let r = build(alt_grammar("of_v8_parse_alt"));
    let v: Value = serde_json::from_str(&r.node_types_json).expect("valid JSON");
    assert!(v.is_array());
}

#[test]
fn test_node_types_parseable_chain() {
    let r = build(chain_grammar("of_v8_parse_chain"));
    let v: Value = serde_json::from_str(&r.node_types_json).expect("valid JSON");
    assert!(v.is_array());
}

#[test]
fn test_node_types_parseable_left_prec() {
    let r = build(left_prec_grammar("of_v8_parse_lp"));
    let v: Value = serde_json::from_str(&r.node_types_json).expect("valid JSON");
    assert!(v.is_array());
}

#[test]
fn test_node_types_parseable_right_prec() {
    let r = build(right_prec_grammar("of_v8_parse_rp"));
    let v: Value = serde_json::from_str(&r.node_types_json).expect("valid JSON");
    assert!(v.is_array());
}

#[test]
fn test_node_types_parseable_arith() {
    let r = build(arithmetic_grammar("of_v8_parse_arith"));
    let v: Value = serde_json::from_str(&r.node_types_json).expect("valid JSON");
    assert!(v.is_array());
}

// ═════════════════════════════════════════════════════════════════════════
// 7–10. node_types entries — "type" and "named" keys
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_node_types_entries_have_type_key_single() {
    let r = build(single_token("of_v8_type_key_1"));
    for entry in parse_node_types(&r.node_types_json) {
        assert!(entry.as_object().unwrap().contains_key("type"));
    }
}

#[test]
fn test_node_types_entries_have_type_key_arith() {
    let r = build(arithmetic_grammar("of_v8_type_key_arith"));
    for entry in parse_node_types(&r.node_types_json) {
        assert!(entry.as_object().unwrap().contains_key("type"));
    }
}

#[test]
fn test_node_types_entries_have_named_key_single() {
    let r = build(single_token("of_v8_named_key_1"));
    for entry in parse_node_types(&r.node_types_json) {
        assert!(entry.as_object().unwrap().contains_key("named"));
    }
}

#[test]
fn test_node_types_entries_have_named_key_arith() {
    let r = build(arithmetic_grammar("of_v8_named_key_arith"));
    for entry in parse_node_types(&r.node_types_json) {
        assert!(entry.as_object().unwrap().contains_key("named"));
    }
}

#[test]
fn test_node_types_type_values_are_strings_single() {
    let r = build(single_token("of_v8_type_str_1"));
    for entry in parse_node_types(&r.node_types_json) {
        assert!(entry["type"].is_string());
    }
}

#[test]
fn test_node_types_type_values_are_strings_two() {
    let r = build(two_token("of_v8_type_str_2"));
    for entry in parse_node_types(&r.node_types_json) {
        assert!(entry["type"].is_string());
    }
}

#[test]
fn test_node_types_named_values_are_bools_single() {
    let r = build(single_token("of_v8_named_bool_1"));
    for entry in parse_node_types(&r.node_types_json) {
        assert!(entry["named"].is_boolean());
    }
}

#[test]
fn test_node_types_named_values_are_bools_chain() {
    let r = build(chain_grammar("of_v8_named_bool_ch"));
    for entry in parse_node_types(&r.node_types_json) {
        assert!(entry["named"].is_boolean());
    }
}

#[test]
fn test_node_types_type_values_nonempty_strings() {
    let r = build(arithmetic_grammar("of_v8_type_nonempty"));
    for entry in parse_node_types(&r.node_types_json) {
        let t = entry["type"].as_str().unwrap();
        assert!(!t.is_empty(), "type value must not be empty");
    }
}

#[test]
fn test_node_types_all_entries_are_objects() {
    let r = build(five_token("of_v8_entries_obj"));
    for entry in parse_node_types(&r.node_types_json) {
        assert!(entry.is_object(), "each node_types entry must be an object");
    }
}

// ═════════════════════════════════════════════════════════════════════════
// 11–12. parser_code — no null bytes, valid UTF-8
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_parser_code_no_null_bytes_single() {
    let r = build(single_token("of_v8_null_1"));
    assert!(!r.parser_code.contains('\0'));
}

#[test]
fn test_parser_code_no_null_bytes_arith() {
    let r = build(arithmetic_grammar("of_v8_null_arith"));
    assert!(!r.parser_code.contains('\0'));
}

#[test]
fn test_parser_code_no_null_bytes_chain() {
    let r = build(chain_grammar("of_v8_null_chain"));
    assert!(!r.parser_code.contains('\0'));
}

#[test]
fn test_parser_code_valid_utf8_single() {
    // String type guarantees UTF-8, but verify no replacement chars
    let r = build(single_token("of_v8_utf8_1"));
    assert!(!r.parser_code.contains('\u{FFFD}'));
}

#[test]
fn test_parser_code_valid_utf8_arith() {
    let r = build(arithmetic_grammar("of_v8_utf8_arith"));
    assert!(!r.parser_code.contains('\u{FFFD}'));
}

// ═════════════════════════════════════════════════════════════════════════
// 13. node_types — at least 1 entry
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_node_types_nonempty_single() {
    let r = build(single_token("of_v8_nt_ne_1"));
    let entries = parse_node_types(&r.node_types_json);
    assert!(!entries.is_empty());
}

#[test]
fn test_node_types_nonempty_two() {
    let r = build(two_token("of_v8_nt_ne_2"));
    let entries = parse_node_types(&r.node_types_json);
    assert!(!entries.is_empty());
}

#[test]
fn test_node_types_nonempty_chain() {
    let r = build(chain_grammar("of_v8_nt_ne_ch"));
    let entries = parse_node_types(&r.node_types_json);
    assert!(!entries.is_empty());
}

#[test]
fn test_node_types_nonempty_alt() {
    let r = build(alt_grammar("of_v8_nt_ne_alt"));
    let entries = parse_node_types(&r.node_types_json);
    assert!(!entries.is_empty());
}

#[test]
fn test_node_types_nonempty_arith() {
    let r = build(arithmetic_grammar("of_v8_nt_ne_arith"));
    let entries = parse_node_types(&r.node_types_json);
    assert!(!entries.is_empty());
}

// ═════════════════════════════════════════════════════════════════════════
// 14. BuildStats Debug format
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_build_stats_debug_format_contains_state_count() {
    let r = build(single_token("of_v8_dbg_sc"));
    let dbg = format!("{:?}", r.build_stats);
    assert!(dbg.contains("state_count"));
}

#[test]
fn test_build_stats_debug_format_contains_symbol_count() {
    let r = build(single_token("of_v8_dbg_sym"));
    let dbg = format!("{:?}", r.build_stats);
    assert!(dbg.contains("symbol_count"));
}

#[test]
fn test_build_stats_debug_format_contains_conflict_cells() {
    let r = build(single_token("of_v8_dbg_cc"));
    let dbg = format!("{:?}", r.build_stats);
    assert!(dbg.contains("conflict_cells"));
}

#[test]
fn test_build_stats_debug_is_nonempty() {
    let r = build(single_token("of_v8_dbg_ne"));
    let dbg = format!("{:?}", r.build_stats);
    assert!(!dbg.is_empty());
}

#[test]
fn test_build_stats_debug_contains_buildstats() {
    let r = build(single_token("of_v8_dbg_name"));
    let dbg = format!("{:?}", r.build_stats);
    assert!(dbg.contains("BuildStats"));
}

#[test]
fn test_build_result_debug_format_readable() {
    let r = build(single_token("of_v8_result_dbg"));
    let dbg = format!("{:?}", r);
    assert!(dbg.contains("BuildResult"));
    assert!(dbg.contains("grammar_name"));
}

// ═════════════════════════════════════════════════════════════════════════
// 15. parser_code length > 100
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_parser_code_length_gt_100_single() {
    let r = build(single_token("of_v8_len_1"));
    assert!(r.parser_code.len() > 100);
}

#[test]
fn test_parser_code_length_gt_100_two() {
    let r = build(two_token("of_v8_len_2"));
    assert!(r.parser_code.len() > 100);
}

#[test]
fn test_parser_code_length_gt_100_chain() {
    let r = build(chain_grammar("of_v8_len_ch"));
    assert!(r.parser_code.len() > 100);
}

#[test]
fn test_parser_code_length_gt_100_arith() {
    let r = build(arithmetic_grammar("of_v8_len_arith"));
    assert!(r.parser_code.len() > 100);
}

#[test]
fn test_parser_code_length_gt_100_left_prec() {
    let r = build(left_prec_grammar("of_v8_len_lp"));
    assert!(r.parser_code.len() > 100);
}

// ═════════════════════════════════════════════════════════════════════════
// 16. Various grammar sizes → all produce valid format
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_full_format_valid_single_token() {
    let r = build(single_token("of_v8_full_1"));
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

#[test]
fn test_full_format_valid_two_token() {
    let r = build(two_token("of_v8_full_2"));
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

#[test]
fn test_full_format_valid_three_token() {
    let r = build(three_token("of_v8_full_3"));
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

#[test]
fn test_full_format_valid_five_token() {
    let r = build(five_token("of_v8_full_5"));
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

#[test]
fn test_full_format_valid_alt() {
    let r = build(alt_grammar("of_v8_full_alt"));
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

#[test]
fn test_full_format_valid_chain() {
    let r = build(chain_grammar("of_v8_full_chain"));
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

#[test]
fn test_full_format_valid_left_prec() {
    let r = build(left_prec_grammar("of_v8_full_lp"));
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

#[test]
fn test_full_format_valid_right_prec() {
    let r = build(right_prec_grammar("of_v8_full_rp"));
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

#[test]
fn test_full_format_valid_none_prec() {
    let r = build(none_prec_grammar("of_v8_full_np"));
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

#[test]
fn test_full_format_valid_extras() {
    let r = build(extras_grammar("of_v8_full_ext"));
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

#[test]
fn test_full_format_valid_arithmetic() {
    let r = build(arithmetic_grammar("of_v8_full_arith"));
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

// ═════════════════════════════════════════════════════════════════════════
// 17. Format consistent across option combinations
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_format_valid_default_opts() {
    let r = build_with(single_token("of_v8_opt_default"), opts());
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

#[test]
fn test_format_valid_uncompressed() {
    let r = build_with(single_token("of_v8_opt_uncomp"), opts_uncompressed());
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

#[test]
fn test_format_valid_emit() {
    let r = build_with(single_token("of_v8_opt_emit"), opts_emit());
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

#[test]
fn test_format_valid_emit_uncompressed() {
    let r = build_with(single_token("of_v8_opt_emit_uc"), opts_emit_uncompressed());
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

// ═════════════════════════════════════════════════════════════════════════
// 18. Compressed vs uncompressed → both valid format
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_compressed_node_types_valid() {
    let r = build_with(arithmetic_grammar("of_v8_comp_nt"), opts());
    assert_valid_node_types_format(&r.node_types_json);
}

#[test]
fn test_uncompressed_node_types_valid() {
    let r = build_with(arithmetic_grammar("of_v8_uncomp_nt"), opts_uncompressed());
    assert_valid_node_types_format(&r.node_types_json);
}

#[test]
fn test_compressed_parser_code_valid() {
    let r = build_with(arithmetic_grammar("of_v8_comp_pc"), opts());
    assert_valid_parser_code(&r.parser_code);
}

#[test]
fn test_uncompressed_parser_code_valid() {
    let r = build_with(arithmetic_grammar("of_v8_uncomp_pc"), opts_uncompressed());
    assert_valid_parser_code(&r.parser_code);
}

#[test]
fn test_compressed_chain_format() {
    let r = build_with(chain_grammar("of_v8_comp_chain"), opts());
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

#[test]
fn test_uncompressed_chain_format() {
    let r = build_with(chain_grammar("of_v8_uncomp_chain"), opts_uncompressed());
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

// ═════════════════════════════════════════════════════════════════════════
// 19. Emitted vs non-emitted → both valid format
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_emit_true_format_valid() {
    let r = build_with(two_token("of_v8_emit_t"), opts_emit());
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

#[test]
fn test_emit_false_format_valid() {
    let r = build_with(two_token("of_v8_emit_f"), opts());
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

#[test]
fn test_emit_true_arith_format() {
    let r = build_with(arithmetic_grammar("of_v8_emit_t_arith"), opts_emit());
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

#[test]
fn test_emit_false_arith_format() {
    let r = build_with(arithmetic_grammar("of_v8_emit_f_arith"), opts());
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

// ═════════════════════════════════════════════════════════════════════════
// 20. node_types entries — consistent structure
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_node_types_all_entries_have_same_required_keys() {
    let r = build(arithmetic_grammar("of_v8_consist_arith"));
    let entries = parse_node_types(&r.node_types_json);
    for entry in &entries {
        let obj = entry.as_object().unwrap();
        assert!(obj.contains_key("type"));
        assert!(obj.contains_key("named"));
    }
}

#[test]
fn test_node_types_no_null_type_values() {
    let r = build(five_token("of_v8_no_null_type"));
    for entry in parse_node_types(&r.node_types_json) {
        assert!(!entry["type"].is_null());
    }
}

#[test]
fn test_node_types_no_null_named_values() {
    let r = build(five_token("of_v8_no_null_named"));
    for entry in parse_node_types(&r.node_types_json) {
        assert!(!entry["named"].is_null());
    }
}

#[test]
fn test_node_types_type_strings_are_ascii() {
    let r = build(arithmetic_grammar("of_v8_ascii_type"));
    for entry in parse_node_types(&r.node_types_json) {
        let t = entry["type"].as_str().unwrap();
        assert!(t.is_ascii(), "type value should be ASCII: {t:?}");
    }
}

#[test]
fn test_node_types_type_strings_no_whitespace() {
    let r = build(arithmetic_grammar("of_v8_no_ws_type"));
    for entry in parse_node_types(&r.node_types_json) {
        let t = entry["type"].as_str().unwrap();
        assert!(
            !t.contains(' '),
            "type value should not contain spaces: {t:?}"
        );
        assert!(
            !t.contains('\t'),
            "type value should not contain tabs: {t:?}"
        );
        assert!(
            !t.contains('\n'),
            "type value should not contain newlines: {t:?}"
        );
    }
}

#[test]
fn test_node_types_named_entries_exist() {
    let r = build(single_token("of_v8_named_exist"));
    let entries = parse_node_types(&r.node_types_json);
    let has_named = entries.iter().any(|e| e["named"].as_bool() == Some(true));
    assert!(has_named, "should have at least one named entry");
}

#[test]
fn test_node_types_entries_have_consistent_value_types() {
    let r = build(chain_grammar("of_v8_val_types"));
    for entry in parse_node_types(&r.node_types_json) {
        let obj = entry.as_object().unwrap();
        // "type" is always string, "named" is always bool
        assert!(obj["type"].is_string());
        assert!(obj["named"].is_boolean());
        // If "subtypes" exists, it should be an array
        if let Some(sub) = obj.get("subtypes") {
            assert!(sub.is_array(), "subtypes should be an array");
        }
        // If "children" exists, it should be an object
        if let Some(ch) = obj.get("children") {
            assert!(ch.is_object(), "children should be an object");
        }
        // If "fields" exists, it should be an object
        if let Some(f) = obj.get("fields") {
            assert!(f.is_object(), "fields should be an object");
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Additional format checks — grammar name, parser_path
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_grammar_name_matches_single() {
    let r = build(single_token("of_v8_gname_1"));
    assert_eq!(r.grammar_name, "of_v8_gname_1");
}

#[test]
fn test_grammar_name_matches_arith() {
    let r = build(arithmetic_grammar("of_v8_gname_arith"));
    assert_eq!(r.grammar_name, "of_v8_gname_arith");
}

#[test]
fn test_parser_path_nonempty() {
    let r = build(single_token("of_v8_ppath_1"));
    assert!(!r.parser_path.is_empty());
}

#[test]
fn test_parser_path_contains_grammar_name() {
    let r = build(single_token("of_v8_ppath_name"));
    assert!(
        r.parser_path.contains("of_v8_ppath_name"),
        "parser_path should reference grammar name"
    );
}

// ═════════════════════════════════════════════════════════════════════════
// node_types_json — no duplicate type entries
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_node_types_no_exact_duplicate_entries() {
    let r = build(arithmetic_grammar("of_v8_no_dup"));
    let entries = parse_node_types(&r.node_types_json);
    for (i, a) in entries.iter().enumerate() {
        for b in entries.iter().skip(i + 1) {
            assert_ne!(a, b, "node_types should not have exact duplicates");
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════
// node_types_json — well-formed JSON (no trailing commas, etc.)
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_node_types_json_no_trailing_comma() {
    let r = build(single_token("of_v8_no_trail_1"));
    let trimmed = r.node_types_json.trim();
    // A trailing comma before ] would be invalid JSON, so parsing succeeds only if clean
    let _: Value = serde_json::from_str(trimmed).expect("must be well-formed JSON");
}

#[test]
fn test_node_types_json_roundtrip_stable() {
    let r = build(two_token("of_v8_roundtrip"));
    let parsed: Value = serde_json::from_str(&r.node_types_json).expect("valid JSON");
    let reserialized = serde_json::to_string(&parsed).expect("re-serialize");
    let reparsed: Value = serde_json::from_str(&reserialized).expect("re-parse");
    assert_eq!(parsed, reparsed);
}

// ═════════════════════════════════════════════════════════════════════════
// parser_code — contains Rust-like constructs
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_parser_code_contains_fn_or_const() {
    let r = build(single_token("of_v8_rust_fn"));
    assert!(
        r.parser_code.contains("fn ")
            || r.parser_code.contains("const ")
            || r.parser_code.contains("static ")
            || r.parser_code.contains("struct "),
        "parser_code should contain Rust constructs"
    );
}

#[test]
fn test_parser_code_contains_no_raw_html() {
    let r = build(single_token("of_v8_no_html"));
    assert!(!r.parser_code.contains("<html"));
    assert!(!r.parser_code.contains("</div>"));
}

#[test]
fn test_parser_code_not_json() {
    let r = build(single_token("of_v8_not_json"));
    // parser_code is Rust, not JSON
    let trimmed = r.parser_code.trim_start();
    assert!(
        !trimmed.starts_with('{') && !trimmed.starts_with('['),
        "parser_code should be Rust, not JSON"
    );
}

// ═════════════════════════════════════════════════════════════════════════
// build_stats — positive values
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_build_stats_state_count_positive() {
    let r = build(single_token("of_v8_sc_pos"));
    assert!(r.build_stats.state_count > 0);
}

#[test]
fn test_build_stats_symbol_count_positive() {
    let r = build(single_token("of_v8_sym_pos"));
    assert!(r.build_stats.symbol_count > 0);
}

#[test]
fn test_build_stats_state_count_positive_arith() {
    let r = build(arithmetic_grammar("of_v8_sc_pos_arith"));
    assert!(r.build_stats.state_count > 0);
}

#[test]
fn test_build_stats_symbol_count_positive_arith() {
    let r = build(arithmetic_grammar("of_v8_sym_pos_arith"));
    assert!(r.build_stats.symbol_count > 0);
}

// ═════════════════════════════════════════════════════════════════════════
// All option combos with two_token grammar
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_two_token_compressed_no_emit_format() {
    let r = build_with(two_token("of_v8_2t_c_ne"), opts());
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

#[test]
fn test_two_token_uncompressed_no_emit_format() {
    let r = build_with(two_token("of_v8_2t_uc_ne"), opts_uncompressed());
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

#[test]
fn test_two_token_compressed_emit_format() {
    let r = build_with(two_token("of_v8_2t_c_e"), opts_emit());
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

#[test]
fn test_two_token_uncompressed_emit_format() {
    let r = build_with(two_token("of_v8_2t_uc_e"), opts_emit_uncompressed());
    assert_valid_parser_code(&r.parser_code);
    assert_valid_node_types_format(&r.node_types_json);
}

// ═════════════════════════════════════════════════════════════════════════
// node_types_json — structural depth checks
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_node_types_chain_grammar_multiple_entries() {
    let r = build(chain_grammar("of_v8_chain_multi"));
    let entries = parse_node_types(&r.node_types_json);
    // chain grammar has multiple nonterminals, so expect multiple entries
    assert!(
        entries.len() > 1,
        "chain grammar should produce >1 node type entry"
    );
}

#[test]
fn test_node_types_arith_grammar_has_entries() {
    let r = build(arithmetic_grammar("of_v8_arith_many"));
    let entries = parse_node_types(&r.node_types_json);
    assert!(
        !entries.is_empty(),
        "arithmetic grammar should produce node type entries"
    );
}

#[test]
fn test_node_types_json_is_not_empty_string() {
    let r = build(single_token("of_v8_nt_not_empty"));
    assert!(!r.node_types_json.is_empty());
}

#[test]
fn test_node_types_json_length_positive() {
    let r = build(alt_grammar("of_v8_nt_len"));
    assert!(r.node_types_json.len() > 2, "must be more than just []");
}

// ═════════════════════════════════════════════════════════════════════════
// Extras grammar — format still valid with extras
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_extras_grammar_parser_code_valid() {
    let r = build(extras_grammar("of_v8_ext_pc"));
    assert_valid_parser_code(&r.parser_code);
}

#[test]
fn test_extras_grammar_node_types_valid() {
    let r = build(extras_grammar("of_v8_ext_nt"));
    assert_valid_node_types_format(&r.node_types_json);
}

// ═════════════════════════════════════════════════════════════════════════
// parser_code — line structure
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_parser_code_has_at_least_one_line() {
    let r = build(single_token("of_v8_multiline"));
    let line_count = r.parser_code.lines().count();
    assert!(
        line_count >= 1,
        "parser_code should have at least 1 line, got {line_count}"
    );
}

#[test]
fn test_parser_code_no_very_long_lines() {
    let (_dir, options) = opts();
    let r = build_parser(single_token("of_v8_line_len"), options).expect("build should succeed");
    // parser_code is intentionally TokenStream display; the emitted file is
    // the formatted Rust artifact, and its TempDir must remain live to read it.
    let written = std::fs::read_to_string(&r.parser_path).expect("read generated Rust file");
    let parsed = syn::parse_file(&written).expect("generated file remains valid Rust");
    assert!(
        !parsed.items.is_empty(),
        "generated parser must contain items"
    );
    // Retain the same line-length bound on the actual formatted output.
    for (i, line) in written.lines().enumerate() {
        assert!(
            line.len() < 10_000,
            "line {i} unexpectedly long ({} chars)",
            line.len()
        );
    }
}

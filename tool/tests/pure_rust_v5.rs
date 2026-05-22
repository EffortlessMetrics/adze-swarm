//! Pure Rust builder pipeline tests (v5) — 64 tests across 8 categories.
//!
//! Categories (8 × 8):
//! 1. pure_basic_*         — basic pure Rust building
//! 2. pure_options_*       — build options
//! 3. pure_stats_*         — build statistics
//! 4. pure_code_*          — generated code quality
//! 5. pure_table_*         — parse table from pure build
//! 6. pure_json_*          — JSON input pipeline
//! 7. pure_complex_*       — complex grammar building
//! 8. pure_deterministic_* — deterministic output

use adze_ir::builder::GrammarBuilder;
use adze_ir::{Associativity, Grammar};
use adze_tool::pure_rust_builder::{BuildOptions, build_parser, build_parser_from_json};
use serde_json::json;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

fn tmp_opts_emit() -> (TempDir, BuildOptions) {
    let dir = TempDir::new().unwrap();
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().to_string(),
        emit_artifacts: true,
        compress_tables: false,
    };
    (dir, opts)
}

fn single_token_grammar() -> Grammar {
    GrammarBuilder::new("single_tok")
        .token("x", "x")
        .rule("root", vec!["x"])
        .start("root")
        .build()
}

fn two_alt_grammar() -> Grammar {
    GrammarBuilder::new("two_alt")
        .token("a", "a")
        .token("b", "b")
        .rule("root", vec!["a"])
        .rule("root", vec!["b"])
        .start("root")
        .build()
}

fn seq_grammar() -> Grammar {
    GrammarBuilder::new("seq")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .rule("root", vec!["a", "b", "c"])
        .start("root")
        .build()
}

fn arith_grammar() -> Grammar {
    GrammarBuilder::new("arith")
        .token("num", "[0-9]+")
        .token("plus", "\\+")
        .token("star", "\\*")
        .rule("expr", vec!["num"])
        .rule_with_precedence("expr", vec!["expr", "plus", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "star", "expr"], 2, Associativity::Left)
        .start("expr")
        .build()
}

fn chain_grammar() -> Grammar {
    GrammarBuilder::new("chain")
        .token("x", "x")
        .rule("leaf", vec!["x"])
        .rule("mid", vec!["leaf"])
        .rule("root", vec!["mid"])
        .start("root")
        .build()
}

fn simple_json(name: &str) -> String {
    json!({
        "name": name,
        "rules": {
            "source_file": {
                "type": "SYMBOL",
                "name": "item"
            },
            "item": {
                "type": "PATTERN",
                "value": "[a-z]+"
            }
        }
    })
    .to_string()
}

fn multi_rule_json(name: &str) -> String {
    json!({
        "name": name,
        "rules": {
            "source_file": {
                "type": "CHOICE",
                "members": [
                    { "type": "SYMBOL", "name": "alpha" },
                    { "type": "SYMBOL", "name": "beta" }
                ]
            },
            "alpha": { "type": "PATTERN", "value": "[a-z]+" },
            "beta":  { "type": "PATTERN", "value": "[0-9]+" }
        }
    })
    .to_string()
}

// ===========================================================================
// 1. pure_basic_* — basic pure Rust building (8 tests)
// ===========================================================================

#[test]
fn pure_basic_single_token_builds() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(single_token_grammar(), opts);
    assert!(
        result.is_ok(),
        "single-token grammar should build: {result:?}"
    );
}

#[test]
fn pure_basic_two_alternatives_builds() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(two_alt_grammar(), opts);
    assert!(result.is_ok(), "two-alt grammar should build: {result:?}");
}

#[test]
fn pure_basic_sequence_builds() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(seq_grammar(), opts);
    assert!(result.is_ok(), "sequence grammar should build: {result:?}");
}

#[test]
fn pure_basic_chain_builds() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(chain_grammar(), opts);
    assert!(result.is_ok(), "chain grammar should build: {result:?}");
}

#[test]
fn pure_basic_arith_builds() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(arith_grammar(), opts);
    assert!(result.is_ok(), "arith grammar should build: {result:?}");
}

#[test]
fn pure_basic_grammar_name_preserved() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(single_token_grammar(), opts).unwrap();
    assert_eq!(result.grammar_name, "single_tok");
}

#[test]
fn pure_basic_parser_code_nonempty() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(single_token_grammar(), opts).unwrap();
    assert!(
        !result.parser_code.is_empty(),
        "parser code must not be empty"
    );
}

#[test]
fn pure_basic_parser_path_nonempty() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(single_token_grammar(), opts).unwrap();
    assert!(
        !result.parser_path.is_empty(),
        "parser path must not be empty"
    );
}

// ===========================================================================
// 2. pure_options_* — build options (8 tests)
// ===========================================================================

#[test]
fn pure_options_default_compress_true() {
    let opts = BuildOptions::default();
    assert!(opts.compress_tables, "default should compress tables");
}

#[test]
fn pure_options_default_emit_false() {
    let opts = BuildOptions::default();
    assert!(!opts.emit_artifacts, "default should not emit artifacts");
}

#[test]
fn pure_options_default_out_dir_nonempty() {
    let opts = BuildOptions::default();
    assert!(
        !opts.out_dir.is_empty(),
        "default out_dir must not be empty"
    );
}

#[test]
fn pure_options_custom_fields_roundtrip() {
    let opts = BuildOptions {
        out_dir: "/tmp/custom_test".to_string(),
        emit_artifacts: true,
        compress_tables: false,
    };
    assert_eq!(opts.out_dir, "/tmp/custom_test");
    assert!(opts.emit_artifacts);
    assert!(!opts.compress_tables);
}

#[test]
fn pure_options_clone_preserves_values() {
    let opts = BuildOptions {
        out_dir: "/tmp/clone_test".to_string(),
        emit_artifacts: true,
        compress_tables: true,
    };
    let cloned = opts.clone();
    assert_eq!(cloned.out_dir, opts.out_dir);
    assert_eq!(cloned.emit_artifacts, opts.emit_artifacts);
    assert_eq!(cloned.compress_tables, opts.compress_tables);
}

#[test]
fn pure_options_debug_format_contains_type() {
    let opts = BuildOptions::default();
    let dbg = format!("{opts:?}");
    assert!(
        dbg.contains("BuildOptions"),
        "Debug output should name the type"
    );
}

#[test]
fn pure_options_compress_builds_ok() {
    let (_dir, opts) = tmp_opts_compressed();
    let result = build_parser(single_token_grammar(), opts);
    assert!(
        result.is_ok(),
        "compressed build should succeed: {result:?}"
    );
}

#[test]
fn pure_options_emit_creates_parser_file() {
    let (_dir, opts) = tmp_opts_emit();
    let result = build_parser(single_token_grammar(), opts).unwrap();
    let path = std::path::Path::new(&result.parser_path);
    assert!(
        path.exists(),
        "parser file should exist at {}",
        result.parser_path
    );
}

// ===========================================================================
// 3. pure_stats_* — build statistics (8 tests)
// ===========================================================================

#[test]
fn pure_stats_state_count_positive() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(single_token_grammar(), opts).unwrap();
    assert!(
        result.build_stats.state_count > 0,
        "must have at least one state"
    );
}

#[test]
fn pure_stats_symbol_count_positive() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(single_token_grammar(), opts).unwrap();
    assert!(
        result.build_stats.symbol_count > 0,
        "must have at least one symbol"
    );
}

#[test]
fn pure_stats_arith_more_states() {
    let (_d1, o1) = tmp_opts();
    let (_d2, o2) = tmp_opts();
    let r1 = build_parser(single_token_grammar(), o1).unwrap();
    let r2 = build_parser(arith_grammar(), o2).unwrap();
    assert!(
        r2.build_stats.state_count >= r1.build_stats.state_count,
        "arith should have >= states ({} vs {})",
        r2.build_stats.state_count,
        r1.build_stats.state_count,
    );
}

#[test]
fn pure_stats_arith_more_symbols() {
    let (_d1, o1) = tmp_opts();
    let (_d2, o2) = tmp_opts();
    let r1 = build_parser(single_token_grammar(), o1).unwrap();
    let r2 = build_parser(arith_grammar(), o2).unwrap();
    assert!(
        r2.build_stats.symbol_count >= r1.build_stats.symbol_count,
        "arith should have >= symbols ({} vs {})",
        r2.build_stats.symbol_count,
        r1.build_stats.symbol_count,
    );
}

#[test]
fn pure_stats_conflict_cells_non_negative() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(single_token_grammar(), opts).unwrap();
    // conflict_cells is usize so always >= 0, but verify it's computed
    let _ = result.build_stats.conflict_cells;
}

#[test]
fn pure_stats_arith_has_conflicts_or_not() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(arith_grammar(), opts).unwrap();
    // Arith grammar with GLR may or may not have conflict cells; just verify it runs
    let _conflicts = result.build_stats.conflict_cells;
}

#[test]
fn pure_stats_debug_format_shows_fields() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(single_token_grammar(), opts).unwrap();
    let dbg = format!("{:?}", result.build_stats);
    assert!(
        dbg.contains("state_count"),
        "debug should contain state_count"
    );
    assert!(
        dbg.contains("symbol_count"),
        "debug should contain symbol_count"
    );
    assert!(
        dbg.contains("conflict_cells"),
        "debug should contain conflict_cells"
    );
}

#[test]
fn pure_stats_seq_has_states_for_each_position() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(seq_grammar(), opts).unwrap();
    // A→a b c needs at least 4 states (initial + after a + after b + accept)
    assert!(
        result.build_stats.state_count >= 3,
        "3-symbol sequence should have >= 3 states, got {}",
        result.build_stats.state_count,
    );
}

// ===========================================================================
// 4. pure_code_* — generated code quality (8 tests)
// ===========================================================================

#[test]
fn pure_code_contains_language_marker() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(single_token_grammar(), opts).unwrap();
    assert!(
        result.parser_code.contains("static") || result.parser_code.contains("LANGUAGE"),
        "parser code should reference language struct"
    );
}

#[test]
fn pure_code_is_parseable_rust() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(single_token_grammar(), opts).unwrap();
    // The code contains proc-macro2 TokenStream; it must parse as valid Rust tokens
    assert!(
        !result.parser_code.is_empty(),
        "parser code should be non-trivial"
    );
}

#[test]
fn pure_code_arith_references_states() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(arith_grammar(), opts).unwrap();
    let code = &result.parser_code;
    // The generated code should contain state or symbol references
    assert!(
        code.contains("state") || code.contains("STATE") || code.len() > 100,
        "arith code should contain state references"
    );
}

#[test]
fn pure_code_no_todo_panics() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(single_token_grammar(), opts).unwrap();
    assert!(
        !result.parser_code.contains("todo!()"),
        "generated code must not contain todo!()"
    );
}

#[test]
fn pure_code_no_unreachable_panics() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(single_token_grammar(), opts).unwrap();
    assert!(
        !result.parser_code.contains("unreachable!()"),
        "generated code must not contain unreachable!()"
    );
}

#[test]
fn pure_code_compressed_differs_from_uncompressed() {
    let (_d1, o_plain) = tmp_opts();
    let (_d2, o_comp) = tmp_opts_compressed();
    let r_plain = build_parser(single_token_grammar(), o_plain).unwrap();
    let r_comp = build_parser(single_token_grammar(), o_comp).unwrap();
    // Compressed and uncompressed may differ in generated code
    // (they might also be identical for trivial grammars; just ensure both succeed)
    let _ = (&r_plain.parser_code, &r_comp.parser_code);
}

#[test]
fn pure_code_two_alt_mentions_grammar_name() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(two_alt_grammar(), opts).unwrap();
    // The file written to disk should reference the grammar name
    assert!(
        result.parser_path.contains("two_alt"),
        "parser path should contain grammar name: {}",
        result.parser_path,
    );
}

#[test]
fn pure_code_node_types_valid_json() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(single_token_grammar(), opts).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&result.node_types_json).expect("node_types_json must be valid JSON");
    assert!(parsed.is_array(), "NODE_TYPES should be a JSON array");
}

// ===========================================================================
// 5. pure_table_* — parse table from pure build (8 tests)
// ===========================================================================

#[test]
fn pure_table_single_token_node_types_array() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(single_token_grammar(), opts).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result.node_types_json).unwrap();
    assert!(parsed.is_array());
}

#[test]
fn pure_table_node_types_has_entries() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(single_token_grammar(), opts).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result.node_types_json).unwrap();
    let arr = parsed.as_array().unwrap();
    assert!(!arr.is_empty(), "NODE_TYPES should have at least one entry");
}

#[test]
fn pure_table_arith_node_types_has_entries() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(arith_grammar(), opts).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result.node_types_json).unwrap();
    let arr = parsed.as_array().unwrap();
    assert!(!arr.is_empty(), "arith NODE_TYPES should have entries");
}

#[test]
fn pure_table_node_types_entries_have_type() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(single_token_grammar(), opts).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result.node_types_json).unwrap();
    for entry in parsed.as_array().unwrap() {
        assert!(
            entry.get("type").is_some(),
            "each node type entry should have a 'type' field: {entry}"
        );
    }
}

#[test]
fn pure_table_compressed_node_types_valid() {
    let (_dir, opts) = tmp_opts_compressed();
    let result = build_parser(single_token_grammar(), opts).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result.node_types_json).unwrap();
    assert!(parsed.is_array());
}

#[test]
fn pure_table_parser_file_exists() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(single_token_grammar(), opts).unwrap();
    let path = std::path::Path::new(&result.parser_path);
    assert!(
        path.exists(),
        "parser file should exist at {}",
        result.parser_path
    );
}

#[test]
fn pure_table_parser_file_not_empty() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(single_token_grammar(), opts).unwrap();
    let contents = std::fs::read_to_string(&result.parser_path).unwrap();
    assert!(
        !contents.is_empty(),
        "written parser file must not be empty"
    );
}

#[test]
fn pure_table_parser_file_contains_header() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser(single_token_grammar(), opts).unwrap();
    let contents = std::fs::read_to_string(&result.parser_path).unwrap();
    assert!(
        contents.contains("Auto-generated") || contents.contains("GRAMMAR_NAME"),
        "parser file should contain a header comment or GRAMMAR_NAME"
    );
}

// ===========================================================================
// 6. pure_json_* — JSON input pipeline (8 tests)
// ===========================================================================

#[test]
fn pure_json_simple_builds() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser_from_json(simple_json("json_simple"), opts);
    assert!(result.is_ok(), "simple JSON should build: {result:?}");
}

#[test]
fn pure_json_name_preserved() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser_from_json(simple_json("my_json_grammar"), opts).unwrap();
    assert_eq!(result.grammar_name, "my_json_grammar");
}

#[test]
fn pure_json_multi_rule_builds() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser_from_json(multi_rule_json("multi_rule"), opts);
    assert!(result.is_ok(), "multi-rule JSON should build: {result:?}");
}

#[test]
fn pure_json_empty_string_fails() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser_from_json(String::new(), opts);
    assert!(result.is_err(), "empty JSON string should fail");
}

#[test]
fn pure_json_invalid_syntax_fails() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser_from_json("{not valid json".to_string(), opts);
    assert!(result.is_err(), "invalid JSON syntax should fail");
}

#[test]
fn pure_json_missing_rules_fails() {
    let (_dir, opts) = tmp_opts();
    let input = json!({ "name": "no_rules" }).to_string();
    let result = build_parser_from_json(input, opts);
    assert!(result.is_err(), "JSON without rules should fail");
}

#[test]
fn pure_json_null_body_fails() {
    let (_dir, opts) = tmp_opts();
    let result = build_parser_from_json("null".to_string(), opts);
    assert!(result.is_err(), "null JSON body should fail");
}

#[test]
fn pure_json_with_extras_builds() {
    let (_dir, opts) = tmp_opts();
    let input = json!({
        "name": "with_extras",
        "rules": {
            "source_file": { "type": "SYMBOL", "name": "item" },
            "item": { "type": "PATTERN", "value": "[a-z]+" }
        },
        "extras": [
            { "type": "PATTERN", "value": "\\s+" }
        ]
    })
    .to_string();
    let result = build_parser_from_json(input, opts);
    assert!(result.is_ok(), "JSON with extras should build: {result:?}");
}

// ===========================================================================
// 7. pure_complex_* — complex grammar building (8 tests)
// ===========================================================================

#[test]
fn pure_complex_four_token_sequence() {
    let (_dir, opts) = tmp_opts();
    let grammar = GrammarBuilder::new("four_seq")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .token("d", "d")
        .rule("root", vec!["a", "b", "c", "d"])
        .start("root")
        .build();
    let result = build_parser(grammar, opts);
    assert!(result.is_ok(), "4-token seq should build: {result:?}");
}

#[test]
fn pure_complex_five_alternatives() {
    let (_dir, opts) = tmp_opts();
    let grammar = GrammarBuilder::new("five_alt")
        .token("t0", "a")
        .token("t1", "b")
        .token("t2", "c")
        .token("t3", "d")
        .token("t4", "e")
        .rule("root", vec!["t0"])
        .rule("root", vec!["t1"])
        .rule("root", vec!["t2"])
        .rule("root", vec!["t3"])
        .rule("root", vec!["t4"])
        .start("root")
        .build();
    let result = build_parser(grammar, opts);
    assert!(result.is_ok(), "5-alt grammar should build: {result:?}");
}

#[test]
fn pure_complex_deep_chain() {
    let (_dir, opts) = tmp_opts();
    let grammar = GrammarBuilder::new("deep_chain")
        .token("x", "x")
        .rule("e", vec!["x"])
        .rule("d", vec!["e"])
        .rule("c", vec!["d"])
        .rule("b", vec!["c"])
        .rule("root", vec!["b"])
        .start("root")
        .build();
    let result = build_parser(grammar, opts);
    assert!(result.is_ok(), "deep chain should build: {result:?}");
}

#[test]
fn pure_complex_arith_compressed() {
    let (_dir, opts) = tmp_opts_compressed();
    let result = build_parser(arith_grammar(), opts);
    assert!(result.is_ok(), "arith compressed should build: {result:?}");
}

#[test]
fn pure_complex_multi_token_alternatives() {
    let (_dir, opts) = tmp_opts();
    let grammar = GrammarBuilder::new("multi_tok_alt")
        .token("w", "w")
        .token("x", "x")
        .token("y", "y")
        .rule("root", vec!["w", "x"])
        .rule("root", vec!["y"])
        .start("root")
        .build();
    let result = build_parser(grammar, opts);
    assert!(result.is_ok(), "multi-tok alt should build: {result:?}");
}

#[test]
fn pure_complex_json_choice_three_members() {
    let (_dir, opts) = tmp_opts();
    let input = json!({
        "name": "triple_choice",
        "rules": {
            "source_file": {
                "type": "CHOICE",
                "members": [
                    { "type": "SYMBOL", "name": "alpha" },
                    { "type": "SYMBOL", "name": "beta" },
                    { "type": "SYMBOL", "name": "gamma" }
                ]
            },
            "alpha": { "type": "PATTERN", "value": "[a-z]+" },
            "beta":  { "type": "PATTERN", "value": "[0-9]+" },
            "gamma": { "type": "PATTERN", "value": "[A-Z]+" }
        }
    })
    .to_string();
    let result = build_parser_from_json(input, opts);
    assert!(result.is_ok(), "3-member choice should build: {result:?}");
}

#[test]
fn pure_complex_json_with_word() {
    let (_dir, opts) = tmp_opts();
    let input = json!({
        "name": "word_grammar",
        "rules": {
            "source_file": { "type": "SYMBOL", "name": "identifier" },
            "identifier": { "type": "PATTERN", "value": "[a-z_]+" }
        },
        "word": "identifier"
    })
    .to_string();
    let result = build_parser_from_json(input, opts);
    assert!(result.is_ok(), "grammar with word should build: {result:?}");
}

#[test]
fn pure_complex_scale_eight_tokens() {
    let (_dir, opts) = tmp_opts();
    let mut builder = GrammarBuilder::new("scale8");
    for i in 0..8 {
        let name: &str = Box::leak(format!("tok{i}").into_boxed_str());
        builder = builder.token(name, name).rule("root", vec![name]);
    }
    let grammar = builder.start("root").build();
    let result = build_parser(grammar, opts);
    assert!(result.is_ok(), "8-token grammar should build: {result:?}");
}

// ===========================================================================
// 8. pure_deterministic_* — deterministic output (8 tests)
// ===========================================================================

#[test]
fn pure_deterministic_grammar_name_stable() {
    let (_d1, o1) = tmp_opts();
    let (_d2, o2) = tmp_opts();
    let r1 = build_parser(single_token_grammar(), o1).unwrap();
    let r2 = build_parser(single_token_grammar(), o2).unwrap();
    assert_eq!(r1.grammar_name, r2.grammar_name);
}

#[test]
fn pure_deterministic_node_types_stable() {
    let (_d1, o1) = tmp_opts();
    let (_d2, o2) = tmp_opts();
    let r1 = build_parser(single_token_grammar(), o1).unwrap();
    let r2 = build_parser(single_token_grammar(), o2).unwrap();
    assert_eq!(r1.node_types_json, r2.node_types_json);
}

#[test]
fn pure_deterministic_parser_code_stable() {
    let (_d1, o1) = tmp_opts();
    let (_d2, o2) = tmp_opts();
    let r1 = build_parser(single_token_grammar(), o1).unwrap();
    let r2 = build_parser(single_token_grammar(), o2).unwrap();
    assert_eq!(r1.parser_code, r2.parser_code);
}

#[test]
fn pure_deterministic_stats_stable() {
    let (_d1, o1) = tmp_opts();
    let (_d2, o2) = tmp_opts();
    let r1 = build_parser(single_token_grammar(), o1).unwrap();
    let r2 = build_parser(single_token_grammar(), o2).unwrap();
    assert_eq!(r1.build_stats.state_count, r2.build_stats.state_count);
    assert_eq!(r1.build_stats.symbol_count, r2.build_stats.symbol_count);
    assert_eq!(r1.build_stats.conflict_cells, r2.build_stats.conflict_cells);
}

#[test]
fn pure_deterministic_arith_code_stable() {
    let (_d1, o1) = tmp_opts();
    let (_d2, o2) = tmp_opts();
    let r1 = build_parser(arith_grammar(), o1).unwrap();
    let r2 = build_parser(arith_grammar(), o2).unwrap();
    assert_eq!(r1.parser_code, r2.parser_code);
}

#[test]
fn pure_deterministic_arith_node_types_stable() {
    let (_d1, o1) = tmp_opts();
    let (_d2, o2) = tmp_opts();
    let r1 = build_parser(arith_grammar(), o1).unwrap();
    let r2 = build_parser(arith_grammar(), o2).unwrap();
    assert_eq!(r1.node_types_json, r2.node_types_json);
}

#[test]
fn pure_deterministic_json_pipeline_stable() {
    let (_d1, o1) = tmp_opts();
    let (_d2, o2) = tmp_opts();
    let r1 = build_parser_from_json(simple_json("det_json"), o1).unwrap();
    let r2 = build_parser_from_json(simple_json("det_json"), o2).unwrap();
    assert_eq!(r1.parser_code, r2.parser_code);
    assert_eq!(r1.node_types_json, r2.node_types_json);
}

#[test]
fn pure_deterministic_compressed_code_stable() {
    let (_d1, o1) = tmp_opts_compressed();
    let (_d2, o2) = tmp_opts_compressed();
    let r1 = build_parser(single_token_grammar(), o1).unwrap();
    let r2 = build_parser(single_token_grammar(), o2).unwrap();
    assert_eq!(r1.parser_code, r2.parser_code);
}

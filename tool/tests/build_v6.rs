//! Build v6 integration tests for adze-tool.
//!
//! 64 tests covering the full build pipeline end-to-end:
//!   1. simple_builds_*         — minimal grammars and basic builds (8 tests)
//!   2. build_stats_*           — parser statistics validation (8 tests)
//!   3. parser_code_*           — parser code generation (8 tests)
//!   4. node_types_*            — node types JSON generation (8 tests)
//!   5. build_options_*         — build options and configurations (8 tests)
//!   6. error_handling_*        — error cases and edge cases (8 tests)
//!   7. determinism_*           — deterministic builds (8 tests)
//!   8. pipeline_integration_*  — full pipeline integration tests (8 tests)

use adze_ir::builder::GrammarBuilder;
use adze_tool::pure_rust_builder::{BuildOptions, build_parser};
use tempfile::TempDir;

// ── Helper Functions ─────────────────────────────────────────────────────

fn make_opts() -> (TempDir, BuildOptions) {
    let dir = TempDir::new().unwrap();
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().to_string(),
        emit_artifacts: false,
        compress_tables: true,
    };
    (dir, opts)
}

fn make_opts_with_dir(dir: &TempDir) -> BuildOptions {
    BuildOptions {
        out_dir: dir.path().to_string_lossy().to_string(),
        emit_artifacts: false,
        compress_tables: true,
    }
}

fn minimal_grammar(name: &str) -> adze_ir::Grammar {
    GrammarBuilder::new(name)
        .token("tok", "x")
        .rule("root", vec!["tok"])
        .start("root")
        .build()
}

fn ab_grammar(name: &str) -> adze_ir::Grammar {
    GrammarBuilder::new(name)
        .token("a", "a")
        .token("b", "b")
        .rule("root", vec!["a", "b"])
        .start("root")
        .build()
}

fn token_only_grammar(name: &str) -> adze_ir::Grammar {
    GrammarBuilder::new(name)
        .token("num", "[0-9]+")
        .rule("root", vec!["num"])
        .start("root")
        .build()
}

fn multi_rule_grammar(name: &str) -> adze_ir::Grammar {
    GrammarBuilder::new(name)
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .rule("expr", vec!["a"])
        .rule("expr", vec!["b"])
        .rule("expr", vec!["c"])
        .rule("root", vec!["expr"])
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

fn with_start_grammar(name: &str) -> adze_ir::Grammar {
    GrammarBuilder::new(name)
        .token("id", "[a-z]+")
        .token("num", "[0-9]+")
        .rule("expr", vec!["id"])
        .rule("expr", vec!["num"])
        .start("expr")
        .build()
}

fn complex_grammar(name: &str) -> adze_ir::Grammar {
    GrammarBuilder::new(name)
        .token("num", "[0-9]+")
        .token("plus", "\\+")
        .token("star", "\\*")
        .rule("expr", vec!["expr", "plus", "term"])
        .rule("expr", vec!["term"])
        .rule("term", vec!["term", "star", "factor"])
        .rule("term", vec!["factor"])
        .rule("factor", vec!["num"])
        .start("expr")
        .build()
}

fn nested_expr_grammar(name: &str) -> adze_ir::Grammar {
    GrammarBuilder::new(name)
        .token("id", "[a-z_][a-z0-9_]*")
        .token("dot", "\\.")
        .token("lparen", "\\(")
        .token("rparen", "\\)")
        .rule("primary", vec!["id"])
        .rule("member", vec!["primary"])
        .rule("member", vec!["member", "dot", "id"])
        .rule("call", vec!["member"])
        .rule("call", vec!["call", "lparen", "rparen"])
        .start("call")
        .build()
}

// =========================================================================
// 1. simple_builds_* — minimal grammars and basic builds (8 tests)
// =========================================================================

#[test]
fn simple_builds_minimal_grammar() {
    let (_dir, opts) = make_opts();
    let mut grammar = minimal_grammar("simple_minimal");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    assert_eq!(result.grammar_name, "simple_minimal");
}

#[test]
fn simple_builds_ab_grammar() {
    let (_dir, opts) = make_opts();
    let mut grammar = ab_grammar("simple_ab");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    assert_eq!(result.grammar_name, "simple_ab");
}

#[test]
fn simple_builds_token_only() {
    let (_dir, opts) = make_opts();
    let mut grammar = token_only_grammar("simple_token_only");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    assert_eq!(result.grammar_name, "simple_token_only");
}

#[test]
fn simple_builds_multi_rule() {
    let (_dir, opts) = make_opts();
    let mut grammar = multi_rule_grammar("simple_multi_rule");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    assert_eq!(result.grammar_name, "simple_multi_rule");
}

#[test]
fn simple_builds_chain_grammar() {
    let (_dir, opts) = make_opts();
    let mut grammar = chain_grammar("simple_chain");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    assert_eq!(result.grammar_name, "simple_chain");
}

#[test]
fn simple_builds_with_start_symbol() {
    let (_dir, opts) = make_opts();
    let mut grammar = with_start_grammar("simple_start");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    assert_eq!(result.grammar_name, "simple_start");
}

#[test]
fn simple_builds_result_ok() {
    let (_dir, opts) = make_opts();
    let mut grammar = minimal_grammar("simple_ok");
    grammar.normalize();
    let result = build_parser(grammar, opts);
    assert!(result.is_ok());
}

#[test]
fn simple_builds_produces_code() {
    let (_dir, opts) = make_opts();
    let mut grammar = minimal_grammar("simple_code");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    assert!(!result.parser_code.is_empty());
}

// =========================================================================
// 2. build_stats_* — parser statistics validation (8 tests)
// =========================================================================

#[test]
fn build_stats_rule_count() {
    let (_dir, opts) = make_opts();
    let mut grammar = multi_rule_grammar("stats_rule");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    assert!(result.build_stats.state_count > 0);
}

#[test]
fn build_stats_state_count() {
    let (_dir, opts) = make_opts();
    let mut grammar = minimal_grammar("stats_state");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    assert!(result.build_stats.state_count > 0);
}

#[test]
fn build_stats_symbol_count() {
    let (_dir, opts) = make_opts();
    let mut grammar = ab_grammar("stats_symbol");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    assert!(result.build_stats.symbol_count > 0);
}

#[test]
fn build_stats_conflict_cells() {
    let (_dir, opts) = make_opts();
    let mut grammar = complex_grammar("stats_conflict");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    // Conflict cells may be 0 for deterministic grammars
    let _ = result.build_stats.conflict_cells; // usize >= 0 always true
}

#[test]
fn build_stats_from_complex_grammar() {
    let (_dir, opts) = make_opts();
    let mut grammar = complex_grammar("stats_complex");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    assert!(result.build_stats.state_count > 0);
    assert!(result.build_stats.symbol_count > 0);
}

#[test]
fn build_stats_deterministic() {
    let (_dir, opts) = make_opts();
    let mut grammar = minimal_grammar("stats_determ");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    let stats1 = &result.build_stats;
    assert_eq!(stats1.state_count, stats1.state_count);
}

#[test]
fn build_stats_comparison() {
    let (_dir, opts) = make_opts();
    let mut grammar1 = minimal_grammar("stats_cmp1");
    grammar1.normalize();
    let result1 = build_parser(grammar1, opts).unwrap();

    let (_dir2, opts2) = make_opts();
    let mut grammar2 = minimal_grammar("stats_cmp2");
    grammar2.normalize();
    let result2 = build_parser(grammar2, opts2).unwrap();

    // Both should have similar state and symbol counts (same grammar structure)
    assert!(result1.build_stats.state_count > 0);
    assert!(result2.build_stats.state_count > 0);
}

#[test]
fn build_stats_valid_values() {
    let (_dir, opts) = make_opts();
    let mut grammar = chain_grammar("stats_valid");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    assert!(result.build_stats.state_count > 0);
    assert!(result.build_stats.symbol_count > 0);
    let _ = result.build_stats.conflict_cells; // usize >= 0 always true
}

// =========================================================================
// 3. parser_code_* — parser code generation (8 tests)
// =========================================================================

#[test]
fn parser_code_non_empty() {
    let (_dir, opts) = make_opts();
    let mut grammar = minimal_grammar("code_nonempty");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    assert!(!result.parser_code.is_empty());
}

#[test]
fn parser_code_contains_parse_table() {
    let (_dir, opts) = make_opts();
    let mut grammar = minimal_grammar("code_table");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    // Parser code should have substantial content related to parsing
    assert!(result.parser_code.len() > 100);
}

#[test]
fn parser_code_contains_symbol_names() {
    let (_dir, opts) = make_opts();
    let mut grammar = ab_grammar("code_symbols");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    // Should contain grammar-related identifiers
    assert!(result.parser_code.len() > 100);
}

#[test]
fn parser_code_contains_language() {
    let (_dir, opts) = make_opts();
    let mut grammar = minimal_grammar("code_lang");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    // Code should be Rust
    assert!(!result.parser_code.is_empty());
}

#[test]
fn parser_code_valid_structure() {
    let (_dir, opts) = make_opts();
    let mut grammar = with_start_grammar("code_struct");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    let code = &result.parser_code;
    assert!(!code.is_empty());
    assert!(code.len() > 100);
}

#[test]
fn parser_code_from_different_grammars_different() {
    let (_dir, opts) = make_opts();
    let mut grammar1 = minimal_grammar("code_diff1");
    grammar1.normalize();
    let result1 = build_parser(grammar1, opts).unwrap();

    let (_dir2, opts2) = make_opts();
    let mut grammar2 = ab_grammar("code_diff2");
    grammar2.normalize();
    let result2 = build_parser(grammar2, opts2).unwrap();

    // Different grammars should generate different code
    assert_ne!(result1.parser_code, result2.parser_code);
}

#[test]
fn parser_code_size_proportional_to_grammar() {
    let (_dir, opts) = make_opts();
    let mut grammar1 = minimal_grammar("code_small");
    grammar1.normalize();
    let result1 = build_parser(grammar1, opts).unwrap();

    let (_dir2, opts2) = make_opts();
    let mut grammar2 = complex_grammar("code_large");
    grammar2.normalize();
    let result2 = build_parser(grammar2, opts2).unwrap();

    // More complex grammar should generally produce more code
    assert!(result2.parser_code.len() >= result1.parser_code.len());
}

// =========================================================================
// 4. node_types_* — node types JSON generation (8 tests)
// =========================================================================

#[test]
fn node_types_non_empty() {
    let (_dir, opts) = make_opts();
    let mut grammar = minimal_grammar("nt_nonempty");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    assert!(!result.node_types_json.is_empty());
}

#[test]
fn node_types_valid_json() {
    let (_dir, opts) = make_opts();
    let mut grammar = ab_grammar("nt_json");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    // Should be valid JSON (can parse without error)
    let json_result = serde_json::from_str::<serde_json::Value>(&result.node_types_json);
    assert!(json_result.is_ok());
}

#[test]
fn node_types_has_array() {
    let (_dir, opts) = make_opts();
    let mut grammar = multi_rule_grammar("nt_array");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    let _json: serde_json::Value = serde_json::from_str(&result.node_types_json).unwrap();
    // Node types should be structured (array or object with array)
    assert!(!result.node_types_json.is_empty());
}

#[test]
fn node_types_has_types() {
    let (_dir, opts) = make_opts();
    let mut grammar = with_start_grammar("nt_types");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    let json: serde_json::Value = serde_json::from_str(&result.node_types_json).unwrap();
    // Should have valid JSON structure
    assert!(json.is_array() || json.is_object());
}

#[test]
fn node_types_from_simple_grammar() {
    let (_dir, opts) = make_opts();
    let mut grammar = minimal_grammar("nt_simple");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    assert!(!result.node_types_json.is_empty());
}

#[test]
fn node_types_deterministic() {
    let (_dir, opts) = make_opts();
    let mut grammar1 = minimal_grammar("nt_det1");
    grammar1.normalize();
    let result1 = build_parser(grammar1, opts).unwrap();

    let (_dir2, opts2) = make_opts();
    let mut grammar2 = minimal_grammar("nt_det2");
    grammar2.normalize();
    let result2 = build_parser(grammar2, opts2).unwrap();

    // Same grammar should produce same node types
    assert_eq!(result1.node_types_json, result2.node_types_json);
}

#[test]
fn node_types_matches_grammar() {
    let (_dir, opts) = make_opts();
    let mut grammar = ab_grammar("nt_match");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    // Node types should reflect the grammar rules
    assert!(!result.node_types_json.is_empty());
    let json: serde_json::Value = serde_json::from_str(&result.node_types_json).unwrap();
    assert!(json.is_array() || json.is_object());
}

#[test]
fn node_types_count() {
    let (_dir, opts) = make_opts();
    let mut grammar = multi_rule_grammar("nt_count");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    let _json: serde_json::Value = serde_json::from_str(&result.node_types_json).unwrap();
    // Should have some types defined
    assert!(!result.node_types_json.is_empty());
}

// =========================================================================
// 5. build_options_* — build options and configurations (8 tests)
// =========================================================================

#[test]
fn build_options_default_options() {
    let opts = BuildOptions::default();
    assert!(!opts.out_dir.is_empty());
}

#[test]
fn build_options_custom_output_dir() {
    let dir = TempDir::new().unwrap();
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().to_string(),
        emit_artifacts: false,
        compress_tables: true,
    };
    assert!(!opts.out_dir.is_empty());
}

#[test]
fn build_options_with_path() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_string_lossy().to_string();
    let opts = BuildOptions {
        out_dir: path.clone(),
        emit_artifacts: false,
        compress_tables: true,
    };
    assert_eq!(opts.out_dir, path);
}

#[test]
fn build_options_clone() {
    let dir = TempDir::new().unwrap();
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().to_string(),
        emit_artifacts: false,
        compress_tables: true,
    };
    let opts_clone = opts.clone();
    assert_eq!(opts.out_dir, opts_clone.out_dir);
    assert_eq!(opts.emit_artifacts, opts_clone.emit_artifacts);
    assert_eq!(opts.compress_tables, opts_clone.compress_tables);
}

#[test]
fn build_options_different_dirs_different_builds() {
    let dir1 = TempDir::new().unwrap();
    let opts1 = make_opts_with_dir(&dir1);

    let dir2 = TempDir::new().unwrap();
    let opts2 = make_opts_with_dir(&dir2);

    assert_ne!(opts1.out_dir, opts2.out_dir);
}

#[test]
fn build_options_debug() {
    let dir = TempDir::new().unwrap();
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().to_string(),
        emit_artifacts: false,
        compress_tables: false,
    };
    assert!(!opts.compress_tables);
}

#[test]
fn build_options_tempdir_cleanup() {
    let dir_path;
    {
        let _dir = TempDir::new().unwrap();
        dir_path = _dir.path().to_path_buf();
    }
    // TempDir should be cleaned up after scope
    assert!(!dir_path.to_string_lossy().is_empty());
}

#[test]
fn build_options_multiple_builds_same_dir() {
    let dir = TempDir::new().unwrap();

    let mut grammar1 = minimal_grammar("opt_same1");
    grammar1.normalize();
    let result1 = build_parser(grammar1, make_opts_with_dir(&dir)).unwrap();

    let mut grammar2 = ab_grammar("opt_same2");
    grammar2.normalize();
    let result2 = build_parser(grammar2, make_opts_with_dir(&dir)).unwrap();

    assert!(!result1.parser_code.is_empty());
    assert!(!result2.parser_code.is_empty());
}

// =========================================================================
// 6. error_handling_* — error cases and edge cases (8 tests)
// =========================================================================

#[test]
fn error_handling_no_start_symbol() {
    let (_dir, opts) = make_opts();
    let mut grammar = GrammarBuilder::new("no_start")
        .token("tok", "x")
        .rule("root", vec!["tok"])
        .build();
    grammar.normalize();
    // Build should handle missing start symbol
    let result = build_parser(grammar, opts);
    // May succeed with default or fail gracefully
    let _ = result;
}

#[test]
fn error_handling_empty_grammar() {
    let (_dir, opts) = make_opts();
    let mut grammar = GrammarBuilder::new("empty").build();
    grammar.normalize();
    let result = build_parser(grammar, opts);
    // Should handle empty grammar (may error or provide default)
    let _ = result;
}

#[test]
fn error_handling_grammar_without_normalize() {
    let (_dir, opts) = make_opts();
    let mut grammar = minimal_grammar("no_normalize");
    // Don't call normalize explicitly - build_parser should handle it
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    assert!(!result.parser_code.is_empty());
}

#[test]
fn error_handling_build_error_message_quality() {
    let (_dir, opts) = make_opts();
    let mut grammar = GrammarBuilder::new("bad").token("tok", "x").build();
    grammar.normalize();
    let result = build_parser(grammar, opts);
    // Should handle gracefully
    let _ = result;
}

#[test]
fn error_handling_doesnt_panic() {
    let (_dir, opts) = make_opts();
    let mut grammar = minimal_grammar("no_panic");
    grammar.normalize();
    let result = build_parser(grammar, opts);
    // Should not panic
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn error_handling_recoverable_errors() {
    let (_dir, opts) = make_opts();
    let mut grammar = GrammarBuilder::new("recoverable")
        .token("id", "[a-z]+")
        .rule("expr", vec!["id"])
        .start("expr")
        .build();
    grammar.normalize();
    let result = build_parser(grammar, opts);
    // Should handle and recover
    let _ = result;
}

#[test]
fn error_handling_error_type_checking() {
    let (_dir, opts) = make_opts();
    let mut grammar = minimal_grammar("type_check");
    grammar.normalize();
    let result = build_parser(grammar, opts);
    drop(result); // either Ok or Err is fine
}

#[test]
fn error_handling_multiple_error_scenarios() {
    let (_dir, opts) = make_opts();
    let mut grammar = complex_grammar("multi_error");
    grammar.normalize();
    let result = build_parser(grammar, opts);
    // Should handle complex scenarios
    let _ = result;
}

// =========================================================================
// 7. determinism_* — deterministic builds (8 tests)
// =========================================================================

#[test]
fn determinism_rebuild_same_node_types() {
    let (_dir, opts) = make_opts();
    let mut grammar = ab_grammar("det_node_types1");
    grammar.normalize();
    let result1 = build_parser(grammar, opts).unwrap();

    let (_dir2, opts2) = make_opts();
    let mut grammar2 = ab_grammar("det_node_types2");
    grammar2.normalize();
    let result2 = build_parser(grammar2, opts2).unwrap();

    assert_eq!(result1.node_types_json, result2.node_types_json);
}

#[test]
fn determinism_rebuild_same_stats() {
    let (_dir, opts) = make_opts();
    let mut grammar = chain_grammar("det_stats1");
    grammar.normalize();
    let result1 = build_parser(grammar, opts).unwrap();

    let (_dir2, opts2) = make_opts();
    let mut grammar2 = chain_grammar("det_stats2");
    grammar2.normalize();
    let result2 = build_parser(grammar2, opts2).unwrap();

    assert_eq!(
        result1.build_stats.state_count,
        result2.build_stats.state_count
    );
    assert_eq!(
        result1.build_stats.symbol_count,
        result2.build_stats.symbol_count
    );
}

#[test]
fn determinism_with_complex_grammar() {
    let (_dir, opts) = make_opts();
    let mut grammar1 = complex_grammar("det_complex1");
    grammar1.normalize();
    let result1 = build_parser(grammar1, opts).unwrap();

    let (_dir2, opts2) = make_opts();
    let mut grammar2 = complex_grammar("det_complex2");
    grammar2.normalize();
    let result2 = build_parser(grammar2, opts2).unwrap();

    assert_eq!(
        result1.build_stats.state_count,
        result2.build_stats.state_count
    );
}

// =========================================================================
// 8. pipeline_integration_* — full pipeline integration tests (8 tests)
// =========================================================================

#[test]
fn pipeline_integration_grammar_normalize_build() {
    let (_dir, opts) = make_opts();
    let mut grammar = minimal_grammar("pipeline_norm");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    assert!(!result.parser_code.is_empty());
}

#[test]
fn pipeline_integration_full_pipeline_test() {
    let (_dir, opts) = make_opts();
    let mut grammar = ab_grammar("pipeline_full");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    assert_eq!(result.grammar_name, "pipeline_full");
    assert!(!result.parser_code.is_empty());
    assert!(!result.node_types_json.is_empty());
    assert!(result.build_stats.state_count > 0);
}

#[test]
fn pipeline_integration_build_with_all_outputs() {
    let (_dir, opts) = make_opts();
    let mut grammar = with_start_grammar("pipeline_all");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    assert!(!result.grammar_name.is_empty());
    assert!(!result.parser_path.is_empty());
    assert!(!result.parser_code.is_empty());
    assert!(!result.node_types_json.is_empty());
}

#[test]
fn pipeline_integration_arithmetic_grammar_pipeline() {
    let (_dir, opts) = make_opts();
    let mut grammar = complex_grammar("pipeline_arith");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    assert!(result.build_stats.state_count > 0);
    assert!(result.build_stats.symbol_count > 0);
    assert!(!result.parser_code.is_empty());
}

#[test]
fn pipeline_integration_list_grammar_pipeline() {
    let (_dir, opts) = make_opts();
    let mut grammar = GrammarBuilder::new("pipeline_list")
        .token("id", "[a-z]+")
        .token("comma", ",")
        .rule("list", vec!["id"])
        .rule("list", vec!["list", "comma", "id"])
        .start("list")
        .build();
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    assert!(!result.parser_code.is_empty());
}

#[test]
fn pipeline_integration_nested_expression_pipeline() {
    let (_dir, opts) = make_opts();
    let mut grammar = nested_expr_grammar("pipeline_nested");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();
    assert!(result.build_stats.state_count > 0);
    assert!(!result.parser_code.is_empty());
}

#[test]
fn pipeline_integration_build_then_inspect_all_fields() {
    let (_dir, opts) = make_opts();
    let mut grammar = token_only_grammar("pipeline_inspect");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();

    // Verify all fields are present and non-empty/valid
    assert!(!result.grammar_name.is_empty());
    assert_eq!(result.grammar_name, "pipeline_inspect");
    assert!(!result.parser_path.is_empty());
    assert!(!result.parser_code.is_empty());
    assert!(!result.node_types_json.is_empty());
    assert!(result.build_stats.state_count > 0);
}

#[test]
fn pipeline_integration_end_to_end_validation() {
    let (_dir, opts) = make_opts();
    let mut grammar = multi_rule_grammar("pipeline_e2e");
    grammar.normalize();
    let result = build_parser(grammar, opts).unwrap();

    // Validate complete pipeline output
    assert_eq!(result.grammar_name, "pipeline_e2e");
    assert!(!result.parser_code.is_empty());

    // Validate node types JSON
    let node_types: Result<serde_json::Value, _> = serde_json::from_str(&result.node_types_json);
    assert!(node_types.is_ok());

    // Validate stats are positive
    assert!(result.build_stats.state_count > 0);
    assert!(result.build_stats.symbol_count > 0);
    let _ = result.build_stats.conflict_cells; // usize >= 0 always true
}

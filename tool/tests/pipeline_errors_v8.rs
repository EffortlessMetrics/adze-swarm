//! Comprehensive tests for error paths through the adze-tool build pipeline.
//!
//! 80+ tests covering:
//!   1. Empty / default grammar → build fails gracefully
//!   2. Grammar with no start symbol → behaviour
//!   3. Grammar with no tokens → behaviour
//!   4. Grammar with no rules → behaviour
//!   5. Valid minimal grammar → Ok
//!   6. BuildOptions defaults / flag combinations
//!   7. Stats validation (state_count, symbol_count, conflict_cells)
//!   8. Build determinism
//!   9. Various grammar patterns, sizes, option combos
//!  10. parser_code / node_types_json content checks

use adze_ir::builder::GrammarBuilder;
use adze_ir::{Associativity, Grammar};
use adze_tool::pure_rust_builder::{BuildOptions, BuildResult, build_parser};
use tempfile::TempDir;

// ── Helpers ─────────────────────────────────────────────────────────────────

fn tmp_opts(emit: bool, compress: bool) -> (TempDir, BuildOptions) {
    let dir = TempDir::new().expect("tempdir");
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().into_owned(),
        emit_artifacts: emit,
        compress_tables: compress,
    };
    (dir, opts)
}

fn default_opts() -> (TempDir, BuildOptions) {
    tmp_opts(false, true)
}

fn run(grammar: Grammar) -> BuildResult {
    let (_dir, opts) = default_opts();
    build_parser(grammar, opts).expect("build_parser should succeed")
}

fn try_build(grammar: Grammar) -> Result<BuildResult, anyhow::Error> {
    let (_dir, opts) = default_opts();
    build_parser(grammar, opts)
}

fn try_build_with(
    grammar: Grammar,
    emit: bool,
    compress: bool,
) -> Result<BuildResult, anyhow::Error> {
    let (_dir, opts) = tmp_opts(emit, compress);
    build_parser(grammar, opts)
}

fn minimal(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("NUMBER", r"\d+")
        .rule("start", vec!["NUMBER"])
        .start("start")
        .build()
}

fn two_alt(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("A", "a")
        .token("B", "b")
        .rule("start", vec!["A"])
        .rule("start", vec!["B"])
        .start("start")
        .build()
}

fn sequence(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("A", "a")
        .token("B", "b")
        .token("C", "c")
        .rule("start", vec!["A", "B", "C"])
        .start("start")
        .build()
}

fn chain(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("X", "x")
        .rule("inner", vec!["X"])
        .rule("start", vec!["inner"])
        .start("start")
        .build()
}

fn arithmetic(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .token("STAR", r"\*")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "STAR", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build()
}

fn many_tokens(name: &str, count: usize) -> Grammar {
    let mut b = GrammarBuilder::new(name);
    let names: Vec<String> = (0..count).map(|i| format!("T{i}")).collect();
    for n in &names {
        b = b.token(n, n);
    }
    for n in &names {
        b = b.rule("start", vec![n.as_str()]);
    }
    b.start("start").build()
}

fn deep_chain(name: &str, depth: usize) -> Grammar {
    let mut b = GrammarBuilder::new(name);
    b = b.token("LEAF", "leaf");
    let names: Vec<String> = (0..depth).map(|i| format!("n{i}")).collect();
    b = b.rule(&names[0], vec!["LEAF"]);
    for i in 1..depth {
        b = b.rule(&names[i], vec![names[i - 1].as_str()]);
    }
    b.start(&names[depth - 1]).build()
}

// =========================================================================
// 1. Empty / default grammar
// =========================================================================

#[test]
fn test_empty_grammar_default_fails() {
    let result = try_build(Grammar::default());
    assert!(result.is_err(), "empty Grammar::default() should fail");
}

#[test]
fn test_empty_grammar_error_message_is_nonempty() {
    let err = try_build(Grammar::default()).unwrap_err();
    let msg = format!("{err}");
    assert!(!msg.is_empty(), "error message should not be empty");
}

#[test]
fn test_empty_grammar_with_emit_artifacts_fails() {
    let result = try_build_with(Grammar::default(), true, true);
    assert!(result.is_err());
}

#[test]
fn test_empty_grammar_with_compress_false_fails() {
    let result = try_build_with(Grammar::default(), false, false);
    assert!(result.is_err());
}

// =========================================================================
// 2. Grammar with no start symbol
// =========================================================================

#[test]
fn test_no_explicit_start_still_builds() {
    // GrammarBuilder infers a start symbol from the first rule
    let g = GrammarBuilder::new("pe_v8_nostart")
        .token("A", "a")
        .rule("start", vec!["A"])
        // no .start()
        .build();
    let result = try_build(g);
    assert!(
        result.is_ok(),
        "grammar without explicit start should still build"
    );
}

#[test]
fn test_no_explicit_start_produces_valid_output() {
    let g = GrammarBuilder::new("pe_v8_nostart2")
        .token("A", "a")
        .rule("start", vec!["A"])
        .build();
    let r = run(g);
    assert!(!r.parser_code.is_empty());
    assert!(r.build_stats.state_count > 0);
}

// =========================================================================
// 3. Grammar with no tokens
// =========================================================================

#[test]
fn test_no_tokens_fails() {
    let g = GrammarBuilder::new("pe_v8_notokens")
        .rule("start", vec!["something"])
        .start("start")
        .build();
    let result = try_build(g);
    assert!(result.is_err(), "grammar with no tokens should fail");
}

#[test]
fn test_no_tokens_error_is_descriptive() {
    let g = GrammarBuilder::new("pe_v8_notokens2")
        .rule("start", vec!["phantom"])
        .start("start")
        .build();
    let err = try_build(g).unwrap_err();
    let msg = format!("{err:?}");
    assert!(!msg.is_empty());
}

// =========================================================================
// 4. Grammar with no rules
// =========================================================================

#[test]
fn test_no_rules_without_explicit_wrapper_relation_fails() {
    // Without explicit wrapper metadata, a start symbol with no rules cannot build.
    let g = GrammarBuilder::new("pe_v8_norules")
        .token("A", "a")
        .start("start")
        .build();
    let result = try_build(g);
    assert!(
        result.is_err(),
        "grammar without rules or explicit wrapper relation should fail: {result:?}"
    );
}

#[test]
fn test_no_rules_no_matching_token_fails() {
    // A grammar with no rules AND no heuristic token match should still fail.
    let g = GrammarBuilder::new("pe_v8_norules_nomatch")
        .token("Z", "z")
        .start("start")
        .build();
    let result = try_build(g);
    assert!(
        result.is_err(),
        "grammar with no rules and no matching token should fail"
    );
}

#[test]
fn test_no_rules_only_tokens_error() {
    let g = GrammarBuilder::new("pe_v8_norules2")
        .token("X", "x")
        .token("Y", "y")
        .start("start")
        .build();
    let err = try_build(g).unwrap_err();
    let msg = format!("{err:?}");
    assert!(!msg.is_empty());
}

// =========================================================================
// 5. Valid minimal grammar → Ok
// =========================================================================

#[test]
fn test_minimal_grammar_succeeds() {
    let result = try_build(minimal("pe_v8_minimal"));
    assert!(result.is_ok(), "minimal grammar should succeed: {result:?}");
}

#[test]
fn test_minimal_grammar_parser_code_nonempty() {
    let r = run(minimal("pe_v8_mincode"));
    assert!(!r.parser_code.is_empty());
}

#[test]
fn test_minimal_grammar_node_types_json_nonempty() {
    let r = run(minimal("pe_v8_minjson"));
    assert!(!r.node_types_json.is_empty());
}

#[test]
fn test_minimal_grammar_name_preserved() {
    let r = run(minimal("pe_v8_namecheck"));
    assert_eq!(r.grammar_name, "pe_v8_namecheck");
}

// =========================================================================
// 6. Default BuildOptions → Ok
// =========================================================================

#[test]
fn test_default_build_options_succeeds() {
    let dir = TempDir::new().unwrap();
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().into_owned(),
        ..BuildOptions::default()
    };
    let result = build_parser(minimal("pe_v8_defopt"), opts);
    assert!(result.is_ok());
}

// =========================================================================
// 7. emit_artifacts: true → Ok
// =========================================================================

#[test]
fn test_emit_artifacts_true_succeeds() {
    let result = try_build_with(minimal("pe_v8_emit_t"), true, true);
    assert!(result.is_ok());
}

#[test]
fn test_emit_artifacts_true_parser_code_nonempty() {
    let r = try_build_with(minimal("pe_v8_emit_t2"), true, true).unwrap();
    assert!(!r.parser_code.is_empty());
}

// =========================================================================
// 8. compress_tables: true → Ok
// =========================================================================

#[test]
fn test_compress_true_succeeds() {
    let result = try_build_with(minimal("pe_v8_comp_t"), false, true);
    assert!(result.is_ok());
}

// =========================================================================
// 9. compress_tables: false → Ok
// =========================================================================

#[test]
fn test_compress_false_succeeds() {
    let result = try_build_with(minimal("pe_v8_comp_f"), false, false);
    assert!(result.is_ok());
}

// =========================================================================
// 10. Both flags true → Ok
// =========================================================================

#[test]
fn test_both_flags_true_succeeds() {
    let result = try_build_with(minimal("pe_v8_both_t"), true, true);
    assert!(result.is_ok());
}

// =========================================================================
// 11. Both flags false → Ok
// =========================================================================

#[test]
fn test_both_flags_false_succeeds() {
    let result = try_build_with(minimal("pe_v8_both_f"), false, false);
    assert!(result.is_ok());
}

// =========================================================================
// 12. Empty out_dir → Ok
// =========================================================================

#[test]
fn test_empty_out_dir_succeeds() {
    let opts = BuildOptions {
        out_dir: String::new(),
        emit_artifacts: false,
        compress_tables: true,
    };
    let result = build_parser(minimal("pe_v8_emptydir"), opts);
    assert!(result.is_ok());
}

// =========================================================================
// 13. out_dir with path → Ok
// =========================================================================

#[test]
fn test_out_dir_with_path_succeeds() {
    let dir = TempDir::new().unwrap();
    let sub = dir.path().join("sub").join("dir");
    std::fs::create_dir_all(&sub).unwrap();
    let opts = BuildOptions {
        out_dir: sub.to_string_lossy().into_owned(),
        emit_artifacts: false,
        compress_tables: true,
    };
    let result = build_parser(minimal("pe_v8_subdir"), opts);
    assert!(result.is_ok());
}

// =========================================================================
// 14. Single rule grammar → small stats
// =========================================================================

#[test]
fn test_single_rule_small_state_count() {
    let r = run(minimal("pe_v8_single_stats"));
    assert!(
        r.build_stats.state_count < 50,
        "state_count={}",
        r.build_stats.state_count
    );
}

#[test]
fn test_single_rule_small_symbol_count() {
    let r = run(minimal("pe_v8_single_sym"));
    assert!(
        r.build_stats.symbol_count < 30,
        "symbol_count={}",
        r.build_stats.symbol_count
    );
}

// =========================================================================
// 15. Large grammar → larger stats
// =========================================================================

#[test]
fn test_large_grammar_more_states() {
    let small = run(minimal("pe_v8_large_sm"));
    let large = run(many_tokens("pe_v8_large_lg", 10));
    assert!(
        large.build_stats.state_count >= small.build_stats.state_count,
        "large grammar should have >= states: small={}, large={}",
        small.build_stats.state_count,
        large.build_stats.state_count,
    );
}

#[test]
fn test_large_grammar_more_symbols() {
    let small = run(minimal("pe_v8_largesym_sm"));
    let large = run(many_tokens("pe_v8_largesym_lg", 10));
    assert!(
        large.build_stats.symbol_count > small.build_stats.symbol_count,
        "large grammar should have more symbols: small={}, large={}",
        small.build_stats.symbol_count,
        large.build_stats.symbol_count,
    );
}

// =========================================================================
// 16. Stats state_count > 0 for valid grammar
// =========================================================================

#[test]
fn test_state_count_positive_minimal() {
    let r = run(minimal("pe_v8_sc_min"));
    assert!(r.build_stats.state_count > 0);
}

#[test]
fn test_state_count_positive_two_alt() {
    let r = run(two_alt("pe_v8_sc_alt"));
    assert!(r.build_stats.state_count > 0);
}

#[test]
fn test_state_count_positive_sequence() {
    let r = run(sequence("pe_v8_sc_seq"));
    assert!(r.build_stats.state_count > 0);
}

// =========================================================================
// 17. Stats symbol_count > 0 for valid grammar
// =========================================================================

#[test]
fn test_symbol_count_positive_minimal() {
    let r = run(minimal("pe_v8_sym_min"));
    assert!(r.build_stats.symbol_count > 0);
}

#[test]
fn test_symbol_count_positive_chain() {
    let r = run(chain("pe_v8_sym_chain"));
    assert!(r.build_stats.symbol_count > 0);
}

#[test]
fn test_symbol_count_positive_arithmetic() {
    let r = run(arithmetic("pe_v8_sym_arith"));
    assert!(r.build_stats.symbol_count > 0);
}

// =========================================================================
// 18. Stats conflict_cells >= 0 (always true for usize, but check semantics)
// =========================================================================

#[test]
fn test_conflict_cells_zero_for_simple_grammar() {
    let r = run(minimal("pe_v8_cc_simple"));
    // Simple LL(1) grammars should have zero conflicts
    assert_eq!(r.build_stats.conflict_cells, 0);
}

#[test]
fn test_conflict_cells_for_ambiguous_grammar() {
    // Arithmetic with precedence may resolve conflicts at table level
    let r = run(arithmetic("pe_v8_cc_arith"));
    // Just verify it's a valid number (non-negative is guaranteed by usize)
    let _ = r.build_stats.conflict_cells;
}

// =========================================================================
// 19. Build is deterministic (same grammar → same result)
// =========================================================================

#[test]
fn test_deterministic_parser_code() {
    let g1 = minimal("pe_v8_det1");
    let g2 = minimal("pe_v8_det1");
    let r1 = run(g1);
    let r2 = run(g2);
    assert_eq!(r1.parser_code, r2.parser_code);
}

#[test]
fn test_deterministic_node_types_json() {
    let g1 = minimal("pe_v8_det2");
    let g2 = minimal("pe_v8_det2");
    let r1 = run(g1);
    let r2 = run(g2);
    assert_eq!(r1.node_types_json, r2.node_types_json);
}

#[test]
fn test_deterministic_stats() {
    let g1 = minimal("pe_v8_det3");
    let g2 = minimal("pe_v8_det3");
    let r1 = run(g1);
    let r2 = run(g2);
    assert_eq!(r1.build_stats.state_count, r2.build_stats.state_count);
    assert_eq!(r1.build_stats.symbol_count, r2.build_stats.symbol_count);
    assert_eq!(r1.build_stats.conflict_cells, r2.build_stats.conflict_cells);
}

#[test]
fn test_deterministic_two_alt() {
    let g1 = two_alt("pe_v8_det4");
    let g2 = two_alt("pe_v8_det4");
    let r1 = run(g1);
    let r2 = run(g2);
    assert_eq!(r1.parser_code, r2.parser_code);
}

// =========================================================================
// 20. Various grammar sizes → all Ok
// =========================================================================

#[test]
fn test_three_tokens_ok() {
    let result = try_build(many_tokens("pe_v8_sz3", 3));
    assert!(result.is_ok());
}

#[test]
fn test_five_tokens_ok() {
    let result = try_build(many_tokens("pe_v8_sz5", 5));
    assert!(result.is_ok());
}

#[test]
fn test_eight_tokens_ok() {
    let result = try_build(many_tokens("pe_v8_sz8", 8));
    assert!(result.is_ok());
}

#[test]
fn test_twelve_tokens_ok() {
    let result = try_build(many_tokens("pe_v8_sz12", 12));
    assert!(result.is_ok());
}

// =========================================================================
// 21–30. Different grammar patterns
// =========================================================================

#[test]
fn test_chain_grammar_ok() {
    let result = try_build(chain("pe_v8_chain"));
    assert!(result.is_ok());
}

#[test]
fn test_sequence_grammar_ok() {
    let result = try_build(sequence("pe_v8_seq"));
    assert!(result.is_ok());
}

#[test]
fn test_arithmetic_grammar_ok() {
    let result = try_build(arithmetic("pe_v8_arith"));
    assert!(result.is_ok());
}

#[test]
fn test_deep_chain_3_ok() {
    let result = try_build(deep_chain("pe_v8_deep3", 3));
    assert!(result.is_ok());
}

#[test]
fn test_deep_chain_5_ok() {
    let result = try_build(deep_chain("pe_v8_deep5", 5));
    assert!(result.is_ok());
}

#[test]
fn test_deep_chain_8_ok() {
    let result = try_build(deep_chain("pe_v8_deep8", 8));
    assert!(result.is_ok());
}

#[test]
fn test_right_assoc_grammar_ok() {
    let g = GrammarBuilder::new("pe_v8_rassoc")
        .token("A", "a")
        .token("OP", r"\^")
        .rule_with_precedence("expr", vec!["expr", "OP", "expr"], 1, Associativity::Right)
        .rule("expr", vec!["A"])
        .start("expr")
        .build();
    let result = try_build(g);
    assert!(result.is_ok());
}

#[test]
fn test_none_assoc_grammar_ok() {
    let g = GrammarBuilder::new("pe_v8_nassoc")
        .token("A", "a")
        .token("EQ", "=")
        .rule_with_precedence("expr", vec!["expr", "EQ", "expr"], 1, Associativity::None)
        .rule("expr", vec!["A"])
        .start("expr")
        .build();
    let result = try_build(g);
    assert!(result.is_ok());
}

#[test]
fn test_multiple_precedence_levels_ok() {
    let g = GrammarBuilder::new("pe_v8_multiprec")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .token("STAR", r"\*")
        .token("HAT", r"\^")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "STAR", "expr"], 2, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "HAT", "expr"], 3, Associativity::Right)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let result = try_build(g);
    assert!(result.is_ok());
}

#[test]
fn test_extra_whitespace_token_ok() {
    let g = GrammarBuilder::new("pe_v8_extras")
        .token("A", "a")
        .token("WS", r"\s+")
        .rule("start", vec!["A"])
        .start("start")
        .extra("WS")
        .build();
    let result = try_build(g);
    assert!(result.is_ok());
}

// =========================================================================
// 31–40. parser_code content checks
// =========================================================================

#[test]
fn test_parser_code_is_valid_utf8() {
    let r = run(minimal("pe_v8_utf8"));
    // parser_code is a String so already valid UTF-8, but confirm non-trivial
    assert!(r.parser_code.len() > 10);
}

#[test]
fn test_parser_code_contains_language_reference() {
    let r = run(minimal("pe_v8_langref"));
    // Generated code should reference the language struct or parse tables
    assert!(
        r.parser_code.contains("language") || r.parser_code.contains("LANGUAGE"),
        "parser code should reference language: first 200 chars = {:?}",
        &r.parser_code[..r.parser_code.len().min(200)],
    );
}

#[test]
fn test_parser_code_differs_for_different_grammars() {
    let r1 = run(minimal("pe_v8_diff1"));
    let r2 = run(two_alt("pe_v8_diff2"));
    assert_ne!(r1.parser_code, r2.parser_code);
}

#[test]
fn test_parser_code_contains_state_data() {
    let r = run(minimal("pe_v8_statedata"));
    // Parser code should contain numeric data for parse tables
    assert!(
        r.parser_code.contains('0') || r.parser_code.contains('1'),
        "parser code should contain numeric data"
    );
}

#[test]
fn test_parser_code_for_chain_grammar() {
    let r = run(chain("pe_v8_chaincode"));
    assert!(!r.parser_code.is_empty());
}

#[test]
fn test_parser_code_for_sequence_grammar() {
    let r = run(sequence("pe_v8_seqcode"));
    assert!(!r.parser_code.is_empty());
}

#[test]
fn test_parser_code_for_arithmetic_grammar() {
    let r = run(arithmetic("pe_v8_arithcode"));
    assert!(!r.parser_code.is_empty());
}

#[test]
fn test_parser_code_length_grows_with_grammar_size() {
    let r_small = run(minimal("pe_v8_pclen_s"));
    let r_large = run(many_tokens("pe_v8_pclen_l", 10));
    assert!(
        r_large.parser_code.len() >= r_small.parser_code.len(),
        "larger grammar should produce at least as much code"
    );
}

// =========================================================================
// 41–50. node_types_json checks
// =========================================================================

#[test]
fn test_node_types_json_is_valid_json() {
    let r = run(minimal("pe_v8_jsonvalid"));
    let parsed: serde_json::Value =
        serde_json::from_str(&r.node_types_json).expect("node_types_json should be valid JSON");
    assert!(parsed.is_array() || parsed.is_object());
}

#[test]
fn test_node_types_json_is_array() {
    let r = run(minimal("pe_v8_jsonarr"));
    let parsed: serde_json::Value = serde_json::from_str(&r.node_types_json).expect("valid JSON");
    assert!(parsed.is_array(), "node_types should be a JSON array");
}

#[test]
fn test_node_types_json_not_empty_array() {
    let r = run(minimal("pe_v8_jsonnotempty"));
    let parsed: Vec<serde_json::Value> =
        serde_json::from_str(&r.node_types_json).expect("valid JSON array");
    assert!(!parsed.is_empty(), "node_types should not be empty");
}

#[test]
fn test_node_types_json_entries_have_type() {
    let r = run(minimal("pe_v8_jsontype"));
    let parsed: Vec<serde_json::Value> =
        serde_json::from_str(&r.node_types_json).expect("valid JSON array");
    for entry in &parsed {
        assert!(
            entry.get("type").is_some(),
            "each entry should have 'type' field"
        );
    }
}

#[test]
fn test_node_types_json_entries_have_named() {
    let r = run(minimal("pe_v8_jsonnamed"));
    let parsed: Vec<serde_json::Value> =
        serde_json::from_str(&r.node_types_json).expect("valid JSON array");
    for entry in &parsed {
        assert!(
            entry.get("named").is_some(),
            "each entry should have 'named' field"
        );
    }
}

#[test]
fn test_node_types_json_for_two_alt() {
    let r = run(two_alt("pe_v8_json_alt"));
    let parsed: Vec<serde_json::Value> =
        serde_json::from_str(&r.node_types_json).expect("valid JSON array");
    assert!(!parsed.is_empty());
}

#[test]
fn test_node_types_json_for_chain() {
    let r = run(chain("pe_v8_json_chain"));
    let parsed: Vec<serde_json::Value> =
        serde_json::from_str(&r.node_types_json).expect("valid JSON array");
    assert!(!parsed.is_empty());
}

#[test]
fn test_node_types_json_for_sequence() {
    let r = run(sequence("pe_v8_json_seq"));
    let parsed: Vec<serde_json::Value> =
        serde_json::from_str(&r.node_types_json).expect("valid JSON array");
    assert!(!parsed.is_empty());
}

#[test]
fn test_node_types_json_for_arithmetic() {
    let r = run(arithmetic("pe_v8_json_arith"));
    let parsed: Vec<serde_json::Value> =
        serde_json::from_str(&r.node_types_json).expect("valid JSON array");
    assert!(!parsed.is_empty());
}

#[test]
fn test_node_types_json_deterministic() {
    let g1 = minimal("pe_v8_json_det");
    let g2 = minimal("pe_v8_json_det");
    let r1 = run(g1);
    let r2 = run(g2);
    assert_eq!(r1.node_types_json, r2.node_types_json);
}

// =========================================================================
// 51–60. Option combos with various grammars
// =========================================================================

#[test]
fn test_two_alt_compress_true() {
    let result = try_build_with(two_alt("pe_v8_opt1"), false, true);
    assert!(result.is_ok());
}

#[test]
fn test_two_alt_compress_false() {
    let result = try_build_with(two_alt("pe_v8_opt2"), false, false);
    assert!(result.is_ok());
}

#[test]
fn test_two_alt_emit_true() {
    let result = try_build_with(two_alt("pe_v8_opt3"), true, true);
    assert!(result.is_ok());
}

#[test]
fn test_chain_compress_false() {
    let result = try_build_with(chain("pe_v8_opt4"), false, false);
    assert!(result.is_ok());
}

#[test]
fn test_chain_emit_true() {
    let result = try_build_with(chain("pe_v8_opt5"), true, true);
    assert!(result.is_ok());
}

#[test]
fn test_sequence_compress_false() {
    let result = try_build_with(sequence("pe_v8_opt6"), false, false);
    assert!(result.is_ok());
}

#[test]
fn test_sequence_emit_true() {
    let result = try_build_with(sequence("pe_v8_opt7"), true, false);
    assert!(result.is_ok());
}

#[test]
fn test_arithmetic_compress_false() {
    let result = try_build_with(arithmetic("pe_v8_opt8"), false, false);
    assert!(result.is_ok());
}

#[test]
fn test_arithmetic_emit_true() {
    let result = try_build_with(arithmetic("pe_v8_opt9"), true, true);
    assert!(result.is_ok());
}

#[test]
fn test_many_tokens_compress_false() {
    let result = try_build_with(many_tokens("pe_v8_opt10", 6), false, false);
    assert!(result.is_ok());
}

// =========================================================================
// 61–70. Stat validation across grammars
// =========================================================================

#[test]
fn test_stats_chain_state_count() {
    let r = run(chain("pe_v8_stchain"));
    assert!(r.build_stats.state_count > 0);
}

#[test]
fn test_stats_chain_symbol_count() {
    let r = run(chain("pe_v8_stchain2"));
    assert!(r.build_stats.symbol_count > 0);
}

#[test]
fn test_stats_sequence_state_count() {
    let r = run(sequence("pe_v8_stseq"));
    assert!(r.build_stats.state_count > 0);
}

#[test]
fn test_stats_sequence_symbol_count() {
    let r = run(sequence("pe_v8_stseq2"));
    assert!(r.build_stats.symbol_count > 0);
}

#[test]
fn test_stats_arithmetic_state_count() {
    let r = run(arithmetic("pe_v8_starith"));
    assert!(r.build_stats.state_count > 0);
}

#[test]
fn test_stats_arithmetic_symbol_count() {
    let r = run(arithmetic("pe_v8_starith2"));
    assert!(r.build_stats.symbol_count > 0);
}

#[test]
fn test_stats_many_tokens_state_count() {
    let r = run(many_tokens("pe_v8_stmany", 7));
    assert!(r.build_stats.state_count > 0);
}

#[test]
fn test_stats_many_tokens_symbol_count() {
    let r = run(many_tokens("pe_v8_stmany2", 7));
    assert!(r.build_stats.symbol_count > 0);
}

#[test]
fn test_stats_deep_chain_state_count() {
    let r = run(deep_chain("pe_v8_stdeep", 4));
    assert!(r.build_stats.state_count > 0);
}

#[test]
fn test_stats_deep_chain_symbol_count() {
    let r = run(deep_chain("pe_v8_stdeep2", 4));
    assert!(r.build_stats.symbol_count > 0);
}

// =========================================================================
// 71–80. BuildStats Clone, grammar_name, parser_path, more patterns
// =========================================================================

#[test]
fn test_build_stats_clone() {
    let r = run(minimal("pe_v8_clone"));
    let stats_clone = r.build_stats.clone();
    assert_eq!(stats_clone.state_count, r.build_stats.state_count);
    assert_eq!(stats_clone.symbol_count, r.build_stats.symbol_count);
    assert_eq!(stats_clone.conflict_cells, r.build_stats.conflict_cells);
}

#[test]
fn test_grammar_name_in_result() {
    let r = run(two_alt("pe_v8_gname"));
    assert_eq!(r.grammar_name, "pe_v8_gname");
}

#[test]
fn test_parser_path_nonempty() {
    let r = run(minimal("pe_v8_ppath"));
    assert!(!r.parser_path.is_empty());
}

#[test]
fn test_parser_path_for_chain() {
    let r = run(chain("pe_v8_ppath2"));
    assert!(!r.parser_path.is_empty());
}

#[test]
fn test_compress_true_vs_false_same_stats() {
    let r_comp = try_build_with(minimal("pe_v8_cmpstat"), false, true).unwrap();
    let r_nocomp = try_build_with(minimal("pe_v8_cmpstat"), false, false).unwrap();
    // Stats come from the parse table, not compression, so should match
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
fn test_emit_true_vs_false_same_code() {
    let r_emit = try_build_with(minimal("pe_v8_emitcmp"), true, true).unwrap();
    let r_noemit = try_build_with(minimal("pe_v8_emitcmp"), false, true).unwrap();
    assert_eq!(r_emit.parser_code, r_noemit.parser_code);
}

#[test]
fn test_inline_rule_grammar_ok() {
    let g = GrammarBuilder::new("pe_v8_inline")
        .token("A", "a")
        .rule("helper", vec!["A"])
        .rule("start", vec!["helper"])
        .start("start")
        .inline("helper")
        .build();
    let result = try_build(g);
    assert!(result.is_ok());
}

#[test]
fn test_supertype_grammar_ok() {
    let g = GrammarBuilder::new("pe_v8_super")
        .token("A", "a")
        .token("B", "b")
        .rule("item", vec!["A"])
        .rule("item", vec!["B"])
        .rule("start", vec!["item"])
        .start("start")
        .supertype("item")
        .build();
    let result = try_build(g);
    assert!(result.is_ok());
}

#[test]
fn test_external_token_grammar_ok() {
    let g = GrammarBuilder::new("pe_v8_extern")
        .token("A", "a")
        .rule("start", vec!["A"])
        .start("start")
        .external("EXT")
        .build();
    let result = try_build(g);
    assert!(result.is_ok());
}

#[test]
fn test_grammar_with_regex_tokens() {
    let g = GrammarBuilder::new("pe_v8_regex")
        .token("IDENT", r"[a-zA-Z_][a-zA-Z0-9_]*")
        .token("NUM", r"[0-9]+")
        .token("STR", r#""[^"]*""#)
        .rule("start", vec!["IDENT"])
        .rule("start", vec!["NUM"])
        .rule("start", vec!["STR"])
        .start("start")
        .build();
    let result = try_build(g);
    assert!(result.is_ok());
}

// =========================================================================
// 81–90. Additional edge cases
// =========================================================================

#[test]
fn test_single_char_token_ok() {
    let g = GrammarBuilder::new("pe_v8_char")
        .token("X", "x")
        .rule("start", vec!["X"])
        .start("start")
        .build();
    assert!(try_build(g).is_ok());
}

#[test]
fn test_multiple_rules_same_lhs() {
    let g = GrammarBuilder::new("pe_v8_multirule")
        .token("A", "a")
        .token("B", "b")
        .token("C", "c")
        .token("D", "d")
        .rule("start", vec!["A"])
        .rule("start", vec!["B"])
        .rule("start", vec!["C"])
        .rule("start", vec!["D"])
        .start("start")
        .build();
    let r = run(g);
    assert!(r.build_stats.state_count > 0);
}

#[test]
fn test_multiple_nonterminals() {
    let g = GrammarBuilder::new("pe_v8_multinont")
        .token("A", "a")
        .token("B", "b")
        .rule("alpha", vec!["A"])
        .rule("beta", vec!["B"])
        .rule("start", vec!["alpha"])
        .rule("start", vec!["beta"])
        .start("start")
        .build();
    assert!(try_build(g).is_ok());
}

#[test]
fn test_two_token_sequence() {
    let g = GrammarBuilder::new("pe_v8_twoseq")
        .token("L", "(")
        .token("R", ")")
        .rule("start", vec!["L", "R"])
        .start("start")
        .build();
    assert!(try_build(g).is_ok());
}

#[test]
fn test_three_level_chain() {
    let g = GrammarBuilder::new("pe_v8_3chain")
        .token("X", "x")
        .rule("c", vec!["X"])
        .rule("b", vec!["c"])
        .rule("a", vec!["b"])
        .start("a")
        .build();
    assert!(try_build(g).is_ok());
}

#[test]
fn test_stats_debug_format() {
    let r = run(minimal("pe_v8_dbg"));
    let debug = format!("{:?}", r.build_stats);
    assert!(debug.contains("state_count"));
    assert!(debug.contains("symbol_count"));
    assert!(debug.contains("conflict_cells"));
}

#[test]
fn test_build_result_debug_format() {
    let r = run(minimal("pe_v8_brdbg"));
    let debug = format!("{:?}", r);
    assert!(debug.contains("grammar_name"));
}

#[test]
fn test_build_options_debug_format() {
    let (_dir, opts) = default_opts();
    let debug = format!("{:?}", opts);
    assert!(debug.contains("out_dir"));
    assert!(debug.contains("emit_artifacts"));
    assert!(debug.contains("compress_tables"));
}

#[test]
fn test_build_options_clone() {
    let (_dir, opts) = default_opts();
    let cloned = opts.clone();
    assert_eq!(cloned.out_dir, opts.out_dir);
    assert_eq!(cloned.emit_artifacts, opts.emit_artifacts);
    assert_eq!(cloned.compress_tables, opts.compress_tables);
}

#[test]
fn test_node_types_json_differs_for_different_grammars() {
    let r1 = run(minimal("pe_v8_jdiff1"));
    let r2 = run(arithmetic("pe_v8_jdiff2"));
    // Different grammars should produce different node_types
    assert_ne!(r1.node_types_json, r2.node_types_json);
}

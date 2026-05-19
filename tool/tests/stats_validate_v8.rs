//! Comprehensive tests for `BuildStats` validation and consistency in adze-tool.
//!
//! 84 tests across 21 categories:
//!  1. Minimal grammar → state_count > 0
//!  2. Minimal grammar → symbol_count > 0
//!  3. Minimal grammar → conflict_cells == 0 (unambiguous)
//!  4. Adding rules → state_count grows or stays same
//!  5. Adding tokens → symbol_count grows
//!  6. Same grammar → same stats (determinism)
//!  7. Stats are deterministic across repeated builds
//!  8. state_count >= 2 for non-trivial grammar
//!  9. symbol_count >= num_tokens + num_nonterminals
//! 10. conflict_cells >= 0 (always; bounded by table size)
//! 11. Grammar with precedence → stats
//! 12. Grammar with alternatives → more states
//! 13. Grammar with chain rules → stats
//! 14. Grammar with recursion → stats
//! 15. Stats Clone equals original
//! 16. Stats Debug is non-empty
//! 17. Comparing stats across grammar sizes
//! 18. 1-rule grammar stats
//! 19. 5-rule grammar stats
//! 20. 10-rule grammar stats
//! 21. Cross-field consistency invariants

use adze_ir::builder::GrammarBuilder;
use adze_ir::{Associativity, Grammar};
use adze_tool::pure_rust_builder::{BuildOptions, BuildResult, BuildStats, build_parser};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn tmp_opts() -> (TempDir, BuildOptions) {
    let dir = TempDir::new().unwrap();
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: false,
    };
    (dir, opts)
}

fn do_build(g: Grammar) -> BuildResult {
    let (_dir, opts) = tmp_opts();
    build_parser(g, opts).expect("build should succeed")
}

fn stats(g: Grammar) -> BuildStats {
    do_build(g).build_stats
}

/// 1-rule grammar: source_file -> NUMBER
fn minimal(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("NUMBER", r"\d+")
        .rule("source_file", vec!["NUMBER"])
        .start("source_file")
        .build()
}

/// 2-alternative grammar: source_file -> NUMBER | IDENT
fn two_alt(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("NUMBER", r"\d+")
        .token("IDENT", r"[a-z]+")
        .rule("source_file", vec!["NUMBER"])
        .rule("source_file", vec!["IDENT"])
        .start("source_file")
        .build()
}

/// 3-alternative grammar: source_file -> NUMBER | IDENT | STRING
fn three_alt(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("NUMBER", r"\d+")
        .token("IDENT", r"[a-z]+")
        .token("STRING", r#""[^"]*""#)
        .rule("source_file", vec!["NUMBER"])
        .rule("source_file", vec!["IDENT"])
        .rule("source_file", vec!["STRING"])
        .start("source_file")
        .build()
}

/// Left-recursive arithmetic: source_file -> NUMBER | source_file PLUS source_file
fn arith_left(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("NUMBER", r"\d+")
        .token("PLUS", "+")
        .rule("source_file", vec!["NUMBER"])
        .rule_with_precedence(
            "source_file",
            vec!["source_file", "PLUS", "source_file"],
            1,
            Associativity::Left,
        )
        .start("source_file")
        .build()
}

/// Right-recursive arithmetic: source_file -> NUMBER | source_file CARET source_file
fn arith_right(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("NUMBER", r"\d+")
        .token("CARET", "^")
        .rule("source_file", vec!["NUMBER"])
        .rule_with_precedence(
            "source_file",
            vec!["source_file", "CARET", "source_file"],
            1,
            Associativity::Right,
        )
        .start("source_file")
        .build()
}

/// Two-operator grammar: +, *
fn two_op(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("NUMBER", r"\d+")
        .token("PLUS", "+")
        .token("STAR", "*")
        .rule("source_file", vec!["NUMBER"])
        .rule_with_precedence(
            "source_file",
            vec!["source_file", "PLUS", "source_file"],
            1,
            Associativity::Left,
        )
        .rule_with_precedence(
            "source_file",
            vec!["source_file", "STAR", "source_file"],
            2,
            Associativity::Left,
        )
        .start("source_file")
        .build()
}

/// Three-operator grammar: +, *, -
fn three_op(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("NUMBER", r"\d+")
        .token("PLUS", "+")
        .token("STAR", "*")
        .token("MINUS", "-")
        .rule("source_file", vec!["NUMBER"])
        .rule_with_precedence(
            "source_file",
            vec!["source_file", "PLUS", "source_file"],
            1,
            Associativity::Left,
        )
        .rule_with_precedence(
            "source_file",
            vec!["source_file", "STAR", "source_file"],
            2,
            Associativity::Left,
        )
        .rule_with_precedence(
            "source_file",
            vec!["source_file", "MINUS", "source_file"],
            1,
            Associativity::Left,
        )
        .start("source_file")
        .build()
}

/// Chain rule: source_file -> atom, atom -> NUMBER
fn chain(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("NUMBER", r"\d+")
        .rule("atom", vec!["NUMBER"])
        .rule("source_file", vec!["atom"])
        .start("source_file")
        .build()
}

/// Deep chain: source_file -> wrapper -> atom -> NUMBER
fn deep_chain(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("NUMBER", r"\d+")
        .rule("atom", vec!["NUMBER"])
        .rule("wrapper", vec!["atom"])
        .rule("source_file", vec!["wrapper"])
        .start("source_file")
        .build()
}

/// Left-recursive list: source_file -> item | source_file item; item -> NUMBER
fn left_recursive(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("NUMBER", r"\d+")
        .rule("item", vec!["NUMBER"])
        .rule("source_file", vec!["item"])
        .rule("source_file", vec!["source_file", "item"])
        .start("source_file")
        .build()
}

/// N-alternative grammar: source_file -> TOK_0 | TOK_1 | ... | TOK_{n-1}
fn n_alt(name: &str, n: usize) -> Grammar {
    let mut b = GrammarBuilder::new(name);
    for i in 0..n {
        let tok_name = format!("TOK_{i}");
        let pattern = format!("t{i}");
        b = b.token(
            Box::leak(tok_name.clone().into_boxed_str()),
            Box::leak(pattern.into_boxed_str()),
        );
        b = b.rule("source_file", vec![Box::leak(tok_name.into_boxed_str())]);
    }
    b.start("source_file").build()
}

/// Sequence grammar: source_file -> TOK_0 TOK_1 ... TOK_{n-1}
fn seq(name: &str, n: usize) -> Grammar {
    let mut b = GrammarBuilder::new(name);
    let mut rhs_names: Vec<&'static str> = Vec::new();
    for i in 0..n {
        let tok_name = format!("TOK_{i}");
        let pattern = format!("s{i}");
        b = b.token(
            Box::leak(tok_name.clone().into_boxed_str()),
            Box::leak(pattern.into_boxed_str()),
        );
        rhs_names.push(Box::leak(tok_name.into_boxed_str()));
    }
    b = b.rule("source_file", rhs_names);
    b.start("source_file").build()
}

/// 5-rule grammar: source_file -> stmt; stmt -> assign | call | ret | break_s | NUMBER
fn five_rule(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("NUMBER", r"\d+")
        .token("IDENT", r"[a-z]+")
        .token("EQ", "=")
        .token("LPAREN", "(")
        .token("RPAREN", ")")
        .rule("assign", vec!["IDENT", "EQ", "NUMBER"])
        .rule("call", vec!["IDENT", "LPAREN", "RPAREN"])
        .rule("ret", vec!["NUMBER"])
        .rule("break_s", vec!["IDENT"])
        .rule("literal", vec!["NUMBER"])
        .rule("source_file", vec!["assign"])
        .rule("source_file", vec!["call"])
        .rule("source_file", vec!["ret"])
        .rule("source_file", vec!["break_s"])
        .rule("source_file", vec!["literal"])
        .start("source_file")
        .build()
}

/// 10-rule grammar: extends five_rule with more statement forms
fn ten_rule(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("NUMBER", r"\d+")
        .token("IDENT", r"[a-z]+")
        .token("EQ", "=")
        .token("LPAREN", "(")
        .token("RPAREN", ")")
        .token("LBRACE", "{")
        .token("RBRACE", "}")
        .token("SEMI", ";")
        .token("COMMA", ",")
        .token("COLON", ":")
        .rule("assign", vec!["IDENT", "EQ", "NUMBER"])
        .rule("call", vec!["IDENT", "LPAREN", "RPAREN"])
        .rule("ret", vec!["NUMBER"])
        .rule("break_s", vec!["IDENT"])
        .rule("literal", vec!["NUMBER"])
        .rule("block", vec!["LBRACE", "RBRACE"])
        .rule("label", vec!["IDENT", "COLON"])
        .rule("pair", vec!["IDENT", "COMMA", "IDENT"])
        .rule("grouped", vec!["LPAREN", "NUMBER", "RPAREN"])
        .rule("terminated", vec!["NUMBER", "SEMI"])
        .rule("source_file", vec!["assign"])
        .rule("source_file", vec!["call"])
        .rule("source_file", vec!["ret"])
        .rule("source_file", vec!["break_s"])
        .rule("source_file", vec!["literal"])
        .rule("source_file", vec!["block"])
        .rule("source_file", vec!["label"])
        .rule("source_file", vec!["pair"])
        .rule("source_file", vec!["grouped"])
        .rule("source_file", vec!["terminated"])
        .start("source_file")
        .build()
}

/// Grammar with precedence declarations
fn with_prec_decl(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("NUMBER", r"\d+")
        .token("PLUS", "+")
        .token("STAR", "*")
        .precedence(1, Associativity::Left, vec!["PLUS"])
        .precedence(2, Associativity::Left, vec!["STAR"])
        .rule("source_file", vec!["NUMBER"])
        .rule_with_precedence(
            "source_file",
            vec!["source_file", "PLUS", "source_file"],
            1,
            Associativity::Left,
        )
        .rule_with_precedence(
            "source_file",
            vec!["source_file", "STAR", "source_file"],
            2,
            Associativity::Left,
        )
        .start("source_file")
        .build()
}

// =========================================================================
// 1. Minimal grammar → state_count > 0 (4 tests)
// =========================================================================

#[test]
fn sv_v8_state_count_positive_minimal() {
    let s = stats(minimal("sv_v8_scp_min"));
    assert!(s.state_count > 0, "state_count must be positive");
}

#[test]
fn sv_v8_state_count_positive_two_alt() {
    let s = stats(two_alt("sv_v8_scp_2a"));
    assert!(s.state_count > 0);
}

#[test]
fn sv_v8_state_count_positive_chain() {
    let s = stats(chain("sv_v8_scp_ch"));
    assert!(s.state_count > 0);
}

#[test]
fn sv_v8_state_count_positive_recursive() {
    let s = stats(left_recursive("sv_v8_scp_lr"));
    assert!(s.state_count > 0);
}

// =========================================================================
// 2. Minimal grammar → symbol_count > 0 (4 tests)
// =========================================================================

#[test]
fn sv_v8_symbol_count_positive_minimal() {
    let s = stats(minimal("sv_v8_syp_min"));
    assert!(s.symbol_count > 0, "symbol_count must be positive");
}

#[test]
fn sv_v8_symbol_count_positive_two_alt() {
    let s = stats(two_alt("sv_v8_syp_2a"));
    assert!(s.symbol_count > 0);
}

#[test]
fn sv_v8_symbol_count_positive_five_rule() {
    let s = stats(five_rule("sv_v8_syp_5r"));
    assert!(s.symbol_count > 0);
}

#[test]
fn sv_v8_symbol_count_positive_ten_rule() {
    let s = stats(ten_rule("sv_v8_syp_10r"));
    assert!(s.symbol_count > 0);
}

// =========================================================================
// 3. Minimal grammar → conflict_cells == 0 (unambiguous) (4 tests)
// =========================================================================

#[test]
fn sv_v8_no_conflicts_minimal() {
    let s = stats(minimal("sv_v8_nc_min"));
    assert_eq!(
        s.conflict_cells, 0,
        "minimal unambiguous grammar should have no conflicts"
    );
}

#[test]
fn sv_v8_no_conflicts_two_alt() {
    let s = stats(two_alt("sv_v8_nc_2a"));
    assert_eq!(s.conflict_cells, 0, "simple alternatives are unambiguous");
}

#[test]
fn sv_v8_no_conflicts_chain() {
    let s = stats(chain("sv_v8_nc_ch"));
    assert_eq!(s.conflict_cells, 0, "chain rules are unambiguous");
}

#[test]
fn sv_v8_no_conflicts_sequence() {
    let s = stats(seq("sv_v8_nc_seq", 4));
    assert_eq!(s.conflict_cells, 0, "sequence grammar is unambiguous");
}

// =========================================================================
// 4. Adding rules → state_count grows or stays same (4 tests)
// =========================================================================

#[test]
fn sv_v8_more_rules_minimal_vs_arith() {
    let s_min = stats(minimal("sv_v8_mr_min"));
    let s_ar = stats(arith_left("sv_v8_mr_ar"));
    assert!(
        s_ar.state_count >= s_min.state_count,
        "arith ({}) should have >= minimal ({}) states",
        s_ar.state_count,
        s_min.state_count,
    );
}

#[test]
fn sv_v8_more_rules_arith_vs_two_op() {
    let s_ar = stats(arith_left("sv_v8_mr_ar2"));
    let s_2o = stats(two_op("sv_v8_mr_2o"));
    assert!(
        s_2o.state_count >= s_ar.state_count,
        "two-op ({}) should have >= single-op ({}) states",
        s_2o.state_count,
        s_ar.state_count,
    );
}

#[test]
fn sv_v8_more_rules_two_op_vs_three_op() {
    let s_2o = stats(two_op("sv_v8_mr_2ob"));
    let s_3o = stats(three_op("sv_v8_mr_3o"));
    assert!(s_3o.state_count >= s_2o.state_count);
}

#[test]
fn sv_v8_more_rules_seq_grows() {
    let s2 = stats(seq("sv_v8_mr_s2", 2));
    let s6 = stats(seq("sv_v8_mr_s6", 6));
    assert!(
        s6.state_count >= s2.state_count,
        "longer sequence ({}) should have >= shorter ({}) states",
        s6.state_count,
        s2.state_count,
    );
}

// =========================================================================
// 5. Adding tokens → symbol_count grows (4 tests)
// =========================================================================

#[test]
fn sv_v8_more_tokens_1_vs_2() {
    let s1 = stats(minimal("sv_v8_mt_1"));
    let s2 = stats(two_alt("sv_v8_mt_2"));
    assert!(
        s2.symbol_count >= s1.symbol_count,
        "2-token ({}) should have >= 1-token ({}) symbols",
        s2.symbol_count,
        s1.symbol_count,
    );
}

#[test]
fn sv_v8_more_tokens_2_vs_3() {
    let s2 = stats(two_alt("sv_v8_mt_2b"));
    let s3 = stats(three_alt("sv_v8_mt_3"));
    assert!(s3.symbol_count >= s2.symbol_count);
}

#[test]
fn sv_v8_more_tokens_5_vs_10() {
    let s5 = stats(n_alt("sv_v8_mt_5", 5));
    let s10 = stats(n_alt("sv_v8_mt_10", 10));
    assert!(s10.symbol_count > s5.symbol_count);
}

#[test]
fn sv_v8_more_tokens_3_vs_15() {
    let s3 = stats(n_alt("sv_v8_mt_3c", 3));
    let s15 = stats(n_alt("sv_v8_mt_15", 15));
    assert!(s15.symbol_count > s3.symbol_count);
}

// =========================================================================
// 6. Same grammar → same stats (determinism) (4 tests)
// =========================================================================

#[test]
fn sv_v8_same_grammar_same_stats_minimal() {
    let a = stats(minimal("sv_v8_sg_min_a"));
    let b = stats(minimal("sv_v8_sg_min_b"));
    assert_eq!(a.state_count, b.state_count);
    assert_eq!(a.symbol_count, b.symbol_count);
    assert_eq!(a.conflict_cells, b.conflict_cells);
}

#[test]
fn sv_v8_same_grammar_same_stats_arith() {
    let a = stats(arith_left("sv_v8_sg_ar_a"));
    let b = stats(arith_left("sv_v8_sg_ar_b"));
    assert_eq!(a.state_count, b.state_count);
    assert_eq!(a.symbol_count, b.symbol_count);
    assert_eq!(a.conflict_cells, b.conflict_cells);
}

#[test]
fn sv_v8_same_grammar_same_stats_two_op() {
    let a = stats(two_op("sv_v8_sg_2o_a"));
    let b = stats(two_op("sv_v8_sg_2o_b"));
    assert_eq!(a.state_count, b.state_count);
    assert_eq!(a.symbol_count, b.symbol_count);
}

#[test]
fn sv_v8_same_grammar_same_stats_n_alt() {
    let a = stats(n_alt("sv_v8_sg_na_a", 7));
    let b = stats(n_alt("sv_v8_sg_na_b", 7));
    assert_eq!(a.state_count, b.state_count);
    assert_eq!(a.symbol_count, b.symbol_count);
}

// =========================================================================
// 7. Stats are deterministic across repeated builds (4 tests)
// =========================================================================

#[test]
fn sv_v8_deterministic_state_count_minimal() {
    let s1 = stats(minimal("sv_v8_dt_min1"));
    let s2 = stats(minimal("sv_v8_dt_min2"));
    let s3 = stats(minimal("sv_v8_dt_min3"));
    assert_eq!(s1.state_count, s2.state_count);
    assert_eq!(s2.state_count, s3.state_count);
}

#[test]
fn sv_v8_deterministic_symbol_count_arith() {
    let s1 = stats(arith_left("sv_v8_dt_ar1"));
    let s2 = stats(arith_left("sv_v8_dt_ar2"));
    let s3 = stats(arith_left("sv_v8_dt_ar3"));
    assert_eq!(s1.symbol_count, s2.symbol_count);
    assert_eq!(s2.symbol_count, s3.symbol_count);
}

#[test]
fn sv_v8_deterministic_conflict_cells_two_op() {
    let s1 = stats(two_op("sv_v8_dt_2o1"));
    let s2 = stats(two_op("sv_v8_dt_2o2"));
    let s3 = stats(two_op("sv_v8_dt_2o3"));
    assert_eq!(s1.conflict_cells, s2.conflict_cells);
    assert_eq!(s2.conflict_cells, s3.conflict_cells);
}

#[test]
fn sv_v8_deterministic_all_fields_chain() {
    let s1 = stats(chain("sv_v8_dt_ch1"));
    let s2 = stats(chain("sv_v8_dt_ch2"));
    assert_eq!(s1.state_count, s2.state_count);
    assert_eq!(s1.symbol_count, s2.symbol_count);
    assert_eq!(s1.conflict_cells, s2.conflict_cells);
}

// =========================================================================
// 8. state_count >= 2 for non-trivial grammar (4 tests)
// =========================================================================

#[test]
fn sv_v8_at_least_two_states_minimal() {
    let s = stats(minimal("sv_v8_a2_min"));
    assert!(
        s.state_count >= 2,
        "need start + accept states, got {}",
        s.state_count
    );
}

#[test]
fn sv_v8_at_least_two_states_arith() {
    let s = stats(arith_left("sv_v8_a2_ar"));
    assert!(s.state_count >= 2);
}

#[test]
fn sv_v8_at_least_two_states_five_rule() {
    let s = stats(five_rule("sv_v8_a2_5r"));
    assert!(s.state_count >= 2);
}

#[test]
fn sv_v8_at_least_two_states_seq() {
    let s = stats(seq("sv_v8_a2_seq", 4));
    assert!(s.state_count >= 2);
}

// =========================================================================
// 9. symbol_count >= num_tokens + num_nonterminals (4 tests)
// =========================================================================

#[test]
fn sv_v8_symbols_cover_tokens_and_nts_minimal() {
    // 1 token + 1 non-terminal (source_file) + EOF = at least 3
    let s = stats(minimal("sv_v8_sctn_min"));
    assert!(
        s.symbol_count >= 3,
        "1 tok + 1 nt + EOF = >=3, got {}",
        s.symbol_count
    );
}

#[test]
fn sv_v8_symbols_cover_tokens_and_nts_two_alt() {
    // 2 tokens + 1 nt + EOF = at least 4
    let s = stats(two_alt("sv_v8_sctn_2a"));
    assert!(
        s.symbol_count >= 4,
        "2 tok + 1 nt + EOF = >=4, got {}",
        s.symbol_count
    );
}

#[test]
fn sv_v8_symbols_cover_tokens_and_nts_chain() {
    // 1 token + 2 nts (atom, source_file) + EOF = at least 4
    let s = stats(chain("sv_v8_sctn_ch"));
    assert!(
        s.symbol_count >= 4,
        "1 tok + 2 nt + EOF = >=4, got {}",
        s.symbol_count
    );
}

#[test]
fn sv_v8_symbols_cover_tokens_and_nts_n_alt() {
    // 10 tokens + 1 nt + EOF = at least 12
    let s = stats(n_alt("sv_v8_sctn_n10", 10));
    assert!(
        s.symbol_count >= 12,
        "10 tok + 1 nt + EOF = >=12, got {}",
        s.symbol_count
    );
}

// =========================================================================
// 10. conflict_cells >= 0 (always; bounded by table size) (4 tests)
// =========================================================================

#[test]
fn sv_v8_conflict_bounded_minimal() {
    let s = stats(minimal("sv_v8_cb_min"));
    assert!(s.conflict_cells <= s.state_count * s.symbol_count);
}

#[test]
fn sv_v8_conflict_bounded_arith() {
    let s = stats(arith_left("sv_v8_cb_ar"));
    assert!(s.conflict_cells <= s.state_count * s.symbol_count);
}

#[test]
fn sv_v8_conflict_bounded_two_op() {
    let s = stats(two_op("sv_v8_cb_2o"));
    assert!(s.conflict_cells <= s.state_count * s.symbol_count);
}

#[test]
fn sv_v8_conflict_bounded_n_alt() {
    let s = stats(n_alt("sv_v8_cb_na", 8));
    assert!(s.conflict_cells <= s.state_count * s.symbol_count);
}

// =========================================================================
// 11. Grammar with precedence → stats (4 tests)
// =========================================================================

#[test]
fn sv_v8_prec_state_count_positive() {
    let s = stats(with_prec_decl("sv_v8_pd_sc"));
    assert!(s.state_count > 0);
}

#[test]
fn sv_v8_prec_symbol_count_positive() {
    let s = stats(with_prec_decl("sv_v8_pd_sym"));
    assert!(s.symbol_count > 0);
}

#[test]
fn sv_v8_prec_at_least_two_states() {
    let s = stats(with_prec_decl("sv_v8_pd_a2"));
    assert!(s.state_count >= 2);
}

#[test]
fn sv_v8_prec_conflict_bounded() {
    let s = stats(with_prec_decl("sv_v8_pd_cb"));
    assert!(s.conflict_cells <= s.state_count * s.symbol_count);
}

// =========================================================================
// 12. Grammar with alternatives → more states (4 tests)
// =========================================================================

#[test]
fn sv_v8_alt_more_states_than_minimal() {
    let s_min = stats(minimal("sv_v8_alt_min"));
    let s_3a = stats(three_alt("sv_v8_alt_3a"));
    assert!(
        s_3a.state_count >= s_min.state_count,
        "3-alt ({}) should have >= minimal ({}) states",
        s_3a.state_count,
        s_min.state_count,
    );
}

#[test]
fn sv_v8_alt_more_symbols_with_more_alternatives() {
    let s_2a = stats(two_alt("sv_v8_alt_2a"));
    let s_3a = stats(three_alt("sv_v8_alt_3a2"));
    assert!(s_3a.symbol_count > s_2a.symbol_count);
}

#[test]
fn sv_v8_alt_n5_vs_n10_more_states() {
    let s5 = stats(n_alt("sv_v8_alt_n5", 5));
    let s10 = stats(n_alt("sv_v8_alt_n10", 10));
    assert!(s10.state_count >= s5.state_count);
}

#[test]
fn sv_v8_alt_no_conflicts_when_distinct_tokens() {
    let s = stats(n_alt("sv_v8_alt_nc", 12));
    assert_eq!(
        s.conflict_cells, 0,
        "distinct token alternatives should be conflict-free"
    );
}

// =========================================================================
// 13. Grammar with chain rules → stats (4 tests)
// =========================================================================

#[test]
fn sv_v8_chain_state_count_positive() {
    let s = stats(chain("sv_v8_ch_sc"));
    assert!(s.state_count > 0);
}

#[test]
fn sv_v8_chain_more_symbols_than_minimal() {
    let s_min = stats(minimal("sv_v8_ch_min"));
    let s_ch = stats(chain("sv_v8_ch_ch"));
    assert!(
        s_ch.symbol_count > s_min.symbol_count,
        "chain adds a nonterminal: {} should be > {}",
        s_ch.symbol_count,
        s_min.symbol_count,
    );
}

#[test]
fn sv_v8_chain_no_conflicts() {
    let s = stats(chain("sv_v8_ch_nc"));
    assert_eq!(s.conflict_cells, 0);
}

#[test]
fn sv_v8_deep_chain_more_symbols_than_chain() {
    let s_ch = stats(chain("sv_v8_ch_dch"));
    let s_dch = stats(deep_chain("sv_v8_ch_dch2"));
    assert!(
        s_dch.symbol_count > s_ch.symbol_count,
        "deep chain adds another nonterminal: {} should be > {}",
        s_dch.symbol_count,
        s_ch.symbol_count,
    );
}

// =========================================================================
// 14. Grammar with recursion → stats (4 tests)
// =========================================================================

#[test]
fn sv_v8_recursive_state_count_positive() {
    let s = stats(left_recursive("sv_v8_lr_sc"));
    assert!(s.state_count > 0);
}

#[test]
fn sv_v8_recursive_at_least_three_states() {
    let s = stats(left_recursive("sv_v8_lr_a3"));
    assert!(
        s.state_count >= 3,
        "recursive grammar needs >=3 states, got {}",
        s.state_count
    );
}

#[test]
fn sv_v8_recursive_symbol_count_positive() {
    let s = stats(left_recursive("sv_v8_lr_sym"));
    assert!(s.symbol_count > 0);
}

#[test]
fn sv_v8_recursive_conflict_bounded() {
    let s = stats(left_recursive("sv_v8_lr_cb"));
    assert!(s.conflict_cells <= s.state_count * s.symbol_count);
}

// =========================================================================
// 15. Stats Clone equals original (4 tests)
// =========================================================================

#[test]
fn sv_v8_clone_preserves_state_count() {
    let s = stats(minimal("sv_v8_cl_sc"));
    let c = s.clone();
    assert_eq!(s.state_count, c.state_count);
}

#[test]
fn sv_v8_clone_preserves_symbol_count() {
    let s = stats(arith_left("sv_v8_cl_sym"));
    let c = s.clone();
    assert_eq!(s.symbol_count, c.symbol_count);
}

#[test]
fn sv_v8_clone_preserves_conflict_cells() {
    let s = stats(two_op("sv_v8_cl_cc"));
    let c = s.clone();
    assert_eq!(s.conflict_cells, c.conflict_cells);
}

#[test]
fn sv_v8_clone_all_fields_equal() {
    let s = stats(three_op("sv_v8_cl_all"));
    let c = s.clone();
    assert_eq!(s.state_count, c.state_count);
    assert_eq!(s.symbol_count, c.symbol_count);
    assert_eq!(s.conflict_cells, c.conflict_cells);
}

// =========================================================================
// 16. Stats Debug is non-empty (4 tests)
// =========================================================================

#[test]
fn sv_v8_debug_not_empty() {
    let s = stats(minimal("sv_v8_dbg_ne"));
    let dbg = format!("{s:?}");
    assert!(!dbg.is_empty());
}

#[test]
fn sv_v8_debug_contains_state_count() {
    let s = stats(minimal("sv_v8_dbg_sc"));
    let dbg = format!("{s:?}");
    assert!(
        dbg.contains("state_count"),
        "Debug should contain 'state_count': {dbg}"
    );
}

#[test]
fn sv_v8_debug_contains_symbol_count() {
    let s = stats(arith_left("sv_v8_dbg_sym"));
    let dbg = format!("{s:?}");
    assert!(
        dbg.contains("symbol_count"),
        "Debug should contain 'symbol_count': {dbg}"
    );
}

#[test]
fn sv_v8_debug_contains_conflict_cells() {
    let s = stats(two_op("sv_v8_dbg_cc"));
    let dbg = format!("{s:?}");
    assert!(
        dbg.contains("conflict_cells"),
        "Debug should contain 'conflict_cells': {dbg}"
    );
}

// =========================================================================
// 17. Comparing stats across grammar sizes (4 tests)
// =========================================================================

#[test]
fn sv_v8_size_n2_vs_n8_symbols() {
    let s2 = stats(n_alt("sv_v8_sz_n2", 2));
    let s8 = stats(n_alt("sv_v8_sz_n8", 8));
    assert!(s8.symbol_count > s2.symbol_count);
}

#[test]
fn sv_v8_size_n4_vs_n12_states() {
    let s4 = stats(n_alt("sv_v8_sz_n4", 4));
    let s12 = stats(n_alt("sv_v8_sz_n12", 12));
    assert!(s12.state_count >= s4.state_count);
}

#[test]
fn sv_v8_size_seq3_vs_seq8_states() {
    let s3 = stats(seq("sv_v8_sz_sq3", 3));
    let s8 = stats(seq("sv_v8_sz_sq8", 8));
    assert!(s8.state_count >= s3.state_count);
}

#[test]
fn sv_v8_size_chain_vs_deep_chain_states() {
    let s_ch = stats(chain("sv_v8_sz_ch"));
    let s_dch = stats(deep_chain("sv_v8_sz_dch"));
    assert!(s_dch.state_count >= s_ch.state_count);
}

// =========================================================================
// 18. 1-rule grammar stats (4 tests)
// =========================================================================

#[test]
fn sv_v8_one_rule_state_count() {
    let s = stats(minimal("sv_v8_1r_sc"));
    assert!(s.state_count >= 2);
}

#[test]
fn sv_v8_one_rule_symbol_count() {
    let s = stats(minimal("sv_v8_1r_sym"));
    // NUMBER + source_file + EOF = at least 3
    assert!(s.symbol_count >= 3);
}

#[test]
fn sv_v8_one_rule_no_conflicts() {
    let s = stats(minimal("sv_v8_1r_nc"));
    assert_eq!(s.conflict_cells, 0);
}

#[test]
fn sv_v8_one_rule_bounded() {
    let s = stats(minimal("sv_v8_1r_bd"));
    assert!(s.conflict_cells <= s.state_count * s.symbol_count);
}

// =========================================================================
// 19. 5-rule grammar stats (4 tests)
// =========================================================================

#[test]
fn sv_v8_five_rule_state_count_positive() {
    let s = stats(five_rule("sv_v8_5r_sc"));
    assert!(s.state_count > 0);
}

#[test]
fn sv_v8_five_rule_symbol_count_ge_tokens_plus_nts() {
    let s = stats(five_rule("sv_v8_5r_sym"));
    // 5 tokens + 6 nts (assign, call, ret, break_s, literal, source_file) + EOF = at least 12
    assert!(
        s.symbol_count >= 12,
        "5-rule grammar should have >= 12 symbols, got {}",
        s.symbol_count
    );
}

#[test]
fn sv_v8_five_rule_preserves_number_reduce_reduce_conflict() {
    let s = stats(five_rule("sv_v8_5r_nc"));
    assert_eq!(
        s.conflict_cells, 1,
        "ret -> NUMBER and literal -> NUMBER should preserve one reduce/reduce conflict"
    );
}

#[test]
fn sv_v8_five_rule_more_states_than_minimal() {
    let s_min = stats(minimal("sv_v8_5r_min"));
    let s_5r = stats(five_rule("sv_v8_5r_5r"));
    assert!(
        s_5r.state_count >= s_min.state_count,
        "5-rule ({}) should have >= minimal ({}) states",
        s_5r.state_count,
        s_min.state_count,
    );
}

// =========================================================================
// 20. 10-rule grammar stats (4 tests)
// =========================================================================

#[test]
fn sv_v8_ten_rule_state_count_positive() {
    let s = stats(ten_rule("sv_v8_10r_sc"));
    assert!(s.state_count > 0);
}

#[test]
fn sv_v8_ten_rule_symbol_count_ge_tokens_plus_nts() {
    let s = stats(ten_rule("sv_v8_10r_sym"));
    // 10 tokens + 11 nts + EOF = at least 22
    assert!(
        s.symbol_count >= 22,
        "10-rule grammar should have >= 22 symbols, got {}",
        s.symbol_count
    );
}

#[test]
fn sv_v8_ten_rule_more_symbols_than_five_rule() {
    let s_5 = stats(five_rule("sv_v8_10r_5r"));
    let s_10 = stats(ten_rule("sv_v8_10r_10r"));
    assert!(
        s_10.symbol_count > s_5.symbol_count,
        "10-rule ({}) should have > 5-rule ({}) symbols",
        s_10.symbol_count,
        s_5.symbol_count,
    );
}

#[test]
fn sv_v8_ten_rule_more_states_than_five_rule() {
    let s_5 = stats(five_rule("sv_v8_10r_5rs"));
    let s_10 = stats(ten_rule("sv_v8_10r_10rs"));
    assert!(
        s_10.state_count >= s_5.state_count,
        "10-rule ({}) should have >= 5-rule ({}) states",
        s_10.state_count,
        s_5.state_count,
    );
}

// =========================================================================
// 21. Cross-field consistency invariants (4 tests)
// =========================================================================

#[test]
fn sv_v8_consistency_left_vs_right_assoc_same_symbols() {
    let s_l = stats(arith_left("sv_v8_con_la"));
    let s_r = stats(arith_right("sv_v8_con_ra"));
    // Both have 2 tokens + 1 nt + EOF → same symbol_count
    assert_eq!(s_l.symbol_count, s_r.symbol_count);
}

#[test]
fn sv_v8_consistency_prec_decl_same_symbols_as_inline_prec() {
    let s_decl = stats(with_prec_decl("sv_v8_con_decl"));
    let s_inline = stats(two_op("sv_v8_con_inl"));
    // Both: NUMBER, PLUS, STAR, source_file → same symbol_count
    assert_eq!(s_decl.symbol_count, s_inline.symbol_count);
}

#[test]
fn sv_v8_consistency_conflict_le_total_cells() {
    let s = stats(three_op("sv_v8_con_cle"));
    assert!(
        s.conflict_cells <= s.state_count * s.symbol_count,
        "conflicts ({}) must be <= total cells ({})",
        s.conflict_cells,
        s.state_count * s.symbol_count,
    );
}

#[test]
fn sv_v8_consistency_all_fields_nonzero_for_nontrivial() {
    let s = stats(ten_rule("sv_v8_con_nz"));
    assert!(s.state_count > 0);
    assert!(s.symbol_count > 0);
    // conflict_cells can be 0 for unambiguous grammars — that's fine
}

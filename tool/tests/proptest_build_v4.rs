//! Property-based tests for the adze-tool build pipeline (v4).
//!
//! 48 proptest property tests across 8 categories (6 each):
//!   1. prop_build_deterministic_*  — deterministic output
//!   2. prop_build_stats_*          — statistics correctness
//!   3. prop_build_valid_*          — build produces valid output
//!   4. prop_build_json_*           — JSON roundtrip properties
//!   5. prop_build_code_*           — generated code properties
//!   6. prop_build_error_*          — error handling properties
//!   7. prop_build_complex_*        — complex grammar properties
//!   8. prop_build_options_*        — build options properties

use adze_ir::builder::GrammarBuilder;
use adze_tool::pure_rust_builder::{
    BuildOptions, BuildResult, build_parser, build_parser_from_json,
};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn test_opts() -> BuildOptions {
    BuildOptions {
        out_dir: "/tmp/proptest_build_v4".to_string(),
        emit_artifacts: false,
        compress_tables: true,
    }
}

/// Build via the IR path and assert success.
fn build_ir(name: &str, tokens: &[(&str, &str)], rules: &[(&str, Vec<&str>)]) -> BuildResult {
    let mut b = GrammarBuilder::new(name);
    for &(tname, tpat) in tokens {
        b = b.token(tname, tpat);
    }
    for (lhs, rhs) in rules {
        b = b.rule(lhs, rhs.clone());
    }
    if let Some((lhs, _)) = rules.first() {
        b = b.start(lhs);
    }
    build_parser(b.build(), test_opts()).expect("build_ir failed")
}

/// Build via the JSON path and assert success.
fn build_json(name: &str) -> BuildResult {
    let json = serde_json::json!({
        "name": name,
        "word": null,
        "rules": {
            "source_file": {
                "type": "SYMBOL",
                "name": "item"
            },
            "item": {
                "type": "PATTERN",
                "value": "\\w+"
            }
        },
        "extras": [
            { "type": "PATTERN", "value": "\\s" }
        ],
        "conflicts": [],
        "precedences": [],
        "externals": [],
        "inline": [],
        "supertypes": []
    })
    .to_string();
    build_parser_from_json(json, test_opts()).expect("build_json failed")
}

/// Build n-alternative grammar: s -> tok0 | tok1 | … | tok(n-1).
fn build_n_alts(name: &str, n: usize) -> BuildResult {
    let tok_names: Vec<String> = (0..n).map(|i| format!("tok{i}")).collect();
    let tok_pats: Vec<String> = (0..n).map(|i| format!("p{i}")).collect();
    let pairs: Vec<(&str, &str)> = tok_names
        .iter()
        .zip(tok_pats.iter())
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let rules: Vec<(&str, Vec<&str>)> = tok_names.iter().map(|t| ("s", vec![t.as_str()])).collect();
    let mut b = GrammarBuilder::new(name);
    for &(tname, tpat) in &pairs {
        b = b.token(tname, tpat);
    }
    for (lhs, rhs) in &rules {
        b = b.rule(lhs, rhs.clone());
    }
    b = b.start("s");
    build_parser(b.build(), test_opts()).expect("build_n_alts failed")
}

/// Build a JSON grammar with `n` rules branching from source_file.
fn build_json_n_rules(name: &str, n: usize) -> BuildResult {
    let mut rules = serde_json::Map::new();
    let alt_members: Vec<serde_json::Value> = (0..n)
        .map(|i| {
            let rule_name = format!("item_{i}");
            rules.insert(
                rule_name.clone(),
                serde_json::json!({ "type": "PATTERN", "value": format!("t{i}") }),
            );
            serde_json::json!({ "type": "SYMBOL", "name": rule_name })
        })
        .collect();
    let top = if alt_members.len() == 1 {
        alt_members.into_iter().next().unwrap()
    } else {
        serde_json::json!({ "type": "CHOICE", "members": alt_members })
    };
    rules.insert("source_file".to_string(), top);
    let grammar = serde_json::json!({
        "name": name,
        "word": null,
        "rules": rules,
        "extras": [{ "type": "PATTERN", "value": "\\s" }],
        "conflicts": [],
        "precedences": [],
        "externals": [],
        "inline": [],
        "supertypes": []
    });
    build_parser_from_json(grammar.to_string(), test_opts()).expect("build_json_n_rules failed")
}

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

/// Alphabetic grammar names that avoid Rust 2024 reserved keywords.
fn grammar_name_strategy() -> impl Strategy<Value = String> {
    "[a-z]{3,8}".prop_filter("must not be a Rust keyword", |s| {
        !matches!(
            s.as_str(),
            "gen"
                | "do"
                | "abstract"
                | "become"
                | "final"
                | "override"
                | "priv"
                | "typeof"
                | "unsized"
                | "virtual"
                | "box"
                | "macro"
                | "try"
                | "yield"
                | "fn"
                | "let"
                | "mut"
                | "ref"
                | "pub"
                | "mod"
                | "use"
                | "for"
                | "if"
                | "else"
                | "loop"
                | "while"
                | "match"
                | "impl"
                | "enum"
                | "struct"
                | "trait"
                | "type"
                | "where"
                | "async"
                | "await"
                | "dyn"
                | "move"
                | "return"
                | "break"
                | "continue"
                | "const"
                | "static"
                | "extern"
                | "crate"
                | "self"
                | "super"
                | "as"
                | "in"
        )
    })
}

/// Strategy for the number of alternative tokens (1..=6).
fn alt_count_strategy() -> impl Strategy<Value = usize> {
    1..=6usize
}

// ===========================================================================
// 1. prop_build_deterministic_* — deterministic output (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_build_deterministic_parser_code(name in grammar_name_strategy()) {
        let r1 = build_ir(&name, &[("a", "a")], &[("s", vec!["a"])]);
        let r2 = build_ir(&name, &[("a", "a")], &[("s", vec!["a"])]);
        prop_assert_eq!(&r1.parser_code, &r2.parser_code);
    }

    #[test]
    fn prop_build_deterministic_node_types(name in grammar_name_strategy()) {
        let r1 = build_ir(&name, &[("a", "a")], &[("s", vec!["a"])]);
        let r2 = build_ir(&name, &[("a", "a")], &[("s", vec!["a"])]);
        prop_assert_eq!(&r1.node_types_json, &r2.node_types_json);
    }

    #[test]
    fn prop_build_deterministic_state_count(name in grammar_name_strategy()) {
        let r1 = build_ir(&name, &[("a", "a")], &[("s", vec!["a"])]);
        let r2 = build_ir(&name, &[("a", "a")], &[("s", vec!["a"])]);
        prop_assert_eq!(r1.build_stats.state_count, r2.build_stats.state_count);
    }

    #[test]
    fn prop_build_deterministic_symbol_count(name in grammar_name_strategy()) {
        let r1 = build_ir(&name, &[("a", "a")], &[("s", vec!["a"])]);
        let r2 = build_ir(&name, &[("a", "a")], &[("s", vec!["a"])]);
        prop_assert_eq!(r1.build_stats.symbol_count, r2.build_stats.symbol_count);
    }

    #[test]
    fn prop_build_deterministic_conflict_cells(name in grammar_name_strategy()) {
        let r1 = build_ir(&name, &[("a", "a")], &[("s", vec!["a"])]);
        let r2 = build_ir(&name, &[("a", "a")], &[("s", vec!["a"])]);
        prop_assert_eq!(r1.build_stats.conflict_cells, r2.build_stats.conflict_cells);
    }

    #[test]
    fn prop_build_deterministic_two_token_grammar(name in grammar_name_strategy()) {
        let toks = [("a", "a"), ("b", "b")];
        let rules = [("s", vec!["a"]), ("s", vec!["b"])];
        let r1 = build_ir(&name, &toks, &rules);
        let r2 = build_ir(&name, &toks, &rules);
        prop_assert_eq!(&r1.parser_code, &r2.parser_code);
        prop_assert_eq!(&r1.node_types_json, &r2.node_types_json);
    }
}

// ===========================================================================
// 2. prop_build_stats_* — statistics correctness (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_build_stats_state_count_positive(name in grammar_name_strategy()) {
        let r = build_ir(&name, &[("a", "a")], &[("s", vec!["a"])]);
        prop_assert!(r.build_stats.state_count > 0);
    }

    #[test]
    fn prop_build_stats_symbol_count_positive(name in grammar_name_strategy()) {
        let r = build_ir(&name, &[("a", "a")], &[("s", vec!["a"])]);
        prop_assert!(r.build_stats.symbol_count > 0);
    }

    #[test]
    fn prop_build_stats_two_tokens_ge_three_symbols(name in grammar_name_strategy()) {
        let r = build_ir(
            &name,
            &[("a", "a"), ("b", "b")],
            &[("s", vec!["a"]), ("s", vec!["b"])],
        );
        prop_assert!(
            r.build_stats.symbol_count >= 3,
            "expected >= 3 symbols, got {}",
            r.build_stats.symbol_count
        );
    }

    #[test]
    fn prop_build_stats_n_alts_symbol_count_ge_n(n in 1..=5usize) {
        let r = build_n_alts("statsn", n);
        prop_assert!(
            r.build_stats.symbol_count >= n,
            "expected >= {n} symbols, got {}",
            r.build_stats.symbol_count
        );
    }

    #[test]
    fn prop_build_stats_conflict_cells_bounded(name in grammar_name_strategy()) {
        let r = build_ir(&name, &[("a", "a")], &[("s", vec!["a"])]);
        let upper = r.build_stats.state_count * r.build_stats.symbol_count;
        prop_assert!(
            r.build_stats.conflict_cells <= upper,
            "conflicts {} > upper bound {}",
            r.build_stats.conflict_cells,
            upper,
        );
    }

    #[test]
    fn prop_build_stats_debug_format_complete(name in grammar_name_strategy()) {
        let r = build_ir(&name, &[("a", "a")], &[("s", vec!["a"])]);
        let dbg = format!("{:?}", r.build_stats);
        prop_assert!(dbg.contains("state_count"));
        prop_assert!(dbg.contains("symbol_count"));
        prop_assert!(dbg.contains("conflict_cells"));
    }
}

// ===========================================================================
// 3. prop_build_valid_* — build produces valid output (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_build_valid_single_token(name in grammar_name_strategy()) {
        let r = build_ir(&name, &[("a", "a")], &[("s", vec!["a"])]);
        prop_assert!(!r.parser_code.is_empty());
        prop_assert!(!r.node_types_json.is_empty());
    }

    #[test]
    fn prop_build_valid_two_token_alt(name in grammar_name_strategy()) {
        let r = build_ir(
            &name,
            &[("a", "a"), ("b", "b")],
            &[("s", vec!["a"]), ("s", vec!["b"])],
        );
        prop_assert!(!r.parser_code.is_empty());
        prop_assert!(r.build_stats.state_count > 0);
    }

    #[test]
    fn prop_build_valid_concat(name in grammar_name_strategy()) {
        let r = build_ir(
            &name,
            &[("a", "a"), ("b", "b")],
            &[("s", vec!["a", "b"])],
        );
        prop_assert!(r.build_stats.state_count > 0);
        prop_assert!(!r.parser_code.is_empty());
    }

    #[test]
    fn prop_build_valid_chain_rule(name in grammar_name_strategy()) {
        let g = GrammarBuilder::new(&name)
            .token("x", "x")
            .rule("inner", vec!["x"])
            .rule("s", vec!["inner"])
            .start("s")
            .build();
        let r = build_parser(g, test_opts()).unwrap();
        prop_assert!(r.build_stats.state_count > 0);
        prop_assert!(!r.parser_code.is_empty());
    }

    #[test]
    fn prop_build_valid_n_alts(n in alt_count_strategy()) {
        let r = build_n_alts("valid", n);
        prop_assert!(r.build_stats.state_count > 0);
        prop_assert!(!r.parser_code.is_empty());
    }

    #[test]
    fn prop_build_valid_with_extras(name in grammar_name_strategy()) {
        let g = GrammarBuilder::new(&name)
            .token("a", "a")
            .token("ws", "ws")
            .rule("s", vec!["a"])
            .extra("ws")
            .start("s")
            .build();
        let r = build_parser(g, test_opts()).unwrap();
        prop_assert!(r.build_stats.symbol_count > 0);
        prop_assert!(!r.parser_code.is_empty());
    }
}

// ===========================================================================
// 4. prop_build_json_* — JSON roundtrip properties (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_build_json_produces_nonempty_code(name in grammar_name_strategy()) {
        let r = build_json(&name);
        prop_assert!(!r.parser_code.is_empty());
    }

    #[test]
    fn prop_build_json_grammar_name_preserved(name in grammar_name_strategy()) {
        let r = build_json(&name);
        prop_assert_eq!(&r.grammar_name, &name);
    }

    #[test]
    fn prop_build_json_state_count_positive(name in grammar_name_strategy()) {
        let r = build_json(&name);
        prop_assert!(r.build_stats.state_count > 0);
    }

    #[test]
    fn prop_build_json_node_types_valid(name in grammar_name_strategy()) {
        let r = build_json(&name);
        let parsed: std::result::Result<serde_json::Value, _> =
            serde_json::from_str(&r.node_types_json);
        prop_assert!(parsed.is_ok(), "node_types_json must be valid JSON");
    }

    #[test]
    fn prop_build_json_deterministic(name in grammar_name_strategy()) {
        let r1 = build_json(&name);
        let r2 = build_json(&name);
        prop_assert_eq!(&r1.parser_code, &r2.parser_code);
        prop_assert_eq!(r1.build_stats.state_count, r2.build_stats.state_count);
    }

    #[test]
    fn prop_build_json_n_rules_scales(n in 1..=4usize) {
        let small = build_json_n_rules("jrs", n);
        let large = build_json_n_rules("jrl", n + 1);
        prop_assert!(
            large.build_stats.symbol_count >= small.build_stats.symbol_count,
            "symbol_count {} (n={}) vs {} (n={})",
            small.build_stats.symbol_count, n,
            large.build_stats.symbol_count, n + 1,
        );
    }
}

// ===========================================================================
// 5. prop_build_code_* — generated code properties (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_build_code_nonempty_single(name in grammar_name_strategy()) {
        let r = build_ir(&name, &[("a", "a")], &[("s", vec!["a"])]);
        prop_assert!(!r.parser_code.is_empty());
    }

    #[test]
    fn prop_build_code_nonempty_multi_alt(n in 1..=5usize) {
        let r = build_n_alts("coden", n);
        prop_assert!(!r.parser_code.is_empty());
    }

    #[test]
    fn prop_build_code_node_types_is_json_array(name in grammar_name_strategy()) {
        let r = build_ir(&name, &[("a", "a")], &[("s", vec!["a"])]);
        let val: serde_json::Value = serde_json::from_str(&r.node_types_json).unwrap();
        prop_assert!(val.is_array(), "NODE_TYPES.json should be an array");
    }

    #[test]
    fn prop_build_code_contains_grammar_name(name in grammar_name_strategy()) {
        let r = build_ir(&name, &[("a", "a")], &[("s", vec!["a"])]);
        prop_assert!(
            r.parser_code.contains(&name),
            "generated code should reference grammar name '{name}'",
        );
    }

    #[test]
    fn prop_build_code_parser_path_contains_name(name in grammar_name_strategy()) {
        let r = build_ir(&name, &[("a", "a")], &[("s", vec!["a"])]);
        prop_assert!(
            r.parser_path.contains(&name),
            "parser_path '{}' should contain '{name}'",
            r.parser_path,
        );
    }

    #[test]
    fn prop_build_code_length_grows_with_alts(n in 1..=4usize) {
        let small = build_n_alts("cls", n);
        let large = build_n_alts("cll", n + 1);
        prop_assert!(
            large.parser_code.len() >= small.parser_code.len(),
            "code len: {} (n={n}) vs {} (n={})",
            small.parser_code.len(),
            large.parser_code.len(), n + 1,
        );
    }
}

// ===========================================================================
// 6. prop_build_error_* — error handling properties (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_build_error_empty_grammar_fails(name in grammar_name_strategy()) {
        let g = GrammarBuilder::new(&name).build();
        let result = build_parser(g, test_opts());
        prop_assert!(result.is_err(), "empty grammar should fail");
    }

    #[test]
    fn prop_build_error_message_nonempty(name in grammar_name_strategy()) {
        let g = GrammarBuilder::new(&name).build();
        if let Err(e) = build_parser(g, test_opts()) {
            let msg = format!("{e}");
            prop_assert!(!msg.is_empty());
        }
    }

    #[test]
    fn prop_build_error_invalid_json_literal(_dummy in 0..1u8) {
        let result = build_parser_from_json("not json".to_string(), test_opts());
        prop_assert!(result.is_err());
    }

    #[test]
    fn prop_build_error_empty_json_string(_dummy in 0..1u8) {
        let result = build_parser_from_json(String::new(), test_opts());
        prop_assert!(result.is_err());
    }

    #[test]
    fn prop_build_error_json_missing_rules(name in grammar_name_strategy()) {
        let json = serde_json::json!({
            "name": name,
            "word": null,
            "rules": {},
            "extras": [],
            "conflicts": [],
            "precedences": [],
            "externals": [],
            "inline": [],
            "supertypes": []
        })
        .to_string();
        let result = build_parser_from_json(json, test_opts());
        prop_assert!(result.is_err(), "grammar with no rules should fail");
    }

    #[test]
    fn prop_build_error_no_start_symbol(name in grammar_name_strategy()) {
        // Grammar with a token but no rule and no start
        let g = GrammarBuilder::new(&name)
            .token("a", "a")
            .build();
        let result = build_parser(g, test_opts());
        prop_assert!(result.is_err(), "grammar with no start should fail");
    }
}

// ===========================================================================
// 7. prop_build_complex_* — complex grammar properties (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_build_complex_chain_two_levels(name in grammar_name_strategy()) {
        let g = GrammarBuilder::new(&name)
            .token("x", "x")
            .rule("leaf", vec!["x"])
            .rule("mid", vec!["leaf"])
            .rule("top", vec!["mid"])
            .start("top")
            .build();
        let r = build_parser(g, test_opts()).unwrap();
        prop_assert!(r.build_stats.state_count > 0);
    }

    #[test]
    fn prop_build_complex_concat_three_tokens(name in grammar_name_strategy()) {
        let g = GrammarBuilder::new(&name)
            .token("a", "a")
            .token("b", "b")
            .token("c", "c")
            .rule("s", vec!["a", "b", "c"])
            .start("s")
            .build();
        let r = build_parser(g, test_opts()).unwrap();
        prop_assert!(r.build_stats.state_count > 0);
        prop_assert!(!r.parser_code.is_empty());
    }

    #[test]
    fn prop_build_complex_multiple_nonterminals(name in grammar_name_strategy()) {
        let g = GrammarBuilder::new(&name)
            .token("a", "a")
            .token("b", "b")
            .rule("left", vec!["a"])
            .rule("right", vec!["b"])
            .rule("s", vec!["left"])
            .rule("s", vec!["right"])
            .start("s")
            .build();
        let r = build_parser(g, test_opts()).unwrap();
        prop_assert!(r.build_stats.symbol_count >= 4);
    }

    #[test]
    fn prop_build_complex_state_monotonic(n in 1..=4usize) {
        let small = build_n_alts("cms", n);
        let large = build_n_alts("cml", n + 1);
        prop_assert!(
            large.build_stats.state_count >= small.build_stats.state_count,
            "state_count: {} (n={n}) vs {} (n={})",
            small.build_stats.state_count,
            large.build_stats.state_count, n + 1,
        );
    }

    #[test]
    fn prop_build_complex_node_types_grows(n in 1..=4usize) {
        let small = build_n_alts("nts", n);
        let large = build_n_alts("ntl", n + 1);
        prop_assert!(
            large.node_types_json.len() >= small.node_types_json.len(),
            "node_types len: {} (n={n}) vs {} (n={})",
            small.node_types_json.len(),
            large.node_types_json.len(), n + 1,
        );
    }

    #[test]
    fn prop_build_complex_single_vs_multi_symbols(n in 2..=5usize) {
        let single = build_n_alts("one", 1);
        let multi = build_n_alts("many", n);
        prop_assert!(
            multi.build_stats.symbol_count >= single.build_stats.symbol_count,
            "multi-alt should have >= symbols than single-alt",
        );
    }
}

// ===========================================================================
// 8. prop_build_options_* — build options properties (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_build_options_compressed_succeeds(name in grammar_name_strategy()) {
        let g = GrammarBuilder::new(&name)
            .token("a", "a")
            .rule("s", vec!["a"])
            .start("s")
            .build();
        let r = build_parser(g, BuildOptions { compress_tables: true, ..test_opts() });
        prop_assert!(r.is_ok());
    }

    #[test]
    fn prop_build_options_uncompressed_succeeds(name in grammar_name_strategy()) {
        let g = GrammarBuilder::new(&name)
            .token("a", "a")
            .rule("s", vec!["a"])
            .start("s")
            .build();
        let r = build_parser(g, BuildOptions { compress_tables: false, ..test_opts() });
        prop_assert!(r.is_ok());
    }

    #[test]
    fn prop_build_options_state_count_same_either_mode(name in grammar_name_strategy()) {
        let g1 = GrammarBuilder::new(&name)
            .token("a", "a")
            .rule("s", vec!["a"])
            .start("s")
            .build();
        let g2 = GrammarBuilder::new(&name)
            .token("a", "a")
            .rule("s", vec!["a"])
            .start("s")
            .build();
        let r1 = build_parser(g1, BuildOptions { compress_tables: true, ..test_opts() }).unwrap();
        let r2 = build_parser(g2, BuildOptions { compress_tables: false, ..test_opts() }).unwrap();
        prop_assert_eq!(r1.build_stats.state_count, r2.build_stats.state_count);
    }

    #[test]
    fn prop_build_options_symbol_count_same_either_mode(name in grammar_name_strategy()) {
        let g1 = GrammarBuilder::new(&name)
            .token("a", "a")
            .rule("s", vec!["a"])
            .start("s")
            .build();
        let g2 = GrammarBuilder::new(&name)
            .token("a", "a")
            .rule("s", vec!["a"])
            .start("s")
            .build();
        let r1 = build_parser(g1, BuildOptions { compress_tables: true, ..test_opts() }).unwrap();
        let r2 = build_parser(g2, BuildOptions { compress_tables: false, ..test_opts() }).unwrap();
        prop_assert_eq!(r1.build_stats.symbol_count, r2.build_stats.symbol_count);
    }

    #[test]
    fn prop_build_options_emit_artifacts_false(name in grammar_name_strategy()) {
        let g = GrammarBuilder::new(&name)
            .token("a", "a")
            .rule("s", vec!["a"])
            .start("s")
            .build();
        let opts = BuildOptions { emit_artifacts: false, ..test_opts() };
        let r = build_parser(g, opts);
        prop_assert!(r.is_ok());
    }

    #[test]
    fn prop_build_options_custom_out_dir(name in grammar_name_strategy()) {
        let g = GrammarBuilder::new(&name)
            .token("a", "a")
            .rule("s", vec!["a"])
            .start("s")
            .build();
        let opts = BuildOptions {
            out_dir: "/tmp/proptest_custom_out".to_string(),
            emit_artifacts: false,
            compress_tables: true,
        };
        let r = build_parser(g, opts).unwrap();
        prop_assert!(!r.parser_code.is_empty());
    }
}

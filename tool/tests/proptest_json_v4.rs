//! Property-based tests for adze-tool JSON grammar conversion properties.
//!
//! 48 proptest property tests across 8 categories (6 each):
//! 1. prop_json_roundtrip_*       — JSON roundtrip properties
//! 2. prop_json_format_*          — format properties
//! 3. prop_json_tokens_*          — token representation
//! 4. prop_json_rules_*           — rule representation
//! 5. prop_json_error_*           — error handling
//! 6. prop_json_complex_*         — complex grammars
//! 7. prop_json_deterministic_*   — deterministic output
//! 8. prop_json_parse_*           — JSON parsing properties

use adze_ir::builder::GrammarBuilder;
use adze_tool::grammar_js::{GrammarJsConverter, from_json};
use adze_tool::pure_rust_builder::{
    BuildOptions, BuildResult, build_parser, build_parser_from_json,
};
use proptest::prelude::*;
use serde_json::{Value, json};

// ===========================================================================
// Helpers
// ===========================================================================

fn test_opts() -> BuildOptions {
    BuildOptions {
        out_dir: "/tmp/proptest_json_v4".to_string(),
        emit_artifacts: false,
        compress_tables: true,
    }
}

fn uncompressed_opts() -> BuildOptions {
    BuildOptions {
        compress_tables: false,
        ..test_opts()
    }
}

fn build_json_ok(v: &Value) -> BuildResult {
    build_parser_from_json(v.to_string(), test_opts()).expect("build_json_ok failed")
}

fn build_ir_ok(
    name: &str,
    tokens: &[(&str, &str)],
    rules: &[(&str, Vec<&str>)],
    start: &str,
) -> BuildResult {
    let mut b = GrammarBuilder::new(name);
    for &(tname, tpat) in tokens {
        b = b.token(tname, tpat);
    }
    for (lhs, rhs) in rules {
        b = b.rule(lhs, rhs.clone());
    }
    b = b.start(start);
    build_parser(b.build(), test_opts()).expect("build_ir_ok failed")
}

/// Minimal JSON grammar with one STRING rule.
fn minimal_json(name: &str) -> Value {
    json!({
        "name": name,
        "rules": {
            "source_file": { "type": "STRING", "value": "hello" }
        }
    })
}

/// JSON grammar with CHOICE alternatives.
fn choice_json(name: &str, alts: &[&str]) -> Value {
    let members: Vec<Value> = alts
        .iter()
        .map(|s| json!({ "type": "STRING", "value": s }))
        .collect();
    json!({
        "name": name,
        "rules": {
            "source_file": { "type": "CHOICE", "members": members }
        }
    })
}

/// JSON grammar with SEQ members.
fn seq_json(name: &str, tokens: &[&str]) -> Value {
    let members: Vec<Value> = tokens
        .iter()
        .map(|s| json!({ "type": "STRING", "value": s }))
        .collect();
    json!({
        "name": name,
        "rules": {
            "source_file": { "type": "SEQ", "members": members }
        }
    })
}

/// JSON grammar with PATTERN rule.
fn pattern_json(name: &str, pat: &str) -> Value {
    json!({
        "name": name,
        "rules": {
            "source_file": { "type": "PATTERN", "value": pat }
        }
    })
}

/// JSON grammar with REPEAT.
fn repeat_json(name: &str, val: &str) -> Value {
    json!({
        "name": name,
        "rules": {
            "source_file": {
                "type": "REPEAT",
                "content": { "type": "STRING", "value": val }
            }
        }
    })
}

/// JSON grammar with REPEAT1.
fn repeat1_json(name: &str, val: &str) -> Value {
    json!({
        "name": name,
        "rules": {
            "source_file": {
                "type": "REPEAT1",
                "content": { "type": "STRING", "value": val }
            }
        }
    })
}

/// JSON grammar with a named sub-rule via SYMBOL reference.
fn symbol_ref_json(name: &str, token_val: &str) -> Value {
    json!({
        "name": name,
        "rules": {
            "source_file": { "type": "SYMBOL", "name": "item" },
            "item": { "type": "STRING", "value": token_val }
        }
    })
}

/// JSON grammar with multiple named sub-rules.
fn multi_rule_json(name: &str, rule_tokens: &[(&str, &str)]) -> Value {
    let mut rules = serde_json::Map::new();
    let members: Vec<Value> = rule_tokens
        .iter()
        .map(|(rname, _)| json!({ "type": "SYMBOL", "name": rname }))
        .collect();
    rules.insert(
        "source_file".to_string(),
        json!({ "type": "CHOICE", "members": members }),
    );
    for &(rname, rval) in rule_tokens {
        rules.insert(
            rname.to_string(),
            json!({ "type": "STRING", "value": rval }),
        );
    }
    json!({ "name": name, "rules": rules })
}

/// JSON grammar with TOKEN wrapper.
fn token_wrapper_json(name: &str, inner_val: &str) -> Value {
    json!({
        "name": name,
        "rules": {
            "source_file": {
                "type": "TOKEN",
                "content": { "type": "STRING", "value": inner_val }
            }
        }
    })
}

/// JSON grammar with PREC_LEFT.
fn prec_left_json(name: &str, val: &str, precedence: i32) -> Value {
    json!({
        "name": name,
        "rules": {
            "source_file": {
                "type": "PREC_LEFT",
                "value": precedence,
                "content": { "type": "STRING", "value": val }
            }
        }
    })
}

/// JSON grammar with OPTIONAL.
fn optional_json(name: &str, val: &str) -> Value {
    json!({
        "name": name,
        "rules": {
            "source_file": {
                "type": "SEQ",
                "members": [
                    { "type": "STRING", "value": "prefix" },
                    {
                        "type": "CHOICE",
                        "members": [
                            { "type": "STRING", "value": val },
                            { "type": "BLANK" }
                        ]
                    }
                ]
            }
        }
    })
}

/// JSON grammar with FIELD.
fn field_json(name: &str, field_name: &str, val: &str) -> Value {
    json!({
        "name": name,
        "rules": {
            "source_file": {
                "type": "FIELD",
                "name": field_name,
                "content": { "type": "STRING", "value": val }
            }
        }
    })
}

// ===========================================================================
// Strategies
// ===========================================================================

fn grammar_name() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("alpha".to_string()),
        Just("beta".to_string()),
        Just("gamma".to_string()),
        Just("delta".to_string()),
        Just("zeta".to_string()),
        Just("eta".to_string()),
    ]
}

fn safe_pattern() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("[a-z]+".to_string()),
        Just("[0-9]+".to_string()),
        Just("[A-Z][a-z]*".to_string()),
        Just("[a-zA-Z_]+".to_string()),
        Just("[0-9a-fA-F]+".to_string()),
    ]
}

fn safe_token_val() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("foo".to_string()),
        Just("bar".to_string()),
        Just("baz".to_string()),
        Just("qux".to_string()),
        Just("nop".to_string()),
    ]
}

fn safe_field_name() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("left".to_string()),
        Just("right".to_string()),
        Just("body".to_string()),
        Just("name".to_string()),
        Just("value".to_string()),
    ]
}

const ALTS_A: [&str; 6] = ["aa", "bb", "cc", "dd", "ee", "ff"];
const SEQ_VALS: [&str; 5] = ["pp", "qq", "rr", "ss", "tt"];

// ===========================================================================
// 1. prop_json_roundtrip_* — JSON roundtrip properties (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_json_roundtrip_name_preserved_through_ir(name in grammar_name()) {
        let g = minimal_json(&name);
        let r = build_json_ok(&g);
        prop_assert_eq!(r.grammar_name, name);
    }

    #[test]
    fn prop_json_roundtrip_ir_and_json_same_name(name in grammar_name()) {
        let ir = build_ir_ok(&name, &[("tok", "hello")], &[("s", vec!["tok"])], "s");
        let js = build_json_ok(&minimal_json(&name));
        prop_assert_eq!(ir.grammar_name.clone(), name.clone());
        prop_assert_eq!(js.grammar_name, name);
    }

    #[test]
    fn prop_json_roundtrip_both_paths_produce_states(name in grammar_name()) {
        let ir = build_ir_ok(&name, &[("tok", "hello")], &[("s", vec!["tok"])], "s");
        let js = build_json_ok(&minimal_json(&name));
        prop_assert!(ir.build_stats.state_count > 0);
        prop_assert!(js.build_stats.state_count > 0);
    }

    #[test]
    fn prop_json_roundtrip_both_paths_produce_symbols(name in grammar_name()) {
        let ir = build_ir_ok(&name, &[("tok", "hello")], &[("s", vec!["tok"])], "s");
        let js = build_json_ok(&minimal_json(&name));
        prop_assert!(ir.build_stats.symbol_count > 0);
        prop_assert!(js.build_stats.symbol_count > 0);
    }

    #[test]
    fn prop_json_roundtrip_both_paths_valid_node_types(name in grammar_name()) {
        let ir = build_ir_ok(&name, &[("tok", "hello")], &[("s", vec!["tok"])], "s");
        let js = build_json_ok(&minimal_json(&name));
        let ir_nt: Value = serde_json::from_str(&ir.node_types_json).unwrap();
        let js_nt: Value = serde_json::from_str(&js.node_types_json).unwrap();
        prop_assert!(ir_nt.is_array());
        prop_assert!(js_nt.is_array());
    }

    #[test]
    fn prop_json_roundtrip_compressed_uncompressed_same_stats(name in grammar_name()) {
        let g = minimal_json(&name);
        let r1 = build_parser_from_json(g.to_string(), test_opts()).unwrap();
        let r2 = build_parser_from_json(g.to_string(), uncompressed_opts()).unwrap();
        prop_assert_eq!(r1.build_stats.state_count, r2.build_stats.state_count);
        prop_assert_eq!(r1.build_stats.symbol_count, r2.build_stats.symbol_count);
    }
}

// ===========================================================================
// 2. prop_json_format_* — format properties (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_json_format_node_types_is_valid_json(name in grammar_name()) {
        let r = build_json_ok(&minimal_json(&name));
        let parsed: Value = serde_json::from_str(&r.node_types_json)
            .expect("node_types_json must be valid JSON");
        prop_assert!(parsed.is_array());
    }

    #[test]
    fn prop_json_format_parser_code_nonempty(name in grammar_name()) {
        let r = build_json_ok(&minimal_json(&name));
        prop_assert!(!r.parser_code.is_empty());
    }

    #[test]
    fn prop_json_format_parser_path_nonempty(name in grammar_name()) {
        let r = build_json_ok(&minimal_json(&name));
        prop_assert!(!r.parser_path.is_empty());
    }

    #[test]
    fn prop_json_format_name_in_parser_code(name in grammar_name()) {
        let r = build_json_ok(&minimal_json(&name));
        prop_assert!(
            r.parser_code.contains(&name),
            "parser_code should embed grammar name '{}'",
            name,
        );
    }

    #[test]
    fn prop_json_format_node_types_entries_have_type(name in grammar_name()) {
        let r = build_json_ok(&symbol_ref_json(&name, "tok"));
        let arr: Vec<Value> = serde_json::from_str(&r.node_types_json).unwrap();
        for entry in &arr {
            prop_assert!(entry.get("type").is_some());
        }
    }

    #[test]
    fn prop_json_format_node_types_type_is_string(name in grammar_name()) {
        let r = build_json_ok(&symbol_ref_json(&name, "tok"));
        let arr: Vec<Value> = serde_json::from_str(&r.node_types_json).unwrap();
        for entry in &arr {
            if let Some(t) = entry.get("type") {
                prop_assert!(t.is_string());
            }
        }
    }
}

// ===========================================================================
// 3. prop_json_tokens_* — token representation (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_json_tokens_string_builds_ok(name in grammar_name()) {
        let r = build_json_ok(&minimal_json(&name));
        prop_assert!(r.build_stats.state_count > 0);
        prop_assert!(r.build_stats.symbol_count > 0);
    }

    #[test]
    fn prop_json_tokens_pattern_builds_ok(pat in safe_pattern()) {
        let r = build_json_ok(&pattern_json("tokpat", &pat));
        prop_assert!(r.build_stats.state_count > 0);
        prop_assert!(r.build_stats.symbol_count > 0);
    }

    #[test]
    fn prop_json_tokens_token_wrapper_builds_ok(name in grammar_name()) {
        let r = build_json_ok(&token_wrapper_json(&name, "lit"));
        prop_assert!(r.build_stats.state_count > 0);
    }

    #[test]
    fn prop_json_tokens_choice_has_enough_symbols(n in 2..=5usize) {
        let alts = &ALTS_A[..n];
        let r = build_json_ok(&choice_json("tok_ch", alts));
        prop_assert!(
            r.build_stats.symbol_count >= n,
            "Expected >= {} symbols, got {}",
            n,
            r.build_stats.symbol_count,
        );
    }

    #[test]
    fn prop_json_tokens_repeat_produces_symbols(name in grammar_name()) {
        let r = build_json_ok(&repeat_json(&name, "rep"));
        prop_assert!(r.build_stats.symbol_count > 0);
    }

    #[test]
    fn prop_json_tokens_repeat1_produces_symbols(name in grammar_name()) {
        let r = build_json_ok(&repeat1_json(&name, "rep1"));
        prop_assert!(r.build_stats.symbol_count > 0);
    }
}

// ===========================================================================
// 4. prop_json_rules_* — rule representation (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_json_rules_symbol_ref_includes_source_file(name in grammar_name()) {
        let r = build_json_ok(&symbol_ref_json(&name, "tok"));
        let arr: Vec<Value> = serde_json::from_str(&r.node_types_json).unwrap();
        let has_sf = arr.iter().any(|e| {
            e.get("type").and_then(|v| v.as_str()) == Some("source_file")
        });
        prop_assert!(has_sf, "node_types should include source_file");
    }

    #[test]
    fn prop_json_rules_seq_builds_ok(name in grammar_name()) {
        let r = build_json_ok(&seq_json(&name, &["mm", "nn"]));
        prop_assert!(!r.parser_code.is_empty());
    }

    #[test]
    fn prop_json_rules_multi_rule_builds_ok(n in 2..=4usize) {
        let pairs: Vec<(&str, &str)> = [("ra", "va"), ("rb", "vb"), ("rc", "vc"), ("rd", "vd")]
            .iter()
            .take(n)
            .copied()
            .collect();
        let r = build_json_ok(&multi_rule_json("mrule", &pairs));
        prop_assert!(r.build_stats.state_count > 0);
    }

    #[test]
    fn prop_json_rules_prec_left_builds_ok(name in grammar_name()) {
        let r = build_json_ok(&prec_left_json(&name, "pval", 1));
        prop_assert!(r.build_stats.state_count > 0);
    }

    #[test]
    fn prop_json_rules_optional_builds_ok(name in grammar_name()) {
        let r = build_json_ok(&optional_json(&name, "maybe"));
        prop_assert!(r.build_stats.state_count > 0);
    }

    #[test]
    fn prop_json_rules_field_builds_ok(
        name in grammar_name(),
        fname in safe_field_name(),
    ) {
        let r = build_json_ok(&field_json(&name, &fname, "fval"));
        prop_assert!(r.build_stats.state_count > 0);
    }
}

// ===========================================================================
// 5. prop_json_error_* — error handling (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_json_error_invalid_json_string(s in "[^{}\\[\\]\"]{1,20}") {
        let r = build_parser_from_json(s, test_opts());
        prop_assert!(r.is_err());
    }

    #[test]
    fn prop_json_error_numeric_input_rejected(n in 0..1000i32) {
        let r = build_parser_from_json(n.to_string(), test_opts());
        prop_assert!(r.is_err());
    }

    #[test]
    fn prop_json_error_empty_grammar_fails(name in grammar_name()) {
        let g = GrammarBuilder::new(&name).build();
        let result = build_parser(g, test_opts());
        prop_assert!(result.is_err(), "empty grammar should fail");
    }

    #[test]
    fn prop_json_error_message_nonempty(name in grammar_name()) {
        let g = GrammarBuilder::new(&name).build();
        if let Err(e) = build_parser(g, test_opts()) {
            let msg = format!("{e}");
            prop_assert!(!msg.is_empty());
        }
    }

    #[test]
    fn prop_json_error_missing_name_field(tok in safe_token_val()) {
        let g = json!({
            "rules": {
                "source_file": { "type": "STRING", "value": tok }
            }
        });
        let r = build_parser_from_json(g.to_string(), test_opts());
        prop_assert!(r.is_err());
    }

    #[test]
    fn prop_json_error_missing_rules_field(name in grammar_name()) {
        let g = json!({ "name": name });
        let r = build_parser_from_json(g.to_string(), test_opts());
        prop_assert!(r.is_err());
    }
}

// ===========================================================================
// 6. prop_json_complex_* — complex grammars (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_json_complex_nested_choice_in_seq(name in grammar_name()) {
        let g = json!({
            "name": name,
            "rules": {
                "source_file": {
                    "type": "SEQ",
                    "members": [
                        { "type": "STRING", "value": "start" },
                        {
                            "type": "CHOICE",
                            "members": [
                                { "type": "STRING", "value": "aa" },
                                { "type": "STRING", "value": "bb" }
                            ]
                        }
                    ]
                }
            }
        });
        let r = build_json_ok(&g);
        prop_assert!(r.build_stats.state_count > 0);
    }

    #[test]
    fn prop_json_complex_repeat_of_choice(name in grammar_name()) {
        let g = json!({
            "name": name,
            "rules": {
                "source_file": {
                    "type": "REPEAT",
                    "content": {
                        "type": "CHOICE",
                        "members": [
                            { "type": "STRING", "value": "xx" },
                            { "type": "STRING", "value": "yy" }
                        ]
                    }
                }
            }
        });
        let r = build_json_ok(&g);
        prop_assert!(!r.parser_code.is_empty());
    }

    #[test]
    fn prop_json_complex_multi_rule_choice(n in 2..=4usize) {
        let pairs: Vec<(&str, &str)> = [("sa", "va"), ("sb", "vb"), ("sc", "vc"), ("sd", "vd")]
            .iter()
            .take(n)
            .copied()
            .collect();
        let r = build_json_ok(&multi_rule_json("cmplx", &pairs));
        let arr: Vec<Value> = serde_json::from_str(&r.node_types_json).unwrap();
        prop_assert!(!arr.is_empty());
    }

    #[test]
    fn prop_json_complex_seq_of_symbols(name in grammar_name()) {
        let g = json!({
            "name": name,
            "rules": {
                "source_file": {
                    "type": "SEQ",
                    "members": [
                        { "type": "SYMBOL", "name": "part_a" },
                        { "type": "SYMBOL", "name": "part_b" }
                    ]
                },
                "part_a": { "type": "STRING", "value": "aaa" },
                "part_b": { "type": "STRING", "value": "bbb" }
            }
        });
        let r = build_json_ok(&g);
        prop_assert!(r.build_stats.symbol_count >= 2);
    }

    #[test]
    fn prop_json_complex_more_alts_more_symbols(n in 2..=4usize) {
        let small = &ALTS_A[..n];
        let large = &ALTS_A[..n + 1];
        let r_s = build_json_ok(&choice_json("cx_s", small));
        let r_l = build_json_ok(&choice_json("cx_l", large));
        prop_assert!(r_l.build_stats.symbol_count >= r_s.build_stats.symbol_count);
    }

    #[test]
    fn prop_json_complex_longer_seq_more_code(n in 2..=4usize) {
        let small = &SEQ_VALS[..n];
        let large = &SEQ_VALS[..n + 1];
        let r_s = build_json_ok(&seq_json("cx_sq_s", small));
        let r_l = build_json_ok(&seq_json("cx_sq_l", large));
        prop_assert!(r_l.parser_code.len() >= r_s.parser_code.len());
    }
}

// ===========================================================================
// 7. prop_json_deterministic_* — deterministic output (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_json_deterministic_parser_code(name in grammar_name()) {
        let g = minimal_json(&name);
        let r1 = build_json_ok(&g);
        let r2 = build_json_ok(&g);
        prop_assert_eq!(r1.parser_code, r2.parser_code);
    }

    #[test]
    fn prop_json_deterministic_node_types(name in grammar_name()) {
        let g = minimal_json(&name);
        let r1 = build_json_ok(&g);
        let r2 = build_json_ok(&g);
        prop_assert_eq!(r1.node_types_json, r2.node_types_json);
    }

    #[test]
    fn prop_json_deterministic_state_count(name in grammar_name()) {
        let g = choice_json(&name, &["ab", "cd", "ef"]);
        let r1 = build_json_ok(&g);
        let r2 = build_json_ok(&g);
        prop_assert_eq!(r1.build_stats.state_count, r2.build_stats.state_count);
    }

    #[test]
    fn prop_json_deterministic_symbol_count(name in grammar_name()) {
        let g = choice_json(&name, &["ab", "cd", "ef"]);
        let r1 = build_json_ok(&g);
        let r2 = build_json_ok(&g);
        prop_assert_eq!(r1.build_stats.symbol_count, r2.build_stats.symbol_count);
    }

    #[test]
    fn prop_json_deterministic_conflict_cells(name in grammar_name()) {
        let g = minimal_json(&name);
        let r1 = build_json_ok(&g);
        let r2 = build_json_ok(&g);
        prop_assert_eq!(r1.build_stats.conflict_cells, r2.build_stats.conflict_cells);
    }

    #[test]
    fn prop_json_deterministic_ir_path(name in grammar_name()) {
        let r1 = build_ir_ok(&name, &[("tok", "lit")], &[("s", vec!["tok"])], "s");
        let r2 = build_ir_ok(&name, &[("tok", "lit")], &[("s", vec!["tok"])], "s");
        prop_assert_eq!(r1.parser_code, r2.parser_code);
        prop_assert_eq!(r1.build_stats.state_count, r2.build_stats.state_count);
    }
}

// ===========================================================================
// 8. prop_json_parse_* — JSON parsing properties (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_json_parse_from_json_accepts_valid_grammar(name in grammar_name()) {
        let g = minimal_json(&name);
        let gjs = from_json(&g);
        prop_assert!(gjs.is_ok(), "from_json should accept valid grammar");
    }

    #[test]
    fn prop_json_parse_from_json_preserves_name(name in grammar_name()) {
        let g = minimal_json(&name);
        let gjs = from_json(&g).unwrap();
        prop_assert_eq!(gjs.name, name);
    }

    #[test]
    fn prop_json_parse_from_json_has_rules(name in grammar_name()) {
        let g = symbol_ref_json(&name, "tok");
        let gjs = from_json(&g).unwrap();
        prop_assert!(!gjs.rules.is_empty(), "parsed grammar should have rules");
    }

    #[test]
    fn prop_json_parse_converter_produces_grammar(name in grammar_name()) {
        let g = minimal_json(&name);
        let gjs = from_json(&g).unwrap();
        let converter = GrammarJsConverter::new(gjs);
        let grammar = converter.convert();
        prop_assert!(grammar.is_ok(), "converter should produce a valid grammar");
    }

    #[test]
    fn prop_json_parse_symbol_ref_has_two_rules(name in grammar_name()) {
        let g = symbol_ref_json(&name, "tok");
        let gjs = from_json(&g).unwrap();
        prop_assert!(
            gjs.rules.len() >= 2,
            "symbol_ref grammar should have at least 2 rules, got {}",
            gjs.rules.len(),
        );
    }

    #[test]
    fn prop_json_parse_choice_json_roundtrips(name in grammar_name()) {
        let g = choice_json(&name, &["xx", "yy"]);
        let gjs = from_json(&g).unwrap();
        prop_assert_eq!(gjs.name, name);
        prop_assert!(!gjs.rules.is_empty());
    }
}

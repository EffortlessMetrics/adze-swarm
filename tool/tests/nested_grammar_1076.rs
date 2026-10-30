//! Regression tests for #1076: nested grammar semantics must not be dropped.

use adze_ir::{Grammar, Symbol, TokenPattern};
use adze_tool::grammar_js::{GrammarJsConverter, from_json};
use adze_tool::pure_rust_builder::{BuildOptions, build_parser_from_json};
use serde_json::json;
use tempfile::TempDir;

fn convert(value: &serde_json::Value) -> anyhow::Result<Grammar> {
    let grammar_js = from_json(value)?;
    GrammarJsConverter::new(grammar_js).convert()
}

fn token_strings(grammar: &Grammar) -> Vec<String> {
    grammar
        .tokens
        .values()
        .filter_map(|t| match &t.pattern {
            TokenPattern::String(s) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

fn build(value: &serde_json::Value) -> anyhow::Result<adze_tool::pure_rust_builder::BuildResult> {
    let dir = TempDir::new()?;
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: false,
    };
    // Leak dir path only as string; builder writes nothing when emit_artifacts=false
    let result = build_parser_from_json(serde_json::to_string(value)?, opts);
    std::mem::forget(dir);
    result
}

#[test]
fn nested_seq_preserves_a_b_c_in_order() {
    let v = json!({
        "name": "nested_seq",
        "rules": {
            "source_file": {"type": "SEQ", "members": [
                {"type": "SEQ", "members": [
                    {"type": "STRING", "value": "a"},
                    {"type": "STRING", "value": "b"}
                ]},
                {"type": "STRING", "value": "c"}
            ]}
        }
    });
    let grammar = convert(&v).expect("nested seq should convert");
    let mut toks = token_strings(&grammar);
    toks.sort();
    assert_eq!(
        toks,
        vec!["a", "b", "c"],
        "nested seq members must not disappear"
    );
    // Source rule must reference two positions (nested seq + "c"), not only "c".
    let src = grammar
        .rule_names
        .iter()
        .find_map(|(id, n)| (n == "source_file").then_some(*id))
        .unwrap();
    let rhs = &grammar.rules[&src][0].rhs;
    assert_eq!(
        rhs.len(),
        2,
        "source_file rhs should keep nested seq and c, got {rhs:?}"
    );
    assert!(matches!(rhs[1], Symbol::Terminal(_)));
}

#[test]
fn nested_optional_preserves_a_and_b() {
    let v = json!({
        "name": "nested_opt",
        "rules": {
            "source_file": {"type": "SEQ", "members": [
                {"type": "OPTIONAL", "content": {"type": "STRING", "value": "a"}},
                {"type": "STRING", "value": "b"}
            ]}
        }
    });
    let grammar = convert(&v).expect("nested optional should convert");
    let mut toks = token_strings(&grammar);
    toks.sort();
    assert_eq!(toks, vec!["a", "b"]);
}

#[test]
fn missing_symbol_is_error_through_public_builder() {
    let v = json!({
        "name": "missing_sym",
        "rules": {
            "source_file": {"type": "SEQ", "members": [
                {"type": "SYMBOL", "name": "missing"},
                {"type": "STRING", "value": "b"}
            ]}
        }
    });
    let err = build(&v).expect_err("missing symbol must fail, not be dropped");
    assert!(
        format!("{err:#}").contains("missing"),
        "error should name missing symbol: {err:#}"
    );
}

#[test]
fn unknown_rule_type_is_error_not_silent_omission() {
    let v = json!({
        "name": "bad_type",
        "rules": {
            "source_file": {"type": "STRING", "value": "x"},
            "bad": {"type": "TOTALLY_INVENTED"}
        }
    });
    let err = build(&v).expect_err("unknown rule type must fail");
    assert!(
        format!("{err:#}").contains("TOTALLY_INVENTED"),
        "error should be contextual: {err:#}"
    );
}

#[test]
fn alias_is_rejected_not_dropped_to_epsilon() {
    let v = json!({
        "name": "aliased",
        "rules": {
            "source_file": {"type": "ALIAS", "content": {"type": "STRING", "value": "a"}, "value": "letter", "named": true}
        }
    });
    let err = build(&v).expect_err("unsupported alias must fail loudly");
    let msg = format!("{err:#}").to_lowercase();
    assert!(msg.contains("alias"), "error should mention alias: {err:#}");
}

#[test]
fn nested_seq_builds_successfully_through_public_builder() {
    let v = json!({
        "name": "nested_seq_build",
        "rules": {
            "source_file": {"type": "SEQ", "members": [
                {"type": "SEQ", "members": [
                    {"type": "STRING", "value": "a"},
                    {"type": "STRING", "value": "b"}
                ]},
                {"type": "STRING", "value": "c"}
            ]}
        }
    });
    let result = build(&v).expect("valid nested seq should build");
    assert_eq!(result.grammar_name, "nested_seq_build");
    assert!(result.build_stats.state_count > 0);
}

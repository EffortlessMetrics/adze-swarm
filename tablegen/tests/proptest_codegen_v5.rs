#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

//! Property-based tests for code generation in adze-tablegen.
//!
//! Categories (46 tests total):
//!  1. prop_codegen_deterministic_*  — deterministic output (6)
//!  2. prop_codegen_valid_*          — valid output (6)
//!  3. prop_codegen_contains_*       — expected content (6)
//!  4. prop_codegen_node_types_*     — node types JSON (6)
//!  5. prop_codegen_abi_*            — ABI layout properties (6)
//!  6. prop_codegen_format_*         — format properties (6)
//!  7. prop_codegen_complex_*        — complex grammar codegen (5)
//!  8. prop_codegen_roundtrip_*      — roundtrip properties (5)

use adze_glr_core::{FirstFollowSets, ParseTable, build_lr1_automaton};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{ExternalToken, FieldId, Grammar, SymbolId};
use adze_tablegen::abi::{
    TREE_SITTER_LANGUAGE_VERSION, TREE_SITTER_MIN_COMPATIBLE_LANGUAGE_VERSION,
};
use adze_tablegen::serializer::serialize_language;
use adze_tablegen::{AbiLanguageBuilder, NodeTypesGenerator, StaticLanguageGenerator};
use proptest::prelude::*;
use serde_json::Value;

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

/// Grammar name: lowercase ASCII, 1–13 chars.
fn grammar_name_strategy() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,12}".prop_filter("non-empty", |s| !s.is_empty())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a grammar and parse table via the full LR(1) pipeline.
fn build_pipeline(
    name: &str,
    tokens: &[(&str, &str)],
    rules: &[(&str, Vec<&str>)],
    start: &str,
) -> (Grammar, ParseTable) {
    let mut builder = GrammarBuilder::new(name);
    for &(tok_name, tok_pat) in tokens {
        builder = builder.token(tok_name, tok_pat);
    }
    for (rule_name, rhs) in rules {
        builder = builder.rule(rule_name, rhs.clone());
    }
    builder = builder.start(start);
    let mut g = builder.build();
    let ff = FirstFollowSets::compute_normalized(&mut g).unwrap();
    let pt = build_lr1_automaton(&g, &ff).unwrap();
    (g, pt)
}

/// Build a simple grammar (1 token, 1 rule) with the given name through the full pipeline.
fn simple_pipeline(name: &str) -> (Grammar, ParseTable) {
    build_pipeline(name, &[("a", "a")], &[("s", vec!["a"])], "s")
}

/// Build a two-alternative grammar through the full pipeline.
fn two_alt_pipeline(name: &str) -> (Grammar, ParseTable) {
    build_pipeline(
        name,
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a"]), ("s", vec!["b"])],
        "s",
    )
}

/// Build a chain grammar: s -> a b.
fn chain_pipeline(name: &str) -> (Grammar, ParseTable) {
    build_pipeline(
        name,
        &[("a", "a"), ("b", "b")],
        &[("s", vec!["a", "b"])],
        "s",
    )
}

/// Build a multi-nonterminal grammar: inner -> a, s -> inner b.
fn multi_nt_pipeline(name: &str) -> (Grammar, ParseTable) {
    build_pipeline(
        name,
        &[("a", "a"), ("b", "b")],
        &[("inner", vec!["a"]), ("s", vec!["inner", "b"])],
        "s",
    )
}

/// Build a left-recursive grammar: s -> a | s a.
fn recursive_pipeline(name: &str) -> (Grammar, ParseTable) {
    build_pipeline(
        name,
        &[("a", "a")],
        &[("s", vec!["a"]), ("s", vec!["s", "a"])],
        "s",
    )
}

/// Build a deep nesting grammar: leaf -> a, mid -> leaf, s -> mid.
fn deep_pipeline(name: &str) -> (Grammar, ParseTable) {
    build_pipeline(
        name,
        &[("a", "a")],
        &[
            ("leaf", vec!["a"]),
            ("mid", vec!["leaf"]),
            ("s", vec!["mid"]),
        ],
        "s",
    )
}

/// Attach external tokens to a grammar (mutates in place).
fn add_externals(grammar: &mut Grammar, names: &[&str]) {
    for (i, name) in names.iter().enumerate() {
        grammar.externals.push(ExternalToken {
            name: (*name).to_string(),
            symbol_id: SymbolId(200 + i as u16),
        });
    }
}

/// Attach field entries to a grammar (mutates in place).
fn add_fields(grammar: &mut Grammar, names: &[&str]) {
    for (i, name) in names.iter().enumerate() {
        grammar
            .fields
            .insert(FieldId(i as u16), (*name).to_string());
    }
}

// ---------------------------------------------------------------------------
// 1. Deterministic output tests (6)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, .. ProptestConfig::default() })]

    /// StaticLanguageGenerator code is identical across two invocations.
    #[test]
    fn prop_codegen_deterministic_static_lang(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let gen1 = StaticLanguageGenerator::new(g.clone(), pt.clone());
        let gen2 = StaticLanguageGenerator::new(g, pt);
        let code1 = gen1.generate_language_code().to_string();
        let code2 = gen2.generate_language_code().to_string();
        prop_assert_eq!(code1, code2, "StaticLanguageGenerator must be deterministic");
    }

    /// AbiLanguageBuilder code is identical across two invocations.
    #[test]
    fn prop_codegen_deterministic_abi(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let code1 = AbiLanguageBuilder::new(&g, &pt).generate().to_string();
        let code2 = AbiLanguageBuilder::new(&g, &pt).generate().to_string();
        prop_assert_eq!(code1, code2, "AbiLanguageBuilder must be deterministic");
    }

    /// NodeTypesGenerator JSON is identical across two invocations.
    #[test]
    fn prop_codegen_deterministic_node_types(name in grammar_name_strategy()) {
        let (g, _pt) = simple_pipeline(&name);
        let j1 = NodeTypesGenerator::new(&g).generate().unwrap();
        let j2 = NodeTypesGenerator::new(&g).generate().unwrap();
        prop_assert_eq!(j1, j2, "NodeTypesGenerator must be deterministic");
    }

    /// StaticLanguageGenerator::generate_node_types is deterministic.
    #[test]
    fn prop_codegen_deterministic_static_node_types(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let slg = StaticLanguageGenerator::new(g, pt);
        let j1 = slg.generate_node_types();
        let j2 = slg.generate_node_types();
        prop_assert_eq!(j1, j2, "generate_node_types must be deterministic");
    }

    /// serialize_language produces identical output on two calls.
    #[test]
    fn prop_codegen_deterministic_serializer(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let s1 = serialize_language(&g, &pt, None).unwrap();
        let s2 = serialize_language(&g, &pt, None).unwrap();
        prop_assert_eq!(s1, s2, "serialize_language must be deterministic");
    }

    /// Different grammar names produce different StaticLanguageGenerator output.
    #[test]
    fn prop_codegen_deterministic_name_sensitivity(
        n1 in grammar_name_strategy(),
        n2 in grammar_name_strategy(),
    ) {
        prop_assume!(n1 != n2);
        let (g1, pt1) = simple_pipeline(&n1);
        let (g2, pt2) = simple_pipeline(&n2);
        let c1 = StaticLanguageGenerator::new(g1, pt1).generate_language_code().to_string();
        let c2 = StaticLanguageGenerator::new(g2, pt2).generate_language_code().to_string();
        prop_assert_ne!(c1, c2, "different names must yield different code");
    }

    // -----------------------------------------------------------------------
    // 2. Valid output tests (6)
    // -----------------------------------------------------------------------

    /// StaticLanguageGenerator produces a non-empty TokenStream.
    #[test]
    fn prop_codegen_valid_static_nonempty(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let code = StaticLanguageGenerator::new(g, pt).generate_language_code();
        prop_assert!(!code.is_empty(), "generated code must not be empty");
    }

    /// AbiLanguageBuilder produces a non-empty TokenStream.
    #[test]
    fn prop_codegen_valid_abi_nonempty(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let code = AbiLanguageBuilder::new(&g, &pt).generate();
        prop_assert!(!code.is_empty(), "ABI code must not be empty");
    }

    /// StaticLanguageGenerator code can be parsed as valid Rust tokens by syn.
    #[test]
    fn prop_codegen_valid_syn_parse(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let code = StaticLanguageGenerator::new(g, pt).generate_language_code().to_string();
        let result = syn::parse_str::<syn::File>(&code);
        prop_assert!(result.is_ok(), "generated code must be valid Rust: {:?}", result.err());
    }

    /// NodeTypesGenerator output is valid JSON.
    #[test]
    fn prop_codegen_valid_node_types_json(name in grammar_name_strategy()) {
        let (g, _pt) = simple_pipeline(&name);
        let json_str = NodeTypesGenerator::new(&g).generate().unwrap();
        let parsed: std::result::Result<Value, _> = serde_json::from_str(&json_str);
        prop_assert!(parsed.is_ok(), "node types must be valid JSON");
    }

    /// serialize_language returns valid JSON.
    #[test]
    fn prop_codegen_valid_serializer_json(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let json_str = serialize_language(&g, &pt, None).unwrap();
        let parsed: std::result::Result<Value, _> = serde_json::from_str(&json_str);
        prop_assert!(parsed.is_ok(), "serialized language must be valid JSON");
    }

    /// Two-alternative grammar produces valid code.
    #[test]
    fn prop_codegen_valid_two_alt(name in grammar_name_strategy()) {
        let (g, pt) = two_alt_pipeline(&name);
        let code = StaticLanguageGenerator::new(g, pt).generate_language_code();
        prop_assert!(!code.is_empty(), "two-alt grammar must produce non-empty code");
    }

    // -----------------------------------------------------------------------
    // 3. Expected content tests (6)
    // -----------------------------------------------------------------------

    /// Generated code contains `language` function.
    #[test]
    fn prop_codegen_contains_language_fn(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let code = StaticLanguageGenerator::new(g, pt).generate_language_code().to_string();
        prop_assert!(code.contains("language"), "code must contain 'language' function");
    }

    /// Generated code contains PARSE_TABLE or parse_table reference.
    #[test]
    fn prop_codegen_contains_parse_table(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let code = StaticLanguageGenerator::new(g, pt).generate_language_code().to_string();
        let has_table = code.contains("PARSE_TABLE") || code.contains("parse_table");
        prop_assert!(has_table, "code must reference parse table");
    }

    /// ABI generated code contains symbol_count field.
    #[test]
    fn prop_codegen_contains_symbol_count(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let code = AbiLanguageBuilder::new(&g, &pt).generate().to_string();
        prop_assert!(code.contains("symbol_count"), "ABI code must contain symbol_count");
    }

    /// ABI generated code contains state_count field.
    #[test]
    fn prop_codegen_contains_state_count(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let code = AbiLanguageBuilder::new(&g, &pt).generate().to_string();
        prop_assert!(code.contains("state_count"), "ABI code must contain state_count");
    }

    /// ABI generated code contains token_count field.
    #[test]
    fn prop_codegen_contains_token_count(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let code = AbiLanguageBuilder::new(&g, &pt).generate().to_string();
        prop_assert!(code.contains("token_count"), "ABI code must contain token_count");
    }

    /// Generated code references SYMBOL_NAMES or symbol_names.
    #[test]
    fn prop_codegen_contains_symbol_names(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let code = StaticLanguageGenerator::new(g, pt).generate_language_code().to_string();
        let has = code.contains("SYMBOL_NAMES") || code.contains("symbol_names");
        prop_assert!(has, "code must reference symbol names");
    }

    // -----------------------------------------------------------------------
    // 4. Node types JSON tests (6)
    // -----------------------------------------------------------------------

    /// Node types JSON is an array.
    #[test]
    fn prop_codegen_node_types_is_array(name in grammar_name_strategy()) {
        let (g, _pt) = simple_pipeline(&name);
        let json_str = NodeTypesGenerator::new(&g).generate().unwrap();
        let val: Value = serde_json::from_str(&json_str).unwrap();
        prop_assert!(val.is_array(), "node types must be a JSON array");
    }

    /// Each node type entry has a "type" field.
    #[test]
    fn prop_codegen_node_types_has_type_field(name in grammar_name_strategy()) {
        let (g, _pt) = simple_pipeline(&name);
        let json_str = NodeTypesGenerator::new(&g).generate().unwrap();
        let val: Value = serde_json::from_str(&json_str).unwrap();
        if let Some(arr) = val.as_array() {
            for entry in arr {
                prop_assert!(entry.get("type").is_some(), "each entry must have 'type' field");
            }
        }
    }

    /// Each node type entry has a "named" field.
    #[test]
    fn prop_codegen_node_types_has_named_field(name in grammar_name_strategy()) {
        let (g, _pt) = simple_pipeline(&name);
        let json_str = NodeTypesGenerator::new(&g).generate().unwrap();
        let val: Value = serde_json::from_str(&json_str).unwrap();
        if let Some(arr) = val.as_array() {
            for entry in arr {
                prop_assert!(entry.get("named").is_some(), "each entry must have 'named' field");
            }
        }
    }

    /// StaticLanguageGenerator node types JSON is also valid.
    #[test]
    fn prop_codegen_node_types_static_gen_valid(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let json_str = StaticLanguageGenerator::new(g, pt).generate_node_types();
        let parsed: std::result::Result<Value, _> = serde_json::from_str(&json_str);
        prop_assert!(parsed.is_ok(), "static gen node types must be valid JSON");
    }

    /// Adding external tokens to grammar includes them in StaticLanguageGenerator node types.
    #[test]
    fn prop_codegen_node_types_includes_externals(name in grammar_name_strategy()) {
        let (mut g, pt) = simple_pipeline(&name);
        add_externals(&mut g, &["ext_alpha"]);
        let slg = StaticLanguageGenerator::new(g, pt);
        let json_str = slg.generate_node_types();
        let val: Value = serde_json::from_str(&json_str).unwrap();
        let arr = val.as_array().unwrap();
        let has_ext = arr.iter().any(|e| {
            e.get("type").and_then(|t| t.as_str()) == Some("ext_alpha")
        });
        prop_assert!(has_ext, "external token must appear in node types");
    }

    /// Hidden externals (starting with _) are excluded from StaticLanguageGenerator node types.
    #[test]
    fn prop_codegen_node_types_excludes_hidden(name in grammar_name_strategy()) {
        let (mut g, pt) = simple_pipeline(&name);
        add_externals(&mut g, &["_hidden_tok"]);
        let slg = StaticLanguageGenerator::new(g, pt);
        let json_str = slg.generate_node_types();
        let val: Value = serde_json::from_str(&json_str).unwrap();
        let arr = val.as_array().unwrap();
        let has_hidden = arr.iter().any(|e| {
            e.get("type").and_then(|t| t.as_str()) == Some("_hidden_tok")
        });
        prop_assert!(!has_hidden, "hidden tokens must not appear in node types");
    }

    // -----------------------------------------------------------------------
    // 5. ABI layout properties (6)
    // -----------------------------------------------------------------------

    /// ABI code references the language version constant.
    #[test]
    fn prop_codegen_abi_version_reference(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let code = AbiLanguageBuilder::new(&g, &pt).generate().to_string();
        prop_assert!(
            code.contains("TREE_SITTER_LANGUAGE_VERSION"),
            "ABI must reference version constant"
        );
    }

    /// ABI state_count matches parse table state count.
    #[test]
    fn prop_codegen_abi_state_count_matches(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let code = AbiLanguageBuilder::new(&g, &pt).generate().to_string();
        let sc = pt.state_count as u32;
        let needle = format!("state_count : {sc}u32");
        prop_assert!(code.contains(&needle), "state_count must be {sc}u32");
    }

    /// ABI symbol_count matches parse table symbol count.
    #[test]
    fn prop_codegen_abi_symbol_count_matches(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let code = AbiLanguageBuilder::new(&g, &pt).generate().to_string();
        let sc = pt.symbol_count as u32;
        let needle = format!("symbol_count : {sc}u32");
        prop_assert!(code.contains(&needle), "symbol_count must be {sc}u32");
    }

    /// ABI version constants are sane.
    #[test]
    fn prop_codegen_abi_version_sane(_dummy in 0u8..1) {
        prop_assert_eq!(TREE_SITTER_LANGUAGE_VERSION, 15u32);
        prop_assert!(
            TREE_SITTER_LANGUAGE_VERSION >= TREE_SITTER_MIN_COMPATIBLE_LANGUAGE_VERSION,
            "max version must be >= min version"
        );
    }

    /// ABI code contains external_token_count field.
    #[test]
    fn prop_codegen_abi_external_token_count(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let code = AbiLanguageBuilder::new(&g, &pt).generate().to_string();
        prop_assert!(
            code.contains("external_token_count"),
            "ABI must contain external_token_count"
        );
    }

    /// ABI code contains field_count.
    #[test]
    fn prop_codegen_abi_field_count(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let code = AbiLanguageBuilder::new(&g, &pt).generate().to_string();
        prop_assert!(
            code.contains("field_count"),
            "ABI must contain field_count"
        );
    }

    // -----------------------------------------------------------------------
    // 6. Format properties (6)
    // -----------------------------------------------------------------------

    /// Serialized language JSON has "version" key.
    #[test]
    fn prop_codegen_format_has_version(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let json_str = serialize_language(&g, &pt, None).unwrap();
        let val: Value = serde_json::from_str(&json_str).unwrap();
        prop_assert!(val.get("version").is_some(), "JSON must have 'version' key");
    }

    /// Serialized language JSON version matches TREE_SITTER_LANGUAGE_VERSION.
    #[test]
    fn prop_codegen_format_version_value(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let json_str = serialize_language(&g, &pt, None).unwrap();
        let val: Value = serde_json::from_str(&json_str).unwrap();
        let ver = val.get("version").and_then(|v| v.as_u64()).unwrap_or(0);
        prop_assert_eq!(ver, u64::from(TREE_SITTER_LANGUAGE_VERSION));
    }

    /// Serialized language JSON has "symbol_count" key.
    #[test]
    fn prop_codegen_format_has_symbol_count(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let json_str = serialize_language(&g, &pt, None).unwrap();
        let val: Value = serde_json::from_str(&json_str).unwrap();
        prop_assert!(val.get("symbol_count").is_some(), "JSON must have 'symbol_count'");
    }

    /// Serialized language JSON has "state_count" key.
    #[test]
    fn prop_codegen_format_has_state_count(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let json_str = serialize_language(&g, &pt, None).unwrap();
        let val: Value = serde_json::from_str(&json_str).unwrap();
        prop_assert!(val.get("state_count").is_some(), "JSON must have 'state_count'");
    }

    /// Serialized language JSON state_count matches parse table.
    #[test]
    fn prop_codegen_format_state_count_matches(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let json_str = serialize_language(&g, &pt, None).unwrap();
        let val: Value = serde_json::from_str(&json_str).unwrap();
        let sc = val.get("state_count").and_then(|v| v.as_u64()).unwrap();
        prop_assert_eq!(sc, pt.state_count as u64, "state_count must match parse table");
    }

    /// Serialized language JSON has "field_count" key.
    #[test]
    fn prop_codegen_format_has_field_count(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let json_str = serialize_language(&g, &pt, None).unwrap();
        let val: Value = serde_json::from_str(&json_str).unwrap();
        prop_assert!(val.get("field_count").is_some(), "JSON must have 'field_count'");
    }

    // -----------------------------------------------------------------------
    // 7. Complex grammar codegen (5)
    // -----------------------------------------------------------------------

    /// Chain grammar (s -> a b) produces valid code.
    #[test]
    fn prop_codegen_complex_chain(name in grammar_name_strategy()) {
        let (g, pt) = chain_pipeline(&name);
        let code = StaticLanguageGenerator::new(g, pt).generate_language_code();
        prop_assert!(!code.is_empty(), "chain grammar must produce non-empty code");
    }

    /// Left-recursive grammar produces valid code.
    #[test]
    fn prop_codegen_complex_recursive(name in grammar_name_strategy()) {
        let (g, pt) = recursive_pipeline(&name);
        let code = StaticLanguageGenerator::new(g, pt).generate_language_code();
        prop_assert!(!code.is_empty(), "recursive grammar must produce non-empty code");
    }

    /// Multi-nonterminal grammar produces valid ABI code.
    #[test]
    fn prop_codegen_complex_multi_nt(name in grammar_name_strategy()) {
        let (g, pt) = multi_nt_pipeline(&name);
        let code = AbiLanguageBuilder::new(&g, &pt).generate();
        prop_assert!(!code.is_empty(), "multi-NT grammar must produce non-empty ABI code");
    }

    /// Deep nesting grammar produces valid code.
    #[test]
    fn prop_codegen_complex_deep_nesting(name in grammar_name_strategy()) {
        let (g, pt) = deep_pipeline(&name);
        let code = StaticLanguageGenerator::new(g, pt).generate_language_code();
        prop_assert!(!code.is_empty(), "deep nesting grammar must produce non-empty code");
    }

    /// Grammar with fields produces code referencing field_count.
    #[test]
    fn prop_codegen_complex_with_fields(name in grammar_name_strategy()) {
        let (mut g, pt) = simple_pipeline(&name);
        add_fields(&mut g, &["lhs", "rhs"]);
        let code = AbiLanguageBuilder::new(&g, &pt).generate().to_string();
        let fc = g.fields.len() as u32;
        let needle = format!("field_count : {fc}u32");
        prop_assert!(
            code.contains(&needle),
            "field_count must be {fc}u32 in ABI code"
        );
    }

    // -----------------------------------------------------------------------
    // 8. Roundtrip properties (5)
    // -----------------------------------------------------------------------

    /// StaticLanguageGenerator code survives a syn roundtrip for chain grammar.
    #[test]
    fn prop_codegen_roundtrip_chain_syn(name in grammar_name_strategy()) {
        let (g, pt) = chain_pipeline(&name);
        let code = StaticLanguageGenerator::new(g, pt).generate_language_code().to_string();
        let result = syn::parse_str::<syn::File>(&code);
        prop_assert!(result.is_ok(), "chain grammar code must parse as Rust: {:?}", result.err());
    }

    /// serialize_language JSON roundtrips through serde_json.
    #[test]
    fn prop_codegen_roundtrip_serializer_json(name in grammar_name_strategy()) {
        let (g, pt) = simple_pipeline(&name);
        let json_str = serialize_language(&g, &pt, None).unwrap();
        let val: Value = serde_json::from_str(&json_str).unwrap();
        let re_serialized = serde_json::to_string_pretty(&val).unwrap();
        let val2: Value = serde_json::from_str(&re_serialized).unwrap();
        prop_assert_eq!(val, val2, "JSON must roundtrip through serde_json");
    }

    /// NodeTypesGenerator JSON roundtrips through serde_json.
    #[test]
    fn prop_codegen_roundtrip_node_types_json(name in grammar_name_strategy()) {
        let (g, _pt) = simple_pipeline(&name);
        let json_str = NodeTypesGenerator::new(&g).generate().unwrap();
        let val: Value = serde_json::from_str(&json_str).unwrap();
        let re_serialized = serde_json::to_string_pretty(&val).unwrap();
        let val2: Value = serde_json::from_str(&re_serialized).unwrap();
        prop_assert_eq!(val, val2, "node types JSON must roundtrip");
    }

    /// Multi-NT grammar code survives a syn roundtrip.
    #[test]
    fn prop_codegen_roundtrip_multi_nt_syn(name in grammar_name_strategy()) {
        let (g, pt) = multi_nt_pipeline(&name);
        let code = StaticLanguageGenerator::new(g, pt).generate_language_code().to_string();
        let result = syn::parse_str::<syn::File>(&code);
        prop_assert!(result.is_ok(), "multi-NT code must parse as Rust: {:?}", result.err());
    }

    /// Recursive grammar code survives a syn roundtrip.
    #[test]
    fn prop_codegen_roundtrip_recursive_syn(name in grammar_name_strategy()) {
        let (g, pt) = recursive_pipeline(&name);
        let code = StaticLanguageGenerator::new(g, pt).generate_language_code().to_string();
        let result = syn::parse_str::<syn::File>(&code);
        prop_assert!(result.is_ok(), "recursive code must parse as Rust: {:?}", result.err());
    }
}

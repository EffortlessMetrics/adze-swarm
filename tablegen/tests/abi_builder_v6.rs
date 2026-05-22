//! ABI language builder v6 — 64 tests in 8 categories (8 × 8).
//!
//! Categories:
//!   abi_basic_*         — construction, pipeline, minimal usage
//!   abi_layout_*        — table dimensions, row/column counts
//!   abi_fields_*        — field metadata, field count, field-name mapping
//!   abi_symbol_*        — symbol names, metadata flags, token indices
//!   abi_complex_*       — recursion, precedence, nested, wide grammars
//!   abi_deterministic_* — determinism, reproducibility, ordering
//!   abi_serialize_*     — JSON serialization, node-types, token-stream output
//!   abi_edge_*          — nullable, empty, extras, external, compressed

use adze_glr_core::{FirstFollowSets, ParseTable, build_lr1_automaton};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{Associativity, FieldId, Grammar};
use adze_tablegen::compress::TableCompressor;
use adze_tablegen::{
    AbiLanguageBuilder, NodeTypesGenerator, StaticLanguageGenerator, collect_token_indices,
    eof_accepts_or_reduces,
};

// ============================================================================
// Helpers
// ============================================================================

fn build_table(grammar: &Grammar) -> ParseTable {
    let mut g = grammar.clone();
    let ff = FirstFollowSets::compute_normalized(&mut g).expect("FIRST/FOLLOW");
    build_lr1_automaton(&g, &ff).expect("LR(1)")
}

fn generate_code(grammar: &Grammar, table: &ParseTable) -> String {
    AbiLanguageBuilder::new(grammar, table)
        .generate()
        .to_string()
}

fn pipeline(name: &str) -> (Grammar, ParseTable, String) {
    let g = GrammarBuilder::new(name)
        .token("x", "x")
        .rule("S", vec!["x"])
        .start("S")
        .build();
    let pt = build_table(&g);
    let code = generate_code(&g, &pt);
    (g, pt, code)
}

fn single_token_grammar() -> Grammar {
    GrammarBuilder::new("single_tok")
        .token("x", "x")
        .rule("S", vec!["x"])
        .start("S")
        .build()
}

fn two_token_grammar() -> Grammar {
    GrammarBuilder::new("two_tok")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["a", "b"])
        .start("S")
        .build()
}

fn alternatives_grammar() -> Grammar {
    GrammarBuilder::new("alts")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .rule("S", vec!["a"])
        .rule("S", vec!["b"])
        .rule("S", vec!["c"])
        .start("S")
        .build()
}

fn nested_grammar() -> Grammar {
    GrammarBuilder::new("nested")
        .token("x", "x")
        .token("y", "y")
        .rule("S", vec!["A", "B"])
        .rule("A", vec!["x"])
        .rule("B", vec!["y"])
        .start("S")
        .build()
}

fn deep_chain_grammar() -> Grammar {
    GrammarBuilder::new("deep_chain")
        .token("z", "z")
        .rule("S", vec!["A"])
        .rule("A", vec!["B"])
        .rule("B", vec!["C"])
        .rule("C", vec!["z"])
        .start("S")
        .build()
}

fn left_recursive_grammar() -> Grammar {
    GrammarBuilder::new("left_rec")
        .token("a", "a")
        .rule("S", vec!["a"])
        .rule("S", vec!["S", "a"])
        .start("S")
        .build()
}

fn right_recursive_grammar() -> Grammar {
    GrammarBuilder::new("right_rec")
        .token("a", "a")
        .rule("S", vec!["a"])
        .rule("S", vec!["a", "S"])
        .start("S")
        .build()
}

fn precedence_grammar() -> Grammar {
    GrammarBuilder::new("prec")
        .token("NUM", r"\d+")
        .token("PLUS", r"\+")
        .token("STAR", r"\*")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "STAR", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build()
}

fn nullable_grammar() -> Grammar {
    GrammarBuilder::new("nullable")
        .token("a", "a")
        .rule("S", vec!["A"])
        .rule("A", vec!["a"])
        .rule("A", vec![])
        .start("S")
        .build()
}

fn wide_alternatives_grammar() -> Grammar {
    let mut gb = GrammarBuilder::new("wide");
    for i in 0..10u8 {
        let name = format!("t{i}");
        let pat = format!("{}", (b'a' + i) as char);
        gb = gb.token(&name, &pat);
        gb = gb.rule("S", vec![&name]);
    }
    gb.start("S").build()
}

fn long_sequence_grammar() -> Grammar {
    GrammarBuilder::new("long_seq")
        .token("t1", "a")
        .token("t2", "b")
        .token("t3", "c")
        .token("t4", "d")
        .token("t5", "e")
        .rule("S", vec!["t1", "t2", "t3", "t4", "t5"])
        .start("S")
        .build()
}

fn grammar_with_fields() -> Grammar {
    let mut g = GrammarBuilder::new("fielded")
        .token("x", "x")
        .token("y", "y")
        .rule("S", vec!["x", "y"])
        .start("S")
        .build();
    g.fields.insert(FieldId(1), "left".to_string());
    g.fields.insert(FieldId(2), "right".to_string());
    g
}

fn grammar_with_extra() -> Grammar {
    GrammarBuilder::new("extra_ws")
        .token("a", "a")
        .token("WS", r"\s+")
        .extra("WS")
        .rule("S", vec!["a"])
        .start("S")
        .build()
}

fn grammar_with_external() -> Grammar {
    GrammarBuilder::new("ext_scan")
        .token("a", "a")
        .external("INDENT")
        .rule("S", vec!["a"])
        .start("S")
        .build()
}

// ============================================================================
// 1. abi_basic_* — construction, pipeline, minimal usage (8 tests)
// ============================================================================

#[test]
fn abi_basic_construct_single_token() {
    let g = single_token_grammar();
    let pt = build_table(&g);
    let _builder = AbiLanguageBuilder::new(&g, &pt);
}

#[test]
fn abi_basic_construct_two_token() {
    let g = two_token_grammar();
    let pt = build_table(&g);
    let _builder = AbiLanguageBuilder::new(&g, &pt);
}

#[test]
fn abi_basic_construct_alternatives() {
    let g = alternatives_grammar();
    let pt = build_table(&g);
    let _builder = AbiLanguageBuilder::new(&g, &pt);
}

#[test]
fn abi_basic_construct_nested() {
    let g = nested_grammar();
    let pt = build_table(&g);
    let _builder = AbiLanguageBuilder::new(&g, &pt);
}

#[test]
fn abi_basic_construct_default_table() {
    let g = Grammar::new("empty".to_string());
    let pt = ParseTable::default();
    let _builder = AbiLanguageBuilder::new(&g, &pt);
}

#[test]
fn abi_basic_generate_produces_nonempty_output() {
    let g = single_token_grammar();
    let pt = build_table(&g);
    let code = generate_code(&g, &pt);
    assert!(!code.is_empty(), "generated code must be non-empty");
}

#[test]
fn abi_basic_pipeline_roundtrip() {
    let (_g, _pt, code) = pipeline("roundtrip");
    assert!(!code.is_empty());
}

#[test]
fn abi_basic_static_generator_produces_code() {
    let g = single_token_grammar();
    let pt = build_table(&g);
    let generator = StaticLanguageGenerator::new(g, pt);
    let code = generator.generate_language_code().to_string();
    assert!(!code.is_empty());
}

// ============================================================================
// 2. abi_layout_* — table dimensions, row/column counts (8 tests)
// ============================================================================

#[test]
fn abi_layout_action_table_rows_match_state_count() {
    let g = single_token_grammar();
    let pt = build_table(&g);
    assert_eq!(pt.action_table.len(), pt.state_count);
}

#[test]
fn abi_layout_goto_table_rows_match_state_count() {
    let g = single_token_grammar();
    let pt = build_table(&g);
    assert_eq!(pt.goto_table.len(), pt.state_count);
}

#[test]
fn abi_layout_action_columns_match_symbol_count() {
    let g = single_token_grammar();
    let pt = build_table(&g);
    for (i, row) in pt.action_table.iter().enumerate() {
        assert_eq!(
            row.len(),
            pt.symbol_count,
            "state {i}: action row width != symbol_count"
        );
    }
}

#[test]
fn abi_layout_goto_columns_match_symbol_count() {
    let g = single_token_grammar();
    let pt = build_table(&g);
    for (i, row) in pt.goto_table.iter().enumerate() {
        assert_eq!(
            row.len(),
            pt.symbol_count,
            "state {i}: goto row width != symbol_count"
        );
    }
}

#[test]
fn abi_layout_state_count_at_least_two() {
    let g = single_token_grammar();
    let pt = build_table(&g);
    assert!(
        pt.state_count >= 2,
        "need >= 2 states, got {}",
        pt.state_count
    );
}

#[test]
fn abi_layout_alternatives_state_count_at_least_two() {
    let g = alternatives_grammar();
    let pt = build_table(&g);
    assert!(pt.state_count >= 2);
}

#[test]
fn abi_layout_deep_chain_state_count_at_least_two() {
    let g = deep_chain_grammar();
    let pt = build_table(&g);
    assert!(pt.state_count >= 2);
}

#[test]
fn abi_layout_left_recursive_state_count_bounded() {
    let g = left_recursive_grammar();
    let pt = build_table(&g);
    assert!(pt.state_count < 1000, "state count should be bounded");
}

// ============================================================================
// 3. abi_fields_* — field metadata, count, name mapping (8 tests)
// ============================================================================

#[test]
fn abi_fields_empty_by_default() {
    let g = single_token_grammar();
    assert!(g.fields.is_empty(), "default grammar has no fields");
}

#[test]
fn abi_fields_inserted_manually() {
    let g = grammar_with_fields();
    assert_eq!(g.fields.len(), 2);
}

#[test]
fn abi_fields_names_preserved() {
    let g = grammar_with_fields();
    let names: Vec<&str> = g.fields.values().map(|s| s.as_str()).collect();
    assert!(names.contains(&"left"));
    assert!(names.contains(&"right"));
}

#[test]
fn abi_fields_code_generation_succeeds_with_fields() {
    let g = grammar_with_fields();
    let pt = build_table(&g);
    let code = generate_code(&g, &pt);
    assert!(!code.is_empty());
}

#[test]
fn abi_fields_id_copy_semantics() {
    let fid = FieldId(42);
    let fid2 = fid;
    assert_eq!(fid, fid2, "FieldId should be Copy");
}

#[test]
fn abi_fields_nested_grammar_no_fields() {
    let g = nested_grammar();
    assert!(g.fields.is_empty());
}

#[test]
fn abi_fields_node_types_reflect_fields() {
    let g = grammar_with_fields();
    let generator = NodeTypesGenerator::new(&g);
    let result = generator.generate();
    // Node types generation succeeds
    assert!(
        result.is_ok(),
        "node types gen should succeed: {:?}",
        result.err()
    );
}

#[test]
fn abi_fields_field_count_in_serialized_language() {
    let g = grammar_with_fields();
    let pt = build_table(&g);
    let json =
        adze_tablegen::serializer::serialize_language(&g, &pt, None).expect("serialize_language");
    assert!(
        json.contains("\"field_count\""),
        "JSON should contain field_count"
    );
}

// ============================================================================
// 4. abi_symbol_* — symbol names, metadata, token indices (8 tests)
// ============================================================================

#[test]
fn abi_symbol_count_single_token() {
    let g = single_token_grammar();
    let pt = build_table(&g);
    // At minimum: EOF + token "x" + non-terminal "S"
    assert!(pt.symbol_count >= 3, "got {}", pt.symbol_count);
}

#[test]
fn abi_symbol_count_alternatives() {
    let g = alternatives_grammar();
    let pt = build_table(&g);
    // EOF + 3 tokens + at least 1 non-terminal
    assert!(pt.symbol_count >= 5, "got {}", pt.symbol_count);
}

#[test]
fn abi_symbol_count_deep_chain() {
    let g = deep_chain_grammar();
    let pt = build_table(&g);
    // EOF + 1 token + 4 non-terminals (S, A, B, C)
    assert!(pt.symbol_count >= 5, "got {}", pt.symbol_count);
}

#[test]
fn abi_symbol_names_in_generated_code() {
    let g = single_token_grammar();
    let pt = build_table(&g);
    let code = generate_code(&g, &pt);
    assert!(
        code.contains("SYMBOL_NAMES") || code.contains("symbol_names"),
        "code should mention symbol names"
    );
}

#[test]
fn abi_symbol_metadata_in_generated_code() {
    let g = single_token_grammar();
    let pt = build_table(&g);
    let code = generate_code(&g, &pt);
    assert!(
        code.contains("SYMBOL_METADATA") || code.contains("symbol_metadata"),
        "code should mention symbol metadata"
    );
}

#[test]
fn abi_symbol_token_indices_nonempty() {
    let g = single_token_grammar();
    let pt = build_table(&g);
    let ti = collect_token_indices(&g, &pt);
    assert!(!ti.is_empty(), "token indices must be non-empty");
}

#[test]
fn abi_symbol_token_indices_match_token_count() {
    let g = alternatives_grammar();
    let pt = build_table(&g);
    let ti = collect_token_indices(&g, &pt);
    // At least as many token indices as declared tokens
    assert!(
        ti.len() >= g.tokens.len(),
        "token_indices ({}) < tokens ({})",
        ti.len(),
        g.tokens.len()
    );
}

#[test]
fn abi_symbol_alternatives_code_contains_all_tokens() {
    let g = alternatives_grammar();
    let pt = build_table(&g);
    let code = generate_code(&g, &pt);
    for name in &["a", "b", "c"] {
        assert!(code.contains(name), "code should contain token '{name}'");
    }
}

// ============================================================================
// 5. abi_complex_* — recursion, precedence, nested, wide (8 tests)
// ============================================================================

#[test]
fn abi_complex_left_recursive_generates() {
    let g = left_recursive_grammar();
    let pt = build_table(&g);
    let code = generate_code(&g, &pt);
    assert!(!code.is_empty());
}

#[test]
fn abi_complex_right_recursive_generates() {
    let g = right_recursive_grammar();
    let pt = build_table(&g);
    let code = generate_code(&g, &pt);
    assert!(!code.is_empty());
}

#[test]
fn abi_complex_precedence_generates() {
    let g = precedence_grammar();
    let pt = build_table(&g);
    let code = generate_code(&g, &pt);
    assert!(!code.is_empty());
}

#[test]
fn abi_complex_deep_chain_generates() {
    let g = deep_chain_grammar();
    let pt = build_table(&g);
    let code = generate_code(&g, &pt);
    assert!(!code.is_empty());
}

#[test]
fn abi_complex_wide_alternatives_generates() {
    let g = wide_alternatives_grammar();
    let pt = build_table(&g);
    let code = generate_code(&g, &pt);
    assert!(!code.is_empty());
}

#[test]
fn abi_complex_long_sequence_generates() {
    let g = long_sequence_grammar();
    let pt = build_table(&g);
    let code = generate_code(&g, &pt);
    assert!(!code.is_empty());
}

#[test]
fn abi_complex_precedence_symbol_count() {
    let g = precedence_grammar();
    let pt = build_table(&g);
    // NUM, PLUS, STAR, EOF, expr
    assert!(pt.symbol_count >= 5, "got {}", pt.symbol_count);
}

#[test]
fn abi_complex_nested_has_goto_entries() {
    let g = nested_grammar();
    let pt = build_table(&g);
    let has_goto = pt
        .goto_table
        .iter()
        .any(|row| row.iter().any(|s| s.0 != u16::MAX));
    assert!(has_goto, "nested grammar should have non-trivial gotos");
}

// ============================================================================
// 6. abi_deterministic_* — determinism, reproducibility, ordering (8 tests)
// ============================================================================

#[test]
fn abi_deterministic_same_grammar_same_code() {
    let g = nested_grammar();
    let pt = build_table(&g);
    let code1 = generate_code(&g, &pt);
    let code2 = generate_code(&g, &pt);
    assert_eq!(code1, code2, "generation must be deterministic");
}

#[test]
fn abi_deterministic_single_token_stable() {
    let g = single_token_grammar();
    let pt = build_table(&g);
    let code1 = generate_code(&g, &pt);
    let code2 = generate_code(&g, &pt);
    assert_eq!(code1, code2);
}

#[test]
fn abi_deterministic_alternatives_stable() {
    let g = alternatives_grammar();
    let pt = build_table(&g);
    let code1 = generate_code(&g, &pt);
    let code2 = generate_code(&g, &pt);
    assert_eq!(code1, code2);
}

#[test]
fn abi_deterministic_different_grammars_differ() {
    let g1 = single_token_grammar();
    let pt1 = build_table(&g1);
    let code1 = generate_code(&g1, &pt1);

    let g2 = two_token_grammar();
    let pt2 = build_table(&g2);
    let code2 = generate_code(&g2, &pt2);

    assert_ne!(code1, code2, "different grammars → different code");
}

#[test]
fn abi_deterministic_name_appears_in_output() {
    let g = single_token_grammar();
    let pt = build_table(&g);
    let code = generate_code(&g, &pt);
    assert!(code.contains("single_tok"), "grammar name in output");
}

#[test]
fn abi_deterministic_static_generator_stable() {
    let g = nested_grammar();
    let pt = build_table(&g);
    let generator1 = StaticLanguageGenerator::new(g.clone(), pt.clone());
    let generator2 = StaticLanguageGenerator::new(g, pt);
    let c1 = generator1.generate_language_code().to_string();
    let c2 = generator2.generate_language_code().to_string();
    assert_eq!(c1, c2);
}

#[test]
fn abi_deterministic_token_order_preserved() {
    // Build twice; token ordering should be identical.
    let g1 = alternatives_grammar();
    let g2 = alternatives_grammar();
    let t1: Vec<_> = g1.tokens.keys().copied().collect();
    let t2: Vec<_> = g2.tokens.keys().copied().collect();
    assert_eq!(t1, t2, "token key order must be stable");
}

#[test]
fn abi_deterministic_rule_order_preserved() {
    let g1 = nested_grammar();
    let g2 = nested_grammar();
    let r1: Vec<_> = g1.rules.keys().copied().collect();
    let r2: Vec<_> = g2.rules.keys().copied().collect();
    assert_eq!(r1, r2, "rule key order must be stable");
}

// ============================================================================
// 7. abi_serialize_* — JSON, node-types, token-stream output (8 tests)
// ============================================================================

#[test]
fn abi_serialize_language_json_valid() {
    let g = single_token_grammar();
    let pt = build_table(&g);
    let json =
        adze_tablegen::serializer::serialize_language(&g, &pt, None).expect("serialize_language");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert!(parsed.is_object());
}

#[test]
fn abi_serialize_json_contains_version() {
    let g = single_token_grammar();
    let pt = build_table(&g);
    let json = adze_tablegen::serializer::serialize_language(&g, &pt, None).unwrap();
    assert!(json.contains("\"version\""));
}

#[test]
fn abi_serialize_json_contains_symbol_count() {
    let g = single_token_grammar();
    let pt = build_table(&g);
    let json = adze_tablegen::serializer::serialize_language(&g, &pt, None).unwrap();
    assert!(json.contains("\"symbol_count\""));
}

#[test]
fn abi_serialize_json_contains_state_count() {
    let g = single_token_grammar();
    let pt = build_table(&g);
    let json = adze_tablegen::serializer::serialize_language(&g, &pt, None).unwrap();
    assert!(json.contains("\"state_count\""));
}

#[test]
fn abi_serialize_node_types_valid_json() {
    let g = nested_grammar();
    let pt = build_table(&g);
    let generator = StaticLanguageGenerator::new(g, pt);
    let node_types = generator.generate_node_types();
    assert!(
        node_types.starts_with('['),
        "node_types should be a JSON array, got: {}",
        &node_types[..node_types.len().min(80)]
    );
}

#[test]
fn abi_serialize_node_types_generator() {
    let g = nested_grammar();
    let generator = NodeTypesGenerator::new(&g);
    let result = generator.generate();
    assert!(
        result.is_ok(),
        "NodeTypesGenerator should succeed: {:?}",
        result.err()
    );
}

#[test]
fn abi_serialize_compressed_tables_json() {
    let g = single_token_grammar();
    let pt = build_table(&g);
    let ti = collect_token_indices(&g, &pt);
    let sce = eof_accepts_or_reduces(&pt);
    let compressed = TableCompressor::new().compress(&pt, &ti, sce).unwrap();
    let json = adze_tablegen::serializer::serialize_compressed_tables(&compressed)
        .expect("serialize compressed");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert!(parsed.is_object());
}

#[test]
fn abi_serialize_alternatives_json_valid() {
    let g = alternatives_grammar();
    let pt = build_table(&g);
    let json =
        adze_tablegen::serializer::serialize_language(&g, &pt, None).expect("serialize_language");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert!(parsed["symbol_count"].is_number());
}

// ============================================================================
// 8. abi_edge_* — nullable, extras, external, compressed, many tokens (8 tests)
// ============================================================================

#[test]
fn abi_edge_nullable_grammar_generates() {
    let g = nullable_grammar();
    let pt = build_table(&g);
    let code = generate_code(&g, &pt);
    assert!(!code.is_empty());
}

#[test]
fn abi_edge_extra_token_grammar_generates() {
    let g = grammar_with_extra();
    let pt = build_table(&g);
    let code = generate_code(&g, &pt);
    assert!(!code.is_empty());
}

#[test]
fn abi_edge_external_grammar_generates() {
    let g = grammar_with_external();
    let pt = build_table(&g);
    let code = generate_code(&g, &pt);
    assert!(!code.is_empty());
}

#[test]
fn abi_edge_with_compressed_tables_builder() {
    let g = single_token_grammar();
    let pt = build_table(&g);
    let ti = collect_token_indices(&g, &pt);
    let sce = eof_accepts_or_reduces(&pt);
    let compressed = TableCompressor::new().compress(&pt, &ti, sce).unwrap();
    let code = AbiLanguageBuilder::new(&g, &pt)
        .with_compressed_tables(&compressed)
        .generate()
        .to_string();
    assert!(!code.is_empty());
}

#[test]
fn abi_edge_many_tokens_symbol_count() {
    let mut gb = GrammarBuilder::new("many_tokens");
    let mut names = Vec::new();
    for i in 0..20 {
        let name = format!("tok{i}");
        let pat = format!("p{i}");
        gb = gb.token(&name, &pat);
        names.push(name);
    }
    gb = gb.rule("S", vec![names[0].as_str()]);
    gb = gb.start("S");
    let g = gb.build();
    let pt = build_table(&g);
    // 20 tokens + EOF + at least 1 non-terminal
    assert!(pt.symbol_count >= 21, "got {}", pt.symbol_count);
}

#[test]
fn abi_edge_eof_accepts_or_reduces_check() {
    let g = single_token_grammar();
    let pt = build_table(&g);
    // Just verify it returns a bool without panicking
    let _result: bool = eof_accepts_or_reduces(&pt);
}

#[test]
fn abi_edge_compress_alternatives_succeeds() {
    let g = alternatives_grammar();
    let pt = build_table(&g);
    let ti = collect_token_indices(&g, &pt);
    let sce = eof_accepts_or_reduces(&pt);
    let result = TableCompressor::new().compress(&pt, &ti, sce);
    assert!(result.is_ok(), "compression should succeed");
}

#[test]
fn abi_edge_static_generator_nullable_start() {
    let g = nullable_grammar();
    let pt = build_table(&g);
    let mut generator = StaticLanguageGenerator::new(g, pt);
    generator.set_start_can_be_empty(true);
    let code = generator.generate_language_code().to_string();
    assert!(!code.is_empty());
}

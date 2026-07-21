//! Comprehensive tests for table compression in adze-tablegen.
//!
//! Covers: TableCompressor, CompressedTables, NodeTypesGenerator,
//! StaticLanguageGenerator, encoding, determinism, idempotency, and edge cases.

use adze_glr_core::{Action, FirstFollowSets, ParseTable, build_lr1_automaton};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{Associativity, Grammar, RuleId, StateId};
use adze_tablegen::compress::{
    CompressedActionEntry, CompressedActionTable, CompressedGotoEntry, CompressedGotoTable,
    CompressedParseTable, TableCompressor,
};
use adze_tablegen::node_types::NodeTypesGenerator;
use adze_tablegen::{StaticLanguageGenerator, TableGenError};
use adze_tablegen::{collect_token_indices, eof_accepts_or_reduces};
use std::collections::BTreeMap;

// ============================================================================
// Helper: build grammar → parse table via the full pipeline
// ============================================================================

fn pipeline(mut g: Grammar) -> ParseTable {
    let ff = FirstFollowSets::compute_normalized(&mut g).expect("FIRST/FOLLOW");
    build_lr1_automaton(&g, &ff).expect("LR(1)")
}

fn token_ix(g: &Grammar, pt: &ParseTable) -> Vec<usize> {
    collect_token_indices(g, pt)
}

fn simple_grammar() -> Grammar {
    GrammarBuilder::new("simple")
        .token("a", "a")
        .rule("S", vec!["a"])
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

fn multi_alt_grammar() -> Grammar {
    GrammarBuilder::new("multi_alt")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .rule("S", vec!["A"])
        .rule("S", vec!["B"])
        .rule("S", vec!["C"])
        .rule("A", vec!["a"])
        .rule("B", vec!["b"])
        .rule("C", vec!["c"])
        .start("S")
        .build()
}

fn recursive_grammar() -> Grammar {
    GrammarBuilder::new("recursive")
        .token("a", "a")
        .rule("S", vec!["a"])
        .rule("S", vec!["a", "S"])
        .start("S")
        .build()
}

fn nullable_grammar() -> Grammar {
    GrammarBuilder::new("nullable")
        .token("a", "a")
        .rule("S", vec!["A"])
        .rule("A", vec!["a", "A"])
        .rule("A", vec![])
        .start("S")
        .build()
}

fn precedence_grammar() -> Grammar {
    GrammarBuilder::new("prec")
        .token("NUMBER", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["NUMBER"])
        .start("expr")
        .build()
}

fn chain_grammar() -> Grammar {
    GrammarBuilder::new("chain")
        .token("x", "x")
        .rule("S", vec!["A"])
        .rule("A", vec!["B"])
        .rule("B", vec!["C"])
        .rule("C", vec!["x"])
        .start("S")
        .build()
}

fn multi_token_grammar() -> Grammar {
    GrammarBuilder::new("multi_tok")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .token("d", "d")
        .token("e", "e")
        .rule("S", vec!["a", "b", "c", "d", "e"])
        .start("S")
        .build()
}

// ============================================================================
// 1. TableCompressor construction and basic compression
// ============================================================================

#[test]
fn t01_compressor_default() {
    let c = TableCompressor::default();
    let _ = c;
}

#[test]
fn t02_compressor_new() {
    let c = TableCompressor::new();
    let _ = c;
}

#[test]
fn t03_compress_simple_grammar() {
    let g = simple_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let sce = eof_accepts_or_reduces(&pt);
    let ct = TableCompressor::new().compress(&pt, &ti, sce).unwrap();
    assert!(!ct.action_table.row_offsets.is_empty());
    assert!(!ct.goto_table.row_offsets.is_empty());
}

#[test]
fn t04_compress_two_token() {
    let g = two_token_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let ct = TableCompressor::new().compress(&pt, &ti, false).unwrap();
    assert!(ct.action_table.row_offsets.len() > 1);
}

#[test]
fn t05_compress_multi_alt() {
    let g = multi_alt_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let ct = TableCompressor::new().compress(&pt, &ti, false).unwrap();
    assert!(ct.action_table.default_actions.len() == pt.state_count);
}

#[test]
fn t06_compress_recursive() {
    let g = recursive_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let ct = TableCompressor::new().compress(&pt, &ti, false).unwrap();
    assert!(ct.action_table.row_offsets.len() == pt.state_count + 1);
}

#[test]
fn t07_compress_nullable_start() {
    let g = nullable_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let sce = eof_accepts_or_reduces(&pt);
    let ct = TableCompressor::new().compress(&pt, &ti, sce).unwrap();
    assert!(ct.goto_table.row_offsets.len() > 1);
}

#[test]
fn t08_compress_chain() {
    let g = chain_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let ct = TableCompressor::new().compress(&pt, &ti, false).unwrap();
    assert!(!ct.action_table.data.is_empty());
}

#[test]
fn t09_compress_multi_token() {
    let g = multi_token_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let ct = TableCompressor::new().compress(&pt, &ti, false).unwrap();
    assert!(ct.action_table.row_offsets.len() == pt.state_count + 1);
}

#[test]
fn t10_compress_precedence_grammar() {
    let g = precedence_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let ct = TableCompressor::new().compress(&pt, &ti, false).unwrap();
    assert!(!ct.action_table.data.is_empty());
}

// ============================================================================
// 2. Compression preserves parse table semantics
// ============================================================================

#[test]
fn t11_row_offsets_monotonic_action() {
    let g = multi_alt_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let ct = TableCompressor::new().compress(&pt, &ti, false).unwrap();
    for w in ct.action_table.row_offsets.windows(2) {
        assert!(w[1] >= w[0]);
    }
}

#[test]
fn t12_row_offsets_monotonic_goto() {
    let g = recursive_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let ct = TableCompressor::new().compress(&pt, &ti, false).unwrap();
    for w in ct.goto_table.row_offsets.windows(2) {
        assert!(w[1] >= w[0]);
    }
}

#[test]
fn t13_default_actions_are_error() {
    let g = simple_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let ct = TableCompressor::new().compress(&pt, &ti, false).unwrap();
    for da in &ct.action_table.default_actions {
        assert_eq!(
            *da,
            Action::Error,
            "Default action optimization is disabled"
        );
    }
}

#[test]
fn t14_action_row_offsets_count_equals_states_plus_one() {
    let g = chain_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let ct = TableCompressor::new().compress(&pt, &ti, false).unwrap();
    assert_eq!(ct.action_table.row_offsets.len(), pt.state_count + 1);
}

#[test]
fn t15_goto_row_offsets_count_equals_states_plus_one() {
    let g = chain_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let ct = TableCompressor::new().compress(&pt, &ti, false).unwrap();
    assert_eq!(ct.goto_table.row_offsets.len(), pt.state_count + 1);
}

#[test]
fn t16_all_shift_actions_preserved() {
    let g = simple_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let ct = TableCompressor::new().compress(&pt, &ti, false).unwrap();
    let has_shift = ct
        .action_table
        .data
        .iter()
        .any(|e| matches!(e.action, Action::Shift(_)));
    assert!(has_shift, "Simple grammar must have at least one shift");
}

#[test]
fn t17_all_reduce_actions_preserved() {
    let g = simple_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let ct = TableCompressor::new().compress(&pt, &ti, false).unwrap();
    let has_reduce = ct
        .action_table
        .data
        .iter()
        .any(|e| matches!(e.action, Action::Reduce(_)));
    assert!(has_reduce, "Simple grammar must have at least one reduce");
}

#[test]
fn t18_accept_action_preserved() {
    let g = simple_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let ct = TableCompressor::new().compress(&pt, &ti, false).unwrap();
    let has_accept = ct
        .action_table
        .data
        .iter()
        .any(|e| matches!(e.action, Action::Accept));
    assert!(has_accept, "Grammar must have an accept action");
}

#[test]
fn t19_sentinel_offset_equals_data_len() {
    let g = multi_alt_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let ct = TableCompressor::new().compress(&pt, &ti, false).unwrap();
    let last_offset = *ct.action_table.row_offsets.last().unwrap();
    assert_eq!(last_offset as usize, ct.action_table.data.len());
}

#[test]
fn t20_goto_sentinel_offset_equals_data_len() {
    let g = multi_alt_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let ct = TableCompressor::new().compress(&pt, &ti, false).unwrap();
    let last = *ct.goto_table.row_offsets.last().unwrap();
    assert_eq!(last as usize, ct.goto_table.data.len());
}

// ============================================================================
// 3. Compressed vs uncompressed size comparison
// ============================================================================

fn raw_action_cells(pt: &ParseTable) -> usize {
    pt.action_table.iter().map(|row| row.len()).sum()
}

#[test]
fn t21_compressed_action_not_larger_for_simple() {
    let g = simple_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let ct = TableCompressor::new().compress(&pt, &ti, false).unwrap();
    // compressed entries <= total raw cells (some are empty/error and skipped)
    assert!(ct.action_table.data.len() <= raw_action_cells(&pt));
}

#[test]
fn t22_compressed_action_not_larger_for_multi() {
    let g = multi_alt_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let ct = TableCompressor::new().compress(&pt, &ti, false).unwrap();
    assert!(ct.action_table.data.len() <= raw_action_cells(&pt));
}

#[test]
fn t23_compressed_action_not_larger_for_chain() {
    let g = chain_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let ct = TableCompressor::new().compress(&pt, &ti, false).unwrap();
    assert!(ct.action_table.data.len() <= raw_action_cells(&pt));
}

#[test]
fn t24_compressed_goto_not_larger_for_chain() {
    let g = chain_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let ct = TableCompressor::new().compress(&pt, &ti, false).unwrap();
    let raw_goto_cells: usize = pt.goto_table.iter().map(|row| row.len()).sum();
    assert!(ct.goto_table.data.len() <= raw_goto_cells);
}

#[test]
fn t25_compressed_goto_uses_rle_for_runs() {
    let compressor = TableCompressor::new();
    let goto_table = vec![vec![StateId(1), StateId(1), StateId(1), StateId(1)]];
    let ct = compressor.compress_goto_table_small(&goto_table).unwrap();
    assert!(
        ct.data
            .iter()
            .any(|e| matches!(e, CompressedGotoEntry::RunLength { state: 1, count: 4 }))
    );
}

// ============================================================================
// 4. Compression determinism (same input → same output)
// ============================================================================

#[test]
fn t26_determinism_simple() {
    let g = simple_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let c = TableCompressor::new();
    let a = c.compress(&pt, &ti, false).unwrap();
    let b = c.compress(&pt, &ti, false).unwrap();
    assert_eq!(a.action_table.data.len(), b.action_table.data.len());
    assert_eq!(a.action_table.row_offsets, b.action_table.row_offsets);
    assert_eq!(a.goto_table.row_offsets, b.goto_table.row_offsets);
}

#[test]
fn t27_determinism_multi_alt() {
    let g = multi_alt_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let c = TableCompressor::new();
    let a = c.compress(&pt, &ti, false).unwrap();
    let b = c.compress(&pt, &ti, false).unwrap();
    assert_eq!(a.action_table.row_offsets, b.action_table.row_offsets);
    assert_eq!(a.goto_table.row_offsets, b.goto_table.row_offsets);
}

#[test]
fn t28_determinism_recursive() {
    let g = recursive_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let c = TableCompressor::new();
    let a = c.compress(&pt, &ti, false).unwrap();
    let b = c.compress(&pt, &ti, false).unwrap();
    assert_eq!(a.action_table.data.len(), b.action_table.data.len());
    assert_eq!(a.goto_table.data.len(), b.goto_table.data.len());
}

#[test]
fn t29_determinism_action_entries_match() {
    let g = two_token_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let c = TableCompressor::new();
    let a = c.compress(&pt, &ti, false).unwrap();
    let b = c.compress(&pt, &ti, false).unwrap();
    for (ea, eb) in a.action_table.data.iter().zip(b.action_table.data.iter()) {
        assert_eq!(ea.symbol, eb.symbol);
        assert_eq!(ea.action, eb.action);
    }
}

#[test]
fn t30_determinism_default_actions_match() {
    let g = multi_token_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let c = TableCompressor::new();
    let a = c.compress(&pt, &ti, false).unwrap();
    let b = c.compress(&pt, &ti, false).unwrap();
    assert_eq!(
        a.action_table.default_actions,
        b.action_table.default_actions
    );
}

// ============================================================================
// 5. NodeTypesGenerator produces valid JSON
// ============================================================================

#[test]
fn t31_node_types_simple_grammar() {
    let g = simple_grammar();
    let ntg = NodeTypesGenerator::new(&g);
    let json = ntg.generate().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.is_array());
}

#[test]
fn t32_node_types_multi_alt() {
    let g = multi_alt_grammar();
    let ntg = NodeTypesGenerator::new(&g);
    let json = ntg.generate().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(!parsed.as_array().unwrap().is_empty());
}

#[test]
fn t33_node_types_recursive() {
    let g = recursive_grammar();
    let ntg = NodeTypesGenerator::new(&g);
    let json = ntg.generate().unwrap();
    let _: serde_json::Value = serde_json::from_str(&json).unwrap();
}

#[test]
fn t34_node_types_nullable() {
    let g = nullable_grammar();
    let ntg = NodeTypesGenerator::new(&g);
    let json = ntg.generate().unwrap();
    let _: serde_json::Value = serde_json::from_str(&json).unwrap();
}

#[test]
fn t35_node_types_precedence() {
    let g = precedence_grammar();
    let ntg = NodeTypesGenerator::new(&g);
    let json = ntg.generate().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.is_array());
}

// ============================================================================
// 6. Node types include named/unnamed distinction
// ============================================================================

#[test]
fn t36_node_types_has_named_rules() {
    let g = simple_grammar();
    let ntg = NodeTypesGenerator::new(&g);
    let json = ntg.generate().unwrap();
    let arr: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
    let named_count = arr.iter().filter(|v| v["named"] == true).count();
    assert!(named_count > 0, "Should have at least one named node type");
}

#[test]
fn t37_node_types_unnamed_for_string_tokens() {
    // Tokens like "+" are unnamed
    let g = GrammarBuilder::new("ops")
        .token("NUMBER", r"\d+")
        .token("+", "+")
        .rule("E", vec!["NUMBER", "+", "NUMBER"])
        .start("E")
        .build();
    let ntg = NodeTypesGenerator::new(&g);
    let json = ntg.generate().unwrap();
    let arr: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
    let unnamed = arr.iter().filter(|v| v["named"] == false).count();
    assert!(
        unnamed > 0,
        "String tokens should produce unnamed node types"
    );
}

#[test]
fn t38_node_types_named_for_regex_tokens() {
    let g = GrammarBuilder::new("rx")
        .token("NUMBER", r"\d+")
        .rule("S", vec!["NUMBER"])
        .start("S")
        .build();
    let ntg = NodeTypesGenerator::new(&g);
    let json = ntg.generate().unwrap();
    let arr: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
    let named_types: Vec<_> = arr.iter().filter(|v| v["named"] == true).collect();
    assert!(!named_types.is_empty());
}

#[test]
fn t39_node_types_sorted_by_name() {
    let g = multi_alt_grammar();
    let ntg = NodeTypesGenerator::new(&g);
    let json = ntg.generate().unwrap();
    let arr: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
    let names: Vec<String> = arr
        .iter()
        .map(|v| v["type"].as_str().unwrap().to_string())
        .collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted, "Node types should be sorted alphabetically");
}

#[test]
fn t40_node_types_no_duplicates() {
    let g = multi_alt_grammar();
    let ntg = NodeTypesGenerator::new(&g);
    let json = ntg.generate().unwrap();
    let arr: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
    let names: Vec<String> = arr
        .iter()
        .map(|v| v["type"].as_str().unwrap().to_string())
        .collect();
    let mut deduped = names.clone();
    deduped.dedup();
    assert_eq!(names.len(), deduped.len(), "No duplicate node types");
}

// ============================================================================
// 7. StaticLanguageGenerator with compressed tables
// ============================================================================

#[test]
fn t41_static_lang_gen_creation() {
    let g = simple_grammar();
    let pt = pipeline(g.clone());
    let slg = StaticLanguageGenerator::new(g, pt);
    assert!(slg.compressed_tables.is_none());
}

#[test]
fn t42_static_lang_gen_set_nullable() {
    let g = nullable_grammar();
    let pt = pipeline(g.clone());
    let mut slg = StaticLanguageGenerator::new(g, pt);
    slg.set_start_can_be_empty(true);
    assert!(slg.start_can_be_empty);
}

#[test]
fn t43_static_lang_gen_node_types_json() {
    let g = simple_grammar();
    let pt = pipeline(g.clone());
    let slg = StaticLanguageGenerator::new(g, pt);
    let json = slg.generate_node_types();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.is_array());
}

#[test]
fn t44_static_lang_gen_code_generation() {
    let g = simple_grammar();
    let pt = pipeline(g.clone());
    let slg = StaticLanguageGenerator::new(g, pt);
    let code = slg.generate_language_code();
    let code_str = code.to_string();
    assert!(
        code_str.contains("TSLanguage"),
        "Code must define TSLanguage"
    );
}

#[test]
fn t45_static_lang_gen_multi_grammar() {
    let g = multi_alt_grammar();
    let pt = pipeline(g.clone());
    let slg = StaticLanguageGenerator::new(g, pt);
    let code = slg.generate_language_code();
    assert!(!code.is_empty());
}

#[test]
fn t46_static_lang_gen_node_types_contains_rules() {
    let g = multi_alt_grammar();
    let pt = pipeline(g.clone());
    let slg = StaticLanguageGenerator::new(g, pt);
    let json = slg.generate_node_types();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let arr = parsed.as_array().unwrap();
    assert!(!arr.is_empty(), "Should have node types for grammar rules");
}

#[test]
fn t47_static_lang_gen_with_compressed() {
    let g = simple_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let sce = eof_accepts_or_reduces(&pt);
    let ct = TableCompressor::new().compress(&pt, &ti, sce).unwrap();
    let mut slg = StaticLanguageGenerator::new(g, pt);
    slg.compressed_tables = Some(ct);
    assert!(slg.compressed_tables.is_some());
}

// ============================================================================
// 8. Multiple compression rounds are idempotent
// ============================================================================

#[test]
fn t48_idempotent_action_row_offsets() {
    let g = simple_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let c = TableCompressor::new();
    let first = c.compress(&pt, &ti, false).unwrap();
    let second = c.compress(&pt, &ti, false).unwrap();
    let third = c.compress(&pt, &ti, false).unwrap();
    assert_eq!(
        first.action_table.row_offsets,
        second.action_table.row_offsets
    );
    assert_eq!(
        second.action_table.row_offsets,
        third.action_table.row_offsets
    );
}

#[test]
fn t49_idempotent_goto_row_offsets() {
    let g = recursive_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let c = TableCompressor::new();
    let first = c.compress(&pt, &ti, false).unwrap();
    let second = c.compress(&pt, &ti, false).unwrap();
    assert_eq!(first.goto_table.row_offsets, second.goto_table.row_offsets);
}

#[test]
fn t50_idempotent_action_data_len() {
    let g = chain_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let c = TableCompressor::new();
    let a = c.compress(&pt, &ti, false).unwrap();
    let b = c.compress(&pt, &ti, false).unwrap();
    assert_eq!(a.action_table.data.len(), b.action_table.data.len());
}

#[test]
fn t51_idempotent_goto_data_len() {
    let g = chain_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let c = TableCompressor::new();
    let a = c.compress(&pt, &ti, false).unwrap();
    let b = c.compress(&pt, &ti, false).unwrap();
    assert_eq!(a.goto_table.data.len(), b.goto_table.data.len());
}

#[test]
fn t52_idempotent_default_actions() {
    let g = multi_token_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let c = TableCompressor::new();
    let a = c.compress(&pt, &ti, false).unwrap();
    let b = c.compress(&pt, &ti, false).unwrap();
    assert_eq!(
        a.action_table.default_actions,
        b.action_table.default_actions
    );
}

// ============================================================================
// 9. Edge cases: minimal grammar, single-state table, direct API
// ============================================================================

#[test]
fn t53_compress_action_table_all_empty_cells() {
    let c = TableCompressor::new();
    let at = vec![vec![vec![]; 10]; 5];
    let s2i = BTreeMap::new();
    let res = c.compress_action_table_small(&at, &s2i).unwrap();
    assert!(res.data.is_empty());
    assert_eq!(res.row_offsets.len(), 6);
}

#[test]
fn t54_compress_action_table_single_state() {
    let c = TableCompressor::new();
    let at = vec![vec![vec![Action::Shift(StateId(1))]]];
    let s2i = BTreeMap::new();
    let res = c.compress_action_table_small(&at, &s2i).unwrap();
    assert_eq!(res.data.len(), 1);
    assert_eq!(res.row_offsets.len(), 2);
}

#[test]
fn t55_compress_goto_table_empty() {
    let c = TableCompressor::new();
    let gt: Vec<Vec<StateId>> = vec![];
    let res = c.compress_goto_table_small(&gt).unwrap();
    assert!(res.data.is_empty());
    assert_eq!(res.row_offsets.len(), 1); // just sentinel
}

#[test]
fn t56_compress_goto_table_single_entry() {
    let c = TableCompressor::new();
    let gt = vec![vec![StateId(42)]];
    let res = c.compress_goto_table_small(&gt).unwrap();
    assert_eq!(res.data.len(), 1);
    assert!(matches!(res.data[0], CompressedGotoEntry::Single(42)));
}

#[test]
fn t57_compress_goto_table_all_same_state() {
    let c = TableCompressor::new();
    let gt = vec![vec![StateId(7); 100]];
    let res = c.compress_goto_table_small(&gt).unwrap();
    // Should use RLE for 100 identical entries
    assert!(res.data.iter().any(|e| matches!(
        e,
        CompressedGotoEntry::RunLength {
            state: 7,
            count: 100
        }
    )));
}

#[test]
fn t58_compress_action_table_error_actions_skipped() {
    let c = TableCompressor::new();
    let at = vec![vec![
        vec![Action::Error],
        vec![Action::Error],
        vec![Action::Shift(StateId(1))],
    ]];
    let s2i = BTreeMap::new();
    let res = c.compress_action_table_small(&at, &s2i).unwrap();
    // Only the shift should be encoded
    assert_eq!(res.data.len(), 1);
    assert!(matches!(res.data[0].action, Action::Shift(StateId(1))));
}

#[test]
fn t59_compress_action_table_accept_preserved() {
    let c = TableCompressor::new();
    let at = vec![vec![vec![Action::Accept]]];
    let s2i = BTreeMap::new();
    let res = c.compress_action_table_small(&at, &s2i).unwrap();
    assert!(res.data.iter().any(|e| e.action == Action::Accept));
}

#[test]
fn t60_compress_action_table_fork_flattens_to_leaf_actions() {
    let c = TableCompressor::new();
    let at = vec![vec![vec![Action::Fork(vec![
        Action::Shift(StateId(1)),
        Action::Reduce(RuleId(0)),
    ])]]];
    let s2i = BTreeMap::new();
    let res = c.compress_action_table_small(&at, &s2i).unwrap();
    assert_eq!(res.data.len(), 2);
    assert_eq!(res.data[0].action, Action::Shift(StateId(1)));
    assert_eq!(res.data[1].action, Action::Reduce(RuleId(0)));
}

#[test]
fn t61_compress_goto_short_run_uses_singles() {
    let c = TableCompressor::new();
    // Run of 2 → should use Single, not RunLength
    let gt = vec![vec![StateId(5), StateId(5)]];
    let res = c.compress_goto_table_small(&gt).unwrap();
    assert_eq!(res.data.len(), 2);
    assert!(
        res.data
            .iter()
            .all(|e| matches!(e, CompressedGotoEntry::Single(5)))
    );
}

#[test]
fn t62_compress_goto_boundary_run_of_3() {
    let c = TableCompressor::new();
    // Run of exactly 3 → should use RunLength
    let gt = vec![vec![StateId(9), StateId(9), StateId(9)]];
    let res = c.compress_goto_table_small(&gt).unwrap();
    assert!(
        res.data
            .iter()
            .any(|e| matches!(e, CompressedGotoEntry::RunLength { state: 9, count: 3 }))
    );
}

// ============================================================================
// 10. Error handling in compression pipeline
// ============================================================================

#[test]
fn t63_encode_action_small_shift() {
    let c = TableCompressor::new();
    let v = c.encode_action_small(&Action::Shift(StateId(42))).unwrap();
    assert_eq!(v, 42);
}

#[test]
fn t64_encode_action_small_reduce() {
    let c = TableCompressor::new();
    let v = c.encode_action_small(&Action::Reduce(RuleId(0))).unwrap();
    assert_eq!(v, 0x8000 | 0x0001); // 1-based
}

#[test]
fn t65_encode_action_small_accept() {
    let c = TableCompressor::new();
    let v = c.encode_action_small(&Action::Accept).unwrap();
    assert_eq!(v, 0xFFFF);
}

#[test]
fn t66_encode_action_small_error() {
    let c = TableCompressor::new();
    let v = c.encode_action_small(&Action::Error).unwrap();
    assert_eq!(v, 0xFFFE);
}

#[test]
fn t67_encode_action_small_recover() {
    let c = TableCompressor::new();
    let v = c.encode_action_small(&Action::Recover).unwrap();
    assert_eq!(v, 0xFFFD);
}

#[test]
fn t68_encode_action_small_shift_too_large() {
    let c = TableCompressor::new();
    let r = c.encode_action_small(&Action::Shift(StateId(0x8000)));
    assert!(r.is_err());
}

#[test]
fn t69_encode_action_small_reduce_too_large() {
    let c = TableCompressor::new();
    let r = c.encode_action_small(&Action::Reduce(RuleId(0x4000)));
    assert!(r.is_err());
}

#[test]
fn t70_encode_action_small_shift_max_valid() {
    let c = TableCompressor::new();
    let v = c
        .encode_action_small(&Action::Shift(StateId(0x7FFF)))
        .unwrap();
    assert_eq!(v, 0x7FFF);
}

#[test]
fn t71_encode_action_small_reduce_max_valid() {
    let c = TableCompressor::new();
    let v = c
        .encode_action_small(&Action::Reduce(RuleId(0x3FFF)))
        .unwrap();
    assert_eq!(v, 0x8000 | 0x4000); // 0x3FFF + 1 = 0x4000
}

#[test]
fn t72_encode_action_small_fork_requires_flattening() {
    let c = TableCompressor::new();
    assert!(
        c.encode_action_small(&Action::Fork(vec![Action::Shift(StateId(1))]))
            .is_err()
    );
}

// ============================================================================
// Additional: CompressedParseTable constructors
// ============================================================================

#[test]
fn t73_compressed_parse_table_new_for_testing() {
    let cpt = CompressedParseTable::new_for_testing(100, 200);
    assert_eq!(cpt.symbol_count(), 100);
    assert_eq!(cpt.state_count(), 200);
}

#[test]
fn t74_compressed_parse_table_from_parse_table() {
    let g = simple_grammar();
    let pt = pipeline(g);
    let cpt = CompressedParseTable::from_parse_table(&pt);
    assert_eq!(cpt.symbol_count(), pt.symbol_count);
    assert_eq!(cpt.state_count(), pt.state_count);
}

// ============================================================================
// Additional: validate method on CompressedTables
// ============================================================================

#[test]
fn t75_compressed_tables_validate_ok() {
    let g = simple_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let ct = TableCompressor::new().compress(&pt, &ti, false).unwrap();
    assert!(ct.validate(&pt).is_ok());
}

// ============================================================================
// Additional: eof_accepts_or_reduces helper
// ============================================================================

#[test]
fn t76_eof_helper_simple_not_nullable() {
    let g = simple_grammar();
    let pt = pipeline(g);
    // Simple grammar S -> a is not nullable
    assert!(!eof_accepts_or_reduces(&pt));
}

#[test]
fn t77_eof_helper_nullable_start() {
    let g = nullable_grammar();
    let pt = pipeline(g);
    // Grammar has A -> ε, so start is nullable
    assert!(eof_accepts_or_reduces(&pt));
}

// ============================================================================
// Additional: collect_token_indices helper
// ============================================================================

#[test]
fn t78_token_indices_sorted_and_deduped() {
    let g = multi_alt_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let mut sorted = ti.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(ti, sorted);
}

#[test]
fn t79_token_indices_contain_eof() {
    let g = simple_grammar();
    let pt = pipeline(g.clone());
    let ti = token_ix(&g, &pt);
    let eof_idx = pt.symbol_to_index.get(&pt.eof_symbol).unwrap();
    assert!(ti.contains(eof_idx));
}

// ============================================================================
// Additional: StaticLanguageGenerator node types for various grammars
// ============================================================================

#[test]
fn t80_static_lang_gen_node_types_chain() {
    let g = chain_grammar();
    let pt = pipeline(g.clone());
    let slg = StaticLanguageGenerator::new(g, pt);
    let json = slg.generate_node_types();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.is_array());
}

#[test]
fn t81_static_lang_gen_code_recursive() {
    let g = recursive_grammar();
    let pt = pipeline(g.clone());
    let slg = StaticLanguageGenerator::new(g, pt);
    let code = slg.generate_language_code();
    assert!(!code.is_empty());
}

#[test]
fn t82_static_lang_gen_code_nullable() {
    let g = nullable_grammar();
    let pt = pipeline(g.clone());
    let mut slg = StaticLanguageGenerator::new(g, pt);
    slg.set_start_can_be_empty(true);
    let code = slg.generate_language_code();
    let s = code.to_string();
    assert!(s.contains("TSLanguage"));
}

// ============================================================================
// Additional: TableGenError variants
// ============================================================================

#[test]
fn t83_error_compression_display() {
    let e = TableGenError::Compression("test msg".to_string());
    let s = format!("{}", e);
    assert!(s.contains("test msg"));
}

#[test]
fn t84_error_invalid_table_display() {
    let e = TableGenError::InvalidTable("bad table".to_string());
    let s = format!("{}", e);
    assert!(s.contains("bad table"));
}

#[test]
fn t85_error_from_string() {
    let e: TableGenError = "something failed".into();
    let s = format!("{}", e);
    assert!(s.contains("something failed"));
}

#[test]
fn t86_error_empty_grammar() {
    let e = TableGenError::EmptyGrammar;
    let s = format!("{}", e);
    assert!(s.contains("empty grammar"));
}

// ============================================================================
// Additional: entry constructors and compression on diverse inputs
// ============================================================================

#[test]
fn t87_compressed_action_entry_new() {
    let e = CompressedActionEntry::new(0, Action::Reduce(RuleId(42)));
    assert_eq!(e.symbol, 0);
    assert_eq!(e.action, Action::Reduce(RuleId(42)));
}

#[test]
fn t88_goto_entry_single_debug() {
    let e = CompressedGotoEntry::Single(999);
    let dbg = format!("{:?}", e);
    assert!(dbg.contains("999"));
}

#[test]
fn t89_goto_entry_runlength_debug() {
    let e = CompressedGotoEntry::RunLength { state: 3, count: 7 };
    let dbg = format!("{:?}", e);
    assert!(dbg.contains("3"));
    assert!(dbg.contains("7"));
}

#[test]
fn t90_action_table_clone() {
    let at = CompressedActionTable {
        data: vec![CompressedActionEntry::new(1, Action::Accept)],
        row_offsets: vec![0, 1],
        default_actions: vec![Action::Error],
    };
    let at2 = at.clone();
    assert_eq!(at2.data.len(), 1);
    assert_eq!(at2.row_offsets, vec![0, 1]);
}

#[test]
fn t91_goto_table_clone() {
    let gt = CompressedGotoTable {
        data: vec![CompressedGotoEntry::Single(5)],
        row_offsets: vec![0, 1],
    };
    let gt2 = gt.clone();
    assert_eq!(gt2.data.len(), 1);
}

#[test]
fn t92_action_table_debug() {
    let at = CompressedActionTable {
        data: vec![],
        row_offsets: vec![0],
        default_actions: vec![],
    };
    let dbg = format!("{:?}", at);
    assert!(dbg.contains("CompressedActionTable"));
}

#[test]
fn t93_goto_table_debug() {
    let gt = CompressedGotoTable {
        data: vec![],
        row_offsets: vec![0],
    };
    let dbg = format!("{:?}", gt);
    assert!(dbg.contains("CompressedGotoTable"));
}

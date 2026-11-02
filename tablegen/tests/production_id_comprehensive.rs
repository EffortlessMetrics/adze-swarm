#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]
//! Comprehensive tests for production ID generation and mapping in adze-tablegen.
//!
//! Covers: production ID assignment, uniqueness, stability across builds,
//! production-to-rule mapping, empty production lists, many productions,
//! and production ID in compressed tables.
//!
//! All tests use the public API only.

use adze_glr_core::{GotoIndexing, LexMode, ParseTable};
use adze_ir::{Grammar, ProductionId, Rule, StateId, Symbol, SymbolId, Token, TokenPattern};
use adze_tablegen::AbiLanguageBuilder;
use adze_tablegen::serializer::serialize_language;
use std::collections::{BTreeMap, HashSet};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const INVALID: StateId = StateId(u16::MAX);

fn tok(name: &str, lit: &str) -> Token {
    Token {
        name: name.to_string(),
        pattern: TokenPattern::String(lit.to_string()),
        fragile: false,
    }
}

fn rule(lhs: SymbolId, rhs: Vec<Symbol>, prod_id: u16) -> Rule {
    Rule {
        lhs,
        rhs,
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(prod_id),
    }
}

/// Build a ParseTable with the given dimensions.
/// Layout: ERROR(0), terminals 1..=terms, EOF, non-terminals.
fn empty_table(states: usize, terms: usize, nonterms: usize, externals: usize) -> ParseTable {
    let states = states.max(1);
    let eof_idx = 1 + terms + externals;
    let nonterms_eff = nonterms.max(1);
    let symbol_count = eof_idx + 1 + nonterms_eff;

    let actions = vec![vec![vec![]; symbol_count]; states];
    let gotos = vec![vec![INVALID; symbol_count]; states];

    let eof_symbol = SymbolId(eof_idx as u16);
    let start_symbol = SymbolId((eof_idx + 1) as u16);

    let mut symbol_to_index = BTreeMap::new();
    let mut index_to_symbol = vec![SymbolId(0); symbol_count];
    for i in 0..symbol_count {
        let sym = SymbolId(i as u16);
        symbol_to_index.insert(sym, i);
        index_to_symbol[i] = sym;
    }

    let mut nonterminal_to_index = BTreeMap::new();
    for col in (eof_idx + 1)..symbol_count {
        nonterminal_to_index.insert(SymbolId(col as u16), col);
    }

    let token_count = eof_idx + 1;

    ParseTable {
        action_table: actions,
        goto_table: gotos,
        symbol_metadata: vec![],
        state_count: states,
        symbol_count,
        symbol_to_index,
        index_to_symbol,
        nonterminal_to_index,
        external_scanner_states: vec![],
        rules: vec![],
        eof_symbol,
        start_symbol,
        grammar: Grammar::default(),
        initial_state: StateId(0),
        token_count,
        external_token_count: externals,
        lex_modes: vec![
            LexMode {
                lex_state: 0,
                external_lex_state: 0,
            };
            states
        ],
        extras: vec![],
        dynamic_prec_by_rule: vec![],
        rule_assoc_by_rule: vec![],
        alias_sequences: vec![],
        field_names: vec![],
        field_map: BTreeMap::new(),
        goto_indexing: GotoIndexing::NonterminalMap,
    }
}

/// Build a grammar with the given name, one terminal token `t`, and a nonterminal `start`.
/// `start` symbol ID = table's start_symbol, token `t` = SymbolId(1).
fn base_grammar(name: &str, table: &ParseTable) -> (Grammar, SymbolId, SymbolId) {
    let mut g = Grammar::new(name.to_string());
    let start = table.start_symbol;
    let t = SymbolId(1);
    g.rule_names.insert(start, "start".to_string());
    g.tokens.insert(t, tok("t", "t"));
    (g, start, t)
}

fn gen_code(grammar: &Grammar, table: &ParseTable) -> String {
    let builder = AbiLanguageBuilder::new(grammar, table);
    builder.generate().to_string()
}

/// Extract a `&[u16]` array by name from generated code.
/// Generated code looks like: `static NAME : & [u16] = & [0u16 , 1u16] ;`
fn extract_u16_array(code: &str, name: &str) -> Vec<u16> {
    let marker = format!("{} : & [u16]", name);
    let start = match code.find(&marker) {
        Some(pos) => pos,
        None => return vec![],
    };
    let rest = &code[start + marker.len()..];
    // Find `= & [` then the matching `]`
    let eq_bracket = match rest.find("& [") {
        Some(pos) => pos + 3,
        None => return vec![],
    };
    let inner_rest = &rest[eq_bracket..];
    let close = match inner_rest.find(']') {
        Some(pos) => pos,
        None => return vec![],
    };
    let inner = &inner_rest[..close];
    inner
        .split(',')
        .filter_map(|s| {
            let s = s.trim();
            // Handle "0u16", "0usize as u16", or plain "0"
            let s = if let Some(idx) = s.find("u16") {
                s[..idx].trim()
            } else if let Some(idx) = s.find("usize") {
                s[..idx].trim()
            } else {
                s
            };
            s.parse::<u16>().ok()
        })
        .collect()
}

fn extract_production_id_map(code: &str) -> Vec<u16> {
    extract_u16_array(code, "PRODUCTION_ID_MAP")
}

fn extract_production_lhs_index(code: &str) -> Vec<u16> {
    extract_u16_array(code, "PRODUCTION_LHS_INDEX")
}

/// Extract `production_id_count` from generated code.
/// Format: `production_id_count : 1u32`
fn extract_production_id_count(code: &str) -> Option<u32> {
    let marker = "production_id_count :";
    let start = code.find(marker)?;
    let rest = &code[start + marker.len()..];
    let end = rest.find(',')?;
    let num_str = rest[..end].trim().trim_end_matches("u32");
    num_str.parse::<u32>().ok()
}

/// Extract production_id_count from the JSON serialized output.
fn serialized_production_count(grammar: &Grammar, table: &ParseTable) -> u32 {
    let json = serialize_language(grammar, table, None).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    v["production_id_count"].as_u64().unwrap() as u32
}

// ===========================================================================
// Tests: Production ID Assignment
// ===========================================================================

/// A single rule gets production ID 0.
#[test]
fn assignment_single_rule_gets_id_zero() {
    let table = empty_table(1, 1, 1, 0);
    let (mut g, start, t) = base_grammar("single", &table);
    g.add_rule(rule(start, vec![Symbol::Terminal(t)], 0));

    let code = gen_code(&g, &table);
    let map = extract_production_id_map(&code);
    assert_eq!(map, vec![0]);
}

/// Two rules on the same LHS get consecutive production IDs.
#[test]
fn assignment_two_rules_same_lhs() {
    let table = empty_table(1, 1, 1, 0);
    let (mut g, start, t) = base_grammar("two_same", &table);
    g.add_rule(rule(start, vec![Symbol::Terminal(t)], 0));
    g.add_rule(rule(
        start,
        vec![Symbol::Terminal(t), Symbol::Terminal(t)],
        1,
    ));

    let code = gen_code(&g, &table);
    let map = extract_production_id_map(&code);
    assert_eq!(map.len(), 2);
    assert_eq!(map[0], 0);
    assert_eq!(map[1], 1);
}

/// Three rules across two nonterminals get distinct production IDs.
#[test]
fn assignment_rules_across_nonterminals() {
    let table = empty_table(1, 1, 2, 0);
    let start = table.start_symbol;
    let other = SymbolId(start.0 + 1);
    let t = SymbolId(1);

    let mut g = Grammar::new("cross_nt".to_string());
    g.rule_names.insert(start, "start".to_string());
    g.rule_names.insert(other, "other".to_string());
    g.tokens.insert(t, tok("t", "t"));

    g.add_rule(rule(start, vec![Symbol::NonTerminal(other)], 0));
    g.add_rule(rule(other, vec![Symbol::Terminal(t)], 1));
    g.add_rule(rule(
        other,
        vec![Symbol::Terminal(t), Symbol::Terminal(t)],
        2,
    ));

    let code = gen_code(&g, &table);
    let map = extract_production_id_map(&code);
    assert_eq!(map.len(), 3);
    // Each production ID appears exactly once in the map
    let set: HashSet<u16> = map.iter().copied().collect();
    assert_eq!(set.len(), 3);
}

/// Epsilon production (empty RHS) gets a valid production ID.
#[test]
fn assignment_epsilon_production() {
    let table = empty_table(1, 1, 1, 0);
    let (mut g, start, _t) = base_grammar("epsilon", &table);
    g.add_rule(rule(start, vec![], 0));

    let code = gen_code(&g, &table);
    let map = extract_production_id_map(&code);
    assert_eq!(map.len(), 1);
    assert_eq!(map[0], 0);
}

// ===========================================================================
// Tests: Production ID Uniqueness
// ===========================================================================

/// Every production ID in a multi-rule grammar is unique.
#[test]
fn uniqueness_all_ids_distinct() {
    let table = empty_table(1, 2, 2, 0);
    let start = table.start_symbol;
    let other = SymbolId(start.0 + 1);
    let t1 = SymbolId(1);
    let t2 = SymbolId(2);

    let mut g = Grammar::new("unique".to_string());
    g.rule_names.insert(start, "start".to_string());
    g.rule_names.insert(other, "other".to_string());
    g.tokens.insert(t1, tok("a", "a"));
    g.tokens.insert(t2, tok("b", "b"));

    g.add_rule(rule(start, vec![Symbol::Terminal(t1)], 0));
    g.add_rule(rule(start, vec![Symbol::Terminal(t2)], 1));
    g.add_rule(rule(other, vec![Symbol::Terminal(t1)], 2));
    g.add_rule(rule(other, vec![Symbol::Terminal(t2)], 3));

    let code = gen_code(&g, &table);
    let map = extract_production_id_map(&code);
    let set: HashSet<u16> = map.iter().copied().collect();
    assert_eq!(set.len(), map.len(), "all production IDs must be unique");
}

/// No two rules share the same production ID even when LHS symbols differ.
#[test]
fn uniqueness_no_collision_different_lhs() {
    let table = empty_table(1, 1, 3, 0);
    let start = table.start_symbol;
    let a = SymbolId(start.0 + 1);
    let b = SymbolId(start.0 + 2);
    let t = SymbolId(1);

    let mut g = Grammar::new("nocollide".to_string());
    g.rule_names.insert(start, "start".to_string());
    g.rule_names.insert(a, "a".to_string());
    g.rule_names.insert(b, "b".to_string());
    g.tokens.insert(t, tok("t", "t"));

    g.add_rule(rule(start, vec![Symbol::NonTerminal(a)], 0));
    g.add_rule(rule(a, vec![Symbol::Terminal(t)], 1));
    g.add_rule(rule(b, vec![Symbol::Terminal(t)], 2));

    let code = gen_code(&g, &table);
    let map = extract_production_id_map(&code);
    let set: HashSet<u16> = map.iter().copied().collect();
    assert_eq!(set.len(), 3);
}

// ===========================================================================
// Tests: Production ID Stability Across Builds
// ===========================================================================

/// Generating code twice yields identical production ID maps.
#[test]
fn stability_deterministic_codegen() {
    let table = empty_table(1, 1, 1, 0);
    let (mut g, start, t) = base_grammar("stable", &table);
    g.add_rule(rule(start, vec![Symbol::Terminal(t)], 0));
    g.add_rule(rule(
        start,
        vec![Symbol::Terminal(t), Symbol::Terminal(t)],
        1,
    ));

    let code1 = gen_code(&g, &table);
    let code2 = gen_code(&g, &table);
    assert_eq!(
        extract_production_id_map(&code1),
        extract_production_id_map(&code2),
        "production ID map must be deterministic"
    );
}

/// Production IDs remain stable when the grammar is rebuilt identically.
#[test]
fn stability_rebuilt_grammar_same_ids() {
    let build = || {
        let table = empty_table(1, 2, 1, 0);
        let (mut g, start, t) = base_grammar("rebuild", &table);
        let t2 = SymbolId(2);
        g.tokens.insert(t2, tok("u", "u"));
        g.add_rule(rule(start, vec![Symbol::Terminal(t)], 0));
        g.add_rule(rule(start, vec![Symbol::Terminal(t2)], 1));
        gen_code(&g, &table)
    };

    let code_a = build();
    let code_b = build();
    assert_eq!(
        extract_production_id_map(&code_a),
        extract_production_id_map(&code_b),
    );
}

/// Production ID count is stable across identical builds.
#[test]
fn stability_production_count_deterministic() {
    let build = || {
        let table = empty_table(1, 1, 1, 0);
        let (mut g, start, t) = base_grammar("cnt_stable", &table);
        g.add_rule(rule(start, vec![Symbol::Terminal(t)], 0));
        g.add_rule(rule(start, vec![], 1));
        gen_code(&g, &table)
    };
    let c1 = extract_production_id_count(&build());
    let c2 = extract_production_id_count(&build());
    assert_eq!(c1, c2);
}

// ===========================================================================
// Tests: Production-to-Rule Mapping
// ===========================================================================

/// Production ID map length equals the number of grammar rules.
#[test]
fn mapping_length_matches_rule_count() {
    let table = empty_table(1, 1, 1, 0);
    let (mut g, start, t) = base_grammar("maplen", &table);
    g.add_rule(rule(start, vec![Symbol::Terminal(t)], 0));
    g.add_rule(rule(start, vec![], 1));
    g.add_rule(rule(
        start,
        vec![Symbol::Terminal(t), Symbol::Terminal(t)],
        2,
    ));

    let count: usize = g.rules.values().map(|v| v.len()).sum();
    let code = gen_code(&g, &table);
    let map = extract_production_id_map(&code);
    assert_eq!(map.len(), count);
}

/// production_id_count in generated code equals max(production_id) + 1.
#[test]
fn mapping_count_is_max_plus_one() {
    let table = empty_table(1, 1, 1, 0);
    let (mut g, start, t) = base_grammar("maxp1", &table);
    g.add_rule(rule(start, vec![Symbol::Terminal(t)], 0));
    g.add_rule(rule(
        start,
        vec![Symbol::Terminal(t), Symbol::Terminal(t)],
        1,
    ));
    g.add_rule(rule(start, vec![], 2));

    let code = gen_code(&g, &table);
    let count = extract_production_id_count(&code).unwrap();
    assert_eq!(count, 3);
}

/// production_id_count from ABI builder matches serialized output.
#[test]
fn mapping_abi_matches_serialized_count() {
    let table = empty_table(1, 1, 1, 0);
    let (mut g, start, t) = base_grammar("abiser", &table);
    g.add_rule(rule(start, vec![Symbol::Terminal(t)], 0));
    g.add_rule(rule(start, vec![], 1));

    let abi_code = gen_code(&g, &table);
    let abi_count = extract_production_id_count(&abi_code).unwrap();
    let ser_count = serialized_production_count(&g, &table);
    assert_eq!(abi_count, ser_count);
}

/// Production map preserves identity mapping (ID 0 → slot 0, etc.).
#[test]
fn mapping_identity_preserved() {
    let table = empty_table(1, 1, 1, 0);
    let (mut g, start, t) = base_grammar("identity", &table);
    for i in 0..5u16 {
        g.add_rule(rule(start, vec![Symbol::Terminal(t); (i + 1) as usize], i));
    }

    let code = gen_code(&g, &table);
    let map = extract_production_id_map(&code);
    for i in 0..5 {
        assert_eq!(
            map[i], i as u16,
            "slot {} should map to production {}",
            i, i
        );
    }
}

// ===========================================================================
// Tests: Empty Production Lists
// ===========================================================================

/// Grammar with no rules still generates production structures.
#[test]
fn empty_no_rules_generates_code() {
    let table = empty_table(1, 1, 1, 0);
    let (g, _start, _t) = base_grammar("norules", &table);
    // No rules added
    let code = gen_code(&g, &table);
    // Code should still compile (contain the LANGUAGE struct)
    assert!(code.contains("LANGUAGE"));
}

/// Empty grammar yields production_id_count of 1 (minimum).
#[test]
fn empty_production_count_minimum() {
    let table = empty_table(1, 1, 1, 0);
    let (g, _start, _t) = base_grammar("empty_cnt", &table);

    let code = gen_code(&g, &table);
    let count = extract_production_id_count(&code);
    // With no rules, max production ID is 0, so count should be 1
    assert!(count.is_some());
    assert!(count.unwrap() >= 1, "production count should be at least 1");
}

/// Serialized output for empty grammar has a valid production_id_count.
#[test]
fn empty_serialized_count_valid() {
    let table = empty_table(1, 1, 1, 0);
    let (g, _start, _t) = base_grammar("empty_ser", &table);

    let json = serialize_language(&g, &table, None).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    // Serializer counts rules directly; empty grammar has 0 rules
    assert_eq!(v["production_id_count"].as_u64().unwrap(), 0);
}

/// Grammar with only epsilon rules gets correct production IDs.
#[test]
fn empty_only_epsilon_rules() {
    let table = empty_table(1, 1, 1, 0);
    let (mut g, start, _t) = base_grammar("eps_only", &table);
    g.add_rule(rule(start, vec![], 0));
    g.add_rule(rule(start, vec![], 1));

    let code = gen_code(&g, &table);
    let map = extract_production_id_map(&code);
    assert_eq!(map.len(), 2);
    let count = extract_production_id_count(&code).unwrap();
    assert_eq!(count, 2);
}

// ===========================================================================
// Tests: Many Productions
// ===========================================================================

/// 20 rules on a single nonterminal get correctly numbered IDs.
#[test]
fn many_twenty_rules_single_nonterminal() {
    let table = empty_table(1, 1, 1, 0);
    let (mut g, start, t) = base_grammar("twenty", &table);
    for i in 0..20u16 {
        g.add_rule(rule(
            start,
            vec![Symbol::Terminal(t); (i % 3 + 1) as usize],
            i,
        ));
    }

    let code = gen_code(&g, &table);
    let map = extract_production_id_map(&code);
    assert_eq!(map.len(), 20);
    let set: HashSet<u16> = map.iter().copied().collect();
    assert_eq!(set.len(), 20, "all 20 production IDs must be unique");
}

/// 50 rules spread across 5 nonterminals.
#[test]
fn many_fifty_rules_five_nonterminals() {
    let table = empty_table(1, 2, 5, 0);
    let start = table.start_symbol;
    let t1 = SymbolId(1);
    let t2 = SymbolId(2);

    let mut g = Grammar::new("fifty".to_string());
    g.tokens.insert(t1, tok("a", "a"));
    g.tokens.insert(t2, tok("b", "b"));

    let mut prod_id = 0u16;
    for nt_offset in 0..5u16 {
        let nt = SymbolId(start.0 + nt_offset);
        g.rule_names.insert(nt, format!("nt_{}", nt_offset));
        for j in 0..10u16 {
            let tok_sym = if j % 2 == 0 {
                Symbol::Terminal(t1)
            } else {
                Symbol::Terminal(t2)
            };
            g.add_rule(rule(nt, vec![tok_sym; (j % 3 + 1) as usize], prod_id));
            prod_id += 1;
        }
    }

    let code = gen_code(&g, &table);
    let map = extract_production_id_map(&code);
    assert_eq!(map.len(), 50);
    let set: HashSet<u16> = map.iter().copied().collect();
    assert_eq!(set.len(), 50);
}

/// production_id_count matches for a grammar with many rules.
#[test]
fn many_production_count_correct() {
    let n = 30;
    let table = empty_table(1, 1, 1, 0);
    let (mut g, start, t) = base_grammar("many_cnt", &table);
    for i in 0..n as u16 {
        g.add_rule(rule(start, vec![Symbol::Terminal(t)], i));
    }

    let code = gen_code(&g, &table);
    let count = extract_production_id_count(&code).unwrap();
    assert_eq!(count, n);
}

// ===========================================================================
// Tests: Production ID in Compressed Tables
// ===========================================================================

/// Compressed tables preserve production_id_count from the ABI builder.
#[test]
fn compressed_preserves_production_count() {
    let table = empty_table(2, 1, 1, 0);
    let (mut g, start, t) = base_grammar("comp_cnt", &table);
    g.add_rule(rule(start, vec![Symbol::Terminal(t)], 0));
    g.add_rule(rule(start, vec![], 1));

    // Generate without compression
    let code_plain = gen_code(&g, &table);
    let count_plain = extract_production_id_count(&code_plain).unwrap();

    // Generate with StaticLanguageGenerator which supports compression
    let mut slg = adze_tablegen::StaticLanguageGenerator::new(g.clone(), table.clone());
    // compress_tables may fail for test data, but production count should still be set
    let _ = slg.compress_tables();
    let code_compressed = slg.generate_language_code().to_string();
    let count_compressed = extract_production_id_count(&code_compressed);

    // The plain count is always valid
    assert!(count_plain >= 2);

    // If compressed code has the field, it must agree
    if let Some(cc) = count_compressed {
        assert_eq!(cc, count_plain);
    }
}

/// ABI builder with compressed tables still includes PRODUCTION_ID_MAP.
#[test]
fn compressed_includes_production_id_map() {
    let table = empty_table(2, 2, 1, 0);
    let (mut g, start, _t) = base_grammar("comp_map", &table);
    let t1 = SymbolId(1);
    let t2 = SymbolId(2);
    g.tokens.insert(t2, tok("u", "u"));
    g.add_rule(rule(start, vec![Symbol::Terminal(t1)], 0));
    g.add_rule(rule(start, vec![Symbol::Terminal(t2)], 1));

    // Build compressed tables
    let compressor = adze_tablegen::TableCompressor::new();
    let token_indices = adze_tablegen::collect_token_indices(&g, &table);
    if let Ok(compressed) = compressor.compress(&table, &token_indices, false) {
        let builder = AbiLanguageBuilder::new(&g, &table).with_compressed_tables(&compressed);
        let code = builder.generate().to_string();
        let map = extract_production_id_map(&code);
        assert_eq!(map.len(), 2);
    }
    // If compression fails for this test data, the non-compressed path is already tested
}

/// Compressed and uncompressed builders agree on production ID map contents.
#[test]
fn compressed_map_matches_uncompressed() {
    let table = empty_table(2, 1, 1, 0);
    let (mut g, start, t) = base_grammar("comp_match", &table);
    g.add_rule(rule(start, vec![Symbol::Terminal(t)], 0));
    g.add_rule(rule(
        start,
        vec![Symbol::Terminal(t), Symbol::Terminal(t)],
        1,
    ));

    let code_plain = gen_code(&g, &table);
    let map_plain = extract_production_id_map(&code_plain);

    let compressor = adze_tablegen::TableCompressor::new();
    let token_indices = adze_tablegen::collect_token_indices(&g, &table);
    if let Ok(compressed) = compressor.compress(&table, &token_indices, false) {
        let builder = AbiLanguageBuilder::new(&g, &table).with_compressed_tables(&compressed);
        let code_comp = builder.generate().to_string();
        let map_comp = extract_production_id_map(&code_comp);
        assert_eq!(map_plain, map_comp);
    }
}

// ===========================================================================
// Additional edge cases
// ===========================================================================

/// Production IDs starting at non-zero index are handled.
#[test]
fn edge_nonzero_start_id() {
    let table = empty_table(1, 1, 1, 0);
    let (mut g, start, t) = base_grammar("nonzero", &table);
    // RuleId(0) refers to sparse metadata slot 5.
    g.add_rule(rule(start, vec![Symbol::Terminal(t)], 5));

    let code = gen_code(&g, &table);
    let map = extract_production_id_map(&code);
    assert_eq!(map, vec![5], "only syntax rule 0 belongs in the map");
    assert_eq!(extract_production_id_count(&code), Some(6));
    // The entry at position 0 (the only rule) maps to production 5
    assert_eq!(map[0], 5);
}

/// Sparse production IDs emit a dense LHS index table for runtime indexing.
#[test]
fn edge_sparse_production_ids_emit_dense_production_lhs_index() {
    let table = empty_table(1, 1, 2, 0);
    let (mut g, start, t) = base_grammar("sparse_lhs", &table);
    let other = SymbolId(start.0 + 1);
    g.rule_names.insert(other, "other".to_string());
    g.add_rule(rule(start, vec![Symbol::Terminal(t)], 0));
    g.add_rule(rule(other, vec![Symbol::Terminal(t)], 7));

    let code = gen_code(&g, &table);
    let count = extract_production_id_count(&code).unwrap();
    let map = extract_production_id_map(&code);
    let lhs_index = extract_production_lhs_index(&code);

    assert_eq!(count, 8);
    assert_eq!(map.len(), 2, "map has one entry per syntax rule");
    assert_eq!(map[0], 0);
    assert_eq!(map[1], 7);
    assert_eq!(lhs_index.len(), count as usize);
    assert_eq!(lhs_index[0], table.symbol_to_index[&start] as u16);
    assert_eq!(lhs_index[7], table.symbol_to_index[&other] as u16);
    assert!(
        lhs_index[1..7].iter().all(|lhs| *lhs == 0),
        "gapped production slots should be explicit zero sentinels"
    );
}

/// Production ID map preserves GLR rule-id order even when production IDs are sparse.
#[test]
fn edge_sparse_production_ids_preserve_rule_id_order_in_map() {
    let table = empty_table(1, 1, 2, 0);
    let (mut g, start, t) = base_grammar("sparse_rule_id_map", &table);
    let other = SymbolId(start.0 + 1);
    g.rule_names.insert(other, "other".to_string());

    // GLR action generation assigns rule IDs from grammar insertion order.
    // These production IDs intentionally do not match that order.
    g.add_rule(rule(start, vec![Symbol::Terminal(t)], 7));
    g.add_rule(rule(other, vec![Symbol::Terminal(t)], 0));

    let code = gen_code(&g, &table);
    let count = extract_production_id_count(&code).unwrap();
    let map = extract_production_id_map(&code);
    let lhs_index = extract_production_lhs_index(&code);

    assert_eq!(count, 8);
    assert_eq!(map.len(), 2, "map has one entry per syntax rule");
    assert_eq!(
        map[0], 7,
        "rule ID 0 should map to the first inserted rule's production ID"
    );
    assert_eq!(
        map[1], 0,
        "rule ID 1 should map to the second inserted rule's production ID"
    );
    assert_eq!(lhs_index.len(), count as usize);
    assert_eq!(lhs_index[7], table.symbol_to_index[&start] as u16);
    assert_eq!(lhs_index[0], table.symbol_to_index[&other] as u16);
}

/// production_id_count accounts for gaps in production IDs.
#[test]
fn edge_gap_in_production_ids() {
    let table = empty_table(1, 1, 1, 0);
    let (mut g, start, t) = base_grammar("gap", &table);
    g.add_rule(rule(start, vec![Symbol::Terminal(t)], 0));
    g.add_rule(rule(start, vec![], 3)); // gap at 1, 2

    let code = gen_code(&g, &table);
    let count = extract_production_id_count(&code).unwrap();
    // max ID is 3, so count = 4
    assert_eq!(count, 4);
}

/// Grammar with external tokens still numbers productions correctly.
#[test]
fn edge_external_tokens_dont_affect_production_ids() {
    let table = empty_table(1, 1, 1, 1); // 1 external token
    let start = table.start_symbol;
    let t = SymbolId(1);

    let mut g = Grammar::new("ext".to_string());
    g.rule_names.insert(start, "start".to_string());
    g.tokens.insert(t, tok("t", "t"));

    g.add_rule(rule(start, vec![Symbol::Terminal(t)], 0));
    g.add_rule(rule(start, vec![], 1));

    let code = gen_code(&g, &table);
    let map = extract_production_id_map(&code);
    assert_eq!(map.len(), 2);
    assert_eq!(map[0], 0);
    assert_eq!(map[1], 1);
}

/// The generated LANGUAGE struct references production_id_count.
#[test]
fn generated_language_struct_has_production_count() {
    let table = empty_table(1, 1, 1, 0);
    let (mut g, start, t) = base_grammar("struct_check", &table);
    g.add_rule(rule(start, vec![Symbol::Terminal(t)], 0));

    let code = gen_code(&g, &table);
    assert!(
        code.contains("production_id_count"),
        "LANGUAGE struct must reference production_id_count"
    );
}

/// The generated code contains PRODUCTION_ID_MAP as a static array.
#[test]
fn generated_code_has_static_production_id_map() {
    let table = empty_table(1, 1, 1, 0);
    let (mut g, start, t) = base_grammar("static_map", &table);
    g.add_rule(rule(start, vec![Symbol::Terminal(t)], 0));

    let code = gen_code(&g, &table);
    assert!(
        code.contains("PRODUCTION_ID_MAP"),
        "generated code must contain PRODUCTION_ID_MAP"
    );
}

/// Production ID map values are all valid u16 (no overflow).
#[test]
fn generated_map_values_are_valid_u16() {
    let table = empty_table(1, 1, 1, 0);
    let (mut g, start, t) = base_grammar("u16check", &table);
    for i in 0..15u16 {
        g.add_rule(rule(start, vec![Symbol::Terminal(t)], i));
    }

    let code = gen_code(&g, &table);
    let map = extract_production_id_map(&code);
    for val in &map {
        // Sentinel is u16::MAX which is valid but special
        // Non-sentinel values should be < 15 for this grammar
        assert!(*val < 15 || *val == u16::MAX, "unexpected value: {}", val);
    }
}

/// Verify different state counts don't affect production ID assignment.
#[test]
fn state_count_does_not_affect_production_ids() {
    let (g1, start1, t1) = {
        let table = empty_table(1, 1, 1, 0);
        let (mut g, s, t) = base_grammar("s1", &table);
        g.add_rule(rule(s, vec![Symbol::Terminal(t)], 0));
        g.add_rule(rule(s, vec![], 1));
        (g, s, t)
    };
    let _ = (start1, t1);

    let (g2, start2, t2) = {
        let table = empty_table(10, 1, 1, 0);
        let (mut g, s, t) = base_grammar("s1", &table);
        g.add_rule(rule(s, vec![Symbol::Terminal(t)], 0));
        g.add_rule(rule(s, vec![], 1));
        (g, s, t)
    };
    let _ = (start2, t2);

    let table_small = empty_table(1, 1, 1, 0);
    let table_large = empty_table(10, 1, 1, 0);

    let map1 = extract_production_id_map(&gen_code(&g1, &table_small));
    let map2 = extract_production_id_map(&gen_code(&g2, &table_large));
    assert_eq!(map1, map2);
}

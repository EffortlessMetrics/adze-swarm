#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]
//! Comprehensive tests for production mapping (PRODUCTION_ID_MAP, PRODUCTION_LHS_INDEX,
//! TS_RULES, production_id_count) in the tablegen crate.
//!
//! All tests use only the public API: `AbiLanguageBuilder::new()`, `.generate()`,
//! and `serialize_language()`. Generated code is inspected as a string.

use adze_glr_core::{GotoIndexing, LexMode, ParseTable};
use adze_ir::{Grammar, ProductionId, Rule, StateId, Symbol, SymbolId, Token, TokenPattern};
use adze_tablegen::AbiLanguageBuilder;
use adze_tablegen::serializer::serialize_language;
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const INVALID: StateId = StateId(u16::MAX);

fn string_token(name: &str, literal: &str) -> Token {
    Token {
        name: name.to_string(),
        pattern: TokenPattern::String(literal.to_string()),
        fragile: false,
    }
}

/// Build a ParseTable with the given dimensions.
///
/// Symbol layout: ERROR(0), terminals 1..=num_terms, EOF, non-terminals.
fn make_empty_table(states: usize, terms: usize, nonterms: usize, externals: usize) -> ParseTable {
    let states = states.max(1);
    let eof_idx = 1 + terms + externals;
    let nonterms_eff = if nonterms == 0 { 1 } else { nonterms };
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

/// Convenience: create a simple rule.
fn simple_rule(lhs: SymbolId, rhs: Vec<Symbol>, prod_id: u16) -> Rule {
    Rule {
        lhs,
        rhs,
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(prod_id),
    }
}

/// Build a grammar with given tokens and rules, using the provided table's start_symbol.
fn build_grammar(
    name: &str,
    table: &ParseTable,
    tokens: Vec<(SymbolId, Token)>,
    rules: Vec<Rule>,
) -> Grammar {
    let mut grammar = Grammar::new(name.to_string());
    for (id, tok) in tokens {
        grammar.tokens.insert(id, tok);
    }
    for r in &rules {
        grammar
            .rule_names
            .entry(r.lhs)
            .or_insert_with(|| format!("nt_{}", r.lhs.0));
    }
    for rule in rules {
        grammar.add_rule(rule);
    }
    let _ = table; // used by caller for start_symbol
    grammar
}

/// Generate code string from grammar + table.
fn gen_code(grammar: &Grammar, table: &ParseTable) -> String {
    AbiLanguageBuilder::new(grammar, table)
        .generate()
        .to_string()
}

/// Extract the `PRODUCTION_ID_MAP` array values from generated code.
/// Looks for `static PRODUCTION_ID_MAP : & [u16] = & [val1 , val2 , ...] ;`
fn extract_production_id_map(code: &str) -> Vec<u16> {
    extract_u16_array(code, "PRODUCTION_ID_MAP")
}

/// Extract the `PRODUCTION_LHS_INDEX` array values from generated code.
fn extract_production_lhs_index(code: &str) -> Vec<u16> {
    extract_u16_array(code, "PRODUCTION_LHS_INDEX")
}

/// Extract a `static NAME: &[u16] = &[...];` array from generated code.
fn extract_u16_array(code: &str, name: &str) -> Vec<u16> {
    // Generated code looks like: `static PRODUCTION_ID_MAP : & [u16] = & [0u16 , 1u16 , 2u16] ;`
    let marker = format!("{} :", name);
    let start = code
        .find(&marker)
        .unwrap_or_else(|| panic!("{name} not found in code"));
    let rest = &code[start..];
    // Skip past the `= ` to find the value array (not the type annotation `& [u16]`)
    let eq_pos = rest.find("= &").expect("= & not found") + 2;
    let after_eq = &rest[eq_pos..];
    let bracket_start = after_eq.find('[').expect("[ not found") + 1;
    let bracket_end = after_eq[bracket_start..].find(']').expect("] not found") + bracket_start;
    let inner = &after_eq[bracket_start..bracket_end];
    if inner.trim().is_empty() {
        return vec![];
    }
    inner
        .split(',')
        .map(|s| {
            s.trim()
                .trim_end_matches("u16")
                .trim()
                .parse::<u16>()
                .unwrap_or_else(|_| panic!("cannot parse u16 from '{}'", s.trim()))
        })
        .collect()
}

/// Count how many `TSRule {` occurrences in code (one per production).
fn count_ts_rules(code: &str) -> usize {
    code.matches("TSRule {").count()
}

/// Extract rhs_len values from TSRule structs in generated code.
fn extract_rhs_lens(code: &str) -> Vec<u8> {
    let mut lens = Vec::new();
    for part in code.split("TSRule {") {
        if let Some(rhs_pos) = part.find("rhs_len :") {
            let after = &part[rhs_pos + "rhs_len :".len()..];
            let val_str = after
                .trim_start()
                .split(|c: char| !c.is_ascii_digit())
                .next()
                .unwrap_or("");
            if let Ok(v) = val_str.parse::<u8>() {
                lens.push(v);
            }
        }
    }
    lens
}

/// Extract the `production_id_count` from the serialized JSON.
fn serialized_production_count(grammar: &Grammar, table: &ParseTable) -> u32 {
    let json = serialize_language(grammar, table, None).expect("serialization must succeed");
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    v["production_id_count"].as_u64().unwrap() as u32
}

// ===========================================================================
// 1. PRODUCTION_ID_MAP: basic cases
// ===========================================================================

#[test]
fn production_id_map_single_rule() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 0);
    let start = table.start_symbol;
    let grammar = build_grammar(
        "single",
        &table,
        vec![(t, string_token("t", "x"))],
        vec![simple_rule(start, vec![Symbol::Terminal(t)], 0)],
    );
    let code = gen_code(&grammar, &table);
    let map = extract_production_id_map(&code);
    assert_eq!(map.len(), 1);
    assert_eq!(map[0], 0);
}

#[test]
fn production_id_map_three_contiguous_rules() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 0);
    let start = table.start_symbol;
    let grammar = build_grammar(
        "three",
        &table,
        vec![(t, string_token("t", "x"))],
        vec![
            simple_rule(start, vec![Symbol::Terminal(t)], 0),
            simple_rule(start, vec![Symbol::Terminal(t)], 1),
            simple_rule(start, vec![Symbol::Terminal(t)], 2),
        ],
    );
    let code = gen_code(&grammar, &table);
    let map = extract_production_id_map(&code);
    assert_eq!(map.len(), 3);
    for i in 0..3 {
        assert_eq!(map[i], i as u16);
    }
}

#[test]
fn production_id_map_preserves_identity_mapping() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 0);
    let start = table.start_symbol;
    let grammar = build_grammar(
        "identity",
        &table,
        vec![(t, string_token("tok", "a"))],
        vec![
            simple_rule(start, vec![Symbol::Terminal(t)], 0),
            simple_rule(start, vec![Symbol::Terminal(t), Symbol::Terminal(t)], 1),
        ],
    );
    let code = gen_code(&grammar, &table);
    let map = extract_production_id_map(&code);
    assert_eq!(map.len(), 2);
    assert_eq!(map[0], 0);
    assert_eq!(map[1], 1);
}

// ===========================================================================
// 2. PRODUCTION_LHS_INDEX: basic cases
// ===========================================================================

#[test]
fn lhs_index_single_nonterminal() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 0);
    let start = table.start_symbol;
    let grammar = build_grammar(
        "lhs_single",
        &table,
        vec![(t, string_token("t", "x"))],
        vec![simple_rule(start, vec![Symbol::Terminal(t)], 0)],
    );
    let code = gen_code(&grammar, &table);
    let lhs = extract_production_lhs_index(&code);
    assert_eq!(lhs.len(), 1);
    assert!(
        lhs[0] as usize >= table.token_count,
        "LHS {} must be in non-terminal region (>= {})",
        lhs[0],
        table.token_count
    );
}

#[test]
fn lhs_index_all_entries_in_nonterminal_region() {
    let t1 = SymbolId(1);
    let t2 = SymbolId(2);
    let table = make_empty_table(2, 2, 2, 0);
    let nt1 = table.start_symbol;
    let nt2 = SymbolId(nt1.0 + 1);
    let grammar = build_grammar(
        "lhs_multi",
        &table,
        vec![(t1, string_token("a", "a")), (t2, string_token("b", "b"))],
        vec![
            simple_rule(nt1, vec![Symbol::Terminal(t1)], 0),
            simple_rule(nt2, vec![Symbol::Terminal(t2)], 1),
        ],
    );
    let code = gen_code(&grammar, &table);
    let lhs = extract_production_lhs_index(&code);
    assert_eq!(lhs.len(), 2);
    for i in 0..lhs.len() {
        assert!(
            lhs[i] as usize >= table.token_count,
            "lhs[{}] = {} must be >= token_count {}",
            i,
            lhs[i],
            table.token_count
        );
    }
}

#[test]
fn lhs_index_multiple_rules_same_nonterminal() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 0);
    let start = table.start_symbol;
    let grammar = build_grammar(
        "multi_same",
        &table,
        vec![(t, string_token("t", "x"))],
        vec![
            simple_rule(start, vec![Symbol::Terminal(t)], 0),
            simple_rule(start, vec![Symbol::Terminal(t), Symbol::Terminal(t)], 1),
        ],
    );
    let code = gen_code(&grammar, &table);
    let lhs = extract_production_lhs_index(&code);
    assert_eq!(lhs.len(), 2);
    assert_eq!(
        lhs[0], lhs[1],
        "same nonterminal must produce same LHS index"
    );
}

#[test]
fn lhs_index_different_nonterminals_get_different_indices() {
    let t = SymbolId(1);
    let table = make_empty_table(2, 1, 2, 0);
    let nt1 = table.start_symbol;
    let nt2 = SymbolId(nt1.0 + 1);
    let grammar = build_grammar(
        "diff_nt",
        &table,
        vec![(t, string_token("t", "t"))],
        vec![
            simple_rule(nt1, vec![Symbol::Terminal(t)], 0),
            simple_rule(nt2, vec![Symbol::Terminal(t)], 1),
        ],
    );
    let code = gen_code(&grammar, &table);
    let lhs = extract_production_lhs_index(&code);
    assert_eq!(lhs.len(), 2);
    assert_ne!(
        lhs[0], lhs[1],
        "different nonterminals must yield different LHS indices"
    );
}

// ===========================================================================
// 3. TS_RULES: RHS length encoding
// ===========================================================================

#[test]
fn ts_rules_single_terminal_rhs() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 0);
    let start = table.start_symbol;
    let grammar = build_grammar(
        "rhs1",
        &table,
        vec![(t, string_token("t", "x"))],
        vec![simple_rule(start, vec![Symbol::Terminal(t)], 0)],
    );
    let code = gen_code(&grammar, &table);
    let lens = extract_rhs_lens(&code);
    assert_eq!(lens.len(), 1);
    assert_eq!(lens[0], 1);
}

#[test]
fn ts_rules_empty_rhs_epsilon_production() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 0);
    let start = table.start_symbol;
    let grammar = build_grammar(
        "eps",
        &table,
        vec![(t, string_token("t", "x"))],
        vec![simple_rule(start, vec![], 0)],
    );
    let code = gen_code(&grammar, &table);
    let lens = extract_rhs_lens(&code);
    assert_eq!(lens.len(), 1);
    assert_eq!(lens[0], 0, "epsilon production rhs_len must be 0");
}

#[test]
fn ts_rules_multiple_rhs_symbols() {
    let t1 = SymbolId(1);
    let t2 = SymbolId(2);
    let table = make_empty_table(1, 2, 1, 0);
    let start = table.start_symbol;
    let grammar = build_grammar(
        "multi_rhs",
        &table,
        vec![(t1, string_token("a", "a")), (t2, string_token("b", "b"))],
        vec![simple_rule(
            start,
            vec![
                Symbol::Terminal(t1),
                Symbol::Terminal(t2),
                Symbol::Terminal(t1),
            ],
            0,
        )],
    );
    let code = gen_code(&grammar, &table);
    let lens = extract_rhs_lens(&code);
    assert_eq!(lens.len(), 1);
    assert_eq!(lens[0], 3);
}

#[test]
fn ts_rules_count_matches_grammar_rules() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 0);
    let start = table.start_symbol;
    let mut grammar = build_grammar("count", &table, vec![(t, string_token("t", "x"))], vec![]);
    grammar.rule_names.insert(start, "s".to_string());
    for i in 0..5 {
        grammar.add_rule(simple_rule(start, vec![Symbol::Terminal(t)], i));
    }
    let code = gen_code(&grammar, &table);
    assert_eq!(count_ts_rules(&code), 5);
}

#[test]
fn ts_rules_varying_rhs_lengths() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 0);
    let start = table.start_symbol;
    let mut grammar = build_grammar("vary", &table, vec![(t, string_token("t", "x"))], vec![]);
    grammar.rule_names.insert(start, "s".to_string());
    let expected_lengths: Vec<u8> = vec![0, 1, 2, 3, 4];
    for (i, &len) in expected_lengths.iter().enumerate() {
        let rhs: Vec<Symbol> = (0..len).map(|_| Symbol::Terminal(t)).collect();
        grammar.add_rule(simple_rule(start, rhs, i as u16));
    }
    let code = gen_code(&grammar, &table);
    let lens = extract_rhs_lens(&code);
    assert_eq!(lens.len(), 5);
    assert_eq!(lens, expected_lengths);
}

// ===========================================================================
// 4. production_id_count via serializer
// ===========================================================================

#[test]
fn production_count_single_rule() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 0);
    let start = table.start_symbol;
    let grammar = build_grammar(
        "cnt1",
        &table,
        vec![(t, string_token("t", "x"))],
        vec![simple_rule(start, vec![Symbol::Terminal(t)], 0)],
    );
    assert_eq!(serialized_production_count(&grammar, &table), 1);
}

#[test]
fn production_count_multiple_rules() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 0);
    let start = table.start_symbol;
    let mut grammar = build_grammar("cnt3", &table, vec![(t, string_token("t", "x"))], vec![]);
    grammar.rule_names.insert(start, "s".to_string());
    for i in 0..3 {
        grammar.add_rule(simple_rule(start, vec![Symbol::Terminal(t)], i));
    }
    assert_eq!(serialized_production_count(&grammar, &table), 3);
}

#[test]
fn production_count_matches_total_rules() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 2, 0);
    let nt1 = table.start_symbol;
    let nt2 = SymbolId(nt1.0 + 1);
    let grammar = build_grammar(
        "cnt_multi",
        &table,
        vec![(t, string_token("t", "x"))],
        vec![
            simple_rule(nt1, vec![Symbol::Terminal(t)], 0),
            simple_rule(nt1, vec![Symbol::Terminal(t), Symbol::Terminal(t)], 1),
            simple_rule(nt2, vec![Symbol::Terminal(t)], 2),
        ],
    );
    assert_eq!(serialized_production_count(&grammar, &table), 3);
}

// ===========================================================================
// 5. Ordering and sorting
// ===========================================================================

#[test]
fn production_id_map_preserves_rule_id_order() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 0);
    let start = table.start_symbol;
    // Intentionally add rules out of production-ID order. Parse actions encode
    // GLR rule IDs, so this map must preserve insertion/rule-ID order.
    let grammar = build_grammar(
        "rule_id_order",
        &table,
        vec![(t, string_token("t", "x"))],
        vec![
            simple_rule(start, vec![Symbol::Terminal(t), Symbol::Terminal(t)], 2),
            simple_rule(start, vec![Symbol::Terminal(t)], 0),
            simple_rule(start, vec![], 1),
        ],
    );
    let code = gen_code(&grammar, &table);
    let map = extract_production_id_map(&code);
    assert_eq!(map.len(), 3);
    assert_eq!(map, vec![2, 0, 1]);
}

#[test]
fn test_ts_rules_are_rule_ordered_for_sparse_production_ids() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 2, 0);
    let nt1 = table.start_symbol;
    let nt2 = SymbolId(nt1.0 + 1);
    let grammar = build_grammar(
        "sparse_ts_rules",
        &table,
        vec![(t, string_token("t", "x"))],
        vec![
            simple_rule(nt1, vec![Symbol::Terminal(t)], 7),
            simple_rule(nt2, vec![Symbol::Terminal(t), Symbol::Terminal(t)], 0),
        ],
    );

    let code = gen_code(&grammar, &table);
    let lens = extract_rhs_lens(&code);
    let lhs = extract_production_lhs_index(&code);

    assert_eq!(extract_production_id_map(&code), vec![7, 0]);
    assert_eq!(lens, vec![1, 2], "TS_RULES stores syntax in RuleId order");
    assert_eq!(lhs.len(), 8, "production metadata keeps sparse slots");
    assert_eq!(lhs[7], table.symbol_to_index[&nt1] as u16);
    assert_eq!(lhs[0], table.symbol_to_index[&nt2] as u16);
}

#[test]
fn lhs_index_sorted_by_production_id() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 2, 0);
    let nt1 = table.start_symbol;
    let nt2 = SymbolId(nt1.0 + 1);
    let mut grammar = Grammar::new("lhs_sorted".to_string());
    grammar.rule_names.insert(nt1, "a".to_string());
    grammar.rule_names.insert(nt2, "b".to_string());
    grammar.tokens.insert(t, string_token("t", "x"));
    // Insert rule with production_id=1 first, then production_id=0
    grammar.add_rule(simple_rule(nt2, vec![Symbol::Terminal(t)], 1));
    grammar.add_rule(simple_rule(nt1, vec![Symbol::Terminal(t)], 0));

    let code = gen_code(&grammar, &table);
    let lhs = extract_production_lhs_index(&code);
    assert_eq!(lhs.len(), 2);
    let idx_nt1 = *table.symbol_to_index.get(&nt1).unwrap() as u16;
    let idx_nt2 = *table.symbol_to_index.get(&nt2).unwrap() as u16;
    assert_eq!(lhs[0], idx_nt1, "production 0 → nt1");
    assert_eq!(lhs[1], idx_nt2, "production 1 → nt2");
}

// ===========================================================================
// 6. Consistency between arrays
// ===========================================================================

#[test]
fn all_production_arrays_same_length() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 0);
    let start = table.start_symbol;
    let mut grammar = build_grammar(
        "consistent",
        &table,
        vec![(t, string_token("t", "x"))],
        vec![],
    );
    grammar.rule_names.insert(start, "s".to_string());
    for i in 0..4 {
        grammar.add_rule(simple_rule(start, vec![Symbol::Terminal(t)], i));
    }
    let code = gen_code(&grammar, &table);
    let id_map = extract_production_id_map(&code);
    let lhs = extract_production_lhs_index(&code);
    let rule_count = count_ts_rules(&code);
    assert_eq!(
        id_map.len(),
        lhs.len(),
        "id_map and lhs_index must have equal length"
    );
    assert_eq!(
        lhs.len(),
        rule_count,
        "lhs_index and ts_rules must have equal length"
    );
}

#[test]
fn lhs_same_for_all_rules_of_single_nonterminal() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 0);
    let start = table.start_symbol;
    let grammar = build_grammar(
        "match_lhs",
        &table,
        vec![(t, string_token("t", "x"))],
        vec![
            simple_rule(start, vec![Symbol::Terminal(t)], 0),
            simple_rule(start, vec![Symbol::Terminal(t), Symbol::Terminal(t)], 1),
        ],
    );
    let code = gen_code(&grammar, &table);
    let lhs = extract_production_lhs_index(&code);
    assert_eq!(
        lhs[0], lhs[1],
        "same nonterminal should produce same LHS index for all rules"
    );
}

// ===========================================================================
// 7. Multiple non-terminals
// ===========================================================================

#[test]
fn three_nonterminals_with_distinct_lhs_indices() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 3, 0);
    let nt1 = table.start_symbol;
    let nt2 = SymbolId(nt1.0 + 1);
    let nt3 = SymbolId(nt1.0 + 2);
    let grammar = build_grammar(
        "three_nt",
        &table,
        vec![(t, string_token("x", "x"))],
        vec![
            simple_rule(nt1, vec![Symbol::NonTerminal(nt2)], 0),
            simple_rule(nt2, vec![Symbol::NonTerminal(nt3)], 1),
            simple_rule(nt3, vec![Symbol::Terminal(t)], 2),
        ],
    );
    let code = gen_code(&grammar, &table);
    let lhs = extract_production_lhs_index(&code);
    assert_eq!(lhs.len(), 3);
    let mut seen = std::collections::HashSet::new();
    for &v in &lhs {
        seen.insert(v);
    }
    assert_eq!(
        seen.len(),
        3,
        "three distinct nonterminals should produce 3 distinct LHS indices"
    );
}

#[test]
fn mixed_nonterminals_lhs_values_consistent() {
    let t1 = SymbolId(1);
    let t2 = SymbolId(2);
    let table = make_empty_table(2, 2, 2, 0);
    let nt1 = table.start_symbol;
    let nt2 = SymbolId(nt1.0 + 1);
    let grammar = build_grammar(
        "mixed",
        &table,
        vec![(t1, string_token("a", "a")), (t2, string_token("b", "b"))],
        vec![
            simple_rule(nt1, vec![Symbol::Terminal(t1)], 0),
            simple_rule(nt1, vec![Symbol::Terminal(t2)], 1),
            simple_rule(nt2, vec![Symbol::Terminal(t1), Symbol::Terminal(t2)], 2),
        ],
    );
    let code = gen_code(&grammar, &table);
    let lhs = extract_production_lhs_index(&code);
    assert_eq!(lhs.len(), 3);
    let expected_nt1_idx = *table.symbol_to_index.get(&nt1).unwrap() as u16;
    let expected_nt2_idx = *table.symbol_to_index.get(&nt2).unwrap() as u16;
    assert_eq!(lhs[0], expected_nt1_idx);
    assert_eq!(lhs[1], expected_nt1_idx);
    assert_eq!(lhs[2], expected_nt2_idx);
}

// ===========================================================================
// 8. Edge cases
// ===========================================================================

#[test]
fn single_epsilon_production_lhs_and_ts_rules() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 0);
    let start = table.start_symbol;
    let grammar = build_grammar(
        "eps_edge",
        &table,
        vec![(t, string_token("t", "x"))],
        vec![simple_rule(start, vec![], 0)],
    );
    let code = gen_code(&grammar, &table);
    let lhs = extract_production_lhs_index(&code);
    let lens = extract_rhs_lens(&code);
    assert_eq!(lhs.len(), 1);
    assert_eq!(lens.len(), 1);
    assert_eq!(lens[0], 0);
}

#[test]
fn many_rules_production_arrays_stay_aligned() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 0);
    let start = table.start_symbol;
    let mut grammar = build_grammar("many", &table, vec![(t, string_token("t", "x"))], vec![]);
    grammar.rule_names.insert(start, "s".to_string());
    let n = 20;
    for i in 0..n {
        let rhs: Vec<Symbol> = (0..=(i % 5)).map(|_| Symbol::Terminal(t)).collect();
        grammar.add_rule(simple_rule(start, rhs, i as u16));
    }
    let code = gen_code(&grammar, &table);
    let id_map = extract_production_id_map(&code);
    let lhs = extract_production_lhs_index(&code);
    let rule_count = count_ts_rules(&code);
    assert_eq!(id_map.len(), n as usize);
    assert_eq!(lhs.len(), n as usize);
    assert_eq!(rule_count, n as usize);
}

#[test]
fn production_id_map_with_production_id_zero_only() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 0);
    let start = table.start_symbol;
    let grammar = build_grammar(
        "zero_only",
        &table,
        vec![(t, string_token("t", "x"))],
        vec![simple_rule(start, vec![Symbol::Terminal(t)], 0)],
    );
    let code = gen_code(&grammar, &table);
    let map = extract_production_id_map(&code);
    assert_eq!(map.len(), 1);
    assert_eq!(map[0], 0);
}

#[test]
fn ts_rules_pad_field_is_zero() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 0);
    let start = table.start_symbol;
    let grammar = build_grammar(
        "pad",
        &table,
        vec![(t, string_token("t", "x"))],
        vec![simple_rule(start, vec![Symbol::Terminal(t)], 0)],
    );
    let code = gen_code(&grammar, &table);
    assert!(code.contains("_pad : 0"), "TSRule _pad must be 0");
}

// ===========================================================================
// 9. RHS with non-terminal symbols
// ===========================================================================

#[test]
fn ts_rules_rhs_with_nonterminals_counted() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 2, 0);
    let nt1 = table.start_symbol;
    let nt2 = SymbolId(nt1.0 + 1);
    let grammar = build_grammar(
        "nt_rhs",
        &table,
        vec![(t, string_token("t", "t"))],
        vec![
            // expr -> t term t (3 symbols)
            simple_rule(
                nt1,
                vec![
                    Symbol::Terminal(t),
                    Symbol::NonTerminal(nt2),
                    Symbol::Terminal(t),
                ],
                0,
            ),
            simple_rule(nt2, vec![Symbol::Terminal(t)], 1),
        ],
    );
    let code = gen_code(&grammar, &table);
    let lens = extract_rhs_lens(&code);
    assert_eq!(lens.len(), 2);
    assert_eq!(lens[0], 3, "expr -> t term t has rhs_len 3");
    assert_eq!(lens[1], 1, "term -> t has rhs_len 1");
}

// ===========================================================================
// 10. generate() integration: arrays present in generated code
// ===========================================================================

#[test]
fn generate_output_contains_production_lhs_index_array() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 0);
    let start = table.start_symbol;
    let grammar = build_grammar(
        "gen_lhs",
        &table,
        vec![(t, string_token("t", "t"))],
        vec![simple_rule(start, vec![Symbol::Terminal(t)], 0)],
    );
    let code = gen_code(&grammar, &table);
    assert!(code.contains("PRODUCTION_LHS_INDEX"));
}

#[test]
fn generate_output_contains_production_id_map_array() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 0);
    let start = table.start_symbol;
    let grammar = build_grammar(
        "gen_map",
        &table,
        vec![(t, string_token("t", "t"))],
        vec![simple_rule(start, vec![Symbol::Terminal(t)], 0)],
    );
    let code = gen_code(&grammar, &table);
    assert!(code.contains("PRODUCTION_ID_MAP"));
}

#[test]
fn generate_output_contains_ts_rules_array() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 0);
    let start = table.start_symbol;
    let grammar = build_grammar(
        "gen_rules",
        &table,
        vec![(t, string_token("t", "t"))],
        vec![simple_rule(start, vec![Symbol::Terminal(t)], 0)],
    );
    let code = gen_code(&grammar, &table);
    assert!(code.contains("TS_RULES"));
}

#[test]
fn generate_output_production_count_field_present() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 0);
    let start = table.start_symbol;
    let grammar = build_grammar(
        "gen_count",
        &table,
        vec![(t, string_token("t", "t"))],
        vec![
            simple_rule(start, vec![Symbol::Terminal(t)], 0),
            simple_rule(start, vec![Symbol::Terminal(t), Symbol::Terminal(t)], 1),
        ],
    );
    let code = gen_code(&grammar, &table);
    assert!(
        code.contains("production_count"),
        "generated code must reference production_count"
    );
}

// ===========================================================================
// 11. Production map with externals
// ===========================================================================

#[test]
fn production_arrays_work_with_external_tokens() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 2);
    let start = table.start_symbol;
    let grammar = build_grammar(
        "ext",
        &table,
        vec![(t, string_token("t", "t"))],
        vec![simple_rule(start, vec![Symbol::Terminal(t)], 0)],
    );
    let code = gen_code(&grammar, &table);
    let id_map = extract_production_id_map(&code);
    let lhs = extract_production_lhs_index(&code);
    let rule_count = count_ts_rules(&code);
    assert_eq!(id_map.len(), 1);
    assert_eq!(lhs.len(), 1);
    assert_eq!(rule_count, 1);
    // LHS must still be in nonterminal region (past EOF and externals)
    assert!(
        lhs[0] as usize >= table.token_count,
        "LHS {} must be >= token_count {} with externals",
        lhs[0],
        table.token_count
    );
}

// ===========================================================================
// 12. Large RHS
// ===========================================================================

#[test]
fn ts_rules_large_rhs_length() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 0);
    let start = table.start_symbol;
    let grammar = build_grammar(
        "large_rhs",
        &table,
        vec![(t, string_token("t", "x"))],
        vec![simple_rule(
            start,
            (0..10).map(|_| Symbol::Terminal(t)).collect(),
            0,
        )],
    );
    let code = gen_code(&grammar, &table);
    let lens = extract_rhs_lens(&code);
    assert_eq!(lens[0], 10);
}

// ===========================================================================
// 13. Serialized language production_id_count
// ===========================================================================

#[test]
fn serialized_production_id_count_three_rules() {
    let t = SymbolId(1);
    let table = make_empty_table(1, 1, 1, 0);
    let start = table.start_symbol;
    let grammar = build_grammar(
        "ser3",
        &table,
        vec![(t, string_token("t", "t"))],
        vec![
            simple_rule(start, vec![Symbol::Terminal(t)], 0),
            simple_rule(start, vec![Symbol::Terminal(t), Symbol::Terminal(t)], 1),
            simple_rule(start, vec![], 2),
        ],
    );
    assert_eq!(serialized_production_count(&grammar, &table), 3);
}

// ===========================================================================
// 14. Production arrays independent of state count
// ===========================================================================

#[test]
fn production_arrays_independent_of_state_count() {
    let t = SymbolId(1);
    let table_small = make_empty_table(1, 1, 1, 0);
    let table_large = make_empty_table(10, 1, 1, 0);

    let make_grammar = |table: &ParseTable, name: &str| {
        build_grammar(
            name,
            table,
            vec![(t, string_token("t", "x"))],
            vec![
                simple_rule(table.start_symbol, vec![Symbol::Terminal(t)], 0),
                simple_rule(
                    table.start_symbol,
                    vec![Symbol::Terminal(t), Symbol::Terminal(t)],
                    1,
                ),
            ],
        )
    };

    let g_small = make_grammar(&table_small, "small");
    let g_large = make_grammar(&table_large, "large");
    let code_s = gen_code(&g_small, &table_small);
    let code_l = gen_code(&g_large, &table_large);
    let map_s = extract_production_id_map(&code_s);
    let map_l = extract_production_id_map(&code_l);
    assert_eq!(map_s.len(), map_l.len());
    for i in 0..map_s.len() {
        assert_eq!(map_s[i], map_l[i]);
    }
}

// ===========================================================================
// 15. Production map with many terminals
// ===========================================================================

#[test]
fn production_lhs_correct_with_many_terminals() {
    let terms: Vec<(SymbolId, Token)> = (1..=5)
        .map(|i| {
            (
                SymbolId(i),
                string_token(
                    &format!("t{}", i),
                    &format!("{}", (b'a' + i as u8 - 1) as char),
                ),
            )
        })
        .collect();
    let table = make_empty_table(1, 5, 1, 0);
    let start = table.start_symbol;
    let grammar = build_grammar(
        "many_terms",
        &table,
        terms,
        vec![simple_rule(
            start,
            vec![
                Symbol::Terminal(SymbolId(1)),
                Symbol::Terminal(SymbolId(2)),
                Symbol::Terminal(SymbolId(3)),
            ],
            0,
        )],
    );
    let code = gen_code(&grammar, &table);
    let lhs = extract_production_lhs_index(&code);
    assert_eq!(lhs.len(), 1);
    assert!(
        lhs[0] as usize >= table.token_count,
        "LHS must be in nonterminal region even with many terminals"
    );
    let lens = extract_rhs_lens(&code);
    assert_eq!(lens[0], 3);
}

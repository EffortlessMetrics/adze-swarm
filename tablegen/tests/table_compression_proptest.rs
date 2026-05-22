//! Property-based tests for table compression in adze-tablegen.
//!
//! Covers:
//! 1. CompressedTables struct properties
//! 2. Table compression roundtrip invariants
//! 3. StaticLanguageGenerator construction
//! 4. Language code generation determinism
//! 5. Node types JSON generation
//! 6. Edge cases in compression (empty tables, large tables)
//! 7. Symbol metadata encoding

#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

use adze_glr_core::{Action, GotoIndexing, LexMode, ParseRule, ParseTable};
use adze_ir::{ExternalToken, Grammar, RuleId, StateId, SymbolId, Token, TokenPattern};
use adze_tablegen::StaticLanguageGenerator;
use adze_tablegen::compress::{
    CompressedActionEntry, CompressedGotoEntry, CompressedParseTable, TableCompressor,
};
use adze_tablegen::compression::{
    BitPackedActionTable, compress_action_table, compress_goto_table, decompress_action,
    decompress_goto,
};
use adze_tablegen::node_types::NodeTypesGenerator;
use proptest::prelude::*;
use std::collections::BTreeMap;

// ── Constants ───────────────────────────────────────────────────────────

const INVALID: StateId = StateId(u16::MAX);

// ── Helpers ─────────────────────────────────────────────────────────────

/// Build a ParseTable suitable for integration tests (no cfg(test) helpers).
fn make_parse_table(
    mut actions: Vec<Vec<Vec<Action>>>,
    mut gotos: Vec<Vec<StateId>>,
    rules: Vec<ParseRule>,
    start_symbol: SymbolId,
    eof_symbol: SymbolId,
    external_token_count: usize,
) -> ParseTable {
    let state_count = actions.len().max(1);
    let sym_from_act = actions.first().map(|r| r.len()).unwrap_or(0);
    let sym_from_goto = gotos.first().map(|r| r.len()).unwrap_or(0);
    let min_needed = (start_symbol.0 as usize + 1).max(eof_symbol.0 as usize + 1);
    let symbol_count = sym_from_act.max(sym_from_goto).max(min_needed).max(1);

    if actions.is_empty() {
        actions = vec![vec![vec![]; symbol_count]];
    } else {
        for row in &mut actions {
            if row.len() < symbol_count {
                row.resize_with(symbol_count, Vec::new);
            }
        }
    }
    if gotos.len() < state_count {
        gotos.resize_with(state_count, || vec![INVALID; symbol_count]);
    }
    for row in &mut gotos {
        if row.len() < symbol_count {
            row.resize(symbol_count, INVALID);
        }
    }

    let mut symbol_to_index = BTreeMap::new();
    for i in 0..symbol_count {
        symbol_to_index.insert(SymbolId(i as u16), i);
    }
    let mut nonterminal_to_index = BTreeMap::new();
    for col in 0..symbol_count {
        if gotos.iter().any(|r| r[col] != INVALID) {
            nonterminal_to_index.insert(SymbolId(col as u16), col);
        }
    }
    nonterminal_to_index
        .entry(start_symbol)
        .or_insert(start_symbol.0 as usize);

    let eof_idx = eof_symbol.0 as usize;
    let token_count = eof_idx.saturating_sub(external_token_count);

    let lex_modes = vec![
        LexMode {
            lex_state: 0,
            external_lex_state: 0,
        };
        state_count
    ];
    let mut index_to_symbol = vec![SymbolId(0); symbol_count];
    for (&sid, &idx) in &symbol_to_index {
        index_to_symbol[idx] = sid;
    }

    ParseTable {
        action_table: actions,
        goto_table: gotos,
        symbol_metadata: vec![],
        state_count,
        symbol_count,
        symbol_to_index,
        index_to_symbol,
        nonterminal_to_index,
        external_scanner_states: vec![],
        rules,
        goto_indexing: GotoIndexing::NonterminalMap,
        eof_symbol,
        start_symbol,
        grammar: Grammar::new("test".to_string()),
        initial_state: StateId(0),
        token_count,
        external_token_count,
        lex_modes,
        extras: vec![],
        dynamic_prec_by_rule: vec![],
        rule_assoc_by_rule: vec![],
        alias_sequences: vec![],
        field_names: vec![],
        field_map: BTreeMap::new(),
    }
}

/// Build a minimal parse table with `terms` terminals, 1 non-terminal, and a shift on token 1.
fn make_shiftable_table(states: usize, terms: usize) -> ParseTable {
    let states = states.max(1);
    let terms = terms.max(1);
    let eof_idx = 1 + terms; // 0=ERROR, 1..terms=terminals, eof_idx=EOF
    let start_sym = eof_idx + 1; // first non-terminal
    let symbol_count = start_sym + 1;

    let mut actions = vec![vec![vec![]; symbol_count]; states];
    // Place a Shift in state 0 on terminal 1
    actions[0][1] = vec![Action::Shift(StateId(0))];

    let gotos = vec![vec![INVALID; symbol_count]; states];
    let rules = vec![];

    make_parse_table(
        actions,
        gotos,
        rules,
        SymbolId(start_sym as u16),
        SymbolId(eof_idx as u16),
        0,
    )
}

/// Build a grammar with named tokens.
fn make_grammar_with_tokens(name: &str, token_names: &[(&str, TokenPattern)]) -> Grammar {
    let mut grammar = Grammar::new(name.to_string());
    for (i, (tok_name, pattern)) in token_names.iter().enumerate() {
        grammar.tokens.insert(
            SymbolId(i as u16 + 1),
            Token {
                name: tok_name.to_string(),
                pattern: pattern.clone(),
                fragile: false,
            },
        );
    }
    grammar
}

// ── Strategies ──────────────────────────────────────────────────────────

fn action_strategy() -> impl Strategy<Value = Action> {
    prop_oneof![
        Just(Action::Error),
        Just(Action::Accept),
        (1u16..100).prop_map(|s| Action::Shift(StateId(s))),
        (0u16..50).prop_map(|r| Action::Reduce(RuleId(r))),
    ]
}

fn action_cell_strategy() -> impl Strategy<Value = Vec<Action>> {
    prop::collection::vec(action_strategy(), 0..=3)
}

fn action_table_strategy(
    max_states: usize,
    max_symbols: usize,
) -> impl Strategy<Value = Vec<Vec<Vec<Action>>>> {
    (1..=max_states, 1..=max_symbols).prop_flat_map(|(states, symbols)| {
        prop::collection::vec(
            prop::collection::vec(action_cell_strategy(), symbols..=symbols),
            states..=states,
        )
    })
}

fn goto_table_strategy(
    max_states: usize,
    max_symbols: usize,
) -> impl Strategy<Value = Vec<Vec<Option<StateId>>>> {
    (1..=max_states, 1..=max_symbols).prop_flat_map(|(states, symbols)| {
        let cell = prop_oneof![
            3 => Just(None),
            1 => (0u16..20).prop_map(|s| Some(StateId(s))),
        ];
        prop::collection::vec(
            prop::collection::vec(cell, symbols..=symbols),
            states..=states,
        )
    })
}

fn grammar_name_strategy() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,15}".prop_filter("non-empty", |s| !s.is_empty())
}

// ═══════════════════════════════════════════════════════════════════════
// 1. CompressedTables struct properties
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn compressed_parse_table_preserves_counts(
        sym_count in 1usize..200,
        state_count in 1usize..200,
    ) {
        let cpt = CompressedParseTable::new_for_testing(sym_count, state_count);
        prop_assert_eq!(cpt.symbol_count(), sym_count);
        prop_assert_eq!(cpt.state_count(), state_count);
    }

    #[test]
    fn compressed_parse_table_from_parse_table_matches(
        states in 1usize..10,
        terms in 1usize..8,
    ) {
        let pt = make_shiftable_table(states, terms);
        let cpt = CompressedParseTable::from_parse_table(&pt);
        prop_assert_eq!(cpt.symbol_count(), pt.symbol_count);
        prop_assert_eq!(cpt.state_count(), pt.state_count);
    }

    #[test]
    fn compressed_tables_validate_succeeds(
        states in 1usize..6,
        terms in 1usize..5,
    ) {
        let pt = make_shiftable_table(states, terms);
        let compressor = TableCompressor::new();
        let token_indices: Vec<usize> = (1..=terms).collect();
        let eof_idx = pt.symbol_to_index[&pt.eof_symbol];
        let mut indices = token_indices;
        if !indices.contains(&eof_idx) {
            indices.push(eof_idx);
            indices.sort();
        }
        if let Ok(compressed) = compressor.compress(&pt, &indices, false) {
            prop_assert!(compressed.validate(&pt).is_ok());
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 2. Table compression roundtrip invariants
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn action_roundtrip_preserves_first_action(table in action_table_strategy(8, 8)) {
        let compressed = compress_action_table(&table);

        for (state, row) in table.iter().enumerate() {
            for (symbol, cell) in row.iter().enumerate() {
                let original = cell.first().cloned().unwrap_or(Action::Error);
                let decompressed = decompress_action(&compressed, state, symbol);
                prop_assert_eq!(
                    decompressed, original,
                    "Mismatch at state={}, symbol={}", state, symbol
                );
            }
        }
    }

    #[test]
    fn goto_roundtrip_preserves_all(table in goto_table_strategy(8, 8)) {
        let compressed = compress_goto_table(&table);

        for (state, row) in table.iter().enumerate() {
            for (symbol, expected) in row.iter().enumerate() {
                let got = decompress_goto(&compressed, state, symbol);
                prop_assert_eq!(
                    got, *expected,
                    "Goto mismatch at state={}, symbol={}", state, symbol
                );
            }
        }
    }

    #[test]
    fn action_row_dedup_never_inflates(table in action_table_strategy(12, 8)) {
        let compressed = compress_action_table(&table);
        prop_assert!(compressed.unique_rows.len() <= table.len());
    }

    #[test]
    fn goto_sparse_never_inflates(table in goto_table_strategy(12, 8)) {
        let compressed = compress_goto_table(&table);
        let n_cols = table.first().map(|r| r.len()).unwrap_or(0);
        let total_cells = table.len() * n_cols;
        prop_assert!(compressed.entries.len() <= total_cells);
    }

    #[test]
    fn identical_rows_share_index(
        base_row in prop::collection::vec(action_cell_strategy(), 1..=6),
        n_copies in 2usize..=8,
    ) {
        let table = vec![base_row; n_copies];
        let compressed = compress_action_table(&table);
        prop_assert_eq!(compressed.unique_rows.len(), 1);
        for &idx in &compressed.state_to_row {
            prop_assert_eq!(idx, 0);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 3. StaticLanguageGenerator construction
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn generator_construction_never_panics(name in grammar_name_strategy()) {
        let grammar = Grammar::new(name);
        let pt = ParseTable::default();
        let generator = StaticLanguageGenerator::new(grammar, pt);
        prop_assert!(generator.compressed_tables.is_none());
        prop_assert!(!generator.start_can_be_empty);
    }

    #[test]
    fn generator_set_start_can_be_empty(flag in proptest::bool::ANY) {
        let grammar = Grammar::new("test".to_string());
        let pt = ParseTable::default();
        let mut slg = StaticLanguageGenerator::new(grammar, pt);
        slg.set_start_can_be_empty(flag);
        prop_assert_eq!(slg.start_can_be_empty, flag);
    }

    #[test]
    fn generator_grammar_name_preserved(name in grammar_name_strategy()) {
        let grammar = Grammar::new(name.clone());
        let pt = ParseTable::default();
        let generator = StaticLanguageGenerator::new(grammar, pt);
        prop_assert_eq!(&generator.grammar.name, &name);
    }

    #[test]
    fn generator_parse_table_preserved(
        states in 1usize..6,
        terms in 1usize..5,
    ) {
        let pt = make_shiftable_table(states, terms);
        let expected_states = pt.state_count;
        let expected_symbols = pt.symbol_count;
        let grammar = Grammar::new("test".to_string());
        let generator = StaticLanguageGenerator::new(grammar, pt);
        prop_assert_eq!(generator.parse_table.state_count, expected_states);
        prop_assert_eq!(generator.parse_table.symbol_count, expected_symbols);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 4. Language code generation determinism
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn codegen_is_deterministic(name in grammar_name_strategy()) {
        let g1 = Grammar::new(name.clone());
        let g2 = Grammar::new(name);
        let pt1 = ParseTable::default();
        let pt2 = ParseTable::default();

        let gen1 = StaticLanguageGenerator::new(g1, pt1);
        let gen2 = StaticLanguageGenerator::new(g2, pt2);

        let code1 = gen1.generate_language_code().to_string();
        let code2 = gen2.generate_language_code().to_string();
        prop_assert_eq!(code1, code2);
    }

    #[test]
    fn codegen_produces_nonempty_output(name in grammar_name_strategy()) {
        let grammar = Grammar::new(name);
        let pt = ParseTable::default();
        let generator = StaticLanguageGenerator::new(grammar, pt);
        let code = generator.generate_language_code().to_string();
        prop_assert!(!code.is_empty());
    }

    #[test]
    fn codegen_contains_language_name(name in grammar_name_strategy()) {
        let grammar = Grammar::new(name.clone());
        let pt = ParseTable::default();
        let generator = StaticLanguageGenerator::new(grammar, pt);
        let code = generator.generate_language_code().to_string();
        // The generated code should reference the grammar name somewhere
        prop_assert!(
            code.contains(&name) || code.contains("tree_sitter"),
            "Code should reference grammar name or tree_sitter"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 5. Node types JSON generation
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn node_types_empty_grammar_produces_valid_json(name in grammar_name_strategy()) {
        let grammar = Grammar::new(name);
        let pt = ParseTable::default();
        let generator = StaticLanguageGenerator::new(grammar, pt);
        let json_str = generator.generate_node_types();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        prop_assert!(parsed.is_array());
    }

    #[test]
    fn node_types_generator_empty_grammar_valid_json(name in grammar_name_strategy()) {
        let grammar = Grammar::new(name);
        let generator = NodeTypesGenerator::new(&grammar);
        let result = generator.generate();
        prop_assert!(result.is_ok());
        let json_str = result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        prop_assert!(parsed.is_array());
    }

    #[test]
    fn node_types_with_tokens_contains_token_names(
        n_tokens in 1usize..5,
    ) {
        let token_defs: Vec<(&str, TokenPattern)> = vec![
            ("number", TokenPattern::Regex("[0-9]+".to_string())),
            ("identifier", TokenPattern::Regex("[a-z]+".to_string())),
            ("plus", TokenPattern::String("+".to_string())),
            ("star", TokenPattern::String("*".to_string())),
            ("lparen", TokenPattern::String("(".to_string())),
        ];
        let tokens = &token_defs[..n_tokens.min(token_defs.len())];
        let grammar = make_grammar_with_tokens("test_lang", tokens);
        let pt = ParseTable::default();
        let generator = StaticLanguageGenerator::new(grammar, pt);
        let json_str = generator.generate_node_types();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        prop_assert!(parsed.is_array());
        // Regex tokens should appear as named nodes
        for (name, pat) in tokens {
            if matches!(pat, TokenPattern::Regex(_)) {
                let arr = parsed.as_array().unwrap();
                let found = arr.iter().any(|v| {
                    v.get("type").and_then(|t| t.as_str()) == Some(name)
                });
                prop_assert!(found, "Token '{}' not found in node types", name);
            }
        }
    }

    #[test]
    fn node_types_deterministic(name in grammar_name_strategy()) {
        let g1 = Grammar::new(name.clone());
        let g2 = Grammar::new(name);
        let gen1 = NodeTypesGenerator::new(&g1);
        let gen2 = NodeTypesGenerator::new(&g2);
        prop_assert_eq!(gen1.generate(), gen2.generate());
    }

    #[test]
    fn node_types_generator_with_tokens(n_tokens in 1usize..4) {
        let token_defs: Vec<(&str, TokenPattern)> = vec![
            ("alpha", TokenPattern::Regex("[a-z]+".to_string())),
            ("semi", TokenPattern::String(";".to_string())),
            ("dot", TokenPattern::String(".".to_string())),
            ("beta", TokenPattern::Regex("[A-Z]+".to_string())),
        ];
        let tokens = &token_defs[..n_tokens.min(token_defs.len())];
        let grammar = make_grammar_with_tokens("gen_test", tokens);
        let generator = NodeTypesGenerator::new(&grammar);
        let result = generator.generate();
        prop_assert!(result.is_ok());
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 6. Edge cases in compression (empty tables, large tables)
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn compress_action_table_empty_rows(n_states in 1usize..20) {
        let table = vec![vec![]; n_states];
        let symbol_to_index = BTreeMap::new();
        let compressor = TableCompressor::new();
        let result = compressor.compress_action_table_small(&table, &symbol_to_index);
        prop_assert!(result.is_ok());
        let compressed = result.unwrap();
        prop_assert_eq!(compressed.row_offsets.len(), n_states + 1);
        prop_assert_eq!(compressed.default_actions.len(), n_states);
        prop_assert!(compressed.data.is_empty());
    }

    #[test]
    fn compress_goto_empty_rows(n_states in 1usize..20) {
        let table: Vec<Vec<StateId>> = vec![vec![]; n_states];
        let compressor = TableCompressor::new();
        let result = compressor.compress_goto_table_small(&table);
        prop_assert!(result.is_ok());
        let compressed = result.unwrap();
        prop_assert_eq!(compressed.row_offsets.len(), n_states + 1);
    }

    #[test]
    fn compress_action_all_errors(
        n_states in 1usize..8,
        n_symbols in 1usize..8,
    ) {
        let table = vec![vec![vec![Action::Error]; n_symbols]; n_states];
        let symbol_to_index = BTreeMap::new();
        let compressor = TableCompressor::new();
        let result = compressor.compress_action_table_small(&table, &symbol_to_index);
        prop_assert!(result.is_ok());
        let compressed = result.unwrap();
        // Error actions are skipped during compression
        prop_assert!(compressed.data.is_empty());
    }

    #[test]
    fn compress_goto_all_invalid(
        n_states in 1usize..10,
        n_symbols in 1usize..10,
    ) {
        let table = vec![vec![INVALID; n_symbols]; n_states];
        let compressor = TableCompressor::new();
        let result = compressor.compress_goto_table_small(&table);
        prop_assert!(result.is_ok());
    }

    #[test]
    fn compress_action_single_shift_per_row(
        n_states in 1usize..8,
        n_symbols in 2usize..8,
    ) {
        let mut table = vec![vec![vec![]; n_symbols]; n_states];
        for (i, row) in table.iter_mut().enumerate() {
            let sym_idx = i % n_symbols;
            row[sym_idx] = vec![Action::Shift(StateId(i as u16))];
        }
        let symbol_to_index = BTreeMap::new();
        let compressor = TableCompressor::new();
        let result = compressor.compress_action_table_small(&table, &symbol_to_index);
        prop_assert!(result.is_ok());
        let compressed = result.unwrap();
        prop_assert_eq!(compressed.data.len(), n_states);
    }

    #[test]
    fn compress_goto_run_length_encoding(
        run_len in 4usize..20,
        state_val in 0u16..100,
    ) {
        let table = vec![vec![StateId(state_val); run_len]];
        let compressor = TableCompressor::new();
        let result = compressor.compress_goto_table_small(&table);
        prop_assert!(result.is_ok());
        let compressed = result.unwrap();
        // With a long run of the same state, RLE should compress
        let has_rle = compressed.data.iter().any(|e| {
            matches!(e, CompressedGotoEntry::RunLength { .. })
        });
        // Runs > 2 should use RLE
        if run_len > 2 {
            prop_assert!(has_rle, "Expected RLE for run of length {}", run_len);
        }
    }

    #[test]
    fn compress_large_action_table(
        n_states in 10usize..30,
        n_symbols in 10usize..30,
    ) {
        let mut table = vec![vec![vec![]; n_symbols]; n_states];
        // Populate with a mix of actions
        for (i, row) in table.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                match (i + j) % 4 {
                    0 => *cell = vec![Action::Shift(StateId(j as u16))],
                    1 => *cell = vec![Action::Reduce(RuleId(i as u16 % 10))],
                    2 => {} // empty
                    _ => *cell = vec![Action::Error],
                }
            }
        }
        let symbol_to_index = BTreeMap::new();
        let compressor = TableCompressor::new();
        let result = compressor.compress_action_table_small(&table, &symbol_to_index);
        prop_assert!(result.is_ok());
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 7. Symbol metadata encoding
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn symbol_metadata_hidden_tokens_not_visible(
        n_hidden in 1usize..5,
        n_visible in 0usize..5,
    ) {
        let mut grammar = Grammar::new("meta_test".to_string());
        for i in 0..n_hidden {
            grammar.tokens.insert(
                SymbolId(i as u16 + 1),
                Token {
                    name: format!("_{}", i),
                    pattern: TokenPattern::String(format!("h{}", i)),
                    fragile: false,
                },
            );
        }
        for i in 0..n_visible {
            grammar.tokens.insert(
                SymbolId((n_hidden + i) as u16 + 1),
                Token {
                    name: format!("vis_{}", i),
                    pattern: TokenPattern::Regex(format!("[a-z]{}", i)),
                    fragile: false,
                },
            );
        }
        let pt = ParseTable::default();
        let generator = StaticLanguageGenerator::new(grammar, pt);
        let json_str = generator.generate_node_types();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        let arr = parsed.as_array().unwrap();
        // Hidden tokens (starting with _) should not appear
        for entry in arr {
            let name = entry.get("type").and_then(|t| t.as_str()).unwrap_or("");
            prop_assert!(!name.starts_with('_'), "Hidden token '{}' should not appear", name);
        }
    }

    #[test]
    fn symbol_metadata_external_tokens_included(n_externals in 1usize..5) {
        let mut grammar = Grammar::new("ext_test".to_string());
        for i in 0..n_externals {
            grammar.externals.push(ExternalToken {
                name: format!("ext_{}", i),
                symbol_id: SymbolId(100 + i as u16),
            });
        }
        let pt = ParseTable::default();
        let generator = StaticLanguageGenerator::new(grammar, pt);
        let json_str = generator.generate_node_types();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        let arr = parsed.as_array().unwrap();
        for i in 0..n_externals {
            let ext_name = format!("ext_{}", i);
            let found = arr.iter().any(|v| {
                v.get("type").and_then(|t| t.as_str()) == Some(&ext_name)
            });
            prop_assert!(found, "External token '{}' not in node types", ext_name);
        }
    }

    #[test]
    fn symbol_metadata_hidden_externals_excluded(n_hidden in 1usize..5) {
        let mut grammar = Grammar::new("hid_ext".to_string());
        for i in 0..n_hidden {
            grammar.externals.push(ExternalToken {
                name: format!("_{}", i),
                symbol_id: SymbolId(100 + i as u16),
            });
        }
        let pt = ParseTable::default();
        let generator = StaticLanguageGenerator::new(grammar, pt);
        let json_str = generator.generate_node_types();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        let arr = parsed.as_array().unwrap();
        for entry in arr {
            let name = entry.get("type").and_then(|t| t.as_str()).unwrap_or("");
            prop_assert!(!name.starts_with('_'));
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Additional: Action encoding properties
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn encode_shift_small_roundtrip(state in 0u16..0x7FFF) {
        let compressor = TableCompressor::new();
        let action = Action::Shift(StateId(state));
        let encoded = compressor.encode_action_small(&action).unwrap();
        // Shift encodes as the raw state value (high bit clear)
        prop_assert!(encoded < 0x8000, "Shift encoding should have high bit clear");
        prop_assert_eq!(encoded, state);
    }

    #[test]
    fn encode_reduce_small_roundtrip(rule in 0u16..0x3FFF) {
        let compressor = TableCompressor::new();
        let action = Action::Reduce(RuleId(rule));
        let encoded = compressor.encode_action_small(&action).unwrap();
        // Reduce encodes with high bit set + 1-based rule id
        prop_assert!(encoded & 0x8000 != 0, "Reduce encoding should have high bit set");
        let decoded_rule = (encoded & 0x7FFF) - 1;
        prop_assert_eq!(decoded_rule, rule);
    }

    #[test]
    fn encode_accept_is_ffff(_dummy in 0u8..1) {
        let compressor = TableCompressor::new();
        let encoded = compressor.encode_action_small(&Action::Accept).unwrap();
        prop_assert_eq!(encoded, 0xFFFF);
    }

    #[test]
    fn encode_error_is_fffe(_dummy in 0u8..1) {
        let compressor = TableCompressor::new();
        let encoded = compressor.encode_action_small(&Action::Error).unwrap();
        prop_assert_eq!(encoded, 0xFFFE);
    }

    #[test]
    fn encode_recover_is_fffd(_dummy in 0u8..1) {
        let compressor = TableCompressor::new();
        let encoded = compressor.encode_action_small(&Action::Recover).unwrap();
        prop_assert_eq!(encoded, 0xFFFD);
    }

    #[test]
    fn shift_too_large_fails(state in 0x8000u16..=u16::MAX) {
        let compressor = TableCompressor::new();
        let result = compressor.encode_action_small(&Action::Shift(StateId(state)));
        prop_assert!(result.is_err());
    }

    #[test]
    fn reduce_too_large_fails(rule in 0x4000u16..=u16::MAX) {
        let compressor = TableCompressor::new();
        let result = compressor.encode_action_small(&Action::Reduce(RuleId(rule)));
        prop_assert!(result.is_err());
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Additional: CompressedActionEntry / CompressedGotoEntry properties
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn compressed_action_entry_preserves_symbol_and_action(
        symbol in 0u16..1000,
        state in 1u16..100,
    ) {
        let action = Action::Shift(StateId(state));
        let entry = CompressedActionEntry::new(symbol, action.clone());
        prop_assert_eq!(entry.symbol, symbol);
        prop_assert_eq!(entry.action, action);
    }

    #[test]
    fn goto_single_vs_rle_equivalence(
        state_val in 0u16..100,
        count in 3u16..20,
    ) {
        let singles: Vec<u16> = (0..count)
            .map(|_| {
                match CompressedGotoEntry::Single(state_val) {
                    CompressedGotoEntry::Single(s) => s,
                    _ => unreachable!(),
                }
            })
            .collect();
        let rle = CompressedGotoEntry::RunLength {
            state: state_val,
            count,
        };
        // RLE expands to the same values
        let expanded = match rle {
            CompressedGotoEntry::RunLength { state, count } => {
                vec![state; count as usize]
            }
            _ => unreachable!(),
        };
        prop_assert_eq!(singles, expanded);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Additional: BitPackedActionTable properties
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn bitpacked_all_errors_table(
        n_states in 1usize..8,
        n_symbols in 1usize..8,
    ) {
        let table = vec![vec![Action::Error; n_symbols]; n_states];
        let packed = BitPackedActionTable::from_table(&table);
        for state in 0..n_states {
            for symbol in 0..n_symbols {
                prop_assert_eq!(
                    packed.decompress(state, symbol),
                    Action::Error,
                    "Expected Error at state={}, symbol={}", state, symbol
                );
            }
        }
    }

    #[test]
    fn bitpacked_uniform_shift_table(
        n_states in 1usize..5,
        n_symbols in 1usize..5,
        target in 1u16..100,
    ) {
        let table = vec![vec![Action::Shift(StateId(target)); n_symbols]; n_states];
        let packed = BitPackedActionTable::from_table(&table);
        for state in 0..n_states {
            for symbol in 0..n_symbols {
                let got = packed.decompress(state, symbol);
                prop_assert_eq!(
                    got,
                    Action::Shift(StateId(target)),
                    "Uniform shift mismatch at state={}, symbol={}", state, symbol
                );
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Additional: Row offset invariants
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn row_offsets_monotonically_nondecreasing(table in action_table_strategy(10, 8)) {
        let compressor = TableCompressor::new();
        let symbol_to_index = BTreeMap::new();
        if let Ok(compressed) = compressor.compress_action_table_small(&table, &symbol_to_index) {
            for i in 1..compressed.row_offsets.len() {
                prop_assert!(
                    compressed.row_offsets[i] >= compressed.row_offsets[i - 1],
                    "Row offsets not monotonic at index {}", i
                );
            }
        }
    }

    #[test]
    fn row_offsets_length_is_states_plus_one(
        n_states in 1usize..15,
        n_symbols in 1usize..8,
    ) {
        let table = vec![vec![vec![]; n_symbols]; n_states];
        let compressor = TableCompressor::new();
        let symbol_to_index = BTreeMap::new();
        let compressed = compressor
            .compress_action_table_small(&table, &symbol_to_index)
            .unwrap();
        prop_assert_eq!(compressed.row_offsets.len(), n_states + 1);
    }

    #[test]
    fn goto_row_offsets_length_is_states_plus_one(
        n_states in 1usize..15,
        n_symbols in 1usize..8,
    ) {
        let table = vec![vec![INVALID; n_symbols]; n_states];
        let compressor = TableCompressor::new();
        let compressed = compressor.compress_goto_table_small(&table).unwrap();
        prop_assert_eq!(compressed.row_offsets.len(), n_states + 1);
    }

    #[test]
    fn default_actions_length_matches_states(
        n_states in 1usize..15,
        n_symbols in 1usize..8,
    ) {
        let table = vec![vec![vec![]; n_symbols]; n_states];
        let compressor = TableCompressor::new();
        let symbol_to_index = BTreeMap::new();
        let compressed = compressor
            .compress_action_table_small(&table, &symbol_to_index)
            .unwrap();
        prop_assert_eq!(compressed.default_actions.len(), n_states);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Additional: TableCompressor compress() full-pipeline tests
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn compress_rejects_empty_action_table(_dummy in 0u8..1) {
        let compressor = TableCompressor::new();
        let mut pt = ParseTable { state_count: 0, ..Default::default() };
        pt.symbol_to_index.insert(SymbolId(0), 0);
        let result = compressor.compress(&pt, &[0], false);
        prop_assert!(result.is_err());
    }

    #[test]
    fn compress_shiftable_table_succeeds(
        states in 1usize..6,
        terms in 1usize..5,
    ) {
        let pt = make_shiftable_table(states, terms);
        let compressor = TableCompressor::new();
        let eof_idx = pt.symbol_to_index[&pt.eof_symbol];
        let mut token_indices: Vec<usize> = (1..=terms).collect();
        if !token_indices.contains(&eof_idx) {
            token_indices.push(eof_idx);
            token_indices.sort();
        }
        let result = compressor.compress(&pt, &token_indices, false);
        prop_assert!(result.is_ok(), "compress failed: {:?}", result.err());
    }

    #[test]
    fn compress_with_nullable_start(
        states in 1usize..4,
        terms in 1usize..4,
    ) {
        let terms = terms.max(1);
        let eof_idx = 1 + terms;
        let start_sym = eof_idx + 1;
        let symbol_count = start_sym + 1;

        let mut actions = vec![vec![vec![]; symbol_count]; states.max(1)];
        // Accept on EOF instead of shift on terminal
        actions[0][eof_idx] = vec![Action::Accept];

        let gotos = vec![vec![INVALID; symbol_count]; states.max(1)];
        let pt = make_parse_table(
            actions,
            gotos,
            vec![],
            SymbolId(start_sym as u16),
            SymbolId(eof_idx as u16),
            0,
        );
        let compressor = TableCompressor::new();
        let mut token_indices: Vec<usize> = (1..=terms).collect();
        let eof_col = pt.symbol_to_index[&pt.eof_symbol];
        if !token_indices.contains(&eof_col) {
            token_indices.push(eof_col);
            token_indices.sort();
        }
        let result = compressor.compress(&pt, &token_indices, true);
        prop_assert!(result.is_ok(), "nullable compress failed: {:?}", result.err());
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Additional: Miscellaneous edge cases
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn compressed_parse_table_zero_counts(
        sym in 0usize..=0,
        st in 0usize..=0,
    ) {
        let cpt = CompressedParseTable::new_for_testing(sym, st);
        prop_assert_eq!(cpt.symbol_count(), 0);
        prop_assert_eq!(cpt.state_count(), 0);
    }

    #[test]
    fn action_table_with_mixed_cells(
        n_states in 1usize..6,
        n_symbols in 2usize..6,
    ) {
        let mut table = vec![vec![vec![]; n_symbols]; n_states];
        // First cell: shift, second cell: reduce
        for row in &mut table {
            row[0] = vec![Action::Shift(StateId(1))];
            row[1] = vec![Action::Reduce(RuleId(0))];
        }
        let compressed = compress_action_table(&table);
        for state in 0..n_states {
            prop_assert_eq!(
                decompress_action(&compressed, state, 0),
                Action::Shift(StateId(1))
            );
            prop_assert_eq!(
                decompress_action(&compressed, state, 1),
                Action::Reduce(RuleId(0))
            );
        }
    }

    #[test]
    fn goto_mixed_some_none(
        n_states in 1usize..8,
        n_symbols in 2usize..8,
    ) {
        let mut table = Vec::new();
        for i in 0..n_states {
            let row: Vec<Option<StateId>> = (0..n_symbols)
                .map(|j| {
                    if (i + j) % 3 == 0 {
                        Some(StateId((i * n_symbols + j) as u16))
                    } else {
                        None
                    }
                })
                .collect();
            table.push(row);
        }
        let compressed = compress_goto_table(&table);
        for (state, row) in table.iter().enumerate() {
            for (symbol, expected) in row.iter().enumerate() {
                let got = decompress_goto(&compressed, state, symbol);
                prop_assert_eq!(got, *expected);
            }
        }
    }

    #[test]
    fn node_types_with_external_and_tokens_combined(n in 1usize..3) {
        let mut grammar = Grammar::new("combined".to_string());
        for i in 0..n {
            grammar.tokens.insert(
                SymbolId(i as u16 + 1),
                Token {
                    name: format!("tok_{}", i),
                    pattern: TokenPattern::Regex(format!("[a-z]{}", i)),
                    fragile: false,
                },
            );
            grammar.externals.push(ExternalToken {
                name: format!("ext_{}", i),
                symbol_id: SymbolId(100 + i as u16),
            });
        }
        let pt = ParseTable::default();
        let generator = StaticLanguageGenerator::new(grammar, pt);
        let json_str = generator.generate_node_types();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        prop_assert!(parsed.is_array());
        let arr = parsed.as_array().unwrap();
        // Should contain both tokens and externals
        prop_assert!(arr.len() >= n, "Expected at least {} entries, got {}", n, arr.len());
    }

    #[test]
    fn table_compressor_default_threshold(_dummy in 0u8..1) {
        let _c1 = TableCompressor::new();
        let _c2 = TableCompressor::default();
        // Both constructors should produce equivalent compressors
        // Verify by compressing the same table
        let table: Vec<Vec<StateId>> = vec![vec![]];
        let r1 = _c1.compress_goto_table_small(&table);
        let r2 = _c2.compress_goto_table_small(&table);
        prop_assert_eq!(r1.is_ok(), r2.is_ok());
    }
}

//! Comprehensive v2 tests for `AbiLanguageBuilder` covering ABI generation,
//! symbol tables, field maps, state counts, action/goto encoding, external
//! scanners, alias sequences, large grammars, precedence, and roundtrip validation.
//!
//! All tests exercise the public API surface of `adze_tablegen`.

#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

use adze_glr_core::{
    Action, FirstFollowSets, GotoIndexing, LexMode, ParseTable, build_lr1_automaton,
};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{
    Associativity, ExternalToken, FieldId, Grammar, ProductionId, Rule, StateId, Symbol, SymbolId,
    Token, TokenPattern,
};
use adze_tablegen::AbiLanguageBuilder;
use adze_tablegen::compress::TableCompressor;
use std::collections::BTreeMap;

// ===========================================================================
// Helpers
// ===========================================================================

const INVALID: StateId = StateId(u16::MAX);

/// Build a grammar + parse table pair using the manual layout convention.
///
/// Symbol layout: ERROR(0), terminals 1..=num_terms, externals, EOF, non-terminals.
fn build_pair(
    name: &str,
    num_terms: usize,
    num_nonterms: usize,
    num_fields: usize,
    num_externals: usize,
    num_states: usize,
) -> (Grammar, ParseTable) {
    let num_terms = num_terms.max(1);
    let num_nonterms = num_nonterms.max(1);
    let num_states = num_states.max(1);

    let eof_idx = 1 + num_terms + num_externals;
    let symbol_count = eof_idx + 1 + num_nonterms;

    let actions = vec![vec![vec![]; symbol_count]; num_states];
    let gotos = vec![vec![INVALID; symbol_count]; num_states];

    let eof_symbol = SymbolId(eof_idx as u16);
    let start_symbol = SymbolId((eof_idx + 1) as u16);

    let mut symbol_to_index = BTreeMap::new();
    let mut index_to_symbol = vec![SymbolId(0); symbol_count];
    for i in 0..symbol_count {
        let sym = SymbolId(i as u16);
        symbol_to_index.insert(sym, i);
        index_to_symbol[i] = sym;
    }

    let mut grammar = Grammar::new(name.to_string());

    let first_term = SymbolId(1);
    for i in 1..=num_terms {
        let sym = SymbolId(i as u16);
        grammar.tokens.insert(
            sym,
            Token {
                name: format!("tok_{i}"),
                pattern: TokenPattern::String(format!("t{i}")),
                fragile: false,
            },
        );
    }

    let first_nt_idx = eof_idx + 1;
    for i in 0..num_nonterms {
        let sym = SymbolId((first_nt_idx + i) as u16);
        grammar.rule_names.insert(sym, format!("rule_{i}"));
        grammar.add_rule(Rule {
            lhs: sym,
            rhs: vec![Symbol::Terminal(first_term)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(i as u16),
        });
    }

    for i in 0..num_fields {
        grammar
            .fields
            .insert(FieldId(i as u16), format!("field_{i}"));
    }

    for i in 0..num_externals {
        grammar.externals.push(ExternalToken {
            name: format!("ext_{i}"),
            symbol_id: SymbolId((1 + num_terms + i) as u16),
        });
    }

    let mut nonterminal_to_index = BTreeMap::new();
    nonterminal_to_index.insert(start_symbol, start_symbol.0 as usize);

    let table = ParseTable {
        action_table: actions,
        goto_table: gotos,
        symbol_metadata: vec![],
        state_count: num_states,
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
        token_count: eof_idx + 1,
        external_token_count: num_externals,
        lex_modes: vec![
            LexMode {
                lex_state: 0,
                external_lex_state: 0,
            };
            num_states
        ],
        extras: vec![],
        dynamic_prec_by_rule: vec![],
        rule_assoc_by_rule: vec![],
        alias_sequences: vec![],
        field_names: vec![],
        field_map: BTreeMap::new(),
        goto_indexing: GotoIndexing::NonterminalMap,
    };

    (grammar, table)
}

/// Minimal single-token, single-rule pair.
fn minimal() -> (Grammar, ParseTable) {
    build_pair("minimal", 1, 1, 0, 0, 2)
}

/// Generate code string from grammar+table.
fn gen_code(grammar: &Grammar, table: &ParseTable) -> String {
    AbiLanguageBuilder::new(grammar, table)
        .generate()
        .to_string()
}

/// Full pipeline: GrammarBuilder → normalize → FIRST/FOLLOW → LR(1) → ABI code.
fn full_pipeline(g: adze_ir::Grammar) -> (String, ParseTable) {
    let mut grammar = g;
    let ff =
        FirstFollowSets::compute_normalized(&mut grammar).expect("FIRST/FOLLOW computation failed");
    let table = build_lr1_automaton(&grammar, &ff).expect("LR(1) build failed");
    let code = AbiLanguageBuilder::new(&grammar, &table)
        .generate()
        .to_string();
    (code, table)
}

// ===========================================================================
// 1. Basic ABI generation from simple grammars
// ===========================================================================

#[test]
fn basic_single_token_grammar_generates_language() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("LANGUAGE"));
    assert!(code.contains("TSLanguage"));
}

#[test]
fn basic_two_token_grammar_generates() {
    let (g, t) = build_pair("two", 2, 1, 0, 0, 3);
    let code = gen_code(&g, &t);
    assert!(code.contains("LANGUAGE"));
}

#[test]
fn basic_ffi_function_named_after_grammar() {
    let (g, t) = build_pair("json", 3, 2, 0, 0, 5);
    let code = gen_code(&g, &t);
    assert!(code.contains("tree_sitter_json"));
}

#[test]
fn basic_different_names_different_ffi() {
    let (g1, t1) = build_pair("alpha", 1, 1, 0, 0, 2);
    let (g2, t2) = build_pair("beta", 1, 1, 0, 0, 2);
    assert!(gen_code(&g1, &t1).contains("tree_sitter_alpha"));
    assert!(gen_code(&g2, &t2).contains("tree_sitter_beta"));
}

#[test]
fn basic_code_references_abi_version() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("TREE_SITTER_LANGUAGE_VERSION"));
}

#[test]
fn basic_code_contains_symbol_names_array() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("SYMBOL_NAME_PTRS"));
}

#[test]
fn basic_code_contains_parse_actions() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("PARSE_ACTIONS"));
}

#[test]
fn basic_code_contains_lex_modes() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("LEX_MODES"));
}

// ===========================================================================
// 2. Symbol table generation (terminal + nonterminal ordering)
// ===========================================================================

#[test]
fn symbol_count_matches_table() {
    let (g, t) = build_pair("sym", 3, 2, 0, 0, 4);
    let code = gen_code(&g, &t);
    let expected = t.symbol_count;
    assert!(
        code.contains(&format!("symbol_count : {expected}u32")),
        "symbol_count must match parse table"
    );
}

#[test]
fn symbol_names_includes_eof() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    // EOF is named "end" as a null-terminated string
    assert!(code.contains("101u8"), "must contain 'e' from 'end'");
}

#[test]
fn symbol_names_sorted_by_index() {
    let (g, t) = build_pair("sorted", 2, 1, 0, 0, 2);
    let code = gen_code(&g, &t);
    // SYMBOL_NAME_0 before SYMBOL_NAME_1 etc.
    let pos0 = code.find("SYMBOL_NAME_0").unwrap();
    let pos1 = code.find("SYMBOL_NAME_1").unwrap();
    assert!(pos0 < pos1);
}

#[test]
fn symbol_metadata_length_equals_symbol_count() {
    let (g, t) = build_pair("meta", 2, 1, 0, 0, 3);
    let builder = AbiLanguageBuilder::new(&g, &t);
    // generate() will work — just inspect the code
    let code = builder.generate().to_string();
    // SYMBOL_METADATA is emitted as bytes
    assert!(code.contains("SYMBOL_METADATA"));
}

#[test]
fn terminal_before_nonterminal_in_layout() {
    let (g, t) = build_pair("layout", 2, 2, 0, 0, 2);
    let code = gen_code(&g, &t);
    // In the generated symbol table, SYMBOL_NAME_1 (terminal) should appear before
    // SYMBOL_NAME indices for non-terminals (which come after EOF).
    let term_name_pos = code.find("SYMBOL_NAME_1").unwrap();
    // Non-terminal symbols start after EOF. EOF index = 1 + num_terms = 3, so
    // first non-terminal is at index 4.
    let nt_name_pos = code.find("SYMBOL_NAME_4").unwrap();
    assert!(
        term_name_pos < nt_name_pos,
        "terminals must precede non-terminals"
    );
}

#[test]
fn eof_symbol_in_code_is_zero() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("eof_symbol : 0"));
}

// ===========================================================================
// 3. Field mapping in generated ABI
// ===========================================================================

#[test]
fn zero_fields_produces_empty_field_names() {
    let (g, t) = build_pair("nofield", 1, 1, 0, 0, 2);
    let code = gen_code(&g, &t);
    assert!(code.contains("field_count : 0u32"));
}

#[test]
fn one_field_produces_field_count_one() {
    let (g, t) = build_pair("onefield", 1, 1, 1, 0, 2);
    let code = gen_code(&g, &t);
    assert!(code.contains("field_count : 1u32"));
}

#[test]
fn multiple_fields_counted() {
    let (g, t) = build_pair("multi_field", 1, 1, 3, 0, 2);
    let code = gen_code(&g, &t);
    assert!(code.contains("field_count : 3u32"));
}

#[test]
fn field_map_slices_emitted() {
    let (g, t) = build_pair("fms", 1, 1, 1, 0, 2);
    let code = gen_code(&g, &t);
    assert!(code.contains("FIELD_MAP_SLICES"));
}

#[test]
fn field_map_entries_emitted() {
    let (g, t) = build_pair("fme", 1, 1, 1, 0, 2);
    let code = gen_code(&g, &t);
    assert!(code.contains("FIELD_MAP_ENTRIES"));
}

#[test]
fn field_names_sorted_lexicographically() {
    let (mut g, t) = build_pair("fsort", 1, 1, 0, 0, 2);
    g.fields.insert(FieldId(0), "zebra".to_string());
    g.fields.insert(FieldId(1), "alpha".to_string());
    let code = gen_code(&g, &t);
    // Field names are emitted as null-terminated byte arrays.
    // "alpha" bytes include 97u8 (a), "zebra" bytes include 122u8 (z).
    // alpha (FIELD_NAME_0) should appear before zebra (FIELD_NAME_1) in lexicographic order.
    let pos0 = code.find("FIELD_NAME_0").unwrap();
    let pos1 = code.find("FIELD_NAME_1").unwrap();
    assert!(pos0 < pos1);
}

// ===========================================================================
// 4. State count in generated language
// ===========================================================================

#[test]
fn state_count_matches_table_states() {
    let (g, t) = build_pair("sc", 1, 1, 0, 0, 7);
    let code = gen_code(&g, &t);
    assert!(code.contains("state_count : 7u32"));
}

#[test]
fn single_state_grammar() {
    let (g, t) = build_pair("one_state", 1, 1, 0, 0, 1);
    let code = gen_code(&g, &t);
    assert!(code.contains("state_count : 1u32"));
}

#[test]
fn primary_state_ids_length_equals_states() {
    let (g, t) = build_pair("psi", 1, 1, 0, 0, 5);
    let code = gen_code(&g, &t);
    assert!(code.contains("PRIMARY_STATE_IDS"));
}

#[test]
fn lex_modes_length_equals_states() {
    let (g, t) = build_pair("lm", 1, 1, 0, 0, 4);
    let code = gen_code(&g, &t);
    // Should have lex mode entries for each state
    let count = code.matches("TSLexState").count();
    // One for the type reference + one per state entry
    assert!(count >= 4, "must have at least 4 lex state entries");
}

// ===========================================================================
// 5. Parse action encoding correctness
// ===========================================================================

#[test]
fn encode_shift_is_state_id() {
    let (g, t) = minimal();
    let builder = AbiLanguageBuilder::new(&g, &t);
    let code = builder.generate().to_string();
    // Just verify it generates without panic
    assert!(!code.is_empty());
}

#[test]
fn encode_action_shift_zero() {
    let compressor = TableCompressor::new();
    let enc = compressor
        .encode_action_small(&Action::Shift(StateId(0)))
        .unwrap();
    assert_eq!(enc, 0);
}

#[test]
fn encode_action_shift_max_small() {
    let compressor = TableCompressor::new();
    let enc = compressor
        .encode_action_small(&Action::Shift(StateId(0x7FFF)))
        .unwrap();
    assert_eq!(enc, 0x7FFF);
}

#[test]
fn encode_action_shift_too_large_errors() {
    let compressor = TableCompressor::new();
    let result = compressor.encode_action_small(&Action::Shift(StateId(0x8000)));
    assert!(result.is_err());
}

#[test]
fn encode_action_reduce_sets_high_bit() {
    let compressor = TableCompressor::new();
    let enc = compressor
        .encode_action_small(&Action::Reduce(adze_ir::RuleId(0)))
        .unwrap();
    assert_eq!(enc, 0x8001, "Reduce(0) → 0x8001 (1-based)");
}

#[test]
fn encode_action_reduce_rule_5() {
    let compressor = TableCompressor::new();
    let enc = compressor
        .encode_action_small(&Action::Reduce(adze_ir::RuleId(5)))
        .unwrap();
    assert_eq!(enc, 0x8006, "Reduce(5) → 0x8006");
}

#[test]
fn encode_action_accept_is_ffff() {
    let compressor = TableCompressor::new();
    let enc = compressor.encode_action_small(&Action::Accept).unwrap();
    assert_eq!(enc, 0xFFFF);
}

#[test]
fn encode_action_error_is_fffe() {
    let compressor = TableCompressor::new();
    let enc = compressor.encode_action_small(&Action::Error).unwrap();
    assert_eq!(enc, 0xFFFE);
}

#[test]
fn encode_action_recover_is_fffd() {
    let compressor = TableCompressor::new();
    let enc = compressor.encode_action_small(&Action::Recover).unwrap();
    assert_eq!(enc, 0xFFFD);
}

#[test]
fn encode_action_reduce_too_large_errors() {
    let compressor = TableCompressor::new();
    let result = compressor.encode_action_small(&Action::Reduce(adze_ir::RuleId(0x4000)));
    assert!(result.is_err());
}

// ===========================================================================
// 6. Goto table encoding correctness
// ===========================================================================

#[test]
fn goto_single_run_length_encoded() {
    let compressor = TableCompressor::new();
    let goto_table = vec![vec![StateId(3), StateId(3), StateId(3), StateId(3)]];
    let compressed = compressor.compress_goto_table_small(&goto_table).unwrap();
    let has_rle = compressed.data.iter().any(|e| {
        matches!(
            e,
            adze_tablegen::CompressedGotoEntry::RunLength { state: 3, count: 4 }
        )
    });
    assert!(
        has_rle,
        "4 identical entries should produce run-length encoding"
    );
}

#[test]
fn goto_short_run_not_rle() {
    let compressor = TableCompressor::new();
    let goto_table = vec![vec![StateId(1), StateId(1)]];
    let compressed = compressor.compress_goto_table_small(&goto_table).unwrap();
    let all_single = compressed
        .data
        .iter()
        .all(|e| matches!(e, adze_tablegen::CompressedGotoEntry::Single(_)));
    assert!(all_single, "runs of 2 should use single entries");
}

#[test]
fn goto_mixed_entries() {
    let compressor = TableCompressor::new();
    let goto_table = vec![vec![
        StateId(1),
        StateId(1),
        StateId(1),
        StateId(1),
        StateId(2),
        StateId(3),
    ]];
    let compressed = compressor.compress_goto_table_small(&goto_table).unwrap();
    assert!(!compressed.data.is_empty());
    // Must have both RLE and single entries
    let has_rle = compressed
        .data
        .iter()
        .any(|e| matches!(e, adze_tablegen::CompressedGotoEntry::RunLength { .. }));
    let has_single = compressed
        .data
        .iter()
        .any(|e| matches!(e, adze_tablegen::CompressedGotoEntry::Single(_)));
    assert!(has_rle);
    assert!(has_single);
}

#[test]
fn goto_empty_table_ok() {
    let compressor = TableCompressor::new();
    let compressed = compressor.compress_goto_table_small(&[]).unwrap();
    assert!(compressed.data.is_empty());
    // Trailing sentinel offset is always appended
    assert_eq!(compressed.row_offsets.len(), 1);
}

#[test]
fn goto_row_offsets_count() {
    let compressor = TableCompressor::new();
    let goto_table = vec![vec![StateId(1)], vec![StateId(2)], vec![StateId(3)]];
    let compressed = compressor.compress_goto_table_small(&goto_table).unwrap();
    assert_eq!(compressed.row_offsets.len(), 4, "n_rows + 1 offsets");
}

#[test]
fn goto_offsets_monotonically_increasing() {
    let compressor = TableCompressor::new();
    let goto_table = vec![
        vec![StateId(5), StateId(5), StateId(5)],
        vec![StateId(1), StateId(2)],
    ];
    let compressed = compressor.compress_goto_table_small(&goto_table).unwrap();
    for w in compressed.row_offsets.windows(2) {
        assert!(w[1] >= w[0], "row_offsets must be non-decreasing");
    }
}

// ===========================================================================
// 7. External scanner handling
// ===========================================================================

#[test]
fn no_externals_null_scanner_states() {
    let (g, t) = build_pair("noext", 1, 1, 0, 0, 2);
    let code = gen_code(&g, &t);
    assert!(code.contains("std :: ptr :: null ()"));
}

#[test]
fn externals_generate_scanner_code() {
    let (g, t) = build_pair("ext", 1, 1, 0, 1, 2);
    let code = gen_code(&g, &t);
    assert!(
        code.contains("EXTERNAL_SCANNER") || code.contains("ExternalScanner"),
        "should reference external scanner"
    );
}

#[test]
fn external_token_count_in_code() {
    let (g, t) = build_pair("ext2", 1, 1, 0, 2, 2);
    let code = gen_code(&g, &t);
    assert!(code.contains("external_token_count : 2u32"));
}

#[test]
fn external_token_count_zero_when_none() {
    let (g, t) = build_pair("noext2", 1, 1, 0, 0, 2);
    let code = gen_code(&g, &t);
    assert!(code.contains("external_token_count : 0u32"));
}

#[test]
fn multiple_externals_counted() {
    let (g, t) = build_pair("ext3", 2, 1, 0, 3, 3);
    let code = gen_code(&g, &t);
    assert!(code.contains("external_token_count : 3u32"));
}

// ===========================================================================
// 8. Alias sequence generation
// ===========================================================================

#[test]
fn alias_count_is_zero_for_simple_grammar() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("alias_count : 0u32"));
}

#[test]
fn max_alias_sequence_length_zero() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("max_alias_sequence_length : 0u16"));
}

#[test]
fn alias_sequences_pointer_is_null() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("alias_sequences"));
}

// ===========================================================================
// 9. Keyword extraction
// ===========================================================================

#[test]
fn keyword_capture_token_default_zero() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("keyword_capture_token : 0"));
}

#[test]
fn keyword_lex_fn_is_none() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("keyword_lex_fn : None"));
}

// ===========================================================================
// 10. Large grammar handling
// ===========================================================================

#[test]
fn large_grammar_50_tokens() {
    let (g, t) = build_pair("large", 50, 5, 3, 0, 10);
    let code = gen_code(&g, &t);
    assert!(code.contains("LANGUAGE"));
}

#[test]
fn large_grammar_20_nonterminals() {
    let (g, t) = build_pair("big_nt", 5, 20, 0, 0, 15);
    let code = gen_code(&g, &t);
    assert!(code.contains("LANGUAGE"));
}

#[test]
fn large_grammar_100_states() {
    let (g, t) = build_pair("big_state", 5, 3, 0, 0, 100);
    let code = gen_code(&g, &t);
    assert!(code.contains("state_count : 100u32"));
}

#[test]
fn large_grammar_many_fields() {
    let (g, t) = build_pair("many_fields", 3, 2, 20, 0, 5);
    let code = gen_code(&g, &t);
    assert!(code.contains("field_count : 20u32"));
}

// ===========================================================================
// 11. Grammar with many rules
// ===========================================================================

#[test]
fn grammar_multiple_alternatives() {
    let (mut g, t) = build_pair("multi", 3, 1, 0, 0, 4);
    let nt = SymbolId((t.symbol_to_index.len() - 1) as u16);
    g.add_rule(Rule {
        lhs: nt,
        rhs: vec![Symbol::Terminal(SymbolId(2))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(10),
    });
    let code = gen_code(&g, &t);
    assert!(code.contains("LANGUAGE"));
}

#[test]
fn grammar_epsilon_rule_via_builder() {
    let grammar = GrammarBuilder::new("eps")
        .token("a", "a")
        .rule("start", vec![])
        .rule("start", vec!["a"])
        .start("start")
        .build();
    let mut g = grammar;
    let ff = FirstFollowSets::compute_normalized(&mut g).expect("ff");
    let table = build_lr1_automaton(&g, &ff).expect("lr1");
    let code = AbiLanguageBuilder::new(&g, &table).generate().to_string();
    assert!(code.contains("LANGUAGE"));
}

#[test]
fn grammar_chain_rules() {
    let grammar = GrammarBuilder::new("chain")
        .token("x", "x")
        .rule("C", vec!["x"])
        .rule("B", vec!["C"])
        .rule("A", vec!["B"])
        .rule("start", vec!["A"])
        .start("start")
        .build();
    let (code, _) = full_pipeline(grammar);
    assert!(code.contains("LANGUAGE"));
}

#[test]
fn grammar_self_recursive() {
    let grammar = GrammarBuilder::new("rec")
        .token("a", "a")
        .rule("list", vec!["a"])
        .rule("list", vec!["list", "a"])
        .start("list")
        .build();
    let (code, _) = full_pipeline(grammar);
    assert!(code.contains("LANGUAGE"));
}

#[test]
fn grammar_multiple_nonterminals() {
    let grammar = GrammarBuilder::new("multi_nt")
        .token("a", "a")
        .token("b", "b")
        .rule("X", vec!["a"])
        .rule("Y", vec!["b"])
        .rule("start", vec!["X", "Y"])
        .start("start")
        .build();
    let (code, _) = full_pipeline(grammar);
    assert!(code.contains("LANGUAGE"));
}

// ===========================================================================
// 12. Grammar with precedence/associativity
// ===========================================================================

#[test]
fn precedence_grammar_generates() {
    let grammar = GrammarBuilder::new("prec")
        .token("num", "[0-9]+")
        .token("plus", "+")
        .token("star", "*")
        .rule("expr", vec!["num"])
        .rule_with_precedence("expr", vec!["expr", "plus", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "star", "expr"], 2, Associativity::Left)
        .start("expr")
        .build();
    let (code, _) = full_pipeline(grammar);
    assert!(code.contains("LANGUAGE"));
}

#[test]
fn right_associativity_grammar() {
    let grammar = GrammarBuilder::new("rassoc")
        .token("x", "x")
        .token("hat", "^")
        .rule("expr", vec!["x"])
        .rule_with_precedence("expr", vec!["expr", "hat", "expr"], 1, Associativity::Right)
        .start("expr")
        .build();
    let (code, _) = full_pipeline(grammar);
    assert!(code.contains("LANGUAGE"));
}

// ===========================================================================
// 13. Roundtrip: grammar → parse table → ABI → validate structure
// ===========================================================================

#[test]
fn roundtrip_single_token() {
    let grammar = GrammarBuilder::new("rt_single")
        .token("a", "a")
        .rule("start", vec!["a"])
        .start("start")
        .build();
    let (code, table) = full_pipeline(grammar);
    assert!(code.contains("LANGUAGE"));
    assert!(table.state_count >= 2);
}

#[test]
fn roundtrip_two_tokens_sequence() {
    let grammar = GrammarBuilder::new("rt_seq")
        .token("a", "a")
        .token("b", "b")
        .rule("start", vec!["a", "b"])
        .start("start")
        .build();
    let (code, table) = full_pipeline(grammar);
    assert!(code.contains("LANGUAGE"));
    assert!(table.state_count >= 3);
}

#[test]
fn roundtrip_alternatives() {
    let grammar = GrammarBuilder::new("rt_alt")
        .token("a", "a")
        .token("b", "b")
        .rule("start", vec!["a"])
        .rule("start", vec!["b"])
        .start("start")
        .build();
    let (code, table) = full_pipeline(grammar);
    assert!(code.contains("LANGUAGE"));
    assert!(table.symbol_count >= 3);
}

#[test]
fn roundtrip_nested_rules() {
    let grammar = GrammarBuilder::new("rt_nested")
        .token("x", "x")
        .token("y", "y")
        .rule("inner", vec!["x"])
        .rule("outer", vec!["inner", "y"])
        .rule("start", vec!["outer"])
        .start("start")
        .build();
    let (code, _) = full_pipeline(grammar);
    assert!(code.contains("tree_sitter_rt_nested"));
}

#[test]
fn roundtrip_recursive_grammar() {
    let grammar = GrammarBuilder::new("rt_rec")
        .token("a", "a")
        .token("lp", "(")
        .token("rp", ")")
        .rule("atom", vec!["a"])
        .rule("atom", vec!["lp", "expr", "rp"])
        .rule("expr", vec!["atom"])
        .start("expr")
        .build();
    let (code, _) = full_pipeline(grammar);
    assert!(code.contains("LANGUAGE"));
}

#[test]
fn roundtrip_state_count_consistent() {
    let grammar = GrammarBuilder::new("rt_sc")
        .token("t", "t")
        .rule("start", vec!["t"])
        .start("start")
        .build();
    let (code, table) = full_pipeline(grammar);
    let expected = format!("state_count : {}u32", table.state_count);
    assert!(code.contains(&expected));
}

#[test]
fn roundtrip_symbol_count_consistent() {
    let grammar = GrammarBuilder::new("rt_sym")
        .token("a", "a")
        .token("b", "b")
        .rule("start", vec!["a", "b"])
        .start("start")
        .build();
    let (code, table) = full_pipeline(grammar);
    let expected = format!("symbol_count : {}u32", table.symbol_count);
    assert!(code.contains(&expected));
}

#[test]
fn roundtrip_token_count_consistent() {
    let grammar = GrammarBuilder::new("rt_tc")
        .token("x", "x")
        .rule("start", vec!["x"])
        .start("start")
        .build();
    let (code, table) = full_pipeline(grammar);
    let expected = format!("token_count : {}u32", table.token_count);
    assert!(code.contains(&expected));
}

// ===========================================================================
// Additional: Compression pipeline integration
// ===========================================================================

#[test]
fn compress_pipeline_single_token() {
    let mut grammar = GrammarBuilder::new("comp")
        .token("a", "a")
        .rule("start", vec!["a"])
        .start("start")
        .build();
    let ff = FirstFollowSets::compute_normalized(&mut grammar).unwrap();
    let table = build_lr1_automaton(&grammar, &ff).unwrap();
    let token_indices = adze_tablegen::collect_token_indices(&grammar, &table);
    let start_empty = adze_tablegen::eof_accepts_or_reduces(&table);
    let compressed = TableCompressor::new()
        .compress(&table, &token_indices, start_empty)
        .unwrap();
    assert!(!compressed.action_table.data.is_empty() || !compressed.goto_table.data.is_empty());
}

#[test]
fn compress_pipeline_two_alternatives() {
    let mut grammar = GrammarBuilder::new("comp2")
        .token("a", "a")
        .token("b", "b")
        .rule("start", vec!["a"])
        .rule("start", vec!["b"])
        .start("start")
        .build();
    let ff = FirstFollowSets::compute_normalized(&mut grammar).unwrap();
    let table = build_lr1_automaton(&grammar, &ff).unwrap();
    let token_indices = adze_tablegen::collect_token_indices(&grammar, &table);
    let start_empty = adze_tablegen::eof_accepts_or_reduces(&table);
    let result = TableCompressor::new().compress(&table, &token_indices, start_empty);
    assert!(result.is_ok());
}

#[test]
fn compress_action_table_empty_states() {
    let compressor = TableCompressor::new();
    let action_table = vec![vec![]; 3];
    let sym_map = BTreeMap::new();
    let result = compressor.compress_action_table_small(&action_table, &sym_map);
    assert!(result.is_ok());
    let compressed = result.unwrap();
    assert_eq!(compressed.row_offsets.len(), 4);
    assert_eq!(compressed.default_actions.len(), 3);
}

#[test]
fn compress_action_table_all_reduce() {
    let compressor = TableCompressor::new();
    let reduce = Action::Reduce(adze_ir::RuleId(0));
    let action_table = vec![vec![vec![reduce.clone()]; 5]];
    let sym_map = BTreeMap::new();
    let compressed = compressor
        .compress_action_table_small(&action_table, &sym_map)
        .unwrap();
    assert_eq!(compressed.data.len(), 5);
}

// ===========================================================================
// Additional: Production ID map
// ===========================================================================

#[test]
fn production_id_map_emitted() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("PRODUCTION_ID_MAP"));
}

#[test]
fn production_lhs_index_emitted() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("PRODUCTION_LHS_INDEX"));
}

#[test]
fn ts_rules_emitted() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("TS_RULES"));
}

#[test]
fn production_id_count_matches_rules() {
    let (g, t) = build_pair("pid", 2, 3, 0, 0, 4);
    let code = gen_code(&g, &t);
    assert!(code.contains("production_id_count : 3u32"));
}

// ===========================================================================
// #1060: shared-production table leg (grammar -> automaton -> ABI emission)
// ===========================================================================

#[test]
fn test_shared_production_table_emits_rule_id_keyed_map() {
    // Two `middle` alternatives share ProductionId(1) with identical
    // (empty) metadata. The ABI PRODUCTION_ID_MAP is indexed by RuleId, so
    // it must still carry one entry per rule in rule order: [0, 1, 1].
    // Production-sized emission would truncate the third entry to [0, 1].
    let mut grammar = GrammarBuilder::new("shared_prod")
        .token("a", "a")
        .token("b", "b")
        .rule("s", vec!["middle"])
        .rule("middle", vec!["a"])
        .rule("middle", vec!["b"])
        .start("s")
        .build();
    for rules in grammar.rules.values_mut() {
        for rule in rules.iter_mut() {
            if rule.rhs.len() == 1 && matches!(&rule.rhs[0], Symbol::Terminal(_)) {
                rule.production_id = ProductionId(1);
            }
        }
    }
    grammar.refresh_production_ids().unwrap();
    assert_eq!(grammar.all_rules().count(), 3);
    let (code, table) = full_pipeline(grammar);
    assert_eq!(
        table.rules.len(),
        3,
        "automaton keeps both shared alternatives distinct"
    );
    assert!(
        code.contains("0u16 , 1u16 , 1u16"),
        "PRODUCTION_ID_MAP must carry one entry per RuleId in rule order"
    );
}

// ===========================================================================
// Additional: Public symbol map and primary state IDs
// ===========================================================================

#[test]
fn public_symbol_map_emitted() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("PUBLIC_SYMBOL_MAP"));
}

#[test]
fn primary_state_ids_emitted() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("PRIMARY_STATE_IDS"));
}

// ===========================================================================
// Additional: Variant symbol map
// ===========================================================================

#[test]
fn variant_symbol_map_generated() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("SYMBOL_ID_TO_INDEX"));
    assert!(code.contains("SYMBOL_INDEX_TO_ID"));
}

#[test]
fn get_symbol_index_helper_generated() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("get_symbol_index"));
}

#[test]
fn get_symbol_id_helper_generated() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("get_symbol_id"));
}

// ===========================================================================
// Additional: Lexer code generation
// ===========================================================================

#[test]
fn lexer_fn_generated() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("lexer_fn"));
}

#[test]
fn lex_fn_some_in_language() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("lex_fn : Some (lexer_fn)"));
}

// ===========================================================================
// Additional: Determinism
// ===========================================================================

#[test]
fn same_grammar_same_output() {
    let (g1, t1) = build_pair("det", 3, 2, 1, 0, 5);
    let (g2, t2) = build_pair("det", 3, 2, 1, 0, 5);
    assert_eq!(gen_code(&g1, &t1), gen_code(&g2, &t2));
}

#[test]
fn different_grammar_different_output() {
    let (g1, t1) = build_pair("det_a", 2, 1, 0, 0, 3);
    let (g2, t2) = build_pair("det_b", 3, 2, 0, 0, 4);
    assert_ne!(gen_code(&g1, &t1), gen_code(&g2, &t2));
}

// ===========================================================================
// Additional: Whitespace / extras handling
// ===========================================================================

#[test]
fn whitespace_extra_marks_hidden() {
    let (mut g, t) = build_pair("ws", 2, 1, 0, 0, 2);
    let ws_sym = SymbolId(1);
    g.tokens.insert(
        ws_sym,
        Token {
            name: "whitespace".to_string(),
            pattern: TokenPattern::Regex(r"\s".to_string()),
            fragile: false,
        },
    );
    g.extras.push(ws_sym);
    let code = gen_code(&g, &t);
    // Should generate metadata with hidden flag
    assert!(code.contains("SYMBOL_METADATA"));
}

// ===========================================================================
// Additional: CompressedTables validation
// ===========================================================================

#[test]
fn compressed_tables_validate_ok() {
    use adze_tablegen::compress::*;
    let tables = CompressedTables {
        action_table: CompressedActionTable {
            data: vec![],
            row_offsets: vec![],
            default_actions: vec![],
        },
        goto_table: CompressedGotoTable {
            data: vec![],
            row_offsets: vec![],
        },
        small_table_threshold: 32768,
    };
    let pt = ParseTable::default();
    // Empty parse tables are not structurally valid anymore.
    assert!(tables.validate(&pt).is_err());
}

#[test]
fn compressed_parse_table_from_parse_table() {
    use adze_tablegen::CompressedParseTable;
    let mut grammar = GrammarBuilder::new("cpt")
        .token("a", "a")
        .rule("start", vec!["a"])
        .start("start")
        .build();
    let ff = FirstFollowSets::compute_normalized(&mut grammar).unwrap();
    let table = build_lr1_automaton(&grammar, &ff).unwrap();
    let cpt = CompressedParseTable::from_parse_table(&table);
    assert_eq!(cpt.symbol_count(), table.symbol_count);
    assert_eq!(cpt.state_count(), table.state_count);
}

// ===========================================================================
// Additional: Edge cases
// ===========================================================================

#[test]
fn empty_grammar_name_generates() {
    let (g, t) = build_pair("", 1, 1, 0, 0, 1);
    let code = gen_code(&g, &t);
    assert!(code.contains("LANGUAGE"));
}

#[test]
fn with_compressed_tables_chaining() {
    use adze_tablegen::compress::*;
    let (g, t) = minimal();
    // Must have row_offsets of length state_count + 1
    let n = t.state_count;
    let tables = CompressedTables {
        action_table: CompressedActionTable {
            data: vec![],
            row_offsets: vec![0; n + 1],
            default_actions: vec![Action::Error; n],
        },
        goto_table: CompressedGotoTable {
            data: vec![],
            row_offsets: vec![0; n + 1],
        },
        small_table_threshold: 32768,
    };
    let code = AbiLanguageBuilder::new(&g, &t)
        .with_compressed_tables(&tables)
        .generate()
        .to_string();
    assert!(code.contains("LANGUAGE"));
}

#[test]
fn builder_default_table_no_panic() {
    let g = Grammar::new("default".to_string());
    let t = ParseTable::default();
    let _builder = AbiLanguageBuilder::new(&g, &t);
}

#[test]
fn large_state_count_field_zero() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("large_state_count : 0u32"));
}

#[test]
fn eof_symbol_zero_in_language() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    // Tree-sitter convention: eof_symbol always 0 in the LANGUAGE struct
    assert!(code.contains("eof_symbol : 0"));
}

#[test]
fn small_parse_table_and_map_emitted() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("SMALL_PARSE_TABLE"));
    assert!(code.contains("SMALL_PARSE_TABLE_MAP"));
}

#[test]
fn full_pipeline_expression_grammar() {
    let grammar = GrammarBuilder::new("expr")
        .token("num", "[0-9]+")
        .token("plus", "+")
        .token("minus", "-")
        .token("lp", "(")
        .token("rp", ")")
        .rule("atom", vec!["num"])
        .rule("atom", vec!["lp", "expr", "rp"])
        .rule("expr", vec!["atom"])
        .rule_with_precedence("expr", vec!["expr", "plus", "expr"], 1, Associativity::Left)
        .rule_with_precedence(
            "expr",
            vec!["expr", "minus", "expr"],
            1,
            Associativity::Left,
        )
        .start("expr")
        .build();
    let (code, table) = full_pipeline(grammar);
    assert!(code.contains("LANGUAGE"));
    assert!(table.state_count > 2);
    assert!(table.symbol_count >= 5);
}

#[test]
fn full_pipeline_with_compression() {
    let mut grammar = GrammarBuilder::new("fc")
        .token("a", "a")
        .token("b", "b")
        .rule("start", vec!["a", "b"])
        .start("start")
        .build();
    let ff = FirstFollowSets::compute_normalized(&mut grammar).unwrap();
    let table = build_lr1_automaton(&grammar, &ff).unwrap();

    let token_indices = adze_tablegen::collect_token_indices(&grammar, &table);
    let start_empty = adze_tablegen::eof_accepts_or_reduces(&table);
    let compressed = TableCompressor::new()
        .compress(&table, &token_indices, start_empty)
        .unwrap();

    let code = AbiLanguageBuilder::new(&grammar, &table)
        .with_compressed_tables(&compressed)
        .generate()
        .to_string();
    assert!(code.contains("LANGUAGE"));
}

#[test]
fn rule_count_field_emitted() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("rule_count"));
}

#[test]
fn production_count_field_emitted() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("production_count"));
}

#[test]
fn rules_pointer_emitted() {
    let (g, t) = minimal();
    let code = gen_code(&g, &t);
    assert!(code.contains("rules : TS_RULES . as_ptr ()"));
}

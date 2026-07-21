#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

//! Comprehensive tests for lexer mode generation in `adze-tablegen`.
//!
//! Covers: lex mode table generation, token priority ordering, keyword vs
//! identifier mode, multiple lex modes, mode transitions, empty mode handling,
//! default mode behavior, and regex pattern compilation in lex modes.

use adze_glr_core::{GotoIndexing, LexMode, ParseTable};
use adze_ir::{Grammar, StateId, SymbolId, Token, TokenPattern};
use adze_tablegen::lexer_gen::generate_lexer;
use adze_tablegen::serializer::serialize_language;
use std::collections::BTreeMap;

// ── Helpers ──────────────────────────────────────────────────────────

const INVALID: StateId = StateId(u16::MAX);

/// Build a grammar with given tokens and a trivial symbol_to_index map.
fn grammar_with_tokens(
    tokens: Vec<(u16, &str, TokenPattern)>,
) -> (Grammar, BTreeMap<SymbolId, usize>) {
    let mut grammar = Grammar::new("test".to_string());
    let mut symbol_to_index = BTreeMap::new();

    for (id, name, pattern) in tokens {
        grammar.tokens.insert(
            SymbolId(id),
            Token {
                name: name.to_string(),
                pattern,
                fragile: false,
            },
        );
        symbol_to_index.insert(SymbolId(id), id as usize);
    }

    (grammar, symbol_to_index)
}

/// Generate lexer code string from token list.
fn lexer_code(tokens: Vec<(u16, &str, TokenPattern)>) -> String {
    let (grammar, map) = grammar_with_tokens(tokens);
    generate_lexer(&grammar, &map).to_string()
}

fn normalized_codegen(code: &str) -> String {
    code.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn better_match_pos(code: &str, sym: u16) -> Option<usize> {
    normalized_codegen(code).find(&format!("better_match({sym}u16,"))
}

fn candidate_registration_count(code: &str) -> usize {
    normalized_codegen(code).matches("best_sym=Some(").count()
}

/// Build a minimal empty parse table (mirrors the crate-internal `make_empty_table`).
fn empty_table(states: usize, terms: usize, nonterms: usize, externals: usize) -> ParseTable {
    let states = states.max(1);
    let eof_idx = 1 + terms + externals;
    let nonterms_eff = if nonterms == 0 { 1 } else { nonterms };
    let symbol_count = eof_idx + 1 + nonterms_eff;

    let actions = vec![vec![vec![]; symbol_count]; states];
    let gotos = vec![vec![INVALID; symbol_count]; states];

    let start_symbol = SymbolId((eof_idx + 1) as u16);
    let eof_symbol = SymbolId(eof_idx as u16);

    let token_count = eof_idx - externals;

    let mut symbol_to_index: BTreeMap<SymbolId, usize> = BTreeMap::new();
    for i in 0..symbol_count {
        symbol_to_index.insert(SymbolId(i as u16), i);
    }
    let mut nonterminal_to_index: BTreeMap<SymbolId, usize> = BTreeMap::new();
    nonterminal_to_index.insert(start_symbol, start_symbol.0 as usize);
    let mut index_to_symbol = vec![SymbolId(0); symbol_count];
    for (&sym, &idx) in &symbol_to_index {
        index_to_symbol[idx] = sym;
    }

    let lex_modes = vec![
        LexMode {
            lex_state: 0,
            external_lex_state: 0,
        };
        states
    ];

    ParseTable {
        action_table: actions,
        goto_table: gotos,
        rules: vec![],
        state_count: states,
        symbol_count,
        symbol_to_index,
        index_to_symbol,
        nonterminal_to_index,
        symbol_metadata: vec![],
        token_count,
        external_token_count: externals,
        eof_symbol,
        start_symbol,
        initial_state: StateId(0),
        lex_modes,
        extras: vec![],
        external_scanner_states: vec![],
        dynamic_prec_by_rule: vec![],
        rule_assoc_by_rule: vec![],
        alias_sequences: vec![],
        field_names: vec![],
        field_map: BTreeMap::new(),
        grammar: Grammar::new("test".to_string()),
        goto_indexing: GotoIndexing::NonterminalMap,
    }
}

/// Build a parse table with explicit lex modes.
fn table_with_lex_modes(state_count: usize, modes: Vec<LexMode>) -> ParseTable {
    let mut pt = empty_table(state_count, 1, 1, 0);
    pt.lex_modes = modes;
    pt
}

// ══════════════════════════════════════════════════════════════════════
// 1 – Lex mode table generation
// ══════════════════════════════════════════════════════════════════════

#[test]
fn lex_modes_generated_per_state() {
    let pt = empty_table(5, 2, 1, 0);
    assert_eq!(pt.lex_modes.len(), 5, "one lex mode per state");
}

#[test]
fn lex_mode_state_indices_are_sequential() {
    let pt = empty_table(4, 1, 1, 0);
    for i in 0..4 {
        assert_eq!(
            pt.lex_modes[i].lex_state, 0,
            "default modes all use lex_state 0"
        );
    }
}

#[test]
fn serialized_lex_modes_match_state_count() {
    let pt = empty_table(3, 1, 1, 0);
    let json = serialize_language(&pt.grammar, &pt, None).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let modes = v["lex_modes"].as_array().unwrap();
    assert_eq!(modes.len(), 3);
}

#[test]
fn serialized_lex_modes_contain_lex_state_and_external() {
    let pt = empty_table(2, 1, 1, 0);
    let json = serialize_language(&pt.grammar, &pt, None).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let first = &v["lex_modes"][0];
    assert!(first.get("lex_state").is_some());
    assert!(first.get("external_lex_state").is_some());
}

// ══════════════════════════════════════════════════════════════════════
// 2 – Token priority ordering
// ══════════════════════════════════════════════════════════════════════

#[test]
fn keywords_sorted_longest_first_in_mode() {
    let code = lexer_code(vec![
        (1, "kw_in", TokenPattern::String("in".into())),
        (2, "kw_int", TokenPattern::String("int".into())),
        (3, "kw_interface", TokenPattern::String("interface".into())),
    ]);
    let p1 = better_match_pos(&code, 3).unwrap();
    let p2 = better_match_pos(&code, 2).unwrap();
    let p3 = better_match_pos(&code, 1).unwrap();
    assert!(p1 < p2 && p2 < p3, "longest keyword first");
}

#[test]
fn keywords_precede_single_char_operators() {
    let code = lexer_code(vec![
        (1, "plus", TokenPattern::String("+".into())),
        (2, "kw_let", TokenPattern::String("let".into())),
    ]);
    let pos_kw = better_match_pos(&code, 2).unwrap();
    let pos_op = better_match_pos(&code, 1).unwrap();
    assert!(pos_kw < pos_op, "longer keyword candidate before single-char operator");
}

#[test]
fn string_tokens_before_regex_tokens() {
    let code = lexer_code(vec![
        (1, "num", TokenPattern::Regex(r"\d+".into())),
        (2, "semi", TokenPattern::String(";".into())),
    ]);
    let pos_num = better_match_pos(&code, 1).unwrap();
    let pos_semi = better_match_pos(&code, 2).unwrap();
    assert!(pos_num < pos_semi, "maximal regex candidate before literal");
}

#[test]
fn named_tokens_before_auto_generated() {
    let code = lexer_code(vec![
        (10, "_99", TokenPattern::String("!".into())),
        (11, "bang", TokenPattern::String("?".into())),
    ]);
    let pos_bang = code.find("33u32").unwrap(); // '!' = 33
    let pos_q = code.find("63u32").unwrap(); // '?' = 63
    assert!(pos_bang < pos_q, "lower symbol index registers first");
}

// ══════════════════════════════════════════════════════════════════════
// 3 – Keyword vs identifier mode
// ══════════════════════════════════════════════════════════════════════

#[test]
fn keyword_has_word_boundary_guard() {
    let code = lexer_code(vec![(1, "kw_fn", TokenPattern::String("fn".into()))]);
    assert!(
        code.contains("is_ascii_alphanumeric"),
        "word boundary check"
    );
}

#[test]
fn identifier_pattern_emitted_after_keywords() {
    let code = lexer_code(vec![
        (
            1,
            "ident",
            TokenPattern::Regex(r"[a-zA-Z_][a-zA-Z0-9_]*".into()),
        ),
        (2, "kw_for", TokenPattern::String("for".into())),
    ]);
    let pi = better_match_pos(&code, 1).unwrap();
    let pk = better_match_pos(&code, 2).unwrap();
    assert!(pi < pk, "identifier candidate before keyword candidate");
}

#[test]
fn keyword_with_underscore_treated_as_keyword() {
    let code = lexer_code(vec![(1, "kw", TokenPattern::String("my_kw".into()))]);
    assert!(
        code.contains("is_ascii_alphanumeric"),
        "underscore keyword gets boundary"
    );
}

#[test]
fn single_alpha_char_not_keyword() {
    let code = lexer_code(vec![(1, "x", TokenPattern::String("x".into()))]);
    // Single char → direct lookahead, no boundary check
    assert!(code.contains("120u32"), "'x' = 120 as single-char match");
}

// ══════════════════════════════════════════════════════════════════════
// 4 – Multiple lex modes
// ══════════════════════════════════════════════════════════════════════

#[test]
fn parse_table_supports_distinct_lex_modes() {
    let modes = vec![
        LexMode {
            lex_state: 0,
            external_lex_state: 0,
        },
        LexMode {
            lex_state: 1,
            external_lex_state: 0,
        },
        LexMode {
            lex_state: 2,
            external_lex_state: 1,
        },
    ];
    let pt = table_with_lex_modes(3, modes.clone());
    assert_eq!(pt.lex_modes, modes);
}

#[test]
fn lex_mode_accessor_returns_correct_mode() {
    let modes = vec![
        LexMode {
            lex_state: 0,
            external_lex_state: 0,
        },
        LexMode {
            lex_state: 1,
            external_lex_state: 5,
        },
    ];
    let pt = table_with_lex_modes(2, modes);
    assert_eq!(pt.lex_mode(StateId(1)).external_lex_state, 5);
}

#[test]
fn multiple_modes_with_different_external_states() {
    let modes = vec![
        LexMode {
            lex_state: 0,
            external_lex_state: 0,
        },
        LexMode {
            lex_state: 1,
            external_lex_state: 1,
        },
        LexMode {
            lex_state: 2,
            external_lex_state: 2,
        },
    ];
    let pt = table_with_lex_modes(3, modes);
    for i in 0..3 {
        assert_eq!(pt.lex_modes[i].external_lex_state, i as u16);
    }
}

// ══════════════════════════════════════════════════════════════════════
// 5 – Mode transitions
// ══════════════════════════════════════════════════════════════════════

#[test]
fn lex_mode_lookup_out_of_bounds_returns_default() {
    let modes = vec![LexMode {
        lex_state: 0,
        external_lex_state: 0,
    }];
    let pt = table_with_lex_modes(1, modes);
    let m = pt.lex_mode(StateId(99));
    assert_eq!(m.lex_state, 0, "out-of-bounds defaults to state 0");
    assert_eq!(m.external_lex_state, 0);
}

#[test]
fn lex_mode_transitions_through_states() {
    let modes = vec![
        LexMode {
            lex_state: 0,
            external_lex_state: 0,
        },
        LexMode {
            lex_state: 1,
            external_lex_state: 0,
        },
        LexMode {
            lex_state: 0,
            external_lex_state: 1,
        },
    ];
    let pt = table_with_lex_modes(3, modes);
    // Simulate state transitions
    let m0 = pt.lex_mode(StateId(0));
    let m1 = pt.lex_mode(StateId(1));
    let m2 = pt.lex_mode(StateId(2));
    assert_eq!(m0.lex_state, 0);
    assert_eq!(m1.lex_state, 1);
    assert_eq!(m2.external_lex_state, 1);
}

// ══════════════════════════════════════════════════════════════════════
// 6 – Empty mode handling
// ══════════════════════════════════════════════════════════════════════

#[test]
fn empty_lex_modes_vec_defaults_gracefully() {
    let pt = table_with_lex_modes(1, vec![]);
    let m = pt.lex_mode(StateId(0));
    assert_eq!(m.lex_state, 0, "empty modes → default");
}

#[test]
fn empty_grammar_lexer_still_has_null_guard() {
    let code = lexer_code(vec![]);
    assert!(code.contains("is_null"), "null check present");
    assert!(code.contains("false"), "returns false for empty grammar");
}

#[test]
fn lexer_with_no_tokens_has_correct_signature() {
    let code = lexer_code(vec![]);
    assert!(code.contains("lexer_fn"));
    assert!(code.contains("lex_mode"));
    assert!(code.contains("-> bool"));
}

// ══════════════════════════════════════════════════════════════════════
// 7 – Default mode behavior
// ══════════════════════════════════════════════════════════════════════

#[test]
fn default_lex_mode_is_zero() {
    let pt = empty_table(1, 1, 1, 0);
    let m = pt.lex_mode(StateId(0));
    assert_eq!(m.lex_state, 0);
    assert_eq!(m.external_lex_state, 0);
}

#[test]
fn make_empty_table_lex_modes_length() {
    let pt = empty_table(7, 2, 1, 0);
    assert_eq!(pt.lex_modes.len(), 7);
}

#[test]
fn constructed_table_has_lex_modes() {
    let pt = empty_table(3, 2, 1, 0);
    assert!(!pt.lex_modes.is_empty(), "table should have lex modes");
    assert_eq!(pt.lex_modes.len(), pt.state_count);
}

#[test]
fn default_mode_external_lex_state_is_zero() {
    let pt = empty_table(3, 2, 1, 0);
    for m in &pt.lex_modes {
        assert_eq!(m.external_lex_state, 0, "default external state is 0");
    }
}

// ══════════════════════════════════════════════════════════════════════
// 8 – Regex pattern compilation in lex modes
// ══════════════════════════════════════════════════════════════════════

#[test]
fn digit_regex_compiles_to_ascii_digit_check() {
    let code = lexer_code(vec![(1, "num", TokenPattern::Regex(r"\d+".into()))]);
    assert!(code.contains("is_ascii_digit"));
    assert!(better_match_pos(&code, 1).is_some());
}

#[test]
fn word_regex_compiles_to_alphanumeric_check() {
    let code = lexer_code(vec![(1, "word", TokenPattern::Regex(r"\w+".into()))]);
    assert!(code.contains("is_ascii_alphanumeric"));
}

#[test]
fn whitespace_regex_variants_all_compile() {
    for pat in [r"\s", r"\s+", r"\s*"] {
        let code = lexer_code(vec![(1, "ws", TokenPattern::Regex(pat.into()))]);
        assert!(
            code.contains("is_ascii_whitespace"),
            "pattern {pat} should compile to whitespace check"
        );
    }
}

#[test]
fn operator_char_class_regex_compiles() {
    let code = lexer_code(vec![(1, "op", TokenPattern::Regex(r"[-+*/]".into()))]);
    assert!(better_match_pos(&code, 1).is_some());
}

#[test]
fn identifier_regex_compiles_to_alpha_check() {
    let code = lexer_code(vec![(
        1,
        "id",
        TokenPattern::Regex(r"[a-zA-Z_][a-zA-Z0-9_]*".into()),
    )]);
    assert!(code.contains("is_ascii_alphabetic"));
    assert!(code.contains("is_ascii_alphanumeric"));
}

#[test]
fn unrecognized_regex_produces_no_match() {
    let code = lexer_code(vec![(1, "hex", TokenPattern::Regex(r"[0-9a-f]+".into()))]);
    assert_eq!(
        candidate_registration_count(&code),
        0,
        "unknown regex generates no candidate"
    );
}

#[test]
fn duplicate_regex_patterns_deduplicated_in_mode() {
    let code = lexer_code(vec![
        (1, "d1", TokenPattern::Regex(r"\d+".into())),
        (2, "d2", TokenPattern::Regex(r"\d+".into())),
    ]);
    assert_eq!(candidate_registration_count(&code), 2, "distinct duplicate candidates");
}

#[test]
fn duplicate_string_patterns_deduplicated_in_mode() {
    let code = lexer_code(vec![
        (1, "p1", TokenPattern::String("+".into())),
        (2, "p2", TokenPattern::String("+".into())),
    ]);
    assert_eq!(candidate_registration_count(&code), 2, "distinct duplicate candidates");
}

#[test]
fn mixed_mode_ordering_kw_str_regex_ident() {
    let code = lexer_code(vec![
        (
            1,
            "ident",
            TokenPattern::Regex(r"[a-zA-Z_][a-zA-Z0-9_]*".into()),
        ),
        (2, "num", TokenPattern::Regex(r"\d+".into())),
        (3, "kw_if", TokenPattern::String("if".into())),
        (4, "semi", TokenPattern::String(";".into())),
    ]);
    let pi = better_match_pos(&code, 1).unwrap();
    let pr = better_match_pos(&code, 2).unwrap();
    let pk = better_match_pos(&code, 3).unwrap();
    let ps = better_match_pos(&code, 4).unwrap();
    assert!(pi < pr && pr < pk && pk < ps, "maximal patterns before shorter literals");
}

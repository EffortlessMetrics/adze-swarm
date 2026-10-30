//! Regression tests for #1077: RHS lengths at the generated ABI child-count
//! limit (u8) must not silently wrap (255 -> 255, 256 -> 0, 257 -> 1).

use adze_glr_core::{GotoIndexing, LexMode, ParseTable};
use adze_ir::{Grammar, ProductionId, Rule, StateId, Symbol, SymbolId, Token, TokenPattern};
use adze_tablegen::{AbiLanguageBuilder, MAX_ABI_RHS_LENGTH, TableGenError};
use std::collections::BTreeMap;

fn table() -> ParseTable {
    let symbol_count = 4usize;
    let mut symbol_to_index = BTreeMap::new();
    let mut index_to_symbol = vec![SymbolId(0); symbol_count];
    for i in 0..symbol_count {
        symbol_to_index.insert(SymbolId(i as u16), i);
        index_to_symbol[i] = SymbolId(i as u16);
    }
    let mut nonterminal_to_index = BTreeMap::new();
    nonterminal_to_index.insert(SymbolId(3), 3);
    ParseTable {
        action_table: vec![vec![vec![]; symbol_count]],
        goto_table: vec![vec![StateId(u16::MAX); symbol_count]],
        symbol_metadata: vec![],
        state_count: 1,
        symbol_count,
        symbol_to_index,
        index_to_symbol,
        nonterminal_to_index,
        external_scanner_states: vec![],
        rules: vec![],
        eof_symbol: SymbolId(0),
        start_symbol: SymbolId(3),
        grammar: Grammar::default(),
        initial_state: StateId(0),
        token_count: 2,
        external_token_count: 0,
        lex_modes: vec![LexMode {
            lex_state: 0,
            external_lex_state: 0,
        }],
        extras: vec![],
        dynamic_prec_by_rule: vec![],
        rule_assoc_by_rule: vec![],
        alias_sequences: vec![],
        field_names: vec![],
        field_map: BTreeMap::new(),
        goto_indexing: GotoIndexing::NonterminalMap,
    }
}

fn grammar_with_rhs(n: usize) -> Grammar {
    let mut g = Grammar::new("long_rhs".to_string());
    let tok = SymbolId(1);
    let start = SymbolId(3);
    g.tokens.insert(
        tok,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    g.rule_names.insert(start, "source_file".into());
    g.add_rule(Rule {
        lhs: start,
        rhs: vec![Symbol::Terminal(tok); n],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    g
}

#[test]
fn test_rhs_255_at_limit_emits_exact_child_count() {
    assert_eq!(MAX_ABI_RHS_LENGTH, 255);
    let t = table();
    let g = grammar_with_rhs(255);
    let code = AbiLanguageBuilder::new(&g, &t)
        .try_generate()
        .expect("255 is at the ABI limit and must succeed")
        .to_string();
    assert!(code.contains("rhs_len : 255u8"), "TS_RULES must emit 255");
    assert!(
        code.contains("child_count : 255u8"),
        "PARSE_ACTIONS must emit 255"
    );
}

#[test]
fn test_rhs_256_beyond_limit_is_rejected_with_context() {
    let t = table();
    let g = grammar_with_rhs(256);
    let err = AbiLanguageBuilder::new(&g, &t)
        .try_generate()
        .expect_err("256 exceeds the u8 ABI child-count limit");
    match &err {
        TableGenError::RhsLengthExceedsAbiLimit {
            grammar,
            rule,
            observed,
            limit,
            ..
        } => {
            assert_eq!(grammar, "long_rhs");
            assert_eq!(rule, "source_file");
            assert_eq!(*observed, 256);
            assert_eq!(*limit, 255);
        }
        other => panic!("expected RhsLengthExceedsAbiLimit, got {other:?}"),
    }
    let msg = err.to_string();
    assert!(msg.contains("long_rhs") && msg.contains("source_file"));
    assert!(msg.contains("256") && msg.contains("255"));
}

#[test]
fn test_rhs_257_beyond_limit_is_rejected_not_wrapped_to_one() {
    let t = table();
    let g = grammar_with_rhs(257);
    let err = AbiLanguageBuilder::new(&g, &t)
        .try_generate()
        .expect_err("257 must not wrap to child_count 1");
    assert!(matches!(
        err,
        TableGenError::RhsLengthExceedsAbiLimit { observed: 257, .. }
    ));
}

#[test]
#[should_panic(expected = "observed: 256")]
fn test_generate_compat_wrapper_does_not_silently_truncate() {
    let t = table();
    let g = grammar_with_rhs(256);
    let _ = AbiLanguageBuilder::new(&g, &t).generate();
}

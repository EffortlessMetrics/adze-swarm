//! Test helper utilities for creating stub languages and parse tables.
//!
//! This module provides common functionality for tests and examples that need
//! to create minimal Language instances for testing purposes.

use crate::{Language, Token, language::SymbolMetadata};

#[cfg(feature = "glr")]
fn empty_parse_table() -> &'static adze_glr_core::ParseTable {
    use adze_glr_core::{GotoIndexing, ParseTable};
    use adze_ir::{Grammar, StateId, SymbolId};
    use std::collections::BTreeMap;

    Box::leak(Box::new(ParseTable {
        action_table: vec![],
        goto_table: vec![],
        symbol_metadata: vec![],
        state_count: 0,
        symbol_count: 0,
        symbol_to_index: BTreeMap::new(),
        index_to_symbol: vec![],
        external_scanner_states: vec![],
        rules: vec![],
        nonterminal_to_index: BTreeMap::new(),
        goto_indexing: GotoIndexing::NonterminalMap,
        eof_symbol: SymbolId(0),
        start_symbol: SymbolId(0),
        grammar: Grammar::new("stub".to_string()),
        initial_state: StateId(0),
        token_count: 0,
        external_token_count: 0,
        lex_modes: vec![],
        extras: vec![],
        dynamic_prec_by_rule: vec![],
        rule_assoc_by_rule: vec![],
        alias_sequences: vec![],
        field_names: vec![],
        field_map: BTreeMap::new(),
    }))
}

#[cfg(not(feature = "glr"))]
fn empty_parse_table() -> crate::language::ParseTable {
    crate::language::ParseTable {
        state_count: 0,
        action_table: vec![],
        small_parse_table: None,
        small_parse_table_map: None,
    }
}

#[cfg(feature = "glr")]
fn metadata(is_terminal: bool, is_visible: bool) -> SymbolMetadata {
    SymbolMetadata {
        is_terminal,
        is_visible,
        is_supertype: false,
    }
}

/// Create a GLR language for grammars shaped like `start -> token_1 ... token_n`.
///
/// Token symbols are assigned IDs from `1..=token_names.len()`, EOF remains `0`,
/// and the generated tokenizer recognizes one-byte token names in the input.
/// This keeps integration tests from open-coding the same parse-table, metadata,
/// and tokenizer setup for every small grammar fixture.
#[cfg(feature = "glr")]
pub fn linear_token_language(grammar_name: &str, token_names: &[&str]) -> Language {
    use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
    use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token as IrToken, TokenPattern};

    assert!(
        !token_names.is_empty(),
        "linear_token_language requires at least one token"
    );
    assert!(
        token_names.iter().all(|name| name.len() == 1),
        "linear_token_language only tokenizes one-byte token names"
    );

    let mut grammar = Grammar::new(grammar_name.to_string());
    let mut rhs = Vec::with_capacity(token_names.len());
    for (index, token_name) in token_names.iter().enumerate() {
        let symbol_id = SymbolId((index + 1) as u16);
        grammar.tokens.insert(
            symbol_id,
            IrToken {
                name: (*token_name).to_string(),
                pattern: TokenPattern::String((*token_name).to_string()),
                fragile: false,
            },
        );
        rhs.push(Symbol::Terminal(symbol_id));
    }

    let start_id = SymbolId((token_names.len() + 1) as u16);
    grammar.rule_names.insert(start_id, "start".to_string());
    grammar.rules.insert(
        start_id,
        vec![Rule {
            lhs: start_id,
            rhs,
            precedence: None,
            associativity: None,
            production_id: ProductionId(0),
            fields: vec![],
        }],
    );

    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let table = build_lr1_automaton(&grammar, &ff)
        .expect("table")
        .normalize_eof_to_zero()
        .with_detected_goto_indexing();
    let table: &'static _ = Box::leak(Box::new(table));

    let mut symbol_names = Vec::with_capacity(token_names.len() + 2);
    symbol_names.push("EOF".to_string());
    symbol_names.extend(token_names.iter().map(|name| (*name).to_string()));
    symbol_names.push("start".to_string());

    let mut symbol_metadata = Vec::with_capacity(token_names.len() + 2);
    symbol_metadata.push(metadata(true, false));
    symbol_metadata.extend((0..token_names.len()).map(|_| metadata(true, true)));
    symbol_metadata.push(metadata(false, true));

    let token_bytes: Vec<u8> = token_names.iter().map(|name| name.as_bytes()[0]).collect();
    Language::builder()
        .parse_table(table)
        .symbol_names(symbol_names)
        .symbol_metadata(symbol_metadata)
        .tokenizer(move |input: &[u8]| {
            let mut toks = Vec::new();
            for (i, &byte) in input.iter().enumerate() {
                if let Some(index) = token_bytes
                    .iter()
                    .position(|&token_byte| token_byte == byte)
                {
                    toks.push(Token {
                        kind: (index + 1) as u32,
                        start: i as u32,
                        end: (i + 1) as u32,
                    });
                }
            }
            toks.push(Token {
                kind: 0,
                start: input.len() as u32,
                end: input.len() as u32,
            });
            Box::new(toks.into_iter()) as Box<dyn Iterator<Item = Token> + '_>
        })
        .build()
        .unwrap()
}

/// Create a GLR language for `start -> mid` and `mid -> a`.
///
/// This provides the common non-terminal chain fixture used by forest builder
/// tests without duplicating parse-table and tokenizer construction.
#[cfg(feature = "glr")]
pub fn single_token_chain_language() -> Language {
    use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
    use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token as IrToken, TokenPattern};

    let mut grammar = Grammar::new("chain".to_string());
    let a_id = SymbolId(1);
    grammar.tokens.insert(
        a_id,
        IrToken {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );

    let mid_id = SymbolId(2);
    grammar.rule_names.insert(mid_id, "mid".to_string());
    grammar.rules.insert(
        mid_id,
        vec![Rule {
            lhs: mid_id,
            rhs: vec![Symbol::Terminal(a_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(0),
            fields: vec![],
        }],
    );

    let start_id = SymbolId(3);
    grammar.rule_names.insert(start_id, "start".to_string());
    grammar.rules.insert(
        start_id,
        vec![Rule {
            lhs: start_id,
            rhs: vec![Symbol::NonTerminal(mid_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(1),
            fields: vec![],
        }],
    );

    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let table = build_lr1_automaton(&grammar, &ff)
        .expect("table")
        .normalize_eof_to_zero()
        .with_detected_goto_indexing();
    let table: &'static _ = Box::leak(Box::new(table));

    Language::builder()
        .parse_table(table)
        .symbol_names(vec!["EOF".into(), "a".into(), "mid".into(), "start".into()])
        .symbol_metadata(vec![
            metadata(true, false),
            metadata(true, true),
            metadata(false, true),
            metadata(false, true),
        ])
        .tokenizer(|input: &[u8]| {
            let mut toks = Vec::new();
            for (i, &byte) in input.iter().enumerate() {
                if byte == b'a' {
                    toks.push(Token {
                        kind: 1,
                        start: i as u32,
                        end: (i + 1) as u32,
                    });
                }
            }
            toks.push(Token {
                kind: 0,
                start: input.len() as u32,
                end: input.len() as u32,
            });
            Box::new(toks.into_iter()) as Box<dyn Iterator<Item = Token> + '_>
        })
        .build()
        .unwrap()
}

/// Create a minimal stub language for testing purposes.
///
/// This creates a Language with:
/// - Empty parse tables (will not actually parse successfully)
/// - Single placeholder symbol with metadata
/// - Empty field names
/// - Optional tokenizer (GLR mode only) or static tokens
pub fn stub_language() -> Language {
    let table = empty_parse_table();
    let builder = Language::builder()
        .parse_table(table)
        .symbol_names(vec!["placeholder".into()])
        .symbol_metadata(vec![SymbolMetadata {
            is_terminal: true,
            is_visible: true,
            is_supertype: false,
        }])
        .field_names(vec![]);

    #[cfg(feature = "glr")]
    let builder = builder.tokenizer(|_| Box::new(std::iter::empty()));

    builder.build().unwrap()
}

/// Create a stub language with pre-defined tokens (for GLR mode).
///
/// In non-GLR mode, tokens are ignored since there's no tokenizer field.
#[cfg(feature = "glr")]
pub fn stub_language_with_tokens(tokens: Vec<Token>) -> Language {
    Language::builder()
        .parse_table(empty_parse_table())
        .symbol_names(vec!["placeholder".into()])
        .symbol_metadata(vec![SymbolMetadata {
            is_terminal: true,
            is_visible: true,
            is_supertype: false,
        }])
        .field_names(vec![])
        .tokenizer(move |_| Box::new(tokens.clone().into_iter()))
        .build()
        .unwrap()
}

/// For non-GLR builds, tokens parameter is ignored
#[cfg(not(feature = "glr"))]
pub fn stub_language_with_tokens(_tokens: Vec<Token>) -> Language {
    stub_language()
}

/// Create a test language with more symbols for complex testing
pub fn multi_symbol_test_language(symbol_count: usize) -> Language {
    let table = empty_parse_table();
    let builder = Language::builder()
        .parse_table(table)
        .symbol_names((0..symbol_count).map(|i| format!("symbol_{}", i)).collect())
        .symbol_metadata(vec![
            SymbolMetadata {
                is_terminal: true,
                is_visible: true,
                is_supertype: false,
            };
            symbol_count
        ])
        .field_names(vec![]);

    #[cfg(feature = "glr")]
    let builder = builder.tokenizer(|_| Box::new(std::iter::empty()));

    builder.build().unwrap()
}

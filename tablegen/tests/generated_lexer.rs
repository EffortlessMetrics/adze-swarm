//! Characterization tests for mode-aware non-destructive generated lexer (#926).

use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{Grammar, LexicalMetadata, SymbolId};
use adze_tablegen::lexer_gen::{generate_lexer, generate_lexer_with_table};
use std::collections::BTreeMap;

fn build_table(grammar: &Grammar) -> adze_glr_core::ParseTable {
    let mut working = grammar.clone();
    let first_follow = FirstFollowSets::compute_normalized(&mut working).expect("FIRST/FOLLOW");
    build_lr1_automaton(&working, &first_follow).expect("LR(1)")
}

fn codegen(grammar: &Grammar, table: Option<&adze_glr_core::ParseTable>) -> String {
    let symbol_to_index: BTreeMap<SymbolId, usize> = grammar
        .tokens
        .keys()
        .enumerate()
        .map(|(idx, id)| (*id, idx + 1))
        .collect();
    generate_lexer_with_table(grammar, &symbol_to_index, table).to_string()
}

#[test]
fn generated_lexer_uses_non_destructive_byte_peek() {
    let grammar = GrammarBuilder::new("peek")
        .token("if_kw", "if")
        .token("in_kw", "in")
        .rule("S", vec!["if_kw"])
        .start("S")
        .build();
    let code = codegen(&grammar, None);
    assert!(code.contains("lexer_byte_at_rel"), "non-destructive peek");
    assert!(code.contains("lexer_set_pos"), "commit cursor once");
    assert!(
        !code.contains("advance)(lexer"),
        "no destructive advance in candidates"
    );
}

#[test]
fn generated_lexer_maximal_munch_emits_both_eq_candidates() {
    let grammar = GrammarBuilder::new("eq")
        .token("assign", "=")
        .token("eq_eq", "==")
        .rule("S", vec!["assign", "eq_eq"])
        .start("S")
        .build();
    let code = codegen(&grammar, None);
    assert!(code.contains("better_match"), "maximal-munch selector");
    assert!(code.contains("61u32"), "'=' byte check");
    assert!(
        code.matches("61u32").count() >= 2,
        "'==' uses two byte checks"
    );
}

#[test]
fn generated_lexer_enforces_lex_mode_dispatch() {
    let grammar = GrammarBuilder::new("modes")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["a", "b"])
        .start("S")
        .build();
    let table = build_table(&grammar);
    let code = codegen(&grammar, Some(&table));
    assert!(
        code.contains("lex_mode") && code.contains("lex_state"),
        "generated lexer must read lex_mode; code snippet: {}",
        &code[code.find("lexer_fn").unwrap_or(0)
            ..code.len().min(code.find("lexer_fn").unwrap_or(0) + 400)]
    );
}

#[test]
fn generated_lexer_keyword_and_word_token_both_register_candidates() {
    let mut grammar = GrammarBuilder::new("word")
        .token("identifier", "[a-zA-Z_][a-zA-Z0-9_]*")
        .token("if_kw", "if")
        .rule("S", vec!["identifier"])
        .start("S")
        .build();
    let ident = grammar
        .tokens
        .iter()
        .find(|(_, token)| token.name == "identifier")
        .map(|(id, _)| *id)
        .expect("identifier");
    grammar.word_token = Some(ident);
    grammar.set_lexical_metadata(
        ident,
        LexicalMetadata {
            lexical_priority: 0,
            immediate: false,
        },
    );
    let if_id = grammar
        .tokens
        .iter()
        .find(|(_, token)| token.name == "if_kw")
        .map(|(id, _)| *id)
        .expect("if");
    grammar.set_lexical_metadata(
        if_id,
        LexicalMetadata {
            lexical_priority: 1,
            immediate: false,
        },
    );

    let code = codegen(&grammar, None);
    assert!(code.contains("is_identifier_start"), "word-token pattern");
    assert!(code.contains("is_word_char"), "keyword boundary check");
    assert!(code.contains("better_match"), "priority tie-break");
}

#[test]
fn generated_lexer_duplicate_patterns_keep_distinct_symbols() {
    use adze_ir::{Grammar, Token, TokenPattern};
    let mut grammar = Grammar::new("dup".to_string());
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "plus_a".to_string(),
            pattern: TokenPattern::String("+".into()),
            fragile: false,
        },
    );
    grammar.tokens.insert(
        SymbolId(2),
        Token {
            name: "plus_b".to_string(),
            pattern: TokenPattern::String("+".into()),
            fragile: false,
        },
    );
    let map = BTreeMap::from([(SymbolId(1), 1usize), (SymbolId(2), 2usize)]);
    let code = generate_lexer_with_table(&grammar, &map, None).to_string();
    assert!(
        code.matches("better_match").count() >= 2,
        "duplicate literals should register distinct candidates"
    );
}

#[test]
fn legacy_generate_lexer_entry_still_available() {
    let grammar = GrammarBuilder::new("legacy")
        .token("x", "x")
        .rule("S", vec!["x"])
        .start("S")
        .build();
    let map = BTreeMap::from([(SymbolId(1), 1usize)]);
    let code = generate_lexer(&grammar, &map).to_string();
    assert!(code.contains("lexer_fn"));
}

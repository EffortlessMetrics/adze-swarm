//! Characterization tests for lexical metadata in the IR (#924).

use adze_ir::{
    Grammar, LexicalMetadata, SymbolId, TOKEN_WRAPPER_PRIORITY, Token, TokenPattern,
    compare_lexical_priority, sorted_lexical_metadata, validate_token_pattern,
};
use indexmap::IndexMap;

#[test]
fn word_token_field_roundtrips_in_json() {
    let mut grammar = Grammar::new("with_word".to_string());
    let ident_id = SymbolId(5);
    grammar.tokens.insert(
        ident_id,
        Token {
            name: "identifier".to_string(),
            pattern: TokenPattern::Regex(r"[a-zA-Z_][a-zA-Z0-9_]*".to_string()),
            fragile: false,
        },
    );
    grammar.word_token = Some(ident_id);
    grammar
        .rule_names
        .insert(ident_id, "identifier".to_string());

    let json = serde_json::to_string(&grammar).expect("serialize grammar");
    let decoded: Grammar = serde_json::from_str(&json).expect("deserialize grammar");
    assert_eq!(decoded.word_token, Some(ident_id));
}

#[test]
fn lexical_metadata_immediate_and_priority_roundtrip() {
    let mut grammar = Grammar::new("lexical".to_string());
    let dot_id = SymbolId(2);
    grammar.tokens.insert(
        dot_id,
        Token {
            name: ".".to_string(),
            pattern: TokenPattern::String(".".to_string()),
            fragile: false,
        },
    );
    grammar.set_lexical_metadata(
        dot_id,
        LexicalMetadata {
            immediate: true,
            lexical_priority: TOKEN_WRAPPER_PRIORITY,
        },
    );

    let json = serde_json::to_string(&grammar).expect("serialize grammar");
    let decoded: Grammar = serde_json::from_str(&json).expect("deserialize grammar");
    let meta = decoded.lexical_metadata_for(dot_id);
    assert!(meta.immediate);
    assert_eq!(meta.lexical_priority, TOKEN_WRAPPER_PRIORITY);
}

#[test]
fn validate_token_pattern_rejects_zero_width_regex() {
    let err = validate_token_pattern("bad", &TokenPattern::Regex(String::new()))
        .expect_err("empty regex should fail");
    assert!(err.message.contains("zero-width"));
}

#[test]
fn validate_token_pattern_rejects_lookaround() {
    let err = validate_token_pattern("lookahead", &TokenPattern::Regex("(?=foo)".to_string()))
        .expect_err("lookahead should fail");
    assert!(err.message.contains("lookahead"));
}

#[test]
fn lexical_priority_tie_break_is_deterministic() {
    let left_id = SymbolId(10);
    let right_id = SymbolId(5);
    let left_meta = LexicalMetadata {
        immediate: false,
        lexical_priority: 1,
    };
    let right_meta = LexicalMetadata {
        immediate: false,
        lexical_priority: 2,
    };

    assert_eq!(
        compare_lexical_priority((&left_id, &left_meta), (&right_id, &right_meta)),
        std::cmp::Ordering::Greater
    );
}

#[test]
fn sorted_lexical_metadata_orders_by_priority_then_symbol_id() {
    let mut metadata = IndexMap::new();
    metadata.insert(
        SymbolId(3),
        LexicalMetadata {
            immediate: false,
            lexical_priority: 1,
        },
    );
    metadata.insert(
        SymbolId(1),
        LexicalMetadata {
            immediate: false,
            lexical_priority: 2,
        },
    );
    metadata.insert(
        SymbolId(2),
        LexicalMetadata {
            immediate: false,
            lexical_priority: 2,
        },
    );

    let sorted = sorted_lexical_metadata(&metadata);
    assert_eq!(sorted[0].0, SymbolId(1));
    assert_eq!(sorted[1].0, SymbolId(2));
    assert_eq!(sorted[2].0, SymbolId(3));
}

#[test]
fn grammar_validate_lexical_patterns_collects_errors() {
    let mut grammar = Grammar::new("invalid".to_string());
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "bad".to_string(),
            pattern: TokenPattern::Regex(String::new()),
            fragile: false,
        },
    );

    let errors = grammar
        .validate_lexical_patterns()
        .expect_err("invalid patterns should fail");
    assert_eq!(errors.len(), 1);
}

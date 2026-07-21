//! Converter tests for GrammarJs word-token and lexical metadata propagation (#924).

use adze_ir::{LexicalMetadata, TOKEN_WRAPPER_PRIORITY};
use adze_tool::grammar_js::{GrammarJs, GrammarJsConverter, Rule};
use indexmap::IndexMap;

fn convert(grammar_js: GrammarJs) -> adze_ir::Grammar {
    GrammarJsConverter::new(grammar_js)
        .convert()
        .expect("convert to IR")
}

#[test]
fn word_token_survives_json_to_ir_conversion() {
    let mut grammar_js = GrammarJs::new("keywords".to_string());
    grammar_js.word = Some("identifier".to_string());
    grammar_js.rules.insert(
        "identifier".to_string(),
        Rule::Pattern {
            value: "[a-zA-Z_][a-zA-Z0-9_]*".to_string(),
        },
    );
    grammar_js.rules.insert(
        "if".to_string(),
        Rule::String {
            value: "if".to_string(),
        },
    );

    let grammar = convert(grammar_js);
    let word = grammar
        .word_token_symbol()
        .expect("word token should be preserved");
    let word_name = grammar
        .rule_names
        .get(&word)
        .or_else(|| grammar.tokens.get(&word).map(|t| &t.name))
        .map(String::as_str);
    assert_eq!(word_name, Some("identifier"));
}

#[test]
fn immediate_token_sets_lexical_metadata() {
    let mut grammar_js = GrammarJs::new("immediate".to_string());
    grammar_js.rules.insert(
        "dot".to_string(),
        Rule::ImmediateToken {
            content: Box::new(Rule::String {
                value: ".".to_string(),
            }),
        },
    );

    let grammar = convert(grammar_js);
    let dot_token = grammar
        .tokens
        .iter()
        .find(|(_, token)| token.name == ".")
        .map(|(id, _)| *id)
        .expect("dot token");
    let meta = grammar.lexical_metadata_for(dot_token);
    assert!(meta.immediate);
}

#[test]
fn token_wrapper_sets_lexical_priority() {
    let mut grammar_js = GrammarJs::new("token_wrap".to_string());
    grammar_js.rules.insert(
        "kw".to_string(),
        Rule::Token {
            content: Box::new(Rule::String {
                value: "if".to_string(),
            }),
        },
    );

    let grammar = convert(grammar_js);
    let if_token = grammar
        .tokens
        .iter()
        .find(|(_, token)| token.name == "if")
        .map(|(id, _)| *id)
        .expect("if token");
    let meta = grammar.lexical_metadata_for(if_token);
    assert_eq!(meta.lexical_priority, TOKEN_WRAPPER_PRIORITY);
}

#[test]
fn duplicate_regex_patterns_keep_distinct_token_names() {
    let mut grammar_js = GrammarJs::new("dup_regex".to_string());
    grammar_js.rules.insert(
        "id_a".to_string(),
        Rule::Pattern {
            value: "[a-z]+".to_string(),
        },
    );
    grammar_js.rules.insert(
        "id_b".to_string(),
        Rule::Pattern {
            value: "[a-z]+".to_string(),
        },
    );

    let grammar = convert(grammar_js);
    let names: Vec<_> = grammar
        .tokens
        .values()
        .map(|token| token.name.as_str())
        .collect();
    assert!(names.contains(&"id_a"));
    assert!(names.contains(&"id_b"));
    assert_eq!(grammar.tokens.len(), 2);
}

#[test]
fn unsupported_pattern_fails_conversion_with_context() {
    let mut grammar_js = GrammarJs::new("bad_regex".to_string());
    grammar_js.rules.insert(
        "lookahead".to_string(),
        Rule::Pattern {
            value: "(?=foo)".to_string(),
        },
    );

    let err = GrammarJsConverter::new(grammar_js)
        .convert()
        .expect_err("lookahead pattern should fail");
    let message = err.to_string();
    assert!(message.contains("lookahead") || message.contains("invalid lexical pattern"));
}

#[test]
fn rust_native_word_field_matches_json_path() {
    let mut grammar_js = GrammarJs::new("native".to_string());
    grammar_js.word = Some("identifier".to_string());
    grammar_js.rules.insert(
        "identifier".to_string(),
        Rule::Pattern {
            value: "[a-z]+".to_string(),
        },
    );

    let grammar = convert(grammar_js);
    assert!(grammar.word_token_symbol().is_some());
}

#[test]
fn keyword_and_word_token_fixture_has_expected_metadata_shape() {
    let mut grammar_js = GrammarJs::new("kw_id".to_string());
    grammar_js.word = Some("identifier".to_string());
    grammar_js.rules = IndexMap::from([
        (
            "identifier".to_string(),
            Rule::Pattern {
                value: "[a-zA-Z_][a-zA-Z0-9_]*".to_string(),
            },
        ),
        (
            "if".to_string(),
            Rule::String {
                value: "if".to_string(),
            },
        ),
        (
            "in".to_string(),
            Rule::String {
                value: "in".to_string(),
            },
        ),
    ]);

    let grammar = convert(grammar_js);
    assert!(grammar.word_token_symbol().is_some());
    assert!(grammar.validate_lexical_patterns().is_ok());

    let if_meta = grammar
        .tokens
        .iter()
        .find(|(_, token)| token.name == "if")
        .map(|(id, _)| grammar.lexical_metadata_for(*id))
        .unwrap_or_else(LexicalMetadata::default);
    assert!(!if_meta.immediate);
}

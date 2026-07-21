//! Characterization tests for lexical ABI propagation (#924 PR3).

use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{Grammar, LexicalMetadata};
use adze_tablegen::AbiLanguageBuilder;
use adze_tablegen::abi::symbol_metadata;

fn build_table(grammar: &Grammar) -> adze_glr_core::ParseTable {
    let mut working = grammar.clone();
    let first_follow = FirstFollowSets::compute_normalized(&mut working).expect("FIRST/FOLLOW");
    build_lr1_automaton(&working, &first_follow).expect("LR(1)")
}

#[test]
fn word_token_maps_to_keyword_capture_token_in_generated_abi() {
    let mut grammar = GrammarBuilder::new("kw_capture")
        .token("identifier", "[a-zA-Z_][a-zA-Z0-9_]*")
        .token("if", "if")
        .rule("S", vec!["identifier"])
        .start("S")
        .build();
    let ident = grammar
        .tokens
        .iter()
        .find(|(_, token)| token.name == "identifier")
        .map(|(id, _)| *id)
        .expect("identifier token");
    grammar.word_token = Some(ident);

    let table = build_table(&grammar);
    let code = AbiLanguageBuilder::new(&grammar, &table)
        .generate()
        .to_string();
    let expected = table.symbol_to_index[&ident] as u16;

    assert!(
        code.lines().any(|line| {
            line.contains("keyword_capture_token") && line.contains(&expected.to_string())
        }),
        "generated ABI must encode explicit word-token index; expected {expected} in codegen"
    );
}

#[test]
fn immediate_token_sets_auxiliary_metadata_flag() {
    let grammar = GrammarBuilder::new("immediate_abi")
        .immediate_token("dot", ".")
        .rule("S", vec!["dot"])
        .start("S")
        .build();
    let dot = grammar
        .tokens
        .iter()
        .find(|(_, token)| token.name == "dot")
        .map(|(id, _)| *id)
        .expect("dot token");

    assert!(grammar.lexical_metadata_for(dot).immediate);

    let table = build_table(&grammar);
    let code = AbiLanguageBuilder::new(&grammar, &table)
        .generate()
        .to_string();
    let expected_meta =
        symbol_metadata::VISIBLE | symbol_metadata::NAMED | symbol_metadata::AUXILIARY;

    assert!(
        code.contains(&format!("{expected_meta}u8")),
        "immediate token metadata should include AUXILIARY in SYMBOL_METADATA"
    );
}

#[test]
fn duplicate_regex_patterns_keep_distinct_abi_symbol_names() {
    let grammar = GrammarBuilder::new("dup_abi")
        .token("id_a", "[a-z]+")
        .token("id_b", "[a-z]+")
        .rule("S", vec!["id_a", "id_b"])
        .start("S")
        .build();

    let table = build_table(&grammar);

    let names: Vec<_> = grammar.tokens.values().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["id_a", "id_b"]);
    assert_eq!(table.symbol_to_index.len(), grammar.tokens.len() + 2);
}

#[test]
fn language_builder_propagates_keyword_capture_token() {
    let mut grammar = GrammarBuilder::new("lang_builder_word")
        .token("identifier", "[a-z]+")
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
            immediate: false,
            lexical_priority: 1,
        },
    );

    let table = build_table(&grammar);
    let lang = adze_tablegen::LanguageBuilder::new(grammar, table.clone())
        .generate_language()
        .expect("language");

    let expected = table.symbol_to_index[&ident] as u16;
    assert_eq!(lang.keyword_capture_token, expected);
}

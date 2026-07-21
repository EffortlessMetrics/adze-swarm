//! Extraction emits explicit compiler-identity metadata (#862 PR2).

use adze_tool::generate_grammars;
use adze_tool::grammar_js::{GrammarJsConverter, from_json};
use std::path::PathBuf;

fn write_temp_grammar(contents: &str) -> PathBuf {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("grammar.rs");
    std::fs::write(&path, contents).expect("write grammar");
    // Leak the tempdir so the path stays valid for the duration of the test.
    std::mem::forget(dir);
    path
}

#[test]
fn extraction_emits_explicit_start_symbol_from_language_attr() {
    let path = write_temp_grammar(
        r##"
        #[adze::grammar("root9")]
        pub mod grammar {
            #[adze::language]
            pub enum Root9 {
                Tok(#[adze::leaf(pattern = r"\d+")] i32),
            }
        }
        "##,
    );

    let grammars = generate_grammars(&path).expect("generate grammars");
    assert_eq!(grammars.len(), 1);
    assert_eq!(
        grammars[0]["start_symbol"].as_str(),
        Some("Root9"),
        "start_symbol must match #[adze::language] type"
    );
}

#[test]
fn extraction_emits_wrapper_token_relations_for_leaf_patterns() {
    let path = write_temp_grammar(
        r##"
        #[adze::grammar("wrappers")]
        pub mod grammar {
            #[adze::language]
            pub struct Identifier {
                #[adze::leaf(pattern = r"[a-z]+")]
                value: String,
            }
        }
        "##,
    );

    let grammars = generate_grammars(&path).expect("generate grammars");
    let relations = grammars[0]["wrapper_token_relations"]
        .as_object()
        .expect("wrapper_token_relations object");
    assert!(
        relations
            .values()
            .any(|token| token.as_str() == Some("_/[a-z]+/")),
        "expected hidden pattern token name, got: {relations:?}"
    );
}

#[test]
fn extracted_metadata_survives_json_to_ir_conversion() {
    let path = write_temp_grammar(
        r##"
        #[adze::grammar("pipeline")]
        pub mod grammar {
            #[adze::language]
            pub struct Root9 {
                #[adze::leaf(pattern = r"\d+")]
                num: i32,
            }
        }
        "##,
    );

    let grammars = generate_grammars(&path).expect("generate grammars");
    let grammar_js = from_json(&grammars[0]).expect("parse extracted json");
    assert_eq!(grammar_js.start_symbol.as_deref(), Some("Root9"));
    assert!(!grammar_js.wrapper_token_relations.is_empty());

    let grammar = GrammarJsConverter::new(grammar_js)
        .convert()
        .expect("convert to IR");
    let root_id = grammar.find_symbol_by_name("Root9").expect("Root9 symbol");
    assert_eq!(grammar.explicit_start_symbol(), Some(root_id));
    assert_eq!(grammar.start_symbol(), Some(root_id));

    let wrapper_id = grammar
        .wrapper_token_relations
        .keys()
        .next()
        .copied()
        .expect("wrapper relation");
    assert!(grammar.wrapper_token_for(wrapper_id).is_some());
}

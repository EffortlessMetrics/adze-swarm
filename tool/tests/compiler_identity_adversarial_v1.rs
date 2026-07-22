//! Adversarial extraction/import/build fixtures for compiler identity (#862 PR5).

use adze_ir::builder::GrammarBuilder;
use adze_tool::generate_grammars;
use adze_tool::grammar_js::{GrammarJsConverter, from_json};
use adze_tool::pure_rust_builder::{BuildOptions, build_parser};
use serde_json::json;
use std::path::PathBuf;
use tempfile::TempDir;

struct TempGrammar {
    _dir: TempDir,
    path: PathBuf,
}

fn write_temp_grammar(contents: &str) -> TempGrammar {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("grammar.rs");
    std::fs::write(&path, contents).expect("write grammar");
    TempGrammar { _dir: dir, path }
}

fn build_with_default_options(
    grammar: adze_ir::Grammar,
) -> adze_tool::pure_rust_builder::BuildResult {
    let dir = TempDir::new().expect("tempdir");
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().into_owned(),
        emit_artifacts: false,
        compress_tables: true,
    };
    build_parser(grammar, opts).expect("build_parser should succeed")
}

#[test]
fn test_two_grammar_modules_with_adversarial_names_keep_independent_explicit_starts() {
    let fixture = write_temp_grammar(
        r##"
        #[adze::grammar("root9_mod")]
        pub mod grammar_a {
            #[adze::language]
            pub struct Root9 {
                #[adze::leaf(pattern = r"\d+")]
                num: i32,
            }
        }

        #[adze::grammar("other_mod")]
        pub mod grammar_b {
            #[adze::language]
            pub struct ModuleBRoot {
                #[adze::leaf(pattern = r"[a-z]+")]
                word: String,
            }
        }
        "##,
    );

    let grammars = generate_grammars(fixture.path.as_path()).expect("generate grammars");
    assert_eq!(grammars.len(), 2);

    let starts: Vec<_> = grammars
        .iter()
        .map(|g| g["start_symbol"].as_str().expect("start_symbol"))
        .collect();
    assert!(starts.contains(&"Root9"));
    assert!(starts.contains(&"ModuleBRoot"));
}

#[test]
fn test_extraction_pipeline_with_overlapping_tokens_preserves_wrapper_relations() {
    let fixture = write_temp_grammar(
        r##"
        #[adze::grammar("overlap")]
        pub mod grammar {
            #[adze::language]
            pub struct Root9 {
                #[adze::leaf(pattern = r"id")]
                id_tok: String,
                #[adze::leaf(pattern = r"[a-zA-Z_][a-zA-Z0-9_]*")]
                identifier_tok: String,
                #[adze::leaf(pattern = r"_suffix")]
                suffix_tok: String,
            }
        }
        "##,
    );

    let grammars = generate_grammars(fixture.path.as_path()).expect("generate grammars");
    assert_eq!(grammars[0]["start_symbol"].as_str(), Some("Root9"));

    let relations = grammars[0]["wrapper_token_relations"]
        .as_object()
        .expect("wrapper_token_relations");
    assert_eq!(relations.len(), 3);

    let grammar_js = from_json(&grammars[0]).expect("parse extracted json");
    let grammar = GrammarJsConverter::new(grammar_js)
        .convert()
        .expect("convert to IR");
    assert_eq!(
        grammar.explicit_start_symbol(),
        grammar.find_symbol_by_name("Root9")
    );
    assert_eq!(grammar.wrapper_token_relations.len(), 3);
}

#[test]
fn test_imported_underscore_helper_before_explicit_start_resolves_root9() {
    let value = json!({
        "name": "imported",
        "start_symbol": "Root9",
        "rules": {
            "_helper": { "type": "PATTERN", "value": "\\s+" },
            "Root9": { "type": "SYMBOL", "name": "_helper" }
        }
    });

    let grammar_js = from_json(&value).expect("parse imported json");
    let grammar = GrammarJsConverter::new(grammar_js)
        .convert()
        .expect("convert imported json");
    let start_id = grammar.explicit_start_symbol().expect("explicit start");
    assert_eq!(
        grammar.rule_names.get(&start_id),
        Some(&"Root9".to_string())
    );
}

#[test]
fn test_adversarial_overlap_with_explicit_relations_builds_states() {
    let grammar = GrammarBuilder::new("overlap_build")
        .token("id", "id")
        .token("identifier", r"[a-zA-Z_][a-zA-Z0-9_]*")
        .token("identifier_suffix", "_suffix")
        .wrapper_token("IdWrap", "id")
        .wrapper_token("IdentifierWrap", "identifier")
        .wrapper_token("SuffixWrap", "identifier_suffix")
        .rule("Root9", vec!["IdWrap"])
        .start("Root9")
        .build();

    let result = build_with_default_options(grammar);
    assert!(
        result.build_stats.state_count > 0,
        "expected non-trivial parse table for adversarial overlap grammar"
    );
}

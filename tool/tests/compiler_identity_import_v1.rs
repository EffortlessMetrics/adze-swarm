//! Imported grammar JSON/grammar.js preserves explicit compiler identity (#862 PR3).

use adze_tool::grammar_js::{GrammarJsConverter, GrammarJsParserV3, from_json};
use serde_json::json;

#[test]
fn imported_json_without_start_symbol_uses_first_rule_in_ir() {
    let value = json!({
        "name": "imported",
        "rules": {
            "source_file": {
                "type": "SYMBOL",
                "name": "expression"
            },
            "expression": {
                "type": "PATTERN",
                "value": "\\d+"
            }
        }
    });

    let grammar_js = from_json(&value).expect("parse imported json");
    assert_eq!(grammar_js.start_symbol.as_deref(), Some("source_file"));

    let grammar = GrammarJsConverter::new(grammar_js)
        .convert()
        .expect("convert imported json");
    let start_id = grammar.explicit_start_symbol().expect("explicit start");
    assert_eq!(
        grammar.rule_names.get(&start_id),
        Some(&"source_file".to_string())
    );
}

#[test]
fn imported_json_preserves_explicit_start_symbol_in_ir() {
    let value = json!({
        "name": "adversarial",
        "start_symbol": "Root9",
        "rules": {
            "_helper": { "type": "PATTERN", "value": "\\s+" },
            "Root9": { "type": "SYMBOL", "name": "_helper" }
        }
    });

    let grammar_js = from_json(&value).expect("parse imported json");
    assert_eq!(grammar_js.start_symbol.as_deref(), Some("Root9"));

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
fn imported_json_pattern_wrapper_relations_survive_ir_conversion() {
    let value = json!({
        "name": "wrappers",
        "rules": {
            "source_file": { "type": "SYMBOL", "name": "identifier" },
            "identifier": { "type": "PATTERN", "value": "[a-z]+" }
        }
    });

    let grammar_js = from_json(&value).expect("parse imported json");
    assert_eq!(
        grammar_js
            .wrapper_token_relations
            .get("identifier")
            .map(String::as_str),
        Some("_/[a-z]+/")
    );

    let grammar = GrammarJsConverter::new(grammar_js)
        .convert()
        .expect("convert imported json");
    let identifier_id = grammar
        .find_symbol_by_name("identifier")
        .expect("identifier symbol");
    let token_id = grammar
        .wrapper_token_relations
        .get(&identifier_id)
        .copied()
        .expect("wrapper relation");
    assert_eq!(
        grammar.tokens.get(&token_id).map(|token| token.name.as_str()),
        Some("_/[a-z]+/")
    );
}

#[test]
fn imported_grammar_js_records_first_rule_start_and_pattern_relations() {
    let content = r#"
        module.exports = grammar({
            name: "imported_js",
            rules: {
                source_file: $ => $.expression,
                expression: $ => /\d+/,
            },
        });
    "#;

    let mut parser = GrammarJsParserV3::new(content.to_string());
    let grammar_js = parser.parse().expect("parse grammar.js");

    assert_eq!(grammar_js.start_symbol.as_deref(), Some("source_file"));
    assert_eq!(
        grammar_js
            .wrapper_token_relations
            .get("expression")
            .map(String::as_str),
        Some("_/\\d+/")
    );

    let grammar = GrammarJsConverter::new(grammar_js)
        .convert()
        .expect("convert grammar.js");
    let start_id = grammar.explicit_start_symbol().expect("explicit start");
    assert_eq!(
        grammar.rule_names.get(&start_id),
        Some(&"source_file".to_string())
    );
}

//! Imported token bindings do not request semantic wrapper preservation.
use adze_ir::{Grammar, Symbol};
use adze_tool::grammar_js::{GrammarJsConverter, from_json};
use serde_json::json;

fn source_value(choice: bool, literal: bool) -> serde_json::Value {
    let leaf = if literal {
        json!({"type": "STRING", "value": "q"})
    } else {
        json!({"type": "PATTERN", "value": "[a-z]+"})
    };
    let reference = json!({"type": "SYMBOL", "name": "leaf"});
    let entry = if choice {
        json!({"type": "CHOICE", "members": [reference, {"type": "STRING", "value": "!"}]})
    } else {
        json!({"type": "SEQ", "members": [reference]})
    };
    json!({"name": "imported_leaf_context", "start_symbol": "source_file", "rules": {
        "source_file": entry, "leaf": leaf,
    }})
}
fn imported_leaf(choice: bool, literal: bool) -> anyhow::Result<Grammar> {
    GrammarJsConverter::new(from_json(&source_value(choice, literal))?).convert()
}
fn assert_binding_is_terminal(choice: bool, literal: bool) -> anyhow::Result<()> {
    let grammar = imported_leaf(choice, literal)?;
    assert_grammar_binding(&grammar, choice)
}
fn assert_grammar_binding(grammar: &Grammar, choice: bool) -> anyhow::Result<()> {
    let owner = grammar
        .explicit_start_symbol()
        .ok_or_else(|| anyhow::anyhow!("start"))?;
    let wrapper = grammar
        .find_symbol_by_name("leaf")
        .ok_or_else(|| anyhow::anyhow!("leaf"))?;
    let binding = grammar
        .wrapper_token_for(wrapper)
        .ok_or_else(|| anyhow::anyhow!("retained token binding"))?;
    assert_eq!(
        grammar.rules[&wrapper][0].rhs,
        [Symbol::Terminal(binding)],
        "the binding must name the actual constructed unit-rule token"
    );
    assert!(
        grammar.rules[&owner]
            .iter()
            .any(|rule| rule.rhs == [Symbol::Terminal(binding)]),
        "an inferred imported binding must stay terminal in {}: {:?}",
        if choice { "CHOICE" } else { "SEQ" },
        grammar.rules[&owner]
    );
    assert!(
        !grammar.rules[&owner]
            .iter()
            .any(|rule| rule.rhs.contains(&Symbol::NonTerminal(wrapper))),
        "an inferred relation cannot request an intermediate semantic wrapper"
    );
    Ok(())
}
#[test]
fn test_imported_pattern_sequence_retains_terminal_binding() -> anyhow::Result<()> {
    assert_binding_is_terminal(false, false)
}
#[test]
fn test_imported_pattern_choice_retains_terminal_binding() -> anyhow::Result<()> {
    assert_binding_is_terminal(true, false)
}
#[test]
fn test_imported_literal_sequence_retains_terminal_binding() -> anyhow::Result<()> {
    assert_binding_is_terminal(false, true)
}
#[test]
fn test_imported_literal_choice_retains_terminal_binding() -> anyhow::Result<()> {
    assert_binding_is_terminal(true, true)
}

#[test]
fn test_imported_origin_survives_typed_and_source_json_round_trips() -> anyhow::Result<()> {
    for literal in [false, true] {
        let source = from_json(&source_value(true, literal))?;
        assert!(source.inferred_wrapper_rules.contains("leaf"));
        let value = serde_json::to_value(&source)?;
        let typed: adze_tool::grammar_js::GrammarJs = serde_json::from_value(value.clone())?;
        for restored in [typed, from_json(&value)?] {
            assert!(restored.inferred_wrapper_rules.contains("leaf"));
            assert_grammar_binding(&GrammarJsConverter::new(restored).convert()?, true)?;
        }
    }
    Ok(())
}
#[test]
fn test_explicit_source_relation_keeps_semantic_choice_wrapper() -> anyhow::Result<()> {
    for literal in [false, true] {
        let mut source = source_value(true, literal);
        source["wrapper_token_relations"] =
            json!({"leaf": if literal { "q" } else { "_/[a-z]+/" }});
        let source = from_json(&source)?;
        assert!(!source.inferred_wrapper_rules.contains("leaf"));
        let grammar = GrammarJsConverter::new(source).convert()?;
        let owner = grammar.explicit_start_symbol().unwrap();
        let wrapper = grammar.find_symbol_by_name("leaf").unwrap();
        assert!(
            grammar.rules[&owner]
                .iter()
                .any(|rule| rule.rhs == [Symbol::NonTerminal(wrapper)])
        );
        assert_eq!(
            grammar.rules[&wrapper][0].rhs,
            [Symbol::Terminal(
                grammar.wrapper_token_for(wrapper).unwrap()
            )]
        );
    }
    Ok(())
}
#[test]
fn test_malformed_inferred_origin_is_rejected_without_changing_bindings() {
    for (origins, cause) in [
        (json!(true), "inferred_wrapper_rules must be an array"),
        (json!([42]), "inferred_wrapper_rules entry must be a string"),
        (
            json!(["unknown"]),
            "inferred wrapper origin 'unknown' has no token binding",
        ),
    ] {
        let mut value = source_value(true, false);
        value["inferred_wrapper_rules"] = origins;
        let result = from_json(&value).and_then(|source| GrammarJsConverter::new(source).convert());
        assert!(result.is_err());
        assert!(format!("{:#}", result.unwrap_err()).contains(cause));
    }
}
#[test]
fn test_imported_grammar_js_choice_retains_only_terminal_binding() -> anyhow::Result<()> {
    for leaf in ["/[a-z]+/", "'q'"] {
        let source = format!(
            "module.exports = grammar({{name:'imported_js',rules:{{source_file:$=>choice($.leaf,'!'),leaf:$=>{leaf}}}}});"
        );
        let grammar = adze_tool::grammar_js::GrammarJsParserV3::new(source).parse()?;
        assert!(grammar.inferred_wrapper_rules.contains("leaf"));
        assert_grammar_binding(&GrammarJsConverter::new(grammar).convert()?, true)?;
    }
    Ok(())
}
#[test]
fn test_imported_binding_is_stable_when_leaf_is_declared_before_reference() -> anyhow::Result<()> {
    for literal in [false, true] {
        for choice in [false, true] {
            let mut value = source_value(choice, literal);
            let rules = value["rules"].as_object_mut().unwrap();
            let leaf = rules.remove("leaf").unwrap();
            let entry = rules.remove("source_file").unwrap();
            rules.insert("leaf".into(), leaf);
            rules.insert("source_file".into(), entry);
            assert_grammar_binding(
                &GrammarJsConverter::new(from_json(&value)?).convert()?,
                choice,
            )?;
        }
    }
    Ok(())
}

//! Imported token bindings do not request semantic wrapper preservation.
use adze_ir::{Grammar, Symbol};
use adze_tool::grammar_js::{GrammarJsConverter, from_json};
use serde_json::json;

fn imported_leaf(choice: bool, literal: bool) -> anyhow::Result<Grammar> {
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
    let source = json!({"name": "imported_leaf_context", "start_symbol": "source_file", "rules": {
        "source_file": entry, "leaf": leaf,
    }});
    GrammarJsConverter::new(from_json(&source)?).convert()
}
fn assert_binding_is_terminal(choice: bool, literal: bool) -> anyhow::Result<()> {
    let grammar = imported_leaf(choice, literal)?;
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

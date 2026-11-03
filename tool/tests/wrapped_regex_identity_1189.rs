//! Explicit wrapper origins must survive identical regex diagnostic spellings.
use adze_ir::{Grammar, Symbol, SymbolId, TokenPattern};
use adze_tool::grammar_js::{GrammarJs, GrammarJsConverter, Rule as JsRule};

fn fixture() -> anyhow::Result<Grammar> {
    let mut source = GrammarJs::new("wrapped_identity".into());
    source.start_symbol = Some("Choice".into());
    for name in ["ASeed", "Choice_ASecond", "Choice_ZFirst"] {
        source.rules.insert(
            name.into(),
            JsRule::Pattern {
                value: r"\d+".into(),
            },
        );
        source
            .wrapper_token_relations
            .insert(name.into(), r"_/\d+/".into());
    }
    source.rules.insert(
        "Choice".into(),
        JsRule::Choice {
            members: vec![
                JsRule::Symbol {
                    name: "Choice_ZFirst".into(),
                },
                JsRule::Symbol {
                    name: "Choice_ASecond".into(),
                },
                JsRule::Pattern {
                    value: r"\d+".into(),
                },
            ],
        },
    );
    GrammarJsConverter::new(source).convert()
}
fn wrapper_token(grammar: &Grammar, name: &str) -> anyhow::Result<(SymbolId, SymbolId)> {
    let wrapper = grammar
        .find_symbol_by_name(name)
        .ok_or_else(|| anyhow::anyhow!("fixture wrapper {name}"))?;
    let rules = grammar
        .rules
        .get(&wrapper)
        .ok_or_else(|| anyhow::anyhow!("fixture wrapper rule {name}"))?;
    assert_eq!(rules.len(), 1);
    let [Symbol::Terminal(token)] = rules[0].rhs.as_slice() else {
        anyhow::bail!(
            "expected unit token rule for {name}, got {:?}",
            rules[0].rhs
        );
    };
    Ok((wrapper, *token))
}
#[test]
fn test_choice_preserves_explicit_wrappers_and_separate_inline_regex_identity() -> anyhow::Result<()>
{
    let grammar = fixture()?;
    grammar.validate()?;
    let owner = grammar
        .explicit_start_symbol()
        .ok_or_else(|| anyhow::anyhow!("explicit Choice start"))?;
    let first = wrapper_token(&grammar, "Choice_ZFirst")?;
    let second = wrapper_token(&grammar, "Choice_ASecond")?;
    let rules = &grammar.rules[&owner];
    assert_eq!(rules.len(), 3);
    assert!(
        rules
            .iter()
            .any(|rule| rule.rhs == [Symbol::NonTerminal(first.0)])
    );
    assert!(
        rules
            .iter()
            .any(|rule| rule.rhs == [Symbol::NonTerminal(second.0)])
    );
    let inline = rules
        .iter()
        .find_map(|rule| match rule.rhs.as_slice() {
            [Symbol::Terminal(token)] => Some(*token),
            _ => None,
        })
        .ok_or_else(|| anyhow::anyhow!("inline regex alternative"))?;
    assert_eq!(grammar.tokens[&inline].name, r"/\d+/");
    assert_eq!(
        grammar.tokens[&inline].pattern,
        TokenPattern::Regex(r"\d+".into())
    );
    assert_ne!(inline, first.1);
    assert_ne!(inline, second.1);
    Ok(())
}
#[test]
fn test_same_regex_wrapper_metadata_binds_its_actual_scoped_token() -> anyhow::Result<()> {
    let grammar = fixture()?;
    let mut backing = std::collections::BTreeSet::new();
    for name in ["ASeed", "Choice_ASecond", "Choice_ZFirst"] {
        let (wrapper, token) = wrapper_token(&grammar, name)?;
        assert_eq!(
            grammar.wrapper_token_for(wrapper),
            Some(token),
            "{name} must bind its own unit-rule token"
        );
        assert_eq!(grammar.tokens[&token].name, r"_/\d+/");
        assert_eq!(
            grammar.tokens[&token].pattern,
            TokenPattern::Regex(r"\d+".into())
        );
        backing.insert(token);
    }
    assert_eq!(
        backing.len(),
        3,
        "equal display names cannot collapse rule-scoped identities"
    );
    Ok(())
}

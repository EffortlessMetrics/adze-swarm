//! Producer-owned document roots survive the same identity boundaries as starts.
use adze_ir::optimizer::GrammarOptimizer;
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

fn grammar() -> Grammar {
    let mut grammar = Grammar::new("distinct_roles".into());
    grammar.tokens.insert(
        SymbolId(90),
        Token {
            name: "token".into(),
            pattern: TokenPattern::String("x".into()),
            fragile: false,
        },
    );
    for (id, target, name) in [
        (7, Symbol::Terminal(SymbolId(90)), "analysis"),
        (51, Symbol::NonTerminal(SymbolId(7)), "document_owner"),
    ] {
        grammar.rule_names.insert(SymbolId(id), name.into());
        grammar.add_rule(Rule {
            lhs: SymbolId(id),
            rhs: vec![target],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(id),
        });
    }
    grammar.set_start_symbol(SymbolId(7));
    grammar.document_start_symbol = Some(SymbolId(51));
    grammar.refresh_production_ids().unwrap();
    grammar
}
#[test]
fn test_document_and_analysis_roles_survive_optimizer_and_json() {
    let mut grammar = grammar();
    grammar.try_normalize().unwrap();
    assert_eq!(grammar.document_start_symbol, Some(SymbolId(51)));
    assert_eq!(grammar.explicit_start_symbol(), Some(SymbolId(7)));
    GrammarOptimizer::new().optimize(&mut grammar);
    let analysis = grammar.find_symbol_by_name("analysis").unwrap();
    let document = grammar.find_symbol_by_name("document_owner").unwrap();
    assert_ne!(analysis, document);
    assert_eq!(grammar.explicit_start_symbol(), Some(analysis));
    assert_eq!(grammar.document_start_symbol, Some(document));
    assert!(grammar.rules.contains_key(&document));
    let restored: Grammar =
        serde_json::from_str(&serde_json::to_string(&grammar).unwrap()).unwrap();
    assert_eq!(restored.document_start_symbol, Some(document));
    assert_eq!(restored.explicit_start_symbol(), Some(analysis));
}
#[test]
fn test_document_role_remaps_and_rejects_stale_owner() {
    let mut grammar = grammar();
    let remap = std::collections::HashMap::from([
        (SymbolId(7), SymbolId(17)),
        (SymbolId(51), SymbolId(41)),
    ]);
    adze_ir::remap_compiler_identity(&mut grammar, &remap);
    assert_eq!(grammar.explicit_start_symbol(), Some(SymbolId(17)));
    assert_eq!(grammar.document_start_symbol, Some(SymbolId(41)));
    assert!(grammar.validate().is_err());
}
#[test]
fn test_older_serialized_grammar_has_no_invented_document_role() {
    let mut value = serde_json::to_value(grammar()).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .remove("document_start_symbol");
    let restored: Grammar = serde_json::from_value(value).unwrap();
    assert_eq!(restored.document_start_symbol, None);
    assert_eq!(restored.explicit_start_symbol(), Some(SymbolId(7)));
}

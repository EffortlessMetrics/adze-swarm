//! Characterization tests for compiler-identity metadata in the IR (#862 PR1).

use adze_ir::builder::GrammarBuilder;
use adze_ir::{
    Grammar, ProductionId, Rule, Symbol, SymbolId, WrapperTokenRelation,
    sorted_wrapper_token_relations,
};
use indexmap::IndexMap;

#[test]
fn explicit_start_symbol_roundtrips_in_json() {
    let grammar = GrammarBuilder::new("root9")
        .token("x", "x")
        .rule("Root9", vec!["x"])
        .rule("helper", vec!["x"])
        .start("Root9")
        .build();

    let root_id = grammar.find_symbol_by_name("Root9").unwrap();
    assert_eq!(grammar.explicit_start_symbol(), Some(root_id));
    assert_eq!(grammar.start_symbol(), Some(root_id));

    let json = serde_json::to_string(&grammar).expect("serialize grammar");
    let decoded: Grammar = serde_json::from_str(&json).expect("deserialize grammar");
    assert_eq!(decoded.explicit_start_symbol(), Some(root_id));
    assert_eq!(decoded.start_symbol(), Some(root_id));
}

#[test]
fn explicit_start_symbol_survives_rule_map_reordering() {
    let mut grammar = GrammarBuilder::new("reordered")
        .token("t", "t")
        .rule("Root9", vec!["t"])
        .rule("noise", vec!["t"])
        .start("Root9")
        .build();

    let root_id = grammar.find_symbol_by_name("Root9").unwrap();
    let noise_id = grammar.find_symbol_by_name("noise").unwrap();

    // Reverse insertion order in the rules map.
    let root_rules = grammar.rules.shift_remove(&root_id).unwrap();
    let noise_rules = grammar.rules.shift_remove(&noise_id).unwrap();
    grammar.rules.insert(noise_id, noise_rules);
    grammar.rules.insert(root_id, root_rules);

    assert_eq!(grammar.start_symbol(), Some(root_id));
}

#[test]
fn explicit_start_symbol_beats_source_file_heuristic() {
    let mut grammar = Grammar::new("adversarial".to_string());
    let source_id = SymbolId(10);
    let root_id = SymbolId(11);

    for (id, name) in [(source_id, "source_file"), (root_id, "Root9")] {
        grammar.rule_names.insert(id, name.to_string());
        grammar.rules.entry(id).or_default().push(Rule {
            lhs: id,
            rhs: vec![Symbol::Epsilon],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });
    }
    grammar.set_start_symbol(root_id);

    assert_eq!(grammar.start_symbol(), Some(root_id));
}

#[test]
fn wrapper_token_relations_roundtrip_in_json() {
    let grammar = GrammarBuilder::new("wrappers")
        .token("identifier", r"[a-zA-Z_][a-zA-Z0-9_]*")
        .token("id", "id")
        .rule("Identifier", vec!["identifier"])
        .wrapper_token("Identifier", "identifier")
        .build();

    let wrapper_id = grammar.find_symbol_by_name("Identifier").unwrap();
    let token_id = grammar
        .tokens
        .iter()
        .find(|(_, token)| token.name == "identifier")
        .map(|(id, _)| *id)
        .unwrap();

    assert_eq!(grammar.wrapper_token_for(wrapper_id), Some(token_id));

    let json = serde_json::to_string(&grammar).expect("serialize grammar");
    let decoded: Grammar = serde_json::from_str(&json).expect("deserialize grammar");
    assert_eq!(decoded.wrapper_token_for(wrapper_id), Some(token_id));
}

#[test]
fn wrapper_token_relations_sorted_deterministically() {
    let mut relations = IndexMap::new();
    relations.insert(SymbolId(30), SymbolId(3));
    relations.insert(SymbolId(10), SymbolId(1));
    relations.insert(SymbolId(20), SymbolId(2));

    let sorted = sorted_wrapper_token_relations(&relations);
    assert_eq!(
        sorted,
        vec![
            WrapperTokenRelation {
                wrapper: SymbolId(10),
                token: SymbolId(1),
            },
            WrapperTokenRelation {
                wrapper: SymbolId(20),
                token: SymbolId(2),
            },
            WrapperTokenRelation {
                wrapper: SymbolId(30),
                token: SymbolId(3),
            },
        ]
    );
}

#[test]
fn start_symbol_is_none_without_explicit_metadata() {
    let mut grammar = Grammar::new("legacy".to_string());
    let sf_id = SymbolId(10);
    grammar.rule_names.insert(sf_id, "source_file".into());
    grammar.rules.entry(sf_id).or_default().push(Rule {
        lhs: sf_id,
        rhs: vec![Symbol::Epsilon],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    assert_eq!(grammar.explicit_start_symbol(), None);
    assert_eq!(grammar.start_symbol(), None);
}

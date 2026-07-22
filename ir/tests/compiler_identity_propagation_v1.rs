//! Propagation tests for compiler-identity metadata (#862 PR4).

use adze_ir::builder::GrammarBuilder;
use adze_ir::optimizer::GrammarOptimizer;
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId};

#[test]
fn optimizer_renumber_preserves_explicit_start_symbol() {
    let mut grammar = GrammarBuilder::new("root9")
        .token("x", "x")
        .rule("Root9", vec!["x"])
        .rule("noise", vec!["x"])
        .start("Root9")
        .build();

    let root_id = grammar.find_symbol_by_name("Root9").unwrap();
    assert_eq!(grammar.explicit_start_symbol(), Some(root_id));

    let mut optimizer = GrammarOptimizer::new();
    optimizer.optimize(&mut grammar);

    let remapped_root = grammar.find_symbol_by_name("Root9").unwrap();
    assert_eq!(grammar.explicit_start_symbol(), Some(remapped_root));
    assert_eq!(grammar.start_symbol(), Some(remapped_root));
}

#[test]
fn optimizer_renumber_preserves_wrapper_token_relations() {
    let mut grammar = GrammarBuilder::new("wrappers")
        .token("identifier", r"[a-z]+")
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

    let mut optimizer = GrammarOptimizer::new();
    optimizer.optimize(&mut grammar);

    let remapped_wrapper = grammar.find_symbol_by_name("Identifier").unwrap();
    let remapped_token = grammar
        .tokens
        .iter()
        .find(|(_, token)| token.name == "identifier")
        .map(|(id, _)| *id)
        .unwrap();
    assert_eq!(
        grammar.wrapper_token_for(remapped_wrapper),
        Some(remapped_token)
    );
}

#[test]
fn explicit_start_symbol_survives_json_roundtrip_after_optimizer() {
    let mut grammar = GrammarBuilder::new("adversarial")
        .token("ws", r"\s+")
        .rule("Root9", vec!["ws"])
        .rule("source_file", vec!["ws"])
        .start("Root9")
        .build();

    let mut optimizer = GrammarOptimizer::new();
    optimizer.optimize(&mut grammar);

    let json = serde_json::to_string(&grammar).expect("serialize grammar");
    let decoded: Grammar = serde_json::from_str(&json).expect("deserialize grammar");
    let root_id = decoded.find_symbol_by_name("Root9").unwrap();

    assert_eq!(decoded.explicit_start_symbol(), Some(root_id));
    assert_eq!(decoded.start_symbol(), Some(root_id));
}

#[test]
fn explicit_start_beats_source_file_after_optimizer_renumber() {
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

    let mut optimizer = GrammarOptimizer::new();
    optimizer.optimize(&mut grammar);

    let remapped_root = grammar.find_symbol_by_name("Root9").unwrap();
    assert_eq!(grammar.explicit_start_symbol(), Some(remapped_root));
    assert_eq!(grammar.start_symbol(), Some(remapped_root));
}

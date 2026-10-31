//! #1060: RuleId (dense syntax rule) vs ProductionId (shared metadata).
use adze_glr_core::{FirstFollowSets, ItemSet, LRItem, StaticPrecedenceResolver};
use adze_ir::{
    Associativity, Grammar, PrecedenceKind, ProductionId, Rule, RuleId, Symbol, SymbolId, Token,
    TokenPattern,
};

fn shared_grammar() -> Grammar {
    let mut g = Grammar::new("shared".into());
    let (s, a_nt, a_tok) = (SymbolId(1), SymbolId(2), SymbolId(3));
    g.tokens.insert(
        a_tok,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    g.rule_names.insert(s, "S".into());
    g.rule_names.insert(a_nt, "A".into());
    g.start_symbol = Some(s);
    // S -> A (prod 5), A -> a (prod 0), A -> epsilon (prod 0, shared)
    g.add_rule(Rule {
        lhs: s,
        rhs: vec![Symbol::NonTerminal(a_nt)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(5),
    });
    g.add_rule(Rule {
        lhs: a_nt,
        rhs: vec![Symbol::Terminal(a_tok)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    g.add_rule(Rule {
        lhs: a_nt,
        rhs: vec![Symbol::Epsilon],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    g.refresh_production_ids().unwrap();
    g
}

#[test]
fn correct_rule_id_lookup_does_not_infer_from_production() {
    let g = shared_grammar();
    assert_eq!(g.production_id_for_rule(RuleId(0)), Some(ProductionId(5)));
    assert_eq!(
        g.rule_ids_for_production(ProductionId(0))
            .collect::<Vec<_>>(),
        vec![RuleId(1), RuleId(2)]
    );
    let item = LRItem::new(RuleId(0), 0, SymbolId(0));
    assert_eq!(
        item.next_symbol(&g),
        Some(&Symbol::NonTerminal(SymbolId(2)))
    );
}

#[test]
fn closure_keeps_both_shared_production_alternatives() {
    let g = shared_grammar();
    let ff = FirstFollowSets::compute(&g).expect("ff");
    let mut set = ItemSet::new(adze_ir::StateId(0));
    set.add_item(LRItem::new(RuleId(0), 0, SymbolId(0)));
    set.closure(&g, &ff).expect("closure");
    assert!(
        set.items.iter().any(|i| i.rule_id == RuleId(1)),
        "A->a lost: {:?}",
        set.items
    );
    assert!(
        set.items.iter().any(|i| i.rule_id == RuleId(2)),
        "A->eps lost: {:?}",
        set.items
    );
    let eps = LRItem::new(RuleId(2), 0, SymbolId(0));
    assert!(
        eps.is_reduce_item(&g),
        "epsilon alternative must be a reduce item"
    );
}

#[test]
fn precedence_is_keyed_by_rule_not_shared_production() {
    let mut g = shared_grammar();
    // Give the two ProductionId(0)-sharing rules different precedence via distinct productions would share key under old code; here give S and A->a same production 5 with different levels by editing:
    g.rules.get_mut(&SymbolId(2)).unwrap()[0].production_id = ProductionId(5);
    g.rules.get_mut(&SymbolId(1)).unwrap()[0].precedence = Some(PrecedenceKind::Static(1));
    g.rules.get_mut(&SymbolId(1)).unwrap()[0].associativity = Some(Associativity::Left);
    g.rules.get_mut(&SymbolId(2)).unwrap()[0].precedence = Some(PrecedenceKind::Static(9));
    g.rules.get_mut(&SymbolId(2)).unwrap()[0].associativity = Some(Associativity::Left);
    let resolver = StaticPrecedenceResolver::from_grammar(&g);
    assert_eq!(
        resolver.rule_precedence(RuleId(0)).map(|p| p.level),
        Some(1)
    );
    assert_eq!(
        resolver.rule_precedence(RuleId(1)).map(|p| p.level),
        Some(9)
    );
}

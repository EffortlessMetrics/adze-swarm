//! #980: `Grammar::production_ids` is the total, many-to-one
//! `RuleId -> ProductionId` map. `RuleId` is the dense flattened index of
//! `Grammar::all_rules()`; `ProductionId` stays a grammar-local slot that
//! distinct rules may legitimately share.

use adze_ir::builder::GrammarBuilder;
use adze_ir::{Grammar, ProductionId, Rule, RuleId, Symbol, SymbolId};

fn rule(lhs: u16, production_id: u16) -> Rule {
    Rule {
        lhs: SymbolId(lhs),
        rhs: vec![Symbol::Epsilon],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(production_id),
    }
}

#[test]
fn production_ids_total_after_builder_build() {
    let grammar = GrammarBuilder::new("total")
        .token("A", "a")
        .rule("s", vec!["A"])
        .rule("t", vec!["A"])
        .start("s")
        .build();

    let total = grammar.all_rules().count();
    assert!(total >= 2, "fixture should have at least two rules");
    assert_eq!(
        grammar.production_ids.len(),
        total,
        "production_ids must cover every rule after build()"
    );
    for (index, rule) in grammar.all_rules().enumerate() {
        let rule_id = RuleId(u16::try_from(index).unwrap());
        assert_eq!(
            grammar.production_id_for_rule(rule_id),
            Some(rule.production_id),
            "RuleId {index} must map to its rule's production"
        );
    }
}

#[test]
fn production_ids_many_to_one_for_shared_production() {
    let mut grammar = Grammar::new("shared".to_string());
    grammar.add_rule(rule(1, 100));
    grammar.add_rule(rule(1, 100));
    grammar.add_rule(rule(2, 100));
    grammar.refresh_production_ids();

    assert_eq!(grammar.production_ids.len(), 3);
    assert_eq!(
        grammar.production_id_for_rule(RuleId(0)),
        Some(ProductionId(100))
    );
    assert_eq!(
        grammar.production_id_for_rule(RuleId(2)),
        Some(ProductionId(100))
    );
    let sharing: Vec<RuleId> = grammar.rule_ids_for_production(ProductionId(100)).collect();
    assert_eq!(sharing, vec![RuleId(0), RuleId(1), RuleId(2)]);
}

#[test]
fn production_ids_total_for_sparse_production_ids() {
    let mut grammar = Grammar::new("sparse".to_string());
    grammar.add_rule(rule(1, 100));
    grammar.add_rule(rule(2, 200));
    grammar.refresh_production_ids();

    assert_eq!(grammar.production_ids.len(), 2);
    assert_eq!(
        grammar.production_id_for_rule(RuleId(0)),
        Some(ProductionId(100))
    );
    assert_eq!(
        grammar.production_id_for_rule(RuleId(1)),
        Some(ProductionId(200))
    );
    assert_eq!(grammar.production_id_for_rule(RuleId(2)), None);
    assert_eq!(
        grammar.rule_ids_for_production(ProductionId(7)).count(),
        0,
        "unrelated production has no rules"
    );
}

#[test]
fn production_ids_refresh_replaces_stale_entries() {
    let mut grammar = Grammar::new("refresh".to_string());
    grammar.add_rule(rule(1, 1));
    grammar.refresh_production_ids();
    assert_eq!(grammar.production_ids.len(), 1);

    grammar.add_rule(rule(1, 2));
    grammar.refresh_production_ids();
    assert_eq!(
        grammar.production_ids.len(),
        2,
        "refresh must rebuild the map from current rules, not append"
    );
    assert_eq!(
        grammar.production_id_for_rule(RuleId(1)),
        Some(ProductionId(2))
    );
}

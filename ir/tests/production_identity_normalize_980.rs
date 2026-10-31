//! #980 PR3: normalization helpers get fresh production slots, the total
//! `RuleId -> ProductionId` map is refreshed at normalize/optimize
//! boundaries, and conflicting production shares are rejected.

use adze_ir::optimizer::{GrammarOptimizer, optimize_grammar};
use adze_ir::{
    AliasSequence, FieldId, Grammar, GrammarError, GrammarValidator, ProductionId, Rule, RuleId,
    Symbol, SymbolId, ValidationError,
};

fn rule(lhs: u16, rhs: Vec<Symbol>, production_id: u16) -> Rule {
    Rule {
        lhs: SymbolId(lhs),
        rhs,
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(production_id),
    }
}

fn complex_grammar() -> Grammar {
    let mut grammar = Grammar::new("complex".to_string());
    grammar.add_rule(rule(
        1,
        vec![
            Symbol::Optional(Box::new(Symbol::Terminal(SymbolId(2)))),
            Symbol::Repeat(Box::new(Symbol::Terminal(SymbolId(3)))),
            Symbol::RepeatOne(Box::new(Symbol::Terminal(SymbolId(4)))),
            Symbol::Choice(vec![
                Symbol::Terminal(SymbolId(5)),
                Symbol::Terminal(SymbolId(6)),
            ]),
        ],
        10,
    ));
    grammar
}

fn assert_total(grammar: &Grammar) {
    let total = grammar.all_rules().count();
    assert_eq!(grammar.production_ids.len(), total, "map must be total");
    for (index, rule) in grammar.all_rules().enumerate() {
        let rule_id = RuleId(u16::try_from(index).unwrap());
        assert_eq!(
            grammar.production_id_for_rule(rule_id),
            Some(rule.production_id)
        );
        // Lookup by RuleId resolves to the exact rule.
        assert_eq!(grammar.all_rules().nth(index), Some(rule));
    }
}

#[test]
fn normalize_helpers_have_distinct_fresh_production_slots() {
    let mut grammar = complex_grammar();
    grammar.normalize();

    let helpers: Vec<&Rule> = grammar.all_rules().filter(|r| r.lhs.0 >= 1001).collect();
    assert!(
        helpers.len() >= 6,
        "expected helper rules, got {}",
        helpers.len()
    );
    let mut seen = std::collections::HashSet::new();
    for helper in &helpers {
        assert_ne!(
            helper.production_id,
            ProductionId(0),
            "helper must not share slot 0"
        );
        assert!(
            helper.production_id.0 > 10,
            "helper slot must be fresh: {:?}",
            helper.production_id
        );
        assert!(
            seen.insert(helper.production_id),
            "duplicate helper slot {:?}",
            helper.production_id
        );
        // A fresh helper slot inverts to exactly one rule.
        assert_eq!(
            grammar
                .rule_ids_for_production(helper.production_id)
                .count(),
            1
        );
    }
}

#[test]
fn production_ids_total_after_normalize() {
    let mut grammar = complex_grammar();
    grammar.normalize();
    assert_total(&grammar);
}

#[test]
fn production_ids_total_after_optimize() {
    // Grammar::optimize boundary
    let mut grammar = complex_grammar();
    grammar.production_ids.clear();
    grammar.optimize();
    assert_total(&grammar);

    // GrammarOptimizer boundary (mutates/removes rules)
    let mut grammar = complex_grammar();
    grammar.normalize();
    let mut optimizer = GrammarOptimizer::new();
    optimizer.optimize(&mut grammar);
    assert_total(&grammar);

    // optimize_grammar convenience boundary
    let grammar = optimize_grammar(complex_grammar()).expect("optimize_grammar");
    assert_total(&grammar);
}

fn share_grammar(fields_a: Vec<(FieldId, usize)>, fields_b: Vec<(FieldId, usize)>) -> Grammar {
    let mut grammar = Grammar::new("share".to_string());
    grammar.fields.insert(FieldId(1), "a".to_string());
    grammar.fields.insert(FieldId(2), "b".to_string());
    let mut a = rule(1, vec![Symbol::Epsilon], 42);
    a.fields = fields_a;
    let mut b = rule(1, vec![Symbol::Epsilon], 42);
    b.fields = fields_b;
    grammar.add_rule(a);
    grammar.add_rule(b);
    grammar.refresh_production_ids().unwrap();
    grammar
}

#[test]
fn conflicting_field_share_is_rejected() {
    let grammar = share_grammar(vec![(FieldId(1), 0)], vec![(FieldId(2), 0)]);
    let err = grammar.validate().expect_err("conflicting share must fail");
    assert!(
        matches!(
            err,
            GrammarError::ConflictingProductionShare {
                production_id: ProductionId(42),
                ..
            }
        ),
        "unexpected error: {err:?}"
    );
    let result = GrammarValidator::new().validate(&grammar);
    assert!(
        result.errors.iter().any(|e| matches!(
            e,
            ValidationError::ConflictingProductionShare {
                production_id: ProductionId(42),
                ..
            }
        )),
        "validator must report conflict: {:?}",
        result.errors
    );
}

#[test]
fn conflicting_alias_share_is_rejected() {
    let mut grammar = share_grammar(vec![], vec![]);
    // Second rule has a longer RHS, so one alias sequence cannot describe both.
    grammar.rules.get_mut(&SymbolId(1)).unwrap()[1].rhs = vec![Symbol::Epsilon, Symbol::Epsilon];
    grammar.alias_sequences.insert(
        ProductionId(42),
        AliasSequence {
            aliases: vec![Some("x".into()), None],
        },
    );
    grammar.refresh_production_ids().unwrap();
    assert!(matches!(
        grammar.validate(),
        Err(GrammarError::ConflictingProductionShare {
            production_id: ProductionId(42),
            ..
        })
    ));
}

#[test]
fn equal_metadata_share_remains_allowed() {
    let grammar = share_grammar(vec![(FieldId(1), 0)], vec![(FieldId(1), 0)]);
    assert!(grammar.validate().is_ok(), "{:?}", grammar.validate());
    let result = GrammarValidator::new().validate(&grammar);
    assert!(
        !result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::ConflictingProductionShare { .. }))
    );
}

#[test]
fn equal_alias_share_remains_allowed() {
    let mut grammar = share_grammar(vec![], vec![]);
    grammar.alias_sequences.insert(
        ProductionId(42),
        AliasSequence {
            aliases: vec![Some("x".into())],
        },
    );
    assert!(grammar.validate().is_ok(), "{:?}", grammar.validate());
}

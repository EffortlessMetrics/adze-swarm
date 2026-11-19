//! Independent explicit-IR producer oracles. No selected-node or ABI claim.
use adze_ir::{FieldId, Grammar, ProductionId, Rule, RuleId, SourceRuleId, Symbol, SymbolId};

fn term(n: u16) -> Symbol {
    Symbol::Terminal(SymbolId(n))
}
fn named(n: u16) -> Symbol {
    Symbol::NonTerminal(SymbolId(n))
}
fn repeat(item: Symbol) -> Symbol {
    Symbol::Repeat(Box::new(item))
}
fn add(g: &mut Grammar, lhs: u16, rhs: Vec<Symbol>, fields: Vec<(FieldId, usize)>, p: u16) {
    g.add_rule(Rule {
        lhs: SymbolId(lhs),
        rhs,
        fields,
        production_id: ProductionId(p),
        associativity: None,
        precedence: None,
    });
}
fn fixture() -> Grammar {
    let mut g = Grammar::new("bracketed-items".into());
    g.fields.insert(FieldId(1), "items".into());
    g.rule_names.insert(SymbolId(11), "item".into());
    add(
        &mut g,
        10,
        vec![term(2), repeat(named(11)), term(3)],
        vec![(FieldId(1), 1)],
        0,
    );
    add(&mut g, 11, vec![term(1)], vec![], 1);
    g
}
#[test]
fn test_records_actual_bracketed_item_producer_without_changing_native_or_wire_data() {
    let mut g = fixture();
    let mut legacy = g.clone();
    legacy.try_normalize().unwrap();
    let origins = g.try_normalize_with_repeat_field_origins().unwrap();
    assert_eq!(g, legacy);
    assert_eq!(
        postcard::to_stdvec(&g).unwrap(),
        postcard::to_stdvec(&legacy).unwrap()
    );
    let [site] = origins.as_slice() else {
        panic!("one source field site")
    };
    assert_eq!(
        site.source_rule(),
        SourceRuleId {
            owner: SymbolId(10),
            alternative: 0
        }
    );
    assert_eq!(
        (
            site.field(),
            site.source_rhs_position(),
            site.lowered_rhs_position()
        ),
        (FieldId(1), 1, 1)
    );
    assert_eq!(site.helper(), SymbolId(1011));
    assert_eq!(site.item(), &named(11)); // The target is the item, not its letter (token 1).
    assert_eq!(g.production_id_for_rule(RuleId(0)), Some(ProductionId(2)));
    assert_eq!(g.production_id_for_rule(RuleId(1)), Some(ProductionId(3)));
    let helper_rules = &g.rules[&site.helper()];
    assert_eq!(
        helper_rules[site.recursive_alternative()].rhs,
        [named(1011), named(11)]
    );
    assert_eq!(
        helper_rules[site.empty_alternative()].rhs,
        [Symbol::Epsilon]
    );
    assert!(
        g.try_normalize_with_repeat_field_origins()
            .unwrap()
            .is_empty()
    );
}
#[test]
fn test_distinct_alternatives_and_sites_survive_shared_productions_and_sequence_expansion() {
    let mut g = Grammar::new("distinct".into());
    g.fields.insert(FieldId(1), "xs".into());
    for atom in [term(3), Symbol::External(SymbolId(3))] {
        add(
            &mut g,
            10,
            vec![
                Symbol::Sequence(vec![term(1), Symbol::Sequence(vec![term(2), term(2)])]),
                repeat(atom.clone()),
                repeat(atom),
            ],
            vec![(FieldId(1), 1), (FieldId(1), 2)],
            7,
        );
    }
    let sites = g.try_normalize_with_repeat_field_origins().unwrap();
    assert_eq!(sites.len(), 4);
    let mut helpers = std::collections::BTreeSet::new();
    for (i, site) in sites.iter().enumerate() {
        assert_eq!(site.source_rule().alternative, i / 2);
        assert_eq!(site.source_rhs_position(), 1 + i % 2);
        assert_eq!(site.lowered_rhs_position(), 3 + i % 2);
        assert_eq!(
            site.item(),
            &if i < 2 {
                term(3)
            } else {
                Symbol::External(SymbolId(3))
            }
        );
        assert!(helpers.insert(site.helper()));
        let alternative = &g.rules[&SymbolId(10)][i / 2];
        assert_eq!(alternative.production_id, ProductionId(7));
        assert_eq!(
            alternative.rhs[3 + i % 2],
            Symbol::NonTerminal(site.helper())
        );
    }
}
#[test]
fn test_matching_normalized_shapes_and_ids_cannot_create_a_producer_record() {
    let mut original = fixture();
    let sites = original.try_normalize_with_repeat_field_origins().unwrap();
    let site = &sites[0];
    let mut foreign = Grammar::new("unrelated".into());
    foreign.fields.insert(site.field(), "other-field".into());
    add(
        &mut foreign,
        10,
        vec![term(2), named(1011), term(3)],
        vec![(FieldId(1), 1)],
        10,
    );
    add(&mut foreign, 1011, vec![named(1011), named(11)], vec![], 20);
    add(&mut foreign, 1011, vec![Symbol::Epsilon], vec![], 30);
    add(&mut foreign, 11, vec![term(1)], vec![], 40);
    assert_eq!(
        foreign.rules[&site.source_rule().owner][0].rhs[site.lowered_rhs_position()],
        Symbol::NonTerminal(site.helper())
    );
    assert!(
        foreign
            .try_normalize_with_repeat_field_origins()
            .unwrap()
            .is_empty()
    );
}
#[test]
fn test_no_invented_origins_for_unfielded_composite_or_non_source_helpers() {
    let mut g = Grammar::new("unsupported".into());
    g.fields.insert(FieldId(1), "xs".into());
    add(
        &mut g,
        10,
        vec![
            repeat(term(1)),
            repeat(Symbol::Sequence(vec![term(1), term(2)])),
            Symbol::RepeatOne(Box::new(term(2))),
            Symbol::Sequence(vec![repeat(term(2))]),
        ],
        vec![(FieldId(1), 1), (FieldId(1), 2), (FieldId(1), 3)],
        0,
    );
    assert!(
        g.try_normalize_with_repeat_field_origins()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn test_failed_normalization_leaks_neither_mutated_rules_nor_partial_origins() {
    use adze_ir::{AliasSequence, GrammarError};
    let mut g = fixture();
    // The first RHS slot allocates its repeat carrier before a later slot's
    // multi-child sequence refuses the alias. This exercises rollback, not
    // just a preflight error.
    let rule = &mut g.rules.get_mut(&SymbolId(10)).unwrap()[0];
    rule.rhs = vec![repeat(named(11)), Symbol::Sequence(vec![term(2), term(3)])];
    rule.fields = vec![(FieldId(1), 0)];
    g.alias_sequences.insert(
        ProductionId(0),
        AliasSequence {
            aliases: vec![None, Some("wide".to_owned())],
        },
    );
    let before = g.clone();
    assert!(matches!(
        g.try_normalize_with_repeat_field_origins(),
        Err(GrammarError::AliasOnExpandedSequence { .. })
    ));
    assert_eq!(g, before);
}

//! #1059 transactional normalization + #981 positional alias origins.
use adze_ir::{
    AliasSequence, FieldId, Grammar, GrammarError, ProductionId, Rule, Symbol, SymbolId, Token,
    TokenPattern,
};

fn tok(id: u16) -> Token {
    Token {
        name: format!("t{id}"),
        pattern: TokenPattern::String(format!("t{id}")),
        fragile: false,
    }
}
fn rule(lhs: u16, rhs: Vec<Symbol>, prod: u16) -> Rule {
    Rule {
        lhs: SymbolId(lhs),
        rhs,
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(prod),
    }
}

#[test]
fn nested_repeatone_optional_exhaustion_leaves_original_unchanged() {
    let mut g = Grammar::new("exhaust".into());
    g.add_rule(rule(
        1,
        vec![Symbol::RepeatOne(Box::new(Symbol::Optional(Box::new(
            Symbol::Terminal(SymbolId(2)),
        ))))],
        7,
    ));
    for id in 2..=65533u16 {
        g.tokens.insert(SymbolId(id), tok(id));
    }
    // Free SymbolIds are exactly 65534, 65535; RepeatOne(Optional) needs 3 helpers.
    let before = g.clone();
    let err = g.try_normalize().expect_err("must exhaust");
    assert!(matches!(err, GrammarError::SymbolSpaceExhausted), "{err:?}");
    assert_eq!(g, before);
}

#[test]
fn sparse_free_ids_and_repeated_normalization_are_stable() {
    let mut g = Grammar::new("sparse".into());
    g.tokens.insert(SymbolId(10), tok(10));
    g.tokens.insert(SymbolId(60000), tok(60000));
    g.add_rule(rule(
        5,
        vec![Symbol::Optional(Box::new(Symbol::Terminal(SymbolId(10))))],
        3,
    ));
    g.try_normalize().expect("sparse ids have room");
    let after_first = g.clone();
    assert!(g.all_rules().all(|r| r.lhs != SymbolId(0)));
    g.try_normalize().expect("second normalize");
    assert_eq!(g, after_first, "repeated normalization must be idempotent");
}

#[test]
fn helper_never_collides_with_eof_or_category_ids() {
    // Legacy hand-built grammars may use SymbolId(0) as an LHS/token; the
    // helper allocator must still never hand out 0 or an occupied id.
    let mut g = Grammar::new("eof".into());
    g.tokens.insert(SymbolId(0), tok(0));
    g.tokens.insert(SymbolId(2), tok(2));
    g.add_rule(rule(
        1,
        vec![Symbol::Optional(Box::new(Symbol::Terminal(SymbolId(2))))],
        0,
    ));
    g.try_normalize().expect("normalize");
    let helpers: Vec<_> = g
        .all_rules()
        .map(|r| r.lhs)
        .filter(|l| *l != SymbolId(1))
        .collect();
    assert!(!helpers.is_empty() && helpers.iter().all(|l| *l != SymbolId(0)));
}

#[test]
fn builder_symbol_exhaustion_is_typed_not_wrapped_to_eof() {
    let mut builder = adze_ir::builder::GrammarBuilder::new("cap");
    for i in 1..=65_536u32 {
        builder = builder.token(&format!("TOKEN_{i}"), "a");
    }
    let err = builder.try_build().expect_err("symbol capacity");
    assert!(matches!(err, GrammarError::SymbolSpaceExhausted), "{err:?}");
}

#[test]
fn builder_production_exhaustion_is_typed() {
    let mut builder = adze_ir::builder::GrammarBuilder::new("cap2").token("a", "a");
    for _ in 0..65_537 {
        builder = builder.rule("s", vec!["a"]);
    }
    let err = builder.try_build().expect_err("production capacity");
    assert!(
        matches!(err, GrammarError::ProductionSpaceExhausted),
        "{err:?}"
    );
}

#[test]
fn primitive_grammar_at_saturated_production_max_needs_no_helpers() {
    // #1060 capacity packet 1: `S -> a` at ProductionId(65535) requires no
    // helper rules, so normalization must succeed without allocating a
    // production slot (the pre-#1081 allocator panicked here).
    let mut g = Grammar::new("saturated_prim".into());
    g.tokens.insert(SymbolId(2), tok(2));
    g.add_rule(rule(1, vec![Symbol::Terminal(SymbolId(2))], 65535));
    g.try_normalize()
        .expect("no helpers needed at saturated max");
    assert_eq!(g.all_rules().count(), 1);
    let only = g.all_rules().next().expect("one rule");
    assert_eq!(only.production_id, ProductionId(65535));
    assert_eq!(only.rhs, vec![Symbol::Terminal(SymbolId(2))]);
    g.validate().expect("saturated primitive validates");
}

#[test]
fn single_choice_consumes_last_production_slot() {
    // #1060 capacity packet 2: with max ProductionId(65534) one slot
    // remains; the single Choice alternative must take ProductionId(65535)
    // (the pre-#1081 allocator panicked before returning it).
    let mut g = Grammar::new("last_slot".into());
    g.tokens.insert(SymbolId(2), tok(2));
    g.add_rule(rule(
        1,
        vec![Symbol::Choice(vec![Symbol::Terminal(SymbolId(2))])],
        65534,
    ));
    g.try_normalize().expect("last slot must be usable");
    let mut prods: Vec<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    prods.sort_unstable();
    assert_eq!(prods, vec![65534, 65535]);
    g.validate()
        .expect("normalized last-slot grammar validates");
}

#[test]
fn single_choice_past_last_slot_fails_typed_and_unchanged() {
    // Companion to the last-slot case: with max ProductionId(65535) no
    // slot remains, so the same shape must fail typed and atomically.
    let mut g = Grammar::new("no_slot".into());
    g.tokens.insert(SymbolId(2), tok(2));
    g.add_rule(rule(
        1,
        vec![Symbol::Choice(vec![Symbol::Terminal(SymbolId(2))])],
        65535,
    ));
    let before = g.clone();
    let err = g.try_normalize().expect_err("no production slot remains");
    assert!(
        matches!(err, GrammarError::ProductionSpaceExhausted),
        "{err:?}"
    );
    assert_eq!(g, before, "failed normalize must leave grammar unchanged");
}

#[test]
fn alias_and_field_after_sequence_keep_original_child() {
    let mut g = Grammar::new("alias_origin".into());
    for id in [2u16, 3, 4] {
        g.tokens.insert(SymbolId(id), tok(id));
    }
    g.fields.insert(FieldId(0), "c_field".into());
    let mut r = rule(
        1,
        vec![
            Symbol::Sequence(vec![
                Symbol::Terminal(SymbolId(2)),
                Symbol::Terminal(SymbolId(3)),
            ]),
            Symbol::Terminal(SymbolId(4)),
        ],
        7,
    );
    r.fields = vec![(FieldId(0), 1)];
    g.add_rule(r);
    g.alias_sequences.insert(
        ProductionId(7),
        AliasSequence {
            aliases: vec![None, Some("c".into())],
        },
    );
    g.max_alias_sequence_length = 2;
    g.try_normalize().expect("normalize");
    let s = g.all_rules().find(|r| r.lhs == SymbolId(1)).unwrap();
    assert_eq!(s.rhs.len(), 3);
    assert_eq!(s.fields, vec![(FieldId(0), 2)]);
    assert_eq!(
        g.alias_sequences[&ProductionId(7)].aliases,
        vec![None, None, Some("c".to_string())]
    );
    assert_eq!(g.max_alias_sequence_length, 3);
    g.validate().expect("remapped alias validates");
    // downstream origin: position 2 is terminal c (4)
    assert_eq!(s.rhs[2], Symbol::Terminal(SymbolId(4)));
}

#[test]
fn alias_on_multi_child_sequence_is_typed_error_and_atomic() {
    let mut g = Grammar::new("alias_ambig".into());
    for id in [2u16, 3] {
        g.tokens.insert(SymbolId(id), tok(id));
    }
    g.add_rule(rule(
        1,
        vec![Symbol::Sequence(vec![
            Symbol::Terminal(SymbolId(2)),
            Symbol::Terminal(SymbolId(3)),
        ])],
        7,
    ));
    g.alias_sequences.insert(
        ProductionId(7),
        AliasSequence {
            aliases: vec![Some("both".into())],
        },
    );
    let before = g.clone();
    assert!(matches!(
        g.try_normalize(),
        Err(GrammarError::AliasOnExpandedSequence { .. })
    ));
    assert_eq!(g, before);
}

#[test]
fn rule_space_exhaustion_is_typed_not_panic_and_unchanged() {
    let mut g = Grammar::new("rule_cap".into());
    g.tokens.insert(SymbolId(2), tok(2));
    for _ in 0..65_537 {
        g.add_rule(rule(1, vec![Symbol::Terminal(SymbolId(2))], 0));
    }
    assert_eq!(g.all_rules().count(), 65_537);
    let before = g.clone();
    let err = g.try_normalize().expect_err("rule capacity must be typed");
    assert!(matches!(err, GrammarError::RuleSpaceExhausted), "{err:?}");
    assert_eq!(
        g, before,
        "failed try_normalize must leave grammar unchanged"
    );
    // The fallible refresh itself reports the same typed error, and
    // validation's conflict scan must not panic on the oversized grammar.
    let mut probe = before.clone();
    assert!(matches!(
        probe.refresh_production_ids(),
        Err(GrammarError::RuleSpaceExhausted)
    ));
    let _ = g.validate();
}

#[test]
fn probe_all_none_alias_after_flatten_validates() {
    let mut g = Grammar::new("all_none_alias".into());
    for id in [2u16, 3, 4] {
        g.tokens.insert(SymbolId(id), tok(id));
    }
    g.add_rule(rule(
        1,
        vec![
            Symbol::Sequence(vec![
                Symbol::Terminal(SymbolId(2)),
                Symbol::Terminal(SymbolId(3)),
            ]),
            Symbol::Terminal(SymbolId(4)),
        ],
        7,
    ));
    g.alias_sequences.insert(
        ProductionId(7),
        AliasSequence {
            aliases: vec![None, None],
        },
    );
    g.max_alias_sequence_length = 2;
    g.try_normalize().expect("normalize");
    let s = g.all_rules().find(|r| r.lhs == SymbolId(1)).unwrap();
    assert_eq!(s.rhs.len(), 3);
    assert_eq!(
        g.alias_sequences[&ProductionId(7)].aliases,
        vec![None, None, None],
        "all-None aliases must be padded to the flattened RHS length"
    );
    assert_eq!(g.max_alias_sequence_length, 3);
    g.validate().expect("padded all-None alias validates");
}

#[test]
fn nested_sequence_field_shifts_cumulatively() {
    let mut g = Grammar::new("nested".into());
    for id in [2u16, 3, 4, 5] {
        g.tokens.insert(SymbolId(id), tok(id));
    }
    g.fields.insert(FieldId(0), "d".into());
    let mut r = rule(
        1,
        vec![
            Symbol::Sequence(vec![
                Symbol::Sequence(vec![
                    Symbol::Terminal(SymbolId(2)),
                    Symbol::Terminal(SymbolId(3)),
                ]),
                Symbol::Terminal(SymbolId(4)),
            ]),
            Symbol::Terminal(SymbolId(5)),
        ],
        1,
    );
    r.fields = vec![(FieldId(0), 1)];
    g.add_rule(r);
    g.try_normalize().expect("normalize");
    let s = g.all_rules().find(|r| r.lhs == SymbolId(1)).unwrap();
    assert_eq!(s.rhs.len(), 4);
    assert_eq!(s.fields, vec![(FieldId(0), 3)]);
    assert_eq!(s.rhs[3], Symbol::Terminal(SymbolId(5)));
}

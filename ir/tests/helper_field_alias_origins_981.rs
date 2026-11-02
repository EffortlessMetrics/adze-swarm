//! #981 slice: helper rewrites carry field and alias positions.
//!
//! `Optional`/`Repeat`/`RepeatOne`/`Choice` positions rewrite to a single
//! generated helper non-terminal. Field ids and alias annotations anchored
//! on (or after) such a wrapper must ride onto the rewritten RHS at the
//! helper's position — never dropped or misattached — and the stored alias
//! vector must stay exactly as long as the normalized RHS so `validate()`
//! and the ABI stride keep agreeing on positions.

use adze_ir::{
    AliasSequence, FieldId, Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern,
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

fn grammar_with_tokens(ids: &[u16]) -> Grammar {
    let mut g = Grammar::new("helper_origins_981".into());
    for id in ids {
        g.tokens.insert(SymbolId(*id), tok(*id));
    }
    g.fields.insert(FieldId(0), "f".into());
    g.fields.insert(FieldId(1), "g".into());
    g
}

fn helper_wrappers() -> Vec<(&'static str, Symbol)> {
    vec![
        (
            "optional",
            Symbol::Optional(Box::new(Symbol::Terminal(SymbolId(2)))),
        ),
        (
            "repeat",
            Symbol::Repeat(Box::new(Symbol::Terminal(SymbolId(2)))),
        ),
        (
            "repeat_one",
            Symbol::RepeatOne(Box::new(Symbol::Terminal(SymbolId(2)))),
        ),
        (
            "choice",
            Symbol::Choice(vec![
                Symbol::Terminal(SymbolId(2)),
                Symbol::Terminal(SymbolId(4)),
            ]),
        ),
    ]
}

#[test]
fn test_field_on_helper_wrapper_rides_generated_helper() {
    for (name, wrapper) in helper_wrappers() {
        let mut g = grammar_with_tokens(&[2, 3, 4]);
        let mut r = rule(1, vec![wrapper], 7);
        r.fields = vec![(FieldId(0), 0)];
        g.add_rule(r);
        g.try_normalize().expect("normalize {name}");
        let s = g.all_rules().find(|r| r.lhs == SymbolId(1)).unwrap();
        assert_eq!(s.rhs.len(), 1, "{name}: wrapper is one helper child");
        assert!(
            matches!(s.rhs[0], Symbol::NonTerminal(_)),
            "{name}: position 0 must be the helper, got {:?}",
            s.rhs[0]
        );
        assert_eq!(
            s.fields,
            vec![(FieldId(0), 0)],
            "{name}: field must stay on the helper"
        );
        g.validate().expect("{name}: normalized grammar validates");
        // Second normalization is metadata-idempotent.
        let before = g.clone();
        g.try_normalize().expect("{name}: second normalize");
        assert_eq!(g, before, "{name}: second normalization must be a no-op");
    }
}

#[test]
fn test_fields_before_and_after_helper_keep_original_children() {
    for (name, wrapper) in helper_wrappers() {
        let mut g = grammar_with_tokens(&[2, 3, 4]);
        let mut r = rule(
            1,
            vec![
                Symbol::Terminal(SymbolId(3)),
                wrapper,
                Symbol::Terminal(SymbolId(2)),
            ],
            7,
        );
        r.fields = vec![(FieldId(0), 0), (FieldId(1), 2)];
        g.add_rule(r);
        g.try_normalize().expect("normalize {name}");
        let s = g.all_rules().find(|r| r.lhs == SymbolId(1)).unwrap();
        assert_eq!(s.rhs.len(), 3, "{name}: rhs shape");
        assert_eq!(
            s.fields,
            vec![(FieldId(0), 0), (FieldId(1), 2)],
            "{name}: fields keep positions"
        );
        // Semantic origin, not just in-bounds indices.
        assert_eq!(
            s.rhs[0],
            Symbol::Terminal(SymbolId(3)),
            "{name}: field f child"
        );
        assert_eq!(
            s.rhs[2],
            Symbol::Terminal(SymbolId(2)),
            "{name}: field g child"
        );
        g.validate().expect("{name}: normalized grammar validates");
    }
}

#[test]
fn test_alias_on_helper_keeps_position_and_full_length() {
    for (name, wrapper) in helper_wrappers() {
        let mut g = grammar_with_tokens(&[2, 3, 4]);
        g.add_rule(rule(1, vec![wrapper, Symbol::Terminal(SymbolId(3))], 7));
        g.alias_sequences.insert(
            ProductionId(7),
            AliasSequence {
                // Trailing `None` is length information: the rewritten RHS
                // still has two children, so the sequence must too.
                aliases: vec![Some("w".into()), None],
            },
        );
        g.max_alias_sequence_length = 2;
        g.validate().expect("{name}: pre-normalize validates");
        g.try_normalize().expect("normalize {name}");
        let s = g.all_rules().find(|r| r.lhs == SymbolId(1)).unwrap();
        assert_eq!(s.rhs.len(), 2, "{name}: rhs shape");
        assert!(
            matches!(s.rhs[0], Symbol::NonTerminal(_)),
            "{name}: position 0 must be the helper, got {:?}",
            s.rhs[0]
        );
        assert_eq!(s.rhs[1], Symbol::Terminal(SymbolId(3)), "{name}: child c");
        assert_eq!(
            g.alias_sequences[&ProductionId(7)].aliases,
            vec![Some("w".to_string()), None],
            "{name}: alias keeps helper position and RHS length"
        );
        assert_eq!(g.max_alias_sequence_length, 2, "{name}: max length agrees");
        g.validate().expect("{name}: normalized grammar validates");
        let before = g.clone();
        g.try_normalize().expect("{name}: second normalize");
        assert_eq!(g, before, "{name}: second normalization must be a no-op");
    }
}

#[test]
fn test_alias_after_helper_resolves_to_intended_child() {
    let mut g = grammar_with_tokens(&[2, 3]);
    g.add_rule(rule(
        1,
        vec![
            Symbol::Repeat(Box::new(Symbol::Terminal(SymbolId(2)))),
            Symbol::Terminal(SymbolId(3)),
        ],
        7,
    ));
    g.alias_sequences.insert(
        ProductionId(7),
        AliasSequence {
            aliases: vec![Some("rep".into()), Some("c".into())],
        },
    );
    g.max_alias_sequence_length = 2;
    g.try_normalize().expect("normalize");
    let s = g.all_rules().find(|r| r.lhs == SymbolId(1)).unwrap();
    assert_eq!(s.rhs.len(), 2);
    assert_eq!(
        g.alias_sequences[&ProductionId(7)].aliases,
        vec![Some("rep".to_string()), Some("c".to_string())]
    );
    // The second alias still designates `c`, not the helper.
    assert_eq!(s.rhs[1], Symbol::Terminal(SymbolId(3)));
    g.validate().expect("normalized grammar validates");
}

#[test]
fn test_field_after_nested_helper_expansion_keeps_child() {
    // The wrapper itself needs a second expansion pass (helper rule holds a
    // `Sequence`); a field after the wrapper must still designate `c`.
    let mut g = grammar_with_tokens(&[2, 3, 4]);
    let mut r = rule(
        1,
        vec![
            Symbol::Optional(Box::new(Symbol::Sequence(vec![
                Symbol::Terminal(SymbolId(2)),
                Symbol::Terminal(SymbolId(4)),
            ]))),
            Symbol::Terminal(SymbolId(3)),
        ],
        7,
    );
    r.fields = vec![(FieldId(0), 1)];
    g.add_rule(r);
    g.try_normalize().expect("normalize");
    let s = g.all_rules().find(|r| r.lhs == SymbolId(1)).unwrap();
    assert_eq!(s.fields, vec![(FieldId(0), 1)]);
    assert_eq!(s.rhs[1], Symbol::Terminal(SymbolId(3)));
    g.validate().expect("normalized grammar validates");
}

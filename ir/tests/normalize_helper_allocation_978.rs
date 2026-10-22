//! #978: normalization helper symbols must be allocated from the full
//! occupied `SymbolId` domain, never colliding with tokens, externals,
//! metadata references, or the reserved EOF id `SymbolId(0)`.

use adze_ir::{ExternalToken, Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

fn token(name: &str) -> Token {
    Token {
        name: name.to_string(),
        pattern: TokenPattern::String(name.to_string()),
        fragile: false,
    }
}

fn rule(lhs: u16, rhs: Vec<Symbol>) -> Rule {
    Rule {
        lhs: SymbolId(lhs),
        rhs,
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(7),
    }
}

fn assert_no_category_collision(grammar: &Grammar) {
    for id in grammar.rules.keys() {
        assert!(
            !grammar.tokens.contains_key(id),
            "helper {id} collides with a token"
        );
        assert!(
            !grammar.externals.iter().any(|ext| ext.symbol_id == *id),
            "helper {id} collides with an external token"
        );
    }
    assert!(
        !grammar.rules.contains_key(&SymbolId(0)),
        "helper allocated the reserved EOF id"
    );
}

#[test]
fn normalize_helper_never_collides_with_token_at_old_candidate() {
    // #978 witness: old allocator chose max_rule_id + 1000 = 1001, which is
    // this grammar's token id.
    let mut grammar = Grammar::new("helper_collision".to_string());
    grammar.start_symbol = Some(SymbolId(1));
    grammar.tokens.insert(SymbolId(1001), token("a"));
    grammar.add_rule(rule(
        1,
        vec![Symbol::Optional(Box::new(Symbol::Terminal(SymbolId(1001))))],
    ));

    grammar.normalize();

    assert!(grammar.tokens.contains_key(&SymbolId(1001)));
    assert_no_category_collision(&grammar);
    grammar
        .validate()
        .expect("normalized grammar must validate");
}

#[test]
fn normalize_helper_never_allocates_reserved_eof_id() {
    // #978 witness: old allocator computed 64536 + 1000, wrapping to EOF 0.
    let mut grammar = Grammar::new("eof_wrap".to_string());
    grammar.start_symbol = Some(SymbolId(64536));
    grammar.tokens.insert(SymbolId(1), token("a"));
    grammar.add_rule(rule(
        64536,
        vec![Symbol::Optional(Box::new(Symbol::Terminal(SymbolId(1))))],
    ));

    grammar.normalize();

    assert_no_category_collision(&grammar);
    grammar
        .validate()
        .expect("normalized grammar must validate");
}

#[test]
fn normalize_helper_avoids_external_and_metadata_only_ids() {
    // Occupy the low ids only through externals and metadata references:
    // none of them is a rule LHS or token, so a rules-only inventory would
    // hand them out as helpers.
    let mut grammar = Grammar::new("metadata_domain".to_string());
    grammar.start_symbol = Some(SymbolId(50));
    grammar.tokens.insert(SymbolId(60), token("a"));
    grammar.externals.push(ExternalToken {
        name: "ext".to_string(),
        symbol_id: SymbolId(1),
    });
    grammar.supertypes.push(SymbolId(2));
    grammar.extras.push(SymbolId(3));
    grammar
        .rule_names
        .insert(SymbolId(4), "metadata_only".to_string());
    // The old allocator's candidate for rule LHS 50 is 1050: occupy the
    // candidate band through metadata alone so a rules-only inventory
    // collides even though no rule or token uses those ids.
    grammar
        .rule_names
        .insert(SymbolId(1050), "candidate_band".to_string());
    grammar.supertypes.push(SymbolId(1051));
    grammar.externals.push(ExternalToken {
        name: "ext_band".to_string(),
        symbol_id: SymbolId(1052),
    });
    grammar.add_rule(rule(
        50,
        vec![
            Symbol::Optional(Box::new(Symbol::Terminal(SymbolId(60)))),
            Symbol::Repeat(Box::new(Symbol::External(SymbolId(1)))),
        ],
    ));

    grammar.normalize();

    for helper in [1u16, 2, 3, 4, 1050, 1051, 1052] {
        assert!(
            !grammar.rules.contains_key(&SymbolId(helper)),
            "helper allocated metadata/external id {helper}"
        );
    }
    assert_no_category_collision(&grammar);
}

#[test]
fn normalize_helpers_fill_gaps_deterministically_and_idempotently() {
    let build = || {
        let mut grammar = Grammar::new("gaps".to_string());
        grammar.start_symbol = Some(SymbolId(10));
        grammar.tokens.insert(SymbolId(11), token("a"));
        grammar.tokens.insert(SymbolId(12), token("b"));
        grammar.add_rule(rule(
            10,
            vec![
                Symbol::Optional(Box::new(Symbol::Terminal(SymbolId(11)))),
                Symbol::RepeatOne(Box::new(Symbol::Choice(vec![
                    Symbol::Terminal(SymbolId(11)),
                    Symbol::Terminal(SymbolId(12)),
                ]))),
                Symbol::Repeat(Box::new(Symbol::Terminal(SymbolId(12)))),
            ],
        ));
        grammar
    };

    let mut first = build();
    first.normalize();
    let first_keys: Vec<SymbolId> = first.rules.keys().copied().collect();
    assert_no_category_collision(&first);

    // Deterministic: an identical grammar normalizes to identical helpers.
    let mut second = build();
    second.normalize();
    assert_eq!(first_keys, second.rules.keys().copied().collect::<Vec<_>>());

    // Idempotent: a second pass allocates nothing new.
    first.normalize();
    assert_eq!(first_keys, first.rules.keys().copied().collect::<Vec<_>>());
    first.validate().expect("normalized grammar must validate");
}

#[test]
fn try_normalize_reports_typed_exhaustion_without_mutating() {
    // Occupy every non-EOF id as a rule LHS; one Optional then needs a
    // helper that cannot exist. The failure must be the typed error, not
    // a panic, wrap, or fabricated id, and the grammar stays untouched.
    let mut grammar = Grammar::new("exhausted".to_string());
    for id in 1..=u16::MAX {
        grammar.add_rule(rule(id, vec![Symbol::Epsilon]));
    }
    grammar.start_symbol = Some(SymbolId(1));
    // Replace one rule with a complex one requiring a helper.
    grammar.rules.insert(
        SymbolId(1),
        vec![rule(
            1,
            vec![Symbol::Optional(Box::new(Symbol::NonTerminal(SymbolId(2))))],
        )],
    );
    let rules_before = grammar.rules.len();

    let error = grammar
        .try_normalize()
        .expect_err("exhausted ID space must fail");
    assert_eq!(
        error.to_string(),
        "symbol ID space exhausted during normalization: no unoccupied SymbolId remains"
    );
    assert_eq!(grammar.rules.len(), rules_before);
}

#[test]
fn validate_detects_cross_category_symbol_collision() {
    let mut grammar = Grammar::new("pre_collided".to_string());
    grammar.tokens.insert(SymbolId(5), token("a"));
    grammar.add_rule(rule(5, vec![Symbol::Epsilon]));

    assert!(
        grammar.validate().is_err(),
        "one numeric id assigned to both a token and a rule must be rejected"
    );
}

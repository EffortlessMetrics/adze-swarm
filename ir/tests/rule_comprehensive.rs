#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

use adze_ir::builder::GrammarBuilder;
use adze_ir::{
    Associativity, FieldId, Grammar, PrecedenceKind, ProductionId, Rule, Symbol, SymbolId, Token,
    TokenPattern,
};

// ── helpers ──────────────────────────────────────────────────────────────────

fn simple_rule(lhs: u16, rhs: &[Symbol]) -> Rule {
    Rule {
        lhs: SymbolId(lhs),
        rhs: rhs.to_vec(),
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    }
}

fn term(id: u16) -> Symbol {
    Symbol::Terminal(SymbolId(id))
}

fn nonterm(id: u16) -> Symbol {
    Symbol::NonTerminal(SymbolId(id))
}

/// Build a minimal grammar where every SymbolId used in rules is registered as
/// either a token or a rule_name so that `validate()` can pass.
fn grammar_with_token_rule(
    token_ids: &[(u16, &str)],
    rule_lhs: u16,
    rule_name: &str,
    rhs: Vec<Symbol>,
) -> Grammar {
    let mut g = Grammar::new("test".into());
    for &(id, name) in token_ids {
        g.tokens.insert(
            SymbolId(id),
            Token {
                name: name.into(),
                pattern: TokenPattern::String(name.into()),
                fragile: false,
            },
        );
    }
    g.rule_names.insert(SymbolId(rule_lhs), rule_name.into());
    g.add_rule(Rule {
        lhs: SymbolId(rule_lhs),
        rhs,
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    g
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Rule construction with every Symbol variant
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn construct_terminal() {
    let r = simple_rule(1, &[term(2)]);
    assert!(matches!(r.rhs[0], Symbol::Terminal(SymbolId(2))));
}

#[test]
fn construct_nonterminal() {
    let r = simple_rule(1, &[nonterm(3)]);
    assert!(matches!(r.rhs[0], Symbol::NonTerminal(SymbolId(3))));
}

#[test]
fn construct_optional() {
    let r = simple_rule(1, &[Symbol::Optional(Box::new(term(4)))]);
    assert!(matches!(r.rhs[0], Symbol::Optional(_)));
}

#[test]
fn construct_repeat() {
    let r = simple_rule(1, &[Symbol::Repeat(Box::new(term(5)))]);
    assert!(matches!(r.rhs[0], Symbol::Repeat(_)));
}

#[test]
fn construct_repeat_one() {
    let r = simple_rule(1, &[Symbol::RepeatOne(Box::new(nonterm(6)))]);
    assert!(matches!(r.rhs[0], Symbol::RepeatOne(_)));
}

#[test]
fn construct_choice() {
    let r = simple_rule(1, &[Symbol::Choice(vec![term(7), nonterm(8)])]);
    if let Symbol::Choice(ref c) = r.rhs[0] {
        assert_eq!(c.len(), 2);
    } else {
        panic!("expected Choice");
    }
}

#[test]
fn construct_sequence() {
    let r = simple_rule(1, &[Symbol::Sequence(vec![term(9), term(10), nonterm(11)])]);
    if let Symbol::Sequence(ref s) = r.rhs[0] {
        assert_eq!(s.len(), 3);
    } else {
        panic!("expected Sequence");
    }
}

#[test]
fn construct_epsilon() {
    let r = simple_rule(1, &[Symbol::Epsilon]);
    assert_eq!(r.rhs[0], Symbol::Epsilon);
}

#[test]
fn construct_external() {
    let r = simple_rule(1, &[Symbol::External(SymbolId(12))]);
    assert!(matches!(r.rhs[0], Symbol::External(SymbolId(12))));
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Nested symbols (Optional inside Choice inside Sequence etc.)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn optional_inside_choice() {
    let sym = Symbol::Choice(vec![Symbol::Optional(Box::new(term(1))), term(2)]);
    let r = simple_rule(0, &[sym]);
    if let Symbol::Choice(ref choices) = r.rhs[0] {
        assert!(matches!(choices[0], Symbol::Optional(_)));
    } else {
        panic!("expected Choice");
    }
}

#[test]
fn repeat_inside_sequence() {
    let sym = Symbol::Sequence(vec![term(1), Symbol::Repeat(Box::new(nonterm(2))), term(3)]);
    let r = simple_rule(0, &[sym]);
    if let Symbol::Sequence(ref seq) = r.rhs[0] {
        assert!(matches!(seq[1], Symbol::Repeat(_)));
    } else {
        panic!("expected Sequence");
    }
}

#[test]
fn choice_inside_optional_inside_repeat() {
    let inner_choice = Symbol::Choice(vec![term(1), term(2)]);
    let opt = Symbol::Optional(Box::new(inner_choice));
    let rep = Symbol::Repeat(Box::new(opt));
    let r = simple_rule(0, &[rep]);

    // Repeat -> Optional -> Choice
    if let Symbol::Repeat(ref a) = r.rhs[0] {
        if let Symbol::Optional(ref b) = **a {
            assert!(matches!(**b, Symbol::Choice(_)));
        } else {
            panic!("expected Optional inside Repeat");
        }
    } else {
        panic!("expected Repeat");
    }
}

#[test]
fn sequence_inside_choice_alternatives() {
    let sym = Symbol::Choice(vec![
        Symbol::Sequence(vec![term(1), term(2)]),
        Symbol::Sequence(vec![term(3), term(4)]),
    ]);
    let r = simple_rule(0, &[sym]);
    if let Symbol::Choice(ref choices) = r.rhs[0] {
        assert_eq!(choices.len(), 2);
        assert!(matches!(choices[0], Symbol::Sequence(_)));
        assert!(matches!(choices[1], Symbol::Sequence(_)));
    } else {
        panic!("expected Choice");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Deep symbol nesting (10+ levels)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn deep_nesting_12_levels() {
    // Build: Optional(Repeat(Optional(Repeat(... term(42) ...))))
    let mut sym = term(42);
    for i in 0..12 {
        sym = if i % 2 == 0 {
            Symbol::Optional(Box::new(sym))
        } else {
            Symbol::Repeat(Box::new(sym))
        };
    }
    let r = simple_rule(0, &[sym.clone()]);
    assert_eq!(r.rhs[0], sym);

    // Serde roundtrip still works at depth 12
    let json = serde_json::to_string(&r).unwrap();
    let back: Rule = serde_json::from_str(&json).unwrap();
    assert_eq!(r, back);
}

#[test]
fn deep_choice_nesting() {
    // Choice([Choice([Choice([... term(1) ...])])])
    let mut sym = term(1);
    for _ in 0..10 {
        sym = Symbol::Choice(vec![sym, term(99)]);
    }
    let r = simple_rule(0, &[sym]);
    // Just verify it constructs and serializes without stack overflow
    let json = serde_json::to_string(&r).unwrap();
    let back: Rule = serde_json::from_str(&json).unwrap();
    assert_eq!(r, back);
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Grammar construction with multiple rules
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn grammar_multiple_rules_via_add_rule() {
    let mut g = Grammar::new("multi".into());
    g.add_rule(simple_rule(1, &[term(2)]));
    g.add_rule(simple_rule(1, &[term(3)]));
    g.add_rule(simple_rule(4, &[nonterm(1)]));
    assert_eq!(g.get_rules_for_symbol(SymbolId(1)).unwrap().len(), 2);
    assert_eq!(g.get_rules_for_symbol(SymbolId(4)).unwrap().len(), 1);
    assert_eq!(g.all_rules().count(), 3);
}

#[test]
fn grammar_builder_constructs_valid_grammar() {
    let g = GrammarBuilder::new("arith")
        .token("NUMBER", r"\d+")
        .token("+", "+")
        .rule("expr", vec!["expr", "+", "expr"])
        .rule("expr", vec!["NUMBER"])
        .start("expr")
        .build();

    assert_eq!(g.name, "arith");
    assert!(g.tokens.len() >= 2);
    // expr should have two alternative productions
    let expr_id = g.find_symbol_by_name("expr").unwrap();
    assert_eq!(g.get_rules_for_symbol(expr_id).unwrap().len(), 2);
}

#[test]
fn grammar_builder_with_precedence() {
    let g = GrammarBuilder::new("prec")
        .token("N", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .rule("expr", vec!["N"])
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .start("expr")
        .build();

    let expr_id = g.find_symbol_by_name("expr").unwrap();
    let rules = g.get_rules_for_symbol(expr_id).unwrap();
    assert_eq!(rules.len(), 3);
    // The '+' rule should have prec=1, '*' rule prec=2
    let prec_rules: Vec<_> = rules.iter().filter(|r| r.precedence.is_some()).collect();
    assert_eq!(prec_rules.len(), 2);
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Grammar validation (valid grammars pass, invalid fail)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn valid_grammar_passes_validation() {
    let g = grammar_with_token_rule(&[(2, "x")], 1, "start", vec![term(2)]);
    assert!(g.validate().is_ok());
}

#[test]
fn unresolved_terminal_fails_validation() {
    // Rule references SymbolId(99) as terminal but no token or rule registered for it
    let g = grammar_with_token_rule(&[(2, "x")], 1, "start", vec![term(99)]);
    assert!(g.validate().is_err());
}

#[test]
fn unresolved_nonterminal_fails_validation() {
    let g = grammar_with_token_rule(&[(2, "x")], 1, "start", vec![nonterm(99)]);
    assert!(g.validate().is_err());
}

#[test]
fn epsilon_only_rule_passes_validation() {
    let g = grammar_with_token_rule(&[], 1, "start", vec![Symbol::Epsilon]);
    assert!(g.validate().is_ok());
}

#[test]
fn invalid_field_ordering_fails_validation() {
    let mut g = grammar_with_token_rule(&[(2, "x")], 1, "start", vec![term(2)]);
    // Insert fields in reverse lexicographic order
    g.fields.insert(FieldId(0), "zebra".into());
    g.fields.insert(FieldId(1), "alpha".into());
    assert!(g.validate().is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Grammar normalization effects
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn normalize_expands_optional() {
    let mut g = Grammar::new("norm".into());
    g.tokens.insert(
        SymbolId(2),
        Token {
            name: "t".into(),
            pattern: TokenPattern::String("t".into()),
            fragile: false,
        },
    );
    g.rule_names.insert(SymbolId(1), "start".into());
    g.add_rule(Rule {
        lhs: SymbolId(1),
        rhs: vec![Symbol::Optional(Box::new(term(2)))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let before_count = g.all_rules().count();
    g.normalize();
    // Normalization should have introduced auxiliary rules
    assert!(g.all_rules().count() > before_count);
    // Original rule's rhs should now be a NonTerminal (the aux symbol)
    let start_rules = g.get_rules_for_symbol(SymbolId(1)).unwrap();
    assert!(
        start_rules
            .iter()
            .all(|r| r.rhs.iter().all(|s| !matches!(s, Symbol::Optional(_))))
    );
}

#[test]
fn normalize_expands_repeat() {
    let mut g = Grammar::new("norm_rep".into());
    g.tokens.insert(
        SymbolId(2),
        Token {
            name: "t".into(),
            pattern: TokenPattern::String("t".into()),
            fragile: false,
        },
    );
    g.rule_names.insert(SymbolId(1), "start".into());
    g.add_rule(simple_rule(1, &[Symbol::Repeat(Box::new(term(2)))]));

    g.normalize();

    // No Repeat symbols should remain after normalization
    for rule in g.all_rules() {
        for sym in &rule.rhs {
            assert!(!matches!(sym, Symbol::Repeat(_)));
        }
    }
}

#[test]
fn normalize_flattens_sequence() {
    let mut g = Grammar::new("norm_seq".into());
    g.tokens.insert(
        SymbolId(2),
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    g.tokens.insert(
        SymbolId(3),
        Token {
            name: "b".into(),
            pattern: TokenPattern::String("b".into()),
            fragile: false,
        },
    );
    g.rule_names.insert(SymbolId(1), "start".into());
    g.add_rule(simple_rule(1, &[Symbol::Sequence(vec![term(2), term(3)])]));

    g.normalize();

    // The Sequence should have been flattened into the rhs directly
    let start_rules = g.get_rules_for_symbol(SymbolId(1)).unwrap();
    assert_eq!(start_rules.len(), 1);
    assert_eq!(start_rules[0].rhs, vec![term(2), term(3)]);
}

#[test]
fn normalize_expands_choice() {
    let mut g = Grammar::new("norm_choice".into());
    g.tokens.insert(
        SymbolId(2),
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    g.tokens.insert(
        SymbolId(3),
        Token {
            name: "b".into(),
            pattern: TokenPattern::String("b".into()),
            fragile: false,
        },
    );
    g.rule_names.insert(SymbolId(1), "start".into());
    g.add_rule(simple_rule(1, &[Symbol::Choice(vec![term(2), term(3)])]));

    g.normalize();

    // No Choice symbols should remain after normalization
    for rule in g.all_rules() {
        for sym in &rule.rhs {
            assert!(!matches!(sym, Symbol::Choice(_)));
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. Rule name handling (empty, unicode, long)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn grammar_empty_rule_name() {
    let mut g = Grammar::new("names".into());
    g.rule_names.insert(SymbolId(1), String::new());
    g.add_rule(simple_rule(1, &[Symbol::Epsilon]));
    // Empty name is valid structurally
    assert_eq!(g.rule_names[&SymbolId(1)], "");
}

#[test]
fn grammar_unicode_rule_name() {
    let mut g = Grammar::new("unicode".into());
    let name = "表达式_αβγ_🦀";
    g.rule_names.insert(SymbolId(1), name.into());
    g.add_rule(simple_rule(1, &[Symbol::Epsilon]));
    assert_eq!(g.rule_names[&SymbolId(1)], name);
    assert_eq!(g.find_symbol_by_name(name), Some(SymbolId(1)));
}

#[test]
fn grammar_long_rule_name() {
    let long_name: String = "a".repeat(10_000);
    let mut g = Grammar::new("long".into());
    g.rule_names.insert(SymbolId(1), long_name.clone());
    g.add_rule(simple_rule(1, &[Symbol::Epsilon]));
    assert_eq!(g.rule_names[&SymbolId(1)].len(), 10_000);
    assert_eq!(g.find_symbol_by_name(&long_name), Some(SymbolId(1)));
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. SymbolId uniqueness within a grammar
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn symbol_id_copy_semantics() {
    let id = SymbolId(7);
    let id2 = id; // Copy
    assert_eq!(id, id2);
}

#[test]
fn symbol_id_ordering() {
    let ids: Vec<SymbolId> = vec![SymbolId(5), SymbolId(1), SymbolId(3)];
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(sorted, vec![SymbolId(1), SymbolId(3), SymbolId(5)]);
}

#[test]
fn symbol_id_hash_set_dedup() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(SymbolId(1));
    set.insert(SymbolId(2));
    set.insert(SymbolId(1)); // duplicate
    assert_eq!(set.len(), 2);
}

#[test]
fn grammar_builder_assigns_unique_ids() {
    let g = GrammarBuilder::new("unique")
        .token("A", "a")
        .token("B", "b")
        .token("C", "c")
        .rule("x", vec!["A"])
        .rule("y", vec!["B"])
        .rule("z", vec!["C"])
        .start("x")
        .build();

    // All rule_names keys should be distinct
    let ids: Vec<SymbolId> = g.rule_names.keys().copied().collect();
    let unique: std::collections::HashSet<SymbolId> = ids.iter().copied().collect();
    assert_eq!(ids.len(), unique.len());
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. Precedence and associativity combinations
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn all_associativity_variants() {
    for assoc in [
        Associativity::Left,
        Associativity::Right,
        Associativity::None,
    ] {
        let r = Rule {
            lhs: SymbolId(0),
            rhs: vec![term(1)],
            precedence: Some(PrecedenceKind::Static(1)),
            associativity: Some(assoc),
            fields: vec![],
            production_id: ProductionId(0),
        };
        assert_eq!(r.associativity, Some(assoc));
    }
}

#[test]
fn static_and_dynamic_precedence_kinds() {
    let s = PrecedenceKind::Static(5);
    let d = PrecedenceKind::Dynamic(5);
    assert_ne!(s, d);
    assert_eq!(s, PrecedenceKind::Static(5));
    assert_eq!(d, PrecedenceKind::Dynamic(5));
}

#[test]
fn negative_dynamic_precedence() {
    let r = Rule {
        lhs: SymbolId(0),
        rhs: vec![term(1)],
        precedence: Some(PrecedenceKind::Dynamic(i16::MIN)),
        associativity: Some(Associativity::Right),
        fields: vec![],
        production_id: ProductionId(0),
    };
    assert_eq!(r.precedence, Some(PrecedenceKind::Dynamic(i16::MIN)));
    assert_eq!(r.associativity, Some(Associativity::Right));
}

#[test]
fn precedence_without_associativity() {
    let r = Rule {
        lhs: SymbolId(0),
        rhs: vec![term(1)],
        precedence: Some(PrecedenceKind::Static(3)),
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    };
    assert!(r.precedence.is_some());
    assert!(r.associativity.is_none());
}

#[test]
fn associativity_without_precedence() {
    let r = Rule {
        lhs: SymbolId(0),
        rhs: vec![term(1)],
        precedence: None,
        associativity: Some(Associativity::Left),
        fields: vec![],
        production_id: ProductionId(0),
    };
    assert!(r.precedence.is_none());
    assert!(r.associativity.is_some());
}

// ═══════════════════════════════════════════════════════════════════════════
// 10. Edge cases: empty rules, single-symbol rules, recursive references
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn empty_rhs_rule() {
    let r = simple_rule(1, &[]);
    assert!(r.rhs.is_empty());
}

#[test]
fn single_terminal_rule() {
    let r = simple_rule(1, &[term(2)]);
    assert_eq!(r.rhs.len(), 1);
    assert!(matches!(r.rhs[0], Symbol::Terminal(_)));
}

#[test]
fn single_nonterminal_rule() {
    let r = simple_rule(1, &[nonterm(2)]);
    assert_eq!(r.rhs.len(), 1);
    assert!(matches!(r.rhs[0], Symbol::NonTerminal(_)));
}

#[test]
fn direct_left_recursive_reference() {
    // S -> S a (direct left recursion)
    let r = simple_rule(1, &[nonterm(1), term(2)]);
    assert_eq!(r.lhs, SymbolId(1));
    assert_eq!(r.rhs[0], nonterm(1));
}

#[test]
fn direct_right_recursive_reference() {
    // S -> a S (direct right recursion)
    let r = simple_rule(1, &[term(2), nonterm(1)]);
    assert_eq!(r.rhs[1], nonterm(1));
}

#[test]
fn mutual_recursion_grammar() {
    let mut g = Grammar::new("mutual".into());
    g.tokens.insert(
        SymbolId(3),
        Token {
            name: "x".into(),
            pattern: TokenPattern::String("x".into()),
            fragile: false,
        },
    );
    g.rule_names.insert(SymbolId(1), "A".into());
    g.rule_names.insert(SymbolId(2), "B".into());
    // A -> B x
    g.add_rule(simple_rule(1, &[nonterm(2), term(3)]));
    // B -> A x | x
    g.add_rule(simple_rule(2, &[nonterm(1), term(3)]));
    g.add_rule(simple_rule(2, &[term(3)]));

    assert!(g.validate().is_ok());
    assert_eq!(g.all_rules().count(), 3);
}

#[test]
fn serde_roundtrip_with_all_symbol_variants() {
    let rule = Rule {
        lhs: SymbolId(0),
        rhs: vec![
            term(1),
            nonterm(2),
            Symbol::External(SymbolId(3)),
            Symbol::Optional(Box::new(term(4))),
            Symbol::Repeat(Box::new(nonterm(5))),
            Symbol::RepeatOne(Box::new(term(6))),
            Symbol::Choice(vec![term(7), term(8)]),
            Symbol::Sequence(vec![nonterm(9), term(10)]),
            Symbol::Epsilon,
        ],
        precedence: Some(PrecedenceKind::Dynamic(-2)),
        associativity: Some(Associativity::None),
        fields: vec![(FieldId(0), 0), (FieldId(1), 3)],
        production_id: ProductionId(77),
    };
    let json = serde_json::to_string(&rule).unwrap();
    let back: Rule = serde_json::from_str(&json).unwrap();
    assert_eq!(rule, back);
}

#[test]
fn clone_independence_of_nested_symbol() {
    let sym = Symbol::Optional(Box::new(Symbol::Choice(vec![term(1), term(2)])));
    let r = simple_rule(0, &[sym]);
    let mut cloned = r.clone();
    cloned.rhs.push(term(99));
    assert_ne!(r.rhs.len(), cloned.rhs.len());
    // Original unchanged
    assert_eq!(r.rhs.len(), 1);
}

#[test]
fn debug_format_readable() {
    let r = simple_rule(42, &[term(1), nonterm(2)]);
    let dbg = format!("{r:?}");
    assert!(dbg.contains("42"));
    assert!(dbg.contains("Terminal"));
    assert!(dbg.contains("NonTerminal"));
}

#[test]
fn grammar_start_symbol_requires_explicit_metadata() {
    let g = GrammarBuilder::new("explicit_start")
        .token("NUM", r"\d+")
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    assert_eq!(g.start_symbol(), g.explicit_start_symbol());
}

#[test]
fn max_symbol_id_serde_roundtrip() {
    let r = Rule {
        lhs: SymbolId(u16::MAX),
        rhs: vec![Symbol::Terminal(SymbolId(u16::MAX))],
        precedence: Some(PrecedenceKind::Static(i16::MAX)),
        associativity: Some(Associativity::Left),
        fields: vec![(FieldId(u16::MAX), usize::MAX)],
        production_id: ProductionId(u16::MAX),
    };
    let json = serde_json::to_string(&r).unwrap();
    let back: Rule = serde_json::from_str(&json).unwrap();
    assert_eq!(r, back);
}

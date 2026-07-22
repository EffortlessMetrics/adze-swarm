#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

//! Comprehensive tests for symbol resolution and ID management in adze-ir.

use adze_ir::{
    ExternalToken, FieldId, Grammar, ProductionId, Rule, Symbol, SymbolId, SymbolMetadata,
    SymbolRegistry, Token, TokenPattern,
};
use std::collections::{BTreeSet, HashMap, HashSet};

// ---------------------------------------------------------------------------
// SymbolId creation and comparison
// ---------------------------------------------------------------------------

#[test]
fn symbol_id_creation_from_u16() {
    let id = SymbolId(0);
    assert_eq!(id.0, 0);
    let id2 = SymbolId(65535);
    assert_eq!(id2.0, 65535);
}

#[test]
fn symbol_id_equality() {
    assert_eq!(SymbolId(1), SymbolId(1));
    assert_ne!(SymbolId(1), SymbolId(2));
}

#[test]
fn symbol_id_ordering() {
    assert!(SymbolId(0) < SymbolId(1));
    assert!(SymbolId(100) > SymbolId(99));

    let mut ids = vec![SymbolId(5), SymbolId(1), SymbolId(3)];
    ids.sort();
    assert_eq!(ids, vec![SymbolId(1), SymbolId(3), SymbolId(5)]);
}

#[test]
fn symbol_id_hash_in_set() {
    let mut set = HashSet::new();
    set.insert(SymbolId(10));
    set.insert(SymbolId(20));
    set.insert(SymbolId(10)); // duplicate
    assert_eq!(set.len(), 2);
    assert!(set.contains(&SymbolId(10)));
    assert!(!set.contains(&SymbolId(30)));
}

#[test]
fn symbol_id_as_btree_key() {
    let mut tree = BTreeSet::new();
    for i in (0..10).rev() {
        tree.insert(SymbolId(i));
    }
    let ordered: Vec<u16> = tree.iter().map(|s| s.0).collect();
    assert_eq!(ordered, (0..10).collect::<Vec<u16>>());
}

#[test]
fn symbol_id_display_format() {
    assert_eq!(format!("{}", SymbolId(0)), "Symbol(0)");
    assert_eq!(format!("{}", SymbolId(42)), "Symbol(42)");
}

#[test]
fn symbol_id_debug_format() {
    let debug = format!("{:?}", SymbolId(7));
    assert!(debug.contains("SymbolId"));
    assert!(debug.contains("7"));
}

#[test]
fn symbol_id_copy_semantics() {
    let a = SymbolId(5);
    let b = a; // Copy
    assert_eq!(a, b);
    // `a` is still usable after copy
    assert_eq!(a.0, 5);
}

// ---------------------------------------------------------------------------
// Symbol enum variant construction
// ---------------------------------------------------------------------------

#[test]
fn symbol_terminal_construction() {
    let s = Symbol::Terminal(SymbolId(1));
    assert!(matches!(s, Symbol::Terminal(SymbolId(1))));
}

#[test]
fn symbol_nonterminal_construction() {
    let s = Symbol::NonTerminal(SymbolId(2));
    assert!(matches!(s, Symbol::NonTerminal(SymbolId(2))));
}

#[test]
fn symbol_external_construction() {
    let s = Symbol::External(SymbolId(3));
    assert!(matches!(s, Symbol::External(SymbolId(3))));
}

#[test]
fn symbol_epsilon_construction() {
    let s = Symbol::Epsilon;
    assert!(matches!(s, Symbol::Epsilon));
}

// ---------------------------------------------------------------------------
// Terminal vs NonTerminal distinction
// ---------------------------------------------------------------------------

#[test]
fn terminal_and_nonterminal_same_id_are_distinct() {
    let t = Symbol::Terminal(SymbolId(1));
    let nt = Symbol::NonTerminal(SymbolId(1));
    assert_ne!(t, nt);
}

#[test]
fn terminal_not_equal_to_external() {
    let t = Symbol::Terminal(SymbolId(5));
    let e = Symbol::External(SymbolId(5));
    assert_ne!(t, e);
}

// ---------------------------------------------------------------------------
// Symbol matching and lookup
// ---------------------------------------------------------------------------

#[test]
fn match_terminal_extracts_id() {
    let sym = Symbol::Terminal(SymbolId(42));
    match &sym {
        Symbol::Terminal(id) => assert_eq!(id.0, 42),
        _ => panic!("expected Terminal"),
    }
}

#[test]
fn symbol_in_hashmap_lookup() {
    let mut map: HashMap<Symbol, &str> = HashMap::new();
    map.insert(Symbol::Terminal(SymbolId(1)), "number");
    map.insert(Symbol::NonTerminal(SymbolId(2)), "expr");
    map.insert(Symbol::Epsilon, "empty");

    assert_eq!(map.get(&Symbol::Terminal(SymbolId(1))), Some(&"number"));
    assert_eq!(map.get(&Symbol::NonTerminal(SymbolId(2))), Some(&"expr"));
    assert_eq!(map.get(&Symbol::Epsilon), Some(&"empty"));
    assert_eq!(map.get(&Symbol::Terminal(SymbolId(99))), None);
}

// ---------------------------------------------------------------------------
// Symbol display/debug formatting
// ---------------------------------------------------------------------------

#[test]
fn symbol_debug_terminal() {
    let s = Symbol::Terminal(SymbolId(1));
    let dbg = format!("{:?}", s);
    assert!(dbg.contains("Terminal"));
    assert!(dbg.contains("1"));
}

#[test]
fn symbol_debug_nonterminal() {
    let s = Symbol::NonTerminal(SymbolId(2));
    let dbg = format!("{:?}", s);
    assert!(dbg.contains("NonTerminal"));
}

#[test]
fn symbol_debug_epsilon() {
    assert_eq!(format!("{:?}", Symbol::Epsilon), "Epsilon");
}

#[test]
fn symbol_debug_optional() {
    let s = Symbol::Optional(Box::new(Symbol::Terminal(SymbolId(1))));
    let dbg = format!("{:?}", s);
    assert!(dbg.contains("Optional"));
    assert!(dbg.contains("Terminal"));
}

// ---------------------------------------------------------------------------
// Symbol cloning and equality
// ---------------------------------------------------------------------------

#[test]
fn symbol_clone_equals_original() {
    let original = Symbol::Choice(vec![
        Symbol::Terminal(SymbolId(1)),
        Symbol::NonTerminal(SymbolId(2)),
    ]);
    let cloned = original.clone();
    assert_eq!(original, cloned);
}

#[test]
fn symbol_deep_clone_independence() {
    let original = Symbol::Optional(Box::new(Symbol::Repeat(Box::new(Symbol::Terminal(
        SymbolId(1),
    )))));
    let mut cloned = original.clone();
    // Reassign cloned; original is unaffected.
    let _ = &cloned;
    cloned = Symbol::Epsilon;
    assert_ne!(original, cloned);
}

// ---------------------------------------------------------------------------
// Complex symbol (Optional, Repeat, Choice, Sequence) handling
// ---------------------------------------------------------------------------

#[test]
fn optional_wraps_inner_symbol() {
    let inner = Symbol::Terminal(SymbolId(10));
    let opt = Symbol::Optional(Box::new(inner.clone()));
    if let Symbol::Optional(boxed) = &opt {
        assert_eq!(**boxed, inner);
    } else {
        panic!("expected Optional");
    }
}

#[test]
fn repeat_wraps_inner_symbol() {
    let inner = Symbol::NonTerminal(SymbolId(3));
    let rep = Symbol::Repeat(Box::new(inner.clone()));
    if let Symbol::Repeat(boxed) = &rep {
        assert_eq!(**boxed, inner);
    } else {
        panic!("expected Repeat");
    }
}

#[test]
fn repeat_one_wraps_inner_symbol() {
    let inner = Symbol::Terminal(SymbolId(4));
    let rep1 = Symbol::RepeatOne(Box::new(inner.clone()));
    if let Symbol::RepeatOne(boxed) = &rep1 {
        assert_eq!(**boxed, inner);
    } else {
        panic!("expected RepeatOne");
    }
}

#[test]
fn choice_preserves_alternatives() {
    let alts = vec![
        Symbol::Terminal(SymbolId(1)),
        Symbol::Terminal(SymbolId(2)),
        Symbol::NonTerminal(SymbolId(3)),
    ];
    let choice = Symbol::Choice(alts.clone());
    if let Symbol::Choice(v) = &choice {
        assert_eq!(v.len(), 3);
        for i in 0..3 {
            assert_eq!(v[i], alts[i]);
        }
    } else {
        panic!("expected Choice");
    }
}

#[test]
fn sequence_preserves_order() {
    let elems = vec![
        Symbol::Terminal(SymbolId(10)),
        Symbol::NonTerminal(SymbolId(20)),
        Symbol::Epsilon,
    ];
    let seq = Symbol::Sequence(elems.clone());
    if let Symbol::Sequence(v) = &seq {
        assert_eq!(v.len(), 3);
        for i in 0..3 {
            assert_eq!(v[i], elems[i]);
        }
    } else {
        panic!("expected Sequence");
    }
}

#[test]
fn nested_complex_symbols() {
    // Optional(Repeat(Choice([Terminal(1), NonTerminal(2)])))
    let sym = Symbol::Optional(Box::new(Symbol::Repeat(Box::new(Symbol::Choice(vec![
        Symbol::Terminal(SymbolId(1)),
        Symbol::NonTerminal(SymbolId(2)),
    ])))));

    if let Symbol::Optional(rep) = &sym {
        if let Symbol::Repeat(choice) = rep.as_ref() {
            if let Symbol::Choice(v) = choice.as_ref() {
                assert_eq!(v.len(), 2);
            } else {
                panic!("expected Choice inside Repeat");
            }
        } else {
            panic!("expected Repeat inside Optional");
        }
    } else {
        panic!("expected Optional");
    }
}

#[test]
fn complex_symbol_equality() {
    let a = Symbol::Choice(vec![Symbol::Terminal(SymbolId(1)), Symbol::Epsilon]);
    let b = Symbol::Choice(vec![Symbol::Terminal(SymbolId(1)), Symbol::Epsilon]);
    let c = Symbol::Choice(vec![Symbol::Epsilon, Symbol::Terminal(SymbolId(1))]);
    assert_eq!(a, b);
    assert_ne!(a, c); // order matters
}

// ---------------------------------------------------------------------------
// Symbol in Grammar context
// ---------------------------------------------------------------------------

#[test]
fn grammar_add_and_retrieve_rule() {
    let mut g = Grammar::new("test".to_string());
    let lhs = SymbolId(0);
    g.tokens.insert(
        SymbolId(1),
        Token {
            name: "NUM".into(),
            pattern: TokenPattern::String("0".into()),
            fragile: false,
        },
    );
    let rule = Rule {
        lhs,
        rhs: vec![Symbol::Terminal(SymbolId(1))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    };
    g.add_rule(rule.clone());

    let rules = g.get_rules_for_symbol(lhs).unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].rhs, rule.rhs);
}

#[test]
fn grammar_all_rules_iterator() {
    let mut g = Grammar::new("iter_test".to_string());
    for i in 0..3 {
        g.add_rule(Rule {
            lhs: SymbolId(i),
            rhs: vec![Symbol::Epsilon],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(i),
        });
    }
    assert_eq!(g.all_rules().count(), 3);
}

// ---------------------------------------------------------------------------
// rule_names lookup
// ---------------------------------------------------------------------------

#[test]
fn rule_names_find_by_name() {
    let mut g = Grammar::new("names".to_string());
    g.rule_names.insert(SymbolId(0), "expression".into());
    g.rule_names.insert(SymbolId(1), "statement".into());

    assert_eq!(g.find_symbol_by_name("expression"), Some(SymbolId(0)));
    assert_eq!(g.find_symbol_by_name("statement"), Some(SymbolId(1)));
    assert_eq!(g.find_symbol_by_name("missing"), None);
}

#[test]
fn rule_names_empty_grammar_returns_none() {
    let g = Grammar::new("empty".to_string());
    assert_eq!(g.find_symbol_by_name("anything"), None);
}

#[test]
fn source_file_without_explicit_start_is_not_inferred() {
    let mut g = Grammar::new("start".to_string());
    let sf_id = SymbolId(10);
    g.rule_names.insert(sf_id, "source_file".into());
    g.rules.entry(sf_id).or_default().push(Rule {
        lhs: sf_id,
        rhs: vec![Symbol::Epsilon],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    assert_eq!(g.start_symbol(), None);
}

// ---------------------------------------------------------------------------
// symbol_to_index mapping (via SymbolRegistry)
// ---------------------------------------------------------------------------

#[test]
fn registry_to_index_map_covers_all_symbols() {
    let mut reg = SymbolRegistry::new();
    let meta = SymbolMetadata {
        visible: true,
        named: false,
        hidden: false,
        terminal: true,
    };
    reg.register("plus", meta);
    reg.register("minus", meta);

    let idx_map = reg.to_index_map();
    // "end" (0), "plus" (1), "minus" (2) → 3 entries
    assert_eq!(idx_map.len(), 3);
    // Each SymbolId maps to a unique index
    let indices: HashSet<usize> = idx_map.values().copied().collect();
    assert_eq!(indices.len(), 3);
}

#[test]
fn registry_to_symbol_map_inverse() {
    let mut reg = SymbolRegistry::new();
    let meta = SymbolMetadata {
        visible: true,
        named: true,
        hidden: false,
        terminal: false,
    };
    reg.register("expr", meta);

    let idx_map = reg.to_index_map();
    let sym_map = reg.to_symbol_map();

    for (&sym_id, &idx) in &idx_map {
        assert_eq!(sym_map.get(&idx), Some(&sym_id));
    }
}

#[test]
fn registry_get_id_and_name_roundtrip() {
    let mut reg = SymbolRegistry::new();
    let meta = SymbolMetadata {
        visible: true,
        named: false,
        hidden: false,
        terminal: true,
    };
    let id = reg.register("token_x", meta);

    assert_eq!(reg.get_id("token_x"), Some(id));
    assert_eq!(reg.get_name(id), Some("token_x"));
}

#[test]
fn registry_contains_id() {
    let mut reg = SymbolRegistry::new();
    let meta = SymbolMetadata {
        visible: true,
        named: false,
        hidden: false,
        terminal: true,
    };
    let id = reg.register("tok", meta);
    assert!(reg.contains_id(id));
    assert!(!reg.contains_id(SymbolId(9999)));
}

#[test]
fn registry_len_and_is_empty() {
    let reg = SymbolRegistry::new();
    // new() registers "end" automatically
    assert!(!reg.is_empty());
    assert_eq!(reg.len(), 1);
}

#[test]
fn registry_duplicate_register_updates_metadata() {
    let mut reg = SymbolRegistry::new();
    let m1 = SymbolMetadata {
        visible: true,
        named: false,
        hidden: false,
        terminal: true,
    };
    let id1 = reg.register("tok", m1);

    let m2 = SymbolMetadata {
        visible: false,
        named: true,
        hidden: true,
        terminal: false,
    };
    let id2 = reg.register("tok", m2);

    // Same ID returned
    assert_eq!(id1, id2);
    // Metadata updated
    assert_eq!(reg.get_metadata(id1), Some(m2));
    // Length unchanged
    assert_eq!(reg.len(), 2); // "end" + "tok"
}

// ---------------------------------------------------------------------------
// Grammar.build_registry integration
// ---------------------------------------------------------------------------

#[test]
fn grammar_build_registry_includes_tokens_and_rules() {
    let mut g = Grammar::new("reg_test".to_string());
    g.tokens.insert(
        SymbolId(1),
        Token {
            name: "PLUS".into(),
            pattern: TokenPattern::String("+".into()),
            fragile: false,
        },
    );
    g.rule_names.insert(SymbolId(2), "expression".into());

    let registry = g.build_registry();
    // "end" + "PLUS" + "expression"
    assert!(registry.len() >= 3);
    assert!(registry.get_id("PLUS").is_some());
    assert!(registry.get_id("expression").is_some());
}

#[test]
fn grammar_build_registry_externals() {
    let mut g = Grammar::new("ext_test".to_string());
    g.externals.push(ExternalToken {
        name: "indent".into(),
        symbol_id: SymbolId(50),
    });

    let registry = g.build_registry();
    assert!(registry.get_id("indent").is_some());
}

// ---------------------------------------------------------------------------
// Normalization of complex symbols
// ---------------------------------------------------------------------------

#[test]
fn normalize_optional_creates_auxiliary_rules() {
    let mut g = Grammar::new("norm_opt".to_string());
    g.tokens.insert(
        SymbolId(1),
        Token {
            name: "A".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    g.add_rule(Rule {
        lhs: SymbolId(0),
        rhs: vec![Symbol::Optional(Box::new(Symbol::Terminal(SymbolId(1))))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    g.normalize();

    // After normalization the original rule's rhs should reference a NonTerminal aux
    let rules_for_s = g.get_rules_for_symbol(SymbolId(0)).unwrap();
    assert_eq!(rules_for_s.len(), 1);
    assert!(matches!(rules_for_s[0].rhs[0], Symbol::NonTerminal(_)));

    // The auxiliary symbol should have two productions (inner + epsilon)
    let total_rules: usize = g.rules.values().map(|v| v.len()).sum();
    assert!(total_rules >= 3); // original + 2 aux
}

#[test]
fn normalize_repeat_creates_recursive_rules() {
    let mut g = Grammar::new("norm_rep".to_string());
    g.add_rule(Rule {
        lhs: SymbolId(0),
        rhs: vec![Symbol::Repeat(Box::new(Symbol::Terminal(SymbolId(1))))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    g.normalize();

    let total_rules: usize = g.rules.values().map(|v| v.len()).sum();
    // original + aux->aux inner + aux->epsilon
    assert!(total_rules >= 3);
}

#[test]
fn normalize_sequence_flattens_into_rhs() {
    let mut g = Grammar::new("norm_seq".to_string());
    g.add_rule(Rule {
        lhs: SymbolId(0),
        rhs: vec![Symbol::Sequence(vec![
            Symbol::Terminal(SymbolId(1)),
            Symbol::Terminal(SymbolId(2)),
        ])],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    g.normalize();

    let rules = g.get_rules_for_symbol(SymbolId(0)).unwrap();
    // Sequence flattens: rhs should now contain the two terminals directly
    assert_eq!(rules[0].rhs.len(), 2);
    assert!(matches!(rules[0].rhs[0], Symbol::Terminal(SymbolId(1))));
    assert!(matches!(rules[0].rhs[1], Symbol::Terminal(SymbolId(2))));
}

// ---------------------------------------------------------------------------
// Serialization roundtrip for SymbolId and Symbol
// ---------------------------------------------------------------------------

#[test]
fn symbol_id_serde_roundtrip() {
    let id = SymbolId(123);
    let json = serde_json::to_string(&id).unwrap();
    let back: SymbolId = serde_json::from_str(&json).unwrap();
    assert_eq!(id, back);
}

#[test]
fn symbol_serde_roundtrip_complex() {
    let sym = Symbol::Choice(vec![
        Symbol::Optional(Box::new(Symbol::Terminal(SymbolId(1)))),
        Symbol::Repeat(Box::new(Symbol::NonTerminal(SymbolId(2)))),
        Symbol::Epsilon,
    ]);
    let json = serde_json::to_string(&sym).unwrap();
    let back: Symbol = serde_json::from_str(&json).unwrap();
    assert_eq!(sym, back);
}

// ---------------------------------------------------------------------------
// SymbolId ordering (Ord)
// ---------------------------------------------------------------------------

#[test]
fn symbol_id_ord_consistent_with_u16() {
    for a in 0u16..10 {
        for b in 0u16..10 {
            assert_eq!(SymbolId(a).cmp(&SymbolId(b)), a.cmp(&b));
        }
    }
}

// ---------------------------------------------------------------------------
// FieldId and ProductionId display
// ---------------------------------------------------------------------------

#[test]
fn field_id_display() {
    assert_eq!(format!("{}", FieldId(3)), "Field(3)");
}

#[test]
fn production_id_display() {
    assert_eq!(format!("{}", ProductionId(7)), "Production(7)");
}

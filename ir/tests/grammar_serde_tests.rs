//! Comprehensive serde/serialization tests for Grammar and related IR types.

use adze_ir::builder::GrammarBuilder;
use adze_ir::{
    AliasSequence, Associativity, ConflictDeclaration, ConflictResolution, FieldId, Grammar,
    GrammarError, PrecedenceKind, ProductionId, Rule, Symbol, SymbolId, SymbolMetadata, Token,
    TokenPattern,
};
use indexmap::IndexMap;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Serialize to JSON and deserialize back, returning the round-tripped value.
fn roundtrip_json<T: serde::Serialize + serde::de::DeserializeOwned>(val: &T) -> T {
    let json = serde_json::to_string(val).expect("serialize");
    serde_json::from_str(&json).expect("deserialize")
}

/// Build a minimal arithmetic grammar via `GrammarBuilder`.
fn arith_grammar() -> Grammar {
    GrammarBuilder::new("arith")
        .token("NUMBER", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .rule("expr", vec!["expr", "+", "expr"])
        .rule("expr", vec!["expr", "*", "expr"])
        .rule("expr", vec!["NUMBER"])
        .start("expr")
        .build()
}

// ---------------------------------------------------------------------------
// 1. Basic empty grammar roundtrip
// ---------------------------------------------------------------------------
#[test]
fn test_empty_grammar_roundtrip() {
    let g = Grammar::new("empty".into());
    let g2 = roundtrip_json(&g);
    assert_eq!(g.name, g2.name);
    assert_eq!(g.rules.len(), g2.rules.len());
    assert_eq!(g.tokens.len(), g2.tokens.len());
}

// ---------------------------------------------------------------------------
// 2. Name preservation
// ---------------------------------------------------------------------------
#[test]
fn test_name_preserved_after_roundtrip() {
    let g = Grammar::new("my_grammar".into());
    let g2 = roundtrip_json(&g);
    assert_eq!(g2.name, "my_grammar");
}

// ---------------------------------------------------------------------------
// 3. Builder-produced grammar roundtrip
// ---------------------------------------------------------------------------
#[test]
fn test_builder_grammar_roundtrip() {
    let g = arith_grammar();
    let g2 = roundtrip_json(&g);

    assert_eq!(g.name, g2.name);
    assert_eq!(g.rules.len(), g2.rules.len());
    assert_eq!(g.tokens.len(), g2.tokens.len());
    assert_eq!(g.extras.len(), g2.extras.len());
    assert_eq!(g.rule_names.len(), g2.rule_names.len());
}

// ---------------------------------------------------------------------------
// 4. Token fields preserved
// ---------------------------------------------------------------------------
#[test]
fn test_token_fields_preserved() {
    let g = arith_grammar();
    let g2 = roundtrip_json(&g);

    for (id, token) in &g.tokens {
        let token2 = &g2.tokens[id];
        assert_eq!(token.name, token2.name);
        assert_eq!(token.pattern, token2.pattern);
        assert_eq!(token.fragile, token2.fragile);
    }
}

// ---------------------------------------------------------------------------
// 5. Rule fields preserved (lhs, rhs, precedence, associativity, fields, production_id)
// ---------------------------------------------------------------------------
#[test]
fn test_rule_fields_preserved() {
    let g = GrammarBuilder::new("prec")
        .token("N", r"\d+")
        .token("+", "+")
        .rule_with_precedence("e", vec!["e", "+", "e"], 5, Associativity::Left)
        .rule("e", vec!["N"])
        .start("e")
        .build();

    let g2 = roundtrip_json(&g);

    for (sym_id, rules) in &g.rules {
        let rules2 = &g2.rules[sym_id];
        assert_eq!(rules.len(), rules2.len());
        for (r, r2) in rules.iter().zip(rules2.iter()) {
            assert_eq!(r.lhs, r2.lhs);
            assert_eq!(r.rhs, r2.rhs);
            assert_eq!(r.precedence, r2.precedence);
            assert_eq!(r.associativity, r2.associativity);
            assert_eq!(r.fields, r2.fields);
            assert_eq!(r.production_id, r2.production_id);
        }
    }
}

// ---------------------------------------------------------------------------
// 6. Precedence kind variants (Static / Dynamic)
// ---------------------------------------------------------------------------
#[test]
fn test_precedence_kind_serde() {
    let static_prec = PrecedenceKind::Static(42);
    let dynamic_prec = PrecedenceKind::Dynamic(-3);

    let s = roundtrip_json(&static_prec);
    let d = roundtrip_json(&dynamic_prec);

    assert_eq!(s, PrecedenceKind::Static(42));
    assert_eq!(d, PrecedenceKind::Dynamic(-3));
}

// ---------------------------------------------------------------------------
// 7. Associativity variants
// ---------------------------------------------------------------------------
#[test]
fn test_associativity_serde() {
    for assoc in [
        Associativity::Left,
        Associativity::Right,
        Associativity::None,
    ] {
        assert_eq!(roundtrip_json(&assoc), assoc);
    }
}

// ---------------------------------------------------------------------------
// 8. Symbol enum variants (all arms)
// ---------------------------------------------------------------------------
#[test]
fn test_symbol_variants_serde() {
    let symbols = vec![
        Symbol::Terminal(SymbolId(1)),
        Symbol::NonTerminal(SymbolId(2)),
        Symbol::External(SymbolId(3)),
        Symbol::Optional(Box::new(Symbol::Terminal(SymbolId(4)))),
        Symbol::Repeat(Box::new(Symbol::NonTerminal(SymbolId(5)))),
        Symbol::RepeatOne(Box::new(Symbol::Terminal(SymbolId(6)))),
        Symbol::Choice(vec![
            Symbol::Terminal(SymbolId(7)),
            Symbol::NonTerminal(SymbolId(8)),
        ]),
        Symbol::Sequence(vec![
            Symbol::Terminal(SymbolId(9)),
            Symbol::Terminal(SymbolId(10)),
        ]),
        Symbol::Epsilon,
    ];

    for sym in &symbols {
        assert_eq!(&roundtrip_json(sym), sym);
    }
}

// ---------------------------------------------------------------------------
// 9. TokenPattern variants
// ---------------------------------------------------------------------------
#[test]
fn test_token_pattern_serde() {
    let string_pat = TokenPattern::String("hello".into());
    let regex_pat = TokenPattern::Regex(r"[a-z]+".into());

    assert_eq!(roundtrip_json(&string_pat), string_pat);
    assert_eq!(roundtrip_json(&regex_pat), regex_pat);
}

// ---------------------------------------------------------------------------
// 10. ConflictResolution variants
// ---------------------------------------------------------------------------
#[test]
fn test_conflict_resolution_serde() {
    let resolutions = vec![
        ConflictResolution::Precedence(PrecedenceKind::Static(1)),
        ConflictResolution::Associativity(Associativity::Right),
        ConflictResolution::GLR,
    ];
    for r in &resolutions {
        assert_eq!(&roundtrip_json(r), r);
    }
}

// ---------------------------------------------------------------------------
// 11. SymbolMetadata roundtrip
// ---------------------------------------------------------------------------
#[test]
fn test_symbol_metadata_serde() {
    let meta = SymbolMetadata {
        visible: true,
        named: false,
        hidden: true,
        terminal: false,
    };
    let meta2 = roundtrip_json(&meta);
    assert_eq!(meta, meta2);
}

// ---------------------------------------------------------------------------
// 12. Grammar with externals, extras, conflicts, precedences
// ---------------------------------------------------------------------------
#[test]
fn test_full_grammar_features_roundtrip() {
    let g = GrammarBuilder::new("full")
        .token("ID", r"[a-z]+")
        .token("NUM", r"\d+")
        .token("+", "+")
        .token("INDENT", "INDENT")
        .external("INDENT")
        .extra("WHITESPACE")
        .token("WHITESPACE", r"\s+")
        .precedence(1, Associativity::Left, vec!["+"])
        .rule("expr", vec!["expr", "+", "expr"])
        .rule("expr", vec!["NUM"])
        .rule("expr", vec!["ID"])
        .start("expr")
        .build();

    let g2 = roundtrip_json(&g);

    assert_eq!(g.externals.len(), g2.externals.len());
    assert_eq!(g.extras.len(), g2.extras.len());
    assert_eq!(g.precedences.len(), g2.precedences.len());

    // External token details
    assert_eq!(g.externals[0].name, g2.externals[0].name);
    assert_eq!(g.externals[0].symbol_id, g2.externals[0].symbol_id);

    // Precedence details
    assert_eq!(g.precedences[0].level, g2.precedences[0].level);
    assert_eq!(
        g.precedences[0].associativity,
        g2.precedences[0].associativity
    );
}

// ---------------------------------------------------------------------------
// 13. Python-like grammar (nullable start) roundtrip
// ---------------------------------------------------------------------------
#[test]
fn test_python_like_grammar_roundtrip() {
    let g = GrammarBuilder::python_like();
    let g2 = roundtrip_json(&g);

    assert_eq!(g.name, g2.name);
    assert_eq!(g.rules.len(), g2.rules.len());
    assert_eq!(g.externals.len(), g2.externals.len());
    assert_eq!(g.extras.len(), g2.extras.len());
}

// ---------------------------------------------------------------------------
// 14. JavaScript-like grammar roundtrip
// ---------------------------------------------------------------------------
#[test]
fn test_javascript_like_grammar_roundtrip() {
    let g = GrammarBuilder::javascript_like();
    let g2 = roundtrip_json(&g);

    assert_eq!(g.name, g2.name);
    assert_eq!(g.rules.len(), g2.rules.len());

    // Verify precedence-bearing rules survived
    let prec_count = g2
        .rules
        .values()
        .flat_map(|rs| rs.iter())
        .filter(|r| r.precedence.is_some())
        .count();
    assert!(prec_count >= 4, "expected ≥4 precedence rules");
}

// ---------------------------------------------------------------------------
// 15. from_macro_output (JSON deserialization path)
// ---------------------------------------------------------------------------
#[test]
fn test_from_macro_output_roundtrip() {
    let g = arith_grammar();
    let json = serde_json::to_string(&g).expect("serialize");
    let g2 = Grammar::from_macro_output(&json).expect("from_macro_output");
    assert_eq!(g.name, g2.name);
    assert_eq!(g.rules.len(), g2.rules.len());
}

// ---------------------------------------------------------------------------
// 16. from_macro_output with invalid JSON
// ---------------------------------------------------------------------------
#[test]
fn test_from_macro_output_invalid_json() {
    let result = Grammar::from_macro_output("not valid json {{{");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, GrammarError::ParseError(_)));
}

// ---------------------------------------------------------------------------
// 17. Nested symbol roundtrip (deeply nested Optional/Repeat)
// ---------------------------------------------------------------------------
#[test]
fn test_deeply_nested_symbol_roundtrip() {
    let sym = Symbol::Optional(Box::new(Symbol::Repeat(Box::new(Symbol::Choice(vec![
        Symbol::Terminal(SymbolId(1)),
        Symbol::Sequence(vec![
            Symbol::NonTerminal(SymbolId(2)),
            Symbol::RepeatOne(Box::new(Symbol::External(SymbolId(3)))),
        ]),
    ])))));

    assert_eq!(roundtrip_json(&sym), sym);
}

// ---------------------------------------------------------------------------
// 18. Rule names / rule_names map preserved
// ---------------------------------------------------------------------------
#[test]
fn test_rule_names_preserved() {
    let g = arith_grammar();
    let g2 = roundtrip_json(&g);

    assert_eq!(g.rule_names.len(), g2.rule_names.len());
    for (id, name) in &g.rule_names {
        assert_eq!(g2.rule_names.get(id), Some(name));
    }
}

// ---------------------------------------------------------------------------
// 19. Grammar with alias_sequences and fields
// ---------------------------------------------------------------------------
#[test]
fn test_alias_sequences_and_fields_roundtrip() {
    let mut g = Grammar::new("aliases".into());

    g.fields.insert(FieldId(0), "left".into());
    g.fields.insert(FieldId(1), "right".into());
    g.alias_sequences.insert(
        ProductionId(0),
        AliasSequence {
            aliases: vec![Some("lhs".into()), None, Some("rhs".into())],
        },
    );
    g.max_alias_sequence_length = 3;

    let g2 = roundtrip_json(&g);

    assert_eq!(g2.fields.len(), 2);
    assert_eq!(g2.fields[&FieldId(0)], "left");
    assert_eq!(g2.fields[&FieldId(1)], "right");
    assert_eq!(g2.alias_sequences.len(), 1);

    let seq = &g2.alias_sequences[&ProductionId(0)];
    assert_eq!(seq.aliases.len(), 3);
    assert_eq!(seq.aliases[0], Some("lhs".into()));
    assert_eq!(seq.aliases[1], None);
    assert_eq!(seq.aliases[2], Some("rhs".into()));
    assert_eq!(g2.max_alias_sequence_length, 3);
}

// ---------------------------------------------------------------------------
// 20. Pretty-printed JSON roundtrip
// ---------------------------------------------------------------------------
#[test]
fn test_pretty_json_roundtrip() {
    let g = arith_grammar();
    let pretty = serde_json::to_string_pretty(&g).expect("pretty serialize");
    let g2: Grammar = serde_json::from_str(&pretty).expect("deserialize pretty");
    assert_eq!(g.name, g2.name);
    assert_eq!(g.rules.len(), g2.rules.len());
}

// ---------------------------------------------------------------------------
// 21. Grammar with symbol_registry roundtrip
// ---------------------------------------------------------------------------
#[test]
fn test_grammar_with_registry_roundtrip() {
    let mut g = arith_grammar();
    // Force building the registry
    let _ = g.get_or_build_registry();
    assert!(g.symbol_registry.is_some());

    let g2 = roundtrip_json(&g);
    assert!(g2.symbol_registry.is_some());
    let reg2 = g2.symbol_registry.as_ref().unwrap();
    let reg = g.symbol_registry.as_ref().unwrap();
    assert_eq!(reg.len(), reg2.len());
}

// ---------------------------------------------------------------------------
// 22. Idempotent double roundtrip
// ---------------------------------------------------------------------------
#[test]
fn test_double_roundtrip_idempotent() {
    let g = GrammarBuilder::javascript_like();
    let json1 = serde_json::to_string(&g).expect("ser1");
    let g2: Grammar = serde_json::from_str(&json1).expect("de1");
    let json2 = serde_json::to_string(&g2).expect("ser2");

    // JSON strings must be identical after two roundtrips
    assert_eq!(json1, json2);
}

// ---------------------------------------------------------------------------
// 23. Conflict declaration roundtrip
// ---------------------------------------------------------------------------
#[test]
fn test_conflict_declaration_roundtrip() {
    let mut g = Grammar::new("conflicts".into());
    g.conflicts.push(ConflictDeclaration {
        symbols: vec![SymbolId(1), SymbolId(2)],
        resolution: ConflictResolution::GLR,
    });
    g.conflicts.push(ConflictDeclaration {
        symbols: vec![SymbolId(3)],
        resolution: ConflictResolution::Precedence(PrecedenceKind::Dynamic(10)),
    });

    let g2 = roundtrip_json(&g);
    assert_eq!(g2.conflicts.len(), 2);
    assert_eq!(g2.conflicts[0].symbols, vec![SymbolId(1), SymbolId(2)]);
    assert_eq!(g2.conflicts[0].resolution, ConflictResolution::GLR);
    assert_eq!(
        g2.conflicts[1].resolution,
        ConflictResolution::Precedence(PrecedenceKind::Dynamic(10))
    );
}

// ---------------------------------------------------------------------------
// 24. Fragile token roundtrip
// ---------------------------------------------------------------------------
#[test]
fn test_fragile_token_roundtrip() {
    let g = GrammarBuilder::new("fragile")
        .fragile_token("NEWLINE", r"\n")
        .token("ID", r"[a-z]+")
        .rule("stmt", vec!["ID", "NEWLINE"])
        .start("stmt")
        .build();

    let g2 = roundtrip_json(&g);

    let newline_fragile = g2.tokens.values().find(|t| t.name == "NEWLINE").unwrap();
    let id_not_fragile = g2.tokens.values().find(|t| t.name == "ID").unwrap();

    assert!(newline_fragile.fragile);
    assert!(!id_not_fragile.fragile);
}

// ---------------------------------------------------------------------------
// 25. Supertypes and inline_rules roundtrip
// ---------------------------------------------------------------------------
#[test]
fn test_supertypes_and_inline_rules_roundtrip() {
    let mut g = Grammar::new("supertypes".into());
    g.supertypes = vec![SymbolId(10), SymbolId(20)];
    g.inline_rules = vec![SymbolId(30)];

    let g2 = roundtrip_json(&g);
    assert_eq!(g2.supertypes, vec![SymbolId(10), SymbolId(20)]);
    assert_eq!(g2.inline_rules, vec![SymbolId(30)]);
}

// ---------------------------------------------------------------------------
// 26. Production IDs map roundtrip
// ---------------------------------------------------------------------------
#[test]
fn test_production_ids_map_roundtrip() {
    use adze_ir::RuleId;

    let mut g = Grammar::new("prod_ids".into());
    g.production_ids.insert(RuleId(0), ProductionId(100));
    g.production_ids.insert(RuleId(1), ProductionId(200));

    let g2 = roundtrip_json(&g);
    assert_eq!(g2.production_ids.len(), 2);
    assert_eq!(g2.production_ids[&RuleId(0)], ProductionId(100));
    assert_eq!(g2.production_ids[&RuleId(1)], ProductionId(200));
}

// ---------------------------------------------------------------------------
// 27. Normalized grammar roundtrip (normalize then serialize)
// ---------------------------------------------------------------------------
#[test]
fn test_normalized_grammar_roundtrip() {
    let mut g = Grammar::new("norm".into());

    // Manually add a rule that uses Optional
    let tok_id = SymbolId(1);
    g.tokens.insert(
        tok_id,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    g.add_rule(Rule {
        lhs: SymbolId(2),
        rhs: vec![Symbol::Optional(Box::new(Symbol::Terminal(tok_id)))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    g.rule_names.insert(SymbolId(2), "start".into());

    g.normalize();

    // After normalization there should be auxiliary rules
    let total_rules: usize = g.rules.values().map(|rs| rs.len()).sum();
    assert!(total_rules > 1, "normalization should create aux rules");

    // Round-trip the normalized grammar
    let g2 = roundtrip_json(&g);
    let total_rules2: usize = g2.rules.values().map(|rs| rs.len()).sum();
    assert_eq!(total_rules, total_rules2);
}

// ---------------------------------------------------------------------------
// 28. Empty collections edge case
// ---------------------------------------------------------------------------
#[test]
fn test_empty_collections_roundtrip() {
    let g = Grammar {
        name: "bare".into(),
        rules: IndexMap::new(),
        tokens: IndexMap::new(),
        precedences: vec![],
        conflicts: vec![],
        externals: vec![],
        extras: vec![],
        fields: IndexMap::new(),
        supertypes: vec![],
        inline_rules: vec![],
        alias_sequences: IndexMap::new(),
        production_ids: IndexMap::new(),
        max_alias_sequence_length: 0,
        rule_names: IndexMap::new(),
        symbol_registry: None,
        word_token: None,
        lexical_metadata: IndexMap::new(),
        start_symbol: None,
        wrapper_token_relations: IndexMap::new(),
    };

    let g2 = roundtrip_json(&g);
    assert_eq!(g2.rules.len(), 0);
    assert_eq!(g2.tokens.len(), 0);
    assert_eq!(g2.precedences.len(), 0);
    assert_eq!(g2.conflicts.len(), 0);
    assert_eq!(g2.externals.len(), 0);
    assert_eq!(g2.extras.len(), 0);
    assert_eq!(g2.fields.len(), 0);
    assert!(g2.symbol_registry.is_none());
}

// ---------------------------------------------------------------------------
// 29. Unicode grammar name
// ---------------------------------------------------------------------------
#[test]
fn test_unicode_grammar_name_roundtrip() {
    let g = Grammar::new("日本語文法".into());
    let g2 = roundtrip_json(&g);
    assert_eq!(g2.name, "日本語文法");
}

// ---------------------------------------------------------------------------
// 30. JSON stability – key order (IndexMap preserves insertion order)
// ---------------------------------------------------------------------------
#[test]
fn test_json_key_order_stable() {
    let g = arith_grammar();
    let json1 = serde_json::to_string(&g).unwrap();
    let json2 = serde_json::to_string(&g).unwrap();
    assert_eq!(json1, json2, "serialization must be deterministic");
}

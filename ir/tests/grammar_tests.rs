use adze_ir::*;

#[test]
fn test_grammar_creation() {
    let mut grammar = Grammar {
        name: "TestGrammar".to_string(),
        ..Default::default()
    };

    // Add a simple rule
    let rule = Rule {
        lhs: SymbolId(0),
        rhs: vec![Symbol::Terminal(SymbolId(1))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    };

    grammar.add_rule(rule);
    grammar.rule_names.insert(SymbolId(0), "start".to_string());
    grammar.set_start_symbol(SymbolId(0));

    assert_eq!(grammar.name, "TestGrammar");
    assert_eq!(grammar.rules.len(), 1);
    assert_eq!(grammar.get_rules_for_symbol(SymbolId(0)).unwrap().len(), 1);
    assert_eq!(grammar.start_symbol(), Some(SymbolId(0)));
}

#[test]
fn test_multiple_rules_for_symbol() {
    let mut grammar = Grammar::default();

    // Add multiple rules for same LHS
    for i in 0..3 {
        let rule = Rule {
            lhs: SymbolId(0),
            rhs: vec![Symbol::Terminal(SymbolId(i + 1))],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(i),
        };
        grammar.add_rule(rule);
    }

    let rules = grammar.get_rules_for_symbol(SymbolId(0)).unwrap();
    assert_eq!(rules.len(), 3);

    let all_rules: Vec<_> = grammar.all_rules().collect();
    assert_eq!(all_rules.len(), 3);
}

#[test]
fn test_rule_with_precedence() {
    let rule = Rule {
        lhs: SymbolId(0),
        rhs: vec![
            Symbol::NonTerminal(SymbolId(1)),
            Symbol::Terminal(SymbolId(2)),
        ],
        precedence: Some(PrecedenceKind::Static(10)),
        associativity: Some(Associativity::Left),
        fields: vec![(FieldId(0), 0), (FieldId(1), 1)],
        production_id: ProductionId(0),
    };

    assert_eq!(rule.precedence, Some(PrecedenceKind::Static(10)));
    assert_eq!(rule.associativity, Some(Associativity::Left));
    assert_eq!(rule.fields.len(), 2);
}

#[test]
fn test_token_creation() {
    let token = Token {
        name: "number".to_string(),
        pattern: TokenPattern::Regex(r"\d+".to_string()),
        fragile: false,
    };

    assert_eq!(token.name, "number");
    assert!(!token.fragile);
    assert!(matches!(token.pattern, TokenPattern::Regex(_)));

    let fragile_token = Token {
        name: "keyword".to_string(),
        pattern: TokenPattern::String("if".to_string()),
        fragile: true,
    };

    assert!(fragile_token.fragile);
    assert!(matches!(fragile_token.pattern, TokenPattern::String(_)));
}

#[test]
fn test_external_token() {
    let external = ExternalToken {
        name: "block_comment".to_string(),
        symbol_id: SymbolId(100),
    };

    assert_eq!(external.name, "block_comment");
    assert_eq!(external.symbol_id, SymbolId(100));
}

#[test]
fn test_alias_sequence() {
    let alias_seq = AliasSequence {
        aliases: vec![
            Some("operator".to_string()),
            None,
            Some("value".to_string()),
        ],
    };

    assert_eq!(alias_seq.aliases.len(), 3);
    assert_eq!(alias_seq.aliases[0].as_ref().unwrap(), "operator");
    assert!(alias_seq.aliases[1].is_none());
    assert_eq!(alias_seq.aliases[2].as_ref().unwrap(), "value");
}

#[test]
fn test_conflict_declaration() {
    let conflict = ConflictDeclaration {
        symbols: vec![SymbolId(1), SymbolId(2), SymbolId(3)],
        resolution: ConflictResolution::GLR,
    };

    assert_eq!(conflict.symbols.len(), 3);
    assert_eq!(conflict.symbols[0], SymbolId(1));
    assert!(matches!(conflict.resolution, ConflictResolution::GLR));
}

#[test]
fn test_precedence_kinds() {
    let static_prec = PrecedenceKind::Static(5);
    let dynamic_prec = PrecedenceKind::Dynamic(-3);

    assert!(matches!(static_prec, PrecedenceKind::Static(5)));
    assert!(matches!(dynamic_prec, PrecedenceKind::Dynamic(-3)));
}

#[test]
fn test_symbol_types() {
    let terminal = Symbol::Terminal(SymbolId(1));
    let non_terminal = Symbol::NonTerminal(SymbolId(2));
    let external = Symbol::External(SymbolId(3));

    assert!(matches!(terminal, Symbol::Terminal(_)));
    assert!(matches!(non_terminal, Symbol::NonTerminal(_)));
    assert!(matches!(external, Symbol::External(_)));
}

#[test]
fn test_grammar_with_fields() {
    let mut grammar = Grammar::default();

    // Add fields
    grammar.fields.insert(FieldId(0), "left".to_string());
    grammar.fields.insert(FieldId(1), "operator".to_string());
    grammar.fields.insert(FieldId(2), "right".to_string());

    assert_eq!(grammar.fields.len(), 3);
    assert_eq!(grammar.fields.get(&FieldId(0)).unwrap(), "left");
    assert_eq!(grammar.fields.get(&FieldId(1)).unwrap(), "operator");
    assert_eq!(grammar.fields.get(&FieldId(2)).unwrap(), "right");
}

#[test]
fn test_grammar_with_supertypes() {
    let grammar = Grammar {
        supertypes: vec![SymbolId(10), SymbolId(11), SymbolId(12)],
        ..Default::default()
    };

    assert_eq!(grammar.supertypes.len(), 3);
    assert_eq!(grammar.supertypes[0], SymbolId(10));
}

#[test]
fn test_associativity() {
    let left = Associativity::Left;
    let right = Associativity::Right;
    let none = Associativity::None;

    assert!(matches!(left, Associativity::Left));
    assert!(matches!(right, Associativity::Right));
    assert!(matches!(none, Associativity::None));
}

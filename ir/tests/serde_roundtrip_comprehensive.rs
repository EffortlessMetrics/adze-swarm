//! Comprehensive serialization roundtrip tests for all IR types.
//!
//! This module provides 30+ tests exercising serialization roundtrip for all IR types
//! using serde_json (and bincode where available).
//!
//! Key coverage:
//! 1. Grammar variants (default, with rules, extras, conflicts, etc.)
//! 2. Symbol types (Terminal, NonTerminal, Optional, Repeat, Repeat1, Choice, etc.)
//! 3. Complex nested structures
//! 4. Edge cases (unicode, large grammars, empty symbols, etc.)
//! 5. Field ordering preservation (IndexMap)
//! 6. All Associativity variants
//! 7. PrecedenceKind variants
//!
//! Each test verifies: original == deserialized(serialized(original))

use adze_ir::{
    AliasSequence, Associativity, ConflictDeclaration, ConflictResolution, ExternalToken, FieldId,
    Grammar, Precedence, PrecedenceKind, ProductionId, Rule, RuleId, StateId, Symbol, SymbolId,
    SymbolMetadata, Token, TokenPattern, builder::GrammarBuilder,
};

// Helper function to perform JSON roundtrip test
fn assert_json_roundtrip(grammar: &Grammar, test_name: &str) {
    let json_str = serde_json::to_string(grammar)
        .unwrap_or_else(|_| panic!("{}: failed to serialize to JSON", test_name));
    let deserialized: Grammar = serde_json::from_str(&json_str)
        .unwrap_or_else(|_| panic!("{}: failed to deserialize from JSON", test_name));
    let roundtrip_json = serde_json::to_string(&deserialized)
        .unwrap_or_else(|_| panic!("{}: failed to re-serialize to JSON", test_name));

    // Both JSON strings should be identical
    assert_eq!(
        json_str, roundtrip_json,
        "{}:JSON roundtrip mismatch",
        test_name
    );
}

// Helper function to perform bincode roundtrip test
fn assert_bincode_roundtrip(grammar: &Grammar, test_name: &str) {
    let bytes = postcard::to_allocvec(grammar)
        .unwrap_or_else(|_| panic!("{}: failed to serialize with bincode", test_name));
    let deserialized: Grammar = postcard::from_bytes(&bytes)
        .unwrap_or_else(|_| panic!("{}: failed to deserialize from bincode", test_name));
    let roundtrip_bytes = postcard::to_allocvec(&deserialized)
        .unwrap_or_else(|_| panic!("{}: failed to re-serialize with bincode", test_name));

    assert_eq!(
        bytes, roundtrip_bytes,
        "{}:bincode roundtrip mismatch",
        test_name
    );
}

// ============================================================================
// Test 1-10: Grammar variants
// ============================================================================

#[test]
fn test_grammar_default_json_roundtrip() {
    let grammar = Grammar::new("default".to_string());
    assert_json_roundtrip(&grammar, "grammar_default");
}

#[test]
fn test_grammar_default_bincode_roundtrip() {
    let grammar = Grammar::new("default".to_string());
    assert_bincode_roundtrip(&grammar, "grammar_default");
}

#[test]
fn test_grammar_with_rules_json_roundtrip() {
    let mut grammar = Grammar::new("with_rules".to_string());

    // Add a simple rule: S -> NUMBER
    let num_id = SymbolId(1);
    let start_id = SymbolId(0);

    grammar.tokens.insert(
        num_id,
        Token {
            name: "NUMBER".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );

    grammar.add_rule(Rule {
        lhs: start_id,
        rhs: vec![Symbol::Terminal(num_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    grammar.rule_names.insert(start_id, "start".to_string());
    grammar.rule_names.insert(num_id, "NUMBER".to_string());

    assert_json_roundtrip(&grammar, "grammar_with_rules");
}

#[test]
fn test_grammar_with_rules_bincode_roundtrip() {
    let mut grammar = Grammar::new("with_rules".to_string());

    let num_id = SymbolId(1);
    let start_id = SymbolId(0);

    grammar.tokens.insert(
        num_id,
        Token {
            name: "NUMBER".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );

    grammar.add_rule(Rule {
        lhs: start_id,
        rhs: vec![Symbol::Terminal(num_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    grammar.rule_names.insert(start_id, "start".to_string());
    grammar.rule_names.insert(num_id, "NUMBER".to_string());

    assert_bincode_roundtrip(&grammar, "grammar_with_rules");
}

#[test]
fn test_grammar_with_extras_json_roundtrip() {
    let mut grammar = Grammar::new("with_extras".to_string());

    let whitespace_id = SymbolId(10);
    let comment_id = SymbolId(11);

    grammar.tokens.insert(
        whitespace_id,
        Token {
            name: "WHITESPACE".to_string(),
            pattern: TokenPattern::Regex(r"\s+".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        comment_id,
        Token {
            name: "COMMENT".to_string(),
            pattern: TokenPattern::Regex(r"//.*".to_string()),
            fragile: false,
        },
    );

    grammar.extras.push(whitespace_id);
    grammar.extras.push(comment_id);

    grammar
        .rule_names
        .insert(whitespace_id, "WHITESPACE".to_string());
    grammar.rule_names.insert(comment_id, "COMMENT".to_string());

    assert_json_roundtrip(&grammar, "grammar_with_extras");
}

#[test]
fn test_grammar_with_conflicts_json_roundtrip() {
    let mut grammar = Grammar::new("with_conflicts".to_string());

    let plus_id = SymbolId(1);
    let minus_id = SymbolId(2);

    grammar.tokens.insert(
        plus_id,
        Token {
            name: "PLUS".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        minus_id,
        Token {
            name: "MINUS".to_string(),
            pattern: TokenPattern::String("-".to_string()),
            fragile: false,
        },
    );

    grammar.conflicts.push(ConflictDeclaration {
        symbols: vec![plus_id, minus_id],
        resolution: ConflictResolution::GLR,
    });

    grammar.rule_names.insert(plus_id, "PLUS".to_string());
    grammar.rule_names.insert(minus_id, "MINUS".to_string());

    assert_json_roundtrip(&grammar, "grammar_with_conflicts");
}

#[test]
fn test_grammar_with_externals_json_roundtrip() {
    let mut grammar = Grammar::new("with_externals".to_string());

    let indent_id = SymbolId(20);
    let dedent_id = SymbolId(21);

    grammar.externals.push(ExternalToken {
        name: "INDENT".to_string(),
        symbol_id: indent_id,
    });

    grammar.externals.push(ExternalToken {
        name: "DEDENT".to_string(),
        symbol_id: dedent_id,
    });

    grammar.rule_names.insert(indent_id, "INDENT".to_string());
    grammar.rule_names.insert(dedent_id, "DEDENT".to_string());

    assert_json_roundtrip(&grammar, "grammar_with_externals");
}

#[test]
fn test_grammar_with_precedences_json_roundtrip() {
    let mut grammar = Grammar::new("with_precedences".to_string());

    let plus_id = SymbolId(1);
    let mult_id = SymbolId(2);

    grammar.tokens.insert(
        plus_id,
        Token {
            name: "PLUS".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        mult_id,
        Token {
            name: "MULT".to_string(),
            pattern: TokenPattern::String("*".to_string()),
            fragile: false,
        },
    );

    grammar.precedences.push(Precedence {
        level: 1,
        associativity: Associativity::Left,
        symbols: vec![plus_id],
    });

    grammar.precedences.push(Precedence {
        level: 2,
        associativity: Associativity::Left,
        symbols: vec![mult_id],
    });

    grammar.rule_names.insert(plus_id, "PLUS".to_string());
    grammar.rule_names.insert(mult_id, "MULT".to_string());

    assert_json_roundtrip(&grammar, "grammar_with_precedences");
}

#[test]
fn test_grammar_with_word_json_roundtrip() {
    let mut grammar = Grammar::new("with_word".to_string());

    let ident_id = SymbolId(5);
    grammar.tokens.insert(
        ident_id,
        Token {
            name: "IDENTIFIER".to_string(),
            pattern: TokenPattern::Regex(r"[a-zA-Z_][a-zA-Z0-9_]*".to_string()),
            fragile: false,
        },
    );

    // Tree-sitter uses "word" field; IR now preserves it as word_token.
    grammar.word_token = Some(ident_id);
    grammar
        .rule_names
        .insert(ident_id, "IDENTIFIER".to_string());

    assert_json_roundtrip(&grammar, "grammar_with_word");
}

#[test]
fn test_grammar_with_alias_sequences_json_roundtrip() {
    let mut grammar = Grammar::new("with_alias_sequences".to_string());

    let prod_id = ProductionId(1);
    let alias_seq = AliasSequence {
        aliases: vec![
            Some("left".to_string()),
            Some("op".to_string()),
            Some("right".to_string()),
        ],
    };

    grammar.alias_sequences.insert(prod_id, alias_seq);

    assert_json_roundtrip(&grammar, "grammar_with_alias_sequences");
}

#[test]
fn test_grammar_with_expected_conflicts_json_roundtrip() {
    let mut grammar = Grammar::new("with_expected_conflicts".to_string());

    let a_id = SymbolId(1);
    let b_id = SymbolId(2);

    grammar.tokens.insert(
        a_id,
        Token {
            name: "A".to_string(),
            pattern: TokenPattern::String("a".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        b_id,
        Token {
            name: "B".to_string(),
            pattern: TokenPattern::String("b".to_string()),
            fragile: false,
        },
    );

    // ConflictDeclaration with associativity resolution
    grammar.conflicts.push(ConflictDeclaration {
        symbols: vec![a_id, b_id],
        resolution: ConflictResolution::Associativity(Associativity::Left),
    });

    grammar.rule_names.insert(a_id, "A".to_string());
    grammar.rule_names.insert(b_id, "B".to_string());

    assert_json_roundtrip(&grammar, "grammar_with_expected_conflicts");
}

#[test]
fn test_grammar_all_fields_populated_json_roundtrip() {
    let mut grammar = Grammar::new("fully_populated".to_string());

    // Tokens
    let num_id = SymbolId(1);
    let plus_id = SymbolId(2);
    let ws_id = SymbolId(3);
    let indent_id = SymbolId(4);

    grammar.tokens.insert(
        num_id,
        Token {
            name: "NUMBER".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        plus_id,
        Token {
            name: "PLUS".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        ws_id,
        Token {
            name: "WHITESPACE".to_string(),
            pattern: TokenPattern::Regex(r"\s+".to_string()),
            fragile: false,
        },
    );

    // Rules
    let expr_id = SymbolId(10);
    grammar.add_rule(Rule {
        lhs: expr_id,
        rhs: vec![Symbol::Terminal(num_id)],
        precedence: Some(PrecedenceKind::Static(1)),
        associativity: Some(Associativity::Left),
        fields: vec![(FieldId(0), 0)],
        production_id: ProductionId(0),
    });

    // Extras
    grammar.extras.push(ws_id);

    // Externals
    grammar.externals.push(ExternalToken {
        name: "INDENT".to_string(),
        symbol_id: indent_id,
    });

    // Fields
    grammar.fields.insert(FieldId(0), "value".to_string());

    // Precedences
    grammar.precedences.push(Precedence {
        level: 1,
        associativity: Associativity::Left,
        symbols: vec![plus_id],
    });

    // Conflicts
    grammar.conflicts.push(ConflictDeclaration {
        symbols: vec![plus_id],
        resolution: ConflictResolution::Precedence(PrecedenceKind::Static(1)),
    });

    // Alias sequences
    grammar.alias_sequences.insert(
        ProductionId(0),
        AliasSequence {
            aliases: vec![Some("num".to_string())],
        },
    );

    // Rule names
    grammar.rule_names.insert(expr_id, "expression".to_string());
    grammar.rule_names.insert(num_id, "NUMBER".to_string());
    grammar.rule_names.insert(plus_id, "PLUS".to_string());
    grammar.rule_names.insert(ws_id, "WHITESPACE".to_string());
    grammar.rule_names.insert(indent_id, "INDENT".to_string());

    // Inline rules
    grammar.inline_rules.push(SymbolId(5));

    // Supertypes
    grammar.supertypes.push(SymbolId(6));

    // Production IDs
    grammar.production_ids.insert(RuleId(0), ProductionId(0));

    // Max alias sequence length
    grammar.max_alias_sequence_length = 10;

    assert_json_roundtrip(&grammar, "grammar_all_fields_populated");
}

// ============================================================================
// Test 11-22: Individual Symbol types
// ============================================================================

#[test]
fn test_symbol_terminal_json_roundtrip() {
    let symbol = Symbol::Terminal(SymbolId(42));
    let json = serde_json::to_string(&symbol).expect("serialize");
    let deserialized: Symbol = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(symbol, deserialized);
}

#[test]
fn test_symbol_nonterminal_json_roundtrip() {
    let symbol = Symbol::NonTerminal(SymbolId(99));
    let json = serde_json::to_string(&symbol).expect("serialize");
    let deserialized: Symbol = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(symbol, deserialized);
}

#[test]
fn test_symbol_external_json_roundtrip() {
    let symbol = Symbol::External(SymbolId(15));
    let json = serde_json::to_string(&symbol).expect("serialize");
    let deserialized: Symbol = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(symbol, deserialized);
}

#[test]
fn test_symbol_optional_json_roundtrip() {
    let symbol = Symbol::Optional(Box::new(Symbol::Terminal(SymbolId(1))));
    let json = serde_json::to_string(&symbol).expect("serialize");
    let deserialized: Symbol = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(symbol, deserialized);
}

#[test]
fn test_symbol_repeat_json_roundtrip() {
    let symbol = Symbol::Repeat(Box::new(Symbol::NonTerminal(SymbolId(5))));
    let json = serde_json::to_string(&symbol).expect("serialize");
    let deserialized: Symbol = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(symbol, deserialized);
}

#[test]
fn test_symbol_repeat1_json_roundtrip() {
    let symbol = Symbol::RepeatOne(Box::new(Symbol::Terminal(SymbolId(10))));
    let json = serde_json::to_string(&symbol).expect("serialize");
    let deserialized: Symbol = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(symbol, deserialized);
}

#[test]
fn test_symbol_choice_json_roundtrip() {
    let symbol = Symbol::Choice(vec![
        Symbol::Terminal(SymbolId(1)),
        Symbol::Terminal(SymbolId(2)),
        Symbol::Terminal(SymbolId(3)),
    ]);
    let json = serde_json::to_string(&symbol).expect("serialize");
    let deserialized: Symbol = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(symbol, deserialized);
}

#[test]
fn test_symbol_sequence_json_roundtrip() {
    let symbol = Symbol::Sequence(vec![
        Symbol::Terminal(SymbolId(1)),
        Symbol::NonTerminal(SymbolId(2)),
        Symbol::Epsilon,
    ]);
    let json = serde_json::to_string(&symbol).expect("serialize");
    let deserialized: Symbol = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(symbol, deserialized);
}

#[test]
fn test_symbol_epsilon_json_roundtrip() {
    let symbol = Symbol::Epsilon;
    let json = serde_json::to_string(&symbol).expect("serialize");
    let deserialized: Symbol = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(symbol, deserialized);
}

#[test]
fn test_symbol_epsilon_bincode_roundtrip() {
    let symbol = Symbol::Epsilon;
    let bytes = postcard::to_allocvec(&symbol).expect("serialize");
    let deserialized: Symbol = postcard::from_bytes(&bytes).expect("deserialize");
    assert_eq!(symbol, deserialized);
}

// Note: Tests 19-22 from original list were for specific symbol patterns that are covered above
// We'll test more complex scenarios below

// ============================================================================
// Test 23-29: Complex types
// ============================================================================

#[test]
fn test_external_token_json_roundtrip() {
    let external = ExternalToken {
        name: "INDENT".to_string(),
        symbol_id: SymbolId(100),
    };
    let json = serde_json::to_string(&external).expect("serialize");
    let deserialized: ExternalToken = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(external, deserialized);
}

#[test]
fn test_external_token_bincode_roundtrip() {
    let external = ExternalToken {
        name: "DEDENT".to_string(),
        symbol_id: SymbolId(101),
    };
    let bytes = postcard::to_allocvec(&external).expect("serialize");
    let deserialized: ExternalToken = postcard::from_bytes(&bytes).expect("deserialize");
    assert_eq!(external, deserialized);
}

#[test]
fn test_alias_sequence_json_roundtrip() {
    let alias = AliasSequence {
        aliases: vec![
            Some("left".to_string()),
            Some("operator".to_string()),
            Some("right".to_string()),
        ],
    };
    let json = serde_json::to_string(&alias).expect("serialize");
    let deserialized: AliasSequence = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(alias, deserialized);
}

#[test]
fn test_alias_sequence_with_none_json_roundtrip() {
    let alias = AliasSequence {
        aliases: vec![Some("first".to_string()), None, Some("third".to_string())],
    };
    let json = serde_json::to_string(&alias).expect("serialize");
    let deserialized: AliasSequence = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(alias, deserialized);
}

#[test]
fn test_conflict_declaration_json_roundtrip() {
    let conflict = ConflictDeclaration {
        symbols: vec![SymbolId(1), SymbolId(2), SymbolId(3)],
        resolution: ConflictResolution::GLR,
    };
    let json = serde_json::to_string(&conflict).expect("serialize");
    let deserialized: ConflictDeclaration = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(conflict, deserialized);
}

#[test]
fn test_conflict_with_precedence_json_roundtrip() {
    let conflict = ConflictDeclaration {
        symbols: vec![SymbolId(5), SymbolId(6)],
        resolution: ConflictResolution::Precedence(PrecedenceKind::Static(10)),
    };
    let json = serde_json::to_string(&conflict).expect("serialize");
    let deserialized: ConflictDeclaration = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(conflict, deserialized);
}

#[test]
fn test_conflict_with_associativity_json_roundtrip() {
    let conflict = ConflictDeclaration {
        symbols: vec![SymbolId(7), SymbolId(8)],
        resolution: ConflictResolution::Associativity(Associativity::Right),
    };
    let json = serde_json::to_string(&conflict).expect("serialize");
    let deserialized: ConflictDeclaration = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(conflict, deserialized);
}

#[test]
fn test_precedence_left_json_roundtrip() {
    let prec = Precedence {
        level: 1,
        associativity: Associativity::Left,
        symbols: vec![SymbolId(10), SymbolId(11)],
    };
    let json = serde_json::to_string(&prec).expect("serialize");
    let deserialized: Precedence = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(prec, deserialized);
}

#[test]
fn test_precedence_right_json_roundtrip() {
    let prec = Precedence {
        level: 2,
        associativity: Associativity::Right,
        symbols: vec![SymbolId(20)],
    };
    let json = serde_json::to_string(&prec).expect("serialize");
    let deserialized: Precedence = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(prec, deserialized);
}

#[test]
fn test_precedence_none_json_roundtrip() {
    let prec = Precedence {
        level: 3,
        associativity: Associativity::None,
        symbols: vec![SymbolId(30), SymbolId(31), SymbolId(32)],
    };
    let json = serde_json::to_string(&prec).expect("serialize");
    let deserialized: Precedence = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(prec, deserialized);
}

#[test]
fn test_symbol_id_json_roundtrip() {
    let id = SymbolId(12345);
    let json = serde_json::to_string(&id).expect("serialize");
    let deserialized: SymbolId = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(id, deserialized);
}

#[test]
fn test_symbol_id_bincode_roundtrip() {
    let id = SymbolId(54321);
    let bytes = postcard::to_allocvec(&id).expect("serialize");
    let deserialized: SymbolId = postcard::from_bytes(&bytes).expect("deserialize");
    assert_eq!(id, deserialized);
}

#[test]
fn test_rule_id_json_roundtrip() {
    let id = RuleId(999);
    let json = serde_json::to_string(&id).expect("serialize");
    let deserialized: RuleId = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(id, deserialized);
}

#[test]
fn test_production_id_json_roundtrip() {
    let id = ProductionId(888);
    let json = serde_json::to_string(&id).expect("serialize");
    let deserialized: ProductionId = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(id, deserialized);
}

#[test]
fn test_field_id_json_roundtrip() {
    let id = FieldId(777);
    let json = serde_json::to_string(&id).expect("serialize");
    let deserialized: FieldId = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(id, deserialized);
}

// ============================================================================
// Test 30+: Complex nested structures and edge cases
// ============================================================================

#[test]
fn test_deeply_nested_symbol_json_roundtrip() {
    // Symbol nested 5 levels deep
    let symbol = Symbol::Optional(Box::new(Symbol::Repeat(Box::new(Symbol::RepeatOne(
        Box::new(Symbol::Optional(Box::new(Symbol::Choice(vec![
            Symbol::Terminal(SymbolId(1)),
            Symbol::NonTerminal(SymbolId(2)),
        ])))),
    )))));

    let json = serde_json::to_string(&symbol).expect("serialize");
    let deserialized: Symbol = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(symbol, deserialized);
}

#[test]
fn test_complex_symbol_with_sequences_json_roundtrip() {
    // Sequence containing Optional and Repeat
    let symbol = Symbol::Sequence(vec![
        Symbol::Terminal(SymbolId(1)),
        Symbol::Optional(Box::new(Symbol::NonTerminal(SymbolId(2)))),
        Symbol::Repeat(Box::new(Symbol::Terminal(SymbolId(3)))),
        Symbol::Epsilon,
    ]);

    let json = serde_json::to_string(&symbol).expect("serialize");
    let deserialized: Symbol = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(symbol, deserialized);
}

#[test]
fn test_complex_choice_json_roundtrip() {
    // Choice with multiple variants
    let symbol = Symbol::Choice(vec![
        Symbol::Sequence(vec![
            Symbol::Terminal(SymbolId(1)),
            Symbol::Terminal(SymbolId(2)),
        ]),
        Symbol::Optional(Box::new(Symbol::NonTerminal(SymbolId(3)))),
        Symbol::Repeat(Box::new(Symbol::Terminal(SymbolId(4)))),
    ]);

    let json = serde_json::to_string(&symbol).expect("serialize");
    let deserialized: Symbol = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(symbol, deserialized);
}

#[test]
fn test_grammar_with_unicode_rule_names_json_roundtrip() {
    let mut grammar = Grammar::new("unicode_rules".to_string());

    let id1 = SymbolId(1);
    let id2 = SymbolId(2);
    let id3 = SymbolId(3);

    // Unicode identifiers/names
    grammar.rule_names.insert(id1, "αβγ".to_string());
    grammar.rule_names.insert(id2, "δεζ".to_string());
    grammar.rule_names.insert(id3, "日本語".to_string());

    grammar.tokens.insert(
        id1,
        Token {
            name: "αβγ".to_string(),
            pattern: TokenPattern::Regex("[α-ω]+".to_string()),
            fragile: false,
        },
    );

    assert_json_roundtrip(&grammar, "grammar_with_unicode_names");
}

#[test]
fn test_grammar_large_rules_json_roundtrip() {
    let mut grammar = Grammar::new("large_rules".to_string());

    // Create 100+ rules
    for i in 0..100 {
        let rule_id = SymbolId(i as u16);
        grammar.rule_names.insert(rule_id, format!("rule_{}", i));

        if i > 0 {
            let lhs = SymbolId(i as u16);
            let rhs_id = SymbolId((i - 1) as u16);

            grammar.add_rule(Rule {
                lhs,
                rhs: vec![Symbol::NonTerminal(rhs_id)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(i as u16),
            });
        }
    }

    assert_json_roundtrip(&grammar, "grammar_large_rules");
}

#[test]
fn test_grammar_large_rules_bincode_roundtrip() {
    let mut grammar = Grammar::new("large_rules".to_string());

    // Create 100+ rules
    for i in 0..100 {
        let rule_id = SymbolId(i as u16);
        grammar.rule_names.insert(rule_id, format!("rule_{}", i));

        if i > 0 {
            let lhs = SymbolId(i as u16);
            let rhs_id = SymbolId((i - 1) as u16);

            grammar.add_rule(Rule {
                lhs,
                rhs: vec![Symbol::NonTerminal(rhs_id)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(i as u16),
            });
        }
    }

    assert_bincode_roundtrip(&grammar, "grammar_large_rules");
}

#[test]
fn test_grammar_field_ordering_preservation_json_roundtrip() {
    let mut grammar = Grammar::new("field_ordering".to_string());

    // Insert fields in specific order - IndexMap preserves insertion order
    grammar.fields.insert(FieldId(0), "zebra".to_string());
    grammar.fields.insert(FieldId(1), "alpha".to_string());
    grammar.fields.insert(FieldId(2), "beta".to_string());

    let json = serde_json::to_string(&grammar).expect("serialize");
    let deserialized: Grammar = serde_json::from_str(&json).expect("deserialize");
    let roundtrip_json = serde_json::to_string(&deserialized).expect("re-serialize");

    // The JSON should be identical, proving field order is preserved
    assert_eq!(json, roundtrip_json);

    // Also verify the order is maintained
    let field_names: Vec<String> = grammar.fields.values().cloned().collect();
    assert_eq!(field_names, vec!["zebra", "alpha", "beta"]);

    let deserialized_field_names: Vec<String> = deserialized.fields.values().cloned().collect();
    assert_eq!(deserialized_field_names, vec!["zebra", "alpha", "beta"]);
}

#[test]
fn test_grammar_indexmap_order_preservation_json_roundtrip() {
    let mut grammar = Grammar::new("indexmap_order".to_string());

    // Insert rules in specific order
    let rules_order = vec!["rule_z", "rule_a", "rule_m"];
    for (idx, name) in rules_order.iter().enumerate() {
        let rule_id = SymbolId(idx as u16);
        grammar.rule_names.insert(rule_id, name.to_string());

        grammar.add_rule(Rule {
            lhs: rule_id,
            rhs: vec![Symbol::Epsilon],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(idx as u16),
        });
    }

    let json = serde_json::to_string(&grammar).expect("serialize");
    let deserialized: Grammar = serde_json::from_str(&json).expect("deserialize");

    // Verify rule_names order is preserved
    let names_after: Vec<String> = deserialized.rule_names.values().cloned().collect();
    assert_eq!(names_after, rules_order);
}

#[test]
fn test_token_pattern_string_roundtrip() {
    let token = Token {
        name: "KEYWORD".to_string(),
        pattern: TokenPattern::String("hello".to_string()),
        fragile: false,
    };

    let json = serde_json::to_string(&token).expect("serialize");
    let deserialized: Token = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(token, deserialized);
}

#[test]
fn test_token_pattern_regex_roundtrip() {
    let token = Token {
        name: "NUMBER".to_string(),
        pattern: TokenPattern::Regex(r"\d+(\.\d+)?".to_string()),
        fragile: true,
    };

    let json = serde_json::to_string(&token).expect("serialize");
    let deserialized: Token = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(token, deserialized);
}

#[test]
fn test_rule_with_all_fields_json_roundtrip() {
    let rule = Rule {
        lhs: SymbolId(100),
        rhs: vec![
            Symbol::Terminal(SymbolId(1)),
            Symbol::NonTerminal(SymbolId(2)),
            Symbol::Optional(Box::new(Symbol::Terminal(SymbolId(3)))),
        ],
        precedence: Some(PrecedenceKind::Dynamic(5)),
        associativity: Some(Associativity::Right),
        fields: vec![(FieldId(0), 0), (FieldId(1), 2)],
        production_id: ProductionId(42),
    };

    let json = serde_json::to_string(&rule).expect("serialize");
    let deserialized: Rule = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(rule, deserialized);
}

#[test]
fn test_precedence_kind_static_json_roundtrip() {
    let kind = PrecedenceKind::Static(42);
    let json = serde_json::to_string(&kind).expect("serialize");
    let deserialized: PrecedenceKind = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(kind, deserialized);
}

#[test]
fn test_precedence_kind_dynamic_json_roundtrip() {
    let kind = PrecedenceKind::Dynamic(-15);
    let json = serde_json::to_string(&kind).expect("serialize");
    let deserialized: PrecedenceKind = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(kind, deserialized);
}

#[test]
fn test_precedence_kind_static_bincode_roundtrip() {
    let kind = PrecedenceKind::Static(0);
    let bytes = postcard::to_allocvec(&kind).expect("serialize");
    let deserialized: PrecedenceKind = postcard::from_bytes(&bytes).expect("deserialize");
    assert_eq!(kind, deserialized);
}

#[test]
fn test_precedence_kind_dynamic_bincode_roundtrip() {
    let kind = PrecedenceKind::Dynamic(100);
    let bytes = postcard::to_allocvec(&kind).expect("serialize");
    let deserialized: PrecedenceKind = postcard::from_bytes(&bytes).expect("deserialize");
    assert_eq!(kind, deserialized);
}

#[test]
fn test_associativity_left_json_roundtrip() {
    let assoc = Associativity::Left;
    let json = serde_json::to_string(&assoc).expect("serialize");
    let deserialized: Associativity = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(assoc, deserialized);
}

#[test]
fn test_associativity_right_json_roundtrip() {
    let assoc = Associativity::Right;
    let json = serde_json::to_string(&assoc).expect("serialize");
    let deserialized: Associativity = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(assoc, deserialized);
}

#[test]
fn test_associativity_none_json_roundtrip() {
    let assoc = Associativity::None;
    let json = serde_json::to_string(&assoc).expect("serialize");
    let deserialized: Associativity = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(assoc, deserialized);
}

#[test]
fn test_associativity_all_variants_bincode_roundtrip() {
    let variants = vec![
        Associativity::Left,
        Associativity::Right,
        Associativity::None,
    ];

    for assoc in variants {
        let bytes = postcard::to_allocvec(&assoc).expect("serialize");
        let deserialized: Associativity = postcard::from_bytes(&bytes).expect("deserialize");
        assert_eq!(assoc, deserialized);
    }
}

#[test]
fn test_empty_grammar_comprehensive_roundtrip() {
    let grammar = Grammar::default();

    // JSON roundtrip
    let json = serde_json::to_string(&grammar).expect("serialize");
    let from_json: Grammar = serde_json::from_str(&json).expect("deserialize");
    let json_roundtrip = serde_json::to_string(&from_json).expect("re-serialize");
    assert_eq!(json, json_roundtrip);

    // Bincode roundtrip
    let bytes = postcard::to_allocvec(&grammar).expect("serialize");
    let from_bytes: Grammar = postcard::from_bytes(&bytes).expect("deserialize");
    let bytes_roundtrip = postcard::to_allocvec(&from_bytes).expect("re-serialize");
    assert_eq!(bytes, bytes_roundtrip);
}

#[test]
fn test_grammar_with_multiple_rules_per_lhs_json_roundtrip() {
    let mut grammar = Grammar::new("multi_rules".to_string());

    let expr_id = SymbolId(10);
    let num_id = SymbolId(1);
    let plus_id = SymbolId(2);

    grammar.rule_names.insert(expr_id, "expression".to_string());
    grammar.rule_names.insert(num_id, "NUMBER".to_string());
    grammar.rule_names.insert(plus_id, "PLUS".to_string());

    grammar.tokens.insert(
        num_id,
        Token {
            name: "NUMBER".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        plus_id,
        Token {
            name: "PLUS".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );

    // Add multiple rules for the same LHS
    grammar.add_rule(Rule {
        lhs: expr_id,
        rhs: vec![Symbol::Terminal(num_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    grammar.add_rule(Rule {
        lhs: expr_id,
        rhs: vec![
            Symbol::NonTerminal(expr_id),
            Symbol::Terminal(plus_id),
            Symbol::NonTerminal(expr_id),
        ],
        precedence: Some(PrecedenceKind::Static(1)),
        associativity: Some(Associativity::Left),
        fields: vec![],
        production_id: ProductionId(1),
    });

    assert_json_roundtrip(&grammar, "grammar_multi_rules_per_lhs");
}

#[test]
fn test_grammar_with_max_id_values_json_roundtrip() {
    let mut grammar = Grammar::new("max_ids".to_string());

    // Use high ID values (close to u16 max)
    let id1 = SymbolId(65534);
    let id2 = SymbolId(65533);

    grammar.rule_names.insert(id1, "high_id_1".to_string());
    grammar.rule_names.insert(id2, "high_id_2".to_string());

    grammar.add_rule(Rule {
        lhs: id1,
        rhs: vec![Symbol::NonTerminal(id2)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(65535),
    });

    assert_json_roundtrip(&grammar, "grammar_with_max_ids");
}

#[test]
fn test_complex_conflict_scenarios_json_roundtrip() {
    let mut grammar = Grammar::new("complex_conflicts".to_string());

    let op1 = SymbolId(1);
    let op2 = SymbolId(2);
    let op3 = SymbolId(3);

    grammar.tokens.insert(
        op1,
        Token {
            name: "OP1".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        op2,
        Token {
            name: "OP2".to_string(),
            pattern: TokenPattern::String("*".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        op3,
        Token {
            name: "OP3".to_string(),
            pattern: TokenPattern::String("-".to_string()),
            fragile: false,
        },
    );

    // Multiple conflict declarations
    grammar.conflicts.push(ConflictDeclaration {
        symbols: vec![op1, op2],
        resolution: ConflictResolution::GLR,
    });

    grammar.conflicts.push(ConflictDeclaration {
        symbols: vec![op2, op3],
        resolution: ConflictResolution::Precedence(PrecedenceKind::Static(1)),
    });

    grammar.conflicts.push(ConflictDeclaration {
        symbols: vec![op1, op3],
        resolution: ConflictResolution::Associativity(Associativity::Right),
    });

    grammar.rule_names.insert(op1, "OP1".to_string());
    grammar.rule_names.insert(op2, "OP2".to_string());
    grammar.rule_names.insert(op3, "OP3".to_string());

    assert_json_roundtrip(&grammar, "grammar_complex_conflicts");
}

#[test]
fn test_all_precedence_levels_json_roundtrip() {
    let mut grammar = Grammar::new("all_precedences".to_string());

    // Create precedence declarations with different levels
    for level in -100..=100i16 {
        let id = SymbolId((level + 100) as u16);
        grammar.rule_names.insert(id, format!("op_level_{}", level));

        grammar.precedences.push(Precedence {
            level,
            associativity: if level % 3 == 0 {
                Associativity::Left
            } else if level % 3 == 1 {
                Associativity::Right
            } else {
                Associativity::None
            },
            symbols: vec![id],
        });
    }

    assert_json_roundtrip(&grammar, "all_precedence_levels");
}

#[test]
fn test_externals_with_various_names_json_roundtrip() {
    let mut grammar = Grammar::new("external_names".to_string());

    let names = ["INDENT", "DEDENT", "NEWLINE", "newline", "comment_token"];

    for (idx, name) in names.iter().enumerate() {
        let id = SymbolId(idx as u16);
        grammar.externals.push(ExternalToken {
            name: name.to_string(),
            symbol_id: id,
        });
        grammar.rule_names.insert(id, name.to_string());
    }

    assert_json_roundtrip(&grammar, "externals_various_names");
}

#[test]
fn test_very_long_alias_sequence_json_roundtrip() {
    let mut grammar = Grammar::new("long_aliases".to_string());

    // Create an alias sequence with many entries
    let mut aliases = vec![];
    for i in 0..50 {
        if i % 3 == 0 {
            aliases.push(None);
        } else {
            aliases.push(Some(format!("alias_{}", i)));
        }
    }

    grammar
        .alias_sequences
        .insert(ProductionId(0), AliasSequence { aliases });

    assert_json_roundtrip(&grammar, "long_alias_sequence");
}

#[test]
fn test_mixed_token_patterns_json_roundtrip() {
    let mut grammar = Grammar::new("mixed_patterns".to_string());

    let patterns = vec![
        ("STRING", TokenPattern::Regex(r#""[^"]*""#.to_string())),
        ("NUMBER", TokenPattern::Regex(r"\d+".to_string())),
        ("PLUS", TokenPattern::String("+".to_string())),
        ("STAR", TokenPattern::String("*".to_string())),
        ("REGEX_SIMPLE", TokenPattern::Regex(r"[a-z]+".to_string())),
    ];

    for (idx, (name, pattern)) in patterns.into_iter().enumerate() {
        let id = SymbolId(idx as u16);
        grammar.tokens.insert(
            id,
            Token {
                name: name.to_string(),
                pattern,
                fragile: idx % 2 == 0,
            },
        );
        grammar.rule_names.insert(id, name.to_string());
    }

    assert_json_roundtrip(&grammar, "mixed_token_patterns");
}

// ============================================================================
// Bincode-specific tests
// ============================================================================

#[test]
fn test_grammar_with_many_rules_bincode_roundtrip() {
    let mut grammar = Grammar::new("many_rules_bincode".to_string());

    // Create many rules for comprehensive bincode coverage
    for i in 0..50 {
        let rule_id = SymbolId(i as u16);
        grammar.rule_names.insert(rule_id, format!("rule_{}", i));

        grammar.add_rule(Rule {
            lhs: rule_id,
            rhs: vec![
                Symbol::Terminal(SymbolId((i + 1) as u16)),
                Symbol::NonTerminal(SymbolId((i + 2) as u16)),
            ],
            precedence: if i % 2 == 0 {
                Some(PrecedenceKind::Static(i as i16))
            } else {
                Some(PrecedenceKind::Dynamic(-(i as i16)))
            },
            associativity: if i % 3 == 0 {
                Some(Associativity::Left)
            } else if i % 3 == 1 {
                Some(Associativity::Right)
            } else {
                Some(Associativity::None)
            },
            fields: vec![(FieldId((i % 5) as u16), i % 3)],
            production_id: ProductionId(i as u16),
        });
    }

    assert_bincode_roundtrip(&grammar, "many_rules_bincode");
}

#[test]
fn test_all_symbol_types_bincode_roundtrip() {
    let symbols = vec![
        Symbol::Terminal(SymbolId(1)),
        Symbol::NonTerminal(SymbolId(2)),
        Symbol::External(SymbolId(3)),
        Symbol::Optional(Box::new(Symbol::Terminal(SymbolId(4)))),
        Symbol::Repeat(Box::new(Symbol::NonTerminal(SymbolId(5)))),
        Symbol::RepeatOne(Box::new(Symbol::External(SymbolId(6)))),
        Symbol::Choice(vec![
            Symbol::Terminal(SymbolId(7)),
            Symbol::Terminal(SymbolId(8)),
        ]),
        Symbol::Sequence(vec![
            Symbol::NonTerminal(SymbolId(9)),
            Symbol::Terminal(SymbolId(10)),
        ]),
        Symbol::Epsilon,
    ];

    for (idx, symbol) in symbols.into_iter().enumerate() {
        let bytes = postcard::to_allocvec(&symbol).unwrap_or_else(|_| panic!("serialize {}", idx));
        let deserialized: Symbol =
            postcard::from_bytes(&bytes).unwrap_or_else(|_| panic!("deserialize {}", idx));
        assert_eq!(symbol, deserialized, "Symbol roundtrip failed for {}", idx);
    }
}

#[test]
fn test_json_pretty_printing_roundtrip() {
    let mut grammar = Grammar::new("pretty_print".to_string());

    let id = SymbolId(1);
    grammar.rule_names.insert(id, "test_rule".to_string());
    grammar.tokens.insert(
        id,
        Token {
            name: "TEST".to_string(),
            pattern: TokenPattern::String("test".to_string()),
            fragile: false,
        },
    );

    // Serialize with pretty printing
    let pretty_json = serde_json::to_string_pretty(&grammar).expect("pretty serialize");
    let deserialized: Grammar = serde_json::from_str(&pretty_json).expect("deserialize");

    // Roundtrip should produce identical output
    let roundtrip_json = serde_json::to_string_pretty(&deserialized).expect("re-serialize");
    assert_eq!(pretty_json, roundtrip_json);
}

#[test]
fn test_grammar_with_all_optional_fields_none_json_roundtrip() {
    let mut grammar = Grammar::new("all_none".to_string());

    let rule_id = SymbolId(1);
    grammar.rule_names.insert(rule_id, "rule".to_string());

    // Add rule with no optional fields set
    grammar.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::Terminal(SymbolId(2))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    assert_json_roundtrip(&grammar, "all_optional_fields_none");
}

#[test]
fn test_grammar_with_all_optional_fields_some_json_roundtrip() {
    let mut grammar = Grammar::new("all_some".to_string());

    let rule_id = SymbolId(1);
    grammar.rule_names.insert(rule_id, "rule".to_string());

    // Add rule with all optional fields set
    grammar.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![
            Symbol::Terminal(SymbolId(2)),
            Symbol::NonTerminal(SymbolId(3)),
        ],
        precedence: Some(PrecedenceKind::Dynamic(42)),
        associativity: Some(Associativity::Left),
        fields: vec![(FieldId(0), 0), (FieldId(1), 1)],
        production_id: ProductionId(0),
    });

    grammar.fields.insert(FieldId(0), "field_0".to_string());
    grammar.fields.insert(FieldId(1), "field_1".to_string());

    assert_json_roundtrip(&grammar, "all_optional_fields_some");
}

#[test]
fn test_grammar_serialized_to_canonical_form_json() {
    let mut grammar = Grammar::new("canonical".to_string());

    let id = SymbolId(1);
    grammar.rule_names.insert(id, "rule".to_string());

    let json1 = serde_json::to_string(&grammar).expect("serialize 1");
    let deserialized: Grammar = serde_json::from_str(&json1).expect("deserialize");
    let json2 = serde_json::to_string(&deserialized).expect("serialize 2");

    // Both should produce identical JSON, proving canonical form
    assert_eq!(json1, json2, "Canonical form test failed");
}

// ============================================================================
// Tests: StateId roundtrip (missing from original coverage)
// ============================================================================

#[test]
fn test_state_id_json_roundtrip() {
    let id = StateId(456);
    let json = serde_json::to_string(&id).expect("serialize");
    let deserialized: StateId = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(id, deserialized);
}

#[test]
fn test_state_id_zero_json_roundtrip() {
    let id = StateId(0);
    let json = serde_json::to_string(&id).expect("serialize");
    let deserialized: StateId = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(id, deserialized);
}

#[test]
fn test_state_id_max_json_roundtrip() {
    let id = StateId(u16::MAX);
    let json = serde_json::to_string(&id).expect("serialize");
    let deserialized: StateId = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(id, deserialized);
}

#[test]
fn test_state_id_bincode_roundtrip() {
    let id = StateId(1024);
    let bytes = postcard::to_allocvec(&id).expect("serialize");
    let deserialized: StateId = postcard::from_bytes(&bytes).expect("deserialize");
    assert_eq!(id, deserialized);
}

// ============================================================================
// Tests: All ID types at boundary values
// ============================================================================

#[test]
fn test_symbol_id_zero_json_roundtrip() {
    let id = SymbolId(0);
    let json = serde_json::to_string(&id).expect("serialize");
    let deserialized: SymbolId = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(id, deserialized);
}

#[test]
fn test_symbol_id_max_json_roundtrip() {
    let id = SymbolId(u16::MAX);
    let json = serde_json::to_string(&id).expect("serialize");
    let deserialized: SymbolId = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(id, deserialized);
}

#[test]
fn test_rule_id_max_json_roundtrip() {
    let id = RuleId(u16::MAX);
    let json = serde_json::to_string(&id).expect("serialize");
    let deserialized: RuleId = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(id, deserialized);
}

#[test]
fn test_field_id_max_json_roundtrip() {
    let id = FieldId(u16::MAX);
    let json = serde_json::to_string(&id).expect("serialize");
    let deserialized: FieldId = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(id, deserialized);
}

#[test]
fn test_production_id_max_json_roundtrip() {
    let id = ProductionId(u16::MAX);
    let json = serde_json::to_string(&id).expect("serialize");
    let deserialized: ProductionId = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(id, deserialized);
}

// ============================================================================
// Tests: Grammar via GrammarBuilder
// ============================================================================

#[test]
fn test_grammar_builder_simple_json_roundtrip() {
    let grammar = GrammarBuilder::new("simple")
        .token("NUMBER", r"\d+")
        .rule("expr", vec!["NUMBER"])
        .start("expr")
        .build();

    assert_json_roundtrip(&grammar, "grammar_builder_simple");
}

#[test]
fn test_grammar_builder_arithmetic_json_roundtrip() {
    let grammar = GrammarBuilder::new("arithmetic")
        .token("NUMBER", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .rule("expr", vec!["expr", "+", "expr"])
        .rule("expr", vec!["expr", "*", "expr"])
        .rule("expr", vec!["NUMBER"])
        .start("expr")
        .build();

    assert_json_roundtrip(&grammar, "grammar_builder_arithmetic");
}

#[test]
fn test_grammar_builder_with_precedence_json_roundtrip() {
    let grammar = GrammarBuilder::new("prec_grammar")
        .token("NUMBER", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["NUMBER"])
        .start("expr")
        .build();

    assert_json_roundtrip(&grammar, "grammar_builder_with_precedence");
}

#[test]
fn test_grammar_builder_with_extras_json_roundtrip() {
    let grammar = GrammarBuilder::new("extras_grammar")
        .token("NUMBER", r"\d+")
        .token("WS", r"\s+")
        .rule("expr", vec!["NUMBER"])
        .extra("WS")
        .start("expr")
        .build();

    assert_json_roundtrip(&grammar, "grammar_builder_with_extras");
}

#[test]
fn test_grammar_builder_with_externals_json_roundtrip() {
    let grammar = GrammarBuilder::new("ext_grammar")
        .token("NUMBER", r"\d+")
        .rule("expr", vec!["NUMBER"])
        .external("INDENT")
        .external("DEDENT")
        .start("expr")
        .build();

    assert_json_roundtrip(&grammar, "grammar_builder_with_externals");
}

#[test]
fn test_grammar_builder_empty_json_roundtrip() {
    let grammar = GrammarBuilder::new("empty_builder").build();
    assert_json_roundtrip(&grammar, "grammar_builder_empty");
}

#[test]
fn test_grammar_builder_with_precedence_decl_json_roundtrip() {
    let grammar = GrammarBuilder::new("prec_decl")
        .token("+", "+")
        .token("*", "*")
        .token("NUMBER", r"\d+")
        .precedence(1, Associativity::Left, vec!["+"])
        .precedence(2, Associativity::Left, vec!["*"])
        .rule("expr", vec!["NUMBER"])
        .start("expr")
        .build();

    assert_json_roundtrip(&grammar, "grammar_builder_precedence_decl");
}

#[test]
fn test_grammar_builder_fragile_token_json_roundtrip() {
    let grammar = GrammarBuilder::new("fragile")
        .token("NUMBER", r"\d+")
        .fragile_token("ERROR_TOKEN", r".")
        .rule("expr", vec!["NUMBER"])
        .start("expr")
        .build();

    assert_json_roundtrip(&grammar, "grammar_builder_fragile_token");
}

#[test]
fn test_grammar_builder_bincode_roundtrip() {
    let grammar = GrammarBuilder::new("bincode_builder")
        .token("NUMBER", r"\d+")
        .token("+", "+")
        .rule("expr", vec!["expr", "+", "expr"])
        .rule("expr", vec!["NUMBER"])
        .start("expr")
        .build();

    assert_bincode_roundtrip(&grammar, "grammar_builder_bincode");
}

// ============================================================================
// Tests: Special characters in names
// ============================================================================

#[test]
fn test_grammar_special_chars_in_token_names_json_roundtrip() {
    let mut grammar = Grammar::new("special_chars".to_string());

    let names_and_patterns = vec![
        (SymbolId(1), "tab\there", "tab\there"),
        (SymbolId(2), "quote\"inside", "quote\"inside"),
        (SymbolId(3), "back\\slash", "back\\slash"),
        (SymbolId(4), "newline\nin_name", "newline\nin_name"),
    ];

    for (id, name, pattern) in names_and_patterns {
        grammar.tokens.insert(
            id,
            Token {
                name: name.to_string(),
                pattern: TokenPattern::String(pattern.to_string()),
                fragile: false,
            },
        );
        grammar.rule_names.insert(id, name.to_string());
    }

    assert_json_roundtrip(&grammar, "special_chars_in_names");
}

#[test]
fn test_grammar_emoji_in_names_json_roundtrip() {
    let mut grammar = Grammar::new("emoji_grammar".to_string());

    let id = SymbolId(1);
    grammar.tokens.insert(
        id,
        Token {
            name: "🔥fire🔥".to_string(),
            pattern: TokenPattern::String("🔥".to_string()),
            fragile: false,
        },
    );
    grammar.rule_names.insert(id, "🔥fire🔥".to_string());

    assert_json_roundtrip(&grammar, "emoji_in_names");
}

#[test]
fn test_grammar_empty_string_name_json_roundtrip() {
    let mut grammar = Grammar::new("".to_string());

    let id = SymbolId(1);
    grammar.rule_names.insert(id, "".to_string());

    assert_json_roundtrip(&grammar, "empty_name");
}

// ============================================================================
// Tests: SymbolMetadata roundtrip
// ============================================================================

#[test]
fn test_symbol_metadata_json_roundtrip() {
    let metadata = SymbolMetadata {
        visible: true,
        named: false,
        hidden: true,
        terminal: false,
    };
    let json = serde_json::to_string(&metadata).expect("serialize");
    let deserialized: SymbolMetadata = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(metadata, deserialized);
}

#[test]
fn test_symbol_metadata_all_true_json_roundtrip() {
    let metadata = SymbolMetadata {
        visible: true,
        named: true,
        hidden: true,
        terminal: true,
    };
    let json = serde_json::to_string(&metadata).expect("serialize");
    let deserialized: SymbolMetadata = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(metadata, deserialized);
}

// ============================================================================
// Tests: ConflictResolution variants roundtrip
// ============================================================================

#[test]
fn test_conflict_resolution_glr_json_roundtrip() {
    let resolution = ConflictResolution::GLR;
    let json = serde_json::to_string(&resolution).expect("serialize");
    let deserialized: ConflictResolution = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(resolution, deserialized);
}

#[test]
fn test_conflict_resolution_dynamic_prec_json_roundtrip() {
    let resolution = ConflictResolution::Precedence(PrecedenceKind::Dynamic(-5));
    let json = serde_json::to_string(&resolution).expect("serialize");
    let deserialized: ConflictResolution = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(resolution, deserialized);
}

// ============================================================================
// Tests: Negative and extreme precedence values
// ============================================================================

#[test]
fn test_precedence_kind_min_max_i16_json_roundtrip() {
    for val in [i16::MIN, -1, 0, 1, i16::MAX] {
        let static_kind = PrecedenceKind::Static(val);
        let json = serde_json::to_string(&static_kind).expect("serialize");
        let deserialized: PrecedenceKind = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(static_kind, deserialized, "Failed for Static({})", val);

        let dynamic_kind = PrecedenceKind::Dynamic(val);
        let json = serde_json::to_string(&dynamic_kind).expect("serialize");
        let deserialized: PrecedenceKind = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(dynamic_kind, deserialized, "Failed for Dynamic({})", val);
    }
}

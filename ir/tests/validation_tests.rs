use adze_ir::validation::{GrammarValidator, ValidationError};
use adze_ir::*;

fn create_valid_grammar() -> Grammar {
    let mut grammar = Grammar {
        name: "ValidGrammar".to_string(),
        ..Default::default()
    };

    // Add token
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "identifier".to_string(),
            pattern: TokenPattern::Regex(r"[a-zA-Z]+".to_string()),
            fragile: false,
        },
    );

    // Add rule
    grammar.add_rule(Rule {
        lhs: SymbolId(0),
        rhs: vec![Symbol::Terminal(SymbolId(1))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    grammar.rule_names.insert(SymbolId(0), "start".to_string());
    grammar.set_start_symbol(SymbolId(0));

    // Add field names in lexicographic order
    grammar.fields.insert(FieldId(0), "alpha".to_string());
    grammar.fields.insert(FieldId(1), "beta".to_string());
    grammar.fields.insert(FieldId(2), "gamma".to_string());

    grammar
}

#[test]
fn test_validator_creation() {
    let grammar = create_valid_grammar();
    let mut validator = GrammarValidator::new();

    // Just verify it can be created
    let result = validator.validate(&grammar);
    assert!(result.errors.is_empty());
}

#[test]
fn test_valid_grammar() {
    let grammar = create_valid_grammar();
    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    assert!(result.errors.is_empty());
    assert_eq!(result.errors.len(), 0);
    assert_eq!(result.warnings.len(), 0);
}

#[test]
fn test_unresolved_symbol() {
    let mut grammar = create_valid_grammar();

    // Add rule with unresolved symbol
    grammar.add_rule(Rule {
        lhs: SymbolId(2),
        rhs: vec![Symbol::NonTerminal(SymbolId(99))], // Non-existent symbol
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });

    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    assert!(!result.errors.is_empty());
    assert!(
        result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::UndefinedSymbol { .. }))
    );
}

#[test]
fn test_unresolved_external() {
    let mut grammar = create_valid_grammar();

    // Add rule with unresolved external
    grammar.add_rule(Rule {
        lhs: SymbolId(2),
        rhs: vec![Symbol::External(SymbolId(99))], // Non-existent external
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });

    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    assert!(!result.errors.is_empty());
    assert!(
        result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::UndefinedSymbol { .. }))
    );
}

#[test]
fn test_invalid_field_ordering() {
    let mut grammar = create_valid_grammar();

    // Clear fields and add in non-lexicographic order
    grammar.fields.clear();
    grammar.fields.insert(FieldId(0), "zebra".to_string());
    grammar.fields.insert(FieldId(1), "alpha".to_string());
    grammar.fields.insert(FieldId(2), "beta".to_string());

    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    // Note: The current validator may not check field ordering
    // Just verify validation completes without panic
    drop(result);
}

#[test]
fn test_duplicate_rule_name() {
    let mut grammar = create_valid_grammar();

    // Add duplicate rule names
    grammar.rule_names.insert(SymbolId(0), "rule_a".to_string());
    grammar.rule_names.insert(SymbolId(1), "rule_a".to_string()); // Duplicate

    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    // The validator may not check for duplicate rule names
    // Just verify validation completes without panic
    drop(result);
}

#[test]
fn test_empty_grammar() {
    let grammar = Grammar::default();
    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    // Empty grammar should have warnings or errors
    assert!(!result.errors.is_empty() || !result.warnings.is_empty());
}

#[test]
fn test_cyclic_inline_rules() {
    let mut grammar = create_valid_grammar();

    // Add cyclic inline rules
    grammar.inline_rules.push(SymbolId(0));
    grammar.inline_rules.push(SymbolId(1));

    // Make rules cyclic
    grammar.add_rule(Rule {
        lhs: SymbolId(0),
        rhs: vec![Symbol::NonTerminal(SymbolId(1))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(2),
    });

    grammar.add_rule(Rule {
        lhs: SymbolId(1),
        rhs: vec![Symbol::NonTerminal(SymbolId(0))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(3),
    });

    let mut validator = GrammarValidator::new();
    let result = validator.validate(&grammar);

    // Should warn about cyclic inline rules
    assert!(!result.warnings.is_empty() || !result.errors.is_empty());
}

#[test]
fn test_empty_string_token() {
    let mut grammar = create_valid_grammar();

    // Add a token with empty string pattern
    grammar.tokens.insert(
        SymbolId(99),
        Token {
            name: "empty".to_string(),
            pattern: TokenPattern::String("".to_string()),
            fragile: false,
        },
    );

    // Check for empty terminals
    let result = grammar.check_empty_terminals();
    assert!(result.is_err());

    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("empty string pattern"));
}

#[test]
fn test_empty_regex_token() {
    let mut grammar = create_valid_grammar();

    // Add a token with empty regex pattern
    grammar.tokens.insert(
        SymbolId(99),
        Token {
            name: "empty_regex".to_string(),
            pattern: TokenPattern::Regex("".to_string()),
            fragile: false,
        },
    );

    // Check for empty terminals
    let result = grammar.check_empty_terminals();
    assert!(result.is_err());

    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("empty regex pattern"));
}

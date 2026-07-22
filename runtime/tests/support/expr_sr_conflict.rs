//! Expression grammar with shift/reduce conflicts for testing conflict resolution
//! This grammar intentionally creates SR conflicts to test the resolution policy

use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

/// Build an expression grammar with shift/reduce conflicts (no precedence)
#[allow(dead_code)]
pub fn build_expr_sr_conflict() -> Grammar {
    let mut grammar = Grammar::new("expr_conflict".to_string());

    // Terminals
    let plus_id = SymbolId(1);
    let times_id = SymbolId(2);
    let lparen_id = SymbolId(3);
    let rparen_id = SymbolId(4);
    let id_id = SymbolId(5);

    grammar.tokens.insert(
        plus_id,
        Token {
            name: "PLUS".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        times_id,
        Token {
            name: "TIMES".to_string(),
            pattern: TokenPattern::String("*".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        lparen_id,
        Token {
            name: "LPAREN".to_string(),
            pattern: TokenPattern::String("(".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        rparen_id,
        Token {
            name: "RPAREN".to_string(),
            pattern: TokenPattern::String(")".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        id_id,
        Token {
            name: "ID".to_string(),
            pattern: TokenPattern::Regex(r"[a-z]+".to_string()),
            fragile: false,
        },
    );

    // Non-terminal
    let expr_id = SymbolId(100);

    // Rules that create shift/reduce conflicts:
    // E -> E + E  (creates SR conflict with itself)
    // E -> E * E  (creates SR conflict with itself and with +)
    // E -> ID
    // E -> ( E )

    // Rule 0: E -> E + E (no precedence/associativity)
    grammar.add_rule(Rule {
        lhs: expr_id,
        rhs: vec![
            Symbol::NonTerminal(expr_id),
            Symbol::Terminal(plus_id),
            Symbol::NonTerminal(expr_id),
        ],
        precedence: None, // No precedence - will create conflict
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    // Rule 1: E -> E * E (no precedence/associativity)
    grammar.add_rule(Rule {
        lhs: expr_id,
        rhs: vec![
            Symbol::NonTerminal(expr_id),
            Symbol::Terminal(times_id),
            Symbol::NonTerminal(expr_id),
        ],
        precedence: None, // No precedence - will create conflict
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });

    // Rule 2: E -> ID
    grammar.add_rule(Rule {
        lhs: expr_id,
        rhs: vec![Symbol::Terminal(id_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(2),
    });

    // Rule 3: E -> ( E )
    grammar.add_rule(Rule {
        lhs: expr_id,
        rhs: vec![
            Symbol::Terminal(lparen_id),
            Symbol::NonTerminal(expr_id),
            Symbol::Terminal(rparen_id),
        ],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(3),
    });

    grammar.set_start_symbol(expr_id);
    grammar
}

/// Build the same grammar WITH precedence to compare
#[allow(dead_code)]
pub fn build_expr_with_precedence() -> Grammar {
    let mut grammar = Grammar::new("expr_precedence".to_string());

    // Terminals
    let plus_id = SymbolId(1);
    let times_id = SymbolId(2);
    let lparen_id = SymbolId(3);
    let rparen_id = SymbolId(4);
    let id_id = SymbolId(5);

    grammar.tokens.insert(
        plus_id,
        Token {
            name: "PLUS".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        times_id,
        Token {
            name: "TIMES".to_string(),
            pattern: TokenPattern::String("*".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        lparen_id,
        Token {
            name: "LPAREN".to_string(),
            pattern: TokenPattern::String("(".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        rparen_id,
        Token {
            name: "RPAREN".to_string(),
            pattern: TokenPattern::String(")".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        id_id,
        Token {
            name: "ID".to_string(),
            pattern: TokenPattern::Regex(r"[a-z]+".to_string()),
            fragile: false,
        },
    );

    // Non-terminal
    let expr_id = SymbolId(100);

    // Rules with precedence to resolve conflicts:

    // Rule 0: E -> E + E (precedence 1, left associative)
    grammar.add_rule(Rule {
        lhs: expr_id,
        rhs: vec![
            Symbol::NonTerminal(expr_id),
            Symbol::Terminal(plus_id),
            Symbol::NonTerminal(expr_id),
        ],
        precedence: Some(adze_ir::PrecedenceKind::Static(1)),
        associativity: Some(adze_ir::Associativity::Left),
        fields: vec![],
        production_id: ProductionId(0),
    });

    // Rule 1: E -> E * E (precedence 2, left associative - higher than +)
    grammar.add_rule(Rule {
        lhs: expr_id,
        rhs: vec![
            Symbol::NonTerminal(expr_id),
            Symbol::Terminal(times_id),
            Symbol::NonTerminal(expr_id),
        ],
        precedence: Some(adze_ir::PrecedenceKind::Static(2)),
        associativity: Some(adze_ir::Associativity::Left),
        fields: vec![],
        production_id: ProductionId(1),
    });

    // Rule 2: E -> ID
    grammar.add_rule(Rule {
        lhs: expr_id,
        rhs: vec![Symbol::Terminal(id_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(2),
    });

    // Rule 3: E -> ( E )
    grammar.add_rule(Rule {
        lhs: expr_id,
        rhs: vec![
            Symbol::Terminal(lparen_id),
            Symbol::NonTerminal(expr_id),
            Symbol::Terminal(rparen_id),
        ],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(3),
    });

    grammar.set_start_symbol(expr_id);
    grammar
}

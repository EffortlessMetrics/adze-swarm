// Test GLR conflict resolution with classic ambiguous grammars

use adze::glr_parser::GLRParser;
use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::{
    Associativity, Grammar, Precedence, PrecedenceKind, ProductionId, Rule, Symbol, SymbolId,
    Token, TokenPattern,
};

/// Build a simple arithmetic grammar with operator precedence
fn build_arithmetic_grammar() -> Grammar {
    let mut grammar = Grammar::new("arithmetic".to_string());

    // Tokens (start from 1, as 0 is reserved for EOF)
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        SymbolId(2),
        Token {
            name: "plus".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        SymbolId(3),
        Token {
            name: "minus".to_string(),
            pattern: TokenPattern::String("-".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        SymbolId(4),
        Token {
            name: "times".to_string(),
            pattern: TokenPattern::String("*".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        SymbolId(5),
        Token {
            name: "divide".to_string(),
            pattern: TokenPattern::String("/".to_string()),
            fragile: false,
        },
    );

    // Non-terminals
    let expr_id = SymbolId(10);

    // Precedence declarations
    grammar.precedences.push(Precedence {
        level: 1,
        associativity: Associativity::Left,
        symbols: vec![SymbolId(2), SymbolId(3)], // + -
    });

    grammar.precedences.push(Precedence {
        level: 2,
        associativity: Associativity::Left,
        symbols: vec![SymbolId(4), SymbolId(5)], // * /
    });

    // Rules
    // expr -> number
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![Symbol::Terminal(SymbolId(1))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    // expr -> expr + expr
    grammar
        .rules
        .entry(expr_id) // Key should be the LHS non-terminal
        .or_default()
        .push(Rule {
            lhs: expr_id,
            rhs: vec![
                Symbol::NonTerminal(expr_id),
                Symbol::Terminal(SymbolId(2)),
                Symbol::NonTerminal(expr_id),
            ],
            precedence: Some(PrecedenceKind::Static(1)),
            associativity: Some(Associativity::Left),
            fields: vec![],
            production_id: ProductionId(1),
        });

    // expr -> expr - expr
    grammar
        .rules
        .entry(expr_id) // Key should be the LHS non-terminal
        .or_default()
        .push(Rule {
            lhs: expr_id,
            rhs: vec![
                Symbol::NonTerminal(expr_id),
                Symbol::Terminal(SymbolId(3)),
                Symbol::NonTerminal(expr_id),
            ],
            precedence: Some(PrecedenceKind::Static(1)),
            associativity: Some(Associativity::Left),
            fields: vec![],
            production_id: ProductionId(2),
        });

    // expr -> expr * expr
    grammar
        .rules
        .entry(expr_id) // Key should be the LHS non-terminal
        .or_default()
        .push(Rule {
            lhs: expr_id,
            rhs: vec![
                Symbol::NonTerminal(expr_id),
                Symbol::Terminal(SymbolId(4)),
                Symbol::NonTerminal(expr_id),
            ],
            precedence: Some(PrecedenceKind::Static(2)),
            associativity: Some(Associativity::Left),
            fields: vec![],
            production_id: ProductionId(3),
        });

    // expr -> expr / expr
    grammar
        .rules
        .entry(expr_id) // Key should be the LHS non-terminal
        .or_default()
        .push(Rule {
            lhs: expr_id,
            rhs: vec![
                Symbol::NonTerminal(expr_id),
                Symbol::Terminal(SymbolId(5)),
                Symbol::NonTerminal(expr_id),
            ],
            precedence: Some(PrecedenceKind::Static(2)),
            associativity: Some(Associativity::Left),
            fields: vec![],
            production_id: ProductionId(4),
        });

    grammar.set_start_symbol(expr_id);
    grammar
}

/// Build a grammar with the classic dangling-else ambiguity
fn build_dangling_else_grammar() -> Grammar {
    let mut grammar = Grammar::new("dangling_else".to_string());

    // Tokens (start from 1)
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "if".to_string(),
            pattern: TokenPattern::String("if".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        SymbolId(2),
        Token {
            name: "then".to_string(),
            pattern: TokenPattern::String("then".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        SymbolId(3),
        Token {
            name: "else".to_string(),
            pattern: TokenPattern::String("else".to_string()),
            fragile: true, // else is fragile in Tree-sitter
        },
    );

    grammar.tokens.insert(
        SymbolId(4),
        Token {
            name: "expr".to_string(),
            pattern: TokenPattern::String("e".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        SymbolId(5),
        Token {
            name: "stmt".to_string(),
            pattern: TokenPattern::String("s".to_string()),
            fragile: false,
        },
    );

    // Non-terminals
    let stmt_id = SymbolId(10);

    // Rules
    // stmt -> if expr then stmt
    grammar
        .rules
        .entry(stmt_id) // Key should be the LHS non-terminal
        .or_default()
        .push(Rule {
            lhs: stmt_id,
            rhs: vec![
                Symbol::Terminal(SymbolId(1)), // if
                Symbol::Terminal(SymbolId(4)), // expr
                Symbol::Terminal(SymbolId(2)), // then
                Symbol::NonTerminal(stmt_id),  // stmt
            ],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });

    // stmt -> if expr then stmt else stmt
    grammar
        .rules
        .entry(stmt_id) // Key should be the LHS non-terminal
        .or_default()
        .push(Rule {
            lhs: stmt_id,
            rhs: vec![
                Symbol::Terminal(SymbolId(1)), // if
                Symbol::Terminal(SymbolId(4)), // expr
                Symbol::Terminal(SymbolId(2)), // then
                Symbol::NonTerminal(stmt_id),  // stmt
                Symbol::Terminal(SymbolId(3)), // else
                Symbol::NonTerminal(stmt_id),  // stmt
            ],
            precedence: Some(PrecedenceKind::Static(1)), // Higher precedence to bind else to nearest if
            associativity: None,
            fields: vec![],
            production_id: ProductionId(1),
        });

    // stmt -> simple_stmt
    grammar
        .rules
        .entry(stmt_id) // Key should be the LHS non-terminal
        .or_default()
        .push(Rule {
            lhs: stmt_id,
            rhs: vec![Symbol::Terminal(SymbolId(5))], // s
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(2),
        });

    grammar.set_start_symbol(stmt_id);
    grammar
}

/// Build a grammar with dynamic precedence
fn build_dynamic_precedence_grammar() -> Grammar {
    let mut grammar = Grammar::new("dynamic".to_string());

    // Tokens (start from 1)
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "a".to_string(),
            pattern: TokenPattern::String("a".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        SymbolId(2),
        Token {
            name: "b".to_string(),
            pattern: TokenPattern::String("b".to_string()),
            fragile: false,
        },
    );

    // Non-terminals
    let s_id = SymbolId(10);

    // Rules with different dynamic precedences
    // S -> a (dynamic precedence 1)
    grammar
        .rules
        .entry(s_id) // Key should be the LHS non-terminal
        .or_default()
        .push(Rule {
            lhs: s_id,
            rhs: vec![Symbol::Terminal(SymbolId(1))],
            precedence: Some(PrecedenceKind::Dynamic(1)),
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });

    // S -> b (dynamic precedence 2)
    grammar
        .rules
        .entry(s_id) // Key should be the LHS non-terminal
        .or_default()
        .push(Rule {
            lhs: s_id,
            rhs: vec![Symbol::Terminal(SymbolId(2))],
            precedence: Some(PrecedenceKind::Dynamic(2)),
            associativity: None,
            fields: vec![],
            production_id: ProductionId(1),
        });

    grammar.set_start_symbol(s_id);
    grammar
}

#[test]
fn test_arithmetic_precedence() {
    let grammar = build_arithmetic_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let table = build_lr1_automaton(&grammar, &first_follow).unwrap();

    let mut parser = GLRParser::new(table, grammar);

    // Parse "2 + 3 * 4" - should parse as "2 + (3 * 4)"
    parser.process_token(SymbolId(1), "2", 0);
    parser.process_token(SymbolId(2), "+", 2);
    parser.process_token(SymbolId(1), "3", 4);
    parser.process_token(SymbolId(4), "*", 6);
    parser.process_token(SymbolId(1), "4", 8);
    parser.process_eof(10);

    let result = parser.get_best_parse();
    assert!(result.is_some());

    // In a full implementation, we'd verify the tree structure
}

#[test]
fn test_dangling_else() {
    let grammar = build_dangling_else_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let table = build_lr1_automaton(&grammar, &first_follow).unwrap();

    let mut parser = GLRParser::new(table, grammar);

    // Parse "if e then if e then s else s"
    // Should bind else to nearest if due to precedence
    parser.process_token(SymbolId(1), "if", 0);
    parser.process_token(SymbolId(4), "e", 3);
    parser.process_token(SymbolId(2), "then", 5);
    parser.process_token(SymbolId(1), "if", 10);
    parser.process_token(SymbolId(4), "e", 13);
    parser.process_token(SymbolId(2), "then", 15);
    parser.process_token(SymbolId(5), "s", 20);
    parser.process_token(SymbolId(3), "else", 22);
    parser.process_token(SymbolId(5), "s", 27);
    parser.process_eof(10);

    let result = parser.get_best_parse();
    assert!(result.is_some());
}

#[test]
fn test_dynamic_precedence() {
    let grammar = build_dynamic_precedence_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let table = build_lr1_automaton(&grammar, &first_follow).unwrap();

    let mut parser = GLRParser::new(table, grammar);

    // Both 'a' and 'b' are valid, but 'b' has higher dynamic precedence
    parser.process_token(SymbolId(2), "b", 0);
    parser.process_eof(10);

    let result = parser.get_best_parse();
    assert!(result.is_some());

    // The result should use the rule with higher dynamic precedence
    let tree = result.unwrap();
    assert_eq!(tree.dynamic_prec, 2);
}

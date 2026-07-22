use adze_ir::{
    Associativity, Grammar, PrecedenceKind, ProductionId, Rule, Symbol, SymbolId, Token,
    TokenPattern,
};

/// Build an expression grammar with left recursion
/// expr: expr '+' expr | expr '*' expr | '(' expr ')' | NUM
#[allow(dead_code)]
pub fn build_expr_grammar() -> Grammar {
    let mut g = Grammar::new("expr".to_string());

    // Terminals
    let eof = SymbolId(0);
    let num = SymbolId(1);
    let plus = SymbolId(2);
    let times = SymbolId(3);
    let lparen = SymbolId(4);
    let rparen = SymbolId(5);

    // Non-terminals
    let expr = SymbolId(10);

    // Add EOF token
    g.tokens.insert(
        eof,
        Token {
            name: "EOF".to_string(),
            pattern: TokenPattern::String("".to_string()),
            fragile: false,
        },
    );

    // Add tokens
    g.tokens.insert(
        num,
        Token {
            name: "NUM".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );
    g.tokens.insert(
        plus,
        Token {
            name: "+".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );
    g.tokens.insert(
        times,
        Token {
            name: "*".to_string(),
            pattern: TokenPattern::String("*".to_string()),
            fragile: false,
        },
    );
    g.tokens.insert(
        lparen,
        Token {
            name: "(".to_string(),
            pattern: TokenPattern::String("(".to_string()),
            fragile: false,
        },
    );
    g.tokens.insert(
        rparen,
        Token {
            name: ")".to_string(),
            pattern: TokenPattern::String(")".to_string()),
            fragile: false,
        },
    );

    // Add non-terminals
    g.rule_names.insert(expr, "expr".to_string());

    // expr rules (left-recursive)
    // expr → expr + expr
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![
            Symbol::NonTerminal(expr),
            Symbol::Terminal(plus),
            Symbol::NonTerminal(expr),
        ],
        precedence: Some(PrecedenceKind::Static(1)),
        associativity: Some(Associativity::Left),
        fields: vec![],
        production_id: ProductionId(0),
    });

    // expr → expr * expr
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![
            Symbol::NonTerminal(expr),
            Symbol::Terminal(times),
            Symbol::NonTerminal(expr),
        ],
        precedence: Some(PrecedenceKind::Static(2)), // Higher precedence than +
        associativity: Some(Associativity::Left),
        fields: vec![],
        production_id: ProductionId(1),
    });

    // expr → ( expr )
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![
            Symbol::Terminal(lparen),
            Symbol::NonTerminal(expr),
            Symbol::Terminal(rparen),
        ],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(2),
    });

    // expr → NUM
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![Symbol::Terminal(num)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(3),
    });

    // Return the grammar with an explicit start symbol.
    g.set_start_symbol(expr);
    g
}

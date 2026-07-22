// Debug parse table generation
use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::{
    Associativity, Grammar, Precedence, PrecedenceKind, ProductionId, Rule, Symbol, SymbolId,
    Token, TokenPattern,
};

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
    grammar.rule_names.insert(expr_id, "expr".to_string());

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
    grammar.rules.entry(SymbolId(11)).or_default().push(Rule {
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
    grammar.rules.entry(SymbolId(12)).or_default().push(Rule {
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
    grammar.rules.entry(SymbolId(13)).or_default().push(Rule {
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
    grammar.rules.entry(SymbolId(14)).or_default().push(Rule {
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

#[test]
#[ignore = "KNOWN BUG: FixedBitSet sizing - size 12 too small for symbol ID 15"]
fn test_parse_table_debug() {
    let grammar = build_arithmetic_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let table = build_lr1_automaton(&grammar, &first_follow).unwrap();

    println!("Symbol to index mapping:");
    for (symbol, idx) in &table.symbol_to_index {
        println!("  Symbol {} -> index {}", symbol.0, idx);
    }

    println!("\nState 0 actions:");
    for (idx, actions) in table.action_table[0].iter().enumerate() {
        if !actions.is_empty()
            && !actions
                .iter()
                .all(|a| matches!(a, adze_glr_core::Action::Error))
        {
            println!("  Index {} -> {:?}", idx, actions);
        }
    }

    println!("\nState 1 actions:");
    for (idx, actions) in table.action_table[1].iter().enumerate() {
        if !actions.is_empty()
            && !actions
                .iter()
                .all(|a| matches!(a, adze_glr_core::Action::Error))
        {
            println!("  Index {} -> {:?}", idx, actions);
        }
    }

    println!("\nState 2 actions:");
    for (symbol, &idx) in &table.symbol_to_index {
        let actions = &table.action_table[2][idx];
        if !actions.is_empty()
            && !actions
                .iter()
                .all(|a| matches!(a, adze_glr_core::Action::Error))
        {
            println!("  Symbol {} (idx {}) -> {:?}", symbol.0, idx, actions);
        }
    }

    // Check if we need reductions after shifting a number
    println!("\nChecking what should happen after shifting a number:");
    println!("After state 0 shifts number (1), we're in state 2");
    println!("State 2 should be able to reduce expr->number on both EOF and +");
}

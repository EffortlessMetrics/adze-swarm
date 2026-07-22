//! Minimal indent grammar for testing external tokens
//! This grammar uses an INDENT external token to test external scanner integration

use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

/// Build a minimal grammar with an external INDENT token
#[allow(dead_code)]
pub fn build_indent_grammar() -> Grammar {
    let mut grammar = Grammar::new("indent".to_string());

    // Tokens
    let indent_id = SymbolId(1); // External token - no pattern
    let word_id = SymbolId(2);
    let newline_id = SymbolId(3);

    // Regular tokens with patterns
    grammar.tokens.insert(
        word_id,
        Token {
            name: "WORD".to_string(),
            pattern: TokenPattern::Regex(r"[A-Za-z]+".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        newline_id,
        Token {
            name: "NEWLINE".to_string(),
            pattern: TokenPattern::String("\n".to_string()),
            fragile: false,
        },
    );

    // External token INDENT - no pattern as it's handled by external scanner
    // We still need to register it for symbol mapping
    grammar.tokens.insert(
        indent_id,
        Token {
            name: "INDENT".to_string(),
            pattern: TokenPattern::String("".to_string()), // Empty pattern for external
            fragile: false,
        },
    );

    // Non-terminals
    let line_id = SymbolId(100);
    let block_id = SymbolId(101);
    let program_id = SymbolId(102);

    // Rules
    // program -> block*
    grammar.add_rule(Rule {
        lhs: program_id,
        rhs: vec![],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    grammar.add_rule(Rule {
        lhs: program_id,
        rhs: vec![Symbol::NonTerminal(block_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });

    grammar.add_rule(Rule {
        lhs: program_id,
        rhs: vec![
            Symbol::NonTerminal(program_id),
            Symbol::NonTerminal(block_id),
        ],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(2),
    });

    // block -> INDENT line
    grammar.add_rule(Rule {
        lhs: block_id,
        rhs: vec![Symbol::Terminal(indent_id), Symbol::NonTerminal(line_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(3),
    });

    // line -> WORD | WORD NEWLINE
    grammar.add_rule(Rule {
        lhs: line_id,
        rhs: vec![Symbol::Terminal(word_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(4),
    });

    grammar.add_rule(Rule {
        lhs: line_id,
        rhs: vec![Symbol::Terminal(word_id), Symbol::Terminal(newline_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(5),
    });

    grammar.set_start_symbol(program_id);
    grammar
}

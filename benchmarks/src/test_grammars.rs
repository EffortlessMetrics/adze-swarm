use adze_glr_core::{Action, GotoIndexing, ParseRule, ParseTable, SymbolMetadata};
/// Test grammars for benchmarking incremental parsing
use adze_ir::{
    Associativity, Grammar, PrecedenceKind, ProductionId, Rule, RuleId, StateId, Symbol, SymbolId,
    Token, TokenPattern,
};
use indexmap::IndexMap;
use std::collections::BTreeMap;

/// Test token structure
#[derive(Debug, Clone)]
pub struct TestToken {
    pub symbol: SymbolId,
    pub text: Vec<u8>,
    pub start_byte: usize,
    pub end_byte: usize,
}

/// Load a simple arithmetic grammar for testing
/// Creates a minimal but functional grammar that can parse expressions like "1 + 2 * 3"
pub fn load_arithmetic_grammar() -> (Grammar, ParseTable) {
    // Create symbol IDs
    let expr_symbol = SymbolId(0); // Non-terminal: expression
    let number_token = SymbolId(1); // Terminal: number
    let plus_token = SymbolId(2); // Terminal: +
    let mult_token = SymbolId(3); // Terminal: *
    let lparen_token = SymbolId(4); // Terminal: (
    let rparen_token = SymbolId(5); // Terminal: )

    // Create a simple arithmetic grammar
    let mut grammar = Grammar {
        name: "arithmetic".to_string(),
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
    };

    // Add tokens
    grammar.tokens.insert(
        number_token,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex("[0-9]+".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        plus_token,
        Token {
            name: "plus".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        mult_token,
        Token {
            name: "mult".to_string(),
            pattern: TokenPattern::String("*".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        lparen_token,
        Token {
            name: "lparen".to_string(),
            pattern: TokenPattern::String("(".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        rparen_token,
        Token {
            name: "rparen".to_string(),
            pattern: TokenPattern::String(")".to_string()),
            fragile: false,
        },
    );

    // Add rules
    // Rule 0: expression -> number
    // Rule 1: expression -> expression '+' expression (left associative, precedence 1)
    // Rule 2: expression -> expression '*' expression (left associative, precedence 2)
    // Rule 3: expression -> '(' expression ')'

    let rules = vec![
        Rule {
            lhs: expr_symbol,
            rhs: vec![Symbol::Terminal(number_token)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        },
        Rule {
            lhs: expr_symbol,
            rhs: vec![
                Symbol::NonTerminal(expr_symbol),
                Symbol::Terminal(plus_token),
                Symbol::NonTerminal(expr_symbol),
            ],
            precedence: Some(PrecedenceKind::Static(1)),
            associativity: Some(Associativity::Left),
            fields: vec![],
            production_id: ProductionId(1),
        },
        Rule {
            lhs: expr_symbol,
            rhs: vec![
                Symbol::NonTerminal(expr_symbol),
                Symbol::Terminal(mult_token),
                Symbol::NonTerminal(expr_symbol),
            ],
            precedence: Some(PrecedenceKind::Static(2)),
            associativity: Some(Associativity::Left),
            fields: vec![],
            production_id: ProductionId(2),
        },
        Rule {
            lhs: expr_symbol,
            rhs: vec![
                Symbol::Terminal(lparen_token),
                Symbol::NonTerminal(expr_symbol),
                Symbol::Terminal(rparen_token),
            ],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(3),
        },
    ];

    let parse_rules = rules
        .iter()
        .map(|rule| ParseRule {
            lhs: rule.lhs,
            rhs_len: rule.rhs.len() as u16,
        })
        .collect();

    grammar.rules.insert(expr_symbol, rules);

    // Add rule names
    grammar
        .rule_names
        .insert(expr_symbol, "expression".to_string());
    grammar
        .rule_names
        .insert(number_token, "number".to_string());
    grammar.rule_names.insert(plus_token, "plus".to_string());
    grammar.rule_names.insert(mult_token, "mult".to_string());
    grammar
        .rule_names
        .insert(lparen_token, "lparen".to_string());
    grammar
        .rule_names
        .insert(rparen_token, "rparen".to_string());

    // Create a minimal parse table
    // This is a simplified table - in reality, it would be generated by LR table construction
    // We'll create a table with 10 states and 6 symbols
    let state_count = 10;
    let symbol_count = 6;

    // Initialize action table (states x symbols)
    let mut action_table = vec![vec![vec![]; symbol_count]; state_count];

    // Initialize goto table
    let goto_table = vec![vec![StateId(0); symbol_count]; state_count];

    // Add some basic actions for a minimal working parser
    // State 0: Initial state - can shift a number or lparen
    action_table[0][1] = vec![Action::Shift(StateId(1))]; // Shift number
    action_table[0][4] = vec![Action::Shift(StateId(2))]; // Shift lparen

    // State 1: After number - reduce to expression
    action_table[1][0] = vec![Action::Reduce(RuleId(0))]; // Reduce by rule 0 (expr -> number)
    action_table[1][2] = vec![Action::Reduce(RuleId(0))]; // Can also see plus next
    action_table[1][3] = vec![Action::Reduce(RuleId(0))]; // Can also see mult next
    action_table[1][5] = vec![Action::Reduce(RuleId(0))]; // Can also see rparen next

    // State 2: After lparen - can shift number or another lparen
    action_table[2][1] = vec![Action::Shift(StateId(1))]; // Shift number
    action_table[2][4] = vec![Action::Shift(StateId(2))]; // Shift lparen (nested)

    // Create symbol metadata
    let symbol_metadata = vec![
        SymbolMetadata {
            name: "expression".to_string(),
            is_visible: true,
            is_named: true,
            is_supertype: false,
            // Additional fields required by GLR core API contracts
            is_terminal: false, // expression is a non-terminal
            is_extra: false,
            is_fragile: false,
            symbol_id: SymbolId(0),
        },
        SymbolMetadata {
            name: "number".to_string(),
            is_visible: true,
            is_named: true,
            is_supertype: false,
            // Additional fields required by GLR core API contracts
            is_terminal: true, // number is a terminal
            is_extra: false,
            is_fragile: false,
            symbol_id: SymbolId(1),
        },
        SymbolMetadata {
            name: "plus".to_string(),
            is_visible: true,
            is_named: false,
            is_supertype: false,
            // Additional fields required by GLR core API contracts
            is_terminal: true, // plus is a terminal
            is_extra: false,
            is_fragile: false,
            symbol_id: SymbolId(2),
        },
        SymbolMetadata {
            name: "mult".to_string(),
            is_visible: true,
            is_named: false,
            is_supertype: false,
            // Additional fields required by GLR core API contracts
            is_terminal: true, // mult is a terminal
            is_extra: false,
            is_fragile: false,
            symbol_id: SymbolId(3),
        },
        SymbolMetadata {
            name: "lparen".to_string(),
            is_visible: true,
            is_named: false,
            is_supertype: false,
            // Additional fields required by GLR core API contracts
            is_terminal: true, // lparen is a terminal
            is_extra: false,
            is_fragile: false,
            symbol_id: SymbolId(4),
        },
        SymbolMetadata {
            name: "rparen".to_string(),
            is_visible: true,
            is_named: false,
            is_supertype: false,
            // Additional fields required by GLR core API contracts
            is_terminal: true, // rparen is a terminal
            is_extra: false,
            is_fragile: false,
            symbol_id: SymbolId(5),
        },
    ];

    // Create symbol to index mapping
    let mut symbol_to_index = BTreeMap::new();
    for i in 0..symbol_count {
        symbol_to_index.insert(SymbolId(i as u16), i);
    }

    // Build nonterminal_to_index (expression is the only non-terminal)
    let mut nonterminal_to_index = BTreeMap::new();
    nonterminal_to_index.insert(expr_symbol, 0);

    use adze_glr_core::LexMode;

    // Build index_to_symbol from symbol_to_index
    let mut index_to_symbol = vec![SymbolId(0); symbol_count];
    for (symbol_id, index) in &symbol_to_index {
        index_to_symbol[*index] = *symbol_id;
    }

    let table = ParseTable {
        // Core grids
        action_table,
        goto_table,

        // Grammar rules used by GLR reductions.
        rules: parse_rules,

        // Shapes
        state_count,
        symbol_count,

        // Symbol bookkeeping
        symbol_to_index,
        index_to_symbol,
        nonterminal_to_index,
        symbol_metadata,

        // Token layout / sentinels
        token_count: 5,            // five terminals (excluding EOF)
        external_token_count: 0,   // no externals
        eof_symbol: SymbolId(5),   // EOF column = token_count + externals
        start_symbol: expr_symbol, // Expression is our start symbol

        // Parsing config
        initial_state: StateId(0),

        // Lexing config
        lex_modes: vec![
            LexMode {
                lex_state: 0,
                external_lex_state: 0
            };
            state_count
        ],
        external_scanner_states: vec![vec![false; 0]; state_count], // No external scanner
        extras: vec![],

        // Dynamic precedence
        dynamic_prec_by_rule: vec![],
        rule_assoc_by_rule: vec![],

        // Aliasing
        alias_sequences: vec![],

        // Fields
        field_names: vec![],
        field_map: BTreeMap::new(),

        // Grammar metadata
        grammar: Grammar::new("arithmetic".to_string()),

        // GOTO indexing mode
        goto_indexing: GotoIndexing::NonterminalMap,
    };

    (grammar, table)
}

/// Tokenize arithmetic expression
pub fn tokenize_arithmetic(input: &str) -> Vec<TestToken> {
    let mut tokens = Vec::new();
    let mut position = 0;
    let bytes = input.as_bytes();

    while position < bytes.len() {
        // Skip whitespace
        while position < bytes.len() && bytes[position].is_ascii_whitespace() {
            position += 1;
        }

        if position >= bytes.len() {
            break;
        }

        let start = position;

        // Number
        if bytes[position].is_ascii_digit() {
            while position < bytes.len() && bytes[position].is_ascii_digit() {
                position += 1;
            }
            tokens.push(TestToken {
                symbol: SymbolId(1), // number
                text: bytes[start..position].to_vec(),
                start_byte: start,
                end_byte: position,
            });
        }
        // Plus
        else if bytes[position] == b'+' {
            position += 1;
            tokens.push(TestToken {
                symbol: SymbolId(2), // plus
                text: vec![b'+'],
                start_byte: start,
                end_byte: position,
            });
        }
        // Mult
        else if bytes[position] == b'*' {
            position += 1;
            tokens.push(TestToken {
                symbol: SymbolId(3), // mult
                text: vec![b'*'],
                start_byte: start,
                end_byte: position,
            });
        }
        // Left paren
        else if bytes[position] == b'(' {
            position += 1;
            tokens.push(TestToken {
                symbol: SymbolId(4), // lparen
                text: vec![b'('],
                start_byte: start,
                end_byte: position,
            });
        }
        // Right paren
        else if bytes[position] == b')' {
            position += 1;
            tokens.push(TestToken {
                symbol: SymbolId(5), // rparen
                text: vec![b')'],
                start_byte: start,
                end_byte: position,
            });
        }
        // Unknown - skip
        else {
            position += 1;
        }
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_grammar() {
        let (grammar, table) = load_arithmetic_grammar();

        // Check grammar has rules
        assert!(grammar.rules.contains_key(&SymbolId(0)));
        assert_eq!(grammar.rules.get(&SymbolId(0)).unwrap().len(), 4); // 4 rules for expression

        // Check table dimensions
        assert_eq!(table.state_count, 10);
        assert_eq!(table.symbol_count, 6);
    }

    #[test]
    fn test_tokenize() {
        let tokens = tokenize_arithmetic("1 + 2 * 3");

        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0].symbol, SymbolId(1)); // Number
        assert_eq!(tokens[1].symbol, SymbolId(2)); // Plus
        assert_eq!(tokens[2].symbol, SymbolId(1)); // Number
        assert_eq!(tokens[3].symbol, SymbolId(3)); // Mult
        assert_eq!(tokens[4].symbol, SymbolId(1)); // Number
    }

    #[test]
    fn test_tokenize_with_parens() {
        let tokens = tokenize_arithmetic("(1 + 2) * 3");

        assert_eq!(tokens.len(), 7);
        assert_eq!(tokens[0].symbol, SymbolId(4)); // LParen
        assert_eq!(tokens[1].symbol, SymbolId(1)); // Number
        assert_eq!(tokens[2].symbol, SymbolId(2)); // Plus
        assert_eq!(tokens[3].symbol, SymbolId(1)); // Number
        assert_eq!(tokens[4].symbol, SymbolId(5)); // RParen
        assert_eq!(tokens[5].symbol, SymbolId(3)); // Mult
        assert_eq!(tokens[6].symbol, SymbolId(1)); // Number
    }
}

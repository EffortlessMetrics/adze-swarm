// Integration test for GLR lexer and parser
use adze::glr_lexer::{GLRLexer, tokenize_and_parse};
use adze::glr_parser::GLRParser;
use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
use std::sync::Arc;

#[test]
fn test_arithmetic_with_lexer() {
    let mut grammar = Grammar::new("arithmetic".to_string());

    // Define tokens
    let number_id = SymbolId(1);
    let plus_id = SymbolId(2);
    let times_id = SymbolId(3);
    let lparen_id = SymbolId(4);
    let rparen_id = SymbolId(5);

    // Define non-terminals
    let expr_id = SymbolId(6);
    let term_id = SymbolId(7);
    let factor_id = SymbolId(8);

    // Add tokens
    grammar.tokens.insert(
        number_id,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        plus_id,
        Token {
            name: "plus".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        times_id,
        Token {
            name: "times".to_string(),
            pattern: TokenPattern::String("*".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        lparen_id,
        Token {
            name: "lparen".to_string(),
            pattern: TokenPattern::String("(".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        rparen_id,
        Token {
            name: "rparen".to_string(),
            pattern: TokenPattern::String(")".to_string()),
            fragile: false,
        },
    );

    // Add rules with precedence
    // expr → expr + term (left associative)
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![
            Symbol::NonTerminal(expr_id),
            Symbol::Terminal(plus_id),
            Symbol::NonTerminal(term_id),
        ],
        precedence: None,
        associativity: Some(adze_ir::Associativity::Left),
        fields: vec![],
        production_id: ProductionId(0),
    });

    // expr → term
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![Symbol::NonTerminal(term_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });

    // term → term * factor (left associative, higher precedence)
    grammar.rules.entry(term_id).or_default().push(Rule {
        lhs: term_id,
        rhs: vec![
            Symbol::NonTerminal(term_id),
            Symbol::Terminal(times_id),
            Symbol::NonTerminal(factor_id),
        ],
        precedence: None,
        associativity: Some(adze_ir::Associativity::Left),
        fields: vec![],
        production_id: ProductionId(2),
    });

    // term → factor
    grammar.rules.entry(term_id).or_default().push(Rule {
        lhs: term_id,
        rhs: vec![Symbol::NonTerminal(factor_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(3),
    });

    // factor → number
    grammar.rules.entry(factor_id).or_default().push(Rule {
        lhs: factor_id,
        rhs: vec![Symbol::Terminal(number_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(4),
    });

    // factor → ( expr )
    grammar.rules.entry(factor_id).or_default().push(Rule {
        lhs: factor_id,
        rhs: vec![
            Symbol::Terminal(lparen_id),
            Symbol::NonTerminal(expr_id),
            Symbol::Terminal(rparen_id),
        ],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(5),
    });

    // Add rule names
    grammar.rule_names.insert(expr_id, "expr".to_string());
    grammar.rule_names.insert(term_id, "term".to_string());
    grammar.rule_names.insert(factor_id, "factor".to_string());

    grammar.set_start_symbol(expr_id);
    let grammar = Arc::new(grammar);
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();

    match build_lr1_automaton(&grammar, &first_follow) {
        Ok(parse_table) => {
            let mut parser = GLRParser::new(parse_table, (*grammar).clone());

            // Test parsing "1 + 2 * 3"
            let input = "1 + 2 * 3";

            // Use lexer to tokenize
            let result = tokenize_and_parse(&grammar, input, |symbol_id, text, offset| {
                parser.process_token(symbol_id, text, offset);
            });

            assert!(result.is_ok());
            parser.process_eof(input.len());

            let tree = parser.get_best_parse();
            assert!(tree.is_some());
        }
        Err(e) => {
            panic!("Failed to build parse table: {:?}", e);
        }
    }
}

#[test]
fn test_json_with_lexer() {
    let mut grammar = Grammar::new("json".to_string());

    // Simple JSON: number | { }
    let value_id = SymbolId(1);
    let object_id = SymbolId(2);
    let number_id = SymbolId(3);
    let lbrace_id = SymbolId(4);
    let rbrace_id = SymbolId(5);

    // Add tokens
    grammar.tokens.insert(
        number_id,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        lbrace_id,
        Token {
            name: "lbrace".to_string(),
            pattern: TokenPattern::String("{".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        rbrace_id,
        Token {
            name: "rbrace".to_string(),
            pattern: TokenPattern::String("}".to_string()),
            fragile: false,
        },
    );

    // Rules: value → number | object
    grammar.rules.entry(value_id).or_default().push(Rule {
        lhs: value_id,
        rhs: vec![Symbol::Terminal(number_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    grammar.rules.entry(value_id).or_default().push(Rule {
        lhs: value_id,
        rhs: vec![Symbol::NonTerminal(object_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });

    // object → { }
    grammar.rules.entry(object_id).or_default().push(Rule {
        lhs: object_id,
        rhs: vec![Symbol::Terminal(lbrace_id), Symbol::Terminal(rbrace_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(2),
    });

    grammar.rule_names.insert(value_id, "value".to_string());
    grammar.rule_names.insert(object_id, "object".to_string());

    grammar.set_start_symbol(value_id);
    let grammar = Arc::new(grammar);
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();

    match build_lr1_automaton(&grammar, &first_follow) {
        Ok(parse_table) => {
            // Test parsing "42"
            let mut parser = GLRParser::new(parse_table.clone(), (*grammar).clone());
            let mut lexer = GLRLexer::new(&grammar, "42".to_string()).unwrap();

            while let Some(token) = lexer.next_token() {
                parser.process_token(token.symbol_id, &token.text, token.byte_offset);
            }
            parser.process_eof(2); // "42" is 2 bytes

            assert!(parser.get_best_parse().is_some());

            // Test parsing "{}"
            let mut parser = GLRParser::new(parse_table, (*grammar).clone());
            let mut lexer = GLRLexer::new(&grammar, "{ }".to_string()).unwrap();

            while let Some(token) = lexer.next_token() {
                parser.process_token(token.symbol_id, &token.text, token.byte_offset);
            }
            parser.process_eof(3); // "{ }" is 3 bytes

            assert!(parser.get_best_parse().is_some());
        }
        Err(e) => {
            panic!("Failed to build parse table: {:?}", e);
        }
    }
}

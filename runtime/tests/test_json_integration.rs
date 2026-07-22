// Integration test for JSON parsing with GLR parser
use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
use std::sync::Arc;

// Import from the glr_parser module
use adze::glr_parser::GLRParser;

#[test]
fn test_simple_json_grammar() {
    let mut grammar = Grammar::new("json".to_string());

    // Simple grammar for testing: value → number | string
    let value_id = SymbolId(1);
    let number_id = SymbolId(2);
    let string_id = SymbolId(3);

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
        string_id,
        Token {
            name: "string".to_string(),
            pattern: TokenPattern::Regex(r#""[^"]*""#.to_string()),
            fragile: false,
        },
    );

    // Add rules: value → number
    grammar.rules.entry(value_id).or_default().push(Rule {
        lhs: value_id,
        rhs: vec![Symbol::Terminal(number_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    // Add another rule: value → string
    grammar.rules.entry(value_id).or_default().push(Rule {
        lhs: value_id,
        rhs: vec![Symbol::Terminal(string_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });

    // Add rule name
    grammar.rule_names.insert(value_id, "value".to_string());

    grammar.set_start_symbol(value_id);

    let grammar = Arc::new(grammar);
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();

    match build_lr1_automaton(&grammar, &first_follow) {
        Ok(parse_table) => {
            // Create parser
            let mut parser = GLRParser::new(parse_table, (*grammar).clone());

            // Test parsing a number
            parser.process_token(number_id, "42", 0);
            parser.process_eof(2); // "42" is 2 bytes

            assert!(parser.get_best_parse().is_some());
        }
        Err(e) => {
            panic!("Failed to build parse table: {:?}", e);
        }
    }
}

#[test]
fn test_json_object_grammar() {
    let mut grammar = Grammar::new("json".to_string());

    // Grammar: object → { }
    let object_id = SymbolId(1);
    let lbrace_id = SymbolId(2);
    let rbrace_id = SymbolId(3);

    // Add tokens
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

    // Add rule: object → { }
    grammar.rules.entry(object_id).or_default().push(Rule {
        lhs: object_id,
        rhs: vec![Symbol::Terminal(lbrace_id), Symbol::Terminal(rbrace_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    grammar.rule_names.insert(object_id, "object".to_string());

    grammar.set_start_symbol(object_id);

    let grammar = Arc::new(grammar);
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();

    match build_lr1_automaton(&grammar, &first_follow) {
        Ok(parse_table) => {
            let mut parser = GLRParser::new(parse_table, (*grammar).clone());

            // Test parsing {}
            parser.process_token(lbrace_id, "{", 0);
            parser.process_token(rbrace_id, "}", 1);
            parser.process_eof(2); // "{}" is 2 bytes

            assert!(parser.get_best_parse().is_some());
        }
        Err(e) => {
            panic!("Failed to build parse table: {:?}", e);
        }
    }
}

#[test]
fn test_json_array_with_numbers() {
    let mut grammar = Grammar::new("json".to_string());

    // Grammar:
    // array → [ elements ]
    // elements → number | number , elements
    let array_id = SymbolId(1);
    let elements_id = SymbolId(2);
    let number_id = SymbolId(3);
    let lbracket_id = SymbolId(4);
    let rbracket_id = SymbolId(5);
    let comma_id = SymbolId(6);

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
        lbracket_id,
        Token {
            name: "lbracket".to_string(),
            pattern: TokenPattern::String("[".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        rbracket_id,
        Token {
            name: "rbracket".to_string(),
            pattern: TokenPattern::String("]".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        comma_id,
        Token {
            name: "comma".to_string(),
            pattern: TokenPattern::String(",".to_string()),
            fragile: false,
        },
    );

    // Rule: array → [ elements ]
    grammar.rules.entry(array_id).or_default().push(Rule {
        lhs: array_id,
        rhs: vec![
            Symbol::Terminal(lbracket_id),
            Symbol::NonTerminal(elements_id),
            Symbol::Terminal(rbracket_id),
        ],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    // Rule: elements → number
    grammar.rules.entry(elements_id).or_default().push(Rule {
        lhs: elements_id,
        rhs: vec![Symbol::Terminal(number_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });

    // Rule: elements → number , elements
    grammar.rules.entry(elements_id).or_default().push(Rule {
        lhs: elements_id,
        rhs: vec![
            Symbol::Terminal(number_id),
            Symbol::Terminal(comma_id),
            Symbol::NonTerminal(elements_id),
        ],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(2),
    });

    grammar.rule_names.insert(array_id, "array".to_string());
    grammar
        .rule_names
        .insert(elements_id, "elements".to_string());

    grammar.set_start_symbol(array_id);

    let grammar = Arc::new(grammar);
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();

    match build_lr1_automaton(&grammar, &first_follow) {
        Ok(parse_table) => {
            let mut parser = GLRParser::new(parse_table, (*grammar).clone());

            // Test parsing [1, 2, 3]
            parser.process_token(lbracket_id, "[", 0);
            parser.process_token(number_id, "1", 1);
            parser.process_token(comma_id, ",", 2);
            parser.process_token(number_id, "2", 4);
            parser.process_token(comma_id, ",", 5);
            parser.process_token(number_id, "3", 7);
            parser.process_token(rbracket_id, "]", 8);
            parser.process_eof(9); // "[1, 2, 3]" is 9 bytes

            assert!(parser.get_best_parse().is_some());
        }
        Err(e) => {
            panic!("Failed to build parse table: {:?}", e);
        }
    }
}

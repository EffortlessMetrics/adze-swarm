// Test epsilon (empty) productions in GLR parser

use adze::glr_lexer::GLRLexer;
use adze::glr_parser::GLRParser;
use adze::subtree::Subtree;
use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
use std::sync::Arc;

fn create_epsilon_grammar() -> Grammar {
    let mut grammar = Grammar::new("epsilon_test".to_string());

    // Tokens
    let a_id = SymbolId(1);
    let b_id = SymbolId(2);

    grammar.tokens.insert(
        a_id,
        Token {
            name: "a".to_string(),
            pattern: TokenPattern::String("a".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        b_id,
        Token {
            name: "b".to_string(),
            pattern: TokenPattern::String("b".to_string()),
            fragile: false,
        },
    );

    // Non-terminals
    let s_id = SymbolId(10); // S
    let opt_id = SymbolId(11); // Optional

    grammar.rule_names.insert(s_id, "S".to_string());
    grammar.rule_names.insert(opt_id, "Optional".to_string());

    // Rules:
    // S → Optional a Optional b
    grammar
        .rules
        .entry(s_id) // Key should be the LHS non-terminal
        .or_default()
        .push(Rule {
            lhs: s_id,
            rhs: vec![
                Symbol::NonTerminal(opt_id),
                Symbol::Terminal(a_id),
                Symbol::NonTerminal(opt_id),
                Symbol::Terminal(b_id),
            ],
            precedence: None,
            associativity: None,
            production_id: ProductionId(0),
            fields: vec![],
        });

    // Optional → 'a'
    grammar
        .rules
        .entry(opt_id) // Key should be the LHS non-terminal
        .or_default()
        .push(Rule {
            lhs: opt_id,
            rhs: vec![Symbol::Terminal(a_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(1),
            fields: vec![],
        });

    // Optional → ε (empty)
    grammar
        .rules
        .entry(opt_id) // Key should be the LHS non-terminal
        .or_default()
        .push(Rule {
            lhs: opt_id,
            rhs: vec![], // Empty production
            precedence: None,
            associativity: None,
            production_id: ProductionId(2),
            fields: vec![],
        });

    grammar.set_start_symbol(s_id);
    grammar
}

fn parse_with_grammar(grammar: &Grammar, input: &str) -> Option<Arc<Subtree>> {
    // Generate parse table
    let first_follow = FirstFollowSets::compute(grammar).unwrap();
    let table = build_lr1_automaton(grammar, &first_follow).unwrap();

    // Create parser
    let mut parser = GLRParser::new(table, grammar.clone());

    // Tokenize
    let mut lexer = GLRLexer::new(grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    // Parse
    parser.reset();
    for token in &tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
    }
    parser.process_eof(
        tokens
            .last()
            .map(|t| t.byte_offset + t.text.len())
            .unwrap_or(0),
    );
    parser.finish().ok()
}

#[test]
#[ignore = "Epsilon integration needs more work"]
fn test_epsilon_productions() {
    let grammar = create_epsilon_grammar();

    // Test 1: "ab" - both optionals are empty
    let tree = parse_with_grammar(&grammar, "ab");
    assert!(
        tree.is_some(),
        "Failed to parse 'ab' with epsilon productions"
    );

    // Test 2: "aab" - first optional is 'a', second is empty
    let tree = parse_with_grammar(&grammar, "aab");
    assert!(
        tree.is_some(),
        "Failed to parse 'aab' with epsilon productions"
    );

    // Test 3: "aabb" - both optionals are 'a'
    let tree = parse_with_grammar(&grammar, "aabb");
    assert!(
        tree.is_some(),
        "Failed to parse 'aabb' with epsilon productions"
    );

    // Test 4: "aaab" - first optional is 'a', second is 'a'
    let tree = parse_with_grammar(&grammar, "aaab");
    assert!(
        tree.is_some(),
        "Failed to parse 'aaab' with epsilon productions"
    );
}

#[test]
#[ignore = "Epsilon integration needs more work"]
fn test_multiple_epsilon_paths() {
    let mut grammar = Grammar::new("multi_epsilon".to_string());

    // Token 'x'
    let x_id = SymbolId(1);
    grammar.tokens.insert(
        x_id,
        Token {
            name: "x".to_string(),
            pattern: TokenPattern::String("x".to_string()),
            fragile: false,
        },
    );

    // Non-terminals
    let s_id = SymbolId(10);
    let a_id = SymbolId(11);
    let b_id = SymbolId(12);

    grammar.rule_names.insert(s_id, "S".to_string());
    grammar.rule_names.insert(a_id, "A".to_string());
    grammar.rule_names.insert(b_id, "B".to_string());

    // S → A B
    grammar
        .rules
        .entry(s_id) // Key should be the LHS non-terminal
        .or_default()
        .push(Rule {
            lhs: s_id,
            rhs: vec![Symbol::NonTerminal(a_id), Symbol::NonTerminal(b_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(0),
            fields: vec![],
        });

    // A → x
    grammar
        .rules
        .entry(a_id) // Key should be the LHS non-terminal
        .or_default()
        .push(Rule {
            lhs: a_id,
            rhs: vec![Symbol::Terminal(x_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(1),
            fields: vec![],
        });

    // A → ε
    grammar
        .rules
        .entry(a_id) // Key should be the LHS non-terminal
        .or_default()
        .push(Rule {
            lhs: a_id,
            rhs: vec![],
            precedence: None,
            associativity: None,
            production_id: ProductionId(2),
            fields: vec![],
        });

    // B → x
    grammar
        .rules
        .entry(b_id) // Key should be the LHS non-terminal
        .or_default()
        .push(Rule {
            lhs: b_id,
            rhs: vec![Symbol::Terminal(x_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(3),
            fields: vec![],
        });

    // B → ε
    grammar
        .rules
        .entry(b_id) // Key should be the LHS non-terminal
        .or_default()
        .push(Rule {
            lhs: b_id,
            rhs: vec![],
            precedence: None,
            associativity: None,
            production_id: ProductionId(4),
            fields: vec![],
        });

    grammar.set_start_symbol(s_id);

    // Test parsing different inputs
    let tree = parse_with_grammar(&grammar, "");
    assert!(tree.is_some(), "Failed to parse empty string with A→ε, B→ε");

    let tree = parse_with_grammar(&grammar, "x");
    assert!(
        tree.is_some(),
        "Failed to parse 'x' (ambiguous: could be A→x,B→ε or A→ε,B→x)"
    );

    let tree = parse_with_grammar(&grammar, "xx");
    assert!(tree.is_some(), "Failed to parse 'xx' with A→x, B→x");
}

#[test]
#[ignore = "Epsilon integration needs more work"]
fn test_epsilon_with_recursion() {
    let mut grammar = Grammar::new("epsilon_recursion".to_string());

    // Token 'a'
    let a_id = SymbolId(1);
    grammar.tokens.insert(
        a_id,
        Token {
            name: "a".to_string(),
            pattern: TokenPattern::String("a".to_string()),
            fragile: false,
        },
    );

    // Non-terminal List
    let list_id = SymbolId(10);
    grammar.rule_names.insert(list_id, "List".to_string());

    // List → List a
    grammar
        .rules
        .entry(list_id) // Key should be the LHS non-terminal
        .or_default()
        .push(Rule {
            lhs: list_id,
            rhs: vec![Symbol::NonTerminal(list_id), Symbol::Terminal(a_id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(0),
            fields: vec![],
        });

    // List → ε
    grammar
        .rules
        .entry(list_id) // Key should be the LHS non-terminal
        .or_default()
        .push(Rule {
            lhs: list_id,
            rhs: vec![],
            precedence: None,
            associativity: None,
            production_id: ProductionId(1),
            fields: vec![],
        });

    grammar.set_start_symbol(list_id);

    // Test parsing
    let tree = parse_with_grammar(&grammar, "");
    assert!(tree.is_some(), "Failed to parse empty list");

    let tree = parse_with_grammar(&grammar, "a");
    assert!(tree.is_some(), "Failed to parse single element list");

    let tree = parse_with_grammar(&grammar, "aaa");
    assert!(tree.is_some(), "Failed to parse multi-element list");
}

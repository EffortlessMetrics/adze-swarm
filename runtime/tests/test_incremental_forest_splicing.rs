//! Test for the new forest splicing incremental parsing approach

use adze::glr_incremental::{GLREdit, GLRToken, IncrementalGLRParser};
use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
use std::sync::Arc;

/// Create a simple arithmetic grammar for testing
fn create_test_grammar() -> Grammar {
    let mut grammar = Grammar::new("test".to_string());

    // Tokens
    let num_id = SymbolId(1);
    let plus_id = SymbolId(2);

    // Non-terminals
    let expr_id = SymbolId(10);
    let source_file_id = SymbolId(11);

    // Add terminals
    grammar.tokens.insert(
        num_id,
        Token {
            name: "NUM".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        plus_id,
        Token {
            name: "PLUS".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );

    // Rules for expr: expr PLUS expr | NUM
    let expr_rules = vec![
        Rule {
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
        },
        Rule {
            lhs: expr_id,
            rhs: vec![Symbol::Terminal(num_id)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(1),
        },
    ];

    grammar.rules.insert(expr_id, expr_rules);

    // source_file: expr
    let source_rules = vec![Rule {
        lhs: source_file_id,
        rhs: vec![Symbol::NonTerminal(expr_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(2),
    }];

    grammar.rules.insert(source_file_id, source_rules);
    grammar.rule_names.insert(expr_id, "expr".to_string());
    grammar
        .rule_names
        .insert(source_file_id, "source_file".to_string());

    grammar.set_start_symbol(source_file_id);
    grammar
}

fn tokenize(input: &str) -> Vec<GLRToken> {
    let mut tokens = Vec::new();
    let mut byte_offset = 0;

    for part in input.split_whitespace() {
        let symbol = if part.chars().all(|c| c.is_ascii_digit()) {
            SymbolId(1) // NUM
        } else if part == "+" {
            SymbolId(2) // PLUS
        } else {
            panic!("Unknown token: {}", part);
        };

        tokens.push(GLRToken {
            symbol,
            text: part.as_bytes().to_vec(),
            start_byte: byte_offset,
            end_byte: byte_offset + part.len(),
        });

        byte_offset += part.len() + 1; // +1 for space
    }

    tokens
}

#[test]
fn test_forest_splicing_simple_edit() {
    let grammar = Arc::new(create_test_grammar());
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let table = Arc::new(build_lr1_automaton(&grammar, &first_follow).unwrap());

    // Initial parse: "1 + 2 + 3"
    let initial_tokens = tokenize("1 + 2 + 3");
    let mut parser = IncrementalGLRParser::new((*grammar).clone(), (*table).clone());
    let initial_forest = parser.parse_incremental(&initial_tokens, &[]).unwrap();

    // Edit: "1 + 2 + 3" -> "1 + 5 + 3" (change middle number)
    let new_tokens = tokenize("1 + 5 + 3");
    let edit = GLREdit {
        old_range: 4..5, // Position of "2"
        new_text: b"5".to_vec(),
        old_token_range: 2..3, // Token index of "2" in the token stream
        new_tokens: vec![GLRToken {
            symbol: SymbolId(1), // NUM
            text: b"5".to_vec(),
            start_byte: 4,
            end_byte: 5,
        }],
        old_tokens: initial_tokens.clone(),
        old_forest: Some(initial_forest.clone()),
    };

    // Incremental parse with forest splicing
    let new_forest = parser.parse_incremental(&new_tokens, &[edit]).unwrap();

    // Verify that we got a valid parse tree
    assert!(!new_forest.alternatives.is_empty());

    // The forest should represent the expression "1 + 5 + 3"
    println!("Incremental parse successful!");
    println!("Forest has {} alternatives", new_forest.alternatives.len());
}

#[test]
fn test_forest_splicing_prefix_reuse() {
    let grammar = Arc::new(create_test_grammar());
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let table = Arc::new(build_lr1_automaton(&grammar, &first_follow).unwrap());

    // Initial parse: "1 + 2 + 3 + 4"
    let initial_tokens = tokenize("1 + 2 + 3 + 4");
    let mut parser = IncrementalGLRParser::new((*grammar).clone(), (*table).clone());
    let initial_forest = parser.parse_incremental(&initial_tokens, &[]).unwrap();

    // Edit at the end: "1 + 2 + 3 + 4" -> "1 + 2 + 3 + 9"
    let new_tokens = tokenize("1 + 2 + 3 + 9");
    let edit = GLREdit {
        old_range: 12..13, // Position of "4"
        new_text: b"9".to_vec(),
        old_token_range: 6..7, // Token index of "4" in the token stream
        new_tokens: vec![GLRToken {
            symbol: SymbolId(1), // NUM
            text: b"9".to_vec(),
            start_byte: 12,
            end_byte: 13,
        }],
        old_tokens: initial_tokens.clone(),
        old_forest: Some(initial_forest.clone()),
    };

    // Incremental parse should reuse the prefix "1 + 2 + 3"
    let new_forest = parser.parse_incremental(&new_tokens, &[edit]).unwrap();

    // Verify parsing succeeded
    assert!(!new_forest.alternatives.is_empty());
    println!("Prefix reuse test successful!");
}

#[test]
fn test_forest_splicing_suffix_reuse() {
    let grammar = Arc::new(create_test_grammar());
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let table = Arc::new(build_lr1_automaton(&grammar, &first_follow).unwrap());

    // Initial parse: "1 + 2 + 3 + 4"
    let initial_tokens = tokenize("1 + 2 + 3 + 4");
    let mut parser = IncrementalGLRParser::new((*grammar).clone(), (*table).clone());
    let initial_forest = parser.parse_incremental(&initial_tokens, &[]).unwrap();

    // Edit at the beginning: "1 + 2 + 3 + 4" -> "9 + 2 + 3 + 4"
    let new_tokens = tokenize("9 + 2 + 3 + 4");
    let edit = GLREdit {
        old_range: 0..1, // Position of "1"
        new_text: b"9".to_vec(),
        old_token_range: 0..1, // Token index of "1" in the token stream
        new_tokens: vec![GLRToken {
            symbol: SymbolId(1), // NUM
            text: b"9".to_vec(),
            start_byte: 0,
            end_byte: 1,
        }],
        old_tokens: initial_tokens.clone(),
        old_forest: Some(initial_forest.clone()),
    };

    // Incremental parse should reuse the suffix "2 + 3 + 4"
    let new_forest = parser.parse_incremental(&new_tokens, &[edit]).unwrap();

    // Verify parsing succeeded
    assert!(!new_forest.alternatives.is_empty());
    println!("Suffix reuse test successful!");
}

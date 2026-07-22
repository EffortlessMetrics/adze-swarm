// Test deep parentheses nesting
use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

// Import internal modules for testing
#[path = "../src/error_recovery.rs"]
mod error_recovery;
#[path = "../src/glr_lexer.rs"]
mod glr_lexer;
#[path = "../src/glr_parser.rs"]
mod glr_parser;
#[path = "../src/subtree.rs"]
mod subtree;

use glr_lexer::GLRLexer;
use glr_parser::GLRParser;
use std::sync::Arc;

fn create_simple_grammar() -> Grammar {
    let mut grammar = Grammar::new("simple".to_string());

    // Terminals
    let num_id = SymbolId(1);
    let lparen_id = SymbolId(2);
    let rparen_id = SymbolId(3);

    grammar.tokens.insert(
        num_id,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
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

    // Non-terminal
    let expr_id = SymbolId(10);
    grammar.rule_names.insert(expr_id, "expr".to_string());

    // Rules
    // Rule 1: expr → number
    grammar.rules.entry(SymbolId(20)).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![Symbol::Terminal(num_id)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });

    // Rule 2: expr → ( expr )
    grammar.rules.entry(SymbolId(21)).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![
            Symbol::Terminal(lparen_id),
            Symbol::NonTerminal(expr_id),
            Symbol::Terminal(rparen_id),
        ],
        precedence: None,
        associativity: None,
        production_id: ProductionId(1),
        fields: vec![],
    });

    grammar.set_start_symbol(expr_id);
    grammar
}

#[test]
#[ignore = "GLR parser issue with simple grammar - needs investigation"]
fn test_very_deep_parentheses() {
    let grammar = create_simple_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();
    // Test various depths
    let depths = vec![1, 5, 10, 20, 50, 100, 200, 500];

    for depth in depths {
        // Build input string with 'depth' nested parentheses
        let mut input = String::new();
        for _ in 0..depth {
            input.push('(');
        }
        input.push('1');
        for _ in 0..depth {
            input.push(')');
        }

        println!(
            "\nTesting depth {}: {}",
            depth,
            if input.len() > 20 {
                format!("{}...{}", &input[..10], &input[input.len() - 10..])
            } else {
                input.clone()
            }
        );

        // Create parser for new parse
        let mut parser = GLRParser::new(parse_table.clone(), grammar.clone());
        let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
        let tokens = lexer.tokenize_all();

        println!("Token count: {}", tokens.len());

        // Process tokens
        for token in &tokens {
            parser.process_token(token.symbol_id, &token.text, token.byte_offset);
        }

        parser.process_eof(input.len());

        let result = parser.finish();
        match result {
            Ok(tree) => {
                // Count actual depth
                let mut max_depth = 0;
                count_depth(&tree, 0, &mut max_depth);
                println!("✓ Parse succeeded! Tree depth: {}", max_depth);
            }
            Err(e) => {
                println!("✗ Parse FAILED at depth {}: {:?}", depth, e);
                panic!(
                    "Failed to parse deeply nested parentheses at depth {}",
                    depth
                );
            }
        }
    }
}

fn count_depth(tree: &Arc<subtree::Subtree>, current: usize, max: &mut usize) {
    if current > *max {
        *max = current;
    }
    for edge in &tree.children {
        count_depth(&edge.subtree, current + 1, max);
    }
}

// Debug test for parentheses parsing issue

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
use subtree::Subtree;

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
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![Symbol::Terminal(num_id)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });

    // Rule 2: expr → ( expr )
    grammar.rules.entry(expr_id).or_default().push(Rule {
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
fn test_paren_debug() {
    let grammar = create_simple_grammar();

    println!("Grammar rules:");
    for (id, rules) in &grammar.rules {
        for rule in rules {
            println!("  Rule {}: {:?} → {:?}", id.0, rule.lhs, rule.rhs);
        }
    }

    let first_follow = FirstFollowSets::compute(&grammar).unwrap();

    println!("\nFirst/Follow sets computed");

    let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();

    println!(
        "\nParse table built with {} states",
        parse_table.state_count
    );

    let mut parser = GLRParser::new(parse_table, grammar.clone());

    // Test cases
    let test_cases = vec![
        ("1", "single number"),
        ("(1)", "single paren"),
        ("((1))", "double paren"),
        ("(((1)))", "triple paren"),
    ];

    for (input, desc) in test_cases {
        println!("\n\n=== Testing: {} ({}) ===", input, desc);

        // parser.reset(); // Not available in current API
        let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
        let tokens = lexer.tokenize_all();

        println!(
            "Tokens: {:?}",
            tokens
                .iter()
                .map(|t| (t.symbol_id.0, &t.text))
                .collect::<Vec<_>>()
        );

        // Calculate total bytes
        let total_bytes = tokens
            .last()
            .map(|t| t.byte_offset + t.text.len())
            .unwrap_or(0);

        // Process tokens with debug output
        for (i, token) in tokens.iter().enumerate() {
            println!(
                "\n--- Processing token {}: {:?} ---",
                i,
                (token.symbol_id.0, &token.text)
            );
            parser.process_token(token.symbol_id, &token.text, token.byte_offset);

            // Print parser state info
            println!("Parser state after token {}", i);
        }

        println!("\n--- Processing EOF ---");
        parser.process_eof(total_bytes);

        match parser.finish() {
            Ok(tree) => {
                println!("\n✓ Parse succeeded!");
                print_tree(&tree, 0);
            }
            Err(e) => {
                println!("\n✗ Parse failed: {:?}", e);
            }
        }
    }
}

fn print_tree(tree: &Arc<Subtree>, indent: usize) {
    let spaces = " ".repeat(indent);
    println!("{}Symbol {}", spaces, tree.node.symbol_id.0);
    for edge in &tree.children {
        print_tree(&edge.subtree, indent + 2);
    }
}

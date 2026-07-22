// Test GLR fork/merge functionality
use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
use std::sync::Arc;

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

/// Create an ambiguous grammar: E → E E | a
/// This grammar is ambiguous for strings like "aaa"
fn create_ambiguous_grammar() -> Grammar {
    let mut grammar = Grammar::new("ambiguous".to_string());

    // Terminal 'a'
    let a_id = SymbolId(1);
    grammar.tokens.insert(
        a_id,
        Token {
            name: "a".to_string(),
            pattern: TokenPattern::String("a".to_string()),
            fragile: false,
        },
    );

    // Non-terminal E
    let e_id = SymbolId(10);
    grammar.rule_names.insert(e_id, "E".to_string());

    // Rule 1: E → a
    grammar.rules.entry(e_id).or_default().push(Rule {
        lhs: e_id,
        rhs: vec![Symbol::Terminal(a_id)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });

    // Rule 2: E → E E (ambiguous concatenation)
    grammar.rules.entry(e_id).or_default().push(Rule {
        lhs: e_id,
        rhs: vec![Symbol::NonTerminal(e_id), Symbol::NonTerminal(e_id)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(1),
        fields: vec![],
    });

    grammar.set_start_symbol(e_id);
    grammar
}

/// Create ambiguous arithmetic grammar with precedence
fn create_arithmetic_grammar() -> Grammar {
    let mut grammar = Grammar::new("arithmetic".to_string());

    // Terminals
    let num_id = SymbolId(1);
    let plus_id = SymbolId(2);
    let times_id = SymbolId(3);

    grammar.tokens.insert(
        num_id,
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

    // Non-terminal E
    let e_id = SymbolId(10);
    grammar.rule_names.insert(e_id, "E".to_string());

    // Rules with precedence
    // E → E + E (lower precedence)
    grammar.rules.entry(e_id).or_default().push(Rule {
        lhs: e_id,
        rhs: vec![
            Symbol::NonTerminal(e_id),
            Symbol::Terminal(plus_id),
            Symbol::NonTerminal(e_id),
        ],
        precedence: Some(adze_ir::PrecedenceKind::Static(1)),
        associativity: Some(adze_ir::Associativity::Left),
        production_id: ProductionId(0),
        fields: vec![],
    });

    // E → E * E (higher precedence)
    grammar.rules.entry(e_id).or_default().push(Rule {
        lhs: e_id,
        rhs: vec![
            Symbol::NonTerminal(e_id),
            Symbol::Terminal(times_id),
            Symbol::NonTerminal(e_id),
        ],
        precedence: Some(adze_ir::PrecedenceKind::Static(2)),
        associativity: Some(adze_ir::Associativity::Left),
        production_id: ProductionId(1),
        fields: vec![],
    });

    // E → number
    grammar.rules.entry(e_id).or_default().push(Rule {
        lhs: e_id,
        rhs: vec![Symbol::Terminal(num_id)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(2),
        fields: vec![],
    });

    grammar.set_start_symbol(e_id);
    grammar
}

#[test]
fn test_simple_fork_merge() {
    let grammar = create_ambiguous_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();
    let mut parser = GLRParser::new(parse_table, grammar.clone());

    // Parse "aaa" - should create forks (ambiguity surfaces at length >= 3 in LR(1))
    let input = "aaa";
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    println!("\n=== Testing fork/merge with input '{}' ===", input);

    let mut stack_counts = Vec::new();
    for token in &tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
        let count = parser.stack_count();
        stack_counts.push(count);
        println!("After token '{}': {} active stacks", token.text, count);
    }

    parser.process_eof(input.len());
    println!("After EOF: {} active stacks", parser.stack_count());

    let result = parser.finish();
    assert!(result.is_ok(), "Parser should handle ambiguous input");

    // We should have seen multiple stacks during parsing
    assert!(
        stack_counts.iter().any(|&c| c > 1),
        "Expected multiple stacks during parsing of ambiguous input, but got {:?}",
        stack_counts
    );

    println!("✓ Fork/merge working correctly");
}

#[test]
fn test_complex_ambiguity() {
    let grammar = create_ambiguous_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();
    let mut parser = GLRParser::new(parse_table, grammar.clone());

    // Parse "aaaa" - highly ambiguous
    let input = "aaaa";
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    println!("\n=== Testing complex ambiguity with input '{}' ===", input);

    let mut max_stacks = 0;
    for token in &tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
        let count = parser.stack_count();
        max_stacks = max_stacks.max(count);
        println!("After token '{}': {} active stacks", token.text, count);
    }

    parser.process_eof(input.len());
    let final_count = parser.stack_count();
    println!("After EOF: {} active stacks", final_count);
    println!("Maximum stacks during parsing: {}", max_stacks);

    let result = parser.finish();
    assert!(
        result.is_ok(),
        "Parser should handle highly ambiguous input"
    );

    // For "aaaa", we expect many possible parse trees
    assert!(
        max_stacks > 2,
        "Expected many stacks for highly ambiguous input, but got max {}",
        max_stacks
    );

    println!("✓ Complex ambiguity handled correctly");
}

// Helper function for extracting operators from parse trees
fn find_operator(tree: &Arc<subtree::Subtree>) -> Option<SymbolId> {
    if tree.children.len() == 3 {
        // Middle child should be operator
        Some(tree.children[1].subtree.node.symbol_id)
    } else {
        None
    }
}

#[test]
fn test_precedence_disambiguation() {
    let grammar = create_arithmetic_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();

    // Debug: Print parse table information
    println!("\n=== Parse Table Debug Information ===");
    println!("Number of states: {}", parse_table.action_table.len());
    println!("Symbol to index mapping: {:?}", parse_table.symbol_to_index);
    if !parse_table.dynamic_prec_by_rule.is_empty() {
        println!(
            "Dynamic precedences by rule: {:?}",
            parse_table.dynamic_prec_by_rule
        );
    }
    if !parse_table.rule_assoc_by_rule.is_empty() {
        println!("Rule associativities: {:?}", parse_table.rule_assoc_by_rule);
    }

    let mut parser = GLRParser::new(parse_table, grammar.clone());

    // Parse "1+2*3" - should disambiguate using precedence
    let input = "1+2*3";
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    println!(
        "\n=== Testing precedence disambiguation with input '{}' ===",
        input
    );

    for token in &tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
        println!(
            "After token '{}': {} active stacks",
            token.text,
            parser.stack_count()
        );
    }

    parser.process_eof(input.len());

    let result = parser.finish();
    assert!(result.is_ok(), "Parser should handle arithmetic expression");

    let tree = result.unwrap();

    // Verify the parse tree structure
    // With correct precedence, should parse as 1+(2*3), not (1+2)*3

    fn print_tree_structure(tree: &Arc<subtree::Subtree>, depth: usize) {
        let indent = "  ".repeat(depth);
        println!(
            "{}Node: symbol_id={:?}, children={}",
            indent,
            tree.node.symbol_id,
            tree.children.len()
        );
        for (i, child) in tree.children.iter().enumerate() {
            println!("{}Child {}: ", indent, i);
            print_tree_structure(&child.subtree, depth + 1);
        }
    }

    fn analyze_tree_operators(tree: &Arc<subtree::Subtree>) {
        // Find all operator nodes in the tree
        let mut operators = Vec::new();
        fn collect_operators(tree: &Arc<subtree::Subtree>, operators: &mut Vec<(SymbolId, usize)>) {
            // Check if this is an operator node (has 3 children: operand, op, operand)
            if tree.children.len() == 3
                && let Some(op_child) = tree.children.get(1)
            {
                operators.push((op_child.subtree.node.symbol_id, tree.children.len()));
            }
            for child in &tree.children {
                collect_operators(&child.subtree, operators);
            }
        }

        collect_operators(tree, &mut operators);
        println!("Found operators: {:?}", operators);

        // Check if this looks like 1+(2*3) or (1+2)*3
        if tree.children.len() == 3 {
            let left = &tree.children[0].subtree;
            let op = &tree.children[1].subtree;
            let right = &tree.children[2].subtree;

            println!(
                "Root operation: left={:?}, op={:?}, right={:?}",
                left.node.symbol_id, op.node.symbol_id, right.node.symbol_id
            );

            if op.node.symbol_id == SymbolId(2) {
                // addition
                println!("Root is ADDITION - this is correct for 1+2*3");
                if right.children.len() == 3 {
                    println!("Right side is a binary operation - checking if it's multiplication");
                    if let Some(right_op) = right.children.get(1) {
                        if right_op.subtree.node.symbol_id == SymbolId(3) {
                            println!("Right side is MULTIPLICATION - structure is 1+(2*3) ✓");
                        } else {
                            println!(
                                "Right side operator is {:?} - not multiplication",
                                right_op.subtree.node.symbol_id
                            );
                        }
                    }
                } else {
                    println!(
                        "Right side has {} children - not a binary operation",
                        right.children.len()
                    );
                }
            } else if op.node.symbol_id == SymbolId(3) {
                // multiplication
                println!("Root is MULTIPLICATION - this is WRONG for 1+2*3");
                if left.children.len() == 3 {
                    println!("Left side is a binary operation - structure is (1+2)*3 ✗");
                } else {
                    println!("Left side has {} children", left.children.len());
                }
            } else {
                println!("Root operator is {:?} - unexpected", op.node.symbol_id);
            }
        }
    }

    println!("Parse tree structure:");
    print_tree_structure(&tree, 0);

    println!("\nAnalyzing tree structure:");
    analyze_tree_operators(&tree);

    // The root should be addition (lower precedence)
    let root_op = find_operator(&tree);
    println!("\nRoot operator: {:?}", root_op);

    // Verify the tree structure is correct: should be 1+(2*3), not (1+2)*3
    if tree.children.len() == 3 {
        let right_child = &tree.children[2].subtree;
        if right_child.children.len() == 3 {
            // This is correct! The right side should be (2*3)
            println!("✓ Correct tree structure: 1+(2*3)");
            let right_op = &right_child.children[1].subtree;
            assert_eq!(
                right_op.node.symbol_id,
                SymbolId(3),
                "Right side operator should be multiplication (SymbolId(3)), got {:?}",
                right_op.node.symbol_id
            );
        } else {
            panic!(
                "PRECEDENCE BUG: Right child has {} children (should have 3 for binary operation 2*3). \
                This indicates precedence is not working correctly.",
                right_child.children.len()
            );
        }
    }

    assert_eq!(
        root_op,
        Some(SymbolId(2)),
        "Root should be addition operator (SymbolId(2)), but got {:?}",
        root_op
    );

    println!("✓ Precedence disambiguation working correctly");
}

#[test]
fn test_precedence_disambiguation_complex() {
    let grammar = create_arithmetic_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();
    let mut parser = GLRParser::new(parse_table, grammar.clone());

    // Test more complex precedence: 1+2*3+4 should be (1+(2*3))+4
    let input = "1+2*3+4";
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    println!(
        "\n=== Testing complex precedence disambiguation with input '{}' ===",
        input
    );

    for token in &tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
        println!(
            "After token '{}': {} active stacks",
            token.text,
            parser.stack_count()
        );
    }

    parser.process_eof(input.len());

    let result = parser.finish();
    assert!(
        result.is_ok(),
        "Parser should handle complex arithmetic expression"
    );

    let tree = result.unwrap();

    // The root should be the outer addition
    let root_op = find_operator(&tree);
    assert_eq!(
        root_op,
        Some(SymbolId(2)),
        "Root should be addition operator for complex expression"
    );

    // The left side should contain the multiplication (1+(2*3))
    if tree.children.len() == 3 {
        let left_child = &tree.children[0].subtree;
        if left_child.children.len() == 3 {
            // This should be 1+(2*3)
            let left_right = &left_child.children[2].subtree;
            if left_right.children.len() == 3 {
                let mult_op = &left_right.children[1].subtree;
                assert_eq!(
                    mult_op.node.symbol_id,
                    SymbolId(3),
                    "Should have multiplication operator in left subtree"
                );
            }
        }
    }

    println!("✓ Complex precedence disambiguation working correctly");
}

#[test]
fn test_precedence_disambiguation_right_associative() {
    // Test right-associative operators would work (though our grammar uses left associative)
    let grammar = create_arithmetic_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();
    let mut parser = GLRParser::new(parse_table, grammar.clone());

    // Test chained multiplication: 2*3*4 should be (2*3)*4 due to left associativity
    let input = "2*3*4";
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    println!(
        "\n=== Testing left associativity with input '{}' ===",
        input
    );

    for token in &tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
    }

    parser.process_eof(input.len());

    let result = parser.finish();
    assert!(
        result.is_ok(),
        "Parser should handle chained multiplication"
    );

    let tree = result.unwrap();

    // The root should be multiplication
    let root_op = find_operator(&tree);
    assert_eq!(
        root_op,
        Some(SymbolId(3)),
        "Root should be multiplication operator for chained multiplication"
    );

    // For left associativity (2*3)*4, the left child should be a binary operation
    if tree.children.len() == 3 {
        let left_child = &tree.children[0].subtree;
        assert_eq!(
            left_child.children.len(),
            3,
            "Left child should be binary operation (2*3) for left associativity"
        );
    }

    println!("✓ Left associativity working correctly");
}

#[test]
fn test_merge_identical_stacks() {
    let grammar = create_ambiguous_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();
    let mut parser = GLRParser::new(parse_table, grammar.clone());

    // Parse "aaa" and track stack counts
    let input = "aaa";
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    println!("\n=== Testing stack merging with input '{}' ===", input);

    let mut stack_history = Vec::new();
    for token in &tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
        let count = parser.stack_count();
        stack_history.push(count);
        println!("After token '{}': {} active stacks", token.text, count);
    }

    parser.process_eof(input.len());
    let final_count = parser.stack_count();
    stack_history.push(final_count);
    println!("After EOF: {} active stacks", final_count);

    // Verify that merging is happening
    // Without merging, the number of stacks would grow exponentially
    // With merging, it should be more controlled
    let max_stacks = *stack_history.iter().max().unwrap();
    println!("Stack count history: {:?}", stack_history);
    println!("Maximum stacks: {}", max_stacks);

    // For "aaa", without merging we'd have many more stacks
    assert!(
        max_stacks < 20,
        "Stack count suggests merging may not be working properly: {}",
        max_stacks
    );

    let result = parser.finish();
    assert!(result.is_ok(), "Parser should complete successfully");

    println!("✓ Stack merging working correctly");
}

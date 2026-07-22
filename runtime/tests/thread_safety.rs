//! Thread safety tests for the adze runtime crate.
//!
//! Verifies Send/Sync bounds, concurrent parsing, shared grammar access,
//! concurrent parse + visitor usage, error recovery under concurrent load,
//! and parse table sharing across threads.

use adze::error_recovery::{ErrorNode, ErrorRecoveryConfig, RecoveryStrategy};
use adze::glr_lexer::GLRLexer;
use adze::glr_parser::GLRParser;
use adze::subtree::Subtree;
use adze_glr_core::{FirstFollowSets, ParseTable, build_lr1_automaton};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
use std::sync::Arc;
use std::thread;

// ---------------------------------------------------------------------------
// Static assertions for Send + Sync
// ---------------------------------------------------------------------------

fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}

#[test]
fn send_sync_bounds() {
    // Core IR/table types
    assert_send::<Grammar>();
    assert_sync::<Grammar>();
    assert_send::<ParseTable>();
    assert_sync::<ParseTable>();

    // Subtree (used in Arc across threads)
    assert_send::<Subtree>();
    assert_sync::<Subtree>();

    // Error recovery types
    assert_send::<ErrorRecoveryConfig>();
    assert_sync::<ErrorRecoveryConfig>();
    assert_send::<ErrorNode>();
    assert_sync::<ErrorNode>();
    assert_send::<RecoveryStrategy>();
    assert_sync::<RecoveryStrategy>();
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a simple arithmetic grammar: expr → number | expr '+' expr
fn arithmetic_grammar() -> Grammar {
    let mut grammar = Grammar::new("arithmetic".to_string());

    let num_id = SymbolId(1);
    let plus_id = SymbolId(2);
    let expr_id = SymbolId(10);

    grammar.tokens.insert(
        num_id,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex("[0-9]+".to_string()),
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

    grammar.rule_names.insert(expr_id, "expr".to_string());

    // expr → number
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![Symbol::Terminal(num_id)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });

    // expr → expr '+' expr
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![
            Symbol::NonTerminal(expr_id),
            Symbol::Terminal(plus_id),
            Symbol::NonTerminal(expr_id),
        ],
        precedence: None,
        associativity: None,
        production_id: ProductionId(1),
        fields: vec![],
    });

    grammar.set_start_symbol(expr_id);
    grammar
}

fn build_table(grammar: &Grammar) -> ParseTable {
    let ff = FirstFollowSets::compute(grammar).unwrap();
    build_lr1_automaton(grammar, &ff).expect("Failed to build parse table")
}

/// Parse `input` using the given grammar+table, returning the root subtree.
fn parse_input(grammar: &Grammar, table: &ParseTable, input: &str) -> Arc<Subtree> {
    let mut parser = GLRParser::new(table.clone(), grammar.clone());
    let mut lexer = GLRLexer::new(grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();
    parser.reset();
    for token in &tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
    }
    let total_bytes = tokens
        .last()
        .map(|t| t.byte_offset + t.text.len())
        .unwrap_or(0);
    parser.process_eof(total_bytes);
    parser.finish().expect("parse should succeed")
}

// ---------------------------------------------------------------------------
// 1. Concurrent parsing: multiple threads parse simultaneously
// ---------------------------------------------------------------------------

#[test]
fn concurrent_parsing() {
    let grammar = arithmetic_grammar();
    let table = build_table(&grammar);

    let inputs = vec!["1 + 2", "3 + 4 + 5", "42", "1 + 2 + 3 + 4"];

    let handles: Vec<_> = inputs
        .into_iter()
        .map(|input| {
            let g = grammar.clone();
            let t = table.clone();
            thread::spawn(move || {
                let tree = parse_input(&g, &t, input);
                assert!(
                    !tree.children.is_empty(),
                    "parsed tree should have structure"
                );
                tree
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread should not panic");
    }
}

// ---------------------------------------------------------------------------
// 2. Shared grammar: multiple threads share the same Grammar via Arc
// ---------------------------------------------------------------------------

#[test]
fn shared_grammar_across_threads() {
    let grammar = Arc::new(arithmetic_grammar());
    let table = Arc::new(build_table(&grammar));

    let mut handles = Vec::new();
    for i in 0..4 {
        let g = Arc::clone(&grammar);
        let t = Arc::clone(&table);
        handles.push(thread::spawn(move || {
            let input = format!("{} + {}", i, i + 1);
            let tree = parse_input(&g, &t, &input);
            assert!(!tree.children.is_empty());
        }));
    }

    for h in handles {
        h.join().expect("thread should not panic");
    }
}

// ---------------------------------------------------------------------------
// 3. Parse + visitor concurrently: one thread parses, another visits
// ---------------------------------------------------------------------------

#[test]
fn parse_and_visit_concurrently() {
    let grammar = arithmetic_grammar();
    let table = build_table(&grammar);

    // Pre-parse a tree that we can share for visiting
    let shared_tree = parse_input(&grammar, &table, "1 + 2 + 3");

    // Clone grammar/table for the parsing thread
    let g = grammar.clone();
    let t = table.clone();

    // Thread 1: parse a new expression
    let parse_handle = thread::spawn(move || parse_input(&g, &t, "10 + 20"));

    // Thread 2: visit the pre-parsed tree (read-only traversal via Arc<Subtree>)
    let tree_clone = Arc::clone(&shared_tree);
    let visit_handle = thread::spawn(move || {
        // Walk the subtree recursively, counting nodes
        fn count_nodes(subtree: &Subtree) -> usize {
            let mut count = 1;
            for edge in &subtree.children {
                count += count_nodes(&edge.subtree);
            }
            count
        }
        let node_count = count_nodes(&tree_clone);
        assert!(node_count > 0, "tree should have at least one node");
        node_count
    });

    let parsed = parse_handle.join().expect("parse thread should not panic");
    let count = visit_handle.join().expect("visit thread should not panic");

    assert!(!parsed.children.is_empty());
    assert!(count >= 1);
}

// ---------------------------------------------------------------------------
// 4. Error recovery under concurrent load
// ---------------------------------------------------------------------------

#[test]
fn error_recovery_concurrent() {
    let grammar = arithmetic_grammar();
    let table = build_table(&grammar);

    // Inputs with errors (e.g., missing operand, double operator)
    let bad_inputs = vec!["1 + ", "+ 2", "1 + + 3", ""];

    let handles: Vec<_> = bad_inputs
        .into_iter()
        .map(|input| {
            let g = grammar.clone();
            let t = table.clone();
            thread::spawn(move || {
                let config = ErrorRecoveryConfig::default();
                let mut parser = GLRParser::new(t, g.clone());
                parser.enable_error_recovery(config);

                let lexer_result = GLRLexer::new(&g, input.to_string());
                if let Ok(mut lexer) = lexer_result {
                    let tokens = lexer.tokenize_all();
                    parser.reset();
                    for token in &tokens {
                        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
                    }
                    let total_bytes = tokens
                        .last()
                        .map(|t| t.byte_offset + t.text.len())
                        .unwrap_or(0);
                    parser.process_eof(total_bytes);
                }
                // We don't assert success here — error recovery may or may not produce
                // a tree. The key invariant is no panic/data race.
                let _result = parser.finish();
            })
        })
        .collect();

    for h in handles {
        h.join().expect("error recovery thread should not panic");
    }
}

// ---------------------------------------------------------------------------
// 5. Parse table sharing across threads
// ---------------------------------------------------------------------------

#[test]
fn parse_table_sharing() {
    let grammar = arithmetic_grammar();
    let table = Arc::new(build_table(&grammar));

    // Many threads share the same Arc<ParseTable> for read-only access
    let handles: Vec<_> = (0..8)
        .map(|i| {
            let t = Arc::clone(&table);
            let g = grammar.clone();
            thread::spawn(move || {
                // Each thread reads table metadata
                assert!(t.state_count > 0, "table should have states");
                assert!(t.symbol_count > 0, "table should have symbols");

                // Each thread independently parses using a clone of the shared table
                let input = format!("{}", i + 1);
                let tree = parse_input(&g, &t, &input);
                assert!(
                    tree.node.symbol_id.0 > 0 || !tree.children.is_empty(),
                    "parsed tree should have valid structure"
                );
            })
        })
        .collect();

    for h in handles {
        h.join().expect("table-sharing thread should not panic");
    }
}

// ---------------------------------------------------------------------------
// 6. Subtree sharing across threads (Arc<Subtree> is Send+Sync)
// ---------------------------------------------------------------------------

#[test]
fn subtree_shared_across_threads() {
    let grammar = arithmetic_grammar();
    let table = build_table(&grammar);
    let tree = parse_input(&grammar, &table, "1 + 2 + 3");

    // Multiple threads read the same Arc<Subtree> simultaneously
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let t = Arc::clone(&tree);
            thread::spawn(move || {
                // Read symbol info from shared tree
                let sym = t.node.symbol_id;
                assert!(sym.0 > 0 || !t.children.is_empty());
                sym
            })
        })
        .collect();

    let results: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().expect("subtree reader should not panic"))
        .collect();

    // All threads should see the same root symbol
    assert!(results.windows(2).all(|w| w[0] == w[1]));
}

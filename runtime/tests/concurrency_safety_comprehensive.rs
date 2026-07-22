#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

//! Comprehensive concurrency and thread-safety tests for the adze runtime.
//!
//! Covers Send/Sync bounds, concurrent parsing, concurrency cap initialization,
//! bounded_parallel_map correctness, race condition prevention, thread pool sizing,
//! and multiple parsers in parallel.

use adze::concurrency_caps::{
    ConcurrencyCaps, DEFAULT_RAYON_NUM_THREADS, DEFAULT_TOKIO_WORKER_THREADS,
    ParallelPartitionPlan, bounded_parallel_map, init_concurrency_caps, init_rayon_global_once,
    normalized_concurrency, parse_positive_usize_or_default,
};
use adze::error_recovery::{ErrorRecoveryConfig, RecoveryStrategy};
use adze::glr_lexer::GLRLexer;
use adze::glr_parser::GLRParser;
use adze::subtree::Subtree;
use adze_glr_core::{FirstFollowSets, ParseTable, build_lr1_automaton};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}

/// Simple arithmetic grammar: expr → number | expr '+' expr
fn arithmetic_grammar() -> Grammar {
    let mut g = Grammar::new("arithmetic".to_string());

    let num = SymbolId(1);
    let plus = SymbolId(2);
    let expr = SymbolId(10);

    g.tokens.insert(
        num,
        Token {
            name: "number".into(),
            pattern: TokenPattern::Regex("[0-9]+".into()),
            fragile: false,
        },
    );
    g.tokens.insert(
        plus,
        Token {
            name: "plus".into(),
            pattern: TokenPattern::String("+".into()),
            fragile: false,
        },
    );

    g.rule_names.insert(expr, "expr".into());

    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![Symbol::Terminal(num)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![
            Symbol::NonTerminal(expr),
            Symbol::Terminal(plus),
            Symbol::NonTerminal(expr),
        ],
        precedence: None,
        associativity: None,
        production_id: ProductionId(1),
        fields: vec![],
    });

    g.set_start_symbol(expr);
    g
}

/// Identifier grammar: ident → ID
fn ident_grammar() -> Grammar {
    let mut g = Grammar::new("ident".to_string());

    let id = SymbolId(1);
    let ident = SymbolId(10);

    g.tokens.insert(
        id,
        Token {
            name: "ID".into(),
            pattern: TokenPattern::Regex("[a-zA-Z_][a-zA-Z0-9_]*".into()),
            fragile: false,
        },
    );

    g.rules.entry(ident).or_default().push(Rule {
        lhs: ident,
        rhs: vec![Symbol::Terminal(id)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });

    g.rule_names.insert(ident, "ident".into());
    g.set_start_symbol(ident);
    g
}

fn build_table(grammar: &Grammar) -> ParseTable {
    let ff = FirstFollowSets::compute(grammar).unwrap();
    build_lr1_automaton(grammar, &ff).expect("build parse table")
}

fn parse_input(grammar: &Grammar, table: &ParseTable, input: &str) -> Arc<Subtree> {
    let mut parser = GLRParser::new(table.clone(), grammar.clone());
    let mut lexer = GLRLexer::new(grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();
    parser.reset();
    for tok in &tokens {
        parser.process_token(tok.symbol_id, &tok.text, tok.byte_offset);
    }
    let total_bytes = tokens
        .last()
        .map(|t| t.byte_offset + t.text.len())
        .unwrap_or(0);
    parser.process_eof(total_bytes);
    parser.finish().expect("parse should succeed")
}

fn count_nodes(subtree: &Subtree) -> usize {
    1 + subtree
        .children
        .iter()
        .map(|e| count_nodes(&e.subtree))
        .sum::<usize>()
}

// ===========================================================================
// 1. Send/Sync bounds for public types
// ===========================================================================

#[test]
fn send_sync_grammar() {
    assert_send::<Grammar>();
    assert_sync::<Grammar>();
}

#[test]
fn send_sync_parse_table() {
    assert_send::<ParseTable>();
    assert_sync::<ParseTable>();
}

#[test]
fn send_sync_subtree() {
    assert_send::<Subtree>();
    assert_sync::<Subtree>();
}

#[test]
fn send_sync_error_recovery_config() {
    assert_send::<ErrorRecoveryConfig>();
    assert_sync::<ErrorRecoveryConfig>();
}

#[test]
fn send_sync_recovery_strategy() {
    assert_send::<RecoveryStrategy>();
    assert_sync::<RecoveryStrategy>();
}

#[test]
fn send_sync_concurrency_caps() {
    assert_send::<ConcurrencyCaps>();
    assert_sync::<ConcurrencyCaps>();
}

// ===========================================================================
// 2. Concurrent parsing from multiple threads
// ===========================================================================

#[test]
fn concurrent_parse_same_grammar() {
    let grammar = Arc::new(arithmetic_grammar());
    let table = Arc::new(build_table(&grammar));
    let barrier = Arc::new(Barrier::new(6));

    let handles: Vec<_> = (0..6)
        .map(|i| {
            let g = Arc::clone(&grammar);
            let t = Arc::clone(&table);
            let b = Arc::clone(&barrier);
            thread::spawn(move || {
                b.wait();
                let input = format!("{} + {}", i, i + 1);
                let tree = parse_input(&g, &t, &input);
                assert!(!tree.children.is_empty());
                count_nodes(&tree)
            })
        })
        .collect();

    let counts: Vec<usize> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    // Same-shape inputs yield same node counts
    assert!(counts.windows(2).all(|w| w[0] == w[1]));
}

#[test]
fn concurrent_parse_different_grammars() {
    let arith_g = Arc::new(arithmetic_grammar());
    let arith_t = Arc::new(build_table(&arith_g));
    let id_g = Arc::new(ident_grammar());
    let id_t = Arc::new(build_table(&id_g));

    let mut handles = Vec::new();
    for i in 0..3 {
        let g = Arc::clone(&arith_g);
        let t = Arc::clone(&arith_t);
        handles.push(thread::spawn(move || {
            let input = format!("{} + {}", i, i + 1);
            let tree = parse_input(&g, &t, &input);
            assert!(!tree.children.is_empty());
        }));
    }
    for _ in 0..3 {
        let g = Arc::clone(&id_g);
        let t = Arc::clone(&id_t);
        handles.push(thread::spawn(move || {
            let tree = parse_input(&g, &t, "foo");
            assert!(count_nodes(&tree) >= 1);
        }));
    }

    for h in handles {
        h.join().expect("no panic in thread");
    }
}

// ===========================================================================
// 3. Concurrency cap initialization
// ===========================================================================

#[test]
fn init_caps_is_idempotent() {
    init_concurrency_caps();
    init_concurrency_caps();
    // No panic — proves idempotency
}

#[test]
fn init_rayon_global_once_idempotent() {
    let r1 = init_rayon_global_once(2);
    let r2 = init_rayon_global_once(8);
    assert!(r1.is_ok());
    assert_eq!(r1, r2);
}

#[test]
fn init_rayon_global_normalizes_zero() {
    // 0 threads should not panic
    assert!(init_rayon_global_once(0).is_ok());
}

#[test]
fn caps_default_matches_documented_constants() {
    let caps = ConcurrencyCaps::default();
    assert_eq!(caps.rayon_threads, DEFAULT_RAYON_NUM_THREADS);
    assert_eq!(caps.tokio_worker_threads, DEFAULT_TOKIO_WORKER_THREADS);
}

// ===========================================================================
// 4. bounded_parallel_map correctness
// ===========================================================================

#[test]
fn bounded_map_preserves_all_elements() {
    let input: Vec<i32> = (0..100).collect();
    let mut result = bounded_parallel_map(input.clone(), 4, |x| x * 2);
    result.sort_unstable();
    let expected: Vec<i32> = input.iter().map(|x| x * 2).collect();
    assert_eq!(result, expected);
}

#[test]
fn bounded_map_empty_input() {
    let result: Vec<i32> = bounded_parallel_map(vec![], 4, |x: i32| x + 1);
    assert!(result.is_empty());
}

#[test]
fn bounded_map_single_element() {
    let result = bounded_parallel_map(vec![7], 4, |x| x * 3);
    assert_eq!(result, vec![21]);
}

#[test]
fn bounded_map_with_zero_concurrency() {
    let mut result = bounded_parallel_map(vec![3, 1, 2], 0, |x| x * 10);
    result.sort_unstable();
    assert_eq!(result, vec![10, 20, 30]);
}

#[test]
fn bounded_map_concurrency_exceeds_items() {
    let mut result = bounded_parallel_map(vec![5], 1000, |x| x + 1);
    result.sort_unstable();
    assert_eq!(result, vec![6]);
}

#[test]
fn bounded_map_captures_environment() {
    let multiplier = 7;
    let input: Vec<i32> = (1..=10).collect();
    let mut result = bounded_parallel_map(input, 4, |x| x * multiplier);
    result.sort_unstable();
    let expected: Vec<i32> = (1..=10).map(|x| x * 7).collect();
    assert_eq!(result, expected);
}

#[test]
fn bounded_map_type_transformation() {
    let input = vec![1, 2, 3];
    let mut result = bounded_parallel_map(input, 2, |x: i32| format!("v{x}"));
    result.sort();
    assert_eq!(result, vec!["v1", "v2", "v3"]);
}

// ===========================================================================
// 5. Race condition prevention
// ===========================================================================

#[test]
fn atomic_counter_under_concurrent_parsing() {
    let grammar = Arc::new(arithmetic_grammar());
    let table = Arc::new(build_table(&grammar));
    let counter = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(Barrier::new(8));

    let handles: Vec<_> = (0..8)
        .map(|i| {
            let g = Arc::clone(&grammar);
            let t = Arc::clone(&table);
            let c = Arc::clone(&counter);
            let b = Arc::clone(&barrier);
            thread::spawn(move || {
                b.wait();
                let input = format!("{}", i + 1);
                let _tree = parse_input(&g, &t, &input);
                c.fetch_add(1, Ordering::SeqCst);
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(counter.load(Ordering::SeqCst), 8);
}

#[test]
fn mutex_protected_result_collection() {
    let grammar = Arc::new(arithmetic_grammar());
    let table = Arc::new(build_table(&grammar));
    let results = Arc::new(Mutex::new(Vec::new()));

    let handles: Vec<_> = (0..6)
        .map(|i| {
            let g = Arc::clone(&grammar);
            let t = Arc::clone(&table);
            let r = Arc::clone(&results);
            thread::spawn(move || {
                let input = format!("{} + {}", i, i + 1);
                let tree = parse_input(&g, &t, &input);
                let n = count_nodes(&tree);
                r.lock().unwrap().push(n);
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let collected = results.lock().unwrap();
    assert_eq!(collected.len(), 6);
    assert!(collected.iter().all(|&n| n > 0));
}

#[test]
fn arc_subtree_shared_read_no_race() {
    let grammar = arithmetic_grammar();
    let table = build_table(&grammar);
    let tree = parse_input(&grammar, &table, "1 + 2 + 3");

    let handles: Vec<_> = (0..8)
        .map(|_| {
            let t = Arc::clone(&tree);
            thread::spawn(move || {
                let sym = t.node.symbol_id;
                let n = count_nodes(&t);
                (sym, n)
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    // All readers see identical data
    let first = &results[0];
    for r in &results[1..] {
        assert_eq!(r, first);
    }
}

// ===========================================================================
// 6. Thread pool sizing
// ===========================================================================

#[test]
fn normalized_concurrency_clamps_zero() {
    assert_eq!(normalized_concurrency(0), 1);
}

#[test]
fn normalized_concurrency_preserves_positive() {
    assert_eq!(normalized_concurrency(1), 1);
    assert_eq!(normalized_concurrency(16), 16);
    assert_eq!(normalized_concurrency(1024), 1024);
}

#[test]
fn partition_plan_small_workload() {
    let plan = ParallelPartitionPlan::for_item_count(4, 4);
    assert!(plan.chunk_size >= 1);
}

#[test]
fn partition_plan_large_workload_chunks_correctly() {
    let plan = ParallelPartitionPlan::for_item_count(100, 4);
    assert_eq!(plan.concurrency, 4);
    assert_eq!(plan.chunk_size, 25);
}

#[test]
fn partition_plan_zero_concurrency_normalizes() {
    let plan = ParallelPartitionPlan::for_item_count(10, 0);
    assert_eq!(plan.concurrency, 1);
}

#[test]
fn parse_positive_usize_or_default_cases() {
    assert_eq!(parse_positive_usize_or_default(None, 5), 5);
    assert_eq!(parse_positive_usize_or_default(Some("0"), 5), 5);
    assert_eq!(parse_positive_usize_or_default(Some(""), 5), 5);
    assert_eq!(parse_positive_usize_or_default(Some("abc"), 5), 5);
    assert_eq!(parse_positive_usize_or_default(Some("10"), 5), 10);
    assert_eq!(parse_positive_usize_or_default(Some("  8  "), 5), 8);
}

// ===========================================================================
// 7. Multiple parsers in parallel
// ===========================================================================

#[test]
fn multiple_independent_parsers_parallel() {
    let grammar = Arc::new(arithmetic_grammar());
    let table = Arc::new(build_table(&grammar));

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let g = Arc::clone(&grammar);
            let t = Arc::clone(&table);
            thread::spawn(move || {
                // Each thread creates its own parser instance
                let mut parser = GLRParser::new((*t).clone(), (*g).clone());
                let input = format!("{} + {}", i * 10, i * 10 + 1);
                let mut lexer = GLRLexer::new(&g, input.to_string()).unwrap();
                let tokens = lexer.tokenize_all();
                parser.reset();
                for tok in &tokens {
                    parser.process_token(tok.symbol_id, &tok.text, tok.byte_offset);
                }
                let total = tokens
                    .last()
                    .map(|t| t.byte_offset + t.text.len())
                    .unwrap_or(0);
                parser.process_eof(total);
                parser.finish().expect("parse succeeds")
            })
        })
        .collect();

    let trees: Vec<Arc<Subtree>> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    for tree in &trees {
        assert!(!tree.children.is_empty());
    }
}

#[test]
fn parallel_parsers_with_error_recovery() {
    let grammar = Arc::new(arithmetic_grammar());
    let table = Arc::new(build_table(&grammar));

    let inputs = vec!["1 + ", "+ 2", "", "1 + + 3"];
    let handles: Vec<_> = inputs
        .into_iter()
        .map(|input| {
            let g = Arc::clone(&grammar);
            let t = Arc::clone(&table);
            thread::spawn(move || {
                let config = ErrorRecoveryConfig::default();
                let mut parser = GLRParser::new((*t).clone(), (*g).clone());
                parser.enable_error_recovery(config);

                if let Ok(mut lexer) = GLRLexer::new(&g, input.to_string()) {
                    let tokens = lexer.tokenize_all();
                    parser.reset();
                    for tok in &tokens {
                        parser.process_token(tok.symbol_id, &tok.text, tok.byte_offset);
                    }
                    let total = tokens
                        .last()
                        .map(|t| t.byte_offset + t.text.len())
                        .unwrap_or(0);
                    parser.process_eof(total);
                }
                // No panic is the success criterion
                let _result = parser.finish();
            })
        })
        .collect();

    for h in handles {
        h.join().expect("error recovery thread should not panic");
    }
}

#[test]
fn bounded_parallel_map_with_parsing() {
    let grammar = Arc::new(arithmetic_grammar());
    let table = Arc::new(build_table(&grammar));

    let inputs: Vec<String> = (0..20).map(|i| format!("{} + {}", i, i + 1)).collect();

    let results = bounded_parallel_map(inputs, 4, |input: String| {
        let tree = parse_input(&grammar, &table, &input);
        count_nodes(&tree)
    });

    assert_eq!(results.len(), 20);
    assert!(results.iter().all(|&n| n > 0));
    // All same-shape expressions yield same node count
    let first = results[0];
    assert!(results.iter().all(|&n| n == first));
}

#[test]
fn many_threads_parse_then_read_shared_tree() {
    let grammar = arithmetic_grammar();
    let table = build_table(&grammar);
    let shared_tree = parse_input(&grammar, &table, "10 + 20 + 30");
    let expected_count = count_nodes(&shared_tree);

    let barrier = Arc::new(Barrier::new(10));
    let handles: Vec<_> = (0..10)
        .map(|_| {
            let t = Arc::clone(&shared_tree);
            let b = Arc::clone(&barrier);
            thread::spawn(move || {
                b.wait();
                count_nodes(&t)
            })
        })
        .collect();

    let counts: Vec<usize> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    for c in &counts {
        assert_eq!(*c, expected_count);
    }
}

#[test]
fn concurrent_parse_and_read_simultaneously() {
    let grammar = Arc::new(arithmetic_grammar());
    let table = Arc::new(build_table(&grammar));
    let shared_tree = parse_input(&grammar, &table, "1 + 2 + 3");

    let mut handles = Vec::new();

    // 4 reader threads
    for _ in 0..4 {
        let t = Arc::clone(&shared_tree);
        handles.push(thread::spawn(move || count_nodes(&t)));
    }

    // 4 parser threads
    for i in 0..4 {
        let g = Arc::clone(&grammar);
        let tbl = Arc::clone(&table);
        handles.push(thread::spawn(move || {
            let input = format!("{} + {}", i * 5, i * 5 + 1);
            let tree = parse_input(&g, &tbl, &input);
            count_nodes(&tree)
        }));
    }

    let results: Vec<usize> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert!(results.iter().all(|&c| c > 0));
    // Readers should all agree
    let reader_counts = &results[0..4];
    assert!(reader_counts.windows(2).all(|w| w[0] == w[1]));
}

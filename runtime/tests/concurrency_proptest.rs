#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

//! Property-based tests for concurrent safety in the adze runtime.
//!
//! Verifies that parsing, tree traversal, cloning, error handling, and shared
//! access all behave correctly under concurrent workloads. Uses proptest to
//! generate random inputs and thread configurations.

#![cfg(not(miri))]

mod common;

use adze::Spanned;
use adze::error_recovery::ErrorRecoveryConfig;
use adze::glr_lexer::GLRLexer;
use adze::glr_parser::GLRParser;
use adze::glr_tree_bridge::{GLRTree, subtree_to_tree};
use adze::pure_parser::{ParsedNode, Point};
use adze::subtree::{ChildEdge, Subtree, SubtreeNode};
use adze_glr_core::{FirstFollowSets, ParseTable, build_lr1_automaton};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
use proptest::prelude::*;
use smallvec::SmallVec;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;

// ---------------------------------------------------------------------------
// Grammar builders
// ---------------------------------------------------------------------------

/// Unambiguous left-recursive grammar: sum → sum '+' number | number
fn unambiguous_grammar() -> Grammar {
    let mut g = Grammar::new("unambiguous".to_string());

    let num = SymbolId(1);
    let plus = SymbolId(2);
    let sum = SymbolId(10);

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

    g.rules.entry(sum).or_default().push(Rule {
        lhs: sum,
        rhs: vec![Symbol::Terminal(num)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });
    g.rules.entry(sum).or_default().push(Rule {
        lhs: sum,
        rhs: vec![
            Symbol::NonTerminal(sum),
            Symbol::Terminal(plus),
            Symbol::Terminal(num),
        ],
        precedence: None,
        associativity: None,
        production_id: ProductionId(1),
        fields: vec![],
    });

    g.rule_names.insert(sum, "sum".into());
    g.set_start_symbol(sum);
    g
}

/// Ambiguous grammar: expr → expr '+' expr | number
fn ambiguous_grammar() -> Grammar {
    let mut g = Grammar::new("ambiguous".to_string());

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

    g.rule_names.insert(expr, "expr".into());
    g.set_start_symbol(expr);
    g
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_table(grammar: &Grammar) -> ParseTable {
    let ff = FirstFollowSets::compute(grammar).unwrap();
    build_lr1_automaton(grammar, &ff).expect("Failed to build parse table")
}

fn parse(grammar: &Grammar, table: &ParseTable, input: &str) -> Arc<Subtree> {
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

fn count_nodes(subtree: &Subtree) -> usize {
    1 + subtree
        .children
        .iter()
        .map(|e| count_nodes(&e.subtree))
        .sum::<usize>()
}

fn collect_symbol_ids(subtree: &Subtree) -> Vec<SymbolId> {
    let mut ids = vec![subtree.node.symbol_id];
    for edge in &subtree.children {
        ids.extend(collect_symbol_ids(&edge.subtree));
    }
    ids
}

fn collect_byte_ranges(subtree: &Subtree) -> Vec<std::ops::Range<usize>> {
    let mut ranges = vec![subtree.node.byte_range.clone()];
    for edge in &subtree.children {
        ranges.extend(collect_byte_ranges(&edge.subtree));
    }
    ranges
}

fn build_glr_tree(grammar: &Grammar, subtree: Arc<Subtree>, source: &str) -> GLRTree {
    subtree_to_tree(subtree, source.as_bytes().to_vec(), grammar.clone())
}

fn arith_expr_from_parts(nums: &[u32]) -> String {
    if nums.is_empty() {
        return "0".to_string();
    }
    nums.iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(" + ")
}

/// Wrapper to allow sending test-constructed ParsedNode across threads.
///
/// # Safety
///
/// Only safe for nodes where `language` is `None` (the raw pointer field is
/// null). All nodes built by `make_parsed_node` satisfy this.
struct SendNode(ParsedNode);
// SAFETY: Our test-constructed nodes always have language: None (null ptr).
unsafe impl Send for SendNode {}
unsafe impl Sync for SendNode {}

impl SendNode {
    fn inner(&self) -> &ParsedNode {
        &self.0
    }
}

/// Build a ParsedNode without a language pointer (safe to clone across threads
/// since we set language to None).
fn make_parsed_node(symbol: u16, start: usize, end: usize) -> ParsedNode {
    let mut uninit = MaybeUninit::<ParsedNode>::uninit();
    let ptr = uninit.as_mut_ptr();
    unsafe {
        std::ptr::write_bytes(ptr, 0, 1);
        std::ptr::addr_of_mut!((*ptr).symbol).write(symbol);
        std::ptr::addr_of_mut!((*ptr).children).write(vec![]);
        std::ptr::addr_of_mut!((*ptr).start_byte).write(start);
        std::ptr::addr_of_mut!((*ptr).end_byte).write(end);
        std::ptr::addr_of_mut!((*ptr).start_point).write(Point {
            row: 0,
            column: start as u32,
        });
        std::ptr::addr_of_mut!((*ptr).end_point).write(Point {
            row: 0,
            column: end as u32,
        });
        std::ptr::addr_of_mut!((*ptr).is_extra).write(false);
        std::ptr::addr_of_mut!((*ptr).is_error).write(false);
        std::ptr::addr_of_mut!((*ptr).is_missing).write(false);
        std::ptr::addr_of_mut!((*ptr).is_named).write(true);
        std::ptr::addr_of_mut!((*ptr).field_id).write(None);
        uninit.assume_init()
    }
}

fn make_parsed_node_with_children(
    symbol: u16,
    start: usize,
    end: usize,
    children: Vec<ParsedNode>,
) -> ParsedNode {
    let mut node = make_parsed_node(symbol, start, end);
    node.children = children;
    node
}

// ---------------------------------------------------------------------------
// Proptest strategies
// ---------------------------------------------------------------------------

fn arith_input_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(0u32..1000, 1..=5).prop_map(|nums| arith_expr_from_parts(&nums))
}

fn thread_count_strategy() -> impl Strategy<Value = usize> {
    prop_oneof![Just(2), Just(4), Just(8)]
}

// ===========================================================================
// Tests
// ===========================================================================

// ---------------------------------------------------------------------------
// 1. Multiple parsers in parallel threads produce identical results
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn parallel_parsers_identical_output(input in arith_input_strategy()) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));
        let num_threads = 4;
        let barrier = Arc::new(Barrier::new(num_threads));

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let g = Arc::clone(&grammar);
                let t = Arc::clone(&table);
                let b = Arc::clone(&barrier);
                let inp = input.clone();
                thread::spawn(move || {
                    b.wait();
                    collect_symbol_ids(&parse(&g, &t, &inp))
                })
            })
            .collect();

        let results: Vec<Vec<SymbolId>> =
            handles.into_iter().map(|h| h.join().unwrap()).collect();
        for r in &results[1..] {
            prop_assert_eq!(r, &results[0]);
        }
    }
}

// ---------------------------------------------------------------------------
// 2. Shared immutable Arc<Subtree> accessed from many threads
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn shared_immutable_subtree_refs(input in arith_input_strategy()) {
        let grammar = unambiguous_grammar();
        let table = build_table(&grammar);
        let tree = parse(&grammar, &table, &input);
        let num_threads = 8;
        let barrier = Arc::new(Barrier::new(num_threads));

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let t = Arc::clone(&tree);
                let b = Arc::clone(&barrier);
                thread::spawn(move || {
                    b.wait();
                    (count_nodes(&t), collect_symbol_ids(&t))
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        for r in &results[1..] {
            prop_assert_eq!(&r.0, &results[0].0);
            prop_assert_eq!(&r.1, &results[0].1);
        }
    }
}

// ---------------------------------------------------------------------------
// 3. Independent ParsedNode clones in threads
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn parsed_node_clones_independent(
        sym in 1u16..100,
        start in 0usize..100,
        len in 1usize..50,
    ) {
        let end = start + len;
        let node = make_parsed_node(sym, start, end);

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let sn = SendNode(node.clone());
                thread::spawn(move || {
                    let clone = sn.inner();
                    assert_eq!(clone.symbol(), sym);
                    assert_eq!(clone.start_byte(), start);
                    assert_eq!(clone.end_byte(), end);
                    assert!(!clone.is_error());
                    assert!(clone.is_named());
                    (clone.symbol(), clone.start_byte(), clone.end_byte())
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        for r in &results {
            prop_assert_eq!(r.0, sym);
            prop_assert_eq!(r.1, start);
            prop_assert_eq!(r.2, end);
        }
    }
}

// ---------------------------------------------------------------------------
// 4. Thread-local error states don't leak between threads
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn thread_local_error_states(
        good_nums in prop::collection::vec(1u32..100, 1..=3),
    ) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));
        let good_input = arith_expr_from_parts(&good_nums);
        let bad_input = "+ +".to_string();

        let barrier = Arc::new(Barrier::new(2));

        // Thread A: parse valid input
        let g1 = Arc::clone(&grammar);
        let t1 = Arc::clone(&table);
        let b1 = Arc::clone(&barrier);
        let gi = good_input.clone();
        let ha = thread::spawn(move || {
            b1.wait();
            let tree = parse(&g1, &t1, &gi);
            // Must succeed without error nodes
            assert!(!tree.node.is_error);
            collect_symbol_ids(&tree)
        });

        // Thread B: parse invalid input (with error recovery)
        let g2 = Arc::clone(&grammar);
        let t2 = Arc::clone(&table);
        let b2 = Arc::clone(&barrier);
        let bi = bad_input;
        let hb = thread::spawn(move || {
            b2.wait();
            let mut parser = GLRParser::new((*t2).clone(), (*g2).clone());
            parser.enable_error_recovery(ErrorRecoveryConfig::default());
            let lexer_result = GLRLexer::new(&g2, bi);
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
            // Whether it succeeds or fails, thread B's state is isolated
            let _result = parser.finish();
        });

        let result_a = ha.join().expect("good parse thread must not panic");
        hb.join().expect("bad parse thread must not panic");

        // Thread A's result must match single-threaded reference
        let ref_syms = collect_symbol_ids(&parse(&grammar, &table, &good_input));
        prop_assert_eq!(&result_a, &ref_syms);
    }
}

// ---------------------------------------------------------------------------
// 5. Concurrent read access to shared Grammar and ParseTable
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn concurrent_read_grammar_table(
        thread_count in thread_count_strategy(),
    ) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));
        let barrier = Arc::new(Barrier::new(thread_count));

        let handles: Vec<_> = (0..thread_count)
            .map(|_| {
                let g = Arc::clone(&grammar);
                let t = Arc::clone(&table);
                let b = Arc::clone(&barrier);
                thread::spawn(move || {
                    b.wait();
                    // Read-only access to grammar metadata
                    let token_count = g.tokens.len();
                    let rule_count = g.rules.len();
                    let state_count = t.state_count;
                    let sym_count = t.symbol_count;
                    (token_count, rule_count, state_count, sym_count)
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        for r in &results[1..] {
            prop_assert_eq!(r, &results[0]);
        }
    }
}

// ---------------------------------------------------------------------------
// 6. No data races: atomic counter incremented from parsing threads
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn no_data_races_atomic_counter(
        inputs in prop::collection::vec(arith_input_strategy(), 2..=6),
    ) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));
        let counter = Arc::new(AtomicUsize::new(0));
        let barrier = Arc::new(Barrier::new(inputs.len()));

        let handles: Vec<_> = inputs
            .iter()
            .map(|inp| {
                let g = Arc::clone(&grammar);
                let t = Arc::clone(&table);
                let c = Arc::clone(&counter);
                let b = Arc::clone(&barrier);
                let input = inp.clone();
                thread::spawn(move || {
                    b.wait();
                    let tree = parse(&g, &t, &input);
                    let nc = count_nodes(&tree);
                    c.fetch_add(nc, Ordering::SeqCst);
                    nc
                })
            })
            .collect();

        let individual_counts: Vec<usize> =
            handles.into_iter().map(|h| h.join().unwrap()).collect();
        let expected_total: usize = individual_counts.iter().sum();
        prop_assert_eq!(counter.load(Ordering::SeqCst), expected_total);
    }
}

// ---------------------------------------------------------------------------
// 7. Spanned<T> clones are independent across threads
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn spanned_clones_independent(
        val in 0i64..1000,
        start in 0usize..100,
        len in 1usize..50,
    ) {
        let end = start + len;
        let spanned = Spanned { value: val, span: (start, end) };

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let s = spanned.clone();
                thread::spawn(move || {
                    assert_eq!(*s, val);
                    assert_eq!(s.span, (start, end));
                    (s.value, s.span)
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        for r in &results {
            prop_assert_eq!(r.0, val);
            prop_assert_eq!(r.1, (start, end));
        }
    }
}

// ---------------------------------------------------------------------------
// 8. Concurrent GLRTree construction from shared subtree
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn concurrent_glr_tree_from_shared_subtree(input in arith_input_strategy()) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));
        let tree = parse(&grammar, &table, &input);
        let num_threads = 4;
        let barrier = Arc::new(Barrier::new(num_threads));

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let g = Arc::clone(&grammar);
                let t = Arc::clone(&tree);
                let b = Arc::clone(&barrier);
                let src = input.clone();
                thread::spawn(move || {
                    b.wait();
                    let glr_tree = build_glr_tree(&g, t, &src);
                    glr_tree.root_node().to_sexp()
                })
            })
            .collect();

        let results: Vec<String> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        for r in &results[1..] {
            prop_assert_eq!(r, &results[0]);
        }
    }
}

// ---------------------------------------------------------------------------
// 9. Concurrent parse + read of previously built tree
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn concurrent_parse_and_read(
        input_a in arith_input_strategy(),
        input_b in arith_input_strategy(),
    ) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));

        // Pre-parse tree A
        let tree_a = parse(&grammar, &table, &input_a);
        let ref_syms_a = collect_symbol_ids(&tree_a);

        let barrier = Arc::new(Barrier::new(2));

        // Thread 1: read tree A
        let ta = Arc::clone(&tree_a);
        let b1 = Arc::clone(&barrier);
        let reader = thread::spawn(move || {
            b1.wait();
            collect_symbol_ids(&ta)
        });

        // Thread 2: parse input B
        let g2 = Arc::clone(&grammar);
        let t2 = Arc::clone(&table);
        let b2 = Arc::clone(&barrier);
        let ib = input_b.clone();
        let parser_h = thread::spawn(move || {
            b2.wait();
            collect_symbol_ids(&parse(&g2, &t2, &ib))
        });

        let read_result = reader.join().unwrap();
        let _parse_result = parser_h.join().unwrap();

        // Reader must see the original tree unchanged
        prop_assert_eq!(&read_result, &ref_syms_a);
    }
}

// ---------------------------------------------------------------------------
// 10. Multiple parsed node trees cloned into threads
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn parsed_node_tree_clones_in_threads(
        child_count in 1usize..5,
        sym in 1u16..50,
    ) {
        let children: Vec<ParsedNode> = (0..child_count)
            .map(|i| make_parsed_node(sym + i as u16, i * 10, i * 10 + 5))
            .collect();
        let root = make_parsed_node_with_children(sym, 0, child_count * 10, children);

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let sn = SendNode(root.clone());
                thread::spawn(move || {
                    let clone = sn.inner();
                    assert_eq!(clone.child_count(), child_count);
                    for i in 0..child_count {
                        let child = clone.child(i).unwrap();
                        assert_eq!(child.symbol(), sym + i as u16);
                    }
                    clone.child_count()
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        for r in &results {
            prop_assert_eq!(*r, child_count);
        }
    }
}

// ---------------------------------------------------------------------------
// 11. Concurrent Subtree deep-clone consistency
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn subtree_deep_clone_concurrent(input in arith_input_strategy()) {
        let grammar = unambiguous_grammar();
        let table = build_table(&grammar);
        let tree = parse(&grammar, &table, &input);

        // Each thread gets its own deep clone of the Arc<Subtree>
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let deep = Arc::new((*tree).clone());
                thread::spawn(move || {
                    let syms = collect_symbol_ids(&deep);
                    let count = count_nodes(&deep);
                    (syms, count)
                })
            })
            .collect();

        let ref_syms = collect_symbol_ids(&tree);
        let ref_count = count_nodes(&tree);

        for h in handles {
            let (syms, count) = h.join().unwrap();
            prop_assert_eq!(&syms, &ref_syms);
            prop_assert_eq!(count, ref_count);
        }
    }
}

// ---------------------------------------------------------------------------
// 12. Mutex-guarded results collected from concurrent parsers
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn mutex_collected_results(
        inputs in prop::collection::vec(arith_input_strategy(), 2..=5),
    ) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));
        let results = Arc::new(Mutex::new(Vec::new()));
        let barrier = Arc::new(Barrier::new(inputs.len()));

        let handles: Vec<_> = inputs
            .iter()
            .enumerate()
            .map(|(idx, inp)| {
                let g = Arc::clone(&grammar);
                let t = Arc::clone(&table);
                let r = Arc::clone(&results);
                let b = Arc::clone(&barrier);
                let input = inp.clone();
                thread::spawn(move || {
                    b.wait();
                    let tree = parse(&g, &t, &input);
                    let syms = collect_symbol_ids(&tree);
                    r.lock().unwrap().push((idx, syms));
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        let collected = results.lock().unwrap();
        prop_assert_eq!(collected.len(), inputs.len());

        // Verify each result matches single-threaded reference
        for (idx, syms) in collected.iter() {
            let ref_syms = collect_symbol_ids(&parse(&grammar, &table, &inputs[*idx]));
            prop_assert_eq!(syms, &ref_syms);
        }
    }
}

// ---------------------------------------------------------------------------
// 13. Concurrent byte range collection consistency
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn concurrent_byte_ranges_match(input in arith_input_strategy()) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));
        let ref_ranges = collect_byte_ranges(&parse(&grammar, &table, &input));
        let barrier = Arc::new(Barrier::new(4));

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let g = Arc::clone(&grammar);
                let t = Arc::clone(&table);
                let b = Arc::clone(&barrier);
                let inp = input.clone();
                thread::spawn(move || {
                    b.wait();
                    collect_byte_ranges(&parse(&g, &t, &inp))
                })
            })
            .collect();

        for h in handles {
            let ranges = h.join().unwrap();
            prop_assert_eq!(&ranges, &ref_ranges);
        }
    }
}

// ---------------------------------------------------------------------------
// 14. Concurrent s-expression output deterministic
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn concurrent_sexp_deterministic(input in arith_input_strategy()) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));
        let tree = parse(&grammar, &table, &input);

        let ref_sexp = build_glr_tree(&grammar, Arc::clone(&tree), &input)
            .root_node()
            .to_sexp();

        let barrier = Arc::new(Barrier::new(4));

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let g = Arc::clone(&grammar);
                let t = Arc::clone(&tree);
                let b = Arc::clone(&barrier);
                let src = input.clone();
                thread::spawn(move || {
                    b.wait();
                    build_glr_tree(&g, t, &src).root_node().to_sexp()
                })
            })
            .collect();

        for h in handles {
            let sexp = h.join().unwrap();
            prop_assert_eq!(&sexp, &ref_sexp);
        }
    }
}

// ---------------------------------------------------------------------------
// 15. Error recovery in parallel: each thread's errors are isolated
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn error_recovery_isolation(
        good_count in 1usize..4,
    ) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));
        let barrier = Arc::new(Barrier::new(good_count + 1));

        let mut handles = Vec::new();

        // Good parse threads
        for i in 0..good_count {
            let g = Arc::clone(&grammar);
            let t = Arc::clone(&table);
            let b = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                b.wait();
                let input = format!("{}", i + 1);
                let tree = parse(&g, &t, &input);
                assert!(!tree.node.is_error, "good input must not produce error");
            }));
        }

        // Bad parse thread
        let g = Arc::clone(&grammar);
        let t = Arc::clone(&table);
        let b = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            b.wait();
            let mut parser = GLRParser::new((*t).clone(), (*g).clone());
            parser.enable_error_recovery(ErrorRecoveryConfig::default());
            if let Ok(mut lexer) = GLRLexer::new(&g, "+++".to_string()) {
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
            let _result = parser.finish();
        }));

        for h in handles {
            h.join().expect("no thread should panic");
        }
    }
}

// ---------------------------------------------------------------------------
// 16. Concurrent node_count matches single-threaded reference
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn node_count_matches_reference(input in arith_input_strategy()) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));
        let ref_count = count_nodes(&parse(&grammar, &table, &input));
        let barrier = Arc::new(Barrier::new(4));

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let g = Arc::clone(&grammar);
                let t = Arc::clone(&table);
                let b = Arc::clone(&barrier);
                let inp = input.clone();
                thread::spawn(move || {
                    b.wait();
                    count_nodes(&parse(&g, &t, &inp))
                })
            })
            .collect();

        for h in handles {
            prop_assert_eq!(h.join().unwrap(), ref_count);
        }
    }
}

// ---------------------------------------------------------------------------
// 17. Rapid parser create/parse/drop across threads
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    #[test]
    fn rapid_create_parse_drop(iterations in 3u32..10) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));
        let barrier = Arc::new(Barrier::new(4));

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let g = Arc::clone(&grammar);
                let t = Arc::clone(&table);
                let b = Arc::clone(&barrier);
                thread::spawn(move || {
                    b.wait();
                    for _ in 0..iterations {
                        let _tree = parse(&g, &t, "1 + 2");
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().expect("rapid create/parse/drop must not panic");
        }
    }
}

// ---------------------------------------------------------------------------
// 18. Thread-scaling: parse produces same result at 2, 4, 8 threads
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn thread_scaling_deterministic(
        input in arith_input_strategy(),
        thread_count in thread_count_strategy(),
    ) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));
        let ref_syms = collect_symbol_ids(&parse(&grammar, &table, &input));
        let barrier = Arc::new(Barrier::new(thread_count));

        let handles: Vec<_> = (0..thread_count)
            .map(|_| {
                let g = Arc::clone(&grammar);
                let t = Arc::clone(&table);
                let b = Arc::clone(&barrier);
                let inp = input.clone();
                thread::spawn(move || {
                    b.wait();
                    collect_symbol_ids(&parse(&g, &t, &inp))
                })
            })
            .collect();

        for h in handles {
            prop_assert_eq!(&h.join().unwrap(), &ref_syms);
        }
    }
}

// ---------------------------------------------------------------------------
// 19. Shared SubtreeNode fields read concurrently
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn shared_subtree_node_reads(
        sym_val in 1u16..100,
        range_start in 0usize..50,
        range_len in 1usize..50,
    ) {
        let sym = SymbolId(sym_val);
        let range_end = range_start + range_len;
        let node = SubtreeNode {
            symbol_id: sym,
            is_error: false,
            byte_range: range_start..range_end,
        };
        let subtree = Arc::new(Subtree {
            node,
            dynamic_prec: 0,
            children: vec![],
            alternatives: SmallVec::new(),
        });

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let s = Arc::clone(&subtree);
                thread::spawn(move || {
                    assert_eq!(s.node.symbol_id, sym);
                    assert!(!s.node.is_error);
                    assert_eq!(s.node.byte_range, range_start..range_end);
                    s.node.symbol_id
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        for r in &results {
            prop_assert_eq!(*r, sym);
        }
    }
}

// ---------------------------------------------------------------------------
// 20. Concurrent traversal with cursor is deterministic
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn concurrent_cursor_deterministic(input in arith_input_strategy()) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));
        let tree = parse(&grammar, &table, &input);

        fn cursor_dfs(glr_tree: &GLRTree) -> Vec<String> {
            let mut kinds = Vec::new();
            let mut cursor = glr_tree.root_node().walk();
            let mut reached_root = false;
            loop {
                kinds.push(cursor.node().kind().to_string());
                if cursor.goto_first_child() { continue; }
                if cursor.goto_next_sibling() { continue; }
                loop {
                    if !cursor.goto_parent() { reached_root = true; break; }
                    if cursor.goto_next_sibling() { break; }
                }
                if reached_root { break; }
            }
            kinds
        }

        let barrier = Arc::new(Barrier::new(4));
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let g = Arc::clone(&grammar);
                let t = Arc::clone(&tree);
                let b = Arc::clone(&barrier);
                let src = input.clone();
                thread::spawn(move || {
                    b.wait();
                    let glr_tree = build_glr_tree(&g, t, &src);
                    cursor_dfs(&glr_tree)
                })
            })
            .collect();

        let results: Vec<Vec<String>> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        for r in &results[1..] {
            prop_assert_eq!(r, &results[0]);
        }
    }
}

// ---------------------------------------------------------------------------
// 21. Different inputs produce different trees (no cross-contamination)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn different_inputs_different_trees(
        nums_a in prop::collection::vec(1u32..100, 1..=3),
        nums_b in prop::collection::vec(100u32..200, 2..=4),
    ) {
        let input_a = arith_expr_from_parts(&nums_a);
        let input_b = arith_expr_from_parts(&nums_b);

        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));
        let barrier = Arc::new(Barrier::new(2));

        let g1 = Arc::clone(&grammar);
        let t1 = Arc::clone(&table);
        let b1 = Arc::clone(&barrier);
        let ia = input_a.clone();
        let ha = thread::spawn(move || {
            b1.wait();
            collect_symbol_ids(&parse(&g1, &t1, &ia))
        });

        let g2 = Arc::clone(&grammar);
        let t2 = Arc::clone(&table);
        let b2 = Arc::clone(&barrier);
        let ib = input_b.clone();
        let hb = thread::spawn(move || {
            b2.wait();
            collect_symbol_ids(&parse(&g2, &t2, &ib))
        });

        let ref_a = collect_symbol_ids(&parse(&grammar, &table, &input_a));
        let ref_b = collect_symbol_ids(&parse(&grammar, &table, &input_b));

        prop_assert_eq!(&ha.join().unwrap(), &ref_a);
        prop_assert_eq!(&hb.join().unwrap(), &ref_b);
    }
}

// ---------------------------------------------------------------------------
// 22. Ambiguous grammar: no panics under concurrent parse
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn ambiguous_grammar_no_panics(
        nums in prop::collection::vec(1u32..100, 1..=4),
    ) {
        let grammar = Arc::new(ambiguous_grammar());
        let table = Arc::new(build_table(&grammar));
        let input = arith_expr_from_parts(&nums);
        let barrier = Arc::new(Barrier::new(4));

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let g = Arc::clone(&grammar);
                let t = Arc::clone(&table);
                let b = Arc::clone(&barrier);
                let inp = input.clone();
                thread::spawn(move || {
                    b.wait();
                    let tree = parse(&g, &t, &inp);
                    assert!(count_nodes(&tree) >= 1);
                })
            })
            .collect();

        for h in handles {
            h.join().expect("ambiguous parse must not panic");
        }
    }
}

// ---------------------------------------------------------------------------
// 23. Concurrent has_error checks on ParsedNode clones
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn parsed_node_has_error_concurrent(
        sym in 1u16..50,
        child_count in 0usize..4,
    ) {
        let children: Vec<ParsedNode> = (0..child_count)
            .map(|i| make_parsed_node(sym + 1 + i as u16, i * 5, i * 5 + 3))
            .collect();
        let root = make_parsed_node_with_children(sym, 0, child_count * 5 + 3, children);

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let sn = SendNode(root.clone());
                thread::spawn(move || {
                    let clone = sn.inner();
                    assert!(!clone.has_error());
                    assert!(!clone.is_error());
                    clone.has_error()
                })
            })
            .collect();

        let results: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        for r in &results {
            prop_assert!(!r);
        }
    }
}

// ---------------------------------------------------------------------------
// 24. Concurrent child access on ParsedNode clones
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn parsed_node_concurrent_child_access(
        sym in 1u16..50,
        depth in 1usize..4,
    ) {
        // Build a chain: root -> child -> grandchild -> ...
        let mut current = make_parsed_node(sym + depth as u16, depth * 10, depth * 10 + 5);
        for d in (0..depth).rev() {
            current = make_parsed_node_with_children(
                sym + d as u16,
                d * 10,
                (depth + 1) * 10,
                vec![current],
            );
        }
        let root = current;

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let sn = SendNode(root.clone());
                thread::spawn(move || {
                    let mut node = sn.inner();
                    let mut depth_found = 0;
                    while node.child_count() > 0 {
                        node = node.child(0).unwrap();
                        depth_found += 1;
                    }
                    depth_found
                })
            })
            .collect();

        let results: Vec<usize> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        for r in &results {
            prop_assert_eq!(*r, depth);
        }
    }
}

// ---------------------------------------------------------------------------
// 25. Concurrent Spanned deref correctness
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn spanned_deref_concurrent(
        values in prop::collection::vec(0i32..1000, 2..=6),
    ) {
        let spanned_vec: Vec<Spanned<i32>> = values
            .iter()
            .enumerate()
            .map(|(i, &v)| Spanned { value: v, span: (i, i + 1) })
            .collect();

        let handles: Vec<_> = spanned_vec
            .iter()
            .map(|s| {
                let clone = s.clone();
                thread::spawn(move || *clone)
            })
            .collect();

        let results: Vec<i32> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        prop_assert_eq!(&results, &values);
    }
}

// ---------------------------------------------------------------------------
// 26. Sequential vs concurrent parsing equivalence
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn sequential_vs_concurrent_equivalence(
        inputs in prop::collection::vec(arith_input_strategy(), 2..=5),
    ) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));

        let sequential: Vec<Vec<SymbolId>> = inputs
            .iter()
            .map(|inp| collect_symbol_ids(&parse(&grammar, &table, inp)))
            .collect();

        let barrier = Arc::new(Barrier::new(inputs.len()));
        let handles: Vec<_> = inputs
            .iter()
            .map(|inp| {
                let g = Arc::clone(&grammar);
                let t = Arc::clone(&table);
                let b = Arc::clone(&barrier);
                let input = inp.clone();
                thread::spawn(move || {
                    b.wait();
                    collect_symbol_ids(&parse(&g, &t, &input))
                })
            })
            .collect();

        let concurrent: Vec<Vec<SymbolId>> =
            handles.into_iter().map(|h| h.join().unwrap()).collect();

        for (i, (seq, conc)) in sequential.iter().zip(&concurrent).enumerate() {
            prop_assert_eq!(seq, conc, "input {} differs", i);
        }
    }
}

// ---------------------------------------------------------------------------
// 27. ChildEdge construction is safe across threads
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn child_edge_concurrent_construction(
        sym_val in 1u16..100,
        field_id in 0u16..100,
    ) {
        let sym = SymbolId(sym_val);
        let subtree = Arc::new(Subtree {
            node: SubtreeNode {
                symbol_id: sym,
                is_error: false,
                byte_range: 0..10,
            },
            dynamic_prec: 0,
            children: vec![],
            alternatives: SmallVec::new(),
        });

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let s = Arc::clone(&subtree);
                let fid = field_id;
                thread::spawn(move || {
                    let edge = ChildEdge::new(s, fid);
                    assert_eq!(edge.subtree.node.symbol_id, sym);
                    assert_eq!(edge.field_id, fid);
                    edge.field_id
                })
            })
            .collect();

        let results: Vec<u16> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        for r in &results {
            prop_assert_eq!(*r, field_id);
        }
    }
}

// ---------------------------------------------------------------------------
// 28. GLRTree root_node metadata consistent across threads
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn glr_tree_root_metadata_concurrent(input in arith_input_strategy()) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));
        let tree = parse(&grammar, &table, &input);
        let barrier = Arc::new(Barrier::new(4));

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let g = Arc::clone(&grammar);
                let t = Arc::clone(&tree);
                let b = Arc::clone(&barrier);
                let src = input.clone();
                thread::spawn(move || {
                    b.wait();
                    let glr_tree = build_glr_tree(&g, t, &src);
                    let root = glr_tree.root_node();
                    (
                        root.kind().to_string(),
                        root.start_byte(),
                        root.end_byte(),
                        root.child_count(),
                    )
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        for r in &results[1..] {
            prop_assert_eq!(r, &results[0]);
        }
    }
}

// ---------------------------------------------------------------------------
// 29. ParsedNode utf8_text from shared source is safe
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn parsed_node_utf8_text_concurrent(
        text in "[a-z]{3,10}",
    ) {
        let source = text.as_bytes().to_vec();
        let node = make_parsed_node(1, 0, text.len());
        let src = Arc::new(source);

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let sn = SendNode(node.clone());
                let s = Arc::clone(&src);
                thread::spawn(move || sn.inner().utf8_text(&s).unwrap().to_string())
            })
            .collect();

        let results: Vec<String> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        for r in &results {
            prop_assert_eq!(r, &text);
        }
    }
}

// ---------------------------------------------------------------------------
// 30. Send+Sync static assertions under proptest
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1))]

    #[test]
    fn static_send_sync_assertions(_dummy in 0u8..1) {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<Grammar>();
        assert_sync::<Grammar>();
        assert_send::<ParseTable>();
        assert_sync::<ParseTable>();
        assert_send::<Subtree>();
        assert_sync::<Subtree>();
        assert_send::<Arc<Subtree>>();
        assert_sync::<Arc<Subtree>>();
        assert_send::<SubtreeNode>();
        assert_sync::<SubtreeNode>();
        assert_send::<ChildEdge>();
        assert_sync::<ChildEdge>();
        assert_send::<ErrorRecoveryConfig>();
        assert_sync::<ErrorRecoveryConfig>();
        assert_send::<Spanned<i32>>();
        assert_sync::<Spanned<i32>>();
        assert_send::<Spanned<String>>();
        assert_sync::<Spanned<String>>();
    }
}

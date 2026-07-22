//! Property-based concurrent parsing tests for the adze runtime.
//!
//! Uses proptest to generate random valid inputs and verify parsing invariants
//! under concurrent access: determinism, thread-safety, isolation, and
//! consistency across varying thread counts.
//!
//! Uses an unambiguous left-recursive grammar (`sum → sum '+' number | number`)
//! to ensure deterministic tree structure across parse calls. The ambiguous
//! `expr → expr '+' expr` grammar is used only for safety/no-panic tests.

#![cfg(not(miri))]

mod common;

use adze::glr_lexer::GLRLexer;
use adze::glr_parser::GLRParser;
use adze::glr_tree_bridge::{GLRTree, subtree_to_tree};
use adze::subtree::Subtree;
use adze_glr_core::{FirstFollowSets, ParseTable, build_lr1_automaton};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
use proptest::prelude::*;
use std::sync::{Arc, Barrier};
use std::thread;

// ---------------------------------------------------------------------------
// Grammar builders
// ---------------------------------------------------------------------------

/// Unambiguous left-recursive grammar: sum → sum '+' number | number
///
/// Unlike `expr → expr '+' expr`, this grammar has exactly one parse tree
/// for any input, making it suitable for determinism comparisons.
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

    // sum → number
    g.rules.entry(sum).or_default().push(Rule {
        lhs: sum,
        rhs: vec![Symbol::Terminal(num)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });
    // sum → sum '+' number  (left-recursive, unambiguous)
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

/// Build a GLRTree from a subtree for s-expression serialization.
fn build_glr_tree(grammar: &Grammar, subtree: Arc<Subtree>, source: &str) -> GLRTree {
    subtree_to_tree(subtree, source.as_bytes().to_vec(), grammar.clone())
}

/// Generate a valid arithmetic expression string from parts.
fn arith_expr_from_parts(nums: &[u32]) -> String {
    if nums.is_empty() {
        return "0".to_string();
    }
    nums.iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(" + ")
}

// ---------------------------------------------------------------------------
// Proptest strategies
// ---------------------------------------------------------------------------

/// Strategy producing valid arithmetic expressions like "1 + 23 + 4"
fn arith_input_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(0u32..1000, 1..=5).prop_map(|nums| arith_expr_from_parts(&nums))
}

/// Strategy producing valid identifiers
fn ident_input_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z_][a-zA-Z0-9_]{0,15}"
}

/// Strategy producing thread counts from the allowed set
fn thread_count_strategy() -> impl Strategy<Value = usize> {
    prop_oneof![Just(1), Just(2), Just(4), Just(8)]
}

// ---------------------------------------------------------------------------
// 1. Multiple threads parsing same input produce identical tree structure
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn same_input_same_tree_across_threads(input in arith_input_strategy()) {
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
                    let tree = parse(&g, &t, &inp);
                    collect_symbol_ids(&tree)
                })
            })
            .collect();

        let results: Vec<Vec<SymbolId>> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let first = &results[0];
        for r in &results[1..] {
            prop_assert_eq!(r, first, "all threads must produce identical tree structure");
        }
    }
}

// ---------------------------------------------------------------------------
// 2. Multiple threads parsing different inputs don't interfere
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn different_inputs_no_interference(
        input_a in arith_input_strategy(),
        input_b in arith_input_strategy(),
    ) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));

        // Parse each input single-threaded for reference
        let ref_a = collect_symbol_ids(&parse(&grammar, &table, &input_a));
        let ref_b = collect_symbol_ids(&parse(&grammar, &table, &input_b));

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

        let result_a = ha.join().unwrap();
        let result_b = hb.join().unwrap();

        prop_assert_eq!(&result_a, &ref_a, "thread A result differs from single-threaded");
        prop_assert_eq!(&result_b, &ref_b, "thread B result differs from single-threaded");
    }
}

// ---------------------------------------------------------------------------
// 3. Parser instances are thread-safe (Send + Sync static assertions)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn grammar_and_table_are_send_sync(_dummy in 0u8..1) {
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
    }
}

// ---------------------------------------------------------------------------
// 4. Concurrent tree traversal produces consistent results
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn concurrent_traversal_consistent(input in arith_input_strategy()) {
        let grammar = unambiguous_grammar();
        let table = build_table(&grammar);
        let tree = parse(&grammar, &table, &input);
        let num_threads = 4;
        let barrier = Arc::new(Barrier::new(num_threads));

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let t = Arc::clone(&tree);
                let b = Arc::clone(&barrier);
                thread::spawn(move || {
                    b.wait();
                    let node_count = count_nodes(&t);
                    let symbols = collect_symbol_ids(&t);
                    let ranges = collect_byte_ranges(&t);
                    (node_count, symbols, ranges)
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let (first_count, first_syms, first_ranges) = &results[0];
        for (count, syms, ranges) in &results[1..] {
            prop_assert_eq!(count, first_count);
            prop_assert_eq!(syms, first_syms);
            prop_assert_eq!(ranges, first_ranges);
        }
    }
}

// ---------------------------------------------------------------------------
// 5. Concurrent serialization produces deterministic output
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn concurrent_serialization_deterministic(input in arith_input_strategy()) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));
        let tree = parse(&grammar, &table, &input);
        let num_threads = 4;
        let barrier = Arc::new(Barrier::new(num_threads));

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let t = Arc::clone(&tree);
                let g = Arc::clone(&grammar);
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
        let first = &results[0];
        for (i, r) in results.iter().enumerate().skip(1) {
            prop_assert_eq!(r, first, "thread {} produced different s-expression", i);
        }
    }
}

// ---------------------------------------------------------------------------
// 6. Parse results under concurrency match single-threaded results
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn concurrent_matches_single_threaded(input in arith_input_strategy()) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));

        // Single-threaded reference
        let ref_syms = collect_symbol_ids(&parse(&grammar, &table, &input));
        let ref_count = ref_syms.len();

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

        for h in handles {
            let result = h.join().unwrap();
            prop_assert_eq!(result.len(), ref_count);
            prop_assert_eq!(&result, &ref_syms);
        }
    }
}

// ---------------------------------------------------------------------------
// 7. No panics under concurrent random input stress
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn no_panics_under_concurrent_stress(
        nums in prop::collection::vec(0u32..10000, 1..=8)
    ) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));

        let handles: Vec<_> = nums
            .iter()
            .map(|&n| {
                let g = Arc::clone(&grammar);
                let t = Arc::clone(&table);
                let input = format!("{}", n);
                thread::spawn(move || {
                    let tree = parse(&g, &t, &input);
                    // Must not panic; tree must have structure
                    prop_assert!(count_nodes(&tree) >= 1);
                    Ok(())
                })
            })
            .collect();

        for h in handles {
            h.join().expect("thread must not panic")?;
        }
    }
}

// ---------------------------------------------------------------------------
// 8. Thread count scaling: 1, 2, 4, 8 threads all produce same result
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn thread_count_scaling_deterministic(
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
            let result = h.join().unwrap();
            prop_assert_eq!(&result, &ref_syms);
        }
    }
}

// ---------------------------------------------------------------------------
// 9. Sequential vs concurrent parsing produces identical trees
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn sequential_vs_concurrent_identical(
        inputs in prop::collection::vec(arith_input_strategy(), 2..=6)
    ) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));

        // Sequential reference
        let sequential_results: Vec<Vec<SymbolId>> = inputs
            .iter()
            .map(|inp| collect_symbol_ids(&parse(&grammar, &table, inp)))
            .collect();

        // Concurrent
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

        let concurrent_results: Vec<Vec<SymbolId>> =
            handles.into_iter().map(|h| h.join().unwrap()).collect();

        for (i, (seq, conc)) in sequential_results.iter().zip(&concurrent_results).enumerate() {
            prop_assert_eq!(seq, conc, "input {} differs between sequential and concurrent", i);
        }
    }
}

// ---------------------------------------------------------------------------
// 10. Rapid create/drop of parsers in multiple threads doesn't leak
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn rapid_parser_create_drop_no_leak(iterations in 5u32..20) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));
        let num_threads = 4;
        let barrier = Arc::new(Barrier::new(num_threads));

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let g = Arc::clone(&grammar);
                let t = Arc::clone(&table);
                let b = Arc::clone(&barrier);
                thread::spawn(move || {
                    b.wait();
                    for _ in 0..iterations {
                        let mut parser = GLRParser::new((*t).clone(), (*g).clone());
                        let mut lexer = GLRLexer::new(&g, "1 + 2".to_string()).unwrap();
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
                        let _tree = parser.finish();
                        // parser and tree are dropped here each iteration
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().expect("rapid create/drop must not panic");
        }
    }
}

// ---------------------------------------------------------------------------
// 11. Different grammars parsed concurrently don't interfere
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn different_grammars_no_interference(
        arith_input in arith_input_strategy(),
        ident_input in ident_input_strategy(),
    ) {
        let arith_g = Arc::new(unambiguous_grammar());
        let arith_t = Arc::new(build_table(&arith_g));
        let ident_g = Arc::new(ident_grammar());
        let ident_t = Arc::new(build_table(&ident_g));

        // Single-threaded references
        let ref_arith = collect_symbol_ids(&parse(&arith_g, &arith_t, &arith_input));
        let ref_ident = collect_symbol_ids(&parse(&ident_g, &ident_t, &ident_input));

        let barrier = Arc::new(Barrier::new(2));

        let g1 = Arc::clone(&arith_g);
        let t1 = Arc::clone(&arith_t);
        let b1 = Arc::clone(&barrier);
        let ai = arith_input.clone();
        let ha = thread::spawn(move || {
            b1.wait();
            collect_symbol_ids(&parse(&g1, &t1, &ai))
        });

        let g2 = Arc::clone(&ident_g);
        let t2 = Arc::clone(&ident_t);
        let b2 = Arc::clone(&barrier);
        let ii = ident_input.clone();
        let hi = thread::spawn(move || {
            b2.wait();
            collect_symbol_ids(&parse(&g2, &t2, &ii))
        });

        prop_assert_eq!(&ha.join().unwrap(), &ref_arith);
        prop_assert_eq!(&hi.join().unwrap(), &ref_ident);
    }
}

// ---------------------------------------------------------------------------
// 12. Node count consistency across threads
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn node_count_consistent_across_threads(input in arith_input_strategy()) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));
        let ref_count = count_nodes(&parse(&grammar, &table, &input));
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
                    count_nodes(&parse(&g, &t, &inp))
                })
            })
            .collect();

        for h in handles {
            let count = h.join().unwrap();
            prop_assert_eq!(count, ref_count);
        }
    }
}

// ---------------------------------------------------------------------------
// 13. Concurrent s-expression serialization matches single-threaded
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn sexp_matches_single_threaded(input in arith_input_strategy()) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));
        let tree = parse(&grammar, &table, &input);

        // Single-threaded reference
        let ref_sexp = build_glr_tree(&grammar, Arc::clone(&tree), &input)
            .root_node()
            .to_sexp();

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
// 14. Shared Arc<Subtree> traversal under contention is safe
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn shared_subtree_traversal_under_contention(input in arith_input_strategy()) {
        let grammar = unambiguous_grammar();
        let table = build_table(&grammar);
        let tree = parse(&grammar, &table, &input);

        // 8 threads all reading the same Arc<Subtree> simultaneously
        let num_threads = 8;
        let barrier = Arc::new(Barrier::new(num_threads));

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let t = Arc::clone(&tree);
                let b = Arc::clone(&barrier);
                thread::spawn(move || {
                    b.wait();
                    // Multiple independent traversals of the same shared tree
                    let c1 = count_nodes(&t);
                    let s1 = collect_symbol_ids(&t);
                    let c2 = count_nodes(&t);
                    let s2 = collect_symbol_ids(&t);
                    // Even within one thread, repeated traversals must be identical
                    assert_eq!(c1, c2);
                    assert_eq!(s1, s2);
                    (c1, s1)
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let (first_count, first_syms) = &results[0];
        for (count, syms) in &results[1..] {
            prop_assert_eq!(count, first_count);
            prop_assert_eq!(syms, first_syms);
        }
    }
}

// ---------------------------------------------------------------------------
// 15. Byte ranges are consistent across concurrent parses
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn byte_ranges_consistent_concurrent(input in arith_input_strategy()) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));

        let ref_ranges = collect_byte_ranges(&parse(&grammar, &table, &input));

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
// 16. Concurrent GLRTree construction is safe and deterministic
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn concurrent_glr_tree_construction(input in arith_input_strategy()) {
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
                    let root = glr_tree.root_node();
                    // Collect basic node metadata
                    let kind = root.kind().to_string();
                    let start = root.start_byte();
                    let end = root.end_byte();
                    let child_count = root.child_count();
                    (kind, start, end, child_count)
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let first = &results[0];
        for r in &results[1..] {
            prop_assert_eq!(r, first);
        }
    }
}

// ---------------------------------------------------------------------------
// 17. Concurrent cursor traversal is deterministic
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn concurrent_cursor_traversal_deterministic(input in arith_input_strategy()) {
        let grammar = Arc::new(unambiguous_grammar());
        let table = Arc::new(build_table(&grammar));
        let tree = parse(&grammar, &table, &input);

        // Use a cursor to collect all node kinds in DFS order
        fn cursor_dfs_kinds(glr_tree: &GLRTree) -> Vec<String> {
            let mut kinds = Vec::new();
            let mut cursor = glr_tree.root_node().walk();
            // DFS: go as deep as possible, then siblings, then up
            let mut reached_root = false;
            loop {
                kinds.push(cursor.node().kind().to_string());
                if cursor.goto_first_child() {
                    continue;
                }
                if cursor.goto_next_sibling() {
                    continue;
                }
                // Go up until we can go to a sibling or reach root
                loop {
                    if !cursor.goto_parent() {
                        reached_root = true;
                        break;
                    }
                    if cursor.goto_next_sibling() {
                        break;
                    }
                }
                if reached_root {
                    break;
                }
            }
            kinds
        }

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
                    cursor_dfs_kinds(&glr_tree)
                })
            })
            .collect();

        let results: Vec<Vec<String>> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let first = &results[0];
        for r in &results[1..] {
            prop_assert_eq!(r, first);
        }
    }
}

//! Classification: build_pipeline
//! Status: active
//! CI coverage: criterion-smoke.yml benchmark compile check (compile-only),
//!   pure-rust-ci.yml dispatch-only benchmark compile check (compile-only)
//!
//! Criterion baselines for core crates:
//!   1. IR normalization (varying grammar sizes)
//!   2. FIRST/FOLLOW set computation
//!   3. Table compression
//!   4. Parse table generation
//!
//! Run: cargo bench --bench core_baselines

use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
use adze_tablegen::compress::TableCompressor;
use adze_tablegen::helpers;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

/// Build a grammar with `n` binary-operator rules over a single expression nonterminal.
/// This creates: expr -> expr OP_i expr  (for i in 0..n)  plus  expr -> NUMBER.
fn make_grammar(operator_count: usize) -> Grammar {
    let expr = SymbolId(0);
    let number = SymbolId(1);

    let mut grammar = Grammar::new("bench".to_string());

    // Terminal: number
    grammar.tokens.insert(
        number,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex("[0-9]+".to_string()),
            fragile: false,
        },
    );

    // Terminals: one per operator
    for i in 0..operator_count {
        let op_id = SymbolId((i + 2) as u16);
        grammar.tokens.insert(
            op_id,
            Token {
                name: format!("op_{i}"),
                pattern: TokenPattern::String(format!("o{i}")),
                fragile: false,
            },
        );
    }

    // Rules for `expr`
    let mut rules = Vec::new();

    // expr -> number
    rules.push(Rule {
        lhs: expr,
        rhs: vec![Symbol::Terminal(number)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    // expr -> expr OP_i expr
    for i in 0..operator_count {
        let op_id = SymbolId((i + 2) as u16);
        rules.push(Rule {
            lhs: expr,
            rhs: vec![
                Symbol::NonTerminal(expr),
                Symbol::Terminal(op_id),
                Symbol::NonTerminal(expr),
            ],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId((i + 1) as u16),
        });
    }

    grammar.rules.insert(expr, rules);
    grammar
}

/// Build a grammar with complex symbols (Optional, Repeat) that normalize() must expand.
fn make_complex_grammar(rule_count: usize) -> Grammar {
    let start = SymbolId(0);
    let tok_a = SymbolId(1);
    let tok_b = SymbolId(2);

    let mut grammar = Grammar::new("complex_bench".to_string());

    grammar.tokens.insert(
        tok_a,
        Token {
            name: "a".to_string(),
            pattern: TokenPattern::String("a".to_string()),
            fragile: false,
        },
    );
    grammar.tokens.insert(
        tok_b,
        Token {
            name: "b".to_string(),
            pattern: TokenPattern::String("b".to_string()),
            fragile: false,
        },
    );

    let mut rules = Vec::new();
    for i in 0..rule_count {
        // Alternate between Optional and Repeat to stress normalize()
        let complex_sym = if i % 2 == 0 {
            Symbol::Optional(Box::new(Symbol::Terminal(tok_a)))
        } else {
            Symbol::Repeat(Box::new(Symbol::Terminal(tok_b)))
        };
        rules.push(Rule {
            lhs: start,
            rhs: vec![complex_sym, Symbol::Terminal(tok_a)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(i as u16),
        });
    }

    grammar.rules.insert(start, rules);
    grammar
}

// ---------- 1. IR normalization ----------

fn bench_normalize(c: &mut Criterion) {
    let mut group = c.benchmark_group("ir_normalize");

    for size in [2, 8, 32] {
        group.bench_with_input(BenchmarkId::new("complex_rules", size), &size, |b, &n| {
            b.iter(|| {
                let mut g = make_complex_grammar(n);
                black_box(g.normalize());
            });
        });
    }

    group.finish();
}

// ---------- 2. FIRST/FOLLOW ----------

fn bench_first_follow(c: &mut Criterion) {
    let mut group = c.benchmark_group("first_follow");

    for ops in [1, 4, 16] {
        group.bench_with_input(BenchmarkId::new("operators", ops), &ops, |b, &n| {
            let g = make_grammar(n);
            b.iter(|| {
                black_box(FirstFollowSets::compute(&g).unwrap());
            });
        });
    }

    group.finish();
}

// ---------- 3. Table compression ----------

fn bench_table_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("table_compression");

    for ops in [1, 4, 16] {
        // Pre-build parse table once, then benchmark compression
        let g = make_grammar(ops);
        let ff = FirstFollowSets::compute(&g).unwrap();
        let pt = build_lr1_automaton(&g, &ff).unwrap();
        let token_ix = helpers::collect_token_indices(&g, &pt);
        let start_empty = helpers::eof_accepts_or_reduces(&pt);
        let compressor = TableCompressor::new();

        group.bench_with_input(BenchmarkId::new("operators", ops), &ops, |b, _| {
            b.iter(|| {
                black_box(compressor.compress(&pt, &token_ix, start_empty).unwrap());
            });
        });
    }

    group.finish();
}

// ---------- 4. Parse table generation ----------

fn bench_parse_table_gen(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_table_gen");

    for ops in [1, 4, 16] {
        let g = make_grammar(ops);
        let ff = FirstFollowSets::compute(&g).unwrap();

        group.bench_with_input(BenchmarkId::new("operators", ops), &ops, |b, _| {
            b.iter(|| {
                black_box(build_lr1_automaton(&g, &ff).unwrap());
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_normalize,
    bench_first_follow,
    bench_table_compression,
    bench_parse_table_gen,
);
criterion_main!(benches);

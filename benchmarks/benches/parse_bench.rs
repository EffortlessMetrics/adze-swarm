//! Classification: real_parser
//! Status: active
//! CI coverage: criterion-smoke.yml benchmark compile check (compile-only),
//!   pure-rust-ci.yml dispatch-only benchmark compile check (compile-only)
//!
//! Baseline parser benchmark for valid arithmetic fixtures.
//!
//! This bench measures real parser work: each iteration parses a valid
//! arithmetic expression through the generated arithmetic grammar.

use adze_example::arithmetic::grammar::parse;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

const ARITH_SMALL: &str = include_str!("../fixtures/arithmetic/small.expr");
const ARITH_MEDIUM: &str = include_str!("../fixtures/arithmetic/medium.expr");
const ARITH_LARGE: &str = include_str!("../fixtures/arithmetic/large.expr");

fn benchmark_arithmetic_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_bench_arithmetic");

    for (label, source) in [
        ("small", ARITH_SMALL),
        ("medium", ARITH_MEDIUM),
        ("large", ARITH_LARGE),
    ] {
        assert!(
            parse(source).is_ok(),
            "fixture {label} must parse before benchmarking"
        );

        group.bench_with_input(BenchmarkId::new("parse", label), source, |b, source| {
            b.iter(|| {
                black_box(parse(black_box(source)).expect("fixture must parse"));
            });
        });
    }

    group.finish();
}

criterion_group!(benches, benchmark_arithmetic_parsing);
criterion_main!(benches);

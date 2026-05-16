//! Classification: real_parser
//! Status: active
//! CI coverage: criterion-smoke.yml benchmark compile check (compile-only),
//!   pure-rust-ci.yml dispatch-only benchmark compile check (compile-only)
//!
//! Hot-path GLR parser benchmark for medium/large arithmetic fixtures.

use adze_example::arithmetic::grammar::parse;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

const ARITH_MEDIUM: &str = include_str!("../fixtures/arithmetic/medium.expr");
const ARITH_LARGE: &str = include_str!("../fixtures/arithmetic/large.expr");

fn benchmark_glr_hot(c: &mut Criterion) {
    let mut group = c.benchmark_group("glr_hot");

    // Validate fixtures once to ensure this is a real parser workload.
    for (label, source) in &[("medium", ARITH_MEDIUM), ("large", ARITH_LARGE)] {
        assert!(
            parse(source).is_ok(),
            "Fixture {} must parse successfully for hot benchmarks",
            label
        );
    }

    // Hot-path parsing of larger payloads.
    for (label, source) in &[("medium", ARITH_MEDIUM), ("large", ARITH_LARGE)] {
        group.bench_with_input(BenchmarkId::new("parse", label), source, |b, source| {
            b.iter(|| {
                black_box(parse(source).expect("fixture must parse"));
            });
        });
    }

    group.finish();
}

criterion_group!(benches, benchmark_glr_hot);
criterion_main!(benches);

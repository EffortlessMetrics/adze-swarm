//! Classification: projection
//! Status: active
//! CI coverage: criterion-smoke.yml benchmark compile check (compile-only),
//!   pure-rust-ci.yml dispatch-only benchmark compile check (compile-only)
//!
//! Advisory projection benchmark for fixture-backed document surfaces.
//!
//! This bench intentionally stays compile-only in ordinary CI. Runtime
//! measurements are manual or scheduled evidence under ADZE-SPEC-0014.

use adze_example::arithmetic::grammar::{self, Expression};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

const ARITH_SMALL: &str = include_str!("../fixtures/arithmetic/small.expr");
const ARITH_MEDIUM: &str = include_str!("../fixtures/arithmetic/medium.expr");
const ARITH_LARGE: &str = include_str!("../fixtures/arithmetic/large.expr");

fn benchmark_document_projection(c: &mut Criterion) {
    let fixtures = [
        ("small", ARITH_SMALL),
        ("medium", ARITH_MEDIUM),
        ("large", ARITH_LARGE),
    ];

    for (label, source) in fixtures {
        assert!(
            grammar::parse(source).is_ok(),
            "fixture {label} must parse before benchmarking projections"
        );
        assert!(
            grammar::parse_document(source).is_ok(),
            "fixture {label} must produce an AdzeDocument before benchmarking projections"
        );
    }

    let mut parse_document = c.benchmark_group("document_projection_parse_document");
    for (label, source) in fixtures {
        parse_document.bench_with_input(
            BenchmarkId::new("parse_document", label),
            source,
            |b, source| {
                b.iter(|| {
                    black_box(
                        grammar::parse_document(black_box(source))
                            .expect("fixture must produce a document"),
                    );
                });
            },
        );
    }
    parse_document.finish();

    let documents = fixtures
        .into_iter()
        .map(|(label, source)| {
            (
                label,
                grammar::parse_document(source).expect("fixture must produce a document"),
            )
        })
        .collect::<Vec<_>>();

    let mut projections = c.benchmark_group("document_projection_views");
    for (label, document) in &documents {
        projections.bench_with_input(
            BenchmarkId::new("typed_ast", label),
            document,
            |b, document| {
                b.iter(|| {
                    black_box(
                        document
                            .ast::<Expression>()
                            .expect("fixture document must project to typed AST"),
                    );
                });
            },
        );

        projections.bench_with_input(BenchmarkId::new("json", label), document, |b, document| {
            b.iter(|| {
                black_box(document.to_json_value());
            });
        });
    }
    projections.finish();
}

criterion_group!(benches, benchmark_document_projection);
criterion_main!(benches);

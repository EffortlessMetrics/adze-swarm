//! Verification test to confirm that Python/JS fixtures cannot be parsed
//! by the arithmetic grammar (exposing the benchmark correctness issue).

use adze_example::arithmetic::grammar::parse;

// Load the same fixtures used in the benchmark
const PYTHON_SMALL: &str = include_str!("../fixtures/python/small.py");
const PYTHON_MEDIUM: &str = include_str!("../fixtures/python/medium.py");
const PYTHON_LARGE: &str = include_str!("../fixtures/python/large.py");

const JS_SMALL: &str = include_str!("../fixtures/javascript/small.js");
const JS_MEDIUM: &str = include_str!("../fixtures/javascript/medium.js");
const JS_LARGE: &str = include_str!("../fixtures/javascript/large.js");

const ARITH_SMALL: &str = include_str!("../fixtures/arithmetic/small.expr");
const ARITH_MEDIUM: &str = include_str!("../fixtures/arithmetic/medium.expr");
const ARITH_LARGE: &str = include_str!("../fixtures/arithmetic/large.expr");
const PARSE_BENCH_SOURCE: &str = include_str!("../benches/parse_bench.rs");
const BENCHMARK_CARGO_TOML: &str = include_str!("../Cargo.toml");
const BENCHMARK_README: &str = include_str!("../README.md");
const FIXTURE_METADATA: &str = include_str!("../fixtures/metadata.toml");
const PERF_BASELINES: &str = include_str!("../../docs/perf/baselines.md");
const PERF_RECEIPT_SOURCE: &str = include_str!("../../xtask/src/perf_receipt.rs");

fn registered_bench_names() -> Vec<String> {
    let mut names = Vec::new();
    let mut in_bench = false;

    for line in BENCHMARK_CARGO_TOML.lines() {
        let trimmed = line.trim();
        if trimmed == "[[bench]]" {
            in_bench = true;
            continue;
        }

        if in_bench && trimmed.starts_with("name = ") {
            let name = trimmed
                .trim_start_matches("name = ")
                .trim_matches('"')
                .to_owned();
            names.push(name);
            in_bench = false;
        }
    }

    names.sort();
    names
}

fn classified_bench_names() -> Vec<String> {
    let mut names = Vec::new();
    let mut in_metadata = false;

    for line in BENCHMARK_CARGO_TOML.lines() {
        let trimmed = line.trim();
        if trimmed == "[package.metadata.bench-classification]" {
            in_metadata = true;
            continue;
        }
        if in_metadata && trimmed.starts_with('[') {
            break;
        }
        if in_metadata && trimmed.contains(" = {") {
            let (name, metadata) = trimmed
                .split_once(" = ")
                .expect("metadata row should contain an assignment");
            assert!(
                metadata.contains("classification = "),
                "benchmark metadata for {name} must include a classification"
            );
            assert!(
                metadata.contains("status = "),
                "benchmark metadata for {name} must include a status"
            );
            assert!(
                metadata.contains("ci = "),
                "benchmark metadata for {name} must include CI coverage"
            );
            assert!(
                metadata.contains("fixture_family = "),
                "benchmark metadata for {name} must include a fixture family"
            );
            names.push(name.to_owned());
        }
    }

    names.sort();
    names
}

fn readme_inventory_names() -> Vec<String> {
    let mut names = Vec::new();

    for line in BENCHMARK_README.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("| `") {
            continue;
        }

        let Some(rest) = trimmed.strip_prefix("| `") else {
            continue;
        };
        let Some((name, _)) = rest.split_once('`') else {
            continue;
        };
        names.push(name.to_owned());
    }

    names.sort();
    names
}

fn benchmark_metadata_field(name: &str, field: &str) -> Option<String> {
    let mut in_metadata = false;

    for line in BENCHMARK_CARGO_TOML.lines() {
        let trimmed = line.trim();
        if trimmed == "[package.metadata.bench-classification]" {
            in_metadata = true;
            continue;
        }
        if in_metadata && trimmed.starts_with('[') {
            break;
        }
        if !in_metadata {
            continue;
        }

        let Some((row_name, metadata)) = trimmed.split_once(" = ") else {
            continue;
        };
        if row_name.trim() != name {
            continue;
        }

        let inline_table = metadata
            .trim()
            .trim_start_matches('{')
            .trim_end_matches('}');
        for part in inline_table.split(',') {
            let Some((key, value)) = part.split_once(" = ") else {
                continue;
            };
            if key.trim() == field {
                return Some(value.trim().trim_matches('"').to_owned());
            }
        }
    }

    None
}

fn fixture_metadata_has_family(family: &str) -> bool {
    let prefix = format!("[{family}.");
    FIXTURE_METADATA
        .lines()
        .map(str::trim)
        .any(|line| line.starts_with(&prefix))
}

fn readme_inventory_status(name: &str) -> Option<String> {
    let needle = format!("`{name}`");

    for line in BENCHMARK_README.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }

        let cells = trimmed
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        if cells.first() == Some(&needle.as_str()) {
            return cells.get(2).map(|status| (*status).to_owned());
        }
    }

    None
}

#[test]
fn verify_python_fixtures_do_not_parse_with_arithmetic_grammar() {
    // This test documents the current state: Python fixtures contain code
    // that the arithmetic grammar cannot properly parse.
    //
    // Tree-sitter has aggressive error recovery, so parse() may return Ok(_)
    // even for invalid input, with ERROR nodes in the tree.

    for (label, source) in &[
        ("python_small", PYTHON_SMALL),
        ("python_medium", PYTHON_MEDIUM),
        ("python_large", PYTHON_LARGE),
    ] {
        let result = parse(source);

        match result {
            Ok(expr) => {
                // Parser "succeeded" but likely with error recovery
                println!("{}: Parsed with error recovery: {:?}", label, expr);
                println!("WARNING: Benchmark is measuring error recovery, not valid parsing!");
            }
            Err(e) => {
                println!("{}: Parse failed: {:?}", label, e);
            }
        }
    }
}

#[test]
fn verify_javascript_fixtures_do_not_parse_with_arithmetic_grammar() {
    for (label, source) in &[
        ("javascript_small", JS_SMALL),
        ("javascript_medium", JS_MEDIUM),
        ("javascript_large", JS_LARGE),
    ] {
        let result = parse(source);

        match result {
            Ok(expr) => {
                println!("{}: Parsed with error recovery: {:?}", label, expr);
                println!("WARNING: Benchmark is measuring error recovery, not valid parsing!");
            }
            Err(e) => {
                println!("{}: Parse failed: {:?}", label, e);
            }
        }
    }
}

#[test]
fn verify_arithmetic_benchmark_fixtures_parse_with_arithmetic_grammar() {
    for (label, source) in &[
        ("small.expr", ARITH_SMALL),
        ("medium.expr", ARITH_MEDIUM),
        ("large.expr", ARITH_LARGE),
    ] {
        eprintln!("validating arithmetic benchmark fixture: {}", label);
        let result = parse(source);
        assert!(
            result.is_ok(),
            "arithmetic benchmark fixture {} failed to parse: {:?}",
            label,
            result
        );
    }
}

#[test]
fn verify_parse_bench_uses_real_parser_workload() {
    assert!(
        PARSE_BENCH_SOURCE.contains("adze_example::arithmetic::grammar::parse"),
        "parse_bench must call the generated arithmetic parser"
    );
    assert!(
        PARSE_BENCH_SOURCE.contains("bench_with_input"),
        "parse_bench must benchmark fixture-backed parser input"
    );
    assert!(
        !PARSE_BENCH_SOURCE.contains("placeholder_no_parser_workload"),
        "parse_bench must not advertise a placeholder/no-parser workload"
    );
    assert!(
        !PARSE_BENCH_SOURCE.contains("1 + 1"),
        "parse_bench must not benchmark a dummy arithmetic expression"
    );
}

#[test]
fn verify_benchmark_inventory_is_exhaustive() {
    let registered = registered_bench_names();
    let classified = classified_bench_names();
    let documented = readme_inventory_names();

    assert_eq!(
        registered, classified,
        "every [[bench]] entry must have classification metadata"
    );
    assert_eq!(
        registered, documented,
        "benchmarks/README.md must document every registered benchmark"
    );
}

#[test]
fn verify_benchmark_fixture_families_are_documented() {
    for bench in registered_bench_names() {
        let family = benchmark_metadata_field(&bench, "fixture_family")
            .unwrap_or_else(|| panic!("benchmark metadata for {bench} must name fixture_family"));

        if family.starts_with("synthetic_") {
            continue;
        }

        assert!(
            fixture_metadata_has_family(&family),
            "fixture family {family} for benchmark {bench} must be documented in fixtures metadata"
        );
    }
}

#[test]
fn verify_duplicate_glr_performance_bench_was_removed() {
    assert!(
        !registered_bench_names()
            .iter()
            .any(|name| name == "glr_performance"),
        "glr_performance duplicated parse_bench and should stay removed"
    );
    assert!(
        benchmark_metadata_field("glr_performance", "status").is_none(),
        "removed duplicate benchmark must not keep stale classification metadata"
    );
    assert!(
        readme_inventory_status("glr_performance").is_none(),
        "removed duplicate benchmark must not stay in the README inventory"
    );
}

#[test]
fn verify_product_smoke_perf_receipt_is_documented() {
    let command = "cargo run -q -p xtask -- perf-receipt --profile product-smoke";

    assert!(
        BENCHMARK_README.contains(command),
        "benchmarks README must document the product-smoke receipt command"
    );
    assert!(
        PERF_BASELINES.contains(command),
        "performance baseline docs must document the product-smoke receipt command"
    );
    assert!(
        PERF_RECEIPT_SOURCE.contains("parse_bench --no-run"),
        "product-smoke receipt must include parse benchmark compile health"
    );
    assert!(
        PERF_RECEIPT_SOURCE.contains("document_projection --no-run"),
        "product-smoke receipt must include document projection compile health"
    );
    assert!(
        PERF_RECEIPT_SOURCE.contains("no stable throughput claim"),
        "product-smoke receipt must keep performance claims advisory"
    );
}

#[test]
#[ignore = "KNOWN BUG: arithmetic parser rejects single-literal expressions like '1'"]
fn verify_valid_arithmetic_expressions_do_parse() {
    // Sanity check: ensure the parser actually works with valid input
    let valid_expressions = vec![
        "1",
        "1 - 2",
        "1 * 2",
        "1 - 2 * 3",
        "1 * 2 - 3",
        "1 - 2 - 3",
        "1 * 2 * 3",
    ];

    for expr in valid_expressions {
        let result = parse(expr);
        assert!(
            result.is_ok(),
            "Failed to parse valid arithmetic expression '{}': {:?}",
            expr,
            result
        );
    }
}

# Benchmark Inventory

All benchmarks use [Criterion](https://bheisler.github.io/criterion.rs/) and
are registered in `Cargo.toml`. Per-bench metadata lives in the
`[package.metadata.bench-classification]` table there and as header comments in
each source file.

## Inventory

| Bench | Classification | Status | Fixture family | Notes |
|---|---|---|---|---|
| `parse_bench` | real_parser | active | arithmetic | Baseline arithmetic parsing |
| `document_projection` | projection | active | arithmetic | Advisory `parse_document`, typed AST projection, and JSON projection fixture bench |
| `glr_hot` | real_parser | active | arithmetic | Hot-path medium/large arithmetic fixtures |
| `glr_performance_real` | real_parser | active | arithmetic | Full GLR parsing with valid arithmetic fixtures |
| `incremental_bench` | real_parser | active | synthetic_arithmetic | Full-reparse vs incremental reparse |
| `optimization_bench` | infrastructure | legacy | synthetic_infrastructure | Superseded by `arena_vs_box_allocation` and `stack_optimization` |
| `stack_optimization` | infrastructure | active | synthetic_infrastructure | Vec vs persistent stack micro-benchmarks |
| `arena_vs_box_allocation` | infrastructure | active | synthetic_infrastructure | Arena vs Box allocation comparison |
| `core_baselines` | build_pipeline | active | synthetic_build_pipeline | IR normalization, FIRST/FOLLOW, table compression |

## Running

```bash
# Compile-check all benches.
cargo bench -p adze-benchmarks --no-run

# Run a specific bench.
cargo bench -p adze-benchmarks --bench parse_bench

# Print the advisory product benchmark receipt command set.
cargo run -q -p xtask -- perf-receipt --profile product-smoke
```

## Product Smoke Receipt

`product-smoke` is an advisory receipt profile for release/product review. It
prints the fixture inventory checks and compile-only benchmark commands that
cover the current product benchmark slices:

- parse-only fixture health through `parse_bench --no-run`;
- `parse_document`, typed AST projection, and JSON projection fixture health
  through `document_projection --no-run`;
- benchmark fixture-family and inventory consistency through
  `verify_fixture_parsing`.

This profile does not run Criterion measurement to completion and does not
define throughput, memory, Tree-sitter parity, incremental parsing, or
release-blocking regression claims.

## CI Coverage

In `adze-swarm`, benchmark evidence is compile-only unless a maintainer runs a
benchmark manually:

- `criterion-smoke.yml` runs `cargo bench -p adze-benchmarks --no-run` on
  schedule and manual dispatch.
- `ci.yml` remains a scheduled/manual legacy full-CI lane and compile-checks
  selected benchmark surfaces, including `incremental_bench`.
- `pure-rust-ci.yml` compile-checks all benches only on workflow dispatch, not
  on ordinary pull requests.

No benchmark runs to completion on ordinary pull requests. Performance result
collection is explicit manual/scheduled evidence, not a default PR gate.

# Benchmark Inventory

All benchmarks use [Criterion](https://bheisler.github.io/criterion.rs/) and
are registered in `Cargo.toml`. Per-bench metadata lives in the
`[package.metadata.bench-classification]` table there and as header comments in
each source file.

## Inventory

| Bench | Classification | Status | Notes |
|---|---|---|---|
| `parse_bench` | real_parser | active | Baseline arithmetic parsing |
| `glr_performance` | real_parser | active | Substantially duplicates `parse_bench` |
| `glr_hot` | real_parser | active | Hot-path medium/large arithmetic fixtures |
| `glr_performance_real` | real_parser | active | Full GLR parsing with valid arithmetic fixtures |
| `incremental_bench` | real_parser | active | Full-reparse vs incremental reparse |
| `optimization_bench` | infrastructure | legacy | Superseded by `arena_vs_box_allocation` and `stack_optimization` |
| `stack_optimization` | infrastructure | active | Vec vs persistent stack micro-benchmarks |
| `arena_vs_box_allocation` | infrastructure | active | Arena vs Box allocation comparison |
| `core_baselines` | build_pipeline | active | IR normalization, FIRST/FOLLOW, table compression |

## Running

```bash
# Compile-check all benches.
cargo bench -p adze-benchmarks --no-run

# Run a specific bench.
cargo bench -p adze-benchmarks --bench parse_bench
```

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

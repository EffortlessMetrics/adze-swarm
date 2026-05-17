# Testing

This page gives a practical overview of running and writing tests in the Adze
workspace. For support-tiered product proof, see
`docs/status/SUPPORT_TIERS.md`. For the full testing strategy see the
[Development Testing Guide](development/testing.md).

## Running the test suite

```bash
# Required local supported gate for the core parser pipeline
just ci-supported

# Core library tests for fast iteration
just test

# All workspace tests when you explicitly need broad local coverage
cargo test --workspace

# Concurrency-capped variants (more stable on CI or constrained machines)
cargo t2                  # 2 test threads
cargo test-safe           # safe defaults
cargo test-ultra-safe     # single-threaded
./scripts/test-capped.sh  # auto-detect caps
```

### Per-crate tests

```bash
cargo test -p adze            # runtime
cargo test -p adze-macro       # proc-macro
cargo test -p adze-ir          # grammar IR
cargo test -p adze-glr-core    # GLR analysis (use --features test-api for internal helpers)
cargo test -p adze-tablegen    # table compression
cargo test -p adze-tool        # build tool
```

### Feature combinations

Some product canaries exercise feature-specific runtime paths. Prefer explicit
feature sets over broad feature aliases so the command describes the surface
being proven:

```bash
cargo test -p adze --features glr
cargo test -p adze --features "pure-rust,glr"
cargo test -p adze --features incremental_glr
```

When a feature-specific result becomes a public claim, it should have a row in
`docs/status/SUPPORT_TIERS.md` with the exact proof command and known limits.

## Golden tests

Golden tests are advisory Tree-sitter parity receipts. They are useful for
language and projection work, but they are not the default merge gate.

```bash
cd golden-tests

# Generate reference S-expressions and SHA256 hashes (one-time)
./generate_references.sh

# Run a focused package-level canary
cargo test -p adze-golden-tests javascript_canary_expression_golden --features javascript-grammar -- --nocapture

# Update references after intentional parser changes
UPDATE_GOLDEN=1 cargo test --features python-grammar
```

See [Golden Tests Maintenance](guide/golden-tests-maintenance.md) for the full workflow.

## Snapshot tests (insta)

Example and generated-output tests may use [insta](https://insta.rs) for
snapshot testing:

```bash
cargo test -p adze --features "pure-rust,serialization" --test adze_document_json -- --nocapture
cargo insta review  # interactive diff review
```

When grammar output changes intentionally, review and accept the new snapshots.

## Writing a grammar test

The simplest pattern is to parse a string and assert against the typed AST:

```rust
#[cfg(test)]
mod tests {
    use super::grammar;

    #[test]
    fn addition() {
        let ast = grammar::parse("1 + 2").unwrap();
        assert_eq!(
            ast,
            grammar::Expression::Add(
                Box::new(grammar::Expression::Number(1)),
                (),
                Box::new(grammar::Expression::Number(2)),
            )
        );
    }

    #[test]
    fn precedence() {
        // Multiplication binds tighter than addition
        let ast = grammar::parse("1 + 2 * 3").unwrap();
        match ast {
            grammar::Expression::Add(_, _, rhs) => {
                assert!(matches!(*rhs, grammar::Expression::Mul(..)));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn rejects_invalid_input() {
        assert!(grammar::parse("1 + + 2").is_err());
    }
}
```

## Governance BDD Matrix

The post-collapse BDD/governance support crate is
`adze-bdd-governance-core`. It owns governance matrix primitives and parser
feature policy receipts; it is not a separate product runtime.

```rust
use adze_bdd_governance_core::{
    bdd_progress,
    BddPhase,
    GLR_CONFLICT_PRESERVATION_GRID,
};

let (implemented, total) =
    bdd_progress(BddPhase::Core, GLR_CONFLICT_PRESERVATION_GRID);
assert!(implemented <= total);
```

Run the focused governance proof when touching the BDD matrix or feature policy
surface:

```bash
cargo test -p adze-bdd-governance-core
cargo test -p adze-bdd-governance-core --lib grid::tests::progress_summary_reports_counts -- --exact --nocapture
cargo test -p glr-test-support grammar_
```

The package boundary ledger documents the current durable support crates. If a
test change adds, removes, or reclassifies a package, also run:

```bash
cargo run -q -p xtask -- check-package-boundary
```

## Test connectivity safeguards

Several layers prevent tests from being silently disconnected:

1. **CI job** — blocks `.rs.disabled` files and enforces non-zero test counts per crate.
2. **Pre-commit hook** — warns about disabled test files.
3. **Local check** — `./scripts/check-test-connectivity.sh` reports per-crate counts and orphans.

## Concurrency tips

| Variable | Default | Purpose |
|---|---|---|
| `RUST_TEST_THREADS` | 2 | Rust test parallelism |
| `RAYON_NUM_THREADS` | 4 | Rayon pool size |
| `TOKIO_WORKER_THREADS` | 2 | Tokio async workers |
| `CARGO_BUILD_JOBS` | 4 | Cargo compile jobs |

Lower these if tests fail with "Too many open files" or thread-creation errors. The `./scripts/preflight.sh` script auto-detects safe values.

## Further reading

- [Development Testing Guide](development/testing.md) — exhaustive testing strategy
- [Golden Tests Guide](development/golden-tests.md) — golden test internals
- [Performance Optimization](guide/performance.md) — benchmarking and profiling

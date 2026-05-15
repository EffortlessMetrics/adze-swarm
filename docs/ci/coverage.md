# Coverage

Test execution coverage is an **execution-surface signal** for the Adze Rust codebase.

## What coverage answers

Coverage reports the **test execution** of the Rust parser, runtime, and codegen surfaces:

> Did tests exercise this Rust source code?

It is a necessary but not sufficient condition for correctness.

## What coverage does NOT answer

Coverage is **not** proof of:

- **Parser correctness** — coverage only shows that code ran, not that it produced correct output
- **GLR ambiguity handling** — conflict resolution and forking correctness requires separate semantic tests
- **Mutation adequacy** — high coverage with weak assertions can hide correctness gaps
- **Fuzz robustness** — fuzzing is a separate execution surface with different failure modes
- **Miri/sanitizer proof** — undefined behavior requires memory safety tools, not execution coverage
- **Release packaging** — packaging, versioning, and distribution require separate proof lanes
- **Public API stability** — semver compliance and stability are separate governance signals

## Coverage lanes

Coverage is split by cost.

### `coverage-lite`

`coverage-lite` is the PR-facing lane. It runs on Linux/stable when a PR carries
the `coverage` label or touches primary runtime/test paths. It starts with a
deliberately narrow package set so the lane is cheap enough to use as PR
evidence:

```text
adze
```

Additional package groups should be added only after measured runtime shows the
lane still fits the lite budget.

### `coverage-full`

`coverage-full` is explicit evidence. It runs by `workflow_dispatch` or on PRs
with `full-ci`. It uses the broader workspace/features command.

Both lanes run `cargo-llvm-cov` to:

1. Generate `lcov.info` (LCOV format)
2. Upload the LCOV file as a GitHub artifact
3. Upload `lcov.info` to Codecov when `CODECOV_TOKEN` is present

### Execution

**When `coverage-lite` runs:**
- pull requests labeled `coverage`
- pull requests that touch primary runtime/test coverage paths
- `workflow_dispatch` with mode `lite`

**When `coverage-full` runs:**
- pull requests labeled `full-ci`
- `workflow_dispatch` with mode `full`

**Cost:**
- `coverage-lite`: lower-cost primary runtime package evidence
- `coverage-full`: broader high-cost evidence
- Zero impact on docs-only/policy-only ordinary pull requests

**Blocking:**
- No — advisory only, does not block PRs or merges

### Configuration

See `codecov.yml` for:

- **Project threshold**: 5% (informational; no auto-block)
- **Patch threshold**: 70% target, 20% threshold (informational)
- **Comments**: Disabled (rely on dashboard)
- **Annotations**: Disabled (reduce PR noise)

Codecov upload is publication, not the source of truth. A Codecov upload
failure should not fail the lane when `lcov.info` was generated and uploaded as
a GitHub artifact.

## Using the Coverage dashboard

The Codecov dashboard at https://codecov.io/gh/EffortlessMetrics/adze shows:

- Current main branch coverage by file and function
- Historical coverage trend over time
- Patch coverage for labeled PRs
- Comparative view across branches

## Durable evidence

Coverage evidence is recorded in:

1. **Codecov dashboard** — persistent, queryable by commit and date range
2. **GitHub Actions artifact** — `coverage-lite-lcov` or `coverage-full-lcov` with `lcov.info` (14-day retention)
3. **Local runs** — `cargo llvm-cov` on your machine with `--output-path lcov.info`

## Related lanes and evidence

- **ci-supported** (`just ci-supported`) — local supported/product proof covering core parser/runtime/tooling surface; `adze-swarm` branch protection requires `Rust Small Result`
- **pure-rust-os-matrix** — OS and toolchain compatibility (Linux/macOS/Windows, stable/beta/nightly)
- **test-policy** — policy/docs-routed test hygiene, disabled-test prevention, and static inventory; runtime-cap proof on manual dispatch
- **fuzz-build-smoke** — fuzz harness freshness (compile-only)
- **Miri** — undefined behavior detection (separate nightly-only lane)
- **Sanitizers** — ASAN/UBSAN proof (separate lane)

Coverage is complementary to these: high coverage with passing tests is necessary but not sufficient for release readiness.

## Roadmap

1. Baseline `coverage-lite` runtime on primary runtime paths.
2. Decide whether `coverage-lite` should move to CX53 once the `rust-large`
   runner class is available.
3. Establish ratchet thresholds only after stable baseline data exists.


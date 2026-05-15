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

## The Coverage lane

The Coverage lane runs `cargo-llvm-cov` and `cargo-nextest` on the Adze workspace to:

1. Generate `coverage.json` (JSON coverage report)
2. Generate `coverage.txt` (human-readable summary)
3. Generate `lcov.info` (LCOV format for Codecov)
4. Upload `lcov.info` to Codecov (when `CODECOV_TOKEN` secret is present)
5. Upload all artifacts to GitHub Actions for inspection

### Execution

**When it runs:**
- `push` to `main` (every commit)
- `workflow_dispatch` (on-demand)
- Pull requests labeled `coverage` or `full-ci` (voluntary, label-driven)

**Cost:**
- ~45 Linux-equivalent minutes per run
- Zero impact on ordinary pull requests (not `default_pr`)

**Blocking:**
- No — advisory only, does not block PRs or merges

### Configuration

See `codecov.yml` for:

- **Project threshold**: 5% (informational; no auto-block)
- **Patch threshold**: 70% target, 20% threshold (informational)
- **Comments**: Disabled (rely on dashboard)
- **Annotations**: Disabled (reduce PR noise)

## Using the Coverage dashboard

The Codecov dashboard at https://codecov.io/gh/EffortlessMetrics/adze shows:

- Current main branch coverage by file and function
- Historical coverage trend over time
- Patch coverage for labeled PRs
- Comparative view across branches

## Durable evidence

Coverage evidence is recorded in:

1. **Codecov dashboard** — persistent, queryable by commit and date range
2. **GitHub Actions artifact** — `coverage-report` with coverage.json, coverage.txt, lcov.info (14-day retention)
3. **Local runs** — `cargo llvm-cov` on your machine with `--output-path lcov.info`

## Related lanes and evidence

- **ci-supported** (`just ci-supported`) — required gate covering core parser/runtime/tooling surface
- **pure-rust-os-matrix** — OS and toolchain compatibility (Linux/macOS/Windows, stable/beta/nightly)
- **test-policy** — policy/docs-routed test hygiene, disabled-test prevention, and static inventory; runtime-cap proof on manual dispatch
- **fuzz-build-smoke** — fuzz harness freshness (compile-only)
- **Miri** — undefined behavior detection (separate nightly-only lane)
- **Sanitizers** — ASAN/UBSAN proof (separate lane)

Coverage is complementary to these: high coverage with passing tests is necessary but not sufficient for release readiness.

## Roadmap

1. **PR 1–3 (current)**: Add Coverage workflow, config, and badge
2. **PR 4–6**: Register lane in policy, document proof boundaries, add receipt
3. **PR 7+**: Baseline coverage data, establish ratchet thresholds, integrate into governance reports


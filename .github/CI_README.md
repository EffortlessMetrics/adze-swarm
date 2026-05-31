# CI Infrastructure for adze-swarm

This document describes the active CI setup for `adze-swarm`.

For the full lane classification (required vs advisory vs push-only), see [CI_LANES.md](./CI_LANES.md).
For runner capacity classes and fallback policy, see
[runner-classes.md](../docs/ci/runner-classes.md).

## Overview

`adze-swarm` uses a bounded same-repo PR gate so agents can merge small
changes without paying the full public-release CI cost on every PR.

Jobs are classified into three visibility tiers:

- **Required** (`Rust Small Result`, `Product Proof Result`): The aggregate
  merge gates in branch protection. Both must be green before merge.
- **Push / scheduled**: Runs on schedules, manual dispatch, labels, or explicit paths. Not the default PR blocker.
- **Advisory** (prefixed `Advisory / `): Uses nightly/unstable toolchains. May be red due to toolchain drift. Inspect, don't block.

### Jobs

1. **Rust Small Result** - Required routed Rust Small aggregate check for ordinary swarm PRs.
2. **Product Proof Result** - Required aggregate Stable-claim proof gate.
3. **Route Rust Small** - Hosted control-plane router that chooses CPX42, CX43, CX33, or explicit fallback; the route job runner is logged with `current=true` when it appears in the self-hosted runner list, and CX53 rust-small plus planned rust-large candidate state is logged with label/group diagnostics but not selected while #598 is blocked.
4. **Rust Small implementation lanes** - Conditional implementation lanes; one selected lane runs when eligible capacity exists, while the others usually skip.
5. **ci-supported** - Legacy public full-CI support lane; retained for schedule/manual dispatch in `ci.yml`.
6. **Policy checks** - Source-of-truth and lane-whitelist guardrails.
7. **Deep lanes** - Feature matrix, OS matrix, coverage, benchmarks, fuzzing, security, docs, and advisory product proof; scheduled, manual, label, or path-routed unless explicitly promoted.
8. **CX53 Rust Large Diagnostic** - Manual-only runner burn-in probe for #598; it checks CX53 `rust-large` candidate visibility before running a selected host-smoke job.

Routing, path detection, no-capacity diagnostics, and aggregate result checks
are allowed to use hosted control-plane runners so branch protection does not
consume scarce self-hosted slots before selecting real work. Rust and product
canary execution remains self-hosted unless the existing explicit fallback path
is selected.

## Manual CI triggers

The `CI` workflow supports manual dispatch with two toggles:

- `run_full_ci` (workflow_dispatch only): Run the full non-PR lane in addition to PR-required lanes.
- `run_ci_supported_examples` (workflow_dispatch only): Enable experimental examples in `feature-matrix`.
  If `run_full_ci` is false, this is the only non-PR lane that runs on manual dispatch.
  Outside manual dispatch, experimental examples in `feature-matrix` only run when commit message includes `[test-examples]`.

`CX53 Rust Large Diagnostic` is a separate manual workflow for runner evidence.
It is not a PR gate and does not change Rust Small branch protection.

## Required GitHub Settings

To make the CI effective, configure these branch protection rules:

### Required Status Checks

Required status checks are intentionally aggregate-gated. Set `adze-swarm`
branch protection to require only:

- `Rust Small Result`
- `Product Proof Result`

Do not require conditional implementation jobs such as `Rust Small on CX43`,
`Rust Small on GitHub Hosted`, or `ci-product stable canaries` directly.
Everything else is optional signal unless explicitly promoted in
[CI_LANES.md](./CI_LANES.md) and repository settings.

### Recommended Settings
- Require branches to be up to date before merging
- Do not require conversation resolution in `adze-swarm`; bot review
  conversations are advisory and must not block the single-operator merge loop
  once `Rust Small Result` and `Product Proof Result` are green.

## Local Development

### Running CI Locally

```bash
# Install required tools
cargo install cargo-nextest cargo-hack cargo-deny cargo-llvm-cov

# Run the supported proof locally
just ci-supported  # Stable local supported/product proof, not the swarm GitHub required context
cargo nextest run  # Fast parallel test runner
cargo deny check   # Security and license checks

# Test feature combinations
cargo hack test --feature-powerset --skip tree-sitter-standard
```

### Snapshot Testing

We use `insta` for snapshot testing of generated code:

```bash
# Review snapshot changes
cargo insta review

# Update snapshots in CI
INSTA_UPDATE=auto cargo test
```

### Fuzzing

The default CI fuzz lane exercises runtime fuzz targets in the `runtime/fuzz/`
directory:

```bash
# Install cargo-fuzz
cargo install cargo-fuzz

# Run fuzzer (requires nightly)
cd runtime/fuzz
cargo +nightly fuzz run fuzz_lexer_simple
cargo +nightly fuzz run fuzz_glr_parser
cargo +nightly fuzz run fuzz_incremental_edits
```

## Testing Strategies

### Contract Tests
- **Snapshot tests** - Ensure stable output format
- **Compile-fail tests** - Verify error messages
- **Property tests** - Check parser invariants

### Performance Regression
- Benchmarks run on main branch commits
- 10% regression threshold triggers alerts

### API Stability
- `cargo-public-api` - Detects API changes
- `cargo-semver-checks` - Validates semantic versioning

## Security

### Supply Chain Security
- `cargo-deny` checks for:
  - Security advisories
  - License compatibility
  - Banned dependencies
  - Duplicate dependencies

### Unsafe Code
- `cargo-geiger` reports unsafe usage
- Summary posted to PR comments

## Coverage

Coverage is generated with `cargo-llvm-cov` and uploaded to Codecov as LCOV.

Local run:

```bash
cargo llvm-cov --workspace \
  --exclude adze-wasm-demo \
  --features "pure-rust,glr,incremental_glr,external_scanners,serialization,ts-compat" \
  --lcov \
  --output-path lcov.info
```

The first Codecov integration is advisory. Codecov checks and comments should not be required in branch protection until the baseline has stabilized on `main`.

Required `adze-swarm` branch protection remains `Rust Small Result` and
`Product Proof Result`.

## Maintenance

### Updating Dependencies
```bash
cargo update
cargo deny check
```

### Updating MSRV
1. Update `rust-version` in `Cargo.toml`
2. Update `.github/workflows/ci.yml` MSRV job
3. Test with: `cargo +1.XX.0 build --workspace`

## Troubleshooting

### Common Issues

**Snapshot test failures**
- Review with `cargo insta review`
- Accept changes if intentional

**Feature combination failures**
- Use `cargo hack` to test locally
- Consider adding feature gates

**API breaking changes**
- Run `cargo semver-checks` before PR
- Document breaking changes in CHANGELOG

**Fuzzing crashes**
- Minimize with `cargo fuzz tmin`
- Add regression test case

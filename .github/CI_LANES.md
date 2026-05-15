# CI Lane Map

**Last updated:** 2026-05-08
**Purpose:** Classify every CI check so contributors can immediately tell
whether a red mark means "must fix before merge" or "inspect at your leisure."

## Lane semantics

| Signal | Meaning | Contributor action |
|--------|---------|--------------------|
| **Required** | Merge is blocked. Fix before requesting review. | Must fix. |
| **PR-only signal** | Runs on every PR, informational, not a merge gate. | Review if red; not a blocker. |
| **Push / scheduled** | Runs on schedules or manual dispatch for legacy/deep lanes. Not PR-blocking. | Inspect trend; fix in a follow-up. |
| **Advisory** | Uses nightly / unstable toolchains, `continue-on-error`, or non-blocking labels. May be red due to toolchain drift. | Inspect if curious. Not actionable for most PRs. |

Branch protection in `adze-swarm` requires exactly one check:
**`Rust Small Result`** from `em-ci-routed-rust.yml`.

---

## Complete lane inventory

### Required (merge gate)

| Workflow | Job name | Trigger | Meaning |
|----------|----------|---------|---------|
| `em-ci-routed-rust.yml` | `Rust Small Result` | PR + merge_group | Aggregate check for the selected routed Rust small lane |

Required branch protection context: `Rust Small Result`.

### PR-only signal (non-blocking)

| Workflow | Job name | Trigger | Lane | Notes |
|----------|----------|---------|------|-------|
| `em-ci-routed-rust.yml` | `Route Rust Small` | PR + merge_group | PR-only | Selects CX43 or GitHub-hosted Rust small execution |
| `em-ci-routed-rust.yml` | `Rust Small on CX43` | PR + merge_group | PR-only | Runs when the trusted CX43 runner is idle |
| `em-ci-routed-rust.yml` | `Rust Small on GitHub Hosted` | PR + merge_group | PR-only | Fallback when CX43 is unavailable |
| `pr-gate.yml` | `Supported Rust Gate` | PR + merge_group | PR-only | Legacy public gate signal; not the swarm required context |
| `pr-gate.yml` | `PR Gate Success` | PR + merge_group | PR-only | Aggregate: plan + supported/docs gate |
| `pr-gate.yml` | `PR Plan` | PR | PR-only | Computes docs_only, estimated LEM, budget band |
| `ci.yml` | `ci-supported` | Schedule + dispatch | Scheduled/manual | Legacy public full-CI support lane; routed swarm PRs use `Rust Small Result` |
| `ci.yml` | `semver-checks` | PR only | PR-only | Detects breaking API changes |
| `ci.yml` | `api-stability` | PR only | PR-only | `cargo-public-api` diff; `continue-on-error` |
| `ci.yml` | `package-validation` | PR only | PR-only | Validates package manifests for release surface |
| `ci-policy.yml` | `CI Lane Whitelist` | PR + push | Advisory | xtask lane whitelist lint |
| `ci-policy.yml` | `Source of Truth` | PR + push | Advisory | doc-artifacts and active-goal ledger checks |
| `ripr.yml` | `ripr advisory` | PR | PR-only | Advisory report; non-blocking |
| `droid-review.yml` | `droid-review` | PR (non-draft) | PR-only | Factory Droid auto-review; `continue-on-error` |

### Push / scheduled (main health, not PR-blocking)

In `adze-swarm`, the legacy `ci.yml` jobs run on schedule or via `workflow_dispatch` with `run_full_ci`. They do **not** run on ordinary PRs or every merge to `main`; swarm PRs use `Rust Small Result` as the active base gate.

| Workflow | Job name | Trigger | Lane | Notes |
|----------|----------|---------|------|-------|
| `ci.yml` | `Lint` | Schedule + dispatch | Scheduled/manual | Full lint suite (bare no_mangle, debug blocks, fmt, clippy) |
| `ci.yml` | `Test` | Schedule + dispatch | Scheduled/manual | OS x features x toolchain matrix (3 OS, 4 features, 2 toolchains) |
| `ci.yml` | `Matrix Smoke Test` | Schedule + dispatch | Scheduled/manual | Workspace default + all-features test |
| `ci.yml` | `Test with Debug Assertions` | Schedule + dispatch | Scheduled/manual | Debug-assertion tests for glr-core, runtime, tablegen |
| `ci.yml` | `Test Release Mode` | Schedule + dispatch | Scheduled/manual | Release-mode tests with strict-invariants |
| `ci.yml` | `Benchmark Compilation` | Schedule + dispatch | Scheduled/manual | Bench compile check (no-run) |
| `ci.yml` | `Backend Build Matrix` | Schedule + dispatch | Scheduled/manual | pure-rust backend check + test |
| `ci.yml` | `Tree-sitter Compatibility API` | Schedule + dispatch | Scheduled/manual | ts-compat feature build + test |
| `ci.yml` | `Deterministic Codegen` | Schedule + dispatch | Scheduled/manual | Verifies build determinism |
| `ci.yml` | `Feature Matrix` | Schedule + dispatch | Scheduled/manual | Per-crate feature matrix checks |
| `ci.yml` | `Feature Matrix Extras` | Schedule + dispatch | Scheduled/manual | Feature powerset via cargo-hack |
| `ci.yml` | `MSRV (1.95.0)` | Schedule + dispatch | Scheduled/manual | Minimum Supported Rust Version check |
| `ci.yml` | `Security & Supply Chain` | Schedule + dispatch | Scheduled/manual | `cargo deny check` |
| `ci.yml` | `Documentation` | Schedule + dispatch | Scheduled/manual | `cargo doc --workspace` with `-D warnings` |
| `ci.yml` | `adze-python (Optimized Build)` | Schedule + dispatch | Scheduled/manual | Python grammar build + test |
| `ci.yml` | `Test Connectivity (Tripwires)` | Schedule + dispatch | Scheduled/manual | Enforces no disabled tests, non-zero discovery |
| `ci.yml` | `Code Coverage` | Schedule + dispatch | Scheduled/manual | `cargo llvm-cov` with threshold check |
| `ci.yml` | `Advisory / Unsafe Audit` | Schedule + dispatch | Advisory | `cargo geiger` report; `continue-on-error` |
| `ci.yml` | `Advisory / Cross Compilation (${{ matrix.target }})` | Schedule + dispatch | Advisory | 32-bit / ARM64 / WASM cross builds; `continue-on-error` |
| `ci.yml` | `Cross-platform` | Schedule + dispatch | Scheduled/manual | macOS + Windows cargo check + lib tests |
| `ci.yml` | `Advisory / WASM Build` | Schedule + dispatch | Advisory | WASM target check; `continue-on-error` |
| `ci.yml` | `Benches (unstable, opt-in)` | Dispatch only | Advisory | `unstable-benches` feature; only with `run_full_ci` |
| `pure-rust-ci.yml` | `Test Pure Rust Implementation` | Push + labeled PR | Push | OS x toolchain matrix for pure-rust path |
| `pure-rust-ci.yml` | `Test WASM Build` | Push + labeled PR | Advisory | WASM build + size check |
| `pure-rust-ci.yml` | `Golden Tests` | Push + labeled PR | Advisory | Tree-sitter parity; label-gated |
| `pure-rust-ci.yml` | `Integration Tests` | PR + push | PR-only | c2rust backend test |
| `pure-rust-ci.yml` | `Performance Regression Tests` | Push + labeled PR | Advisory | Benchmark run; label-gated |
| `pure-rust-ci.yml` | `Code Coverage` | Push + labeled PR | Advisory | Coverage report; label-gated |
| `core-tests.yml` | `core` | Scheduled (nightly) + dispatch | Scheduled | Full nightly canary: clippy, doc, all-features |
| `benchmarks.yml` | `Performance Benchmarks` | Push + labeled PR | Push | Benchmark comparison for PRs |
| `benchmarks.yml` | `Criterion HTML Report` | Dispatch only | Advisory | Manual Criterion HTML report generation |
| `coverage.yml` | `Codecov Coverage` | Push + labeled PR | Push | Dedicated coverage lane |
| `microcrate-ci.yml` | `Formatting` through `Strict Docs` | Push + path-routed PR | Push | Governance micro-crate tests |
| `golden-tests.yml` | `Golden Tests` | Push + path-routed PR | Push | Tree-sitter parity validation |
| `performance.yml` | `Performance Regression Check` | PR (path-routed) | PR-only | Benchmark comparison on perf-impact changes |
| `test-policy.yml` | `Enforce Test Policy` | Policy/docs PR + push + manual | Advisory | Test naming, disabled-test prevention, static inventory; runtime caps on push/manual with cold-compile hang guard |
| `mdbook.yml` | `build` + `deploy` | Push + PR | Push | Documentation site build |
| `smoke-ts-bridge.yml` | `smoke` | Push + PR | PR-only | ts-bridge link verification |
| `ts-bridge-smoke.yml` | `smoke` | Push + PR | PR-only | ts-bridge smoke with libtree-sitter |
| `release.yml` | Various release jobs | Dispatch only | Dispatch | Manual release workflow |

### Advisory (nightly / unstable / non-blocking)

These jobs use nightly toolchains, unstable features, or are explicitly marked
`continue-on-error: true`. Red here means "inspect" not "block."

| Workflow | Job name | Trigger | Why advisory |
|----------|----------|---------|-------------|
| `ci.yml` | `Advisory / Miri` | Schedule + dispatch | Nightly miri; `continue-on-error` |
| `ci.yml` | `Advisory / Sanitizers` | Schedule + dispatch | Nightly + `-Zbuild-std`; `continue-on-error` |
| `ci.yml` | `Advisory / Minimal Versions` | Schedule + dispatch | Nightly + `-Z minimal-versions`; `continue-on-error` |
| `ci.yml` | `Advisory / Cross Compilation (${{ matrix.target }})` | Schedule + dispatch | Cross toolchain drift; `continue-on-error` |
| `ci.yml` | `Advisory / WASM Build` | Schedule + dispatch | Compile-check only; `continue-on-error` |
| `ci.yml` | `Advisory / Unsafe Audit` | Schedule + dispatch | `cargo-geiger` may lag toolchain; `continue-on-error` |
| `product-proof.yml` | `ci-product advisory canaries` | Scheduled (weekly) + dispatch | Intentionally advisory; `continue-on-error` |
| `criterion-smoke.yml` | `benchmark` | Scheduled (weekly) + dispatch | Non-blocking; `continue-on-error` |
| `ts-bridge-parity.yml` | `parity` | Scheduled (nightly) + dispatch | Non-blocking; `continue-on-error` |
| `clippy-quarantine-report.yml` | `quarantine-report` | Scheduled (weekly) + dispatch | Report only |
| `droid-security-scan.yml` | `droid-security-scan` | Scheduled (weekly) + dispatch | Advisory scan; `continue-on-error` |
| `fuzz.yml` | `fuzz` | Scheduled + labeled PR + dispatch | Fuzz targets; time-boxed |
| `droid-review.yml` | `droid-review` | PR (non-draft) | AI review; `continue-on-error` |
| `droid.yml` | `droid` | @droid mentions | AI assistant; `continue-on-error` |

---

## Advisory job name convention

Advisory jobs in `ci.yml` carry the `Advisory / ` prefix so the GitHub Checks
UI makes their non-blocking nature immediately visible. The following renames
have already been applied:

| New name (current) | Previous name |
|--------------------|---------------|
| `Advisory / Miri` | `Miri (UB Detection)` |
| `Advisory / Sanitizers` | `Sanitizers (ASAN/UBSAN)` |
| `Advisory / Minimal Versions` | `Minimal Versions` |
| `Advisory / Cross Compilation (${{ matrix.target }})` | `Cross Compilation (...)` |
| `Advisory / WASM Build` | `WASM Build Verification` |
| `Advisory / Unsafe Audit` | `Unsafe Code Audit` |

Jobs in other workflows already carry clear names or are inherently advisory
(scheduled/dispatch-only).

---

## Branch protection

Current required status check (via `.github/settings.yml`):

```yaml
required_status_checks:
contexts:
    - "Rust Small Result"
```

This is correct and intentionally single-gated. All other checks are optional
signal.

---

## How to read the GitHub Checks panel

1. **`Rust Small Result` red?** — Stop. Fix before merge.
2. **`PR Gate / PR Gate Success` red?** — Inspect, but do not block the swarm
   base lane on it unless it is explicitly promoted again.
3. **Any `Advisory / *` red?** — Inspect when convenient. May be nightly drift.
4. **Scheduled/manual jobs red?** — Create a follow-up issue. Not a PR blocker.
5. **PR-only signal red?** — Worth reviewing, but not a merge blocker.

---

## Relationship to other docs

- **`docs/status/KNOWN_RED.md`** — Tracks intentional exclusions from the supported lane.
- **`docs/status/SUPPORT_TIERS.md`** — Maps feature surfaces to proof commands and CI lanes.
- **`.github/CI_README.md`** — General CI infrastructure documentation.
- **This file** — Lane classification and contributor-facing reading guide.

---

## Maintenance

When adding a new CI job:
1. Add it to the appropriate table above.
2. If advisory, use `continue-on-error: true` and the `Advisory / ` name prefix.
3. If required, update `.github/settings.yml` branch protection contexts **and** this file.
4. If push-only, ensure it does not trigger on PRs (use `if:` guards).

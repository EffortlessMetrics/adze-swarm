# CI Lane Map

**Last updated:** 2026-05-17
**Purpose:** Classify every CI check so contributors can immediately tell
whether a red mark means "must fix before merge" or "inspect at your leisure."

## Lane semantics

| Signal | Meaning | Contributor action |
|--------|---------|--------------------|
| **Required** | Merge is blocked. Fix before requesting review. | Must fix. |
| **PR-only signal** | Runs on every PR, informational, not a merge gate. | Review if red; not a blocker. |
| **Push / scheduled** | Runs on schedules or manual dispatch for legacy/deep lanes. Not PR-blocking. | Inspect trend; fix in a follow-up. |
| **Advisory** | Uses nightly / unstable toolchains, `continue-on-error`, or non-blocking labels. May be red due to toolchain drift. | Inspect if curious. Not actionable for most PRs. |

Branch protection in `adze-swarm` requires two aggregate checks:

- **`Rust Small Result`** from `em-ci-routed-rust.yml`.
- **`Product Proof Result`** from `product-proof.yml`.

`Product Proof Result` is the required Stable-claim proof gate. It remains an
aggregate context: Stable product canaries run only when path detection selects
Stable product surfaces, schedule, or manual dispatch, and the result passes
when no Stable product surface changed.

Runner capacity classes are defined in
[`docs/ci/runner-classes.md`](../docs/ci/runner-classes.md).

---

## Complete lane inventory

### Required (merge gate)

| Workflow | Job name | Trigger | Meaning |
|----------|----------|---------|---------|
| `em-ci-routed-rust.yml` | `Rust Small Result` | PR + merge_group | Aggregate check for the selected routed Rust small lane |
| `product-proof.yml` | `Product Proof Result` | PR + scheduled + dispatch | Aggregate Stable-claim proof gate for Product Proof path detection and selected stable canaries; skipped after cancelled upstream jobs so superseded PR runs do not queue stale result work |

Required branch protection contexts: `Rust Small Result`, `Product Proof Result`.

### PR-only signal (non-blocking)

| Workflow | Job name | Trigger | Lane | Notes |
|----------|----------|---------|------|-------|
| `em-ci-routed-rust.yml` | `Route Rust Small` | PR + merge_group | PR-only | Selects CPX42, CX43, CX33, or explicit fallback; the current route runner is diagnostics-only and excluded from idle counts; CX53 rust-small and planned rust-large candidates are logged with label/group diagnostics but quarantined from selection while #598 is blocked |
| `em-ci-routed-rust.yml` | `Rust Small on CPX42` | PR + merge_group | PR-only | Runs when the trusted CPX42 runner is idle |
| `em-ci-routed-rust.yml` | `Rust Small on CX43` | PR + merge_group | PR-only | Runs when the trusted CX43 runner is idle |
| `em-ci-routed-rust.yml` | `Rust Small on CX33` | PR + merge_group | PR-only | Runs when CPX42 and CX43 are unavailable and trusted CX33 capacity is idle |
| `em-ci-routed-rust.yml` | `Rust Small on CX53` | PR + merge_group | PR-only | Dormant while #598 is blocked; do not select for the required Rust Small route |
| `em-ci-routed-rust.yml` | `Rust Small on GitHub Hosted` | PR + merge_group | PR-only | Runs only when the route target is `github` and explicit fallback is allowed; skipped by default |
| `em-ci-routed-rust.yml` | `Rust Small Result` | PR + merge_group | Required | Aggregate required base gate; skipped after cancelled upstream route/lane jobs so superseded PR runs do not queue stale result work |
| `em-ci-routed-rust.yml` | `Runner Capacity / Fallback Policy` | PR + merge_group | PR-only | Fails intentionally when no trusted runner is idle and hosted fallback is not allowed; the aggregate `Rust Small Result` records that policy outcome |
| `pr-gate.yml` | `Supported Rust Gate` | PR + merge_group | PR-only | Legacy public gate signal; not the swarm required context; `pull_request.closed` cancels stale same-PR runs, and cancelled plan jobs do not schedule the supported lane |
| `pr-gate.yml` | `PR Gate Success` | PR + merge_group | PR-only | Aggregate: plan + supported/docs gate; skipped on `pull_request.closed` and after cancelled upstream advisory jobs |
| `pr-gate.yml` | `PR Plan` | PR | PR-only | Computes docs_only, estimated LEM, budget band; skipped on `pull_request.closed` |
| `ci.yml` | `ci-supported` | Schedule + dispatch | Scheduled/manual | Legacy public full-CI support lane; routed swarm PRs use `Rust Small Result` |
| `ci.yml` | `semver-checks` | PR only | PR-only | Detects breaking API changes |
| `ci.yml` | `api-stability` | PR only | PR-only | `cargo-public-api` diff; `continue-on-error` |
| `ci.yml` | `package-validation` | PR only | PR-only | Validates package manifests for release surface |
| `ci-policy.yml` | `CI Lane Whitelist` | PR + push | Advisory | xtask lane whitelist lint |
| `ci-policy.yml` | `Source of Truth` | PR + push | Advisory | doc-artifacts and active-goal ledger checks |
| `ripr.yml` | `ripr advisory` | PR | PR-only | Advisory report; non-blocking; `pull_request.closed` cancels stale same-PR runs and skips advisory work |

### Push / scheduled (main health, not PR-blocking)

In `adze-swarm`, the legacy `ci.yml` jobs run on schedule or via `workflow_dispatch` with `run_full_ci`. They do **not** run on ordinary PRs or every merge to `main`; swarm PRs use `Rust Small Result` as the active base gate.

| Workflow | Job name | Trigger | Lane | Notes |
|----------|----------|---------|------|-------|
| `ci.yml` | `Lint` | Schedule + dispatch | Scheduled/manual | Full lint suite (`cargo xtask check-no-mangle`, debug blocks, fmt, clippy) |
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
| `pure-rust-ci.yml` | `Test Pure Rust Implementation` | Code-path PR + dispatch | PR-only/manual | Skips docs/policy-only PRs; Ubuntu/stable on code-path PRs; full matrix on labels or dispatch |
| `pure-rust-ci.yml` | `Test WASM Build` | Labeled PR + dispatch | Advisory | WASM build + size check |
| `pure-rust-ci.yml` | `Golden Tests` | Labeled PR + dispatch | Advisory | Tree-sitter parity; label-gated |
| `pure-rust-ci.yml` | `Integration Tests` | Code-path PR + dispatch | PR-only | c2rust backend test; skipped for docs/policy-only PRs |
| `pure-rust-ci.yml` | `Performance Regression Tests` | Labeled PR + dispatch | Advisory | Benchmark run; label-gated |
| `coverage.yml` | `Detect Coverage Paths` | PR + dispatch | Advisory | Cheap path/label detector for coverage-lite/full routing |
| `coverage.yml` | `Coverage Lite` | Path/labeled PR + dispatch | Advisory | Core-package LCOV artifact; Codecov upload non-blocking |
| `coverage.yml` | `Coverage Full` | `full-ci` PR + dispatch | Advisory | Broader workspace/features LCOV artifact; Codecov upload non-blocking |
| `cx53-rust-large-diagnostic.yml` | `Probe CX53 Rust Large Capacity` | Dispatch only | Manual | Logs CX53 `rust-large` runner visibility and runs the smoke job only when idle capacity exists |
| `cx53-rust-large-diagnostic.yml` | `CX53 Rust Large Smoke` | Dispatch only | Manual | Selected host-smoke proof for the planned CX53 `rust-large` lane; not a PR or branch-protection gate |
| `product-proof.yml` | `Detect Product Proof Paths` | PR + scheduled + dispatch | PR-only/manual | Cheap detector that decides whether Stable product canaries are selected |
| `product-proof.yml` | `ci-product stable canaries` | Selected PR + scheduled + dispatch | PR-only/manual | Bounded Stable README/support-tier and claim-boundary docs proof lane; manual dispatch defaults to this lane |
| `core-tests.yml` | `core` | Scheduled (nightly) + dispatch | Scheduled | Full nightly canary: clippy, doc, all-features |
| `microcrate-ci.yml` | `Formatting` through `Strict Docs` | Path-routed PR + dispatch | PR-only/manual | Receipt jobs and crate-group tests route by affected Rust/package surface |
| `golden-tests.yml` | `Golden Tests` | Path-routed PR + dispatch | PR-only/manual | Tree-sitter parity validation |
| `test-policy.yml` | `Enforce Test Policy` | Policy/docs PR + manual | Advisory | Test naming, disabled-test prevention, static inventory; runtime caps on manual dispatch with cold-compile hang guard |
| `ts-bridge-smoke.yml` | `ts-bridge matrix setup` | Path-routed PR + dispatch | Advisory | Selects Linux-only default smoke or full OS matrix on explicit request |
| `ts-bridge-smoke.yml` | `smoke` | Path-routed PR + dispatch | PR-only/manual | Linux-only ts-bridge smoke by default; full OS on `platform-matrix`, `full-ci`, or dispatch |
| `badge-endpoints.yml` | `refresh` | Dispatch only | Advisory | Manual badge endpoint refresh that opens an automation PR |

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
| `product-proof.yml` | `ci-product advisory canaries` | Scheduled (weekly) + dispatch with `lane=all` | Broad advisory lane; `continue-on-error`; skipped on PRs and stable-only manual dispatches |
| `criterion-smoke.yml` | `benchmark` | Scheduled (weekly) + dispatch | Non-blocking; compile-checks `adze-benchmarks` |
| `ts-bridge-parity.yml` | `parity` | Scheduled (nightly) + dispatch | Non-blocking; `continue-on-error` |
| `clippy-quarantine-report.yml` | `quarantine-report` | Scheduled (weekly) + dispatch | Report only |

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
    - "Product Proof Result"
```

This is correct and intentionally aggregate-gated: `Rust Small Result` proves
the selected routed Rust base lane, and `Product Proof Result` proves Stable
claim path detection plus selected Stable product canaries. All implementation
jobs and broader advisory lanes remain optional signal unless explicitly
promoted later.

---

## How to read the GitHub Checks panel

1. **`Rust Small Result` red?** — Stop. Fix before merge.
2. **`Product Proof Result` red?** — Stop. Fix the Stable product surface or
   Product Proof routing failure before merge.
3. **`PR Gate / PR Gate Success` red?** — Inspect, but do not block the swarm
   base lane on it unless it is explicitly promoted again.
4. **`Runner Capacity / Fallback Policy` red while `Rust Small Result` is
   green?** — No trusted runner was idle and hosted fallback stayed blocked;
   inspect the route log, but the required aggregate context is the merge gate.
5. **Any `Advisory / *` red?** — Inspect when convenient. May be nightly drift.
6. **Scheduled/manual jobs red?** — Create a follow-up issue. Not a PR blocker.
7. **PR-only signal red?** — Worth reviewing, but not a merge blocker.

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

# ADZE-SPEC-0015: Required merge gate contract

Status: accepted
Owner: release/ci
Created: 2026-06-22
Linked proposal: ../proposals/ADZE-PROP-0001-0.9-contract-convergence.md
Linked ADRs:
Linked plan: ../../plans/0.9.0/implementation-plan.md
Linked issues: #769, #770
Linked PRs: #815
Support-tier impact: ../status/SUPPORT_TIERS.md
Policy impact: ../../.github/workflows/em-ci-routed-rust.yml, ../../.github/workflows/product-proof.yml

## Problem

The required merge gate (`Rust Small Result`) only proves the workspace *compiles*
— it runs `cargo fetch --locked && cargo check --workspace --locked` and nothing
else. No formatting check, no clippy, no tests. A PR that breaks formatting,
fails clippy `-D warnings`, or fails supported-crate unit tests will merge green
as long as `cargo check --workspace` succeeds.

Additionally, the routed-rust aggregate can silently report success (green) when
no implementation lane actually ran — any router-side failure (missing token,
API error, parse failure, capacity exhaustion) collapses to `exit 0` with a
notice, leaving the required gate green without proof.

There is no spec defining what the required gate must prove or the no-false-green
invariant. This spec closes that gap.

## Behavior

### B1. The required merge gate must prove more than compilation

The required context `Rust Small Result` (emitted by
`.github/workflows/em-ci-routed-rust.yml`) must, at minimum, run:

1. **Format check**: `cargo fmt --all -- --check` (or the repo's equivalent
   `scripts/fmt-workspace.sh --check` on the supported crate set).
2. **Lint**: `cargo clippy` with `-D warnings` on the 7 supported core crates
   (`adze`, `adze-macro`, `adze-tool`, `adze-common`, `adze-ir`,
   `adze-glr-core`, `adze-tablegen`).
3. **Unit/integration tests**: `cargo test --lib --tests --bins` on the same
   7 supported crates.

This is the same proof that `just ci-supported` / `scripts/ci-supported.sh`
runs locally. The merge gate must not be weaker than the local supported proof.

### B2. The aggregate must be red when no implementation lane executed

The `Rust Small Result` aggregate must report **failure** (red) whenever no
implementation lane actually ran. Specifically:

- **Router failure** (missing token, API non-2xx, JSON parse error): the
  aggregate must fail, not pass with a notice. A router outage means the gate
  could not verify the PR — that is a failure, not a silent pass.
- **Capacity exhaustion** (no self-hosted runner idle, no fallback label): the
  aggregate must fail. The `Runner Capacity / Fallback Policy` diagnostic job
  carries the red signal; the required aggregate must not override it to green.

The only case where the aggregate may pass without a self-hosted lane is an
**explicit GitHub-hosted fallback** (`target=github` with `fallback_allowed=true`),
which requires either a `workflow_dispatch` trigger or the `allow-github-hosted`
label on the PR.

### B3. `Product Proof Result` remains the second required context

`Product Proof Result` (emitted by `.github/workflows/product-proof.yml`) is
unchanged by this spec. It runs the stable-surface canaries when stable paths
are touched and short-circuits green otherwise. Both `Rust Small Result` and
`Product Proof Result` are required for merge.

### B4. Non-Goals

This spec does not:

- Define the routed-rust runner selection algorithm (that is operational, not
  a behavior contract).
- Require `just ci-supported` to be a required context by name — the contract
  is on *what the gate proves*, not *which recipe name* is used.
- Change the advisory lane model (pure-rust-ci, microcrate-ci, golden-tests,
  coverage, etc. remain advisory).

## Acceptance examples

| Scenario | Required gate result | Reason |
|---|---|---|
| PR breaks `cargo fmt` | **fail** | B1: format check is part of the gate |
| PR breaks clippy `-D warnings` on supported crates | **fail** | B1: lint is part of the gate |
| PR breaks a supported-crate unit test | **fail** | B1: tests are part of the gate |
| PR compiles but has no test/lint changes | **pass** (if all 3 checks pass) | B1: the gate proves all 3 |
| Router token missing | **fail** | B2: no lane ran |
| All runners busy, no fallback label | **fail** | B2: capacity exhaustion fails the gate |
| `allow-github-hosted` label present, GitHub lane passes | **pass** | B2: explicit fallback is allowed |

## Test mapping

| Behavior | Proof |
|---|---|
| B1 (gate proves fmt+clippy+tests) | The workflow YAML for the selected lane includes fmt/clippy/test steps; verified by reading `.github/workflows/em-ci-routed-rust.yml` |
| B2 (no false green) | The `rust-small-result` aggregate job exits non-zero on `target=github && fallback_allowed != true`; verified by reading the aggregate job's `case` statement |

## Implementation mapping

The fix for B1 is to either:
1. Add fmt/clippy/test steps to each `rust-small-*` implementation lane, OR
2. Promote `pr-gate.yml Supported Rust Gate` (which runs `just ci-supported`) to a required context.

The fix for B2 is to change the `target=github && fallback_allowed != true` branch
in `rust-small-result` from `exit 0` to `exit 1`.

## CI Proof

- `Rust Small Result` and `Product Proof Result` remain the two required contexts
  in branch protection (`docs/ci/branch-protection.md`).
- The local proof command `just ci-supported` remains the developer-facing
  equivalent.

## Metrics / Promotion Rule

This spec is immediately active (status: accepted). The implementation plan is
to land the B1 and B2 fixes in separate PRs linked to #769 and #770.

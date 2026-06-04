# CI Economics Rollout Inventory

Formal inventory of files that make up the CI economics rollout.
See `adze-rollout-status.md` for status of each item.

## Policy files (`policy/`)

| File | Purpose | State |
| --- | --- | --- |
| `ci-lane-whitelist.toml` | Every PR lane registered with owner, LEM, triggers, evidence | Active (advisory lint via `ci-policy.yml`) |
| `ci-risk-packs.toml` | Risk pack → lane routing map | Active (consumed by `xtask ci-plan`) |
| `ci-whitelist-exceptions.toml` | Exceptions for temporarily expensive default-PR lanes | Active; entries close as routing lands |
| `clippy-lints.toml` | Workspace lint policy manifest (active, staged, planned lints) | Active |
| `no-panic-allowlist.toml` | Semantic panic-family exception ledger with owner/expiry | Schema ready; populate via `cargo xtask no-panic-propose --baseline` |
| `non-rust-allowlist.toml` | Non-Rust file surface registrations | Active |
| `ripr-suppressions.toml` | ripr finding suppression ledger with owner/expiry | Active (empty baseline) |

## Workflow files (`.github/workflows/`)

| File | Role | State |
| --- | --- | --- |
| `em-ci-routed-rust.yml` | Routed `Rust Small Result` base gate for `adze-swarm` PRs and merge groups | Active required context |
| `cx53-rust-large-diagnostic.yml` | Manual CX53 `rust-large` probe and selected host-smoke diagnostic for #598 | Active dispatch |
| `pr-plan.yml` | Reusable; computes LEM/band/docs_only from `xtask ci-plan` | Active |
| `pr-gate.yml` | Aggregates Supported + Docs Gate → `PR Gate Success` | Active |
| `ci.yml` | Legacy public full-CI support lane; schedule/dispatch in swarm | Active scheduled/manual |
| `ci-policy.yml` | CI lane whitelist advisory lint | Active |
| `ripr.yml` | ripr advisory with workspace MSRV install and stub fallback | Active advisory |
| `pure-rust-ci.yml` | Matrix-setup: ubuntu/stable on code-path PRs; full matrix on labels/dispatch | Active |
| `ts-bridge-smoke.yml` | Path-routed bridge smoke; Linux by default, full OS matrix on labels/dispatch | Active |
| `ts-bridge-parity.yml` | Scheduled/manual bridge parity advisory lane | Active advisory |
| `golden-tests.yml` | Grammar-path and `ci:golden`/`full-ci` label gated | Active |
| `microcrate-ci.yml` | Risk-pack-routed crate groups plus path-routed receipt jobs | Active |
| `coverage.yml` | Path/label-routed coverage-lite/full; LCOV artifacts are proof and Codecov upload is non-blocking | Active advisory |
| `criterion-smoke.yml` | Scheduled/manual benchmark compile smoke | Active advisory |
| `product-proof.yml` | Required `Product Proof Result` aggregate gate plus selected Stable canaries and broad advisory product proof lanes | Active required aggregate; broad lane advisory |
| `core-tests.yml` | Scheduled/manual core all-features canary | Active scheduled/manual |
| `test-policy.yml` | Static policy checks on PRs plus manual runtime caps | Active advisory |
| `clippy-quarantine-report.yml` | Weekly/manual quarantine report | Active advisory |
| `badge-endpoints.yml` | Manual badge endpoint refresh automation | Active dispatch |

## xtask commands

| Command | Purpose |
| --- | --- |
| `cargo xtask ci-plan` | Compute CI plan from git diff + labels; emit `ci-plan.json` |
| `cargo xtask check-ci-lane-whitelist` | Lint workflow lanes against whitelist |
| `cargo xtask check-lint-policy` | Verify Clippy policy manifest |
| `cargo xtask check-no-panic-family` | Semantic panic debt checker |
| `cargo xtask check-file-policy` | Non-Rust surface checker |
| `cargo xtask policy-report` | Combined policy report |
| `cargo xtask no-panic-propose` | Propose new no-panic exception |

## Scripts (`scripts/ci/`)

| File | Purpose |
| --- | --- |
| `emit-ci-actuals.py` | Emits `ci-actuals.json` with plan vs actual LEM data |
| `pr-plan.py` | Python fallback for `xtask ci-plan` |

## Docs files (`docs/ci/`)

| File | Purpose |
| --- | --- |
| `adze-rollout-plan.md` | Original per-PR rollout plan |
| `adze-rollout-status.md` | Live status of each rollout item |
| `inventory.md` | This file — formal file inventory |
| `branch-protection.md` | Branch protection migration criteria |
| `change-class-surface.md` | Expected swarm CI proof surface by change class |
| `ci-lane-whitelist.md` | Whitelist usage docs |
| `ci-actuals.md` | ci-actuals telemetry schema docs |
| `cost-and-verification-policy.md` | CI economics verification policy |
| `coverage.md` | Coverage configuration |
| `labels.md` | CI label definitions |
| `learned-estimates.md` | Learned LEM estimate model (deferred; needs ≥30 days actuals) |
| `lem-budgeting.md` | LEM budgeting rules and band thresholds |
| `pr-plan.md` | PR Plan docs |
| `risk-packs.md` | Risk pack documentation |
| `ripr.md` | ripr advisory docs |
| `tooling-substrate.md` | Standard upstream tooling substrate and xtask wrapper doctrine |
| `verification-ladder.md` | Verification tier ladder |

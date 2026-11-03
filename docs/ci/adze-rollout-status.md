# CI Economics Rollout Status

Last review: 2026-05-14.

This file is a status snapshot, not a live source of truth. Before using it for
execution, refresh with `gh pr list` and the current workflow state.

## Bounded #1027 update (2026-11-03)

The legacy `pr-gate.yml` supported-core job is retired. Its planning,
selected docs-only workspace formatting, and per-run `ci-actuals` remain
on merge-group/manual events, reported as optional `PR Planning Result`.
The canonical `ci-supported` ledger id now points to the existing required
`em-ci-routed-rust.yml` supported-proof job; its static 20 LEM estimate is
retained, not recalibrated from one run.

The F04/B02/B03 and default-PR cost rows below describe the historical
rollout. The old promotion proposal is retired; the planning result does
not qualify Rust/product behavior. See [branch-protection.md](branch-protection.md)
and [ci-actuals.md](ci-actuals.md) for the current scope and telemetry boundary.

## Status legend

- ✅ landed — present on `main` and working
- 🟡 in progress — open PR or active follow-up
- ⏳ planned — not yet started
- ⏸ deferred — waiting on actuals or coordination

## Foundation layer

| # | Item | Status | Notes |
| --- | --- | --- | --- |
| F01 | CI lane whitelist (`policy/ci-lane-whitelist.toml`) | ✅ | All workflows mapped with owner, LEM, triggers |
| F02 | CI risk packs (`policy/ci-risk-packs.toml`) | ✅ | 10 risk packs; microcrate governance now routes only durable BDD governance support and governance integration tests |
| F03 | PR Plan workflow (`pr-plan.yml`) | ✅ | Calls `xtask ci-plan`, emits `ci-plan.json` with outputs `docs_only`, `estimated_lem`, `band` |
| F04 | PR Gate Success workflow (`pr-gate.yml`) | ✅ | Supported Gate + Docs Gate + `PR Gate Success` aggregator |
| F05 | ci-actuals telemetry scaffold | ✅ | `scripts/ci/emit-ci-actuals.py` emits plan vs actual; uploaded as artifact |
| F06 | ripr advisory (`ripr.yml`) | ✅ | Advisory install attempts run on the workspace MSRV toolchain and fall back to a stub report on install failure |

## Control plane

| # | Item | Status | Notes |
| --- | --- | --- | --- |
| C01 | CI policy workflow (`ci-policy.yml`) | ✅ | Runs `check-ci-lane-whitelist --mode advisory` plus source-of-truth ledger checks on every PR |
| C02 | Synchronize-only cancellation | ✅ | PR #563 merged — prevents label events from killing running jobs |
| C03 | Lane whitelist cost alignment | ✅ | PR #564 merged — corrects stale LEM for already-gated lanes |
| C04 | Real ripr provisioning | ✅ | PR #565 merged — graceful stub fallback remains; install now uses the workspace MSRV toolchain |
| C05 | Benchmark deduplication | ✅ | PR #566 merged — `performance-check` gated to `ci:perf` label |

## Routing

| # | Item | Status | Notes |
| --- | --- | --- | --- |
| R01 | Fuzz gating (label/push only) | ✅ | `fuzz.yml` — runtime fuzz requires `fuzz`/`full-ci` label or push/schedule |
| R02 | Pure-Rust PR matrix reduction | ✅ | `pure-rust-ci.yml` — ubuntu/stable default; full matrix on `platform-matrix`/`full-ci`/main |
| R03 | Golden tests grammar routing | ✅ | `golden-tests.yml` — paths + `ci:golden`/`full-ci` label gates |
| R04 | Microcrate CI risk-pack routing | ✅ | `microcrate-ci.yml` — per-group path detection after SRP collapse (governance integration, BDD support, parser support, core, runtime) |
| R05 | Benchmark PR ownership cleanup | ✅ | PR #566 merged — removes duplicate baseline+comparison from default PR path |

## Policy ledgers

| # | Item | Status | Notes |
| --- | --- | --- | --- |
| P01 | Clippy lint policy (`policy/clippy-lints.toml`) | ✅ | Schema, active, staged, and planned lints documented |
| P02 | No-panic allowlist (`policy/no-panic-allowlist.toml`) | ✅ | Schema in place; intentionally empty until `cargo xtask no-panic-propose --baseline` run |
| P03 | Non-Rust file policy (`policy/non-rust-allowlist.toml`) | ✅ | All non-Rust surfaces registered |
| P04 | ripr suppressions (`policy/ripr-suppressions.toml`) | ✅ | Schema in place; empty baseline |
| P05 | Workspace rust lints (`[workspace.lints.rust]`) | ✅ | `unsafe_op_in_unsafe_fn`, `unused_must_use`, `missing_docs`, `unused_extern_crates` |
| P06 | Strict Clippy Stage A (`[workspace.lints.clippy]`) | 🟡 | PR #567: `policy/clippy-stage-a` — `allow_attributes_without_reason` + stage-A rust lints |

## MSRV and toolchain

| # | Item | Status | Notes |
| --- | --- | --- | --- |
| T01 | MSRV 1.95 | ✅ | PR #760 landed after the microcrate collapse; Clippy policy refresh is the next lint ratchet |

## Branch protection

| # | Item | Status | Notes |
| --- | --- | --- | --- |
| B01 | Branch protection migration docs | ✅ | `docs/ci/branch-protection.md` — criteria for promoting required check |
| B02 | `PR Gate Success` stable run history | ⏸ | Needs ≥14 days and ≥5 distinct PRs per path before promotion |
| B03 | Migrate required check to `PR Gate Success` | ⏸ | After B02 — see `docs/ci/branch-protection.md` |

## Learned estimates

| # | Item | Status | Notes |
| --- | --- | --- | --- |
| L01 | ci-actuals collection | ✅ | Artifact upload on every PR Gate run |
| L02 | Learned LEM model | ⏸ | Needs ≥30 days of actuals; see `docs/ci/learned-estimates.md` |

## Effective default PR lane cost (current)

With all routing already in place, the effective default PR cost estimate:

| Lane | Effective default PR cost |
| --- | --- |
| PR Plan | ~1 LEM |
| Supported Rust Gate | ~20 LEM |
| PR Gate Success | ~1 LEM |
| CI Lane Whitelist | ~2 LEM |
| ripr advisory | ~4 LEM (isolated install attempted; stub report on toolchain/binary failure) |
| Test Policy | ~2 LEM |
| Pure Rust (ubuntu/stable only) | ~18 LEM |
| Microcrate CI (routed by risk pack) | ~5–20 LEM depending on changed surface |
| Fuzz build smoke (parser/glr paths only) | ~3 LEM |
| Criterion smoke | ~6 LEM |
| ts-bridge lanes | ~8 LEM |
| Clippy quarantine report | ~4 LEM |
| **Estimated total (typical runtime PR)** | **~55–70 LEM** |

Target is ≤35 LEM for ordinary PRs. The gap comes from `pure-rust-ci` (18 LEM)
and `microcrate-ci` (variable) running broadly on every PR. Active exceptions in
`policy/ci-whitelist-exceptions.toml` track these while tightening continues.

See `docs/ci/lem-budgeting.md` for budget bands and override labels.

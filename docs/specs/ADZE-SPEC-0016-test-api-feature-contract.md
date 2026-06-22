# ADZE-SPEC-0016: test-api feature contract

Status: accepted
Owner: core/test
Created: 2026-06-22
Linked proposal: ../proposals/ADZE-PROP-0001-0.9-contract-convergence.md
Linked ADRs:
Linked plan: ../../plans/0.9.0/implementation-plan.md
Linked issues: #771
Support-tier impact: ../status/SUPPORT_TIERS.md

## Problem

The `test-api` Cargo feature gates ~4,417 integration tests across 81 files in
`glr-core/tests/` (39% of the crate's test surface). The feature is non-default
and is NOT enabled by `just ci-supported` (the local supported proof command).
As a result, those tests are silently skipped in the supported lane — only
`core-tests.yml` (nightly) and `microcrate-ci.yml` (label-triggered) run them.

There is no spec defining whether `test-api` is a published-surface contract,
pure test infrastructure, or something in between. This spec closes that gap.

## Behavior

### B1. test-api is test infrastructure, not a published surface

The `test-api` feature exposes internal APIs for testing purposes (e.g.
`Driver`, `Forest`, `GSS` internals). It is **not** part of the stable public
API. Users should not depend on it.

### B2. test-api-gated tests are advisory, not supported proof

Integration tests gated behind `test-api` are **advisory** proof, not part of
the supported proof surface. The supported proof (`just ci-supported`) is not
required to enable `test-api`.

### B3. test-api tests must run in nightly CI

The `core-tests.yml` nightly workflow must enable `test-api` when running
`glr-core` tests. If nightly CI drops `test-api` coverage, that is a regression
in the advisory proof surface.

### B4. Non-Goals

This spec does not:
- Require `test-api` to be a default feature (it should stay opt-in).
- Require the supported proof to run `test-api` tests (they are advisory).
- Change the library's public API surface (`#[cfg(any(test, feature = "test-api"))]`
  guards are the correct pattern for test-only access to internals).

## Acceptance examples

| Scenario | Expected behavior |
|---|---|
| Developer runs `just ci-supported` | `test-api` tests are NOT run (advisory only) |
| Nightly `core-tests.yml` runs | `test-api` tests ARE run |
| External user builds `adze-glr-core` | `test-api` is off; internal APIs are hidden |
| Test file has `#![cfg(feature = "test-api")]` | It compiles only when the feature is enabled |

## Test mapping

| Behavior | Proof |
|---|---|
| B1 (test-api is infra) | The feature is non-default; documented here |
| B2 (advisory, not supported) | `scripts/ci-supported.sh` does not pass `--features test-api` |
| B3 (nightly runs them) | `core-tests.yml` includes `--features test-api` for glr-core |

## CI Proof

The supported gate (`Rust Small Result` + `Supported Proof`) runs `just ci-supported`
without `test-api`. The nightly lane runs with it. Both are correct per this spec.

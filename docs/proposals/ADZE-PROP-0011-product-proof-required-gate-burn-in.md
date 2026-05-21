# ADZE-PROP-0011: Product Proof required-gate burn-in

Status: accepted
Owner: release/product
Created: 2026-05-21

## Summary

Burn in the always-present `Product Proof Result` context before any branch
protection change requires it.

`Product Proof Result` now exists on every Product Proof PR event and aggregates
the Stable product canary selection. That closes the missing-check hazard, but
it does not by itself prove that the result context is ready to become a merge
gate. This lane records the required burn-in receipts and keeps the future
promotion separate from release, publish, signing, Cargo-token, and crates.io
install work.

## Problem

`docs/status/PRODUCT_OBJECTIVE_AUDIT.md` records that every Stable README claim
maps to the stable-product proof lane, but the lane remains advisory until
branch protection deliberately promotes `Product Proof Result`. The repo needs a
small, explicit burn-in step so agents do not either:

- treat the advisory result as already required; or
- flip branch protection without recent evidence that the result is stable on
  ordinary PRs.

## Goal

Define the receipt threshold for a future policy PR that may require
`Product Proof Result` alongside `Rust Small Result`.

## Non-Goals

- No branch-protection change in this lane-opening PR.
- No release tag, crate publish, signing, Cargo-token, or crates.io install
  work.
- No public `adze` implementation PRs.
- No new Stable support-tier claims.
- No `cargo install adze-cli` claim.
- No broad advisory `ci-product` default on routine PRs.

## Acceptance

- A fresh active goal owns Product Proof required-gate burn-in.
- The promotion criteria are documented in the CI branch-protection guidance.
- The criteria require recent green `Product Proof Result` receipts before any
  required-check PR.
- Future branch-protection promotion must update `.github/settings.yml`,
  `.github/CI_LANES.md`, and the product audit in the same PR.
- `Rust Small Result` remains the only required context until that future
  policy PR lands.

## Source Of Truth Links

- Spec: `ADZE-SPEC-0011`
- Plan: `plans/product-proof-required-gate/implementation-plan.md`
- Active goal: `.adze/goals/active.toml`
- Tracker: `EffortlessMetrics/adze-swarm#325`

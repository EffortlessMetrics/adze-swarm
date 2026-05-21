# ADZE-PROP-0010: Product Proof Result Readiness

Status: accepted
Owner: release/product
Created: 2026-05-21

## Summary

Prepare the Stable README claim proof lane for a future deliberate branch
protection promotion without making that promotion in this lane.

The current `Product Proof` workflow runs `ci-product stable canaries` for
Stable-claim and claim-boundary paths. That is useful advisory evidence, but
the workflow is path-filtered. A path-filtered job is not safe to require in
branch protection because unrelated PRs may never create the required check.

## Problem

`docs/status/PRODUCT_OBJECTIVE_AUDIT.md` records that Stable README claims map
to proof, but also records that `ci-product-stable` remains advisory. The next
non-release hardening step is to make this lane promotable without increasing
ordinary PR cost.

## Goal

Add a cheap, always-present Product Proof result check that can later become a
required branch-protection context. The expensive Stable canaries should still
run only when relevant paths, manual dispatch, or schedule request them.

## Non-Goals

- No branch-protection change.
- No release, tag, publish, signing, Cargo-token, or crates.io install work.
- No public `adze` implementation PRs.
- No new Stable support-tier claims.
- No `cargo install adze-cli` claim.
- No full advisory `ci-product` lane on routine PRs.

## Acceptance

- A fresh active goal owns this work in `adze-swarm`.
- The next implementation PR makes the Product Proof workflow create an
  always-present result check.
- The Stable canary job remains path/manual/schedule gated.
- The result check fails if selected Stable canaries fail.
- The result check passes with an explicit skip reason when no Stable product
  surface changed.
- Branch protection remains `Rust Small Result` only until a later explicit
  policy PR promotes Product Proof.

## Source Of Truth Links

- Spec: `ADZE-SPEC-0011`
- Plan: `plans/product-proof-result-readiness/implementation-plan.md`
- Active goal: `.adze/goals/active.toml`
- Tracker: `EffortlessMetrics/adze-swarm#325`

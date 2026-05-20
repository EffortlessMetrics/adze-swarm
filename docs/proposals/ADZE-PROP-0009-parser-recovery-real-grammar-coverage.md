# ADZE-PROP-0009: Parser Recovery Real-Grammar Coverage

Status: accepted
Owner: runtime/diagnostics
Created: 2026-05-20

## Summary

Close the next non-release product-trust gap named by
`docs/status/PRODUCT_OBJECTIVE_AUDIT.md`: broader real-grammar
parser-generated external-scanner recovery coverage.

Adze already has focused parser-v4 external-scanner canaries and a generated
external-token example recovery matrix. This proposal narrows the remaining gap
without promoting external scanners beyond their current support tier.

## Problem

The current recovery proof is strong for:

- generated object-like parser errors;
- parser-v4 external-scanner dispatch and rejected-token safety;
- the generated `external_word_example` diagnostic-document matrix.

The remaining audit gap is that this is still too narrow to support a broader
"real grammar external-scanner recovery is product-ready" claim.

## Goal

Add focused real-grammar recovery receipts that prove parser-generated
external-scanner grammars fail clearly under ordinary malformed input.

## Non-Goals

- No release, tag, publish, signing, or Cargo-token work.
- No public `adze` implementation PRs.
- No external-scanner Stable promotion.
- No full Tree-sitter external-scanner parity claim.
- No broad grammar corpus parity claim.

## Acceptance

- A fresh active goal owns this lane in `adze-swarm`.
- The next implementation PRs add focused recovery proof for generated
  external-scanner grammar shapes.
- Support-tier wording changes only after new proof commands exist.
- Any remaining real-grammar coverage limits stay explicit.

## Source Of Truth Links

- Spec: `ADZE-SPEC-0005`, `ADZE-SPEC-0011`
- ADR: `ADZE-ADR-0001`
- Plan: `plans/parser-recovery-real-grammar/implementation-plan.md`
- Active goal: `.adze/goals/active.toml`

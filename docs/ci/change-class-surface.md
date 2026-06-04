# Swarm CI surface by change class

Status: active guidance
Owner: repo governance
Created: 2026-06-04
Linked issue: adze-swarm#617
Linked operating model: `docs/reference/adze-swarm-operating-model.md`
Support-tier impact: none
Policy impact: none

This document classifies the expected `adze-swarm` CI proof surface by change
class. It is guidance for planning and review; it does not change workflow
routing, branch protection, release authority, support tiers, or hosted fallback
policy.

`Rust Small Result` remains the normalized base gate for swarm PRs. Additional
lanes should be selected only by path routing, labels, schedule, manual dispatch,
or an explicit source-of-truth plan item.

## Expected surfaces

| Change class | Expected PR surface | Notes |
| --- | --- | --- |
| Docs-only | `Rust Small Result`; docs/source-of-truth smoke when selected | Docs-only changes should not require full CI, coverage, platform matrix, release workflows, or public `adze` promotion. |
| CI policy docs | `Rust Small Result`; CI policy/source-of-truth checks when selected | Documentation of CI behavior is not a workflow behavior change by itself. |
| Workflow or runner routing | `Rust Small Result`; workflow-specific proof from the linked issue or plan | Runner labels and route policy must stay explicit. Do not broaden hosted fallback silently. |
| `tools/ts-bridge` | `Rust Small Result`; `ts-bridge` smoke or parity lane when path-routed, labeled, or manually selected | Bridge lanes remain advisory unless a source-of-truth item makes a narrower gate explicit. |
| Runtime/core/macro/common | `Rust Small Result`; affected Rust lane or `just ci-supported` when the supported core pipeline changes | Runtime and parser changes need proof tied to the selected spec or plan item. |
| Grammar/golden | `Rust Small Result`; golden path when grammar, fixture, or parity files change | Do not claim broader Tree-sitter or query parity than the selected receipts prove. |
| Coverage | `coverage-lite` by path or label; `coverage-full` manual or scheduled | Coverage is an evidence lane, not a default PR tax. |
| Product proof | `Product Proof Result`; selected stable canaries only when stable product surfaces change | Product proof constrains README/support-tier claims; it is not release authorization. |
| Release or publish | Public `EffortlessMetrics/adze` only after explicit authorization | `adze-swarm` can prepare decision packets and release-candidate proof, but it does not publish, tag, sign, use Cargo tokens, or claim crates.io install receipts. |

## Review rule

Every PR should state:

- source-of-truth issue or artifact,
- claim boundary,
- proof commands,
- CI cost expectation,
- rollback,
- what the PR does not authorize.

If the expected surface is unclear, ask for the missing source-of-truth decision
instead of adding broad CI fanout.

# ADZE-PROP-0012: Parser Runtime Maintainability Hardening

Status: accepted
Owner: runtime/product
Created: 2026-05-21
Target milestone: post-0.9 / non-release maintainability
Linked specs:
- docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
- docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
Linked ADRs:
- ADZE-ADR-0001-adze-document-one-parse-truth
Linked plan:
- ../../plans/parser-runtime-maintainability/implementation-plan.md
Linked issues:
- EffortlessMetrics/adze-swarm#325
Linked PRs:
- EffortlessMetrics/adze-swarm#443
- EffortlessMetrics/adze-swarm#444
- EffortlessMetrics/adze-swarm#445
- EffortlessMetrics/adze-swarm#446
- EffortlessMetrics/adze-swarm#447
- EffortlessMetrics/adze-swarm#448
- EffortlessMetrics/adze-swarm#449
Support-tier impact:
- No support-tier promotion by campaign setup.
Policy impact:
- Keeps development and maintainability work in `EffortlessMetrics/adze-swarm`.
- Keeps release, tag, publish, signing, Cargo-token, and crates.io install
  receipt work in public `EffortlessMetrics/adze` after explicit authorization.

## Problem

The release-boundary/product-trust lane is paused because routine swarm proof is
complete and the remaining release work requires explicit human authorization.
The repo still needs a safe non-release lane for parser/runtime maintenance so
agents do not keep refreshing status receipts, reopening solved product-proof
work, or drifting implementation PRs into public `adze`.

The risk is not a missing foundation campaign. The risk is entropy in the
supported parser/runtime surfaces after fast productization: small refactors,
diagnostic fixes, tablegen changes, query hardening, and projection maintenance
need a source-of-truth lane that keeps proof narrow and claims honest.

## Users And Surfaces

- Rust parser users need the generated-parser and typed extraction path to stay
  boring and reliable.
- Tooling users need `parse_document()`, diagnostics, projections, and selected
  Tree-sitter-compatible views to remain coherent.
- Maintainers need small PRs with focused proof rather than broad refactor churn.
- Release reviewers need a clear boundary between swarm maintenance and public
  publish authorization.

## Success Criteria

- `adze-swarm` remains the only target for non-release implementation and proof
  work.
- Public `adze` remains release/public-intake/promotion/publish surface only.
- The active goal names a maintainability lane rather than a paused release
  boundary or a completed product-proof campaign.
- Parser/runtime/tablegen maintenance PRs link back to this lane and name their
  focused proof commands.
- Stable product claims are not promoted unless support-tier rows and proof
  receipts change together.
- Release/publish blockers remain tracked on issue #325 and are not treated as
  completed without explicit authorization and real crates.io install receipts.

## Proposed Shape

Use this lane for small, maintainability-driven work:

```text
source-of-truth setup
  -> supported-surface audit
    -> focused parser/runtime/tablegen hardening PRs
      -> closeout and release-boundary refresh
```

The default task shape is a narrow PR with a bounded diff, a local proof command
for the edited surface, and the aggregate GitHub gates left to `Rust Small
Result` and `Product Proof Result`.

## Alternatives Considered

### Continue The Paused Release Goal

Rejected. The paused lane has release/publish blockers left. Continuing routine
development under that manifest makes it too easy to blur non-release
maintenance with tag, publish, signing, Cargo-token, and crates.io work.

### Start Release Work Implicitly

Rejected. Release execution requires explicit human authorization and belongs
in public `adze` after an explicit promotion/sync from `adze-swarm`.

### Keep Creating One-Off Maintenance PRs

Rejected. One-off PRs without a current active goal invite duplicate work,
claim drift, and broad refactor churn.

## Specs To Create Or Update

No new behavior spec is required for the campaign setup. Existing specs remain
authoritative:

- `ADZE-SPEC-0011` for product proof, support tiers, and claim boundaries.
- `ADZE-SPEC-0012` for the GLR toolkit product contract.

Create or update behavior specs only when a maintenance PR changes a behavior
contract.

## Architecture Decisions Needed

No new ADR is required at campaign start. The durable rule remains
`AdzeDocument` as the one parse truth. Tree-sitter compatibility, query, JSON,
and CLI output remain projections over the supported parser/document facts.

## Implementation Campaign Shape

1. Start the parser/runtime maintainability active goal.
2. Confirm release authorization remains blocked on #325.
3. Audit supported parser, runtime, tablegen, diagnostics, query, and
   projection surfaces for small proof-backed maintenance candidates.
4. Land focused hardening PRs only when the improvement has a product or proof
   reason.
5. Refresh closeout notes only when there is a new material fact.

## Evidence Plan

- Source-of-truth proof:
  - `cargo run -q -p xtask -- check-active-goal --mode blocking`
  - `cargo run -q -p xtask -- check-doc-artifacts --mode blocking`
- Product and supported proof:
  - `just ci-product-stable`
  - `CARGO_PROFILE_TEST_DEBUG=0 just ci-supported`
- Focused proof:
  - parser/runtime/tablegen commands chosen per PR
- Hygiene:
  - `git diff --check`

## Risks

- Maintainability work can become unbounded refactor churn. Keep PRs tied to a
  parser/runtime proof or product surface.
- Agents can drift back into public `adze`. Keep the active goal and PRs
  explicit that non-release work targets `adze-swarm`.
- Support-tier claims can creep through docs. Claim promotion requires support
  tier and proof updates, not wording alone.
- Release blockers can be treated as routine work. Keep #325 as the explicit
  authorization checkpoint.

## Non-Goals

- No tag, crate publish, signing, Cargo-token, or release workflow work.
- No `cargo install adze-cli` claim until a real crates.io install receipt
  exists.
- No public `adze` implementation PRs.
- No support-tier promotion by campaign setup.
- No full Tree-sitter, query, incremental, or performance parity claim.

## Exit Criteria

This campaign is complete when the near-term parser/runtime maintainability
queue is empty or superseded, focused hardening PRs have receipts for the
surfaces they touched, and release-only work remains explicitly blocked on #325
rather than mixed into normal swarm development.

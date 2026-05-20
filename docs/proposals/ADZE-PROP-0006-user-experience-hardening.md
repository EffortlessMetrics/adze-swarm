# ADZE-PROP-0006: User Experience Hardening

Status: accepted
Owner: runtime/product
Created: 2026-05-20
Target milestone: post-0.9 / adoption polish
Linked specs:
- docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
- docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
- docs/specs/ADZE-SPEC-0013-query-compatibility.md
- docs/specs/ADZE-SPEC-0014-performance-and-regression.md
Linked ADRs:
- ADZE-ADR-0001-adze-document-one-parse-truth
- ADZE-ADR-0003-summary-first-glr-ambiguity
- ADZE-ADR-0004-schema-versioned-projections
Linked plan:
- ../../plans/user-experience-hardening/implementation-plan.md
Linked issues:
- EffortlessMetrics/adze-swarm#325
Linked PRs:
- none yet
Support-tier impact:
- Prepares usability and proof receipts without promoting support tiers by itself.
Policy impact:
- Keeps development and proof work in `EffortlessMetrics/adze-swarm`.
- Keeps release, tag, publish, signing, Cargo-token, and crates.io install
  receipt work in public `EffortlessMetrics/adze` after explicit authorization.

## Problem

The release-boundary/product-trust lane is paused because routine proof work is
complete and the remaining actions require explicit human release/publish
authorization. Agents still need a non-release development lane so they do not
keep reopening release blocker status PRs or inventing work in public `adze`.

Adze also still benefits from adoption polish: starter-project quality,
example coverage, API navigation, diagnostics/query/Tree-sitter usability, and
local developer ergonomics can improve without changing release claims.

## Users And Surfaces

- New users need starter projects and examples that feel complete.
- Rust library authors need the generated parser path to stay obvious.
- Tooling authors need examples around `parse_document()`, diagnostics,
  selected-tree compatibility, queries, and JSON.
- Maintainers need local proof loops that remain runnable on Windows and
  constrained machines.
- Release reviewers need this non-release work to avoid changing public release
  claims without support-tier proof.

## Success Criteria

- `adze-swarm` remains the only target for non-release development work.
- Public `adze` remains release/public-intake and publish surface only.
- Starter, examples, diagnostics, query, Tree-sitter, and performance docs
  become easier to use without unsupported claims.
- Local developer proof friction is recorded and reduced when it blocks routine
  work.
- Release/publish blockers stay tracked on issue #325 and are not treated as
  completed without explicit authorization and real install receipts.

## Proposed Shape

Use this lane for small, user-facing polish and proof improvements:

```text
starter project
  -> examples
    -> API navigation
      -> diagnostics/query/ts-compat walkthroughs
        -> performance receipt guidance
          -> local proof-loop ergonomics
```

Each PR should be small, evidence-backed, and clear about what claim boundary it
does not change.

## Alternatives Considered

### Continue The Paused Release Goal

Rejected. That lane has only release/publish blocked items left. Continuing to
edit status files under it creates churn without a new material fact.

### Start Release Work Implicitly

Rejected. Release, publish, signing, Cargo-token, and crates.io install receipt
work requires explicit human authorization and belongs in public `adze`.

### Do Nothing Until Release

Rejected. Non-release adoption polish can continue safely in `adze-swarm` as
long as it does not broaden claims or touch release machinery.

## Specs To Create Or Update

No new behavior spec is required at campaign start. Existing specs remain
authoritative:

- `ADZE-SPEC-0011` for claim proof and support tiers.
- `ADZE-SPEC-0012` for the GLR toolkit product contract.
- `ADZE-SPEC-0013` for query compatibility.
- `ADZE-SPEC-0014` for performance evidence.

Create new specs only if this campaign exposes a new behavior contract, not for
ordinary example or docs polish.

## Architecture Decisions Needed

No new ADR is required at campaign start. The durable rules remain:

- `AdzeDocument` is the one parse truth.
- GLR ambiguity is summary-first.
- Serialized projections are schema-versioned.
- Tree-sitter and query compatibility remain documented subsets until support
  tiers prove broader claims.

## Implementation Campaign Shape

1. Start the user-experience hardening active goal.
2. Keep release authorization tracked on #325 without starting release work.
3. Polish the starter and downstream examples where focused proof exists.
4. Improve API navigation and example discoverability.
5. Add or refresh diagnostics/query/Tree-sitter walkthroughs only when backed by
   runnable examples.
6. Add performance receipt guidance without public speed claims.
7. Reduce local proof-loop friction when it blocks `just ci-supported` or the
   small product lanes.

## Evidence Plan

- Source-of-truth proof:
  - `cargo run -q -p xtask -- check-doc-artifacts --mode blocking`
  - `cargo run -q -p xtask -- check-active-goal --mode blocking`
- Local supported proof:
  - `just ci-supported`
- Product proof where relevant:
  - `just ci-product-stable`
  - focused example or CLI tests for the edited surface
- Hygiene:
  - `git diff --check`

## Risks

- Adoption docs can accidentally become release claims. Keep support tiers as
  the claim source of truth.
- Agents can drift back into public `adze`. PR templates and active goals must
  continue to name `adze-swarm` as the development target.
- Release blockers can be treated as routine work. Keep #325 as the explicit
  authorization checkpoint.

## Non-Goals

- No tag, crate publish, signing, Cargo-token, or release workflow work.
- No `cargo install adze-cli` claim until a real crates.io install receipt
  exists.
- No support-tier promotion by docs polish alone.
- No full Tree-sitter compatibility claim.
- No full query parity claim.
- No stable incremental reuse or speedup claim.

## Exit Criteria

This campaign is complete when the near-term adoption polish queue is empty or
superseded, local proof-loop friction has current receipts, and any remaining
release-only work is still explicitly blocked on #325 rather than mixed into
normal swarm development.

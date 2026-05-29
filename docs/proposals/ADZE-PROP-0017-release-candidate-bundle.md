# ADZE-PROP-0017: Release Candidate Bundle Readiness

Status: accepted
Owner: release/product
Created: 2026-05-29
Target milestone: next public promotion
Linked specs:
- docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADRs:
- ADZE-ADR-0001-adze-document-one-parse-truth
Linked plan:
- ../../plans/release-candidate-bundle/implementation-plan.md
Linked issues:
- EffortlessMetrics/adze-swarm#325
- EffortlessMetrics/adze-swarm#549
Support-tier impact:
- Does not promote any support tier.
- Requires any candidate bundle to preserve existing support-tier claim
  boundaries and known limitations.
Policy impact:
- Keeps `EffortlessMetrics/adze-swarm` as the development/proof forge.
- Keeps public `EffortlessMetrics/adze` as the release, promotion, publish,
  signing, Cargo-token, and crates.io receipt surface.

## Problem

`adze-swarm` has the operating model for a development and proof forge, and it
has recent non-release proof receipts. The next release-facing step still must
not be tag or publish work: release authorization and the real crates.io
install receipt remain blocked on #325.

What is still useful inside `adze-swarm` is a reviewable release-candidate
bundle shape. Maintainers need one place to see:

- the selected swarm commit;
- open PR queue state in both repos;
- public drift state;
- proof commands and receipts;
- claim boundaries;
- rollback path;
- blocked release actions that still require human authorization.

Without that bundle, agents can keep refreshing scattered receipts or mistake
pre-publish readiness for permission to publish.

## Users And Surfaces

- Maintainers need a concise review packet before authorizing public promotion
  or release work.
- Release reviewers need to distinguish non-publish preflight evidence from a
  real post-publish crates.io install receipt.
- Agents need a bounded non-release lane that does not touch public `adze`.
- Users need public docs to avoid unsupported `cargo install adze-cli` claims
  until the registry receipt exists.

## Success Criteria

- A new active goal selects release-candidate bundle readiness as the current
  non-release `adze-swarm` lane.
- The lane defines the candidate snapshot, proof commands, claim boundaries,
  drift checks, and rollback evidence needed before public promotion.
- The lane keeps public promotion, tag, publish, signing, Cargo-token, and
  real crates.io install verification out of `adze-swarm`.
- The lane does not broaden Stable, parity, performance, or install claims.

## Proposed Shape

The campaign should use small PRs:

1. source-of-truth PR for this proposal, plan, active goal, and artifact ledger;
2. current candidate snapshot and queue/drift refresh;
3. promotion-bundle checklist or reference page;
4. non-publish proof receipt refresh;
5. closeout that records whether maintainers should proceed, defer, or split.

## Evidence Plan

Source-of-truth proof:

```bash
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

Candidate bundle proof may include:

```bash
gh pr list --repo EffortlessMetrics/adze-swarm --state open --json number,title,url
gh pr list --repo EffortlessMetrics/adze --state open --json number,title,url
just ci-supported
just ci-product-stable
just check-publishable
cargo run -q -p xtask -- verify-crates-io-install adze-cli --bin adze --version X.Y.Z --locked --dry-run
```

The install verifier dry run is command-shape evidence only. It is not a
crates.io install receipt.

## Non-Goals

- No public `adze` PR.
- No release tag.
- No crate publish.
- No signing workflow change.
- No Cargo-token work.
- No crates.io install command.
- No `cargo install adze-cli` claim.
- No Tree-sitter parity, query parity, incremental performance, GLR generality,
  or benchmark performance claim promotion.

## Exit Criteria

This campaign is complete when `adze-swarm` has a current release-candidate
bundle that a maintainer can use to decide one of:

- authorize public promotion or release work under #325;
- defer release with named blockers;
- split remaining work into another non-release lane.

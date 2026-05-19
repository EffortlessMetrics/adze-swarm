# ADZE-PROP-0005: Release Promotion Readiness

Status: accepted
Owner: release/product
Created: 2026-05-19
Target milestone: next public release promotion
Linked specs:
- docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADRs:
- ADZE-ADR-0001-adze-document-one-parse-truth
Linked plan:
- ../../plans/release-promotion/implementation-plan.md
Linked issues:
- none yet
Linked PRs:
- none yet
Support-tier impact:
- Requires every public claim promoted from `adze-swarm` to public `adze` to
  match `docs/status/SUPPORT_TIERS.md`.
- Does not create new Stable claims by itself.
Policy impact:
- Keeps `EffortlessMetrics/adze-swarm` as the working repo.
- Treats public `EffortlessMetrics/adze` as release/public-intake until an
  explicit promotion PR is prepared.

## Problem

The `adze-swarm` repo now has completed source-of-truth, GLR toolkit, and
toolkit excellence campaigns. That creates a good product-shaped state, but it
does not automatically mean public `EffortlessMetrics/adze` should be updated
opportunistically.

Public promotion needs a deliberate readiness pass:

- confirm what landed in `adze-swarm`;
- confirm public `adze` has not drifted in a conflicting way;
- freeze release-facing claims against support tiers;
- choose what to promote, defer, or keep swarm-only;
- prepare a public promotion PR with proof and rollback.

Without this campaign, agents can either keep working from a completed manifest
or accidentally reopen public-repo drift.

## Users And Surfaces

- Users need public `adze` release notes and README claims to match proof.
- Maintainers need a clear promotion inventory before touching public `adze`.
- Agents need a new active manifest that says what happens after toolkit
  excellence closeout.
- Release reviewers need to distinguish swarm readiness from public release.

## Success Criteria

- `adze-swarm` has a release-readiness inventory for completed campaigns.
- Public `adze` drift is audited before any promotion PR.
- README, support tiers, product proof map, and known-red status are aligned.
- Any public promotion PR has proof commands, non-goals, claim boundaries, and
  rollback.
- Work remains in `adze-swarm` until a promotion PR is explicitly prepared.

## Proposed Shape

The campaign should sequence:

1. open release-promotion readiness source of truth;
2. inventory completed swarm campaigns and current support-tier claims;
3. audit public `adze` for drift against `adze-swarm`;
4. freeze release-facing claims and known limitations;
5. prepare the public promotion PR plan;
6. close out with the exact public promotion decision.

## Alternatives Considered

### Promote Immediately

Rejected. The toolkit is now product-shaped, but public promotion needs a
separate claim and drift check.

### Keep Working Without An Active Goal

Rejected. A completed active manifest invites duplicate or chat-derived work.

### Continue Adding Toolkit Features First

Rejected for this lane. New product work should start from a new proposal after
release readiness is understood.

## Specs To Create Or Update

No new behavior spec is required at campaign start.

Existing contracts remain authoritative:

- `ADZE-SPEC-0011` for support-tier and product-proof behavior.
- `docs/status/SUPPORT_TIERS.md` for public claim tiers.
- `docs/status/PRODUCT_PROOF_MAP.md` for release-readable claim summaries.

## Architecture Decisions Needed

No new ADR is required at campaign start.

The durable repo rule remains:

```text
adze-swarm = working repo
public adze = release/public-intake surface
```

## Implementation Campaign Shape

The campaign should use small docs/proof PRs:

1. campaign source-of-truth PR;
2. release-readiness inventory PR;
3. public-drift audit PR;
4. release-claim freeze PR;
5. public promotion plan PR;
6. closeout PR.

## Evidence Plan

- Source-of-truth:
  - `cargo run -q -p xtask -- check-active-goal --mode blocking`
  - `cargo run -q -p xtask -- check-doc-artifacts --mode blocking`
  - `git diff --check`
- Public drift:
  - live `gh pr list` checks for `EffortlessMetrics/adze` and
    `EffortlessMetrics/adze-swarm`
  - explicit compare command between public and swarm heads
- Claim proof:
  - support-tier rows and product proof map
  - `Rust Small Result`
  - product-stable canaries when release-facing claims move

## Risks

- Public promotion can accidentally include swarm-only CI or runner changes.
- Release docs can overclaim Stabilizing surfaces as Stable.
- Public `adze` can receive new drift while the promotion plan is being built.
- Agents can mistake this campaign for permission to work directly in public
  `adze`.

## Non-Goals

- No public `adze` PR in the campaign source-of-truth PR.
- No release tag.
- No publish/signing/Cargo token workflow changes.
- No branch-protection changes.
- No new Stable claims.
- No new runtime features.

## Exit Criteria

This campaign is complete when one of these outcomes is recorded:

- a public promotion PR is prepared with proof and rollback;
- promotion is explicitly deferred with reasons and next conditions; or
- promotion is split into smaller named campaigns.

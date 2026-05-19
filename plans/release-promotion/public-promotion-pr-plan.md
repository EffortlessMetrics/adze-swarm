# Public Promotion PR Plan

Status: complete
Owner: release/product
Created: 2026-05-19
Linked proposal: ../../docs/proposals/ADZE-PROP-0005-release-promotion-readiness.md
Linked plan: ./implementation-plan.md
Readiness inventory: ./readiness-inventory.md
Public drift audit: ./public-drift-audit.md
Claim freeze: ./claim-freeze.md

## Purpose

This plan defines how to prepare a future public `EffortlessMetrics/adze`
promotion PR from `EffortlessMetrics/adze-swarm`. It does not open that public
PR, tag a release, publish crates, or change release/signing workflows.

This plan is a completed readiness artifact, not an active authorization to
start public-repo work from a stale snapshot. Before opening any public
promotion PR, create or select a fresh execution goal, refresh live public and
swarm PR queues, and rerun the proof commands below from current
`adze-swarm/main`.

## Promotion Preconditions

Before opening a public promotion PR:

- `adze-swarm/main` is clean and current.
- Public `EffortlessMetrics/adze` has no open PR that conflicts with the
  promotion scope.
- Existing open `adze-swarm` PRs are merged, superseded, closed, or explicitly
  deferred from the promotion candidate.
- Post-readiness product-proof alignment PRs merged after closeout are included
  or deliberately excluded by name.
- `plans/release-promotion/readiness-inventory.md`,
  `plans/release-promotion/public-drift-audit.md`, and
  `plans/release-promotion/claim-freeze.md` are current.
- README Stable claims still pass the support-tier guard.
- No release/publish/signing/Cargo-token workflow change is included unless a
  separate release-surface plan explicitly owns it.

## Candidate Scope

The candidate public PR should promote the release-readable state already
merged to `adze-swarm/main`:

- source-of-truth stack and agent operating discipline;
- 0.9 contract-convergence closeout and release-operation proof receipts;
- GLR toolkit productization receipts and examples;
- toolkit excellence and adoption receipts;
- release promotion readiness inventory, drift audit, and claim freeze;
- public #783 file-policy migration-candidate reporting, now ported through
  `adze-swarm`;
- README capability table aligned with `SUPPORT_TIERS.md`;
- product-proof alignment updates through `adze-swarm` #241-#246.

The candidate public PR should not include:

- unmerged `adze-swarm` PRs observed when the promotion branch is prepared
  unless they are separately refreshed and merged to `adze-swarm/main`;
- release/publish/signing workflow changes;
- Cargo token, crates.io publishing, or tag automation changes;
- branch-protection or merge-queue changes;
- new Stable claims beyond the claim freeze;
- full Tree-sitter parity, full query parity, stable CLI/WASM schema,
  stable raw GLR forest export, or stable incremental performance claims.

## Required Proof Before Opening Public PR

Run from `adze-swarm/main`:

```bash
git status --short
gh pr list --repo EffortlessMetrics/adze-swarm --state open
gh pr list --repo EffortlessMetrics/adze --state open
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
cargo test -p adze-cli readme_stable_claims_are_in_stable_product_lane -- --exact --nocapture
git diff --check
```

Run release-surface proof from the public-promotion branch before requesting
review:

```bash
just ci-supported
just ci-product-stable
just check-publishable
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

If package verification needs local co-release patching, use the local package
proof commands recorded in `../0.9.0/closeout.md` instead of weakening the
claim.

## Public PR Body Skeleton

```md
## Summary

Promotes the release-readable `adze-swarm` state into public `adze`.

## Source-of-truth links

- Proposal: ADZE-PROP-0005
- Readiness inventory: plans/release-promotion/readiness-inventory.md
- Public drift audit: plans/release-promotion/public-drift-audit.md
- Claim freeze: plans/release-promotion/claim-freeze.md
- Support tiers: docs/status/SUPPORT_TIERS.md

## Scope

What is promoted.

## Exclusions

Release/publish/signing, branch protection, unmerged swarm PRs, and unsupported
Stable claims are excluded.

## Proof

Paste fresh proof commands and results.

## Rollback

Close the public PR if unmerged. If merged before tagging, revert the squash
merge. If tagged or published, follow the release incident process instead of a
silent revert.
```

## Rollback

If the promotion PR has not merged, close it.

If the promotion PR has merged but no release tag or publish happened, revert
the squash merge in public `adze`.

If a release tag or crate publish happened, do not rewrite history. Open a
release incident/patch plan that records:

- tag or crate versions affected;
- claim or proof that failed;
- user impact;
- patch or yanked-version decision;
- release note correction.

## Decision Point

After this plan lands, the closeout should record one of these outcomes:

```text
proceed:
  open the public promotion PR using this plan

defer:
  keep public adze unchanged and list blockers

split:
  promote a smaller named subset first
```

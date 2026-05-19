# Public Promotion Execution Plan

Status: active
Owner: release/product
Created: 2026-05-19
Linked proposal: ../../docs/proposals/ADZE-PROP-0005-release-promotion-readiness.md
Linked specs:
- ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADRs:
- ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/public-promotion-execution.toml
Support-tier impact: no tier changes in this plan
Policy impact: keeps public `adze` untouched until an explicit promotion PR is prepared

## Goal

Execute the public-promotion decision from current `adze-swarm/main` without
opening accidental public-repo work. This plan starts after the completed
release-promotion readiness campaign and the product gap burn-down lane.

## Scope

This execution lane may:

- refresh live `adze-swarm` and public `adze` PR queues;
- rerun source-of-truth, stable-product, and publishability proof commands;
- refresh public drift against `public/main`;
- record whether promotion proceeds, defers, or splits;
- prepare a public promotion PR only after the refreshed proof and drift audit
  support that decision.

This execution lane does not:

- tag a release;
- publish crates;
- move release, signing, Cargo-token, or branch-protection workflows;
- add Stable claims;
- treat `cargo install adze-cli` as proven before a crates.io install receipt
  exists.

## Work Item: promotion-execution-source-of-truth

Status: complete
PR: EffortlessMetrics/adze-swarm#287
Linked proposal: ADZE-PROP-0005-release-promotion-readiness
Linked spec: ADZE-SPEC-0011
Linked ADR: ADZE-ADR-0001
Blocks: fresh-promotion-preflight, public-drift-refresh
Blocked by: n/a

### Goal

Replace the completed product gap burn-down active manifest with a fresh
machine-readable execution queue for the public promotion decision.

### Production Delta

Docs and active-goal metadata only.

### Non-Goals

No public `adze` PR, release tag, crate publish, branch-protection change,
runtime behavior change, or support-tier promotion.

### Acceptance

- `.adze/goals/active.toml` points to this execution lane.
- `.adze/goals/public-promotion-execution.toml` mirrors the named lane.
- This plan defines ready preflight and drift-refresh work items.
- Source-of-truth checks pass.

### Proof Commands

```bash
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Revert the source-of-truth PR to return to the completed product gap burn-down
manifest.

## Work Item: fresh-promotion-preflight

Status: ready
Linked proposal: ADZE-PROP-0005-release-promotion-readiness
Linked spec: ADZE-SPEC-0011
Linked ADR: ADZE-ADR-0001
Blocks: promotion-decision-record
Blocked by: promotion-execution-source-of-truth

### Goal

Refresh proof from current `adze-swarm/main` before any public promotion PR is
prepared.

### Acceptance

- Local worktree is clean.
- `adze-swarm` and public `adze` open PR queues are refreshed.
- Active-goal and doc-artifact checks pass.
- README Stable claim guard passes.
- Stable product canaries pass or blockers are recorded.
- Publishability proof passes or blockers are recorded.

### Proof Commands

```bash
git status --short
gh pr list --repo EffortlessMetrics/adze-swarm --state open
gh pr list --repo EffortlessMetrics/adze --state open
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
cargo test -p adze-cli readme_stable_claims_are_in_stable_product_lane -- --exact --nocapture
just ci-product-stable
just check-publishable
git diff --check
```

### Rollback

Record defer if any proof fails and do not open a public PR.

## Work Item: public-drift-refresh

Status: ready
Linked proposal: ADZE-PROP-0005-release-promotion-readiness
Linked spec: ADZE-SPEC-0011
Linked ADR: ADZE-ADR-0001
Blocks: promotion-decision-record
Blocked by: promotion-execution-source-of-truth

### Goal

Refresh the public `adze` drift picture before deciding whether the current
`adze-swarm` state should be promoted.

### Acceptance

- `origin/main` and `public/main` are fetched.
- Ahead/behind counts are recorded.
- Public-only commits are classified as release, intake, already-ported, or
  promotion blockers.
- Swarm-only commits after the previous claim freeze are either included or
  deliberately excluded by name.

### Proof Commands

```bash
git fetch origin --prune
git fetch public --prune
git rev-list --left-right --count public/main...origin/main
git log --oneline --decorate --left-right public/main...origin/main --
```

### Rollback

Record defer if the drift state is ambiguous or if public-only work conflicts
with the promotion scope.

## Work Item: promotion-decision-record

Status: blocked
Linked proposal: ADZE-PROP-0005-release-promotion-readiness
Linked spec: ADZE-SPEC-0011
Linked ADR: ADZE-ADR-0001
Blocks: public promotion PR if outcome is proceed
Blocked by: fresh-promotion-preflight, public-drift-refresh

### Goal

Record one decision:

```text
proceed:
  prepare the public promotion PR using public-promotion-pr-plan.md

defer:
  keep public adze unchanged and list blockers

split:
  promote a smaller named subset first
```

### Acceptance

- The decision is recorded in this plan.
- Proof receipts and drift notes are linked.
- If proceeding, the public PR body uses
  `plans/release-promotion/public-promotion-pr-plan.md`.
- If deferring or splitting, blockers and next conditions are concrete.

### Proof Commands

```bash
git diff --check
cargo run -q -p xtask -- check-active-goal --mode blocking
```

### Rollback

Revert the decision note if it is incorrect. Close any unmerged public PR that
was opened from a superseded decision.

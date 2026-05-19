# Release Promotion Readiness Plan

Status: complete
Owner: release/product
Created: 2026-05-19
Linked proposal: ../../docs/proposals/ADZE-PROP-0005-release-promotion-readiness.md
Linked specs:
- ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADRs:
- ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/release-promotion-readiness.toml
Support-tier map: ../../docs/status/SUPPORT_TIERS.md

## Goal

Prepare `adze-swarm` for a deliberate public promotion decision without
reopening public-repo drift or creating unsupported release claims.

## Operating Rules

- Work in `EffortlessMetrics/adze-swarm`.
- Do not open public `EffortlessMetrics/adze` PRs until the promotion plan says
  exactly what is being promoted.
- Public `adze` remains release/public-intake.
- Every release-facing claim must map to `docs/status/SUPPORT_TIERS.md`.
- No Stable claims are added without proof and README/book wording alignment.
- `Rust Small Result` remains the required swarm gate.

## Phase 0: Campaign Setup

### Work Item: release-promotion-campaign-source-of-truth

Status: complete
PR: EffortlessMetrics/adze-swarm#234

#### Goal

Open the release-promotion readiness campaign with a proposal, plan, active
manifest, and artifact-ledger entries.

#### Production Delta

Docs and policy only. No runtime behavior changes and no public `adze` PR.

#### Proof Commands

```bash
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

## Phase 1: Inventory

### Work Item: release-readiness-inventory

Status: complete
PR: EffortlessMetrics/adze-swarm#235
Inventory: ./readiness-inventory.md

#### Goal

Inventory completed `adze-swarm` campaigns, release-facing docs, support-tier
rows, and proof commands that would matter for a public promotion.

#### Acceptance

- Completed campaign closeouts are listed.
- Public-facing claims are mapped to support tiers.
- Deferred or swarm-only surfaces are named.
- Proof commands are repeatable.

## Phase 2: Public Drift Audit

### Work Item: public-drift-audit

Status: complete
PRs:
- EffortlessMetrics/adze-swarm#236
- EffortlessMetrics/adze-swarm#237
Audit: ./public-drift-audit.md

#### Goal

Compare public `EffortlessMetrics/adze` with `EffortlessMetrics/adze-swarm`
before preparing any public promotion PR.

#### Acceptance

- Open public and swarm PR queues are checked live.
- Public-only commits are classified as release, intake, drift, or already
  ported.
- Promotion blockers are named before any public PR is opened.

## Phase 3: Claim Freeze

### Work Item: release-claim-freeze

Status: complete
PR: EffortlessMetrics/adze-swarm#238
Claim freeze: ./claim-freeze.md

#### Goal

Freeze README, docs, support tiers, product proof map, and known limitations for
the promotion candidate.

#### Acceptance

- No README Stable claim lacks support-tier proof.
- Stabilizing, Experimental, Advisory, and future surfaces remain labeled.
- Performance, Tree-sitter, query, CLI, WASM, and incremental limitations are
  visible.

## Phase 4: Promotion Plan

### Work Item: public-promotion-pr-plan

Status: complete
PR: EffortlessMetrics/adze-swarm#239
Promotion plan: ./public-promotion-pr-plan.md

#### Goal

Prepare the public promotion PR plan, including scope, proof, rollback, and
excluded surfaces.

#### Acceptance

- Public PR scope is explicit.
- Release/publish/signing workflows are excluded unless separately planned.
- Rollback path is documented.
- Required proof commands are listed.

## Phase 5: Closeout

### Work Item: release-promotion-readiness-closeout

Status: complete
PR: EffortlessMetrics/adze-swarm#240
Closeout: ./closeout.md

#### Goal

Record whether promotion proceeds, defers, or splits into smaller campaigns.

#### Acceptance

- Outcome is recorded in a closeout note.
- Active manifest is closed or replaced by the next campaign.
- Public `adze` remains free of accidental swarm work.

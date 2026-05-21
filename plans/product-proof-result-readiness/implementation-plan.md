# Product Proof Result Readiness Plan

Status: active
Owner: release/product
Created: 2026-05-21
Linked proposal: `docs/proposals/ADZE-PROP-0010-product-proof-result-readiness.md`
Linked specs: `docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md`
Linked ADRs: n/a
Active goal: `.adze/goals/active.toml`
Support-tier impact: no promotion by setup; future work may update proof receipts
Policy impact: prepares a branch-protection-safe result context without changing branch protection

## Work Item: product-proof-result-source-of-truth

Status: active
Linked proposal: `ADZE-PROP-0010`
Linked spec: `ADZE-SPEC-0011`
Linked ADR: n/a
Blocks: `product-proof-result-workflow`
Blocked by: n/a

### Goal

Open the non-release lane that prepares the Stable product proof workflow for a
future branch-protection promotion.

### Production Delta

- Add the proposal.
- Add this implementation plan.
- Replace the completed active manifest with a focused active goal.
- Register the new source-of-truth artifacts.

### Non-Goals

- No workflow behavior change.
- No branch-protection change.
- No release, tag, publish, signing, Cargo-token, or crates.io install work.

### Acceptance

- Source-of-truth checks pass.
- The active manifest names `adze-swarm` as the operating repo.
- Work items keep the implementation and policy promotion separate.

### Proof Commands

```bash
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Revert the setup PR to restore the previous completed parser-recovery active
manifest and remove the Product Proof result readiness artifacts.

## Work Item: product-proof-result-workflow

Status: ready
Linked proposal: `ADZE-PROP-0010`
Linked spec: `ADZE-SPEC-0011`
Linked ADR: n/a
Blocks: `product-proof-result-policy-receipt`
Blocked by: `product-proof-result-source-of-truth`

### Goal

Make `.github/workflows/product-proof.yml` emit a cheap, always-present
Product Proof result check while keeping Stable canaries gated to relevant
surfaces.

### Production Delta

- Add changed-path detection for Stable product surfaces.
- Run `ci-product stable canaries` only for selected paths, manual dispatch, or
  schedule.
- Add a `Product Proof Result` job that succeeds with a skip reason when the
  Stable canaries are not selected and fails when selected canaries fail.
- Update the routing canary so workflow shape stays guarded.

### Non-Goals

- No `.github/settings.yml` branch-protection change.
- No broad advisory `ci-product` PR default.
- No support-tier promotion.

### Acceptance

- Docs-only/product-surface PRs run the Stable canaries and Product Proof
  result.
- Unrelated PRs create Product Proof result without running Stable canaries.
- Manual stable dispatch still runs Stable canaries.
- Scheduled/advisory behavior remains explicit.

### Proof Commands

```bash
cargo test -p adze-cli product_proof_workflow_routes_stable_claim_surfaces -- --exact --nocapture
just ci-product-stable
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Revert the workflow PR to restore the existing path-filtered Product Proof
workflow.

## Work Item: product-proof-result-policy-receipt

Status: ready
Linked proposal: `ADZE-PROP-0010`
Linked spec: `ADZE-SPEC-0011`
Linked ADR: n/a
Blocks: n/a
Blocked by: `product-proof-result-workflow`

### Goal

Record the new result-check behavior without making it required.

### Production Delta

- Refresh `docs/status/PRODUCT_OBJECTIVE_AUDIT.md`,
  `docs/status/NOW_NEXT_LATER.md`, and CI lane docs if the workflow proof
  changes the promotion readiness state.

### Non-Goals

- No branch-protection promotion unless a later explicit policy PR chooses it.
- No release/publish work.

### Acceptance

- Status docs distinguish "branch-protection-ready result exists" from
  "branch protection requires it".
- Issue `adze-swarm#325` remains the release/publish authorization tracker,
  not a Product Proof promotion authorization.

### Proof Commands

```bash
cargo test -p adze-cli product_proof_workflow_routes_stable_claim_surfaces -- --exact --nocapture
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
cargo run -q -p xtask -- check-active-goal --mode blocking
git diff --check
```

### Rollback

Revert the receipt PR to restore the previous advisory-only status wording.

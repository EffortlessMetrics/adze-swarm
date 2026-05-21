# Product Proof required-gate burn-in plan

Status: active
Owner: release/product
Created: 2026-05-21
Linked proposal: `docs/proposals/ADZE-PROP-0011-product-proof-required-gate-burn-in.md`
Linked specs: `docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md`
Linked ADRs: n/a
Active goal: `.adze/goals/active.toml`
Support-tier impact: `docs/status/SUPPORT_TIERS.md`
Policy impact: `.github/settings.yml`, `.github/CI_LANES.md`, `docs/ci/branch-protection.md`

## Work Item: burn-in-source-of-truth

Status: complete
Linked proposal: `ADZE-PROP-0011`
Linked spec: `ADZE-SPEC-0011`
Linked ADR: n/a
Blocks: `collect-product-proof-result-receipts`
Blocked by: n/a
PR: `adze-swarm#386`

### Goal

Create the source-of-truth lane for burning in `Product Proof Result` before it
is considered for branch protection.

### Production Delta

- Add the proposal, plan, and active goal manifest.
- Register the artifacts in `policy/doc-artifacts.toml`.
- Update CI branch-protection docs with Product Proof promotion criteria.
- Keep current branch protection unchanged.

### Non-Goals

- No required-check promotion.
- No release, publish, signing, Cargo-token, or crates.io install work.
- No support-tier promotion.

### Acceptance

- The active manifest validates.
- The doc artifact ledger validates.
- `.github/settings.yml` still requires only `Rust Small Result`.

### Proof Commands

```bash
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Revert the proposal, plan, active-goal manifest, doc-artifact registration, and
branch-protection wording changes. Branch protection itself is unchanged.

## Work Item: collect-product-proof-result-receipts

Status: active
Linked proposal: `ADZE-PROP-0011`
Linked spec: `ADZE-SPEC-0011`
Linked ADR: n/a
Blocks: `promote-product-proof-result-policy`
Blocked by: `burn-in-source-of-truth`

### Goal

Collect recent ordinary-PR receipts showing `Product Proof Result` is
consistently present and green both when Stable canaries run and when they skip.

### Production Delta

Update the product audit and CI lane docs with receipt links. No workflow or
branch-protection change is required.

### Non-Goals

- No required-check promotion.
- No release/publish work.

### Acceptance

- At least five distinct merged PRs show `Product Proof Result`.
- At least two receipts selected `ci-product stable canaries`.
- At least two receipts skipped Stable canaries with a clear skip reason.
- No unexplained `Product Proof Result` flake remains open.

Current receipts:

| PR | Product Proof Result | Stable canaries | Rust Small Result |
| --- | --- | --- | --- |
| `adze-swarm#386` | success | selected, success | success |
| `adze-swarm#387` | success | selected, success | success |
| `adze-swarm#388` | success | skipped, no Stable product surface changed | success |

### Proof Commands

```bash
gh pr view <number> --repo EffortlessMetrics/adze-swarm --json statusCheckRollup
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Revert the receipt update if the evidence is later found to be stale or wrong.

## Work Item: promote-product-proof-result-policy

Status: blocked
Linked proposal: `ADZE-PROP-0011`
Linked spec: `ADZE-SPEC-0011`
Linked ADR: n/a
Blocks: n/a
Blocked by: `collect-product-proof-result-receipts`

### Goal

Make a deliberate policy PR that requires `Product Proof Result` only after
burn-in receipts prove the context is stable enough to become a merge gate.

### Production Delta

- Add `Product Proof Result` to `.github/settings.yml` required contexts.
- Update `.github/CI_LANES.md` to classify it as required.
- Update `docs/status/PRODUCT_OBJECTIVE_AUDIT.md` and
  `docs/status/KNOWN_RED.md` so they no longer call Product Proof advisory.

### Non-Goals

- No replacement of `Rust Small Result`.
- No release, tag, publish, signing, Cargo-token, or crates.io install work.
- No support-tier promotion beyond claim-proof gate wording.

### Acceptance

- `Rust Small Result` remains required.
- `Product Proof Result` becomes required only in the same PR that updates the
  policy docs.
- The PR body includes rollback to `Rust Small Result` only.

### Proof Commands

```bash
cargo test -p adze-cli product_proof_workflow_routes_stable_claim_surfaces -- --exact --nocapture
cargo run -q -p xtask -- check-ci-lane-whitelist --mode blocking-strict
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
cargo run -q -p xtask -- check-active-goal --mode blocking
git diff --check
```

### Rollback

Remove `Product Proof Result` from `.github/settings.yml` required contexts and
restore the docs to the advisory Product Proof wording.

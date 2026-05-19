# Release Promotion Readiness Closeout

Status: complete
Owner: release/product
Closed: 2026-05-19
Proposal: ../../docs/proposals/ADZE-PROP-0005-release-promotion-readiness.md
Plan: ./implementation-plan.md
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/release-promotion-readiness.toml

## Outcome

Outcome: **proceed conditionally**.

`adze-swarm` now has the inventory, drift audit, claim freeze, and public PR
plan needed for a deliberate public promotion decision. This closeout does not
open the public `EffortlessMetrics/adze` PR and does not tag, publish, sign, or
change release workflows.

The next operator can either:

```text
proceed:
  open a public promotion PR using ./public-promotion-pr-plan.md

defer:
  keep public adze unchanged and record blockers

split:
  promote a smaller named subset first
```

## Landed Work

| Work item | PRs | Result |
| --- | --- | --- |
| Campaign source of truth | #234 | Opened ADZE-PROP-0005, release-promotion plan, active manifest, named goal, and artifact-ledger entries. |
| Release readiness inventory | #235 | Listed completed campaigns, release-facing docs, support-tier scope, proof commands, and deferred surfaces. |
| Public drift audit | #236 | Confirmed public `adze` had no open PRs, classified public-only commits, and identified public #783 as useful unported drift. |
| Public drift port | #237 | Ported public #783 file-policy migration-candidate reporting into `adze-swarm`. |
| Release claim freeze | #238 | Aligned README non-Stable tier labels with `SUPPORT_TIERS.md` and recorded explicit non-claims. |
| Public promotion PR plan | #239 | Defined promotion preconditions, scope, exclusions, proof commands, PR body skeleton, and rollback. |
| Closeout | #240 | Closes this campaign and leaves the public promotion decision explicit. |

Post-closeout product-proof alignment continued in `adze-swarm` #241-#267.
Those PRs kept the public promotion candidate current by aligning acceptance
matrix, archived proof commands, performance receipts, downstream proof rows,
starter-project proof, CLI recovery-diagnostics proof, objective audit, install
claim boundaries, local CLI package receipts, dangling-else selected-tree proof,
generated reduce/reduce proof, and wrapper-preservation receipts with the
current support-tier and README claim surfaces.

## Current State

- `adze-swarm` remains the operating repo.
- Public `adze` remains release/public-intake until an explicit promotion PR.
- Public #783 drift has been ported into `adze-swarm`.
- README Stable claims are still limited to typed extraction, Pure-Rust parser,
  proven operator precedence, and core table serialization.
- `AdzeDocument`, selected-tree Tree-sitter compatibility, query subset, and
  CLI are Stabilizing, not Stable.
- Typed CST, incremental, WASM, benchmarks, and full compatibility claims remain
  Experimental or Advisory as recorded in `SUPPORT_TIERS.md`.
- `just check-publishable` passed on 2026-05-19 from `adze-swarm/main` after
  #253, covering publish-order metadata and package file-list checks for the
  core release surface. This is not a publish or crates.io install claim.
- Live queue refresh after #267 showed no open PRs in `EffortlessMetrics/adze`
  or `EffortlessMetrics/adze-swarm`.

## Remaining Preconditions Before A Public PR

Before opening public promotion:

- refresh both PR queues;
- merge, supersede, close, or explicitly defer any open `adze-swarm` PRs
  observed during the fresh promotion audit;
- run the proof commands in `./public-promotion-pr-plan.md`;
- confirm no release/publish/signing workflow change is included unless a
  separate release-surface plan owns it.

## Proof Receipts

Representative proof commands run during this campaign:

```bash
gh pr list --repo EffortlessMetrics/adze --state open
gh pr list --repo EffortlessMetrics/adze-swarm --state open
git rev-list --left-right --count public/main...origin/main
cargo test -p adze-cli readme_stable_claims_are_in_stable_product_lane -- --exact --nocapture
cargo test -p xtask file_policy -- --nocapture
cargo run -q -p xtask -- check-file-policy
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
just check-publishable
just package-local adze-cli
git diff --check
```

CI receipts across the campaign included `Rust Small Result`, Source of Truth,
PR Gate, and the README stable-product canary where README claim wording
changed.

## Non-Claims

This closeout does not claim:

- public promotion has happened;
- a release tag or crate publish is ready;
- release/publish/signing workflow changes are approved;
- branch protection changed;
- full Tree-sitter or query parity;
- stable CLI/WASM schema compatibility;
- stable raw GLR forest export;
- stable incremental reuse or performance;
- benchmark throughput or regression thresholds.

## Next Step

Use `./public-promotion-pr-plan.md` for the public promotion decision. If
promotion is deferred or split, record that outcome in a new active goal rather
than reopening this closed campaign.

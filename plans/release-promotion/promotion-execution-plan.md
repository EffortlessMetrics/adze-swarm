# Public Promotion Execution Plan

Status: complete
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

Status: complete
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

### Fresh Receipt - 2026-05-19

Refreshed from `adze-swarm/main` after PR #287.

Queue state:

- `EffortlessMetrics/adze-swarm`: no open PRs.
- `EffortlessMetrics/adze`: no open PRs.
- Local checkout: `main` was clean before the receipt branch was created.

Proof results:

- `cargo run -q -p xtask -- check-active-goal --mode blocking`: passed.
- `cargo run -q -p xtask -- check-doc-artifacts --mode blocking`: passed with 35 registered artifacts.
- `cargo test -p adze-cli readme_stable_claims_are_in_stable_product_lane -- --exact --nocapture`: passed.
- `just ci-product-stable`: passed after local host remediation.
- `just check-publishable`: passed for `adze-common`, `adze-ir`, `adze-glr-core`, `adze-tablegen`, `adze-macro`, `adze-tool`, `adze-cli`, and `adze`.
- `git diff --check`: passed before recording this receipt.

Host note:

The first `just ci-product-stable` attempt failed while building the clean-room
README quickstart because the local `C:` drive had roughly 309 MB free and
returned `os error 112`. `cargo clean` was run only inside this `adze-swarm`
checkout, removing 151.2 GiB of local build artifacts. The same command passed
after disk space was restored, so this was recorded as host remediation rather
than a repo proof failure.

### Fresh Receipt - 2026-05-19 After Residual Product-Trust PRs

Refreshed from `adze-swarm/main` at commit `b613ebbb` after PRs #295-#301.

Historical queue state:

- `EffortlessMetrics/adze-swarm`: no open PRs.
- `EffortlessMetrics/adze`: public promotion PR #794 was open for this
  receipt, then later closed/unmerged and superseded by #795.
- Local checkout: `main` was clean before the receipt branch was created.

Historical proof results:

- `gh pr view 794 --repo EffortlessMetrics/adze --json
  state,mergeStateStatus,reviewDecision,autoMergeRequest`: passed at the time;
  #794 was open, blocked by normal public review/merge controls, and had no
  auto-merge request.
- `just ci-product-stable`: passed from current `adze-swarm/main`.
- `just check-publishable`: passed for `adze-common`, `adze-ir`,
  `adze-glr-core`, `adze-tablegen`, `adze-macro`, `adze-tool`, `adze-cli`, and
  `adze`.

Promotion boundary:

- The public #794 CI receipt at commit `2550b21f` remains historical.
- The public promotion branch must be refreshed from current `adze-swarm/main`
  or superseded before review/merge whenever `adze-swarm/main` advances.

### Fresh Receipt - 2026-05-20 After Promotion Watch Refresh

Refreshed from `adze-swarm/main` at commit `e965cba2` after PRs #309-#310.

Proof results:

- `gh pr view 795 --repo EffortlessMetrics/adze --json
  state,mergeable,headRefOid,reviewDecision,autoMergeRequest`: passed; #795 is
  open, mergeable, green, auto-merge-enabled, and still blocked only by the
  public approving-review requirement.
- `just ci-product-stable`: passed from current `adze-swarm/main`.
- `just check-publishable`: passed for `adze-common`, `adze-ir`,
  `adze-glr-core`, `adze-tablegen`, `adze-macro`, `adze-tool`, `adze-cli`, and
  `adze`.

Promotion boundary:

- Public PR #795 remains the current explicit promotion PR.
- The public promotion branch must be refreshed from current `adze-swarm/main`
  if the swarm branch advances again before review/merge.

### Fresh Receipt - 2026-05-20 Public Promotion Merge

Public `EffortlessMetrics/adze#795` was refreshed from `adze-swarm/main` at
commit `464a32a9` after PR #311 and merged into public `main` on 2026-05-20 as
squash commit `a0d593e8`.

Proof results:

- `gh pr view 795 --repo EffortlessMetrics/adze --json state,mergedAt,mergeCommit,headRefOid`: passed; #795 is merged and the promoted head was `bcfddbd4`.
- `gh pr checks 795 --repo EffortlessMetrics/adze`: passed after the final branch refresh.
- `gh workflow run ci.yml --repo EffortlessMetrics/adze --ref public/promote-swarm-2026-05-19 -f run_full_ci=false -f run_ci_supported_examples=false`: passed and emitted the legacy public `ci-supported` context.
- `just ci-product-stable`: passed from current `adze-swarm/main`.
- `just check-publishable`: passed for `adze-common`, `adze-ir`,
  `adze-glr-core`, `adze-tablegen`, `adze-macro`, `adze-tool`, `adze-cli`, and
  `adze`.

Promotion boundary:

- The public branch-protection required context was corrected from stale
  `ci-supported` to `Rust Small Result`, matching `.github/settings.yml` in
  the promoted tree.
- No release tag, crate publish, signing workflow, Cargo-token workflow, or
  new Stable claim was added by this promotion.
- Public `main` must be refreshed locally before any follow-up tag, publish,
  release-workflow, or crates.io install-receipt work.

### Rollback

Record defer if any proof fails and do not open a public PR.

## Work Item: public-drift-refresh

Status: complete
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

### Drift Receipt - 2026-05-19

Fetched `origin` and `public`, then refreshed the comparison:

```text
git rev-list --left-right --count public/main...origin/main
6 262
```

Public-only commits by commit identity:

| Commit | Public PR | Classification |
| ------ | --------- | -------------- |
| `5fc7924b` | #789 `xtask: own goto indexing guard` | Content present in `adze-swarm`: `xtask/src/goto_indexing.rs` and `CheckGotoIndexing` are available; equivalent guard work appears in the swarm guard consolidation history. No promotion blocker. |
| `5685ae35` | #788 `xtask: own no-mangle guard` | Content present in `adze-swarm`: `xtask/src/no_mangle.rs` and `CheckNoMangle` are available; equivalent guard work appears in the swarm guard consolidation history. No promotion blocker. |
| `f8ba5ff9` | #787 `plans: add 0.9 contract convergence closeout` | Superseded by swarm release-promotion, product-gap, and current promotion-execution closeouts. No promotion blocker. |
| `a788f921` | #786 `plans: align 0.9 closeout state` | Superseded by current swarm closeout and promotion execution state. No promotion blocker. |
| `92cc08ae` | #783 `xtask: report Rust migration candidates in file-policy` | Ported into `adze-swarm` as #237 (`92b7cbe1`). No promotion blocker. |
| `c9c40728` | #785 `test(cli): align README capability tiers with support tiers` | Superseded by swarm release claim freeze and README Stable-claim guard work, including #238 and #283. No promotion blocker. |

Promotion boundary:

- No public PR was opened.
- No publish, tag, signing, branch-protection, or release workflow change was made.
- Public `adze` remains the release/public-intake surface until an explicit
  promotion PR is prepared.

### Rollback

Record defer if the drift state is ambiguous or if public-only work conflicts
with the promotion scope.

## Work Item: promotion-decision-record

Status: complete
Linked proposal: ADZE-PROP-0005-release-promotion-readiness
Linked spec: ADZE-SPEC-0011
Linked ADR: ADZE-ADR-0001
Blocks: public promotion PR if outcome is proceed
Blocked by: n/a

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

### Decision Receipt - 2026-05-19

Decision: proceed.

Rationale:

- PR #288 recorded the fresh promotion preflight and public drift refresh from
  current `adze-swarm/main`.
- Both `EffortlessMetrics/adze-swarm` and public `EffortlessMetrics/adze` had
  no open PRs at the time of the decision.
- Stable product and publishability receipts passed from the current swarm
  state.
- Public-only commits were classified as already present in `adze-swarm`,
  superseded by newer swarm source-of-truth, or otherwise non-blocking.
- No conflicting public-release work was observed.

Next action:

Public `EffortlessMetrics/adze` promotion PR #794 was prepared from this
decision and later superseded by public PR #795 after the promotion branch was
refreshed to current `adze-swarm/main`. Review and merge the current public
promotion PR manually if the public repo owner accepts the promotion.

Promotion boundaries:

- Do not tag a release.
- Do not publish crates.
- Do not move release, signing, Cargo-token, branch-protection, or merge-queue
  workflows.
- Do not add new Stable claims beyond the claim freeze.
- Do not claim crates.io `cargo install adze-cli` is proven until a real
  crates.io install receipt exists.

### Rollback

Revert the decision note if it is incorrect. Close any unmerged public PR that
was opened from a superseded decision.

## Public Promotion PR Receipt

Status: merged
Current public PR: EffortlessMetrics/adze#795
Superseded public PR: EffortlessMetrics/adze#794 (closed/unmerged)
Public branch: `public/promote-swarm-2026-05-19`
Public head before merge: `bcfddbd4`
Public merge commit: `a0d593e8`
Prepared: 2026-05-20
Merged: 2026-05-20

### Scope

PR #795 was the explicit public promotion PR prepared from this execution
decision. It promoted the current `adze-swarm` source, documentation, proof
maps, CI receipts, product fixtures, and support-tier-aligned claim boundaries
into public `EffortlessMetrics/adze`.

This public PR does not:

- tag a release;
- publish crates;
- move release, signing, Cargo-token, branch-protection, or merge-queue
  workflows;
- add Stable claims beyond the recorded claim freeze;
- claim crates.io `cargo install adze-cli` is proven.

### Source-Side Fixups Included Before The Current Public Receipt

Source-side `adze-swarm` fixes landed while proving and refreshing the public
promotion branch:

| PR | Result |
| --- | --- |
| #290 `ci: fix promotion receipt checks` | Merged. Refreshed `tools/ts-bridge/Cargo.lock` and raised the coverage path-detection timeout so the public promotion receipt could run without a checkout timeout. |
| #291 `test(glr): require distinct reduces in rr proptest` | Merged. Corrected the reduce/reduce proptest so duplicate reduce rule IDs are not treated as a two-rule reduce/reduce conflict; this matches the existing `rr_duplicate_rule_ids` contract. |
| #303 `docs(status): keep install claim receipt bounded` | Merged. Kept the install-proof gap explicit without immediately stale promotion SHA text. |
| #304 `docs(status): refresh public promotion audit` | Merged. Refreshed the product objective audit from the active promotion branch. |
| #305 `docs(status): stabilize promotion audit receipt wording` | Merged. Made audit wording less SHA-sensitive for routine promotion branch refreshes. |
| #306 `test(example): declare GLR core dev dependency` | Merged. Added the direct example-crate dev dependency needed for direct excluded-manifest GLR/example tests. |

These fixes were mirrored into the current public promotion branch before the
latest public CI receipt.

### Public CI Receipt - 2026-05-20

On public `EffortlessMetrics/adze#795`, the refreshed public check set passed
on 2026-05-20, including:

- `Rust Small Result`;
- `Supported Rust Gate`;
- `PR Gate Success`;
- `Source of Truth`;
- `CI Lane Whitelist`;
- `GLR Invariants`;
- `Coverage Lite`;
- `smoke (ubuntu-latest)` for `tools/ts-bridge`;
- `ci-product stable canaries`;
- `Test Core Crates (ir, glr-core, tablegen)`;
- `Test Runtime Crates`;
- `Test Pure Rust Implementation (ubuntu-latest, stable)`.

Intentional skips remained skips: `Coverage Full`, broad advisory product
canaries, CX43/CX53 implementation lanes not selected by routing, WASM build in
the pure-rust matrix, and performance regression tests.

### Current Boundary

PR #795 is merged. Public PR #794 is closed/unmerged and superseded by #795.
The live public `main` tree now matches the `adze-swarm/main` tree at commit
`464a32a9`.

Refresh public/main before starting any follow-up public release, tag, publish,
or crates.io install-receipt work. The promotion itself did not tag a release,
publish crates, or prove `cargo install adze-cli`.

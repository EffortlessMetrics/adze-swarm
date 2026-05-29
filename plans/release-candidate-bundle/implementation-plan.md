# Release Candidate Bundle Readiness Plan

Status: complete
Owner: release/product
Created: 2026-05-29
Linked proposal: ../../docs/proposals/ADZE-PROP-0017-release-candidate-bundle.md
Linked specs:
- ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADRs:
- ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/release-candidate-bundle.toml
Closeout: ./closeout.md
Release authorization tracker: https://github.com/EffortlessMetrics/adze-swarm/issues/325
Lane selection tracker: https://github.com/EffortlessMetrics/adze-swarm/issues/549

## Goal

Prepare a reviewable, non-publish release-candidate bundle from
`adze-swarm/main` so maintainers can decide whether to promote a selected swarm
state into public `adze`.

This lane is not release execution. It does not open public PRs, tag, publish,
touch signing, use Cargo tokens, or run a real crates.io install verification.

## Operating Rules

- Work in `EffortlessMetrics/adze-swarm`.
- Keep public `EffortlessMetrics/adze` untouched unless a later task is
  explicitly public promotion or release.
- Preserve `AdzeDocument` as the one parse truth; this lane should not change
  parser semantics.
- Do not broaden README, support-tier, parity, performance, or install claims.
- Distinguish pre-publish proof from post-publish receipts.
- `Rust Small Result` remains the normalized required base gate.

## Phase 0: Source Of Truth

### Work Item: release-candidate-bundle-source-of-truth

Status: complete
PR: EffortlessMetrics/adze-swarm#554

#### Goal

Select this non-release lane from the paused forge standby state and add the
proposal, plan, active goal, named goal, and artifact ledger entries.

#### Production Delta

Docs and source-of-truth only. No public `adze` work, no runtime behavior
change, no release action.

#### Proof Commands

```bash
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

## Phase 1: Candidate Snapshot

### Work Item: current-candidate-snapshot

Status: complete
PR: EffortlessMetrics/adze-swarm#555
Snapshot: ./current-candidate-snapshot.md

#### Goal

Record the selected `adze-swarm` commit, local cleanliness, live open PR state
for both repos, and the current public drift boundary.

#### Acceptance

- The selected commit is explicit.
- Both open PR queues are checked live.
- Public drift is treated as a promotion blocker until an explicit public PR.
- No public repo mutation is performed.

## Phase 2: Bundle Checklist

### Work Item: promotion-bundle-checklist

Status: complete
PR: EffortlessMetrics/adze-swarm#556
Reference: ../../docs/reference/RELEASE_CANDIDATE_BUNDLE.md

#### Goal

Create a concise reference/checklist for the release-candidate bundle contents:
candidate commit, proof commands, claim boundaries, drift state, rollback, and
blocked release actions.

#### Acceptance

- The checklist names non-goals and blocked actions.
- Pre-publish readiness is separated from post-publish crates.io install proof.
- Claim boundaries link back to support-tier and product proof artifacts.

## Phase 3: Non-Publish Receipts

### Work Item: non-publish-preflight-receipts

Status: complete
PR: EffortlessMetrics/adze-swarm#557
Receipts: ./non-publish-preflight-receipts.md

#### Goal

Refresh non-publish proof receipts for the selected candidate without implying
release authorization.

#### Candidate Commands

```bash
just ci-supported
just ci-product-stable
just check-publishable
cargo run -q -p xtask -- verify-crates-io-install adze-cli --bin adze --version X.Y.Z --locked --dry-run
```

The dry-run install verifier is not a crates.io receipt.

## Phase 4: Closeout

### Work Item: release-candidate-bundle-closeout

Status: complete
PR: EffortlessMetrics/adze-swarm#560
Closeout: ./closeout.md

#### Goal

Record whether the candidate should proceed to a public promotion/release
decision, defer with blockers, or split into another non-release lane.

#### Acceptance

- The closeout keeps #325 as the release authorization gate unless a maintainer
  explicitly authorizes release work.
- The closeout does not claim `cargo install adze-cli` works from crates.io.
- The next active state is either a selected follow-up lane or paused standby.

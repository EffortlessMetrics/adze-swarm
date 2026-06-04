# adze-swarm PR operating checklist

Status: active checklist
Owner: repo governance
Created: 2026-06-04
Linked issue: adze-swarm#617
Linked operating model: `docs/reference/adze-swarm-operating-model.md`
Support-tier impact: none
Policy impact: none

This checklist is a review aid for ordinary `adze-swarm` PRs. It summarizes the
normal PR path from the operating model; that model remains the authority if the
two documents diverge. This checklist does not authorize release, publish,
signing, Cargo-token, public `adze` promotion, support-tier promotion,
branch-protection changes, merge queue, hosted fallback, or broad CI fanout.

## Required base gate

- `Rust Small Result`

## Normal PR behavior

- same-repo branch
- no `em-ci` label required for ordinary PRs
- auto-merge when ready and green
- squash merge
- branch auto-delete

## Do not require yet

- full CI
- coverage
- platform matrix
- release workflows
- public `adze` promotion

## Review before merge

- source-of-truth issue linked
- live `adze-swarm` and public `adze` PR queues checked
- proof commands listed
- CI cost expectation stated
- rollback stated
- claim boundary stated

## Proof

For docs-only checklist changes, use:

```bash
git diff --check
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
cargo run -q -p xtask -- check-active-goal --mode blocking
```

Runtime, workflow, policy-ledger, release, or support-tier changes need the
proof commands from their selected source-of-truth plan item instead of this
checklist alone.

## Rollback

Revert this document if it stops matching the active `adze-swarm` operating
model or starts duplicating a more specific source-of-truth artifact.

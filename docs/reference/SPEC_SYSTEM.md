# Repo source-of-truth system

The current-v2 direction is tracked in #915–#920. #853 holds product execution;
#1029 repairs operational guidance. This file separates authority, live work,
implementation scope, execution evidence and public claims.

## Artifact roles

| Artifact | Owns | Does not own |
| --- | --- | --- |
| Roadmap / #853 | Product endpoint, priorities, dependencies | A repository-wide active-work lock |
| GitHub issues and PRs | Live coordination, findings, ownership, blockers | Proof merely because a checkbox or PR is closed |
| Proposal | Problem, users, alternatives, rationale | Accepted behavior until ratified |
| Accepted ADZE requirements / specs | Normative behavior and acceptance | Worker scheduling or current PR state |
| ADR | Accepted durable decision and consequences | Automatic acceptance of a new proposal |
| Plan / PR-local slice | Sequence, requirement deltas, owned/shared/forbidden seams | Another canonical parser/schema or global task queue |
| Receipts | Exact executed work, source/configuration and outcomes | Stronger claims than the run supports |
| Support tiers | Public claims, limitations, proof and promotion criteria | Work selection or silent feature promotion |
| Policy ledgers | Reviewed exceptions, owners, coverage and expiry | Automatic approval of generated baselines |

```text
issue + accepted requirements/decisions
 -> PR-local slice + implementation
 -> local proof + review/repair
 -> integration qualification + protected landing
 -> acceptance reconciliation
```

There is no authoritative repository-global active task. Many independent lanes
may operate concurrently. One coherent semantic work item per PR is a review
boundary, not a one-PR-at-a-time scheduling rule.

## Before and during implementation

Read `AGENTS.md`, the operating model, the current issue/related PRs, and linked
accepted specs/ADRs. Inspect the current source and worktree state. Refresh the
queue before starting a writer or opening a PR:

```bash
gh pr list --repo EffortlessMetrics/adze-swarm --state open
```

Choose an eligible bounded slice from the selected product campaign. Record:

- the behavior to establish and the independent acceptance oracle;
- owned, shared and forbidden files/interfaces, plus existing PR dependencies;
- focused proof commands and feature profiles, cost and rollback;
- support-tier impact and explicit non-claims.

Use the existing owning PR rather than duplicating it. Coordinate shared seams
and isolate writer worktrees. A research task has a named question/output;
waiting CI or a slow baseline is not a reason to idle independent lanes.

Specs define behavior; plans and PR slices define sequencing. Preserve proposal,
spec and ADR distinctions. A needed decision packet can be prepared without
claiming its recommendation has been accepted. Runtime changes cite their actual
accepted behavior owner; issue prose does not overrule a conflicting accepted ADR.

Until #918's structural slice format lands, carry equivalent scope, requirements
and seam information in PR prose. Do not wait for the entire governance migration
to begin an otherwise specified, independently testable product repair.

## Legacy migration boundary

`.adze/goals/active.toml`, its archives, older active-goal references and checkers
are legacy migration artifacts. Their paused/complete/missing state is not
current work-selection authority and does not stop independent eligible work.
Older plans/specs that point at that file must be read with this migration rule.

#920 owns retirement after structural parity. Do not delete or bypass an enforced
check, fabricate an active-goal row, or introduce a replacement singleton to get
a PR through. Record the exact affected check and migration owner. An unresolved
qualification blocker remains real for that lane; other useful lanes continue.

## Evidence and claims

Record actual source/build identity, toolchain/features/configuration, full
commands, exit status and selected/executed/ignored test counts. A test-name
substring, helper function, cfg-disabled test, zero-test invocation or missing
failure text is not evidence that the intended behavior ran. Distinguish local
proof from actual integration qualification and landed acceptance.

A prior clean review does not suppress further substantive review. Preserve a
new finding with its owning PR or a linked follow-up. Replace weak assertions
with independent expected facts, not merely agreement between two projections
that may share the same bug. Do not bless snapshots from output alone.

Run named generators/checkers rather than hand-editing their output. Record
unavailable proof, substitutes at their actual scope, and what remains blocked.
For example, a source review or Python model check is not a Rust execution receipt.
No support-tier promotion follows from issue counts, declarations or local green.

Policy exceptions retain owner, reason, coverage, review date and required expiry.
A proposal generator does not approve an exception; an expired receipt does not
become current authorization merely because its text still matches.

## Checkpoints and blockers

After a merge or completed slice, reconcile its actual acceptance against the
landed source and select the next eligible issue. A child can finish while its
parent remains open. A finished batch, empty PR list or session progress estimate
is not the selected product endpoint.

A lane pauses when its decision/interface/capability/evidence is unavailable or
the proposed action exceeds authority. State that blocker precisely and complete
independent work. Stop the campaign only when selected acceptance is satisfied,
Steven directs a pause, or concrete blockers cover all useful remaining work.
Do not overwrite unrelated staged work or take another writer's worktree.

## Required headers and closeout

New proposals/specs/ADRs/plans retain their local templates and stable IDs.
Use `n/a` for fields that genuinely do not apply. Link the owning issues/PRs,
accepted requirements/decisions, proof, support-tier impact and policy impact.
A bundled operational-documentation change may implement one issue across
multiple entrypoints; it is not a license to mix unrelated semantic changes.

Use an existing lane closeout where one is maintained, otherwise the owning
issue/PR. Record what landed, its evidence, remaining acceptance and next action.
Do not duplicate the live queue or turn historical closeouts into current status.

A cold reader should be able to locate the behavior contract, current owner,
next eligible slice, proving command, evidence limits and public claim boundary
without reconstructing the campaign from chat.

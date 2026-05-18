# Repo source-of-truth system

This repo uses a linked source-of-truth stack so humans and agents can find the
right kind of truth without scraping chat history or treating stale status as
current state.

## Stack

```text
Roadmap
  -> Proposal
    -> Spec
      -> ADR
        -> Implementation plan
          -> Active goal
            -> PR
              -> Proof
```

## Artifact roles

| Artifact | Owns | Does not own |
| --- | --- | --- |
| Roadmap | release direction, milestone framing, lanes | PR queue, live status, proof receipts |
| Proposal | why, users, alternatives, success criteria | behavior contract, detailed task list |
| Spec | behavior, acceptance, proof, support impact | PR sequence, product rationale |
| ADR | durable decision, context, consequences | task list, current metric state |
| Plan | PR order, work items, proof commands, rollback | product rationale, durable decisions |
| Active goal | current machine-readable work, commands, claim boundaries | generated status, long prose |
| Support tiers | public claim proof, tier, limitations, promotion rule | feature design, task sequencing |
| Policy ledgers | exceptions, owners, coverage, review dates | broad architecture or product strategy |

## Rules

1. One kind of truth per artifact.
2. One semantic artifact per PR unless the selected plan item says otherwise.
3. Specs define behavior; plans define sequencing.
4. Proposals explain why; ADRs record durable choices.
5. Active goals tell agents what to do now.
6. Generated status is updated by tools, not by hand.
7. Public claims require a support-tier row or equivalent proof pointer.
8. Policy exceptions require owner, reason, coverage, and review date.

## Required headers

New proposals, specs, ADRs, and plans should use stable IDs and include the
headers required by their local README templates. Use `n/a` where a field does
not apply.

Common linkage fields are:

```text
Status:
Owner:
Created:
Linked proposal:
Linked specs:
Linked ADRs:
Linked plan:
Linked issues:
Linked PRs:
Support-tier impact:
Policy impact:
```

## Agent workflow

Agents must:

1. Read `AGENTS.md` or `CLAUDE.md`.
2. Read this file.
3. Read `.adze/goals/active.toml`.
4. Read the linked implementation plan.
5. Read the linked proposal only for why.
6. Read the linked spec for acceptance.
7. Read linked ADRs for constraints.
8. Inspect the current git status.
9. Refresh the live PR queue with
   `gh pr list --repo EffortlessMetrics/adze-swarm --state open`.
10. Check for same-title, same-scope, or overlapping PRs.
11. Pick exactly one ready work item.
12. Implement only that work item.
13. Run the proof commands.
14. Update status, receipts, support tiers, or policy ledgers only when the work
    item requires it.

If no ready work item exists, stop and write a handoff instead of inventing one.
If an overlapping PR exists, stop and report whether the existing PR should be
merged, amended, superseded, or closed instead of opening another PR.

## Stop conditions

Stop and report instead of guessing when:

- the active goal is missing, stale, or contradictory;
- linked files do not exist;
- generated status is dirty;
- proof commands cannot run;
- unrelated staged files exist;
- an open PR already covers the same work item or semantic scope;
- requested work conflicts with an ADR;
- a public claim lacks support-tier proof;
- the request would require a new proposal, spec, or ADR that was not asked for.

## Active goal lifecycle

The active goal lives at:

```text
.adze/goals/active.toml
```

Use `status = "active"` when a lane has a selected ready queue. Use
`status = "paused"` with a reason when no lane is selected. Archive completed or
superseded manifests under:

```text
.adze/goals/archive/YYYY-MM-DD-<lane>.toml
```

Do not leave multiple active goals.

## Closeout format

At the end of a lane, write a closeout at:

```text
plans/<lane>/closeout.md
```

Closeouts record what shipped, proof commands, receipts, PRs, CI runs, generated
status, support-tier updates, policy updates, deferred work, claim boundaries,
and the next lane recommendation.

## Common failure modes

### Spec becomes a task list

Move PR order to `plans/<lane>/implementation-plan.md`; keep the spec to
behavior, examples, evidence, and claim boundaries.

### Plan becomes product rationale

Move why and user value to the proposal; keep the plan to work items,
dependencies, proof, and rollback.

### Active goal becomes prose

Keep `active.toml` machine-readable. Link out to proposals, specs, ADRs, plans,
status docs, and policy ledgers.

### Agent hand-edits generated status

Run the named generator or checker instead. If it cannot run, record the command,
reason, substitute evidence, and whether that blocks merge.

### Support claims drift

Require a support-tier impact field and a proof pointer before broadening README
or release claims.

### Policy exceptions become silent debt

Every exception must have an owner, reason, `covered_by`, `review_after`, and an
expiry when temporary.

### Mega PR

Use one semantic artifact per PR and one implementation work item per runtime PR
unless the plan explicitly permits a bundled documentation batch.

### Duplicate PR

Before opening a PR, inspect the live `adze-swarm` queue. If same-scope work is
already open, do not create a competing branch. Amend the existing branch when
you own it, or report a merge/supersede/close recommendation to the maintainer.

## What good looks like

A new contributor or agent can arrive cold and answer:

```text
What are we doing?
Why?
What must be true?
What decision constrains it?
What PR lands next?
What command proves it?
What may we claim?
What must we not claim?
```

If the repo answers those questions without chat history, the source-of-truth
system is working.

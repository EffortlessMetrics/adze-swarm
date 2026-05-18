# Repo Source-Of-Truth System

This repository uses a linked source-of-truth stack. Each artifact has one job;
do not make every document do every job.

## Stack

```text
Roadmap
  -> Proposal / PRD
    -> Spec
      -> ADR where needed
        -> Implementation plan
          -> Active goal manifest
            -> Issue / PR
              -> Proof command
              -> CI receipt
              -> Support-tier update
              -> Policy ledger update
```

## Artifact Roles

| Artifact | Owns | Does not own |
|---|---|---|
| Roadmap | Release direction, milestone framing, high-level product strategy | Detailed PR order, live status, proof receipts |
| Proposal | Why the lane exists, users, alternatives, risks, success criteria | Behavior contract, PR sequence, generated status |
| Spec | Required behavior, acceptance examples, proof requirements, test mapping | Product rationale, PR order, live queue |
| ADR | Durable architecture or operating decision, context, consequences | Task list, current metric state, implementation queue |
| Plan | PR order, work items, dependencies, proof commands, rollback | Product motivation, durable decisions, generated status truth |
| Active goal | Current lane, machine-readable objective, ready work items, commands, claim boundaries | Long prose, generated metrics, durable decisions |
| Support tiers | Public claim proof, support classification, limitations, promotion proof | Feature design, architecture decisions |
| Policy ledgers | Exceptions, CI/policy receipts, owner, reason, coverage, review dates | Broad architecture or product strategy |

## Canonical Locations

| Question | Source of truth |
|---|---|
| Why are we doing this? | `docs/proposals/` |
| What must be true? | `docs/specs/` |
| What durable decision constrains future work? | `docs/adr/` |
| What PR lands next? | `plans/<lane>/implementation-plan.md` |
| What is the agent actively executing? | `.adze/goals/active.toml` |
| What proves a public claim? | `docs/status/SUPPORT_TIERS.md`, receipts, and CI |
| What exceptions exist? | `policy/*.toml` |

## Rules

1. One kind of truth per artifact.
2. One semantic artifact per PR unless the selected plan item says otherwise.
3. Proposals explain why; specs define behavior; ADRs record decisions.
4. Plans sequence implementation work; active goals tell agents what to do now.
5. Runtime/code PRs must link to the spec and plan item they implement.
6. Generated status is updated by tools, not by hand.
7. Public README/product claims require a support-tier row or equivalent proof pointer.
8. Policy exceptions require owner, reason, coverage, created date, and review date.
9. If source-of-truth files conflict, stop and report instead of guessing.

## Required Headers

Use `n/a` when a field does not apply.

### Proposal

```md
Status:
Owner:
Created:
Target milestone:
Linked specs:
Linked ADRs:
Linked plan:
Linked issues:
Linked PRs:
Support-tier impact:
Policy impact:
```

### Spec

```md
Status:
Owner:
Created:
Linked proposal:
Linked ADRs:
Linked plan:
Linked issues:
Linked PRs:
Support-tier impact:
Policy impact:
```

### ADR

```md
Status:
Date:
Owner:
Linked proposal:
Linked specs:
Linked plan:
```

### Implementation Plan

```md
Status:
Owner:
Linked proposal:
Linked specs:
Linked ADRs:
Active goal:
```

## Agent Workflow

Agents must:

1. Read `AGENTS.md` and this document.
2. Read `.adze/goals/active.toml`.
3. Read the linked implementation plan.
4. Read the linked proposal only for why.
5. Read the linked spec for acceptance.
6. Read linked ADRs for constraints.
7. Inspect git status before editing.
8. Pick exactly one ready work item.
9. Implement only that item.
10. Run the work item's proof commands and `git diff --check`.
11. Update support tiers, policy ledgers, status docs, or receipts only when the
    work item requires it.
12. Commit/open one focused PR.

If there is no ready work item, agents should not invent one. They should write
a handoff or ask for lane selection.

## Stop Conditions

Stop and report instead of guessing when:

- `.adze/goals/active.toml` is missing, stale, paused without an explicit task,
  or points to missing files;
- the selected work item lacks a linked spec or plan anchor;
- proof commands cannot run and no substitute evidence is allowed;
- generated status differs from committed status;
- unrelated staged changes exist;
- requested work conflicts with an ADR;
- a public claim lacks support-tier proof;
- a policy exception lacks owner, reason, coverage, or review date.

## Active Goal Lifecycle

The active goal lives at:

```text
.adze/goals/active.toml
```

Valid top-level states are:

- `active` — one implementation lane is selected;
- `paused` — no selected implementation lane, with a reason;
- `complete` — the current lane is finished and should be archived before a new
  active lane starts.

Archive replaced goals under:

```text
.adze/goals/archive/YYYY-MM-DD-<lane>.toml
```

Do not leave multiple active goal manifests.

## Closeout

At the end of a lane, write:

```text
plans/<lane>/closeout.md
```

A closeout records what shipped, proof commands and receipts, PRs, CI runs,
support-tier updates, policy updates, deferred work, claim boundaries, and the
recommended next lane.

## Common Failure Modes

### Spec Becomes A Task List

Move PR order to `plans/<lane>/implementation-plan.md`; keep the spec focused on
behavior, examples, proof, and claim boundaries.

### Plan Becomes Product Rationale

Move why/users/alternatives to `docs/proposals/`; keep the plan focused on work
items, proof commands, dependencies, and rollback.

### Active Goal Becomes Prose

Keep `.adze/goals/active.toml` machine-readable TOML. Link out to prose docs
instead of embedding long tables.

### Generated Status Is Hand-Edited

Run the named generator or checker. If no generator exists, the plan must say
manual edits are allowed.

### Support Claims Drift

Require `Support-tier impact:` headers and update `docs/status/SUPPORT_TIERS.md`
when a PR changes public claims.

### Policy Exceptions Become Silent Debt

Every exception in `policy/*.toml` needs an owner, reason, `covered_by`, created
date, and `review_after` date, with `expires` when temporary.

### Mega PR

Split into one semantic artifact or one implementation work item per PR unless a
plan explicitly authorizes a combined change.

## What Good Looks Like

A contributor or agent can arrive cold and answer:

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

If the repository answers those questions without chat history, the system is
working.

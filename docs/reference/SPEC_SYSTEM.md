# Repo Source-of-Truth System

This repo uses a linked source-of-truth stack so humans and agents can find the
right kind of truth in the right file. Do not make every document do every job.
Separate why, what, decision, how, what now, and what proves it.

## Stack

```text
Roadmap
  -> Proposal
    -> Spec
      -> ADR
        -> Implementation plan
          -> Active goal
            -> Issue / PR
              -> Proof
```

## Artifact Roles

| Artifact | Owns | Does not own |
| --- | --- | --- |
| Roadmap | release direction, milestone framing, lane list | PR queue, live status, generated metrics |
| Proposal | why, users, success criteria, alternatives, risks | behavior contract, PR sequence, implementation details |
| Spec | behavior, acceptance, examples, proof requirements | product rationale, active queue, durable architecture choice |
| ADR | durable decision, context, consequences, rejected alternatives | task list, current metric state, implementation queue |
| Plan | PR order, work items, dependencies, proof, rollback | product rationale, durable architecture, generated status truth |
| Active goal | current lane, machine-readable work items, proof commands, claim boundaries | long prose, generated metrics, durable decisions |
| Support tiers | public claim proof, supported/experimental/blocked classification | feature design, behavior specification |
| Policy ledgers | exceptions, CI/policy receipts, owners, review dates | broad architecture, product behavior |

## Source-of-Truth Map

| Question | Source of truth |
| --- | --- |
| Why are we doing this? | `docs/proposals/` |
| What must be true? | `docs/specs/` |
| What architecture decision did we make? | `docs/adr/` |
| What PR lands next? | `plans/<lane>/implementation-plan.md` or the lane plan named by the active goal |
| What is the agent actively executing? | `.adze/goals/active.toml` |
| What proves the claim? | `docs/status/SUPPORT_TIERS.md`, receipts, and CI proof commands |
| What exceptions exist? | `policy/*.toml` |

## Rules

1. One kind of truth per artifact.
2. One semantic artifact or implementation work item per PR unless the plan says otherwise.
3. Specs define behavior; plans define sequencing.
4. Proposals explain why; ADRs record durable decisions.
5. Active goals tell agents what to do now.
6. Generated status is updated by tools, not by hand.
7. Public claims require support-tier proof.
8. Policy exceptions require owner, reason, coverage, and review date.

## Required Headers

Every proposal, spec, ADR, and plan should include the applicable source-of-truth
headers. Use `n/a` when a field does not apply.

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

ADRs may use `Date:` instead of `Created:` when following the established ADR
format, but they still need enough links to connect the decision to relevant
proposals, specs, and plans.

## Agent Boot Order

Agents must begin by reading:

1. `AGENTS.md` and any other repo-level agent instructions.
2. This file.
3. `.adze/goals/active.toml`.
4. The linked implementation plan.
5. The linked proposal only for why.
6. The linked spec for acceptance.
7. Linked ADRs for constraints.
8. Current git status.

After boot, an agent should pick exactly one ready work item, implement only that
item, run its proof commands, update only required receipts/status/policy files,
and open or update one focused PR.

## Stop Conditions

Stop and report instead of guessing when:

- the active goal is missing, paused without a selected lane, or stale;
- linked files do not exist;
- no ready work item can be identified;
- generated status is dirty and no generator/checker is named;
- proof commands cannot run;
- unrelated staged files exist;
- requested work conflicts with an ADR;
- a public claim lacks support-tier proof;
- a policy exception lacks owner, reason, coverage, or review date.

## Active Goal Lifecycle

The current execution state lives at:

```text
.adze/goals/active.toml
```

An active lane uses:

```toml
status = "active"
```

A paused repo state must explain why no lane is selected:

```toml
status = "paused"
reason = "No selected implementation lane."
```

When changing lanes, archive the old manifest under:

```text
.adze/goals/archive/YYYY-MM-DD-<lane>.toml
```

Then create the new active manifest. Do not leave multiple active goals.

## Closeout Format

At the end of a lane, write a closeout at:

```text
plans/<lane>/closeout.md
```

Closeouts should include:

- what shipped;
- proof commands and receipts;
- PRs and CI runs;
- generated status updates;
- support-tier updates;
- policy updates;
- what did not ship;
- deferred work;
- claim boundary;
- next lane recommendation.

Closeout prevents the next agent from rediscovering old work through chat
history or stale plan notes.

## Common Failure Modes

### Spec becomes a task list

Move PR order to `plans/<lane>/implementation-plan.md`; keep the spec focused on
behavior, examples, and proof.

### Plan becomes product rationale

Move the why to `docs/proposals/`; keep the plan focused on work items,
dependencies, proof commands, and rollback.

### Active goal becomes prose

Keep `.adze/goals/active.toml` machine-readable. Link out to docs for long
explanations and generated tables.

### Agent hand-edits generated status

Add or run the named generator/checker. Generated status should not be edited by
hand unless the plan explicitly says so.

### Support claims drift

Require support-tier impact in source-of-truth artifacts and map README/product
claims to `docs/status/SUPPORT_TIERS.md` rows or equivalent proof pointers.

### Policy exceptions become silent debt

Every exception needs an owner, reason, `covered_by`, `review_after`, and an
expiry when temporary.

### Mega PR

Use one semantic artifact per PR and one implementation work item per PR unless
the selected plan item explicitly authorizes a combined change.

## What Good Looks Like

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

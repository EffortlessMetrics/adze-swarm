# Implementation Plans

Plans are the PR sequencing layer in Adze's source-of-truth stack. They answer
"how do we land this safely?" and point back to proposals, specs, ADRs, active
goals, status docs, and policy ledgers instead of duplicating their truth.

```text
Roadmap -> Proposal -> Spec -> ADR -> Plan -> Active goal -> PR -> Proof
```

## What Plans Own

Implementation plans own:

- PR-sized work items;
- dependencies and ordering;
- active/blocked/completed work-item state;
- acceptance for each PR-sized slice;
- proof commands;
- rollback notes;
- status handoff and closeout links.

Plans do not own product motivation, durable architecture decisions, generated
status, or support-tier claim tables. Put those in `docs/proposals/`,
`docs/adr/`, generated status files, and `docs/status/SUPPORT_TIERS.md`.

## Layout

Use a lane directory for each active or historical campaign:

```text
plans/<lane>/
  README.md
  implementation-plan.md
  closeout.md
```

Legacy single-file plans may remain where they are, but new lanes should use
the directory shape above.

## Plan Header

Each implementation plan should begin with:

```md
# <Lane> implementation plan

Status: proposed | active | completed | superseded
Owner:
Created:
Linked proposal:
Linked specs:
Linked ADRs:
Active goal:
Support-tier impact:
Policy impact:
```

Use `n/a` where a field does not apply.

## Work Item Template

````md
## Work item: short-id

Status: ready | active | blocked | completed | superseded
Linked proposal:
Linked spec:
Linked ADR:
Blocks:
Blocked by:

### Goal

One paragraph.

### Production delta

What files, commands, APIs, workflows, or behavior change?

### Non-goals

What is explicitly out of scope?

### Acceptance

What must be true for the PR to merge?

### Proof commands

```bash
cargo test ...
git diff --check
```

### Rollback

How to undo this PR safely.

### Notes

Optional.
````

## Agent Rules

Agents consuming plans must:

1. read `docs/reference/SPEC_SYSTEM.md` first;
2. read `.adze/goals/active.toml`;
3. select exactly one `ready` work item from the active lane;
4. read the linked spec and ADR constraints;
5. implement only that work item;
6. run the listed proof commands plus `git diff --check`;
7. update plan/status/receipts only if the work item says to.

If a plan lacks proof commands, rollback notes, or valid linked artifacts, stop
and report the gap instead of broadening scope.

## Closeout

At lane completion, add or update `plans/<lane>/closeout.md` with:

- what shipped;
- proof commands and receipts;
- merged PRs and CI runs;
- support-tier updates;
- policy updates;
- deferred work;
- claim boundary;
- next lane recommendation.

# Repo Source-Of-Truth System

Adze uses a linked source-of-truth stack so humans and automation can find the
right kind of truth without scraping chat history or treating stale status as
current work.

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

The operating rule is: do not make every document do every job. Separate why,
what, durable decisions, sequencing, active execution, and proof.

## Artifact Roles

| Artifact | Owns | Does not own |
|---|---|---|
| Roadmap | Release direction, milestone framing, high-level product strategy | Detailed PR queue, live status, generated metrics |
| Proposal | Why, users, affected surfaces, alternatives, success criteria | Behavior contract, PR order, generated status |
| Spec | Behavior, acceptance examples, proof, CI/test mapping | Product rationale, task sequencing, active queue |
| ADR | Durable architecture or operating decision | Task list, current metric state, implementation queue |
| Plan | PR order, work items, proof commands, rollback | Product rationale, durable architecture, generated truth |
| Active goal | Current machine-readable execution state | Long prose, generated metrics, durable decisions |
| Support tiers | Public claim proof and promotion requirements | Feature design or implementation sequencing |
| Policy ledgers | Exceptions, CI/policy intent, owners, coverage, review dates | Broad architecture or product rationale |

## Source-Of-Truth Locations

| Question | Source of truth |
|---|---|
| Why are we doing this? | `docs/proposals/` |
| What must be true? | `docs/specs/` |
| What architecture or operating decision constrains it? | `docs/adr/` |
| What PR-sized work lands next? | `plans/<lane>/implementation-plan.md` |
| What is the agent actively executing? | `.adze/goals/active.toml` |
| What proves a public claim? | `docs/status/SUPPORT_TIERS.md`, receipts, CI |
| What exceptions exist? | `policy/*.toml` |

## Required Headers

Use `n/a` where a field does not apply. More-specific artifact READMEs may add
fields, but these fields keep documents linkable and checkable.

### Proposal Headers

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

### Spec Headers

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

### ADR Headers

```md
Status:
Date:
Owner:
Linked proposal:
Linked specs:
Linked plan:
Linked issues:
Linked PRs:
Support-tier impact:
Policy impact:
```

### Plan Headers

```md
Status:
Owner:
Created:
Linked proposal:
Linked specs:
Linked ADRs:
Active goal:
Support-tier impact:
Policy impact:
```

## Rules

1. One kind of truth per artifact.
2. One semantic artifact per PR unless the selected plan item says otherwise.
3. Specs define behavior; plans define sequencing.
4. Proposals explain why; ADRs record durable choices.
5. Active goals tell agents what to do now.
6. Generated status is updated by tools, not by hand.
7. Public README or release claims require support-tier proof.
8. Policy exceptions require owner, reason, coverage, and review date.
9. Runtime/code PRs must link to the spec and plan item they implement.
10. If proof cannot run, record the command, reason, substitute evidence, and
    whether that blocks merge.

## Agent Boot Order

Agents must start from the rails, then narrow to one work item:

1. Read `AGENTS.md` or `CLAUDE.md`.
2. Read this file.
3. Read `.adze/goals/active.toml`.
4. Read the linked implementation plan.
5. Read the linked proposal only for why.
6. Read the linked spec for acceptance.
7. Read linked ADRs for constraints.
8. Inspect `git status --short` and do not overwrite unrelated work.
9. Pick exactly one ready work item.
10. Implement only that work item.
11. Run the listed proof commands plus `git diff --check`.
12. Update plan/status/receipts only when the work item requires it.
13. Open or update one focused PR.

If no ready work item is identifiable, stop and write a handoff instead of
inventing work.

## Stop Conditions

Stop and report instead of guessing when:

- the active goal is missing, paused without a selected lane, or stale;
- linked proposal/spec/ADR/plan files do not exist;
- a work item points to a missing plan anchor;
- requested work conflicts with an ADR;
- proof commands cannot run;
- generated status is dirty and no generator/check command is provided;
- unrelated staged changes exist;
- a public claim lacks support-tier proof;
- adding an exception would omit a policy ledger owner, reason, coverage, or
  review date.

## Active Goal Lifecycle

Activate exactly one current manifest at:

```text
.adze/goals/active.toml
```

Use `status = "active"` for an executable lane. Use `status = "paused"` with a
short `reason` when there is no selected implementation lane.

Archive superseded or completed manifests under:

```text
.adze/goals/archive/YYYY-MM-DD-<lane>.toml
```

Do not leave multiple active manifests.

## Policy Ledger Rules

Policy ledgers are receipts, not casual allowlists. Every exception entry should
include:

- stable `id`;
- owned `glob` or scope;
- `owner`;
- `reason`;
- `covered_by` proof commands or checks;
- `created` date;
- `review_after` date;
- `expires` when temporary.

Broad globs require an explicit reason and review date.

## Support-Tier Claim Rule

No README, release, or user-facing claim should be stronger than the matching
row in `docs/status/SUPPORT_TIERS.md` or an equivalent proof pointer. Stable
claims need concrete proof commands or receipts. Experimental and advisory
claims need limitations and next-promotion proof.

## Validation Commands

The intended validator family is:

```bash
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
cargo run -q -p xtask -- check-active-goal --mode blocking
git diff --check
```

Future repo-native checks may split this into `spec-system`, `goals`, `policy`,
and `support-tiers` commands. Until then, plan items should list the strongest
available checks for the artifacts they touch.

## Closeout Format

At the end of a lane, add or update:

```text
plans/<lane>/closeout.md
```

Use this shape:

```md
# Lane closeout: <lane>

Status: completed
Date:
Owner:
Linked proposal:
Linked specs:
Linked ADRs:
Linked plan:
Active goal archive:

## What Shipped

## Proof

## Receipts

## What Did Not Ship

## Deferred Work

## Claim Boundary

## Next Lane Recommendation
```

Closeout prevents the next agent from rediscovering old work.

## Common Failure Modes

### Spec Becomes A Task List

Move PR order to `plans/<lane>/implementation-plan.md`; keep the spec focused
on behavior, examples, and proof.

### Plan Becomes Product Rationale

Move why to `docs/proposals/`; keep the plan focused on work items,
dependencies, rollback, and proof.

### Active Goal Becomes Prose

Keep `.adze/goals/active.toml` machine-readable. Link out to long-form docs.

### Generated Status Is Hand-Edited

Run the generator/checker named in the plan and commit its output only when the
plan item requires it.

### Support Claims Drift

Require support-tier impact headers and map public claims to proof rows.

### Policy Exceptions Become Silent Debt

Every exception needs an owner, reason, coverage, and review date.

### Mega PR

Split work into one semantic artifact or one implementation work item per PR.

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

If the repository answers those questions without chat history, the method is
working.

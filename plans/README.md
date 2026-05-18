# Plans

Plans are the implementation sequencing layer. They answer "what PR lands next?"
and "what proof makes the PR mergeable?" for a proposal/spec lane.

Plans are not product rationale, durable architecture decisions, or generated
status. Link to those layers instead of duplicating them.

## Source Of Truth

Plans own:

- PR-sized work item order;
- dependencies, blockers, and status for work items;
- production delta for each item;
- acceptance checks and proof commands;
- rollback notes and handoff pointers.

Other artifacts own:

- why the lane exists: `docs/proposals/`;
- behavior contracts: `docs/specs/`;
- durable decisions: `docs/adr/`;
- active agent/operator state: `.adze/goals/active.toml`;
- public claim proof: `docs/status/SUPPORT_TIERS.md`;
- policy receipts and exceptions: `policy/*.toml`.

## Naming

Use a directory per lane or milestone:

```text
plans/<lane>/README.md
plans/<lane>/implementation-plan.md
plans/<lane>/closeout.md
```

Existing milestone plans under `plans/0.9.0/` and focused campaign plans under
`plans/glr-toolkit/` should keep their current names unless a selected work item
explicitly migrates them.

## Implementation Plan Header

Every implementation plan should start with:

```md
Status:
Owner:
Linked proposal:
Linked specs:
Linked ADRs:
Active goal:
```

## Work Item Template

```md
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
git diff --check
```

### Rollback

How to undo this PR safely.

### Notes

Optional.
```

## Agent Rules

- Select exactly one `ready` work item unless the user asks for planning only.
- Do not broaden a docs-only plan item into runtime/code changes.
- Run the proof commands listed on the selected item.
- If proof cannot run, record the command, reason, substitute evidence, and
  whether the failure blocks merge.
- Update support tiers, policy ledgers, generated status, or receipts only when
  the selected work item requires it.

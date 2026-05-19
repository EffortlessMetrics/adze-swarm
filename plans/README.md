# Implementation Plans

Implementation plans are the PR-sequencing layer of the Adze source-of-truth
stack.

They answer "what lands next?" for a lane. They do not own product motivation,
behavior contracts, durable architecture decisions, generated status, or public
support claims.

## Source of truth

Plans own:

- PR-sized work items;
- dependencies and blockers;
- acceptance criteria for each work item;
- proof commands;
- rollback notes;
- handoff and closeout pointers.

Other artifacts own:

- why the work exists: `../docs/proposals/`;
- behavior contracts: `../docs/specs/`;
- durable decisions: `../docs/adr/`;
- current machine-readable execution state: `../.adze/goals/active.toml`;
- public claim proof: `../docs/status/SUPPORT_TIERS.md`;
- exception receipts: `../policy/*.toml`.

## Layout

Use a stable lane directory:

```text
plans/<lane>/
  README.md
  implementation-plan.md
  closeout.md
```

Existing lanes include:

- [`0.9.0/`](./0.9.0/README.md) for contract convergence and API foundation work.
- [`glr-toolkit/`](./glr-toolkit/productization-plan.md) for the GLR toolkit productization campaign.
- [`toolkit-excellence/`](./toolkit-excellence/implementation-plan.md) for the completed toolkit excellence and adoption campaign.
- [`release-promotion/`](./release-promotion/implementation-plan.md) for the completed public promotion readiness campaign, prepared public-promotion decision plan, and active promotion execution plan.
- [`product-gap-burn-down/`](./product-gap-burn-down/implementation-plan.md) for the completed product-objective blocker burn-down lane.

## Plan header

Each implementation plan should start with linkage fields like:

```md
# Lane implementation plan

Status: active | complete | paused | superseded
Owner:
Created:
Linked proposal:
Linked specs:
Linked ADRs:
Active goal:
Support-tier impact:
Policy impact:
```

Use `n/a` when a field does not apply.

## Work item shape

Each work item should include:

````md
## Work Item: short-id

Status: ready | active | blocked | complete | superseded
Linked proposal:
Linked spec:
Linked ADR:
Blocks:
Blocked by:

### Goal

### Production Delta

### Non-Goals

### Acceptance

### Proof Commands

```bash
git diff --check
```

### Rollback
````

## Rules

- Keep plans as queues, not product strategy documents.
- Link to specs for behavior and acceptance contracts.
- Link to ADRs for durable constraints.
- Do not copy policy ledgers or support-tier tables into plans.
- Mark completed work items with the PR or receipt that proved them.
- If proof cannot run, record the command, reason, substitute evidence, and
  whether the missing proof blocks merge.

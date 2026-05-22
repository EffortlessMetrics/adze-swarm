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
- [`user-experience-hardening/`](./user-experience-hardening/closeout.md) for the completed non-release adoption polish lane.
- [`external-scanner-recovery/`](./external-scanner-recovery/closeout.md) for the completed non-release parser-generated external-token recovery proof lane.
- [`product-proof-result-readiness/`](./product-proof-result-readiness/closeout.md) for the completed Product Proof result-readiness lane.
- [`product-proof-required-gate/`](./product-proof-required-gate/implementation-plan.md) for the completed Product Proof required-gate burn-in and promotion lane.
- [`ci-lane-policy-hygiene/`](./ci-lane-policy-hygiene/implementation-plan.md) for the completed routed CI lane map cleanup.
- [`parser-runtime-maintainability/`](./parser-runtime-maintainability/implementation-plan.md) for the completed non-release parser/runtime maintainability lane.
- [`cli-parse-surface/`](./cli-parse-surface/closeout.md) for the completed non-release CLI parse-surface hardening lane.
- [`cli-static-sexp/`](./cli-static-sexp/closeout.md) for the completed non-release CLI static S-expression output lane.
- [`cli-static-json-dot/`](./cli-static-json-dot/closeout.md) for the completed non-release CLI static JSON and DOT output lane.
- [`cli-dynamic-parse/`](./cli-dynamic-parse/closeout.md) for the completed non-release CLI dynamic parse boundary-hardening lane.
- [`product-gap-burn-down/`](./product-gap-burn-down/implementation-plan.md) also carries the current paused release boundary: routine swarm proof is complete, while release/publish and crates.io install receipt work remain blocked on explicit authorization.

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

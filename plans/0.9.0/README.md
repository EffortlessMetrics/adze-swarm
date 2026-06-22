# Adze 0.9.0 Plans

This directory is the operational planning layer for the 0.9.0 milestone.
Implementation plans define PR-sized sequencing and proof commands. They do not
own product motivation, behavior contracts, or durable architecture decisions.

## Source Of Truth

Plans own:

- work item order
- PR-sized production deltas
- blockers and dependencies
- proof commands
- rollback notes
- closeout or handoff status

Other artifacts own:

- release direction and milestone strategy: `../../ROADMAP.md`
- why a campaign exists: `../../docs/proposals/`
- behavior contracts: `../../docs/specs/`
- durable architecture decisions: `../../docs/adr/`
- active agent/operator state: `../../.adze/goals/active.toml`
- product claim proof mapping: `../../docs/status/SUPPORT_TIERS.md`
- policy ledgers: `../../policy/*.toml`

## Expected Files

Use `implementation-plan.md` for the milestone overview. Substantial work
that was split into focused files:

```text
implementation-plan.md
api-foundation.md
microcrate-collapse.md
rust-1.95.md
closeout.md
```

## Work Item Template

````md
## Work Item: short-kebab-id

Status: ready | active | blocked | complete | superseded
Linked proposal:
Linked spec:
Linked ADR:
Blocks:
Blocked by:

### Goal

What outcome does this PR-sized item produce?

### Production Delta

What files, crates, policy entries, or docs are expected to change?

### Non-Goals

What must stay out of this work item?

### Acceptance

What must be true when the item is complete?

### Proof Commands

```bash
just ci-supported
```

### Rollback

How should this be reverted or disabled if it fails?
````

## Duplication Rule

Plans should link to specs for behavior and to `docs/status/SUPPORT_TIERS.md`
for product-claim proof. They should not recreate the full support-tier table
or policy TOML contents.

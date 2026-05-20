# The spec/proposal system, fully explained

The system is a **repo source-of-truth stack**. Its central rule is:

> **Do not make every document do every job.**

Each artifact owns one kind of truth: **why**, **what**, **what decision**, **how**, **what now**, **what proves it**, and **what changed**.

The end result is a repo where a human, Codex, Droid, Claude, or CI can answer:

```text
Why are we doing this?
What exact behavior must be true?
What architecture decision did we make?
What PR-sized work comes next?
What is the active lane right now?
What proves the claim?
Which support tier changed?
Which policy ledgers changed?
What happened after merge?
```

## 1) The stack at a glance

```text
Roadmap
  -> Proposal / PRD
    -> Specs
      -> ADRs where needed
        -> Implementation plan
          -> Active goal manifest
            -> Issues / PRs
              -> Proof commands
              -> CI lanes
              -> support-tier updates
              -> policy receipts
                -> Closeout / handoff
```

Each layer narrows the previous one.

- **Roadmap**: direction.
- **Proposal**: why this initiative should exist.
- **Spec**: behavior contract.
- **ADR**: architecture decision.
- **Implementation plan**: PR sequence.
- **Active goal manifest**: what Codex is executing now.
- **Support-tier map**: what users may believe.
- **Policy ledger**: exceptions/rules/receipts.
- **Closeout**: what actually happened.

## 2) Why the system exists

The point is **repo-operational memory**.

Without a source-of-truth stack, work drifts to stale chat context, old PR notes,
ambiguous READMEs, and unverified assumptions. With the stack, the repo itself
routes execution:

```text
.adze/goals/active.toml
  -> linked implementation plan
    -> linked spec
      -> linked proposal
        -> linked support-tier and policy proof
```

## 3) Artifact ownership model

### Roadmap
- Owns: release direction and milestone themes.
- Not for: acceptance tests or PR-sized checklists.

### Proposal / PRD
- Owns: why the work exists (problem, users, value, alternatives, risks).
- Not for: exact PR queue.

### Spec
- Owns: what behavior must be true, claim boundaries, and required evidence.
- Not for: sequencing.

### ADR
- Owns: durable architecture decisions.
- Not for: routine task-level changes.

### Implementation plan
- Owns: PR-sized sequencing, dependencies, proof commands, rollback.
- Not for: product rationale.

### Active goal manifest
- Owns: machine-readable “what now” execution state.
- Not for: long prose or generated status hand-edits.

### Support tiers
- Owns: claim -> proof mapping and tier posture.

### Policy ledgers
- Own: governed exceptions and CI/package/lint/file policies.

### Closeout / handoff
- Owns: what landed, what passed, what changed, and what remains.

## 4) Directory shape

```text
docs/
  proposals/
  specs/
  adr/
  status/
  handoffs/
plans/
  <milestone>/implementation-plan.md
.adze/goals/
  active.toml
policy/
  *.toml
```

## 5) Link discipline

The stack works when links are explicit and validated:

- roadmap -> proposal
- proposal -> spec + ADR + plan
- spec -> proposal + proof
- plan -> proposal/spec/ADR IDs + proof commands
- active goal -> plan work items
- PR -> plan/spec/proposal
- closeout -> landed artifacts + proof

## 6) Status lifecycle

Recommended controlled vocabularies:

- Proposals/specs/ADRs: `draft`, `proposed`, `accepted`, `implemented`, `superseded`, `rejected`
- Plan items: `ready`, `active`, `blocked`, `done`, `superseded`
- Active goals: `active`, `paused`, `complete`, `archived`

## 7) Anti-duplication rule

Keep each truth in one source and reference it from others instead of copying.

- Claim stability: `docs/status/SUPPORT_TIERS.md`
- CI lane policy: `policy/ci-lane-whitelist.toml`
- Package classification: `policy/package-boundary.toml`
- Active agent lane: `.adze/goals/active.toml`
- PR order: `plans/<milestone>/implementation-plan.md`

## 8) Codex operating flow

1. Read `.adze/goals/active.toml`.
2. Pick next `ready` work item.
3. Read linked plan item.
4. Read linked spec.
5. Read proposal for rationale only.
6. Read ADRs for constraints.
7. Make one PR-sized change.
8. Run listed proof commands.
9. Update only required ledgers/support tiers.
10. Record closeout/handoff context when a lane ends.

## 9) CI enforcement

Recommended checks:

```text
cargo xtask check-doc-artifacts
cargo xtask check-goals
cargo xtask check-package-boundary
cargo xtask check-ci-lanes
cargo xtask check-support-tiers
cargo xtask policy-report
```

## 10) PR contract

PRs should declare links, scope, non-goals, support-tier impact, policy impact,
proof commands, claim boundary, and rollback.

## 11) Minimal rollout sequence

1. Define model + templates.
2. Add doc artifact ledger.
3. Add doc artifact validator.
4. Add active goal manifest.
5. Add active-goal validator.
6. Add first proposal.
7. Add first spec.
8. Add support tiers.
9. Add package/CI/policy ledgers.
10. Wire CI checks (advisory first, then blocking where appropriate).

## 12) Simplest mental model

```text
Proposal = why.
Spec = what.
ADR = durable decision.
Plan = how.
Active goal = what Codex is doing now.
Support tiers = what users may believe.
Policy ledgers = exceptions + proof obligations.
CI = what proved it.
Closeout = what happened.
```

The system succeeds when layers are linked, validated, and non-duplicative.

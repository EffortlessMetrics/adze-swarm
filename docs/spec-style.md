# Adze spec-style system

Adze keeps a full source-of-truth stack for durable delivery memory:

```text
Roadmap -> Proposal -> Spec -> ADR -> Lane tracker -> Implementation plan
-> PR -> Proof -> Support/policy updates -> Closeout
```

## Durable home

The durable repo-native knowledge base lives in `.adze-spec/`.

- proposals = why, alternatives, success criteria
- specs = behavior contract and required evidence
- ADRs = durable architecture decisions
- lanes = focused implementation trackers and PR-sized execution plans
- support/policy maps = claim-to-proof and ledger references
- closeouts = what landed, what proved it, what remains

## External tool state

Agent and external tool namespaces are awareness-only for this system:

- `.codex/`
- `.spec/`
- `.claude/`
- `.jules/`

These locations may point to or read from durable artifacts, but they do not
own durable rails for this repo.

# Rails framework

This repository uses `.rails/` as the durable Rails knowledge base.

- `.rails/` contains long-lived source-of-truth artifacts (proposals, specs, ADRs, lane trackers, closeouts, support maps, policy references, and receipts).
- `docs/` explains Rails conventions for human readers and contributors.

## Awareness-only external namespaces

Rails is aware of external tool or agent namespaces, but does not own them:

- `.codex/` is Codex execution state and is not owned by Rails.
- `.spec/` is Spec Kit (speckit) state and is not owned by Rails.
- `.claude/` and `.jules/` are external agent/session spaces and are not owned by Rails.

## Ownership boundary

No Rails-owned artifact path may live under `.codex/`, `.spec/`, `.claude/`, or `.jules/`.
All Rails-owned artifacts must live under `.rails/` and be linked through `.rails/index.toml`.

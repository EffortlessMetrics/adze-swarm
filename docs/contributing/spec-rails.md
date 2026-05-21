# Contributing guide: repo-native spec rails

When adding or updating source-of-truth artifacts in Adze, use `.adze-spec/`
for durable rails and keep tool-specific directories external.

## Required boundaries

Owned scope for this system:

- `.adze-spec/`
- `docs/spec-style.md`
- `docs/contributing/spec-rails.md`
- `policy/*.toml` only when referencing existing live ledgers
- `plans/` only where already used by the repo as a non-agent planning surface

Do not treat these directories as durable-owned rails:

- `.codex/`
- `.spec/`
- `.claude/`
- `.jules/`

## Working rule

Use the complete stack while preserving artifact separation:

```text
proposal -> spec -> ADR -> lane tracker -> implementation plan -> PR -> proof -> support/policy -> closeout
```

Each durable artifact should be indexed through `.adze-spec/index.toml`. No
owned artifact path should live under `.codex/`, `.spec/`, `.claude/`, or
`.jules/`.

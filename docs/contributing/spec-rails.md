# Contributing: repo-native spec rails

When adding or updating durable source-of-truth artifacts, place them under
`.adze-spec/`.

## Rules

1. Keep one kind of truth per artifact.
2. Keep durable rails in `.adze-spec/`, not agent/session namespaces.
3. Reference live policy ledgers in `policy/*.toml` instead of duplicating them.
4. Use `docs/` for human-facing explanation and contributor guidance.

## Awareness-only namespaces

The following directories are external/tool-specific state and are not owned by
this durable spec system:

- `.codex/`
- `.spec/`
- `.claude/`
- `.jules/`

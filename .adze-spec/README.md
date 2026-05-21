# .adze-spec — durable repo-native spec namespace

This directory is the durable, repo-owned source-of-truth namespace for Adze
spec rails.

It is intended to hold long-lived artifacts such as roadmap items, proposals,
specs, ADRs, lane trackers, implementation plans, support claim maps, policy
references, and closeouts.

## Namespace ownership

Owned durable rails:

- `.adze-spec/` (this namespace)
- `docs/` human guidance that explains how to use the rails
- `policy/*.toml` as live ledgers referenced from durable artifacts

Awareness-only external namespaces (not owned by this system):

- `.codex/`
- `.spec/`
- `.claude/`
- `.jules/`

Those directories may exist for tool/session execution state, but they are not
managed as durable repo-native rails.

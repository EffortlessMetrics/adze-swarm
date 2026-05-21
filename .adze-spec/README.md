# .adze-spec — durable repo-native spec rails

This directory is Adze's durable source-of-truth control plane for repo-owned
spec artifacts.

## Ownership

The `.adze-spec/` namespace owns long-lived repository knowledge rails:

- proposals (`.adze-spec/proposals/`)
- specs (`.adze-spec/specs/`)
- ADRs (`.adze-spec/adr/`)
- lane trackers and implementation plans (`.adze-spec/lanes/`)
- support claim maps or support references (`.adze-spec/support/`)
- policy ledger references (`.adze-spec/policy/`)
- closeouts (`.adze-spec/closeouts/`)

## External tool namespaces

Tool-specific or session-specific directories are awareness-only for this lane:

- `.codex/`
- `.spec/`
- `.claude/`
- `.jules/`

These directories are not owned by the repo-native spec rails. They may read
from `.adze-spec/` but they do not define durable artifact truth.

# Spec style and source-of-truth ownership

Adze keeps the full source-of-truth stack:

```text
Roadmap -> Proposal -> Spec -> ADR -> Lane tracker -> Implementation plan -> PR -> Proof -> Support/policy -> Closeout
```

The durable rails live in a repo-owned namespace:

```text
.adze-spec/
```

## Namespace boundaries

### Durable, repo-owned rails

- `.adze-spec/` contains durable proposal/spec/ADR/lane/closeout artifacts.
- `docs/` explains the method and contributes human-facing guidance.
- `policy/*.toml` stays the live enforcement ledger surface and may be
  referenced by `.adze-spec/`.
- `plans/` remains valid when already used as the repo's non-agent planning
  surface.

### Awareness-only tool state

- `.codex/` is Codex execution state.
- `.spec/` is Spec Kit/speckit execution state.
- `.claude/` and `.jules/` are tool/session execution state.

These namespaces are not owned by this spec system and are not the durable
source of truth for the lane.

## Artifact discipline

Separate artifact concerns instead of collapsing multiple responsibilities into
one file:

- proposals own *why*
- specs own *what must be true*
- ADRs own *durable architectural decisions*
- lane trackers and plans own *how work is sequenced*
- proof commands own *what proves behavior*
- closeouts own *what happened*

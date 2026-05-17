# Branch protection

## Today

`adze-swarm` branch protection requires exactly one GitHub status check:

```text
Rust Small Result
```

This check is emitted by `.github/workflows/em-ci-routed-rust.yml` after the
routed Rust Small lane runs on CX43 or GitHub-hosted fallback.

`just ci-supported` remains the local supported/product proof command. It is
not the required GitHub branch-protection context in `adze-swarm`.

Conversation resolution is intentionally disabled in `adze-swarm`. Review bots
can still leave useful comments, but unresolved advisory threads must not block
the single-operator swarm merge path after the required gate is green.

## Future

The public-era rollout also defines an aggregated check called
**PR Gate Success** (see `.github/workflows/pr-gate.yml`). It depends on:

- `PR Plan` (advisory)
- `Supported Rust Gate` (= `just ci-supported`)
- `Docs Gate` (fmt only, runs only on docs-only PRs)

`PR Gate Success` succeeds when exactly one of `Supported Rust Gate` /
`Docs Gate` succeeded and the other was skipped. PR Plan must not fail.

In `adze-swarm`, this remains optional signal unless a later PR explicitly
updates both `.github/settings.yml` and [CI_LANES.md](../../.github/CI_LANES.md).

## PR 17 — promotion criteria

Any future branch-protection promotion away from `Rust Small Result` is gated on:

| Criterion | Target |
| --- | --- |
| `PR Gate Success` job has run on every PR for | ≥ 14 calendar days |
| `PR Gate Success` flake rate | < 1% |
| Number of distinct PRs that exercised both `Supported Rust Gate` and `Docs Gate` paths | ≥ 5 each |
| `ci-actuals.json` artifacts uploaded | ≥ 30 PRs |
| Manual review of band/LEM accuracy | passes |

When all five gates clear, the promotion PR itself only:

1. updates `.github/settings.yml` (and any equivalent platform config) to
   require the new context and stop requiring `Rust Small Result`, **and**
2. updates this file and [CI_LANES.md](../../.github/CI_LANES.md) in the same
   PR.

## Rollback

If a future required-check promotion causes problems, the rollback is to
restore `.github/settings.yml` to require `Rust Small Result` and update
[CI_LANES.md](../../.github/CI_LANES.md) to match.

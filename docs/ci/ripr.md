# ripr advisory

`ripr` is static mutation-exposure analysis. It sits between line coverage
(too coarse) and runtime mutation testing (too expensive) and asks a
narrower question for each behavioral delta:

> Is this changed code path exposed to a meaningful test discriminator?

It catches mutation-shaped weak-test and weak-oracle signal earlier and
cheaper because it is static and PR-time. It does **not** run mutants, report
killed/survived outcomes, prove correctness, or replace runtime mutation
testing. Mutation testing remains the slower runtime backstop. Reading or
writing ripr policy without that distinction will produce garbled mental
models — see `docs/ci/cost-and-verification-policy.md`.

## Where it sits in the ladder

`ripr` runs at Tier 1 (frontdoor advisory). It runs on every Rust-touching
PR, never on docs-only PRs, never on `merge_group`, and is configured with
`continue-on-error: true` so that failures are visible but never block.

## MSRV and provisioning

Adze pins `rust-toolchain.toml` to `1.95.0`. `ripr` requires `1.93+`, so the
advisory workflow can use the workspace MSRV toolchain. Provisioning options,
in order of preference:

1. **Pinned prebuilt binary** — drop a `ripr` binary on the runner via a
   release asset URL or self-hosted artifact mirror.
2. **Workspace toolchain** — install through the pinned MSRV toolchain with
   `cargo install --locked ripr`.
3. **Stub report** — if install fails, upload a skipped advisory report rather
   than blocking the PR.

Until one of those is wired in, the workflow detects the absence of `ripr`
and emits a stub `ripr-report.json` with `"status": "skipped"`. The
advisory step is therefore never an obstacle to landing PRs.

## Configuration

| File | Purpose |
| --- | --- |
| `ripr.toml` | analysis mode, severities, suppression ledger pointer |
| `policy/ripr-suppressions.toml` | per-path suppressions with owner/expiry |

Severities are pinned to `notice` / `warning`. There is no `error`
severity in the adze configuration; ripr is advisory by policy and by
config, not just by workflow flag.

## Suppressions

Each suppression must declare `path-glob`, `finding`, `owner`, `reason`,
and `expires`. Unsuppressed `weakly_exposed` findings are a reviewer
prompt, not a build break.

## When to take a finding seriously

| Finding | Take seriously when |
| --- | --- |
| `exposed` | rarely — the test surface looks fine |
| `weakly_exposed` | new behavior, parser/runtime, or hot path |
| `reachable_unrevealed` | always for parser/glr-core/tablegen changes |
| `no_static_path` | almost never — it usually reflects analyzer limits |
| `*_unknown` | only if surrounding tests are also new |

## Rollback

Removing `.github/workflows/ripr.yml` removes the lane. Suppressions and
config TOML are inert without the workflow.

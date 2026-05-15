# Runner classes

This document defines the runner-capacity model for `adze-swarm` and sibling
high-volume repos. The goal is to keep GitHub Actions as the coordination
layer, not the default Linux build farm.

## Classes

| Class | Intended use | Not for |
| --- | --- | --- |
| `cx43` / `rust-small` | Required same-repo PR base gate, warm-cache `cargo check`, small bounded Linux checks | Full platform matrix, fuzz, benchmarks, release signing |
| `cx53` / `rust-large` | Larger Linux lanes, parser/golden shards, heavier affected tests, `coverage-lite` if still CPU-heavy | Windows, macOS, public fork PRs, publish/signing tokens |
| GitHub-hosted | Scoped fallback, Windows/macOS, public-fork-safe checks, release/publish/signing, small receipt jobs | Default Linux compute for high-volume same-repo PRs |

## Required base lane

`adze-swarm` branch protection requires:

```text
Rust Small Result
```

The result job is the branch-protection contract. Conditional implementation
jobs such as `Rust Small on CX43` and `Rust Small on GitHub Hosted` are not
required directly because one is expected to skip.

Current route:

```text
Rust Small Result
  -> CX43 when idle
  -> GitHub-hosted fallback when CX43 is unavailable
```

Future route once `cx53` is online:

```text
rust-small:
  CX43 first
  CX53 second if explicitly allowed
  GitHub-hosted fallback

rust-large:
  CX53 first
  GitHub-hosted fallback
```

`rust-large` must be introduced as an optional result lane first. Do not make it
branch-protection-required until it has burn-in receipts.

## CX53 rust-large prep

`cx53` is the planned larger Linux capacity class. It should be registered as a
single-slot self-hosted runner with precise labels:

```text
self-hosted
linux
x64
em-ci
cx53
rust-large
rust-small
trusted-pr
```

The first `rust-large` workflow should be optional. It should target parser,
golden, coverage-lite, or other heavier Linux-only affected lanes after those
lanes are already path-routed. It must not become a back door for the old full
OS matrix.

Fallback rules:

```text
rust-small:
  CX43 first
  CX53 overflow only when explicitly allowed
  GitHub-hosted fallback

rust-large:
  CX53 first
  GitHub-hosted fallback
```

Burn-in before any branch-protection change:

- [ ] Runner group access is limited to the intended repos.
- [ ] Workflows target `cx53` / `rust-large`, not generic `self-hosted`.
- [ ] Public fork PRs cannot select the self-hosted runner.
- [ ] Manual dispatch succeeds.
- [ ] Same-repo PR smoke succeeds.
- [ ] GitHub-hosted fallback succeeds for the same scoped lane.
- [ ] Router logs distinguish `cx53_idle`, `no_idle_runner`, and fallback.
- [ ] At least three clean PRs prove the optional result lane.
- [ ] Branch protection still requires only `Rust Small Result`.

## Lane policy

| Lane type | Trigger | Examples |
| --- | --- | --- |
| Base | Every same-repo PR and merge queue entry | `Rust Small Result` |
| Affected | Path-routed or risk-pack-routed | Pure Rust code-path lane, microcrate groups, golden tests |
| Heavy | Label, manual, schedule, or release-bound | coverage-full, full OS matrix, fuzz, benchmarks, Miri, sanitizers |
| Release | Public repo only unless explicitly promoted | publish, signing, external credentials |

Docs-only updates are the expected routing probe: they should run the base
result and cheap docs/policy receipts while Pure Rust, coverage, golden, and
microcrate implementation lanes skip unless explicitly requested.
After microcrate receipt routing, the same probe should also skip Microcrate
CI formatting, workspace docs, WASM, and strict-feature receipt jobs.

Fallback must mean fallback for the selected scoped lane. It must not recreate
the old public-style full-CI fanout.

Label events follow the same rule. Adding an unrelated label to a PR should not
restart path detectors or implementation jobs; only labels that request a lane
should wake that lane.

## Coverage split

Coverage should have two modes:

```text
coverage-lite:
  path-routed or label-routed PRs
  Linux stable only
  selected packages or changed crate group
  artifact generation is the proof
  Codecov upload is non-blocking publication

coverage-full:
  workflow_dispatch, schedule, release, or full-ci
  full workspace and broader feature set
```

## Router observability

Router logs should expose why a target was selected:

```text
router_target=cx43
router_target=cx53
router_target=github
router_reason=cx43_idle
router_reason=cx53_idle
router_reason=no_idle_runner
router_reason=runner_api_failed
router_reason=token_missing
router_reason=parse_failed
```

Watch fallback counts. If GitHub-hosted fallback dominates a same-repo lane,
either add capacity or narrow the trigger.

## Repo migration checklist

Use this checklist before moving another high-volume repo to the routed model:

- [ ] Repo has access to the intended runner group.
- [ ] Runner labels are precise; workflows do not target generic `self-hosted`.
- [ ] Router token is scoped to runner read-only use.
- [ ] Same-repo PR guard excludes public forks from self-hosted runners.
- [ ] Required result job exists and is the only branch-protection context.
- [ ] Manual dispatch is green.
- [ ] Same-repo PR smoke is green.
- [ ] GitHub-hosted fallback path is green for the scoped lane.
- [ ] Broad matrix is label/manual/schedule gated.
- [ ] Release, publish, signing, and external credentials stay public-repo-only.
- [ ] Branch protection is deferred until several clean PRs have burned in.

## Current migration order

Prioritize repos by gross hosted-runner pressure and agentic PR volume:

1. `BitNet-rs`
2. `adze` / `adze-swarm`
3. `perl-lsp`
4. `tokmd`

For each repo, start with a small Rust result lane. Add larger lanes only after
the base route is predictable.

# Runner classes

This document defines the runner-capacity model for `adze-swarm` and sibling
high-volume repos. The goal is to keep GitHub Actions as the coordination
layer, not the default Linux build farm.

## Classes

| Class | Intended use | Not for |
| --- | --- | --- |
| `cx43` / `rust-small` | Required same-repo PR base gate, warm-cache `cargo check`, small bounded Linux checks | Full platform matrix, fuzz, benchmarks, release signing |
| `cx33` / `rust-small` | Backfill Rust Small capacity when the primary small lane is busy | Default hosted fallback, release signing, heavy matrices |
| `cx53` / `rust-large` | Larger Linux lanes, parser/golden shards, heavier affected tests, `coverage-lite` if still CPU-heavy | Windows, macOS, public fork PRs, publish/signing tokens |
| GitHub-hosted | Explicitly approved fallback, Windows/macOS, public-fork-safe checks, release/publish/signing, small receipt jobs | Default Linux compute for high-volume same-repo PRs |

## Required base lane

`adze-swarm` branch protection requires:

```text
Rust Small Result
Product Proof Result
```

The result jobs are the branch-protection contracts. Conditional implementation
jobs such as `Rust Small on CPX42`, `Rust Small on CX43`,
`Rust Small on CX33`, `Rust Small on CX53`, and
`Rust Small on GitHub Hosted` are not required directly because only one routed
Rust Small implementation lane is expected to run.

Current route:

```text
Rust Small Result
  -> CPX42 when idle
  -> CX43 when CPX42 is unavailable and CX43 is idle
  -> CX33 when CPX42/CX43 are unavailable and CX33 is idle
  -> explicit GitHub-hosted fallback only when a recorded exception allows it
```

CX53 is quarantined from the required Rust Small route while
`adze-swarm#598` remains blocked. The router still logs CX53 candidate state so
the runner can be diagnosed, but it must not select CX53 for the normalized
base gate until runner-group, label, scheduling, and burn-in evidence are
recorded.

The route job's own runner is excluded from idle counts, even when it has
matching labels. Candidate summaries still mark it with `current=true`, but the
router must not treat the scheduler slot currently running the route job as
immediately available selected-lane capacity.

Planned optional `rust-large` route:

```text
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
trusted-pr
```

The first `rust-large` workflow should be optional. It should target parser,
golden, coverage-lite, or other heavier Linux-only affected lanes after those
lanes are already path-routed. It must not become a back door for the old full
OS matrix.

Fallback rules:

```text
rust-small:
  CPX42 first
  CX43 primary small lane
  CX33 backfill
  explicit GitHub-hosted fallback only by recorded exception

rust-large:
  CX53 first
  explicit GitHub-hosted fallback only by recorded exception
```

Burn-in before any branch-protection change:

- [ ] Runner group access is limited to the intended repos.
- [ ] Workflows target `cx53` / `rust-large`, not generic `self-hosted`.
- [ ] Public fork PRs cannot select the self-hosted runner.
- [ ] Manual dispatch succeeds.
- [ ] Same-repo PR smoke succeeds.
- [ ] GitHub-hosted fallback succeeds for the same scoped lane.
- [ ] Router logs show CX53 candidate state while Rust Small selection remains
      quarantined.
- [ ] At least three clean PRs prove the optional result lane.
- [ ] Branch protection does not require `rust-large` directly; required
      contexts remain the aggregate `Rust Small Result` and
      `Product Proof Result`.

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
router_target=cpx42
router_target=cx43
router_target=cx33
router_target=github
router_reason=cpx42_idle
router_reason=cx43_idle
router_reason=cx33_idle
router_reason=no_idle_runner
router_reason=runner_api_failed
router_reason=token_missing
router_reason=parse_failed
```

While `adze-swarm#598` is blocked, CX53 should appear only in candidate
summaries for the Rust Small router, not as a selected `router_target`.
Candidate rows with `current=true` are diagnostics only; the current route
runner is not counted as idle capacity.

Watch fallback counts. If GitHub-hosted fallback dominates a same-repo lane,
either add capacity or narrow the trigger.

## Repo migration checklist

Use this checklist before moving another high-volume repo to the routed model:

- [ ] Repo has access to the intended runner group.
- [ ] Runner labels are precise; workflows do not target generic `self-hosted`.
- [ ] Router token is scoped to runner read-only use.
- [ ] Same-repo PR guard excludes public forks from self-hosted runners.
- [ ] Required result jobs are aggregate contexts, not implementation jobs.
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

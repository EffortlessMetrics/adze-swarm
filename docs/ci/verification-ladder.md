# Verification ladder

Adze verification is organized as a ladder. Every PR climbs as far up the
ladder as its risk pack justifies — no further.

## Tier 0 – Frontdoor (every PR, blocking)

| Lane | Source | Why |
| --- | --- | --- |
| `ci-supported` | `just ci-supported` | Portable workspace format proof, clippy, tests on the supported crate set |
| PR Gate Success | summary check | One required target for branch protection |

## Tier 1 – Frontdoor advisory (every PR, non-blocking)

| Lane | Why |
| --- | --- |
| PR Plan | LEM forecast + lane selection |
| ripr | static RIPR exposure for changed code |
| CI lane whitelist | enforce that workflows are governed |

## Tier 2 – Risk-routed (selected by risk pack)

| Risk pack | Adds |
| --- | --- |
| `core_runtime` | core tests, ripr |
| `glr_core` | parser tests, fuzz build smoke, perf compile |
| `tablegen` | tablegen ABI/canary lanes |
| `grammar_golden` | golden tests for grammars |
| `microcrate_governance` | governance integration tests and BDD governance support |
| test-policy paths | test hygiene and static inventory for test-policy changes; runtime caps run on main/manual |
| `concurrency` | concurrency owner-module opt-in; no standalone concurrency microcrates remain |
| `wasm` | wasm-check |
| `performance` | quick benchmark compile |
| `manifest_release` | api-stability advisory |

## Tier 3 – Deep (main / nightly / label only)

| Lane | Trigger |
| --- | --- |
| pure-rust OS matrix | `main`, nightly, `platform-matrix`, `full-ci` |
| fuzz runtime | nightly, `fuzz`, `full-ci` |
| coverage | nightly, `coverage`, `full-ci` |
| Miri | nightly, `full-ci` |
| sanitizers | nightly, `full-ci` |
| feature matrix | nightly, `full-ci` |
| full benchmarks | `main`, `ci:perf`, `full-ci` |
| product proof advisory | weekly, `workflow_dispatch` |

## Tier 4 – Release (tag / manual / weekly)

| Lane | Trigger |
| --- | --- |
| MSRV check | `main`, manual |
| security audit | weekly, `security-audit`, manifest changes |
| API/semver stability | release branches, `release-check` |
| docs build | docs paths, weekly |
| publish dry-run | release tag |

## ripr in the ladder

`ripr` is intentionally placed at Tier 1 (advisory). It gives mutation-testing-
*lite* signal at static-analysis prices: it does not run mutants or report
killed/survived outcomes; it asks whether changed behavior appears exposed to
a meaningful test discriminator.

That makes it a good fit for parser/runtime/tablegen deltas where small
changes need meaningful oracles, but a poor fit for hard gating: a "weakly
exposed" finding is a reviewer prompt, not a build break.

## What climbs the ladder

The ladder is climbed by:

1. The **PR Plan** advisory step, which selects lanes from
   `policy/ci-risk-packs.toml` based on changed paths and labels.
2. **Labels**, which can opt in to higher tiers (`full-ci`, `platform-matrix`,
   `fuzz`, etc.).
3. **Branch context** — `main` and nightly always run more than PR.

What does *not* climb the ladder is "we ran this last time so we'll run it
again." Every Tier 2+ run must come from a risk pack match or an explicit
trigger.

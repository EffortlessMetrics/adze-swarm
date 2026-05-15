# CI cost and verification policy

This document is the doctrine anchor for the adze CI economics rollout.
It is referenced by every workflow, every policy ledger entry, and every
follow-up PR in the rollout.

## Why this exists

We are not reducing CI because we want less verification.

Adze needs *more* verification than traditional PR workflows can economically
support: parser correctness, GLR behavior, generated tables, typed extraction,
grammar parity, product proof, WASM, feature compatibility, benchmarks, and
release/API stability.

The problem is not verification. The problem is verification economics.

At high agentic PR volume, broad defaults become the product's operating cost.
Adze targets a different model: Rust-native checks, cheap oracle-gap detection
with `ripr`, LEM visibility, and risk-routed deep lanes.

The runner-class model is documented in
[`runner-classes.md`](./runner-classes.md). GitHub-hosted fallback must stay
scoped to the selected lane; it must not recreate the public-era full-CI fanout.

## The unit: LEM

`LEM = wall-clock job minutes × runner multiplier`

Linux is the unit (`1.0`). Windows costs `2x`, macOS costs `10x`, and some
external services (Docker build farms, AI review) carry their own multipliers.

| Band | LEM | Behavior |
| --- | --- | --- |
| ordinary | 0–35 | green; preferred default <25 |
| elevated | 36–75 | warning; explicit risk surface |
| high | 76–125 | high warning; explicit label/ack |
| over ceiling | >125 | fails unless `full-ci` or `ci-budget-override` |

The target is sub-`$0.50` ordinary PRs when possible. `$1` is a ceiling, not
the design center.

## What gets verified, where

| Tier | Trigger | Examples |
| --- | --- | --- |
| frontdoor | every PR, blocking | `Rust Small Result` in `adze-swarm`; `just ci-supported` remains the local supported/product proof |
| advisory | every PR, non-blocking | PR Plan, ripr |
| risk-routed | risk pack or path matches | parser fuzz build, golden, microcrate group, test-policy |
| deep | `main`, nightly, label | OS matrix, fuzz runtime, full benchmarks |
| release | tag, manual | semver, MSRV, security audit |

## How we get there

The rollout is not "delete CI". It is, in order:

1. Document the policy (this doc).
2. Inventory every existing CI lane in `policy/ci-lane-whitelist.toml`.
3. Lint workflows against that whitelist (advisory).
4. Forecast each PR's cost with PR Plan (advisory).
5. Stand up `PR Gate Success` as the future required check.
6. Normalize cache save semantics so PRs restore but only `main` saves.
7. Add `ripr` as cheap oracle-gap detection.
8. Encode risk packs so routing has a vocabulary.
9. Make planning testable with `xtask ci plan`.
10. Route the heavy lanes (fuzz, OS matrix, benchmarks, golden, microcrate) by
    risk pack and label.
11. Calibrate from actuals.
12. Promote `PR Gate Success` to the required branch protection.

See `docs/ci/adze-rollout-plan.md` for the per-PR breakdown.

## What we will not do

- Weaken the supported product proof lane (`just ci-supported`).
- Make `ripr` blocking.
- Enforce learned LEM budgets before actuals exist.
- Combine docs, policy, and routing changes into a single PR.
- Remove broad validation from `main`/nightly/label paths.

## Related

- `docs/ci/lem-budgeting.md` – how LEM is computed and budgeted
- `docs/ci/runner-classes.md` – CX43, CX53, and GitHub-hosted runner roles
- `docs/ci/verification-ladder.md` – tiers and what they prove
- `docs/ci/adze-rollout-plan.md` – per-PR rollout plan and status
- `docs/ci/labels.md` – label vocabulary used by routing
- `policy/ci-lane-whitelist.toml` – lane registry
- `policy/ci-risk-packs.toml` – risk pack routing map

# Public Drift Audit

Status: active
Owner: release/product
Created: 2026-05-19
Linked proposal: ../../docs/proposals/ADZE-PROP-0005-release-promotion-readiness.md
Linked plan: ./implementation-plan.md
Previous inventory: ./readiness-inventory.md

## Purpose

This audit compares public `EffortlessMetrics/adze` with the operating
`EffortlessMetrics/adze-swarm` repository before any public promotion PR is
prepared. It is a classification receipt only; it does not open, merge, or
prepare a public PR.

## Initial Live Queue Check

Commands run from `C:\Code\Rust2\adze-swarm`:

```bash
gh pr list --repo EffortlessMetrics/adze --state open --json number,title,headRefName,baseRefName,isDraft,updatedAt,url
gh pr list --repo EffortlessMetrics/adze-swarm --state open --json number,title,headRefName,baseRefName,isDraft,updatedAt,url
```

Results:

| Repo | Open PRs | Classification |
| --- | ---: | --- |
| `EffortlessMetrics/adze` | 0 | Public repo has no active drift PR queue. |
| `EffortlessMetrics/adze-swarm` | 7 | Initial snapshot only: swarm-side code-quality/test PRs #205-#212; later resolved before the follow-up refresh below. |

Open `adze-swarm` PRs observed:

| PR | Title | Classification |
| ---: | --- | --- |
| #205 | Add unit tests for common type ops | Swarm-side test PR; not promotion scope unless refreshed and accepted. |
| #206 | test: expand TreeNodeData unit coverage | Swarm-side test PR; not promotion scope unless refreshed and accepted. |
| #207 | test(tool): cover grammar discovery traversal | Swarm-side test PR; not promotion scope unless refreshed and accepted. |
| #208 | Improve FIRST/FOLLOW unit coverage and handle external symbols | Swarm-side parser/test PR; likely product-relevant but must be refreshed against current `main`. |
| #209 | Improve external scanner discovery | Swarm-side implementation PR; likely product-relevant but must be refreshed against current `main`. |
| #211 | Improve GLR conflict inspection details | Swarm-side implementation/test PR; likely product-relevant but must be refreshed against current `main`. |
| #212 | Improve IR registry construction | Swarm-side implementation PR; likely product-relevant but must be refreshed against current `main`. |

## Follow-up Queue Refresh

After release-readiness closeout and post-closeout product-proof alignment PRs
#241-#246, a live queue refresh from `C:\Code\Rust2\adze-swarm` showed no open
PRs in `EffortlessMetrics/adze` or `EffortlessMetrics/adze-swarm`.

The initial #205-#212 entries above are historical audit entries, not active
promotion blockers. Do not revive those PR numbers unless a fresh GitHub query
shows the corresponding work open again.

## Branch Comparison

Commands:

```bash
git fetch public --prune --tags
git rev-list --left-right --count public/main...origin/main
git log --oneline public/main --not origin/main
git log --oneline origin/main --not public/main -30
```

Observed heads:

| Remote | Head |
| --- | --- |
| `public/main` | `5fc7924b xtask: own goto indexing guard (#789)` |
| `origin/main` | `a68c21b8 docs(release): inventory promotion readiness (#235)` |

These heads are the initial audit receipt. Refresh this comparison from current
`adze-swarm/main` before opening any public promotion branch.

Commit count:

```text
public-only: 6
swarm-only: 204
```

The large swarm-only count is expected. `adze-swarm` is the working repo and
contains the completed 0.9, GLR toolkit, toolkit excellence, CI economics, and
release-promotion readiness campaigns.

## Public-Only Commit Classification

| Public commit | Title | Classification | Promotion action |
| --- | --- | --- | --- |
| `c9c40728` | `test(cli): align README capability tiers with support tiers (#785)` | Subsumed in `adze-swarm`. Current swarm `README`, `SUPPORT_TIERS.md`, `scripts/ci-product-stable.sh`, and `cli/tests/readme_quickstart.rs` include the Stable-claim guardrail and support-tier alignment. | No direct cherry-pick. Re-check during claim freeze. |
| `92cc08ae` | `xtask: report Rust migration candidates in file-policy (#783)` | Useful public-only policy improvement identified by this audit. | Ported to `adze-swarm` in #237. |
| `a788f921` | `plans: align 0.9 closeout state (#786)` | Superseded in `adze-swarm` by completed 0.9 closeout and later campaign closeouts. | No direct cherry-pick. Use swarm closeouts as source of truth. |
| `f8ba5ff9` | `plans: add 0.9 contract convergence closeout (#787)` | Superseded in `adze-swarm`; `plans/0.9.0/closeout.md` exists with later release-operation proof and known gaps. | No direct cherry-pick. Use swarm closeout. |
| `5685ae35` | `xtask: own no-mangle guard (#788)` | Ported/superseded in `adze-swarm` by `a2178cc3 xtask: own Rust guard checks (#124)`. Current swarm has `xtask/src/no_mangle.rs` and `cargo xtask check-no-mangle`. | No direct cherry-pick. |
| `5fc7924b` | `xtask: own goto indexing guard (#789)` | Ported/superseded in `adze-swarm` by `a2178cc3 xtask: own Rust guard checks (#124)` and later GLR invariant routing. Current swarm has `xtask/src/goto_indexing.rs`, GOTO docs, and CI lane policy proof. | No direct cherry-pick. |

## Promotion Blockers From This Audit

- Public `adze` has no open PR blocker.
- The initial `adze-swarm` PRs #205-#212 were resolved before the latest
  refresh. They are not current blockers.
- Any new public promotion decision must still refresh both queues and classify
  whatever is open at that time.
- Public-only commit `92cc08ae` was the only observed public commit that
  appeared useful and not already represented in `adze-swarm`; it was ported in
  #237 and is no longer a blocker.

## Recommended Next Step

Before opening a public promotion PR, use
`./public-promotion-pr-plan.md` from current `adze-swarm/main`, refresh both
PR queues, rerun the required proof commands, and record any new drift by name.

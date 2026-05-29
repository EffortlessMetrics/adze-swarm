# Current Release Candidate Snapshot

Status: current snapshot
Owner: release/product
Updated: 2026-05-29
Linked proposal: ../../docs/proposals/ADZE-PROP-0017-release-candidate-bundle.md
Linked active goal: ../../.adze/goals/active.toml
Release authorization tracker: https://github.com/EffortlessMetrics/adze-swarm/issues/325

## Scope

This snapshot records the current non-publish release-candidate boundary from
`EffortlessMetrics/adze-swarm`. It is evidence for a future promotion decision,
not authorization to promote, tag, publish, sign, use Cargo tokens, or claim
crates.io installation.

## Selected Swarm State

Current selected `adze-swarm/main`:

```text
509ff83fa94cabf5ba111a598addd95b929465a0
docs(goal): start release candidate bundle readiness (#554)
```

Local checkout state when captured:

```text
git status --short --branch
## main...origin/main
```

Open PR queues when captured:

```text
gh pr list --repo EffortlessMetrics/adze-swarm --state open --json number,title,url
[]

gh pr list --repo EffortlessMetrics/adze --state open --json number,title,url
[]
```

## Public Drift Boundary

Public `adze/main` when captured:

```text
6263c6a80046d13fb98e3ad319dfe726f32f1010
docs(status): sync paused product trust handoff (#798)
```

Read-only drift commands from the `adze-swarm` checkout:

```text
git fetch public --prune
git rev-list --left-right --count public/main...origin/main
10    514

git diff --shortstat public/main..origin/main
382 files changed, 19863 insertions(+), 9811 deletions(-)

git diff --name-only public/main..origin/main | Measure-Object
382
```

Interpretation:

- public `adze/main` does not currently contain the selected
  `adze-swarm/main` proof state;
- the non-empty diff is a promotion blocker;
- the blocker must be resolved by an explicit public promotion PR if
  maintainers authorize promotion;
- the drift is not a reason to publish from `adze-swarm` or to move release
  secrets there.

## Claim Boundary

This snapshot does not prove or authorize:

- `cargo install adze-cli` from crates.io;
- crate publish readiness beyond prior non-publish checks;
- public promotion;
- release tag creation;
- signing workflow changes;
- Cargo-token usage;
- Tree-sitter parity, query parity, incremental performance, GLR generality, or
  benchmark performance claims.

The real `cargo install adze-cli` claim remains blocked until #325 authorizes a
public release/publish path and a post-publish crates.io install verifier passes.

## Next Work Item

The next release-candidate bundle item is `promotion-bundle-checklist`. It
should turn this snapshot shape into a reusable checklist for future selected
swarm commits.

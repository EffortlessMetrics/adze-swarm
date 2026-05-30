# Release Candidate Bundle

Status: active checklist
Owner: release/product
Updated: 2026-05-29
Linked proposal: ../proposals/ADZE-PROP-0017-release-candidate-bundle.md
Linked plan: ../../plans/release-candidate-bundle/implementation-plan.md
Linked active goal: ../../.adze/goals/active.toml
Release authorization tracker: https://github.com/EffortlessMetrics/adze-swarm/issues/325

This checklist defines the bundle that `adze-swarm` should produce before a
maintainer decides whether to promote a selected swarm state into public
`adze`.

It is a pre-promotion, non-publish checklist. It does not authorize public
promotion, release tags, crate publishing, signing, Cargo-token work, or real
crates.io install verification.

## Boundary Checklist

Before opening or reviewing a bundle PR, classify the work:

| Action | Where it belongs | Authorization |
| --- | --- | --- |
| collect proof receipts for a selected swarm state | `adze-swarm` | allowed by a non-release active goal |
| update support tiers, proof maps, and claim boundaries for proven facts | `adze-swarm` | allowed when backed by receipts |
| freeze a promotion bundle for maintainer review | `adze-swarm` | allowed when it remains non-publish |
| open a public promotion PR | public `adze` | requires explicit #325 direction |
| tag, publish, sign, or use Cargo tokens | public `adze` release path | requires explicit #325 direction |
| verify a real crates.io install after publish | public `adze` release path | requires explicit #325 direction |

Stop the swarm bundle if any answer below is "no":

- Is this PR confined to `EffortlessMetrics/adze-swarm`?
- Is the selected state an `adze-swarm/main` commit?
- Are release, publish, signing, Cargo-token, and real crates.io install steps
  still blocked on #325?
- Does every public-facing claim point to a support-tier row, proof map row, or
  named limitation?
- Is any `cargo install adze-cli` wording explicitly described as blocked until
  a post-publish crates.io receipt exists?
- Is rollback a normal revert of the swarm bundle, not a release incident path?

## Required Bundle Fields

Each candidate bundle must include:

- selected `adze-swarm/main` commit and title;
- local worktree status from the candidate checkout;
- live open PR queue for `EffortlessMetrics/adze-swarm`;
- live open PR queue for public `EffortlessMetrics/adze`;
- public drift state between `public/main` and `origin/main`;
- proof commands and their results;
- claim boundary and non-goals;
- rollback path;
- explicit blocked actions that remain under #325.

## Capture Commands

Run from `C:\Code\Rust2\adze-swarm` with `origin` pointing to
`EffortlessMetrics/adze-swarm` and `public` pointing to
`EffortlessMetrics/adze`.

```bash
git fetch origin --prune
git fetch public --prune
git status --short --branch
git rev-parse HEAD
git log -1 --oneline origin/main
git log -1 --oneline public/main
gh pr list --repo EffortlessMetrics/adze-swarm --state open --json number,title,url
gh pr list --repo EffortlessMetrics/adze --state open --json number,title,url
git rev-list --left-right --count public/main...origin/main
git diff --shortstat public/main..origin/main
```

Treat any non-empty public drift as a promotion blocker. Do not publish from
`adze-swarm` to bypass that drift.

## Source-Of-Truth Proof

Every bundle update should keep the source-of-truth checks green:

```bash
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

## Product And Release Preflight

When the bundle needs refreshed proof receipts, use non-publish commands:

```bash
just ci-supported
just ci-product-stable
just check-publishable
cargo run -q -p xtask -- verify-crates-io-install adze-cli --bin adze --version X.Y.Z --locked --dry-run
```

The install verifier dry run is command-shape evidence only. It is not a real
crates.io install receipt and must not support a `cargo install adze-cli`
public claim.

## Claim Boundary

A release candidate bundle must not claim:

- `cargo install adze-cli` works from crates.io;
- public promotion has happened;
- release tag or crate publish has been authorized;
- signing or Cargo-token work has happened;
- full Tree-sitter parity;
- full query parity;
- stable incremental reuse or performance;
- general GLR support beyond proven grammar classes;
- benchmark throughput or memory numbers as public claims.

Public claims remain governed by `docs/status/SUPPORT_TIERS.md` and
`docs/status/PRODUCT_PROOF_MAP.md`.

## Rollback

For an `adze-swarm` bundle PR, rollback is a normal revert of that PR.

For a future public promotion PR, rollback must be recorded in the promotion PR
itself and should usually be:

```text
revert the public promotion PR before any tag or publish command
```

If a release tag or publish has already happened, this checklist is no longer
the controlling artifact; follow the public release incident and crates.io
remediation process selected by maintainers.

## Handoff Decision

After reviewing a bundle, maintainers should choose one:

1. authorize public promotion or release work under #325;
2. defer release with named blockers;
3. split remaining work into another non-release `adze-swarm` lane.

No agent should infer release authorization from a green bundle alone.

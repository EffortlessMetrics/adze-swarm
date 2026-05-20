# Product Gap Burn-Down Handoff

Status: paused
Owner: runtime/product
Created: 2026-05-20
Active goal: ../../.adze/goals/active.toml
Plan: ./implementation-plan.md
Support-tier impact: no tier changes
Policy impact: no branch-protection, release, publish, signing, or Cargo-token change

## State

The residual product-trust lane has no ready routine swarm work.

Completed proof work includes:

- release-facing wording boundary sweep;
- focused external-scanner recovery proof;
- product objective audit refresh;
- public promotion blocker watch and public promotion merge;
- crates.io install-gap source-of-truth receipt;
- release/publish decision preflight receipt.

The remaining active-manifest items are blocked:

- `explicit-release-publish-workflow`;
- `crates-io-cli-install-receipt`.

## Why Paused

The remaining work requires an explicit human release/publish decision. Agents
must not infer that local package verification, publishability checks, dry-run
install command shape, public promotion, or green CI grants permission to tag,
publish crates, mutate signing or Cargo-token workflows, or claim
`cargo install adze-cli`.

## Latest Receipts

```bash
cargo info adze-cli
just package-local adze-cli
cargo run -q -p xtask -- verify-crates-io-install adze-cli --bin adze --version X.Y.Z --locked --dry-run
just check-publishable
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

Observed results:

- `cargo info adze-cli` reported that `adze-cli` is not present in crates.io.
- `just package-local adze-cli` passed for the local CLI package.
- The crates.io install verifier dry run printed the post-publish command
  shape and did not contact crates.io.
- `just check-publishable` passed for the release surface.
- Source-of-truth checks passed.

## Resume Conditions

Resume this lane only when one of these is true:

- a human explicitly authorizes release/publish execution;
- a new product-proof gap is discovered that does not require release/publish
  authorization;
- a support-tier, README, or release-facing claim changes and needs a fresh
  proof-boundary audit.

## Next Authorized Release Step

If release/publish is authorized, start from current public `adze/main`, refresh
both PR queues, verify public and swarm trees, rerun the release preflight, and
follow `docs/reference/PUBLISH_CHECKLIST.md` plus the release incident rules in
`plans/release-promotion/public-promotion-pr-plan.md`.

The post-publish install receipt remains:

```bash
cargo run -q -p xtask -- verify-crates-io-install adze-cli --bin adze --version X.Y.Z --locked
```

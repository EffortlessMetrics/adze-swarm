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
- focused and generated-matrix external-scanner recovery proof;
- product objective audit refresh;
- public promotion blocker watch and public promotion merge;
- crates.io install-gap source-of-truth receipt;
- release/publish decision preflight receipt.

The remaining active-manifest items are blocked:

- `explicit-release-publish-workflow`;
- `crates-io-cli-install-receipt`.

Tracker issue:
[`adze-swarm#325`](https://github.com/EffortlessMetrics/adze-swarm/issues/325).

## Why Paused

The remaining work requires an explicit human release/publish decision. Agents
must not infer that local package verification, publishability checks, dry-run
install command shape, public promotion, or green CI grants permission to tag,
publish crates, mutate signing or Cargo-token workflows, or claim
`cargo install adze-cli`.

## Latest Receipts

```bash
cargo info --registry crates-io adze-cli
just package-local adze-cli
cargo run -q -p xtask -- verify-crates-io-install adze-cli --bin adze --version X.Y.Z --locked --dry-run
just check-publishable
scripts/ci-product.sh --dry-run
cargo test --manifest-path example/Cargo.toml external_word_example::tests::generated_external_grammar_bad_input_matrix_returns_diagnostic_document --features pure-rust -- --exact --nocapture
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

Observed results:

- `cargo info --registry crates-io adze-cli` reported that `adze-cli` is not
  present in crates.io, including a 2026-05-20 refresh from current
  `adze-swarm/main` at commit `fc959ec1`. The explicit registry flag avoids
  resolving the local workspace package.
- `just package-local adze-cli` passed for the local CLI package.
- The crates.io install verifier dry run printed the post-publish command
  shape and did not contact crates.io. As of PRs #319-#320 on
  `adze-swarm/main` at commit `df4be63a`, the dry-run prints both
  `cargo info --registry crates-io adze-cli` and
  `cargo install --registry crates-io adze-cli --root <temp-root> --version X.Y.Z --locked`.
- The verifier dry run and `just check-publishable` were refreshed again from
  current `adze-swarm/main` at commit `fc959ec1`; `just check-publishable`
  passed for the release surface.
- `adze-swarm` PR #316 expanded the generated external-token
  diagnostic-document canary into a malformed-input matrix, and the focused
  matrix command passed locally and in the PR's `ci-product stable canaries`
  receipt.
- `adze-swarm` PR #343 added multibyte expression, invalid body, and
  newline-boundary body inputs to that generated external-token matrix and
  proves generated `parse()` errors agree with `parse_document()` diagnostics
  on spans and expected-token names.
- `adze-swarm` PR #345 registered the focused external-scanner commands in the
  advisory `scripts/ci-product.sh` lane, routed edits to that script through
  Product Proof, and passed `Rust Small Result`, `Source of Truth`,
  `ci-product stable canaries`, `Supported Rust Gate`, and the broad Pure Rust
  implementation tail.
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

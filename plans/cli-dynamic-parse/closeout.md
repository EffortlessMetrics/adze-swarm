# CLI Dynamic Parse Boundary Closeout

Status: complete
Owner: cli/product
Created: 2026-05-21
Closed: 2026-05-21
Linked proposal: ../../docs/proposals/ADZE-PROP-0016-cli-dynamic-parse-boundary.md
Linked plan: ./implementation-plan.md
Linked goal: ../../.adze/goals/cli-dynamic-parse-boundary.toml
Linked issue: EffortlessMetrics/adze-swarm#325
Linked PRs:
- EffortlessMetrics/adze-swarm#471
- EffortlessMetrics/adze-swarm#472
- EffortlessMetrics/adze-swarm#473

## Summary

The CLI dynamic parse boundary lane is complete.

PR #471 opened the non-release source-of-truth lane and made
`dynamic-cli-boundary-receipts` the one ready behavior item.

PR #472 implemented the behavior and claim-boundary receipts. `adze parse
--dynamic` now has explicit executable coverage for the no-feature gate and
the feature-enabled missing-library boundary, and dynamic helper tests cover
symbol-name normalization and bounded missing-grammar errors without requiring
a system grammar library.

## Behavior Now Covered

- Building `adze-cli` without the `dynamic` feature and invoking
  `adze parse --dynamic` reports that the `dynamic` feature is required.
- Building `adze-cli` with `dynamic` and pointing to a missing grammar reports
  a bounded missing-library error.
- Dynamic symbol lookup helpers preserve existing null terminators and append
  one when needed.
- The dynamic-loading guide is a design sketch, not a supported recipe.
- `cli/README.md` and `docs/status/SUPPORT_TIERS.md` keep dynamic parse output
  outside supported CLI output claims.
- `cargo install adze-cli` remains unclaimed until explicit public release and
  crates.io install receipts exist.

## Proof Receipts

Local proof from #472:

```bash
cargo test -p adze-cli test_parse_dynamic_without_feature_reports_feature_gate -- --exact --nocapture
cargo test -p adze-cli --features dynamic dynamic -- --nocapture
cargo test -p adze-cli cargo_install_adze_cli_claims_stay_release_surface_bounded -- --exact --nocapture
cargo test -p adze-cli test_parse_help_documents_available_modes -- --exact --nocapture
cargo test -p adze-cli test_parse_reports_available_modes -- --exact --nocapture
cargo fmt -p adze-cli -- --check
cargo clippy -p adze-cli --all-targets --features dynamic -- -D warnings
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
just ci-product-stable
```

GitHub proof from #472:

```text
Rust Small Result: success
Product Proof Result: success
Source of Truth: success
PR Plan: success
Test Runtime Crates: success
ci-product stable canaries: success
```

## Boundaries

This closeout does not authorize or perform release work.

Still blocked on explicit release authorization in #325:

```text
tag
cargo publish
signing
Cargo-token work
crates.io install receipt
public cargo install adze-cli claim
```

Public `adze` remains the release, public-intake, promotion, tag, publish,
signing, and Cargo-token surface. `adze-swarm` remains the implementation and
proof repo.

## Remaining Non-Release Gaps

No ready CLI dynamic parse boundary work remains in this lane.

Future dynamic parse implementation work must open a fresh active goal and
prove a real document-backed output path before changing support-tier claims.

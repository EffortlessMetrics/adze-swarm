# Product Gap Burn-Down Closeout

Status: complete
Owner: runtime/product
Closed: 2026-05-19
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/product-gap-burn-down.toml
Plan: ./implementation-plan.md

## Outcome

Outcome: **complete; public promotion remains conditional**.

This campaign burned down the named blockers from
`docs/status/PRODUCT_OBJECTIVE_AUDIT.md` without opening public `adze` work.
`adze-swarm` remains the operating repo. Public `EffortlessMetrics/adze`
remains release/public-intake until a fresh explicit promotion execution goal
opens.

## Landed Work

| Work item | Result |
| --- | --- |
| Product gap source of truth | Opened the campaign manifest and plan. |
| Stable product receipt refresh | Reconfirmed README Stable claim proof alignment. |
| Dangling-else selected tree gap | Fixed generated nearest-else selected typed AST and ambiguity-summary proof. |
| Generated reduce/reduce gap | Fixed generated conflict-cell preservation, selected typed AST extraction, and document ambiguity summaries. |
| Public promotion decision refresh | Kept the outcome at proceed conditionally, with no public PR opened by default. |

## Fresh Receipts

Representative receipts from current `adze-swarm/main` after the generated
reduce/reduce follow-up:

```bash
gh pr list --repo EffortlessMetrics/adze --state open
gh pr list --repo EffortlessMetrics/adze-swarm --state open
gh pr checks 267 --repo EffortlessMetrics/adze-swarm
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
cargo test -p adze-cli readme_stable_claims_are_in_stable_product_lane -- --exact --nocapture
just ci-product-stable
just check-publishable
git diff --check
```

Local `just ci-supported` was attempted from Windows and reached the tablegen
test link step, then failed with `LNK1318 Unexpected PDB error; LIMIT (12)`.
The hosted PR #267 `Supported Rust Gate` passed and is the supported-lane
receipt for this closeout.

## Remaining Non-Claims

- Public promotion has not happened.
- No release tag, crate publish, signing workflow, or Cargo-token workflow has
  changed.
- No crates.io `cargo install adze-cli` receipt exists yet.
- `ci-product-stable` remains advisory, not a branch-protection requirement.
- No new Stable claim is added for full GLR, full Tree-sitter, full query,
  stable CLI/WASM schema, raw GLR forest, or incremental performance.

## Next Step

If public promotion proceeds, create a fresh execution goal and use
`../release-promotion/public-promotion-pr-plan.md` from current
`adze-swarm/main`. Otherwise continue product work in `adze-swarm` only.

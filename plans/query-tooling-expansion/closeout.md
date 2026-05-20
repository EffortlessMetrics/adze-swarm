# Query and Tooling Expansion Closeout

Status: complete
Owner: runtime/tooling
Closed: 2026-05-20
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/query-tooling-expansion.toml
Plan: ./implementation-plan.md
Proposal: ../../docs/proposals/ADZE-PROP-0008-query-tooling-expansion.md

## Outcome

Outcome: **complete; no query parity promotion and no release authorization
implied**.

This campaign refreshed the documented query subset proof without changing the
repo split. Work stayed in `EffortlessMetrics/adze-swarm`; public
`EffortlessMetrics/adze` remains the release, public-intake, tag, signing, and
publish surface.

## Landed Work

| Work item | PRs | Result |
| --- | --- | --- |
| Source-of-truth setup | #371 | Opened the focused non-release query/tooling proposal, plan, active goal, and artifact registration. |
| Query example receipt | #372 | Added a compact `query_highlighting` receipt and example test covering highlight ranges, source-aware captures, byte-range filtering, clear-byte-range, and root-only matching. |
| Query gap matrix receipts | #373 | Expanded `query_differential` with wrong-field rejection, source-aware predicate rejection, `+`/`*` child quantifier tail fixtures, and source-aware literal-token matching. |
| Support-tier boundary refresh | final closeout PR | Refreshed support-tier and product-proof wording without promoting the query subset beyond Stabilizing. |

## Proof Receipts

Representative proof commands from the campaign:

```bash
cargo test -p adze --features query --example query_highlighting -- --nocapture
cargo run -p adze --features query --example query_highlighting
cargo test -p adze --features query --lib query -- --nocapture
cargo test -p adze --features query --lib query::matcher_v2 -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat,query" --test query_differential -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat,query" --test ts_compat_imported_shape_smoke imported_shape_smoke_covers_query_captures -- --exact --nocapture
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

GitHub receipts across the campaign included `Rust Small Result`, Source of
Truth, Docs Build, Product Proof, and path-routed runtime receipts where
relevant.

## Claim Boundaries

This closeout does not claim:

- full Tree-sitter query parity;
- directive-driven highlighting or injection semantics;
- query matching over every GLR forest alternative;
- broad imported grammar corpus parity;
- a stable CLI/WASM schema;
- release, tag, publish, signing, Cargo-token, or crates.io install work was
  authorized or performed;
- public `EffortlessMetrics/adze` is the swarm working repo.

## Next Step

No ready routine work remains in this campaign. Future non-release work should
open a fresh active goal in `adze-swarm`. Release/publish work remains blocked
until explicit human authorization and must execute from public
`EffortlessMetrics/adze`.

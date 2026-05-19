# Toolkit Excellence And Adoption Closeout

Status: complete
Owner: runtime/product
Closed: 2026-05-19
Active goal: ../../.adze/goals/active.toml
Plan: ./implementation-plan.md
Proposal: ../../docs/proposals/ADZE-PROP-0004-toolkit-excellence.md
Support tiers: ../../docs/status/SUPPORT_TIERS.md

## Summary

The Toolkit Excellence and Adoption campaign converted the completed GLR toolkit
foundation into product-shaped proof. The campaign focused on first-use
workflow, downstream integration, API choice, runnable examples, compatibility
documentation, performance receipts, and support-tier promotion.

## Landed Work

| Work item | PRs | Result |
| --- | --- | --- |
| Campaign source of truth | #217 | Opened proposal, active goal, plan, and ledger entries. |
| Product acceptance matrix | #218 | Defined product workflows, proof commands, and claim boundaries. |
| Starter project hardening | #219 | Hardened `adze init` generated starter behavior. |
| Downstream starter fixture | #222 | Added user-shaped downstream crate proof. |
| Beginner docs alignment | #223, #224 | Aligned README/book/quickstart with the starter path. |
| API choice guide | #225 | Added user-facing API decision guide. |
| GLR ambiguity example | #226 | Added runnable ambiguity summary example. |
| Query highlighting example | #227 | Added runnable query subset example. |
| Diagnostics/recovery example | #228 | Added runnable diagnostics and recovery example. |
| Tree-sitter compatibility matrix | #229 | Published selected-tree compatibility boundaries. |
| Imported-shape smoke | #230 | Added alias/field/hidden/error/missing/external/query smoke. |
| Benchmark product receipts | #231 | Added advisory product-smoke performance receipt command. |
| Proven slice promotion | #232 | Promoted proven slices to Stabilizing without Stable overclaims. |

## Support-Tier Outcome

The campaign promoted these slices to **Stabilizing**:

- `AdzeDocument` generated `parse_document()` tooling path;
- Tree-sitter-compatible selected-tree subset;
- documented query compatibility subset;
- CLI starter and document-projection smoke behavior.

Benchmarks remain **Advisory** with the `product-smoke` receipt. No new Stable
claims were added.

## Proof Receipts

Representative proof commands used across the closeout:

```bash
cargo test --manifest-path testing/downstream-starter/Cargo.toml
cargo run --manifest-path testing/downstream-starter/Cargo.toml --example parse
cargo test -p adze --features "pure-rust,glr,ts-compat" --test ts_compat_selected_tree -- --nocapture
cargo run -p adze --features query --example query_highlighting
cargo test -p adze --features query --lib query -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat,query" --test query_differential -- --nocapture
cargo run -q -p xtask -- perf-receipt --profile product-smoke
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

GitHub receipt: #232 passed `Rust Small Result`, source-of-truth checks, docs
gate, PR gate, and `ci-product stable canaries`.

## Non-Claims

This closeout does not claim:

- full Tree-sitter API parity;
- full Tree-sitter query parity;
- stable CLI/WASM schema compatibility;
- stable native document API;
- stable throughput or memory performance;
- release-blocking benchmark thresholds;
- branch-protection changes beyond the existing `Rust Small Result` model.

## Next Work

The next campaign should be opened explicitly instead of extending this one.
Likely candidates:

- public release promotion from `adze-swarm` to public `adze`;
- remaining Tree-sitter/query parity gaps;
- CLI/WASM schema stabilization;
- performance baseline collection on stable runner classes;
- incremental parsing beyond honest full-reparse fallback.

# Adze Adoption Hardening Closeout

Status: complete
Owner: runtime/product
Closed: 2026-05-30
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/adze-adoption-hardening.toml
Plan: ./implementation-plan.md
Proposal: ../../docs/proposals/ADZE-PROP-0006-user-experience-hardening.md
Release authorization tracker: https://github.com/EffortlessMetrics/adze-swarm/issues/325

## Outcome

Outcome: **complete; no release or publish authorization implied**.

This campaign made the already-proven GLR toolkit easier to adopt, inspect, and
promote without changing the repo split. Work stayed in
`EffortlessMetrics/adze-swarm`; public `EffortlessMetrics/adze` remains the
release, public-intake, promotion, tag, publish, signing, Cargo-token, and
crates.io receipt surface.

## Current State

Current `adze-swarm/main` at closeout setup:

```text
374847430ef4cc5a5b330790267361a8f5c7afeb
docs(release): clarify swarm promotion boundary (#574)
```

Open PR queues at closeout setup:

```text
EffortlessMetrics/adze-swarm: []
EffortlessMetrics/adze: []
```

The post-closeout active manifest returns to the paused
`adze-swarm-forge-standby` state. Release/publish authorization remains blocked
on #325, and selection of the next non-release lane remains tracked by #549.

## Landed Work

| Work item | PRs | Result |
| --- | --- | --- |
| Source-of-truth setup | #562 | Opened the Adze Adoption Hardening active goal, plan, and artifact registrations. |
| Starter downstream fixture | #563 | Mirrored the generated starter layout in `testing/downstream-starter` and proved build/run behavior from a checkout. |
| API choice guide | #564 | Tied API choice guidance to starter proof while keeping `grammar::parse()` as the beginner path and `parse_document()` / `AdzeDocument` as the tooling path. |
| GLR ambiguity walkthrough | #567 | Clarified supported ambiguity walkthrough behavior and kept selected-tree/native ambiguity boundaries explicit. |
| Diagnostics/recovery walkthrough | #568 | Clarified recovery guidance, diagnostics, and selected-tree error facts without broadening recovery claims. |
| Query cookbook | #569, #570 | Clarified the query compatibility subset, runnable highlighting receipt, and source-of-truth closeout metadata. |
| Self-hosted runner assumption fix | #571 | Ran the CX33 Rust Small lane natively instead of assuming a local `em-ci-rust:1.95` Docker image, without restoring default hosted fallback. |
| Tree-sitter selected-tree guide | #572 | Clarified document-backed `Tree::from_document` guidance and added the selected-tree matrix canary. |
| Benchmark receipt guide | #573 | Clarified `product-smoke` as an advisory receipt index and added benchmark inventory proof rows. |
| Public release boundary checklist | #574 | Made the swarm/public release boundary explicit in the release-candidate bundle checklist and developer guide. |
| Closeout and standby restoration | #575 | Archived the completed lane, registered this closeout, and restored the paused forge standby active manifest. |

## Proof Receipts

Representative local proof commands from the campaign:

```bash
cargo test -p adze-cli test_init -- --nocapture
cargo test --manifest-path testing/downstream-starter/Cargo.toml
cargo run --manifest-path testing/downstream-starter/Cargo.toml --example parse -- "1 + 2 * 3"
cargo run -p adze --features "pure-rust,glr" --example glr_ambiguity
cargo run -p adze --features "pure-rust,glr,serialization" --example diagnostics_recovery
cargo run -p adze --features query --example query_highlighting
cargo test -p adze --features query --lib query -- --nocapture
cargo test -p adze --features "pure-rust,glr,ts-compat" --test ts_compat_selected_tree -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat,query" --test ts_compat_imported_shape_smoke -- --nocapture
cargo run -q -p xtask -- perf-receipt --profile product-smoke
cargo test -p adze-benchmarks --test verify_fixture_parsing verify_product_smoke_perf_receipt_is_documented -- --exact --nocapture
cargo test -p adze-benchmarks --test verify_fixture_parsing verify_benchmark_inventory_is_exhaustive -- --exact --nocapture
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

GitHub receipts across the campaign included `Rust Small Result`,
`Product Proof Result`, `Source of Truth`, `CI Lane Whitelist`, and
`GLR Invariants` on the closing PRs. The routed Rust lane stayed self-hosted;
GitHub-hosted Rust Small jobs stayed skipped.

## Claim Boundaries

This closeout does not claim or authorize:

- public `adze` promotion;
- release tags;
- crate publishing;
- signing workflow changes;
- Cargo-token work;
- real crates.io install verification;
- a public `cargo install adze-cli` claim;
- full Tree-sitter parity;
- full query parity;
- stable incremental reuse or performance;
- general GLR support beyond proven grammar classes;
- benchmark throughput or memory numbers as public claims.

`AdzeDocument` remains the canonical parse product. Typed AST, typed CST,
diagnostics, ambiguity summaries, Tree-sitter-compatible selected-tree output,
query surfaces, JSON, CLI, WASM, and benchmark receipts remain projections,
proof rows, or advisory surfaces over that parse truth according to their
support-tier boundaries.

## Next Step

No routine adoption-hardening work remains in this campaign. The repo should
return to paused forge standby until maintainers select a new non-release lane
under #549 or explicitly authorize public promotion/release work under #325.

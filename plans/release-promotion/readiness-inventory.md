# Release Promotion Readiness Inventory

Status: active
Owner: release/product
Created: 2026-05-19
Linked proposal: ../../docs/proposals/ADZE-PROP-0005-release-promotion-readiness.md
Linked plan: ./implementation-plan.md
Support-tier map: ../../docs/status/SUPPORT_TIERS.md
Product proof map: ../../docs/status/PRODUCT_PROOF_MAP.md

## Purpose

This inventory records what `adze-swarm` has already proven before any public
promotion decision. It is not a public promotion PR, release tag, publish
instruction, or support-tier promotion by itself.

The operating rule remains:

```text
adze-swarm = swarm working repo
public adze = release/public-intake surface
```

## Completed Campaigns

| Campaign | Closeout / plan | Release relevance |
| --- | --- | --- |
| 0.9 contract convergence | `../0.9.0/closeout.md` | Defines the source-of-truth stack, package surface convergence, CI economics, MSRV/lint posture, product-proof map, support-tier cleanup, and release-operation proof commands. |
| GLR toolkit productization | `../glr-toolkit/productization-plan.md` | Records the parser toolkit foundation: product contract, starter path, fixture taxonomy, projection equivalence, GLR conflict matrix, ABI roundtrip, GOTO proof, ts-compat parity, query subset, recovery matrix, CLI diagnostic projection, examples, migration guide, incremental fallback metadata, benchmark fixtures, and support-tier promotion pass. |
| Toolkit excellence and adoption | `../toolkit-excellence/closeout.md` | Converts the foundation into product-shaped evidence: downstream starter fixture, API choice guide, runnable GLR/query/diagnostics examples, selected-tree compatibility matrix, imported-shape smoke, product-smoke performance receipts, and proven-slice support-tier promotion. |
| Release promotion readiness | `./implementation-plan.md` | Active campaign. Inventories completed work, audits public drift, freezes release-facing claims, defines public promotion scope and rollback, then records the decision. |

## Release-Facing Docs

The following docs are release-relevant because they affect user expectations,
product claims, or promotion decisions:

| Surface | Source |
| --- | --- |
| README claims | `../../README.md` |
| Documentation map | `../../docs/README.md` |
| Source-of-truth system | `../../docs/reference/SPEC_SYSTEM.md` |
| Product acceptance matrix | `../../docs/product/ACCEPTANCE_MATRIX.md` |
| Beginner path | `../../docs/tutorials/quickstart-10-minutes.md` |
| Mental model | `../../docs/explanations/mental-model.md` |
| API choice guide | `../../docs/reference/which-api-should-i-use.md` |
| Tree-sitter compatibility | `../../docs/reference/tree-sitter-compatibility.md` |
| Query compatibility | `../../docs/reference/query-compatibility.md` |
| Performance baselines and receipts | `../../docs/perf/baselines.md` |
| Support tiers | `../../docs/status/SUPPORT_TIERS.md` |
| Product proof map | `../../docs/status/PRODUCT_PROOF_MAP.md` |
| Known red / intentional exclusions | `../../docs/status/KNOWN_RED.md` |
| Current operating status | `../../docs/status/NOW_NEXT_LATER.md` |

## Support-Tier Inventory

### Stable Release Claims

Only these surfaces are currently Stable:

| Surface | Release-facing meaning | Representative proof |
| --- | --- | --- |
| Typed extraction | Generated parser can return typed Rust values. | `just ci-supported`; `cargo test -p adze-cli readme_arithmetic_quickstart_builds_and_runs -- --exact --nocapture` |
| Pure-Rust parser | Core generated parser path is supported. | `just ci-supported`; downstream starter fixture commands |
| Operator precedence | Proven expression grammar shapes are supported. | README arithmetic canary and precedence canary |
| Core table serialization | Core parse-table serialization is stable. | `cargo test -p adze-glr-core --features serialization --doc` |

No public README claim should be broader than these rows unless
`SUPPORT_TIERS.md` is updated with proof and limitations in the same PR.

### Stabilizing Product Slices

These are implemented and product-shaped, but not Stable:

| Surface | Current scope |
| --- | --- |
| GLR conflict routing | Stabilizing for proven conflict classes and deterministic selected-tree behavior; full forest stability remains future work. |
| Structured parse errors | Stabilizing for the generated parser matrix and documented diagnostics/recovery examples. |
| `AdzeDocument` native API | Stabilizing for generated `parse_document()` tooling path and document-backed projections. |
| Tree-sitter compatibility API | Stabilizing for the documented selected-tree subset. |
| Query compatibility subset | Stabilizing for the documented subset and examples. |
| CLI | Stabilizing for starter-project and document-projection smoke behavior. |

### Experimental / Advisory / Future

These surfaces must stay visibly bounded during promotion:

| Surface | Current boundary |
| --- | --- |
| Typed CST native view | Experimental; generated wrapper proof exists, but no broad parity or stable visitor/rewriter contract. |
| External scanners | Experimental; not in required gate. |
| Incremental parsing | Experimental; full-reparse fallback metadata is honest, but stable reuse and speedup claims are not supported. |
| Tree-sitter interop bridge | Advisory smoke surface. |
| WASM | Advisory compile-check surface. |
| Runtime2 | Intentionally excluded proving ground. |
| Grammars | Advisory examples/integration surfaces. |
| Golden tests | Advisory parity signal. |
| Benchmarks | Advisory receipts; no stable throughput or regression-threshold claim. |

## Repeatable Proof Commands

Fresh promotion planning should choose from these existing receipts instead of
scraping PR conversations:

```bash
just ci-supported
just ci-product-stable
cargo test --manifest-path testing/downstream-starter/Cargo.toml
cargo run --manifest-path testing/downstream-starter/Cargo.toml --example parse
cargo test -p adze --features "pure-rust,glr,ts-compat" --test ts_compat_selected_tree -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat,query" --test ts_compat_imported_shape_smoke -- --nocapture
cargo test -p adze --features query --lib query -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat,query" --test query_differential -- --nocapture
cargo run -p adze --features query --example query_highlighting
cargo run -q -p xtask -- perf-receipt --profile product-smoke
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

Release/publish planning may also need the release-surface receipts recorded in
`../0.9.0/closeout.md`, including `just check-publishable` and local package
checks. Those are promotion-planning inputs, not ordinary swarm PR gates.

## Deferred Or Swarm-Only Surfaces

These should not be promoted into public release wording without a separate
plan:

- stable CLI/WASM schema compatibility;
- full Tree-sitter API parity;
- full Tree-sitter query parity;
- stable raw GLR forest export;
- stable incremental reuse or performance guarantees;
- benchmark regression thresholds as branch-protection gates;
- release/publish/signing workflow changes.

## Next Audit

The next work item is `public-drift-audit`. It should compare live public
`EffortlessMetrics/adze` and `EffortlessMetrics/adze-swarm` state before any
promotion PR is prepared, including open PR queues and public-only commits.

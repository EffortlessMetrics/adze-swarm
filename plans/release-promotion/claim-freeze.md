# Release Claim Freeze

Status: active
Owner: release/product
Created: 2026-05-19
Linked proposal: ../../docs/proposals/ADZE-PROP-0005-release-promotion-readiness.md
Linked plan: ./implementation-plan.md
Support-tier map: ../../docs/status/SUPPORT_TIERS.md
Product proof map: ../../docs/status/PRODUCT_PROOF_MAP.md
Drift audit: ./public-drift-audit.md

## Purpose

This freeze records the release-facing claim boundary for a future public
promotion decision. It aligns README capability tiers with
`SUPPORT_TIERS.md`, but it does not tag, publish, or claim public release
readiness.

## Stable Claims

These are the only README surfaces currently labeled Stable:

| Surface | Boundary |
| --- | --- |
| Typed extraction | Stable for supported generated grammars and typed Rust values. |
| Pure-Rust parser | Stable for generated parser use from clean downstream crates. |
| Operator precedence | Stable for proven expression grammar shapes. |
| Serialization (core tables) | Stable for core parse-table serialization, not document JSON. |

The guard for this boundary is:

```bash
cargo test -p adze-cli readme_stable_claims_are_in_stable_product_lane -- --exact --nocapture
```

## Stabilizing Claims

These release-facing surfaces are implemented and proof-backed, but remain
below Stable:

| Surface | Boundary |
| --- | --- |
| GLR conflict routing | Proven conflict classes and deterministic selected-tree behavior; no stable full-forest export. |
| Tablegen `TSLanguage` ABI | Proven compressed metadata/action/field/alias decode slices; broader generated-language parity remains future work. |
| Structured parse errors | Proven generated-parser diagnostics matrix; wording and broader invalid-span coverage are still maturing. |
| `AdzeDocument` native API | Generated `parse_document()` tooling path and document-backed projections; not a stable public API yet. |
| Tree-sitter compatibility API | Documented selected-tree subset only; no full API, corpus, or query parity claim. |
| Query compatibility subset | Documented subset with examples; no directives, full parity, or GLR-forest-wide matching claim. |
| CLI | Starter-project and document-projection smoke behavior; no stable CLI/WASM schema claim. |

## Experimental / Advisory Boundaries

These stay visibly bounded in release wording:

- typed CST native view remains Experimental;
- external scanners remain Experimental;
- incremental parsing remains Experimental with honest full-reparse fallback
  metadata, not stable reuse or speed claims;
- Tree-sitter bridge/interoperability remains Advisory;
- WASM remains Advisory compile-check evidence;
- runtime2 remains intentionally excluded from the primary public contract;
- grammars, golden tests, and benchmarks remain Advisory;
- benchmark receipts are not throughput claims or release-blocking thresholds.

## README Alignment

The README capability table is part of the release-facing surface. This freeze
aligns these rows with `SUPPORT_TIERS.md` and `PRODUCT_PROOF_MAP.md`:

- `AdzeDocument native API`: Experimental -> Stabilizing;
- `Tree-sitter compatibility API`: Advisory -> Stabilizing;
- `Query compatibility subset`: added as Stabilizing;
- `CLI`: Advisory -> Stabilizing;
- `Benchmarks`: keeps Advisory and points to the product-smoke receipt.

## Proof Commands

```bash
cargo test -p adze-cli readme_stable_claims_are_in_stable_product_lane -- --exact --nocapture
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
cargo run -q -p xtask -- check-active-goal --mode blocking
git diff --check
```

## Non-Claims

This freeze does not claim:

- public promotion is ready;
- any new Stable surface;
- full Tree-sitter compatibility;
- full Tree-sitter query compatibility;
- stable document JSON, CLI JSON, or WASM schema compatibility;
- stable incremental parsing reuse or performance;
- stable benchmark throughput or regression thresholds.

## Next Step

Prepare the public promotion PR plan with scope, proof commands, excluded
surfaces, and rollback. If the plan defers promotion, record that explicitly in
the closeout.

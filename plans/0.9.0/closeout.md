# 0.9.0 Contract Convergence Closeout

Status: ready for release operation
Updated: 2026-05-16

This closeout records the 0.9.0 contract-convergence evidence now present in
`adze-swarm`. It is not a tag or publish instruction by itself. Release tagging,
publishing, signing, and public-repo promotion remain explicit release-surface
operations.

## What Shipped

- Source-of-truth stack for proposals, specs, ADRs, implementation plans,
  active goals, support tiers, policy ledgers, and proof receipts.
- Microcrate-to-SRP package surface convergence, with no tracked durable
  unpublished production-crate category.
- Rust 1.95/MSRV and lint/readiness receipts for the 0.9 API-foundation lane.
- CI economics cuts for `adze-swarm`, including scoped main-push behavior,
  path-gated Pure Rust lanes, coverage-lite/full split, tighter microcrate
  routing, guarded label wakeups, runner-class docs, and benchmark compile
  smoke routing.
- Product-proof map and support-tier cleanup that keeps Stable README claims
  bounded to proof-backed surfaces.
- `AdzeDocument` product-proof coverage for document boundaries, typed AST/CST
  projections, diagnostics, GLR ambiguity summaries, Tree-sitter compatibility
  adapters, and experimental document JSON.
- Advisory CLI document projection output via `adze parse --output
  document-json/tree-json/diagnostics-json/ambiguity-json`.
- Publishability receipts for tracked release crates, using local release
  packaging for co-release siblings that cannot resolve against older crates.io
  versions.

## Proof Commands

Core release gates:

```bash
just ci-supported
just ci-product-stable
just fmt
just clippy
```

Product-proof receipts:

```bash
cargo test -p adze --features pure-rust --test document_parse_agreement -- --nocapture
cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test document_parse_agreement -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat" --test adze_document_alpha -- --nocapture
cargo test -p adze --features pure-rust --test typed_cst_generated_document -- --nocapture
cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors -- --nocapture
cargo test -p adze --features "pure-rust,glr" --test error_display_tests -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_to_sexp -- --nocapture
cargo test -p adze --features "pure-rust,serialization" --test adze_document_json -- --nocapture
cargo test -p adze --features "pure-rust,serialization,glr" --test adze_document_json parse_document_json_serializes_glr_ambiguity_summary -- --exact --nocapture
cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test test_e2e_ambiguous_grammar_glr generated_ambiguous_expr_glr_runtime_retains_three_or_more_complete_alternatives -- --exact --nocapture
cargo test -p adze-cli test_parse_document_projection_modes_emit_schema_envelopes -- --exact --nocapture
```

Release-quality receipts:

```bash
cargo test -p adze-cli readme_stable_claims_are_in_stable_product_lane -- --exact --nocapture
cargo test -p adze-benchmarks --test verify_fixture_parsing -- --nocapture
cargo bench -p adze-benchmarks --no-run
just check-publishable
scripts/package-local-release.sh adze
scripts/package-local-release.sh adze-tool
cargo package -p adze-macro --allow-dirty
cargo package -p adze-cli --allow-dirty
cargo package -p adze-ir --allow-dirty
cargo package -p adze-glr-core --allow-dirty
cargo package -p adze-tablegen --allow-dirty
cargo package -p adze-common --allow-dirty
cargo package -p adze-common-type-ops-core --allow-dirty
```

Source-of-truth receipts:

```bash
cargo run -q -p xtask -- check-doc-artifacts
cargo run -q -p xtask -- check-active-goal
git diff --check
```

## Support-Tier Changes

- Stable README claims are limited to the stable product lane recorded in
  `../../docs/status/SUPPORT_TIERS.md`.
- `AdzeDocument`, typed CST, document JSON, CLI document output, GLR ambiguity
  summary, Tree-sitter compatibility, WASM, and benchmarks remain proof-backed
  but tiered honestly as Experimental or Advisory where applicable.
- CLI document JSON is implemented as an advisory smoke surface, not as a stable
  CLI/WASM schema contract.

## Policy Changes

- CI lane and risk-pack policy now reflect scoped `adze-swarm` economics.
- Package publishability uses `scripts/package-local-release.sh` for co-release
  siblings that must compile against the local release surface.
- Document artifact and active-goal checks keep the source-of-truth stack
  machine-readable for agents.

## Known Gaps

- Tagging, publishing, signing, and Cargo-token workflows are not swarm tasks.
- Public `adze` promotion/sync must be deliberate and should not be inferred
  from `adze-swarm` merge state.
- Full Tree-sitter parity, stable CLI/WASM schema guarantees, raw GLR forest
  export, and per-AST-node provenance remain future work.
- `adze` and `adze-tool` package verification depends on local co-release
  patching until the matching sibling crates are published.

## Final Release Operation

Before tagging 0.9.0, perform a fresh release-surface proof run from the repo
that owns publishing:

```bash
git status --short
just ci-supported
just ci-product-stable
just check-publishable
cargo run -q -p xtask -- check-doc-artifacts
git diff --check
```

Then update changelog/release notes, tag, publish, and archive this campaign's
active goal manifest.

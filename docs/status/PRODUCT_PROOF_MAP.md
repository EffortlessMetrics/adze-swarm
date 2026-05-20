# Product Proof Map

**Status:** release-readable companion to `SUPPORT_TIERS.md`
**Source of truth:** `SUPPORT_TIERS.md`
**Last updated:** 2026-05-19

This map answers one question quickly: which product claims have proof, and
where is that proof owned?

`SUPPORT_TIERS.md` remains the authoritative support-tier ledger. This file
summarizes the current release-facing claims so users, maintainers, and agents
do not have to mine the dense tier table for the common decision points.
For the stricter objective-level completion audit, see
[`PRODUCT_OBJECTIVE_AUDIT.md`](./PRODUCT_OBJECTIVE_AUDIT.md).

## Claim Map

| Product claim | Tier | Source of truth | Representative proof | Release note |
|---|---|---|---|---|
| Typed Rust extraction works for supported generated grammars. | Stable | `SUPPORT_TIERS.md`; README capability table | `just ci-supported`; `cargo test -p adze --features pure-rust --test typed_ast_contract typed_ast_contract_left_associative_addition -- --exact --nocapture`; `cargo test -p adze --features pure-rust --test typed_ast_contract typed_ast_contract_repeated_parse_is_deterministic -- --exact --nocapture` | Stable 0.8+ user contract. |
| Pure-Rust generated parsers can be used from clean downstream crates. | Stable | `SUPPORT_TIERS.md`; README quickstart; getting-started tutorial | `just ci-product-stable`; `cargo test -p adze-cli readme_arithmetic_quickstart_builds_and_runs -- --exact --nocapture`; `cargo test -p downstream-demo -- --nocapture`; `cargo run -p downstream-demo --quiet`; `cargo test --manifest-path testing/downstream-starter/Cargo.toml`; `cargo run --manifest-path testing/downstream-starter/Cargo.toml --example parse` | Stable claim with downstream clean-room proof. |
| Operator precedence works for the documented arithmetic shape. | Stable | `SUPPORT_TIERS.md`; README example | `cargo test -p adze-cli readme_arithmetic_quickstart_builds_and_runs -- --exact --nocapture`; `cargo test -p adze-glr-core --test ambiguity_detection_comprehensive test_precedence_resolves_add_mul -- --exact --nocapture` | Stable for proven expression grammar shapes, not every ambiguous grammar. |
| Core parse-table serialization roundtrips. | Stable | `SUPPORT_TIERS.md` | `cargo test -p adze-glr-core --features serialization --doc`; `cargo test -p adze-glr-core --features serialization --test serialization_v9 sv9_complex_precedence_roundtrip -- --exact --nocapture` | Stable for core table serialization. Document JSON remains experimental. |
| GLR conflict routing preserves and selects proven ambiguity cases. | Stabilizing | `SUPPORT_TIERS.md`; `ADZE-SPEC-0007`; `ADZE-ADR-0003` | `cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test test_e2e_ambiguous_grammar_glr generated_ambiguous_expr_glr_runtime_retains_multiple_complete_alternatives -- --exact --nocapture`; `cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test test_e2e_ambiguous_grammar_glr generated_ambiguous_expr_parse_document_reports_ambiguity_summary -- --exact --nocapture` | Stabilizing; full forest export and broader ambiguity policy remain future work. |
| Generated parse errors expose structured spans, expected tokens, and no-panic bad-input behavior. | Stabilizing | `SUPPORT_TIERS.md`; `ADZE-SPEC-0005` | `cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors generated_typed_parser_unexpected_eof_expected_field_is_populated -- --exact --nocapture`; `cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors generated_typed_parser_bad_inputs_return_errors_without_panicking -- --exact --nocapture`; `cargo test -p adze --features "pure-rust,glr,serialization,ts-compat" --test recovery_matrix generated_object_like_bad_input_matrix_preserves_document_diagnostics_and_json -- --exact --nocapture` | Stabilizing; object-like document/JSON recovery proof now covers separator, UTF-8, multiline value, and multiline EOF cases, while external-scanner recovery remains future work. |
| External scanners dispatch parser-v4 tokens with preserved byte spans and text in focused canaries. | Experimental | `SUPPORT_TIERS.md`; `ADZE-SPEC-0005` | `cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests::test_parser_with_external_scanner -- --exact --nocapture`; `cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests::test_external_scanner_rejects_token_not_in_valid_symbols -- --exact --nocapture`; `cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests::test_external_scanner_parse_document_bad_input_returns_diagnostic_document -- --exact --nocapture`; `cargo test --manifest-path example/Cargo.toml external_word_example::tests::generated_external_grammar_bad_input_returns_diagnostic_document --features pure-rust -- --exact --nocapture`; `cargo test -p adze --features external_scanners` | Experimental; focused dispatch/span proof, direct parser-v4 diagnostic-document proof, and generated external-token grammar diagnostic-document proof. Full parser-generated external-scanner recovery and stable public scanner API claims remain future work. |
| `AdzeDocument` is the native parse-product boundary for document-oriented tooling. | Stabilizing | `ADZE-SPEC-0003`; `ADZE-ADR-0001`; `SUPPORT_TIERS.md` | `cargo test -p adze --features "pure-rust,ts-compat" --test adze_document_alpha -- --nocapture`; `cargo test -p adze --features pure-rust --test typed_ast_contract typed_ast_contract_parse_document_ast_matches_parse -- --exact --nocapture`; `cargo test -p adze --features "pure-rust,serialization" --test adze_document_json -- --nocapture`; `cargo test --manifest-path testing/downstream-starter/Cargo.toml` | Stabilizing generated `parse_document()` tooling boundary; not a stable public API yet. |
| Incremental document lifecycle records full-reparse fallback metadata. | Experimental | `ADZE-SPEC-0009`; `SUPPORT_TIERS.md` | `cargo test -p adze --features incremental_glr reparse_fallback_metadata -- --nocapture` | Honest fallback metadata only; no stable incremental reuse, changed-range, or performance claim. |
| Typed CST wrappers are generated views over document node IDs and edge fields. | Experimental | `ADZE-SPEC-0004`; `ADZE-ADR-0001`; `SUPPORT_TIERS.md` | `cargo test -p adze --features pure-rust --test typed_cst_generated_document -- --nocapture`; `cargo test -p adze-tablegen typed_cst_generator -- --nocapture` | Experimental Rust-native syntax layer; no visitor/rewriter or broad parity matrix yet. |
| Tree-sitter compatibility projects from native document/schema data. | Stabilizing | `ADZE-SPEC-0006`; `docs/reference/tree-sitter-compatibility.md`; `SUPPORT_TIERS.md` | `cargo test -p adze --features "pure-rust,glr,ts-compat" --test ts_compat_selected_tree -- --nocapture`; `cargo test -p adze --features "pure-rust,ts-compat,query" --test ts_compat_imported_shape_smoke -- --nocapture`; `cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_node_types -- --nocapture` | Stabilizing selected-tree subset; not full Tree-sitter API, grammar-corpus, or query parity. |
| Query compatibility exposes a documented Tree-sitter-like subset. | Stabilizing | `ADZE-SPEC-0013`; `docs/reference/query-compatibility.md`; `SUPPORT_TIERS.md` | `cargo run -p adze --features query --example query_highlighting`; `cargo test -p adze --features query --lib query -- --nocapture`; `cargo test -p adze --features "pure-rust,ts-compat,query" --test query_differential -- --nocapture` | Stabilizing for the documented subset; directives, full parity, and GLR-forest-wide matching remain future work. |
| Native document JSON uses an experimental schema-tagged envelope. | Experimental | `ADZE-SPEC-0008`; `ADZE-ADR-0004`; `SUPPORT_TIERS.md` | `cargo test -p adze --features "pure-rust,serialization" --test adze_document_json -- --nocapture`; `cargo test -p adze --features "pure-rust,serialization,glr" --test adze_document_json parse_document_json_serializes_glr_ambiguity_summary -- --exact --nocapture` | Experimental `adze.document.v1`; not yet a stable CLI/WASM schema contract. |
| CLI project scaffolding and document-output smoke checks are product-shaped tools. | Stabilizing | `SUPPORT_TIERS.md`; `cli/README.md` | `cargo test -p adze-cli test_init_default_cwd_generates_buildable_project -- --exact --nocapture`; `cargo test -p adze-cli test_init_generates_buildable_project -- --exact --nocapture`; `cargo test -p adze-cli getting_started_quickstart_builds_parses_and_reports_diagnostics -- --exact --nocapture`; `cargo test -p adze-cli test_parse_document_projection_modes_emit_schema_envelopes -- --exact --nocapture`; `cargo test -p adze-cli parse_document_json_modes_emit_recovery_diagnostics -- --exact --nocapture`; `cargo test -p adze-cli cargo_install_adze_cli_claims_stay_release_surface_bounded -- --exact --nocapture`; `just package-local adze-cli` | Stabilizing for starter, document-projection smoke behavior, local CLI package verification, and the claim boundary that `cargo install adze-cli` remains unclaimed until a crates.io receipt exists; still not a stable CLI/WASM schema contract or crates.io install claim. |
| WASM compiles for the demo target. | Advisory | `SUPPORT_TIERS.md`; `KNOWN_RED.md` | `cargo check --manifest-path wasm-demo/Cargo.toml --target wasm32-unknown-unknown` | Compile signal only; browser/runtime behavior is not certified. |
| Benchmarks use documented parser/projection fixture families and compile on explicit benchmark lanes. | Advisory | `SUPPORT_TIERS.md`; `benchmarks/README.md`; `docs/perf/baselines.md` | `cargo run -q -p xtask -- perf-receipt --profile product-smoke`; `cargo test -p adze-benchmarks --test verify_fixture_parsing verify_parse_bench_uses_real_parser_workload -- --exact --nocapture`; `cargo test -p adze-benchmarks --test verify_fixture_parsing verify_benchmark_fixture_families_are_documented -- --exact --nocapture`; `cargo bench -p adze-benchmarks --bench document_projection --no-run` | Benchmark evidence is manual/scheduled and advisory; compile-only projection benchmark proof is not a throughput or regression-threshold claim. |

## Promotion Rules

- A README Stable claim must have a matching `SUPPORT_TIERS.md` row and a
  repeatable proof command.
- `ci-product stable canaries` is the bounded Stable-claim canary lane. It runs
  on stable-claim PR surfaces, schedule, and stable-only manual dispatch, but
  remains advisory until branch protection explicitly promotes it. Manual
  dispatch runs the broad advisory product lane only when `lane=all` is
  selected.
- Latest hosted receipt: GitHub workflow dispatch
  [`Product Proof` run 26104726428](https://github.com/EffortlessMetrics/adze-swarm/actions/runs/26104726428)
  passed on 2026-05-19 from current `adze-swarm/main` after PR #281. The
  `ci-product stable canaries` job passed in 3m02s and the broad advisory
  canaries skipped under the stable-only default. Treat this as current proof
  evidence, not a required-gate promotion.
- Experimental, Stabilizing, and Advisory rows here are not marketing claims.
  They are current evidence snapshots.
- Do not promote any Stabilizing or Advisory surface to Stable from this
  summary alone. Stable promotion happens by updating `SUPPORT_TIERS.md` with
  proof, limitations, README/book wording, and a release-readable rollback.

## Related Artifacts

- `docs/status/SUPPORT_TIERS.md` - authoritative tier and proof ledger.
- `docs/status/PRODUCT_OBJECTIVE_AUDIT.md` - objective-to-proof completion
  audit and remaining non-completion reasons.
- `docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md` - behavior
  contract for Stable claim proof.
- `scripts/ci-product-stable.sh` - bounded Stable README product canaries.
- `docs/status/KNOWN_RED.md` - exclusions and non-required surfaces.
- `docs/specs/ADZE-SPEC-0003-canonical-parse-document.md` - native document
  contract.
- `docs/specs/ADZE-SPEC-0008-json-cli-wasm-projections.md` - serialized
  projection contract.

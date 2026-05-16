# Product Proof Map

**Status:** release-readable companion to `SUPPORT_TIERS.md`
**Source of truth:** `SUPPORT_TIERS.md`
**Last updated:** 2026-05-16

This map answers one question quickly: which product claims have proof, and
where is that proof owned?

`SUPPORT_TIERS.md` remains the authoritative support-tier ledger. This file
summarizes the current release-facing claims so users, maintainers, and agents
do not have to mine the dense tier table for the common decision points.

## Claim Map

| Product claim | Tier | Source of truth | Representative proof | Release note |
|---|---|---|---|---|
| Typed Rust extraction works for supported generated grammars. | Stable | `SUPPORT_TIERS.md`; README capability table | `just ci-supported`; `cargo test -p adze --features pure-rust --test typed_ast_contract typed_ast_contract_left_associative_addition -- --exact --nocapture`; `cargo test -p adze --features pure-rust --test typed_ast_contract typed_ast_contract_repeated_parse_is_deterministic -- --exact --nocapture` | Stable 0.8+ user contract. |
| Pure-Rust generated parsers can be used from clean downstream crates. | Stable | `SUPPORT_TIERS.md`; README quickstart; getting-started tutorial | `just ci-product-stable`; `cargo test -p adze-cli readme_arithmetic_quickstart_builds_and_runs -- --exact --nocapture`; `cargo test -p downstream-demo -- --nocapture`; `cargo run -p downstream-demo --quiet` | Stable claim with downstream clean-room proof. |
| Operator precedence works for the documented arithmetic shape. | Stable | `SUPPORT_TIERS.md`; README example | `cargo test -p adze-cli readme_arithmetic_quickstart_builds_and_runs -- --exact --nocapture`; `cargo test -p adze-glr-core --test ambiguity_detection_comprehensive test_precedence_resolves_add_mul -- --exact --nocapture` | Stable for proven expression grammar shapes, not every ambiguous grammar. |
| Core parse-table serialization roundtrips. | Stable | `SUPPORT_TIERS.md` | `cargo test -p adze-glr-core --features serialization --doc`; `cargo test -p adze-glr-core --features serialization --test serialization_v9 sv9_complex_precedence_roundtrip -- --exact --nocapture` | Stable for core table serialization. Document JSON remains experimental. |
| GLR conflict routing preserves and selects proven ambiguity cases. | Stabilizing | `SUPPORT_TIERS.md`; `ADZE-SPEC-0007`; `ADZE-ADR-0003` | `cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test test_e2e_ambiguous_grammar_glr generated_ambiguous_expr_glr_runtime_retains_multiple_complete_alternatives -- --exact --nocapture`; `cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test test_e2e_ambiguous_grammar_glr generated_ambiguous_expr_parse_document_reports_ambiguity_summary -- --exact --nocapture` | Stabilizing; full forest export and broader ambiguity policy remain future work. |
| Generated parse errors expose structured spans, expected tokens, and no-panic bad-input behavior. | Stabilizing | `SUPPORT_TIERS.md`; `ADZE-SPEC-0005` | `cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors generated_typed_parser_unexpected_eof_expected_field_is_populated -- --exact --nocapture`; `cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors generated_typed_parser_bad_inputs_return_errors_without_panicking -- --exact --nocapture` | Stabilizing; diagnostic wording and broader grammar matrix are still maturing. |
| `AdzeDocument` is the native parse-product boundary for document-oriented tooling. | Experimental | `ADZE-SPEC-0003`; `ADZE-ADR-0001`; `SUPPORT_TIERS.md` | `cargo test -p adze --features "pure-rust,ts-compat" --test adze_document_alpha -- --nocapture`; `cargo test -p adze --features pure-rust --test typed_ast_contract typed_ast_contract_parse_document_ast_matches_parse -- --exact --nocapture`; `cargo test -p adze --features "pure-rust,serialization" --test adze_document_json -- --nocapture` | 0.9 API foundation surface; not a stable public API yet. |
| Typed CST wrappers are generated views over document node IDs and edge fields. | Experimental | `ADZE-SPEC-0004`; `ADZE-ADR-0001`; `SUPPORT_TIERS.md` | `cargo test -p adze --features pure-rust --test typed_cst_generated_document -- --nocapture`; `cargo test -p adze-tablegen typed_cst_generator -- --nocapture` | Experimental Rust-native syntax layer; no visitor/rewriter or broad parity matrix yet. |
| Tree-sitter compatibility projects from native document/schema data. | Advisory | `ADZE-SPEC-0006`; `docs/reference/tree-sitter-compatibility.md`; `SUPPORT_TIERS.md` | `cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_to_sexp -- --nocapture`; `cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_language_metadata -- --nocapture`; `cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_node_types -- --nocapture` | Useful compatibility adapter; not full Tree-sitter parity. |
| Native document JSON uses an experimental schema-tagged envelope. | Experimental | `ADZE-SPEC-0008`; `ADZE-ADR-0004`; `SUPPORT_TIERS.md` | `cargo test -p adze --features "pure-rust,serialization" --test adze_document_json -- --nocapture`; `cargo test -p adze --features "pure-rust,serialization,glr" --test adze_document_json parse_document_json_serializes_glr_ambiguity_summary -- --exact --nocapture` | Experimental `adze.document.v1`; not yet a stable CLI/WASM schema contract. |
| CLI project scaffolding and document-output smoke checks are advisory tools. | Advisory | `SUPPORT_TIERS.md`; `cli/README.md` | `cargo test -p adze-cli test_init_default_cwd_generates_buildable_project -- --exact --nocapture`; `cargo test -p adze-cli test_parse_help_documents_available_modes -- --exact --nocapture`; `cargo test -p adze-cli test_parse_document_json_mode_emits_schema_envelope -- --exact --nocapture` | CLI is useful but outside the required support contract; document projection output is advisory and not yet a stable CLI/WASM schema contract. |
| WASM compiles for the demo target. | Advisory | `SUPPORT_TIERS.md`; `KNOWN_RED.md` | `cargo check --manifest-path wasm-demo/Cargo.toml --target wasm32-unknown-unknown` | Compile signal only; browser/runtime behavior is not certified. |
| Benchmarks use valid parser workloads and compile on explicit benchmark lanes. | Advisory | `SUPPORT_TIERS.md`; `benchmarks/README.md` | `cargo test -p adze-benchmarks --test verify_fixture_parsing verify_parse_bench_uses_real_parser_workload -- --exact --nocapture`; `cargo bench -p adze-benchmarks --no-run` | Benchmark evidence is manual/scheduled, not merge-blocking product proof. |

## Promotion Rules

- A README Stable claim must have a matching `SUPPORT_TIERS.md` row and a
  repeatable proof command.
- `ci-product-stable` is the bounded Stable-claim canary lane, but it remains
  advisory until branch protection explicitly promotes it.
- Experimental, Stabilizing, and Advisory rows here are not marketing claims.
  They are current evidence snapshots.
- Do not promote `AdzeDocument`, typed CST, Tree-sitter compatibility, GLR full
  forest, CLI JSON, or WASM behavior from this summary alone. Promotion happens
  by updating `SUPPORT_TIERS.md` with proof.

## Related Artifacts

- `docs/status/SUPPORT_TIERS.md` - authoritative tier and proof ledger.
- `docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md` - behavior
  contract for Stable claim proof.
- `scripts/ci-product-stable.sh` - bounded Stable README product canaries.
- `docs/status/KNOWN_RED.md` - exclusions and non-required surfaces.
- `docs/specs/ADZE-SPEC-0003-canonical-parse-document.md` - native document
  contract.
- `docs/specs/ADZE-SPEC-0008-json-cli-wasm-projections.md` - serialized
  projection contract.

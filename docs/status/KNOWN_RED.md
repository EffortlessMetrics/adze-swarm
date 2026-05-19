# Known red

**Last updated:** 2026-05-19

This file tracks intentional exclusions from the supported lane:

- Required `adze-swarm` PR gate: `Rust Small Result` in GitHub checks
- Supported local/product proof: `just ci-supported`
- Lane classification: [CI_LANES.md](../../.github/CI_LANES.md)

Rule: if something is excluded from the supported lane, it must be listed here with:
- what is excluded
- why
- how it becomes supported (or why it won't)

Support tiers and proof commands for major surfaces are tracked in [`docs/status/SUPPORT_TIERS.md`](./SUPPORT_TIERS.md). The post-queue correctness proof plan is tracked in [`docs/status/CORRECTNESS_PUSH.md`](./CORRECTNESS_PUSH.md).

---

## ✅ Previously broken — now fixed

### `adze` (runtime) crate — RESOLVED
- **Was:** `cargo check -p adze` failed with ~20 errors (lifetime, type, borrow-checker issues).
- **Fixed:** All compile errors resolved. `cargo check -p adze` passes. `cargo fmt` and `cargo clippy` clean.
- **Date:** 2026-03-04

### Core pipeline crates
- `adze-ir`, `adze-glr-core`, `adze-tablegen`, `adze-common`, `adze-macro`, `adze-tool` all pass `cargo check`, `cargo clippy`, and `cargo test`.

---

## What the supported lane covers

`ci-supported` currently checks the **core pipeline** (7 crates: `adze`, `adze-macro`, `adze-tool`, `adze-common`, `adze-ir`, `adze-glr-core`, `adze-tablegen`):

- `scripts/fmt-workspace.sh --check` for the declared workspace-member formatting proof. This replaces the intended `cargo fmt --all --check` release check with per-member invocations so Windows does not fail before rustfmt starts.
- `cargo clippy` (supported crates, `-D warnings`)
- `cargo test` (supported crates: lib, tests, bins)
- `adze-glr-core` doctests with `serialization` feature

This lane is intentionally bounded so it stays reliable and fast enough for day-to-day work.

**Current required status:** GREEN when `Rust Small Result` passes in `adze-swarm`. `just ci-supported` remains the local supported/product proof. Broader feature matrices, audit, WASM, and product-proof checks are useful optional signal, but they are not part of the swarm merge gate unless explicitly promoted here, in [CI_LANES.md](../../.github/CI_LANES.md), and in [`SUPPORT_TIERS.md`](./SUPPORT_TIERS.md).

---

## What is excluded (and why)

### Not in the supported lane (workspace members / tools)
These are intentionally excluded for now because they are prototypes, platform-sensitive, heavier than the supported contract, or still stabilizing:

- `runtime2/` (experimental proving ground; still converging, not in merge-blocking lane)
  - **Support tier:** `experimental proving ground`.
  - **Bounded expectation (as of 2026-04-26):** we only treat runtime2 as an opt-in surface for API and behavior experiments; `ci-supported` does not certify it as stable/public-primary runtime yet.
  - **Current smoke proof:** `runtime2/tests/basic.rs::language_smoke_exposes_metadata_queries` validates that a minimal language object can be constructed and queried for symbol metadata.
- `cli/`, `lsp-generator/`, `playground/`, `wasm-demo/` (tooling/prototypes)
- `golden-tests/` (useful contract, but can be heavy and multi-language)
- `benchmarks/` (signal, not merge-blocking)
- `grammars/*` (valuable, but not yet a stable published surface)
- `crates/*` support surfaces are not part of the required `ci-supported`
  lane unless one is also a core pipeline dependency. The 0.9
  microcrate-to-SRP collapse is complete; release readiness is guarded by
  `cargo run -q -p xtask -- check-package-boundary --release-gate`, which
  fails if a temporary owner-module migration target returns.

### Not in the supported lane (workflows)
These may run as optional signal (nightly/manual/canary), but are not required for merge:

- fuzzing lanes (20 targets exist but run on schedule/manual dispatch)
- wide platform matrices
- workflow_dispatch-only CI lanes and manual opt-ins (e.g. feature-matrix examples/burn-in paths)
- deployment workflows (mdBook / pages)
- performance regression canaries
- All other `.github/workflows/ci.yml` jobs are optional unless explicitly promoted in settings.
- Published `cargo install adze-cli` proof. `just package-local adze-cli`
  passed on 2026-05-19 and verifies the local CLI package with co-release
  patches, but current product proof still uses the repo-built CLI and
  downstream fixtures. Treat crates.io CLI installation as release-surface work
  until an install receipt exists.

---


## Advisory product proof lane (non-blocking)

A broad-surface advisory lane now exists as `.github/workflows/product-proof.yml` and runs `scripts/ci-product.sh` on schedule or manual dispatch with `lane=all`.

This lane is **not** part of required merge gates. It provides bounded canary proof across product surfaces that are outside `ci-supported`. A narrower `ci-product stable canaries` job runs `just ci-product-stable` on stable-claim PR surfaces, schedule, and stable-only manual dispatch, but it is also advisory until branch protection explicitly promotes it.

Latest stable-product receipt: GitHub workflow dispatch
[`Product Proof` run 26104726428](https://github.com/EffortlessMetrics/adze-swarm/actions/runs/26104726428)
passed on 2026-05-19 from `adze-swarm/main` after PR #281. The
`ci-product stable canaries` job passed in 3m02s and `ci-product advisory
canaries` skipped under the stable-only default. This remains advisory and is
not part of required branch protection.

Current canaries:

- `adze` runtime pure-rust typed extraction — **behavior** (`cargo test -p adze --features pure-rust --test typed_ast_contract typed_ast_contract_left_associative_addition -- --exact --nocapture`)
- `adze` GLR ambiguous typed extraction — **behavior** (`cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test test_e2e_ambiguous_grammar_glr test_ambiguous_grammar_glr_parsing -- --exact --nocapture`)
- `adze` GLR multi-conflict selection determinism — **behavior** (`cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test test_e2e_ambiguous_grammar_glr generated_ambiguous_expr_multi_conflict_selection_is_deterministic -- --exact --nocapture`)
- `adze` GLR generated parser bad-input no-panic guardrail — **behavior** (`cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test test_e2e_ambiguous_grammar_glr generated_glr_parser_bad_inputs_return_errors_without_panicking -- --exact --nocapture`)
- `adze` GLR generated conflict preservation — **behavior** (`cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test test_e2e_ambiguous_grammar_glr test_ambiguous_grammar_conflict_generation -- --exact --nocapture`)
- `adze` GLR nested fork conflict inspection — **behavior** (`cargo test -p adze-glr-core --test conflict_inspection_comprehensive nested_fork_conflict_cells_are_detected -- --exact --nocapture`)
- `adze` GLR reduce-reduce driver canary — **behavior** (`cargo test -p adze-glr-core --test parser_driver_tests reduce_reduce_parses_despite_conflict -- --exact --nocapture`)
- `adze` GLR parser_v4 canonical conflict routing — **behavior** (`cargo test -p adze --features "pure-rust,glr" --test parser_v4_comprehensive test_parser_v4_rejects_single_action_fork_conflict_before_parsing -- --exact --nocapture`)
- `adze` GLR dangling-else conflict preservation — **behavior** (`cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test test_dangling_else_conflicts verify_conflict_preservation_behavior -- --exact --nocapture`)
- `adze` GLR dangling-else selected tree — **behavior** (`cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test test_dangling_else_conflicts generated_dangling_else_selects_nearest_else_and_records_ambiguity -- --exact --nocapture`)
- `adze` generated reduce/reduce preservation and selected tree — **behavior** (`cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test generated_reduce_reduce_gap -- --nocapture`)
- `adze` structured parse diagnostics — **behavior** (`cargo test -p adze --test error_display_tests reporting_parse_with_errors_includes_source_excerpt_after_bad_input --features "pure-rust,glr" -- --exact --nocapture`)
- `adze` multiline parse diagnostic location — **behavior** (`cargo test -p adze --test error_display_tests reporting_parse_with_errors_tracks_multiline_bad_input_location_and_excerpt --features "pure-rust,glr" -- --exact --nocapture`)
- `adze` parse diagnostic byte spans — **behavior** (`cargo test -p adze --test error_display_tests reporting_parse_diagnostics_include_byte_span_for_multiline_bad_input --features "pure-rust,glr" -- --exact --nocapture`)
- `adze` parse diagnostic display excerpts — **behavior** (`cargo test -p adze --test error_display_tests reporting_parse_diagnostics_display_includes_multiline_excerpt --features "pure-rust,glr" -- --exact --nocapture`)
- `adze` generated typed parser parse diagnostics — **behavior** (`cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors generated_typed_parser_bad_token_reports_source_span -- --exact --nocapture`)
- `adze` generated typed parser UTF-8 parse diagnostics — **behavior** (`cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors generated_typed_parser_multibyte_bad_token_reports_utf8_byte_span -- --exact --nocapture`)
- `adze` generated typed parser EOF parse diagnostics — **behavior** (`cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors generated_typed_parser_unexpected_eof_reports_zero_width_source_span -- --exact --nocapture`)
- `adze` generated typed parser expected-token diagnostics — **behavior** (`cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors generated_typed_parser_unexpected_eof_lists_expected_tokens -- --exact --nocapture`)
- `adze` generated typed parser structured expected-token field — **behavior** (`cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors generated_typed_parser_unexpected_eof_expected_field_is_populated -- --exact --nocapture`)
- `adze` generated typed parser expected-token set names — **behavior** (`cargo test -p adze --features pure-rust --test generated_parse_errors expected_token_sets_are_reported -- --exact --nocapture`)
- `adze` generated typed parser LR diagnostic contract — **behavior** (`cargo test -p adze --features pure-rust --test generated_parse_errors generated_typed_parser_error_contract_is_feature_stable -- --exact --nocapture`)
- `adze` generated typed parser GLR-feature diagnostic contract — **behavior** (`cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors generated_typed_parser_error_contract_is_feature_stable -- --exact --nocapture`)
- `adze` generated precedence arithmetic LR diagnostic contract — **behavior** (`cargo test -p adze --features pure-rust --test generated_parse_errors generated_precedence_arithmetic_parser_error_contract_is_feature_stable -- --exact --nocapture`)
- `adze` generated precedence arithmetic GLR-feature diagnostic contract — **behavior** (`cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors generated_precedence_arithmetic_parser_error_contract_is_feature_stable -- --exact --nocapture`)
- `adze` generated words LR diagnostic contract — **behavior** (`cargo test -p adze --features pure-rust --test generated_parse_errors generated_words_parser_error_contract_is_feature_stable -- --exact --nocapture`)
- `adze` generated words GLR-feature diagnostic contract — **behavior** (`cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors generated_words_parser_error_contract_is_feature_stable -- --exact --nocapture`)
- `adze` generated CSV-list LR diagnostic contract — **behavior** (`cargo test -p adze --features pure-rust --test generated_parse_errors generated_csv_list_parser_error_contract_is_feature_stable -- --exact --nocapture`)
- `adze` generated CSV-list GLR-feature diagnostic contract — **behavior** (`cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors generated_csv_list_parser_error_contract_is_feature_stable -- --exact --nocapture`)
- `adze` generated object-like LR diagnostic contract — **behavior** (`cargo test -p adze --features pure-rust --test generated_parse_errors generated_object_like_parser_error_contract_is_feature_stable -- --exact --nocapture`)
- `adze` generated object-like GLR-feature diagnostic contract — **behavior** (`cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors generated_object_like_parser_error_contract_is_feature_stable -- --exact --nocapture`)
- `adze` generated typed parser bad-input no-panic guardrail — **behavior** (`cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors generated_typed_parser_bad_inputs_return_errors_without_panicking -- --exact --nocapture`)
- `adze` generated typed parser multiline parse diagnostics — **behavior** (`cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors generated_typed_parser_multiline_bad_token_reports_line_column_and_excerpt -- --exact --nocapture`)
- `adze` core parse-table serialization doctests — **behavior** (`cargo test -p adze-glr-core --features serialization --doc`)
- `adze` core parse-table serialization roundtrip — **behavior** (`cargo test -p adze-glr-core --features serialization --test serialization_v9 sv9_complex_precedence_roundtrip -- --exact --nocapture`)
- `adze` tablegen ABI compressed decode roundtrip — **behavior** (`cargo test -p adze --features "pure-rust,glr,ts-compat" --test tablegen_abi_decode_roundtrip compressed_tslanguage_decode_preserves_metadata_actions_and_fields -- --exact --nocapture`)
- `adze` tablegen ABI public symbol map roundtrip — **behavior** (`cargo test -p adze --features "pure-rust,glr,ts-compat" --test tablegen_abi_decode_roundtrip compressed_tslanguage_decode_preserves_public_symbol_map -- --exact --nocapture`)
- `adze` tablegen ABI external token metadata roundtrip — **behavior** (`cargo test -p adze --features "pure-rust,glr,ts-compat" --test tablegen_abi_decode_roundtrip compressed_tslanguage_decode_preserves_external_token_metadata -- --exact --nocapture`)
- `adze` tablegen ABI lex mode roundtrip — **behavior** (`cargo test -p adze --features "pure-rust,glr,ts-compat" --test tablegen_abi_decode_roundtrip compressed_tslanguage_decode_preserves_lex_modes -- --exact --nocapture`)
- `adze` tablegen ABI combined metadata roundtrip — **behavior** (`cargo test -p adze --features "pure-rust,glr,ts-compat" --test tablegen_abi_decode_roundtrip combined_tslanguage_decode_preserves_metadata_fields_aliases_externals_and_lex_modes -- --exact --nocapture`)
- `adze` tablegen ABI conflict decode preservation — **behavior** (`cargo test -p adze --features "pure-rust,glr,runtime-e2e,ts-compat" --test test_e2e_ambiguous_grammar_glr tablegen_abi_decode_preserves_generated_conflict_cells -- --exact --nocapture`)
- `adze` tablegen ABI alias decode preservation — **behavior** (`cargo test -p adze --features "pure-rust,glr,ts-compat" --test tablegen_abi_decode_roundtrip compressed_tslanguage_decode_preserves_alias_sequences -- --exact --nocapture`)
- `adze-tablegen` node-types rule-name preservation — **behavior** (`cargo test -p adze-tablegen --test static_language_gen_comprehensive generate_node_types_preserves_rule_names -- --exact --nocapture`)
- `adze-tablegen` alias ABI pointer/data preservation — **behavior** (`cargo test -p adze-tablegen --test alias_handling_comprehensive alias_abi_emits_non_null_pointers_when_counters_nonzero -- --exact --nocapture`)
- `adze-tablegen` sparse production LHS ABI preservation — **behavior** (`cargo test -p adze-tablegen --test production_id_comprehensive edge_sparse_production_ids_emit_dense_production_lhs_index -- --exact --nocapture`)
- README arithmetic quickstart clean-room parse and diagnostics — **behavior** (`cargo test -p adze-cli readme_arithmetic_quickstart_builds_and_runs -- --exact --nocapture`)
- Getting Started quickstart clean-room parse and diagnostics — **behavior** (`cargo test -p adze-cli getting_started_quickstart_builds_parses_and_reports_diagnostics -- --exact --nocapture`)
- Book quickstart clean-room parse and diagnostics — **behavior** (`cargo test -p adze-cli book_quickstart_builds_parses_and_reports_diagnostics -- --exact --nocapture`)
- Checked-in downstream quickstart sample — **behavior** (`cargo test -p downstream-demo -- --nocapture`)
- `adze` typed AST repeated-parse determinism — **behavior** (`cargo test -p adze --features pure-rust --test typed_ast_contract typed_ast_contract_repeated_parse_is_deterministic -- --exact --nocapture`)
- `adze-cli` default-cwd init/check smoke — **behavior** (`cargo test -p adze-cli test_init_default_cwd_generates_buildable_project -- --exact --nocapture`)
- `adze-cli` generated starter test/example/check smoke — **behavior** (`cargo test -p adze-cli test_init_generates_buildable_project -- --exact --nocapture`)
- `adze-cli` clean-room init dependency smoke — **behavior** (`cargo test -p adze-cli test_init_cargo_toml_references_adze_dependency -- --exact --nocapture`)
- `adze-cli` check rejects non-grammar Rust files — **behavior** (`cargo test -p adze-cli test_check_rejects_file_without_adze_grammar -- --exact --nocapture`)
- `adze-cli` stats rejects non-grammar Rust files — **behavior** (`cargo test -p adze-cli test_stats_rejects_file_without_adze_grammar -- --exact --nocapture`)
- `adze-cli` parse unsupported-mode truthfulness — **behavior** (`cargo test -p adze-cli test_parse_static_mode_is_explicitly_unimplemented -- --exact --nocapture`)
- `adze-cli` parse help output-mode documentation — **behavior** (`cargo test -p adze-cli test_parse_help_documents_available_modes -- --exact --nocapture`)
- `adze-cli` static document projection output modes — **behavior** (`cargo test -p adze-cli test_parse_document_projection_modes_emit_schema_envelopes -- --exact --nocapture`)
- `adze-cli` document JSON recovery diagnostics — **behavior** (`cargo test -p adze-cli parse_document_json_modes_emit_recovery_diagnostics -- --exact --nocapture`)
- `adze-tool` test command rejects corpus without parser execution — **behavior** (`cargo test -p adze-tool --test cli_test test_test_command_rejects_corpus_without_parser -- --exact --nocapture`)
- `adze-golden-tests` JavaScript canary — **behavior** (`cargo test -p adze-golden-tests javascript_canary_expression_golden --features javascript-grammar -- --nocapture`)
- benchmark arithmetic fixture validity — **behavior** (`cargo test -p adze-benchmarks --test verify_fixture_parsing verify_arithmetic_benchmark_fixtures_parse_with_arithmetic_grammar -- --exact --nocapture`)
- benchmark parse_bench real parser workload — **behavior** (`cargo test -p adze-benchmarks --test verify_fixture_parsing verify_parse_bench_uses_real_parser_workload -- --exact --nocapture`)
- `adze-benchmarks` canary — **compile-only** (`cargo bench -p adze-benchmarks --no-run`)
- `wasm-demo` canary — **compile-only** (`cargo check --manifest-path wasm-demo/Cargo.toml --target wasm32-unknown-unknown`)
- grammar metadata smoke (`adze-python`) — **behavior** (`cargo test -p adze-python test_python_language_exists -- --exact --nocapture`)
- `runtime2` metadata canary — **behavior** (`cargo test --manifest-path runtime2/Cargo.toml --features test-utils --test basic language_smoke_exposes_metadata_queries -- --exact --nocapture`)
- governance/BDD grid owner smoke (`adze-bdd-governance-core`) — **behavior** (`cargo test -p adze-bdd-governance-core --lib grid::tests::progress_summary_reports_counts -- --exact --nocapture`)

Notes:
- This lane intentionally does not provide full product proof; it is bounded canary signal only.
- Compile-only canaries remain only where the current truthful claim is compile/no-run signal, notably benchmarks and WASM.
- The next promotion step is making `ci-product-stable` required for README-stable claims only, after the advisory canaries are consistently green.
- If one canary is red, the advisory job can fail while remaining non-blocking due to workflow `continue-on-error: true`.

## Known warnings (non-blocking)

- ~~`rustdoc::private_intra_doc_links` warning in `adze` (runtime) crate doc build~~ — **Resolved.** 0 rustdoc warnings across supported crates.
- `unused manifest key` warnings in `lsp-generator/Cargo.toml` and `wasm-demo/Cargo.toml` — these are excluded crates.

---

## How something graduates into the supported lane

To add a crate/workflow to the supported lane, it must be:
- reproducible on a normal dev machine
- stable across the supported toolchain/MSRV
- bounded in time/resources
- behavior-proven, not just compile-proven
- documented (how to run it locally; common failure modes)

When you add something to `ci-supported`, update this file in the same PR.

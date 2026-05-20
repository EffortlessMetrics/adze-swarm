#!/usr/bin/env bash
set -euo pipefail

DRY_RUN=false
if [[ "${1:-}" == "--dry-run" ]]; then
  DRY_RUN=true
fi

# Canary definitions: "label|proof_type|command"
CANARIES=(
  "adze runtime pure-rust typed extraction|behavior|cargo test -p adze --features pure-rust --test typed_ast_contract typed_ast_contract_left_associative_addition -- --exact --nocapture"
  "adze typed AST repeated-parse determinism|behavior|cargo test -p adze --features pure-rust --test typed_ast_contract typed_ast_contract_repeated_parse_is_deterministic -- --exact --nocapture"
  "adze GLR ambiguous typed extraction|behavior|cargo test -p adze --features \"pure-rust,glr,runtime-e2e\" --test test_e2e_ambiguous_grammar_glr test_ambiguous_grammar_glr_parsing -- --exact --nocapture"
  "adze GLR multi-conflict selection determinism|behavior|cargo test -p adze --features \"pure-rust,glr,runtime-e2e\" --test test_e2e_ambiguous_grammar_glr generated_ambiguous_expr_multi_conflict_selection_is_deterministic -- --exact --nocapture"
  "adze GLR generated alternatives retention|behavior|cargo test -p adze --features \"pure-rust,glr,runtime-e2e\" --test test_e2e_ambiguous_grammar_glr generated_ambiguous_expr_glr_runtime_retains_multiple_complete_alternatives -- --exact --nocapture"
  "adze GLR generated parser bad-input no-panic guardrail|behavior|cargo test -p adze --features \"pure-rust,glr,runtime-e2e\" --test test_e2e_ambiguous_grammar_glr generated_glr_parser_bad_inputs_return_errors_without_panicking -- --exact --nocapture"
  "adze GLR generated conflict preservation|behavior|cargo test -p adze --features \"pure-rust,glr,runtime-e2e\" --test test_e2e_ambiguous_grammar_glr test_ambiguous_grammar_conflict_generation -- --exact --nocapture"
  "adze GLR nested fork conflict inspection|behavior|cargo test -p adze-glr-core --test conflict_inspection_comprehensive nested_fork_conflict_cells_are_detected -- --exact --nocapture"
  "adze GLR reduce-reduce driver canary|behavior|cargo test -p adze-glr-core --test parser_driver_tests reduce_reduce_parses_despite_conflict -- --exact --nocapture"
  "adze GLR parser_v4 canonical conflict routing|behavior|cargo test -p adze --features \"pure-rust,glr\" --test parser_v4_comprehensive test_parser_v4_rejects_single_action_fork_conflict_before_parsing -- --exact --nocapture"
  "adze GLR dangling-else conflict preservation|behavior|cargo test -p adze --features \"pure-rust,glr,runtime-e2e\" --test test_dangling_else_conflicts verify_conflict_preservation_behavior -- --exact --nocapture"
  "adze GLR dangling-else selection gap guardrail|behavior|cargo test -p adze --features \"pure-rust,glr,runtime-e2e\" --test test_dangling_else_conflicts generated_dangling_else_selection_gap_returns_error_without_panicking -- --exact --nocapture"
  "adze structured parse diagnostics|behavior|cargo test -p adze --test error_display_tests reporting_parse_with_errors_includes_source_excerpt_after_bad_input --features \"pure-rust,glr\" -- --exact --nocapture"
  "adze multiline parse diagnostic location|behavior|cargo test -p adze --test error_display_tests reporting_parse_with_errors_tracks_multiline_bad_input_location_and_excerpt --features \"pure-rust,glr\" -- --exact --nocapture"
  "adze parse diagnostic byte spans|behavior|cargo test -p adze --test error_display_tests reporting_parse_diagnostics_include_byte_span_for_multiline_bad_input --features \"pure-rust,glr\" -- --exact --nocapture"
  "adze parse diagnostic display excerpts|behavior|cargo test -p adze --test error_display_tests reporting_parse_diagnostics_display_includes_multiline_excerpt --features \"pure-rust,glr\" -- --exact --nocapture"
  "adze generated typed parser parse diagnostics|behavior|cargo test -p adze --features \"pure-rust,glr\" --test generated_parse_errors generated_typed_parser_bad_token_reports_source_span -- --exact --nocapture"
  "adze generated typed parser UTF-8 parse diagnostics|behavior|cargo test -p adze --features \"pure-rust,glr\" --test generated_parse_errors generated_typed_parser_multibyte_bad_token_reports_utf8_byte_span -- --exact --nocapture"
  "adze generated typed parser EOF parse diagnostics|behavior|cargo test -p adze --features \"pure-rust,glr\" --test generated_parse_errors generated_typed_parser_unexpected_eof_reports_zero_width_source_span -- --exact --nocapture"
  "adze generated typed parser expected-token diagnostics|behavior|cargo test -p adze --features \"pure-rust,glr\" --test generated_parse_errors generated_typed_parser_unexpected_eof_lists_expected_tokens -- --exact --nocapture"
  "adze generated typed parser structured expected-token field|behavior|cargo test -p adze --features \"pure-rust,glr\" --test generated_parse_errors generated_typed_parser_unexpected_eof_expected_field_is_populated -- --exact --nocapture"
  "adze generated typed parser expected-token set names|behavior|cargo test -p adze --features pure-rust --test generated_parse_errors expected_token_sets_are_reported -- --exact --nocapture"
  "adze generated typed parser LR diagnostic contract|behavior|cargo test -p adze --features pure-rust --test generated_parse_errors generated_typed_parser_error_contract_is_feature_stable -- --exact --nocapture"
  "adze generated typed parser GLR-feature diagnostic contract|behavior|cargo test -p adze --features \"pure-rust,glr\" --test generated_parse_errors generated_typed_parser_error_contract_is_feature_stable -- --exact --nocapture"
  "adze generated precedence arithmetic LR diagnostic contract|behavior|cargo test -p adze --features pure-rust --test generated_parse_errors generated_precedence_arithmetic_parser_error_contract_is_feature_stable -- --exact --nocapture"
  "adze generated precedence arithmetic GLR-feature diagnostic contract|behavior|cargo test -p adze --features \"pure-rust,glr\" --test generated_parse_errors generated_precedence_arithmetic_parser_error_contract_is_feature_stable -- --exact --nocapture"
  "adze generated words LR diagnostic contract|behavior|cargo test -p adze --features pure-rust --test generated_parse_errors generated_words_parser_error_contract_is_feature_stable -- --exact --nocapture"
  "adze generated words GLR-feature diagnostic contract|behavior|cargo test -p adze --features \"pure-rust,glr\" --test generated_parse_errors generated_words_parser_error_contract_is_feature_stable -- --exact --nocapture"
  "adze generated CSV-list LR diagnostic contract|behavior|cargo test -p adze --features pure-rust --test generated_parse_errors generated_csv_list_parser_error_contract_is_feature_stable -- --exact --nocapture"
  "adze generated CSV-list GLR-feature diagnostic contract|behavior|cargo test -p adze --features \"pure-rust,glr\" --test generated_parse_errors generated_csv_list_parser_error_contract_is_feature_stable -- --exact --nocapture"
  "adze generated object-like LR diagnostic contract|behavior|cargo test -p adze --features pure-rust --test generated_parse_errors generated_object_like_parser_error_contract_is_feature_stable -- --exact --nocapture"
  "adze generated object-like GLR-feature diagnostic contract|behavior|cargo test -p adze --features \"pure-rust,glr\" --test generated_parse_errors generated_object_like_parser_error_contract_is_feature_stable -- --exact --nocapture"
  "adze generated typed parser bad-input no-panic guardrail|behavior|cargo test -p adze --features \"pure-rust,glr\" --test generated_parse_errors generated_typed_parser_bad_inputs_return_errors_without_panicking -- --exact --nocapture"
  "adze generated typed parser multiline parse diagnostics|behavior|cargo test -p adze --features \"pure-rust,glr\" --test generated_parse_errors generated_typed_parser_multiline_bad_token_reports_line_column_and_excerpt -- --exact --nocapture"
  "adze parser_v4 external scanner dispatch spans|behavior|cargo test -p adze --features \"pure-rust,external_scanners\" parser_v4::tests::test_parser_with_external_scanner -- --exact --nocapture"
  "adze parser_v4 external scanner valid-symbol rejection|behavior|cargo test -p adze --features \"pure-rust,external_scanners\" parser_v4::tests::test_external_scanner_rejects_token_not_in_valid_symbols -- --exact --nocapture"
  "adze parser_v4 external scanner diagnostic document|behavior|cargo test -p adze --features \"pure-rust,external_scanners\" parser_v4::tests::test_external_scanner_parse_document_bad_input_returns_diagnostic_document -- --exact --nocapture"
  "adze generated external-token recovery matrix|behavior|cargo test --manifest-path example/Cargo.toml external_word_example::tests::generated_external_grammar_bad_input_matrix_returns_diagnostic_document --features pure-rust -- --exact --nocapture"
  "adze core parse-table serialization roundtrip|behavior|cargo test -p adze-glr-core --features serialization --test serialization_v9 sv9_complex_precedence_roundtrip -- --exact --nocapture"
  "adze tablegen ABI compressed decode roundtrip|behavior|cargo test -p adze --features \"pure-rust,glr,ts-compat\" --test tablegen_abi_decode_roundtrip compressed_tslanguage_decode_preserves_metadata_actions_and_fields -- --exact --nocapture"
  "adze tablegen ABI public symbol map roundtrip|behavior|cargo test -p adze --features \"pure-rust,glr,ts-compat\" --test tablegen_abi_decode_roundtrip compressed_tslanguage_decode_preserves_public_symbol_map -- --exact --nocapture"
  "adze tablegen ABI external token metadata roundtrip|behavior|cargo test -p adze --features \"pure-rust,glr,ts-compat\" --test tablegen_abi_decode_roundtrip compressed_tslanguage_decode_preserves_external_token_metadata -- --exact --nocapture"
  "adze tablegen ABI lex mode roundtrip|behavior|cargo test -p adze --features \"pure-rust,glr,ts-compat\" --test tablegen_abi_decode_roundtrip compressed_tslanguage_decode_preserves_lex_modes -- --exact --nocapture"
  "adze tablegen ABI combined metadata roundtrip|behavior|cargo test -p adze --features \"pure-rust,glr,ts-compat\" --test tablegen_abi_decode_roundtrip combined_tslanguage_decode_preserves_metadata_fields_aliases_externals_and_lex_modes -- --exact --nocapture"
  "adze tablegen ABI conflict decode preservation|behavior|cargo test -p adze --features \"pure-rust,glr,runtime-e2e,ts-compat\" --test test_e2e_ambiguous_grammar_glr tablegen_abi_decode_preserves_generated_conflict_cells -- --exact --nocapture"
  "adze tablegen ABI alias decode preservation|behavior|cargo test -p adze --features \"pure-rust,glr,ts-compat\" --test tablegen_abi_decode_roundtrip compressed_tslanguage_decode_preserves_alias_sequences -- --exact --nocapture"
  "adze-tablegen node-types rule-name preservation|behavior|cargo test -p adze-tablegen --test static_language_gen_comprehensive generate_node_types_preserves_rule_names -- --exact --nocapture"
  "adze-tablegen node-types hidden-rule exclusion|behavior|cargo test -p adze-tablegen --test static_language_gen_comprehensive generate_node_types_excludes_hidden_tokens -- --exact --nocapture"
  "adze-tablegen alias ABI pointer/data preservation|behavior|cargo test -p adze-tablegen --test alias_handling_comprehensive alias_abi_emits_non_null_pointers_when_counters_nonzero -- --exact --nocapture"
  "adze-tablegen sparse production LHS ABI preservation|behavior|cargo test -p adze-tablegen --test production_id_comprehensive edge_sparse_production_ids_emit_dense_production_lhs_index -- --exact --nocapture"
  "adze ts_compat child traversal|behavior|cargo test -p adze --features \"pure-rust,ts-compat\" --test ts_compat_tree_children -- --nocapture"
  "adze ts_compat byte-range descendants|behavior|cargo test -p adze --features \"pure-rust,ts-compat\" --test ts_compat_byte_range -- --nocapture"
  "adze ts_compat point-range descendants|behavior|cargo test -p adze --features \"pure-rust,ts-compat\" --test ts_compat_point_range -- --nocapture"
  "adze ts_compat cursor position lookup|behavior|cargo test -p adze --features \"pure-rust,ts-compat\" --test ts_compat_cursor_position -- --nocapture"
  "adze ts_compat cursor reverse navigation|behavior|cargo test -p adze --features \"pure-rust,ts-compat\" --test ts_compat_cursor_reverse -- --nocapture"
  "adze ts_compat cursor depth|behavior|cargo test -p adze --features \"pure-rust,ts-compat\" --test ts_compat_cursor_depth -- --nocapture"
  "adze ts_compat cursor reset|behavior|cargo test -p adze --features \"pure-rust,ts-compat\" --test ts_compat_cursor_reset -- --nocapture"
  "adze ts_compat cursor descendant index|behavior|cargo test -p adze --features \"pure-rust,ts-compat\" --test ts_compat_cursor_descendant -- --nocapture"
  "adze ts_compat tree cursor|behavior|cargo test -p adze --features \"pure-rust,ts-compat\" --test ts_compat_tree_cursor -- --nocapture"
  "adze ts_compat language field metadata|behavior|cargo test -p adze --features \"pure-rust,ts-compat\" --test ts_compat_language_fields -- --nocapture"
  "adze ts_compat child field id lookup|behavior|cargo test -p adze --features \"pure-rust,ts-compat\" --test ts_compat_language_fields -- --nocapture"
  "adze ts_compat language node metadata|behavior|cargo test -p adze --features \"pure-rust,ts-compat\" --test ts_compat_language_metadata -- --nocapture"
  "adze ts_compat node descendant counts|behavior|cargo test -p adze --features \"pure-rust,ts-compat\" --test ts_compat_node_descendant_count -- --nocapture"
  "adze ts_compat node byte child lookup|behavior|cargo test -p adze --features \"pure-rust,ts-compat\" --test ts_compat_node_first_child -- --nocapture"
  "adze ts_compat node metadata|behavior|cargo test -p adze --features \"pure-rust,ts-compat\" --test ts_compat_node_metadata -- --nocapture"
  "adze ts_compat node ranges|behavior|cargo test -p adze --features \"pure-rust,ts-compat\" --test ts_compat_node_range -- --nocapture"
  "adze ts_compat node error-state guardrails|behavior|cargo test -p adze --features \"pure-rust,ts-compat\" --test ts_compat_node_error -- --nocapture"
  "adze ts_compat missing-node guardrail|behavior|cargo test -p adze --features \"pure-rust,ts-compat\" --test ts_compat_node_error generated_tree_reports_zero_width_error_root_as_missing -- --exact --nocapture"
  "adze ts_compat S-expression output|behavior|cargo test -p adze --features \"pure-rust,ts-compat\" --test ts_compat_to_sexp -- --nocapture"
  "README arithmetic quickstart clean-room parse and diagnostics|behavior|cargo test -p adze-cli readme_arithmetic_quickstart_builds_and_runs -- --exact --nocapture"
  "Book quickstart clean-room parse and diagnostics|behavior|cargo test -p adze-cli book_quickstart_builds_parses_and_reports_diagnostics -- --exact --nocapture"
  "adze-cli default-cwd init/check smoke|behavior|cargo test -p adze-cli test_init_default_cwd_generates_buildable_project -- --exact --nocapture"
  "adze-cli clean-room init dependency smoke|behavior|cargo test -p adze-cli test_init_cargo_toml_references_adze_dependency -- --exact --nocapture"
  "adze-cli check rejects non-grammar rust file|behavior|cargo test -p adze-cli test_check_rejects_file_without_adze_grammar -- --exact --nocapture"
  "adze-cli stats rejects non-grammar rust file|behavior|cargo test -p adze-cli test_stats_rejects_file_without_adze_grammar -- --exact --nocapture"
  "adze-cli parse unsupported-mode truthfulness|behavior|cargo test -p adze-cli test_parse_static_mode_is_explicitly_unimplemented -- --exact --nocapture"
  "adze-cli parse help output-mode documentation|behavior|cargo test -p adze-cli test_parse_help_documents_available_modes -- --exact --nocapture"
  "adze-tool test rejects corpus without parser|behavior|cargo test -p adze-tool --test cli_test test_test_command_rejects_corpus_without_parser -- --exact --nocapture"
  "golden-tests javascript canary|behavior|cargo test -p adze-golden-tests javascript_canary_expression_golden --features javascript-grammar -- --nocapture"
  "benchmark arithmetic fixture validity|behavior|cargo test -p adze-benchmarks --test verify_fixture_parsing verify_arithmetic_benchmark_fixtures_parse_with_arithmetic_grammar -- --exact --nocapture"
  "benchmark parse_bench real parser workload|behavior|cargo test -p adze-benchmarks --test verify_fixture_parsing verify_parse_bench_uses_real_parser_workload -- --exact --nocapture"
  "benchmarks canary|compile-only|cargo bench -p adze-benchmarks --no-run"
  "wasm-demo canary|compile-only|cargo check --manifest-path wasm-demo/Cargo.toml --target wasm32-unknown-unknown"
  "grammar metadata smoke (python)|behavior|cargo test -p adze-python test_python_language_exists -- --exact --nocapture"
  "runtime2 metadata smoke|behavior|cargo test --manifest-path runtime2/Cargo.toml --features test-utils --test basic language_smoke_exposes_metadata_queries -- --exact --nocapture"
  "governance/BDD grid owner smoke|behavior|cargo test -p adze-bdd-governance-core --lib grid::tests::progress_summary_reports_counts -- --exact --nocapture"
)

printf '== ci-product advisory canaries ==\n'
printf 'Mode: %s\n\n' "$([[ "$DRY_RUN" == true ]] && echo dry-run || echo execute)"

failures=0
for entry in "${CANARIES[@]}"; do
  IFS='|' read -r label proof_type cmd <<<"$entry"
  printf '\n[%s] %s\n' "$proof_type" "$label"
  printf '  $ %s\n' "$cmd"

  if [[ "$DRY_RUN" == true ]]; then
    continue
  fi

  if eval "$cmd"; then
    printf '  -> PASS\n'
  else
    printf '  -> FAIL\n'
    failures=$((failures + 1))
  fi
done

if [[ "$DRY_RUN" == true ]]; then
  printf '\nDry run complete.\n'
  exit 0
fi

if [[ $failures -gt 0 ]]; then
  printf '\nci-product completed with %d failing canary(s).\n' "$failures"
  exit 1
fi

printf '\nci-product completed successfully.\n'

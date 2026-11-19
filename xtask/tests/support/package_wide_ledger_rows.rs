//! Exact package-wide commands from the support-tier ledger, cheapest first.

pub const PACKAGE_WIDE_LEDGER_ROWS: &[&str] = &[
    "cargo test -p downstream-demo -- --nocapture",
    "cargo test -p adze-python test_python_language_exists -- --exact --nocapture",
    "cargo test --manifest-path example/Cargo.toml external_word_example::tests::generated_external_grammar_bad_input_matrix_returns_diagnostic_document --features pure-rust -- --exact --nocapture",
    "cargo test -p adze-golden-tests javascript_canary_expression_golden --features javascript-grammar -- --nocapture",
    "cargo test -p adze-cli test_parse_help_documents_available_modes -- --exact --nocapture",
    // Batch 1: glr-core, tablegen, adze substring filters.
    "cargo test -p adze-glr-core conflict -- --nocapture",
    "cargo test -p adze-glr-core ambiguity -- --nocapture",
    "cargo test -p adze-tablegen typed_cst_generator -- --nocapture",
    "cargo test -p adze --features \"pure-rust,glr,runtime-e2e\" glr_ -- --nocapture",
    "cargo test -p adze --features \"pure-rust,glr\" parse_error -- --nocapture",
    // Batch 2: adze external-scanner and incremental rows.
    "cargo test -p adze --features external_scanners",
    "cargo test -p adze --features \"pure-rust,external_scanners\" parser_v4::tests::test_parser_with_external_scanner -- --exact --nocapture",
    "cargo test -p adze --features \"pure-rust,external_scanners\" parser_v4::tests::test_external_scanner_rejects_token_not_in_valid_symbols -- --exact --nocapture",
    "cargo test -p adze --features \"pure-rust,external_scanners\" parser_v4::tests::test_external_scanner_parse_document_bad_input_returns_diagnostic_document -- --exact --nocapture",
    "cargo test -p adze --features incremental_glr reparse_fallback_metadata -- --nocapture",
    // Batch 3: cli quickstarts, init, check, stats.
    "cargo test -p adze-cli readme_stable_claims_are_in_stable_product_lane -- --exact --nocapture",
    "cargo test -p adze-cli readme_arithmetic_quickstart_builds_and_runs -- --exact --nocapture",
    "cargo test -p adze-cli getting_started_quickstart_builds_parses_and_reports_diagnostics -- --exact --nocapture",
    "cargo test -p adze-cli book_quickstart_builds_parses_and_reports_diagnostics -- --exact --nocapture",
    "cargo test -p adze-cli test_init_default_cwd_generates_buildable_project -- --exact --nocapture",
    "cargo test -p adze-cli test_init_generates_buildable_project -- --exact --nocapture",
    "cargo test -p adze-cli test_init_cargo_toml_references_adze_dependency -- --exact --nocapture",
    "cargo test -p adze-cli test_check_rejects_file_without_adze_grammar -- --exact --nocapture",
    "cargo test -p adze-cli test_stats_rejects_file_without_adze_grammar -- --exact --nocapture",
    // Batch 4: cli document-output and dynamic rows.
    "cargo test -p adze-cli test_parse_static_tree_mode_emits_document_backed_tree -- --exact --nocapture",
    "cargo test -p adze-cli test_parse_static_sexp_mode_emits_document_backed_sexp -- --exact --nocapture",
    "cargo test -p adze-cli test_parse_static_json_mode_emits_document_json -- --exact --nocapture",
    "cargo test -p adze-cli test_parse_static_dot_mode_emits_document_backed_graph -- --exact --nocapture",
    "cargo test -p adze-cli test_parse_document_projection_modes_emit_schema_envelopes -- --exact --nocapture",
    "cargo test -p adze-cli parse_document_json_modes_emit_recovery_diagnostics -- --exact --nocapture",
];

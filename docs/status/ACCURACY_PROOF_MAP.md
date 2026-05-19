# 0.9 Accuracy and Proof Coverage Map

**Last updated:** 2026-05-16
**Purpose:** List the behavior that must be accurate for 0.9, the current proof, the missing proof, and the next test to add. This is a correctness work queue, not a feature roadmap.

**Rules:**
- Every row names a concrete behavior claim.
- Every row has a proof command that can be run.
- Gaps are honest. If a test exists, it is listed. If not, the cell says "none."
- No aspirational rows. Only surfaces with public API, docs claims, or support-tier status appear.

**Related:**
- Support tiers and proof commands: [`SUPPORT_TIERS.md`](./SUPPORT_TIERS.md)
- API stability inventory: [`API_STABILITY.md`](./API_STABILITY.md)
- Correctness execution plan: [`CORRECTNESS_PUSH.md`](./CORRECTNESS_PUSH.md)
- Release readiness checklist: [`../../plans/0.9.0/release-readiness.md`](../../plans/0.9.0/release-readiness.md)

---

## Surface 1: parse() and parse_document() agreement

**Claim:** `parse(source)` and `parse_document(source).ast()` produce the same typed AST for exact inputs.

| Aspect | Current proof | Missing | Next test |
|--------|--------------|---------|-----------|
| Same AST from both paths (non-GLR) | `typed_ast_contract_parse_document_ast_matches_parse` | — | — |
| Extraction provenance recorded | `typed_ast_contract_parse_document_ast_records_extraction_provenance` | — | — |
| Determinism (16 repeats) | `typed_ast_contract_repeated_parse_is_deterministic` | — | — |
| Same CST topology (node count, byte ranges) | `document_parse_agreement` non-GLR topology and determinism tests | — | — |
| GLR-path agreement | `document_parse_agreement` GLR topology and AST agreement tests | — | — |
| Bad input: both report diagnostics | Diagnostics agreement covered in typed_ast_contract | — | — |
| Bad input: strict AST extraction refuses recovered doc | `parse_document_recovered_doc_refuses_strict_ast_projection` | — | — |

**Proof commands:**
```bash
cargo test -p adze --features pure-rust --test typed_ast_contract -- --nocapture
cargo test -p adze --features pure-rust --test document_parse_agreement -- --nocapture
cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test test_e2e_ambiguous_grammar_glr -- --nocapture
cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test document_parse_agreement -- --nocapture
```

---

## Surface 2: AdzeDocument boundaries

**Claim:** `AdzeDocument` is the single canonical parse product. All projections read from the same document data.

| Aspect | Current proof | Missing | Next test |
|--------|--------------|---------|-----------|
| Source text, tree, language, diagnostics, metadata | `parse_document_exposes_generic_tree_and_ts_projection_from_same_parse` | — | — |
| Alias-visible identity from native node data | `parse_document_projects_alias_visible_identity_from_native_node_data` | — | — |
| ts_compat projection reads same document | Covered in adze_document_alpha | — | — |
| JSON projection (serialization feature) | `adze_document_json` suite (clean, EOF, multibyte, multiline, GLR fixtures) | — | — |
| `source_slice()` with invalid UTF-8 boundary | `parse_document_source_slice_respects_utf8_boundaries` | — | — |
| `diagnostics_for_node()` with node that has no diagnostics | `parse_document_exposes_generic_tree_and_ts_projection_from_same_parse` | — | — |
| `ambiguities()` non-empty only for GLR | Covered in test_e2e_ambiguous_grammar_glr | — | — |

**Proof commands:**
```bash
cargo test -p adze --features "pure-rust,ts-compat" --test adze_document_alpha -- --nocapture
cargo test -p adze --features "pure-rust,serialization" --test adze_document_json -- --nocapture
```

---

## Surface 3: Edge field metadata

**Claim:** Field metadata lives on edges. `child_by_field_name`, `field_name_for_child`, typed CST accessors, and ts_compat field lookup all read the same edge data.

| Aspect | Current proof | Missing | Next test |
|--------|--------------|---------|-----------|
| Field ID roundtrip (name↔id) | `field_name_to_id_roundtrip_is_lossless`, `field_id_to_name_roundtrip_is_lossless` | — | — |
| Unknown field IDs return None | `unknown_field_ids_return_none` | — | — |
| Unknown field names return None | `unknown_field_names_return_none` | — | — |
| Node field IDs match language field IDs | `node_field_ids_match_language_field_ids` | — | — |
| Typed CST generated accessors agree with generic CST | `generated_parse_document_helper_feeds_generated_syntax_module` | — | — |
| Fielded-struct fields survive Rust expansion → ABI → edge metadata | Covered in typed_cst_generated_document | — | — |
| Empty field map (grammar with zero fields) | `parse_document_empty_field_map_has_no_edge_fields` | — | — |
| Field values on missing/error nodes | `document::tests::field_lookup_resolves_missing_error_child` | — | — |
| Repeated field (multiple children with same field name) | `parse_document_repeated_field_edges_remain_iterable` | — | — |

**Proof commands:**
```bash
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_language_fields -- --nocapture
cargo test -p adze --features pure-rust --test typed_cst_generated_document -- --nocapture
cargo test -p adze --lib --features "pure-rust,ts-compat" document::tests::field_lookup_resolves_missing_error_child -- --exact --nocapture
```

---

## Surface 4: Diagnostics — expected-token normalization

**Claim:** Parse diagnostics report deduplicated, stable, human-readable expected tokens with accurate byte/point spans.

| Aspect | Current proof | Missing | Next test |
|--------|--------------|---------|-----------|
| Expected tokens populated | `generated_typed_parser_unexpected_eof_lists_expected_tokens` | — | — |
| Expected tokens are human-readable (no SymbolId) | Covered in generated_parse_errors + error_display_tests | — | — |
| Expected tokens sorted and deduped | `generated_typed_parser_unexpected_eof_expected_field_sorted_and_deduped` | — | — |
| Byte span accuracy (ASCII, multibyte, EOF, multiline) | Covered in generated_parse_errors | — | — |
| Source excerpt with caret marker | Covered in error_display_tests | — | — |
| No internal symbol leaks in Display | Covered in error_display_tests | — | — |
| Byte↔point span consistency | `generated_parse_document_diagnostics_byte_and_point_ranges_agree` | — | — |
| Multi-error deduplication | `generated_parser_multi_error_diagnostics_are_not_duplicated` | — | — |
| Diagnostic ordering | `generated_parser_multi_error_diagnostics_are_ordered` | — | — |

**Proof commands:**
```bash
cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors -- --nocapture
cargo test -p adze --features "pure-rust,glr" --test error_display_tests -- --nocapture
```

---

## Surface 5: UTF-8 and zero-width span edge cases

**Claim:** Multibyte input and zero-width EOF recovery spans are correct.

| Aspect | Current proof | Missing | Next test |
|--------|--------------|---------|-----------|
| Multibyte byte span | `generated_typed_parser_multibyte_bad_token_reports_utf8_byte_span` | — | — |
| Multibyte line/column | `generated_parse_document_diagnostics_include_multibyte_byte_span` | — | — |
| EOF zero-width span | `generated_typed_parser_unexpected_eof_reports_zero_width_source_span` | — | — |
| Multiline point range | `generated_parse_document_diagnostics_include_multiline_point_range` | — | — |
| Source excerpt alignment with multibyte | Covered in error_display_tests | — | — |
| Line/column rendering at file boundary | `generated_typed_parser_unexpected_eof_after_newline_reports_file_boundary_location` | — | — |
| Mixed ASCII/multibyte line counting | `generated_object_like_parser_counts_mixed_ascii_multibyte_lines` | — | — |

**Proof commands:**
```bash
cargo test -p adze --features pure-rust --test generated_parse_errors -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_node_error -- --nocapture
```

---

## Surface 6: ts_compat adapter identity

**Claim:** `ts_compat` is an adapter over document facts, not a second source of truth. It projects alias-visible identity correctly.

| Aspect | Current proof | Missing | Next test |
|--------|--------------|---------|-----------|
| Visible kind vs grammar kind distinct | `alias_visible_kind_and_grammar_identity_are_distinct` | — | — |
| Anonymous alias controls named-child filtering | `anonymous_alias_controls_named_child_filtering` | — | — |
| S-expression uses alias-visible identity | `alias_visible_identity_is_used_in_sexp` | — | — |
| Field labels in S-expression | `to_sexp_includes_field_labels_for_named_children` | — | — |
| node-types.json projection | `ts_compat_node_types` suite | — | — |
| Error/MISSING nodes in S-expression | `to_sexp_includes_missing_nodes_for_recovered_input`; `ts_compat::tests::node_to_sexp_renders_error_and_missing_nodes` | — | — |
| Nested aliases (alias of alias) | `nested_alias_visible_identity_is_used_in_sexp`; `nested_aliases_preserve_visible_and_grammar_identity` | — | — |
| Supertype alias behavior | `supertype_alias_preserves_visible_identity_and_metadata` | — | — |

**Proof commands:**
```bash
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_node_metadata -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_to_sexp -- --nocapture
cargo test -p adze --lib --features "pure-rust,ts-compat" ts_compat::tests::node_to_sexp_renders_error_and_missing_nodes -- --exact --nocapture
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_node_types -- --nocapture
```

---

## Surface 7: Ambiguity summary determinism

**Claim:** GLR ambiguity summary is deterministic: same input → same selected tree, same reason, same alternative count.

| Aspect | Current proof | Missing | Next test |
|--------|--------------|---------|-----------|
| Selection is deterministic (2 parses) | `generated_ambiguous_expr_multi_conflict_selection_is_deterministic` | — | — |
| Ambiguity summary populated | `generated_ambiguous_expr_parse_document_reports_ambiguity_summary` | — | — |
| AST from selected tree matches summary | `generated_ambiguous_expr_parse_document_ast_matches_selected_parse` | — | — |
| Multiple alternatives retained | `generated_ambiguous_expr_glr_runtime_retains_multiple_complete_alternatives` | — | — |
| Selection reason present and stable | `SelectionReason::StableStructuralTieBreak` asserted | — | — |
| CST-level determinism (not just AST) | `generated_ambiguous_expr_parse_document_cst_topology_is_deterministic` | — | — |
| Fork count stability | `generated_ambiguous_expr_runtime_fork_count_is_deterministic` | — | — |
| Larger ambiguity (3+ alternatives) | `generated_ambiguous_expr_glr_runtime_retains_three_or_more_complete_alternatives` | — | — |

**Proof commands:**
```bash
cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test test_e2e_ambiguous_grammar_glr -- --nocapture
cargo test -p adze --features "pure-rust,glr,glr_telemetry,runtime-e2e" --test test_e2e_ambiguous_grammar_glr generated_ambiguous_expr_runtime_fork_count_is_deterministic -- --exact --nocapture
```

---

## Surface 8: JSON projection

**Claim:** `AdzeDocument::to_json_value()` emits schema-versioned facts that agree with native projections.

| Aspect | Current proof | Missing | Next test |
|--------|--------------|---------|-----------|
| Clean document JSON | `adze_document_json` clean fixture | — | — |
| EOF diagnostic JSON | `adze_document_json` EOF fixture | — | — |
| Multibyte diagnostic JSON | `adze_document_json` multibyte fixture | — | — |
| Multiline diagnostic JSON | `adze_document_json` multiline fixture | — | — |
| GLR ambiguity JSON | `parse_document_json_serializes_glr_ambiguity_summary` | — | — |
| Schema snapshot stability | `adze_document_json_schema_identifier_is_pinned`; snapshots exist for each fixture | — | — |
| JSON ↔ native field parity | Clean fixture cross-checks edge/field/child data; `parse_document_json_diagnostic_fields_match_native_diagnostic` | — | — |

**Proof commands:**
```bash
cargo test -p adze --features "pure-rust,serialization" --test adze_document_json -- --nocapture
cargo test -p adze --features "pure-rust,serialization,glr" --test adze_document_json parse_document_json_serializes_glr_ambiguity_summary -- --exact --nocapture
```

---

## Surface 9: CLI output truth

**Claim:** CLI commands produce correct, documented output.

| Aspect | Current proof | Missing | Next test |
|--------|--------------|---------|-----------|
| `adze init` generates buildable project | `test_init_generates_buildable_project` | — | — |
| README quickstart builds and runs | `readme_arithmetic_quickstart_builds_and_runs` | — | — |
| `adze check` rejects non-grammar file | `test_check_rejects_file_without_adze_grammar` | — | — |
| `adze stats` rejects non-grammar file | `test_stats_rejects_file_without_adze_grammar` | — | — |
| `adze parse` documents available modes | `test_parse_help_documents_available_modes` | — | — |
| `adze parse --output document-json/tree-json/diagnostics-json/ambiguity-json` output | `test_parse_document_projection_modes_emit_schema_envelopes`; `parse_document_json_modes_emit_recovery_diagnostics` | — | — |
| `adze check` with broken grammar | `test_check_reports_invalid_grammar_syntax` | — | — |
| Missing grammar path handling | `test_check_reports_missing_grammar_path` | — | — |

**Proof commands:**
```bash
cargo test -p adze-cli -- --nocapture
```

---

## Surface 10: Benchmarks truth

**Claim:** Benchmarks measure real parser work, not infrastructure overhead.

| Aspect | Current proof | Missing | Next test |
|--------|--------------|---------|-----------|
| Fixtures parse correctly | `verify_arithmetic_benchmark_fixtures_parse_with_arithmetic_grammar` | — | — |
| parse_bench uses real parser | `verify_parse_bench_uses_real_parser_workload` | — | — |
| Duplicate bench is deprecated | `verify_duplicate_glr_performance_bench_is_deprecated` | — | — |
| Benchmark inventory is classified | `benchmarks/Cargo.toml` metadata and `benchmarks/README.md` inventory | — | — |

**Proof commands:**
```bash
cargo test -p adze-benchmarks --test verify_fixture_parsing -- --nocapture
cargo bench -p adze-benchmarks --no-run
```

---

## Surface 11: Package publishability

**Claim:** Core crates are publishable to crates.io with correct metadata.

| Aspect | Current proof | Missing | Next test |
|--------|--------------|---------|-----------|
| Core crates have correct metadata | `cargo metadata --format-version 1` succeeds | — | — |
| Local package verification for co-release siblings | `scripts/package-local-release.sh adze`; `scripts/package-local-release.sh adze-tool` | — | — |
| Direct package verification for independent publishable crates | `cargo package -p adze-macro --allow-dirty`; `cargo package -p adze-cli --allow-dirty`; `cargo package -p adze-ir --allow-dirty`; `cargo package -p adze-glr-core --allow-dirty`; `cargo package -p adze-tablegen --allow-dirty`; `cargo package -p adze-common --allow-dirty`; `cargo package -p adze-common-type-ops-core --allow-dirty` | — | — |
| Publishability recipe exists | `just check-publishable` | — | — |

**Proof commands:**
```bash
cargo metadata --format-version 1 --locked
scripts/package-local-release.sh adze
scripts/package-local-release.sh adze-tool
cargo package -p adze-macro --allow-dirty
cargo package -p adze-cli --allow-dirty
cargo package -p adze-ir --allow-dirty
cargo package -p adze-glr-core --allow-dirty
cargo package -p adze-tablegen --allow-dirty
cargo package -p adze-common --allow-dirty
just check-publishable
```

---

## Summary: gap count by surface

| Surface | Tested aspects | Named gaps | Next canary priority |
|---------|---------------|------------|---------------------|
| parse() / parse_document() agreement | 7 | 0 | — |
| AdzeDocument boundaries | 7 | 0 | — |
| Edge field metadata | 9 | 0 | — |
| Diagnostics normalization | 9 | 0 | — |
| UTF-8 / zero-width spans | 7 | 0 | — |
| ts_compat adapter identity | 8 | 0 | — |
| Ambiguity determinism | 8 | 0 | — |
| JSON projection | 7 | 0 | — |
| CLI output truth | 8 | 0 | — |
| Benchmarks truth | 4 | 0 | — |
| Package publishability | 4 | 0 | — |
| **Total** | **78** | **0** | — |

---

## Recommended test PR sequence

Each PR adds focused proof for one surface gap. Test-only PRs are preferred; production code changes stay limited to gaps where the claimed receipt is not wired honestly yet.

1. **test(document): prove parse and parse_document agree** — GLR-path agreement and CST topology comparison
2. **test(document): prove recovered-doc AST refusal** — strict AST extraction refuses recovered diagnostic documents through document AST entry points
3. **test(document): cover document boundary canaries** — UTF-8 `source_slice()` boundaries and empty clean-node diagnostics
4. **test(document): prove edge field metadata invariants** — empty field map and repeated field iteration
5. **test(diagnostics): prove expected-token normalization** — byte↔point span agreement, multi-error dedup, diagnostic ordering
6. **test(diagnostics): cover UTF-8 and EOF recovery spans** — EOF boundary, mixed ASCII/multibyte line counting
7. **test(ts-compat): prove adapter identity and alias behavior** — complete
8. **test(glr): prove ambiguity summary determinism** — complete
9. **docs: audit README claims against proof map** — no new tests, verify existing proof commands still pass
10. **benchmarks: classify benchmark inventory** — complete
11. **release: audit publishable package metadata** — complete
12. **release: add 0.9 readiness receipt** — complete

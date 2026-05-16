# 0.9 Accuracy and Proof Coverage Map

**Last updated:** 2026-05-15
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
| Bad input: strict AST extraction refuses recovered doc | none | No test that `parse_document().ast()` on recovered input returns error or refuses extraction by default | Add test: truncated source → document with diagnostics → `ast()` returns error |

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
| `source_slice()` with invalid UTF-8 boundary | none | No test for boundary cases on `source_slice()` | Add test: slice at non-char boundary |
| `diagnostics_for_node()` with node that has no diagnostics | none | No test for empty diagnostic list per node | Add test: clean parse → `diagnostics_for_node(root) == []` |
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
| Empty field map (grammar with zero fields) | none | No test for grammar with no fields | Add canary: zero-field grammar → `field_count() == 0`, all lookups return None |
| Field values on missing/error nodes | none | No test for field lookup when child is ERROR or MISSING | Add test: error node → `child_by_field_name()` behavior |
| Repeated field (multiple children with same field name) | none | No test for repeated field iteration | Add test: grammar with repeat field → iterator returns all children |

**Proof commands:**
```bash
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_language_fields -- --nocapture
cargo test -p adze --features pure-rust --test typed_cst_generated_document -- --nocapture
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
| Byte↔point span consistency | none | No test asserts byte range and point range describe the same position | Add test: assert `byte_span().start` maps to `point_range().start` via source text |
| Multi-error deduplication | none | No test that the same diagnostic doesn't appear twice for one parse | Add test: grammar that triggers same error at same position → single diagnostic |
| Diagnostic ordering | none | No test that diagnostics are sorted by byte position | Add test: multiple errors → diagnostics ordered by start byte |

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
| Line/column rendering at file boundary | none | No test for error at very last byte of file | Add test: error at EOF → correct line/col |
| Mixed ASCII/multibyte line counting | none | No test for line count accuracy with multibyte newlines | Add test: CJK text with embedded newlines |

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
| Error/MISSING nodes in S-expression | none | No test for how ERROR and MISSING nodes render in S-expression | Add test: bad input → S-expression contains `(ERROR)` / `(MISSING)` |
| Nested aliases (alias of alias) | none | No test for chained alias sequences | Add test: double-aliased production → `kind()` returns outer alias |
| Supertype alias behavior | none | No test for supertype-style aliases | Add test when grammar supports supertypes |

**Proof commands:**
```bash
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_node_metadata -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_to_sexp -- --nocapture
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
| CST-level determinism (not just AST) | none | No test comparing raw CST topology across repeated GLR parses | Add test: parse twice → compare tree node IDs, edge counts |
| Fork count stability | none | No test that fork count is stable across runs | Add test: parse twice → same fork count |
| Larger ambiguity (3+ alternatives) | none | No test for input producing 3+ structurally distinct parse trees | Add test: grammar with 3-way ambiguity |

**Proof commands:**
```bash
cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test test_e2e_ambiguous_grammar_glr -- --nocapture
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
| Schema snapshot stability | Snapshots exist for each fixture | Snapshot drift across versions | Add schema version pin test: assert `schema_version` field matches expected value |
| JSON ↔ native field parity | Clean fixture cross-checks edge/field/child data | No cross-check on diagnostic fixtures | Add test: diagnostic fixture JSON fields match native `ParseDiagnostic` values |

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
| `adze parse --mode document` output | none | No test for document-JSON parse output mode | Add test: `adze parse --mode document` → valid JSON with document envelope |
| `adze check` with broken grammar | none | No test for check output on intentionally invalid grammar | Add test: syntax error in grammar → check reports error |
| Error handling (bad path, permission) | none | No CLI error-path tests | Add test: nonexistent file → graceful error message |

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
| Duplicate benches documented | `glr_performance.rs` self-documents duplication | No deprecation or removal | Mark `glr_performance.rs` as superseded by `glr_performance_real.rs` |
| Error-recovery benchmark | none | No benchmark for error recovery throughput | Add benchmark: bad-input parsing throughput |
| Real-language benchmark | none | No benchmark for Python/JS grammars | Add benchmark when grammar support matures |
| Serialization benchmark | none | No benchmark for JSON/S-expression output throughput | Add benchmark when serialization surface stabilizes |

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
| `cargo package` succeeds (adze) | none | No CI step for `cargo package -p adze --allow-dirty` | Add CI check or local recipe: `just check-publishable` |
| `cargo package` succeeds (all core crates) | none | Only checked manually or ad-hoc | Add recipe: iterate core crates, run `cargo package --allow-dirty` |

**Proof commands:**
```bash
cargo metadata --format-version 1 --locked
# Per crate:
cargo package -p adze --allow-dirty
cargo package -p adze-macro --allow-dirty
cargo package -p adze-tool --allow-dirty
cargo package -p adze-cli --allow-dirty
cargo package -p adze-ir --allow-dirty
cargo package -p adze-glr-core --allow-dirty
cargo package -p adze-tablegen --allow-dirty
cargo package -p adze-common --allow-dirty
```

---

## Summary: gap count by surface

| Surface | Tested aspects | Named gaps | Next canary priority |
|---------|---------------|------------|---------------------|
| parse() / parse_document() agreement | 6 | 1 | Recovered-doc strict AST extraction refusal |
| AdzeDocument boundaries | 4 | 2 | `source_slice()` boundary test |
| Edge field metadata | 6 | 3 | Empty field map canary |
| Diagnostics normalization | 7 | 3 | Byte↔point span agreement |
| UTF-8 / zero-width spans | 5 | 2 | EOF boundary error test |
| ts_compat adapter identity | 6 | 3 | ERROR/MISSING in S-expression |
| Ambiguity determinism | 5 | 3 | CST-level determinism test |
| JSON projection | 5 | 2 | Schema version pin test |
| CLI output truth | 5 | 3 | `adze parse --mode document` test |
| Benchmarks truth | 2 | 4 | Deprecate duplicate bench |
| Package publishability | 1 | 2 | `just check-publishable` recipe |
| **Total** | **52** | **28** | — |

---

## Recommended test PR sequence

Each PR adds focused tests for one surface gap. No code changes to production crates.

1. **test(document): prove parse and parse_document agree** — GLR-path agreement and CST topology comparison
2. **test(document): prove recovered-doc AST refusal** — strict AST extraction refuses recovered diagnostic documents
3. **test(document): prove edge field metadata invariants** — empty field map, missing/error node fields, repeated field iteration
4. **test(diagnostics): prove expected-token normalization** — byte↔point span agreement, multi-error dedup, diagnostic ordering
5. **test(diagnostics): cover UTF-8 and EOF recovery spans** — EOF boundary, mixed ASCII/multibyte line counting
6. **test(ts-compat): prove adapter identity and alias behavior** — ERROR/MISSING S-expression, nested aliases
7. **test(glr): prove ambiguity summary determinism** — CST-level determinism, fork count stability
8. **docs: audit README claims against proof map** — no new tests, verify existing proof commands still pass
9. **benchmarks: classify benchmark inventory** — mark duplicates, document what each bench measures
10. **release: audit publishable package metadata** — add `just check-publishable` recipe
11. **release: add 0.9 readiness receipt** — update this map with final gap status

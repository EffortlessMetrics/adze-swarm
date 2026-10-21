# Generated Grammar Feature Matrix

Status: active
Owner: runtime/product
Created: 2026-10-21
Linked issue: #876
Support-tier map: ../status/SUPPORT_TIERS.md

This matrix answers one user question: "Which Rust type and annotation shapes
can I rely on in a generated grammar, and what evidence proves each one?" It
is not a support-tier promotion by itself. Support tiers in
`docs/status/SUPPORT_TIERS.md` remain the sole source of truth for public
stability claims; rows below link to that file instead of restating tiers.

## Rules

- A row is `Proved` only when a checked-in test asserts the exact typed
  AST/CST value, not merely parse success.
- Unproved constructs stay visible as `Gap` or `Blocked` rows. Do not mark an
  unproved construct supported, and do not adjust snapshots to make a failing
  construct look proved (issue #876 rollback rule).
- Adding a future supported annotation requires adding or updating a matrix
  row and its proof command.
- Proof for every `Proved` row in PR-1 is:
  `cargo test -p adze --features pure-rust --test generated_grammar_matrix -- --nocapture`

## Proved rows (PR-1)

| Construct | Fixture | Expected typed shape | Engine | Status |
| --- | --- | --- | --- | --- |
| Regex leaf + transform closure (`\d+` -> `i32`) | `example/src/typed_ast_contract.rs` | `parse("42") == Expr::Number(42)` | pure-rust | Proved (`test_regex_leaf_transform_valid_number_exact_value`) |
| Tuple enum variant + `Box<T>` recursion + `#[adze::prec_left(1)]` | `example/src/typed_ast_contract.rs` | `1 + 2 + 3` parses as left-nested `Add(Add(1, 2), 3)` | pure-rust | Proved (`test_tuple_variant_box_recursion_prec_left_exact_shape`) |
| Extras (whitespace) skipped around tokens | `example/src/typed_ast_contract.rs` | `"  1   +   2  "` equals the unspaced `Add(1, 2)` value | pure-rust | Proved (`test_extras_whitespace_skipped_exact_value`) |
| Named-field struct variants + two precedence levels | `example/src/fielded_precedence_typed_cst_contract.rs` | `1 + 2 * 3` parses as `Add { left: 1, right: Mul { left: 2, right: 3 } }` | pure-rust | Proved (`test_named_fields_precedence_levels_exact_shape`) |
| `parse()` / `parse_document().ast()` agreement | `example/src/fielded_precedence_typed_cst_contract.rs` | Document-extracted AST equals the typed `parse()` value for `1 + 2 * 3` | pure-rust | Proved (`test_parse_and_document_when_clean_input_ast_agrees`) |

## Gap and blocked rows (not supported claims)

| Construct | Status | Evidence / blocker |
| --- | --- | --- |
| Delimited non-empty repetition (CSV list) | Gap | `example/src/csv_list.rs` tests assert only `is_ok() \|\| is_err()`; the fixture tests accept either outcome and no checked-in test asserts an exact multi-item value, so no exact-value proof exists. |
| `Option<T>` and optional fields | Gap | `example/src/optionals.rs` exists; no exact-value matrix proof yet. Cardinality agreement across typed AST, typed CST, and node schema is unproved. |
| Undelimited `Vec<T>` repetition | Gap | `example/src/repetitions.rs` exists; no exact-value matrix proof yet. |
| Keyword-versus-identifier (word tokens) | Gap | `example/src/words.rs` exists; keyword/identifier interaction is not proved by exact values here. |
| Fields/aliases cardinality in node schema | Blocked by #863 | Semantic schema work in #863 is open; `node_types`-style cardinality is not yet derived from the grammar contract. |
| Numeric transform overflow | Gap | The proved leaf transform unwraps its `i32` conversion, so digit strings beyond `i32` range panic instead of producing a value or a diagnostic; no overflow proof exists. |
| Fallible transforms | Blocked by #864 | Fallible-transform work in #864 is open; only unwrapping transform closures are proved above. |
| Invalid annotation diagnostics | Gap | The trybuild-style proof is not wired as a blocking harness; invalid combinations do not yet have bounded compile-diagnostic proof in this matrix. |
| External tokens/scanners | Gap (honest tier in SUPPORT_TIERS) | Kept at the tier stated in `docs/status/SUPPORT_TIERS.md`; not part of the generated-grammar contract proved here. |
| Hidden/inline rules, supertypes, empty/nullable productions | Gap | No exact-value proof in this PR; rows stay visible until fixtures prove them. |

## Follow-up rule

Each subsequent slice moves exactly one `Gap` row to `Proved` by adding an
exact-value test to `runtime/tests/generated_grammar_matrix.rs` (or a named
sibling harness) and updating this table in the same PR. Issue #876 stays
open until every selected 0.10.0 row is either `Proved` or explicitly
excluded with a named blocker.

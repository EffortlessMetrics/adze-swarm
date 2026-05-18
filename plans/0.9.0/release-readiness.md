# 0.9.0 Release Readiness

**Last updated:** 2026-05-16
**Status:** Ready for release closeout — all tracked items covered

This document tracks the release-readiness checklist for Adze 0.9.0.
It is a receipt, not a plan: each item must have a proof command that passes before the item is marked complete.

**Source of truth for gaps:** [`docs/status/ACCURACY_PROOF_MAP.md`](../../docs/status/ACCURACY_PROOF_MAP.md)

---

## Release criteria

A 0.9.0 release is ready when all of the following are true:

1. All **Stable** and **Stabilizing** support-tier surfaces have named proof commands that pass.
2. No **Stable** claim in README lacks a proof command in `SUPPORT_TIERS.md`.
3. The accuracy proof map has zero "Missing" entries for Stable surfaces.
4. All publishable crates pass package verification. Crates that depend on unpublished co-release siblings use `scripts/package-local-release.sh <crate>` so package verification compiles against the local release surface instead of older crates.io versions.
5. Benchmarks are classified (real / synthetic / placeholder / duplicate).
6. `just ci-supported` passes on `main`.
7. `just ci-product-stable` passes on `main`.

As of PR #97, every tracked checklist item below is covered and
`docs/status/ACCURACY_PROOF_MAP.md` has zero named gaps. Tagging and publishing
remain explicit release operations and should be performed from the release
surface after a fresh final proof run.

---

## Checklist

### Product proof (accuracy map gaps)

| # | Item | Status | Proof command | PR |
|---|------|--------|---------------|----|
| 1 | parse() / parse_document() GLR-path agreement | covered | `cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test document_parse_agreement -- --nocapture` | #26 |
| 2 | parse() / parse_document() CST topology comparison | covered | `cargo test -p adze --features pure-rust --test document_parse_agreement -- --nocapture` | #26 |
| 3 | Recovered document refuses strict AST extraction | covered | `cargo test -p adze --features pure-rust --test document_parse_agreement -- --nocapture` | #27 |
| 4 | AdzeDocument source_slice() and empty node diagnostics | covered | `cargo test -p adze --features "pure-rust,ts-compat" --test adze_document_alpha -- --nocapture` | #28 |
| 5 | Empty field map canary | covered | `cargo test -p adze --features "pure-rust,ts-compat" --test adze_document_alpha -- --nocapture` | #29 |
| 6 | Field lookup on error/missing nodes | covered | `cargo test -p adze --lib --features "pure-rust,ts-compat" document::tests::field_lookup_resolves_missing_error_child -- --exact --nocapture` | #52 |
| 7 | Repeated field iteration | covered | `cargo test -p adze --features "pure-rust,ts-compat" --test adze_document_alpha -- --nocapture` | #29 |
| 8 | Byte↔point span agreement | covered | `cargo test -p adze --features pure-rust --test typed_cst_generated_document -- --nocapture` | #30 |
| 9 | Multi-error deduplication | covered | `cargo test -p adze --features pure-rust --test generated_parse_errors -- --nocapture` | #31 |
| 10 | Diagnostic ordering by position | covered | `cargo test -p adze --features pure-rust --test generated_parse_errors -- --nocapture` | #31 |
| 11 | EOF boundary error span | covered | `cargo test -p adze --features pure-rust --test generated_parse_errors -- --nocapture` | #36 |
| 12 | Mixed ASCII/multibyte line counting | covered | `cargo test -p adze --features pure-rust --test generated_parse_errors -- --nocapture` | #36 |
| 13 | ERROR/MISSING nodes in S-expression | covered | `cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_to_sexp -- --nocapture`; `cargo test -p adze --lib --features "pure-rust,ts-compat" ts_compat::tests::node_to_sexp_renders_error_and_missing_nodes -- --exact --nocapture` | #37 |
| 14 | Nested alias behavior | covered | `cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_to_sexp nested_alias_visible_identity_is_used_in_sexp -- --exact --nocapture`; `cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_node_metadata nested_aliases_preserve_visible_and_grammar_identity -- --exact --nocapture` | #46 |
| 15 | CST-level GLR determinism | covered | `cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test test_e2e_ambiguous_grammar_glr generated_ambiguous_expr_parse_document_cst_topology_is_deterministic -- --exact --nocapture` | #48 |
| 16 | Fork count stability | covered | `cargo test -p adze --features "pure-rust,glr,glr_telemetry,runtime-e2e" --test test_e2e_ambiguous_grammar_glr generated_ambiguous_expr_runtime_fork_count_is_deterministic -- --exact --nocapture` | #49 |
| 17 | Schema version pin for JSON | covered | `cargo test -p adze --features "pure-rust,serialization" --test adze_document_json adze_document_json_schema_identifier_is_pinned -- --exact --nocapture` | #50 |
| 18 | CLI `parse --output document-json/tree-json/diagnostics-json/ambiguity-json` output | covered | `cargo test -p adze-cli test_parse_document_projection_modes_emit_schema_envelopes -- --exact --nocapture` | #95, #103 |

### Release quality

| # | Item | Status | Proof command | PR |
|---|------|--------|---------------|----|
| 19 | README claims audit against proof map | covered | `cargo test -p adze-cli readme_stable_claims_are_in_stable_product_lane -- --exact --nocapture` | #55 |
| 20 | Benchmark classification inventory | covered | `cargo test -p adze-benchmarks --test verify_fixture_parsing -- --nocapture` | #61 |
| 21 | Duplicate bench removal (`glr_performance.rs`) | covered | `cargo check -p adze-benchmarks --benches`; `cargo test -p adze-benchmarks --test verify_fixture_parsing -- --nocapture` | #62 |
| 22 | Package publishability — adze | covered | `scripts/package-local-release.sh adze` | #93 |
| 23 | Package publishability — adze-macro | covered | `cargo package -p adze-macro --allow-dirty` | #58 |
| 24 | Package publishability — adze-tool | covered | `scripts/package-local-release.sh adze-tool` | #93 |
| 25 | Package publishability — adze-cli | covered | `cargo package -p adze-cli --allow-dirty` | #90 |
| 26 | Package publishability — adze-ir | covered | `cargo package -p adze-ir --allow-dirty` | #57 |
| 27 | Package publishability — adze-glr-core | covered | `cargo package -p adze-glr-core --allow-dirty` | #59 |
| 28 | Package publishability — adze-tablegen | covered | `cargo package -p adze-tablegen --allow-dirty` | #58 |
| 29 | Package publishability — adze-common | covered | `cargo package -p adze-common --allow-dirty` | #57 |
| 30 | Package publishability — adze-common-type-ops-core | covered | `cargo package -p adze-common-type-ops-core --allow-dirty` | #104 |
| 31 | `just check-publishable` recipe exists | covered | `just check-publishable` | #56 |

Current package blockers:

- None for the tracked 0.9 release-readiness package checks. `adze` and `adze-tool` require `scripts/package-local-release.sh <crate>` until their matching co-release siblings are published.

### CI gates

| # | Item | Status | Proof command | PR |
|---|------|--------|---------------|----|
| 32 | `just ci-supported` passes | covered | `just ci-supported` | #94 |
| 33 | `just ci-product-stable` passes | covered | `just ci-product-stable` | #91 |
| 34 | Workspace formatting passes | covered | `just fmt` | #92 |
| 35 | `just clippy` passes | covered | `just clippy` | #70 |

---

## Blocking definition

An item is **blocking** if:
- It covers a Stable support-tier surface with no existing proof.
- It covers a README claim with no proof command.
- It exposes a correctness gap that would mislead users if released.

An item is **advisory** if:
- It covers Experimental or Advisory surfaces.
- It improves quality signal but does not affect correctness of Stable claims.

Items 1–18 are blocking for the surfaces they cover. Items 19–35 are release quality, not correctness blockers — but all must pass before `0.9.0` is tagged.

---

## Recommended PR sequence

```
1. docs(status): add 0.9 accuracy and coverage map          ← this document
2. test(document): prove parse and parse_document agree      ← items 1-2
3. test(document): prove recovered-doc AST refusal           ← item 3
4. test(document): cover document boundary canaries          ← item 4
5. test(document): prove edge field metadata invariants      ← items 5 and 7
6. test(diagnostics): prove expected-token normalization     ← items 8-10
7. test(diagnostics): cover UTF-8 and EOF recovery spans     ← items 11-12
8. test(ts-compat): prove adapter identity and alias         ← items 13-14
9. test(glr): prove ambiguity summary determinism            ← items 15-16
10. docs: audit README claims against proof map              ← item 19
11. benchmarks: classify benchmark inventory                 ← items 20-21
12. release: audit publishable package metadata              ← items 22-30
13. release: update this receipt, tag 0.9.0              ← receipt complete; tag/publish remains explicit release operation
```

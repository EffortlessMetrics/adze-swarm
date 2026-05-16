# 0.9 API Foundation Implementation Plan

Status: complete
Owner: runtime/api
Created: 2026-05-13
Completed: 2026-05-16
Linked proposal: ../../docs/proposals/ADZE-PROP-0002-api-foundation.md
Linked specs:
- ../../docs/specs/ADZE-SPEC-0003-canonical-parse-document.md
- ../../docs/specs/ADZE-SPEC-0004-typed-cst-and-ast-projections.md
- ../../docs/specs/ADZE-SPEC-0005-diagnostics-and-recovery.md
- ../../docs/specs/ADZE-SPEC-0006-tree-sitter-compatibility-adapter.md
- ../../docs/specs/ADZE-SPEC-0007-glr-ambiguity-summary.md
- ../../docs/specs/ADZE-SPEC-0008-json-cli-wasm-projections.md
- ../../docs/specs/ADZE-SPEC-0009-incremental-document-lifecycle.md
- ../../docs/specs/ADZE-SPEC-0010-language-metadata-and-node-types.md
Linked ADRs:
- ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
- ../../docs/adr/ADZE-ADR-0003-summary-first-glr-ambiguity.md
- ../../docs/adr/ADZE-ADR-0004-schema-versioned-projections.md
Active goal: ../../.adze/goals/active.toml
Support-tier map: ../../docs/status/SUPPORT_TIERS.md
Closeout: ./closeout.md

## Goal

Make `AdzeDocument` the native parse-product boundary and move typed CST, typed
AST, diagnostics, Tree-sitter compatibility, GLR ambiguity, JSON, CLI, and WASM
output toward projections over one parse truth.

This plan sequences implementation. It does not promote support tiers by
itself; `../../docs/status/SUPPORT_TIERS.md` owns product claims and proof.

The planned 0.9 API-foundation implementation slices are complete. Future API
work should open a new plan or active goal rather than reusing this closed work
queue.

## Work Item: api-foundation-spec-stack

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0002-api-foundation.md
Linked specs:
- ../../docs/specs/ADZE-SPEC-0003-canonical-parse-document.md
- ../../docs/specs/ADZE-SPEC-0004-typed-cst-and-ast-projections.md
- ../../docs/specs/ADZE-SPEC-0005-diagnostics-and-recovery.md
- ../../docs/specs/ADZE-SPEC-0006-tree-sitter-compatibility-adapter.md
- ../../docs/specs/ADZE-SPEC-0007-glr-ambiguity-summary.md
- ../../docs/specs/ADZE-SPEC-0008-json-cli-wasm-projections.md
- ../../docs/specs/ADZE-SPEC-0009-incremental-document-lifecycle.md
- ../../docs/specs/ADZE-SPEC-0010-language-metadata-and-node-types.md
Linked ADRs:
- ../../docs/adr/ADZE-ADR-0003-summary-first-glr-ambiguity.md
- ../../docs/adr/ADZE-ADR-0004-schema-versioned-projections.md

### Goal

Encode the API design as source-of-truth docs before runtime changes.

### Production Delta

Add proposal, behavior specs, ADRs, policy ledger entries, and this
implementation plan.

### Non-Goals

No runtime code, support-tier promotion, JSON schema implementation, CLI output,
or WASM binding changes.

### Acceptance

The repo contains linked artifacts for the API foundation and future agents can
identify the next implementation PRs from this plan and `.adze/goals/active.toml`.

### Proof Commands

```bash
python -c "import tomllib; tomllib.load(open('policy/doc-artifacts.toml', 'rb')); tomllib.load(open('.adze/goals/active.toml', 'rb'))"
git diff --check
```

### Rollback

Revert the docs/policy PR. No runtime behavior changes need rollback.

## Work Item: document-model-alpha-v2

Status: complete
Linked spec: ../../docs/specs/ADZE-SPEC-0003-canonical-parse-document.md
Blocked by: none
PR: #765

### Goal

Tighten the target `AdzeDocument` data model without removing existing parser
paths.

### Production Delta

Add or refine direct document model types for source, selected tree, nodes,
edges, diagnostics, metadata, provenance, and language schema.

### Non-Goals

No broad parser rewrite, no stable public claim, and no removal of transitional
parse-node paths.

### Proof Commands

```bash
cargo test -p adze --features "pure-rust,ts-compat" --test adze_document_alpha -- --nocapture
cargo clippy -p adze --features "pure-rust,ts-compat" --all-targets -- -D warnings
git diff --check
```

### Rollback

Remove the new document model changes and tests.

## Work Item: pure-parser-document-bridge

Status: complete
Linked spec: ../../docs/specs/ADZE-SPEC-0003-canonical-parse-document.md
Blocked by: none
PR: #766

### Goal

Build `AdzeDocument` from existing pure parser results while preserving node
IDs, ranges, parent/edge relationships, fields, flags, diagnostics, and
metadata.

### Proof Commands

```bash
cargo test -p adze --features "pure-rust,ts-compat" --test adze_document_alpha -- --nocapture
cargo test -p adze --features pure-rust --test typed_cst_generated_document generated_parse_document_bridge_populates_direct_node_edge_records -- --exact --nocapture
cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors -- --nocapture
git diff --check
```

## Work Item: generated-parse-document-canonical

Status: complete
Linked spec: ../../docs/specs/ADZE-SPEC-0003-canonical-parse-document.md
Blocked by: none
PR: #767

### Goal

Generated grammars expose `parse_document()` returning document-shaped output
backed by the canonical document.

### Proof Commands

```bash
cargo test -p adze --features pure-rust --test typed_ast_contract -- --nocapture
cargo test -p adze --features pure-rust --test typed_cst_generated_document -- --nocapture
git diff --check
```

## Work Item: parse-fast-path-delegates-to-document

Status: complete
Linked spec: ../../docs/specs/ADZE-SPEC-0004-typed-cst-and-ast-projections.md
Blocked by: none
PR: #768

### Goal

Prove `grammar::parse(source)` returns the same typed AST as the document-backed
`doc.ast()` projection for supported fixtures.

### Proof Commands

```bash
cargo test -p adze --features pure-rust --test typed_ast_contract typed_ast_contract_parse_document_ast_matches_parse -- --exact --nocapture
git diff --check
```

## Work Item: typed-cst-schema-generation

Status: complete
Linked spec: ../../docs/specs/ADZE-SPEC-0004-typed-cst-and-ast-projections.md
Blocked by: none
PR: #769

### Goal

Generate typed CST wrappers over document node IDs and field edge metadata.

### Proof Commands

```bash
cargo test -p adze --features pure-rust --test typed_cst_generated_document -- --nocapture
cargo test -p adze-tablegen --lib typed_cst -- --nocapture
git diff --check
```

## Work Item: document-diagnostics-store

Status: complete
Linked spec: ../../docs/specs/ADZE-SPEC-0005-diagnostics-and-recovery.md
Blocked by: none
PR: #770

### Goal

Store parse diagnostics and recovery facts directly on `AdzeDocument` and use
them for rendering and projections.

### Proof Commands

```bash
cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors -- --nocapture
cargo test -p adze --features "pure-rust,glr" --test error_display_tests -- --nocapture
git diff --check
```

## Work Item: ts-compat-document-adapter

Status: complete
Linked spec: ../../docs/specs/ADZE-SPEC-0006-tree-sitter-compatibility-adapter.md
Blocked by: none
PR: #771

### Goal

Make Tree-sitter-compatible APIs project from document node, edge, identity,
field, range, and error data.

### Proof Commands

```bash
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_to_sexp -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_node_error -- --nocapture
git diff --check
```

## Work Item: document-ambiguity-summary

Status: complete
Linked spec: ../../docs/specs/ADZE-SPEC-0007-glr-ambiguity-summary.md
Blocked by: none
PR: #772

### Goal

Expose summary-level GLR ambiguity data on `AdzeDocument`.

### Proof Commands

```bash
cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test test_e2e_ambiguous_grammar_glr generated_ambiguous_expr_parse_document_reports_ambiguity_summary -- --exact --nocapture
git diff --check
```

## Work Item: document-json-schema-alpha

Status: complete
Linked spec: ../../docs/specs/ADZE-SPEC-0008-json-cli-wasm-projections.md
Blocked by: none
PR: #775

### Goal

Expose schema-versioned `adze.document.v1` JSON as an experimental document
projection.

### Proof Commands

```bash
cargo test -p adze --features "pure-rust,serialization" --test adze_document_json -- --nocapture
git diff --check
```

## Work Item: language-schema-node-types

Status: complete
Linked spec: ../../docs/specs/ADZE-SPEC-0010-language-metadata-and-node-types.md
Blocked by: none
PR: #776

### Goal

Generate language metadata and node-types projections from the same schema used
by typed CST and Tree-sitter compatibility.

### Proof Commands

```bash
cargo test -p adze-tablegen node_types -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_language_metadata -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_language_fields -- --nocapture
git diff --check
```

## Build Order

1. `api-foundation-spec-stack`
2. `document-model-alpha-v2`
3. `pure-parser-document-bridge`
4. `generated-parse-document-canonical`
5. `parse-fast-path-delegates-to-document`
6. `typed-cst-schema-generation`
7. `document-diagnostics-store`
8. `ts-compat-document-adapter`
9. `document-ambiguity-summary`
10. `document-json-schema-alpha`
11. `language-schema-node-types`
12. support-tier promotion pass

All listed 0.9 work items are complete as of the 2026-05-16 closeout.

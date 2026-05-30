# ADZE-SPEC-0003: Canonical parse document

Status: accepted
Owner: runtime/api
Created: 2026-05-13
Linked proposal: ../proposals/ADZE-PROP-0002-api-foundation.md
Linked ADRs: ../adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Linked plan: ../../plans/0.9.0/api-foundation.md
Linked issues:
Linked PRs:
Support-tier impact: ../status/SUPPORT_TIERS.md
Policy impact: ../../policy/doc-artifacts.toml

## Problem

Adze needs one native parse product that owns the selected syntax tree, source
snapshot, diagnostics, metadata, ambiguity summaries, and projection inputs.
Without this document, generic CST, typed CST, typed AST, diagnostics,
Tree-sitter compatibility, JSON, CLI, WASM, and GLR output can drift.

## Behavior

### B1. `AdzeDocument` is the canonical parse product

All native and compatibility views must derive from `AdzeDocument`.

Views include:

- generic CST;
- generated typed CST;
- typed AST lowering;
- diagnostics rendering;
- GLR ambiguity summaries;
- Tree-sitter compatibility;
- JSON, CLI, and WASM projections.

### B2. `parse()` remains the typed AST fast path

Generated grammars continue to expose the stable ergonomic path:

```rust
let ast: ast::Root = grammar::parse(source)?;
```

The target implementation may delegate through `parse_document().ast()`, but
the public shortcut remains.

### B3. `parse_document()` returns document-shaped output

Generated grammars expose document parsing:

```rust
let doc = grammar::parse_document(source)?;
```

Syntax errors, recovery, missing nodes, bad tokens, and ambiguity should usually
produce a document with diagnostics. Infrastructure failures are separate.

### B4. `AdzeDocument` is monomorphic

The canonical document is not generic over AST type. Typed CST and typed AST are
views or projections, not fields on the document.

### B5. The document owns direct node and edge facts

The production document must expose document-local nodes, edge order, field
metadata, parent relationships, node identity, flags, ranges, diagnostics, and
parse metadata.

Fields live on edges. A field is a parent-child relation, not an intrinsic child
node property.

### B6. Nodes expose visible and grammar identity

Nodes must be able to answer visible identity and grammar identity separately so
aliases, typed CST casts, node-types metadata, and Tree-sitter compatibility can
be precise.

### B7. IDs are document-local

`NodeId`, token IDs, diagnostic IDs, and ambiguity IDs are local to a document.
They are not stable across reparses unless an explicit incremental mapping says
otherwise.

### B8. Projections are lazy where practical

The document eagerly stores the selected tree, source snapshot, diagnostics,
metadata, and summary ambiguity data when available. Typed CST wrappers, typed
AST lowering, Tree-sitter views, JSON serialization, query indexes, and full
forest data should be lazy where possible.

## Non-Goals

- No stable cross-document node identity guarantee.
- No full GLR forest by default.
- No Tree-sitter query parity.
- No stable JSON schema until schema snapshots exist.
- No stabilization of legacy placeholder node APIs.

## Required Evidence

- Document root kind/span/text canary.
- Node ID lookup canary.
- Edge field metadata canary.
- Visible versus grammar identity canary.
- Missing/error/extra flag canary.
- Diagnostics canary over bad input.
- `parse()` equals `parse_document().ast()` for supported fixtures.
- Typed CST wrapper reads the same node/span/text as generic CST.
- `ts_compat` projection reads from document data.

## Acceptance Examples

```rust
let doc = grammar::parse_document("1 +")?;
assert!(!doc.diagnostics().is_empty());
assert!(doc.root().is_some());
```

```rust
let fast_ast = grammar::parse("1 + 2")?;
let doc_ast: ast::Expr = grammar::parse_document("1 + 2")?.ast()?;
assert_eq!(fast_ast, doc_ast);
```

```rust
let doc = grammar::parse_document("1 + 2")?;
let syntax: syntax::SourceFile = doc.syntax()?;
assert_eq!(syntax.syntax().text(), "1 + 2");
```

## Test Mapping

Expected or existing tests include:

- `runtime/tests/adze_document_alpha.rs`
- `runtime/tests/typed_ast_contract.rs`
- `runtime/tests/typed_cst_generated_document.rs`
- `runtime/tests/adze_document_json.rs`
- future `runtime/tests/ts_compat_document_projection.rs`

## Implementation Mapping

Primary implementation surfaces:

- `runtime/src/document*`
- `runtime/src/ts_compat/`
- `tool/src/pure_rust_builder.rs`
- `tablegen/`
- generated grammar modules

## CI Proof

```bash
cargo test -p adze --features "pure-rust,ts-compat" --test adze_document_alpha -- --nocapture
cargo test -p adze --features pure-rust --test typed_ast_contract -- --nocapture
cargo test -p adze --features pure-rust --test typed_cst_generated_document -- --nocapture
git diff --check
```

## Metrics / Promotion Rule

`AdzeDocument` remains experimental until document construction, node/edge
lookup, typed AST projection, diagnostics, typed CST projection, and
Tree-sitter projection canaries pass for supported generated parser shapes.

It can move toward stabilizing only when `docs/status/SUPPORT_TIERS.md` maps the
public claim to proof commands.

## Open Questions

- Exact failed-document semantics for severe parse failures.
- Whether parent tables and point ranges are eager or lazy.
- Whether source text is always retained or can be externally referenced.
- Which ambiguity summaries are collected by default.

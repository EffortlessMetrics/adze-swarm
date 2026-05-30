# ADZE-SPEC-0004: Typed CST and typed AST projections

Status: accepted
Owner: tablegen/runtime
Created: 2026-05-13
Linked proposal: ../proposals/ADZE-PROP-0002-api-foundation.md
Linked ADRs: ../adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Linked plan: ../../plans/0.9.0/api-foundation.md
Linked issues:
Linked PRs:
Support-tier impact: ../status/SUPPORT_TIERS.md
Policy impact: ../../policy/doc-artifacts.toml

## Problem

Tree-sitter-style CST is flexible but dynamic and stringly. Typed AST is
ergonomic but usually lossy. Adze needs a Rust-native typed syntax layer for
formatters, codemods, LSPs, diagnostics, and agents, while preserving the typed
AST front door for semantic users.

## Behavior

### B1. Typed CST is a generated view over `AdzeDocument`

Typed CST wrappers are borrowed handles over document node IDs. They do not own
a second tree and do not reparse source.

```rust
pub trait TypedSyntaxNode<'doc>: Sized + Copy {
    fn cast(doc: &'doc AdzeDocument, id: NodeId) -> Option<Self>;
    fn syntax(&self) -> AdzeNode<'doc>;
    fn node_id(&self) -> NodeId;
}
```

### B2. Field accessors use document edge metadata

Generated accessors such as `left()`, `right()`, `name()`, or `body()` must use
document field IDs on edges. They must not infer fields from child kind names.

### B3. Accessors are recovery-aware

Generated field accessors should return `Option<T>` or iterators because
recovered and partial syntax may omit required grammar fields.

Strict helpers may be added later, but the first product surface must work on
broken source.

### B4. Typed AST is semantic projection

Typed AST lowering reads the document and may normalize or omit concrete syntax.
It is not the parse truth.

### B5. `parse()` remains the simple semantic API

Existing stable typed AST APIs remain:

```rust
let ast: ast::Module = grammar::parse(source)?;
```

The document-backed path is:

```rust
let ast: ast::Module = grammar::parse_document(source)?.ast()?;
```

### B6. Provenance is sidecar-based

AST-to-CST links are not always one-to-one. Provenance should support exact node,
composite nodes, span-only, synthetic, and ambiguity-selection cases without
forcing user AST structs to store node IDs.

## Non-Goals

- No visitor, rewriter, or typed query API in the first stable slice.
- No generated wrapper for every anonymous token by default.
- No forced provenance fields in user AST structs.
- No typed CST support-tier promotion without parity canaries.

## Required Evidence

- Generated wrapper compiles.
- Wrapper `NodeId` matches generic CST node.
- Wrapper span/text matches generic CST.
- Field accessors read edge fields.
- Typed AST projection equals existing `parse()` for supported fixtures.
- Recovered input does not panic typed CST casts.

## Acceptance Examples

```rust
let doc = grammar::parse_document(source)?;
let syntax: syntax::SourceFile = doc.syntax()?;
let first = syntax.functions().next().unwrap();
assert_eq!(first.name().unwrap().syntax().text(), "main");
```

```rust
let via_parse = grammar::parse(source)?;
let via_doc: ast::Module = grammar::parse_document(source)?.ast()?;
assert_eq!(via_parse, via_doc);
```

## Test Mapping

- `runtime/tests/typed_cst_arithmetic_spike.rs`
- `runtime/tests/typed_cst_generated_document.rs`
- `runtime/tests/typed_ast_contract.rs`
- `tablegen` typed CST generator tests
- `adze-tool` codegen tests

## Implementation Mapping

Primary implementation surfaces:

- `tablegen/src/typed_cst*`
- `tool/src/pure_rust_builder.rs`
- `runtime/src/document*`
- generated `syntax` modules
- `runtime/src/extract*`

## CI Proof

```bash
cargo test -p adze --features pure-rust --test typed_cst_generated_document -- --nocapture
cargo test -p adze --features pure-rust --test typed_ast_contract -- --nocapture
cargo test -p adze-tablegen typed_cst -- --nocapture
git diff --check
```

## Metrics / Promotion Rule

Typed AST fast-path claims remain stable only for already proven shapes.

Typed CST remains experimental until generated wrappers, field accessors,
span/text parity, recovery behavior, and generic CST parity are proven across a
representative fixture set.

# ADZE-ADR-0001: AdzeDocument is one parse truth

Status: accepted
Date: 2026-05-12
Owner: Adze maintainers
Linked proposal: ../proposals/ADZE-PROP-0002-api-foundation.md
Linked specs: ../specs/ADZE-SPEC-0003-canonical-parse-document.md

## Decision

`AdzeDocument` is the canonical native parse product for Adze.

Generic CST, typed CST, typed AST extraction, Tree-sitter-compatible projection,
diagnostics, parse metadata, and GLR ambiguity summaries must project from the
same document rather than from parallel parse products.

The native document is monomorphic:

```rust
pub struct AdzeDocument {
    source: SourceText,
    tree: AdzeTree,
    diagnostics: Vec<ParseDiagnostic>,
    ambiguities: AmbiguitySet,
    metadata: ParseMetadata,
}
```

Typed ASTs and typed CSTs are views or extractions:

```rust
let doc = grammar::parse_document(source)?;

let tree = doc.tree();
let syntax: syntax::SourceFile = doc.syntax()?;
let ast: ast::Module = doc.ast()?;
let ts_tree = adze::ts_compat::Tree::from_document(language.clone(), &doc);
```

They are not fields that make `AdzeDocument` generic over one AST type.

## Context

Adze needs two public stories to share one parse truth:

- Tree-sitter-compatible output for adoption, editor tooling, S-expressions,
  field lookup, node metadata, and familiar CST inspection.
- Adze-native output for Rust typed ASTs, typed CST, structured diagnostics,
  parse provenance, and GLR ambiguity insight.

If each surface builds its own tree or error model, the repo will accumulate
semantic drift. A field label could work in Tree-sitter compatibility but be
missing from typed CST. A diagnostic could exist in parse errors but not in the
native tree. A GLR ambiguity could be known by runtime internals but invisible
to the document API.

The current design documents already point to the desired shape:

- `../design/adze-document.md` defines `AdzeDocument` as the planned native
  parse-product boundary and source of truth for multiple views.
- `../design/typed-cst.md` defines typed CST as a generated view over
  `AdzeDocument`, not a separate tree.
- `../status/SUPPORT_TIERS.md` keeps `AdzeDocument` and typed CST experimental
  until proof commands and promotion criteria support stronger claims.

This ADR records the durable architecture rule so future compatibility,
serialization, WASM, CLI, and GLR work do not create independent parse truths.

## Consequences

### Enabled

- Tree-sitter compatibility becomes a projection and regression harness over
  native document facts.
- Typed CST wrappers can resolve fields through native `AdzeEdge` metadata.
- Typed AST extraction can record provenance against document node IDs, spans,
  or synthetic recovery facts.
- Structured diagnostics can attach to document nodes and ranges.
- GLR ambiguity summaries can explain the selected tree without forcing forest
  internals into the Tree-sitter-compatible API.
- CLI and WASM outputs can serialize the same facts through different schemas.

### Constrained

- `ts_compat` must not be the only place where field IDs, node identity, error
  state, or alias-visible behavior exists.
- Typed CST must not own copied syntax data or a second tree.
- Typed AST extraction must not silently discard provenance when the document has
  enough information to describe it.
- GLR forest internals should not be exposed through Tree-sitter compatibility;
  compatibility sees the selected tree, while native APIs expose ambiguity
  summaries and later forest data.
- Serialized output schemas must identify which projection of `AdzeDocument`
  they represent.

### Costs

- The native document model must carry enough metadata for all projections,
  including fields on edges, node identity, ranges, flags, diagnostics, and
  ambiguity summaries.
- Some projections should be lazy so common parsing paths do not pay for typed
  AST extraction, JSON serialization, or full forest export unless requested.
- Compatibility adapter code may need refactoring when a feature currently lives
  only in `ts_compat`.

## Alternatives Considered

### Tree-sitter-compatible tree as the core

Adze could make the Tree-sitter-shaped `Tree` and `Node` API the central runtime
model.

Rejected because Tree-sitter compatibility is an upstream conformance target,
not Adze's full native product. It should expose the selected compatible tree,
but it should not own typed ASTs, diagnostics, GLR ambiguity, or provenance.

### Generic `AdzeDocument<TAst>`

Adze could store one typed AST inside the document:

```rust
pub struct AdzeDocument<TAst = ()> {
    tree: AdzeTree,
    typed_ast: Option<TAst>,
    diagnostics: Vec<ParseDiagnostic>,
    ambiguities: AmbiguitySet,
    metadata: ParseMetadata,
}
```

Rejected because it ties a canonical document to one AST projection, complicates
serialization and WASM, and makes multiple semantic views over one CST harder.
The document should be monomorphic; ASTs should be extracted from it.

### Separate parse products per output

Adze could generate separate structures for generic CST, typed CST, typed AST,
Tree-sitter compatibility, diagnostics, and GLR output.

Rejected because separate parse products invite drift and make tests less
meaningful. A passing S-expression test would not prove the native document has
the same field metadata unless both use the same source of truth.

### Raw GLR forest as the first native API

Adze could expose the full forest before defining document-level ambiguity
summaries.

Rejected for the first stable native shape. Raw forest internals are useful for
advanced tooling, but the first product API should answer user-facing questions:
where ambiguity occurred, which alternatives existed, which tree was selected,
and why.

## Follow-Up Specs / Plans

This ADR requires follow-up behavior specs for:

- `ADZE-SPEC-0003-canonical-parse-document.md`;
- typed CST wrapper generation over document node IDs and field edges;
- typed AST provenance;
- structured diagnostics and node/range attachment;
- GLR ambiguity summaries and selection reasons;
- Tree-sitter compatibility projection over document facts;
- JSON schema versioning for native and compatibility outputs.

Implementation plans should sequence these as PR-sized changes and keep support
tier promotion tied to `../status/SUPPORT_TIERS.md`.

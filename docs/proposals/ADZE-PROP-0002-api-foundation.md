# ADZE-PROP-0002: API foundation

Status: accepted
Owner: runtime/api
Created: 2026-05-13
Target milestone: 0.9.x / 1.0 foundation
Linked specs: ADZE-SPEC-0003 canonical parse document; ADZE-SPEC-0004 typed CST and typed AST projections; ADZE-SPEC-0005 diagnostics and recovery; ADZE-SPEC-0006 Tree-sitter compatibility adapter; ADZE-SPEC-0007 GLR ambiguity summary; ADZE-SPEC-0008 JSON, CLI, and WASM projections; ADZE-SPEC-0009 incremental document lifecycle; ADZE-SPEC-0010 language metadata and node-types
Linked ADRs: ADZE-ADR-0001 AdzeDocument one parse truth; ADZE-ADR-0003 summary-first GLR ambiguity; ADZE-ADR-0004 schema-versioned projections
Linked plan: ../../plans/0.9.0/api-foundation.md
Support-tier impact: ../status/SUPPORT_TIERS.md
Policy impact: ../../policy/doc-artifacts.toml

## Problem

Adze has several parse-shaped surfaces: the stable typed-AST `parse()` fast
path, the experimental `parse_document()` API, generic CST accessors, generated
typed CST wrappers, structured diagnostics, GLR ambiguity summaries,
Tree-sitter compatibility, JSON projection, CLI output, and future WASM output.

These are valuable only if they share one parse truth. If each surface grows its
own tree, field model, diagnostics, alias handling, or ambiguity state, users
will see drift and agents will not know which behavior to trust.

The API foundation work turns the native parser product into a contract:

```text
source -> parser runtime -> AdzeDocument -> projections
```

## Users And Surfaces

- Rust users need `grammar::parse(source)` to stay the ergonomic typed value
  entry point.
- Tooling users need `parse_document()` for a concrete syntax tree,
  diagnostics, metadata, provenance, and ambiguity summaries.
- Formatter, codemod, LSP, and agent users need typed CST wrappers that keep
  concrete syntax and spans.
- Tree-sitter adopters need a compatibility adapter with explicit subset and
  deviation rules.
- CLI and WASM users need schema-versioned structured output.
- Grammar authors need ambiguity and diagnostics receipts that explain parser
  behavior instead of only returning a selected tree.

## Success Criteria

The API foundation is successful when:

- `AdzeDocument` is the canonical native parse product;
- `parse()` remains stable as the typed AST shortcut;
- generated `parse_document()` exposes document-shaped output;
- typed CST wrappers are generated over document node IDs and edge fields;
- typed AST lowering derives from the document and can record provenance;
- diagnostics and recovery are document facts;
- GLR ambiguity is exposed first as summaries and selection reasons;
- Tree-sitter compatibility is an adapter over document data;
- JSON, CLI, and WASM projections carry explicit schema versions;
- `docs/status/SUPPORT_TIERS.md` maps public claims to proof commands.

## Proposed Shape

Adze should expose one parse product and several views:

```rust
let ast: ast::Module = grammar::parse(source)?;

let doc = grammar::parse_document(source)?;

let tree = doc.tree();
let syntax: syntax::SourceFile = doc.syntax()?;
let ast: ast::Module = doc.ast()?;
let diagnostics = doc.diagnostics();
let ambiguities = doc.ambiguities();
let ts_tree = adze::ts_compat::Tree::from_document(language.clone(), &doc);
```

The views may be lazy, generated, or serialized, but they must not become
independent parse products.

## Alternatives Considered

### AST-first only

Rejected. Typed ASTs are the stable ergonomic front door, but ASTs are too lossy
for formatting, codemods, Tree-sitter compatibility, structured diagnostics,
and GLR ambiguity inspection.

### Tree-sitter-first

Rejected. Tree-sitter compatibility is an adoption and conformance adapter, not
Adze's native product model. It should project the selected tree from the
document.

### Forest-first

Rejected as the default. Raw GLR forest data is important for advanced tooling,
but the first native product surface should be selected tree plus ambiguity
summaries and selection reasons.

### Parallel outputs

Rejected. Separate CST, typed CST, AST, compatibility, and JSON parse products
would make proof weaker and drift easier.

## Specs To Create Or Update

- `ADZE-SPEC-0003-canonical-parse-document.md`
- `ADZE-SPEC-0004-typed-cst-and-ast-projections.md`
- `ADZE-SPEC-0005-diagnostics-and-recovery.md`
- `ADZE-SPEC-0006-tree-sitter-compatibility-adapter.md`
- `ADZE-SPEC-0007-glr-ambiguity-summary.md`
- `ADZE-SPEC-0008-json-cli-wasm-projections.md`
- `ADZE-SPEC-0009-incremental-document-lifecycle.md`
- `ADZE-SPEC-0010-language-metadata-and-node-types.md`

## Architecture Decisions Needed

- `ADZE-ADR-0001-adze-document-one-parse-truth.md`
- `ADZE-ADR-0003-summary-first-glr-ambiguity.md`
- `ADZE-ADR-0004-schema-versioned-projections.md`

## Implementation Campaign Shape

Implement the foundation one beam at a time:

1. Encode this proposal, specs, ADRs, and plan.
2. Add or tighten the target `AdzeDocument` model.
3. Bridge existing pure parser output into the document.
4. Generate `parse_document()` returning document-shaped output.
5. Prove `parse()` equals document-backed typed AST lowering.
6. Generate typed CST wrappers over document node IDs.
7. Store diagnostics and recovery facts on the document.
8. Move `ts_compat` projections over document data.
9. Add GLR ambiguity summaries.
10. Add schema-versioned JSON, CLI, and WASM projections.
11. Update support tiers only after proof commands exist.

## Evidence Plan

Each projection needs proof that it reads the same document:

- document root, node ID, edge, field, and span canaries;
- typed CST span/text/field parity canaries;
- typed AST equality against `parse()`;
- diagnostics byte/point range and recovery canaries;
- Tree-sitter field, identity, error, missing, S-expression, and node-types
  canaries;
- GLR ambiguity summary and selected-tree canaries;
- JSON schema snapshot and roundtrip canaries.

## Risks

- The native API can sprawl if every projection is implemented before the
  document contract is solid.
- Typed CST generation can produce a large surface without enough wrapper
  invariants.
- Tree-sitter compatibility can hide document gaps if adapter code invents
  behavior locally.
- GLR forest output can become too expensive or too unstable if exposed before
  summary-level ambiguity is proven.
- JSON/WASM consumers can become brittle if serialized schemas are unversioned.

## Non-Goals

This proposal does not:

- stabilize `AdzeDocument`, typed CST, GLR forest, Tree-sitter compatibility,
  CLI output, or WASM output by declaration;
- implement runtime changes;
- claim full Tree-sitter query compatibility;
- claim raw GLR forest stability;
- replace `docs/status/SUPPORT_TIERS.md` as the proof map.

## Exit Criteria

This proposal is implemented when the linked specs, ADRs, and plan exist, the
active goal manifest names the API-foundation work items, and the first runtime
implementation PRs can proceed from the plan without relying on chat history.

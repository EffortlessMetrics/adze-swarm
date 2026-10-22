# ADZE-SPEC-0007: GLR ambiguity summary

Status: accepted
Owner: runtime/glr
Created: 2026-05-13
Linked proposal: ../proposals/ADZE-PROP-0002-api-foundation.md
Linked ADRs: ../adr/ADZE-ADR-0003-summary-first-glr-ambiguity.md
Linked plan: ../../plans/0.9.0/api-foundation.md
Linked issues:
Linked PRs:
Support-tier impact: ../status/SUPPORT_TIERS.md
Policy impact: ../../policy/doc-artifacts.toml

## Problem

Tree-sitter exposes one selected tree. Adze can expose richer GLR truth, but raw
forest internals are too expensive and unstable as the first public product
surface. Users need to know where ambiguity occurred, which alternative was
selected, and why.

## Behavior

### B1. Selected tree is always the ordinary tree

When a document exists, `doc.tree()` returns the selected tree used by typed AST,
typed CST, Tree-sitter compatibility, and default JSON projections.

### B2. Ambiguity summaries are native document facts

`doc.ambiguities()` exposes summary-level ambiguity sites when the parser route
records them.

A summary should include:

- document-local ambiguity ID;
- byte and point range;
- selected alternative when known;
- selection reason;
- alternative summaries;
- enough shape or cost metadata to debug why a selection occurred.

### B3. Full forest is opt-in

Full packed forest data is not collected or stabilized by default. A later
`doc.forest()` API may expose it behind feature flags, options, or experimental
support tiers.

### B4. Typed AST uses the selected tree by default

Typed AST lowering reads the selected tree unless a future explicit alternative
API is requested.

### B5. Tree-sitter compatibility does not expose ambiguity

The compatibility adapter exposes the selected tree only. Ambiguity summaries
remain an Adze-native capability.

### B6. Alternative ordering and deduplication are canonical

Ambiguity alternatives are emitted in a canonical deterministic order, not
in runtime retention order (implemented by #1021, closing #1017):

- Alternatives sort by the structural subtree selection key first, then by
  dynamic precedence, cost, error flag, root symbol, node count, and span.
- An alternative is a duplicate only when its structural key and every
  serialized summary field agree; duplicates are removed. Alternatives that
  share summary fields but have different tree structures are retained.
- `index` is reassigned `0..n` after ordering, and `selected` points at the
  canonical position of the alternative that selection picked before
  reordering. Selection itself is unchanged by canonicalization.
- When deduplication leaves fewer than two distinct alternatives, no
  ambiguity summary is reported for that site.

Two root conventions exist today and both are accepted: the streaming
builder reports the bare grammar root (wrapper excluded from `node_count`),
while the legacy builder reports the augmented start root (wrapper
included). Unifying them is an open spec question, not current behavior;
consumers must not assume one convention across engines. The selected tree
is byte-identical under either convention.

## Non-Goals

- No stable full SPPF/forest schema yet.
- No stable tree enumeration API.
- No user-defined GLR selection policy in the first slice.
- No claim that every conflict currently records complete ambiguity metadata.

## Required Evidence

- Ambiguous grammar produces an ambiguity summary.
- Selected tree is deterministic.
- Selection reason is present.
- Typed AST projection uses the selected tree.
- Raw forest export remains experimental or absent by default.
- Alternative ordering and deduplication follow B6: the ambiguity snapshot
  test passes in repeated separate processes with no snapshot drift, and a
  duplicate-only retention set produces no summary.

## Acceptance Examples

```rust
let doc = grammar::parse_document(ambiguous_source)?;
let ambiguities = doc.ambiguities();
assert!(!ambiguities.is_empty());
assert!(ambiguities[0].selected().is_some());
```

```rust
let ast: ast::Expr = doc.ast()?;
assert_eq!(ast, ambiguities[0].selected_ast_shape());
```

## Test Mapping

- `runtime/tests/test_e2e_ambiguous_grammar_glr.rs`
- `adze-glr-core` ambiguity and conflict tests
- `runtime/tests/adze_document_json.rs` ambiguity snapshot and determinism
  tests (B6)
- future document ambiguity projection tests

## Implementation Mapping

Primary implementation surfaces:

- GLR runtime selection and conflict routing;
- `runtime/src/document/ambiguity*`;
- typed AST document projection;
- JSON/CLI ambiguity projection later.

## CI Proof

```bash
cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test test_e2e_ambiguous_grammar_glr -- --nocapture
cargo test -p adze-glr-core ambiguity -- --nocapture
git diff --check
```

## Metrics / Promotion Rule

Ambiguity summaries may move toward stabilizing after selected-tree
determinism, selected alternative, selection reason, and typed-AST-selected-tree
canaries pass for generated GLR fixtures. Full forest output remains
experimental until separately specified.

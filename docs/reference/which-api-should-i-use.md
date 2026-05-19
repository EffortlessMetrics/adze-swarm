# Which API Should I Use?

> **Doc status:** Product guide for the Toolkit Excellence campaign. Support
> status remains authoritative in
> [`SUPPORT_TIERS.md`](../status/SUPPORT_TIERS.md).

Adze has one native parse truth and several views over it:

```text
grammar::parse(source)
  -> typed Rust value

grammar::parse_document(source)
  -> AdzeDocument
    -> diagnostics
    -> generic syntax tree
    -> typed CST wrappers
    -> typed AST projection
    -> Tree-sitter-shaped selected tree
    -> query matching
    -> JSON / CLI / WASM projections
    -> ambiguity summaries
```

Most users should start with generated `grammar::parse(source)`. Move to
`grammar::parse_document(source)` when you need tooling facts, diagnostics,
ranges, fields, Tree-sitter-shaped traversal, JSON, or GLR ambiguity summaries.

## Quick Choice Table

| Need | Use | Why | Claim boundary |
| --- | --- | --- | --- |
| Typed Rust value | `grammar::parse(source)` | This is the stable generated parser front door. | Stable for the supported typed-extraction and pure-Rust parser rows in support tiers. |
| Parse errors while building a typed parser | `grammar::parse(source)` error values | Generated parser errors carry spans and expected-token information for the documented matrix. | Structured parse errors are Stabilizing, not a blanket parser-recovery claim. |
| Diagnostics, ranges, metadata, or tooling facts | `grammar::parse_document(source)` | Returns the native `AdzeDocument` parse product. | `AdzeDocument` is proof-backed but still Experimental. |
| Generic syntax-tree traversal | `document.tree()` and document node APIs | Walks the selected document tree without Tree-sitter adapter assumptions. | Native document APIs are still promoted by support-tier rows, not by this guide. |
| Typed CST traversal | Generated `syntax::*` wrappers over document node IDs | Gives Rust-native syntax wrappers while staying document-backed. | Typed CST is not Stable yet; use current generated-wrapper proof and known gaps. |
| Typed AST from a tooling parse | Document typed-AST projection or generated document helpers | Keeps typed extraction and tooling facts tied to the same parse. | Projection support follows the document and typed-extraction support tiers. |
| Tree-sitter-style tree traversal | `adze::ts_compat::Tree::from_document(...)` or `adze::ts_compat::Parser` | Adapts document facts into the selected-tree compatibility subset. | Not full Tree-sitter runtime, query, node-types, or imported-corpus parity. |
| Query matching | `adze::query` with the documented subset | Matches the selected tree and language metadata. | Source-aware predicates need source text; unsupported query features remain known gaps. |
| GLR ambiguity information | `document.ambiguities()` | Uses native ambiguity summaries while selected-tree projections expose one chosen tree. | Raw forest exposure and typed extraction from alternatives are not stable product claims. |
| JSON output | Document JSON projection or CLI `document-json` / `tree-json` / `diagnostics-json` modes | Provides schema-versioned transport for tooling experiments. | JSON schemas and CLI output remain tiered separately from the typed parser front door. |
| CLI scaffolding and smoke use | `adze init`, `adze check`, `adze parse` | Good for getting started and for examples. | CLI surfaces are useful but not the Stable parser API unless support tiers say so. |
| Incremental/editor lifecycle | Document lifecycle and fallback metadata | Lets tools integrate with explicit fallback behavior. | Incremental reuse and changed-range precision are Experimental. |

## Recommended Path

1. Build the grammar around generated Rust types and `grammar::parse(source)`.
2. Add `grammar::parse_document(source)` when the application needs diagnostics,
   source ranges, syntax facts, or projections.
3. Add Tree-sitter compatibility only when existing tooling expects a
   Tree-sitter-shaped selected tree.
4. Add queries only after checking the documented supported subset.
5. Treat performance, JSON, WASM, and incremental behavior as receipt-backed
   surfaces instead of assuming stable broad claims.

## What To Avoid By Default

- Do not start from low-level parser constructors unless you are working on
  generated-code, runtime, tablegen, or compatibility internals.
- Do not treat Tree-sitter compatibility as full Tree-sitter parity.
- Do not treat query compatibility as full Tree-sitter query parity.
- Do not build stable product claims on raw GLR forest internals.
- Do not quote performance numbers without benchmark receipts.

## Related References

- [Quickstart: First Parser In 10 Minutes](../tutorials/quickstart-10-minutes.md)
- [API Reference](./api.md)
- [Parser Cookbook](./parser-cookbook.md)
- [Tree-sitter Compatibility](./tree-sitter-compatibility.md)
- [Query Compatibility](./query-compatibility.md)
- [Product Acceptance Matrix](../product/ACCEPTANCE_MATRIX.md)
- [Support Tiers](../status/SUPPORT_TIERS.md)

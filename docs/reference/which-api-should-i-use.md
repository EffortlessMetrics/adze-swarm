# Which API Should I Use?

> **Doc status:** User-experience hardening guide. Support status remains
> authoritative in [`SUPPORT_TIERS.md`](../status/SUPPORT_TIERS.md).

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

## First Decision

Use this ladder before reaching for lower-level runtime types:

```rust
let ast = grammar::parse(source)?;

let document = grammar::parse_document(source)?;
let diagnostics = document.diagnostics();
let tree = document.as_tree_sitter();
let ambiguities = document.ambiguities();
```

The typed parser path is the beginner and library-author front door. The
document path is the tooling front door. Compatibility, query, JSON, CLI, WASM,
and performance surfaces stay bounded by their support-tier rows.

## Adoption Proof Ladder

The beginner path and tooling path are both backed by starter-shaped proof:

| User step | API surface | Local proof |
| --- | --- | --- |
| Generate starter project | `adze init` output | `cargo test -p adze-cli test_init_default_cwd_generates_buildable_project -- --exact --nocapture`<br>`cargo test -p adze-cli test_init_generates_buildable_project -- --exact --nocapture` |
| Use typed values | `grammar::parse(source)` | `cargo test --manifest-path testing/downstream-starter/Cargo.toml` |
| Run the generated parse example | generated `examples/parse.rs` | `cargo run --manifest-path testing/downstream-starter/Cargo.toml --example parse -- "1 + 2 * 3"` |
| Inspect diagnostics and document facts | `grammar::parse_document(source)` / `AdzeDocument` | `cargo test --manifest-path testing/downstream-starter/Cargo.toml` |

These commands prove local checkout and path-dependency starter behavior. They
do not prove `cargo install adze-cli`, crates.io dependency resolution, or
public release availability; those remain release-surface receipts tracked
separately.

## Quick Choice Table

| Need | Use | Why | Claim boundary |
| --- | --- | --- | --- |
| Typed Rust value | `grammar::parse(source)` | This is the stable generated parser front door. | Stable for the supported typed-extraction and pure-Rust parser rows in support tiers. |
| Parse errors while building a typed parser | `grammar::parse(source)` error values | Generated parser errors carry spans and expected-token information for the documented matrix. | Structured parse errors are Stabilizing, not a blanket parser-recovery claim. |
| Diagnostics, ranges, metadata, or tooling facts | `grammar::parse_document(source)` | Returns the native `AdzeDocument` parse product. | `AdzeDocument` is Stabilizing for the documented generated-parser tooling path, not a Stable API claim. |
| Generic syntax-tree traversal | `document.tree()` and document node APIs | Walks the selected document tree without Tree-sitter adapter assumptions. | Native document APIs are still promoted by support-tier rows, not by this guide. |
| Typed CST traversal | Generated `syntax::*` wrappers over document node IDs | Gives Rust-native syntax wrappers while staying document-backed. | Typed CST is not Stable yet; use current generated-wrapper proof and known gaps. |
| Typed AST from a tooling parse | Document typed-AST projection or generated document helpers | Keeps typed extraction and tooling facts tied to the same parse. | Projection support follows the document and typed-extraction support tiers. |
| Tree-sitter-style tree traversal | `adze::ts_compat::Tree::from_document(...)` or `adze::ts_compat::Parser` | Adapts document facts into the selected-tree compatibility subset. | Not full Tree-sitter runtime, query, node-types, or imported-corpus parity. |
| Query matching | `adze::query` with the documented subset | Matches the selected tree and language metadata. | Source-aware predicates need source text; unsupported query features remain known gaps. |
| GLR ambiguity information | `document.ambiguities()` | Uses native ambiguity summaries while selected-tree projections expose one chosen tree. | Raw forest exposure and typed extraction from alternatives are not stable product claims. |
| JSON output | Document JSON projection or CLI `document-json` / `tree-json` / `diagnostics-json` modes | Provides schema-versioned transport for tooling experiments. | JSON schemas and CLI output remain tiered separately from the typed parser front door. |
| CLI scaffolding and smoke use | `adze init`, `adze check`, `adze parse`, and `testing/downstream-starter` | Good for getting started and for examples; the checked-in fixture mirrors generated starter layout. | CLI surfaces are useful but not the Stable parser API unless support tiers say so. |
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

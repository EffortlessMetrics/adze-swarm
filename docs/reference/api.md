# Adze API Reference

> **Doc status:** Aligned with the 2026-05 support-tier ledger.

This page describes the user-facing API ladder. For the authoritative support
status of each surface, use [`SUPPORT_TIERS.md`](../status/SUPPORT_TIERS.md).
If you are choosing between generated `parse()`, `parse_document()`, document
projections, Tree-sitter compatibility, queries, JSON, and CLI surfaces, start
with [Which API Should I Use?](./which-api-should-i-use.md).

## The Stable Front Door: Generated `parse()`

Most users should start with a generated grammar module:

```rust
#[adze::grammar("arithmetic")]
pub mod grammar {
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub enum Expr {
        Number(
            #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
            i32,
        ),

        #[adze::prec_left(1)]
        Add(Box<Expr>, #[adze::leaf(text = "+")] (), Box<Expr>),

        #[adze::prec_left(2)]
        Mul(Box<Expr>, #[adze::leaf(text = "*")] (), Box<Expr>),
    }

    #[adze::extra]
    #[allow(dead_code)]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s+")]
        _ws: (),
    }
}

let ast = grammar::parse("1 + 2 * 3")?;
```

`grammar::parse(input)` is the ergonomic stable path. It returns typed Rust
values generated from your grammar types.

Proof is tracked as the Stable typed-extraction and Pure-Rust parser rows in
[`SUPPORT_TIERS.md`](../status/SUPPORT_TIERS.md), including clean downstream
quickstart canaries.

## Tooling Path: Generated `parse_document()`

Use `parse_document()` when you need parse facts for tooling instead of only a
typed semantic value:

```rust
let document = grammar::parse_document("1 +")?;
let diagnostics = document.diagnostics();
let tree = document.tree();
```

`AdzeDocument` is the native parse-product boundary. It stores source text,
selected tree facts, diagnostics, metadata, and ambiguity summaries. Typed AST,
typed CST, Tree-sitter-compatible output, query matching, JSON, CLI, and WASM
views are projections from the same document model.

`AdzeDocument` is still **Experimental**. Its behavior is proof-backed, but it
is not yet a stable public API contract.

## Grammar Attributes

The common grammar attributes are:

| Attribute | Purpose | Support |
|---|---|---|
| `#[adze::grammar("name")]` | Declares a generated grammar module. | Stable front door |
| `#[adze::language]` | Marks a struct or enum as part of the grammar. | Stable front door |
| `#[adze::leaf(pattern = "...")]` | Defines a token by regex pattern. | Stable front door |
| `#[adze::leaf(text = "...")]` | Defines a literal token. | Stable front door |
| `#[adze::leaf(transform = ...)]` | Converts matched token text into a Rust value. | Stable front door |
| `#[adze::extra]` | Defines whitespace/comments or other skipped tokens. | Covered by parser proof; scanner-heavy uses may be experimental |
| `#[adze::prec_left(...)]`, `#[adze::prec_right(...)]` | Defines precedence/associativity for expression grammars. | Stable for documented arithmetic shapes |

See [Getting Started](../tutorials/getting-started.md) and the
[Parser Cookbook](./parser-cookbook.md) for complete examples.

## Runtime Traits

### `Extract`

`Extract` powers generated typed AST extraction. Most users should not implement
it manually; the macros generate the implementation for grammar structs and
enums.

Stable supported shapes include:

- `String` token text extraction;
- `Vec<T>` repeated elements;
- `Option<T>` optional elements;
- `Box<T>` recursive structures;
- generated structs and enums.

The current proof includes exact-value and repeated-parse determinism canaries.

### `Spanned<T>` and parse errors

`Spanned<T>` attaches source spans to extracted values. Generated parser errors
are structured and include spans and expected-token information for the
documented generated grammar matrix. The structured parse-error surface is
**Stabilizing**, not yet Stable.

## Feature Flags

| Feature | Description | Support |
|---|---|---|
| `pure-rust` | Pure-Rust parser backend. Default stable front door. | Stable |
| `glr` | Enables GLR parsing for ambiguous grammars. | Stabilizing |
| `serialization` | Enables core table serialization and experimental document JSON. | Stable for core tables; document JSON experimental |
| `ts-compat` | Tree-sitter-compatible selected-tree adapter. | Advisory |
| `incremental_glr` | Incremental parsing and fallback metadata. | Experimental |
| `wasm` | WASM build support. | Advisory compile signal |

## Compatibility And Tooling Surfaces

### Tree-sitter compatibility

Tree-sitter compatibility is an adapter over native document data. It exposes a
selected-tree subset and language metadata where proof exists. It is not a full
Tree-sitter runtime, query, node-types, or imported grammar corpus parity claim.

Use [Tree-sitter Compatibility](./tree-sitter-compatibility.md) for the exact
subset and known gaps.

### Query compatibility

Adze has a documented Tree-sitter query subset. Source-aware predicates,
captures, byte ranges, root-only matching, fields, anchors, and differential
fixtures have advisory proof. Full query parity is not claimed.

Use [Query Compatibility](./query-compatibility.md) for the current subset.

### CLI, WASM, runtime2, and grammar crates

These surfaces are useful but outside the stable product contract unless the
support-tier row says otherwise:

- CLI project scaffolding and document projection output are **Advisory**.
- WASM currently has compile-check signal, not browser/runtime certification.
- `runtime2/` is intentionally excluded from the public-primary runtime
  contract.
- bundled grammar crates are reference/integration fixtures, not stable
  language packages.

## Lower-Level Parser APIs

The runtime contains lower-level parser, tree, and compatibility modules for
generated code and implementation work. They are not the ordinary user entry
point. Prefer generated `grammar::parse()` for typed values and
`grammar::parse_document()` for tooling-oriented parse facts.

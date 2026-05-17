# API Reference

Adze's stable user API is generated from your Rust grammar types. The normal
path is:

```text
Rust grammar types -> generated parser -> typed Rust AST
```

For the authoritative support status, use `docs/status/SUPPORT_TIERS.md` in the
repository.

## Generated `parse()`

Most users should call the generated parser function:

```rust
let ast = grammar::parse("1 + 2 * 3")?;
```

This is the stable front door. It returns the typed Rust value described by
your grammar structs and enums.

## Generated `parse_document()`

Use `parse_document()` when building tooling:

```rust
let document = grammar::parse_document("1 +")?;
let diagnostics = document.diagnostics();
let tree = document.tree();
```

`AdzeDocument` is the native parse-product boundary. Typed CST, typed AST,
diagnostics, Tree-sitter-compatible output, query matching, JSON, CLI, and WASM
views should project from the same document facts. This surface is still
experimental until promoted in the support tiers.

## Grammar Attributes

Common attributes:

- `#[adze::grammar("name")]` declares a generated grammar module.
- `#[adze::language]` marks a struct or enum as grammar data.
- `#[adze::leaf(pattern = "...")]` defines a regex token.
- `#[adze::leaf(text = "...")]` defines a literal token.
- `#[adze::leaf(transform = ...)]` converts matched text to a Rust value.
- `#[adze::extra]` defines whitespace/comments or other skipped tokens.
- `#[adze::prec_left(...)]` and `#[adze::prec_right(...)]` define operator
  precedence and associativity.

## Runtime Traits

`Extract` powers generated typed AST extraction. Most users should not implement
it manually; macros generate it for grammar structs and enums.

Supported generated shapes include:

- `String` token text extraction;
- `Vec<T>` repeated elements;
- `Option<T>` optional elements;
- `Box<T>` recursive structures;
- generated structs and enums.

`Spanned<T>` attaches source spans to extracted values. Structured parse errors
include spans and expected-token information for the documented generated
grammar matrix, but parse-error wording and broad invalid-input coverage remain
stabilizing.

## Feature Flags

| Feature | Description | Support |
|---|---|---|
| `pure-rust` | Pure-Rust parser backend. | Stable |
| `glr` | GLR parsing for ambiguous grammars. | Stabilizing |
| `serialization` | Core table serialization and experimental document JSON. | Stable for core tables; document JSON experimental |
| `ts-compat` | Tree-sitter-compatible selected-tree adapter. | Advisory |
| `incremental_glr` | Incremental parsing and fallback metadata. | Experimental |
| `wasm` | WASM build support. | Advisory compile signal |

## Compatibility And Tooling

Tree-sitter compatibility is an adapter over native document data. It exposes a
selected-tree subset, not full Tree-sitter runtime/query/node-types parity.

Query compatibility is a documented subset with source-aware predicate proof.
Full query parity is not claimed.

CLI project scaffolding and document projection output are advisory. WASM has
compile-check signal only. `runtime2/` and bundled grammar crates are not stable
public product contracts.

## Lower-Level Parser APIs

The runtime contains lower-level parser and tree modules for generated code and
implementation work. They are not the ordinary user entry point. Prefer
generated `grammar::parse()` for typed values and `grammar::parse_document()`
for tooling-oriented parse facts.

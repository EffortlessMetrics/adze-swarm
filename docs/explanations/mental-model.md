# Mental Model

Adze's product model is:

```text
Rust type annotations
  -> grammar IR
  -> parse table
  -> generated parser
  -> typed AST
  -> AdzeDocument
  -> optional projections
```

The beginner path is `grammar::parse(input)`. It returns the typed Rust value
described by your grammar types.

The tooling path is `grammar::parse_document(input)`. It returns the native
parse product used for diagnostics, syntax tree inspection, ambiguity summaries,
Tree-sitter-compatible output, JSON, CLI, and future editor integrations.

## One Parse Truth

`AdzeDocument` is the canonical parse product. Everything else is a projection:

```text
source
  -> parser runtime
  -> AdzeDocument
      -> typed AST
      -> generic CST
      -> typed CST
      -> diagnostics
      -> ambiguity summaries
      -> Tree-sitter-compatible selected tree
      -> query cursor subset
      -> JSON / CLI / WASM projections
```

This matters because Adze should not maintain separate parse truths for typed
ASTs, syntax trees, diagnostics, Tree-sitter compatibility, and JSON output.
Those views can evolve independently, but they must agree about the document
facts they project.

## Which API Should I Use?

Use `parse()` when you want typed Rust values:

```rust
let expr = grammar::parse("1 + 2 * 3")?;
```

Use `parse_document()` when you are building tooling:

```rust
let document = grammar::parse_document("1 +")?;
let diagnostics = document.diagnostics();
let tree = document.tree();
```

Use Tree-sitter-compatible views when you need ecosystem interop:

```rust
let document = grammar::parse_document(source)?;
let tree = adze::ts_compat::Tree::from_document(language.clone(), &document);
```

That compatibility surface exposes the selected tree. Native Adze APIs expose
ambiguity summaries separately.

## Support Tiers

Not every surface is equally stable. `parse()` and typed extraction are the
stable front door. `AdzeDocument`, typed CST, Tree-sitter compatibility, query,
CLI, WASM, and GLR ambiguity surfaces move upward only when their support-tier
rows have repeatable proof.

Use [Support Tiers](../status/SUPPORT_TIERS.md) when you need to know whether a
surface is stable, stabilizing, experimental, or advisory.

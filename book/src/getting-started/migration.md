# Migration Guide: Generated Parser Surface

> **Doc status:** This page replaces the old low-level runtime migration story.
> The current product path is generated parser modules:
> `grammar::parse()` for typed Rust values and `grammar::parse_document()` for
> tooling facts.

Adze no longer asks ordinary users to migrate from one low-level runtime API to
another. The supported migration is from hand-managed parser construction to
generated grammar modules.

## What Changed

| Old mental model | Current product model |
|---|---|
| Construct a parser manually. | Define grammar types and call generated `grammar::parse()`. |
| Convert a generic tree into an AST yourself. | The generated parser returns typed Rust values directly. |
| Treat Tree-sitter compatibility as the core runtime. | Treat compatibility as a projection from `AdzeDocument`. |
| Assume incremental reuse/performance by default. | Treat incremental lifecycle as Experimental with visible fallback metadata. |
| Use runtime internals as public API. | Use generated APIs first; reach lower only for implementation work. |

## Dependencies

For a fresh generated-parser crate, start with the same shape as the book
quickstart:

```toml
[dependencies]
adze = { version = "0.8.0-dev", default-features = false }

[build-dependencies]
adze-tool = "0.8.0-dev"

[features]
default = ["pure-rust"]
pure-rust = ["adze/pure-rust"]
```

Create a `build.rs` that asks `adze-tool` to generate parsers from your grammar
source:

```rust
use std::path::PathBuf;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(adze_unsafe_attrs)");
    adze_tool::build_parsers(&PathBuf::from("src/lib.rs"));
}
```

## Parser Usage

### Before: manual parser construction

Older examples often used a low-level parser object and then manually mapped a
tree into application data:

```rust,ignore
let mut parser = make_parser_somehow();
let tree = parser.parse(source)?;
let ast = extract_ast_from_tree(tree)?;
```

### After: generated typed parser

Write Rust grammar types and call the generated function:

```rust
let ast = grammar::parse(source)?;
```

The result is the typed AST described by your Rust grammar module.

## Tooling Usage

Use `parse_document()` when your application needs parse facts instead of only a
typed semantic value:

```rust
let document = grammar::parse_document(source)?;

let diagnostics = document.diagnostics();
let root = document.tree().root();
let ambiguities = document.ambiguities();
```

`AdzeDocument` is the native parse product. Generic CST, typed CST, typed AST,
diagnostics, GLR ambiguity summaries, Tree-sitter-compatible output, JSON, CLI,
and future editor projections should agree with the same document facts.

## GLR Migration

GLR support is not a separate user-facing runtime migration. Ambiguous grammars
still use generated parser modules:

```rust
let ast = grammar::parse(source)?;
let document = grammar::parse_document(source)?;
```

Current support posture:

- GLR conflict routing is Stabilizing.
- Selected-tree behavior is deterministic for proven generated-parser slices.
- Ambiguity summaries are native document facts.
- Tree-sitter-compatible output exposes the selected tree only.
- Full forest export and broad grammar-class stability are not claimed.

## Tree-sitter Compatibility

If you are migrating from a Tree-sitter mental model, keep this split:

```text
Adze native API:
  generated parse() / parse_document() / AdzeDocument

Tree-sitter compatibility:
  selected-tree adapter over document data
```

Use compatibility projections for ecosystem interop, not as the core parse
truth:

```rust
let document = grammar::parse_document(source)?;
let tree = document.as_tree_sitter();
```

See the API reference and known limitations before assuming Tree-sitter method,
query, node-types, or imported grammar corpus parity.

## Incremental Migration

Incremental parsing is Experimental. Do not migrate editor integrations around a
guaranteed reuse percentage or a stable incremental performance claim.

The accepted model is:

- documents are immutable snapshots;
- edits produce new document snapshots;
- node IDs are document-local;
- fallback to full reparse must be visible in metadata.

Tooling should remain correct when Adze reports a full-reparse fallback.

## Common Migration Fixes

### Replace parser construction

Prefer:

```rust
let ast = grammar::parse(source)?;
```

Instead of carrying a parser object unless you are inside generated code or an
implementation/proof lane.

### Replace tree-first AST extraction

Prefer:

```rust
let ast = grammar::parse(source)?;
```

Use:

```rust
let ast = grammar::parse_document(source)?.ast::<grammar::Expr>()?;
```

only when the document path is specifically needed.

### Replace broad compatibility assumptions

Prefer support-tier-specific language:

```text
Tree-sitter-compatible selected-tree subset
```

instead of:

```text
full Tree-sitter replacement
```

## Validation

For a migrated quickstart-style crate:

```bash
cargo test
```

For repository product changes:

```bash
just ci-supported
```

For support-tier decisions, use `docs/status/SUPPORT_TIERS.md`; do not promote a
surface based only on migration prose.

## Next Steps

- [Quick Start](quickstart.md)
- [Parser Generation](../guide/parser-generation.md)
- [Incremental Parsing](../guide/incremental-parsing.md)
- [API Reference](../reference/api.md)
- [Known Limitations](../reference/known-limitations.md)

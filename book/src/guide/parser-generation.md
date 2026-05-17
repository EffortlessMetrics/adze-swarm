# Parser Generation

> **Doc status:** Aligned with the generated-parser product surface. The stable
> front door is generated `grammar::parse()`. Tooling uses generated
> `grammar::parse_document()`. GLR, Tree-sitter compatibility, document JSON,
> and incremental surfaces follow their support-tier rows.

Adze turns Rust grammar types into generated parser code at build time. The
ordinary user path is:

```text
Rust type annotations
  -> grammar IR
  -> parse table
  -> generated parser module
  -> grammar::parse(source)
```

The tooling path uses the same generated parser and returns the canonical
document model:

```text
source
  -> generated parser
  -> AdzeDocument
      -> typed AST
      -> generic CST
      -> typed CST
      -> diagnostics
      -> ambiguity summaries
      -> Tree-sitter-compatible selected tree
      -> JSON / CLI projections
```

`AdzeDocument` is the one parse truth. Optional views project from it rather
than creating separate parser products.

## Build-Time Generation

Adze generation has four main steps:

1. **Grammar extraction**: `adze-tool` reads Rust source marked with Adze
   attributes.
2. **IR generation**: Rust types become grammar IR.
3. **Table generation**: tablegen builds compressed parser tables and metadata.
4. **Parser emission**: generated Rust modules expose the user APIs.

The generated module should be the API most application code calls:

```rust
let ast = grammar::parse("1 + 2 * 3")?;
```

Use the document path when a tool needs syntax facts, diagnostics, ambiguity, or
compatibility projections:

```rust
let document = grammar::parse_document("1 +")?;
let diagnostics = document.diagnostics();
let root = document.tree().root();
```

## Generated Artifacts

When `ADZE_EMIT_ARTIFACTS=true`, builds can emit inspectable artifacts under the
build output directory. Exact paths depend on the generated grammar, but common
artifact families are:

```text
grammar.json
parser_tables.rs
node-types.json
*.parsetable
```

Treat emitted artifacts as receipts and debugging aids unless a support-tier row
promotes them for a public surface. For example, `node-types.json` and
Tree-sitter-compatible metadata are useful, but full Tree-sitter parity is not a
stable claim.

## Runtime Surfaces

| Surface | Use | Support posture |
|---|---|---|
| `grammar::parse(source)` | Typed Rust values | Stable front door |
| `grammar::parse_document(source)` | Tooling document facts | Experimental/Stabilizing by surface |
| `document.as_tree_sitter()` | Selected-tree compatibility | Advisory subset |
| document JSON | Schema-tagged document projection | Experimental/advisory |
| incremental document lifecycle | Reparse/fallback metadata | Experimental |

Lower-level parser, table, and compatibility modules exist for generated code
and implementation work. They are not the ordinary user entry point.

## GLR Generation

GLR support preserves and routes conflict cells that ordinary LR parsing cannot
represent as a single action. The current product posture is:

- GLR conflict routing is **Stabilizing**.
- Generated parser canaries prove selected-tree determinism for specific
  conflict classes.
- Ambiguity summaries are native document facts.
- Tree-sitter-compatible output exposes the selected tree only.

Example generated usage stays the same:

```rust
let ast = grammar::parse(source)?;

let document = grammar::parse_document(source)?;
println!("ambiguities: {}", document.ambiguities().len());
```

Do not infer broad stable GLR, full forest, full Tree-sitter parity, or query
parity from the existence of generated GLR paths. Use
`docs/status/SUPPORT_TIERS.md` for the current proof map.

## Debugging Generation

### Emit artifacts

```bash
ADZE_EMIT_ARTIFACTS=true cargo build
```

Use this when you need to inspect generated grammar JSON, parser tables,
node-types metadata, or `.parsetable` receipts.

### Enable runtime logging

```bash
ADZE_LOG_PERFORMANCE=true cargo test -p adze --features "pure-rust,glr"
```

Runtime performance logs are diagnostic output, not a performance contract.
Performance claims should be tied to benchmark fixtures and receipts.

### Run focused proof

```bash
cargo test -p adze-glr-core conflict ambiguity -- --nocapture
cargo test -p adze-tablegen --all-features
cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test test_e2e_ambiguous_grammar_glr -- --nocapture
```

Before submitting product-facing changes, run the supported gate:

```bash
just ci-supported
```

## Common Issues

### Large parse tables

Large grammars can produce large tables. Prefer focused changes first:

1. simplify grammar rules when possible;
2. reduce accidental conflicts;
3. inspect emitted artifacts;
4. run tablegen/GLR proof before claiming product support.

### Slow build times

Parser generation can be slow for complex grammars:

1. use `cargo check` or `just check-fast` during iteration;
2. keep generated artifacts available when debugging;
3. split large changes into smaller grammar/tablegen slices;
4. avoid broad full-matrix or coverage proof unless the lane requires it.

### GLR memory usage

GLR parsing may retain more state because it explores conflicting paths:

1. benchmark representative fixtures before optimizing;
2. inspect ambiguity summaries to understand conflict shape;
3. keep full-forest export experimental until the support tier changes;
4. prefer selected-tree/document projections for ordinary tooling.

## Choosing An API

Choose `grammar::parse()` when:

- you want typed semantic Rust values;
- you are writing an application parser;
- you do not need diagnostics or syntax-tree inspection.

Choose `grammar::parse_document()` when:

- you need diagnostics, source ranges, fields, or CST facts;
- you are building CLI/editor/tooling integrations;
- you need GLR ambiguity summaries;
- you need Tree-sitter-compatible selected-tree or JSON projections.

Use lower-level table or compatibility modules only for implementation work,
advanced integration, or proof lanes.

## Next Steps

- [Quickstart](../getting-started/quickstart.md)
- [API Reference](../reference/api.md)
- [Known Limitations](../reference/known-limitations.md)
- `docs/status/SUPPORT_TIERS.md`

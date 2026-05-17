# Architecture

Adze transforms annotated Rust types into generated parsers. The stable front
door is `grammar::parse()`, which returns typed Rust values directly. Tooling
uses `grammar::parse_document()` and projects diagnostics, CST, GLR ambiguity,
Tree-sitter-compatible selected-tree output, and JSON from the same
`AdzeDocument`.

## Crate Organization

The workspace is split into focused microcrates (see the [Microcrate Guide](microcrates.md) for the full list). At the highest level they fall into three layers:

```text
┌─ Grammar Definition ──────────┐
│  macro/      proc-macro attrs  │
│  common/     shared expansion  │
└────────────────────────────────┘
          │  extracts rules
          ▼
┌─ Build-Time Generation ───────┐
│  ir/         grammar IR        │
│  glr-core/   LR(1) + GLR      │
│  tablegen/   table compression │
│  tool/       build.rs driver   │
└────────────────────────────────┘
          │  emits parser code
          ▼
┌─ Runtime Execution ───────────┐
│  runtime/    generated API     │
│  runtime2/   experimental lab  │
└────────────────────────────────┘
```

### Dependency flow

```text
tool ─┐
      ├─▶ common ─▶ ir ─▶ glr-core ─▶ tablegen
macro ┘
```

Build-time crates are **never** linked into the user's final binary; only the runtime crate is.

## Build Pipeline

When you run `cargo build` on a project that uses Adze:

```text
Rust types          IR               Parse tables        Compiled parser
with annotations ──▶ Grammar ──▶ LR(1) automaton ──▶ compressed ──▶ linked into
                    (ir/)       (glr-core/)          (tablegen/)    binary
```

### Stage 1 — Macro Expansion (`macro/`, `common/`)

`#[adze::grammar]` and friends collect type information. The `common` crate contains the actual expansion logic shared between the proc-macro and the build tool.

### Stage 2 — IR Construction (`ir/`)

The tool reads the annotated source and builds a `Grammar` IR. Complex symbols (`Optional`, `Repeat`, `Choice`, `Sequence`) are normalized into auxiliary rules via `Grammar::normalize()`.

### Stage 3 — GLR Analysis (`glr-core/`)

FIRST/FOLLOW sets are computed, then the canonical LR(1) collection is built. Conflicts are **preserved** (not eliminated) to support GLR parsing — each state/symbol cell can hold multiple actions.

### Stage 4 — Table Generation (`tablegen/`)

Parse tables are compressed using Tree-sitter-compatible algorithms and emitted as static `Language` structs with FFI-compatible layout. The output also includes `NODE_TYPES` JSON metadata.

### Stage 5 — Code Emission (`tool/`)

`adze_tool::build_parsers()` ties the stages together. It writes generated Rust
source files and compile instructions for Cargo.

## Runtime

### The `Extract` trait

The runtime crate (`runtime/`) provides `Extract`, the core trait that converts
selected parse facts into typed Rust values. The generated code implements
`Extract` for every type in your grammar module.

### Generated parser and document path

The generated parser module is the user-facing runtime surface:

| Component | File | Purpose |
|---|---|---|
| Typed parse | generated module | `grammar::parse(source)` returns typed AST values |
| Document parse | generated module + `runtime/` | `grammar::parse_document(source)` returns `AdzeDocument` |
| Extraction | `runtime/` | Generated `Extract` implementations build typed values |
| Compatibility projections | `runtime/` | Tree-sitter-shaped selected-tree and JSON/document views |

Parsing flow:

```text
source text
    │
    ▼
generated parser ──▶ AdzeDocument
                         │
                         ├─▶ Extract ──▶ typed AST
                         ├─▶ diagnostics
                         ├─▶ ambiguity summaries
                         └─▶ compatibility / JSON projections
```

`runtime2/` remains an experimental proving ground, not the public-primary
runtime contract.

### Performance monitoring

Set `ADZE_LOG_PERFORMANCE=true` for diagnostic runtime logging where supported.
Performance claims still need benchmark fixtures and receipts.

## Key Design Decisions

1. **Two-stage processing** — macros mark types; the build tool generates the parser. This avoids proc-macro limitations (no file I/O, no cross-crate state).
2. **Conflict preservation** — GLR tables keep all shift/reduce and reduce/reduce conflicts so the parser can fork at runtime, enabling ambiguous-grammar support.
3. **Document-centered compatibility** — Tree-sitter compatibility is a
   selected-tree adapter over `AdzeDocument`, not the core parse product.
4. **Bounded concurrency** — all parallel work respects configurable caps (`RUST_TEST_THREADS`, `RAYON_NUM_THREADS`) to prevent resource exhaustion.

## Further Reading

- [Microcrate Guide](microcrates.md) — detailed per-crate responsibilities
- [Development Architecture](development/architecture.md) — deeper diagrams and data-flow details
- [GLR Parsing](advanced/glr-parsing.md) — how the GLR algorithm works in Adze

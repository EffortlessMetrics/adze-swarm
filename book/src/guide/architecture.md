# Architecture

This chapter explains how Adze works from end to end — what happens when you
annotate Rust types, run `cargo build`, and parse source text at runtime. Use it
to build a mental model of the system so you can make informed decisions about
grammar design, performance tuning, and extensibility.

> **See also**
> [Architecture Overview](../architecture.md) for the crate-level view,
> [Microcrate Guide](../microcrates.md) for the full crate catalogue, and
> [Development Architecture](../development/architecture.md) for contributor-oriented details.

---

## High-Level Architecture

```text
                          ┌──────────────────────────────────────┐
                          │           YOUR CRATE                 │
                          │                                      │
                          │   src/grammar.rs   ← you write this  │
                          │       │                              │
                          │       ▼                              │
                          │   build.rs                           │
                          │   adze_tool::build_parsers()         │
                          │       │                              │
                          │       ├── Macro expansion (common/)  │
                          │       ├── IR construction  (ir/)     │
                          │       ├── GLR analysis  (glr-core/)  │
                          │       ├── Table compression          │
                          │       │      (tablegen/)             │
                          │       └── Code emission   (tool/)   │
                          │               │                      │
                          │               ▼                      │
                          │   OUT_DIR/parser.rs  ← generated     │
                          │               │                      │
                          │               ▼                      │
                          │   runtime / runtime2  ← linked       │
                          │       │                              │
                          │       ▼                              │
                          │   your_crate::grammar::parse(text)   │
                          └──────────────────────────────────────┘
```

Everything above the dashed line happens **once at build time**; everything
below runs **in your application**.  The generated parser code lives in Cargo's
`OUT_DIR` and is never checked into version control.

---

## The Build Pipeline — Step by Step

### 1. Grammar Definition (you write this)

You express a grammar as annotated Rust types.  The macro attributes
(`#[adze::grammar]`, `#[adze::leaf]`, `#[adze::prec_left]`, …) are thin
markers — they record metadata but do **no** code generation themselves.

```rust
#[adze::grammar("arithmetic")]
mod grammar {
    #[adze::language]
    pub enum Expr {
        Number(#[adze::leaf(pattern = r"\d+")] String),
        #[adze::prec_left(1)]
        Add(Box<Expr>, #[adze::leaf(text = "+")] (), Box<Expr>),
    }
}
```

Why types and not a DSL?  Because Rust's type system already gives you enums,
generics, `Box`, `Vec`, and `Option` — all of which map naturally to grammar
constructs (choices, recursion, repetition, optionals).

### 2. `build.rs` — The Orchestrator

Your crate's `build.rs` calls one function:

```rust
fn main() {
    adze_tool::build_parsers(&PathBuf::from("src/grammar.rs"));
}
```

Under the hood this kicks off four stages, each owned by a dedicated crate.

### 3. IR Construction (`adze-ir`)

The build tool reads the macro-annotated source and constructs a **Grammar
Intermediate Representation**:

```text
Rust types + annotations
        │
        ▼
  ┌──────────────┐
  │   Grammar IR │   • rules, symbols, fields
  │              │   • precedence / associativity
  │              │   • external scanner declarations
  └──────┬───────┘
         │
         ▼
  Grammar::normalize()
         │
         ▼
  Auxiliary rules for Optional, Repeat, Choice, Sequence
```

`normalize()` rewrites complex symbols into simple auxiliary rules so that
downstream stages only need to handle flat rule lists.

### 4. GLR Analysis (`adze-glr-core`)

From the normalized grammar the GLR core builds the parsing automaton:

```text
  Grammar IR
      │
      ├─▶ FIRST / FOLLOW sets
      │
      ├─▶ LR(1) item sets  →  canonical collection
      │
      └─▶ Action & goto tables
              │
              ├── Shift actions
              ├── Reduce actions
              └── Conflict cells (shift/reduce, reduce/reduce)
```

**Key design choice — conflict preservation.**  Traditional LALR generators
*resolve* conflicts at build time (or report them as errors).  Adze instead
**keeps every conflicting action** in the table.  At parse time the GLR engine
forks, exploring all paths in parallel, and merges them when the ambiguity
resolves.  This means:

- You can parse inherently ambiguous grammars without rewriting them.
- Precedence and associativity *order* actions (preferred first) but never
  discard alternatives.
- Shift/reduce and reduce/reduce conflicts are not errors — they are expected.

### 5. Table Compression (`adze-tablegen`)

Raw LR tables for a production grammar can be enormous (hundreds of states ×
hundreds of symbols).  `tablegen` compresses them using the same algorithms
Tree-sitter uses:

| Technique | Effect |
|---|---|
| Default-action elimination | Removes the most common action per state |
| Sparse row encoding | Stores only non-default entries |
| Symbol grouping | Shares rows across symbols with identical actions |

The output is a **static `Language` struct** with FFI-oriented layout, plus a
`NODE_TYPES` JSON file describing node kinds. Those facts are useful for
Tree-sitter-shaped tooling experiments, but they do not by themselves prove
full Tree-sitter runtime, query, highlighting, folding, or imported-grammar
parity. Use the support-tier ledger and compatibility references for the
currently proven subset.

### 6. Code Emission (`adze-tool`)

Finally the tool writes:

- **Generated Rust source** — `include!`d from `OUT_DIR`.
- **`Extract` implementations** — one per grammar type, converting parse-tree
  nodes into your Rust types.
- **Build script output** — `cargo:rerun-if-changed` directives so Cargo
  re-generates only when the grammar file changes.

---

## Crate Dependency Graph

Only the **runtime** crate ends up in your final binary.  Build-time crates
are dev/build dependencies:

```text
  ┌────────────────── build-time only ──────────────────┐
  │                                                     │
  │  tool ──┐                                           │
  │         ├──▶ common ──▶ ir ──▶ glr-core ──▶ tablegen│
  │  macro ─┘                                           │
  │                                                     │
  └─────────────────────────────────────────────────────┘

  ┌────────────────── shipped in binary ────────────────┐
  │                                                     │
  │  runtime   (Extract trait, error recovery, visitor)  │
  │  runtime2  (GLR engine, tree builder, incremental)   │
  │                                                     │
  └─────────────────────────────────────────────────────┘
```

If you only use the `Extract` API you depend on `runtime`.  If you need the
full GLR `Parser` + `Tree` API, depend on `runtime2` as well.

---

## Runtime Data Flow

Once the parser is compiled into your binary, parsing a string follows this
path:

```text
  "1 + 2 * 3"                      ← input text
       │
       ▼
  ┌──────────┐
  │  Lexer   │  Splits text into tokens:
  │          │    NUMBER("1") PLUS("+") NUMBER("2") STAR("*") NUMBER("3")
  └────┬─────┘
       │
       ▼
  ┌──────────────┐
  │  GLR Driver  │  Pushes tokens through the parse table.
  │              │  On conflicts → fork into parallel stacks.
  │              │  On convergence → merge stacks.
  └────┬─────────┘
       │
       ▼
  ┌──────────────┐
  │ Parse Forest │  A packed representation that may encode
  │              │  multiple valid parse trees (ambiguity).
  └────┬─────────┘
       │
       ▼
  ┌──────────────┐
  │ Tree Builder │  Selects the preferred tree (highest
  │              │  precedence path) and emits a concrete
  │              │  Tree-sitter-compatible Tree.
  └────┬─────────┘
       │
       ▼
  ┌──────────────┐
  │   Extract    │  Walks the tree and populates your
  │              │  typed Rust values (Expr, Statement, …).
  └──────────────┘
```

### Incremental Parsing (Experimental)

When a user edits a small region of a large file, reparsing from scratch is
wasteful.  The incremental path (feature-gated behind `incremental_glr`)
identifies the unchanged prefix and suffix, reparses only the changed middle
section, and splices the result back into the existing forest.

> **Status:** The incremental path currently falls back to a full reparse for
> correctness.  The infrastructure is in place but disabled until remaining
> edge cases are resolved.

---

## Key Design Decisions

| Decision | Rationale |
|---|---|
| **Types _are_ the grammar** | No separate grammar DSL to learn.  Rust enums map to alternatives, `Box<T>` to recursion, `Vec<T>` to repetition, `Option<T>` to optionals. |
| **Two-stage build** | Proc macros cannot do file I/O or share state across crates.  Splitting annotation (macros) from generation (build tool) sidesteps those limitations. |
| **Conflict preservation** | Discarding conflicts forces grammar authors to restructure rules.  Preserving them lets the GLR engine handle ambiguity transparently. |
| **Tree-sitter ABI** | Reusing Tree-sitter's `Language` struct layout means existing editor integrations, syntax highlighting queries, and code-folding rules work unchanged. |
| **Microcrate architecture** | Each stage is a small, independently testable crate.  This keeps compile times low and makes it easy to swap implementations. |
| **Bounded concurrency** | Configurable caps on test threads, rayon pools, and async runtimes prevent resource exhaustion in CI and on constrained machines. |

---

## Extension Points

### Custom External Scanners

Some tokens (Python indentation, Haskell layout rules, template literals)
cannot be expressed as regular expressions.  Implement the `ExternalScanner`
trait:

```rust
use adze::ExternalScanner;

#[derive(Default)]
struct IndentScanner { indent_stack: Vec<usize> }

impl ExternalScanner for IndentScanner {
    fn scan(&mut self, lexer: &mut Lexer, valid_symbols: &[bool]) -> ScanResult {
        // custom indentation logic
    }
    fn serialize(&self) -> Vec<u8>   { /* … */ }
    fn deserialize(&mut self, buf: &[u8]) { /* … */ }
}
```

The scanner is called during lexing whenever the token set includes an
externally-defined symbol.

### Visitor & Query APIs

After parsing you can traverse the tree in two ways:

- **Visitor pattern** (`runtime/visitor.rs`) — implement a visitor trait and
  walk the tree with callbacks for enter/leave on each node kind.
- **Query / pattern matching** — write S-expression patterns to extract nodes
  matching structural constraints (see [Query and Pattern Matching](query-patterns.md)).

### Serialization

Trees can be serialized to JSON or S-expression format for debugging, logging,
or cross-process communication:

```rust
use adze::serialization::{to_json, to_sexp};

let json = to_json(&tree);
let sexp = to_sexp(&tree);
```

### Build-Time Artifact Inspection

Set `ADZE_EMIT_ARTIFACTS=true` before building to write intermediate files
(grammar JSON, node types, generated source) to `target/debug/build/<crate>/out/`.
This is invaluable for debugging grammar issues.

### Adding a New Grammar

1. Create a new crate with `#[adze::grammar]`-annotated types.
2. Add a `build.rs` calling `adze_tool::build_parsers()`.
3. Depend on `adze` (runtime) in `[dependencies]` and `adze-tool` in
   `[build-dependencies]`.
4. Optionally add golden tests to validate against a reference parser.

---

## Performance Characteristics

### Build Time

| Phase | Typical cost | Scales with |
|---|---|---|
| Macro expansion | < 1 s | Number of annotated types |
| IR construction | < 1 s | Grammar rule count |
| GLR analysis | 1–30 s | States × symbols (quadratic worst case) |
| Table compression | < 5 s | Table size |
| Code emission | < 1 s | Grammar size |

For a grammar the size of Python (~270 symbols, ~60 fields) the full pipeline
completes in under 30 seconds on a modern laptop.

### Parse Time

- **Unambiguous input:** The GLR engine behaves like a standard LR parser —
  linear in the input length, O(n).
- **Ambiguous input:** Each conflict introduces a fork.  In the worst case
  (highly ambiguous grammar + adversarial input) parsing can be O(n³), but
  real-world grammars rarely trigger this.
- **Incremental reparse:** When enabled, cost is proportional to the size of
  the edited region, not the whole file.

### Memory

- Parse tables are **static data** — they live in the binary's read-only
  segment and cost zero allocation at runtime.
- The GLR parse forest uses arena allocation internally, reducing allocator
  pressure during parsing.
- Tree nodes are compact structs; a typical 1 000-line source file produces
  a tree under 1 MB.

### Tuning Knobs

| Variable | Default | Effect |
|---|---|---|
| `RUST_TEST_THREADS` | 2 | Max concurrent test threads |
| `RAYON_NUM_THREADS` | 4 | Rayon thread-pool size |
| `ADZE_LOG_PERFORMANCE` | off | Logs forest→tree metrics (node count, depth, time) |
| `ADZE_EMIT_ARTIFACTS` | off | Writes intermediate build artifacts for inspection |

---

## Summary

```text
  You write          Adze generates         You call at runtime
  ──────────         ──────────────         ───────────────────
  Rust types    ──▶  Parse tables     ──▶   parse("input")
  with attrs         + Extract impls        → typed AST
```

The entire build pipeline is **deterministic and reproducible** — the same
grammar always produces the same tables and the same generated code, regardless
of platform.  The runtime is **allocation-light and fork-safe**, suitable for
embedding in editors, CLI tools, and WASM environments.

For deeper dives into individual topics see:

- [Grammar Definition](grammar-definition.md) — annotation syntax reference
- [Parser Generation](parser-generation.md) — build.rs configuration
- [GLR Parsing](../advanced/glr-parsing.md) — how the GLR algorithm works
- [Performance Optimization](performance.md) — profiling and tuning tips

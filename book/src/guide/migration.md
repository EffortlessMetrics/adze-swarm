# Migration Guide: rust-sitter to Adze

This chapter provides a comprehensive guide for migrating projects from
`rust-sitter` (the original project name) to **Adze**. The rename was
formalized in the v0.8.0 release and affects crate names, attribute paths,
import paths, and repository URLs.

> **Looking for GLR, document, or incremental guidance?**
> Use [Parser Generation](parser-generation.md) and
> [Incremental Parsing](incremental-parsing.md). The current product path is
> generated `grammar::parse()` and `grammar::parse_document()`, not a migration
> from one low-level runtime API to another.

---

## Why the Rename?

The project was renamed from `rust-sitter` to **Adze** to:

- Establish a distinct identity separate from the Tree-sitter ecosystem.
- Avoid confusion with the upstream `tree-sitter` crate.
- Reflect the project's evolution into a full grammar toolchain with GLR
  parsing, table generation, and typed AST extraction — well beyond a simple
  Rust wrapper around Tree-sitter.

---

## What Changed

### Crate Names

| Old Name (rust-sitter)      | New Name (Adze)     | Role                              |
|-----------------------------|---------------------|-----------------------------------|
| `rust-sitter`               | `adze`              | Runtime / main user-facing crate  |
| `rust-sitter-macro`         | `adze-macro`        | Procedural macros                 |
| `rust-sitter-tool`          | `adze-tool`         | Build-time code generation        |
| `rust-sitter-common`        | `adze-common`       | Shared expansion logic            |
| —                           | `adze-ir`           | Grammar intermediate representation (new) |
| —                           | `adze-glr-core`     | GLR parser generation (new)       |
| —                           | `adze-tablegen`     | Table compression (new)           |

### Attribute Paths

All procedural-macro attributes moved from the `rust_sitter` namespace to
`adze`:

| Old Attribute                   | New Attribute                  |
|---------------------------------|--------------------------------|
| `#[rust_sitter::grammar("…")]`  | `#[adze::grammar("…")]`       |
| `#[rust_sitter::language]`      | `#[adze::language]`            |
| `#[rust_sitter::leaf(…)]`       | `#[adze::leaf(…)]`            |
| `#[rust_sitter::prec_left(n)]`  | `#[adze::prec_left(n)]`       |
| `#[rust_sitter::prec_right(n)]` | `#[adze::prec_right(n)]`      |
| `#[rust_sitter::extra]`         | `#[adze::extra]`              |
| `#[rust_sitter::word]`          | `#[adze::word]`               |
| `#[rust_sitter::delimited(…)]`  | `#[adze::delimited(…)]`       |
| `#[rust_sitter::repeat(…)]`     | `#[adze::repeat(…)]`          |

### Import Paths

```rust
// ── Before ────────────────────────────────────────
use rust_sitter::Spanned;
use rust_sitter::Parser;

// ── After ─────────────────────────────────────────
use adze::Spanned;
use adze::Parser;
```

Deeper re-exports follow the same pattern:

```rust
// ── Before ────────────────────────────────────────
use rust_sitter::tree_sitter::Node;

// ── After ─────────────────────────────────────────
use adze::ts_compat::Language;
use adze::adze_ir::{Grammar, SymbolId};
use adze::adze_glr_core::{Action, ParseTable};
```

### Cargo.toml Dependencies

```toml
# ── Before ─────────────────────────────────────────
[dependencies]
rust-sitter = "0.4"

[build-dependencies]
rust-sitter-tool = "0.4"
```

```toml
# ── After ──────────────────────────────────────────
[dependencies]
adze = "0.8"

[build-dependencies]
adze-tool = "0.8"
```

### Repository URL

| Before | `github.com/<org>/rust-sitter` |
|--------|-------------------------------|
| **After** | `github.com/EffortlessMetrics/adze` |

Update any CI badges, issue links, or documentation URLs accordingly.

---

## New Features Available After Migration

Migrating to Adze unlocks several capabilities that did not exist under the
`rust-sitter` name:

1. **GLR Parsing** — Handle ambiguous grammars with automatic fork/merge of
   parse paths (`adze-glr-core`).
2. **Pure-Rust Table Generation** — Generate Tree-sitter–compatible parse
   tables entirely in Rust (`adze-tablegen`).
3. **Grammar IR** — Inspect and optimize grammars programmatically via the
   intermediate representation (`adze-ir`).
4. **Incremental Document Lifecycle** — Experimental snapshot/reparse metadata
   with visible fallback behavior.
5. **Performance Monitoring** — Set `ADZE_LOG_PERFORMANCE=true` to get
   forest-to-tree conversion metrics.
6. **WASM Compile Signal** — WASM remains advisory until runtime/browser proof
   is promoted.
7. **Golden Tests** — Validate your parsers against Tree-sitter reference
   implementations with SHA256 hash verification.
8. **External Scanners** — Experimental custom lexing support for cases like
   indentation tracking.

---

## Before / After Code Examples

### Defining a Grammar

```rust
// ── Before (rust-sitter) ──────────────────────────
#[rust_sitter::grammar("arithmetic")]
mod grammar {
    #[rust_sitter::language]
    pub enum Expr {
        Number(
            #[rust_sitter::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
            i32,
        ),
        #[rust_sitter::prec_left(1)]
        Sub(Box<Expr>, #[rust_sitter::leaf(text = "-")] (), Box<Expr>),
        #[rust_sitter::prec_left(2)]
        Mul(Box<Expr>, #[rust_sitter::leaf(text = "*")] (), Box<Expr>),
    }

    #[rust_sitter::extra]
    struct Whitespace {
        #[rust_sitter::leaf(pattern = r"\s")]
        _ws: (),
    }
}
```

```rust
// ── After (adze) ──────────────────────────────────
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
        Sub(Box<Expr>, #[adze::leaf(text = "-")] (), Box<Expr>),
        #[adze::prec_left(2)]
        Mul(Box<Expr>, #[adze::leaf(text = "*")] (), Box<Expr>),
    }

    #[adze::extra]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s")]
        _ws: (),
    }
}
```

### Using the Parser

```rust
// ── Before (rust-sitter) ──────────────────────────
let result = grammar::parse("1 - 2 * 3");
match result {
    Ok(expr) => println!("{:?}", expr),
    Err(errs) => eprintln!("{:?}", errs),
}
```

```rust
// ── After (adze) ──────────────────────────────────
let result = grammar::parse("1 - 2 * 3");
match result {
    Ok(expr) => println!("{expr:?}"),
    Err(errs) => eprintln!("{errs:?}"),
}
```

### Build Script

```rust
// ── Before (rust-sitter) ──────────────────────────
fn main() {
    rust_sitter_tool::build_parsers();
}
```

```rust
// ── After (adze) ──────────────────────────────────
fn main() {
    adze_tool::build_parsers();
}
```

---

## Step-by-Step Migration Checklist

Follow these steps in order. Each step is independently committable.

### 1. Update `Cargo.toml`

Replace every `rust-sitter` dependency with its `adze` equivalent:

```toml
# runtime crate
rust-sitter = "0.4"   →   adze = "0.8"

# build tool
rust-sitter-tool = "0.4"   →   adze-tool = "0.8"
```

Run `cargo update` to pull the new crates.

### 2. Update `build.rs`

```diff
- rust_sitter_tool::build_parsers();
+ adze_tool::build_parsers();
```

### 3. Rename Attributes in Grammar Modules

A project-wide find-and-replace handles this:

```bash
# In your project root
sed -i 's/rust_sitter::/adze::/g' src/**/*.rs
```

Verify that every `#[rust_sitter::…]` becomes `#[adze::…]`.

### 4. Update `use` Statements

```bash
sed -i 's/use rust_sitter/use adze/g' src/**/*.rs
```

### 5. Update Any Direct Tree-sitter Imports

If you used internal Tree-sitter types via `rust-sitter`, they may have moved:

```rust
// Old
use rust_sitter::tree_sitter::Node;

// New — use the ts_compat re-export or the sub-crate directly
use adze::ts_compat::Language;
```

### 6. Run `cargo check`

Resolve any remaining compile errors. The compiler will surface any
missed renames as "unresolved import" or "cannot find attribute" errors.

### 7. Run Your Tests

```bash
cargo test
```

All existing tests should pass without changes beyond the renames.

### 8. Update CI and Documentation

- Change repository URLs from `rust-sitter` to `adze`.
- Update badge URLs, crate links, and README references.
- Update any `Dockerfile` or Nix expressions that reference the old crate
  names.

### 9. (Optional) Adopt New Features

Once the rename migration compiles cleanly, consider enabling:

```toml
[dependencies]
adze = { version = "0.8", features = ["pure-rust"] }
```

Use `grammar::parse_document()` when you need diagnostics, GLR ambiguity
summaries, or compatibility projections. See [Parser Generation](parser-generation.md)
and [Incremental Parsing](incremental-parsing.md) for the current product model.

---

## Common Migration Issues and Solutions

### "cannot find attribute `rust_sitter` in this scope"

**Cause:** An attribute was not renamed.

**Fix:** Search for any remaining `rust_sitter::` references:

```bash
grep -rn 'rust_sitter::' src/
```

Replace them with `adze::`.

### "unresolved import `rust_sitter`"

**Cause:** A `use rust_sitter::…` statement was not updated.

**Fix:**

```bash
grep -rn 'use rust_sitter' src/
```

Replace with `use adze`.

### "no matching package named `rust-sitter`"

**Cause:** `Cargo.toml` still references the old crate name.

**Fix:** Replace `rust-sitter` with `adze` and `rust-sitter-tool` with
`adze-tool` in all `Cargo.toml` files.

### "function `build_parsers` not found in `rust_sitter_tool`"

**Cause:** `build.rs` still calls `rust_sitter_tool::build_parsers()`.

**Fix:** Change to `adze_tool::build_parsers()`.

### Snapshot tests fail after migration

**Cause:** Generated Tree-sitter grammar JSON may embed the crate path, and
snapshot tools like `insta` capture the exact output.

**Fix:** Run `cargo insta review` to accept the updated snapshots, or
regenerate golden hashes:

```bash
UPDATE_GOLDEN=1 cargo test -p adze-golden-tests
```

### Macro-generated code triggers new warnings

**Cause:** Adze may produce slightly different expanded code than the old
`rust-sitter` macros, surfacing new Clippy lints.

**Fix:** Address the warnings or, as a temporary measure:

```rust
#[allow(clippy::all)]
#[adze::grammar("my_lang")]
mod grammar { /* … */ }
```

---

## MSRV Note

Adze v0.8 requires **Rust 1.95.0** (2024 edition). If your project targets an
older MSRV, you will need to update your `rust-toolchain.toml` or CI
configuration:

```toml
# rust-toolchain.toml
[toolchain]
channel = "1.95.0"
components = ["rustfmt", "clippy"]
```

---

## Summary

The migration from `rust-sitter` to Adze is primarily a rename operation.
The core grammar-definition model — annotated Rust types that compile into
generated parser modules — is unchanged. After completing the rename, use
`grammar::parse()` for typed values and `grammar::parse_document()` for tooling
facts. GLR, Tree-sitter compatibility, incremental lifecycle, CLI, WASM, and
grammar-package surfaces remain governed by their support-tier rows.

If you run into issues not covered here, please
[open an issue](https://github.com/EffortlessMetrics/adze/issues) on GitHub.

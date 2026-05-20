# Getting Started

This chapter walks you through installing Adze, defining your first grammar, building the parser, and using it to parse input.

## Installation

Add the runtime and build-tool crates to your `Cargo.toml`:

The versioned dependency block is the intended release-surface shape after the
coordinated publish. Current repo proof uses local/path dependencies from this
checkout until crates.io receipts exist for the co-release crates.

```toml
[dependencies]
adze = "0.9.0"

[build-dependencies]
adze-tool = "0.9.0"
```

Then create a `build.rs` in your project root:

```rust
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=src");
    adze_tool::build_parsers(&PathBuf::from("src/main.rs"));
}
```

> **Tip:** For WASM targets, enable the `pure-rust` feature to avoid C
> dependencies. See [Installation](getting-started/installation.md) for all
> backend options.

## Your First Grammar

Define a simple arithmetic grammar using Rust types annotated with Adze attributes. Create `src/main.rs`:

```rust
#[adze::grammar("arithmetic")]
pub mod grammar {
    /// The root type the parser returns.
    #[adze::language]
    #[derive(Debug, PartialEq, Eq)]
    pub enum Expression {
        Number(
            #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap_or_default())]
            i32,
        ),

        #[adze::prec_left(1)]
        Add(
            Box<Expression>,
            #[adze::leaf(text = "+")] (),
            Box<Expression>,
        ),

        #[adze::prec_left(2)]
        Mul(
            Box<Expression>,
            #[adze::leaf(text = "*")] (),
            Box<Expression>,
        ),
    }

    #[adze::extra]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s")]
        _ws: (),
    }
}
```

**Key annotations:**

| Attribute | Purpose |
|---|---|
| `#[adze::grammar("name")]` | Declares a grammar module and generates the parser |
| `#[adze::language]` | Marks the root AST type |
| `#[adze::leaf(pattern = …)]` | Matches a regex token |
| `#[adze::leaf(text = …)]` | Matches an exact string token |
| `#[adze::prec_left(n)]` | Left-associative precedence (higher = tighter) |
| `#[adze::extra]` | Tokens silently skipped between other tokens |

## Building

Run:

```bash
cargo build
```

During the build Adze:

1. Reads the annotated types in your grammar module.
2. Extracts grammar rules and generates an IR.
3. Builds LR(1)/GLR parse tables.
4. Emits Rust (or C, depending on backend) parser code that is compiled into your binary.

Set `ADZE_EMIT_ARTIFACTS=true` to inspect the generated grammar JSON:

```bash
ADZE_EMIT_ARTIFACTS=true cargo build
# Artifacts land in target/debug/build/<crate>-<hash>/out/
```

## Parsing

Use the generated `grammar::parse` function to parse input and extract a typed AST:

```rust
fn main() {
    let input = "1 + 2 * 3";
    match grammar::parse(input) {
        Ok(expr) => println!("{expr:?}"),
        // Add(Number(1), (), Mul(Number(2), (), Number(3)))
        Err(errs) => {
            for e in errs {
                eprintln!("parse error: {e}");
            }
        }
    }
}
```

Run it:

```bash
cargo run
```

Because multiplication has `prec_left(2)` (higher than addition's `1`), the expression `1 + 2 * 3` correctly parses as `1 + (2 * 3)`.

## Next Steps

- [Quick Start](getting-started/quickstart.md) — a deeper walkthrough with GLR features
- [Grammar Definition](guide/grammar-definition.md) — full reference for grammar attributes
- [Architecture](architecture.md) — how the pieces fit together
- [Microcrate Guide](microcrates.md) — understanding the workspace layout

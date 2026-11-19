# Quick start: The generated arithmetic parser

Generate an Adze starter from a checkout, run its tests, and inspect the Rust
value returned by the parser. [Build your first parser](./docs/tutorials/quickstart-10-minutes.md)
is the shorter walkthrough. Both use the CLI and dependencies from this checkout,
not a published package.

## Step 1: Create the project

From the checkout root, choose an unused project name (`calc` below). The
checkout pins Rust 1.95.0.

```bash
cargo run -p adze-cli -- init calc
cd calc
cargo test
cargo run --quiet --example parse -- "1 + 2 * 3"
```

The generated project contains:

```text
Cargo.toml
build.rs
src/lib.rs
src/grammar.rs
examples/parse.rs
tests/parse.rs
README.md
```

**Expected output**:

```text
Add(Number(1), (), Mul(Number(2), (), Number(3)))
```

The starter grammar parses arithmetic expressions into typed Rust values.
The generated tests also check invalid input and document diagnostics.

---

## Step 2: The Generated Build Script

The generated `build.rs` generates the parser at build time:

```rust
use std::path::PathBuf;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(adze_unsafe_attrs)");
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/grammar.rs");
    adze_tool::build_parsers(&PathBuf::from("src/lib.rs"));
}
```

The generated `src/lib.rs` exports the generated parser module:

```rust
#[path = "grammar.rs"]
mod grammar_file;

pub use grammar_file::grammar;
```

---

## Step 3: Read the grammar

The generated `src/grammar.rs` defines the grammar as Rust types:

```rust
//! calc grammar definition

#[adze::grammar("calc")]
pub mod grammar {
    /// Arithmetic expression grammar.
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub enum Expr {
        Number(
            #[adze::leaf(pattern = r"\d+", try_transform = |v| v.parse::<i32>())]
            i32,
        ),

        #[adze::prec_left(1)]
        Add(Box<Expr>, #[adze::leaf(text = "+")] (), Box<Expr>),

        #[adze::prec_left(2)]
        Mul(Box<Expr>, #[adze::leaf(text = "*")] (), Box<Expr>),
    }

    /// Whitespace ignored between tokens.
    #[adze::extra]
    #[allow(dead_code)]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s+")]
        _ws: (),
    }
}
```

---

## Step 4: Parse and handle errors

The generated `examples/parse.rs` parses into those types:

```rust
use calc::grammar;

fn main() {
    let source = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "1 + 2 * 3".to_string());

    match grammar::parse(&source) {
        Ok(expr) => println!("{expr:?}"),
        Err(errors) => {
            for error in &errors {
                eprintln!("{}", error.display_with_source(&source));
            }
            std::process::exit(1);
        }
    }
}
```

`grammar::parse` returns `Result<Expr, Vec<ParseError>>`, so the error
arm binds the whole `Vec<ParseError>` and renders each `ParseError`
with `display_with_source`. A single `ParseError` implements
`Display`, but a `Vec<ParseError>` does not, so formatting the vector
itself with `{}` does not compile.

Run it with valid and invalid input:

```bash
cargo run --quiet --example parse -- "1 + 2 * 3"
cargo run --quiet --example parse -- "1 + @"
```

The second command exits nonzero and renders the parse errors with
their byte spans, expected tokens, and a source caret.

---

## What Just Happened?

1. **`cargo run -p adze-cli -- init calc`** - Generates the starter project
2. **`#[adze::grammar("calc")]`** - Declares a grammar named "calc"
3. **`#[adze::language]`** - Marks the root type for parsing
4. **`#[adze::leaf]`** - Defines how to match text (`pattern` or `text`)
5. **`#[adze::prec_left(N)]`** - Sets operator precedence (higher = tighter)
6. **`build.rs`** - Generates the parser at build time
7. **`grammar::parse()`** - Parses text into your Rust type, returning
   `Result<Expr, Vec<ParseError>>`

---

## Next Steps

### Try Different Inputs

```bash
cargo run --quiet --example parse -- "1 + 2"
cargo run --quiet --example parse -- "5 * 6"
cargo run --quiet --example parse -- "1 + 2 * 3"
cargo run --quiet --example parse -- "10"
cargo run --quiet --example parse -- "1 + @"
```

### Add More Operators

Add these variants inside the `Expr` enum in `src/grammar.rs`:

```text
#[adze::prec_left(1)]
Sub(Box<Expr>, #[adze::leaf(text = "-")] (), Box<Expr>),

#[adze::prec_left(2)]
Div(Box<Expr>, #[adze::leaf(text = "/")] (), Box<Expr>),
```

### Add Parentheses

Add this variant inside the `Expr` enum in `src/grammar.rs`:

```text
Paren(
    #[adze::leaf(text = "(")] (),
    Box<Expr>,
    #[adze::leaf(text = ")")] (),
),
```

---

## Common Issues

### Build error: "parser not found"

**Solution**: Make sure `build.rs` exists and contains the `build_parsers` call.

### Parse error: "unexpected token"

**Solution**: Check your `pattern` regex or `text` matches what you're parsing.
The parse example renders each error with its source span and expected
tokens.

### Wrong precedence

**Solution**: Higher `prec_left` numbers bind tighter. Multiplication (2) should be higher than addition (1).

---

## Learn More

- [Build your first parser](./docs/tutorials/quickstart-10-minutes.md)
- [Getting started](./docs/tutorials/getting-started.md)
- **More Examples**: [example/src/](./example/src/)
- **API Docs**: [docs/reference/api.md](./docs/reference/api.md)
- **Architecture**: [docs/explanations/architecture.md](./docs/explanations/architecture.md)

---

## Get Help

- **Questions?** Check [FAQ.md](./FAQ.md)
- **Stuck?** [GitHub Issues](https://github.com/EffortlessMetrics/adze/issues)
- **Want to contribute?** See [CONTRIBUTING.md](./CONTRIBUTING.md)

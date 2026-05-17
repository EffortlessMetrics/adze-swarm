# Quickstart: First Parser In 10 Minutes

This is the shortest supported path from a clean machine to a working Adze
parser project.

## Create The Project

Install the CLI and generate the starter parser:

```bash
cargo install adze-cli
adze init calc
cd calc
cargo test
cargo run --example parse -- "1 + 2 * 3"
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

The starter grammar parses arithmetic expressions into typed Rust values.

## The Core Idea

Adze grammars are Rust types:

```rust
#[adze::grammar("calc")]
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
```

Then parse into those types:

```rust
let expr = calc::grammar::parse("1 + 2 * 3")?;
```

For the generated starter, that expression parses as:

```text
Add(Number(1), (), Mul(Number(2), (), Number(3)))
```

## Diagnostics

The generated tests also show the diagnostic path:

```rust
let source = "1 + @";
let errors = calc::grammar::parse(source).expect_err("bad input should fail");
let first = errors.first().expect("expected at least one error");

assert_eq!(first.byte_span(), 4..5);
```

For tooling or editor integration, use the document path:

```rust
let document = calc::grammar::parse_document("1 +")?;
assert!(document.tree().has_errors());
assert!(!document.diagnostics().is_empty());
```

## What To Read Next

- [Mental Model](../explanations/mental-model.md) explains how Rust types,
  generated parsers, `parse()`, and `parse_document()` fit together.
- [GLR Quickstart](./glr-quickstart.md) introduces ambiguous grammars.
- [Support Tiers](../status/SUPPORT_TIERS.md) shows which surfaces are stable,
  stabilizing, experimental, or advisory.

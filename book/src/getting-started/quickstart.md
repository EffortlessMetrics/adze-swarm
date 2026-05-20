# Quick Start

This walkthrough builds a small arithmetic parser in a fresh Rust crate. The
parser is generated from Rust types and returns those typed values directly.

## Create a Project

```bash
cargo new adze-quickstart --lib
cd adze-quickstart
```

## Installation

Add Adze to `Cargo.toml`:

The versioned dependency block is the intended release-surface shape after the
coordinated publish. Current repo proof uses local/path dependencies from this
checkout until crates.io receipts exist for the co-release crates.

```toml
[dependencies]
adze = { version = "0.9.0", default-features = false }

[build-dependencies]
adze-tool = "0.9.0"

[features]
default = ["pure-rust"]
pure-rust = ["adze/pure-rust"]
```

Create `build.rs` in the project root:

```rust
use std::path::PathBuf;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(adze_unsafe_attrs)");
    adze_tool::build_parsers(&PathBuf::from("src/lib.rs"));
}
```

## Create the Grammar

Create `src/lib.rs`:

```rust
#[adze::grammar("book_quickstart_arithmetic")]
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

#[cfg(test)]
mod tests {
    use super::grammar::{self, Expr};

    #[test]
    fn parses_typed_ast_with_precedence() {
        let expr = grammar::parse("1 + 2 * 3").expect("expression should parse");

        assert_eq!(
            expr,
            Expr::Add(
                Box::new(Expr::Number(1)),
                (),
                Box::new(Expr::Mul(
                    Box::new(Expr::Number(2)),
                    (),
                    Box::new(Expr::Number(3)),
                )),
            )
        );
    }
}
```

Run the tests:

```bash
cargo test
```

The generated `grammar::parse` function returns `Result<Expr, Vec<ParseError>>`.
The value is the AST type you wrote, not a generic parse tree that needs a
second hand-written mapping layer.

## Parse Errors

Adze returns structured parse errors for invalid input. Add this test to
`src/lib.rs` if you want to inspect the diagnostic shape:

```rust
#[test]
fn reports_expected_tokens_for_bad_input() {
    let source = "1 + @";
    let errors = grammar::parse(source).expect_err("bad input should fail");
    let first = errors.first().expect("at least one parse error");

    assert_eq!(first.byte_span(), 4..5);
    assert!(first.expected.iter().any(|name| name == r"/\d+/"));

    let rendered = first.display_with_source(source).to_string();
    assert!(rendered.contains("bytes 4..5"));
    assert!(rendered.contains("expected one of:"));
    assert!(rendered.contains("    ^"));
}
```

## What Happened

During `cargo build`, `adze-tool` reads `src/lib.rs`, extracts the
`#[adze::grammar]` module, builds parse tables, and writes generated Rust into
Cargo's build output directory. The compiled crate then exposes
`grammar::parse(input)` from the generated parser module.

## Next Steps

- [Grammar Definition](../guide/grammar-definition.md) covers the attributes in detail.
- [GLR Precedence Resolution](../guide/glr-precedence-resolution.md) explains how conflicts are resolved.
- [Error Recovery](../guide/error-recovery.md) covers parser diagnostics and recovery behavior.

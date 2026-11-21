# Quick Start

This walkthrough builds a small arithmetic parser in a fresh Rust crate. The
parser is generated from Rust types and returns those typed values directly.

## Create a project

Use Rust 1.95.0 or newer for this crate. A sibling crate does not inherit the
Adze checkout's toolchain file, so make sure your own Cargo uses that version
before continuing. For example, with rustup you can select it for a command
with `cargo +1.95.0`.

Create the library next to a checkout named `adze-swarm`. From their parent
directory, run:

```bash
cargo new adze-quickstart --lib
cd adze-quickstart
```

## Installation

Keep the generated `[package]` section in `Cargo.toml` and replace its empty
`[dependencies]` table with the tables below. Adjust the two paths if your
checkout is elsewhere. No published Adze packages are needed for these steps.

```toml
[dependencies]
adze = { path = "../adze-swarm/runtime", default-features = false }

[build-dependencies]
adze-tool = { path = "../adze-swarm/tool" }

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
            #[adze::leaf(pattern = r"\d+", try_transform = |v| v.parse::<i32>())]
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
A successful parse gives you the `Expr` you defined.

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
    assert!(first.expected.iter().any(|name| name == "number"));

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

## Next steps

- [Grammar definition](../guide/grammar-definition.md) explains the attributes
  used in this example.
- [Diagnostics](../guide/error-recovery.md) describes syntax and diagnostic data
  from `grammar::parse_document(source)`.
- [Which API should I use?](https://github.com/EffortlessMetrics/adze-swarm/blob/main/docs/reference/which-api-should-i-use.md)
  helps you choose a parser API.
- [Support tiers](https://github.com/EffortlessMetrics/adze-swarm/blob/main/docs/status/SUPPORT_TIERS.md)
  gives the scope and proof for each surface.

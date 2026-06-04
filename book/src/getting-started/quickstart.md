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

Choose the next page by the job you are doing:

| Goal | Next page | Boundary |
| --- | --- | --- |
| Build an application around typed Rust values | [Grammar Definition](../guide/grammar-definition.md) | Start with `grammar::parse(source)` for the typed parser path. |
| Choose between typed parsing, documents, compatibility, query, JSON, and CLI surfaces | [Which API Should I Use?](../../../docs/reference/which-api-should-i-use.md) | Support status still comes from the support-tier rows, not from this quickstart. |
| Add tooling diagnostics, ranges, or projection data | [Error Recovery](../guide/error-recovery.md) | Use `grammar::parse_document(source)` so diagnostics and projections come from one `AdzeDocument`. |
| Bring Tree-sitter-shaped tooling to Adze | [Migration Guide](migration.md) and [Tree-sitter Compatibility](../../../docs/reference/tree-sitter-compatibility.md) | Compatibility is a selected-tree adapter, not a full Tree-sitter parity claim. |
| Match syntax with Tree-sitter-style queries | [Query and Pattern Matching](../guide/query-patterns.md) and [Query Compatibility](../../../docs/reference/query-compatibility.md) | Query support is a documented subset with explicit known gaps. |
| Understand precedence and ambiguity behavior | [GLR Precedence Resolution](../guide/glr-precedence-resolution.md) | GLR behavior is proof-backed by the documented grammar classes, not a blanket parser-generality claim. |
| Check whether a surface is Stable | [Support Tiers](../../../docs/status/SUPPORT_TIERS.md) | Do not broaden claims without a proof command and limitations row. |
| Check release or install status | [Release Process](../development/release.md) and [#325](https://github.com/EffortlessMetrics/adze-swarm/issues/325) | `cargo install adze-cli` still requires a crates.io install receipt before it becomes a public claim. |

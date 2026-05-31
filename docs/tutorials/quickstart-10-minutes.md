# Quickstart: First Parser In 10 Minutes

This is the shortest currently proven path from a repo checkout to a working
Adze parser project. The intended published install command is
`cargo install adze-cli`, but the current repo proof uses the CLI built from
this checkout until `adze-cli` is published as a crates.io install surface.

## Create The Project

Build the CLI from this checkout and generate the starter parser:

```bash
cargo run -p adze-cli -- init calc
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
The repository keeps the same user-shaped proof in `testing/downstream-starter`,
which builds through a normal `build.rs`, tests the generated parser API, and
runs the parse example from outside the main workspace. That proof covers the
starter-project shape and downstream wiring; it does not prove crates.io
installation until the CLI is published.

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

Choose the next page by the job you are doing:

| Goal | Next page | Boundary |
| --- | --- | --- |
| Build an application around typed Rust values | [Which API Should I Use?](../reference/which-api-should-i-use.md) | Start with `grammar::parse(source)` for the Stable typed-parser path. |
| Add tooling diagnostics, ranges, or projection data | [Diagnostics And Recovery](../reference/diagnostics-and-recovery.md) | Use `grammar::parse_document(source)` so diagnostics and projections come from the same `AdzeDocument`. |
| Bring Tree-sitter-shaped tooling to Adze | [Migrating From Tree-sitter](../reference/migrating-from-tree-sitter.md) | Tree-sitter compatibility is a selected-tree adapter, not a full parity claim. |
| Match syntax with Tree-sitter-style queries | [Query Compatibility](../reference/query-compatibility.md) | Query support is a documented subset with explicit known gaps. |
| Use CLI commands beyond the starter | [`adze-cli` README](../../cli/README.md) | CLI flows are scoped to the current support-tier rows; release-surface `cargo install adze-cli` still needs the #325 crates.io install receipt. |
| Check whether a surface is Stable | [Support Tiers](../status/SUPPORT_TIERS.md) | Do not broaden claims without a proof command and limitations row. |

- [Mental Model](../explanations/mental-model.md) explains how Rust types,
  generated parsers, `parse()`, and `parse_document()` fit together.
- [GLR Quickstart](./glr-quickstart.md) introduces ambiguous grammars.
- [Support Tiers](../status/SUPPORT_TIERS.md) shows which surfaces are stable,
  stabilizing, experimental, or advisory.

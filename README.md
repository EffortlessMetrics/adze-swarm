# Adze

When you change the syntax of a language, you need to keep its grammar and the
Rust types your application reads in sync. Adze puts the grammar rules on those
Rust types. Cargo generates the parser at build time, and `grammar::parse`
returns values of the types you defined.

## Try the starter

Run these commands from the root of an Adze checkout, which pins Rust 1.95.0.
Use a project name other than `calc` if that directory is already in use.

```bash
cargo run -p adze-cli -- init calc
cd calc
cargo test
cargo run --quiet --example parse -- "1 + 2 * 3"
```

```text
Add(Number(1), (), Mul(Number(2), (), Number(3)))
```

The starter uses the CLI and dependencies from this checkout. For a walkthrough
of the generated files and a bad-input example, see [Build your first parser](./docs/tutorials/quickstart-10-minutes.md).

## How the grammar is written

In the starter, the Rust enum describes the values you get back. The attributes
specify the tokens and how the operators bind: `*` binds tighter than `+`.

```rust
#[adze::grammar("arithmetic")]
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
```

Call the generated parser from your code:

```rust
let expr = grammar::parse("1 + 2 * 3").expect("valid arithmetic");
```

Use `try_transform` when converting a token can fail. In this grammar,
`2147483648` matches the digit pattern but is too large for `i32`, so parsing it
returns a conversion error for that token.

## Use Adze in an existing crate

Use Rust 1.95.0 or newer in your crate; it does not inherit the checkout's
toolchain. This example assumes your crate is next to a checkout named
`adze-swarm`. Adjust the paths if the checkout is elsewhere.

Merge these entries into the corresponding tables in your `Cargo.toml`.
If your crate already has default features, keep them and add `"pure-rust"`.

```toml
[dependencies]
adze = { path = "../adze-swarm/runtime", default-features = false }

[build-dependencies]
adze-tool = { path = "../adze-swarm/tool" }

[features]
default = ["pure-rust"]
pure-rust = ["adze/pure-rust"]
```

Add a `build.rs` pointing to the file that contains your `#[adze::grammar]` module:

```rust
use std::path::PathBuf;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(adze_unsafe_attrs)");
    // src/lib.rs for library crates, src/main.rs for binary crates
    adze_tool::build_parsers(&PathBuf::from("src/lib.rs"));
}
```

Put the grammar module above in `src/lib.rs`. Use `src/main.rs` in the build
script if the grammar is defined in a binary crate's entry point instead.

## Choose an API

- Use `grammar::parse(source)` when your application needs a typed Rust value.
  In the example above, it returns `Result<Expr, Vec<ParseError>>`.
- Use `grammar::parse_document(source)` when a tool needs syntax nodes, source
  ranges, or diagnostics from a document. This API is **Stabilizing**.

[Which API should I use?](./docs/reference/which-api-should-i-use.md) describes
these paths and the scope of queries and document projections.

## Current status

The Stable core covers typed extraction with the pure-Rust parser for documented
grammar shapes, precedence for expressions like the one above, and core
parse-table serialization. The starter CLI and native document API are Stabilizing. See
[Support tiers](./docs/status/SUPPORT_TIERS.md) for exact scope and proof and
[Known limitations](./docs/reference/known-limitations.md) for gaps. The
[release tracker](https://github.com/EffortlessMetrics/adze-swarm/issues/325)
records publication and crates.io install verification separately from checkout builds.

<details>
<summary>Proof for the Stable capabilities above</summary>

### Stable proof commands

- Typed extraction: `just ci-supported`; `cargo test -p adze --features pure-rust --test typed_ast_contract typed_ast_contract_left_associative_addition -- --exact --nocapture`; `cargo test -p adze --features pure-rust --test typed_ast_contract typed_ast_contract_repeated_parse_is_deterministic -- --exact --nocapture`
- Pure-Rust parser: `just ci-supported`; `cargo test -p adze-cli readme_arithmetic_quickstart_builds_and_runs -- --exact --nocapture`; `cargo test -p adze-cli getting_started_quickstart_builds_parses_and_reports_diagnostics -- --exact --nocapture`; `cargo test -p adze-cli book_quickstart_builds_parses_and_reports_diagnostics -- --exact --nocapture`; `cargo test -p downstream-demo -- --nocapture`; `cargo run -p downstream-demo --quiet`
- Operator precedence: `cargo test -p adze-cli readme_arithmetic_quickstart_builds_and_runs -- --exact --nocapture`; `cargo test -p adze-glr-core --test ambiguity_detection_comprehensive test_precedence_resolves_add_mul -- --exact --nocapture`
- Serialization (core tables): `cargo test -p adze-glr-core --features serialization --doc`; `cargo test -p adze-glr-core --features serialization --test serialization_v9 sv9_complex_precedence_roundtrip -- --exact --nocapture`

</details>

## Documentation

- [Getting started](./docs/tutorials/getting-started.md): wire up a crate and learn the grammar attributes.
- [Grammar examples](./docs/reference/grammar-examples.md): patterns for common constructs.
- [Architecture](./docs/explanations/architecture.md): how the macro, build tool, and runtime fit together.
- [Documentation index](./docs/README.md) and [navigation](./NAVIGATION.md): guides and references by task.

## Contributing

See [Contributing](./CONTRIBUTING.md), the [developer guide](./docs/DEVELOPER_GUIDE.md),
and the [roadmap](./ROADMAP.md). For the supported-core checks from a checkout,
run `just ci-supported`. Use `just ci-product-stable` to check the
quickstart examples and other Stable canaries.

## License

Adze, formerly `rust-sitter`, is licensed under either [Apache-2.0](./LICENSE-APACHE)
or [MIT](./LICENSE-MIT), at your option.

<details>
<summary>Public project badges</summary>

<!-- Public Shields endpoint JSON under badges/ is generated by `cargo xtask badges`; verify with `cargo xtask badges --check`. -->

<p align="center">
  <a href="https://github.com/EffortlessMetrics/adze/actions/workflows/ci.yml"><img src="https://github.com/EffortlessMetrics/adze/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI" /></a>
  <a href="https://codecov.io/gh/EffortlessMetrics/adze"><img src="https://codecov.io/gh/EffortlessMetrics/adze/graph/badge.svg?branch=main" alt="Codecov" /></a>
  <a href="https://github.com/EffortlessMetrics/ripr/blob/main/docs/BADGE_POLICY.md"><img src="https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/EffortlessMetrics/adze/main/badges/ripr-plus.json" alt="ripr+" /></a>
  <a href="https://crates.io/crates/adze"><img src="https://img.shields.io/crates/v/adze" alt="Crates.io" /></a>
  <a href="https://docs.rs/adze"><img src="https://img.shields.io/docsrs/adze" alt="docs.rs" /></a>
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue" alt="License: MIT OR Apache-2.0" /></a>
</p>

<p align="center">
  <a href="https://github.com/EffortlessMetrics/adze/releases"><img src="https://img.shields.io/github/v/release/EffortlessMetrics/adze?sort=semver&label=release" alt="GitHub release" /></a>
  <a href="https://crates.io/crates/adze"><img src="https://img.shields.io/crates/d/adze.svg?label=crates.io%20downloads" alt="crates.io downloads" /></a>
</p>

</details>

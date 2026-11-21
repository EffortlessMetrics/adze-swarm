# Build your first parser

Use the CLI from an Adze checkout to generate an arithmetic parser. If `calc`
already exists, choose an unused project name. Run the commands from the root
of the checkout, which pins Rust 1.95.0.

## Create the project

```bash
cargo run -p adze-cli -- init calc
cd calc
cargo test
cargo run --quiet --example parse -- "1 + 2 * 3"
```

The example prints the parsed expression as a Rust value. The multiplication is
inside the addition because `*` has higher precedence.

```text
Add(Number(1), (), Mul(Number(2), (), Number(3)))
```

The generated project depends on the local checkout. This walkthrough does not
require a published CLI or published Adze crates.

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

## Read the grammar

Open `src/grammar.rs`. The enum defines the Rust values the parser returns;
the attributes define the tokens and operator precedence. The grammar module
looks like this:

```rust
#[adze::grammar("calc")]
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

From code in the generated project, call the parser like this:

```rust
let expr = calc::grammar::parse("1 + 2 * 3").expect("valid arithmetic");
```

A `Number` contains an `i32`. The `try_transform` runs the conversion after the
token matches; it can report a conversion error if the number is out of range.

## Read a diagnostic

Try a token the grammar doesn't accept:

```bash
cargo run --quiet --example parse -- "1 + @"
```

The example exits with an error and points to `@`. The generated tests check
the same span using the typed parser:

```rust
let source = "1 + @";
let errors = calc::grammar::parse(source).expect_err("bad input should fail");
let first = errors.first().expect("expected at least one error");

assert_eq!(first.byte_span(), 4..5);
```

For tooling that needs syntax nodes or diagnostics, the generated tests also
show `parse_document`. The document API is **Stabilizing**:

```rust
let document = calc::grammar::parse_document("1 +").expect("document parses recoverably");
assert!(document.tree().has_errors());
assert!(!document.diagnostics().is_empty());
```

## Next steps

- [Getting started](./getting-started.md) covers wiring up a crate and writing
  grammar rules.
- [Which API should I use?](../reference/which-api-should-i-use.md) helps you
  choose between typed values and a document.
- [Support tiers](../status/SUPPORT_TIERS.md) lists the supported surfaces and
  what is still developing.

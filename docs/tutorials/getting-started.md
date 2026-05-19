# Getting Started with Adze

> **Doc status:** Up to date for Adze 0.8.0-dev.
> If something here disagrees with the repo, treat the repo as truth
> and log it in [`docs/status/FRICTION_LOG.md`](../status/FRICTION_LOG.md).

A comprehensive guide to building parsers with Adze's macro-based grammar generation.

## Table of Contents

1. [Quick Start](#quick-start)
2. [Basic Concepts](#basic-concepts)
3. [Creating Your First Grammar](#creating-your-first-grammar)
4. [Working Examples](#working-examples)
5. [Common Patterns](#common-patterns)
6. [Troubleshooting](#troubleshooting)

## Quick Start

The fastest currently proven path is the generated starter project from a repo
checkout. The intended published install command is `cargo install adze-cli`,
but the current proof uses the CLI built from this checkout until `adze-cli` is
published as a crates.io install surface:

```bash
cargo run -p adze-cli -- init calc
cd calc
cargo test
cargo run --example parse -- "1 + 2 * 3"
```

That path teaches the default user flow:

```rust
let ast = grammar::parse("1 + 2 * 3")?;
let document = grammar::parse_document("1 +")?;
```

The repo backs this shape with `testing/downstream-starter`, a standalone
fixture that proves path dependencies, `build.rs`, generated parser imports,
diagnostics, and the parse example. Published `cargo install adze-cli` remains
release-surface work, not a proven Stable claim yet.

### Installation

Add adze to your `Cargo.toml`:

```toml
[dependencies]
adze = { version = "0.8.0-dev", default-features = false }

[build-dependencies]
adze-tool = "0.8.0-dev"

[features]
default = ["pure-rust"]
pure-rust = ["adze/pure-rust"]
```

Create `build.rs`:

```rust
use std::path::PathBuf;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(adze_unsafe_attrs)");
    adze_tool::build_parsers(&PathBuf::from("src/lib.rs"));
}
```

### Create a Simple Grammar

Create `src/lib.rs`:

```rust
#[adze::grammar("my_lang")]
pub mod grammar {
    #[adze::language]
    pub struct Program {
        // The type String automatically extracts the matched text
        #[adze::leaf(pattern = r"\d+")]
        pub number: String,
    }
}

#[cfg(test)]
mod tests {
    use super::grammar;

    #[test]
    fn test_parse() {
        let result = grammar::parse("42");
        assert!(result.is_ok());
        let program = result.unwrap();
        assert_eq!(program.number, "42");
    }
}
```

### Build and Run

```bash
cargo build
cargo test
```

That's it! You now have a working parser for numbers.

## Basic Concepts

### Grammar Attributes

- **`#[adze::grammar("name")]`**: Marks a module as a grammar definition.
- **`#[adze::language]`**: Marks a type (struct or enum) as part of the grammar.
- **`#[adze::leaf]`**: Defines a terminal symbol (token) using regex.
- **`#[adze::extra]`**: Defines symbols to ignore (like whitespace).
- **`#[adze::repeat]`**: Allows repetition of elements.
- **`#[adze::prec]`**: Sets precedence levels for disambiguation.

### Grammar Types

1. **Structs**: Sequences of required fields.
2. **Enums**: Alternatives (choice between variants).
3. **Vec<T>**: Repetition of elements.
4. **Option<T>**: Optional elements.
5. **Box<T>**: Recursive structures.
6. **String**: Extracts the text content of a token.

## Creating Your First Grammar

### Example 1: Simple Calculator

```rust
#[adze::grammar("calc")]
pub mod grammar {
    #[adze::language]
    pub struct Program {
        pub expression: Expression,
    }

    #[adze::language]
    pub enum Expression {
        Number(NumberLiteral),
        #[adze::prec_left(1)]
        Add(Box<Expression>, #[adze::leaf(text = "+")] (), Box<Expression>),
        #[adze::prec_left(2)]
        Multiply(Box<Expression>, #[adze::leaf(text = "*")] (), Box<Expression>),
    }

    #[adze::language]
    pub struct NumberLiteral {
        #[adze::leaf(pattern = r"\d+")]
        pub value: String,
    }

    #[adze::extra]
    #[allow(dead_code)]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s+")]
        _ws: (),
    }
}
```

### Example 2: Lists and Repetition

```rust
#[adze::grammar("list")]
pub mod grammar {
    #[adze::language]
    pub struct ItemList {
        #[adze::repeat(non_empty = false)]
        pub items: Vec<Item>,
    }

    #[adze::language]
    pub struct Item {
        #[adze::leaf(pattern = r"\w+")]
        pub name: String,
    }

    #[adze::extra]
    #[allow(dead_code)]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s+")]
        _ws: (),
    }
}
```

### Example 3: JSON-like Structure

```rust
#[adze::grammar("json_simple")]
pub mod grammar {
    #[adze::language]
    pub enum Value {
        Number(NumberLiteral),
        String(StringLiteral),
        Object(Object),
        Array(Array),
    }

    #[adze::language]
    pub struct Object {
        #[adze::leaf(text = "{")]
        _open: (),
        #[adze::repeat(non_empty = false)]
        pub pairs: Vec<Pair>,
        #[adze::leaf(text = "}")]
        _close: (),
    }

    #[adze::language]
    pub struct Pair {
        pub key: StringLiteral,
        #[adze::leaf(text = ":")]
        _colon: (),
        pub value: Value,
    }

    #[adze::language]
    pub struct Array {
        #[adze::leaf(text = "[")]
        _open: (),
        #[adze::repeat(non_empty = false)]
        pub items: Vec<Value>,
        #[adze::leaf(text = "]")]
        _close: (),
    }

    #[adze::language]
    pub struct NumberLiteral {
        #[adze::leaf(pattern = r"-?\d+")]
        pub value: String,
    }

    #[adze::language]
    pub struct StringLiteral {
        #[adze::leaf(pattern = r#""[^"]*""#)]
        pub value: String,
    }

    #[adze::extra]
    #[allow(dead_code)]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s+")]
        _ws: (),
    }
}
```

## Working Examples

The following examples are included in the repository and are fully working:

### test-mini

The simplest possible grammar - parses a single number.

**Location**: `/test-mini/`

**Usage**:
```bash
cd test-mini
cargo test
```

### test-vec-wrapper

Demonstrates Vec repetition with multiple numbers.

**Location**: `/grammars/test-vec-wrapper/`

**Features**:
- Repetition with `Vec<>`
- Multiple statements
- Whitespace handling

### arithmetic (example crate)

Full expression parser with precedence.

**Location**: `/example/src/arithmetic.rs`

**Features**:
- Operator precedence
- Left associativity
- Recursive expressions

## Common Patterns

### Text Extraction

To extract the actual text from a token, simply use the `String` type for your field:

```rust
#[adze::leaf(pattern = r"\d+")]
pub value: String,
```

Adze automatically extracts the text corresponding to the token.

### Ignoring Whitespace

Use `#[adze::extra]` for whitespace:

```rust
#[adze::extra]
#[allow(dead_code)]
struct Whitespace {
    #[adze::leaf(pattern = r"\s+")]
    _ws: (),
}
```

### Precedence

Use precedence numbers for operators (higher = tighter binding):

```rust
#[adze::prec_left(1)]  // Lower precedence
Add(Box<Expression>, #[adze::leaf(text = "+")] (), Box<Expression>),

#[adze::prec_left(2)]  // Higher precedence
Multiply(Box<Expression>, #[adze::leaf(text = "*")] (), Box<Expression>),
```

This ensures `1 + 2 * 3` parses as `1 + (2 * 3)`.

### Repetition

For zero or more items:
```rust
#[adze::repeat(non_empty = false)]
pub items: Vec<Item>,
```

For one or more items:
```rust
#[adze::repeat(non_empty = true)]
pub items: Vec<Item>,
```

### Optional Elements

Use `Option<T>` for optional elements:

```rust
pub optional_field: Option<Identifier>,
```

## Troubleshooting

### Common Issues

#### "Could not determine enum variant from tree structure"

This error occurs when the Extract trait cannot determine which variant to use. Make sure your enum variants have distinct structures.

**Solution**: Ensure each variant has unique structure or use named structs instead of inline tuples.

#### Parse errors at position 0

Usually means a token_count issue or mismatched versions.

**Solution**: Upgrade to the latest Adze release and ensure `adze` and `adze-tool` versions match.

#### Empty string extraction

If you need raw token text, use `String` fields. If you need a typed value, use
`#[adze::leaf(transform = ...)]` and keep the transform small and deterministic.

### Debug Tips

1. **Add Debug derives** to your types:
   ```rust
   #[derive(Debug)]
   #[adze::language]
   pub struct MyType { ... }
   ```

2. **Check parse result**:
   ```rust
   match grammar::parse(input) {
       Ok(result) => println!("Success: {:?}", result),
       Err(errors) => println!("Errors: {:?}", errors),
   }
   ```

3. **Use `--nocapture`** with tests:
   ```bash
   cargo test -- --nocapture
   ```

## Next Steps

- Read the [API Reference](../reference/api.md) for advanced features
- Check out [Grammar Examples](../reference/grammar-examples.md) for more patterns
- Explore the [Performance Guide](../how-to/optimize-performance.md) for optimization tips
- Review the [Roadmap](../../ROADMAP.md) for planned features

## Support

- **Issues**: [GitHub Issues](https://github.com/EffortlessMetrics/adze/issues)
- **Examples**: See `/example/` and `/grammars/` directories

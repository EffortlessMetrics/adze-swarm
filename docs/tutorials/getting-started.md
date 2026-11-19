# Getting Started with Adze

This guide shows how to add Adze to a Rust crate and write a grammar for it.
If you want to run a generated project first, start with
[Build your first parser](./quickstart-10-minutes.md).

## Table of Contents

1. [Quick Start](#quick-start)
2. [Basic Concepts](#basic-concepts)
3. [Creating Your First Grammar](#creating-your-first-grammar)
4. [Working Examples](#working-examples)
5. [Common Patterns](#common-patterns)
6. [Troubleshooting](#troubleshooting)

## Quick Start

This example builds a library that parses a number into a Rust struct.

Use Rust 1.95.0 or newer for this crate. A sibling crate does not inherit the
Adze checkout's toolchain file, so make sure your own Cargo uses that version
before continuing. For example, with rustup you can select it for a command
with `cargo +1.95.0`.

### Installation

Create a library next to a checkout named `adze-swarm`. From their parent
folder, run:

```bash
cargo new --lib number-parser
cd number-parser
```

Keep the generated `[package]` section in `Cargo.toml` and replace its empty
`[dependencies]` table with the tables below. Adjust the two paths if your
checkout is elsewhere.

```toml
[dependencies]
adze = { path = "../adze-swarm/runtime", default-features = false }

[build-dependencies]
adze-tool = { path = "../adze-swarm/tool" }

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

### Run the test

```bash
cargo test
```

The test checks that `grammar::parse("42")` returns a `Program` with `number`
set to the string `"42"`.

## Basic Concepts

### Grammar Attributes

- **`#[adze::grammar("name")]`**: Marks a module as a grammar definition.
- **`#[adze::language]`**: Marks the type returned by the generated parser.
- **`#[adze::leaf]`**: Matches a token by literal text or a regular expression.
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

These examples show common grammar shapes. See the
[generated grammar feature matrix](../product/GENERATED_GRAMMAR_FEATURE_MATRIX.md)
for the tested scope of repetition, optional elements, and other constructs.

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

The following examples are included in the repository as tutorial and smoke
fixtures. Their support status follows
[`SUPPORT_TIERS.md`](../status/SUPPORT_TIERS.md), local status notes, and the
known-limitations reference; do not treat every example as a stable language
package.

### Downstream arithmetic demo

[`samples/downstream-demo`](../../samples/downstream-demo) shows the parser in
a library and a small executable. From the checkout root, run:

```bash
cargo run -p downstream-demo --quiet
```

### test-vec-wrapper

Demonstrates Vec repetition with multiple numbers.

**Location**: `/grammars/test-vec-wrapper/`

**Features**:
- Repetition with `Vec<>`
- Multiple statements
- Whitespace handling

### arithmetic (example crate)

An arithmetic grammar with operator precedence.

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

Inspect the first error and its expected tokens. Check that the first token in
the input can match the grammar, that `build.rs` points to the grammar source,
and that `adze` and `adze-tool` come from the same checkout.

#### Empty string extraction

If you need the matched text, use a `String` field. If you need to convert the
text to a typed value and that conversion can fail, use `try_transform`.

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

# Tutorial: Building a JSON Parser with Adze

In this tutorial, we will build a fully functional JSON parser from scratch using Adze. You will learn how to define complex grammars using Rust enums and structs, handle recursive structures, and extract typed data.

## What You'll Build

We will implement a parser for a subset of JSON that supports:
- Numbers and Strings
- Arrays
- Objects (key-value pairs)
- Nested structures

## Step 1: Project Setup

Create a new Rust library project:

```bash
cargo new --lib adze-json-demo
cd adze-json-demo
```

Add Adze to your `Cargo.toml`:

The versioned dependency block below is a release-surface shape for the
coordinated 0.9.0 publish. Current repo proof uses local/path dependencies until
the crates.io metadata and install receipts exist.

```toml
[dependencies]
adze = "0.9.0"

[build-dependencies]
adze-tool = "0.9.0"
```

Create a `build.rs` file in the root of your project:

```rust
use std::path::PathBuf;

fn main() {
    adze_tool::build_parsers(&PathBuf::from("src/lib.rs"));
}
```

## Step 2: Define the AST Types

In JSON, a value can be many things. We'll use a Rust `enum` to represent these alternatives.

Open `src/lib.rs` and add the following:

```rust
#[adze::grammar("json")]
pub mod grammar {
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub enum Value {
        Number(NumberLiteral),
        String(StringLiteral),
        Object(Object),
        Array(Array),
        True(#[adze::leaf(text = "true")] ()),
        False(#[adze::leaf(text = "false")] ()),
        Null(#[adze::leaf(text = "null")] ()),
    }

    #[derive(Debug, PartialEq)]
    pub struct NumberLiteral {
        #[adze::leaf(pattern = r"-?\d+(\.\d+)?")]
        pub value: String,
    }

    #[derive(Debug, PartialEq)]
    pub struct StringLiteral {
        // Simple string pattern: anything between double quotes
        #[adze::leaf(pattern = r#""[^"]*""#)]
        pub value: String,
    }
}
```

## Step 3: Handle Recursive Structures

JSON objects and arrays can contain other JSON values. We use `Box<T>` or `Vec<T>` to handle this recursion.

Update your `grammar` module in `src/lib.rs`:

```rust
    // ... inside pub mod grammar ...

    #[derive(Debug, PartialEq)]
    pub struct Object {
        #[adze::leaf(text = "{")]
        _open: (),
        
        #[adze::delimited(#[adze::leaf(text = ",")] ())]
        pub pairs: Vec<Pair>,
        
        #[adze::leaf(text = "}")]
        _close: (),
    }

    #[derive(Debug, PartialEq)]
    pub struct Pair {
        pub key: StringLiteral,
        #[adze::leaf(text = ":")]
        _colon: (),
        pub value: Box<Value>, // Use Box for recursion
    }

    #[derive(Debug, PartialEq)]
    pub struct Array {
        #[adze::leaf(text = "[")]
        _open: (),
        
        #[adze::delimited(#[adze::leaf(text = ",")] ())]
        pub items: Vec<Value>,
        
        #[adze::leaf(text = "]")]
        _close: (),
    }

    #[adze::extra]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s+")]
        _ws: (),
    }
```

## Step 4: Add a Test Case

Now let's add a test to verify our parser works with a complex nested JSON string.

Add this to the bottom of `src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::grammar;

    #[test]
    fn test_json_parse() {
        let input = r#"{ "key": [1, true, null], "nested": { "ok": false } }"#;
        let result = grammar::parse(input);
        
        assert!(result.is_ok(), "Parse failed: {:?}", result.err());
        println!("Parsed: {:#?}", result.unwrap());
    }
}
```

## Step 5: Run the Parser

```bash
cargo test -- --nocapture
```

You should see the structured Rust representation of your JSON string printed to the console!

## Key Concepts Learned

1. **`#[adze::language]`**: Identifies the root and component types of your grammar.
2. **`enum` for Choice**: Represents alternatives like `true` vs `false` vs `number`.
3. **`Vec<T>` and `#[adze::delimited]`**: Handles sequences of items with a separator.
4. **`Box<T>` for Recursion**: Allows types to contain themselves (essential for tree structures).
5. **`#[adze::extra]`**: Configures the parser to automatically skip whitespace or comments.

## Next Steps

- Try adding support for escape sequences in strings.
- Implement a `From<Value>` trait to convert the AST into your own internal data model.
- Check out the [Grammar Reference](../reference/grammar-examples.md) for more complex patterns.

# Tutorial: Building a JSON Parser with Adze

You'll build a parser for a small subset of JSON. The grammar uses Rust
enums and structs to represent nested objects and arrays as typed values.

The example handles nonnegative integers, quoted ASCII words, arrays, objects,
and `true`, `false`, and `null`. Object keys use the same quoted words. This is
a tutorial grammar, not a complete JSON validator or a separate `adze-json`
package. It does not cover general string escapes or the whole JSON number
syntax. The grammar skips whitespace inside the quotes around a word and does
not reject leading zeros in integers.

## Step 1: Project Setup

Use Rust 1.95.0 or newer. This example uses the Adze source checkout, not
published Adze packages. From the parent directory of a checkout named
`adze-swarm`, create the library next to it:

```bash
cargo new --lib adze-json-demo
cd adze-json-demo
```

Keep the generated `[package]` section in `Cargo.toml`. Replace its empty
`[dependencies]` table with the tables below, adjusting the paths if the
checkout is elsewhere:

```toml
[dependencies]
adze = { path = "../adze-swarm/runtime", default-features = false }

[build-dependencies]
adze-tool = { path = "../adze-swarm/tool" }

[features]
default = ["pure-rust"]
pure-rust = ["adze/pure-rust"]
```

Create a `build.rs` file in the root of your project:

```rust
use std::path::PathBuf;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(adze_unsafe_attrs)");
    adze_tool::build_parsers(&PathBuf::from("src/lib.rs"));
}
```

## Step 2: Define the AST Types

In JSON, a value can be many things. We'll use a Rust `enum` to represent these alternatives.

Replace the generated contents of `src/lib.rs` with the following:

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
        #[adze::leaf(pattern = r"\d+")]
        pub value: String,
    }

    #[derive(Debug, PartialEq)]
    pub struct StringLiteral {
        #[adze::leaf(text = "\"")]
        _open: (),
        #[adze::leaf(pattern = r"\w+")]
        pub value: String,
        #[adze::leaf(text = "\"")]
        _close: (),
    }
}
```

## Step 3: Handle Recursive Structures

JSON objects and arrays can contain other JSON values. We use `Box<T>` or `Vec<T>` to handle this recursion.

Insert the following before the closing brace of `mod grammar` in `src/lib.rs`:

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
    #[allow(dead_code)]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s+")]
        _ws: (),
    }
```

## Step 4: Add a Test Case

Now let's add a test that exercises the parser with a nested JSON string.

Append this after the closing brace of `mod grammar`, at the bottom of
`src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::grammar;

    #[test]
    fn parses_nested_values() {
        use grammar::Value;

        let input = r#"{ "key": [1, true, false, null], "nested": { "tag": "ok" } }"#;
        let parsed = grammar::parse(input).expect("valid tutorial input");
        let Value::Object(object) = &parsed else {
            panic!("expected an object");
        };
        assert_eq!(object.pairs.len(), 2);
        assert_eq!(object.pairs[0].key.value, "key");

        let Value::Array(array) = object.pairs[0].value.as_ref() else {
            panic!("expected an array");
        };
        assert!(matches!(
            array.items.as_slice(),
            [Value::Number(number), Value::True(()), Value::False(()), Value::Null(())]
                if number.value == "1"
        ));

        assert_eq!(object.pairs[1].key.value, "nested");
        let Value::Object(nested) = object.pairs[1].value.as_ref() else {
            panic!("expected a nested object");
        };
        assert_eq!(nested.pairs.len(), 1);
        assert_eq!(nested.pairs[0].key.value, "tag");
        assert!(matches!(
            nested.pairs[0].value.as_ref(),
            Value::String(text) if text.value == "ok"
        ));
    }

    #[test]
    fn rejects_a_missing_colon() {
        assert!(grammar::parse(r#"{ "key" 1 }"#).is_err());
    }
}
```

## Step 5: Run the Parser

```bash
cargo test -- --nocapture
```

The tests check the typed values of the nested input and reject an object entry
with no colon.

## Key Concepts Learned

1. **`#[adze::language]`**: Marks the root type returned by the generated parser.
2. **`enum` for Choice**: Represents alternatives like `true` vs `false` vs `number`.
3. **`Vec<T>` and `#[adze::delimited]`**: Handles sequences of items with a separator.
4. **`Box<T>` for Recursion**: Allows types to contain themselves (essential for tree structures).
5. **`#[adze::extra]`**: Configures the parser to automatically skip whitespace or comments.

## Next Steps

- Read the [grammar examples](../reference/grammar-examples.md) for other token and grammar shapes.
- Implement a `From<Value>` trait to convert the AST into your own internal data model.

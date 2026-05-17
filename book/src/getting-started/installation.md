# Installation

This chapter covers how to install and set up Adze in your project.

## Prerequisites

- Rust 1.95.0 or later (2024 edition)
- Cargo (comes with Rust)

## Adding Dependencies

Add Adze to your `Cargo.toml`:

```toml
[dependencies]
adze = { version = "0.8.0-dev", default-features = false }

[build-dependencies]
adze-tool = "0.8.0-dev"

[features]
default = ["pure-rust"]
pure-rust = ["adze/pure-rust"]
```

The generated pure-Rust parser path is the supported front door. Lower-level
compatibility, WASM, CLI, and experimental runtime surfaces follow their
support-tier rows and should not be treated as the default installation path.

## Feature Posture

| Feature | Use | Support posture |
|---|---|---|
| `pure-rust` | Generated parser front door | Stable |
| `glr` | Ambiguous grammar conflict routing | Stabilizing |
| `serialization` | Core table serialization and document JSON | Stable/advisory by surface |
| `ts-compat` | Tree-sitter-compatible selected-tree adapter | Advisory |
| `incremental_glr` | Incremental lifecycle experiments | Experimental |

## Build Configuration

Create a `build.rs` file in your project root:

```rust
use std::path::PathBuf;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(adze_unsafe_attrs)");
    adze_tool::build_parsers(&PathBuf::from("src/lib.rs"));
}
```

## Verifying Installation

Create `src/lib.rs` with a tiny grammar and test:

```rust
#[adze::grammar("test")]
pub mod grammar {
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub enum Word {
        Hello(#[adze::leaf(text = "hello")] ()),
    }
}

#[cfg(test)]
mod tests {
    use super::grammar::{self, Word};

    #[test]
    fn parses_hello() {
        assert_eq!(grammar::parse("hello").unwrap(), Word::Hello(()));
    }
}
```

Run with:

```bash
cargo test
```

## Troubleshooting

### Common Issues

1. **Build fails with "cannot find macro `adze`"**
   - Ensure both `adze` and `adze-tool` are in your dependencies
   - Check that your `build.rs` is properly configured

2. **"Multiple applicable items in scope" errors**
   - This usually means you have conflicting features enabled
   - Start from the `pure-rust` feature shape above

3. **WASM compilation fails**
   - WASM is advisory compile-signal territory, not the beginner path
   - Check the current support-tier row before relying on browser/runtime behavior

## Next Steps

Now that you have Adze installed, proceed to the [Quick Start](quickstart.md)
guide to create your first grammar.

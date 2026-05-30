# Copilot Instructions — Adze

Adze is an AST-first grammar toolchain for Rust. Rust 2024 edition, MSRV 1.95.0, 29-member workspace.

## Code Conventions

### Edition and Language
- Rust 2024 edition — use `gen` keyword awareness, `unsafe_op_in_unsafe_fn` is denied
- All crates inherit `edition = "2024"` and `rust-version = "1.95.0"` from workspace
- Dependencies use `workspace = true` in per-crate Cargo.toml:
  ```toml
  [dependencies]
  serde = { workspace = true }
  thiserror = { workspace = true }
  ```

### Error Handling
- **Library crates**: Use `thiserror` for typed errors
  ```rust
  use thiserror::Error;

  #[derive(Debug, Error)]
  pub enum ParseError {
      #[error("unexpected token at position {position}")]
      UnexpectedToken { position: usize },
      #[error("grammar validation failed: {0}")]
      Validation(String),
  }
  ```
- **Application/CLI crates**: Use `anyhow` for ad-hoc errors
- Never use `.unwrap()` in library code — use `?` or explicit error handling

### Collections
- Use `rustc_hash::FxHashMap` / `FxHashSet` in hot paths (parser tables, symbol lookups)
- Use `smallvec::SmallVec` for small, stack-allocated collections
- Use `indexmap::IndexMap` when insertion order matters (grammar rules)
- Standard `HashMap`/`HashSet` only in non-performance-critical code

### Common Derives
```rust
#[derive(Debug, Clone, PartialEq, Eq)]      // Value types
#[derive(Debug, Clone, Hash, PartialEq, Eq)] // Types used as map keys
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)] // Serializable types
```

## Naming Conventions

- **Crate names**: `kebab-case` (e.g., `adze-glr-core`, `common-type-ops-core`)
- **Modules**: `snake_case`
- **Types**: `PascalCase` with purpose suffix — `SymbolId`, `ParseTable`, `ActionCell`
- **Features**: `kebab-case` (e.g., `test-api`, `glr-core`, `external-scanners`)
- **Test functions**: `test_<what>_<condition>_<expected>` (e.g., `test_parse_empty_input_returns_error`)
- **Newtype IDs**: `pub struct SymbolId(pub u16)` — numeric newtypes for type safety

## Testing Patterns

### Snapshot Testing (insta)
```rust
#[test]
fn test_grammar_output() {
    let result = process_grammar(input);
    insta::assert_snapshot!(result);
}
```
Review snapshots with `cargo insta review` or `just snap`.

### Property Testing (proptest)
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn roundtrip_serialization(input in ".*") {
        let encoded = serialize(&input);
        let decoded = deserialize(&encoded)?;
        prop_assert_eq!(input, decoded);
    }
}
```

### Feature-Gated Test Helpers
```rust
#[cfg(feature = "test-api")]
pub fn debug_parse_table(table: &ParseTable) -> String { /* ... */ }
```

### Concurrency
Default test thread count is 2 (`RUST_TEST_THREADS=2`). Do not spawn unbounded threads in tests.

### Test Organization
- Unit tests: `#[cfg(test)] mod tests` in the same file
- Integration tests: `tests/` directory in each crate
- Snapshot files: `snapshots/` directory, committed to git

## Workspace Lints

These are enforced workspace-wide via `[workspace.lints.rust]`:
```toml
unsafe_op_in_unsafe_fn = "deny"
unused_must_use = "deny"
missing_docs = "warn"
unused_extern_crates = "deny"
```

## Common Imports

```rust
// Grammar IR
use adze_ir::{Grammar, Rule, Symbol, SymbolId, SymbolKind};

// GLR core
use adze_glr_core::{ParseTable, ActionCell, Action, FirstFollowSets};

// Serialization
use serde::{Serialize, Deserialize};
use serde_json;

// Collections
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use indexmap::IndexMap;
```

## Key Architecture

```
User Rust types → #[adze::grammar] macros → adze-common expansion
    → adze-ir Grammar → adze-glr-core tables → adze-tablegen compression
    → FFI Language struct → runtime parsing
```

Core pipeline crates: `adze`, `adze-macro`, `adze-tool`, `adze-common`, `adze-ir`, `adze-glr-core`, `adze-tablegen`

## Key Commands

```bash
just ci-supported          # Local supported/product proof
cargo t2                   # Test with 2 threads
just clippy                # Lint core crates
cargo fmt --all --check    # Check formatting
cargo insta review         # Review snapshots
```

## GitHub PR Gate

For `EffortlessMetrics/adze-swarm`, the required hosted merge gate is
`Rust Small Result` from `.github/workflows/em-ci-routed-rust.yml`.

`Rust Small on CPX42`, `Rust Small on CX43`, `Rust Small on CX33`, and
`Rust Small on GitHub Hosted` are routed implementation lanes. `Rust Small on
CX53` is dormant while adze-swarm#598 is blocked. Do not treat any
implementation lane as a separate required branch-protection context.

Use `just ci-supported` for local supported/release-facing proof. It is not the
normal hosted merge gate for swarm PRs.

## Do NOT

- Add `unsafe` blocks without `// SAFETY:` comment and compliance with `unsafe_op_in_unsafe_fn`
- Add workspace members without updating root `Cargo.toml` members list
- Use `std::collections::HashMap` in parser hot paths — use `FxHashMap`
- Hardcode symbol IDs — use `SymbolId` newtype
- Use `.unwrap()` in library code
- Skip `workspace = true` for shared dependencies
- Introduce `tokio` or async without explicit approval
- Add `#[allow(unused)]` to suppress warnings — fix the root cause

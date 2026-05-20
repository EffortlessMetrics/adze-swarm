# Downstream Starter Fixture

This fixture behaves like a user crate outside the Adze workspace. It proves
that a generated parser can be consumed through local path dependencies, a
normal `build.rs`, public imports, library tests, and a runnable example.

The user path mirrors `adze init`:

```text
Rust grammar types -> generated parser -> grammar::parse(...) -> typed Expr
```

Use `grammar::parse(source)` when the application wants typed Rust values. Use
`grammar::parse_document(source)` when tooling needs recovered document facts,
diagnostics, ranges, or projection data.

Proof commands:

```bash
cargo test --manifest-path testing/downstream-starter/Cargo.toml
cargo run --manifest-path testing/downstream-starter/Cargo.toml --example parse
```

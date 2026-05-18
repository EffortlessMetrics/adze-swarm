# Downstream Starter Fixture

This fixture behaves like a user crate outside the Adze workspace. It proves
that a generated parser can be consumed through local path dependencies, a
normal `build.rs`, public imports, library tests, and a runnable example.

Proof commands:

```bash
cargo test --manifest-path testing/downstream-starter/Cargo.toml
cargo run --manifest-path testing/downstream-starter/Cargo.toml --example parse
```

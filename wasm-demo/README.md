# adze-wasm-demo

Advisory WASM compile smoke for Adze parser-facing code.

> **Support status:** this demo is outside the stable parser product contract.
> Current proof is compile-oriented: the WASM target builds and the parser-facing
> exported arithmetic function reaches the generated `grammar::parse(...)` path.
> Browser behavior, package publishing, JS API shape, and stable WASM schemas
> are not certified unless support tiers promote a narrower slice with receipts.

## Local Proof

From the repository root:

```bash
rustup target add wasm32-unknown-unknown
cargo check --manifest-path wasm-demo/Cargo.toml --target wasm32-unknown-unknown
cargo test --manifest-path wasm-demo/Cargo.toml --target wasm32-unknown-unknown --no-run
```

These commands match the advisory WASM support-tier row. They are useful build
health receipts, not a stable browser/runtime guarantee.

## Demo Build

`build.sh` uses `wasm-pack` to build a browser demo bundle. Treat that as a
manual experiment. The stable Adze user path remains generated Rust parsers that
return typed AST values through `grammar::parse(...)`.

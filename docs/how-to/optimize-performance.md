# Adze Performance Guide

Adze is designed for high performance, but parsing is complex. This guide explains how to get the most out of the generated parsers.

## Runtime Modes

Adze supports two primary runtime modes, selected at build time via features or governance contracts.

### 1. Pure Rust (Default)
Optimized for zero-dependency builds and WASM support. It uses a custom LR(1) engine.

- **Pros**: Fast, no C toolchain required, WASM-ready.
- **Cons**: Still maturing optimization compared to C runtime.

### 2. GLR (Generalized LR)
Enabled when your grammar has conflicts (ambiguities).

- **Pros**: Can parse ambiguous grammars (e.g. C++, ambiguous expressions).
- **Cons**: Slower than deterministic LR(1). Worst case $O(n^3)$, though typically near-linear.

To check if you are using GLR, inspect the build output or check your grammar for conflicts using `ADZE_EMIT_ARTIFACTS=true`.

## Optimization Tips

### 1. Enable SIMD
The `simd` feature enables AVX2/NEON accelerated lexing for common patterns.

```toml
[dependencies]
adze = { version = "0.8", features = ["simd"] }
```

### 2. Grammar Optimization
- **Avoid Ambiguity**: Ambiguities force the parser to fork (GLR), which is expensive. Resolve conflicts by rewriting rules or using precedence (`#[adze::prec_left]`) if possible.
- **Regex Performance**: `adze-ir` optimizes regexes, but simpler patterns (like string literals) are always faster to match than complex regexes.

### 3. Build Configuration
Always build with release optimizations for production use.

```toml
# Cargo.toml
[profile.release]
lto = true
codegen-units = 1
panic = "abort"
```

## Profiling

Adze uses standard Rust infrastructure. You can profile your parser using `flamegraph`.

```bash
cargo install flamegraph
cargo flamegraph --bin my-parser-app
```

## Benchmarking

We recommend `criterion` for micro-benchmarks.

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use my_grammar::grammar;

fn bench_parse(c: &mut Criterion) {
    let input = "some long input...";
    c.bench_function("parse_input", |b| b.iter(|| {
        grammar::parse(black_box(input)).unwrap()
    }));
}

criterion_group!(benches, bench_parse);
criterion_main!(benches);
```

## Memory Usage

Some Adze runtime paths include arena-backed parse-tree allocation to reduce
allocation overhead. Treat arena performance as an internal optimization detail
unless a specific support-tier row and benchmark receipt backs the claim.
- **Tree Size**: The `Extract` trait converts the raw arena nodes into your Rust structs. This involves allocation (unless you use `&str` references, which is advanced).
- **Node Reuse**: Incremental parsing (experimental) attempts to reuse subtrees.

## Debugging Performance

Set `ADZE_LOG_PERFORMANCE=true` to see internal metrics during parsing (if enabled in runtime features).

```bash
ADZE_LOG_PERFORMANCE=true cargo run --release
```

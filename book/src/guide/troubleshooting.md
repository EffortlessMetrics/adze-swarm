# Troubleshooting

This chapter covers common issues you may encounter when working with Adze grammars and parsers, along with solutions.

## "Grammar not found"

**Symptom:** Your build fails with an error indicating the grammar cannot be found or the parser language function is missing.

**Cause:** The `build.rs` script didn't run, or `adze-tool` is not listed as a build dependency.

**Solution:**

1. Ensure your `Cargo.toml` includes `adze-tool` as a build dependency:

   ```toml
   [build-dependencies]
   adze-tool = "0.1"
   ```

2. Ensure your `build.rs` calls `build_parsers()`:

   ```rust
   use std::path::PathBuf;

   fn main() {
       // Point this at the file containing your `#[adze::grammar(...)]` module.
       // Use `src/lib.rs` for library crates.
       adze_tool::build_parsers(&PathBuf::from("src/main.rs"));
   }
   ```

3. Run `cargo clean` and rebuild to force `build.rs` to re-execute.

## "Conflicting shift/reduce"

**Symptom:** The parser generator reports a shift/reduce or reduce/reduce conflict.

**Cause:** Your grammar is ambiguous — there are multiple valid parse trees for some inputs.

**Solution:**

1. Add precedence annotations to disambiguate. Use `#[adze::prec_left(N)]` or `#[adze::prec_right(N)]` on the relevant rules, where higher values bind tighter:

   ```rust
   #[adze::prec_left(1)]
   Add { left: Box<Expr>, right: Box<Expr> },

   #[adze::prec_left(2)]
   Mul { left: Box<Expr>, right: Box<Expr> },
   ```

2. See the [GLR Precedence Resolution](glr-precedence-resolution.md) chapter for detailed guidance on resolving conflicts.

3. If the grammar is intentionally ambiguous, use the GLR support-tier proof
   commands to check whether that ambiguity shape is covered. Adze exposes a
   deterministic selected parse plus ambiguity summaries for documented GLR
   cases; it does not make every ambiguous grammar a stable product claim.

## "No matching rule"

**Symptom:** Parsing fails with an error indicating no rule matches the input.

**Cause:** The input text doesn't conform to any rule in your grammar.

**Solution:**

1. Double-check your input against the grammar definition. Common mistakes include missing whitespace handling or incorrect token patterns.

2. Use `ADZE_EMIT_ARTIFACTS=true cargo build` to inspect the generated grammar JSON in `target/debug/build/<crate>-<hash>/out/`. Verify that the grammar matches your expectations.

3. Add broader error recovery rules to handle unexpected input gracefully. See the [Error Recovery](error-recovery.md) chapter.

## "Stack overflow during parse"

**Symptom:** Parsing a deeply nested input causes a stack overflow.

**Cause:** Your grammar has deeply recursive rules that exceed the default stack size.

**Solution:**

1. Prefer left-recursion over right-recursion where possible. Left-recursive rules consume constant stack space in GLR parsers:

   ```rust
   // Prefer this (left-recursive):
   List { head: Box<List>, tail: Item }

   // Over this (right-recursive):
   List { head: Item, tail: Box<List> }
   ```

2. Increase the thread stack size for parsing if needed:

   ```rust
   std::thread::Builder::new()
       .stack_size(8 * 1024 * 1024) // 8 MB
       .spawn(|| { /* parse here */ })
       .unwrap();
   ```

3. Consider restructuring deeply nested grammar rules to use repetition (`Vec<T>`) instead of recursive types.

## Feature flag issues

**Symptom:** Compilation errors related to missing types, traits, or functions when enabling or disabling features.

**Cause:** Adze uses feature flags to control optional functionality. Some feature combinations have dependencies.

**Common feature flag combinations:**

| Use case | Features |
|---|---|
| Default generated parser/runtime path | `default` |
| GLR parsing support | `glr` |
| Document JSON projection | `serialization` |
| Tree-sitter-compatible selected-tree projection | `ts-compat` |
| Incremental parsing (experimental) | `incremental_glr` |

**Solution:**

1. Check that you're enabling the correct features in `Cargo.toml`:

   ```toml
   [dependencies]
   adze = { version = "0.8", features = ["glr"] }
   ```

2. Prefer explicit feature sets that name the surface you are testing or using.
   Do not rely on broad feature aliases to imply product support.

3. When running tests across feature combinations, use focused commands:

   ```bash
   cargo test -p adze
   cargo test -p adze --features glr
   cargo test -p adze --features "pure-rust,glr"
   ```

4. If a feature is part of a public claim, confirm its status and proof command
   in `docs/status/SUPPORT_TIERS.md`.

## Build time is slow

**Symptom:** Builds take a long time, especially when grammar generation runs repeatedly.

**Solution:**

1. **Use `cargo check` during development** — it skips code generation and linking, which are the slowest steps.

2. **Cache build artifacts** — avoid running `cargo clean` unnecessarily. The `build.rs` script only re-runs when source files change.

3. **Limit parallel build jobs** if your machine is resource-constrained:

   ```bash
   CARGO_BUILD_JOBS=4 cargo build
   ```

4. **Use release mode selectively** — only use `cargo build --release` when you need optimized binaries. Debug builds are significantly faster.

5. **Split large grammars** into smaller crates so that only the changed grammar triggers a rebuild.

## WASM compilation issues

**Symptom:** Compilation fails when targeting `wasm32-unknown-unknown` or `wasm32-wasi`.

**Cause:** Browser/WASM support is an advisory surface. Core Rust parsing and
the demo compile path are separate claims; Tree-sitter-compatible projections
do not imply browser runtime support.

**Solution:**

1. Keep the runtime dependency on explicit Adze features:

   ```toml
   [dependencies]
   adze = "0.8"
   ```

2. Avoid enabling experimental or advisory features unless the target proof
   command names them.

3. If you need conditional compilation:

   ```toml
   [target.'cfg(target_arch = "wasm32")'.dependencies]
   adze = "0.8"
   ```

4. Verify the advisory WASM demo compile path with:

   ```bash
   cargo check --manifest-path wasm-demo/Cargo.toml --target wasm32-unknown-unknown
   ```

## Test failures after grammar changes

**Symptom:** Tests fail after modifying grammar definitions, with snapshot mismatches or unexpected parse tree output.

**Cause:** Grammar changes alter the generated parser, which changes parse tree output. Snapshot tests detect this as a failure.

**Solution:**

1. Review the snapshot diffs to confirm the changes are expected:

   ```bash
   cargo insta review
   ```

2. Accept the new snapshots if the output matches your expectations. Reject any that indicate a regression.

3. If you're adding new grammar rules, add corresponding test cases in the `example` crate with snapshot tests.

4. Run the full test suite to catch any downstream breakage:

   ```bash
   cargo test
   ```

5. For golden tests that compare against Tree-sitter reference implementations, regenerate the reference data if the grammar intentionally changed:

   ```bash
   UPDATE_GOLDEN=1 cargo test -p adze-golden-tests
   ```

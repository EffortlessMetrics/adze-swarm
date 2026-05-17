# Frequently Asked Questions

## General Questions

### What is Adze?

Adze is a Rust-native parser generator and runtime. It lets you define a
grammar with Rust types and macros, generate a parser at build time, and parse
directly into typed Rust values.

### How does it differ from Tree-sitter?

Tree-sitter starts with JavaScript grammar files and exposes a syntax-tree API.
Adze starts with Rust grammar types and exposes `grammar::parse()` as the
ergonomic typed-AST path. When tooling needs tree data, `parse_document()`
returns the native parse product, and Tree-sitter-compatible output is a
selected-tree adapter over that document data.

Adze does not claim full Tree-sitter runtime, query, node-types, or imported
grammar corpus parity.

### What languages are supported?

Adze can parse any language you define a grammar for. Example grammars are provided for JavaScript, Python, Go, and more. See the [Language Support](../reference/language-support.md) page for details.

## Technical Questions

### What is GLR parsing?

GLR (Generalized LR) parsing is an extension of LR parsing that can handle ambiguous grammars by maintaining multiple parse stacks. When the parser encounters ambiguity, it forks and explores all possibilities, merging when paths converge.

### Which parser path should I use?

Use the generated pure-Rust parser path by default:

- `grammar::parse(source)` for typed Rust values;
- `grammar::parse_document(source)` for diagnostics, syntax/document facts,
  ambiguity summaries, JSON, and compatibility projections.

Use the Tree-sitter-compatible adapter only when an ecosystem tool expects
Tree-sitter-shaped traversal. It is an advisory selected-tree subset, not the
native parse truth.

### How do I handle whitespace?

Define whitespace as an "extra" token that's automatically skipped:

```rust
#[adze::extra]
struct Whitespace {
    #[adze::leaf(pattern = r"\s+")]
    _ws: (),
}
```

### Can I use external scanners?

Yes, but treat them as an advanced and experimental surface. Existing examples
exercise scanner-style integration, but the full Tree-sitter external scanner
API is not a Stable product contract.

## Performance Questions

### How fast is Adze?

Performance depends on the grammar, conflict shape, generated tables,
diagnostics, and projections used. Benchmarks are advisory evidence until they
are tied to fixture-backed performance receipts and support-tier rows.

### Does it support incremental parsing?

Incremental parsing is experimental. The accepted contract is document-centered:
edits produce a new `AdzeDocument`, fallback to full reparse must be visible in
metadata, and changed ranges may be conservative. Adze does not currently make a
Stable claim about subtree reuse or edit-time speedups.

### What optimizations are available?

Use the default generated pure-Rust path first. Performance work should be
measured against the benchmark and fixture matrix instead of enabled through
undocumented feature flags.

## Troubleshooting

### "Multiple applicable items in scope" error

This usually means two generated helper traits or parser surfaces are both in
scope. Narrow the imports and use the generated module's intended public API:

```rust
let ast = grammar::parse(source)?;
let report = grammar::parse_document(source);
```

Avoid mixing low-level runtime, compatibility, and generated parser imports in
the same module unless you are writing a test or adapter.

### Build fails with macro errors

Ensure both dependencies are present:
```toml
[dependencies]
adze = "0.8.0-dev"

[build-dependencies]
adze-tool = "0.8.0-dev"
```

### Grammar has conflicts

This is normal for ambiguous grammars. Options:
1. Add precedence annotations
2. Refactor to remove ambiguity
3. Use GLR parsing (automatic in 0.8+)

### How do I fix precedence errors?

Common precedence errors and solutions:

**Multiple precedence attributes:**
```rust
// ❌ Error
#[adze::prec(1)]
#[adze::prec_left(2)]
struct Bad { }

// ✅ Fix: Use only one
#[adze::prec_left(2)]
struct Good { }
```

**Invalid precedence value:**
```rust
// ❌ Error: String instead of integer
#[adze::prec("high")]

// ✅ Fix: Use integer literal
#[adze::prec(10)]
```

**Variable instead of literal:**
```rust
// ❌ Error: Cannot use variables
const HIGH: u32 = 10;
#[adze::prec(HIGH)]

// ✅ Fix: Use literal value directly
#[adze::prec(10)]
```

### What precedence values should I use?

**Guidelines:**
- Range: `0` to `4294967295` (u32)
- Zero is valid (lowest precedence)
- Use meaningful gaps (10, 20, 30) for future expansion
- Higher numbers bind tighter

**Common patterns:**
```rust
#[adze::prec_left(10)]  // Addition, subtraction
#[adze::prec_left(20)]  // Multiplication, division
#[adze::prec_right(30)] // Exponentiation
#[adze::prec(40)]       // Comparison operators
```

### WASM build fails

Start with the pure-Rust parser surface and keep optional adapters out of the
build until you need them:

```toml
adze = { version = "0.8.0-dev", features = ["pure-rust"] }
```

## Migration Questions

### How do I migrate from Tree-sitter?

See the comprehensive [Migration Guide](../getting-started/migration.md).

### What changed in v0.8?

Major changes include:
- GLR parsing support
- Enhanced error recovery
- Pure-Rust parser generation as the main path
- Early document, compatibility, and incremental lifecycle foundations

See the [Changelog](changelog.md) for details.

### Is 0.8 stable?

Use the support-tier ledger as the source of truth. Some surfaces are Stable,
while document, Tree-sitter-compatible, incremental, query, CLI, and WASM
surfaces may still be Stabilizing, Experimental, or Advisory.

## Contributing

### How can I contribute?

We welcome contributions! See our [Contributing Guide](../../CONTRIBUTING.md) for:
- Code style guidelines
- Testing requirements
- PR process

### Where do I report bugs?

Please report issues on our [GitHub repository](https://github.com/EffortlessMetrics/adze/issues).

### How do I add a new language grammar?

1. Create a new module with your grammar
2. Add tests for the grammar
3. Submit a PR with examples
4. See [Grammar Examples](../reference/grammar-examples.md) for patterns

# Frequently Asked Questions

> **Doc status:** reviewed against the 0.8.x release line.
> If something here disagrees with the repo, treat the repo as truth
> and log it in [`docs/status/FRICTION_LOG.md`](./docs/status/FRICTION_LOG.md).

Common questions about adze, answered concisely.

**Can't find your question?** Ask in [GitHub Discussions](https://github.com/EffortlessMetrics/adze/discussions) or [open an issue](https://github.com/EffortlessMetrics/adze/issues).

---

## General Questions

### What is adze?

adze is a parser generator for Rust that lets you define grammars using Rust types and attributes. It generates efficient parsers at compile time.

**Think**: "Tree-sitter meets Rust macros"

### Why use adze instead of tree-sitter?

| Feature | adze | tree-sitter |
|---------|-------------|-------------|
| **Language** | Pure Rust | C with bindings |
| **Grammar Definition** | Rust types + attributes | JavaScript DSL |
| **Type Safety** | Compile-time typed AST | Runtime navigation |
| **WASM Support** | ✅ First-class | ⚠️ Requires binding work |
| **GLR Parsing** | ✅ Built-in | ❌ LR only |
| **Incremental Parsing** | 🚧 Experimental | ✅ Mature |
| **Editor Integration** | 🚧 Coming | ✅ Extensive |

**Use adze if**: You want type-safe generated parsers in Rust, can work within
the documented support tiers, or need covered GLR conflict-handling shapes.

**Use tree-sitter if**: You need mature editor integration or battle-tested incremental parsing today.

### What is stable today?

The stable front door is generated pure-Rust parsing into typed Rust values for
the shapes covered in `docs/status/SUPPORT_TIERS.md`.

**Stable or release-readable slices**:
- Core generated parser path: use the default generated `grammar::parse()`.
- Type-safe AST extraction: stable for the documented generated-parser contract.
- Pure-Rust parser table serialization: stable for the support-tier row.

**Developing slices**:
- GLR conflict routing: stabilizing for covered conflict classes.
- Diagnostics, `AdzeDocument`, Tree-sitter-shaped output, query compatibility,
  CLI projections, WASM, and broader editor integrations: follow their support
  tiers and known gaps.

**Recommendation**: Use Adze for Rust-native generated parsers and tooling
experiments that match the support-tier proof. See [ROADMAP.md](./ROADMAP.md)
and `docs/status/SUPPORT_TIERS.md` for current status.

### What does GLR mean? Do I need it?

**GLR** = Generalized LR parsing. It handles ambiguous grammars by exploring multiple parse paths simultaneously.

**You need it if**:
- Your grammar has inherent ambiguity (e.g., C++ templates)
- You want the parser to find all possible interpretations
- You're parsing natural language or DSLs with flexibility

**You don't need it if**:
- Your grammar is unambiguous (most programming languages)
- Standard LR parsing works fine

adze supports both — LR is the default; enable GLR with `features = ["glr"]` in your `Cargo.toml`.

---

## Getting Started

### How do I install adze?

Add to your `Cargo.toml`:

The versioned dependency block is a release-surface shape for the coordinated
publish. Until `docs/status/PRODUCT_OBJECTIVE_AUDIT.md` records crates.io
metadata/install receipts for the co-release crates, use the repo quickstart or
local/path dependencies for proof from this checkout.

```toml
[dependencies]
adze = "0.9.0"

[build-dependencies]
adze-tool = "0.9.0"
```

See [QUICK_START.md](./QUICK_START.md) for a 5-minute tutorial.

### Where are the examples?

- **Quick Start**: [QUICK_START.md](./QUICK_START.md) - 5-minute calculator
- **Full Examples**: [example/src/](./example/src/) - arithmetic, JSON, more
- **Tutorial**: [docs/tutorials/getting-started.md](./docs/tutorials/getting-started.md)

### What's the smallest example?

```rust
#[adze::grammar("tiny")]
mod grammar {
    #[adze::language]
    pub struct Number {
        #[adze::leaf(pattern = r"\d+")]
        value: String,
    }
}

fn main() {
    let result = grammar::parse("42");
    println!("{:?}", result);
}
```

Don't forget `build.rs`:
```rust
fn main() {
    adze_tool::build_parsers(&std::path::PathBuf::from("src/main.rs"));
}
```

---

## Grammar Definition

### How do I handle whitespace?

Mark it as `#[adze::extra]`:

```rust
#[adze::extra]
struct Whitespace {
    #[adze::leaf(pattern = r"\s")]
    _ws: (),
}
```

This tells the parser to skip whitespace anywhere.

### How do I set operator precedence?

Use `#[adze::prec_left(N)]` where higher N = tighter binding:

```rust
#[adze::prec_left(1)]  // Lowest precedence
Add(Box<Expr>, #[leaf(text = "+")] (), Box<Expr>),

#[adze::prec_left(2)]  // Higher precedence
Mul(Box<Expr>, #[leaf(text = "*")] (), Box<Expr>),
```

This makes `2 + 3 * 4` parse as `2 + (3 * 4)`.

### How do I handle optional elements?

Use `Option<T>`:

```rust
pub struct FunctionCall {
    name: String,
    args: Option<Arguments>,  // Optional arguments
}
```

### How do I handle repetition (lists)?

Use `Vec<T>`:

```rust
pub struct ArgumentList {
    #[adze::repeat]
    #[adze::delimited(#[leaf(text = ",")] ())]
    args: Vec<Expression>,
}
```

### Can I transform matched text?

Yes! Use the `transform` parameter:

```rust
Number(
    #[adze::leaf(
        pattern = r"\d+",
        transform = |s| s.parse::<i32>().unwrap()
    )]
    i32
)
```

### How do I handle comments?

Mark them as `#[adze::extra]`:

```rust
#[adze::extra]
struct Comment {
    #[adze::leaf(pattern = r"//[^\n]*")]
    _comment: (),
}
```

---

## Build and Compilation

### Why does my build take so long?

The first build generates the parser, which can take time. Subsequent builds are fast (parser is cached).

**Tips**:
- Use `cargo build --release` only when needed
- The parser generation happens in `build.rs`, not at runtime

### Where are the generated files?

Set `ADZE_EMIT_ARTIFACTS=true` to see generated files:

```bash
ADZE_EMIT_ARTIFACTS=true cargo build
ls target/debug/build/*/out/
```

You'll see:
- `grammar.json` - Tree-sitter grammar
- `parser.c` - Generated parser (if using C backend)
- Parse tables (if using pure-Rust backend)

### How do I debug grammar issues?

1. **Enable artifact emission**:
   ```bash
   ADZE_EMIT_ARTIFACTS=true cargo build
   ```

2. **Check generated grammar**:
   ```bash
   cat target/debug/build/*/out/grammar.json
   ```

3. **Look for conflicts**: Build output shows shift/reduce conflicts

4. **Use tests**: Write tests for grammar rules individually

### Build fails with "conflict detected"

Your grammar has ambiguity. Solutions:

1. **Add precedence**: Use `#[prec_left]` or `#[prec_right]`
2. **Restructure grammar**: Make it unambiguous
3. **Use GLR**: adze handles conflicts automatically via GLR

### How do I see what the parser is doing?

Enable logging:

```bash
RUST_LOG=debug cargo run
```

Or use the parse tree visitor.

---

## Features and Capabilities

### Does adze support error recovery?

✅ Yes! The parser handles malformed input gracefully:

```rust
match grammar::parse("1 + ") {  // Incomplete expression
    Ok(tree) => println!("Partial parse: {:?}", tree),
    Err(e) => println!("Error: {}", e),
}
```

Error nodes are inserted for missing/unexpected tokens.

### Can I use adze with WASM?

✅ Yes! Pure-Rust backend has first-class WASM support:

```toml
[dependencies]
adze = { version = "0.8", features = ["pure-rust"] }
```

Then compile to WASM:
```bash
cargo build --target wasm32-unknown-unknown
```

### Does adze support external scanners?

✅ Yes! For context-sensitive lexing (like Python indentation):

```rust
#[derive(Default)]
struct IndentScanner;

impl adze::ExternalScanner for IndentScanner {
    fn scan(&mut self, lexer: &mut Lexer, valid: &[bool]) -> ScanResult {
        // Custom scanning logic
    }
}
```

See [docs/](./docs/) for examples (Python grammar uses this).

### What about incremental parsing?

Incremental parsing infrastructure exists but is experimental. See [ROADMAP.md](./ROADMAP.md).

### Can I query the parse tree?

Basic tree navigation works now. Full query system (predicates, captures) is in progress.

---

## Performance

### How fast is adze?

**Status**: Baseline being established.

Expected performance:
- Comparable to tree-sitter-c for most grammars
- Pure-Rust overhead is minimal
- GLR has overhead only when grammar is ambiguous

See [docs/archive/PERFORMANCE_BASELINE.md](./docs/archive/PERFORMANCE_BASELINE.md) for upcoming benchmarks.

### How can I make my parser faster?

**Grammar optimization**:
- Minimize backtracking (use clear precedence)
- Keep regexes simple
- Avoid deep nesting in repetitions

**Build optimization**:
- Use `--release` for production
- Profile with `cargo flamegraph`

**Future**:
- Incremental parsing for editors
- Table compression for memory

### Does adze support parallel parsing?

Not currently. Parsing is single-threaded. You can parse multiple files in parallel though:

```rust
use rayon::prelude::*;

files.par_iter().map(|file| {
    grammar::parse(file)
}).collect()
```

---

## Contributing

### How can I contribute?

See [CONTRIBUTING.md](./CONTRIBUTING.md) and browse [GitHub Issues](https://github.com/EffortlessMetrics/adze/issues).

**Quick picks**:
- Re-enable ignored tests (1-4 hours each)
- Add documentation examples
- Write grammar cookbooks
- Performance benchmarking

### I found a bug, what should I do?

1. **Search existing issues**: Check if it's already reported
2. **Create minimal reproduction**: Simplest grammar that shows the bug
3. **Open issue**: [GitHub Issues](https://github.com/EffortlessMetrics/adze/issues)
4. **Include**: adze version, Rust version, minimal example

### Can I add a new grammar to the examples?

Yes! PRs welcome:

1. Add grammar to `example/src/your_grammar.rs`
2. Add tests with expected parse trees
3. Update `example/tests/integration.rs`
4. Submit PR

See [CONTRIBUTING.md](./CONTRIBUTING.md) for guidelines.

---

## Comparison to Alternatives

### adze vs nom

| Feature | adze | nom |
|---------|-------------|-----|
| **Approach** | Declarative grammar | Combinator library |
| **Generated Code** | Yes (build time) | No (runtime combinators) |
| **Error Recovery** | Automatic | Manual |
| **Performance** | Optimized tables | Depends on combinators |
| **Learning Curve** | Moderate (grammars) | Moderate (combinators) |

**Use nom if**: You need fine control, hand-written parsers, or no build step.
**Use adze if**: You have a grammar, want error recovery, or need GLR.

### adze vs pest

| Feature | adze | pest |
|---------|-------------|-----|
| **Grammar Format** | Rust attributes | PEG syntax |
| **Type Safety** | Compile-time AST | Runtime rule matching |
| **Ambiguity** | GLR handles it | PEG doesn't support |
| **Precedence** | Built-in | Manual |

**Use pest if**: You like PEG grammars or want external grammar files.
**Use adze if**: You want type-safe ASTs or need ambiguity support.

### adze vs lalrpop

| Feature | adze | lalrpop |
|---------|-------------|---------|
| **Grammar Format** | Rust attributes | LALR DSL |
| **GLR Support** | Yes | No |
| **Maturity** | New (beta) | Mature |
| **WASM** | First-class | Requires work |

**Use lalrpop if**: You need battle-tested LALR parsing.
**Use adze if**: You need GLR, WASM, or prefer Rust-native grammars.

---

## Roadmap and Future

### What's on the roadmap?

See [ROADMAP.md](./ROADMAP.md) for current status.

### What about v1.0?

See [ROADMAP.md](./ROADMAP.md) for current status.

**v1.0 goals**:
- API stability guarantees
- Full editor plugin support
- Production-grade incremental parsing
- Comprehensive language support (50+ grammars)

### Will adze replace tree-sitter?

No. They serve different use cases:

- **tree-sitter**: Mature, editor-focused, C-based, extensive ecosystem
- **adze**: Pure-Rust, type-safe, GLR, WASM-first

Think of adze as "tree-sitter for Rust-native projects" not a replacement.

### Can I use both tree-sitter and adze?

Yes! adze can:
- Import tree-sitter grammars (via ts-bridge tool)
- Generate tree-sitter compatible parsers
- Interop with tree-sitter tooling

See [tools/ts-bridge/](./tools/ts-bridge/) for the bridge tool.

---

## Still Have Questions?

- **Check**: [GitHub Issues](https://github.com/EffortlessMetrics/adze/issues)
- **Ask**: [GitHub Discussions](https://github.com/EffortlessMetrics/adze/discussions)
- **Report Bugs**: [GitHub Issues](https://github.com/EffortlessMetrics/adze/issues)
- **Tutorial**: [docs/tutorials/getting-started.md](./docs/tutorials/getting-started.md)
- **Examples**: [example/src/](./example/src/)

**Can't find an answer?** Open a discussion - we'll add it to this FAQ!

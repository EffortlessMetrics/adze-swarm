# Quick Start - Get Parsing in 5 Minutes

> **Doc status:** being refreshed to match Adze 0.9.0.
> If something here disagrees with the repo, treat the repo as truth
> and log it in [`docs/status/FRICTION_LOG.md`](./docs/status/FRICTION_LOG.md).

**Goal**: Get a working parser running in 5 minutes or less.

No deep knowledge required. Copy, paste, run, parse! 🚀

---

## Step 1: Install (30 seconds)

```bash
# Create new project
cargo new my-parser
cd my-parser

# Add dependencies
cat >> Cargo.toml <<'EOF'

[dependencies]
adze = { version = "0.9.0", default-features = false }

[build-dependencies]
adze-tool = "0.9.0"

[features]
default = ["pure-rust"]
pure-rust = ["adze/pure-rust"]
EOF
```

> **Release-surface boundary:** the versioned dependency block above is the
> intended crates.io shape after the coordinated publish. Current repo proof
> uses local/path dependencies from this checkout until the publish/install
> receipt is recorded in `docs/status/PRODUCT_OBJECTIVE_AUDIT.md`.

---

## Step 2: Create Build Script (30 seconds)

Create `build.rs`:

```rust
use std::path::PathBuf;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(adze_unsafe_attrs)");
    adze_tool::build_parsers(&PathBuf::from("src/main.rs"));
}
```

---

## Step 3: Define Your Grammar (2 minutes)

Replace `src/main.rs` with this:

```rust
// Define a simple calculator grammar
#[adze::grammar("calc")]
mod grammar {
    #[adze::language]
    #[derive(Debug)]
    pub enum Expr {
        Number(
            #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
            i32
        ),

        #[adze::prec_left(1)]
        Add(
            Box<Expr>,
            #[adze::leaf(text = "+")] (),
            Box<Expr>,
        ),

        #[adze::prec_left(2)]
        Mul(
            Box<Expr>,
            #[adze::leaf(text = "*")] (),
            Box<Expr>,
        ),
    }

    #[adze::extra]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s")]
        _whitespace: (),
    }
}

fn main() {
    // Parse an expression
    let result = grammar::parse("2 + 3 * 4");

    match result {
        Ok(expr) => println!("✅ Parsed: {:#?}", expr),
        Err(e) => println!("❌ Error: {}", e),
    }
}
```

---

## Step 4: Build and Run (1 minute)

```bash
cargo build
cargo run
```

**Expected output**:
```
✅ Parsed: Add(
    Number(2),
    (),
    Mul(
        Number(3),
        (),
        Number(4),
    ),
)
```

---

## 🎉 Success!

You just:
- ✅ Defined a grammar with operator precedence
- ✅ Generated a parser at compile time
- ✅ Parsed `2 + 3 * 4` correctly as `2 + (3 * 4)`
- ✅ Got a typed Rust AST

**Total time**: ~5 minutes

---

## What Just Happened?

1. **`#[adze::grammar("calc")]`** - Declares a grammar named "calc"
2. **`#[adze::language]`** - Marks the root type for parsing
3. **`#[adze::leaf]`** - Defines how to match text (`pattern` or `text`)
4. **`#[adze::prec_left(N)]`** - Sets operator precedence (higher = tighter)
5. **`build.rs`** - Generates the parser at build time
6. **`grammar::parse()`** - Parse text into your Rust type

---

## Next Steps

### Try Different Inputs

```rust
// In main()
for input in &["1 + 2", "5 * 6", "1 + 2 * 3", "10"] {
    match grammar::parse(input) {
        Ok(expr) => println!("✅ '{}' → {:?}", input, expr),
        Err(e) => println!("❌ '{}' → {}", input, e),
    }
}
```

### Add More Operators

```rust
#[adze::prec_left(1)]
Sub(Box<Expr>, #[adze::leaf(text = "-")] (), Box<Expr>),

#[adze::prec_left(2)]
Div(Box<Expr>, #[adze::leaf(text = "/")] (), Box<Expr>),
```

### Add Parentheses

```rust
Paren(
    #[adze::leaf(text = "(")] (),
    Box<Expr>,
    #[adze::leaf(text = ")")] (),
),
```

---

## Common Issues

### Build error: "parser not found"
**Solution**: Make sure `build.rs` exists and contains the `build_parsers` call.

### Parse error: "unexpected token"
**Solution**: Check your `pattern` regex or `text` matches what you're parsing.

### Wrong precedence
**Solution**: Higher `prec_left` numbers bind tighter. Multiplication (2) should be higher than addition (1).

---

## Learn More

- **Full Tutorial**: [docs/tutorials/getting-started.md](./docs/tutorials/getting-started.md)
- **More Examples**: [example/src/](./example/src/)
- **API Docs**: [docs/reference/api.md](./docs/reference/api.md)
- **Architecture**: [docs/explanations/architecture.md](./docs/explanations/architecture.md)

---

## Get Help

- **Questions?** Check [FAQ.md](./FAQ.md)
- **Stuck?** [GitHub Issues](https://github.com/EffortlessMetrics/adze/issues)
- **Want to contribute?** See [CONTRIBUTING.md](./CONTRIBUTING.md)
- **Report bugs**: [GitHub Issues](https://github.com/EffortlessMetrics/adze/issues)

---

**Ready to build something bigger?** Check out the [full tutorial](./docs/tutorials/getting-started.md)!

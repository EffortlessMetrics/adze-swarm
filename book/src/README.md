# Adze Documentation

[![Crates.io](https://img.shields.io/crates/v/adze)](https://crates.io/crates/adze)

Welcome to the official documentation for **Adze** - a Rust framework that makes it easy to create efficient parsers by leveraging the [Tree-sitter](https://tree-sitter.github.io/tree-sitter/) parser generator.

With Adze, you can define your entire grammar with annotations on idiomatic Rust code, and let macros generate the parser and type-safe bindings for you!

## Key Features

### 0.9.0 Highlights

- **Generated Parser Front Door**: Define Rust grammar types and call generated parsers.
- **Pure-Rust Option**: Generate static parsers at compile time without C dependencies.
- **Structured Diagnostics**: Use supported parser errors and document diagnostics for bad input.
- **GLR Ambiguity Handling**: Use documented GLR conflict-routing and ambiguity-summary surfaces.
- **Document Projections**: Inspect `AdzeDocument`, typed views, Tree-sitter-shaped selected trees, and JSON according to support tiers.
- **Proof-Tracked Surfaces**: Support tiers, proof commands, known gaps, and release receipts are tracked in repo docs.

## Quick Example

Here's a simple arithmetic expression parser:

```rust
#[adze::grammar("arithmetic")]
mod grammar {
    #[adze::language]
    pub enum Expr {
        Number(
            #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
            u32,
        ),
        #[adze::prec_left(1)]
        Add(
            Box<Expr>,
            #[adze::leaf(text = "+")] (),
            Box<Expr>,
        )
    }
}

// Usage
let result = grammar::parse("1+2+3");
```

## When to Use Adze

Adze is ideal for:

- **Language Server Protocol (LSP) implementations** - Fast incremental parsing for IDE support
- **Code analysis tools** - Syntax highlighting, linting, formatting
- **Transpilers and interpreters** - Type-safe AST generation
- **Documentation generators** - Parsing code for documentation extraction
- **Any application requiring robust parsing** - With error recovery and ambiguity handling

## How This Book is Organized

- **Getting Started** - Installation, quick start guide, and migration from Tree-sitter
- **User Guide** - Core concepts like grammar definition, parser generation, and queries
- **Advanced Topics** - GLR parsing, optimization, external scanners, and more
- **Reference** - API documentation, examples, and known limitations
- **Development** - Contributing guidelines, architecture overview, and testing

## Getting Help

- **GitHub Issues**: Report bugs or request features at [adze/issues](https://github.com/EffortlessMetrics/adze/issues)
- **Discussions**: Ask questions and share experiences in [GitHub Discussions](https://github.com/EffortlessMetrics/adze/discussions)
- **Examples**: Check out the [example grammars](reference/grammar-examples.md) for inspiration

## License

Adze is licensed under the MIT license. See the [LICENSE](https://github.com/EffortlessMetrics/adze/blob/main/LICENSE) file for details.

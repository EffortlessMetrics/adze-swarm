# Adze Documentation

[![Crates.io](https://img.shields.io/crates/v/adze)](https://crates.io/crates/adze)

When syntax changes, the grammar and the Rust types your application reads need
to stay in sync. Adze puts the grammar rules on those types. It generates the
parser at build time, and `grammar::parse()` returns values of the types you
defined.

## Build Your First Parser

Follow the [Quick Start](getting-started/quickstart.md) to generate and test a
small arithmetic parser in a fresh Rust crate. It covers the grammar
annotations, the build script, and what happens when input is invalid.

## Choose the Output You Need

- **Typed values for an interpreter, transpiler, or analyzer:** use
  `grammar::parse()`.
- **A selected syntax tree and diagnostics for tooling:** use
  `grammar::parse_document()`. See [Error Recovery](guide/error-recovery.md)
  for how to handle incomplete input.
- **A Tree-sitter-shaped view for integrations:** use the
  [selected-tree adapter](https://github.com/EffortlessMetrics/adze-swarm/blob/main/docs/reference/tree-sitter-compatibility.md)
  for the documented traversal subset.

[Incremental parsing](guide/incremental-parsing.md) is Experimental. Editor
integrations should be prepared to reparse the full source after an edit. See
[Support Tiers](https://github.com/EffortlessMetrics/adze-swarm/blob/main/docs/status/SUPPORT_TIERS.md)
for the status and limits of each surface.

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

Adze is available under either the [MIT license](https://github.com/EffortlessMetrics/adze/blob/main/LICENSE-MIT)
or the [Apache License, Version 2.0](https://github.com/EffortlessMetrics/adze/blob/main/LICENSE-APACHE).

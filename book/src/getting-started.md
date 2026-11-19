# Getting started

Adze generates a parser from annotated Rust types. Start with a checkout and
build a small arithmetic parser that returns an `Expr` you can use in Rust.

- [Quick Start](getting-started/quickstart.md) walks through a fresh crate,
  its grammar, and tests for valid and invalid input.
- [Set up Adze from a checkout](getting-started/installation.md) shows how to
  add the runtime and build tool to an existing crate.
- [Grammar definition](guide/grammar-definition.md) explains how to write
  tokens, alternatives, and precedence rules.

Use `grammar::parse(source)` for typed values. If your tool needs syntax nodes,
ranges, or diagnostics, use `grammar::parse_document(source)`. The document
API is Stabilizing. See [Support tiers](https://github.com/EffortlessMetrics/adze-swarm/blob/main/docs/status/SUPPORT_TIERS.md)
for the supported surfaces and their limits.

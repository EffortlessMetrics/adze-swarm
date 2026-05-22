# adze-runtime

Experimental GLR runtime proving ground for Adze parsers.

> **Support status:** `runtime2/` is intentionally excluded from the stable
> public-primary runtime contract. The supported user path is generated parsers
> using the main `adze` runtime. Runtime2 is useful for experiments around
> Tree-sitter-shaped APIs, metadata, incremental ideas, and runtime
> architecture, but it should not be presented as stable or drop-in compatible
> until the support tiers promote a narrower slice with proof.

## Overview

This crate explores a runtime that mimics parts of Tree-sitter's API while
using GLR (Generalized LR) parsing internally to handle ambiguous grammars. It
is **not** a drop-in replacement for Tree-sitter's runtime API.

## Features

- **Tree-sitter-shaped API experiments**: `Parser`, `Tree`, `Node`, and
  `Language` facade types.
- **GLR runtime experiments**: ambiguous-grammar parsing infrastructure under
  development.
- **Incremental parsing experiments**: feature-gated lifecycle work; no stable
  reuse or performance claim.
- **External scanner experiments**: custom lexing hooks outside the stable
  product contract.
- **Arena allocation experiments**: optional optimization infrastructure, not a
  published performance guarantee.

## Quick Start

```rust
use adze_runtime::{Parser, Language};

// Create a parser
let mut parser = Parser::new();

// Set the language (would come from an experimental generated grammar crate)
let language = your_grammar::language();
parser.set_language(language)?;

// Parse some text
let tree = parser.parse_utf8("def hello(): pass", None)?;

// Walk the syntax tree
let root = tree.root_node();
println!("Root kind: {}", root.kind());

for i in 0..root.child_count() {
    if let Some(child) = root.child(i) {
        println!("Child {}: {}", i, child.kind());
    }
}
```

## Architecture

The runtime is structured as follows:

- **Parser**: Main parsing interface, manages language and parsing state
- **Language**: Contains parse tables and grammar metadata
- **Tree**: Represents a parsed syntax tree facade for runtime experiments
- **Node**: A node in the syntax tree with Tree-sitter-shaped traversal APIs
- **ExternalScanner**: Trait for custom lexing logic (e.g., indentation)

## Implementation Status

- [x] Basic API structure
- [x] Parser and Language types
- [x] Tree and Node facades
- [x] External scanner trait
- [ ] GLR parsing engine integration
- [ ] SPPF to Tree conversion
- [ ] Incremental parsing behavior
- [ ] Query system
- [ ] Performance optimizations

## Feature Flags

- `arenas`: Enable arena allocators for optimization experiments
- `pure-rust`: Enable pure-Rust GLR runtime (feature alias: `pure-rust-glr`, legacy)
- `incremental_glr`: Enable incremental parsing support (`incremental` alias preserved for compatibility)
- `external_scanners`: Enable external scanner support (`external-scanners` alias preserved for compatibility)
- `query`: Enable query system (future; `queries` alias preserved for compatibility)

## Testing

```bash
# Run basic tests
cargo test

# Run with all features
cargo test --all-features

# Run examples
cargo run --example simple_parse
```

## Integration with Adze

This runtime is not the normal user integration path. For stable product use,
prefer generated parsers that depend on the main `adze` runtime and return
typed AST values through `grammar::parse(...)`.

A runtime2 experiment usually looks like:

1. Define your grammar using adze macros
2. Generate parser tables at build time
3. Link with this runtime
4. Use the Tree-sitter-shaped facade API to parse text

## License

MIT OR Apache-2.0

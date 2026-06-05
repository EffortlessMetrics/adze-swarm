# Adze Playground

**Status: Planned** -- not yet implemented.

## Vision

An interactive web-based environment for grammar development and testing:

- Live parsing with real-time parse tree updates
- Grammar editor with syntax highlighting
- Tree visualization and node inspection
- Performance metrics

## Current Alternative

For now, develop and test grammars locally:

The `cargo add` commands below are the intended release-surface shape after the
coordinated publish. Current repo proof uses local/path dependencies from this
checkout until crates.io receipts exist for the co-release crates.

```bash
cargo add adze
cargo add --build adze-tool
cargo test
```

See [QUICK_START.md](../../QUICK_START.md) for a full walkthrough and [CONTRIBUTING.md](../../CONTRIBUTING.md) for development setup.

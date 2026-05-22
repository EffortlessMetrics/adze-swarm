# Dynamic Loading Design Sketch

This guide records the current boundary around `adze parse --dynamic`.

> **Current status:** dynamic loading is not a supported parse-output path in
> the current CLI. `adze parse --dynamic ...` can be compiled behind the
> `dynamic` feature and can attempt to load a shared-library symbol, but it
> still exits with `dynamic parse mode is currently unimplemented` after the
> load boundary. Treat this page as a design sketch, not a supported workflow.

## What Works Today

The checked-out CLI can be built with the experimental dynamic feature:

```bash
cargo build -p adze-cli --features dynamic
```

Without that feature, `adze parse --dynamic` fails before attempting to load a
library and tells the user to rebuild with `--features dynamic`.

With that feature, the CLI validates the input file, checks that the dynamic
grammar path exists, opens the library, looks up the requested symbol, and then
stops at the current implementation boundary.

The current boundary is intentional:

```text
dynamic library load: experimental
dynamic parse output: not implemented
stable CLI/WASM schema: not claimed
crates.io install receipt: not claimed
```

## Current Boundary Checks

Expected failure without the feature:

```bash
cargo run -p adze-cli -- parse grammar.so input.txt --dynamic
```

Expected result:

```text
Dynamic parse mode is experimental and requires building adze-cli with --features dynamic.
```

Expected failure with a missing grammar library:

```bash
cargo run -p adze-cli --features dynamic -- parse missing.so input.txt --dynamic
```

Expected result:

```text
dynamic grammar not found: missing.so
```

Expected boundary after a successful load:

```text
dynamic parse mode is currently unimplemented
```

The exact loader error for a non-library file is platform-specific and should
not be treated as a portable product receipt.

## Future Target

A future implementation may support existing Tree-sitter grammar libraries or
Adze-generated dynamic parser libraries. That work needs its own behavior
contract before it can be promoted.

Future dynamic output must answer these questions explicitly:

- Which library ABI is accepted?
- Which exported symbol names are supported?
- Does the output come from `AdzeDocument`, Tree-sitter selected-tree data, or
  another compatibility layer?
- Which output formats are available?
- Which schemas are versioned and tested?
- How are errors, missing nodes, ranges, and diagnostics represented?
- Which proof commands move the surface out of Experimental?

## Non-Claims

This guide does not claim:

- `cargo install adze-cli` works from crates.io before an explicit
  release-surface install receipt exists;
- dynamic parse output works today;
- Tree-sitter dynamic parsing is supported;
- JSON output from dynamic parse has a stable schema;
- dynamic loading is part of the required `just ci-supported` lane.

Use generated Rust parsers, `grammar::parse(...)`, and
`grammar::parse_document(...)` for the current product parsing path.

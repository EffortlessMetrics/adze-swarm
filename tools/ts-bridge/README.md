# ts-bridge: Tree-sitter to GLR Runtime Bridge

This advisory tool experiments with extracting parse tables from compiled
Tree-sitter grammars and converting them into data that Adze runtime experiments
can inspect.

> **Support status:** `tools/ts-bridge/` is outside the stable Adze parser
> product contract. The current proof is smoke-level and advisory. It should not
> be described as a production bridge, full imported-grammar compatibility, or a
> stable Tree-sitter interop surface until support tiers promote a narrower
> slice with proof.

## Features

- **Table extraction experiments**: parse-table data can be extracted from a
  compiled grammar for inspection and follow-on runtime work.
- **ABI guards**: pinned Tree-sitter v15 checks and header hash verification
  catch obvious drift.
- **Advisory parity work**: optional tests and fixtures provide signal, not a
  full Tree-sitter compatibility guarantee.

## Building

### Advisory Build
```bash
# Build with the vendored shim/runtime used by the advisory smoke path.
cargo build --manifest-path tools/ts-bridge/Cargo.toml

# Run the ABI verification
cargo run --manifest-path tools/ts-bridge/Cargo.toml --bin tsb-abi-check

# Optional: link a system libtree-sitter instead of the vendored runtime.
cargo build --manifest-path tools/ts-bridge/Cargo.toml --no-default-features --features link-system-ts
```


## Usage

### Extract Parse Tables (dynamic loading)
```bash
# Extract from a compiled Tree-sitter grammar library.
cargo run --manifest-path tools/ts-bridge/Cargo.toml -- path/to/libtree-sitter-json.so output.json tree_sitter_json

# The output will be a JSON file containing:
# - Symbol names and counts
# - Parse rules with deterministic IDs for this extraction
# - Action table (for terminals) 
# - Goto table (for non-terminals)
# - Start symbol detection
```

### Verify ABI Stability
```bash
# Check header hashes and runtime ABI version
./tools/ts-bridge/scripts/abi-hash.sh
```

## Testing

### Basic Tests (always run)
```bash
cargo test --manifest-path tools/ts-bridge/Cargo.toml --test basic
```

### Parity Tests (requires tree-sitter-json)
```bash
# Enable with-grammars feature for optional grammar-linked tests.
cargo test --manifest-path tools/ts-bridge/Cargo.toml --features with-grammars -- --nocapture
```

## Architecture

The bridge works by:
1. Loading a compiled Tree-sitter grammar (`.so`/`.dll`/`.dylib`)
2. Using FFI shim to call Tree-sitter's table access functions
3. Extracting parse table data with checked type conversions
4. Serializing to JSON for consumption by GLR runtime or static generation

### Key Components

- `ffi/shim.c`: C shim that interfaces with Tree-sitter API
- `src/extract.rs`: Core extraction logic with width checks and buffer safety
- `src/schema.rs`: Data structures for parse table representation
- Optional system-linked builds use `link-system-ts`; the default smoke path
  uses vendored Tree-sitter runtime sources.

### Safety Features

- **Width checks**: All values verified to fit in u16 with debug assertions
- **Dynamic buffer allocation**: Action buffers expand as needed (no truncation)
- **ABI guards**: Runtime version checks prevent silent breakage

## ABI Stability

We pin to Tree-sitter language version 15 and use multiple layers of protection:
- System library integration with libtree-sitter-dev
- Runtime ABI version checks via `tsb_language_version()`
- SHA-256 hash verification of critical headers

## Buffer Management

- Default: 32 actions per table cell (`MAX_ACTIONS_PER_CELL`)
- Automatically expands for larger cells (no silent truncation)
- All buffers properly sized based on actual grammar requirements

## Important Notes

- **Incremental parsing**: Not supported in v1 (requires specialized GLR algorithms)
- **External scanners**: Headers defined but implementation deferred to PR2
- **Field mappings**: Production IDs map to fields via `field_map_slices` (PR2)

## Advisory Checklist

✅ Run `tsb-abi-check` to verify ABI compatibility
✅ Execute `abi-hash.sh` to verify header integrity
✅ Run optional parity tests with actual grammars when available
✅ Verify extracted JSON contains plausible data for the targeted grammar

Passing this checklist is useful interop evidence, not a Stable product claim.

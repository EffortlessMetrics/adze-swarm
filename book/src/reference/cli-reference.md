# CLI Reference

Complete reference for the adze command-line interface.

> **Current support note:** `adze init`, `adze check`, `adze stats`, `adze build`,
> `adze test`, and `adze doc` are implemented. Static `adze parse` emits
> document-backed tree, S-expression, DOT, and JSON representations from a Rust
> grammar file. The CLI remains **Stabilizing**, and public JSON projections
> remain **Experimental**; see [Support Tiers](../../../docs/status/SUPPORT_TIERS.md).
> Dynamic (`--dynamic`) parsing is unavailable and rejects the request before
> loading a library.

## Installation

The crates.io install command is a release-surface target until the release
checklist records a crates.io install receipt for `adze-cli`. For the current
proved path, build the CLI from the checkout under test.

Intended published install command:

```bash
cargo install adze-cli
```

From this checkout:

```bash
cargo build --release -p adze-cli
```

## Global Options

### `--verbose, -v`

Enable verbose output showing detailed processing information.

```bash
adze --verbose parse grammar.rs input.txt
```

### `--help, -h`

Show help information for commands.

```bash
adze --help
adze parse --help
```

## Commands

### `init`

Create a new adze grammar project.

**Usage:**
```bash
adze init <NAME> [OPTIONS]
```

**Arguments:**
- `<NAME>` - Name of the grammar project

**Options:**
- `--output, -o <DIR>` - Output directory (default: current directory)

**Example:**
```bash
# Create a new JSON grammar
adze init json-parser

# Create in specific directory
adze init json-parser --output ~/projects/
```

**Generated Structure:**
```
json-parser/
├── Cargo.toml
├── build.rs
├── src/
│   ├── lib.rs
│   └── grammar.rs
├── tests/
│   └── basic.rs
├── examples/
└── README.md
```

### `build`

Build grammar parsers from adze grammar definitions.

**Usage:**
```bash
adze build [PATH] [OPTIONS]
```

**Arguments:**
- `[PATH]` - Path to grammar file or directory (default: current directory)

**Options:**
- `--watch, -w` - Watch for changes and rebuild automatically

**Examples:**
```bash
# Build grammar in current directory
adze build

# Build specific grammar file
adze build src/grammar.rs

# Watch for changes
adze build --watch
```

**Watch Mode:**
Automatically rebuilds when `.rs` files change. Useful during grammar development.

### `parse`

Parse input files using adze grammars.

**Status:** `adze parse <grammar.rs> <input>` implements the following static
representations through the generated grammar's document path:

| Format | Output |
| --- | --- |
| `tree` (default) | Indented selected tree with node kinds and byte ranges. |
| `sexp` | Selected-tree S-expression. |
| `dot` | Selected-tree Graphviz graph. |
| `json`, `document-json` | Full document JSON (`adze.document.v1`). |
| `tree-json` | Tree projection (`adze.tree.v1`). |
| `diagnostics-json` | Diagnostic projection (`adze.diagnostics.v1`). |
| `ambiguity-json` | Ambiguity projection (`adze.ambiguity.v1`). |

Static parsing compiles a temporary runner. Build progress goes to stderr;
stdout contains the selected representation. The CLI is Stabilizing and these
public JSON projections are Experimental. Their implementation does not change
those [support tiers](../../../docs/status/SUPPORT_TIERS.md).

Dynamic mode is unavailable: `--dynamic` returns an error before loading any
library.

**Usage:**
```bash
adze parse <GRAMMAR> <INPUT> [OPTIONS]
```

**Arguments:**
- `<GRAMMAR>` - Grammar file path, or library path when using `--dynamic`
- `<INPUT>` - Input file to parse

**Options:**
- `--format, -f <FORMAT>` - Output format (alias `--output`): `tree` (default), `json`, `sexp`, `dot`, `document-json`, `tree-json`, `diagnostics-json`, `ambiguity-json`
- `--dynamic` - Request dynamic parsing (currently rejected before library loading)
- `--symbol <SYMBOL>` - Reserved dynamic symbol name (default: "language"); dynamic parsing remains unavailable

**Static Parsing Examples:**

```bash
# Print the selected tree
adze parse grammar.rs input.txt

# Choose another static representation
adze parse grammar.rs input.txt --format sexp
adze parse grammar.rs input.txt --output dot
adze parse grammar.rs input.txt --output document-json
```

On success, the command writes the requested representation to stdout and exits
zero. Use `adze check` to validate a grammar or `adze build` to generate parsers
for a Rust application.

**Dynamic Mode Boundary:**

```bash
adze parse --dynamic libtree-sitter-json.so input.json
```

This command returns a nonzero exit with an unavailable-mode error. It does not
load the library or emit a parse result.

### `test`

Run tests for adze grammars.

**Usage:**
```bash
adze test [PATH] [OPTIONS]
```

**Arguments:**
- `[PATH]` - Path to grammar directory (default: current directory)

**Options:**
- `--update, -u` - Update test snapshots

**Examples:**
```bash
# Run tests
adze test

# Update snapshots
adze test --update
```

Uses `cargo test` internally with `insta` snapshot testing.

### `doc`

Generate documentation from grammar files.

**Usage:**
```bash
adze doc <GRAMMAR> [OPTIONS]
```

**Arguments:**
- `<GRAMMAR>` - Path to grammar file

**Options:**
- `--output, -o <FILE>` - Output file (default: stdout)

**Example:**
```bash
# Output to console
adze doc src/grammar.rs

# Save to file
adze doc src/grammar.rs --output docs/grammar.md
```

Extracts documentation from `///` comments in grammar files.

### `check`

Validate grammar syntax via parser construction.

**Usage:**
```bash
adze check <GRAMMAR>
```

**Arguments:**
- `<GRAMMAR>` - Path to grammar file

**Example:**
```bash
adze check src/grammar.rs
```

**Output:**
```
✅ Grammar syntax is valid (1 grammar definition(s))!
```

Fast validation for CI/CD pipelines and editors.

### `stats`

Show parser-construction statistics from the pure-Rust pipeline.

**Usage:**
```bash
adze stats <GRAMMAR>
```

**Arguments:**
- `<GRAMMAR>` - Path to grammar file

**Example:**
```bash
adze stats src/grammar.rs
```

**Output:**
```
📊 Grammar statistics:
  Grammar: simple
    States: 124
    Symbols: 67
    Conflicts: 3
```

Useful for tracking grammar complexity and growth.

## Dynamic Loading

### Supported Formats

Dynamic loading support is developing. The CLI currently accepts standard
Tree-sitter library paths, but parse output is not implemented.

**Linux:**
- `.so` files (shared objects)
- Example: `libtree-sitter-json.so`

**macOS:**
- `.dylib` files (dynamic libraries)  
- Example: `libtree-sitter-json.dylib`

**Windows:**
- `.dll` files (dynamic link libraries)
- Example: `tree-sitter-json.dll`

### Common Library Locations

**System Package Locations:**
```bash
# Ubuntu/Debian
/usr/lib/x86_64-linux-gnu/libtree-sitter-*.so

# Fedora/CentOS
/usr/lib64/libtree-sitter-*.so

# macOS (Homebrew)
/opt/homebrew/lib/libtree-sitter-*.dylib

# macOS (MacPorts)
/opt/local/lib/libtree-sitter-*.dylib
```

**Language-Specific Locations:**
```bash
# Node.js installations
node_modules/tree-sitter-*/bindings/node/

# Python installations  
~/.local/lib/python*/site-packages/tree_sitter_*/

# Cargo target directory
target/release/deps/libtree_sitter_*.so
```

### Symbol Names

Common symbol naming patterns:

```bash
# Standard pattern: tree_sitter_{language}
tree_sitter_json
tree_sitter_python  
tree_sitter_javascript
tree_sitter_rust
tree_sitter_cpp

# Alternative patterns
language                    # Generic name
tree_sitter_{lang}_language # Extended format
get_language               # Function-style name
```

Use `nm -D library.so | grep tree_sitter` to find available symbols.

## FFI Safety and Security

### Input Validation

- **Library Path**: Existence and readability checks
- **Input Files**: Size limits (100MB), UTF-8 validation
- **Symbol Names**: Alphanumeric + underscore validation
- **Memory Safety**: Pointer null checks, alignment validation

### Error Handling

- **Library Loading**: Detailed error messages for missing dependencies
- **Symbol Resolution**: Clear indication of available symbols
- **Parsing Errors**: Graceful failure with partial results when possible
- **Stack Protection**: Depth limits prevent stack overflow

### Security Considerations

- **No Code Execution**: Only data extraction from compiled libraries
- **Sandboxed Operation**: No file system access beyond specified paths
- **Memory Bounds**: All memory access is bounds-checked
- **Timeout Protection**: Automatic termination for runaway parsing

## Environment Variables

### `RUST_LOG`

Control logging output:

```bash
# Show all debug information
RUST_LOG=debug adze parse --dynamic lib.so input.txt

# Show only warnings and errors
RUST_LOG=warn adze build
```

### `ADZE_LOG_PERFORMANCE`

Enable performance monitoring for GLR parsing:

```bash
ADZE_LOG_PERFORMANCE=true cargo test
```

Output example:
```
🚀 Forest->Tree conversion: 247 nodes, depth 12, took 0.8ms
```

### `INSTA_UPDATE`

Control snapshot testing behavior:

```bash
# Always update snapshots
INSTA_UPDATE=always adze test

# Never update snapshots (CI mode)
INSTA_UPDATE=no adze test
```

## Exit Codes

- `0` - Success
- `1` - General error (parsing failure, invalid grammar, etc.)
- `2` - Missing feature (e.g., dynamic loading not compiled)
- `3` - Invalid arguments or usage
- `4` - File system error (permission denied, file not found)

## Configuration Files

### `rust-toolchain.toml`

The repository tests the following Rust toolchain pin. The workspace and current
`adze`, `adze-tool`, and `adze-cli` manifests declare `rust-version = "1.95.0"`.
Use the tested pin for checkout-based development:

```toml
[toolchain]
channel = "1.95.0"
components = ["rustfmt", "clippy"]
```

The package edition belongs in `Cargo.toml` (`edition = "2024"`), rather than
inside the `[toolchain]` table.

### `.gitignore` Additions

Generated parser files to ignore:

```gitignore
# Adze generated files
target/
parser.c
tree_sitter/
src/tree_sitter/
grammar.json
node-types.json
```

## Troubleshooting

### Common Issues

**"symbol not found"**
```bash
# Check available symbols
nm -D library.so | grep tree_sitter

# Try common symbol names once dynamic parse output is implemented
adze parse --dynamic lib.so input.txt --symbol language
adze parse --dynamic lib.so input.txt --symbol get_language
```

**"library not found"**
```bash
# Check library path
ls -la /path/to/library.so

# Check dependencies
ldd /path/to/library.so

# Install system dependencies
sudo apt install libtree-sitter-dev  # Ubuntu
brew install tree-sitter              # macOS
```

**"Dynamic loading not enabled"**
```bash
# Rebuild with dynamic feature
cargo build --release -p adze-cli --features dynamic

# Intended post-publish install shape; not current repo proof until the
# crates.io install receipt exists.
cargo install adze-cli --features dynamic
```

**"Parse tree too deep"**
- Indicates possible grammar recursion
- Check for left-recursive rules
- Use GLR mode for ambiguous grammars

**"Input too large"**
- Files over 100MB require streaming
- Consider splitting large inputs
- Use incremental parsing for large files

### Debug Mode

Enable verbose debugging:

```bash
adze --verbose parse --dynamic lib.so input.txt
```

Shows:
- Library loading steps
- Symbol resolution details  
- Failure point for currently unsupported parse modes

### Performance Tips

1. **Use Rust `grammar::parse`** for current parser execution
2. **Enable Performance Monitoring** to identify bottlenecks
3. **Treat CLI parse output formats as future work**
4. **Limit Text Length** in serialization for large trees
5. **Use GLR Runtime** for best performance and features

## Integration Examples

### Shell Scripting

```bash
#!/bin/bash
# Future sketch: parse all Python files in a directory once CLI parse output exists

for file in *.py; do
    result=$(adze parse --dynamic libtree-sitter-python.so "$file" --format json)
    status=$(echo "$result" | jq -r '.status')
    
    if [ "$status" = "ok" ]; then
        echo "✅ $file parsed successfully"
    else
        echo "❌ $file failed to parse"
    fi
done
```

### CI/CD Pipeline

```yaml
# GitHub Actions example
- name: Validate Grammar
  run: adze check src/grammar.rs

- name: Test Grammar  
  run: adze test --update=no

- name: Run Parser Tests
  run: cargo test
```

### Editor Integration

```json
// VS Code task
{
    "label": "Check Current Grammar",
    "type": "shell", 
    "command": "adze",
    "args": [
        "check",
        "src/grammar.rs"
    ]
}
```

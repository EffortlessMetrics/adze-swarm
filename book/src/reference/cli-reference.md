# CLI Reference

Reference for the current adze command-line interface.

> **Current support note:** `adze init`, `adze check`, `adze stats`, `adze build`,
> `adze test`, and `adze doc` are implemented. Static `adze parse` emits
> document-backed tree, S-expression, DOT, and JSON representations from a Rust
> grammar file. The CLI remains **Stabilizing**, and public JSON projections
> remain **Experimental**; see [Support Tiers](https://github.com/EffortlessMetrics/adze-swarm/blob/main/docs/status/SUPPORT_TIERS.md).
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
- `<NAME>` - A single project-directory name; existing non-empty destinations are refused

**Options:**
- `--output, -o <DIR>` - Output directory (default: current directory)

**Example:**
```bash
# Create an arithmetic starter project named json-parser
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
│   └── parse.rs
├── examples/
│   └── parse.rs
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
- `--output, -o <DIR>` - Parser artifact directory (default: `<project>/target/adze-build`)
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
those [support tiers](https://github.com/EffortlessMetrics/adze-swarm/blob/main/docs/status/SUPPORT_TIERS.md).

Dynamic mode is unavailable: `--dynamic` returns an error before loading any
library.

**Usage:**
```bash
adze parse <GRAMMAR> <INPUT> [OPTIONS]
```

**Arguments:**
- `<GRAMMAR>` - Rust grammar file path; shared-library parsing is unavailable
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
zero. That reports command completion, not an error-free parse: a document can
contain recovery diagnostics or extraction errors. Inspect its canonical outcome
and diagnostics rather than inferring clean parsing from the exit code alone.
Use `adze check` to validate a grammar or `adze build` to generate parsers for a
Rust application.

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
- `[PATH]` - Grammar project directory or its `Cargo.toml` (default: current directory)

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

Dynamic parsing is unavailable. `--dynamic` rejects the request before loading
any shared library or resolving a symbol. Changing `--symbol`, choosing a
platform-specific library suffix, or rebuilding with the `dynamic` feature does
not provide a working CLI dynamic parser. Use a trusted Rust grammar with the
static path instead.

The remaining dynamic-mode contract is tracked in
[#872](https://github.com/EffortlessMetrics/adze-swarm/issues/872). No supported
plugin ABI, library validation or platform coverage is implied by the flag.

## Execution Trust and Resource Limits

Static parsing compiles and executes a temporary Rust runner that includes the
supplied grammar. Treat the grammar, its dependencies and the Cargo build
configuration as executable code under your normal local trust boundary. This
is **not a sandbox**; do not run an untrusted grammar on the assumption that Adze
restricts its filesystem access or prevents code execution.

The current CLI has no enforced input-size cap, process deadline, bounded
stdout/stderr capture or process-tree teardown guarantee. It collects the child
process output before validating it. Output validation is not a memory, CPU,
stack-depth or arbitrary-code safety guarantee. These process-boundary gaps
remain tracked in
[#1066](https://github.com/EffortlessMetrics/adze-swarm/issues/1066).

Completed successful runner output must be valid UTF-8 and one JSON payload
with the requested existing schema and required serializer fields. A nonzero
child exit remains an error even if stdout looks valid. These checks prevent
malformed output from being rendered as a plausible result; they do not
establish that a document is free of syntax or extraction diagnostics.

## Environment Variables

### `RUST_LOG`

Control logging output:

```bash
# Show all debug information
RUST_LOG=debug adze parse grammar.rs input.txt

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

## Exit Status

Zero indicates successful command completion. Usage errors, unavailable modes,
build/runner failures and other operational errors produce nonzero exits.
The CLI does not implement a stable numeric taxonomy that assigns distinct
codes to missing features, invalid arguments and filesystem failures. Do not
interpret particular nonzero values as those categories.

For `parse`, successful command completion can still carry a recovered document
or diagnostics. Inspect the canonical document metadata and diagnostics for
parse quality. A child's failure status may appear in the CLI error message;
it is not a promise that the parent returns that same numeric status.

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

### Dynamic Requests

`adze parse --dynamic ...` is intentionally unavailable. It does not become a
supported parser by changing a library path, exported symbol or Cargo feature.
Use the static Rust grammar path; see [Dynamic Loading](#dynamic-loading).

### Static Runner Failures

Read stderr for the build or runner error. Check the grammar and input paths,
the available Rust toolchain, and the referenced dependencies. Successful
output goes to stdout; do not combine stderr with stdout when consuming JSON.

Tree, S-expression and DOT rendering retain the existing recursive JSON decoder
limit. JSON forwarding has separate deep-output controls, but those controls do
not establish an arbitrary-depth or bounded-resource guarantee. Changing to GLR
is not a general cure for a renderer-depth or process-resource failure.

### Debugging and Repeated Parsing

```bash
adze --verbose parse grammar.rs input.txt
```

Each static parse currently creates a temporary runner and invokes Cargo.
Executable reuse is unfinished under
[#1067](https://github.com/EffortlessMetrics/adze-swarm/issues/1067); a second
invocation is not a promised zero-Cargo cache hit. Applications that repeatedly
parse with an already compiled grammar can call `grammar::parse` or
`grammar::parse_document` directly. No relative speed or GLR performance claim
follows from this advice.

## Integration Examples

### Shell Scripting

This example records document output from a trusted static grammar. It reports
command completion only; downstream tooling must inspect the document's
canonical outcome and diagnostics to decide whether parsing was clean.

```bash
#!/usr/bin/env bash
set -eu

output=$(mktemp)
trap 'rm -f -- "$output"' EXIT

if adze parse grammar.rs input.txt --output document-json >"$output"; then
    cat "$output"
    printf '%s\n' 'Document emitted; inspect its outcome and diagnostics.' >&2
else
    printf '%s\n' 'Adze command failed; see stderr.' >&2
    exit 1
fi
```

### CI/CD Pipeline

These steps assume a source-built CLI is already on PATH and the selected
project's dependencies are available. `--update` is a boolean opt-in flag;
do not pass a boolean value to it. Keep snapshot updates disabled in CI:

```yaml
# GitHub Actions example
- name: Validate Grammar
  run: adze check src/grammar.rs

- name: Test Grammar
  run: INSTA_UPDATE=no adze test .
```

`adze test` already invokes Cargo tests for the selected project, so this
example does not immediately repeat the same `cargo test` obligation.

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

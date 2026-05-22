# Adze LSP Generator

Experimental Language Server Protocol (LSP) generator prototypes for Adze
grammars.

> **Support boundary:** `adze-lsp-generator` is not part of the Stable Adze
> product contract. Treat this crate as a prototype/advisory surface until the
> support-tier ledger names a stable LSP generation slice and proof command.
> Published `cargo install adze-lsp-generator` usage is not claimed here.

## Features

- **Server scaffolding experiments**: Generate prototype LSP server structure
  from a grammar-oriented configuration.
- **Feature templates**: Exercise completion, hover, and diagnostics handler
  generation where implemented.
- **Rust API surface**: Use `adze_ir` data and typed Rust configuration
  structures for generation experiments.

## Local Checkout Usage

Build or run the prototype from this repository checkout:

```bash
cargo build -p adze-lsp-generator
cargo run -p adze-lsp-generator --bin adze-lsp-gen -- --help
```

Do not use this README as a crates.io install receipt. A published install
claim requires an explicit release/publish approval and post-publish install
verification.

## Usage

### CLI Tool

Generate a prototype LSP server with all currently implemented templates:

```bash
adze-lsp-gen generate \
  --name my-language-lsp \
  --grammar ./my-grammar/src/lib.rs \
  --output ./my-lsp-server \
  --all-features
```

Generate with specific features:

```bash
adze-lsp-gen generate \
  --name my-language-lsp \
  --grammar ./my-grammar/src/lib.rs \
  --completion \
  --hover \
  --diagnostics
```

### Builder API

```rust
use adze_lsp_generator::LspBuilder;

fn main() -> Result<()> {
    LspBuilder::new("my-language-lsp")
        .version("1.0.0")
        .grammar_path("path/to/grammar.rs")
        .output_dir("./output")
        .feature("completion")
        .feature("hover")
        .feature("diagnostics")
        .build()?;
    
    Ok(())
}
```

## Features

### Completion

Provides intelligent code completion based on your grammar:
- Keywords from terminal symbols
- Symbol names from non-terminals
- Context-aware suggestions

### Hover

Shows documentation on hover with UTF-8 safe word extraction:
- **Grammar rule information**: Detailed descriptions of grammar rules
- **Keyword documentation**: Built-in documentation for common language keywords
- **Multi-language support**: Covers Rust, JavaScript/TypeScript, Python, and generic programming concepts
- **UTF-8 safe text processing**: Properly handles multi-byte characters and Unicode
- **Smart word boundaries**: Accurate word extraction at cursor position
- **Error recovery**: Graceful handling of invalid positions and file access errors

### Diagnostics

Prototype syntax error detection:
- Parse errors with exact locations
- Error recovery suggestions
- Incremental-update experiments

### Coming Soon

- **Semantic Tokens**: Syntax highlighting
- **Goto Definition**: Navigate to symbol definitions
- **Find References**: Find all usages of symbols
- **Rename**: Safe symbol renaming
- **Code Actions**: Quick fixes and refactoring

## Generated Server Structure

```
my-lsp-server/
├── Cargo.toml          # Dependencies and build config
├── main.rs             # Entry point
├── server.rs           # LSP server implementation
└── handlers.rs         # Feature handlers
```

## Running the Generated Server

1. Build the server:
   ```bash
   cd my-lsp-server
   cargo build --release
   ```

2. Run the server:
   ```bash
   ./target/release/my-language-lsp
   ```

3. Configure your editor to use the server

### VS Code Configuration

Create `.vscode/settings.json`:

```json
{
  "my-language.server.path": "./my-lsp-server/target/release/my-language-lsp"
}
```

## Configuration

Create an LSP config file:

```json
{
  "name": "my-language-lsp",
  "version": "1.0.0",
  "language_id": "my-language",
  "file_extensions": [".ml", ".mli"],
  "capabilities": {
    "incremental_sync": true,
    "semantic_tokens": false,
    "code_actions": false,
    "formatting": false,
    "goto_definition": false,
    "find_references": false,
    "rename": false
  },
  "logging": {
    "level": "info",
    "stderr": true
  }
}
```

Then generate from config:

```bash
adze-lsp-gen from-config --config lsp-config.json
```

## Examples

### Basic Hover Example

Generate an LSP server with enhanced hover support:

```rust
use adze_lsp_generator::{LspGenerator, LspConfig};
use adze_ir::Grammar;

fn main() -> anyhow::Result<()> {
    // Load your grammar
    let grammar = Grammar::load_from_file("my_grammar.json")?;
    
    // Configure the LSP server
    let config = LspConfig {
        name: "my-language-lsp".to_string(),
        version: "1.0.0".to_string(),
        language_id: "my-lang".to_string(),
        file_extensions: vec![".mylang".to_string()],
        ..Default::default()
    };
    
    // Generate LSP server with hover support
    LspGenerator::new(grammar)
        .with_config(config)
        .with_hover()
        .generate("./generated-lsp")?;
    
    println!("LSP server with hover support generated");
    Ok(())
}
```

### Hover Features in Action

The generated hover handler provides documentation for:

- **Rust keywords**: `fn`, `let`, `mut`, `if`, `match`, `struct`, `enum`, etc.
- **Common types**: `String`, `Vec`, `Option`, `Result`, `bool`, `i32`, etc. 
- **JavaScript/TypeScript**: `function`, `const`, `class`, `interface`, `type`, etc.
- **Python constructs**: `def`, `class`, `import`, `async`, `await`, etc.
- **Generic programming**: `return`, `break`, `continue`, `while`, `for`, `try`, etc.

When the prototype hover provider recognizes one of these keywords, it can
return formatted documentation such as:

```
**fn**: Declares a function
```

### Prototype Examples

Example material is still advisory and should be treated as fixture/prototype
coverage, not a stable editor integration contract. Current and planned example
themes include:
- JavaScript LSP server
- Python LSP server with indentation
- Go LSP server

## Architecture

The LSP generator prototype is intended to:
1. Analyzing your adze grammar
2. Extracting keywords, symbols, and structure
3. Generating handler implementations
4. Creating a tower-lsp based server
5. Configuring capabilities based on features

## Contributing

Contributions are welcome! Areas for improvement:
- Additional LSP features
- Performance optimizations
- More language examples
- Editor integration guides

## License

Same as adze project.

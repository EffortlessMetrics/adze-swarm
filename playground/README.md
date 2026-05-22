# Adze Grammar Playground

Experimental playground prototypes for exercising Adze grammar ideas through
CLI and web-oriented interfaces.

> **Support boundary:** `adze-playground` is not part of the Stable Adze
> product contract. Treat this crate as an advisory developer prototype until
> the support-tier ledger names a stable playground slice and proof command.
> Published `cargo install adze-playground` usage is not claimed here.

## Features

- **CLI prototype**: Exercise grammar parsing and analysis commands from this
  repository checkout.
- **Web prototype**: Explore the shape of an Axum-backed playground surface.
- **Analysis experiments**: Try conflict, ambiguity, and statistics reporting
  ideas without treating them as release-quality diagnostics.
- **Visualization experiments**: Render parser output in developer-oriented
  forms where implemented.

## Local Checkout Usage

Build or run the prototype from this repository checkout:

```bash
cargo build -p adze-playground
cargo run -p adze-playground --bin adze-playground -- --help
```

Do not use this README as a crates.io install receipt. A published install
claim requires an explicit release/publish approval and post-publish install
verification.

## Usage

### Web Interface

Launch the prototype web server:

```bash
adze-playground web --grammar ./my-grammar/src/lib.rs --port 8080
```

Then open http://localhost:8080 in your browser.

### CLI Interface

Launch the prototype interactive terminal session:

```bash
adze-playground cli --grammar ./my-grammar/src/lib.rs
```

Commands:
- `parse <code>` - Parse code and show tree
- `test <name> <code>` - Add a test case
- `run` - Run all tests
- `analyze` - Analyze grammar for issues
- `stats` - Show grammar statistics
- `help` - Show available commands

### Run Tests

Execute a test suite without interaction:

```bash
adze-playground test \
  --grammar ./my-grammar/src/lib.rs \
  --tests ./tests.json \
  --format json
```

### Analyze Grammar

Get detailed analysis of your grammar:

```bash
adze-playground analyze \
  --grammar ./my-grammar/src/lib.rs \
  --format text
```

## Test File Format

Create test files in JSON format:

```json
[
  {
    "name": "Simple expression",
    "input": "1 + 2",
    "expected_tree": "(expr (num 1) + (num 2))",
    "should_pass": true,
    "tags": ["arithmetic", "basic"]
  },
  {
    "name": "Invalid syntax",
    "input": "1 +",
    "should_pass": false,
    "tags": ["error"]
  }
]
```

## Web Interface Features

### Parse Tree Visualization
- See the parse tree structure where implemented
- Experiment with highlighted output
- Explore collapsible node concepts

### Grammar Analysis
- Rule statistics
- Conflict detection
- Advisory performance suggestions
- Ambiguity warnings

### Test Management
- Add tests from current input
- Run test suites
- See pass/fail results
- Export test results

### Performance Metrics
- Lexing time
- Parsing time
- Memory usage
- Token count

## CLI Interface Features

### Interactive Commands
- Prototype parsing feedback
- Test case management
- Grammar analysis
- Session persistence

### Colored Output
- Green for success
- Red for errors
- Blue for prompts
- Statistics tables

## Examples

### JavaScript Grammar Testing

```bash
# Launch web playground
adze-playground web \
  --grammar ../grammars/javascript/src/lib.rs \
  --tests ./js-tests.json

# CLI testing
adze-playground cli --grammar ../grammars/javascript/src/lib.rs
> parse function hello() { return "world"; }
Parse successful
Tree:
(program
  (function_declaration
    name: (identifier "hello")
    parameters: (parameter_list)
    body: (block
      (return_statement
        (string "world")))))
Time: 0.52ms
```

### Python Grammar Testing

```bash
# Analyze Python grammar in the prototype
adze-playground analyze --grammar ../grammars/python/src/lib.rs

Grammar Statistics:
  Rules: 142
  Terminals: 67
  Non-terminals: 75
  Avg rule length: 3.2

Suggestions:
  Info: Consider adding error recovery rules
  Warning: Left recursion detected in expression rules
```

## Prototype Architecture

The playground consists of:

1. **Core Library** - Grammar loading and parsing
2. **Web Server** - Axum-based HTTP API
3. **CLI Interface** - Interactive terminal UI
4. **Analyzer** - Grammar analysis engine
5. **Visualizer** - Tree rendering (SVG, DOT, ASCII)

## API Endpoints (Web Mode)

- `POST /api/parse` - Parse input text
- `POST /api/test` - Add test case
- `GET /api/tests` - Run all tests
- `GET /api/analyze` - Analyze grammar
- `GET /api/export` - Export session
- `POST /api/import` - Import session

## Contributing

The playground is extensible:

1. Add new analysis rules in `analyzer.rs`
2. Extend visualization in `visualizer.rs`
3. Add new CLI commands in `cli.rs`
4. Enhance web UI in `static/`

## Tips

- Use the web interface for visual exploration
- Use the CLI for automated testing
- Export sessions to share with others
- Use stable `adze` parser APIs for product parsing proof
- Add tests for edge cases

## License

Same as adze project.

# Golden-Master Tests for adze

This directory compares selected parser artifacts with checked-in reference outputs. Tree-sitter parity requires an independently generated reference and its provenance; a passing checked-in hash alone does not establish that claim.

## Structure

```
golden-tests/
├── fixtures/
│   └── minimal_json_grammar.json   # Tiny grammar fixture for stable node_types JSON
├── expected/
│   └── minimal_json_grammar.node_types.json
├── python/
│   ├── fixtures/        # Python source files to parse
│   └── expected/        # Expected S-expressions and hashes
├── javascript/
│   ├── fixtures/        # JavaScript source files to parse
│   └── expected/        # Expected S-expressions and hashes
├── generate_references.sh  # Script to generate reference outputs
└── src/lib.rs           # Test harness
```

## Running Tests

### Prerequisites

1. Install Tree-sitter CLI:
   ```bash
   npm install -g tree-sitter-cli
   ```

2. Install language grammars:
   ```bash
   # Python
   git clone https://github.com/tree-sitter/tree-sitter-python
   cd tree-sitter-python
   tree-sitter generate
   cd ..

   # JavaScript
   git clone https://github.com/tree-sitter/tree-sitter-javascript
   cd tree-sitter-javascript
   tree-sitter generate
   cd ..
   ```

### Generate Reference Files

Run this once to generate the golden-master reference files:

```bash
cd golden-tests
./generate_references.sh
```

This will:
- Parse each fixture file using the official Tree-sitter parser
- Generate S-expression output
- Calculate SHA256 hashes for efficient comparison
- Save both in the `expected/` directories

### Run Golden Tests

```bash
# Runs the default canary golden test (no grammar feature flags needed)
cargo test -p adze-golden-tests

# Run all golden tests (when grammars are integrated)
cargo test --features all-grammars

# Run only Python tests
cargo test --features python-grammar

# Run only JavaScript tests  
cargo test --features javascript-grammar
```

### Stable canary (smallest product-facing golden)

The `javascript/fixtures/canary_expression.js` fixture contains a single
semicolon (`;`) and is validated by
`javascript_canary_expression_golden` in strict mode. This test is the current
minimal canary that must parse and match a checked-in S-expression + SHA-256.

```bash
cargo test -p adze-golden-tests javascript_canary_expression_golden --features javascript-grammar -- --nocapture
```

### Language syntax projection and native provenance

The language S-expression view is a syntax projection. It omits a child only
when that child is an **anonymous extra leaf**, has nonempty text consisting
entirely of whitespace, and is neither an error nor a missing node. The native
parse tree still retains that extra, its flags, and its exact byte/point range.
The semicolon canary therefore projects `;` while separately asserting that the
native tree retains both the exact one-byte semicolon and the following newline
extra. Its existing S-expression and hash remain unchanged.

Punctuation, whitespace used as a semantic token, named extras (including named
comments), non-whitespace extras, structural extra nodes, recovery errors, and
missing nodes remain in the projection. Literal expected-output controls cover
these boundaries rather than deriving an expected string from parser output.
This normalization is specific to the golden syntax view; it does not filter
native/document traversal, diagnostics, queries, or typed extraction.

The external `generate_references.sh` script invokes a Tree-sitter CLI. In
contrast, `UPDATE_GOLDEN=1` currently saves **Adze's own projection**; this is a
baseline-maintenance mechanism, not an independent Tree-sitter oracle. Review
reference changes against independent syntax expectations before accepting them.

### Update References

If the expected output changes (e.g., due to grammar updates), regenerate references:

```bash
UPDATE_GOLDEN=1 cargo test --features all-grammars
```

Or use the script:

```bash
./generate_references.sh
```

## Adding New Test Cases

1. Add source file to `{language}/fixtures/`
2. Run `./generate_references.sh` to generate expected output
3. Add test case to `src/lib.rs`:

```rust
#[test]
#[cfg(feature = "python-grammar")]
fn python_my_new_test() -> Result<()> {
    run_golden_test(GoldenTest {
        language: "python",
        fixture_name: "my_new_test.py",
    })
}
```

## How It Works

1. **JSON canary (active by default)**: Builds a tiny grammar from `fixtures/minimal_json_grammar.json` and compares emitted `node_types_json` to `expected/minimal_json_grammar.node_types.json`.
2. **Reference Generation (feature-gated)**: Uses official Tree-sitter to parse fixtures and save S-expressions
3. **Hash Comparison**: Computes SHA256 of S-expressions for fast comparison
4. **Detailed Diff**: On mismatch, saves actual output for debugging
5. **CI Integration**: Tests run in CI to catch regressions

## Current Harness Gap

The language S-expression path in `src/lib.rs` currently soft-skips parse failures (`run_golden_test` prints `Skipping ...` and returns `Ok(())`). The default JSON canary exists so `cargo test -p adze-golden-tests` still proves one user-visible parser artifact is stable while the runtime grammar harness is being completed.

## Benefits

- **Byte-for-byte compatibility**: Ensures adze matches Tree-sitter exactly
- **Real-world code**: Tests against actual Python/JS code, not just synthetic examples
- **Fast comparison**: SHA256 hashes avoid storing large S-expression files in git
- **Easy debugging**: On failure, both expected and actual outputs are available
- **Regression prevention**: CI catches any deviation from expected behavior

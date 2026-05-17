# Golden Tests: Compatibility Receipts

Golden tests in Adze compare selected fixture outputs against Tree-sitter
reference parser output. They are advisory compatibility receipts for specific
grammar and projection surfaces; they are not a blanket full-parity guarantee.

## What Are Golden Tests?

Golden tests (also called reference tests or regression tests) compare Adze
output against "golden" reference files generated from Tree-sitter reference
parsers. They validate the fixture-backed subset of:

- **Parse tree structure**: Expected node hierarchy and relationships
- **Node types**: Expected named and anonymous node classifications
- **Source mapping**: Correct byte ranges and text extraction
- **Field assignments**: Proper field name mappings
- **Error handling**: Documented error recovery behavior where covered

## Tutorial: Running Your First Golden Test

### Prerequisites

Before running golden tests, ensure you have:

```bash
# 1. Install Tree-sitter CLI (for reference generation)
npm install -g tree-sitter-cli

# 2. Install language grammars (example: Python)
git clone https://github.com/tree-sitter/tree-sitter-python
cd tree-sitter-python
tree-sitter generate
cd ..
```

### Step 1: Generate Reference Files

Golden tests require reference files generated from Tree-sitter references:

```bash
# Navigate to golden-tests directory
cd golden-tests

# Generate references for all languages
./generate_references.sh
```

This creates reference files in the expected format:
```
golden-tests/
├── python/
│   ├── fixtures/simple_program.py     # Test source file
│   └── expected/
│       ├── simple_program.sexp        # Expected S-expression
│       └── simple_program.sha256      # Hash for fast comparison
└── javascript/
    ├── fixtures/simple_program.js
    └── expected/
        ├── simple_program.sexp
        └── simple_program.sha256
```

### Step 2: Run Golden Tests

Test adze parsers against the references:

```bash
# Test only Python grammar
cargo test --features python-grammar

# Test only JavaScript grammar  
cargo test --features javascript-grammar

# Current focused advisory canary
cargo test -p adze-golden-tests javascript_canary_expression_golden --features javascript-grammar -- --nocapture
```

### Step 3: Understanding Test Results

**Passing Test:**
```
test python_simple_golden ... ok
```

**Failing Test:**
```
test python_simple_golden ... FAILED

Parse tree mismatch for simple_program.py:
Expected hash: 4a2b8c9d...
Actual hash:   7f3e1a5b...

Expected S-expression saved to: python/expected/simple_program.sexp
Actual S-expression saved to: python/expected/simple_program.actual.sexp

To update golden files, run with UPDATE_GOLDEN=1
```

### Step 4: Debugging Differences

When tests fail, examine the differences:

```bash
# View the expected output
cat golden-tests/python/expected/simple_program.sexp

# View the actual adze output  
cat golden-tests/python/expected/simple_program.actual.sexp

# Use diff tools for detailed comparison
diff -u golden-tests/python/expected/simple_program.sexp \
        golden-tests/python/expected/simple_program.actual.sexp
```

## S-Expression Format

Golden tests use S-expression format for parse trees, matching Tree-sitter's output:

```lisp
(module
  (expression_statement
    (call
      (identifier) "print"
      (argument_list
        (string "\"Hello, world!\"")))))
```

**Format Details:**
- **Named nodes**: `(node_type ...)`
- **Anonymous nodes**: `"literal_text"`
- **Nested structure**: Reflects parent-child relationships
- **Field mapping**: Not shown in basic S-expressions
- **Source text**: Leaf nodes include quoted source text

The runtime `ts_compat::Node::to_sexp()` API has a narrower named-node
contract and includes field labels for named fielded children; these golden
fixtures describe the broader corpus comparison format.

## Working with Test Fixtures

### Adding New Test Cases

1. **Create fixture file**: Add source code to `{language}/fixtures/`
2. **Generate reference**: Run `./generate_references.sh`  
3. **Add test function**: Update `golden-tests/src/lib.rs`

Example test addition:

```rust
#[test]
#[cfg(feature = "python-grammar")]
fn python_class_definition() -> Result<()> {
    run_golden_test(GoldenTest {
        language: "python",
        fixture_name: "class_example.py",
    })
}
```

### Updating References

When parser behavior changes intentionally:

```bash
# Update a focused golden reference set
UPDATE_GOLDEN=1 cargo test --features python-grammar

# Or regenerate with the script
./generate_references.sh
```

## Integration with Continuous Integration

Golden tests run automatically in CI to catch regressions:

```yaml
# .github/workflows/golden-tests.yml
- name: Run Golden Tests
  run: |
    cd golden-tests
    ./generate_references.sh
    cargo test --features javascript-grammar
```

**Benefits for CI:**
- **Fast feedback**: SHA256 hashes enable quick comparison
- **Fixture coverage**: Tests selected real-world code patterns
- **Regression prevention**: Catches parser changes that affect output
- **Cross-platform validation**: Ensures consistent behavior

## Feature Flags and Language Support

Golden tests use feature flags for conditional compilation:

```toml
# Cargo.toml features
[features]
default = []
python-grammar = ["adze-python", "adze"]
javascript-grammar = ["adze-javascript", "adze"] 
```

**Fixture-backed Languages:**
- **Python**: Selected grammar fixtures, including external-scanner-shaped cases
- **JavaScript**: Selected ECMAScript fixture coverage
- **Future languages**: Extensible framework for additional fixture-backed grammars

## Troubleshooting Common Issues

### "Grammar feature not enabled"
```bash
# Error: Python grammar feature not enabled
# Solution: Enable the appropriate feature flag
cargo test --features python-grammar
```

### "No golden reference found"
```bash
# Error: No golden reference found for simple_program.py
# Solution: Generate references first
cd golden-tests && ./generate_references.sh
```

### "Tree-sitter CLI not found"
```bash
# Error: tree-sitter CLI not found
# Solution: Install Tree-sitter globally
npm install -g tree-sitter-cli
```

### "Failed to parse with tree-sitter"
```bash
# Error: Failed to parse input_file.py with tree-sitter
# Solution: Ensure grammar is installed and accessible
cd /path/to/tree-sitter-python
tree-sitter generate
```

## Next Steps

- **Read the [Testing Guide](testing.md)** for comprehensive testing strategies
- **Explore [S-Expression Reference](../reference/s-expression-format.md)** for format details
- **See [Architecture Documentation](architecture.md)** for parser internals
- **Review [Contributing Guide](contributing.md)** for adding new language support

Golden tests are one part of reliable parser development. Product claims still
graduate through `docs/status/SUPPORT_TIERS.md`, with explicit proof commands,
known limitations, and support-tier posture.

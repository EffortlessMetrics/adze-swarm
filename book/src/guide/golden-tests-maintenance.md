# Golden Tests Maintenance Guide

This how-to guide provides practical workflows for maintaining golden tests in
Adze. Golden tests are advisory compatibility receipts: they compare selected
Adze fixture outputs with Tree-sitter reference outputs, but they do not prove
full Tree-sitter parity by themselves.

## Adding New Test Cases

### Step 1: Choose Representative Code Samples

Select code samples that test specific language features:

```python
# good: tests specific syntax
def fibonacci(n):
    if n <= 1:
        return n
    return fibonacci(n-1) + fibonacci(n-2)

# better: tests edge cases  
class MyClass:
    """Docstring with unicode: 🐍"""
    def __init__(self, value=None):
        self._value = value or []
    
    @property
    def value(self):
        return self._value
```

**Selection Criteria:**
- **Language-specific features**: Test unique constructs (Python f-strings, JS arrow functions)
- **Edge cases**: Unicode, empty files, deeply nested structures
- **Common patterns**: Real-world code that users will parse
- **Error conditions**: Malformed input that should produce consistent errors

### Step 2: Create Fixture Files

Add test files to the appropriate language directory:

```bash
# Create Python test case
cat > golden-tests/python/fixtures/class_inheritance.py << 'EOF'
class Animal:
    def __init__(self, name):
        self.name = name
    
    def speak(self):
        raise NotImplementedError

class Dog(Animal):
    def speak(self):
        return f"{self.name} says Woof!"
EOF

# Create JavaScript test case  
cat > golden-tests/javascript/fixtures/async_await.js << 'EOF'
async function fetchData(url) {
    try {
        const response = await fetch(url);
        const data = await response.json();
        return { success: true, data };
    } catch (error) {
        return { success: false, error: error.message };
    }
}
EOF
```

### Step 3: Generate Reference Files

Use the reference generation script:

```bash
cd golden-tests
./generate_references.sh

# Verify files were created
ls -la python/expected/class_inheritance.*
ls -la javascript/expected/async_await.*
```

**Generated Files:**
- `*.sexp`: S-expression parse tree from Tree-sitter
- `*.sha256`: SHA256 hash for fast comparison

### Step 4: Add Test Functions

Update `golden-tests/src/lib.rs` with new test functions:

```rust
#[test]
#[cfg(feature = "python-grammar")]
fn python_class_inheritance() -> Result<()> {
    run_golden_test(GoldenTest {
        language: "python",
        fixture_name: "class_inheritance.py",
    })
}

#[test]
#[cfg(feature = "javascript-grammar")]
fn javascript_async_await() -> Result<()> {
    run_golden_test(GoldenTest {
        language: "javascript", 
        fixture_name: "async_await.js",
    })
}
```

### Step 5: Verify Tests

Run the new tests to ensure they pass:

```bash
# Test specific language
cargo test --features python-grammar python_class_inheritance
cargo test --features javascript-grammar javascript_async_await

# Run the current focused advisory canary
cargo test -p adze-golden-tests javascript_canary_expression_golden --features javascript-grammar -- --nocapture
```

## Updating Existing References

### When Parser Behavior Changes

When adze parser behavior changes intentionally, update references:

```bash
# Update a focused reference set
UPDATE_GOLDEN=1 cargo test --features python-grammar

# Update specific language
UPDATE_GOLDEN=1 cargo test --features python-grammar

# Manual regeneration
cd golden-tests
./generate_references.sh
```

### Reviewing Changes

Before committing updated references, review the changes:

```bash
# View changed files
git diff --name-only | grep -E "\.(sexp|sha256)$"

# Compare specific changes
git diff golden-tests/python/expected/simple_program.sexp

# Check if changes are expected
cat golden-tests/python/expected/simple_program.sexp
```

**Review Checklist:**
- [ ] Parse tree structure looks correct
- [ ] Node types match expected grammar rules
- [ ] Source text is properly preserved
- [ ] Changes align with parser modifications

## Debugging Test Failures

### Hash Mismatch Errors

When golden tests fail with hash mismatches:

```
test python_simple_golden ... FAILED

Parse tree mismatch for simple_program.py:
Expected hash: 4a2b8c9d...
Actual hash:   7f3e1a5b...

Expected S-expression saved to: python/expected/simple_program.sexp
Actual S-expression saved to: python/expected/simple_program.actual.sexp
```

**Debugging Steps:**

1. **Compare S-expressions**: Use diff tools to identify differences
   ```bash
   diff -u golden-tests/python/expected/simple_program.sexp \
           golden-tests/python/expected/simple_program.actual.sexp
   ```

2. **Identify root cause**: Common issues include:
   - Node type changes in grammar
   - Field name modifications  
   - Source text extraction differences
   - Error recovery behavior changes

3. **Verify with Tree-sitter**: Confirm expected behavior with Tree-sitter CLI
   ```bash
   echo "test_code" | tree-sitter parse --quiet
   ```

### Grammar Integration Issues

When tests fail due to grammar integration problems:

```
Error: Python grammar feature not enabled
```

**Solutions:**

1. **Enable feature flags**: Ensure correct features are enabled
   ```bash
   cargo test --features python-grammar
   ```

2. **Check grammar dependencies**: Verify `Cargo.toml` includes required grammars
   ```toml
   [dependencies]
   adze-python = { path = "../grammars/python", optional = true }
   ```

3. **Build grammar crates**: Ensure grammars are built correctly
   ```bash
   cargo build -p adze-python
   ```

### Parser Implementation Issues

When adze parser behavior differs from Tree-sitter:

1. **Test with minimal example**: Create simple test case
   ```python
   # minimal.py
   x = 1
   ```

2. **Compare parse trees**: Use both parsers
   ```bash
   # Tree-sitter reference
   echo "x = 1" | tree-sitter parse --quiet > tree_sitter.sexp
   
   # adze output (debug mode)
   ADZE_DEBUG=1 cargo test --features python-grammar -- --nocapture
   ```

3. **Check grammar rules**: Verify grammar definitions match
   ```rust
   // Check grammar extraction
   emit_ir!(my_grammar);  // Debug macro to show IR
   ```

## Managing Test Data

### Test File Organization

Keep test files organized and maintainable:

```
golden-tests/
├── python/
│   ├── fixtures/
│   │   ├── basic/           # Simple syntax tests
│   │   │   ├── variables.py
│   │   │   └── functions.py
│   │   ├── advanced/        # Complex features
│   │   │   ├── classes.py
│   │   │   └── decorators.py
│   │   └── edge_cases/      # Error conditions
│   │       ├── syntax_error.py
│   │       └── unicode.py
│   └── expected/
│       ├── basic/
│       ├── advanced/
│       └── edge_cases/
```

### Test Size Guidelines

Keep individual test files focused and manageable:

```python
# Good: focused on specific feature
def test_function():
    return "hello"

# Bad: tests too many features at once
class ComplexClass:
    def __init__(self):
        self.data = {}
    
    async def fetch_data(self):
        # ... 100 lines of complex code
```

**Size Recommendations:**
- **Basic tests**: 1-10 lines
- **Feature tests**: 10-50 lines  
- **Integration tests**: 50-200 lines
- **Edge case tests**: Usually small, focused on specific issues

### Fixture Naming

Use descriptive names that indicate what's being tested:

```
# Good naming
python/fixtures/class_inheritance.py
python/fixtures/async_generators.py
python/fixtures/f_string_expressions.py

# Poor naming
python/fixtures/test1.py
python/fixtures/example.py
python/fixtures/code.py
```

## Continuous Integration Integration

### CI Workflow Configuration

Golden tests integrate into CI pipelines:

```yaml
# .github/workflows/golden-tests.yml
name: Golden Tests
on: [push, pull_request]

jobs:
  golden-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Tree-sitter CLI
        run: npm install -g tree-sitter-cli
        
      - name: Install language grammars
        run: |
          git clone https://github.com/tree-sitter/tree-sitter-python
          cd tree-sitter-python && tree-sitter generate
          
      - name: Generate references
        run: |
          cd golden-tests
          ./generate_references.sh
          
      - name: Run golden tests
        run: cargo test --features javascript-grammar
```

### Handling CI Failures

When golden tests fail in CI:

1. **Check if references are committed**: Ensure reference files are in git
   ```bash
   git ls-files golden-tests/*/expected/
   ```

2. **Verify feature flags**: Ensure CI uses focused language features
   ```yaml
   - run: cargo test --features javascript-grammar  # not just 'cargo test'
   ```

3. **Cross-platform consistency**: Test locally on same OS as CI
   ```bash
   # Test in Docker container matching CI
   docker run --rm -v $(pwd):/workspace ubuntu:latest bash -c "
     cd /workspace
     # ... install dependencies and run tests
   "
   ```

## Performance Considerations

### Hash-Based Comparison

Golden tests use SHA256 hashes for efficient comparison:

```rust
// Fast: hash comparison
let expected_hash = "4a2b8c9d...";
let actual_hash = compute_hash(&actual_sexp);
assert_eq!(actual_hash, expected_hash);

// Slow: full string comparison (only on failure)
if actual_hash != expected_hash {
    let expected_sexp = load_expected_sexp();
    show_detailed_diff(&expected_sexp, &actual_sexp);
}
```

**Benefits:**
- Fast comparison for large files
- Small storage footprint in git
- Efficient CI execution

### Selective Test Execution

Run only relevant tests during development:

```bash
# Run single test during development
cargo test --features python-grammar python_simple_golden

# Run category of tests
cargo test --features python-grammar python_

# Run all golden tests for specific language
cargo test --features python-grammar golden
```

## Advanced Maintenance Tasks

### Adding New Languages

To add support for a new language:

1. **Create language directories**:
   ```bash
   mkdir -p golden-tests/rust/{fixtures,expected}
   mkdir -p golden-tests/go/{fixtures,expected}
   ```

2. **Update `Cargo.toml`**:
   ```toml
   [dependencies]
   adze-rust = { path = "../grammars/rust", optional = true }
   
   [features]
   rust-grammar = ["adze-rust", "adze"]
   ```

3. **Extend test framework**: Update parsing functions in `lib.rs`

4. **Update generation script**: Modify `generate_references.sh` for new language

### Bulk Test Updates

When making systematic changes:

```bash
# Update all Python tests
find golden-tests/python/fixtures -name "*.py" -exec basename {} \; | \
while read file; do
    echo "Updating $file..."
    UPDATE_GOLDEN=1 cargo test --features python-grammar "python_${file%.*}"
done

# Mass regeneration
cd golden-tests
rm -rf */expected/*
./generate_references.sh
```

### Test Coverage Analysis

Monitor test coverage across language features:

```bash
# Count test cases per language
echo "Python tests: $(ls golden-tests/python/fixtures/*.py | wc -l)"
echo "JavaScript tests: $(ls golden-tests/javascript/fixtures/*.js | wc -l)"

# Check coverage of language features
grep -r "class " golden-tests/python/fixtures/ | wc -l  # Class definitions
grep -r "async " golden-tests/javascript/fixtures/ | wc -l  # Async functions
```

## Troubleshooting Common Issues

### Tree-sitter CLI Issues

**Problem**: `tree-sitter not found`
```bash
# Solution: Install Tree-sitter CLI
npm install -g tree-sitter-cli
tree-sitter --version
```

**Problem**: Grammar not found
```bash
# Solution: Clone and generate grammar
git clone https://github.com/tree-sitter/tree-sitter-python
cd tree-sitter-python
tree-sitter generate
```

### Reference Generation Issues

**Problem**: Empty or malformed reference files
```bash
# Check Tree-sitter can parse the file
tree-sitter parse golden-tests/python/fixtures/problem.py

# Debug the generation script
bash -x golden-tests/generate_references.sh
```

### Integration Problems

**Problem**: Grammar dependencies not found
```bash
# Check grammar crate builds
cargo build -p adze-python
cargo build -p adze-javascript

# Verify feature flags
cargo metadata --format-version 1 | jq '.packages[] | select(.name=="adze-golden-tests") | .features'
```

## Next Steps

- **Read [Testing Guide](../development/testing.md)** for comprehensive testing strategies
- **See [S-Expression Reference](../reference/s-expression-format.md)** for format details
- **Review [Architecture Documentation](../development/architecture.md)** for parser internals
- **Check [Contributing Guide](../development/contributing.md)** for contribution workflows

Golden tests provide useful compatibility receipts for fixture-backed language
work. Public compatibility claims still need support-tier rows, proof commands,
and known limitations in `docs/status/SUPPORT_TIERS.md`.

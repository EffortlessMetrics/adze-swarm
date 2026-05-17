# Testing Guide

adze uses a comprehensive testing strategy combining multiple test types to ensure parser correctness, performance, and compatibility. This guide covers all testing approaches used in the project.

## Test Architecture Overview

```
adze testing ecosystem:
├── Unit Tests           # Individual component testing
├── Integration Tests    # Cross-component workflow testing  
├── Golden Tests        # Advisory Tree-sitter compatibility receipts
├── Property Tests      # Grammar-agnostic behavior validation
├── Snapshot Tests      # Visual regression testing with insta
├── Benchmark Tests     # Performance measurement and regression detection
└── Fuzzing Tests      # Automated input generation for edge cases
```

## Golden Tests: Compatibility Verification

Golden tests compare selected fixture outputs with Tree-sitter references. They
are advisory compatibility receipts, not a blanket full-parity claim.

### Quick Start

```bash
# Generate reference files (one-time setup)
cd golden-tests
./generate_references.sh

# Run the current focused advisory canary
cargo test -p adze-golden-tests javascript_canary_expression_golden --features javascript-grammar -- --nocapture

# Run language-specific packages when that surface changed
cargo test --features python-grammar
cargo test --features javascript-grammar
```

### Workflow Integration

Golden tests integrate into your development workflow:

```bash
# 1. Make parser changes
vim runtime/src/parser.rs

# 2. Run golden tests to check compatibility
cargo test --features python-grammar

# 3. If expected changes, update references
UPDATE_GOLDEN=1 cargo test --features python-grammar

# 4. Review and commit updated golden files
git add golden-tests/
git commit -m "Update golden tests for parser changes"
```

**When to Use Golden Tests:**
- Checking selected-tree output against documented Tree-sitter references
- Testing real-world code samples
- Regression testing after parser modifications  
- Cross-platform compatibility validation
- Advisory compatibility receipts

## Unit Tests: Component Testing

Unit tests focus on individual functions and modules:

```bash
# Run all unit tests
cargo test

# Run tests for specific components
cargo test -p adze-glr-core
cargo test -p adze-tablegen
cargo test -p adze

# Run tests with specific features
cargo test -p adze --features glr
cargo test -p adze --features incremental_glr
```

### Test Categories

**Parser Core Tests:**
```bash
# GLR parsing engine
cargo test -p adze-glr-core

# Table generation and compression
cargo test -p adze-tablegen

# Runtime parsing functionality
cargo test -p adze test_parse
```

**Grammar Processing Tests:**
```bash
# Grammar extraction from Rust code
cargo test -p adze-tool

# Macro expansion and attribute processing
cargo test -p adze-macro

# Common utilities and shared logic
cargo test -p adze-common
```

## Integration Tests: Workflow Validation

Integration tests verify complete workflows across multiple components:

```bash
# Run integration tests
cargo test --test integration

# Test specific workflows
cargo test --test integration test_grammar_to_parser
cargo test --test integration test_incremental_parsing
cargo test --test integration test_error_recovery
```

### Key Integration Test Areas

**Grammar-to-Parser Pipeline:**
```rust
#[test]
fn test_complete_grammar_pipeline() {
    // 1. Define grammar with macros
    // 2. Extract with tool
    // 3. Generate parser tables  
    // 4. Parse sample input
    // 5. Verify parse tree structure
}
```

**GLR Parsing Integration:**
```rust
#[test]  
fn test_glr_ambiguity_handling() {
    // Test GLR parser with ambiguous grammar
    // Verify all valid parse paths are explored
    // Check conflict resolution and precedence
}
```

**Incremental Parsing Workflow:**
```rust
#[test]
fn test_incremental_editing() {
    // Parse initial source
    // Apply text edits
    // Verify efficient incremental reparsing
    // Check subtree reuse optimization
}
```

## Property Tests: Grammar-Agnostic Validation

Property tests validate parser behavior across different grammars and inputs:

```bash
# Run property tests
cargo test property_test

# Test specific properties
cargo test property_incremental_test
cargo test property_roundtrip_test
cargo test property_tree_invariants
```

### Property Test Examples

**Roundtrip Property:**
```rust
#[test]
fn property_roundtrip_test() {
    // For any valid input:
    // parse(input) -> tree
    // tree.text() == input
    // Ensures parse trees preserve original source
}
```

**Incremental Consistency:**
```rust
#[test] 
fn property_incremental_consistency() {
    // For any input and edit:
    // full_parse(edited_input) == incremental_parse(input, edit)
    // Ensures incremental parsing produces identical results
}
```

**Tree Invariants:**
```rust
#[test]
fn property_tree_invariants() {
    // For any parse tree:
    // - Parent/child relationships are consistent
    // - Byte ranges are non-overlapping and ordered
    // - Source text extraction is correct
}
```

## External Lexer Testing (New in PR #67)

External lexer utilities include comprehensive test coverage for FFI compatibility and Tree-sitter integration:

### Test Categories

**Column Tracking Tests:**
```rust
#[test]
fn test_external_lexer_column_tracking() {
    let input = b"hello\nworld";
    let mut ext = ExternalLexer::new(input, 0, 0);
    let mut ts = create_ts_lexer(&mut ext);

    // Initial column at start
    assert_eq!(unsafe { ExternalLexer::get_column(&mut ts) }, 0);

    // Advance over "hello"
    for _ in 0..5 {
        unsafe { ExternalLexer::advance(&mut ts, false) };
    }
    assert_eq!(unsafe { ExternalLexer::get_column(&mut ts) }, 5);

    // Advance over newline resets column
    unsafe { ExternalLexer::advance(&mut ts, false) };
    assert_eq!(unsafe { ExternalLexer::get_column(&mut ts) }, 0);
}
```

**EOF Detection Tests:**
```rust
#[test]
fn test_external_lexer_eof() {
    let input = b"a";
    let mut ext = ExternalLexer::new(input, 0, 0);
    let mut ts = create_ts_lexer(&mut ext);

    assert!(!unsafe { ExternalLexer::eof(&mut ts) });
    unsafe { ExternalLexer::advance(&mut ts, true) };
    assert!(unsafe { ExternalLexer::eof(&mut ts) });
}
```

**Range Boundary Tests:**
```rust
#[test]
fn test_external_lexer_included_range_start() {
    let input = b"abc";
    let mut ext = ExternalLexer::new(input, 0, 0);
    let mut ts = create_ts_lexer(&mut ext);

    assert!(unsafe { ExternalLexer::is_at_included_range_start(&mut ts) });
    unsafe { ExternalLexer::advance(&mut ts, true) };
    assert!(!unsafe { ExternalLexer::is_at_included_range_start(&mut ts) });
}
```

### Running External Lexer Tests

```bash
# Run all external lexer tests
cargo test test_external_lexer

# Run specific external lexer functionality tests
cargo test test_external_lexer_column_tracking
cargo test test_external_lexer_eof
cargo test test_external_lexer_included_range_start

# Run runtime tests with explicit external-scanner coverage
cargo test -p adze --features external_scanners
```

### Integration Testing

External lexer utilities are tested for integration with:
- **Tree-sitter FFI Interface**: Exercising the supported `TSLexer` adapter subset
- **Memory Safety**: Pointer handling and null checks
- **Position Tracking**: Accurate byte and column position management
- **Error Handling**: Graceful handling of boundary conditions

**Support posture:**
- External scanners are experimental until promoted in `docs/status/SUPPORT_TIERS.md`.
- Focused tests should name the feature and parser surface they prove.
- Broad feature piles are avoided in ordinary PR guidance.

## Snapshot Tests: Visual Regression Testing

Snapshot tests use the `insta` crate for visual regression testing:

```bash
# Run snapshot tests
cargo test -p example

# Review and accept snapshot changes
cargo insta review

# Generate new snapshots
cargo test -p example
cargo insta accept
```

### Snapshot Test Workflow

```rust
#[test]
fn test_arithmetic_parsing() {
    let source = "1 + 2 * 3";
    let tree = parse_arithmetic(source);
    
    // Compare against stored snapshot
    insta::assert_yaml_snapshot!(tree_to_yaml(&tree));
}
```

**Benefits:**
- Visual diff of parse tree changes
- Easy approval workflow for expected changes
- Comprehensive coverage of grammar examples
- Human-readable test output

## Benchmark Tests: Performance Measurement

Benchmark tests measure and track parser performance:

```bash
# Run benchmarks
cargo bench

# Run specific benchmark suites
cargo bench --bench parsing_benchmarks
cargo bench --bench parse_bench
cargo bench --bench incremental_bench

# Generate performance reports
cargo bench -- --output-format html
```

### Benchmark Categories

**Parsing Performance:**
```rust
#[bench]
fn bench_python_large_file(b: &mut Bencher) {
    let source = load_large_python_file();
    b.iter(|| {
        adze_python::parse(&source)
    });
}
```

**GLR Performance:**
```rust
#[bench]
fn bench_glr_ambiguous_grammar(b: &mut Bencher) {
    let source = generate_ambiguous_input();
    b.iter(|| {
        glr_parser.parse(&source)
    });
}
```

**Memory Usage:**
```rust
#[bench]
fn bench_memory_usage(b: &mut Bencher) {
    b.iter_custom(|iters| {
        let start_memory = get_memory_usage();
        // Run parser operations
        let end_memory = get_memory_usage();
        Duration::from_nanos(end_memory - start_memory)
    });
}
```

## Fuzzing Tests: Edge Case Discovery

Fuzzing tests use automated input generation to find edge cases:

```bash
# Install cargo-fuzz
cargo install cargo-fuzz

# Initialize fuzzing
cd runtime/fuzz
cargo fuzz init

# Run fuzzing targets
cargo fuzz run fuzz_parser
cargo fuzz run fuzz_incremental
cargo fuzz run fuzz_glr_conflicts
```

### Fuzzing Targets

**Parser Input Fuzzing:**
```rust
fuzz_target!(|data: &[u8]| {
    if let Ok(source) = std::str::from_utf8(data) {
        let _ = adze_python::parse(source);
        // Should never crash, even on invalid input
    }
});
```

**Edit Sequence Fuzzing:**
```rust
fuzz_target!(|edits: Vec<Edit>| {
    let mut tree = initial_parse();
    for edit in edits {
        tree.edit(&edit);
        let _ = reparse(&tree);
        // Incremental parsing should remain consistent
    }
});
```

## Concurrency-Safe Testing

adze implements concurrency caps to ensure stable testing:

```bash
# Use concurrency-capped test commands
cargo t2                    # Run with 2 threads
cargo test-safe            # Run with safe defaults  
cargo test-ultra-safe      # Run with 1 thread

# Use preflight script for system pressure monitoring
./scripts/preflight.sh
./scripts/test-capped.sh
```

### Concurrency Configuration

**Environment Variables:**
```bash
export RUST_TEST_THREADS=2        # Rust test thread limit
export RAYON_NUM_THREADS=4        # Parallel processing limit  
export TOKIO_WORKER_THREADS=2     # Async runtime threads
export CARGO_BUILD_JOBS=4         # Build parallelism
```

**Automatic Detection:**
```bash
# Preflight script automatically sets conservative limits
# if system is under high PID pressure (>85% of pid_max)
./scripts/preflight.sh
```

## CI/CD Testing Strategy

### GitHub Actions Workflow

```yaml
name: Test Suite
on: [push, pull_request]
jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo test --all
        
  golden-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: |
          cd golden-tests
          ./generate_references.sh
          cargo test --features javascript-grammar
          
  property-tests:
    runs-on: ubuntu-latest
    steps:
      - run: cargo test property_test
      
  benchmarks:
    runs-on: ubuntu-latest
    steps:
      - run: cargo bench --no-run  # Compile but don't run in CI
```

### Test Connectivity Safeguards

adze includes multiple layers of protection against test disconnection:

**CI Test Connectivity Job:**
- Blocks commits containing `.rs.disabled` files
- Enforces non-zero test counts for all crates
- Reports per-crate test counts in PR summaries
- Detects orphaned test files

**Pre-commit Hook:**
```bash
# Prevents accidentally disabling tests
.git/hooks/pre-commit
```

**Local Verification:**
```bash
# Check test connectivity
./scripts/check-test-connectivity.sh
```

## Testing Best Practices

### Writing Effective Tests

**Test Naming:**
```rust
// Good: Descriptive and specific
#[test]
fn test_glr_handles_shift_reduce_conflict_with_precedence()

// Bad: Generic and unclear  
#[test]
fn test_parser()
```

**Test Structure:**
```rust
#[test]
fn test_specific_behavior() {
    // Arrange: Set up test data
    let source = "1 + 2";
    
    // Act: Perform the operation
    let result = grammar::parse(source);
    
    // Assert: Verify expected behavior
    assert!(result.is_ok());
}
```

**Error Testing:**
```rust
#[test]
fn test_error_conditions() {
    // Test both success and failure cases
    assert!(grammar::parse("1 + 2").is_ok());

    let report = grammar::parse_document("1 +");
    assert!(!report.diagnostics().is_empty());
}
```

### Test Maintenance

**Keep Tests Fast:**
```rust
// Use small, focused examples
let source = "x = 1";  // Good
let source = load_entire_django_codebase();  // Bad for unit tests
```

**Avoid Test Dependencies:**
```rust
// Each test should be independent
#[test]
fn test_independent_parsing() {
    let ast = grammar::parse("1 + 2").unwrap();
    assert_eq!(format!("{ast:?}"), "Add(Number(1), (), Number(2))");
}
```

**Use Appropriate Test Types:**
- **Unit tests**: Single function behavior
- **Integration tests**: Multi-component workflows  
- **Golden tests**: Advisory compatibility receipts
- **Property tests**: Universal invariants
- **Benchmarks**: Performance characteristics

## Next Steps

- **Explore [Golden Tests Guide](golden-tests.md)** for compatibility testing
- **Review [Architecture Documentation](architecture.md)** for system understanding
- **Read [Contributing Guide](contributing.md)** for development workflows
- **See [Performance Guide](../guide/performance.md)** for optimization strategies

A robust testing strategy keeps Adze reliable while preserving support-tier
truthfulness: generated typed parsing is the stable user path, document and
compatibility surfaces are promoted only when their proof commands, examples,
and limitations line up.

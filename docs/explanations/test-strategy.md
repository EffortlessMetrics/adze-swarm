# Adze Test Strategy

## Overview

This document outlines the test strategy for Adze's Rust-native parser
toolchain. The goal is not to claim blanket Tree-sitter parity; it is to prove
the advertised product surfaces:

- Rust types define the grammar.
- Generated parsers return typed AST values through `grammar::parse()`.
- `parse_document()` returns the canonical document for tooling.
- GLR conflict handling is deterministic and honest about ambiguity.
- Diagnostics, Tree-sitter-compatible projections, queries, JSON, CLI, WASM,
  and broader grammar support are promoted only when their support-tier rows
  have repeatable proof commands.

## Test Categories

### 1. Unit Tests (Per Module)

#### IR Module (`adze-ir`)
- [ ] Grammar construction and validation
- [ ] Symbol ID allocation and management
- [ ] Field ID ordering and validation
- [ ] Token pattern parsing and validation
- [ ] Precedence and associativity handling
- [ ] Fragile token marking
- [ ] Alias sequence management
- [ ] Production ID generation

#### GLR Core Module (`adze-glr-core`)
- [ ] FIRST set computation correctness
- [ ] FOLLOW set computation correctness
- [ ] LR(0) item generation
- [ ] LR(1) lookahead propagation
- [ ] Closure computation
- [ ] GOTO function correctness
- [ ] Conflict detection (shift/reduce, reduce/reduce)
- [ ] GLR fork point identification
- [ ] State merging logic

#### Table Generation Module (`adze-tablegen`)
- [ ] Action table encoding/decoding
- [ ] Goto table compression
- [ ] Small vs large table selection
- [ ] Row displacement algorithm
- [ ] Default reduction optimization
- [ ] Run-length encoding
- [ ] Symbol remapping
- [ ] Table dimension validation
- [ ] Bit-level encoding verification

### 2. Property-Based Tests

Using `proptest` or `quickcheck`:
- [ ] Grammar generation with random rules
- [ ] Parse table compression/decompression round-trips
- [ ] Action encoding preserves semantics
- [ ] State ID mapping consistency
- [ ] Symbol metadata invariants

### 3. Golden File Tests

Compare the supported compatibility subset against Tree-sitter's output:
- [ ] Grammar JSON generation
- [ ] Parse table binary format
- [ ] NODE_TYPES JSON format
- [ ] Language struct memory layout
- [ ] Compressed table format
- [ ] Symbol name arrays
- [ ] Field name ordering

### 4. Differential Testing

Run both implementations and compare:
- [ ] Parse same input with both parsers
- [ ] Compare parse trees
- [ ] Compare error recovery behavior
- [ ] Compare incremental parsing results
- [ ] Compare syntax highlighting queries

### 5. Grammar Corpus Tests

Test with real-world grammars:
- [ ] JSON grammar
- [ ] JavaScript grammar
- [ ] Python grammar
- [ ] Rust grammar
- [ ] C grammar
- [ ] Go grammar
- [ ] Ruby grammar
- [ ] TypeScript grammar

For each grammar:
- [ ] Grammar loads correctly
- [ ] Parse tables generate without errors
- [ ] Language struct validates
- [ ] Can parse simple examples
- [ ] Can parse complex real-world files
- [ ] Performance is comparable

### 6. Fuzzing

#### Grammar Fuzzing
- [ ] Generate random grammars
- [ ] Ensure no panics during table generation
- [ ] Verify table compression doesn't corrupt data

#### Input Fuzzing
- [ ] Fuzz parser with random inputs
- [ ] Ensure no crashes or panics
- [ ] Verify error recovery doesn't infinite loop

#### Table Fuzzing
- [ ] Corrupt compressed tables
- [ ] Ensure graceful error handling
- [ ] No memory safety issues

### 7. Performance Tests

#### Benchmarks
- [ ] Grammar compilation time
- [ ] Table compression time
- [ ] Parse table size
- [ ] Parsing throughput
- [ ] Memory usage
- [ ] Incremental parsing speed

#### Regression Tests
- [ ] Track performance over time
- [ ] Alert on significant slowdowns
- [ ] Compare with C implementation

### 8. Integration Tests

#### Build System
- [ ] build.rs integration works
- [ ] Multiple grammars in one project
- [ ] Cross-compilation support
- [ ] WASM target support

#### Runtime Integration
- [ ] Extract trait works correctly
- [ ] Error collection and reporting
- [ ] Span tracking accuracy
- [ ] Node navigation APIs

### 9. ABI Compatibility Tests

#### Language Struct
- [ ] Size matches C struct exactly
- [ ] Field offsets are correct
- [ ] Pointer alignment is correct
- [ ] Can be passed through FFI

#### Function Tables
- [ ] Lexer function signature matches
- [ ] External scanner ABI matches
- [ ] All callbacks work correctly

### 10. Error Handling Tests

#### Grammar Errors
- [ ] Ambiguous grammar detection
- [ ] Undefined symbol references
- [ ] Circular rule dependencies
- [ ] Invalid precedence declarations

#### Runtime Errors
- [ ] Parse errors are reported correctly
- [ ] Recovery produces valid trees
- [ ] Timeout handling works
- [ ] Memory limits are respected

## Test Infrastructure

### Continuous Testing
- Run all unit tests on every commit
- Run integration tests on every PR
- Nightly fuzzing runs
- Weekly performance regression tests

### Test Data Management
- Corpus of test grammars
- Known-good parse outputs
- Binary format test vectors
- Performance baseline data

### Test Utilities
- Grammar builder DSL for tests
- Parse tree comparison functions
- Binary diff tools for tables
- Performance profiling harness

## Coverage Goals

- Unit test coverage: >95%
- Integration test coverage: >90%
- Grammar corpus coverage: 100% of popular languages
- Fuzzing: 24 hours without issues
- Zero panics in any test scenario

## Validation Criteria

Before any release:
1. All tests must pass
2. No performance regressions >5%
3. Memory usage comparable to C version
4. Successfully parse all corpus files
5. Fuzzing finds no new issues

## Test Development Process

1. **Test First**: Write tests before implementation
2. **Golden Files**: Capture supported Tree-sitter-compatible outputs as references
3. **Incremental**: Test each layer independently
4. **Automate**: All tests must be automated
5. **Document**: Each test should explain what it validates

## Critical Test Scenarios

### Parser Generation
- Grammar with 10,000+ rules
- Deeply nested grammar rules
- Highly ambiguous grammars
- Grammars with extensive precedence

### Runtime Parsing
- 10MB+ source files
- Deeply nested code structures
- Pathological input patterns
- Unicode edge cases

### Table Compression
- Maximum compression ratio
- Minimum table size
- Compression determinism
- Decompression speed

## Next Steps

1. Set up property-based testing framework
2. Create or import grammar corpus fixtures for the documented compatibility subset
3. Build golden file test infrastructure
4. Implement differential testing harness
5. Set up fuzzing infrastructure
6. Create performance benchmarking suite

This testing approach should make support-tier claims auditable. A surface is
not Stable because many tests exist; it is Stable only when the user-facing
claim, proof command, CI lane, examples, and known limitations agree.

# Architecture Overview

adze is a comprehensive parser generation framework that transforms Rust code annotations into high-performance parsers. This document explains the system architecture, component interactions, and design principles.

## System Architecture

```
adze ecosystem architecture:

┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   User Grammar  │    │  Build Process   │    │  Runtime Parse  │
│   Definition    │───▶│  Generation      │───▶│  Execution      │
└─────────────────┘    └──────────────────┘    └─────────────────┘
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│  macro/         │    │  tool/           │    │  runtime/       │
│  common/        │    │  ir/             │    │  └─ Generated   │
│  └─ Annotations │    │  glr-core/       │    │     parse() and │
│     Extraction  │    │  tablegen/       │    │     documents   │
└─────────────────┘    └──────────────────┘    └─────────────────┘
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│  Testing &      │    │  Quality         │    │  Integration    │
│  Validation     │    │  Assurance       │    │  & Deployment   │
└─────────────────┘    └──────────────────┘    └─────────────────┘
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│  golden-tests/  │    │  benchmarks/     │    │  tools/         │
│  └─ Tree-sitter │    │  testing/        │    │  wasm-demo/     │
│     Compatibility│    │  └─ Fuzzing      │    │  └─ Browser     │
└─────────────────┘    └──────────────────┘    └─────────────────┘
```

## Core Components

### Grammar Definition Layer

**Location**: `macro/`, `common/`

The grammar definition layer processes Rust type annotations and extracts grammar rules:

```rust
#[adze::grammar("arithmetic")]
mod grammar {
    #[adze::language]
    pub enum Expr {
        Number(
            #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
            u32,
        ),
        #[adze::prec_left(1)]
        Add(Box<Expr>, #[adze::leaf(text = "+")] (), Box<Expr>),
    }
}
```

**Process Flow**:
1. **Macro Processing**: `adze::grammar` collects annotated types
2. **Rule Extraction**: `common/` extracts grammar rules from type definitions
3. **IR Generation**: Converts Rust types to grammar intermediate representation

### Build-Time Generation

**Location**: `tool/`, `ir/`, `glr-core/`, `tablegen/`

The build system transforms grammar definitions into executable parsers:

```rust
// build.rs
fn main() {
    adze_tool::build_parsers(&PathBuf::from("src/grammar.rs"));
}
```

**Pipeline Stages**:
1. **IR Construction** (`ir/`): Creates grammar intermediate representation
2. **GLR Analysis** (`glr-core/`): Builds LR(1) automaton with conflict resolution
3. **Table Generation** (`tablegen/`): Compresses parse tables for efficient execution
4. **Code Generation** (`tool/`): Emits Rust code and FFI-compatible structures

### Runtime Execution

**Location**: `runtime/`

The supported runtime surface is generated parser code backed by the `adze`
crate. The normal product path is:

```rust
let ast = grammar::parse(source)?;

let report = grammar::parse_document(source);
let document = report.document();
let diagnostics = document.diagnostics();
```

`AdzeDocument` is the native parse-product boundary. Typed AST extraction,
typed CST wrappers, diagnostics, Tree-sitter-compatible selected-tree views,
JSON, and GLR ambiguity summaries are projections from the same document facts.

**Runtime features by support posture**:
- **Typed parsing**: Stable generated `grammar::parse()` path for supported
  generated grammars.
- **Document parsing**: Experimental `grammar::parse_document()` path for
  tooling, diagnostics, and projections.
- **GLR conflict routing**: Stabilizing for documented conflict classes.
- **Incremental parsing**: Experimental document lifecycle with honest
  full-reparse fallback metadata.
- **Compatibility projections**: Advisory Tree-sitter-compatible selected-tree
  and node metadata surfaces.

## Testing & Quality Assurance Architecture

### Golden Tests: Compatibility Foundation

**Location**: `golden-tests/`

Golden tests are advisory compatibility receipts for selected grammar fixtures.
They are high-value proof for parser and projection work, but they are not a
blanket claim of full Tree-sitter compatibility.

```
golden-tests/
├── python/
│   ├── fixtures/           # Test source files
│   │   ├── simple_program.py
│   │   └── class_example.py
│   └── expected/           # Tree-sitter references
│       ├── simple_program.sexp
│       ├── simple_program.sha256
│       └── class_example.sexp
├── javascript/
│   ├── fixtures/
│   └── expected/
├── generate_references.sh  # Reference generation
└── src/
    ├── lib.rs              # Test framework
    └── example_integration.rs
```

**Golden Test Architecture**:

1. **Reference Generation**: 
   - Uses official Tree-sitter parsers to generate expected S-expressions
   - Computes SHA256 hashes for efficient comparison
   - Maintains compatibility across Tree-sitter versions

2. **Compatibility Testing**:
   ```rust
   #[test]
   #[cfg(feature = "python-grammar")]
   fn python_simple_golden() -> Result<()> {
       run_golden_test(GoldenTest {
           language: "python",
           fixture_name: "simple_program.py",
       })
   }
   ```

3. **Multi-Language Support**:
   - **Python**: Grammar fixture coverage with indentation-sensitive cases
   - **JavaScript**: Grammar fixture coverage for selected ECMAScript shapes
   - **Extensible**: Framework for adding more fixture-backed grammars

4. **Advisory CI Integration**:
   ```yaml
   - name: Run Golden Tests
     run: |
       cd golden-tests
       ./generate_references.sh
       cargo test --features javascript-grammar
   ```

**Golden Test Benefits**:
- **Compatibility Receipts**: S-expression and hash comparisons for selected fixtures
- **Regression Prevention**: Catches parser changes that break compatibility
- **Real-World Validation**: Tests actual code, not just synthetic examples
- **Scoped CI**: Path-routed canaries keep compatibility proof available without
  making every PR pay for every fixture

### Multi-Layered Testing Strategy

```
Testing Pyramid:
                    ┌─────────────────┐
                    │ Integration     │
                    │ & Golden Tests  │
                    └─────────────────┘
               ┌─────────────────────────────┐
               │      Property Tests         │
               │   (Grammar-Agnostic)        │
               └─────────────────────────────┘
          ┌───────────────────────────────────────┐
          │           Unit Tests                  │
          │      (Component-Specific)             │
          └───────────────────────────────────────┘
     ┌─────────────────────────────────────────────────┐
     │                Fuzzing Tests                    │
     │          (Automated Edge Cases)                 │
     └─────────────────────────────────────────────────┘
```

**Testing Components**:
- **Unit Tests**: Individual component validation
- **Property Tests**: Grammar-agnostic behavioral invariants
- **Golden Tests**: Tree-sitter compatibility verification
- **Integration Tests**: End-to-end workflow validation
- **Fuzzing Tests**: Automated edge case discovery
- **Performance Tests**: Regression detection and optimization

## Data Flow Architecture

### Grammar Processing Pipeline

```
Grammar Definition → IR Construction → GLR Analysis → Table Generation → Code Emission
                                                                              │
                                                                              ▼
┌──────────────────┐    ┌─────────────────┐    ┌────────────────────┐    Generated
│ Rust Annotations │───▶│ Grammar Rules   │───▶│ LR(1) Automaton    │    Parser Code
│                  │    │                 │    │                    │        │
│ #[leaf(...)]     │    │ Symbol: "+"     │    │ States: 42         │        │
│ #[prec_left(1)]  │    │ Rule: Expr→..   │    │ Actions: S/R table │        ▼
└──────────────────┘    └─────────────────┘    └────────────────────┘    Runtime
                                                                       Executable
```

### Parse Execution Flow

```
Source Text → Lexical Analysis → GLR Parsing → Forest Building → Tree Construction
                    │                 │              │               │
                    ▼                 ▼              ▼               ▼
             ┌─────────────┐  ┌─────────────┐  ┌──────────┐  ┌─────────────┐
             │ Token       │  │ Parse       │  │ GLR      │  │ Tree-sitter │
             │ Stream      │  │ States      │  │ Forest   │  │ Tree        │
             │             │  │             │  │          │  │             │
             │ NUMBER "1"  │  │ State Stack │  │ Multiple │  │ (expr       │
             │ PLUS   "+"  │  │ Action Tbl  │  │ Parses   │  │   (number   │
             │ NUMBER "2"  │  │ Fork/Merge  │  │ Ambiguity│  │     "1")    │
             └─────────────┘  └─────────────┘  └──────────┘  │   "+"       │
                                                              │   (number   │
                                                              │     "2"))   │
                                                              └─────────────┘
```

## Component Interactions

### Build-Time Dependencies

```
Dependency Graph:
tool ───┐
        ├─→ common ───┐
        │             ├─→ ir ───┐
macro ──┘             │         ├─→ glr-core ───┐
                      │         │               ├─→ tablegen
                      │         └─→ testing ────┘
                      └─→ golden-tests
```

### Runtime Dependencies

```
Runtime Execution Stack:
Application Code
      │
      ▼
┌─────────────┐    ┌──────────────┐
│ runtime/    │───▶│ Generated    │
│ parse APIs  │    │ Language     │
└─────────────┘    │ Tables       │
      │            └──────────────┘
      ▼                    │
┌─────────────┐           ▼
│ GLR Engine  │    ┌──────────────┐
│ Fork/Merge  │───▶│ Parse Tables │
└─────────────┘    │ Action Cells │
      │            └──────────────┘
      ▼
┌─────────────┐
│ AdzeDocument│
│ projections │
└─────────────┘
```

## Design Principles

### 1. Correctness First

**Support-tiered proof**: Stable product claims must map to repeatable proof
commands in `docs/status/SUPPORT_TIERS.md`. Golden tests are one advisory signal
for Tree-sitter compatibility work; they do not promote a surface by themselves.

**Memory Safety**: Comprehensive bounds checking, integer overflow protection, and safe FFI interfaces prevent undefined behavior.

**Algorithmic Correctness**: GLR parser implementation follows established algorithms with formal verification properties.

### 2. Performance by Design

**Compile-Time Generation**: Parsers are generated at build time, enabling aggressive optimizations and zero runtime overhead.

**GLR Optimizations**: Stack merging, action caching, and ambiguity summaries
minimize parsing overhead for ambiguous grammars.

**Incremental Parsing**: The current product contract exposes document lifecycle
metadata and full-reparse fallback truthfully before claiming stable subtree
reuse.

### 3. Developer Experience

**Type Safety**: Generated parsers provide type-safe APIs that prevent common parsing errors at compile time.

**Rich Diagnostics**: Comprehensive error messages with source locations and recovery suggestions.

**Interactive Tools**: Playground environment for rapid grammar development and testing.

### 4. Compatibility

**Tree-sitter Interoperability**: Compatibility APIs project selected-tree and
metadata views from `AdzeDocument` for a documented subset. Full Tree-sitter
parity is not assumed.

**Cross-Platform**: The core Rust crates are portable. Browser/WASM surfaces are
advisory until promoted with support-tier proof.

**ABI Discipline**: Tablegen metadata and compressed table ABI claims require
explicit roundtrip proof before promotion.

## Scalability Architecture

### Concurrency Management

adze implements bounded concurrency to ensure stable operation across different environments:

```rust
// Concurrency caps automatically adjust to system resources
RUST_TEST_THREADS=2        // Test thread limit
RAYON_NUM_THREADS=4        // Parallel processing limit
TOKIO_WORKER_THREADS=2     // Async runtime threads
```

**Benefits**:
- Prevents resource exhaustion on constrained systems
- Ensures consistent performance across platforms
- Enables reliable CI/CD pipeline execution

### Memory Management

**Arena Allocation**: Optional arena allocators reduce fragmentation for large parse operations.

**Copy-on-Write**: Incremental parsing uses COW semantics to minimize memory usage during edits.

**Bounded Collections**: Internal data structures use bounded sizes to prevent memory exhaustion attacks.

## Extension Points

### Adding New Languages

1. **Create Grammar Crate**: Define Rust types with adze annotations
2. **Integration Tests**: Add language to golden-tests framework
3. **Build Configuration**: Update workspace and feature flags
4. **Documentation**: Add language-specific examples and guides

### Custom Scanners

```rust
#[derive(Default)]
struct CustomScanner {
    state: ScannerState,
}

impl ExternalScanner for CustomScanner {
    fn scan(&mut self, lexer: &mut Lexer, valid_symbols: &[bool]) -> ScanResult {
        // Custom lexical analysis
    }
}
```

#### External Lexer Integration (New in PR #67)

The architecture now supports external lexer utilities for seamless integration with Tree-sitter compatible systems:

**Integration Pattern**:
```rust
use adze::external_lexer::ExternalLexer;

// Create FFI-compatible lexer for external scanners
fn create_tree_sitter_lexer(input: &'static [u8]) -> TsLexer {
    let mut ext_lexer = ExternalLexer::new(input, 0, 0);
    create_ts_lexer(&mut ext_lexer)
}

// Usage in external scanner integration
impl ExternalScanner for TreeSitterCompatibleScanner {
    fn scan(&mut self, lexer: &mut Lexer, valid_symbols: &[bool]) -> ScanResult {
        // Create FFI-compatible lexer
        let ts_lexer = create_tree_sitter_lexer(lexer.input());
        
        // Use Tree-sitter FFI functions
        unsafe {
            let ch = ExternalLexer::lookahead(&ts_lexer as *const _ as *mut c_void);
            ExternalLexer::advance(&ts_lexer as *const _ as *mut c_void, false);
            let col = ExternalLexer::get_column(&ts_lexer as *const _ as *mut c_void);
        }
        
        // Return scan result
        ScanResult::Success(token_type)
    }
}
```

**Architecture Benefits**:
- **FFI Compatibility**: Direct integration with Tree-sitter external scanners
- **Column Tracking**: Accurate position tracking for diagnostics
- **Memory Safety**: Safe pointer handling with comprehensive null checks  
- **Range Detection**: Support for included range boundaries
- **EOF Handling**: Robust end-of-input detection

**Testing Integration**:
```rust
#[cfg(test)]
mod external_lexer_tests {
    use super::*;
    
    #[test]
    fn test_column_tracking() {
        let input = b"hello\nworld";
        let mut ext = ExternalLexer::new(input, 0, 0);
        let mut ts = create_ts_lexer(&mut ext);
        
        // Test column advancement and newline handling
        assert_eq!(unsafe { ExternalLexer::get_column(&mut ts) }, 0);
        // ... column tracking tests
    }
    
    #[test] 
    fn test_eof_detection() {
        // Test EOF boundary detection
    }
    
    #[test]
    fn test_range_boundaries() {
        // Test included range detection
    }
}
```

### Parser Extensions

The architecture supports multiple extension points:
- **Visitor Patterns**: For tree traversal and analysis
- **Query Systems**: For pattern matching and extraction
- **Serialization**: For tree persistence and transmission
- **Language Servers**: For editor integration and tooling

## Quality Metrics

### Test Coverage Tracking

```bash
# Test connectivity verification
./scripts/check-test-connectivity.sh

# Per-crate test counts for an explicit package or support lane
cargo test -p adze -- --list | wc -l
just ci-supported

# Golden test coverage
find golden-tests/*/fixtures -name "*.py" | wc -l
find golden-tests/*/fixtures -name "*.js" | wc -l
```

### Performance Benchmarks

```rust
// Built-in performance monitoring
std::env::set_var("ADZE_LOG_PERFORMANCE", "true");

// Automatic metrics:
// - Node count: 1247 nodes processed
// - Tree depth: 23 levels maximum
// - Conversion time: 2.1ms forest-to-tree
// - Memory usage: arena allocation tracking
```

### Compatibility Validation

Golden and projection tests provide quantitative compatibility metrics for the
documented subset:
- **Parse Success Rate**: Percentage of files successfully parsed
- **Selected-tree Accuracy**: Hash or S-expression comparison success rate
- **Performance Ratio**: Adze vs Tree-sitter timing for comparable fixtures
- **Memory Efficiency**: Peak memory usage comparison

## Future Architecture Evolution

### Planned Enhancements

**Query System Stabilization**: Complete query engine with predicate evaluation and pattern matching.

**Large Table Compression**: Advanced compression algorithms for production-scale grammars.

**Distributed Parsing**: Architecture support for parsing across multiple cores or machines.

**WebAssembly Optimization**: Enhanced WASM build pipeline with size optimization.

### Research Directions

**Machine Learning Integration**: Grammar inference and automatic optimization using ML techniques.

**Formal Verification**: Mathematical proofs of parser correctness and completeness.

**Domain-Specific Languages**: Specialized architectures for particular language families.

## Next Steps

- **Explore [Golden Tests Guide](golden-tests.md)** for compatibility testing
- **Read [Testing Guide](testing.md)** for comprehensive testing strategies
- **Review [Contributing Guide](contributing.md)** for development workflows
- **See [API Documentation](../reference/api.md)** for programmatic interfaces

The adze architecture provides a foundation for high-performance,
document-centered parser generation. The product contract is promoted through
support-tiered proof: generated typed parsing first, then document, GLR,
Tree-sitter-compatible, query, JSON, CLI, and WASM surfaces as their evidence
matures.

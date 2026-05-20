# GLR Incremental Parsing Architecture

**Status**: Experimental lifecycle architecture with conservative fallback
**Completion**: PR finalization established the fallback boundary; optimized reuse is future work.

This document explains the architectural decisions and design rationale behind adze's GLR incremental parsing implementation.

## Architecture Overview

### Design Philosophy

The GLR incremental parsing implementation follows a **GLR-first architecture** that prioritizes correctness and GLR-specific requirements over immediate performance optimization. This approach ensures that incremental parsing maintains the same behavioral guarantees as fresh parsing while providing a foundation for future optimizations.

### Key Architectural Decisions

#### 1. Fork-Aware Incremental Parser

**Decision**: Implement incremental parsing with explicit GLR fork tracking
**Rationale**: Traditional incremental parsing assumes a single parse tree, but GLR parsers maintain multiple parse paths (forks) for ambiguous regions. The incremental parser must track which forks are affected by edits.

```rust
pub struct GLRIncrementalParser {
    pub table: Arc<ParseTable>,
    pub grammar: Arc<Grammar>,
    pub fork_tracker: ForkTracker,         // Tracks GLR parse forks
    pub previous_forest: Option<Arc<ForestNode>>, // Previous parse result
}
```

**Trade-offs**:
- ✅ **Correct GLR behavior**: Preserves ambiguities during incremental updates
- ✅ **Selective recomputation**: Only affected forks need revalidation
- ❓ **Complexity**: More complex than single-tree incremental parsing
- ❓ **Memory overhead**: Fork tracking requires additional state

#### 2. Conservative Fallback Strategy

**Decision**: Implement temporary fallback to fresh parsing for consistency
**Rationale**: During development and testing, ensuring behavioral consistency between incremental and fresh parsing takes priority over performance optimization.

```rust
pub fn parse_incremental(
    &mut self,
    tokens: &[GLRToken],
    edits: &[GLREdit],
) -> Result<Arc<ForestNode>, String> {
    // If we have edits and a previous parse, try to reuse
    if !edits.is_empty() {
        let has_old_forest = /* check for old forest */;
        
        if has_old_forest {
            self.reparse_with_edits(tokens, edits) // Currently falls back to fresh
        } else {
            self.parse_fresh(tokens)
        }
    } else {
        self.parse_fresh(tokens)
    }
}
```

**Trade-offs**:
- ✅ **Behavioral consistency**: Incremental and fresh parsing produce identical results
- ✅ **Correctness guarantee**: No risk of incremental-specific bugs
- ✅ **Development velocity**: Allows completion of GLR architecture without premature optimization
- ❓ **Performance**: No immediate performance benefit from incremental parsing
- ❓ **User expectations**: May confuse users expecting immediate performance gains

#### 3. Direct Forest Splicing Foundation

**Decision**: Implement token-level differencing with forest reconstruction capabilities
**Rationale**: Provides the foundation for future high-performance incremental parsing while maintaining GLR compatibility.

**Algorithm Components**:

1. **Chunk Identification**: Token-level diff identifies unchanged prefix/suffix ranges
2. **Middle-Only Parsing**: Capability to parse only edited segments
3. **Forest Extraction**: Infrastructure for extracting reusable nodes from old forests
4. **Surgical Splicing**: Framework for combining prefix + middle + suffix forests

```rust
fn reparse_with_edits(
    &mut self,
    tokens: &[GLRToken],
    edits: &[GLREdit],
) -> Result<Arc<ForestNode>, String> {
    // Foundation for Direct Forest Splicing:
    // 1. Analyze edit ranges
    // 2. Extract reusable forest segments
    // 3. Parse only affected regions
    // 4. Splice results together
    
    // Current implementation: conservative fallback
    self.parse_fresh(tokens)
}
```

**Trade-offs**:
- ✅ **Future-ready**: Architecture prepared for advanced optimizations
- ✅ **GLR compatible**: Designed specifically for GLR forest structures
- ✅ **Modular design**: Clean separation of concerns for incremental components
- ❓ **Current complexity**: Infrastructure overhead without immediate benefits
- ❓ **Maintenance**: More code to maintain during development phase

#### 4. External Scanner Integration

**Decision**: Implement full external scanner support in incremental parsing workflow
**Rationale**: Many real-world grammars (Python, JavaScript) require external scanners for complex tokenization patterns. Incremental parsing must preserve scanner state across edits.

**Integration Points**:

1. **State Persistence**: Scanner state serialization/deserialization
2. **Range Validation**: Proper handling of multi-range token advancement
3. **Incremental Compatibility**: Scanner integration in incremental parse workflow

```rust
impl ExternalScanner for MyScanner {
    fn scan(&mut self, lexer: &mut Lexer, valid_symbols: &[bool]) -> ScanResult {
        // Scanner logic preserves state across incremental parses
    }
    
    fn serialize(&self, buffer: &mut Vec<u8>) {
        // State serialization for incremental persistence
    }
    
    fn deserialize(buffer: &[u8]) -> Self {
        // State restoration for incremental parsing
    }
}
```

**Trade-offs**:
- ✅ **Production readiness**: Supports real-world grammars with complex tokenization
- ✅ **State preservation**: Maintains scanner context across incremental updates
- ✅ **Compatibility**: Works with both pure Rust and C FFI scanners
- ❓ **Complexity**: Additional state management requirements
- ❓ **Performance**: Scanner state overhead during incremental operations

#### 5. Memory Safety and Error Handling

**Decision**: Implement comprehensive error handling and checked arithmetic throughout
**Rationale**: GLR incremental parsing involves complex state management and forest manipulation. Robust error handling prevents crashes and data corruption.

**Safety Features**:

1. **Checked Arithmetic**: All byte offset and range calculations use checked operations
2. **Comprehensive Error Types**: Specific error types for different failure modes
3. **State Validation**: Validation of parser state and forest consistency
4. **Resource Management**: Proper cleanup of GLR forks and forest nodes

```rust
// Example of checked arithmetic in edit handling
fn apply_edit(&mut self, edit: &GLREdit) -> Result<(), String> {
    let new_range = edit.start_byte
        .checked_add(edit.new_end_byte.saturating_sub(edit.start_byte))
        .ok_or("Edit range overflow")?;
    
    // Additional validation and error handling
    Ok(())
}
```

**Trade-offs**:
- ✅ **Memory safety**: Prevents crashes and undefined behavior
- ✅ **Error visibility**: Clear error messages for debugging
- ✅ **Production robustness**: Suitable for production use cases
- ❓ **Performance overhead**: Additional checks during parsing operations
- ❓ **Code complexity**: More error handling code to maintain

## Implementation Timeline and Evolution

### Phase 1: Foundation (Completed)
- ✅ GLR-aware parser architecture
- ✅ Fork tracking infrastructure
- ✅ External scanner integration
- ✅ Basic incremental API

### Phase 2: Conservative Implementation (Completed September 2025)
- ✅ Conservative fallback strategy
- ✅ Comprehensive testing and validation
- ✅ Documentation and examples
- ✅ Production readiness assessment

### Phase 3: Future Optimizations (Planned)
- ⏳ Advanced subtree reuse strategies
- ⏳ Performance optimizations for fork-specific updates
- ⏳ Enhanced ambiguity preservation during incremental parsing
- ⏳ Benchmarking and performance analysis

## Architectural Benefits

### Immediate Benefits (Current Implementation)

1. **GLR Compatibility**: Architecture specifically designed for GLR parsing requirements
2. **External Scanner Support**: Full support for complex tokenization patterns
3. **Correctness Guarantee**: Behavioral consistency with fresh parsing
4. **Foundation for Optimization**: Clean architecture ready for measured future optimization work
5. **Production Robustness**: Comprehensive error handling and memory safety

### Future Benefits (Post-Optimization)

1. **Performance Gains**: Potential for measured speedup through targeted reuse
2. **Ambiguity Preservation**: Maintain parse alternatives during incremental updates
3. **Advanced Use Cases**: Support for research applications and language analysis tools
4. **Resource Efficiency**: Reduced memory and CPU usage for large files

## Design Trade-offs Analysis

### Conservative Approach vs Aggressive Optimization

**Chosen**: Conservative approach with fallback
**Alternative**: Immediate aggressive optimization

**Rationale**: The conservative approach was chosen to:
1. Ensure correctness during development
2. Provide stable foundation for future work
3. Allow comprehensive testing of GLR architecture
4. Prevent premature optimization complications

### Single-Tree vs Multi-Fork Architecture

**Chosen**: Multi-fork architecture with explicit fork tracking
**Alternative**: Adapt single-tree incremental parsing

**Rationale**: GLR parsing fundamentally requires multiple parse paths. Attempting to adapt single-tree approaches would:
1. Lose GLR correctness guarantees
2. Require significant workarounds for ambiguous regions
3. Limit future GLR-specific optimizations

### Direct Forest Splicing vs Traditional State Restoration

**Chosen**: Direct Forest Splicing foundation
**Alternative**: Traditional GSS state restoration

**Rationale**: Direct Forest Splicing provides:
1. A plausible optimization model for GLR
2. Avoids expensive state restoration overhead
3. More natural fit for GLR forest structures
4. Cleaner separation between parsing and incremental logic

## Current Implementation Assessment

### Strengths

1. **Architectural Soundness**: Well-designed foundation for GLR incremental parsing
2. **Comprehensive Coverage**: Supports external scanners, error handling, and complex use cases
3. **Correctness Priority**: Behavioral consistency ensures reliable operation
4. **Future-Ready**: Prepared for performance optimizations without architectural changes
5. **Documentation**: Documentation for the architecture and current fallback boundary

### Areas for Future Enhancement

1. **Performance Optimization**: Enable actual incremental reuse for performance gains
2. **Advanced Fork Management**: More sophisticated fork tracking and reuse strategies
3. **Benchmarking**: Comprehensive performance analysis and optimization targets
4. **Research Applications**: Advanced features for grammar analysis and language research

## Conclusion

The GLR incremental parsing architecture represents a strategic investment in correctness-first design that provides a robust foundation for future measured optimizations. The conservative fallback approach preserves fresh-parse behavior while maintaining the flexibility needed for advanced GLR-specific incremental parsing techniques.

The implementation demonstrates that GLR incremental parsing can be integrated into Adze's architecture without creating a second parse truth. Optimized reuse, performance claims, and support-tier promotion remain future work that needs repeatable receipts.

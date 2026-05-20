# Direct Forest Splicing: Disabled GLR Incremental Parsing Design

> **⚠️ Implementation Status**: The Direct Forest Splicing algorithm is currently **disabled** and falls back to fresh parsing for consistency. The algorithm is implemented but has known issues that cause behavioral differences with fresh parsing. See `glr_incremental.rs` for details.

> **Claim Boundary**: This document is design history and theory, not a supported performance contract. Adze does not currently promise Direct Forest Splicing speedups, stable incremental reuse percentages, or stable incremental node identity.

> **Understanding-Oriented Documentation**: This document explains the theoretical foundations, design decisions, and architectural intent of the Direct Forest Splicing algorithm implemented in PR #62.

## Overview

Direct Forest Splicing is an experimental incremental parsing design for GLR (Generalized LR) parsers. Instead of traditional state restoration approaches, it attempts to combine parse forests through splicing operations. The implementation is disabled because fresh parsing remains the current correctness boundary.

**Key Innovation**: Parse only the changed region and surgically splice it with unchanged regions, avoiding the expensive state restoration that plagues traditional incremental parsers.

## Theoretical Foundation

### Traditional Incremental Parsing Limitations

Classical incremental parsing approaches suffer from several fundamental limitations:

1. **State Restoration Overhead**: Must reconstruct parser state at edit boundaries (3-4x performance penalty)
2. **Context Reconstruction**: Requires rebuilding parse stack and lookahead context
3. **Invalidation Cascades**: Small edits can invalidate large portions of the parse tree
4. **GLR Complexity**: Handling multiple parse stacks and ambiguous regions during state restoration

### The Direct Forest Splicing Paradigm

Direct Forest Splicing addresses these limitations through a fundamentally different approach:

```
Traditional:  [====Parse State Restoration====][Edit Region][====State Restoration====]
Direct:       [Reuse Prefix Forest]  +  [Parse Middle Only]  +  [Reuse Suffix Forest]
```

**Key Insight**: GLR parse forests can be surgically combined if we ensure clean boundaries at token edges and maintain range consistency.

## Algorithm Architecture

### Phase 1: Chunk Identification (Token-Level Diff)

The algorithm begins by performing a token-level diff to identify three distinct regions:

```rust
fn identify_chunks(old_tokens: &[Token], new_tokens: &[Token], edit: &Edit) -> ChunkBoundaries {
    let prefix_end = find_longest_common_prefix(old_tokens, new_tokens, edit.start_byte);
    let suffix_start = find_longest_common_suffix(old_tokens, new_tokens, edit.old_end_byte);
    
    ChunkBoundaries {
        unchanged_prefix: 0..prefix_end,
        edited_middle: prefix_end..suffix_start,
        unchanged_suffix: suffix_start..old_tokens.len(),
    }
}
```

**Critical Design Decision**: Boundaries are aligned to token edges, not arbitrary byte positions. This ensures that forest nodes have clean attachment points and prevents partial token reuse which could cause parsing inconsistencies.

### Phase 2: Middle-Only Parsing (State-Free Approach)

Instead of attempting state restoration, we parse the middle segment in isolation:

```rust
fn parse_middle_segment(
    parser: &mut GLRParser,
    middle_tokens: &[Token],
    left_context: &ForestNode,
    right_context: &ForestNode,
) -> Result<ParseForest> {
    // Key insight: GLR parsers can parse fragments with minimal context
    // because they maintain all possible interpretations internally
    
    let mut parser_state = GLRParserState::new();
    
    // Parse the middle segment completely fresh
    for token in middle_tokens {
        parser_state.process_token(token)?;
    }
    
    // Generate all possible parse forests for the middle segment
    parser_state.extract_all_forests()
}
```

**Design Intent**: The algorithm avoids state restoration by parsing the middle segment fresh. This is plausible because GLR parsers naturally handle ambiguity and can generate multiple interpretations, but Adze does not currently expose this path as a supported optimization.

### Phase 3: Forest Extraction (Conservative Reuse)

The algorithm extracts reusable subtrees from the old parse forest using conservative boundary checking:

```rust
fn extract_reusable_subtrees(
    old_forest: &ParseForest,
    edit_ranges: &[Range<usize>],
) -> Vec<Arc<ForestNode>> {
    let mut reusable = Vec::new();
    
    for node in old_forest.traverse_postorder() {
        // Conservative reuse: Only reuse if completely outside edit ranges
        if is_completely_outside_edits(&node.byte_range(), edit_ranges) {
            // Preserve GLR ambiguities during extraction
            if let Some(ambiguous_nodes) = node.ambiguous_interpretations() {
                for interpretation in ambiguous_nodes {
                    reusable.push(Arc::clone(interpretation));
                }
            } else {
                reusable.push(Arc::clone(node));
            }
        }
    }
    
    reusable
}
```

**GLR-Specific Considerations**: 
- Preserves all ambiguous interpretations during extraction
- Uses Arc-based sharing to avoid expensive deep copying
- Validates that reused nodes don't contain any tokens within edit ranges

### Phase 4: Surgical Splicing (Forest Combination)

The final phase combines all components into a coherent parse forest:

```rust
fn splice_forests(
    prefix_forest: ParseForest,
    middle_forest: ParseForest,
    suffix_forest: ParseForest,
    edit: &Edit,
) -> Result<ParseForest> {
    let mut spliced = ParseForest::new();
    
    // 1. Attach prefix forest (unchanged byte ranges)
    spliced.attach_prefix(prefix_forest);
    
    // 2. Insert middle forest with range correction
    let middle_offset = edit.start_byte;
    let middle_adjusted = middle_forest.adjust_ranges(middle_offset);
    spliced.attach_middle(middle_adjusted);
    
    // 3. Attach suffix forest with updated ranges
    let suffix_offset = edit.start_byte + (edit.new_end_byte - edit.old_end_byte);
    let suffix_adjusted = suffix_forest.adjust_ranges(suffix_offset);
    spliced.attach_suffix(suffix_adjusted);
    
    // 4. Validate forest consistency
    spliced.validate_byte_ranges()?;
    spliced.validate_parse_consistency()?;
    
    Ok(spliced)
}
```

**Range Correction Logic**: All byte positions in the suffix forest must be adjusted by the edit delta (new_size - old_size) to maintain consistency with the new input text.

## Performance Model

This section describes the intended cost model. It is not a benchmark receipt and should not be used as a release claim.

### Complexity Comparison

| Operation | Traditional Incremental | Direct Forest Splicing |
|-----------|------------------------|------------------------|
| State Restoration | O(edit_depth × context_size) | O(1) |
| Middle Parsing | O(middle_size) | O(middle_size) |
| Forest Extraction | O(tree_size) | O(reusable_nodes) |
| Range Adjustment | O(suffix_size) | O(suffix_size) |

**Total Complexity**: O(middle_size + reusable_nodes + suffix_size) vs O(edit_depth × context_size + middle_size + tree_size)

### Expected Optimization Levers

The intended optimization comes from several factors:

1. **State Restoration Elimination**: Avoids restoration work from traditional approaches
2. **Parse Locality**: Only parses the actual changed content, not surrounding context
3. **Forest Sharing**: Arc-based node sharing eliminates redundant memory allocation
4. **Conservative Boundaries**: Aggressive reuse through conservative boundary checking

### Historical Experiment (PR #62)

PR #62 included an experimental arithmetic-expression benchmark while this path was being explored. Those numbers are not a current support-tier receipt and are not repeated here as a product claim. Future incremental performance claims need repeatable fixtures, documented hardware/context, CI or scheduled receipts, and support-tier entries.

## GLR-Specific Advantages

### Ambiguity Preservation

Traditional incremental parsing struggles with ambiguous grammars because state restoration becomes exponentially complex. Direct Forest Splicing naturally handles ambiguity:

```rust
// All ambiguous interpretations are preserved during splicing
for interpretation in ambiguous_middle_forests {
    spliced_forest.add_interpretation(
        prefix_forest.clone(),
        interpretation,
        suffix_forest.clone(),
    );
}
```

### Conflict Resolution Preservation

GLR parsers resolve conflicts by maintaining multiple parse stacks. Direct Forest Splicing preserves these resolutions:

- **Shift/Reduce Conflicts**: All action alternatives are maintained in spliced forests
- **Reduce/Reduce Conflicts**: Multiple reduction interpretations are preserved
- **Precedence Relations**: Precedence-based resolutions remain valid across splice boundaries

## Memory Management and Safety

### Arc-Based Forest Sharing

The algorithm uses Arc (Atomic Reference Counting) for efficient forest node sharing:

```rust
pub struct ForestNode {
    symbol: SymbolId,
    byte_range: Range<usize>,
    children: Vec<Arc<ForestNode>>, // Shared ownership
    ambiguous_alternatives: Option<Vec<Arc<ForestNode>>>,
}
```

**Benefits**:
- No deep copying during extraction
- Automatic memory cleanup when nodes are no longer referenced
- Thread-safe sharing (important for future parallelization)

### Byte Range Validation

The algorithm includes comprehensive range validation to prevent corruption:

```rust
fn validate_forest_ranges(forest: &ParseForest, input_len: usize) -> Result<()> {
    for node in forest.traverse() {
        // Ensure all ranges are within input bounds
        if node.byte_range.end > input_len {
            return Err("Node range exceeds input length");
        }
        
        // Ensure parent ranges contain child ranges
        for child in &node.children {
            if !node.byte_range.contains(&child.byte_range) {
                return Err("Child range not contained in parent");
            }
        }
    }
    Ok(())
}
```

## Correctness Guarantees

### Conservative Reuse Principle

The algorithm follows a **conservative reuse principle**: Only reuse subtrees that are guaranteed to be unaffected by the edit operation.

```rust
fn is_safe_to_reuse(node: &ForestNode, edit_ranges: &[Range<usize>]) -> bool {
    // Conservative: Require complete separation from any edit
    for edit_range in edit_ranges {
        if node.byte_range.overlaps(edit_range) {
            return false; // Any overlap disqualifies reuse
        }
    }
    true
}
```

This conservative approach is designed to prioritize correctness at the cost of potentially reduced reuse. Adze should only document concrete reuse rates after those rates are backed by repeatable benchmark fixtures and support-tier receipts.

### Parse Consistency Validation

After splicing, the algorithm validates that the combined forest represents a valid parse:

1. **Byte Range Consistency**: All node ranges align with actual input positions
2. **Symbol Compatibility**: Adjacent forest segments have compatible symbols
3. **Grammar Rule Validity**: All productions remain valid after splicing
4. **Ambiguity Preservation**: No parse alternatives are lost during combination

## Future Optimizations

### Grammar-Aware Splicing

Future versions could use grammar analysis to determine optimal splice points:

```rust
fn find_optimal_splice_points(
    grammar: &Grammar,
    edit: &Edit,
    parse_tree: &ParseTree,
) -> Vec<SplicePoint> {
    // Analyze grammar productions to find natural boundaries
    // Prefer splice points at statement boundaries, expression boundaries, etc.
}
```

### Parallel Forest Processing

The Arc-based architecture enables parallel forest processing:

```rust
async fn parallel_forest_extraction(
    old_forest: &ParseForest,
    edit_ranges: &[Range<usize>],
) -> Vec<Arc<ForestNode>> {
    // Process different forest regions in parallel
    // Combine results using lock-free data structures
}
```

### Adaptive Reuse Strategies

Machine learning could optimize reuse strategies based on edit patterns:

```rust
struct AdaptiveReuseStrategy {
    edit_history: Vec<Edit>,
    reuse_effectiveness: HashMap<EditPattern, f64>,
}

impl AdaptiveReuseStrategy {
    fn optimize_reuse_boundaries(&self, edit: &Edit) -> OptimalBoundaries {
        // Use historical data to predict optimal reuse boundaries
    }
}
```

## Conclusion

Direct Forest Splicing remains a GLR-specific incremental parsing design that may inform future optimization work. Today, Adze uses fresh parsing as the correctness boundary when this path is disabled, so this document should be read as design context rather than a supported user-facing feature.

The design is still useful because it captures the constraints any future incremental GLR optimizer must satisfy: conservative reuse, range validation, ambiguity preservation, and explicit measurement before promotion.

**Key Takeaways for Implementers**:
1. **Avoid State Restoration**: Parse fresh regions rather than reconstructing complex parser states
2. **Embrace Conservative Reuse**: Aggressive reuse through conservative boundary checking
3. **Leverage Grammar Structure**: GLR forests are naturally composable with proper range management
4. **Validate Everything**: Comprehensive validation catches edge cases before they become bugs
5. **Measure Performance**: Instrument reuse effectiveness to guide optimization decisions

This approach may support future language-server, syntax-highlighting, and interactive-tooling work after correctness and performance receipts exist.

# Arena Allocator User Guide

**Version**: v0.8.0
**Status**: Internal optimization surface; see support tiers for product claims
**Target Audience**: adze library users

## Overview

The arena allocator provides efficient memory management for parse tree nodes through chunked allocation and handle-based references. It delivers **3.7x-5.0x speedup** over individual Box allocations with **99%+ fewer allocations**.

Those figures are historical arena-vs-Box microbenchmark receipts for this
allocator path. They are not a broad parser throughput claim, release promise,
or substitute for the product benchmark receipts tracked in
[`docs/status/SUPPORT_TIERS.md`](../status/SUPPORT_TIERS.md) and
[`docs/perf/baselines.md`](../perf/baselines.md).

## Why Use Arena Allocation?

### Traditional Approach (Box)

```rust
struct Node {
    value: i32,
    children: Vec<Box<Node>>,
}

// Each node = 1 malloc() call
// 10,000 nodes = 10,000 allocations
```

**Problems:**
- High allocation overhead
- Poor cache locality (nodes scattered across heap)
- Allocator contention
- Memory fragmentation

### Arena Approach

```rust
use adze::arena_allocator::{TreeArena, NodeHandle};

let mut arena = TreeArena::new();
let handle = arena.alloc(TreeNode::leaf(42));

// 10,000 nodes = ~10 chunk allocations
// Nodes stored contiguously in memory
```

**Benefits:**
- ✅ **3.7x-5.0x faster** allocation
- ✅ **99%+ fewer allocations** (reduces malloc calls from N to log N)
- ✅ **Better cache locality** (nodes in contiguous memory)
- ✅ **Memory reuse** across multiple parse sessions

## Quick Start

### Basic Usage

```rust
use adze::arena_allocator::{TreeArena, TreeNode, NodeHandle};

// Create arena
let mut arena = TreeArena::new();

// Allocate leaf nodes
let child1 = arena.alloc(TreeNode::leaf(1));
let child2 = arena.alloc(TreeNode::leaf(2));

// Allocate branch node
let parent = arena.alloc(TreeNode::branch(vec![child1, child2]));

// Access nodes
assert_eq!(arena.get(child1).value(), 1);
assert!(arena.get(parent).is_branch());
```

### Reusing Arena Across Parses

```rust
let mut arena = TreeArena::new();

// First parse
for i in 0..1000 {
    arena.alloc(TreeNode::leaf(i));
}

// Reset for next parse (reuses allocated memory)
arena.reset();

// Second parse (no new allocations!)
for i in 0..1000 {
    arena.alloc(TreeNode::leaf(i + 1000));
}
```

## Node Data Structure: TreeNodeData

The arena allocator works with **TreeNodeData** - a carefully optimized struct that represents parse tree nodes in memory.

### What is TreeNodeData?

TreeNodeData is the actual data stored in the arena for each parse tree node:

```rust
use adze::tree_node_data::TreeNodeData;
use adze::arena_allocator::NodeHandle;

// Create node data
let leaf = TreeNodeData::leaf(5, 0, 10);  // symbol=5, bytes 0-10

// Create node with children
let children = vec![NodeHandle::new(0, 0), NodeHandle::new(0, 1)];
let branch = TreeNodeData::branch(10, 0, 50, children);
```

### Why TreeNodeData?

TreeNodeData separates the **node payload** (symbol, byte range, children, flags) from the **tree structure** (how nodes are linked). This enables:

1. **Handle-based references**: Children are `NodeHandle` (8 bytes) instead of `Box<Node>` (8 bytes + heap allocation)
2. **Memory efficiency**: 64 bytes per node (exactly at target size)
3. **Cache-friendly**: All node data in contiguous memory
4. **Zero-copy**: No additional allocations for common cases (≤3 children)

### Key Features

**Compact representation**:
- Symbol/kind ID (u16)
- Byte range (u32 start, u32 end)
- Children handles (SmallVec - 0-3 inline, heap for more)
- Named child count tracking
- Optional field ID
- Packed flags (8 boolean flags in 1 byte)

**Total size**: 64 bytes ✅

### Usage in Parser Integration

When the parser builds trees using the arena:

```rust
// Parser creates TreeNodeData and stores in arena
let node_data = TreeNodeData::new(symbol, start_byte, end_byte);
let handle = arena.alloc(node_data);

// Later access via handle
let node_ref = arena.get(handle);
assert_eq!(node_ref.symbol(), symbol);
```

The arena returns `NodeHandle`, which you use to access the node later. This indirection provides:
- Lifetime safety (tree can't outlive arena)
- Stable references (handles don't change when arena grows)
- Efficient child storage (just 8 bytes per child reference)

**For more details**: See [`docs/specs/TREE_NODE_DATA_SPEC.md`](../specs/TREE_NODE_DATA_SPEC.md)

## API Reference

### TreeArena

#### Construction

```rust
// Create with default capacity (1024 nodes)
let mut arena = TreeArena::new();

// Create with specific initial capacity
let mut arena = TreeArena::with_capacity(4096);
```

#### Allocation

```rust
// Allocate a node and get handle
let handle: NodeHandle = arena.alloc(TreeNode::leaf(42));

// Get immutable reference
let node_ref = arena.get(handle);
assert_eq!(node_ref.value(), 42);

// Get mutable reference
let mut node_mut = arena.get_mut(handle);
node_mut.set_value(100);
```

#### Reset and Clear

```rust
// Reset: Clear all nodes but keep chunks (fast)
arena.reset();
assert_eq!(arena.len(), 0);
assert!(arena.capacity() > 0); // Chunks retained

// Clear: Drop all chunks except first (more aggressive)
arena.clear();
assert_eq!(arena.num_chunks(), 1);
```

#### Metrics

```rust
// Number of allocated nodes
let count = arena.len();

// Check if empty
if arena.is_empty() {
    println!("No nodes allocated");
}

// Total capacity across all chunks
let cap = arena.capacity();

// Number of chunks
let chunks = arena.num_chunks();

// Approximate memory usage in bytes
let bytes = arena.memory_usage();
```

### NodeHandle

Opaque identifier for nodes in the arena.

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeHandle {
    // Internal: chunk_idx + node_idx
}
```

**Properties:**
- **Copy**: Cheap to copy (8 bytes total)
- **Equality**: Can compare handles
- **Hash**: Can use in HashMaps/HashSets
- **Valid until reset**: Handles become invalid after `reset()` or `drop()`

### TreeNode

Simplified node type for demonstration. In production, use your custom node types.

```rust
// Create leaf node
let leaf = TreeNode::leaf(42);

// Create branch node
let branch = TreeNode::branch(vec![child1, child2]);

// Check node type
if node.is_leaf() {
    let value = node.value(); // Panics if branch
}
```

### TreeNodeRef / TreeNodeRefMut

Safe wrappers for node references.

```rust
// Immutable reference
let node_ref: TreeNodeRef<'_> = arena.get(handle);
assert_eq!(node_ref.value(), 42);
assert!(node_ref.is_leaf());

// Mutable reference
let mut node_mut: TreeNodeRefMut<'_> = arena.get_mut(handle);
node_mut.set_value(100);
```

## Common Patterns

### Pattern 1: Single Parse Session

```rust
fn parse(input: &str) -> Result<Tree> {
    let mut arena = TreeArena::new();

    // Build tree using arena...
    let root = build_tree(&mut arena, input)?;

    Ok(Tree { root, arena })
}
```

### Pattern 2: Parser with Reusable Arena

```rust
pub struct Parser {
    arena: TreeArena,
    // ...
}

impl Parser {
    pub fn parse<'a>(&'a mut self, input: &str) -> Result<Tree<'a>> {
        self.arena.reset(); // Reuse memory

        let root = self.build_tree(input)?;

        Ok(Tree {
            root,
            arena: &self.arena,
        })
    }
}
```

### Pattern 3: Long-Running Application

```rust
pub struct MultiParser {
    arena: TreeArena,
}

impl MultiParser {
    pub fn parse_many(&mut self, inputs: &[&str]) {
        for (i, input) in inputs.iter().enumerate() {
            self.arena.reset();

            // Parse input...

            // Every 100 parses, free excess chunks
            if i % 100 == 0 {
                self.arena.clear();
            }
        }
    }
}
```

## Performance Characteristics

### Time Complexity

| Operation | Amortized | Worst Case |
|-----------|-----------|------------|
| `alloc()` | O(1) | O(n) when allocating new chunk |
| `get()` | O(1) | O(1) |
| `get_mut()` | O(1) | O(1) |
| `reset()` | O(chunks) | O(chunks) |
| `clear()` | O(chunks) | O(chunks) |

### Space Complexity

- **Per-node overhead**: 0 bytes (no individual allocation metadata)
- **Arena overhead**: O(log N) for N nodes (chunk metadata)
- **Fragmentation**: At most (chunk_size - 1) nodes wasted per chunk

### Benchmark Results

From `cargo bench --bench arena_vs_box_allocation`:

| Nodes | Arena Time | Box Time | Speedup |
|-------|-----------|----------|---------|
| 100 | 855 ns | 3.37 µs | 3.9x |
| 1,000 | 8.1 µs | 29.9 µs | 3.7x |
| 10,000 | 80.7 µs | 401 µs | 5.0x |
| 100,000 | 841 µs | 3.90 ms | 4.6x |

## Safety and Lifetime Management

### Lifetime Ties Tree to Arena

```rust
pub struct Tree<'arena> {
    root: NodeHandle,
    arena: &'arena TreeArena,
}

impl<'arena> Tree<'arena> {
    pub fn root(&self) -> TreeNodeRef<'arena> {
        self.arena.get(self.root)
    }
}
```

**Compiler prevents use-after-free:**

```rust
// This won't compile!
let tree = {
    let mut arena = TreeArena::new();
    let root = arena.alloc(TreeNode::leaf(1));
    Tree { root, arena: &arena }
}; // ❌ Error: arena doesn't live long enough

tree.root(); // Would be use-after-free
```

### Handle Invalidation

```rust
let mut arena = TreeArena::new();
let handle = arena.alloc(TreeNode::leaf(42));

// Handle is valid
assert_eq!(arena.get(handle).value(), 42);

// Reset invalidates all handles
arena.reset();

// Using handle after reset is undefined behavior (debug panic)
// arena.get(handle); // ⚠️ Debug: panic! Release: UB
```

**Best practice**: Don't store handles across `reset()` calls.

## Choosing Between `reset()` and `clear()`

### Use `reset()` when:
- Parsing multiple inputs of similar size
- Memory reuse is important
- Chunks are reasonably sized

### Use `clear()` when:
- Occasional very large parse followed by many small parses
- Long-running application with varying input sizes
- Memory footprint is a concern

## Migration Guide

### From Box-based Trees

**Before:**
```rust
struct Node {
    value: i32,
    children: Vec<Box<Node>>,
}

fn build() -> Box<Node> {
    Box::new(Node {
        value: 42,
        children: vec![],
    })
}
```

**After:**
```rust
use adze::arena_allocator::{TreeArena, NodeHandle};

fn build(arena: &mut TreeArena) -> NodeHandle {
    arena.alloc(TreeNode::leaf(42))
}

// In your parser:
let mut arena = TreeArena::new();
let root = build(&mut arena);
```

### From Rc/Arc Trees

**Before:**
```rust
use std::rc::Rc;

struct Node {
    value: i32,
    children: Vec<Rc<Node>>,
}
```

**After:**
```rust
// Arena provides better performance and memory usage
// NodeHandle is 8 bytes (vs 8 bytes for Rc pointer + refcount overhead)
// No runtime reference counting overhead
```

## Advanced Usage

### Custom Chunk Sizes

```rust
// Small initial chunk for memory-constrained environments
let mut arena = TreeArena::with_capacity(256);

// Large initial chunk for known-large inputs
let mut arena = TreeArena::with_capacity(8192);
```

### Monitoring Memory Usage

```rust
fn parse_with_monitoring(input: &str) -> Tree {
    let mut arena = TreeArena::new();

    let root = build_tree(&mut arena, input);

    println!("Allocated {} nodes", arena.len());
    println!("Used {} chunks", arena.num_chunks());
    println!("Memory: {} bytes", arena.memory_usage());

    Tree { root, arena }
}
```

## Troubleshooting

### Problem: "Invalid node handle" panic

**Cause**: Using handle after `reset()` or with wrong arena

**Solution**: Ensure handles are only used with the arena that created them, and not after `reset()`.

### Problem: High memory usage

**Cause**: Arena grows but doesn't shrink automatically

**Solution**: Use `clear()` periodically in long-running applications

```rust
// Every N parses, free excess chunks
if parse_count % 1000 == 0 {
    arena.clear();
}
```

### Problem: Performance not as expected

**Cause**: May be measuring first allocation (includes chunk allocation)

**Solution**: Warm up arena before benchmarking

```rust
// Warm up
arena.reset();
for i in 0..1000 {
    arena.alloc(TreeNode::leaf(i));
}
arena.reset();

// Now benchmark actual workload
```

## Testing

Run arena allocator tests:

```bash
# Unit tests
cargo test -p adze arena_allocator

# Memory safety (Miri)
cargo +nightly miri test -p adze --test arena_allocator_test

# Address sanitizer
RUSTFLAGS="-Z sanitizer=address" cargo +nightly test -p adze --test arena_allocator_test

# Benchmark
cargo bench --bench arena_vs_box_allocation
```

## Further Reading

- [ADR-0001: Arena Allocator Decision](../adr/0001-arena-allocator-for-parse-trees.md)
- [Arena Allocator Specification](../specs/ARENA_ALLOCATOR_SPEC.md)
- [Performance Benchmarking Guide](./PERFORMANCE_BENCHMARKING.md)
- [Benchmark Results](../../benchmarks/results/arena_vs_box_summary.md)

## Support

For questions or issues with the arena allocator:
1. Check this guide and the ADR/specification
2. Run the test suite to verify expected behavior
3. File an issue on GitHub with benchmark results if performance is not as expected

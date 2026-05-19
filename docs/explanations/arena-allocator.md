# Arena Allocator Quick Reference

> **Status:** Internal optimization surface. Use the support-tier ledger and
> benchmark receipts for product claims.

The arena allocator's historical arena-vs-Box microbenchmarks report
**3.7x-5.0x faster** parse tree allocation with **99%+ fewer allocations**
compared to Box-based allocation. Treat these as allocator microbenchmark
receipts, not parser throughput guarantees.

## Quick Example

```rust
use adze::arena_allocator::{TreeArena, TreeNode};

let mut arena = TreeArena::new();

// Allocate nodes
let child1 = arena.alloc(TreeNode::leaf(1));
let child2 = arena.alloc(TreeNode::leaf(2));
let parent = arena.alloc(TreeNode::branch(vec![child1, child2]));

// Access nodes
assert_eq!(arena.get(child1).value(), 1);

// Reuse for next parse
arena.reset();
```

## Performance at a Glance

| Metric | Result | Target |
|--------|--------|--------|
| Speedup | **3.7x-5.0x** | ≥20% (1.2x) |
| Allocation Reduction | **99%+** | ≥50% |
| Memory Reuse | **Zero-cost reset** | N/A |

### Benchmark Results (10,000 nodes)

- **Arena**: 80.7 µs, ~10 allocations
- **Box**: 401 µs, 10,000 allocations
- **Speedup**: 5.0x

## When to Use

✅ **Use arena allocation when:**
- Parsing files (most common case)
- Building ASTs or parse trees
- Need predictable performance
- Want to reuse memory across parses

❌ **Consider alternatives when:**
- Nodes need individual lifetimes
- Tree must outlive parser
- Need to incrementally drop subtrees

## Key API

```rust
// Create arena
let mut arena = TreeArena::new();

// Allocate node → returns NodeHandle
let handle = arena.alloc(node);

// Access node → returns TreeNodeRef<'_>
let node = arena.get(handle);

// Reset for reuse
arena.reset();

// Metrics
arena.len()           // Node count
arena.capacity()      // Total capacity
arena.num_chunks()    // Chunk count
arena.memory_usage()  // Bytes used
```

## Node Data: TreeNodeData

The arena stores **TreeNodeData** - a 64-byte struct optimized for parse tree nodes:

```rust
use adze::tree_node_data::TreeNodeData;

// Create node data
let leaf = TreeNodeData::leaf(5, 0, 10);  // symbol, start, end
let branch = TreeNodeData::branch(10, 0, 50, children);

// Access data
node.symbol();        // Symbol/kind ID
node.byte_range();    // (start, end)
node.child_count();   // Number of children
node.children();      // &[NodeHandle]
node.is_named();      // Node flags
```

**Key features**:
- 64 bytes total (cache-friendly)
- SmallVec children (0-3 inline, heap for more)
- Handle-based child references
- Packed flags (8 in 1 byte)

**See**: [`TREE_NODE_DATA_SPEC.md`](specs/TREE_NODE_DATA_SPEC.md)

## Safety Guarantees

✅ **Miri verified** - No undefined behavior
✅ **ASan verified** - No memory errors
✅ **Lifetime safe** - Compile-time prevention of use-after-free
✅ **Handle validation** - Debug assertions catch invalid handles

## Documentation

- **Full Guide**: [docs/guides/ARENA_ALLOCATOR_GUIDE.md](guides/ARENA_ALLOCATOR_GUIDE.md)
- **Design Rationale**: [docs/adr/0001-arena-allocator-for-parse-trees.md](adr/0001-arena-allocator-for-parse-trees.md)
- **Specification**: [docs/specs/ARENA_ALLOCATOR_SPEC.md](specs/ARENA_ALLOCATOR_SPEC.md)
- **Benchmark Results**: [benchmarks/results/arena_vs_box_summary.md](../benchmarks/results/arena_vs_box_summary.md)

## Testing

```bash
# Run tests
cargo test -p adze arena_allocator

# Memory safety
cargo +nightly miri test -p adze --test arena_allocator_test

# Benchmarks
cargo bench --bench arena_vs_box_allocation
```

## Status

- ✅ **Phase 1**: Core implementation (v0.8.0)
- 🚧 **Phase 2**: Parser integration (in progress)
- ⏳ **Phase 3**: Default in v0.9.0
- ⏳ **Phase 4**: Stabilize in v1.0.0

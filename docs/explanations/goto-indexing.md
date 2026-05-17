# GOTO Indexing Invariants

This document describes the critical invariants maintained for GOTO table indexing in the adze GLR parser.

## Key Invariants

### 1. EOF Convention
- **EOF symbol MUST be SymbolId(0)** throughout the runtime
- All grammar symbols should start at SymbolId(1) or higher
- EOF normalization happens at table build time via `normalize_eof_to_zero()`

### 2. GOTO Indexing Modes

The parser supports two GOTO table indexing modes:

- **NonterminalMap**: Compact mode using `nonterminal_to_index` mapping
- **DirectSymbolId**: Direct indexing where column = symbol ID

Mode is auto-detected via `with_detected_goto_indexing()` based on table density.

### 3. Table Building Pattern

Always build tables using this pattern:

```rust
let table = build_lr1_automaton(&grammar, &first_follow)
    .expect("build")
    .normalize_eof_to_zero()
    .with_detected_goto_indexing();

// Verify invariants in debug builds
debug_assert_eq!(table.eof_symbol, SymbolId(0));
debug_assert!(table.symbol_to_index.contains_key(&table.eof_symbol));
```

### 4. GOTO Table Remapping

**NEVER** directly set `goto_indexing` field. Always use remapping methods:

```rust
// ✅ CORRECT - remaps table data
table.remap_goto_to_direct_symbol_id()
table.remap_goto_to_nonterminal_map()

// ❌ WRONG - corrupts table
table.goto_indexing = GotoIndexing::DirectSymbolId;
```

## Test Helpers

Use these helpers instead of direct column math:

```rust
use adze_glr_core::test_helpers::test::*;

// Get actions for state/symbol
let actions = actions_for(&table, state, symbol);

// Get GOTO destination
let next_state = goto_for(&table, state, nonterminal);

// Check for Accept on EOF
if has_accept_on_eof(&table, state) { ... }
```

## Symbol Allocation in Tests

Use the test allocator to avoid EOF collision:

```rust
use adze_glr_core::test_symbol_alloc::test::SymbolAllocator;

let mut alloc = SymbolAllocator::new();
let token_a = alloc.next();  // SymbolId(1)
let token_b = alloc.next();  // SymbolId(2)
// Never produces SymbolId(0)
```

## CI Checks

Run the check script to verify invariants:

```bash
cargo xtask check-goto-indexing
```

This command checks for:
- Direct `goto_indexing` assignments without remapping
- SymbolId(0) usage in non-EOF contexts
- Other indexing invariant violations

## Debugging Tips

If you encounter GOTO indexing issues:

1. Check that EOF is normalized: `assert_eq!(table.eof_symbol, SymbolId(0))`
2. Verify GOTO indexing mode matches table layout
3. Use test helpers instead of direct indexing
4. Run the check script to catch violations

## Historical Context

The GOTO indexing system was refactored in January 2025 to:
- Support both compact and direct indexing modes
- Maintain explicit indexing mode tracking via `goto_indexing` field
- Prevent silent table corruption from mode mismatches
- Enable proper GLR ambiguity handling with multiple parse paths

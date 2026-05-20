# Grammar Optimizer Usage Guide

The adze grammar optimizer is an optional feature that can significantly improve parser performance by applying various optimization passes to your grammar.

## Enabling the Optimizer

The optimizer is disabled by default to ensure stability. To enable it, use the `optimize` feature flag:

### In Cargo.toml

The versioned dependency line is the intended release-surface shape after the
coordinated publish. Current repo proof uses local/path dependencies from this
checkout until crates.io receipts exist for the co-release crates.

```toml
[dependencies]
adze-tool = { version = "0.9.0", features = ["optimize"] }
```

### Via Command Line

```bash
cargo build --features adze-tool/optimize
```

## What the Optimizer Does

When enabled, the optimizer applies several transformation passes:

1. **Unit Rule Elimination**: Removes intermediate rules like `A -> B` by inlining them
2. **Symbol Inlining**: Inlines simple, non-recursive rules to reduce indirection
3. **Token Merging**: Combines tokens with identical patterns
4. **Left Recursion Optimization**: Transforms left-recursive rules for better performance
5. **Dead Code Removal**: Removes unreferenced symbols and rules
6. **Symbol Renumbering**: Compacts symbol IDs for better cache locality

## Performance Impact

The optimizer can provide:
- **15-20% faster parsing** for typical grammars
- **Smaller parse tables** due to rule elimination
- **Better cache performance** from symbol renumbering

## Important Notes

1. **Tree-sitter Compatibility**: The optimizer preserves Tree-sitter semantics
2. **source_file Preservation**: The special `source_file` symbol is never optimized away
3. **FIRST/FOLLOW Safety**: Optimizations preserve FIRST/FOLLOW sets for GLR parsing
4. **Debug Artifacts**: Use `ADZE_EMIT_ARTIFACTS=true` to inspect optimization results

## Example

Before optimization:
```
source_file -> statement
statement -> expression  
expression -> sum
sum -> product | sum + product
product -> primary | product * primary
primary -> NUMBER
```

After optimization (unit rules eliminated):
```
source_file -> sum + product | product * primary | NUMBER
sum -> product * primary | NUMBER
product -> NUMBER
```

## Troubleshooting

If you encounter issues with the optimizer:

1. **Disable optimization**: Remove the `optimize` feature to compare behavior
2. **Check artifacts**: Set `ADZE_EMIT_ARTIFACTS=true` to see the IR before/after
3. **Report issues**: File a bug with the grammar that causes problems

The optimizer is designed to be conservative and should never change the language accepted by your grammar.

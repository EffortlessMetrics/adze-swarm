# Adze Language Support

> **Support contract:** grammar crates are advisory/reference surfaces unless
> [`SUPPORT_TIERS.md`](../status/SUPPORT_TIERS.md) explicitly promotes them.
> The stable product contract is the generated pure-Rust parser path for user
> grammars, not a bundled language-pack guarantee.

## Built-in Grammars

The following grammars are maintained within the Adze repository and serve as
reference implementations and integration fixtures:

| Language | Location | Features Demonstrated |
|----------|----------|-----------------------|
| **Python** | `grammars/python` | External scanners (indentation), Complex rules |
| **JavaScript** | `grammars/javascript` | Large grammar, GLR conflict resolution |
| **Go** | `grammars/go` | Standard grammar structure |
| **Python (Simple)** | `grammars/python-simple` | Simplified subset for testing |

## Importing Tree-sitter Grammars (Experimental)

Adze includes an experimental `ts-bridge` tool for extracting metadata from
existing Tree-sitter grammars. It is not a full imported-grammar compatibility
claim.

### Usage

**Note:** This feature is experimental and may require manual adjustments to the generated Rust code.

```bash
# Build the bridge tool
cargo build --manifest-path tools/ts-bridge/Cargo.toml

# Run it against a tree-sitter grammar repo
cargo run --manifest-path tools/ts-bridge/Cargo.toml -- /path/to/tree-sitter-rust
```

## Language Features Status

| Feature | Status | Notes |
|---------|--------|-------|
| **External Scanners** | Experimental | Python indentation scanner exists, but the scanner API is not a stable product contract. |
| **GLR (Ambiguity)** | Stabilizing | Conflict routing and ambiguity summaries have proof for documented classes; full policy is still maturing. |
| **Query System** | Advisory subset | Tree-sitter query compatibility is documented and proof-backed for a subset, not full parity. |
| **LSP Generation** | Experimental | Prototype available in `lsp-generator` crate. |

## Contributing New Languages

We welcome contributions of new grammars! Please see the [Developer Guide](../DEVELOPER_GUIDE.md) for how to set up a new grammar crate in the `grammars/` directory.

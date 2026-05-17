# Known Limitations

> **Doc status:** Aligned with the 2026-05 support-tier ledger.

Adze's stable product contract is intentionally narrower than the repository's
full implementation surface. The stable path is: define grammar-shaped Rust
types, generate a pure-Rust parser, and parse into typed Rust values through a
normal Cargo build.

For the authoritative tier and proof ledger, use
[`SUPPORT_TIERS.md`](../status/SUPPORT_TIERS.md). This page summarizes the
user-facing limitations.

## Stable Product Surface

- **Typed extraction**: generated parsers return typed Rust values for supported
  generated grammars.
- **Pure-Rust parser path**: clean downstream quickstarts and the checked-in
  downstream demo are covered by stable-product canaries.
- **Operator precedence**: stable for the documented arithmetic expression
  shape, not every possible ambiguous grammar.
- **Core table serialization**: stable for `adze-glr-core` parse-table
  serialization roundtrips.

## ⚠️ Experimental / Limited Features

### 1. GLR Ambiguity

GLR conflict routing has extensive proof for selected conflict classes, retained
alternatives, deterministic selected parses, and ambiguity summaries. It remains
**Stabilizing**, not Stable, until broader conflict coverage and selection
policy are promoted in the support tiers.

### 2. External Scanners

Support for custom Rust-based external scanners exists, but the API is still
experimental. This is required for indentation-sensitive languages like Python.
- **Status**: Used by the Python grammar example; not part of the stable product
  contract.

### 3. Query Language

Tree-sitter compatible query support is a documented subset, not full
Tree-sitter query parity.
- **Status**: Source-aware predicates, byte ranges, root-only matching, field
  constraints, anchors, and differential fixtures have advisory proof. Full
  query parity remains future work.

### 4. Incremental Parsing

Incremental parsing records honest fallback metadata today. It should not be
read as a stable reuse or performance claim.
- **Status**: Conservative full-reparse fallback metadata is covered; active
  forest-splicing and stable changed-range behavior remain experimental.
- **Visibility**: `IncrementalGLRParser::last_parse_status()` and
  `pure_incremental::IncrementalParser::last_parse_status()` expose whether the
  last run reused nodes or explicitly fell back to full reparse, plus edit
  invalidation ranges.

### 5. Tree-sitter Compatibility

Tree-sitter compatibility is an adapter over native document data. It exposes a
selected-tree subset with advisory proof, but Adze does not claim full
Tree-sitter runtime, node-types, query, or imported grammar corpus parity.

### 6. WASM

WASM currently has compile-check signal for the demo target. Browser/runtime
behavior is not certified as part of the stable contract.

## 📊 Language Compatibility

The grammar crates in this repository are reference and integration surfaces.
They are useful for development, but they are not currently Stable product
contracts by themselves.

| Language | Status | Notes |
|----------|--------|-------|
| Arithmetic | Advisory example | Demonstrates the stable typed parser and precedence quickstart shape. |
| JSON | Advisory example | Reference grammar and fixture surface, not a separate stable language package. |
| Go | Advisory example | Grammar crate smoke coverage exists; not a published support contract. |
| JavaScript | Advisory / stabilizing fixture | Large grammar and GLR/golden-test signal; not full ecosystem parity. |
| Python | Advisory / experimental scanner fixture | Exercises indentation scanning; external scanner API is still experimental. |
| Rust | Future | Complex grammar with many edge cases. |

## 🤝 Roadmap

For upcoming features and milestones, see [ROADMAP.md](../../ROADMAP.md).

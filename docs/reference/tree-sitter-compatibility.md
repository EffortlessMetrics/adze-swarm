# Tree-sitter Compatibility Reference

This reference defines Adze's current Tree-sitter compatibility contract. It
covers two related but separate surfaces:

- generated `TSLanguage` table-format and decode invariants;
- the `ts_compat` selected-tree adapter that projects from native Adze document
  facts.

Tree-sitter compatibility is an adapter surface. It does not define Adze's
native parse truth. `AdzeDocument` remains the canonical parse product, and
`ts_compat` exposes the selected tree for ecosystem interop.

Do not read this page as a full Tree-sitter parity claim. The supported subset
below is the current product contract; broader query, node-types, corpus, and
error-recovery parity remain explicitly tiered in
[`SUPPORT_TIERS.md`](../status/SUPPORT_TIERS.md).

## Selected-tree Compatibility Subset

The compatibility adapter exposes one selected tree. Native GLR ambiguity
summaries stay on `AdzeDocument`; `ts_compat` does not expose raw forest data or
multiple parse alternatives.

### Supported now

These method families are covered by current canaries and are the main
selected-tree subset users should build against:

| Area | Supported surface |
| --- | --- |
| Tree entry | `Tree::root_node()`, `Tree::language()`, document-backed tree creation |
| Child traversal | `child(i)`, `named_child(i)`, `child_count()`, `named_child_count()` |
| Sibling and parent traversal | `parent()`, `next_sibling()`, `prev_sibling()`, named sibling filtering |
| Cursor traversal | forward, reverse/end, reset/reuse, depth, descendant indexing |
| Ranges | `start_byte()`, `end_byte()`, `start_position()`, `end_position()` |
| Descendant lookup | byte-range and point-range descendant lookup |
| Field lookup | `child_by_field_name()`, public field IDs, child field-name lookup |
| Identity | alias-visible `kind()` / `kind_id()`, raw `grammar_name()` / `grammar_id()` |
| Node flags | `is_named()`, `is_extra()`, `is_error()`, `has_error()`, `is_missing()` where facts exist |
| S-expression | named-node S-expression output using alias-visible identity |
| Language metadata | field-name/id lookup and node-kind metadata lookup |

### Stabilizing

These surfaces are useful and covered by targeted tests, but still need broader
fixture and imported-grammar proof before promotion:

- alias-visible identity across all generated parser paths;
- selected-tree error and missing-node projection for recovered generated input;
- node-types metadata generated from the same language schema;
- GLR selected-tree determinism for a broader conflict matrix;
- parity against imported grammar fixtures beyond the current smoke/canary set.

### Advisory or future

These are not product claims yet:

- full Tree-sitter query parity;
- alias-visible node-types parity;
- parse-state metadata;
- changed-range/incremental edit parity;
- C ABI stability for arbitrary external consumers;
- full imported grammar corpus compatibility;
- raw GLR forest exposure through `ts_compat`.

### Proof commands

Representative selected-tree proof is tracked in
[`SUPPORT_TIERS.md`](../status/SUPPORT_TIERS.md). The main local canaries are:

```bash
cargo test -p adze --features "pure-rust,glr,ts-compat" --test ts_compat_selected_tree -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_tree_children -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_tree_cursor -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_language_fields -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_node_metadata -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_node_error -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_to_sexp -- --nocapture
```

Promotion of this subset should use the consolidated `ts_compat_selected_tree`
matrix plus targeted method-family canaries rather than relying only on
scattered tests.

## Critical ABI Contract

**DO NOT CHANGE WITHOUT UPDATING BOTH ENCODER AND DECODER**

This document defines the exact binary format and invariants that must be maintained for Tree-sitter compatibility.

### Action Tag Constants
- `Error = 0` - Error action
- `Shift = 1` - Shift to state
- `Reduce = 3` - Reduce by rule (Note: 2 is Recover in Tree-sitter)
- `Accept = 4` - Accept the input

### Column Layout
- **Dense mapping**: Columns are 0..N-1 with no gaps
- **Token-first ordering**: 
  - Tokens occupy columns `[0..tcols)` where `tcols = token_count + external_token_count`
  - Non-terminals occupy columns `[tcols..N)`
- **External tokens**: Must be within the token band

### Action Encoding
- **Shift**: Encodes target state ID
- **Reduce**: Encodes rule ID (child_count derived from `rules[id].rhs_len`)
- **NT GOTO**: Represented as `Shift(next_state)` in NT columns
- **Accept**: Located at `GOTO(I0, start_symbol)` on EOF

### Table Structure
- **Action table**: 2D array `[state][symbol] -> Vec<Action>`
- **Symbol mapping**: `symbol_to_index` provides column for each symbol
- **Rules**: Each rule has `lhs` symbol and `rhs_len` 
- **Production LHS**: `production_lhs_index[i]` gives column index of rule i's LHS

### Decoder Requirements
- Must iterate **all** columns (not just token columns)
- Must handle multi-action cells (GLR)
- Must respect precedence/associativity ordering
- No sentinels (65535) in dense band

### External Scanner Integration
- `lex_modes`: Array of size `state_count`
- `external_token_count`: Number of external tokens
- External scanner results map to columns `[token_count..token_count+external_token_count)`

### Compression
- Small-table uses index pairs for state/symbol lookup
- Large states use full row encoding
- Actions compressed with variable-length encoding

### Invariants Enforced by Tests
1. Tag constants verified at compile time
2. Accept = GOTO(I0, start) shape preserved
3. No sentinel values in symbol tables
4. EOF within token band (typical case)
5. LHS/production agreement
6. External tokens in correct band
7. Normalization performance bounded

## Format Versions
- Current: Tree-sitter Language Version 15
- Minimum Compatible: Version 13

## ABI Stability
The table ABI targets Tree-sitter language version 15 for the covered table
formats and action encodings. Compatibility claims are proof-driven: do not
claim full Tree-sitter runtime parity until the relevant method family, query
behavior, node-types metadata, and imported grammar corpus proof exist.

## Runtime Node Identity

This file covers generated `TSLanguage` table format and decode invariants.
The `ts_compat::Node` runtime identity APIs have a separate contract in
[`ts-compat-node-identity.md`](ts-compat-node-identity.md).
Current alias-aware semantics and remaining parity gaps are documented in
[`ts-compat-alias-semantics.md`](ts-compat-alias-semantics.md).

Current `ts_compat` nodes expose alias-visible identity for known production
alias sequence entries while preserving raw grammar identity:

- `kind()` and `kind_id()` use the alias-visible symbol when a production alias
  applies, otherwise the parsed node symbol,
- `grammar_name()` and `grammar_id()` remain the raw parsed grammar symbol,
  ignoring aliases,
- alias metadata is preserved in generated tables and decode output and is now
  projected into parsed node identity for the covered node-identity/S-expression
  canaries.

Do not change alias-visible node identity, S-expression alias behavior, or
node-types alias behavior without updating that contract and its canaries.

## Runtime Node-Types Metadata

`ts_compat::Language::node_types_json()` exposes an advisory Tree-sitter-style
`node-types.json` projection generated from the language grammar metadata. The
current projection covers generated node kinds and field metadata, but it is not
full Tree-sitter parity.

Alias-visible node-types remain intentionally future work. Current alias
metadata can be preserved at the table/native identity layers while
`node_types_json()` still reports the underlying grammar node types. Do not
promote this surface to query-compatible node-types parity until alias-visible
node-types canaries are added.

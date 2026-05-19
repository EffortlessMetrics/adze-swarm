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

Query compatibility has its own subset reference:
[`query-compatibility.md`](query-compatibility.md).

## How To Use The Adapter

Use the compatibility adapter when existing tooling expects Tree-sitter-shaped
`Tree` and `Node` traversal. Native Adze code should prefer
`grammar::parse()` for typed Rust values or `grammar::parse_document()` for
document facts.

There are two supported entry shapes.

Use `ts_compat::Parser` when the integration is already written in
Tree-sitter terms:

```rust
let mut parser = adze::ts_compat::Parser::new();
parser.set_language(language.clone())?;
let tree = parser.parse(source, None)?;
let root = tree.root_node();
```

Use `Tree::from_document` when the application already has the native document
and wants a compatibility view over the same parse truth:

```rust
let document = grammar::parse_document(source)?;
let tree = adze::ts_compat::Tree::from_document(language.clone(), &document);
let root = tree.root_node();
```

The second shape is the preferred product model for Adze-native tooling:
parse once into `AdzeDocument`, then project the selected Tree-sitter-shaped
view from that document.

## Concept Map

| Tree-sitter concept | Adze source |
| --- | --- |
| `Tree` | selected tree projected from `AdzeDocument` or parsed by `ts_compat::Parser` |
| `Node` | selected document node facts exposed through `ts_compat::Node` |
| `kind()` / `kind_id()` | alias-visible document identity |
| `grammar_name()` / `grammar_id()` | raw grammar identity |
| Fields | document edges plus generated language field metadata |
| Byte and point ranges | document node ranges |
| `ERROR`, missing, extra, aggregate error state | document flags and diagnostics where facts exist |
| S-expression | selected document tree |
| `node-types.json` | language metadata projection, still advisory for alias-visible parity |
| Queries | documented subset in [`query-compatibility.md`](query-compatibility.md) |

Use the native APIs instead when the code needs typed Rust AST values, raw GLR
ambiguity alternatives, user-facing diagnostics, stable JSON/CLI schemas, or
full Tree-sitter query parity. Those are separate Adze surfaces with separate
support-tier rows.

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

### Known gaps and not-planned boundaries

The selected-tree adapter is not a promise that every Tree-sitter consumer can
switch without inspection. Before adopting it, check these boundaries:

- query behavior is a documented subset, not full Tree-sitter query parity;
- alias-visible `node-types.json` parity is not promoted yet;
- imported grammar corpus parity is not promoted yet;
- parse-state metadata and incremental changed-range parity are not promoted;
- raw GLR forest data is native Adze data and is not exposed through
  `ts_compat`;
- diagnostics remain native `AdzeDocument` facts even when error and missing
  flags are projected onto compatibility nodes.

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

Do not promote this page, README wording, or support-tier rows to a Stable
Tree-sitter compatibility claim unless the relevant method family has a proof
command, CI lane, known-gap statement, and rollback path.

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

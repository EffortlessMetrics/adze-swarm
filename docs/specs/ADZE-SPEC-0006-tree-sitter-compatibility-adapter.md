# ADZE-SPEC-0006: Tree-sitter compatibility adapter

Status: accepted
Owner: runtime/ts-compat
Created: 2026-05-13
Linked proposal: ../proposals/ADZE-PROP-0002-api-foundation.md
Linked ADRs: ../adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Linked plan: ../../plans/0.9.0/api-foundation.md
Linked issues:
Linked PRs:
Support-tier impact: ../status/SUPPORT_TIERS.md
Policy impact: ../../policy/doc-artifacts.toml

## Problem

Tree-sitter compatibility is strategically valuable, but Tree-sitter should not
define Adze's native parse product. Compatibility must be an adapter over
document data with explicit subset, proof, and deviation rules.

## Behavior

### B1. `ts_compat` projects from document facts

Compatibility APIs must read native document data where possible. They must not
invent field IDs, error state, alias identity, ranges, or node metadata locally.

### B2. Selected tree only

Tree-sitter-compatible output exposes one selected tree. GLR ambiguity,
alternatives, and forest internals remain native Adze APIs.

### B3. Field behavior is edge-based

Tree-sitter field APIs map to document edge fields:

- `field_name_for_child`
- `field_id_for_child`
- `child_by_field_name`
- `child_by_field_id`
- cursor current field APIs

### B4. Identity behavior is explicit

Compatibility node identity maps to native visible and grammar identity:

| Tree-sitter concept | Adze source |
| --- | --- |
| `kind()` | visible node kind |
| `kind_id()` | visible kind ID |
| `grammar_name()` | grammar/original kind |
| `grammar_id()` | grammar/original kind ID |
| `is_named()` | language metadata plus node flags |
| `is_extra()` | node flags |
| `is_error()` | local error flag |
| `has_error()` | aggregate subtree flag |
| `is_missing()` | missing/recovery flag |
| `to_sexp()` | selected document tree |
| `node-types.json` | language schema |

### B5. Deviations are documented

Any unsupported upstream Tree-sitter behavior must be documented in the
compatibility spec, reference docs, support tiers, or known gaps.

## Non-Goals

- No full Tree-sitter query parity yet.
- No C ABI compatibility guarantee.
- No imported grammar corpus parity claim.
- No GLR forest exposure through `ts_compat`.

## Required Evidence

- Child traversal canary.
- Cursor traversal canary.
- Field name and field ID canaries.
- Kind and grammar identity canaries.
- Missing/error/has_error canaries.
- S-expression canary.
- Node-types JSON advisory canary.

## Acceptance Examples

```rust
let doc = grammar::parse_document("1 + 2")?;
let ts = adze::ts_compat::Tree::from_document(language.clone(), &doc);
assert_eq!(ts.root_node().kind(), doc.root().kind());
```

```rust
let left = ts.root_node().child_by_field_name("left").unwrap();
assert_eq!(left.start_byte(), doc.root().child_by_field_name("left").unwrap().start_byte());
```

## Test Mapping

- `runtime/tests/ts_compat_tree_cursor.rs`
- `runtime/tests/ts_compat_node_error.rs`
- `runtime/tests/ts_compat_to_sexp.rs`
- `runtime/tests/ts_compat_node_metadata.rs`
- `runtime/tests/ts_compat_node_types.rs`
- future `runtime/tests/ts_compat_document_projection.rs`

## Implementation Mapping

Primary implementation surfaces:

- `runtime/src/ts_compat/`
- `runtime/src/document*`
- `tablegen` language metadata and node-types output
- reference docs under `docs/reference/`

## CI Proof

```bash
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_tree_cursor -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_node_error -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_to_sexp -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_node_types -- --nocapture
git diff --check
```

## Metrics / Promotion Rule

`ts_compat` remains advisory or slice-stabilizing until each method family has a
document-backed invariant and canary. No doc may claim full Tree-sitter parity
until query, alias, node-types, missing/error, imported grammar, and corpus
proof are explicit.

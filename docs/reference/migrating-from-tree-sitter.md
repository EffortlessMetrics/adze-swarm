# Migrating From Tree-sitter

Status: advisory guide backed by the selected-tree and query compatibility
contracts.

This guide is for users who already know Tree-sitter's mental model and want to
understand where the same concepts live in Adze.

The key difference is:

```text
Tree-sitter:
  grammar.js -> parser -> Tree/Node -> queries

Adze:
  Rust grammar types -> generated parser -> AdzeDocument
      -> typed AST
      -> typed CST
      -> diagnostics
      -> ambiguity summaries
      -> Tree-sitter-compatible selected tree
      -> queries
      -> JSON / CLI projections
```

`AdzeDocument` is the native parse truth. Tree-sitter compatibility is an
adapter over the selected document tree, not the core parse product.

## Entry Points

| Tree-sitter concept | Adze equivalent | Notes |
| --- | --- | --- |
| `parser.parse(source)` | `grammar::parse(source)` | Use this when you want typed Rust values. |
| `Tree` | `grammar::parse_document(source)` | Use this when you want tooling facts and projections. |
| `Node` | `doc.root()`, document syntax nodes, or `ts_compat::Tree::from_document(...).root_node()` | Native APIs keep document facts; `ts_compat` exposes the selected-tree adapter. |
| `Language` | generated grammar module and `ts_compat::Language` | Metadata comes from generated language schema/table data. |
| `node-types.json` | `ts_compat::Language::node_types_json()` | Advisory until alias-visible node-types parity is proven. |
| Query | `adze::query` | Documented subset; text predicates require source-aware matching. |

## Typed Parser Path

Tree-sitter users often start with a tree and then write extraction code. In
Adze, the primary happy path is typed extraction:

```rust
let ast = grammar::parse("1 + 2 * 3")?;
```

Use this path for compilers, interpreters, code generators, and tests that want
semantic Rust values directly.

## Tooling Path

Use `parse_document()` when you need Tree-sitter-like tooling data:

```rust
let document = grammar::parse_document(source)?;
let diagnostics = document.diagnostics();
let root = document.root();
```

The document owns source text, selected-tree facts, diagnostics, parse metadata,
and ambiguity summaries. Typed AST, typed CST, Tree-sitter-compatible output,
and JSON/CLI output should project from this document rather than reparsing.

## Tree And Node Traversal

If existing code expects Tree-sitter-shaped traversal, use the compatibility
adapter:

```rust
let mut parser = adze::ts_compat::Parser::new();
parser.set_language(language)?;
let tree = parser.parse(source, None)?;
let root = tree.root_node();
```

The selected-tree subset covers root/child traversal, named children, siblings,
parents, byte and point ranges, fields, identity, error/missing flags where
facts exist, S-expressions, and language metadata. See
[Tree-sitter Compatibility](./tree-sitter-compatibility.md) for the exact
subset and proof commands.

## Fields

Tree-sitter fields map to document edge metadata and generated language field
tables. In compatibility mode, use the familiar field APIs:

```rust
let child = node.child_by_field_name("left");
let field_id = language.field_id_for_name("left");
```

Native Adze code should prefer document or typed CST accessors when available,
because those preserve the one-document projection model.

## Node Identity

Tree-sitter splits visible node identity from grammar identity in alias-heavy
grammars. Adze follows the same distinction for the covered selected-tree
subset:

| Question | Compatibility API |
| --- | --- |
| What should users see? | `node.kind()`, `node.kind_id()` |
| What grammar symbol produced it? | `node.grammar_name()`, `node.grammar_id()` |
| Is it named/extra/error/missing? | `is_named()`, `is_extra()`, `is_error()`, `is_missing()` |

Alias-visible runtime identity is covered by selected-tree canaries. Full
alias-visible `node-types.json` parity remains a known gap.

## Node-types JSON

Tree-sitter tooling often consumes `node-types.json`. Adze exposes a generated
projection through the compatibility language metadata:

```rust
let json = language.node_types_json();
```

This is useful for tooling experiments, but it is still advisory. It should not
be treated as full Tree-sitter node-types parity until the support-tier row
lists alias-visible node-types proof.

## Queries

Adze has a Tree-sitter-style query subset:

```scheme
(identifier) @name
```

The current covered subset includes named node patterns, captures, child
sequences, child quantifiers, field constraints, anchors, source-aware text
predicates, byte-range filtering, and root-only matching.

See [Query Compatibility](./query-compatibility.md) for supported behavior,
source-aware requirements, and known gaps. Unsupported query features are
expected gaps, not hidden parity claims.

## Error And Missing Nodes

Tree-sitter users often inspect `ERROR`, missing nodes, and `has_error()`.
Adze exposes selected-tree error and missing facts through `ts_compat` where the
document has those facts, but diagnostics remain native document data:

```rust
let document = grammar::parse_document("1 +")?;
let diagnostics = document.diagnostics();
let tree = adze::ts_compat::Tree::from_document(language, &document);
```

Use diagnostics for user-facing messages. Use `ts_compat` error/missing flags
for adapter-level behavior and editor interop.

## GLR Ambiguity

Tree-sitter-compatible output exposes one selected tree. Native Adze output also
exposes ambiguity summaries:

```rust
let document = grammar::parse_document(ambiguous_source)?;
let ambiguities = document.ambiguities();
```

Do not expect `ts_compat` to expose every GLR forest alternative. Full forest
export is separate future work.

## Migration Checklist

1. Start with `grammar::parse()` if your old code only needed semantic values.
2. Use `parse_document()` if your old code walked a Tree-sitter tree.
3. Use `ts_compat::Parser` or `ts_compat::Tree::from_document(...)` only at
   ecosystem boundaries.
4. Replace ad hoc extraction with typed AST or typed CST projections where
   possible.
5. Check query usage against [Query Compatibility](./query-compatibility.md).
6. Check node metadata usage against
   [Tree-sitter Compatibility](./tree-sitter-compatibility.md).
7. Treat GLR ambiguity summaries as native Adze data, not Tree-sitter adapter
   data.

## Known Gaps

- Full Tree-sitter query parity is not claimed.
- Alias-visible `node-types.json` parity is not claimed.
- Imported grammar corpus parity is not claimed.
- Full forest export through compatibility APIs is not claimed.
- Stable incremental changed-range parity is not claimed.

Support-tier promotion requires repeatable proof commands in
[`SUPPORT_TIERS.md`](../status/SUPPORT_TIERS.md).

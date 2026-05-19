# How To: Query With Language Metadata

This guide covers Adze's metadata-aware query surface. It is an advanced,
advisory API for editor and tooling experiments, not the beginner parser path.

For application parsing, prefer the generated APIs:

```rust
let ast = grammar::parse(source)?;
let report = grammar::parse_document(source);
let document = report.document();
```

Use the query layer when you need Tree-sitter-like pattern matching over a
selected parse tree and the language metadata that names nodes, tokens, fields,
and captures.

## Contract

Adze's query compatibility is a documented subset:

```text
AdzeDocument is the native parse truth.
Tree-sitter compatibility exposes the selected tree.
Queries match selected-tree facts and language metadata.
```

Query matching must not invent node identity, field identity, alias identity, or
range information locally. Those facts belong to the document and language
schema.

See [Query Compatibility](../reference/query-compatibility.md) and
[ADZE-SPEC-0013](../specs/ADZE-SPEC-0013-query-compatibility.md) for the
current support matrix.

## When To Use It

Use metadata-aware queries when you are building:

- syntax highlighting experiments;
- editor or LSP integration prototypes;
- Tree-sitter migration tests;
- selected-tree compatibility canaries;
- fixture-level checks for captures, fields, predicates, or anchors.

Do not use this guide as a claim that Adze has full Tree-sitter query parity.
Unsupported features are expected gaps until the support-tier ledger promotes
them with proof commands.

## Source-Aware Matcher

The source-aware matcher uses source text plus `SymbolMetadata` to evaluate
literal token patterns and text predicates honestly.

```rust
use adze::parser_v4::ParseNode;
use adze::query::{compile_query, matcher_v2::QueryMatcher};
use adze_glr_core::SymbolMetadata;
use adze_ir::Grammar;

fn collect_names(
    grammar: &Grammar,
    metadata: &[SymbolMetadata],
    root: &ParseNode,
    source: &str,
) -> Result<usize, adze::query::QueryError> {
    let query = compile_query(
        r#"
        (function_declaration
          name: (identifier) @name)
        "#,
        grammar,
    )?;

    let matches = QueryMatcher::new(&query, source, metadata).matches(root);
    Ok(matches.iter().map(|m| m.captures.len()).sum())
}
```

The exact way you obtain `Grammar`, `SymbolMetadata`, and the selected
`ParseNode` depends on the generated language or fixture harness. Product APIs
should flow from `parse_document()` first; lower-level query canaries may build
these values directly.

## Source-Free Cursor

`QueryCursor` is useful when the query does not need source text. It can also
apply cursor-level restrictions:

```rust
use adze::parser_v4::ParseNode;
use adze::query::{compile_query, QueryCursor};
use adze_ir::Grammar;

fn names_in_range(
    grammar: &Grammar,
    root: &ParseNode,
    byte_start: usize,
    byte_end: usize,
) -> Result<usize, adze::query::QueryError> {
    let query = compile_query("(identifier) @name", grammar)?;

    let mut cursor = QueryCursor::new();
    cursor.set_byte_range(byte_start..byte_end);

    Ok(cursor.matches(&query, root).count())
}
```

Source-free matching intentionally fails closed for source-sensitive behavior.
Use the source-aware matcher for literal token patterns and text predicates.

## Supported Shapes

The documented subset currently includes:

- named node patterns;
- captures by capture index;
- ordered child sequences;
- child quantifiers: no suffix, `?`, `*`, and `+`;
- field constraints for the covered matcher fixtures;
- first-child, last-child, and adjacent-sibling anchors;
- source-aware predicates: `#eq?`, `#not-eq?`, `#match?`, `#not-match?`, and
  `#any-of?`;
- cursor byte-range filtering;
- root-only matching.

These are subset claims. Imported grammar differential parity and full
Tree-sitter query compatibility remain future work.

## Named Nodes And Anonymous Tokens

Metadata tells the matcher whether a symbol is a named node, an anonymous token,
extra trivia, or another language-specific symbol class. That distinction keeps
queries from matching too broadly.

```scheme
(function_declaration
  "function"
  name: (identifier) @name
  parameters: (parameter_list) @params
  body: (block) @body)
```

In this pattern:

- `function_declaration`, `identifier`, `parameter_list`, and `block` are named
  node patterns;
- `"function"` is a literal token pattern and needs source-aware matching;
- `name:`, `parameters:`, and `body:` are field constraints and depend on field
  metadata.

## Predicates

Text predicates need source text:

```scheme
((identifier) @name
 (#match? @name "^[a-z_][a-z0-9_]*$"))
```

The source-aware matcher can compare capture text to literals or to other
captures. If source text is unavailable, if a capture is missing, or if a node
range cannot be sliced from the source, predicate evaluation fails closed.

## Error Nodes

Querying error nodes is useful for diagnostics and editor experiments:

```scheme
[
  (function_declaration) @function
  (ERROR) @error
]
```

Native diagnostics still live on `AdzeDocument`. Tree-sitter-compatible selected
trees can expose `ERROR`, missing, and `has_error` facts, but those facts should
remain projections from document/parser data.

## Safe Metadata Access

When writing custom fixture code around metadata, use bounds-checked access:

```rust
use adze::parser_v4::ParseNode;
use adze_glr_core::SymbolMetadata;

fn node_flags(node: &ParseNode, metadata: &[SymbolMetadata]) -> (bool, bool) {
    metadata
        .get(node.symbol.0 as usize)
        .map(|m| (m.is_named, m.is_extra))
        .unwrap_or((true, false))
}
```

The fallback should be conservative. Missing metadata must not panic, and it
must not silently turn an advisory compatibility surface into a Stable claim.

## Testing Query Behavior

Use small fixture tests for individual features:

```rust
#[test]
fn query_byte_range_keeps_matching_capture() {
    let grammar = fixture_grammar();
    let root = fixture_tree();

    let query = adze::query::compile_query("(identifier) @name", &grammar)
        .expect("query should compile");

    let mut cursor = adze::query::QueryCursor::new();
    cursor.set_byte_range(3..8);

    let matches: Vec<_> = cursor.matches(&query, &root).collect();
    assert!(!matches.is_empty());
}
```

Prefer one fixture per behavior: fields, anchors, predicates, literal tokens,
byte ranges, and root-only matching. That makes support-tier promotion possible
without relying on broad, noisy compatibility claims.

## Troubleshooting

### No Matches

Check these in order:

- the query compiles against the same `Grammar` used to build the tree;
- node names and field names exist in language metadata;
- source-aware patterns are not being run through the source-free cursor;
- byte-range or root-only cursor options are not filtering out the match.

### Unexpected Matches

Inspect:

- named versus anonymous metadata;
- extra-node metadata;
- alias-visible identity versus grammar identity;
- field names attached to parent-child edges;
- whether the query is matching descendants because root-only mode is disabled.

### Predicate Failures

Text predicates fail closed when source text, captures, regexes, or byte ranges
are invalid. That is intentional; matching by node kind alone would overclaim
compatibility.

## Proof Commands

Representative query proof:

```bash
cargo test -p adze --features query --lib query -- --nocapture
cargo test -p adze --features query --lib query::matcher_v2 -- --nocapture
git diff --check
```

Future promotion requires differential fixtures for the supported subset:

```bash
cargo test -p adze --features "pure-rust,ts-compat,query" --test query_differential -- --nocapture
```

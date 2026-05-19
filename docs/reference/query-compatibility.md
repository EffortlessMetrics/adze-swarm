# Query Compatibility

Adze query compatibility is a documented subset of Tree-sitter query behavior.
It is useful for editor and tooling experiments, but it is not a full parity
claim yet.

The core rule is the same as the rest of Adze's compatibility story:

```text
AdzeDocument is the native parse truth.
Tree-sitter compatibility exposes the selected tree.
Queries match the selected tree and language metadata.
```

GLR ambiguity summaries stay native to `AdzeDocument`; query matching does not
iterate every GLR forest alternative by default.

## Current Status

| Feature | Status | Notes |
| --- | --- | --- |
| Named node patterns | Supported subset | Unknown node kinds should fail during compilation. |
| Captures | Supported subset | Captures are returned by capture index. |
| Child sequences | Supported subset | Ordered child patterns are matched. |
| Child quantifiers | Supported subset | `?`, `*`, and `+` have targeted backtracking canaries. |
| Anonymous token patterns | Source-aware only | Source-free matching fails closed. |
| Field constraints | Supported subset | Covered by matcher canaries for matching and missing/wrong-field rejection. |
| Anchors | Supported subset | Covered for first-child, last-child, and adjacent sibling constraints. |
| Alternation | Advisory/future | Requires parser and matcher matrix coverage. |
| Predicates | Source-aware subset | Text predicates need source text. |
| Directives | Parsed/advisory | No highlighting or injection semantics claim yet. |
| Byte-range filtering | Supported subset | Cursor filtering is covered by canaries. |
| Root-only matching | Supported subset | Cursor root-only matching is covered by canaries. |

## Supported Subset

### Named node captures

```scheme
(expression) @expr
```

Named node patterns are the base supported shape. Query compilation should reject
unknown node kinds instead of treating them as wildcard matches.

### Child sequences and quantifiers

```scheme
(call_expression
  (identifier) @callee
  (argument)* @arg)
```

The covered quantifiers are:

- no suffix: exactly one;
- `?`: zero or one;
- `*`: zero or more;
- `+`: one or more.

Quantified child patterns must leave enough input for later sibling patterns to
match.

### Cursor filtering

`QueryCursor` supports:

- byte-range filtering;
- clearing a byte range;
- root-only matching.

Byte-range filtering may keep a match when either the candidate node or its
captures overlap the configured range.

## Source-Aware Behavior

Text-sensitive features require source text:

```scheme
((identifier) @name
 (#eq? @name "main"))
```

Covered source-aware predicates:

| Predicate | Behavior |
| --- | --- |
| `#eq?` | capture text equals literal or another capture's text |
| `#not-eq?` | inverse of `#eq?`; missing captures fail closed |
| `#match?` | regex matches the full captured text |
| `#not-match?` | inverse of `#match?`; missing captures fail closed |
| `#any-of?` | capture text equals one listed literal |

When source text is unavailable, text predicates and literal token patterns must
fail closed. Missing captures, invalid regexes, and invalid source ranges also
fail closed.

## Field Constraints And Anchors

Field constraints and anchors are part of the covered subset:

```scheme
(binary_expression
  left: (expression) @lhs
  right: (expression) @rhs)
```

```scheme
(argument_list
  . (identifier) @first)
```

These are covered behavior, not a Stable support-tier claim. They still need
differential fixtures before broader Tree-sitter query parity promotion.

## Runnable Cookbook Example

The `query_highlighting` example demonstrates the current useful subset in one
small hand-built parse tree:

- capture-based highlight ranges;
- field constraints;
- first/adjacent/last anchor behavior;
- source-aware `#match?` predicates;
- byte-range filtering;
- root-only matching.

```bash
cargo run -p adze --features query --example query_highlighting
```

## Advisory Or Future

These features are not product claims yet:

- full Tree-sitter query parity;
- directive-driven highlighting and injection semantics;
- alternation parity across imported grammar fixtures;
- query matching over every GLR forest alternative;
- query JSON schema stability;
- broad imported grammar corpus compatibility.

## Proof Commands

Representative local proof:

```bash
cargo test -p adze --features query --lib query -- --nocapture
cargo test -p adze --features query --lib query::matcher_v2 -- --nocapture
```

Future promotion requires a supported-subset differential corpus:

```bash
cargo test -p adze --features "pure-rust,ts-compat,query" --test query_differential -- --nocapture
```

See [`ADZE-SPEC-0013`](../specs/ADZE-SPEC-0013-query-compatibility.md) for the
behavior contract and promotion rule.

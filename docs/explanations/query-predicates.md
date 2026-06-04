# Query Predicate Evaluation in Adze

## Overview

Adze supports a source-aware subset of Tree-sitter-style query predicates.
Predicates are useful when syntax alone is not enough, but they are not a full
Tree-sitter query parity claim.

The current product boundary is:

- query compatibility is a documented Stabilizing subset;
- text predicates require source text;
- source-free matching fails closed for text predicates and literal token
  patterns;
- directives, custom predicate APIs, and full Tree-sitter query parity remain
  future work.

For the authoritative compatibility boundary, see
[`query-compatibility.md`](../reference/query-compatibility.md) and
[`ADZE-SPEC-0013`](../specs/ADZE-SPEC-0013-query-compatibility.md).

## Supported Predicates

### 1. `#eq?` - Equality Predicate

Tests whether captured source text equals a literal or another capture's text.

```scm
; Match only 'if' keywords
(keyword) @kw (#eq? @kw "if")

; Match identifiers that are equal
(identifier) @first . (identifier) @second (#eq? @first @second)
```

### 2. `#not-eq?` - Inequality Predicate

Tests whether captured source text does not equal a literal or another
capture's text.

```scm
; Match keywords that are not 'if'
(keyword) @kw (#not-eq? @kw "if")
```

### 3. `#match?` - Regular Expression Predicate

Tests whether captured source text matches a regular expression.

```scm
; Match identifiers starting with underscore
(identifier) @private (#match? @private "^_")

; Match camelCase identifiers
(identifier) @camel (#match? @camel "^[a-z][a-zA-Z0-9]*$")
```

### 4. `#not-match?` - Negative Regular Expression

Tests whether captured source text does not match a regular expression.

```scm
; Match identifiers that don't start with underscore
(identifier) @public (#not-match? @public "^_")
```

### 5. `#any-of?` - Set Membership Predicate

Tests whether captured source text equals one listed literal.

```scm
; Match control flow keywords
(keyword) @control (#any-of? @control "if" "while" "for" "switch")

; Match visibility modifiers
(modifier) @vis (#any-of? @vis "public" "private" "protected")
```

## Implementation Details

### Architecture

The predicate evaluation path consists of:

1. **PredicateContext** (`query/predicate_eval.rs`): Handles predicate evaluation with source text
2. **Source-aware matcher** (`query/matcher_v2.rs`): Integrates predicate checking into pattern matching
3. **Regex Caching**: Compiled regexes are cached for performance

### Usage Example

This example is intentionally schematic. Real callers need a query grammar,
source text, selected tree, and symbol metadata from the selected-tree surface.

```rust
use adze::{
    query::{compile_query, matcher_v2::QueryMatcher},
};

let source = "if (condition) { return true; }";

// Query with predicates
let query = compile_query(r#"
    (keyword) @kw
    (#eq? @kw "if")
"#, &grammar)?;

// Match with predicate evaluation
let matcher = QueryMatcher::new(&query, source, &metadata);
let matches = matcher.matches(&tree);

// Only 'if' keywords are matched, not 'return'
```

### Performance Considerations

1. **Text Extraction**: Node text is extracted on-demand using byte offsets
2. **Regex Caching**: Regular expressions are compiled once and cached
3. **Early Termination**: Predicates are evaluated after structural matching

## Fail-Closed Behavior

Text-sensitive behavior must not invent matches from node kinds alone. Predicate
evaluation fails closed when:

- source text is unavailable;
- a referenced capture is missing;
- a captured byte range cannot be sliced from the source;
- a regular expression is invalid.

The source-free `QueryCursor` therefore returns no positive matches for text
predicates or anonymous literal token patterns. Use the source-aware matcher for
queries that need captured text.

## Tree-sitter Compatibility Boundary

The predicate surface is intended to behave like Tree-sitter for the documented
subset above. It is not a claim that every Tree-sitter predicate, directive, or
query feature is supported.

Current non-claims:

- no full Tree-sitter query parity;
- no directive-driven highlighting or injection semantics;
- no custom predicate API support contract;
- no query matching over every GLR forest alternative;
- no Stable support-tier promotion.

## Future Work

1. **Directive semantics**: Define consumer behavior for `#set!`, `#is?`, and
   related property directives before making product claims.
2. **Imported-grammar differential proof**: Compare selected query slices
   against upstream Tree-sitter behavior before broad parity promotion.
3. **Custom predicate API**: Design and test an extension contract before
   documenting one as supported.
4. **Streaming evaluation**: Evaluate predicates during matching where it has a
   measured product benefit.

## Testing

Representative tests and receipts:

- Unit tests for each predicate type (`predicate_eval.rs`)
- Source-free fail-closed tests (`query::cursor`)
- Source-aware matcher tests (`query::matcher_v2`)
- Supported-subset fixture canary (`runtime/tests/query_differential.rs`)
- Runnable receipt example (`runtime/examples/query_highlighting.rs`)

Run the focused receipts:

```bash
cargo test -p adze --features query --lib query -- --nocapture
cargo test -p adze --features query --example query_highlighting -- --nocapture
cargo run -p adze --features query --example query_highlighting
cargo test -p adze --features "pure-rust,ts-compat,query" --test query_differential -- --nocapture
```

## Compatibility Note

Treat predicate behavior as part of Adze's documented query subset. Differences
outside that subset are known compatibility gaps unless a support-tier row,
spec, and repeatable proof command say otherwise.

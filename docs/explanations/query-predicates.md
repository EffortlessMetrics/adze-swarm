# Query Predicate Evaluation in adze

## Overview

Adze supports a source-aware subset of Tree-sitter query predicates. Predicates
filter structural query matches by comparing captured source text against
literals, other captures, or regular expressions.

This is a documented subset, not a full Tree-sitter query parity claim. The
supported predicate behavior is governed by
[`ADZE-SPEC-0013`](../specs/ADZE-SPEC-0013-query-compatibility.md) and
[`query-compatibility.md`](../reference/query-compatibility.md), and its support
tier remains Stabilizing in [`SUPPORT_TIERS.md`](../status/SUPPORT_TIERS.md).

## Supported Source-Aware Predicates

The current supported predicate families are:

### 1. `#eq?` - Equality Predicate

Tests if a captured node's text equals a value or another capture.

```scm
; Match only 'if' keywords
(keyword) @kw (#eq? @kw "if")

; Match identifiers that are equal
(identifier) @first . (identifier) @second (#eq? @first @second)
```

### 2. `#not-eq?` - Inequality Predicate

Tests if a captured node's text is NOT equal to a value or another capture.

```scm
; Match keywords that are not 'if'
(keyword) @kw (#not-eq? @kw "if")
```

### 3. `#match?` - Regular Expression Predicate

Tests if a captured node's full text matches a regular expression.

```scm
; Match identifiers starting with underscore
(identifier) @private (#match? @private "^_")

; Match camelCase identifiers
(identifier) @camel (#match? @camel "^[a-z][a-zA-Z0-9]*$")
```

### 4. `#not-match?` - Negative Regular Expression

Tests if a captured node's full text does NOT match a regular expression.

```scm
; Match identifiers that don't start with underscore
(identifier) @public (#not-match? @public "^_")
```

### 5. `#any-of?` - Set Membership Predicate

Tests if a captured node's text is in a set of values.

```scm
; Match control flow keywords
(keyword) @control (#any-of? @control "if" "while" "for" "switch")

; Match visibility modifiers
(modifier) @vis (#any-of? @vis "public" "private" "protected")
```

## Implementation Details

### Architecture

The source-aware predicate evaluation path consists of:

1. **PredicateContext** (`query/predicate_eval.rs`): Handles predicate evaluation with source text
2. **Enhanced Matcher** (`query/matcher_v2.rs`): Integrates predicate checking into pattern matching
3. **Regex Caching**: Compiled regexes are cached for performance

### Source Requirements

Text predicates require source text and valid source ranges. They fail closed
when the matcher cannot honestly compare captured text.

In particular:

- missing captures fail closed;
- invalid regular expressions fail closed;
- invalid source ranges fail closed;
- source-free matching does not produce positive matches for text predicates;
- source-free matching does not produce positive matches for anonymous literal
  token patterns.

### Pseudocode Usage

This sketch is pseudocode. See
[`query-compatibility.md`](../reference/query-compatibility.md) for the current
public compile and source-aware matcher shape.

```text
use adze::{
    parser::ParseNode,
    query::{Query, matcher_v2::QueryMatcher},
};

let tree = parse_code(source);
let source = "if (condition) { return true; }";

// Query with predicates
let query = compile_query(r#"
    (keyword) @kw
    (#eq? @kw "if")
"#);

// Match with predicate evaluation
let matcher = QueryMatcher::new(&query, source);
let matches = matcher.matches(&tree);

// Only 'if' keywords are matched, not 'return'
```

### Performance Considerations

1. **Text Extraction**: Node text is extracted on-demand using byte offsets
2. **Regex Caching**: Regular expressions are compiled once and cached
3. **Early Termination**: Predicates are evaluated after structural matching

## Tree-sitter Compatibility Boundary

The current predicate claim is limited to the source-aware subset above:

- `#eq?`;
- `#not-eq?`;
- `#match?`;
- `#not-match?`;
- `#any-of?`.

Property directives and predicates such as `#set!`, `#is?`, and `#is-not?` may
be parsed or stored in lower-level query structures, but Adze does not currently
claim directive semantics, highlighting/injection behavior, custom predicate
APIs, or full Tree-sitter query parity.

## Future Work

1. **Directive Semantics**: Define and prove consumer behavior for directives
   such as `#set!`, `#is?`, and `#is-not?`.
2. **Custom Predicates**: Define a public API and proof surface before claiming
   extension support.
3. **Upstream Differential Fixtures**: Compare explicit grammar, source, and
   query slices against upstream Tree-sitter before broad parity promotion.
4. **Streaming Evaluation**: Evaluate predicates during matching for better
   performance without changing source-aware fail-closed behavior.

## Testing

The predicate subset is covered by targeted query tests and examples:

- query module tests, including source-aware and source-free fail-closed cases;
- matcher-v2 tests for source-aware matching;
- the `query_highlighting` example receipt;
- the supported-subset `query_differential` fixture canary.

Representative proof commands:

```bash
cargo test -p adze --features query --lib query -- --nocapture
cargo test -p adze --features query --lib query::matcher_v2 -- --nocapture
cargo test -p adze --features query --example query_highlighting -- --nocapture
cargo run -p adze --features query --example query_highlighting
cargo test -p adze --features "pure-rust,ts-compat,query" --test query_differential -- --nocapture
```

## Compatibility Note

Adze's query predicate support is useful for the documented subset, but it is
not a Stable support-tier claim and not a full Tree-sitter predicate
compatibility claim. Unsupported directive semantics, custom predicates, and
broader upstream parity remain future work unless promoted through the support
tier and proof process.

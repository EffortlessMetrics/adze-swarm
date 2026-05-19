# ADZE-SPEC-0013: Query compatibility

Status: accepted
Owner: runtime/query
Created: 2026-05-17
Linked proposal: ../proposals/ADZE-PROP-0003-glr-toolkit-productization.md
Linked ADRs:
Linked plan: ../../plans/glr-toolkit/productization-plan.md
Linked issues:
Linked PRs:
Support-tier impact:
- Defines the promotion path for the Tree-sitter query compatibility subset.
- Does not promote the query surface by itself.
Policy impact:
- Registers the query compatibility spec in `policy/doc-artifacts.toml`.

## Problem

Adze has query parsing and matching machinery, plus recent canaries for cursor
byte-range filtering, root-only matching, source-free predicate behavior, and
child quantifier backtracking. That is useful, but it is not the same as full
Tree-sitter query compatibility.

Users and agents need a contract that says which query features are supported,
which are advisory, which need source text, and which remain future work. Without
that contract, editor and highlighting integrations can accidentally rely on
unproven behavior.

## Behavior

### B1. Query compatibility is a subset claim

Adze must not claim full Tree-sitter query parity until every method family in
this spec has fixture-backed proof and support-tier promotion.

The query compatibility surface starts as a documented subset. Unsupported
features are expected gaps, not hidden failures.

### B2. Query matching uses selected-tree facts

Query matching targets the selected tree exposed by Adze's Tree-sitter
compatibility layer. GLR ambiguity summaries remain native document facts and
are not matched as multiple Tree-sitter trees by default.

Query behavior must not invent alias identity, field metadata, ranges, or node
types independently of the selected tree and language metadata.

### B3. Named node patterns are the base supported shape

Named node patterns are the base query shape:

```scheme
(expression) @expr
```

The query compiler must reject unknown node kinds instead of treating them as
wildcards.

### B4. Captures are stable by query capture index

Captures must preserve query capture indexes and be returned in capture-index
order when a match is materialized.

### B5. Child sequences and quantifiers are supported for the covered subset

Child sequences may match ordered child patterns. The covered quantifiers are:

| Quantifier | Meaning |
| --- | --- |
| none | exactly one |
| `?` | zero or one |
| `*` | zero or more |
| `+` | one or more |

Quantified child patterns must be able to backtrack far enough for later sibling
patterns to match.

### B6. Anonymous token patterns are source-aware

Anonymous token patterns such as `"+"` are supported only when the matcher has
enough source and metadata to compare token text honestly. Source-free matching
must fail closed rather than matching arbitrary nodes.

### B7. Field constraints are supported for the covered subset

Field constraints are part of the target subset:

```scheme
(binary_expression
  left: (expression) @left
  right: (expression) @right)
```

They are a covered subset claim when generated field metadata, matcher behavior,
and negative canaries agree. Broader generated-language parity remains subject
to the differential fixture matrix.

### B8. Anchors are supported for the covered subset

Tree-sitter anchors are supported for first-child, last-child, and adjacent
sibling constraints in the source-aware matcher. They remain scoped to the
covered matcher fixtures until imported grammar differential proof exists.

### B9. Predicates are source-aware

The supported predicate family is:

| Predicate | Target behavior |
| --- | --- |
| `#eq?` | compare capture text to literal or capture text to capture text |
| `#not-eq?` | inverse of `#eq?`; missing captures fail closed |
| `#match?` | full-string regex match against captured source text |
| `#not-match?` | inverse of `#match?`; missing captures fail closed |
| `#any-of?` | captured source text equals one listed literal |

When source text is unavailable, text predicates must fail closed. They must not
return matches on node kind alone. Missing captures, invalid regexes, and node
ranges that cannot be sliced from the source must also fail closed.

### B10. Directives are parsed before they are product claims

Property directives and predicates such as `#set!`, `#is?`, and `#is-not?` may
be parsed and stored, but they are not a product compatibility claim until a
consumer contract and fixture proof exist.

### B11. Cursor options affect matching

The query cursor must honor:

- byte-range filtering;
- clearing byte-range filtering;
- root-only matching.

Byte-range filtering may keep a match when either the candidate node or its
captures overlap the configured range.

### B12. Errors are explicit

Query compilation should reject unsupported syntax, unknown node kinds, unknown
fields, invalid captures, invalid predicates, and invalid regexes with structured
errors where available.

## Non-Goals

- No full Tree-sitter query parity claim.
- No directive semantics for highlighting or injection yet.
- No multiple-parse-alternative query matching over GLR forests.
- No imported grammar corpus parity guarantee.
- No query JSON schema stability.

## Required Evidence

- Parser canaries for node patterns, captures, fields, quantifiers, predicates,
  directives, and expected syntax errors.
- Matcher canaries for named nodes, child sequences, child quantifiers, field
  constraints, anchors, predicates, byte-range filtering, and root-only matching.
- Source-aware predicate canaries for literal and capture comparisons.
- Source-free negative canaries proving text predicates and literal token
  patterns fail closed.
- Differential fixtures for the supported subset against Tree-sitter behavior.

## Acceptance Examples

Supported named-node capture:

```scheme
(expression) @expr
```

Target field constraint:

```scheme
(binary_expression left: (expression) @lhs)
```

Source-aware text predicate:

```scheme
((identifier) @name
 (#match? @name "^[a-z_][a-z0-9_]*$"))
```

Expected gap:

```text
Do not claim directive-driven highlighting or injection compatibility until
consumer behavior and fixture proof exist.
```

## Test Mapping

- `runtime/src/query/parser.rs` parser unit tests;
- `runtime/src/query/matcher.rs` source-free matcher tests;
- `runtime/src/query/matcher_v2.rs` source-aware matcher tests;
- `runtime/src/query/cursor.rs` cursor option tests;
- future `query_differential` fixtures for supported-subset comparison.

## Implementation Mapping

Primary implementation surfaces:

- `runtime/src/query/`;
- `runtime/src/glr_query.rs`;
- `runtime/src/ts_compat/`;
- `docs/reference/query-compatibility.md`;
- `docs/testing/glr-fixture-taxonomy.md`.

## CI Proof

```bash
cargo test -p adze --features query --lib query -- --nocapture
cargo test -p adze --features query --lib query::matcher_v2 -- --nocapture
git diff --check
```

## Metrics And Promotion Rule

The query surface remains advisory until the supported subset has:

- documented feature status;
- parser and matcher canaries;
- source-aware and source-free predicate proof;
- Tree-sitter differential fixtures for supported syntax;
- support-tier rows with proof commands and explicit known gaps.

The planned differential proof command is:

```bash
cargo test -p adze --features "pure-rust,ts-compat,query" --test query_differential -- --nocapture
```

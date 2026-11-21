# Query Predicate Evaluation in Adze

Use text predicates when node kind alone is not enough to pick a match. For
example, to find only identifiers named `main`, capture the identifier and test
its source text:

```scheme
(identifier) @name
(#eq? @name "main")
```

Query matching targets the selected tree. It does not search every retained GLR
alternative. The [Query Compatibility reference](https://github.com/EffortlessMetrics/adze-swarm/blob/main/docs/reference/query-compatibility.md)
tracks the covered query subset and its limits.

## Text Predicates

- `#eq?` compares capture text with a literal or another capture's text.
- `#not-eq?` negates that comparison when both operands are available.
- `#match?` checks the entire captured text against a regular expression. Use
  `"_.*"` to match an identifier that starts with an underscore.
- `#not-match?` negates a valid full-text regex comparison.
- `#any-of?` checks whether the captured text equals one of the listed literals.

Use a source-aware matcher for these predicates. The source-free cursor does
not return a text-sensitive match based on node kind alone. Missing captures,
invalid regexes, and ranges that cannot be sliced from the source do not produce
matches, even for negated predicates.

Property forms such as `#set!`, `#is?`, and `#is-not?` can be parsed, but do not
supply highlighting, injection, or property-filtering behavior by themselves.
The query compiler rejects unknown predicate names.

## Try the Example

From the workspace root, run the source-aware query and highlighting example
against a small in-memory tree:

```bash
cargo run -p adze --features query --example query_highlighting
```

To check the fixture's capture ranges and source-free rejection:

```bash
cargo test -p adze --features query --example query_highlighting -- --nocapture
```

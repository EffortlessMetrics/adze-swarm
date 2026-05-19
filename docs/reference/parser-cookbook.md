# Parser Cookbook

Status: advisory recipes backed by `runtime/tests/cookbook.rs`.

This cookbook shows the canonical Adze product ladder:

```text
Rust grammar types
  -> generated parser
  -> grammar::parse(...)
  -> typed Rust value
  -> grammar::parse_document(...)
  -> diagnostics, typed CST, GLR ambiguity, Tree-sitter-compatible, and query projections
```

The recipes below are intentionally small. They are starting points for users
and regression fixtures for maintainers.

## Arithmetic With Precedence

Use precedence attributes when the typed AST should encode operator binding:

```rust
let expr = grammar::parse("1 + 2 * 3")?;
```

The tested recipe uses
`adze_example::fielded_precedence_typed_cst_contract::grammar` and proves that
`*` binds tighter than `+`.

## List Shape

Use `#[adze::repeat(non_empty = true)]` for repeated data. The current tested
recipe covers the single-item list shape:

```rust
let list = grammar::parse("alpha")?;
```

The tested recipe uses `adze_example::csv_list::grammar`. Comma-separated
multi-item delimiter parsing is a known follow-up and is not promoted by this
cookbook proof yet.

## Object-Like Records

Use explicit token leaves for punctuation and typed fields for semantic values:

```rust
let object = grammar::parse("{ answer: 42 }")?;
```

The tested recipe uses `adze_example::object_like_contract::grammar`.

## Fixed Tokens And Identifiers

Use text leaves for fixed punctuation or operators and regex leaves for
identifier-like values:

```rust
let object = grammar::parse("{ answer: 42 }")?;
```

The tested recipes use `adze_example::object_like_contract::grammar` and
`adze_example::fielded_precedence_typed_cst_contract::grammar`. Word-boundary
keyword grammar behavior is intentionally not promoted by this cookbook until
its happy path has a repeatable proof.

## Document Tooling Path

Use `parse_document()` when building tools instead of only typed semantic
values:

```rust
let document = grammar::parse_document("1+2*3")?;
let root = grammar::syntax::source_file(&document)?;
let ast: grammar::Expr = document.ast()?;
```

This keeps typed AST, typed CST, diagnostics, and serialized projections tied to
one `AdzeDocument`.

## Diagnostics

Bad input should produce structured parse facts when the parser can still build
a trustworthy document:

```rust
let document = grammar::parse_document("1 +")?;
let diagnostic = &document.diagnostics()[0];
assert_eq!(diagnostic.byte_span(), 3..3);
```

The stable contract is the structured span and expected-token data. Diagnostic
wording is still not frozen.

For a runnable example that prints source excerpts, multibyte spans, GLR bad
input diagnostics, and document JSON diagnostic bytes:

```bash
cargo run -p adze --features "pure-rust,glr,serialization" --example diagnostics_recovery
```

## GLR Ambiguity

Use GLR grammars when the language is genuinely ambiguous:

```rust
let document = grammar::parse_document("1 + 2 + 3")?;
let ambiguities = document.ambiguities();
```

Tree-sitter-compatible output exposes the selected tree. Native Adze APIs expose
ambiguity summaries separately.

For a runnable example that prints the selected typed AST, document root,
ambiguity summary, retained alternatives, and bad-input diagnostics:

```bash
cargo run -p adze --features "pure-rust,glr" --example glr_ambiguity
```

## Tree-Sitter-Compatible Selected Tree

Use the compatibility adapter when an editor or ecosystem tool expects
Tree-sitter-shaped traversal:

```rust
let mut parser = adze::ts_compat::Parser::new();
parser.set_language(adze_example::ts_langs::arithmetic())?;
let tree = parser.parse("1-2", None)?;
let root = tree.root_node();
```

This is a selected-tree subset. It does not claim full Tree-sitter parity.

## Query Captures

The supported query subset can match named nodes and captures:

```rust
let query = adze::query::compile_query("(root (identifier @name))", &grammar)?;
let matches = adze::query::matcher_v2::QueryMatcher::new(&query, source, &metadata)
    .matches(&tree);
```

See [Query Compatibility](./query-compatibility.md) for the supported subset and
known gaps.

For a runnable query/highlighting walkthrough:

```bash
cargo run -p adze --features query --example query_highlighting
```

## External Scanners

External scanners remain an advanced integration surface. Use
[External Scanners](../how-to/external-scanners.md) for the current guide and
do not treat scanner examples as Stable unless their support-tier row lists a
proof command.

## Proof

The tested cookbook proof is:

```bash
cargo test -p adze --features "pure-rust,glr,ts-compat" cookbook -- --nocapture
cargo run -p adze --features "pure-rust,glr" --example glr_ambiguity
cargo run -p adze --features query --example query_highlighting
cargo run -p adze --features "pure-rust,glr,serialization" --example diagnostics_recovery
```

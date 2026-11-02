# Parser Cookbook

Status: advisory recipes backed by `runtime/tests/cookbook.rs` and the
`examples/cookbook` crate (`cargo test -p adze-cookbook`, 15 tests by
default, 16 with `--features serialization`).

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

| Task | Recipe | Proof test |
| --- | --- | --- |
| Parse to a typed value with precedence | Arithmetic With Precedence | `cookbook_typed_parser_recipes_cover_common_grammar_shapes` |
| Tell clean input from recovered input | Clean Versus Recovered Documents | `cookbook_document_status_range_and_slice_recipe_distinguishes_clean_and_recovered_input` |
| Read fields without leaving the document | Typed CST Fields On The Document | `cookbook_typed_cst_fields_stay_on_document_recipe` |
| Serialize a document for tools | Document JSON Projection | `cookbook_document_json_projection_uses_pinned_schema_recipe` (serialization feature) |
| Inspect bad input | Diagnostics | `cookbook_document_and_diagnostic_recipes_cover_tooling_path` |
| Work with ambiguous grammars | GLR Ambiguity | `cookbook_glr_and_ts_compat_recipes_cover_selected_tree_and_ambiguity` |
| Match nodes with queries | Query Captures | `cookbook_query_capture_recipe_matches_identifier_nodes` |

Support tiers for every surface below live in
[SUPPORT_TIERS](../status/SUPPORT_TIERS.md); this page does not restate them.

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

Multi-item comma-separated lists of identifiers are additionally proved by
the `examples/cookbook` crate (`cookbook_csv_multi_item_row_is_exact`):
cells use the identifier token shape, which is inside the generated lexer's
fixed pattern subset (`\s+`, `\d+`, identifiers, text literals). Richer cell
patterns are a follow-up until the lexer subset grows.

## Object-Like Records

Use explicit token leaves for punctuation and typed fields for semantic values:

```rust
let object = grammar::parse("{ answer: 42 }")?;
```

The tested recipe uses `adze_example::object_like_contract::grammar`.

The `examples/cookbook` crate adds a fuller object recipe
(`cookbook_config_full_object_is_exact`): fixed-order named fields, a
repeated tag list, keyword leaves for keys and boolean values, and word-token
identifiers for values, with whitespace extras.

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

The `examples/cookbook` crate proves the working boundary
(`cookbook_config_keywords_survive_as_identifiers`): identifier values may
start with or contain keyword text (`hostname`, `truex`) because the longer
identifier match wins. Bare keyword text as an identifier value still lexes
as the keyword, so that shape stays a follow-up.

## Fallible Numeric Conversion

Use `try_transform` (never an unwrapping `transform`) for user-supplied
numbers. Overflow fails as a structured `ConversionFailed` diagnostic over
the token's byte range instead of panicking:

```rust
let errors = grammar::parse("256").expect_err("u8 overflow fails");
assert!(matches!(errors[0].reason, ParseErrorReason::ConversionFailed(_)));
assert_eq!(errors[0].byte_span(), 0..3);
```

The syntax outcome stays `Complete` while `document.ast()` reports the
conversion failure (`cookbook_fallible_document_keeps_syntax_outcome_but_ast_fails`).
The tested recipes live in the `examples/cookbook` crate.

## Document Tooling Path

Use `parse_document()` when building tools instead of only typed semantic
values:

```rust
let document = grammar::parse_document("1+2*3").expect("document parses recoverably");
let root = grammar::syntax::source_file(&document)?;
let ast: grammar::Expr = document.ast()?;
```

This keeps typed AST, typed CST, diagnostics, and serialized projections tied to
one `AdzeDocument`.

## Clean Versus Recovered Documents

`AdzeDocument::outcome()` reports the parse outcome explicitly:
`ParseOutcome::Complete`, `ParseOutcome::Recovered`, or
`ParseOutcome::Failed`. A recovered document keeps a trustworthy selected
tree alongside its diagnostics; a failed document has diagnostics but no
trustworthy tree content (only a synthetic error/missing placeholder), so
it must not be treated as a recovered parse.

```rust
use adze::document::ParseOutcome;

let clean = grammar::parse_document("1+2*3").expect("document parses recoverably");
assert_eq!(clean.outcome(), ParseOutcome::Complete);
assert!(clean.diagnostics().is_empty());
assert!(!clean.tree().has_errors());
assert_eq!(clean.metadata().error_count, 0);
assert_eq!(
    clean.source_slice(clean.tree().root().byte_range()),
    Some("1+2*3")
);

// "{ answer: 42, }" (object-like grammar) recovers with a real tree:
// assert_eq!(recovered.outcome(), ParseOutcome::Recovered);

// "1 +" in the fielded grammar retains only a synthetic missing node, so
// its outcome is Failed, not Recovered:
let failed = grammar::parse_document("1 +").expect("document parses recoverably");
assert_eq!(failed.outcome(), ParseOutcome::Failed);
assert!(!failed.diagnostics().is_empty());
```

The outcome is derived from the same diagnostics and tree facts, and the
JSON projection carries it as `metadata.outcome`. The tested recipe uses
`adze_example::fielded_precedence_typed_cst_contract::grammar` plus the
object-like grammar for the recovered case.

## Typed CST Fields On The Document

Generic field edges and the typed AST describe the same fields, so tools can
stay on the document instead of re-parsing:

```rust
let document = grammar::parse_document("1+2").expect("document parses recoverably");
let add = find_kind(document.tree().root(), "Expr_Add")?;
let left = add.edge_by_field_name("left")?.child()?;
assert_eq!(left.utf8_text().ok(), Some("1"));
```

The tested recipe compares the `left`/`right` field-edge text with the typed
`Expr::Add` value from `grammar::parse("1+2")`.

## Document JSON Projection

With the `serialization` feature, the document JSON carries the pinned
experimental schema id:

```rust
let json = document.to_json_value();
assert_eq!(json["schema"].as_str(), Some(adze::document::ADZE_DOCUMENT_JSON_SCHEMA));
```

The schema id is `adze.document.v1`. Treat the projection as experimental:
the id is pinned, the full payload shape is not a Stable contract.

## Diagnostics

Bad input should produce structured parse facts when the parser can still build
a trustworthy document:

```rust
let document = grammar::parse_document("1 +").expect("document parses recoverably");
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
let document = grammar::parse_document("1 + 2 + 3").expect("document parses recoverably");
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

The supported query subset matches the selected tree. Use it for editor and
tooling recipes that can live inside the documented subset:

```rust
let query = adze::query::compile_query("(root (identifier @name))", &grammar)?;
let matches = adze::query::matcher_v2::QueryMatcher::new(&query, source, &metadata)
    .matches(&tree);
```

Use the source-aware matcher when a query has text predicates or anonymous token
literals. Source-free matching intentionally fails closed for those patterns.
See [Query Compatibility](./query-compatibility.md) for the supported subset,
known gaps, and promotion rule.

For a runnable query/highlighting walkthrough:

```bash
cargo run -p adze --features query --example query_highlighting
```

## External Scanners

External scanners remain an advanced integration surface. Use
[External Scanners](../how-to/external-scanners.md) for the current guide and
do not treat scanner examples as Stable unless their support-tier row lists a
proof command.

## Not Yet Promoted

These shapes have no exact-value cookbook proof yet. They stay out of the
recipes above until a fixture proves them; the generated-grammar matrix in
[GENERATED_GRAMMAR_FEATURE_MATRIX](../product/GENERATED_GRAMMAR_FEATURE_MATRIX.md)
tracks the same gaps.

| Shape | Blocker |
| --- | --- |
| Comma-separated multi-item lists | Identifier-cell lists proved by `examples/cookbook`; richer cell patterns need lexer-subset growth (see below) |
| Keywords versus identifiers | Prefix/superstring identifiers proved by `examples/cookbook`; bare keyword text as an identifier value still lexes as the keyword |
| Optional and repeated fields | `Vec` repetition proved by `examples/cookbook`; skipping `Option<Struct>` mis-generates the continuation (`{ a: x, b: y }` fails where the trailing clause is present-tense fine), so `Option` stays unproved |
| Fallible numeric transforms | Proved by `examples/cookbook` (`cookbook_fallible_*`); matrix row now has a cookbook recipe |
| General regex leaf patterns | The generated `lex_fn` lowers only `\s+`, `\d+`, the identifier shape, and text literals; other patterns (e.g. `[^...]`, `#[ -~]*`) produce no lexer code and never match — no build error is emitted |
| Empty delimited lists | `non_empty = false` with `#[adze::delimited]` still requires an element (`tags: []` fails); use `non_empty = true` until proved |
| Comment extras | Blocked on the regex-subset row above; `#` comment patterns do not lex |
| Document-backed queries | The query recipe builds its tree by hand; queries over a parsed document are a named gap (#861/#878) |
| Performance claims | No cookbook speed claims; see [perf baselines](../perf/baselines.md) for how performance receipts are produced |

## Proof

The tested cookbook proof is:

```bash
cargo test -p adze --features "pure-rust,glr,ts-compat" --test cookbook cookbook -- --nocapture
cargo test -p adze --features "pure-rust,glr,ts-compat,serialization" --test cookbook cookbook -- --nocapture
cargo run -p adze --features "pure-rust,glr" --example glr_ambiguity
cargo run -p adze --features query --example query_highlighting
cargo run -p adze --features "pure-rust,glr,serialization" --example diagnostics_recovery
```

Expected counts: at least 6 tests pass without `serialization`, at least 7
with it (the JSON projection recipe is feature-gated).

The `examples/cookbook` crate proof is:

```bash
cargo test -p adze-cookbook -- --test-threads=1 --nocapture
cargo test -p adze-cookbook --features serialization -- --test-threads=1 --nocapture
```

Expected counts: 15 tests pass without `serialization`, 16 with it
(the document JSON projection recipe is feature-gated). The crate is a
standalone workspace-detached package (same pattern as `test-example/`),
so these commands run from the checkout without touching workspace
manifests.

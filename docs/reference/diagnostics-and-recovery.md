# Diagnostics And Recovery

> **Doc status:** User-experience hardening guide. Support status remains
> authoritative in [`SUPPORT_TIERS.md`](../status/SUPPORT_TIERS.md).

Adze exposes diagnostics through two user paths:

```text
grammar::parse(source)
  -> typed AST or parse errors

grammar::parse_document(source)
  -> AdzeDocument
    -> diagnostics
    -> selected syntax tree with error facts where available
    -> JSON diagnostics projection where enabled
```

Use `grammar::parse(source)` when the application only needs a typed Rust
value or a typed-parser error. Use `grammar::parse_document(source)` when a
tool needs source ranges, recovered document facts, JSON projection data, or a
Tree-sitter-shaped selected tree that can report aggregate error state.

## Supported Product Shape

The current stabilizing diagnostic contract is about structured facts, not
frozen prose:

| Need | Use | Claim boundary |
| --- | --- | --- |
| Typed parser error | `grammar::parse(source)` | Stable typed parser path; structured parse errors remain Stabilizing. |
| Source span | `ParseError::byte_span()` or document diagnostics | Covered for representative generated-parser bad inputs, EOF, multiline, and UTF-8 cases. |
| Human excerpt | `display_with_source(source)` | Useful rendering canaries exist, but exact text is not a frozen public format. |
| Tooling diagnostics | `grammar::parse_document(source)?.diagnostics()` | Document path is Stabilizing for generated-parser tooling facts, not Stable ABI. |
| Error tree facts | `document.tree().has_errors()` and compatibility flags | Error/missing projection is covered for selected-tree subsets where native facts exist. |
| JSON diagnostics | `document.to_json_value()` or CLI `diagnostics-json` | Advisory/stabilizing smoke surface; not a Stable schema guarantee. |
| GLR bad input | `parse_document()` on GLR grammars | Covered no-panic and diagnostic-document canaries exist for representative grammars. |

## Typed Parser Errors

The generated typed parser returns parse errors when it cannot produce a typed
AST:

```rust
let source = "1 + @";
let errors = grammar::parse(source).expect_err("bad input should fail");
let first = errors.first().expect("expected at least one parse error");

assert_eq!(first.byte_span(), 4..5);
eprintln!("{}", first.display_with_source(source));
```

Use this path for ordinary application parsing. It keeps the beginner path
small and avoids requiring document/projection APIs.

## Document Diagnostics

Tools should prefer `parse_document()` when they need diagnostics and parse
facts from the same parse:

```rust
let document = grammar::parse_document("1 +")?;

assert!(document.tree().has_errors());
assert!(!document.diagnostics().is_empty());

for diagnostic in document.diagnostics() {
    eprintln!("{}", diagnostic.display_with_source(document.source_text()));
}
```

The document is the parse truth. Diagnostics, selected tree error facts,
Tree-sitter-compatible error flags, JSON projection, and typed-AST projection
must all come from the same document facts instead of separate reparses.

## Walkthrough: One Error, Several Views

The useful mental model is to start from the smallest API that answers the
question:

1. Use `grammar::parse(source)` when a typed parser should either return a
   typed value or structured parse errors.
2. Use `grammar::parse_document(source)` when tooling needs recovered document
   facts from the same parse.
3. Compare `diagnostic.byte_span()` and `diagnostic.point_range` before
   rendering text. Rendered diagnostic wording is helpful, but it is not the
   stable contract.
4. Inspect `document.tree().has_errors()` and related nodes when a selected
   tree exists. Missing/error-node parity is still support-tier-bounded.
5. Use document JSON only as an experimental projection of the same diagnostic
   facts.

The runnable example checks this ladder for generated parser EOF, multibyte bad
tokens, GLR bad input, and document JSON bytes:

```bash
cargo run -p adze --features "pure-rust,glr,serialization" --example diagnostics_recovery
```

That command is a walkthrough receipt, not a Stable schema or wording promise.
The broader recovery matrix remains the stronger proof for representative bad
input classes:

```bash
cargo test -p adze --features "pure-rust,glr,serialization,ts-compat" --test recovery_matrix -- --nocapture
```

## UTF-8 And Multiline Spans

Diagnostic spans are byte ranges, but they must stay aligned to source text
facts. Current canaries cover representative UTF-8 and multiline cases:

```rust
let source = "1 + \u{03bb}";
let document = grammar::parse_document(source)?;
let diagnostic = &document.diagnostics()[0];

assert_eq!(document.source_slice(diagnostic.byte_span()), Some("\u{03bb}"));
```

Do not treat this as a claim that every possible invalid-span case is Stable.
Support-tier promotion still depends on the generated parser matrix and future
external-scanner recovery proof.

## GLR Recovery

GLR bad input should return structured errors or a diagnostic document instead
of panicking for the covered grammar classes:

```rust
let document = ambiguous_grammar::parse_document("1 + @")?;

assert!(document.tree().has_errors());
assert!(!document.diagnostics().is_empty());
```

GLR ambiguity summaries remain native `AdzeDocument` data. Tree-sitter-shaped
compatibility output exposes one selected tree and may project error state; it
does not expose every GLR forest alternative.

## JSON And CLI Projection

Document JSON and CLI diagnostic JSON are useful for tooling experiments:

```bash
adze parse --format diagnostics-json --grammar src/grammar.rs --input "1 +"
```

Those projection modes are not Stable schema claims. They are proof-backed
smoke surfaces that should remain tied to `parse_document()` and support-tier
rows.

## Runnable Example

The diagnostics walkthrough prints and asserts generated-parser EOF
diagnostics, multibyte bad-token diagnostics, GLR bad-input diagnostics, and
document JSON diagnostic bytes:

```bash
cargo run -p adze --features "pure-rust,glr,serialization" --example diagnostics_recovery
```

## Not Claimed

- Exact human diagnostic wording is not frozen.
- Stable JSON schema compatibility is not claimed.
- Full external-scanner recovery is not promoted.
- Full Tree-sitter error/missing-node parity is not promoted.
- Recovery from every malformed input shape is not promoted.
- `parse_document()` is not a second parser truth; it is the document path.

## Related Surfaces

- [Tree-sitter Compatibility](./tree-sitter-compatibility.md) documents the
  selected-tree error and missing-node subset.
- [Query Compatibility](./query-compatibility.md) documents query matching over
  the selected tree and support-tier-bounded source-aware predicates.
- [Parser Cookbook](./parser-cookbook.md) lists runnable diagnostics, GLR
  ambiguity, and query examples together.

## Proof Commands

Representative local proof:

```bash
cargo run -p adze --features "pure-rust,glr,serialization" --example diagnostics_recovery
cargo test -p adze --features "pure-rust,glr,serialization,ts-compat" --test recovery_matrix generated_object_like_bad_input_matrix_preserves_document_diagnostics_and_json -- --exact --nocapture
cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors generated_typed_parser_multibyte_bad_token_reports_utf8_byte_span -- --exact --nocapture
```

See [`SUPPORT_TIERS.md`](../status/SUPPORT_TIERS.md) for the full diagnostic
proof list and current promotion status.

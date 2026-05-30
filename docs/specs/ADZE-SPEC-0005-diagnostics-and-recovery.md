# ADZE-SPEC-0005: Diagnostics and recovery

Status: accepted
Owner: runtime/diagnostics
Created: 2026-05-13
Linked proposal: ../proposals/ADZE-PROP-0002-api-foundation.md
Linked ADRs: ../adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Linked plan: ../../plans/0.9.0/api-foundation.md
Linked issues:
Linked PRs:
Support-tier impact: ../status/SUPPORT_TIERS.md
Policy impact: ../../policy/doc-artifacts.toml

## Problem

Adze diagnostics have strong canaries, but diagnostics must become document
facts rather than text-only parse errors. Tooling users need byte ranges, point
ranges, expected/found symbols, recovery information, and related nodes.

## Behavior

### B1. Diagnostics are document facts

Every `AdzeDocument` exposes structured diagnostics:

```rust
doc.diagnostics()
```

### B2. Diagnostics are structured data

A diagnostic carries severity, code, message, byte range, point range, expected
symbols, found symbol, recovery information when available, and related
document nodes when available.

Rendered excerpts are views over the structured diagnostic, not the contract
itself.

### B3. Recovery creates tree facts

Missing nodes, error nodes, extras, aggregate `has_error`, and related recovery
metadata must be represented on document nodes or diagnostics so both native and
Tree-sitter-compatible views can answer error questions.

### B4. Bad input does not panic

Invalid tokens, unexpected EOF, multibyte spans, multiline bad input, and GLR
finish/lex errors must return diagnostics or hard failures without panicking.

### B5. AST projection may reject recovered documents by default

Typed AST lowering may fail on recovered syntax unless explicitly configured to
allow recovered or partial ASTs.

## Non-Goals

- No frozen diagnostic wording yet.
- No stable global diagnostic-code taxonomy yet.
- No full LSP mapping in this spec.
- No claim that every parser path has equal diagnostic richness yet.

## Required Evidence

- Byte span canaries.
- UTF-8/multibyte span canaries.
- Zero-width EOF span canaries.
- Expected token set canaries.
- Multiline excerpt canaries.
- No-panic bad input canaries.
- Document diagnostic lookup canaries.
- Related node canaries where document nodes exist.

## Acceptance Examples

```rust
let document = grammar::parse_document("1 +")?;
assert!(!document.diagnostics().is_empty());
assert!(document.diagnostics()[0].byte_range().start <= document.diagnostics()[0].byte_range().end);
```

```rust
let document = grammar::parse_document("é +")?;
assert!(document.diagnostics().iter().all(|d| d.point_range().start.column <= d.point_range().end.column));
```

## Test Mapping

- `runtime/tests/generated_parse_errors.rs`
- `runtime/tests/error_display_tests.rs`
- `runtime/tests/typed_cst_generated_document.rs`
- `runtime/tests/adze_document_alpha.rs`
- diagnostics microcrate tests

## Implementation Mapping

Primary implementation surfaces:

- `runtime/src/error*`
- `runtime/src/document/diagnostics*`
- generated parser error conversion
- GLR lex/finish error conversion
- CLI and JSON diagnostic projection later

## CI Proof

```bash
cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors -- --nocapture
cargo test -p adze --features "pure-rust,glr" --test error_display_tests -- --nocapture
cargo test -p adze-linecol-core
cargo test -p adze-runtime error_location -- --nocapture
git diff --check
```

## Metrics / Promotion Rule

Diagnostics move toward stable only when supported generated parser paths,
document projection, rendered display, and JSON/CLI views agree on byte spans,
point ranges, expected/found data, recovery state, and no-panic behavior.

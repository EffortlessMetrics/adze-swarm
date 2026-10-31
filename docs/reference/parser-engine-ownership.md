# Parser engine ownership and dispatch (#895)

Status: current-v2 ownership map for the generated production routes.
Program: #853. Dispatch: #896. Retirement: #897. Native GLR failure
facts: #1030. This document records ownership; it promotes no support
tier and makes no Stable GLR claim.

## The one dispatch

Generated `parse()` and `parse_document()` in `runtime/src/__private.rs`
both consume `engine_dispatch_for_table(language, &decoded_table)`
(`EngineDispatch`). The decoded table is authoritative; the generated
`HAS_CONFLICTS` hint never overrides it, so a hint/table mismatch
cannot silently select a different engine.

| Decoded table | Capability | Decision | Engine |
| --- | --- | --- | --- |
| Conflict-free / precedence-resolved | any | `DeterministicLr` | `pure_parser::Parser::parse_string` |
| Genuinely conflicted | generated `lex_fn`, streaming-routable | `StreamingGlr` | `adze_glr_core::Driver::parse_streaming` via `glr_streaming_runtime` |
| Genuinely conflicted | no generated `lex_fn` | `LegacyGlrWithoutLexFn` | `glr_parser::GLRParser` + `glr_lexer` (legacy, see below) |
| Genuinely conflicted | lexer not streaming-routable (`glr` on) | `UnsupportedConflictedTable` | none: explicit capability/init error |
| Any | `glr` feature off | `DeterministicLr` | `pure_parser` (the profile's sole selected engine; upfront capability decision, never a retry — profile-semantics compatibility is owned by #1072) |

Rules:

- Exactly one decision is made per invocation, from the table decoded
  in that invocation. No table is decoded twice on the generated path.
- The selected engine's result, recovery document, or error is
  returned as-is. No public path silently retries another engine after
  a selected-engine failure (#897).
- Streaming failures project the GLR driver's own structured
  `GlrUnexpectedToken` facts (found/expected symbol names, bounded
  spans) through `glr_error_to_parse_errors`. No LR observer reparse
  is retained (#1030 remainder, removed with this dispatch).
- Native expected-token metadata has exactly three GLR/table sources,
  never another engine: the driver's structured expected set; a
  recovery-inserted missing terminal's own symbol (its generated name
  is a token the GLR engine expected at that byte); and, for a failure
  at byte 0, the decoded table's initial-state ACTION row. The #1097
  trailing-delimiter presentation (delimiter span, only `end`
  expected) is applied to those native facts on both surfaces.
- A selected-tree conversion failure runs typed extraction exactly
  once and preserves its message/span; obtaining diagnostics never
  runs another engine's extraction or user converters (#1030/#864).

## Entry-point inventory

| Entry point | Owner/status | Notes |
| --- | --- | --- |
| `__private::parse` (generated `grammar::parse`) | canonical production | Pure-rust profile: shared dispatch above. Non-pure profile: tree-sitter C runtime, separate ABI surface. |
| `__private::parse_document` (generated `grammar::parse_document`) | canonical production | Same dispatch; projects the selected tree/diagnostics into `AdzeDocument`. No mandatory full-document allocation on the typed fast path. |
| `pure_parser::Parser::parse_string` | canonical deterministic LR engine | The `DeterministicLr` engine; also the tree-sitter-shaped `Parser`/`Language` compat surface in `tree_sitter` module aliases. |
| `adze_glr_core::Driver` (`parse_streaming`) | canonical GLR engine | The one streaming GLR engine for conflicted tables. |
| `glr_streaming_runtime` | canonical adapter | Lex-mode preparation, forest materialization, native error projection for the streaming driver. |
| `glr_parser::GLRParser` + `glr_lexer` | legacy adapter | Only the `LegacyGlrWithoutLexFn` route (synthetic languages without a generated lexer). Not a generated production route; retirement owner #897 once lex-fn-less callers are gone or explicitly gated. |
| `parser_v4` (`parser` alias) | adapter / document-tree substrate | Supplies `ParseNode` conversion and document materialization. Its conflict path already refuses ordered first-success fallback; it does not own generated dispatch. The "main GLR parser" wording for it is retired. |
| `parser_v2`, `parser_v3`, `glr` (legacy), `incremental`, `incremental_v2`, `incremental_v3` | legacy, `legacy-parsers` feature-gated | No production generated caller. Non-claim: not supported engines. |
| `pure_incremental`, `glr_incremental`, `unified_parser` | experimental adapters | Incremental/reparse helpers; not selected by the generated dispatch. |
| `parser_selection` / `adze_bdd_governance_core::ParserBackend` | governance/reporting shim | Describes feature profiles for BDD governance. It no longer selects the generated engine; the table-derived dispatch above does. |
| `runtime2/` | experimental, dev-only | Proving ground. Non-claim: not a production runtime, not promoted by this map. |
| Tree-sitter C (`__private::parse`, non-`pure-rust`) | compatibility surface | Selected by profile, not by table dispatch; unchanged. |
| Test-only harnesses (`runtime-e2e` helpers such as `lex_with_language_fn`, `glr-test-support`) | test-only | Characterization seams; production routes must not call them. |

## Remaining retirement debt (#897)

- `LegacyGlrWithoutLexFn` (`glr_parser`/`glr_lexer`) remains for
  lex-fn-less synthetic languages. It is explicitly dispatched, never
  a silent fallback, but it is a second GLR implementation: #897 owns
  deleting or feature-gating it when its callers are migrated.
- `parser_v4` remains as the document/`ParseNode` substrate. Deleting
  it is not dispatch work; #897 owns any further consolidation.
- Legacy feature-gated parsers and `runtime2` keep their non-claims
  above; nothing here promotes them.

## Proof

```bash
cargo test -p adze --features "pure-rust,glr" parser_selection -- --nocapture
cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors -- --nocapture
cargo test -p adze --features "pure-rust,glr,runtime-e2e,serialization,query" --test gate_a_failure_stack -- --nocapture
```

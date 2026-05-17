# Incremental Parsing

> **Doc status:** Incremental parsing is Experimental. The accepted contract is
> a document lifecycle contract, not a stable performance or subtree-reuse
> promise.

Incremental parsing in Adze must attach to the canonical document model:

```text
old source
  -> grammar::parse_document(old_source)
  -> AdzeDocument snapshot
  -> edit description
  -> new AdzeDocument snapshot
  -> changed ranges / fallback metadata
```

The key rule is the same as the rest of the parser product:

```text
AdzeDocument is the one parse truth.
```

Incremental parsing must not become a separate runtime path with a separate
tree, separate diagnostics, or hidden reuse claims.

## Current Contract

The accepted lifecycle contract is:

- a parse returns an immutable document snapshot;
- edits produce a new document snapshot;
- document-local node IDs are not stable across documents;
- cross-document identity requires explicit reuse, changed-range, or provenance
  metadata;
- fallback to a full reparse must be visible in parse metadata.

The target shape is not a stable API promise yet, but it describes the behavior
incremental work must converge on:

```rust,ignore
let old = grammar::parse_document(old_source)?;
let newer = old.reparse(new_source, &[edit], ParseOptions::default())?;

assert!(newer.metadata().incremental_requested);

if !newer.metadata().incremental_used {
    eprintln!("full reparse fallback: {:?}", newer.metadata().fallback_reason);
}
```

This is a product honesty requirement. It is better to report a full reparse
fallback than to silently claim incremental reuse that did not happen.

## Non-Goals

The current contract does not promise:

- stable incremental performance;
- a guaranteed reuse percentage;
- cross-document stable node handles;
- full Tree-sitter incremental API parity;
- full GLR forest reuse;
- release-blocking incremental support.

## Recommended User Path

Use the stable typed path when you only need values:

```rust
let ast = grammar::parse(source)?;
```

Use the document path when you are building editor, CLI, or language tooling:

```rust
let document = grammar::parse_document(source)?;
let diagnostics = document.diagnostics();
let tree = document.tree();
```

Incremental behavior should layer on top of that document lifecycle. Tooling
should always be prepared for a full-reparse fallback until the support tier
changes.

## Metadata First

Every incremental result needs metadata that answers:

| Question | Why it matters |
|---|---|
| Was incremental parsing requested? | Distinguishes ordinary parses from editor-driven reparses. |
| Was incremental parsing actually used? | Prevents silent reuse claims. |
| Why did it fall back? | Gives users and agents a debuggable receipt. |
| What changed? | Lets tooling update diagnostics, highlights, and caches conservatively. |

If the runtime cannot safely prove reuse, it should return a correct document
with fallback metadata.

## Changed Ranges

Changed ranges are conservative receipts. They should be safe for downstream
tooling even before Adze claims stable incremental performance.

For a full-reparse fallback, a conservative implementation may report:

- the whole document;
- the text-diff range;
- or another explicit fallback range that does not hide uncertainty.

Do not treat unchanged ranges as proof of stable node identity unless the API
explicitly provides provenance metadata.

## GLR And Ambiguity

GLR incremental work must preserve the same native truth model:

- the new document has one selected tree;
- ambiguity summaries remain document facts;
- Tree-sitter-compatible output exposes the selected tree only;
- raw forest reuse/export stays experimental unless support tiers change.

## Proof Before Promotion

Incremental parsing should remain Experimental until these receipts exist for
supported generated parser paths:

- full-reparse fallback metadata canary;
- changed-range canary;
- unsupported incremental path fails honestly or reports fallback;
- reparse result remains an `AdzeDocument`;
- diagnostics and ambiguity summaries agree with the new document;
- benchmarks are tied to fixtures and do not replace correctness proof.

Focused proof belongs in the incremental lifecycle lane:

```bash
cargo test -p adze --features incremental_glr --test glr_incremental_comprehensive -- --nocapture
git diff --check
```

Use the supported gate before submitting product changes:

```bash
just ci-supported
```

## Known Limits

- Incremental parsing is not a Stable product claim.
- Performance numbers require benchmark receipts.
- Full Tree-sitter incremental API parity is not claimed.
- Full GLR forest reuse is not claimed.
- Node IDs remain document-local.

## Next Steps

- [Parser Generation](parser-generation.md)
- [Error Recovery](error-recovery.md)
- [Performance](performance.md)
- [Known Limitations](../reference/known-limitations.md)
- `docs/specs/ADZE-SPEC-0009-incremental-document-lifecycle.md`
- `docs/status/SUPPORT_TIERS.md`

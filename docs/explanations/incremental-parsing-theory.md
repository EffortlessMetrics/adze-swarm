# Incremental Parsing Theory

Incremental parsing is the editor and tooling problem of preserving useful parse
facts across source edits. In Adze, that problem is framed through
`AdzeDocument`, not through a separate parser truth.

The accepted lifecycle contract is:

```text
source snapshot
  -> parse_document()
  -> immutable AdzeDocument
  -> edit request
  -> new AdzeDocument
  -> changed ranges and metadata explain what happened
```

See `docs/specs/ADZE-SPEC-0009-incremental-document-lifecycle.md` for the
normative behavior contract.

## Current Status

Incremental lifecycle support is experimental. The product contract is accepted,
but Adze does not currently promise stable reuse percentages, stable
cross-document node handles, or incremental parse speedups.

The important guarantee is honesty:

```text
if an incremental request falls back to a full reparse,
the new document metadata must say so.
```

That lets editor and tooling integrations build against the lifecycle without
assuming invisible reuse that may not have happened.

## Document Snapshots

`AdzeDocument` represents one source snapshot and one parse result. Document node
IDs are local to that document. They are useful for navigating one parse product,
but they are not stable identities across edits.

The target shape is:

```rust
let old = grammar::parse_document(old_source).document();
let newer = old.reparse(new_source, &[edit], ParseOptions::default())?;
let changed = old.changed_ranges(&newer);
```

The exact API remains experimental. The concept is stable enough for design
work: edits produce a new document, and any reuse or fallback behavior is exposed
as document metadata rather than hidden parser state.

## Changed Ranges

Changed ranges are the bridge between two document snapshots. They tell an
editor, indexer, or cache which source region should be treated as changed.

Early implementations may be conservative:

```text
full reparse fallback:
  changed range can be the whole document

simple text edit:
  changed range can be the edited byte/point span, widened as needed
```

Conservative changed ranges are acceptable. Silent false precision is not.

## Reuse Metadata

Future incremental implementations may reuse tokens, subtrees, GLR states, or
projection caches. That reuse must be observable through explicit metadata or
proof APIs before documentation can claim it.

Adze should not document claims like:

```text
95 percent reuse
very low latency edits
Tree-sitter-compatible incremental performance
stable node identity across edits
```

unless those claims have repeatable benchmark fixtures, CI receipts, and support
tier entries.

## GLR Considerations

GLR parsing adds an important constraint: ambiguity is part of the parse truth.
An incremental implementation cannot preserve only the selected tree if the
native document also needs ambiguity summaries and diagnostics.

An incremental GLR path must preserve or recompute:

- selected tree facts;
- ambiguity summaries;
- diagnostics and recovery facts;
- byte and point ranges;
- field edges and parent links;
- metadata explaining whether reuse or full fallback happened.

If preserving these facts is uncertain, falling back to a full reparse is the
correct behavior.

## Projection Considerations

Incremental parsing does not create separate parse products. Typed AST, typed
CST, diagnostics, Tree-sitter-compatible output, JSON, and query-facing views
remain projections from the new document.

That means caches for projections can be optimized later, but they must not
become independent sources of truth.

## Proof Needed Before Promotion

Incremental lifecycle support stays experimental until the repo has repeatable
evidence for:

- full-reparse fallback metadata;
- changed-range behavior;
- no silent reuse claims when fallback happens;
- `AdzeDocument` output after edit requests;
- diagnostics and ambiguity summaries after edit requests;
- projection consistency between fresh parse and incremental request paths;
- benchmark fixtures tied to the same correctness cases.

Useful proof commands should be added as those tests exist. Today the baseline
receipt for this explanation is only documentation hygiene:

```bash
git diff --check
```

## User Guidance

Use `grammar::parse()` for typed Rust values.

Use `grammar::parse_document()` when tooling needs source ranges, diagnostics,
document traversal, ambiguity summaries, Tree-sitter-compatible selected-tree
output, JSON, or future incremental lifecycle metadata.

Do not design integrations around stable cross-document node IDs or guaranteed
reuse percentages yet. Design them around immutable document snapshots,
changed ranges, and explicit fallback metadata.

# How To: Work With Incremental Document Lifecycles

Incremental parsing in Adze is experimental. Treat it as a document lifecycle
contract, not as a stable subtree-reuse or performance feature.

The current rule is:

```text
old source -> old AdzeDocument
edit + new source -> new AdzeDocument
metadata says whether incremental reuse was requested
metadata says when Adze fell back to a full reparse
changed ranges are conservative until narrower reuse proof exists
```

Use [ADZE-SPEC-0009](../specs/ADZE-SPEC-0009-incremental-document-lifecycle.md)
as the source of truth.

## Before You Start

Use the generated parser APIs first:

```rust
let ast = grammar::parse(source)?;
let report = grammar::parse_document(source);
let document = report.document();
```

Reach for incremental lifecycle APIs only when you are building an editor,
language server, watch-mode tool, or diagnostic pipeline that needs to compare
successive document snapshots.

## The Supported Mental Model

### Documents are snapshots

An `AdzeDocument` represents one source snapshot. Do not mutate it in place.

### Edits create another snapshot

An edit request produces a new document. The new document owns its source,
selected tree, diagnostics, metadata, ambiguity summaries, and projections.

### Fallback is acceptable

If Adze cannot safely reuse previous structure, it may fully reparse the source.
That is allowed only when the new document metadata records the fallback.

### Node IDs are local

Document node IDs are local to one document. Do not compare node IDs across
snapshots unless a future provenance or reuse API explicitly says that is safe.

## Editor Loop

A conservative editor integration should follow this shape:

```text
open file:
  source = file contents
  document = grammar::parse_document(source).document

on edit:
  next_source = apply edit to source
  next_document = request document for next_source
  changed = conservative changed ranges between document and next_document
  invalidate editor data for changed ranges
  inspect next_document.metadata for fallback
  replace source and document
```

This loop remains useful even when every edit falls back to a full reparse,
because consumers still receive a coherent document and an honest invalidation
range.

## Handling Fallback

Code that uses the lifecycle should branch on metadata, not on assumptions about
reuse:

```text
if metadata.full_reparse_fallback:
  invalidate broad ranges
  avoid reporting reuse statistics
else if metadata.incremental_used:
  consume narrower changed ranges if the support tier allows it
else:
  treat the parse as a normal fresh parse
```

Do not log fallback as an error by itself. It is the expected conservative path
until the active reuse contract is promoted.

## Changed Ranges

Changed ranges should be treated as safe invalidation ranges:

```text
if source changed:
  changed ranges must identify data to invalidate

if full reparse fallback occurred:
  whole-document changed range is valid

if narrower ranges are reported:
  they need canary-backed proof for the supported parser path
```

Avoid using changed ranges as proof of performance or structural reuse.

## Diagnostics

Diagnostics always belong to the new document snapshot:

```text
edit produces invalid source
new document records diagnostics
diagnostic ranges refer to new source
Tree-sitter-compatible error flags project from the same document facts
```

This keeps editor integrations honest: the current diagnostics, selected tree,
JSON projection, and compatibility view all refer to the same source snapshot.

## GLR Ambiguity

GLR ambiguity remains native Adze data. Incremental parsing should not create a
second ambiguity model.

When a reparse touches ambiguous input:

```text
selected tree belongs to the new document
ambiguity summaries belong to the new document
Tree-sitter-compatible output sees only the selected tree
```

Raw forest reuse and query matching across every GLR alternative remain future
work.

## External Scanners

External scanners are an advanced surface. If a parser path uses scanner state,
incremental lifecycle metadata must still be honest:

```text
scanner state reused safely -> metadata may say reuse occurred
scanner state cannot be trusted -> full reparse fallback is acceptable
```

Do not claim Tree-sitter external scanner incremental parity unless the support
tier lists a proof command.

## Performance

Do not assume speedups:

```text
small edit -> may still full reparse
large edit -> may full reparse
ambiguous grammar -> may full reparse
external scanner state -> may full reparse
```

Performance claims need benchmark fixtures, a regression receipt, and a
support-tier row. Until then, the incremental lifecycle is about correct,
observable state transitions.

## Troubleshooting

### Every edit falls back to a full reparse

That is allowed today. Confirm that metadata records the fallback and that the
tool invalidates a safe range.

### Changed ranges are broader than expected

Broad ranges are conservative and valid. Do not narrow them locally unless the
parser path provides proof-backed ranges.

### Node IDs changed after a reparse

That is expected. Node IDs are document-local.

### Diagnostics refer to old source

That is a bug. Diagnostics after an edit must refer to the new document source.

## What Not To Do

Do not:

- instantiate low-level GLR incremental parsers in user docs as the default path;
- promise reuse metrics or speedup ratios;
- compare node IDs across document snapshots;
- hide full-reparse fallback;
- claim Tree-sitter incremental parity;
- mix incremental parser internals with the beginner `grammar::parse()` path.

## Proof Commands

Representative experimental proof:

```bash
cargo test -p adze --features incremental_glr --test glr_incremental_comprehensive -- --nocapture
cargo test -p adze --features incremental_glr reparse_fallback_metadata -- --nocapture
git diff --check
```

Promotion requires document-level canaries for:

- fallback metadata;
- conservative changed ranges;
- diagnostics after edits;
- no hidden reuse claims;
- document-local node identity;
- performance receipts if speed is claimed.

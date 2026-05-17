# Incremental Document Lifecycle Examples

Incremental parsing is an experimental document-lifecycle surface. Adze does
not currently make a Stable claim about subtree reuse, edit-time speedups, or
cross-document node identity.

The product contract is:

```text
parse_document(source) returns an immutable document snapshot.
edits produce a new document snapshot.
fallback to full reparse must be visible in metadata.
changed ranges are conservative until reuse is proven.
```

See [ADZE-SPEC-0009](../specs/ADZE-SPEC-0009-incremental-document-lifecycle.md)
for the source-of-truth behavior contract.

## Current Status

Use incremental APIs as experimental tooling infrastructure:

- documents are immutable source snapshots;
- node IDs are document-local and are not stable across reparses;
- full-reparse fallback is allowed;
- fallback must be recorded instead of hidden;
- changed ranges may conservatively cover the whole document;
- no support-tier promotion happens without fallback, changed-range, and reuse
  canaries.

Do not treat legacy direct parser examples or reuse counters as a product API.

## Basic Lifecycle

The target lifecycle is document-centered:

```text
old_source -> grammar::parse_document(old_source) -> old AdzeDocument
edit list + new_source -> reparse request -> new AdzeDocument
```

The important behavior is not that this reuses nodes. The important behavior is
that the resulting parse product is still an `AdzeDocument`, with metadata that
describes whether incremental reuse was requested and whether the implementation
fell back to a full reparse.

## Fallback Metadata

When Adze cannot reuse prior structure, the result should say so:

```text
metadata.incremental_requested == true
metadata.full_reparse_fallback == true
metadata.fallback_reason == "full-reparse-only" or equivalent structured reason
```

This makes editor and tooling integrations honest. A user can wire the lifecycle
without being promised stable reuse or speedups that are not yet proven.

## Changed Ranges

Changed ranges are a contract between document snapshots:

```text
old document + new document -> changed ranges
changed ranges must not be empty when source changed
```

When full-reparse fallback is used, changed ranges may be conservative:

```text
small source edit -> whole-document changed range is allowed
small source edit -> narrower range is allowed only when proven by canaries
```

Consumers should treat the range as a safe invalidation boundary, not as proof
that Adze reused every other node.

## Diagnostics Still Come From The New Document

Bad input should return a diagnostic document when a trustworthy document can be
constructed:

```text
old valid document
edit produces invalid source
new document has diagnostics
new document metadata records that incremental parsing was requested
```

The diagnostics, selected tree, ambiguity summaries, JSON, and compatibility
views all project from the new document snapshot.

## Node Identity

Do not compare document-local node IDs across snapshots:

```text
old node id 42 belongs only to old document
new node id 42, if present, belongs only to new document
cross-document identity requires explicit reuse or provenance metadata
```

Cross-document identity requires explicit reuse, provenance, or changed-range
metadata. Until that proof exists, assume node IDs are local to one document.

## Editor Loop Shape

A conservative editor integration can use this shape:

```text
open document stores:
  source text
  current AdzeDocument

on edit:
  apply edit to source text
  request a new document snapshot
  read changed ranges from old/new documents
  invalidate editor ranges
  replace current document
  inspect metadata for full-reparse fallback
```

This loop is useful even when the implementation falls back to a full reparse,
because the caller receives a fresh document, honest metadata, and conservative
changed ranges.

## What Not To Claim

Do not claim:

- stable incremental parsing;
- guaranteed subtree reuse;
- cross-document stable node IDs;
- fixed speedup ratios;
- raw GLR forest reuse;
- editor-grade changed ranges for every grammar;
- Tree-sitter incremental parity.

Those claims need dedicated support-tier rows and repeatable proof.

## Proof Commands

Representative proof for the experimental lifecycle:

```bash
cargo test -p adze --features incremental_glr --test glr_incremental_comprehensive -- --nocapture
git diff --check
```

Promotion requires additional document-level canaries for:

- full-reparse fallback metadata;
- conservative changed ranges;
- diagnostic document behavior after edits;
- no hidden reuse claims;
- document-local node identity.

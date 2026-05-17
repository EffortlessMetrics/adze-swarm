# ADZE-SPEC-0009: Incremental document lifecycle

Status: accepted
Owner: runtime/incremental
Created: 2026-05-13
Linked proposal: ../proposals/ADZE-PROP-0002-api-foundation.md
Linked ADRs: ../adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Linked plan: ../../plans/0.9.0/api-foundation.md
Linked issues:
Linked PRs:
Support-tier impact: ../status/SUPPORT_TIERS.md
Policy impact: ../../policy/doc-artifacts.toml

## Problem

Modern parser tooling maintains useful structure over changing source. Adze's
incremental story must attach to the canonical document model instead of a
separate experimental runtime path.

This spec is accepted as the behavior contract. Acceptance does not promote
incremental parsing support tiers or claim stable reuse/performance behavior.

## Behavior

### B1. Documents are immutable snapshots

A parse returns an immutable document for one source snapshot and grammar
fingerprint.

### B2. Edits produce a new document

The target lifecycle is:

```rust
let old = grammar::parse_document(old_source).document();
let new = old.reparse(new_source, edits, options)?;
let changed = old.changed_ranges(&new);
```

### B3. Node IDs are not stable across documents

Cross-document identity is exposed only through explicit reuse, changed-range,
or provenance metadata.

### B4. Fallback is visible

If incremental parsing falls back to a full reparse, metadata must say so.

## Non-Goals

- No stable incremental performance guarantee.
- No cross-document stable node handles.
- No guaranteed reuse percentage.
- No requirement that incremental parsing lands in 0.9.

## Required Evidence

- Full-reparse fallback metadata canary.
- Changed-range canary.
- Unsupported incremental path does not silently claim reuse.
- Reparse result remains an `AdzeDocument`.

## Acceptance Examples

```rust
let newer = old.reparse(new_source, &[edit], ParseOptions::default())?;
assert!(old.changed_ranges(&newer).next().is_some());
assert!(newer.metadata().incremental_requested());
```

## Test Mapping

- future incremental document lifecycle tests;
- existing incremental GLR tests where applicable.

## Implementation Mapping

Primary implementation surfaces:

- incremental parser runtime;
- document metadata;
- changed-range calculation;
- edit model.

## CI Proof

```bash
cargo test -p adze --features incremental_glr --test glr_incremental_comprehensive -- --nocapture
git diff --check
```

## Metrics / Promotion Rule

Incremental document lifecycle remains experimental until fallback, changed
range, metadata honesty, and reuse canaries exist for supported parser paths.

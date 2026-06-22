# ADZE-SPEC-0017: unsafe invariant contract

Status: accepted
Owner: runtime/safety
Created: 2026-06-22
Linked proposal: ../proposals/ADZE-PROP-0001-0.9-contract-convergence.md
Linked ADRs: ../adr/0001-arena-allocator-for-parse-trees.md
Linked plan: ../../plans/0.9.0/implementation-plan.md
Linked issues: #774
Support-tier impact: ../status/SUPPORT_TIERS.md
Policy impact: ../../docs/ci/unsafe-review.md, ../../policy/clippy-lints.toml

## Problem

`runtime/src/` contains 238 `unsafe` occurrences across 27 files. The workspace
lint `unsafe_op_in_unsafe_fn = "deny"` catches unsafe-inside-unsafe-fn but does
not audit raw `unsafe { }` blocks or require safety documentation.

Several `unsafe` blocks carry `TODO(safety)` markers indicating documented but
unverified invariants. The `ffi.rs:132` double-free TODO was resolved in #819
(the existing guard was correct); this spec governs the broader contract.

## Behavior

### B1. Every unsafe block must have a SAFETY comment

Every `unsafe { }` block and `unsafe fn` in the supported crates (`runtime`,
`macro`, `tool`, `common`, `ir`, `glr-core`, `tablegen`) must carry a `// SAFETY:`
comment documenting why the operation is sound. The comment must state the
invariant that makes the operation safe (e.g. "pointer was created by
`Box::into_raw`", "index is bounds-checked above").

### B2. TODO(safety) markers must be resolved or tracked

Any `TODO(safety)` marker in `unsafe` code must either:
1. Be resolved (the invariant is verified, the TODO removed), OR
2. Be tracked as a GitHub issue with the `security` label and a `review_after`
   date in the issue body.

Untracked `TODO(safety)` markers are a spec violation.

### B3. No new unsafe without a witness

New `unsafe` blocks added to supported crates must include a test that exercises
the unsafe path (a "witness"). The witness can be a unit test, integration test,
or Miri run. The `docs/ci/unsafe-review.md` policy defines the witness card
format.

### B4. Non-Goals

This spec does not:
- Require elimination of all `unsafe` (some is necessary for FFI, arena
  allocation, performance).
- Require Miri on every PR (Miri is a nightly/advisory witness, not a gate).
- Change the `unsafe_op_in_unsafe_fn = "deny"` lint (that stays).

## Acceptance examples

| Scenario | Expected |
|---|---|
| PR adds `unsafe { Box::from_raw(ptr) }` without SAFETY comment | clippy/review blocks it |
| Existing `unsafe` block has `TODO(safety)` with no tracking issue | spec violation; file an issue |
| `unsafe fn` in FFI has `# Safety` doc section | compliant |
| New `unsafe` in `pure_parser.rs` has a unit test witness | compliant |

## Test mapping

| Behavior | Proof |
|---|---|
| B1 (SAFETY comments) | `grep -rn 'unsafe' runtime/src --include='*.rs' \| grep -v '// SAFETY:' \| wc -l` approaches 0 (staged) |
| B2 (TODO tracking) | All `TODO(safety)` markers have linked issues |
| B3 (witnesses) | New unsafe paths have test coverage |

## Implementation mapping

The fix is a staged campaign:
1. **Phase 1** (done): resolve the `ffi.rs:132` double-free TODO (#819).
2. **Phase 2**: add SAFETY comments to the remaining ~238 unsafe blocks in
   `runtime/src/`, split per hotspot (pure_parser, parser, ffi, decoder).
3. **Phase 3**: revive the `policy/unsafe-review*.toml` ledgers that
   `docs/ci/unsafe-review.md` references but don't exist yet.
4. **Phase 4**: consider promoting `clippy::missing_safety_doc` to warn in
   `policy/clippy-lints.toml`.

## CI Proof

The supported gate runs `cargo clippy -- -D warnings` which includes
`unsafe_op_in_unsafe_fn = "deny"`. Miri runs are advisory (nightly only).

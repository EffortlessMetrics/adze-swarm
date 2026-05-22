# Incremental Document Lifecycle Closeout

Status: complete
Owner: runtime/incremental
Created: 2026-05-22
Closed: 2026-05-22
Linked proposal: ../../docs/proposals/ADZE-PROP-0002-api-foundation.md
Linked spec: ../../docs/specs/ADZE-SPEC-0009-incremental-document-lifecycle.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Linked plan: ./implementation-plan.md
Linked goal: ../../.adze/goals/incremental-document-lifecycle.toml
Linked issue: EffortlessMetrics/adze-swarm#325
Linked PRs:
- EffortlessMetrics/adze-swarm#534
- EffortlessMetrics/adze-swarm#535

## Summary

The incremental document lifecycle hardening lane is complete.

PR #534 opened the non-release source-of-truth lane and added the first
document-level changed-range canary. PR #535 fixed the hosted clippy receipt
for that helper without changing behavior.

## Behavior Now Covered

- `AdzeDocument::changed_ranges(&newer)` reports conservative byte ranges in
  the newer document.
- Equal document sources return no changed ranges.
- Edited document sources return a changed byte range in the newer document.
- Reported ranges are adjusted to UTF-8 character boundaries.
- Full-reparse fallback metadata remains explicit when incremental reuse is
  requested but not used.

## Proof Receipts

Local proof from #534 and #535:

```bash
cargo clippy -p adze --lib --features incremental_glr -- -D warnings
cargo test -p adze --lib --features incremental_glr document::tests::changed_ranges -- --nocapture
cargo test -p adze --lib --features incremental_glr document::tests::reparse_fallback_metadata_records_full_reparse_fallback -- --exact --nocapture
./scripts/fmt-workspace.sh --check runtime
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

GitHub proof from #534 and #535:

```text
Rust Small Result: success
Product Proof Result: success
Source of Truth: success
PR Plan: success
Coverage Lite: success
GLR Invariants: success
```

## Boundaries

This closeout does not authorize or perform release work.

Still blocked on explicit release authorization in #325:

```text
tag
cargo publish
signing
Cargo-token work
crates.io install receipt
public cargo install adze-cli claim
```

Incremental parsing remains Experimental. This lane does not claim:

```text
real incremental parse reuse
stable changed-range precision
stable cross-document node identity
incremental performance
raw GLR forest reuse
```

Public `adze` remains the release, public-intake, promotion, tag, publish,
signing, and Cargo-token surface. `adze-swarm` remains the implementation and
proof repo.

## Remaining Non-Release Gaps

No ready incremental document lifecycle work remains in this lane.

Future incremental implementation work must open a fresh active goal and prove
the next behavior slice before changing support-tier claims.

# Incremental Document Lifecycle Hardening Plan

Status: complete
Owner: runtime/incremental
Created: 2026-05-22
Closed: 2026-05-22
Linked proposal: ../../docs/proposals/ADZE-PROP-0002-api-foundation.md
Linked specs:
- ../../docs/specs/ADZE-SPEC-0009-incremental-document-lifecycle.md
- ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADRs:
- ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/incremental-document-lifecycle.toml
Closeout: ./closeout.md
Support-tier impact: incremental parsing remains Experimental
Policy impact: no release, publish, signing, Cargo-token, branch-protection, or public-promotion change

## Goal

Make the accepted incremental document lifecycle contract more executable
without claiming stable incremental reuse or performance. The lane starts with
the smallest missing document-level proof from `ADZE-SPEC-0009`: conservative
changed ranges between immutable document snapshots.

## Operating Rules

- Work in `EffortlessMetrics/adze-swarm`.
- Do not open implementation PRs in public `EffortlessMetrics/adze`.
- Do not tag, publish, sign, mutate Cargo-token surfaces, change release
  workflows, or claim crates.io install support in this lane.
- Keep incremental parsing Experimental unless support-tier proof is promoted
  deliberately.
- Do not claim node reuse, stable cross-document node IDs, stable changed-range
  precision, or incremental performance from this lane.
- Inspect open `adze-swarm` PRs before opening duplicate work.

## Work Item: incremental-document-source-of-truth

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0002-api-foundation.md
Linked spec: ../../docs/specs/ADZE-SPEC-0009-incremental-document-lifecycle.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks:
- document-changed-ranges-canary
Blocked by: n/a

### Goal

Replace the completed CLI dynamic parse boundary manifest with a focused
non-release lane for incremental document lifecycle hardening.

### Production Delta

Source-of-truth metadata only.

### Acceptance

- `.adze/goals/active.toml` names the incremental document lifecycle campaign.
- `.adze/goals/incremental-document-lifecycle.toml` exists.
- `policy/doc-artifacts.toml` registers the plan and goal.
- `plans/README.md` lists the lane.
- Release blocker tracker #325 remains the release/publish authorization
  checkpoint.

### Proof Commands

```bash
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Revert the setup portion to restore the completed CLI dynamic parse boundary
active manifest.

## Work Item: document-changed-ranges-canary

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0002-api-foundation.md
Linked spec: ../../docs/specs/ADZE-SPEC-0009-incremental-document-lifecycle.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks:
- incremental-document-lifecycle-closeout
Blocked by:
- incremental-document-source-of-truth

### Goal

Expose an experimental `AdzeDocument::changed_ranges(&newer)` helper that
reports conservative byte ranges in the newer document while preserving the
one-parse-truth architecture.

### Production Delta

Runtime document helper and focused canaries.

### Non-Goals

- No real incremental parse reuse implementation.
- No stable changed-range precision guarantee.
- No stable cross-document node identity.
- No stable incremental performance claim.
- No release/install claim.

### Acceptance

- Equal document sources return no changed ranges.
- Edited document sources return a changed byte range in the newer document.
- Reported ranges stay on UTF-8 boundaries.
- Existing full-reparse fallback metadata remains unchanged and honest.

### Proof Commands

```bash
cargo test -p adze --lib --features incremental_glr document::tests::changed_ranges -- --nocapture
cargo test -p adze --lib --features incremental_glr document::tests::reparse_fallback_metadata_records_full_reparse_fallback -- --exact --nocapture
./scripts/fmt-workspace.sh --check runtime
git diff --check
```

### Rollback

Revert the runtime helper and canaries. Incremental document lifecycle remains
Experimental with only full-reparse fallback metadata proof.

## Work Item: incremental-document-lifecycle-closeout

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0002-api-foundation.md
Linked spec: ../../docs/specs/ADZE-SPEC-0009-incremental-document-lifecycle.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: n/a
Blocked by:
- document-changed-ranges-canary

### Goal

Close the lane after changed-range canaries land and support-tier wording still
matches the proved Experimental surface.

### Production Delta

Source-of-truth closeout only when behavior receipts exist.

### Acceptance

- `plans/incremental-document-lifecycle/closeout.md` records shipped behavior,
  proof commands, claim boundaries, and deferred work.
- The active and named goal manifests mark all lane work items complete.
- `policy/doc-artifacts.toml` registers the closeout and archived goal.
- `plans/README.md` and `docs/status/NOW_NEXT_LATER.md` no longer describe
  this lane as active.

### Proof Commands

```bash
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Revert the closeout PR if it overstates behavior or support-tier status.

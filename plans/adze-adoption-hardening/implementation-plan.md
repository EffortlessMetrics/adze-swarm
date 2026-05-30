# Adze Adoption Hardening Plan

Status: active
Owner: runtime/product
Created: 2026-05-29
Linked proposals:
- ../../docs/proposals/ADZE-PROP-0004-toolkit-excellence.md
- ../../docs/proposals/ADZE-PROP-0006-user-experience-hardening.md
- ../../docs/proposals/ADZE-PROP-0017-release-candidate-bundle.md
Linked specs:
- ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
- ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
- ../../docs/specs/ADZE-SPEC-0013-query-compatibility.md
- ../../docs/specs/ADZE-SPEC-0014-performance-and-regression.md
Linked ADRs:
- ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
- ../../docs/adr/ADZE-ADR-0003-summary-first-glr-ambiguity.md
- ../../docs/adr/ADZE-ADR-0004-schema-versioned-projections.md
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/adze-adoption-hardening.toml
Support-tier map: ../../docs/status/SUPPORT_TIERS.md
Release authorization tracker: https://github.com/EffortlessMetrics/adze-swarm/issues/325
Lane-selection tracker: https://github.com/EffortlessMetrics/adze-swarm/issues/549

## Goal

Refresh and harden the adoption path for the already-proven Adze GLR toolkit
without changing release claims. This lane owns first-use proof, API guidance,
walkthrough clarity, receipt guidance, and release-boundary discipline. It is a
non-release `adze-swarm` lane.

## Operating Rules

- Work in `EffortlessMetrics/adze-swarm`.
- Do not open implementation, docs-proof, CI, examples, runtime, tablegen,
  query, diagnostics, or performance PRs in public `EffortlessMetrics/adze`.
- Do not tag, publish, sign, mutate Cargo-token surfaces, or change release
  workflows in this lane.
- Keep public `adze` as the release, public-intake, promotion, tag, publish,
  signing, Cargo-token, and crates.io receipt surface.
- Keep `AdzeDocument` as the canonical parse product; all public views remain
  projections over that document.
- Keep support-tier claims bounded by `docs/status/SUPPORT_TIERS.md`.
- Use `Rust Small Result` as the normalized base gate for PR proof.
- Inspect open `adze-swarm` PRs before opening duplicate work.
- Keep each PR to one work item with scope, proof, claim boundary, CI cost, and
  rollback recorded in the PR body.

## Work Item: adoption-hardening-source-of-truth

Status: complete
Linked proposals:
- ../../docs/proposals/ADZE-PROP-0006-user-experience-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks:
- starter-project-downstream-fixture
- api-choice-guide
- glr-ambiguity-walkthrough
- diagnostics-recovery-walkthrough
- query-cookbook
- ts-compat-selected-tree-guide
- benchmark-receipt-guide
- public-release-boundary-checklist
Blocked by: n/a

### Goal

Replace the paused forge standby manifest with a selected non-release adoption
hardening lane so agents can continue from a source-of-truth-linked queue
without touching release machinery.

### Production Delta

Docs and source-of-truth metadata only.

### Receipt

Lands in PR #562.

### Non-Goals

- No runtime behavior change.
- No CI workflow change.
- No public `adze` change.
- No release/publish authorization.
- No crates.io install claim.
- No support-tier promotion.

### Acceptance

- `.adze/goals/active.toml` names the adoption hardening lane.
- `.adze/goals/adze-adoption-hardening.toml` exists.
- `policy/doc-artifacts.toml` registers the named goal and implementation plan.
- `docs/status/ADZE_SWARM_FORGE_AUDIT.md` records that the next non-release
  lane has been selected while release work remains blocked.
- Issue #549 is linked as the lane-selection source of truth.
- Issue #325 remains the release/publish authorization checkpoint.

### Proof Commands

```bash
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
gh pr list --repo EffortlessMetrics/adze-swarm --state open --json number,title,url
gh pr list --repo EffortlessMetrics/adze --state open --json number,title,url
```

### Rollback

Revert the setup PR to restore the paused forge standby manifest.

## Work Item: starter-project-downstream-fixture

Status: complete
Blocked by: n/a
Linked proposal: ../../docs/proposals/ADZE-PROP-0006-user-experience-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md

### Goal

Refresh the generated starter and checked-in downstream fixture so the local
install/init/build/parse path stays easy to inspect and proof-backed.

### Receipt

Lands in PR #563.

### Proof Commands

```bash
cargo test -p adze-cli test_init -- --nocapture
cargo test --manifest-path testing/downstream-starter/Cargo.toml
cargo run --manifest-path testing/downstream-starter/Cargo.toml --example parse
```

## Work Item: api-choice-guide

Status: complete
Blocked by: n/a
Linked proposal: ../../docs/proposals/ADZE-PROP-0006-user-experience-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md

### Goal

Keep `grammar::parse(source)` as the beginner path and
`grammar::parse_document(source)` / `AdzeDocument` as the tooling path, with
advanced projections described as support-tier-bounded views.

### Receipt

Lands in PR #564.

### Production Delta

Docs and source-of-truth metadata only.

### Claim Boundary

The guide may point to local checkout and path-dependency starter proof. It must
not claim crates.io install availability, public release availability, full
Tree-sitter parity, full query parity, stable JSON schemas, or stable raw GLR
forest APIs.

### Proof Commands

```bash
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
cargo test --manifest-path testing/downstream-starter/Cargo.toml
cargo run --manifest-path testing/downstream-starter/Cargo.toml --example parse -- "1 + 2 * 3"
git diff --check
```

### Rollback

Revert the guide and source-of-truth metadata changes.

## Work Item: glr-ambiguity-walkthrough

Status: complete
Blocked by: n/a
Linked proposal: ../../docs/proposals/ADZE-PROP-0004-toolkit-excellence.md
Linked spec: ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
Linked ADR: ../../docs/adr/ADZE-ADR-0003-summary-first-glr-ambiguity.md

### Goal

Refresh GLR ambiguity guidance so users can distinguish selected AST behavior,
document ambiguity summaries, and experimental/raw forest boundaries.

### Receipt

Lands in PR #567.

### Production Delta

Docs and focused cookbook proof only.

### Claim Boundary

This work may document the generated `parse()` selected typed AST path,
`parse_document()` ambiguity summaries, and Tree-sitter-shaped selected-tree
projection. It must not claim Stable raw GLR forest export, typed extraction
from ambiguity alternatives, full Tree-sitter parity, full query parity,
general GLR coverage beyond the support-tier row, or release availability.

### Proof Commands

```bash
cargo run -p adze --features "pure-rust,glr" --example glr_ambiguity
cargo test -p adze --features "pure-rust,glr,ts-compat" cookbook -- --nocapture
```

### Rollback

Revert the walkthrough and cookbook-proof updates.

## Work Item: diagnostics-recovery-walkthrough

Status: ready
Blocked by: n/a
Linked proposal: ../../docs/proposals/ADZE-PROP-0006-user-experience-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md

### Goal

Refresh diagnostics and recovery guidance so parse failures, spans, bad input,
missing nodes, and JSON diagnostic projections stay useful and claim-bounded.

## Work Item: query-cookbook

Status: ready
Blocked by: n/a
Linked proposal: ../../docs/proposals/ADZE-PROP-0008-query-tooling-expansion.md
Linked spec: ../../docs/specs/ADZE-SPEC-0013-query-compatibility.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md

### Goal

Refresh query cookbook guidance for the documented subset, examples, known
gaps, and proof commands without claiming full Tree-sitter query parity.

## Work Item: ts-compat-selected-tree-guide

Status: ready
Blocked by: n/a
Linked proposal: ../../docs/proposals/ADZE-PROP-0004-toolkit-excellence.md
Linked spec: ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md

### Goal

Refresh selected-tree compatibility guidance so users understand the supported
Tree-sitter-shaped subset, known gaps, and canary coverage.

## Work Item: benchmark-receipt-guide

Status: ready
Blocked by: n/a
Linked proposal: ../../docs/proposals/ADZE-PROP-0006-user-experience-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0014-performance-and-regression.md

### Goal

Refresh benchmark receipt guidance without creating stable throughput, memory,
Tree-sitter performance parity, incremental performance, or release-blocking
regression claims.

## Work Item: public-release-boundary-checklist

Status: ready
Blocked by: n/a
Linked proposal: ../../docs/proposals/ADZE-PROP-0017-release-candidate-bundle.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md

### Goal

Keep public release-boundary guidance obvious while adoption hardening runs:
`adze-swarm` may produce promotion-ready proof, but public `adze` release,
publish, signing, Cargo-token, and crates.io install receipts require explicit
authorization through #325.

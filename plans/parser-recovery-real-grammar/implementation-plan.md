# Parser Recovery Real-Grammar Coverage Plan

Status: active
Owner: runtime/diagnostics
Created: 2026-05-20
Linked proposal: ../../docs/proposals/ADZE-PROP-0009-parser-recovery-real-grammar-coverage.md
Linked specs:
- ../../docs/specs/ADZE-SPEC-0005-diagnostics-and-recovery.md
- ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADRs:
- ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/parser-recovery-real-grammar.toml
Support-tier impact: no promotion by campaign setup
Policy impact: no release, publish, signing, Cargo-token, or public-repo implementation work

## Goal

Narrow the current product-audit gap around broader real-grammar
parser-generated external-scanner recovery coverage.

## Operating Rules

- Work in `EffortlessMetrics/adze-swarm`.
- Do not open implementation, proof, docs-productization, or CI PRs in public
  `EffortlessMetrics/adze`.
- Do not tag, publish, sign, mutate Cargo-token surfaces, or change release
  workflows in this lane.
- Keep external scanners at their current support tier unless a later
  support-tier review has proof and limitations.
- Keep `AdzeDocument` as the one parse truth for diagnostics and projections.
- Use `Rust Small Result` as the GitHub gate.

## Work Item: parser-recovery-source-of-truth

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0009-parser-recovery-real-grammar-coverage.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks:
- generated-external-real-grammar-matrix
- parser-v4-scanner-recovery-smoke
- support-tier-boundary-refresh
Blocked by: n/a

### Goal

Replace the paused no-active-lane manifest with a focused non-release
real-grammar recovery coverage lane.

### Production Delta

Docs and source-of-truth metadata only.

### Non-Goals

- No runtime behavior change.
- No external-scanner Stable promotion.
- No release/publish authorization.

### Acceptance

- `.adze/goals/active.toml` names this campaign.
- `.adze/goals/parser-recovery-real-grammar.toml` exists.
- `policy/doc-artifacts.toml` registers the proposal, plan, and named goal.
- Release blocker tracker #325 remains outside this lane.

### Proof Commands

```bash
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Revert the setup PR to restore the previous paused query/tooling closeout
manifest.

## Work Item: generated-external-real-grammar-matrix

Status: ready
Linked proposal: ../../docs/proposals/ADZE-PROP-0009-parser-recovery-real-grammar-coverage.md
Linked spec: ../../docs/specs/ADZE-SPEC-0005-diagnostics-and-recovery.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: support-tier-boundary-refresh
Blocked by: parser-recovery-source-of-truth

### Goal

Expand generated external-scanner grammar recovery tests beyond the current
single matrix shape where the codebase has an existing generated grammar that
can prove malformed input returns bounded diagnostics.

### Production Delta

Expected future PRs may add focused test cases under `example/` or
`runtime/tests/`.

### Non-Goals

- No broad corpus parity claim.
- No external-scanner Stable promotion.

### Acceptance

- New cases compare `parse()` errors and `parse_document()` diagnostics where
  both APIs are available.
- Diagnostics have bounded byte spans and matching point ranges.
- Expected token names remain public-facing and do not expose raw symbol IDs.

### Proof Commands

```bash
cargo test --manifest-path example/Cargo.toml external_word_example::tests::generated_external_grammar_bad_input_matrix_returns_diagnostic_document --features pure-rust -- --exact --nocapture
git diff --check
```

### Rollback

Revert the focused test PR.

## Work Item: parser-v4-scanner-recovery-smoke

Status: ready
Linked proposal: ../../docs/proposals/ADZE-PROP-0009-parser-recovery-real-grammar-coverage.md
Linked spec: ../../docs/specs/ADZE-SPEC-0005-diagnostics-and-recovery.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: support-tier-boundary-refresh
Blocked by: parser-recovery-source-of-truth

### Goal

Add or refresh direct parser-v4 scanner recovery smoke where a scanner is
registered, malformed input is parsed, and diagnostics stay source-bounded.

### Production Delta

Expected future PRs may add focused parser-v4 tests only.

### Non-Goals

- No generated-language corpus parity claim.
- No stable public external-scanner API claim.

### Acceptance

- Scanner state does not advance incorrectly on rejected external tokens.
- Diagnostic spans stay within source bounds.
- Rendered diagnostics include source context when available.

### Proof Commands

```bash
cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests -- --nocapture
git diff --check
```

### Rollback

Revert the focused parser-v4 test PR.

## Work Item: support-tier-boundary-refresh

Status: blocked
Linked proposal: ../../docs/proposals/ADZE-PROP-0009-parser-recovery-real-grammar-coverage.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: n/a
Blocked by:
- generated-external-real-grammar-matrix
- parser-v4-scanner-recovery-smoke

### Goal

Refresh support-tier and audit wording only after new proof commands exist.

### Production Delta

Expected future PRs may update `docs/status/SUPPORT_TIERS.md` and
`docs/status/PRODUCT_OBJECTIVE_AUDIT.md`.

### Non-Goals

- No support-tier promotion unless proof and limitations justify it.
- No README Stable claim change by setup.

### Acceptance

- New proof commands are named in the relevant support/audit rows.
- Remaining real-grammar recovery limits are explicit.

### Proof Commands

```bash
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
cargo run -q -p xtask -- check-active-goal --mode blocking
git diff --check
```

### Rollback

Revert the status refresh PR.

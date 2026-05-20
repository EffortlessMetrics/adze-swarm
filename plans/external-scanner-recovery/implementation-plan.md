# External Scanner Recovery Hardening Plan

Status: active
Owner: runtime/diagnostics
Created: 2026-05-20
Linked proposal: ../../docs/proposals/ADZE-PROP-0007-external-scanner-recovery-hardening.md
Linked specs:
- ../../docs/specs/ADZE-SPEC-0005-diagnostics-and-recovery.md
- ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
- ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
Linked ADRs:
- ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/external-scanner-recovery-hardening.toml
Support-tier impact: no promotion by campaign setup
Policy impact: no release, publish, signing, Cargo-token, or public-repo implementation work

## Goal

Close the named non-release product-trust gap around parser-generated
external-scanner recovery by adding focused proof for generated
external-token malformed-input handling and parser-v4 document diagnostics.

## Operating Rules

- Work in `EffortlessMetrics/adze-swarm`.
- Do not open implementation, proof, docs-productization, or CI PRs in public
  `EffortlessMetrics/adze`.
- Do not tag, publish, sign, mutate Cargo-token surfaces, or change release
  workflows in this lane.
- Keep external scanners Experimental unless support tiers explicitly promote a
  proven slice after proof exists.
- Keep `AdzeDocument` as the diagnostic source of truth.
- Use `Rust Small Result` as the GitHub gate.
- Inspect open `adze-swarm` PRs before opening duplicate work.

## Work Item: external-scanner-recovery-source-of-truth

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0007-external-scanner-recovery-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks:
- generated-external-recovery-matrix-expansion
- parser-v4-external-diagnostic-detail
- support-tier-boundary-refresh
Blocked by: n/a

### Goal

Replace the paused no-active-lane manifest with a focused non-release product
trust lane for external-scanner recovery proof.

### Production Delta

Docs and source-of-truth metadata only.

### Non-Goals

- No runtime behavior change.
- No support-tier promotion.
- No release/publish authorization.
- No crates.io install claim.

### Acceptance

- `.adze/goals/active.toml` names this campaign.
- `.adze/goals/external-scanner-recovery-hardening.toml` exists.
- `policy/doc-artifacts.toml` registers the proposal, plan, and named goal.
- Release blocker tracker #325 remains outside this lane.

### Proof Commands

```bash
python -c "import tomllib; tomllib.load(open('.adze/goals/active.toml', 'rb')); tomllib.load(open('.adze/goals/external-scanner-recovery-hardening.toml', 'rb')); tomllib.load(open('policy/doc-artifacts.toml', 'rb'))"
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Revert the setup PR to restore the previous paused no-active-lane manifest.

## Work Item: generated-external-recovery-matrix-expansion

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0007-external-scanner-recovery-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0005-diagnostics-and-recovery.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: support-tier-boundary-refresh
Blocked by: external-scanner-recovery-source-of-truth

### Goal

Expand the generated external-token malformed-input matrix beyond the current
root, keyword, missing-colon, trailing-token, multibyte, body, and newline
boundary cases.

### Receipt

Adds empty-source, whitespace-only, missing-condition, multibyte body-token,
CRLF boundary, and nested invalid-expression cases to the generated
external-token recovery matrix.

### Production Delta

Add focused cases in `example/src/external_word_example.rs`.

### Non-Goals

- No external-scanner Stable claim.
- No runtime rewrite.

### Acceptance

- Additional malformed-input cases prove `parse()` and `parse_document()`
  agree on diagnostic spans and expected-token names.
- Document diagnostics keep ordered byte spans, matching point ranges, selected
  tree error facts, and public expected-token names.

### Proof Commands

```bash
cargo test --manifest-path example/Cargo.toml external_word_example::tests::generated_external_grammar_bad_input_matrix_returns_diagnostic_document --features pure-rust -- --exact --nocapture
git diff --check
```

### Rollback

Revert the test expansion PR.

## Work Item: parser-v4-external-diagnostic-detail

Status: ready
Linked proposal: ../../docs/proposals/ADZE-PROP-0007-external-scanner-recovery-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0005-diagnostics-and-recovery.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: support-tier-boundary-refresh
Blocked by: external-scanner-recovery-source-of-truth

### Goal

Harden direct parser-v4 external-scanner diagnostic canaries so they prove
document error facts, span ordering, point ranges, and public expected names
where available.

### Production Delta

Add or refine focused parser-v4 tests in `runtime/src/parser_v4.rs`.

### Non-Goals

- No generated-parser API change.
- No scanner runtime rewrite.

### Acceptance

- Parser-v4 bad-input external-scanner document diagnostics remain useful and
  source-bounded.
- Invalid scanner emissions remain rejected when the parser state does not
  allow the emitted external token.

### Proof Commands

```bash
cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests::test_external_scanner_parse_document_bad_input_returns_diagnostic_document -- --exact --nocapture
cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests::test_external_scanner_rejects_token_not_in_valid_symbols -- --exact --nocapture
git diff --check
```

### Rollback

Revert the focused parser-v4 test PR.

## Work Item: support-tier-boundary-refresh

Status: ready
Linked proposal: ../../docs/proposals/ADZE-PROP-0007-external-scanner-recovery-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: n/a
Blocked by:
- generated-external-recovery-matrix-expansion
- parser-v4-external-diagnostic-detail

### Goal

Refresh support-tier and product-audit wording after new proof exists, without
overclaiming external-scanner stability.

### Production Delta

Update `docs/status/SUPPORT_TIERS.md` and
`docs/status/PRODUCT_OBJECTIVE_AUDIT.md`.

### Non-Goals

- No support-tier promotion unless the proof and limitations justify it.
- No README Stable claim change.

### Acceptance

- New proof commands are named in the relevant support/audit rows.
- Any remaining external-scanner recovery limitations are explicit.

### Proof Commands

```bash
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
cargo run -q -p xtask -- check-active-goal --mode blocking
git diff --check
```

### Rollback

Revert the status refresh PR.

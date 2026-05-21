# CLI Parse Surface Hardening Plan

Status: active
Owner: cli/product
Created: 2026-05-21
Linked proposal: ../../docs/proposals/ADZE-PROP-0013-cli-parse-surface-hardening.md
Linked specs:
- ../../docs/specs/ADZE-SPEC-0008-json-cli-wasm-projections.md
- ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
- ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
Linked ADRs:
- ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
- ../../docs/adr/ADZE-ADR-0004-schema-versioned-projections.md
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/cli-parse-surface-hardening.toml
Support-tier impact: no promotion by campaign setup
Policy impact: no release, publish, signing, Cargo-token, branch-protection, or public-promotion change

## Goal

Make `adze parse` more useful for local, checked-out CLI users while keeping
all static parse output document-backed, support-tier bounded, and separate
from release/install claims.

## Operating Rules

- Work in `EffortlessMetrics/adze-swarm`.
- Do not open CLI implementation PRs in public `EffortlessMetrics/adze`.
- Do not tag, publish, sign, mutate Cargo-token surfaces, change release
  workflows, or claim crates.io install support in this lane.
- Keep public `adze` as release/public-intake/publish surface.
- Keep support-tier claims bounded by `docs/status/SUPPORT_TIERS.md`.
- Route static CLI parse output through generated `parse_document()`.
- Inspect open `adze-swarm` PRs before opening duplicate work.

## Work Item: cli-parse-surface-source-of-truth

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0013-cli-parse-surface-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks:
- static-cli-selected-tree-output
Blocked by: n/a

### Goal

Replace the completed parser/runtime maintainability active manifest with a
fresh non-release CLI parse-surface lane.

### Production Delta

Docs and source-of-truth metadata only.

### Non-Goals

- No CLI behavior change.
- No release/publish authorization.
- No crates.io install claim.
- No support-tier promotion.

### Acceptance

- `.adze/goals/active.toml` names the CLI parse-surface campaign.
- `.adze/goals/cli-parse-surface-hardening.toml` exists.
- The completed parser/runtime maintainability active manifest is archived.
- `policy/doc-artifacts.toml` registers the proposal, plan, and goal.
- Release blocker tracker #325 remains the release/publish authorization
  checkpoint.
- Completed by #457.

### Proof Commands

```bash
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Revert the setup PR to restore the previous completed parser/runtime
maintainability active manifest.

## Work Item: static-cli-selected-tree-output

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0013-cli-parse-surface-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0008-json-cli-wasm-projections.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks:
- cli-parse-surface-closeout
Blocked by:
- cli-parse-surface-source-of-truth

### Goal

Make default static `adze parse <grammar.rs> <input>` useful by emitting a
selected-tree receipt backed by generated `parse_document()`.

### Production Delta

CLI behavior and focused tests only. The existing document projection modes
must remain unchanged.

### Non-Goals

- No stable CLI/WASM schema claim.
- No dynamic parse output implementation.
- No Tree-sitter CLI parity claim.
- No release/install claim.

### Acceptance

- Default `adze parse <grammar.rs> <input>` no longer reports static parse as
  unimplemented for generated single-file grammars.
- The selected-tree output is derived from `parse_document()`.
- Bad input either emits document-backed recovery facts or fails with the same
  explicit diagnostic boundary as the document projection path.
- Unsupported output modes remain explicit until implemented.
- Existing `document-json`, `tree-json`, `diagnostics-json`, and
  `ambiguity-json` tests still pass.
- Completed by #458.

### Proof Commands

```bash
cargo test -p adze-cli test_parse_document_projection_modes_emit_schema_envelopes -- --exact --nocapture
cargo test -p adze-cli parse_document_json_modes_emit_recovery_diagnostics -- --exact --nocapture
cargo test -p adze-cli <new-static-tree-test> -- --exact --nocapture
git diff --check
```

### Rollback

Revert the behavior PR. The CLI will return to the existing explicit
unimplemented static parse boundary while document projection modes remain
available.

## Work Item: cli-parse-surface-closeout

Status: ready
Linked proposal: ../../docs/proposals/ADZE-PROP-0013-cli-parse-surface-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: n/a
Blocked by:
- static-cli-selected-tree-output

### Goal

Close the lane after the selected CLI behavior has landed and support-tier
language matches the proved surface.

### Production Delta

Source-of-truth closeout and support-tier wording only when behavior receipts
exist.

### Non-Goals

- No support-tier promotion without proof.
- No release/publish execution.
- No crates.io install claim.

### Acceptance

- The CLI row in `docs/status/SUPPORT_TIERS.md` names the proved static parse
  behavior and known limitations.
- The closeout records PRs, proof commands, and remaining unsupported modes.
- Release-only work remains tracked on #325.

### Proof Commands

```bash
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Revert the closeout PR if it overstates behavior or support-tier status.

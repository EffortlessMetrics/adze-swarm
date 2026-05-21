# CLI Static S-Expression Output Plan

Status: complete
Owner: cli/product
Created: 2026-05-21
Linked proposal: ../../docs/proposals/ADZE-PROP-0014-cli-static-sexpression-output.md
Linked specs:
- ../../docs/specs/ADZE-SPEC-0008-json-cli-wasm-projections.md
- ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADRs:
- ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
- ../../docs/adr/ADZE-ADR-0004-schema-versioned-projections.md
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/cli-static-sexp-output.toml
Closeout: ./closeout.md
Support-tier impact: no promotion by campaign setup
Policy impact: no release, publish, signing, Cargo-token, branch-protection, or public-promotion change

## Goal

Make static `adze parse --output sexp <grammar.rs> <input>` useful for
checked-out CLI users by rendering a selected-tree S-expression from generated
`parse_document()` facts.

## Operating Rules

- Work in `EffortlessMetrics/adze-swarm`.
- Do not open CLI implementation PRs in public `EffortlessMetrics/adze`.
- Do not tag, publish, sign, mutate Cargo-token surfaces, change release
  workflows, or claim crates.io install support in this lane.
- Keep public `adze` as release/public-intake/publish surface.
- Keep support-tier claims bounded by `docs/status/SUPPORT_TIERS.md`.
- Route static CLI parse output through generated `parse_document()`.
- Inspect open `adze-swarm` PRs before opening duplicate work.

## Work Item: cli-static-sexp-source-of-truth

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0014-cli-static-sexpression-output.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks:
- static-cli-sexpression-output
Blocked by: n/a

### Goal

Replace the completed CLI parse-surface manifest with a focused non-release
lane for static S-expression output.

### Production Delta

Docs and source-of-truth metadata only.

### Non-Goals

- No CLI behavior change.
- No release/publish authorization.
- No crates.io install claim.
- No support-tier promotion.

### Acceptance

- `.adze/goals/active.toml` names the CLI static S-expression campaign.
- `.adze/goals/cli-static-sexp-output.toml` exists.
- `policy/doc-artifacts.toml` registers the proposal, plan, and goal.
- Release blocker tracker #325 remains the release/publish authorization
  checkpoint.
- Completed by #461.

### Proof Commands

```bash
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Revert the setup PR to restore the previous completed CLI parse-surface active
manifest.

## Work Item: static-cli-sexpression-output

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0014-cli-static-sexpression-output.md
Linked spec: ../../docs/specs/ADZE-SPEC-0008-json-cli-wasm-projections.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks:
- cli-static-sexp-closeout
Blocked by:
- cli-static-sexp-source-of-truth

### Goal

Implement static `sexp` output for generated single-file grammars using the
same generated `parse_document()` runner as the current default `tree` output.

### Production Delta

CLI behavior, focused tests, and support-tier/docs wording only.

### Non-Goals

- No stable CLI/WASM schema claim.
- No dynamic parse output implementation.
- No `json` or `dot` static output implementation.
- No Tree-sitter CLI parity claim.
- No release/install claim.

### Acceptance

- `adze parse --output sexp <grammar.rs> <input>` succeeds for a generated
  single-file grammar.
- S-expression output is derived from `parse_document()` document tree facts.
- Default `tree` output remains unchanged.
- `document-json`, `tree-json`, `diagnostics-json`, and `ambiguity-json` modes
  still pass their schema-envelope and recovery-diagnostics receipts.
- `json` and `dot` remain explicit unsupported static modes.
- CLI support-tier wording records S-expression as Stabilizing, not Stable.
- Completed by #462.

### Proof Commands

```bash
cargo test -p adze-cli test_parse_static_sexp_mode_emits_document_backed_sexp -- --exact --nocapture
cargo test -p adze-cli test_parse_static_non_document_modes_are_explicitly_unimplemented -- --exact --nocapture
cargo test -p adze-cli test_parse_static_tree_mode_emits_document_backed_tree -- --exact --nocapture
cargo test -p adze-cli test_parse_document_projection_modes_emit_schema_envelopes -- --exact --nocapture
cargo test -p adze-cli parse_document_json_modes_emit_recovery_diagnostics -- --exact --nocapture
cargo fmt -p adze-cli -- --check
git diff --check
```

### Rollback

Revert the behavior PR. The CLI will keep default `tree` and document
projection modes while returning to the explicit unsupported boundary for
`sexp`.

## Work Item: cli-static-sexp-closeout

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0014-cli-static-sexpression-output.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: n/a
Blocked by:
- static-cli-sexpression-output

### Goal

Close the lane after static S-expression output has landed and support-tier
language matches the proved surface.

### Production Delta

Source-of-truth closeout and support-tier wording only when behavior receipts
exist.

### Non-Goals

- No support-tier promotion without proof.
- No release/publish execution.
- No crates.io install claim.

### Acceptance

- The CLI row in `docs/status/SUPPORT_TIERS.md` names the proved static S-expression
  behavior and known limitations.
- The closeout records PRs, proof commands, and remaining unsupported modes.
- Release-only work remains tracked on #325.
- Closeout recorded in [`closeout.md`](./closeout.md).
- Completed by #463.

### Proof Commands

```bash
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Revert the closeout PR if it overstates behavior or support-tier status.

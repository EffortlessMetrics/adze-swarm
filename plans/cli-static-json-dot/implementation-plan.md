# CLI Static JSON And DOT Output Plan

Status: complete
Owner: cli/product
Created: 2026-05-21
Linked proposal: ../../docs/proposals/ADZE-PROP-0015-cli-static-json-dot-output.md
Linked specs:
- ../../docs/specs/ADZE-SPEC-0008-json-cli-wasm-projections.md
- ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADRs:
- ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
- ../../docs/adr/ADZE-ADR-0004-schema-versioned-projections.md
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/cli-static-json-dot-output.toml
Closeout: ./closeout.md
Support-tier impact: static JSON and DOT receipts recorded as Stabilizing, not Stable
Policy impact: no release, publish, signing, Cargo-token, branch-protection, or public-promotion change

## Goal

Finish the checked-out static `adze parse` output set by making `json` and
`dot` document-backed outputs while preserving the experimental/stabilizing CLI
boundary.

## Operating Rules

- Work in `EffortlessMetrics/adze-swarm`.
- Do not open CLI implementation PRs in public `EffortlessMetrics/adze`.
- Do not tag, publish, sign, mutate Cargo-token surfaces, change release
  workflows, or claim crates.io install support in this lane.
- Keep support-tier claims bounded by `docs/status/SUPPORT_TIERS.md`.
- Route static CLI parse output through generated `parse_document()`.
- Inspect open `adze-swarm` PRs before opening duplicate work.

## Work Item: cli-static-json-dot-source-of-truth

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0015-cli-static-json-dot-output.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks:
- static-cli-json-dot-output
Blocked by: n/a

### Goal

Replace the completed CLI static S-expression manifest with a focused
non-release lane for static JSON and DOT outputs.

### Production Delta

Docs and source-of-truth metadata only.

### Acceptance

- `.adze/goals/active.toml` names the CLI static JSON/DOT campaign.
- `.adze/goals/cli-static-json-dot-output.toml` exists.
- `policy/doc-artifacts.toml` registers the proposal, plan, and goal.
- Release blocker tracker #325 remains the release/publish authorization
  checkpoint.
- Completed by #464.

### Proof Commands

```bash
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Revert the setup PR to restore the completed CLI static S-expression active
manifest.

## Work Item: static-cli-json-dot-output

Status: complete
Completed by: EffortlessMetrics/adze-swarm#465
Linked proposal: ../../docs/proposals/ADZE-PROP-0015-cli-static-json-dot-output.md
Linked spec: ../../docs/specs/ADZE-SPEC-0008-json-cli-wasm-projections.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks:
- cli-static-json-dot-closeout
Blocked by:
- cli-static-json-dot-source-of-truth

### Goal

Implement static `json` and `dot` output for generated single-file grammars
using the same generated `parse_document()` runner as the current static tree
and S-expression outputs.

### Production Delta

CLI behavior, focused tests, and support-tier/docs wording only.

### Non-Goals

- No stable CLI/WASM schema claim.
- No dynamic parse output implementation.
- No Tree-sitter CLI parity claim.
- No release/install claim.

### Acceptance

- `adze parse --output json <grammar.rs> <input>` succeeds for a generated
  single-file grammar and emits document JSON.
- `adze parse --output dot <grammar.rs> <input>` succeeds for a generated
  single-file grammar and emits a selected-tree DOT graph.
- `tree`, `sexp`, `document-json`, `tree-json`, `diagnostics-json`, and
  `ambiguity-json` modes still pass their receipts.
- CLI support-tier wording records `json` and `dot` as Stabilizing, not Stable.

### Proof Commands

```bash
cargo test -p adze-cli test_parse_static_json_mode_emits_document_json -- --exact --nocapture
cargo test -p adze-cli test_parse_static_dot_mode_emits_document_backed_graph -- --exact --nocapture
cargo test -p adze-cli test_parse_static_tree_mode_emits_document_backed_tree -- --exact --nocapture
cargo test -p adze-cli test_parse_static_sexp_mode_emits_document_backed_sexp -- --exact --nocapture
cargo test -p adze-cli test_parse_document_projection_modes_emit_schema_envelopes -- --exact --nocapture
cargo test -p adze-cli parse_document_json_modes_emit_recovery_diagnostics -- --exact --nocapture
cargo fmt -p adze-cli -- --check
cargo clippy -p adze-cli --all-targets -- -D warnings
git diff --check
```

### Rollback

Revert the behavior PR. The CLI will keep `tree`, `sexp`, and explicit document
projection modes while returning to the explicit unsupported boundary for
`json` and `dot`.

## Work Item: cli-static-json-dot-closeout

Status: blocked
Linked proposal: ../../docs/proposals/ADZE-PROP-0015-cli-static-json-dot-output.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: n/a
Blocked by:
- static-cli-json-dot-output

### Goal

Close the lane after static JSON and DOT output have landed and support-tier
language matches the proved surface.

### Production Delta

Source-of-truth closeout and support-tier wording only when behavior receipts
exist.

### Proof Commands

```bash
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Revert the closeout PR if it overstates behavior or support-tier status.

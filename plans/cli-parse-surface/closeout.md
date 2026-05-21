# CLI Parse Surface Hardening Closeout

Status: complete
Owner: cli/product
Created: 2026-05-21
Closed: 2026-05-21
Linked proposal: ../../docs/proposals/ADZE-PROP-0013-cli-parse-surface-hardening.md
Linked plan: ./implementation-plan.md
Linked goal: ../../.adze/goals/cli-parse-surface-hardening.toml
Linked issue: EffortlessMetrics/adze-swarm#325
Linked PRs:
- EffortlessMetrics/adze-swarm#457
- EffortlessMetrics/adze-swarm#458
- EffortlessMetrics/adze-swarm#459

## Summary

The CLI parse-surface lane is complete.

PR #457 opened the non-release source-of-truth lane after parser/runtime
maintainability closed out. It archived the completed parser/runtime
maintainability active manifest, registered `ADZE-PROP-0013`, and made
`static-cli-selected-tree-output` the one ready work item.

PR #458 implemented that ready item. Default static
`adze parse <grammar.rs> <input>` now compiles the temporary generated
single-grammar runner, calls generated `parse_document()`, reads
`document-json`, and renders a human selected-tree receipt from document facts.

## Behavior Now Covered

- Default `tree` output is useful for single-file generated grammar smoke
  checks.
- `document-json`, `tree-json`, `diagnostics-json`, and `ambiguity-json` still
  emit schema-tagged document projection envelopes.
- Malformed-input document JSON still carries recovery diagnostics.
- `json`, `sexp`, and `dot` remain explicitly unsupported static output modes.
- Dynamic parse output remains unimplemented and experimental.
- CLI output remains Stabilizing. This lane did not promote a stable CLI/WASM
  schema contract.

## Proof Receipts

Local proof from #458:

```bash
cargo fmt -p adze-cli -- --check
cargo test -p adze-cli test_parse_static_tree_mode_emits_document_backed_tree -- --exact --nocapture
cargo test -p adze-cli test_parse_static_non_document_modes_are_explicitly_unimplemented -- --exact --nocapture
cargo test -p adze-cli test_parse_document_projection_modes_emit_schema_envelopes -- --exact --nocapture
cargo test -p adze-cli parse_document_json_modes_emit_recovery_diagnostics -- --exact --nocapture
cargo clippy -p adze-cli --all-targets -- -D warnings
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

GitHub proof from #458:

```text
Rust Small Result: success
Product Proof Result: success
Source of Truth: success
Supported Rust Gate: success
PR Gate Success: success
Test Pure Rust Implementation (ubuntu-latest, stable): success
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

Public `adze` remains the release, public-intake, promotion, tag, publish,
signing, and Cargo-token surface. `adze-swarm` remains the implementation and
proof repo.

## Remaining Non-Release Gaps

No ready CLI parse-surface work remains in this lane. Future CLI work should
open a fresh active goal or reactivate this lane only for a specific material
gap, such as dynamic parse output, stable schema promotion, or additional
static output formats.

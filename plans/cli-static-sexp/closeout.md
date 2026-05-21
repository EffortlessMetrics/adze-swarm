# CLI Static S-Expression Output Closeout

Status: complete
Owner: cli/product
Created: 2026-05-21
Closed: 2026-05-21
Linked proposal: ../../docs/proposals/ADZE-PROP-0014-cli-static-sexpression-output.md
Linked plan: ./implementation-plan.md
Linked goal: ../../.adze/goals/cli-static-sexp-output.toml
Linked issue: EffortlessMetrics/adze-swarm#325
Linked PRs:
- EffortlessMetrics/adze-swarm#461
- EffortlessMetrics/adze-swarm#462
- EffortlessMetrics/adze-swarm#463

## Summary

The CLI static S-expression lane is complete.

PR #461 opened the non-release source-of-truth lane and made
`static-cli-sexpression-output` the one ready work item.

PR #462 implemented that ready item. Static
`adze parse --output sexp <grammar.rs> <input>` now compiles the temporary
generated single-grammar runner, calls generated `parse_document()`, reads
`document-json`, and renders a selected-tree S-expression from document facts.

## Behavior Now Covered

- Static `sexp` output is useful for single-file generated grammar smoke
  checks.
- Default `tree` output remains document-backed and unchanged.
- `document-json`, `tree-json`, `diagnostics-json`, and `ambiguity-json` still
  emit schema-tagged document projection envelopes.
- Malformed-input document JSON still carries recovery diagnostics.
- `json` and `dot` remain explicitly unsupported static output modes.
- Dynamic parse output remains unimplemented and experimental.
- CLI output remains Stabilizing. This lane did not promote a stable CLI/WASM
  schema contract.

## Proof Receipts

Local proof from #462:

```bash
cargo test -p adze-cli test_parse_static_sexp_mode_emits_document_backed_sexp -- --exact --nocapture
cargo test -p adze-cli test_parse_static_non_document_modes_are_explicitly_unimplemented -- --exact --nocapture
cargo test -p adze-cli test_parse_static_tree_mode_emits_document_backed_tree -- --exact --nocapture
cargo test -p adze-cli test_parse_document_projection_modes_emit_schema_envelopes -- --exact --nocapture
cargo test -p adze-cli parse_document_json_modes_emit_recovery_diagnostics -- --exact --nocapture
cargo fmt -p adze-cli -- --check
cargo clippy -p adze-cli --all-targets -- -D warnings
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

GitHub proof from #462:

```text
Rust Small Result: success
Product Proof Result: success
Source of Truth: success
Supported Rust Gate: success
PR Gate Success: success
Test Pure Rust Implementation (ubuntu-latest, stable): success
Test Runtime Crates: success
ci-product stable canaries: success
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

No ready CLI static S-expression work remains in this lane. Future CLI work
should open a fresh active goal only for a specific material gap, such as
dynamic parse output, stable schema promotion, or additional static output
formats.

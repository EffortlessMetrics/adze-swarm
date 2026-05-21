# CLI Static JSON And DOT Output Closeout

Status: complete
Owner: cli/product
Created: 2026-05-21
Closed: 2026-05-21
Linked proposal: ../../docs/proposals/ADZE-PROP-0015-cli-static-json-dot-output.md
Linked plan: ./implementation-plan.md
Linked goal: ../../.adze/goals/cli-static-json-dot-output.toml
Linked issue: EffortlessMetrics/adze-swarm#325
Linked PRs:
- EffortlessMetrics/adze-swarm#464
- EffortlessMetrics/adze-swarm#465
- EffortlessMetrics/adze-swarm#466

## Summary

The CLI static JSON and DOT output lane is complete.

PR #464 opened the non-release source-of-truth lane and made
`static-cli-json-dot-output` the one ready work item.

PR #465 implemented that ready item. Static
`adze parse --output json <grammar.rs> <input>` now compiles the temporary
generated single-grammar runner, calls generated `parse_document()`, and emits
the generated document JSON. Static `adze parse --output dot <grammar.rs>
<input>` uses the same document JSON and renders a selected-tree Graphviz graph
from document facts.

## Behavior Now Covered

- Static `json` output is useful for single-file generated grammar smoke
  checks and aliases the generated document JSON path.
- Static `dot` output renders a document-backed selected-tree graph.
- Default `tree` output remains document-backed and unchanged.
- Static `sexp` output remains document-backed and unchanged.
- `document-json`, `tree-json`, `diagnostics-json`, and `ambiguity-json` still
  emit schema-tagged document projection envelopes.
- Malformed-input document JSON still carries recovery diagnostics.
- Dynamic parse output remains unimplemented and experimental.
- CLI output remains Stabilizing. This lane did not promote a stable CLI/WASM
  schema contract.

## Proof Receipts

Local proof from #465:

```bash
cargo test -p adze-cli test_parse_static_json_mode_emits_document_json -- --exact --nocapture
cargo test -p adze-cli test_parse_static_dot_mode_emits_document_backed_graph -- --exact --nocapture
cargo test -p adze-cli test_parse_static_tree_mode_emits_document_backed_tree -- --exact --nocapture
cargo test -p adze-cli test_parse_static_sexp_mode_emits_document_backed_sexp -- --exact --nocapture
cargo test -p adze-cli test_parse_document_projection_modes_emit_schema_envelopes -- --exact --nocapture
cargo test -p adze-cli parse_document_json_modes_emit_recovery_diagnostics -- --exact --nocapture
cargo fmt -p adze-cli -- --check
cargo clippy -p adze-cli --all-targets -- -D warnings
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

GitHub proof from #465:

```text
Rust Small Result: success
Product Proof Result: success
Source of Truth: success
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

No ready CLI static JSON/DOT output work remains in this lane. Future CLI work
should open a fresh active goal only for a specific material gap, such as
dynamic parse output, stable schema promotion, or additional CLI projection
hardening.

# ADZE-PROP-0015: CLI Static JSON And DOT Output

Status: accepted
Owner: cli/product
Created: 2026-05-21
Target milestone: post-0.9 / non-release CLI hardening
Linked specs:
- docs/specs/ADZE-SPEC-0008-json-cli-wasm-projections.md
- docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADRs:
- ADZE-ADR-0001-adze-document-one-parse-truth
- ADZE-ADR-0004-schema-versioned-projections
Linked plan:
- ../../plans/cli-static-json-dot/implementation-plan.md
Linked issues:
- EffortlessMetrics/adze-swarm#325
Linked PRs:
- EffortlessMetrics/adze-swarm#464
Support-tier impact:
- No support-tier promotion by campaign setup.
Policy impact:
- Keeps CLI output hardening in `EffortlessMetrics/adze-swarm`.
- Keeps release, tag, publish, signing, Cargo-token, and crates.io install
  receipt work in public `EffortlessMetrics/adze` after explicit
  authorization.

## Problem

The checked-out CLI now has document-backed `tree`, `sexp`, and explicit
document projection modes. The remaining static output choices exposed by
`adze parse` are `json` and `dot`, and both still fail as unsupported static
modes.

That is honest but awkward. A user trying a visible output mode should either
receive a useful document-backed receipt or a clear boundary. The document has
enough facts to support both a generic JSON alias and a DOT selected-tree graph
without creating a second parse truth.

## Success Criteria

- `adze-swarm` remains the operating repo for CLI implementation and proof.
- Static `adze parse --output json` emits the same document-backed JSON as the
  explicit `document-json` projection.
- Static `adze parse --output dot` emits a Graphviz DOT selected-tree graph
  from document facts.
- Existing `tree`, `sexp`, and document projection modes keep their receipts.
- CLI support tiers continue to label these outputs as Stabilizing, not Stable.
- Release/publish blockers remain tracked on issue #325 and are not completed
  by local CLI hardening.

## Proposed Shape

Use the existing temporary generated parser runner and `parse_document()` path.

```text
generated parse_document()
  -> document-json
  -> json alias or selected-tree DOT graph
```

## Non-Goals

- No release tag, crate publish, signing, Cargo-token, or release workflow work.
- No `cargo install adze-cli` claim until a real crates.io install receipt
  exists.
- No public `adze` implementation PRs.
- No dynamic parse output implementation.
- No full Tree-sitter CLI parity claim.
- No stable CLI/WASM schema claim.

## Evidence Plan

- Source-of-truth proof:
  - `cargo run -q -p xtask -- check-active-goal --mode blocking`
  - `cargo run -q -p xtask -- check-doc-artifacts --mode blocking`
- CLI proof:
  - focused `adze-cli` static JSON and DOT tests
  - existing selected-tree/document projection tests
- Hygiene:
  - `cargo fmt -p adze-cli -- --check`
  - `cargo clippy -p adze-cli --all-targets -- -D warnings`
  - `git diff --check`

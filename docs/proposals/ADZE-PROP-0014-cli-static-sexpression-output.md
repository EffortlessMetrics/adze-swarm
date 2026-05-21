# ADZE-PROP-0014: CLI Static S-Expression Output

Status: implemented
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
- ../../plans/cli-static-sexp/implementation-plan.md
Linked issues:
- EffortlessMetrics/adze-swarm#325
Linked PRs:
- EffortlessMetrics/adze-swarm#461
- EffortlessMetrics/adze-swarm#462
- EffortlessMetrics/adze-swarm#463
Support-tier impact:
- No support-tier promotion by campaign setup.
Policy impact:
- Keeps CLI output hardening in `EffortlessMetrics/adze-swarm`.
- Keeps release, tag, publish, signing, Cargo-token, and crates.io install
  receipt work in public `EffortlessMetrics/adze` after explicit
  authorization.

## Problem

The checked-out CLI now has a useful default static selected-tree output backed
by generated `parse_document()`, and document projection modes emit schema
envelopes. However, `adze parse --output sexp` is still exposed as an output
choice while failing as an explicitly unimplemented static mode.

That is honest, but it is no longer the best product behavior. A Tree-sitter
or tooling user expects an S-expression view to be the compact selected-tree
receipt, and Adze already has the document facts needed to render one without
creating a second parse truth.

## Users And Surfaces

- New users need one compact CLI tree output that is easy to compare in tests.
- Tree-sitter users expect S-expression output to represent selected-tree
  structure.
- Maintainers need CLI output to stay document-backed and support-tier bounded.
- Release reviewers need local CLI hardening to remain separate from crates.io
  install and publish claims.

## Success Criteria

- `adze-swarm` remains the operating repo for CLI implementation and proof.
- Static `adze parse --output sexp <grammar.rs> <input>` is backed by generated
  `parse_document()`.
- The S-expression output uses selected document tree facts and does not run a
  separate parser.
- Existing default `tree` output and document projection modes keep their
  receipts.
- `json` and `dot` remain explicit unsupported static modes until behavior and
  proof land.
- CLI support tiers continue to label the output as Stabilizing, not Stable.
- Release/publish blockers remain tracked on issue #325 and are not completed
  by local CLI hardening.

## Proposed Shape

Use the existing static parse runner to produce `document-json`, then render a
selected-tree S-expression from the same document tree facts used by the
human-readable tree output.

```text
generated parse_document()
  -> document-json
  -> selected document tree
  -> CLI S-expression
```

## Alternatives Considered

### Leave `sexp` Unsupported

Rejected. The mode remains visible in help and is a small, useful selected-tree
projection once document-backed output exists.

### Implement A Dynamic Parser Path

Rejected for this lane. Dynamic parse remains experimental and is not needed
for checked-out single-file grammar smoke output.

### Claim Stable CLI Schema Compatibility

Rejected. This lane adds a useful receipt format, but it does not promote CLI
or WASM schemas to Stable.

## Specs To Create Or Update

No new behavior spec is required. `ADZE-SPEC-0008` owns projection boundaries,
and `ADZE-SPEC-0011` owns support-tier proof boundaries.

Update support-tier wording only after behavior receipts exist.

## Architecture Decisions Needed

No new ADR is required. The durable constraints remain:

- `AdzeDocument` is the one parse truth.
- CLI output is a projection over document facts.

## Implementation Campaign Shape

1. Start the CLI static S-expression active goal.
2. Implement static `sexp` output through generated `parse_document()`.
3. Refresh support-tier/docs wording and close the lane after behavior proof
   exists.

## Evidence Plan

- Source-of-truth proof:
  - `cargo run -q -p xtask -- check-active-goal --mode blocking`
  - `cargo run -q -p xtask -- check-doc-artifacts --mode blocking`
- CLI proof:
  - focused `adze-cli` static S-expression test
  - existing selected-tree/document projection tests
- Hygiene:
  - `cargo fmt -p adze-cli -- --check`
  - `git diff --check`

## Risks

- S-expression output can imply full Tree-sitter CLI parity. Keep docs scoped
  to selected document tree output.
- Rendering can drift from document facts. Reuse `document-json` and selected
  document nodes rather than introducing another parse path.
- Agents can confuse local CLI hardening with release install proof. Keep #325
  as the release authorization checkpoint.

## Non-Goals

- No release tag, crate publish, signing, Cargo-token, or release workflow
  work.
- No `cargo install adze-cli` claim until a real crates.io install receipt
  exists.
- No public `adze` implementation PRs.
- No dynamic parse output implementation.
- No `json` or `dot` static output implementation.
- No full Tree-sitter CLI parity claim.
- No stable CLI/WASM schema claim.

## Exit Criteria

This campaign is complete when static S-expression output has focused behavior
receipts, remaining unsupported modes fail explicitly, support-tier language
matches the implemented surface, and release-only work remains blocked on #325.

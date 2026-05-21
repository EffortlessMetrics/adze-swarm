# ADZE-PROP-0013: CLI Parse Surface Hardening

Status: accepted
Owner: cli/product
Created: 2026-05-21
Target milestone: post-0.9 / non-release CLI hardening
Linked specs:
- docs/specs/ADZE-SPEC-0008-json-cli-wasm-projections.md
- docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
- docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
Linked ADRs:
- ADZE-ADR-0001-adze-document-one-parse-truth
- ADZE-ADR-0004-schema-versioned-projections
Linked plan:
- ../../plans/cli-parse-surface/implementation-plan.md
Linked issues:
- EffortlessMetrics/adze-swarm#325
Linked PRs:
- EffortlessMetrics/adze-swarm#457
- EffortlessMetrics/adze-swarm#458
Support-tier impact:
- No support-tier promotion by campaign setup.
Policy impact:
- Keeps CLI parse hardening in `EffortlessMetrics/adze-swarm`.
- Keeps release, tag, publish, signing, Cargo-token, and crates.io install
  receipt work in public `EffortlessMetrics/adze` after explicit
  authorization.

## Problem

The core generated-parser and document paths are proof-backed, but the user
facing `adze parse` surface is still uneven. Document projection modes
(`document-json`, `tree-json`, `diagnostics-json`, and `ambiguity-json`) are
implemented through `parse_document()`, while ordinary non-document static
parse modes still report that static parse is unimplemented.

That boundary is honest, but it is no longer the best product experience. A
developer who can run `adze init`, `cargo test`, and `grammar::parse(...)`
should be able to use `adze parse` for at least one selected-tree receipt
without learning internal parser constructors or release-only install details.

## Users And Surfaces

- New users need `adze parse` to provide a useful selected-tree smoke path from
  a checked-out CLI.
- Tooling users need CLI output to stay a projection over `AdzeDocument`, not a
  second parse truth.
- Maintainers need support-tier wording to stay honest while the CLI surface is
  hardened incrementally.
- Release reviewers need this work to avoid tag, publish, signing, Cargo-token,
  and crates.io install claims.

## Success Criteria

- `adze-swarm` remains the operating repo for CLI parse-surface hardening.
- Static CLI selected-tree output is backed by generated `parse_document()`.
- Existing document projection modes keep their schema envelopes and recovery
  diagnostics receipts.
- Unsupported output modes fail explicitly until implemented.
- CLI support tiers move only when behavior, proof commands, and known
  limitations are updated together.
- Release/publish blockers remain tracked on issue #325 and are not treated as
  completed by local CLI hardening.

## Proposed Shape

Use this lane for small CLI parse-surface PRs:

```text
source-of-truth setup
  -> static selected-tree output
    -> optional static JSON/S-expression output
      -> support-tier and docs closeout
```

Every behavior PR should prefer the existing temporary generated parser runner
and `parse_document()` so CLI output remains a projection of the canonical
document.

## Alternatives Considered

### Wait For Release

Rejected. Published `cargo install adze-cli` still needs release authorization,
but local CLI parse behavior can improve without touching public release
machinery.

### Claim The CLI Is Stable

Rejected. This lane adds proof-backed behavior but does not promote CLI or
schema stability by default.

### Implement A Separate CLI Parser

Rejected. That would create a second parse truth. CLI output should flow from
generated parser functions and `AdzeDocument`.

## Specs To Create Or Update

No new behavior spec is required at campaign start. Existing specs remain
authoritative:

- `ADZE-SPEC-0008` for serialized projection behavior.
- `ADZE-SPEC-0011` for product proof and support-tier boundaries.
- `ADZE-SPEC-0012` for the GLR toolkit product contract.

Update specs only if this lane changes the serialized projection contract.

## Architecture Decisions Needed

No new ADR is required. The durable constraints remain:

- `AdzeDocument` is the one parse truth.
- CLI/JSON/WASM surfaces are projections over document facts.

## Implementation Campaign Shape

1. Start the CLI parse-surface active goal.
2. Keep release authorization blocked on #325.
3. Add a static selected-tree CLI output path backed by `parse_document()`.
4. Add any further output modes only when their schema and support-tier
   boundaries are clear.
5. Close the lane with support-tier and docs updates only after behavior
   receipts exist.

## Evidence Plan

- Source-of-truth proof:
  - `cargo run -q -p xtask -- check-active-goal --mode blocking`
  - `cargo run -q -p xtask -- check-doc-artifacts --mode blocking`
- CLI proof:
  - `cargo test -p adze-cli test_parse_document_projection_modes_emit_schema_envelopes -- --exact --nocapture`
  - focused new `adze-cli` parse-output tests per work item
- Hygiene:
  - `git diff --check`

## Risks

- CLI output can accidentally imply a Stable schema. Keep support-tier language
  explicit until promotion evidence exists.
- Static parse output can drift away from `parse_document()`. Route behavior
  through document facts.
- Agents can confuse CLI hardening with release install proof. Keep #325 as the
  explicit release authorization checkpoint.

## Non-Goals

- No release tag, crate publish, signing, Cargo-token, or release workflow work.
- No `cargo install adze-cli` claim until a real crates.io install receipt
  exists.
- No public `adze` implementation PRs.
- No support-tier promotion by campaign setup.
- No full Tree-sitter CLI parity claim.
- No stable CLI/WASM schema claim.

## Exit Criteria

This campaign is complete when the selected static CLI parse outputs have
focused behavior receipts, unsupported modes remain explicit, support-tier
language matches the implemented surface, and release-only work remains blocked
on #325.

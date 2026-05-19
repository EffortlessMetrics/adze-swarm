# ADZE-PROP-0004: Toolkit Excellence And Adoption

Status: accepted
Owner: runtime/product
Created: 2026-05-18
Target milestone: post-0.9 / adoption readiness
Linked specs:
- docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
- docs/specs/ADZE-SPEC-0013-query-compatibility.md
- docs/specs/ADZE-SPEC-0014-performance-and-regression.md
Linked ADRs:
- ADZE-ADR-0001-adze-document-one-parse-truth
- ADZE-ADR-0003-summary-first-glr-ambiguity
- ADZE-ADR-0004-schema-versioned-projections
Linked plan:
- ../../plans/toolkit-excellence/implementation-plan.md
Linked issues:
- none yet
Linked PRs:
- none yet
Support-tier impact:
- Defines the adoption-readiness proof sequence for existing GLR toolkit surfaces.
- Does not promote any surface by itself.
Policy impact:
- Keeps swarm work in `EffortlessMetrics/adze-swarm`.
- Uses support tiers as the only source for product claim promotion.

## Problem

The GLR toolkit productization campaign delivered the parser foundation:
starter project proof, `AdzeDocument` projections, GLR conflict matrices,
Tree-sitter selected-tree parity, query subset proof, diagnostics and recovery
matrices, benchmark fixtures, and support-tier receipts.

That does not automatically make Adze easy to adopt. A user still needs one
boring path from install to a working parser, one obvious API choice ladder, and
examples that connect typed parsing, document facts, diagnostics, ambiguity,
Tree-sitter-shaped traversal, queries, JSON, and performance receipts.

This campaign turns the completed foundation into a cohesive product surface.

## Users And Surfaces

- New Rust users need `adze init`, `cargo test`, and a parse example to work
  without learning internals.
- Library authors need `grammar::parse(source)` to remain the default public
  story.
- Tooling authors need `grammar::parse_document(source)` and `AdzeDocument`
  projections to be discoverable and documented.
- Tree-sitter adopters need a selected-tree compatibility matrix, examples, and
  known gaps.
- Query users need examples for the supported subset and clear failure behavior
  for unsupported or source-free cases.
- Maintainers and agents need PR-sized work items with proof commands, claim
  boundaries, and support-tier impact.

## Success Criteria

- A new user can initialize, build, test, and parse through one documented path.
- Beginner docs teach generated parser APIs first: `grammar::parse` and
  `grammar::parse_document`.
- Advanced docs teach `AdzeDocument` as the tooling boundary and every other
  view as a projection.
- The product acceptance matrix maps each user workflow to repeatable proof.
- GLR ambiguity, diagnostics, Tree-sitter compatibility, query, JSON, CLI, WASM,
  and performance surfaces have examples or receipts before stronger claims.
- Public README, book, and reference docs do not exceed support-tier proof.
- Public `EffortlessMetrics/adze` remains release and public-intake surface;
  swarm implementation work stays in `EffortlessMetrics/adze-swarm`.

## Proposed Shape

The product ladder should be:

```text
Beginner:
  adze init
  grammar::parse(source)

Tooling:
  grammar::parse_document(source)
  AdzeDocument diagnostics, syntax, JSON, and ambiguity summaries

Compatibility:
  document.as_tree_sitter()
  documented query subset
  node-types and selected-tree receipts

Evidence:
  acceptance matrix
  downstream starter fixture
  benchmark receipts
  support-tier proof rows
```

The campaign is deliberately not a runtime rewrite. It should make existing
work usable, tested end to end, and honest about limits before adding broader
claims.

## Alternatives Considered

### Continue Product Work Without A New Campaign

Rejected. The previous active manifest is complete. Continuing without a new
active goal encourages duplicate PRs and chat-derived intent.

### Promote Existing Surfaces Immediately

Rejected. Support tiers own claim promotion. This campaign can create receipts,
but it must not upgrade user-facing claims until the proof map and docs agree.

### Focus On More Internals First

Rejected for this lane. Refactors remain useful when they support starter
project, document, diagnostics, query, Tree-sitter, examples, or receipt work.
Random cleanup should not outrank product acceptance.

## Specs To Create Or Update

No new durable behavior spec is required at campaign start. Existing contracts
remain authoritative:

- `ADZE-SPEC-0012` for GLR toolkit product behavior.
- `ADZE-SPEC-0013` for query compatibility.
- `ADZE-SPEC-0014` for performance and regression evidence.
- `ADZE-SPEC-0011` and `docs/status/SUPPORT_TIERS.md` for product claim proof.

Add narrow specs later only if the acceptance matrix exposes new behavior,
not merely documentation gaps.

## Architecture Decisions Needed

No new ADR is required at campaign start.

The durable rules remain:

- `AdzeDocument` is the canonical parse truth.
- GLR ambiguity is summary-first.
- Serialized projections are schema-versioned.
- Tree-sitter compatibility and query behavior are documented subsets until
  proof justifies stronger claims.

## Implementation Campaign Shape

The campaign starts with:

1. Activate the toolkit excellence goal.
2. Define the product acceptance matrix.
3. Harden the generated starter and downstream starter fixture.
4. Align README, book, quickstart, and API-choice docs with tested behavior.
5. Add runnable GLR ambiguity, query/highlighting, diagnostics, and recovery
   examples.
6. Publish a Tree-sitter compatibility matrix for the selected-tree subset.
7. Add performance receipt commands and baseline receipts.
8. Promote only proven support-tier slices.

## Evidence Plan

- Source-of-truth proof:
  - `cargo run -q -p xtask -- check-doc-artifacts --mode blocking`
  - `cargo run -q -p xtask -- check-active-goal --mode blocking`
- Starter proof:
  - `cargo test -p adze-cli test_init_default_cwd_generates_buildable_project -- --exact --nocapture`
  - `cargo test -p adze-cli test_init_generates_buildable_project -- --exact --nocapture`
  - `cargo test -p adze-cli getting_started_quickstart_builds_parses_and_reports_diagnostics -- --exact --nocapture`
- Product workflow proof:
  - downstream starter crate build, test, and parse example
  - examples for ambiguity, query, diagnostics, and recovery
- Compatibility proof:
  - selected-tree compatibility matrix
  - query subset examples and known gaps
- Performance proof:
  - benchmark receipt command and baselines, without default PR gating
- Claim proof:
  - support-tier rows, limitations, README/book wording, and proof commands

## Risks

- Docs can overclaim because the code exists. Keep every claim tied to support
  tiers and proof commands.
- Duplicate PRs can reappear. Agents must inspect the open `adze-swarm` queue
  before opening a new work item.
- Public `adze` can drift again. Wrong-target swarm PRs should be closed or
  ported into `adze-swarm`.
- Performance receipts can become marketing claims. Treat them as evidence
  until support tiers explicitly promote a guarantee.

## Non-Goals

- No full Tree-sitter compatibility claim.
- No full query parity claim.
- No stable full GLR forest export.
- No stable incremental reuse or speedup claim.
- No default benchmark or coverage-heavy required gate.
- No new swarm implementation PRs against public `EffortlessMetrics/adze`.

## Exit Criteria

This campaign is complete when:

- the acceptance matrix maps user workflows to proof commands;
- the starter path and downstream fixture prove install/init/test/parse behavior;
- API-choice, Tree-sitter, query, GLR ambiguity, diagnostics, recovery, and
  performance docs are aligned with runnable examples or receipts;
- support tiers promote only proven slices; and
- public `adze` remains release/intake while swarm work stays in
  `EffortlessMetrics/adze-swarm`.

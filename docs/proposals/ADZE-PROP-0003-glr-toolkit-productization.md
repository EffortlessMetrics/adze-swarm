# ADZE-PROP-0003: GLR Toolkit Productization

Status: implemented
Owner: runtime/product
Created: 2026-05-17
Target milestone: post-0.9 / 1.0 foundation
Linked specs:
- docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
- docs/specs/ADZE-SPEC-0013-query-compatibility.md
- docs/specs/ADZE-SPEC-0014-performance-and-regression.md
Linked ADRs:
- ADZE-ADR-0001-adze-document-one-parse-truth
- ADZE-ADR-0003-summary-first-glr-ambiguity
- ADZE-ADR-0004-schema-versioned-projections
Linked plan:
- ../../plans/glr-toolkit/productization-plan.md
Linked issues:
- none yet
Linked PRs:
- EffortlessMetrics/adze-swarm#124-#155
Support-tier impact:
- Defines the promotion path for GLR, Tree-sitter compatibility, query, diagnostics, recovery, performance, and first-use claims.
- Does not promote any surface by itself.
Policy impact:
- Added a post-0.9 productization campaign and proof map expectations.
- Recorded fixture, benchmark, recovery, compatibility, and support-tier proof receipts.

## Problem

Adze now has the 0.9 API foundation: `AdzeDocument` is specified as the
canonical parse product, and typed CST, typed AST, diagnostics, GLR ambiguity,
Tree-sitter compatibility, JSON, CLI, and WASM are specified as projections from
one parse truth.

That foundation is necessary, but it is not yet a complete product story. A new
user should have one obvious path from install to a working parser. A tooling or
Tree-sitter user should see the supported compatibility subset without reading
implementation history. Maintainers and agents should be able to follow a
fixture, proof command, support-tier row, and plan item for each public claim.

## Users And Surfaces

- Typed parser users need the boring path: obtain the CLI, run `adze init`,
  run `cargo test`, then call `grammar::parse(input)` to get typed Rust values.
- Language and tooling users need `parse_document()` with diagnostics, ranges,
  fields, JSON, typed CST/AST projections, and ambiguity summaries.
- Editor and Tree-sitter users need selected-tree output, node metadata, fields,
  S-expressions, node-types, and query behavior for a documented subset.
- Grammar authors need conflict, ambiguity, diagnostics, recovery, and
  performance evidence tied to generated grammars.
- Maintainers and agents need fixture taxonomy, proof commands, support-tier
  rows, and active work items instead of chat-derived intent.

## Success Criteria

- A new user can run the generated starter flow, build and test it, and run
  the parse example. Until `adze-cli` is published, the proven command starts
  from the repo-built CLI; `cargo install adze-cli` is a release-surface target
  that needs an install receipt.
- The README links to one beginner path and one mental model instead of forcing
  users through internal architecture docs.
- GLR conflict handling has generated matrix coverage for shift/reduce,
  reduce/reduce, nested fork, multi-conflict expression, dangling-else, and
  ambiguous-list grammars.
- Selected-tree behavior is deterministic and documented.
- Ambiguity summaries report native document facts without making full forest
  export a stable default.
- `AdzeDocument` remains the canonical parse product for typed CST, typed AST,
  diagnostics, Tree-sitter compatibility, JSON, CLI, and WASM projections.
- Tree-sitter-compatible output is proven for a documented selected-tree subset.
- Query compatibility has a documented supported subset and source-aware
  predicate behavior.
- Diagnostics and recovery have bad-input matrix proof across byte spans,
  point ranges, UTF-8, EOF, missing nodes, and ambiguity.
- Benchmarks and advisory regression receipts measure parse, document,
  projection, query, diagnostics, and table decode paths.
- `docs/status/SUPPORT_TIERS.md` promotes only claims backed by proof commands.

## Proposed Shape

The product promise should be simple:

```text
Adze lets Rust developers define a grammar as Rust types,
generate a parser at build time,
parse into typed Rust values,
and optionally inspect the same parse through document, CST,
Tree-sitter-compatible, JSON, diagnostic, query, and ambiguity projections.
```

The internal rule remains:

```text
source
  -> parser runtime
  -> AdzeDocument
      -> generic CST
      -> generated typed CST
      -> typed AST lowering
      -> diagnostics
      -> ambiguity summaries / optional forest
      -> Tree-sitter-compatible selected-tree adapter
      -> query cursor subset
      -> JSON / CLI / WASM projections
```

The campaign is organized as:

1. Define the user-facing GLR toolkit product contract.
2. Make first use trivial through `adze init`, a quickstart, and a mental model.
3. Classify GLR, recovery, Tree-sitter compatibility, and query fixtures.
4. Build a projection equivalence harness over `AdzeDocument`.
5. Prove GLR conflict routing and tablegen conflict-cell ABI behavior.
6. Prove Tree-sitter-compatible selected-tree output and node-types metadata.
7. Define and implement the supported query subset.
8. Expand diagnostics and recovery proof.
9. Add honest incremental lifecycle and performance contracts.
10. Promote only the proven support-tier slices.

## Alternatives Considered

### Continue With Scattered Canaries

Rejected. Existing canaries are valuable, but users and agents need a coherent
fixture taxonomy and product contract that explains which slices are proven.

### Tree-sitter First

Rejected. Tree-sitter compatibility is an adoption adapter. It must project from
`AdzeDocument`, not define Adze's native parse product.

### Full Forest First

Rejected. Full forest exposure is important future work, but the default product
should expose selected tree plus ambiguity summaries before stabilizing a raw
forest API.

### Promote Existing Surfaces Before Proof

Rejected. A claim becomes stable or stabilizing only when support tiers map it
to a proof command and known limitations.

## Specs To Create Or Update

- `ADZE-SPEC-0012-glr-toolkit-product-contract`
- `ADZE-SPEC-0013-query-compatibility`
- `ADZE-SPEC-0014-performance-and-regression`
- Update `ADZE-SPEC-0006-tree-sitter-compatibility-adapter` if selected-tree
  parity proof changes the supported subset.
- Update `ADZE-SPEC-0007-glr-ambiguity-summary` if ambiguity summary shape
  changes.
- Update `ADZE-SPEC-0009-incremental-document-lifecycle` if fallback metadata
  moves from proposed to accepted behavior.

## Architecture Decisions Needed

No new durable ADR is required at campaign start. The campaign is governed by:

- `ADZE-ADR-0001`: `AdzeDocument` is one parse truth.
- `ADZE-ADR-0003`: GLR ambiguity is summary-first.
- `ADZE-ADR-0004`: serialized projections are schema-versioned.

If query semantics, incremental lifecycle, or performance policy reveals a
durable architecture decision, add a narrow ADR rather than burying it in an
implementation PR.

## Implementation Campaign Shape

The completed queue lives in `../../.adze/goals/active.toml`; the durable plan
lives in `../../plans/glr-toolkit/productization-plan.md`.

The first work item already landed in `EffortlessMetrics/adze-swarm#124`: public
`adze` drift for Rust guard checks was synced back into `adze-swarm`. All future
work in this campaign targets `EffortlessMetrics/adze-swarm`.

The campaign completed through `EffortlessMetrics/adze-swarm#155`.

## Evidence Plan

- Source-of-truth proof:
  - `cargo run -q -p xtask -- check-doc-artifacts --mode blocking`
  - `cargo run -q -p xtask -- check-active-goal --mode blocking`
- First-use proof:
  - `adze init` generates a buildable starter project
  - generated quickstart tests parse valid input and report diagnostics for bad input
- GLR proof:
  - generated conflict matrix
  - GOTO and symbol-indexing invariant checks
  - tablegen conflict-cell ABI roundtrip
- Projection proof:
  - document, generic CST, typed CST, typed AST, Tree-sitter-compatible tree,
    diagnostics, ambiguity summary, and JSON equivalence checks
- Compatibility proof:
  - selected-tree Tree-sitter API parity matrix
  - node-types metadata snapshots
  - query subset matrix and differential supported-subset corpus
- Recovery proof:
  - invalid token, EOF, missing close delimiter, bad separator, UTF-8,
    multiline, ambiguous error, and external scanner recovery matrix
- Performance proof:
  - advisory benchmark baselines for parse, document, projection, query,
    diagnostics, table decode, and codegen paths

## Risks

- Compatibility scope can expand faster than proof. Keep Tree-sitter and query
  claims subset-based until fixtures prove them.
- GLR ambiguity can become expensive if full forest data becomes default. Keep
  summary-first behavior as the native contract.
- The first-use path can drift if examples are not tested. Starter projects and
  docs examples need proof commands.
- Support-tier docs can become stale if proof commands move. Promote claims only
  in the same PR family that creates or updates receipts.
- Public `adze` can drift again if agents ignore the swarm target. The plan and
  goal manifests point new work at `EffortlessMetrics/adze-swarm`.

## Non-Goals

- No full Tree-sitter query parity claim.
- No stable full forest API.
- No stable incremental reuse guarantee.
- No default benchmark or coverage-heavy gate for ordinary PRs.
- No promotion of advisory/experimental surfaces without support-tier proof.
- No new swarm PRs against public `EffortlessMetrics/adze`.

## Closeout

Implemented: 2026-05-17

The campaign delivered the product contract, first-use path, GLR/document/query
fixture proof, Tree-sitter selected-tree and node-types receipts, recovery and
diagnostic matrices, incremental fallback metadata, benchmark proof receipts,
and support-tier proof mapping without claiming full Tree-sitter parity, stable
full-forest export, stable incremental reuse, or blocking performance
thresholds.

## Exit Criteria

This proposal is complete when:

- the GLR toolkit product contract is accepted;
- first-use onboarding has tested CLI and docs paths;
- fixture taxonomy and projection equivalence harnesses exist;
- GLR conflict, tablegen ABI, Tree-sitter selected-tree, query subset,
  diagnostics/recovery, and performance proof are represented by commands;
- support tiers are updated only for proven slices; and
- future agents can reconstruct what shipped and choose the next campaign from
  the closeout without chat context.

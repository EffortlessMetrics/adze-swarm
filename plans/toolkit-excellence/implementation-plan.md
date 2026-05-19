# Toolkit Excellence And Adoption Plan

Status: complete
Owner: runtime/product
Created: 2026-05-18
Linked proposal: ../../docs/proposals/ADZE-PROP-0004-toolkit-excellence.md
Linked specs:
- ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
- ../../docs/specs/ADZE-SPEC-0013-query-compatibility.md
- ../../docs/specs/ADZE-SPEC-0014-performance-and-regression.md
Linked ADRs:
- ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
- ../../docs/adr/ADZE-ADR-0003-summary-first-glr-ambiguity.md
- ../../docs/adr/ADZE-ADR-0004-schema-versioned-projections.md
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/toolkit-excellence.toml
Support-tier map: ../../docs/status/SUPPORT_TIERS.md

## Goal

Convert the completed GLR toolkit foundation into a cohesive, easy-to-use
developer product. The work should prove complete user workflows, align docs
with code, add examples and receipts, and promote only support-tier-backed
claims.

This plan sequences PR-sized work. It does not define new parser behavior by
itself and does not promote any support tier.

## Operating Rules

- Work in `EffortlessMetrics/adze-swarm`; public `EffortlessMetrics/adze` is
  release/public-intake unless explicitly promoted or synced.
- Start each task from current `adze-swarm/main`.
- Inspect open `adze-swarm` PRs before creating another PR for the same scope.
- One work item per PR.
- Beginner docs teach `grammar::parse` and `grammar::parse_document` first.
- `AdzeDocument` remains the tooling boundary and one parse truth.
- Tree-sitter compatibility and query behavior remain documented subsets until
  proof and support tiers say otherwise.
- Performance receipts are evidence, not marketing claims.
- `Rust Small Result` remains the required GitHub gate.

## Phase 0: Campaign Setup

### Work Item: toolkit-excellence-campaign-source-of-truth

Status: complete
PR: EffortlessMetrics/adze-swarm#217
Proposal: ../../docs/proposals/ADZE-PROP-0004-toolkit-excellence.md
Active goal: ../../.adze/goals/active.toml

#### Goal

Open the new adoption-readiness campaign with a proposal, plan, active goal
manifest, and document-artifact ledger entries.

#### Production Delta

Docs and policy only. No runtime behavior changes.

#### Proof Commands

```bash
python -c "import tomllib; tomllib.load(open('.adze/goals/active.toml', 'rb')); tomllib.load(open('.adze/goals/toolkit-excellence.toml', 'rb')); tomllib.load(open('policy/doc-artifacts.toml', 'rb'))"
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

## Phase 1: Product Acceptance Matrix

### Work Item: product-acceptance-matrix

Status: complete
PR: EffortlessMetrics/adze-swarm#218

#### Goal

Define `docs/product/ACCEPTANCE_MATRIX.md` as the top-level product acceptance
map. Each row should name a user workflow, required behavior, proof command,
claim boundary, and support-tier impact.

#### Acceptance Rows

- Initialize project.
- Parse typed AST.
- Parse document.
- Diagnostics.
- GLR ambiguity.
- Tree-sitter selected tree.
- Query subset.
- JSON projection.
- CLI parse/check.
- WASM compile.
- Performance receipt.

## Phase 2: First-use And Downstream Proof

### Work Item: starter-project-hardening

Status: complete
PR: EffortlessMetrics/adze-swarm#219

#### Goal

Harden the generated starter project so a new user has one obvious path from
install to parse.

#### Proof Commands

```bash
cargo test -p adze-cli test_init_default_cwd_generates_buildable_project -- --exact --nocapture
cargo test -p adze-cli test_init_generates_buildable_project -- --exact --nocapture
cargo test -p adze-cli test_init_cargo_toml_references_adze_dependency -- --exact --nocapture
cargo test -p adze-cli getting_started_quickstart_builds_parses_and_reports_diagnostics -- --exact --nocapture
```

### Work Item: downstream-starter-fixture

Status: complete
PR: EffortlessMetrics/adze-swarm#222

#### Goal

Add a checked-in downstream starter fixture that behaves like a user crate and
proves dependency wiring, `build.rs`, generated parser module shape, public
imports, diagnostics, and examples.

#### Target Proof

```bash
cargo test --manifest-path testing/downstream-starter/Cargo.toml
cargo run --manifest-path testing/downstream-starter/Cargo.toml --example parse
```

## Phase 3: Documentation And Examples

### Work Item: beginner-docs-alignment

Status: complete
PRs: EffortlessMetrics/adze-swarm#223, EffortlessMetrics/adze-swarm#224

#### Goal

Align README, book, quickstart, and cookbook entry points with the generated
starter project.

#### Claim Boundary

Beginner docs use `grammar::parse(source)` and
`grammar::parse_document(source)`. They do not teach low-level parser
constructors or unsupported performance and incremental claims.

### Work Item: api-choice-guide

Status: complete
PR: EffortlessMetrics/adze-swarm#225

#### Goal

Add an API choice guide that maps user needs to `grammar::parse`,
`grammar::parse_document`, document syntax APIs, typed CST wrappers,
Tree-sitter-shaped traversal, JSON, ambiguity summaries, and query.

### Work Item: glr-ambiguity-example

Status: complete
PR: EffortlessMetrics/adze-swarm#226

#### Goal

Add a runnable example that teaches selected AST, document ambiguity summaries,
Tree-sitter selected-tree output, and the experimental boundary around raw
forest exposure.

### Work Item: query-highlighting-example

Status: complete
PR: EffortlessMetrics/adze-swarm#227

#### Goal

Add query/highlighting examples for captures, field constraints, anchors,
source-aware predicates, byte range, and root-only mode.

### Work Item: diagnostics-recovery-example

Status: complete
PR: EffortlessMetrics/adze-swarm#228

#### Goal

Add diagnostics and recovery examples for bad token, EOF, multibyte spans,
missing nodes, GLR bad input, and JSON diagnostic projection.

## Phase 4: Compatibility And Performance Receipts

### Work Item: ts-compat-matrix-doc

Status: complete
PR: EffortlessMetrics/adze-swarm#229

#### Goal

Publish a selected-tree Tree-sitter compatibility matrix with supported,
stabilizing, advisory, known-gap, and not-planned sections.

### Work Item: ts-compat-imported-shape-smoke

Status: complete
PR: EffortlessMetrics/adze-swarm#230

#### Goal

Add a small imported-shape smoke corpus for aliases, fields, hidden nodes,
anonymous tokens, error nodes, missing nodes, external scanner tokens, and
query captures.

### Work Item: benchmark-product-receipts

Status: complete
PR: EffortlessMetrics/adze-swarm#231

#### Goal

Add product receipt commands and baseline docs for parse, `parse_document`,
typed projections, Tree-sitter projection, query, diagnostics, JSON, GLR
ambiguity, and tablegen/ABI decode.

#### Claim Boundary

This work records advisory benchmark receipt commands. It does not introduce
stable throughput, stable memory, Tree-sitter performance parity, incremental
performance, or release-blocking regression claims.

## Phase 5: Support-tier Promotion

### Work Item: proven-slice-promotion

Status: complete
PR: EffortlessMetrics/adze-swarm#232

#### Goal

Promote only proven product slices in `docs/status/SUPPORT_TIERS.md`, with
proof commands, CI lanes, README/book wording, limitations, and rollback notes.

#### Claim Boundary

This work promotes selected slices to Stabilizing only. It does not create new
Stable claims, branch-protection requirements, full Tree-sitter parity, full
query parity, or performance thresholds.

## Closeout Criteria

- [x] Product acceptance matrix exists and points to proof commands.
- [x] Starter project and downstream fixture prove first-use behavior.
- [x] API choice, Tree-sitter, query, GLR ambiguity, diagnostics, recovery, and
  performance docs are aligned with examples or receipts.
- [x] Support-tier promotions are limited to proven slices.
- [x] Public `adze` drift remains closed or intentionally promoted.

Closeout: ./closeout.md

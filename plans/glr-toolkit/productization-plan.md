# GLR Toolkit Productization Plan

Status: complete
Owner: runtime/product
Created: 2026-05-17
Linked proposal: ../../docs/proposals/ADZE-PROP-0003-glr-toolkit-productization.md
Linked specs:
- ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
- ../../docs/specs/ADZE-SPEC-0013-query-compatibility.md
- ../../docs/specs/ADZE-SPEC-0014-performance-and-regression.md
Linked ADRs:
- ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
- ../../docs/adr/ADZE-ADR-0003-summary-first-glr-ambiguity.md
- ../../docs/adr/ADZE-ADR-0004-schema-versioned-projections.md
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/glr-toolkit-productization.toml
Support-tier map: ../../docs/status/SUPPORT_TIERS.md

## Goal

Turn the completed API foundation into a product-quality GLR parser toolkit:
one easy happy path for new users, correct GLR conflict behavior, deterministic
selected-tree output, honest ambiguity summaries, strong diagnostics and
recovery, document-backed Tree-sitter-compatible projections, a documented query
subset, and measured performance.

This plan sequences PR-sized work. It does not promote support tiers by itself;
`../../docs/status/SUPPORT_TIERS.md` owns product claims and proof mapping.

## Operating Rules

- Work in `EffortlessMetrics/adze-swarm`; public `EffortlessMetrics/adze` is
  release/public-intake surface unless explicitly promoted or synced.
- `AdzeDocument` is the native parse truth.
- Tree-sitter compatibility is a selected-tree projection over document facts.
- Query compatibility is a documented subset until proof says otherwise.
- GLR ambiguity is native and summary-first; full forest remains opt-in and
  experimental.
- Performance and coverage evidence stay scoped, manual, scheduled, or
  advisory unless a later policy PR says otherwise.
- No support-tier promotion happens without proof commands and limitations.

## Phase 0: Repo Discipline And Campaign Setup

### Work Item: repo-target-sync

Status: complete
PR: EffortlessMetrics/adze-swarm#124

#### Goal

Sync the public `adze` guard-check drift into `adze-swarm` and make
`adze-swarm` the operating repo again.

#### Proof Commands

```bash
cargo test -p xtask no_mangle -- --nocapture
cargo test -p xtask goto_indexing -- --nocapture
cargo run -q -p xtask -- check-no-mangle
cargo run -q -p xtask -- check-goto-indexing
cargo check -p adze --features "glr,pure-rust"
cargo test -p adze --features "pure-rust,glr" --test test_e2e_ambiguous_grammar_glr -- --nocapture
git diff --check
```

### Work Item: glr-toolkit-campaign-source-of-truth

Status: complete
Proposal: ../../docs/proposals/ADZE-PROP-0003-glr-toolkit-productization.md
Active goal: ../../.adze/goals/active.toml

#### Goal

Open the new post-0.9 campaign with a proposal, plan, active goal manifest, and
document-artifact ledger entries.

#### Production Delta

Docs and policy only. No runtime behavior changes.

#### Proof Commands

```bash
python -c "import tomllib; tomllib.load(open('.adze/goals/active.toml', 'rb')); tomllib.load(open('.adze/goals/glr-toolkit-productization.toml', 'rb')); tomllib.load(open('policy/doc-artifacts.toml', 'rb'))"
cargo run -q -p xtask -- check-active-goal
cargo run -q -p xtask -- check-doc-artifacts
git diff --check
```

## Phase 1: Define The Product Contract

### Work Item: glr-toolkit-product-contract

Status: complete
Planned spec: ADZE-SPEC-0012-glr-toolkit-product-contract

#### Goal

Define three user paths and the proof each path needs:

| User path | Required product behavior |
| --- | --- |
| Typed parser user | Write Rust types, generate a parser, call `grammar::parse(input)`. |
| Language/tooling user | Use `parse_document()` for diagnostics, ranges, fields, JSON, and ambiguity summaries. |
| Editor/Tree-sitter user | Use selected-tree compatibility, metadata, fields, S-expressions, node-types, and documented query behavior. |

#### Acceptance

The spec describes promotion criteria for GLR conflict routing, structured
parse errors, `AdzeDocument`, `ts_compat`, node-types, query subset, and
performance evidence without promoting any claim by itself.

## Phase 2: First-use Path

### Work Item: canonical-starter-project

Status: complete

#### Goal

Make `adze init calc` generate the canonical starter project:

```text
Cargo.toml
build.rs
src/lib.rs
examples/parse.rs
tests/parse.rs
README.md
```

#### Acceptance

```bash
cargo run -p adze-cli -- init calc
cd calc
cargo test
cargo run --example parse
```

The generated project parses valid arithmetic input and reports diagnostics for
bad input. The `cargo install adze-cli` path remains the intended published
release surface and needs its own install receipt before it is treated as
proven.

### Work Item: quickstart-and-mental-model

Status: complete

#### Goal

Add one beginner path and one mental model:

```text
docs/tutorials/quickstart-10-minutes.md
docs/explanations/mental-model.md
```

The mental model should explain:

```text
Rust type annotations
  -> grammar IR
  -> parse table
  -> generated parser
  -> typed AST
  -> AdzeDocument
  -> optional projections
```

## Phase 3: Build Accuracy Oracles

### Work Item: fixture-taxonomy

Status: complete

#### Goal

Classify fixtures for GLR, Tree-sitter compatibility, query, and recovery.
Each fixture names grammar, input, selected-tree shape, ambiguity summary,
diagnostics, Tree-sitter-compatible projection, and support-tier relevance.

### Work Item: projection-equivalence-harness

Status: complete

#### Goal

For each fixture, compare `parse_document(source)` facts against generic CST,
typed CST, typed AST, Tree-sitter-compatible tree, diagnostics, ambiguity
summary, and JSON where enabled.

## Phase 4: GLR Core Correctness

### Work Item: generated-conflict-matrix

Status: complete

#### Goal

Add generated coverage for shift/reduce, reduce/reduce, nested fork,
multi-conflict expression, dangling-else, ambiguous list, and
prefix/postfix-style grammars.

### Work Item: tablegen-conflict-abi-roundtrip

Status: complete

#### Goal

Unify tablegen conflict-cell, alias, field, metadata, compressed-row, and parse
action encode/decode canaries into one GLR ABI roundtrip matrix.

### Work Item: goto-symbol-indexing-proof

Status: complete

#### Goal

Make GOTO and symbol-indexing invariants part of the GLR/tablegen proof lane.

## Phase 5: Tree-sitter-compatible Selected-tree Output

### Work Item: ts-compat-selected-tree-parity

Status: complete

#### Goal

Prove the documented selected-tree subset: traversal, sibling links, byte and
point ranges, kind and grammar identity, error/missing flags, S-expression, and
field lookup.

### Work Item: node-types-parity

Status: complete

#### Goal

Harden node-types metadata enough for editor and tooling adoption while keeping
known alias-visible gaps explicit until proven.

### Work Item: error-missing-node-compat

Status: complete

#### Goal

Cover ERROR nodes, missing nodes, `has_error` propagation, zero-width ranges,
bad-token spans, EOF, and diagnostic linkage.

## Phase 6: Query Compatibility

### Work Item: query-compatibility-spec

Status: complete

#### Goal

Define the supported Tree-sitter query subset and the explicit known gaps.

### Work Item: query-field-constraints-and-anchors

Status: complete

#### Goal

Implement field constraints and anchor behavior for the documented subset.

### Work Item: query-predicate-parity

Status: complete

#### Goal

Harden source-aware predicate semantics and fail closed for text predicates in
source-free matching.

### Work Item: query-differential-corpus

Status: complete

#### Goal

Compare the supported query subset against Tree-sitter fixtures and record
unsupported features as expected gaps.

## Phase 7: Diagnostics, Recovery, And CLI

### Work Item: recovery-matrix

Status: complete

#### Goal

Add bad-input recovery proof for invalid tokens, EOF, missing delimiters, bad
separators, UTF-8, multiline errors, ambiguous input with errors, and external
scanner errors.

### Work Item: cli-diagnostic-projection

Status: complete

#### Goal

Make parse/check diagnostics pleasant and prove CLI diagnostic, tree, document,
and ambiguity JSON projections read from `parse_document`.

## Phase 8: Examples, Migration, And Documentation

### Work Item: canonical-parser-cookbook

Status: complete

#### Goal

Add canonical examples for arithmetic, CSV/list grammar, object-like grammar,
keywords/identifiers, operator precedence, fielded nodes, external scanners,
ambiguous GLR grammars, Tree-sitter-compatible output, query captures, and
diagnostics.

### Work Item: tree-sitter-migration-guide

Status: complete

#### Goal

Answer the migration questions in one place: node-types, Tree, Node, fields,
queries, ERROR/MISSING, and ambiguity.

## Phase 9: Incremental Document Lifecycle

### Work Item: incremental-lifecycle-acceptance

Status: complete

#### Goal

Decide whether `ADZE-SPEC-0009` can move from proposed to accepted with the
current immutable-document, edit-to-new-document, explicit-fallback model.

### Work Item: reparse-fallback-metadata

Status: complete

#### Goal

Expose honest full-reparse fallback metadata before any stable incremental reuse
claim.

## Phase 10: Performance And Benchmark Evidence

### Work Item: performance-contract

Status: complete

#### Goal

Define benchmark surfaces and regression-receipt policy for parse,
`parse_document`, typed AST/CST, Tree-sitter projection, query, diagnostics,
tablegen, and ABI decode paths.

### Work Item: benchmark-fixtures

Status: complete

#### Goal

Add GLR and projection benchmark fixtures as advisory/manual evidence, not
default PR gates.

## Phase 11: Support-tier Promotion

### Work Item: support-tier-promotion-pass

Status: complete

#### Goal

Promote only proven slices in `../../docs/status/SUPPORT_TIERS.md`, with proof
commands, limitations, and README claim alignment.

## Campaign Closeout

Status: complete
Closed: 2026-05-17

### What Shipped

- Repo-target sync and `EffortlessMetrics/adze-swarm` operating discipline.
- Product contract, first-use CLI/docs path, fixture taxonomy, and projection
  equivalence proof.
- Generated GLR conflict matrix, tablegen ABI roundtrip, and GOTO/symbol
  indexing invariant proof.
- Tree-sitter selected-tree, node-types, and error/missing-node compatibility
  receipts for the documented subset.
- Query compatibility spec, field constraints, anchors, predicates, and
  supported-subset differential corpus.
- Recovery matrix and CLI diagnostic projection proof.
- Canonical parser cookbook and Tree-sitter migration guide.
- Incremental lifecycle acceptance and full-reparse fallback metadata.
- Performance contract plus benchmark fixture and projection receipts.
- Support-tier proof receipts without broad over-promotion.

### What Did Not Ship

- Stable full GLR forest export.
- Full Tree-sitter API or query parity.
- Stable document JSON or WASM schema.
- Stable incremental reuse or performance guarantees.
- Blocking performance thresholds.

### Proof Commands

```bash
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

The work-item proof commands above and the support-tier proof map remain the
receipts for individual product slices.

### Next Campaign Candidates

- Promote specific proven slices after user-facing docs and support-tier rows
  line up with repeatable proof.
- Close remaining Tree-sitter and query compatibility gaps.
- Harden stable document, CLI, and WASM serialized schemas.
- Add measured regression thresholds once benchmark baselines have enough
  history.

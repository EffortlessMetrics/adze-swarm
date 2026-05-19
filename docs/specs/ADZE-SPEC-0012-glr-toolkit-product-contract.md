# ADZE-SPEC-0012: GLR Toolkit Product Contract

Status: accepted
Owner: runtime/product
Created: 2026-05-17
Linked proposal: ../proposals/ADZE-PROP-0003-glr-toolkit-productization.md
Linked ADRs:
- ../adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
- ../adr/ADZE-ADR-0003-summary-first-glr-ambiguity.md
- ../adr/ADZE-ADR-0004-schema-versioned-projections.md
Linked plan: ../../plans/glr-toolkit/productization-plan.md
Linked issues:
Linked PRs:
Support-tier impact:
- Defines promotion criteria for GLR toolkit surfaces.
- Does not promote any support-tier row by itself.
Policy impact:
- Future proof lanes and policy receipts should cite this contract.

## Problem

Adze has a strong foundation: typed Rust grammars, pure-Rust parser generation,
GLR support, `AdzeDocument`, typed CST and AST projections, diagnostics,
Tree-sitter-compatible output, JSON/CLI/WASM projection specs, query work, and
support-tier proof mapping.

The product gap is that users should not need to understand every internal
surface before succeeding. Adze needs one beginner path, one canonical parse
truth, and proof matrices that explain which GLR, Tree-sitter-compatible, query,
diagnostic, JSON, CLI, and performance claims are reliable.

## Behavior

### B1. Adze has one obvious first-use path

The intended published first-use path is:

```bash
cargo install adze-cli
adze init calc
cd calc
cargo test
cargo run --example parse
```

Until `adze-cli` is published as a crates.io install surface, the current
repo-proven path is:

```bash
cargo run -p adze-cli -- init calc
cd calc
cargo test
cargo run --example parse
```

The generated starter project must include:

```text
Cargo.toml
build.rs
src/lib.rs
examples/parse.rs
tests/parse.rs
README.md
```

It must demonstrate the core product promise:

```text
Rust type annotations -> generated parser -> typed Rust values
```

The starter project must also expose the next rung of the ladder: a
`parse_document()` path that can report diagnostics for bad input.

### B2. Adze has three documented user paths

The product documentation and examples must serve three paths:

| User path | Required behavior |
| --- | --- |
| Typed parser user | Write Rust types, generate a parser, call `grammar::parse(input)`, receive typed Rust values. |
| Language/tooling user | Call `parse_document()` and inspect diagnostics, source ranges, fields, JSON, typed CST/AST projections, and ambiguity summaries. |
| Editor/Tree-sitter user | Use selected-tree compatibility, node metadata, fields, S-expressions, node-types, and documented query behavior for a supported subset. |

### B3. `AdzeDocument` remains the canonical parse truth

Every product surface in this contract must derive from `AdzeDocument` or from a
document-backed generated parser path. Runtime parser engines remain
implementation details.

The projection rule is:

```text
source
  -> parser runtime
  -> AdzeDocument
      -> typed AST
      -> generic CST
      -> typed CST
      -> diagnostics
      -> ambiguity summaries / optional forest
      -> Tree-sitter-compatible selected-tree output
      -> query cursor subset
      -> JSON / CLI / WASM projections
```

`parse()` remains the ergonomic typed-AST fast path. `parse_document()` is the
native parse-product boundary for tooling and projection work.

### B4. GLR correctness is proven by a matrix

GLR correctness claims require fixture or generated-matrix proof for:

- single shift/reduce conflict;
- single reduce/reduce conflict;
- nested fork conflict;
- multi-conflict expression grammar;
- dangling-else grammar;
- ambiguous list grammar;
- ambiguous prefix/postfix grammar;
- bad input inside an ambiguous grammar.

For each fixture, proof must cover:

- conflict cells survive grammar generation and table encoding;
- the runtime retains alternatives where expected;
- selected tree is deterministic;
- ambiguity summary records document facts;
- typed AST projection reads the selected tree;
- bad input returns a structured error or diagnostic document instead of
  panicking.

### B5. Selected-tree behavior is deterministic

When a document exists, the selected tree must be stable for the same grammar,
source, parse options, and generated tables. Any selection policy that affects
the selected tree must be documented in GLR or ambiguity specs before promotion.

Tree-sitter-compatible output exposes the selected tree. Native Adze APIs expose
ambiguity summaries separately.

### B6. Ambiguity is summary-first

Native GLR ambiguity must be visible through document-backed summaries. Full
forest export is opt-in and experimental until a separate proof and stability
contract exists.

### B7. Diagnostics and recovery are product behavior

Diagnostics are structured document facts, not just rendered strings. Recovery
proof must cover:

- bad tokens;
- unexpected EOF;
- missing delimiters;
- bad separators;
- multibyte spans;
- multiline spans;
- ambiguous input with errors;
- external scanner errors where supported.

For each, proof must cover byte range, point range, useful source excerpt,
expected tokens when available, no panic, and agreement between document
diagnostics and selected-tree error/missing facts when those facts exist.

### B8. Tree-sitter compatibility is a selected-tree subset

Tree-sitter compatibility must be documented as a subset until parity proof
exists. The selected-tree subset must define support status for:

- root and child traversal;
- named child traversal;
- parent and sibling traversal;
- byte and point ranges;
- kind and kind ID;
- grammar name and grammar ID;
- named, extra, error, missing, and has-error state;
- field lookup by name and ID;
- S-expression rendering;
- language field and node-kind metadata;
- node-types metadata.

Compatibility code must not invent field IDs, aliases, error state, ranges, or
metadata locally if that data belongs in the document or language schema.

### B9. Query compatibility is a documented subset

Tree-sitter query compatibility must have its own subset spec before support
tier promotion. The subset must explicitly classify named node patterns,
anonymous/literal token patterns, captures, field constraints, child
quantifiers, sibling sequences, alternation, anchors, predicates, directives,
byte-range filtering, and root-only matching.

Unsupported query features must be expected gaps, not hidden failures.

### B10. Node-types and language metadata are generated from schema

Node-types output, typed CST metadata, Tree-sitter-compatible metadata, and
query-facing identity must derive from the same language schema. Alias behavior
must distinguish visible identity from grammar identity.

### B11. CLI and JSON are projections, not separate parsers

CLI and JSON output must serialize document-backed projections. They must not
define independent parse semantics. Serialized outputs need explicit schema
families before stable claims.

### B12. Performance claims need fixture-linked receipts

Performance work must measure named product paths:

- parse only;
- `parse_document`;
- typed AST projection;
- typed CST projection;
- Tree-sitter-compatible projection;
- query matching;
- JSON projection;
- ambiguity summary;
- diagnostics rendering;
- tablegen codegen;
- TSLanguage ABI decode.

Benchmarks are advisory/manual/scheduled evidence until a later policy PR
promotes them.

### B13. Support-tier promotion is evidence-based

No GLR toolkit slice may move to a stronger support tier without:

- a support-tier row;
- proof commands;
- known limitations;
- CI or manual lane mapping;
- user-facing docs or examples when the claim is public-facing.

## Non-Goals

- No runtime behavior change in this spec.
- No support-tier promotion by wording alone.
- No full Tree-sitter API or query parity claim.
- No stable full forest API.
- No stable incremental reuse guarantee.
- No default benchmark, coverage-heavy, or full-matrix gate for ordinary PRs.
- No public `EffortlessMetrics/adze` swarm work.

## Required Evidence

Before product slices can promote, the repo needs:

- starter project generation proof;
- quickstart and mental-model docs;
- GLR conflict matrix;
- GOTO and symbol-indexing invariant proof;
- tablegen conflict-cell ABI roundtrip proof;
- document projection equivalence harness;
- selected-tree Tree-sitter parity matrix;
- node-types metadata snapshots;
- query subset spec and tests;
- diagnostics/recovery bad-input matrix;
- CLI/JSON projection tests;
- performance baseline receipts;
- support-tier rows with proof commands and limitations.

## Acceptance Examples

### Accepted: Typed parser happy path

```bash
cargo run -p adze-cli -- init calc
cd calc
cargo test
cargo run --example parse -- "1 + 2 * 3"
```

The example returns a typed Rust value from `grammar::parse`.
The `cargo install adze-cli` variant becomes accepted release-surface proof only
after the CLI is published and the install path has a receipt.

### Accepted: Tooling path

```rust
let report = grammar::parse_document("1 +");
let doc = report.document();
assert!(!doc.diagnostics().is_empty());
```

The document remains available when recovery can produce trustworthy syntax
facts.

### Accepted: Compatibility path

```rust
let doc = grammar::parse_document(source).document();
let tree = doc.as_tree_sitter();
assert_eq!(tree.root_node().to_sexp(), "(source_file ...)");
```

The compatibility view reads selected-tree data from the document.

### Rejected: Unsupported parity claim

Documentation must not say "full Tree-sitter query compatibility" unless the
query subset spec, differential corpus, and support-tier proof establish that
claim.

## Test Mapping

Planned test and fixture surfaces:

```text
adze-cli init quickstart tests
tests/fixtures/glr/**
tests/fixtures/ts-compat/**
tests/fixtures/query/**
tests/fixtures/recovery/**
runtime/tests/projection_equivalence*.rs
runtime/tests/ts_compat_selected_tree*.rs
runtime/tests/query_compat*.rs
runtime/tests/recovery_matrix*.rs
benchmarks/**
```

Existing proof surfaces remain valid where they map to support-tier rows, but
this contract expects future work to collect them into product-shaped matrices.

## Implementation Mapping

| Surface | Primary owner |
| --- | --- |
| Starter project and CLI docs | `cli/`, `docs/tutorials/` |
| Grammar-to-parser pipeline | `macro/`, `tool/`, `common/`, `ir/`, `tablegen/` |
| Runtime parse and document model | `runtime/`, `glr-core/` |
| Typed CST/AST projections | `runtime/`, `tablegen/`, `tool/` |
| Tree-sitter compatibility | `runtime/src/ts_compat/`, language schema generation |
| Query compatibility | `runtime/src/query/` |
| Diagnostics and recovery | `runtime/src/error*`, document diagnostics, generated parser tests |
| JSON/CLI/WASM projections | `runtime/`, `cli/`, `wasm-demo/`, schema docs |
| Benchmarks | `benchmarks/` |
| Product proof | `docs/status/SUPPORT_TIERS.md`, `policy/doc-artifacts.toml` |

## CI Proof

This spec PR is docs/policy only:

```bash
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
cargo run -q -p xtask -- check-active-goal --mode blocking
git diff --check
```

Future implementation PRs should cite their narrower proof commands from
`../../.adze/goals/active.toml` and avoid defaulting to broad CI fanout.

## Metrics And Promotion Rule

A GLR toolkit surface can only promote when:

1. the behavior is covered by a spec or accepted subset;
2. examples or docs describe the user-facing claim;
3. proof commands are repeatable locally and mapped in support tiers;
4. limitations are explicit;
5. the relevant fixture matrix or benchmark receipt exists.

Promotion should be by slice. For example, selected-tree traversal can promote
before full query parity, and structured diagnostics for generated parsers can
promote before all external scanner recovery cases.

## Open Questions

- Which fixture format should become the shared oracle format for GLR,
  Tree-sitter compatibility, query, recovery, and benchmarks?
- Should `adze init` live entirely in `adze-cli`, or should project templates be
  reusable by docs and integration tests?
- Which Tree-sitter query features should be considered "not planned" versus
  "future"?
- What minimum benchmark receipt is required before performance claims appear in
  README or release notes?

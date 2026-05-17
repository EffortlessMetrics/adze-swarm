# ADZE-SPEC-0014: Performance and regression

Status: accepted
Owner: runtime/perf
Created: 2026-05-17
Linked proposal: ../proposals/ADZE-PROP-0003-glr-toolkit-productization.md
Linked ADRs:
Linked plan: ../../plans/glr-toolkit/productization-plan.md
Linked issues:
Linked PRs:
Support-tier impact:
- Defines what evidence is required before performance claims can appear in
  README, support tiers, release notes, or API documentation.
- Does not promote benchmarks or performance behavior by itself.
Policy impact:
- Registers the performance contract in `policy/doc-artifacts.toml`.
- Keeps benchmark execution manual, scheduled, or explicitly requested rather
  than a default ordinary-PR gate.

## Problem

Adze already has benchmark crates, fixture generators, compile-only benchmark CI,
and product proof specs for parser, document, Tree-sitter compatibility, query,
diagnostics, and JSON surfaces. Those pieces are useful, but performance claims
need a contract that keeps measurement honest.

Users need to know which performance paths are measured, what a benchmark result
actually proves, which fixtures back the result, and whether a regression is a
correctness risk, an advisory trend, or a release blocker. Maintainers and agents
need the same map so performance work does not become disconnected micro-bench
optimization.

## Behavior

### B1. Performance evidence is fixture-backed

Every benchmark or performance receipt used for a product claim must name the
fixture family it measures and the correctness oracle that makes the fixture
trustworthy.

Performance fixtures must not be accepted as evidence when they cannot be parsed,
when they exercise only synthetic dummy input, or when their shape no longer maps
to a supported parser/tooling path.

### B2. Performance surfaces are measured separately

The performance contract tracks these surfaces as separate measurements:

| Surface | Required measurement boundary |
| --- | --- |
| parse only | parser runtime work without projection rendering |
| `parse_document` | canonical document construction and metadata capture |
| typed AST projection | document-selected tree to semantic Rust value |
| typed CST projection | generated wrapper casts and field accessors |
| Tree-sitter projection | selected-tree adapter traversal and metadata lookup |
| query matching | supported query subset over selected-tree facts |
| JSON projection | schema-versioned serialization cost |
| GLR ambiguity summary | summary construction without eager raw-forest export |
| diagnostics rendering | structured diagnostic to human or JSON view |
| tablegen codegen | grammar/table generation cost |
| TSLanguage ABI decode | runtime decode of generated table metadata |

One surface must not borrow a benchmark result from another surface. For example,
a parser-only result does not prove document JSON performance, and a query result
does not prove selected-tree traversal performance.

### B3. Benchmark execution is not an ordinary PR default

Ordinary PRs may compile benchmark code when relevant, but they must not run full
Criterion measurement by default.

Benchmark execution is valid when triggered by:

- manual workflow dispatch;
- scheduled performance collection;
- release-readiness evidence;
- an explicit performance label or affected-lane policy;
- maintainer-run local receipts attached to a PR or closeout.

### B4. Compile-only benchmark proof is a health check

Compile-only benchmark CI proves that benchmark code and fixture imports still
build. It does not prove runtime performance, throughput, latency, memory use, or
regression absence.

Docs and support tiers must describe compile-only receipts as build health, not
as performance proof.

### B5. Performance receipts include enough context to compare

A performance receipt must record:

- commit or PR;
- command;
- machine class or runner class;
- OS and architecture;
- Rust version;
- profile and feature flags;
- fixture family and fixture size;
- benchmark group and case;
- current measurement;
- baseline measurement when available;
- regression threshold when one is being enforced;
- whether the result is advisory or release-blocking.

Receipts may be stored as CI artifacts, release artifacts, docs updates, or
explicit handoff notes. `docs/perf/baselines.md` is the release-readable index of
current baseline policy.

### B6. Regression thresholds start advisory

Initial regression thresholds are advisory. A threshold can become blocking only
after:

- the benchmark fixture has a correctness oracle;
- the measurement is stable enough across the selected runner class;
- the threshold is documented in `docs/perf/baselines.md`;
- the support-tier or release checklist names the proof command;
- the fallback or override policy is documented.

### B7. Optimizations preserve projection equivalence

Performance optimizations that touch parse, document, projection, diagnostics,
query, or Tree-sitter compatibility paths must preserve the relevant correctness
matrix before their measurements matter.

An optimization PR must not claim a performance win when it changes selected-tree
shape, diagnostic facts, ambiguity summaries, field identity, node ranges, query
matches, or JSON schema output without an explicit spec change.

### B8. Runner class is part of the claim

Performance receipts must identify the runner class used for measurement.

Target usage:

| Runner class | Role |
| --- | --- |
| developer workstation | local investigation only |
| GitHub-hosted Linux | portable advisory smoke |
| CX43 / rust-small | small compile/check and bounded smoke |
| CX53 / rust-large | heavier Linux benchmark or coverage-lite candidate |
| scheduled release runner | comparable release-readiness baseline |

Cross-runner comparisons are advisory unless a release policy explicitly says
they are comparable.

## Non-Goals

- No default full benchmark run on ordinary PRs.
- No stable performance guarantee for incremental reuse.
- No memory ceiling or throughput SLA in this spec.
- No benchmark threshold promotion without fixture correctness proof.
- No claim that compile-only benchmark CI proves runtime performance.
- No support-tier promotion by this spec alone.

## Required Evidence

- Benchmark inventory maps every registered benchmark to classification, status,
  fixture family, and CI coverage.
- Fixture tests prove benchmark inputs parse or otherwise match their intended
  negative/error purpose.
- Compile-only benchmark CI remains green for relevant benchmark changes.
- Manual or scheduled benchmark receipts include the context fields listed in
  this spec.
- Any blocking threshold is documented before enforcement.

## Acceptance Examples

Accepted compile-only receipt:

```bash
cargo bench -p adze-benchmarks --no-run
```

This proves benchmark code still builds. It does not prove a throughput claim.

Accepted manual measurement receipt:

```bash
cargo bench -p adze-benchmarks --bench parse_bench
```

The receipt is useful only when it records the commit, runner class, fixture
family, and benchmark group.

Rejected claim:

```text
README: Adze is faster than Tree-sitter.
```

This is not allowed without named fixtures, comparison method, runner class,
baseline receipts, limitations, and support-tier proof.

## Test Mapping

- `benchmarks/tests/verify_fixture_parsing.rs`;
- `benchmarks/Cargo.toml` benchmark metadata;
- `benchmarks/README.md` benchmark inventory;
- future GLR and projection benchmark fixture tests;
- projection, query, recovery, and Tree-sitter compatibility matrices that prove
  optimized paths remain correct.

## Implementation Mapping

Primary surfaces:

- `benchmarks/`;
- `.github/workflows/criterion-smoke.yml`;
- `.github/workflows/coverage.yml`;
- `.github/workflows/pure-rust-ci.yml`;
- `docs/perf/baselines.md`;
- `docs/status/SUPPORT_TIERS.md`;
- `plans/glr-toolkit/productization-plan.md`.

## CI Proof

```bash
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
cargo bench -p adze-benchmarks --no-run
git diff --check
```

For this docs-only contract PR, the required proof is:

```bash
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

## Metrics And Promotion Rule

Performance claims stay advisory until the claim has:

- one named measured surface;
- one named fixture family;
- one correctness oracle;
- one repeatable command;
- one runner-class statement;
- one baseline receipt;
- one documented limitation set;
- one support-tier row or release checklist entry.

Stable public claims require a support-tier promotion PR after the measurement
policy and proof receipts exist.

## Open Questions

- Which runner class should own release-comparable benchmark baselines?
- Which surface should get the first advisory regression threshold?
- Should baseline receipts live only as CI artifacts, or should release-relevant
  summaries be copied into `docs/perf/baselines.md`?

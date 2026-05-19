# Performance Baselines

Status: advisory
Owner: runtime/perf
Linked spec: ../specs/ADZE-SPEC-0014-performance-and-regression.md

This file is the release-readable index for Adze performance baseline policy.
It does not replace Criterion output, CI artifacts, or support-tier proof rows.

## Current Policy

Adze performance evidence is advisory unless a release checklist or support-tier
row explicitly names a blocking threshold and proof command.

Ordinary pull requests may compile benchmarks, but they do not run full Criterion
measurement by default.

The current product smoke receipt is:

```bash
cargo run -q -p xtask -- perf-receipt --profile product-smoke
```

It prints advisory proof commands; it does not execute a benchmark run by
itself and does not create a performance claim.

## Baseline Surfaces

| Surface | Status | Current evidence | Default PR behavior |
| --- | --- | --- | --- |
| parse only | advisory | benchmark inventory and compile-only receipts | compile-only when routed |
| `parse_document` | advisory | `document_projection` compile-only benchmark fixture | compile-only when routed |
| typed AST projection | advisory | `document_projection` compile-only benchmark fixture | compile-only when routed |
| typed CST projection | future | needs projection fixture benchmark | not run |
| Tree-sitter projection | future | needs selected-tree projection benchmark | not run |
| query matching | future | needs supported-subset query fixtures | not run |
| JSON projection | advisory | `document_projection` compile-only benchmark fixture | compile-only when routed |
| GLR ambiguity summary | future | needs ambiguity fixture benchmark | not run |
| diagnostics rendering | future | needs recovery fixture benchmark | not run |
| tablegen codegen | advisory | compile/test receipts only | not run |
| TSLanguage ABI decode | advisory | tablegen ABI tests, no runtime threshold | not run |

## Product Smoke Receipt

The `product-smoke` receipt is the current release-readable performance proof
index. It maps product-facing benchmark slices to commands that can be run
locally, manually, or by scheduled evidence lanes.

| Surface | Receipt command category | Claim boundary |
| --- | --- | --- |
| parse only | `parse_bench --no-run` | compile health for fixture-backed parse benchmarks |
| `parse_document` | `document_projection --no-run` | compile health for document projection benchmarks |
| typed AST projection | `document_projection --no-run` | compile health only; no throughput claim |
| JSON projection | `document_projection --no-run` | compile health only; schema correctness is proven elsewhere |
| benchmark inventory | `verify_fixture_parsing` tests | benchmark metadata and fixture families are documented |

The receipt intentionally excludes stable claims for typed CST projection,
Tree-sitter projection throughput, query matching throughput, GLR ambiguity
summary throughput, diagnostics rendering throughput, stable memory use,
incremental performance, and release-blocking thresholds until each surface has
dedicated fixtures and support-tier proof.

## Receipt Fields

A comparable performance receipt should include:

- commit or PR;
- command;
- machine or runner class;
- operating system and architecture;
- Rust version;
- profile and feature flags;
- fixture family and fixture size;
- benchmark group and case;
- current measurement;
- baseline measurement when available;
- threshold, if enforced;
- advisory or blocking status.

## Runner Classes

| Runner class | Baseline use |
| --- | --- |
| developer workstation | local investigation only |
| GitHub-hosted Linux | portable advisory smoke |
| CX43 / rust-small | small compile/check and bounded smoke |
| CX53 / rust-large | candidate for heavier Linux benchmark lanes |
| scheduled release runner | future release-comparable baseline |

Cross-runner comparisons are advisory unless a release policy says otherwise.

## Blocking Thresholds

No blocking performance thresholds are active.

Before a threshold can block a PR or release, it must have:

- a fixture correctness oracle;
- a stable runner class;
- a documented command;
- a documented threshold and override path;
- a support-tier or release-checklist entry.

## Current Proof Commands

Compile-only benchmark health:

```bash
cargo bench -p adze-benchmarks --no-run
```

Product smoke receipt:

```bash
cargo run -q -p xtask -- perf-receipt --profile product-smoke
```

Document projection benchmark compile health:

```bash
cargo bench -p adze-benchmarks --bench document_projection --no-run
```

Fixture-family inventory guard:

```bash
cargo test -p adze-benchmarks --test verify_fixture_parsing verify_benchmark_fixture_families_are_documented -- --exact --nocapture
```

Source-of-truth guard:

```bash
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
```

## Non-Claims

The current baseline policy does not claim:

- stable throughput;
- stable memory use;
- stable incremental parsing performance;
- cross-runner comparability;
- Tree-sitter performance parity;
- release-blocking regression thresholds.

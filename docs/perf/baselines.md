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

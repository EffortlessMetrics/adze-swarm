# Crate And Package Boundary Guide

> **Doc status:** `policy/package-boundary.toml` is the source of truth for
> package classification. This page is an explanatory map, not the policy
> ledger.

Adze used to describe its workspace as a broad microcrate architecture. The
current rule is narrower:

```text
Prefer owner modules inside existing packages.
Create or keep a package only when the package-boundary ledger justifies it.
```

Every workspace package should be classified as one of:

- published product or support crate;
- dev-only tooling/test crate;
- owner-module migration target.

There is no durable category for an unpublished production package without a
clear owner, proof impact, and review path.

## Core Product Pipeline

These crates form the supported grammar-to-parser pipeline:

| Crate | Path | Responsibility |
|---|---|---|
| `adze` | `runtime/` | Main runtime library: generated `parse()`, `parse_document()`, `Extract`, diagnostics, document/projection APIs |
| `adze-macro` | `macro/` | Proc-macro attributes such as `#[adze::grammar]` and `#[adze::leaf]` |
| `adze-tool` | `tool/` | Build-time parser generation and code emission |
| `adze-common` | `common/` | Shared grammar expansion logic |
| `adze-ir` | `ir/` | Grammar intermediate representation and normalization |
| `adze-glr-core` | `glr-core/` | LR/GLR table construction and conflict representation |
| `adze-tablegen` | `tablegen/` | Table compression, metadata, and ABI-oriented generation |

These are the crates covered by the supported PR gate.

## Experimental Runtime Surface

| Crate | Path | Posture |
|---|---|---|
| `adze-runtime` | `runtime2/` | Experimental proving ground; not the public-primary runtime contract |

`runtime2/` can remain useful for implementation experiments, but user-facing
docs should not describe it as the production runtime. Stable product claims
must route through generated parser APIs and support-tier proof.

## Durable Support Crates

The post-collapse support crates are intentionally few:

| Crate | Path | Responsibility |
|---|---|---|
| `adze-bdd-governance-core` | `crates/bdd-governance-core/` | Governance BDD snapshots and matrix composition |
| `adze-common-type-ops-core` | `crates/common-type-ops-core/` | Shared type-shape helpers used by macro/tool code |
| `adze-linecol-core` | `crates/linecol-core/` | Byte/line/column source location support |
| `adze-parsetable-metadata` | `crates/parsetable-metadata/` | Shared parse-table metadata contracts |

`crates/ts-c-harness/` is excluded from ordinary workspace commands and should
not be treated as a supported product package.

## Tooling, Tests, And Fixtures

| Area | Examples | Product posture |
|---|---|---|
| CLI/tooling | `cli/`, `lsp-generator/`, `xtask/` | Support-tiered; not automatically stable |
| Benchmarks/perf | `benchmarks/` | Advisory receipts, not merge-blocking product proof by default |
| Golden/test support | `golden-tests/`, `testing/`, `glr-test-support/`, `test-mini/` | Dev/test proof infrastructure |
| Grammar fixtures | `grammars/python/`, `grammars/javascript/`, `grammars/go/`, `grammars/python-simple/`, `grammars/test-vec-wrapper/` | Reference/dev fixtures unless promoted by support tiers |
| Demos | `playground/`, `wasm-demo/`, `samples/downstream-demo/` | Advisory/demo surfaces |

## Adding Or Keeping A Package

Before adding a package, ask:

1. Can this be an owner module inside an existing crate?
2. Is the package public, dev-only, or a temporary migration target?
3. Who owns it?
4. Which support-tier or policy row does it affect?
5. Which proof command covers it?
6. What is the rollback or owner-module collapse path?

If a new package is still justified, update:

```text
Cargo.toml
policy/package-boundary.toml
docs/status/SUPPORT_TIERS.md, if the package affects a product claim
```

Then run:

```bash
cargo run -q -p xtask -- check-package-boundary
git diff --check
```

## Rule Of Thumb

Use packages for durable ownership boundaries. Use modules for ordinary
implementation structure. Support-tier claims should depend on repeatable proof,
not on the presence of another crate.

# Durable Support Crates

This directory contains the remaining SRP support crates after the 0.9
microcrate-to-owner-module collapse. The package-boundary ledger is the source
of truth for whether a package is part of the release workspace:

```bash
cargo metadata --format-version 1 --no-deps
cargo run -q -p xtask -- check-package-boundary
cargo run -q -p xtask -- check-package-boundary --release-gate
```

The release gate must stay free of `owner-module-migration-target` entries.

## Workspace Support Crates

| Crate | Purpose | Boundary |
|-------|---------|----------|
| [`bdd-governance-core`](bdd-governance-core) | BDD governance matrix primitives and parser feature policy helpers. | Published support crate |
| [`common-type-ops-core`](common-type-ops-core) | Shared type-shape transformation helpers for macro/tool syntax handling. | Published support crate |
| [`linecol-core`](linecol-core) | Byte-to-line/column position tracking for diagnostics and scanner/lexer surfaces. | Published support crate |
| [`parsetable-metadata`](parsetable-metadata) | Shared `.parsetable` metadata model and parsing helpers. | Published support crate |

## Excluded Harness

| Crate | Purpose | Boundary |
|-------|---------|----------|
| [`ts-c-harness`](ts-c-harness) | Tree-sitter C FFI parity test harness. | Excluded from workspace; build explicitly when the C/Tree-sitter environment is present |

## Boundary Rules

- Do not add a new crate under `crates/` without updating
  [`../policy/package-boundary.toml`](../policy/package-boundary.toml).
- Do not reintroduce durable unpublished production crates.
- Temporary owner-module migration targets are release blockers.
- Product claims for support crates must map through
  [`../docs/status/SUPPORT_TIERS.md`](../docs/status/SUPPORT_TIERS.md).

See:

- [`../docs/adr/ADZE-ADR-0002-no-durable-unpublished-production-crates.md`](../docs/adr/ADZE-ADR-0002-no-durable-unpublished-production-crates.md)
- [`../docs/adr/ADZE-ADR-0005-durable-published-support-crates.md`](../docs/adr/ADZE-ADR-0005-durable-published-support-crates.md)
- [`../plans/0.9.0/microcrate-collapse.md`](../plans/0.9.0/microcrate-collapse.md)
- [`../docs/status/MICROCRATE_TEST_COVERAGE.md`](../docs/status/MICROCRATE_TEST_COVERAGE.md)

## Features

Support crates keep feature flags scoped to their actual responsibility. Common
workspace feature names may appear where they are part of the crate contract:

| Feature | Meaning |
|---------|---------|
| `pure-rust` | Enables pure-Rust parser/governance support where applicable. |
| `tree-sitter-standard` | Enables standard Tree-sitter backend policy where applicable. |
| `tree-sitter-c2rust` | Enables c2rust Tree-sitter backend policy where applicable. |
| `glr` | Enables GLR-related policy or parser support where applicable. |
| `strict_api` | Denies unreachable public items for the crate. |
| `strict_docs` | Denies missing documentation for the crate. |

## Adding Dependencies

These support crates are ordinary workspace members, but they are not currently
registered under `[workspace.dependencies]`. Use explicit package-local paths
unless a later PR promotes a support crate into the shared dependency table:

```toml
[dependencies]
adze-bdd-governance-core = { version = "0.1.0", path = "../bdd-governance-core" }
```

# AGENTS.md — Adze

Instructions for autonomous AI agents (OpenAI Codex, etc.) working on this repository.

## Operating Repo

Active swarm implementation, CI, source-hygiene, and productization work targets
`EffortlessMetrics/adze-swarm`.

Public `EffortlessMetrics/adze` remains the release, publishing, and external
contribution intake surface. Do not open new swarm work against public `adze`
or `adze-dev` unless the user explicitly asks for a public-repo promotion or
sync.


## Repo Source-of-Truth Stack

This repo uses a linked source-of-truth stack:

```text
Roadmap → Proposal → Spec → ADR → Plan → Active goal → PR → Proof
```

Before changing files for lane work, read:

1. `docs/reference/SPEC_SYSTEM.md`
2. `.adze/goals/active.toml`
3. The linked implementation plan
4. The linked spec for the selected work item
5. Linked ADRs

Before opening a PR, refresh the live queue:

```bash
gh pr list --repo EffortlessMetrics/adze-swarm --state open
```

Check for same-title, same-scope, or overlapping work. If a similar PR already
exists, do not open another PR. Review the existing PR and report whether it
should be merged, amended, superseded, or closed.

### Scope rule

Implement one work item per PR unless the selected plan item explicitly allows a
bundled documentation batch. Docs-only artifacts are separate semantic changes:

- proposal PRs explain why;
- spec PRs define behavior;
- ADR PRs record durable decisions;
- plan PRs define sequencing;
- active goal PRs define current execution state.

Runtime/code PRs must link to the spec and plan item they implement.

Do not duplicate an open work item. Queue hygiene is part of the source-of-truth
contract: one active PR per semantic work item unless the maintainer explicitly
asks for competing alternatives.

### Proof rule

Run the proof commands listed in the plan item. If a proof command cannot run,
record the command, reason unavailable, substitute evidence if any, and whether
that blocks merge.

### Generated status rule

Do not hand-edit generated status. Run the generator or checker named in the
plan.

### Policy rule

If you add an exception, add or update the relevant `policy/*.toml` ledger with
owner, reason, `covered_by`, `created`, `review_after`, and `expires` when the
exception is temporary.

## Project Overview

Adze is an AST-first grammar toolchain for Rust. It generates Tree-sitter parsers from Rust type annotations using a pure-Rust GLR implementation.

- **Language**: Rust 2024 edition
- **MSRV**: 1.95.0
- **Workspace**: 29 crates
- **Command runner**: `just` (see `justfile`)

## Setup

The toolchain is auto-configured via `rust-toolchain.toml` — no manual Rust installation steps needed.

```bash
# Verify toolchain
rustc --version   # Should be >= 1.95.0
just --version    # Command runner

# System dependency (only needed for ts-bridge)
# apt install libtree-sitter-dev
```

## Essential Commands

### PR Gate (MUST PASS before submitting)

```bash
just ci-supported
```

This runs formatting, clippy, and tests on the 7 core pipeline crates. It is the single required check for branch protection.

### Building

```bash
cargo build                # Build all workspace members
cargo build -p adze        # Build specific crate
just build                 # Build everything
```

### Testing

```bash
just test                  # Core lib tests (recommended)
cargo t2                   # All tests, 2 threads
cargo test -p <crate>      # Test specific crate
cargo test test_name       # Run specific test

# Snapshot testing
cargo insta review         # Review snapshot changes
just snap                  # Same as above

# Advanced
just matrix                # Feature matrix testing
just mutate                # Mutation testing (adze-ir default)
just mutate-all            # Mutation testing all crates
```

### Fast Iteration

For quick iteration during development, use:
```bash
just check-fast  # Fast check with minimal debug info
```

This uses the `dev-fast` profile which sacrifices some debug info for faster builds.

### Build Caching with sccache

For faster rebuilds, especially after `cargo clean`, use sccache:

```bash
# Install sccache
cargo install sccache

# Enable in your shell
export RUSTC_WRAPPER=sccache
```

Add to `~/.bashrc` or `~/.zshrc` for persistence:
```bash
export RUSTC_WRAPPER=sccache
```

sccache caches compiled artifacts, significantly reducing rebuild times. This is
still useful for the post-collapse 29-package workspace, especially after
`cargo clean` or toolchain updates.

### Linting

```bash
just fmt                   # cargo fmt --all --check
just clippy                # clippy on core crates, -D warnings
cargo fmt --all            # Auto-format
```

### MSRV Verification

```bash
just check-msrv            # Verify all Cargo.toml rust-version fields match
```

## Architecture

### Core Pipeline (7 crates — PR gate scope)

These are checked by `just ci-supported`:

| Crate | Path | Purpose |
|-------|------|---------|
| `adze` | `runtime/` | Main runtime library, `Extract` trait |
| `adze-macro` | `macro/` | Proc-macro attributes |
| `adze-tool` | `tool/` | Build-time code generation |
| `adze-common` | `common/` | Shared grammar expansion |
| `adze-ir` | `ir/` | Grammar IR with GLR support |
| `adze-glr-core` | `glr-core/` | GLR parser generation |
| `adze-tablegen` | `tablegen/` | Table compression, FFI generation |

### Other Layers

- **`runtime2/`** — Production GLR runtime with Tree-sitter compatible API
- **`grammars/`** — Language implementations: `python`, `javascript`, `go`, `python-simple`, `test-vec-wrapper`
- **`crates/`** — durable support crates after the microcrate-to-SRP collapse:
  `bdd-governance-core`, `common-type-ops-core`, `linecol-core`,
  `parsetable-metadata`, plus the excluded `ts-c-harness`
- **`tools/ts-bridge/`** — Tree-sitter parse table extraction (excluded from workspace)
- **`cli/`**, **`lsp-generator/`**, **`playground/`**, **`wasm-demo/`** — Developer tools
- **`golden-tests/`** — Tree-sitter parity validation
- **`benchmarks/`** — Criterion benchmarks
- **`testing/`**, **`glr-test-support/`**, **`test-mini/`** — Test infrastructure

### Workspace Exclusions

These are excluded from workspace commands — build with `-p` explicitly:
```
runtime/fuzz, tools/ts-bridge, crates/ts-c-harness, example
```

## Code Conventions

### Dependencies

Always use workspace dependencies:
```toml
[dependencies]
serde = { workspace = true }
thiserror = { workspace = true }
```

Available workspace deps: `tree-sitter`, `serde`, `serde_json`, `proptest`, `insta`, `criterion`, `thiserror`, `syn`, `quote`, `proc-macro2`, `anyhow`, `tempfile`, `indexmap`, `bincode`, `clap`, `rayon`, `rustc-hash`, `smallvec`, `regex`

### Error Types

Library crates use `thiserror`:
```rust
#[derive(Debug, thiserror::Error)]
pub enum MyError {
    #[error("description: {0}")]
    Variant(String),
}
```

Application crates use `anyhow`.

### Workspace Lints

Enforced via `[workspace.lints.rust]`:
- `unsafe_op_in_unsafe_fn = "deny"`
- `unused_must_use = "deny"`
- `missing_docs = "warn"`
- `unused_extern_crates = "deny"`

### Collections

- `rustc_hash::FxHashMap` in hot paths (parser tables, symbol lookups)
- `smallvec::SmallVec` for small stack-allocated collections
- `indexmap::IndexMap` when insertion order matters

### Testing Patterns

- Snapshots: `insta::assert_snapshot!()` — review with `cargo insta review`
- Property tests: `proptest` crate
- Feature-gated helpers: `#[cfg(feature = "test-api")]`
- Test naming: `test_<what>_<condition>_<expected>`
- Default test threads: 2 (`RUST_TEST_THREADS=2`)

## CI Workflows

16 workflows in `.github/workflows/`. Key ones:

| Workflow | Purpose |
|----------|---------|
| `ci.yml` | Main CI with `ci-supported` job (PR gate) |
| `pure-rust-ci.yml` | Pure-Rust implementation |
| `core-tests.yml` | Core crate testing |
| `golden-tests.yml` | Tree-sitter parity |
| `microcrate-ci.yml` | Post-collapse support/governance risk packs |
| `fuzz.yml` | Fuzz testing |
| `benchmarks.yml` | Performance benchmarks |

See `docs/status/KNOWN_RED.md` for intentional exclusions.

## Verification Checklist

Before submitting any changes, run these in order:

```bash
# 1. Format
cargo fmt --all

# 2. Lint
just clippy

# 3. Test
just test

# 4. Full PR gate
just ci-supported

# 5. If snapshots changed
cargo insta review
```

If all pass, the PR is ready for review.

## Environment Variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `RUST_TEST_THREADS` | `2` | Test thread concurrency |
| `RAYON_NUM_THREADS` | `4` | Rayon thread pool size |
| `CARGO_BUILD_JOBS` | `4` (CI: `2`) | Parallel build jobs |
| `TOKIO_WORKER_THREADS` | `2` | Tokio worker threads |
| `ADZE_EMIT_ARTIFACTS` | unset | Set `true` to output generated grammar files |
| `ADZE_LOG_PERFORMANCE` | unset | Set `true` for GLR performance logging |

## Key Files

- `justfile` — Development recipes
- `rust-toolchain.toml` — Toolchain pinning
- `Cargo.toml` — Workspace root with 29 members
- `.githooks/pre-commit` — Pre-commit checks (install via `.githooks/install.sh`)
- `docs/status/KNOWN_RED.md` — Intentional CI exclusions
- `scripts/test-matrix.sh` — Feature matrix testing
- `scripts/check-test-connectivity.sh` — Verify no tests are silently disabled

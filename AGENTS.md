# AGENTS.md — Adze

Instructions for autonomous AI agents (OpenAI Codex, etc.) working on this repository.

## Operating Repo

Active swarm implementation, CI, source-hygiene, and productization work targets
`EffortlessMetrics/adze-swarm`.

Public `EffortlessMetrics/adze` remains the release, publishing, and external
contribution intake surface. Do not open new swarm work against public `adze`
or `adze-dev` unless the user explicitly asks for a public-repo promotion or
sync.

For the durable repo-boundary and release-candidate workflow, read
`docs/reference/adze-swarm-operating-model.md`.


## Current execution contract

This is the canonical agent entrypoint; `CLAUDE.md` points here. Read this file,
`docs/reference/SPEC_SYSTEM.md`, the operating model, the current owning issue
and related PRs, then the linked accepted spec/ADR before changing a seam.

GitHub #853 defines the selected **0.10.0** product endpoint. #617 holds research;
#915–#920 own the current-v2 governance migration; #1029 owns these instructions.
GitHub issues/PRs hold live work. Accepted ADZE requirements/specs hold behavior,
PR-local slices hold scope and owned/shared/forbidden seams, receipts hold
execution evidence, and support tiers hold public claims.

There is no authoritative repository-global active task. `.adze/goals` files
and older singleton checks are migration artifacts, not a work-selection lock.
Do not delete or bypass a still-enforced check: #920 owns retirement after
structural parity. Record a concrete migration blocker for the affected lane
rather than inventing an active-goal entry or stopping independent work.

### Select, implement, review, continue

Refresh the live queue before opening or taking over work:

```bash
gh pr list --repo EffortlessMetrics/adze-swarm --state open
gh issue view 853 --repo EffortlessMetrics/adze-swarm
```

Read the selected issue, its current PR head and review findings. Reuse an
existing same-scope PR; do not create a competing branch. One coherent work item
belongs in each PR, but several independent PRs/workflows may progress at once.
Each writer has an isolated worktree and explicit seam ownership. Coordinate
shared compiler/runtime/CLI interfaces; waiting CI or a baseline worker does not
serialize unrelated useful work. Research answers a named question with a
bounded output; it is not a recurring permission request.

Use focused red/green tests, substantive review and repair before integration
qualification. A previous clean review never suppresses a new finding. Fix a
coherent defect in its owning PR, or link a separate durable owner. Do not change
an oracle merely because the implementation disagrees with it.

After each merge or checkpoint, reconcile the landed child acceptance and pick
the next eligible issue under #853. A session percentage, finished PR batch,
empty queue or green subset is not product completion. Stop for the selected
product acceptance being satisfied, Steven's pause, or concrete blockers covering
all useful remaining work. A blocked lane names the missing capability, decision,
interface or evidence while independent lanes continue.

### Proof and landing

#1025–#1028 own the requested lifecycle:

```text
focused local proof + review + repair
 -> exact source head H + revocable conditional landing authorization
 -> cheap trusted admission
 -> actual integration composition G qualified once
 -> protected landing M + cheap acceptance readback
```

H, a PR test-merge/check subject, G and M are different identities. New findings,
changed heads and revoked authorization invalidate eligibility. Successful G
fulfills the conditional landing decision without a routine second approval wait.

**Deployment is not inferred from this text.** At the October 22, 2026 readback,
#1070 restores expensive non-draft PR qualification before queue qualification;
that is the recorded workaround, not the requested final spending policy.
Consult #1028 and current effective protection/workflows before acting. Do not
remove required producers before their replacement works, synthesize passing
proof statuses, relax protection, or treat Ready/reopen as a cheap control action.
Finish the protected cutover without stranding otherwise qualified candidates.

Record source/build identity, features, full command, exit status and actual
selected/executed/ignored counts. Zero selected tests, an ignored-only run, absent
failure text, or a count of `test result:` lines is not executed proof. Record
unavailable commands and their exact effect on qualification; substitute checks
retain their actual scope. A source review or model cross-check is not a Rust run.

Cancel only explicitly owned process/run IDs after checking repository, workflow
identity, event and subject/ref. A repository-wide sweep based on workflow display
names is forbidden. Do not discard in-progress attempts before selecting the
current required check: an older success must not hide current pending/failing
work. Preserve actual group membership and current authorization; queue limits or
one PR number in a ref are not a membership oracle.

Clean only owned worktrees/caches after changes and receipts are retained. Never
delete another worker's uncommitted work or cancel an unknown owner's run.

### Generated status and policy

Run the named generator/checker; do not hand-edit generated status. New exceptions
need the relevant policy ledger's owner, reason, coverage, review date and required
expiry. Baseline generation proposes exceptions; it does not approve them. Keep
scope and evidence accurate rather than weakening a gate to obtain green.

## Project Overview

Adze is an AST-first grammar toolchain for Rust. It generates Tree-sitter parsers from Rust type annotations using a pure-Rust GLR implementation.

- **Language**: Rust 2024 edition
- **MSRV**: 1.95.0
- **Workspace/package scope**: derive from `Cargo.toml` and the release graph; do not treat the core proof list as the publish denominator.
- **Command runner**: `just` (see `justfile`)

## Setup

`rust-toolchain.toml` selects the toolchain. Verify the required tools are actually available before reporting a build or test receipt.

```bash
# Verify toolchain
rustc --version   # Should be >= 1.95.0
just --version    # Command runner

# System dependency (only needed for ts-bridge)
# apt install libtree-sitter-dev
```

## Essential Commands

### GitHub PR Gate

The required `adze-swarm` merge gates are:

```text
Rust Small Result
Product Proof Result
```

These are the two contexts in the live branch protection for `main` (mirrored
in `.github/settings.yml`). `Rust Small Result` is the normalized result check
from the routed Rust-small workflow. Do not require the implementation lanes
(`Rust Small on CPX42`, `Rust Small on CX43`, `Rust Small on CX33`,
`Rust Small on CX53`, or `Rust Small on GitHub Hosted`) directly; exactly one
of those may run or skip depending on router state.

### Local Supported Proof

Use this when the selected supported-core proof obligation requires the aggregate:

```bash
just ci-supported
```

This runs formatting, clippy, and tests on the 7 core pipeline crates. It is
the local supported/product proof, not the `adze-swarm` branch-protection
context. Do not precede it with duplicate full core lint/test commands by habit.
Start with focused tests; retain all distinct selected obligations.

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
still useful for the workspace, especially after
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

### Core Pipeline (7 crates — supported proof scope)

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

- **`runtime2/`** — Development-only proving ground; not the supported production runtime.
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

These are excluded from workspace commands. Use the selected package’s explicit
`--manifest-path <path>/Cargo.toml` and required features where appropriate:
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

## Verification and CI selection

Read `.github/workflows/`, `docs/ci/inventory.md`, and the current #1028 readback
for actual triggers and required contexts. A workflow catalogue is not an
instruction to run every lane. `docs/status/KNOWN_RED.md` records exclusions.

For each slice:

1. Run focused failing/passing tests with the required feature profile; verify
   nonzero selection. Run relevant formatting/lint checks.
2. Review the diff and repair findings. Recheck affected obligations after edits.
3. Run the selected aggregate only where required, without duplicating its work.
   Broad matrix, fuzz, coverage and performance campaigns need an explicit scope.
4. Review intentional snapshot changes against independently expected semantics.
5. Admit only the exact reviewed head through the current protected path. Retain
   integration and landing identities; reconcile the actual landed acceptance.

`--all-features` is not a substitute for separately advertised feature profiles.
Do not invalidate unaffected evidence merely because main moved; do refresh
proof whose source, dependencies, configuration or integration composition changed.

## Environment Variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `RUST_TEST_THREADS` | `2` | Test thread concurrency |
| `RAYON_NUM_THREADS` | `4` | Rayon thread pool size |
| `CARGO_BUILD_JOBS` | `4` (CI: `2`) | Parallel build jobs |
| `TOKIO_WORKER_THREADS` | `2` | Tokio worker threads |
| `ADZE_EMIT_ARTIFACTS` | unset | Set `true` to output generated grammar files |
| `ADZE_LOG_PERFORMANCE` | unset | Enables GLR performance logging |

## Key Files

- `justfile` — Development recipes
- `rust-toolchain.toml` — Toolchain pinning
- `Cargo.toml` — Workspace membership and dependency definitions
- `.githooks/pre-commit` — Pre-commit checks (install via `.githooks/install.sh`)
- `docs/status/KNOWN_RED.md` — Intentional CI exclusions
- `scripts/test-matrix.sh` — Feature matrix testing
- `scripts/check-test-connectivity.sh` — Verify no tests are silently disabled

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Use TDD. Red-Green-Refactor, spec driven design. User-story driven design.


## First Read: Source-of-Truth Stack

Before changing files for lane work, read:

1. `docs/reference/SPEC_SYSTEM.md`
2. `.adze/goals/active.toml`
3. The linked implementation plan
4. The linked spec for the selected work item
5. Any linked ADRs

Before opening a PR, refresh the live queue:

```bash
gh pr list --repo EffortlessMetrics/adze-swarm --state open
```

If a same-title, same-scope, or overlapping PR already exists, do not open a
duplicate. Review the existing PR and report whether it should be merged,
amended, superseded, or closed.

Work on exactly one work item at a time unless the selected plan item explicitly
allows a bundled documentation batch. Do not create a new lane unless asked. Do
not mix proposal, spec, ADR, plan, active-goal, and runtime changes unless the
selected work item says to. Do not claim success without running proof commands
or recording why proof is unavailable. Do not hand-edit generated status unless
the plan says to.

Stop and report instead of guessing when the active goal is missing or stale,
linked specs are missing, proof commands cannot run, generated status differs
from committed status, requested work conflicts with an ADR, or the branch
contains unrelated staged changes.

## Quick Reference

```bash
just ci-supported          # Required PR gate — must pass before merge
cargo t2                   # Run tests (2 threads, stable)
just test                  # Run core lib tests
just clippy                # Lint core crates
cargo fmt --all --check    # Check formatting
cargo insta review         # Review snapshot changes (or: just snap)
just pre                   # Run pre-commit checks locally
just policy                # Run all policy-stack checks (advisory)
```

## Requirements

### Minimum Rust Version (MSRV)
- **Rust 1.95.0** or later
- **Rust 2024 Edition** - all workspace crates use the latest edition
- Components: `rustfmt`, `clippy` (automatically configured via `rust-toolchain.toml`)

### System Dependencies
- **libtree-sitter-dev**: Required for ts-bridge tool (production mode)
- **libclang**: Required for binding generation in some features
- **Git**: Version control and automated testing workflows
- **just**: Command runner for development recipes (`justfile`)

### Supported Platforms
- Linux (primary development and CI)
- macOS (tested via CI)
- Windows (tested via CI)
- WebAssembly (wasm32-unknown-unknown, wasm32-wasi)

## Common Development Commands

### Building
```bash
cargo build                # Build all workspace members
cargo build --release      # Build with release optimizations
cargo build -p adze        # Build a specific package
just build                 # Build everything
just release               # Release build
```

### Testing
```bash
# Core testing
just test                  # Run core lib tests (adze, adze-glr-core, adze-ir, adze-tablegen)
cargo t2                   # Run tests with 2 threads (stable)
cargo test -p adze         # Test a specific package
cargo test test_name       # Run a specific test
cargo test -- --nocapture  # Run tests with output displayed

# Snapshot testing (uses insta)
cargo insta review         # Review and accept snapshot changes
just snap                  # Alias for cargo insta review

# Feature matrix and advanced testing
just matrix                # Run feature matrix tests (scripts/test-matrix.sh)
just mutate                # Mutation testing on adze-ir (default)
just mutate crate=adze     # Mutation testing on specific crate
just mutate-all            # Mutation testing on all supported crates

# Integration tests
cargo test -p adze-glr-core --features test-api  # With internal debug helpers
cargo test -p adze-golden-tests                   # Golden tests vs Tree-sitter

# Concurrency-capped testing
cargo test-safe            # Run tests with safe defaults
cargo test-ultra-safe      # Run tests with 1 thread
./scripts/test-capped.sh   # Automatic concurrency detection
./scripts/test-local.sh    # Local test runner with nextest fallback
```

### Linting and Formatting
```bash
just fmt                   # Check formatting (cargo fmt --all --check)
just clippy                # Lint core crates with -D warnings
cargo clippy --all         # Lint all workspace members
cargo fmt                  # Auto-format code
just check-msrv            # Verify MSRV consistency across all Cargo.toml files
```

### PR Gate
```bash
just ci-supported          # THE required PR gate — runs fmt, clippy, and tests on core crates
```
This is the single supported CI lane for branch protection. It checks: `adze`, `adze-macro`, `adze-tool`, `adze-common`, `adze-ir`, `adze-glr-core`, `adze-tablegen`. See `docs/status/KNOWN_RED.md` for exclusions.

### Pre-commit Hooks
```bash
just pre                   # Run pre-commit checks
just pre-tests             # Pre-commit with test clippy enabled
just pre-docs              # Pre-commit with strict docs enabled
just pre-warn              # Pre-commit with warnings as errors
```

Hooks are in `.githooks/pre-commit` (install via `.githooks/install.sh`).

## Architecture Overview

Adze is an AST-first grammar toolchain for Rust. Define the shape of your syntax in Rust, then parse into that shape. Build-time: types -> macros -> IR -> tables. Run-time: text -> GLR -> trees -> typed values.

The workspace has **75 members** organized into several layers:

### Core Pipeline (7 crates — covered by `just ci-supported`)

1. **`adze` (runtime crate)** — `/runtime/` — Main runtime library with `Extract` trait and parsing
   - Two Tree-sitter backends: `tree-sitter-c2rust` (default, WASM) and `tree-sitter-standard` (C)
2. **`adze-macro`** — `/macro/` — Proc-macro attributes (`#[adze::grammar]`, `#[adze::language]`, `#[adze::leaf]`)
3. **`adze-tool`** — `/tool/` — Build-time code generation via `build_parsers()`
4. **`adze-common`** — `/common/` — Shared grammar expansion logic
5. **`adze-ir`** — `/ir/` — Grammar Intermediate Representation with GLR support, optimization, validation
6. **`adze-glr-core`** — `/glr-core/` — GLR parser generation: FIRST/FOLLOW, LR(1), conflict resolution
7. **`adze-tablegen`** — `/tablegen/` — Table compression and FFI-compatible Language struct generation

### Grammar Implementations (5 crates)

Located in `grammars/`: `python`, `javascript`, `go`, `python-simple`, `test-vec-wrapper`

### Runtime Layer

- **`runtime2`** — `/runtime2/` — Production GLR runtime with Tree-sitter compatible API
  - `parser.rs` — GLR-compatible Parser API
  - `engine.rs` — GLR engine adapter and forest management
  - `builder.rs` — Forest-to-tree conversion with performance monitoring
  - `tree.rs` — Enhanced Tree with incremental editing support

### Governance-as-Code Micro-Crates (47 crates in `crates/`)

Policy enforcement, concurrency management, and testing infrastructure as code. Categories:
- **`concurrency-*`** (11): Thread pool caps, environment normalization, bootstrap policies
- **`governance-*`** (7): Runtime governance, matrix contracts, metadata
- **`bdd-*`** (7): BDD testing fixtures, governance, grammar analysis
- **`parser-*`** (4): Parser contracts, feature contracts, backend abstraction
- **`feature-policy-*`** (2): Feature flag policy enforcement
- **`runtime-governance*`** (4): Runtime governance API and matrix
- Other: `ts-format-core`, `linecol-core`, `stack-pool-core`, `parsetable-metadata`

These are tested via `microcrate-ci.yml`, **not** `just ci-supported`.

### Developer Tools

- **`cli/`** — Command-line interface
- **`lsp-generator/`** — LSP server generation
- **`playground/`** — Interactive grammar playground
- **`wasm-demo/`** — WebAssembly demonstration

### Supporting Crates

- **`glr-test-support/`** — Test utilities for GLR testing
- **`testing/`** — Shared testing infrastructure
- **`benchmarks/`** — Criterion benchmarks
- **`samples/downstream-demo/`** — Example downstream consumer
- **`test-mini/`** — Minimal test crate
- **`tests/governance/`** — Governance integration tests
- **`golden-tests/`** — Tree-sitter parity validation with SHA256 hash verification
- **`example/`** — Example grammars (excluded from workspace — mutually exclusive features)
- **`xtask/`** — Build automation tasks

### Workspace Exclusions

These crates are excluded from default workspace commands (build separately with `-p`):
```
exclude = ["runtime/fuzz", "tools/ts-bridge", "crates/ts-c-harness", "example"]
```

### Tools

- **`ts-bridge`** — `/tools/ts-bridge/` — Extracts parse tables from compiled Tree-sitter grammars (requires `libtree-sitter-dev`)

## CI and Workflows

16 workflows in `.github/workflows/`:

| Workflow | Purpose |
|----------|---------|
| `ci.yml` | Main CI — includes `ci-supported` job (PR gate) |
| `pure-rust-ci.yml` | Pure-Rust implementation tests |
| `core-tests.yml` | Core crate testing |
| `golden-tests.yml` | Tree-sitter parity validation |
| `microcrate-ci.yml` | Governance micro-crate testing |
| `fuzz.yml` | Fuzz testing |
| `benchmarks.yml` | Performance benchmarks |
| `performance.yml` | Performance tracking |
| `criterion-smoke.yml` | Benchmark smoke tests |
| `test-policy.yml` | Test policy enforcement |
| `smoke-ts-bridge.yml` | ts-bridge link verification |
| `ts-bridge-parity.yml` | ts-bridge parity tests |
| `ts-bridge-smoke.yml` | ts-bridge smoke tests |
| `clippy-quarantine-report.yml` | Clippy quarantine reporting |
| `release.yml` | Release automation |
| `mdbook.yml` | Documentation site build |

See `docs/status/KNOWN_RED.md` for intentional exclusions from the supported lane.

## Key Design Patterns

1. **Grammar Definition Flow**:
   - User defines grammar using Rust types with macro annotations
   - `build.rs` calls `adze_tool::build_parsers()` at build time
   - Tool extracts grammar from Rust code and generates Tree-sitter JSON grammar
   - Tree-sitter generates C parser from JSON
   - C parser is compiled and linked into the final binary

2. **Two-Stage Processing**:
   - Compile-time: Macros mark types but don't generate parser code
   - Build-time: Tool reads the marked types and generates actual parser

3. **Incremental Parsing** (Experimental, Currently Disabled):
   The incremental parsing path falls back to fresh parsing for consistency reasons.
   Infrastructure exists but has architectural issues. See `glr_incremental.rs:281-297`.
   Requires `incremental_glr` feature flag.

4. **Environment Variables**:
   - `ADZE_EMIT_ARTIFACTS=true`: Outputs generated grammar files for debugging
   - `ADZE_LOG_PERFORMANCE=true`: Enables GLR forest-to-tree performance logging
   - `RUST_TEST_THREADS=N`: Test thread concurrency (default: 2)
   - `RAYON_NUM_THREADS=N`: Rayon thread pool size (default: 4)
   - `CARGO_BUILD_JOBS=N`: Parallel build jobs (default: 2 in CI)
   - `TOKIO_WORKER_THREADS=N`: Tokio workers (default: 2)

### Working with the Codebase

When making changes:
1. Grammar expansion logic is shared between macro and tool in the `common` crate
2. The macro crate only provides attribute definitions, not implementations
3. The tool crate handles all build-time code generation
4. Test changes using the example crate which has comprehensive snapshot tests
5. Use `cargo insta review` to update snapshots when grammar output changes intentionally

### Pure-Rust Implementation Development

When working on the pure-Rust implementation:
1. The IR crate defines the grammar representation — modify this for new grammar features
2. The GLR core implements the parser generation algorithms — this is where conflict resolution happens
3. The tablegen crate handles compression — ensure bit-for-bit compatibility with Tree-sitter
4. Use `emit_ir!()` macro to debug grammar extraction
5. Test table generation with `cargo test -p adze-tablegen`
6. Verify Language struct layout matches Tree-sitter ABI exactly

### Testing Guidelines

1. **Grammar Tests**: Add new grammars to `/example/src/` with corresponding snapshot tests
2. **Compression Tests**: Verify table compression maintains Tree-sitter compatibility
3. **FFI Tests**: Ensure generated Language structs match C ABI requirements
4. **Integration Tests**: Test with real Tree-sitter grammars for validation
5. **GLR Runtime Tests**: Test GLR integration with `runtime2/tests/glr_parse.rs`
6. **Feature Flag Tests**: Test all feature combinations (`default`, `glr-core`, `incremental`, `incremental_glr`, `all-features`)
7. **Golden Tests**: Validate against Tree-sitter reference implementations with `cargo test -p adze-golden-tests`
8. **Serialization Tests**: Roundtrip testing for JSON and S-expression formats

### Concurrency Defaults

- Rust test threads: **2** (`RUST_TEST_THREADS`)
- Rayon thread pool: **4** (`RAYON_NUM_THREADS`)
- Tokio worker threads: **2** (`TOKIO_WORKER_THREADS`)
- Cargo build jobs: **4** (`CARGO_BUILD_JOBS`)

All caps are configurable via environment variables. The `preflight.sh` script automatically degrades to ultra-safe mode if the system is under high PID pressure.

### Test Connectivity Safeguards

Multiple layers prevent tests from being silently disconnected:
- **CI `test-connectivity` job**: Blocks `.rs.disabled` files, enforces non-zero test counts
- **Pre-commit hook** (`.githooks/pre-commit`): Prevents committing disabled test files
- **Local verification**: `./scripts/check-test-connectivity.sh`

### Policy stack

Three governance checks live under `policy/` and `xtask`:

| Check | Source | Command | Mode |
|-------|--------|---------|------|
| No-panic ledger | `policy/no-panic-allowlist.toml`, `docs/NO_PANIC_POLICY.md` | `just policy-no-panic` | advisory |
| Non-Rust file ledger | `policy/non-rust-allowlist.toml`, `docs/FILE_POLICY.md` | `just policy-files` | advisory |
| Lint policy manifest | `policy/clippy-lints.toml`, `docs/CLIPPY_POLICY.md` | `just policy-lints` | advisory |

Run all three via `just policy`. Reports land in `target/policy/`. The checks
are advisory until the no-panic baseline is committed; flip them to
`--mode blocking-allowlist` once the debt is receipted. Identity for
panic-family entries is `(path, family, selector)`; line/column drift never
invalidates an entry. New exceptions must be proposed via
`just policy-no-panic-propose` and reviewed before merging.

### Completed Milestones

#### GLR Incremental Parsing *(September 2025)*
GLR-aware incremental parsing with fork-aware subtree reuse, external scanner integration, and tree bridge. Currently uses conservative fallback to fresh parsing. 130+ tests validated.

#### GLR Parser Implementation *(August 2025)*
ActionCell architecture enabling multi-action cells with runtime forking. Fixed critical "State 0" bug for Python grammar (273 symbols, 57 fields). Concurrency caps system implemented.

#### Golden Test Integration
Golden tests validate adze parsers against Tree-sitter reference implementations using SHA256 hash verification. 100+ serialization roundtrip test cases.

See CHANGELOG.md for full history of milestones and fixes.

### Known Issues

1. **GLR Runtime Optimization**: Fork/merge logic needs performance tuning for large files
2. **External Scanner FFI**: Integration with C scanners needs final touches
3. **Incremental Parsing Disabled**: Falls back to fresh parsing due to architectural inconsistencies

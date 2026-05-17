# Contributing to Adze

Thank you for your interest in contributing! This guide covers everything you need to get started developing, testing, and submitting changes.

## Quick Start

```bash
# 1. Fork and clone
git clone https://github.com/<you>/adze.git && cd adze

# 2. Toolchain installs automatically via rust-toolchain.toml (Rust 1.95+)
rustup show  # verify toolchain

# 3. Check local development tools and get the shortest supported-lane path
./scripts/dev-doctor.sh

# 4. Build
cargo build

# 5. Test
cargo test

# 6. Lint
cargo fmt --all --check && cargo clippy --all -- -D warnings
```

**Find work**: Check [open issues](https://github.com/EffortlessMetrics/adze/issues) labeled `good first issue`.

## Prerequisites

- **Rust 1.95.0+** with `rustfmt` and `clippy` (configured via `rust-toolchain.toml`)
- **jq** for crate-aware checks
- **rg** (ripgrep) — optional but recommended
- **libtree-sitter-dev** — only needed for the `ts-bridge` tool
- **libclang** — only needed for binding generation in some features

Run `./scripts/dev-doctor.sh` after cloning to verify required tools and see
actionable install hints. If `just` is already installed, `just doctor` runs the
same check.

### Enable Git Hooks

```bash
git config core.hooksPath .githooks

# Ensure scripts are executable
git update-index --chmod=+x .githooks/pre-commit .githooks/pre-push \
  scripts/affected-crates.sh scripts/check-goto-indexing.sh
```

**Pre-commit (fast path)** — formats staged files with `rustfmt`, runs clippy on affected crates, blocks conflict markers.

**Pre-push (full validation)** — runs clippy across the entire workspace and tests the feature matrix.

## Building

```bash
cargo build                        # Build all workspace members
cargo build --release              # Build with release optimizations
cargo build -p adze                # Build only the runtime crate
cargo build -p adze-macro          # Build only the proc-macro crate
cargo build -p adze-tool           # Build only the build tool
```

## Running Tests

### Unit and Crate Tests

```bash
cargo test                                # All tests in the workspace
cargo test -p adze-glr-core               # Tests for a single crate
cargo test test_name                      # Run a specific test by name
cargo test test_name -- --nocapture       # Show stdout/stderr output
```

### Integration Tests

```bash
cargo test -p adze-golden-tests           # Validate against Tree-sitter reference parsers
cargo test -p adze-glr-core --features test-api  # Internal debug helpers
```

### Snapshot Tests (insta)

Snapshot tests live in `/example/src/` and use [insta](https://insta.rs/):

```bash
cargo test -p example                     # Run snapshot tests
cargo insta review                        # Interactively review pending snapshots
```

When you intentionally change grammar output, run `cargo insta review` to accept the new snapshots.

### Property-Based Tests

Property tests use randomized inputs to validate incremental parsing behavior:

```bash
cargo test -p adze --test property_incremental_test
```

### Feature Flag Combinations

Several features gate optional functionality. Test them explicitly:

```bash
cargo test --features external_scanners
cargo test --features incremental_glr
cargo test --all-features
```

### Fuzzing

Fuzz targets live in `/fuzz/fuzz_targets/`:

```bash
cd fuzz
cargo fuzz list                           # List available targets
cargo fuzz run <target> -- -max_total_time=60  # Run a target for 60 seconds
```

### Concurrency-Capped Testing

For stable results, especially in CI or resource-limited environments:

```bash
cargo t2                                  # 2 test threads
cargo test-safe                           # Safe defaults
cargo test-ultra-safe                     # 1 test thread
./scripts/test-capped.sh                  # Auto-detect and cap concurrency
./scripts/test-local.sh                   # Local runner with nextest fallback
```

### Test Connectivity

Verify that all test files are properly wired into the test harness:

```bash
./scripts/check-test-connectivity.sh
```

## Code Style

### Formatting

All code must pass `rustfmt` using the workspace configuration (`rustfmt.toml`, edition 2024):

```bash
cargo fmt --all             # Format everything
cargo fmt --all --check     # Check without modifying
```

### Linting

Clippy warnings are treated as errors in CI:

```bash
cargo clippy --all -- -D warnings
```

### Style Guidelines

- **Follow existing patterns** in the surrounding code.
- **No unnecessary comments** — code should be self-documenting. Only comment when clarification is genuinely needed.
- **Prefer editing over creating** — modify existing files when possible.
- **Use `debugln!(...)` over raw `eprintln!`/`println!`/`dbg!`** for debug output.
- **Doc comments**: Public APIs should have `///` doc comments. Include a one-line summary, examples where helpful, and `# Panics` / `# Errors` sections as appropriate.

## Pull Request Process

### Branch Naming

Use descriptive branch names with a category prefix:

- `feat/add-json-grammar`
- `fix/eof-symbol-collision`
- `docs/update-contributing`
- `test/golden-test-python`
- `refactor/simplify-ir-optimizer`

### Commit Messages

We use [Conventional Commits](https://www.conventionalcommits.org/):

| Prefix | Use for |
|--------|---------|
| `feat:` | New features |
| `fix:` | Bug fixes |
| `docs:` | Documentation changes |
| `test:` | Test additions or changes |
| `chore:` | Maintenance tasks |
| `refactor:` | Code restructuring without behavior change |
| `perf:` | Performance improvements |

### Submitting a PR

1. Create a feature branch from `main`.
2. Make focused commits following the convention above.
3. Run the full lint and test suite locally before pushing:
   ```bash
   cargo fmt --all --check && cargo clippy --all -- -D warnings && cargo test
   ```
4. Open a PR against `main` and fill out the PR template.
5. All CI checks must pass before merge.

### CI Requirements

Every PR must pass:

- **Formatting**: `cargo fmt --all --check`
- **Linting**: `cargo clippy --all -- -D warnings`
- **Tests**: `cargo test` across all feature combinations
- **Test connectivity**: No `.rs.disabled` files; non-zero test counts per crate

PRs are squash-merged. Your PR title becomes the merge commit message, so make it descriptive.

### Temporary Worktree Hygiene

After closeout, contributors often use temporary worktrees for PR follow-ups. Keep cleanup deterministic:

- create temporary worktrees with a clear `adze-local-*` naming pattern
- validate one path before removing it to avoid standalone `.git` drift
- prune stale registrations after manual cleanup

Example flow:

```bash
# inspect known worktrees
./scripts/cleanup-worktrees.sh list

# validate one path before removing it
./scripts/cleanup-worktrees.sh status /tmp/adze-local-improvements

# if it is a registered linked worktree, remove it safely
./scripts/cleanup-worktrees.sh cleanup /tmp/adze-local-improvements

# trim stale registrations
./scripts/cleanup-worktrees.sh prune-stale
```

On Windows PowerShell, prefer `just worktree-list` and `just worktree-prune-stale` for the common paths. Run direct script calls from Git Bash or another shell where `bash` is on `PATH`.

Use `rm -rf` only for standalone temporary clones that are no longer registered.

## Architecture Overview

Adze is an AST-first grammar toolchain for Rust. You define syntax using annotated Rust types, then parse text into those types. For full architecture details, see [`CLAUDE.md`](CLAUDE.md).

### Core Crates

| Crate | Location | Purpose |
|-------|----------|---------|
| `adze` | `/runtime/` | Main runtime library with `Extract` trait and parsing |
| `adze-glr-core` | `/glr-core/` | GLR parser generation (FIRST/FOLLOW, LR(1), conflicts) |
| `adze-ir` | `/ir/` | Grammar intermediate representation |
| `adze-tablegen` | `/tablegen/` | Parse table generation and compression |

### Supporting Crates

| Crate | Location | Purpose |
|-------|----------|---------|
| `adze-macro` | `/macro/` | Procedural macros (`#[adze::grammar]`, `#[adze::leaf]`, etc.) |
| `adze-tool` | `/tool/` | Build-time code generation (`build_parsers()`) |
| `adze-common` | `/common/` | Shared grammar expansion logic |
| `example` | `/example/` | Example grammars and snapshot tests |
| `golden-tests` | `/golden-tests/` | Reference parser validation (Python, JavaScript) |
| `ts-bridge` | `/tools/ts-bridge/` | Tree-sitter parse table extraction |

### Key Design Patterns

1. **Two-stage processing**: Macros mark types at compile time; the build tool generates parsers at build time.
2. **GLR parsing**: Multi-action cells enable parsing ambiguous grammars without manual conflict resolution.
3. **Feature-gated backends**: `tree-sitter-c2rust` (pure Rust, WASM-compatible) or `tree-sitter-standard` (C runtime).

### Further Reading

- [`CLAUDE.md`](CLAUDE.md) — Detailed architecture, crate descriptions, and design decisions
- [`QUICK_START.md`](QUICK_START.md) — Getting started with Adze as a user
- [`FAQ.md`](FAQ.md) — Frequently asked questions
- [`PERFORMANCE_GUIDE.md.ISSUES_DOCUMENTED`](PERFORMANCE_GUIDE.md.ISSUES_DOCUMENTED) — Performance tuning notes

## How to Add a New Grammar

1. **Create a grammar file** in `/example/src/` (e.g., `my_grammar.rs`).
2. **Define the grammar** using Adze annotations:
   ```rust
   use adze_macro::grammar;

   #[grammar]
   pub mod my_grammar {
       #[adze::language]
       pub struct Language;

       // Define your rules using #[adze::leaf], structs, and enums
   }
   ```
3. **Register the module** in `/example/src/lib.rs`:
   ```rust
   pub mod my_grammar;
   ```
4. **Add snapshot tests** — run `cargo test -p example` and then `cargo insta review` to accept the generated snapshots.
5. **Verify** — the build tool (`build.rs`) will automatically generate the Tree-sitter JSON grammar and C parser at build time.

See existing grammars in `/example/src/` (e.g., `arithmetic.rs`, `words.rs`, `optionals.rs`) for working examples.

## How to Add a New Test

### Unit Test

Add a `#[test]` function in the relevant source file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_my_feature() {
        // ...
    }
}
```

### Integration Test

Create a file in the crate's `tests/` directory (e.g., `runtime/tests/my_test.rs`). Integration tests have access to the crate's public API.

### Snapshot Test

Add a test in `/example/src/` that parses input and snapshots the result using `insta::assert_snapshot!`. Run `cargo insta review` to accept new snapshots.

### Golden Test

Add a test case to `/golden-tests/` that compares Adze parser output against a Tree-sitter reference parser using SHA256 hash verification.

### Verify Connectivity

After adding tests, verify they are discovered by the test harness:

```bash
cargo test -p <crate> -- --list           # List discovered tests
./scripts/check-test-connectivity.sh      # Check all crates
```

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `ADZE_EMIT_ARTIFACTS=true` | Output generated grammar files for debugging |
| `ADZE_LOG_PERFORMANCE=true` | Enable GLR forest-to-tree performance logging |
| `RUST_TEST_THREADS=N` | Limit test thread concurrency (default: 2) |
| `RAYON_NUM_THREADS=N` | Control rayon thread pool size (default: 4) |
| `TOKIO_WORKER_THREADS=N` | Tokio async worker threads (default: 2) |

## Troubleshooting

### "Too many open files" or "Cannot create thread"

System resource exhaustion under high concurrency. Use capped testing:

```bash
./scripts/preflight.sh          # Check system pressure
cargo test-ultra-safe           # 1 test thread
```

### Snapshot test failures after intentional changes

Run `cargo insta review` to interactively accept or reject the new snapshots.

### Build failures in `example` crate

The example crate depends on build-time code generation. If the grammar JSON or C parser is stale:

```bash
cargo clean -p example && cargo build -p example
```

Set `ADZE_EMIT_ARTIFACTS=true` to inspect the generated grammar files in `target/debug/build/`.

### Feature-gated compilation errors

Some code is behind feature flags. If you see missing-symbol errors, check which features are enabled:

```bash
cargo test --all-features       # Enable everything
cargo test --features test-api  # Enable internal test helpers
```

### Inconsistent test results across runs

Pin concurrency to eliminate non-determinism:

```bash
RUST_TEST_THREADS=1 RAYON_NUM_THREADS=1 cargo test
```

### Test connectivity failures

CI enforces that every crate has non-zero test counts and no `.rs.disabled` files:

```bash
./scripts/check-test-connectivity.sh    # Run the same check locally
```

## Release Readiness Checklist

- [ ] Update crate versions and changelog entries together (`CHANGELOG.md` and `book/src/appendix/changelog.md`).
- [ ] Verify all required checks pass locally:
  - `cargo fmt --all --check`
  - `cargo clippy --all -- -D warnings`
  - `cargo test --all-features`
- [ ] Decide and set release-surface mode (`RELEASE_SURFACE_MODE=fixed` or `auto`) and optional `RELEASE_CRATE_FILE` before validating.
- [ ] Decide fixed-mode release-surface strictness (`strict_publish_surface` in workflow or `STRICT_PUBLISH_SURFACE` locally) before running the Release workflow.
- [ ] If using manual Release workflow dispatch, set `release_surface_mode` and `release_crate_file` as needed.
- [ ] Run release validation for crates scheduled for publish (`cargo publish --dry-run -p <crate>`).
- [ ] Confirm docs and migration notes are updated for any API/behavior changes.
- [ ] Get maintainer sign-off for compatibility notes and known issues before tagging.

## Getting Help

- Browse [open issues](https://github.com/EffortlessMetrics/adze/issues) and [discussions](https://github.com/EffortlessMetrics/adze/discussions)
- Check [`CLAUDE.md`](CLAUDE.md) for detailed architecture information
- Run tests with `--nocapture` for debug output
- Use `RUST_LOG=debug` for verbose logging

## Code of Conduct

All participants are expected to follow the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md). Please report unacceptable behavior to git@effortlesssteven.com.

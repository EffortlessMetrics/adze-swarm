# Developer Guide - Adze

> **Doc status:** Up to date for Adze 0.8.0-dev.

## Repository Target

Active swarm implementation, CI, source-hygiene, and productization work targets
`EffortlessMetrics/adze-swarm`.

Public `EffortlessMetrics/adze` remains the release, publishing, and external
contribution intake surface. Do not open new swarm work against public `adze`
unless that work is explicitly being promoted or synced.

## Prerequisites

### System Requirements
- **Rust 1.95.0+** (2024 edition support)
- **just**: Command runner (optional but recommended)

Optional tooling:

- **libtree-sitter-dev**: Needed only for `tools/ts-bridge` work.
- **Node.js / tree-sitter CLI**: Needed only for legacy compatibility
  experiments that explicitly invoke Tree-sitter's CLI.
- **C compiler**: Needed only for native integration experiments or crates that
  explicitly depend on C tooling.

## Maintenance Lanes

Adze uses a "Support Lane" model to keep the core green while allowing experimental features to evolve.

### 🟢 Supported Lane (Must be Green)
These crates are the core product. CI enforces passing tests and lints on every PR.
- `adze` (core runtime)
- `adze-macro`
- `adze-tool`
- `adze-common`
- `adze-ir`
- `adze-glr-core`
- `adze-tablegen`

### 🟡 Experimental/Community Lane (Best Effort)
These crates are useful but may break during major refactors.
- `grammars/*` (Python, JS, Go examples)
- `example/` (Arithmetic demo)
- `runtime2` (alternative runtime path)
- `cli/`
- `playground/`

To run the supported gate locally:
```bash
just ci-supported
```

## Temporary Worktree Cleanup

Adze PR work should use linked git worktrees when a branch needs an isolated checkout. A disposable standalone clone is also valid for experiments, but cleanup differs because a linked worktree has a `.git` file while a standalone clone has a `.git/` directory.

After a PR lands or is abandoned:

```bash
# Inspect registered worktrees for this checkout.
just worktree-list

# Classify a specific temporary path before removing it.
scripts/cleanup-worktrees.sh status /tmp/adze-example-pr
```

On Windows PowerShell, prefer the `just` targets for common cleanup commands. For direct script calls, run them from Git Bash or another shell where `bash` is on `PATH`.

Use the helper only for registered linked worktrees:

```bash
scripts/cleanup-worktrees.sh cleanup /tmp/adze-example-pr
```

If the status command reports `standalone-repo`, inspect the path for uncommitted work and remove it manually only after confirming it is disposable:

```bash
rm -rf /tmp/adze-example-pr
```

If a temp path was already deleted and git still lists it, prune stale metadata:

```bash
just worktree-prune-stale
```

The helper refuses to remove the main repository root and refuses standalone clones so linked-worktree cleanup does not accidentally delete an independent checkout.

## Quick Commands

### Core Development
```bash
# Run tests for core crates only (fast)
just test

# Run strict linting
just clippy

# Format code
just fmt
```

### Full Workspace
```bash
# Build everything (including experimental)
cargo build --workspace

# Run all tests (may require heavy resources)
cargo test --workspace
```

### Grammar Development
```bash
# Build a specific grammar
cargo build -p adze-python

# Snapshot testing
cargo test -p adze-example
cargo insta review
```

### Debugging
If you need to inspect generated parsers:
```bash
export ADZE_EMIT_ARTIFACTS=true
cargo build -p adze-example
# Check target/debug/build/*/out/grammar_*/
```

## Release Process

1. **Verify State**: Ensure `just ci-supported` passes.
2. **Update Docs**: Check [`docs/status/FRICTION_LOG.md`](./status/FRICTION_LOG.md) and [`CHANGELOG.md`](../CHANGELOG.md).
3. **Bump Version**: Update `version` in `Cargo.toml` files (workspace members).
4. **Tag**: `git tag v0.8.0`
5. **Publish**: `cargo publish` (scripted in CI).
6. **Release surface configuration**: choose `RELEASE_SURFACE_MODE` (`fixed`/`auto`) and optional `RELEASE_CRATE_FILE` override as needed.
7. **Release surface strictness**: decide whether to run `strict_publish_surface` (fixed mode only) in the GitHub Release workflow when publishing, or `STRICT_PUBLISH_SURFACE=true` for local helper runs.
8. Optionally set workflow dispatch inputs `release_surface_mode` and `release_crate_file` for one-off releases.

## Code Standards

- **Formatting**: `rustfmt` is enforced.
- **Lints**: `clippy` warnings are errors in the supported lane.
- **Safety**: Unsafe code must be documented with `// SAFETY:` comments.
- **Testing**: New features must have corresponding tests in `tests/` or unit tests.

## Troubleshooting

### "Too many open files" during tests
The full workspace test suite opens many files. Increase your ulimit or run tests per-crate.

### "Memory limit exceeded"
The GLR table generation can be memory intensive for huge grammars. Try `cargo test --release` to use optimized table generation.

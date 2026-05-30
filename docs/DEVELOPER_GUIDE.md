# Developer Guide - Adze

> **Doc status:** Up to date for Adze 0.9.0.

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

### 🟢 Supported Lane (Local Supported Proof)
These crates are the core product. `just ci-supported` verifies their local
formatting, lint, and test proof.
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

The required hosted merge gate for ordinary `adze-swarm` PRs is
`Rust Small Result` from `.github/workflows/em-ci-routed-rust.yml`. The routed
implementation jobs (`Rust Small on CPX42`, `Rust Small on CX43`, `Rust Small
on CX33`, and `Rust Small on GitHub Hosted`) are not independent required
contexts. `Rust Small on CX53` is dormant while adze-swarm#598 is blocked.

To run the supported proof locally:
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

Release, tag, publish, signing, and Cargo-token work requires explicit human
authorization. Do not start it from swarm momentum.

Normal development and product proof happen in `EffortlessMetrics/adze-swarm`.
Actual release execution happens from public `EffortlessMetrics/adze` after the
candidate state has been explicitly promoted.

Use [`RELEASE_CANDIDATE_BUNDLE.md`](./reference/RELEASE_CANDIDATE_BUNDLE.md)
for the swarm-side pre-promotion checklist. A green bundle is evidence for a
maintainer decision; it is not release authorization by itself.

1. **Authorize**: record the human release/publish decision in the release
   tracker before touching tag, publish, signing, or Cargo-token paths.
2. **Promote**: move the selected `adze-swarm` release candidate into public
   `adze` with an explicit public promotion PR.
3. **Preflight**: run the supported, product, and publishable proof commands
   named in [`docs/reference/PUBLISH_CHECKLIST.md`](./reference/PUBLISH_CHECKLIST.md).
4. **Version**: update workspace versions only if the release plan requires it.
5. **Tag and publish**: tag and publish only from public `adze`, and only after
   authorization and preflight are complete.
6. **Install receipt**: after publishing, run the real crates.io install
   receipt before claiming `cargo install adze-cli` works.
7. **Update claims**: update support tiers, README, and release notes only for
   claims backed by proof receipts.

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

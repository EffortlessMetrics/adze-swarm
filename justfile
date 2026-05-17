#!/usr/bin/env just --justfile
# Adze development shortcuts
#
# If you see: error: I/O error in runtime dir ... Permission denied
# Run: source scripts/just-ensure-tmpdir.sh
# Or:  JUST_TEMPDIR=/tmp/just just <recipe>

set shell := ["bash", "-eu", "-o", "pipefail", "-c"]
# Git for Windows installs Bash here, but does not always put it on PATH for
# PowerShell-launched tools. Keep just recipes usable from a clean Windows shell.
set windows-shell := ["C:\\Program Files\\Git\\bin\\bash.exe", "-eu", "-o", "pipefail", "-c"]

# Format declared workspace members
fmt:
    ./scripts/fmt-workspace.sh --check

# Run clippy on core workspace members
clippy:
    cargo clippy -p adze -p adze-glr-core -p adze-ir -p adze-tablegen --lib -- -D warnings

# Run tests on core workspace members
test:
    cargo test -p adze -p adze-glr-core -p adze-ir -p adze-tablegen --lib

# Fast check for iteration (uses dev-fast profile)
check-fast:
    cargo check -p adze -p adze-ir -p adze-glr-core --profile dev-fast

# Run pre-commit checks
pre:
    .githooks/pre-commit

# Run pre-commit with test clippy enabled
pre-tests:
    CLIPPY_TESTS=1 .githooks/pre-commit

# Run pre-commit with strict docs enabled
pre-docs:
    STRICT_DOCS=1 .githooks/pre-commit

# Run pre-commit with warnings as errors
pre-warn:
    RUSTC_WARN_FATAL=1 .githooks/pre-commit

# Build everything
build:
    cargo build --all

# Build with release optimizations
release:
    cargo build --release --all

# Run test matrix script
matrix:
    ./scripts/test-matrix.sh

# Check ts-bridge linking
smoke:
    ./scripts/smoke-link.sh ts-bridge

# Run tests with perf counters enabled
bench-perf:
    cargo test --features perf-counters

# Update insta snapshots
snap:
    cargo insta review

# Required PR gate: this is the single supported CI lane for branch protection
# See docs/status/KNOWN_RED.md; update it whenever ci-supported command targets change.
ci-supported:
    ./scripts/ci-supported.sh

# Candidate product lane for README Stable claims. This is advisory until
# branch protection explicitly promotes it.
ci-product-stable:
    ./scripts/ci-product-stable.sh

# Broad non-blocking product canaries for Stable, Stabilizing, Advisory, and
# intentionally excluded surfaces. See docs/status/KNOWN_RED.md.
ci-product-advisory:
    ./scripts/ci-product.sh

# Run mutation testing on adze-ir (quick check)
mutate crate="adze-ir":
    cargo mutants -p {{crate}} --timeout-multiplier 2 -- --lib

# Run mutation testing on all supported crates
mutate-all:
    cargo mutants -- --lib

# Verify MSRV is consistent across all Cargo.toml files
check-msrv:
    #!/usr/bin/env bash
    set -euo pipefail
    MSRV=$(grep '^channel' rust-toolchain.toml | sed 's/.*"\(.*\)"/\1/')
    echo "MSRV from rust-toolchain.toml: $MSRV"
    errors=0
    while IFS= read -r line; do
      file="${line%%:*}"
      # skip target/ directory
      [[ "$file" == target/* ]] && continue
      value=$(grep '^rust-version' "$file" | head -1)
      if echo "$value" | grep -q 'workspace = true'; then
        echo "  ✓ $file (inherits workspace)"
      elif echo "$value" | grep -q "\"$MSRV\""; then
        echo "  ✓ $file (explicit $MSRV)"
      else
        echo "  ✗ $file — $value (expected $MSRV)"
        errors=$((errors + 1))
      fi
    done < <(grep -rl '^rust-version' --include='Cargo.toml' .)
    if [ "$errors" -gt 0 ]; then
      echo "FAIL: $errors Cargo.toml file(s) have mismatched rust-version"
      exit 1
    fi
    echo "OK: all rust-version fields match MSRV $MSRV"

# Show crates.io publish order
publish-order:
    ./scripts/publish-order.sh

# Verify publish metadata and cargo package file lists for the release surface
check-publishable:
    ./scripts/check-publish.sh

# Verify a crate package with local patches for unpublished co-release crates
package-local crate:
    ./scripts/package-local-release.sh {{crate}}

# Clean build artifacts
clean:
    cargo clean

# Clean disposable target subtrees without dropping the full workspace cache
clean-light:
    #!/usr/bin/env bash
    set -euo pipefail
    rm -rf \
      target/deterministic-codegen \
      target/precommit \
      target/ci \
      target/glr-parity-test \
      target/tmp \
      target/flycheck*

# Clean a single package's artifacts instead of the whole target tree
clean-package crate:
    cargo clean -p {{crate}}

worktree-list:
    ./scripts/cleanup-worktrees.sh list

worktree-prune-stale:
    ./scripts/cleanup-worktrees.sh prune-stale

# ----------------------------------------------------------------------------
# Policy stack — see docs/CLIPPY_POLICY.md, NO_PANIC_POLICY.md, FILE_POLICY.md.
# All checks run in advisory mode by default; flip with --mode blocking-allowlist
# (or blocking-strict) once the baseline is committed.
# ----------------------------------------------------------------------------

# Run every policy check and emit target/policy/policy-report.md
policy:
    cargo run -q -p xtask -- policy-report

# Check unreceipted panic-family debt (advisory)
policy-no-panic mode="advisory":
    cargo run -q -p xtask -- check-no-panic-family --mode {{mode}}

# Generate target/policy/no-panic-proposed.toml for review
policy-no-panic-propose:
    cargo run -q -p xtask -- no-panic-propose

# Check non-Rust files against policy/non-rust-allowlist.toml (advisory)
policy-files mode="advisory":
    cargo run -q -p xtask -- check-file-policy --mode {{mode}}

# Verify Cargo.toml / clippy.toml against policy/clippy-lints.toml (advisory)
policy-lints mode="advisory":
    cargo run -q -p xtask -- check-lint-policy --mode {{mode}}

# Lint workflows against policy/ci-lane-whitelist.toml (advisory)
policy-ci-lane-whitelist mode="advisory":
    cargo run -q -p xtask -- check-ci-lane-whitelist --mode {{mode}}

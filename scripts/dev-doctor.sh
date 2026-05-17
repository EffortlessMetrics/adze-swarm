#!/usr/bin/env bash
# Check that a local checkout has the common tools and settings needed for Adze development.
set -euo pipefail

ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
cd "$ROOT"

required_commands=(cargo rustc git)
recommended_commands=(just rg)
optional_commands=(jq node cc)
failures=0
warnings=0

print_help() {
  cat <<'HELP'
Usage: scripts/dev-doctor.sh [--help]

Checks local development prerequisites and repository settings for Adze.
The doctor is intentionally read-only: it reports missing tools and prints the
commands that fix common setup issues.
HELP
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  print_help
  exit 0
fi

if [[ $# -gt 0 ]]; then
  echo "error: unsupported argument: $1" >&2
  print_help >&2
  exit 2
fi

section() {
  printf '\n== %s ==\n' "$1"
}

ok() {
  printf '  ✓ %s\n' "$1"
}

warn() {
  printf '  ! %s\n' "$1"
  warnings=$((warnings + 1))
}

fail() {
  printf '  ✗ %s\n' "$1"
  failures=$((failures + 1))
}

version_line() {
  local cmd=$1
  case "$cmd" in
    just) just --version 2>/dev/null | head -n1 ;;
    rg) rg --version 2>/dev/null | head -n1 ;;
    jq) jq --version 2>/dev/null | head -n1 ;;
    node) node --version 2>/dev/null | head -n1 ;;
    cc) cc --version 2>/dev/null | head -n1 ;;
    *) "$cmd" --version 2>/dev/null | head -n1 ;;
  esac
}

check_command() {
  local cmd=$1
  local severity=$2
  if command -v "$cmd" >/dev/null 2>&1; then
    local version
    version=$(version_line "$cmd" || true)
    ok "$cmd available${version:+ ($version)}"
  elif [[ "$severity" == "required" ]]; then
    fail "$cmd missing; install it before running the PR gate"
  else
    warn "$cmd missing; install it for smoother local workflows"
  fi
}

section "Toolchain"
for cmd in "${required_commands[@]}"; do
  check_command "$cmd" required
done
for cmd in "${recommended_commands[@]}"; do
  check_command "$cmd" recommended
done

if [[ -f rust-toolchain.toml ]]; then
  expected_channel=$(sed -n 's/^channel *= *"\([^"]*\)".*/\1/p' rust-toolchain.toml | head -n1)
  if [[ -n "$expected_channel" ]]; then
    ok "rust-toolchain.toml pins Rust $expected_channel"
  else
    warn "rust-toolchain.toml exists, but the channel could not be read"
  fi
else
  fail "rust-toolchain.toml is missing from the repository root"
fi

section "Optional integrations"
for cmd in "${optional_commands[@]}"; do
  check_command "$cmd" optional
done

section "Repository setup"
if [[ -f justfile ]]; then
  ok "justfile found"
else
  fail "justfile missing; development recipes are unavailable"
fi

if [[ -x .githooks/pre-commit ]]; then
  ok ".githooks/pre-commit is executable"
else
  warn ".githooks/pre-commit is not executable; run: chmod +x .githooks/pre-commit"
fi

hooks_path=$(git config --get core.hooksPath || true)
if [[ "$hooks_path" == ".githooks" ]]; then
  ok "Git hooks path is set to .githooks"
else
  warn "Git hooks are not installed; run: git config core.hooksPath .githooks"
fi

section "Workspace health"
if cargo metadata --no-deps --format-version 1 >/dev/null; then
  ok "cargo metadata succeeds for the workspace"
else
  fail "cargo metadata failed; run it directly for the full error"
fi

if git diff --quiet -- Cargo.lock; then
  ok "workspace Cargo.lock has no unstaged changes"
else
  warn "workspace Cargo.lock has unstaged changes; review before committing"
fi

excluded_lock_changes=$(git status --short -- \
  'runtime/fuzz/Cargo.lock' \
  'tools/ts-bridge/Cargo.lock' \
  'crates/ts-c-harness/Cargo.lock' \
  'example/Cargo.lock' \
  'repro-issue-74/Cargo.lock' \
  'test-example/Cargo.lock' \
  'test-cli/tree-sitter-mylang/Cargo.lock' || true)
if [[ -z "$excluded_lock_changes" ]]; then
  ok "excluded/example Cargo.lock files are clean"
else
  warn "excluded/example Cargo.lock files have changes; avoid mixing generated lockfile churn into unrelated PRs"
  printf '%s\n' "$excluded_lock_changes" | sed 's/^/    /'
fi

section "Next steps"
if (( failures > 0 )); then
  printf 'Found %d blocking issue(s) and %d warning(s). Fix the blocking items, then rerun: just doctor\n' "$failures" "$warnings"
  exit 1
fi

if (( warnings > 0 )); then
  printf 'Found %d warning(s). You can still run the core gate, but setup improvements are recommended.\n' "$warnings"
else
  printf 'No issues found. Recommended validation: just ci-supported\n'
fi

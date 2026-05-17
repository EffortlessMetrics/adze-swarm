#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT"

errors=0
warnings=0

section() {
  printf '\n==> %s\n' "$1"
}

ok() {
  printf '  ✓ %s\n' "$1"
}

warn() {
  warnings=$((warnings + 1))
  printf '  ! %s\n' "$1"
}

fail() {
  errors=$((errors + 1))
  printf '  ✗ %s\n' "$1"
}

have_cmd() {
  command -v "$1" >/dev/null 2>&1
}

version_at_least() {
  local actual="$1"
  local required="$2"
  [ "$(printf '%s\n%s\n' "$required" "$actual" | sort -V | head -n1)" = "$required" ]
}

rust_channel() {
  awk -F '"' '/^[[:space:]]*channel[[:space:]]*=/{ print $2; exit }' rust-toolchain.toml
}

cargo_toml_member_count() {
  awk '
    /^[[:space:]]*members[[:space:]]*=/ { in_members = 1; next }
    in_members && /^[[:space:]]*\]/ { exit }
    in_members {
      line = $0
      sub(/#.*/, "", line)
      gsub(/[",]/, "", line)
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", line)
      if (line != "") count++
    }
    END { print count + 0 }
  ' Cargo.toml
}

section "Required tools"
for tool in git cargo rustc; do
  if have_cmd "$tool"; then
    ok "$tool: $($tool --version | head -n1)"
  else
    fail "$tool is missing from PATH"
  fi
done

if have_cmd just; then
  ok "just: $(just --version | head -n1)"
else
  warn "just is missing; install it to use repository shortcuts such as 'just ci-supported'"
fi

section "Rust toolchain"
if have_cmd rustc; then
  required_rust="$(rust_channel)"
  actual_rust="$(rustc --version | awk '{print $2}')"
  if [ -n "$required_rust" ] && version_at_least "$actual_rust" "$required_rust"; then
    ok "rustc $actual_rust satisfies rust-toolchain.toml ($required_rust)"
  else
    fail "rustc $actual_rust is older than rust-toolchain.toml requires ($required_rust)"
  fi
fi

section "Workspace metadata"
if have_cmd cargo; then
  member_count="$(cargo_toml_member_count)"
  if cargo metadata --no-deps --format-version 1 >/dev/null; then
    ok "cargo metadata resolved for $member_count declared workspace members"
  else
    fail "cargo metadata failed; run 'cargo metadata --no-deps --format-version 1' for details"
  fi
fi

section "Developer shortcuts"
if have_cmd just; then
  if just --list >/dev/null; then
    ok "justfile parsed successfully"
  else
    fail "justfile failed to parse"
  fi
fi

if [ -x .githooks/pre-commit ]; then
  ok "pre-commit hook is executable at .githooks/pre-commit"
else
  warn "pre-commit hook is not executable; run scripts/install-pre-commit.sh if you want local git hooks"
fi

section "Optional native dependencies"
if have_cmd pkg-config && pkg-config --exists tree-sitter; then
  ok "tree-sitter native library found via pkg-config"
elif [ -f /usr/include/tree_sitter/api.h ] || [ -f /usr/local/include/tree_sitter/api.h ]; then
  ok "tree-sitter native header found"
else
  warn "tree-sitter native library/header not found; only ts-bridge and C backend work may need it"
fi

section "Recommended environment defaults"
printf '  CARGO_BUILD_JOBS=%s\n' "${CARGO_BUILD_JOBS:-<unset; ci-supported defaults to 2>}"
printf '  RUST_TEST_THREADS=%s\n' "${RUST_TEST_THREADS:-<unset; ci-supported defaults to 2>}"
printf '  RAYON_NUM_THREADS=%s\n' "${RAYON_NUM_THREADS:-<unset>}"

section "Next commands"
printf '  Fast edit loop:  just check-fast\n'
printf '  Required PR gate: just ci-supported\n'

printf '\nSummary: %s error(s), %s warning(s)\n' "$errors" "$warnings"
if [ "$errors" -gt 0 ]; then
  exit 1
fi

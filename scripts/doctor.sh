#!/usr/bin/env bash
set -u -o pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT" || exit 1

status=0
warns=0

info() { printf '• %s\n' "$*"; }
ok() { printf '✓ %s\n' "$*"; }
warn() { printf '⚠ %s\n' "$*"; warns=$((warns + 1)); }
fail() { printf '✗ %s\n' "$*"; status=1; }

have() {
  command -v "$1" >/dev/null 2>&1
}

version_ge() {
  local actual="$1" required="$2"
  awk -v actual="$actual" -v required="$required" '
    BEGIN {
      split(actual, a, /[.-]/)
      split(required, r, /[.-]/)
      for (i = 1; i <= 3; i++) {
        av = (a[i] == "" ? 0 : a[i]) + 0
        rv = (r[i] == "" ? 0 : r[i]) + 0
        if (av > rv) exit 0
        if (av < rv) exit 1
      }
      exit 0
    }
  '
}

extract_rust_version() {
  local text="$1"
  case "$text" in
    rustc\ *|cargo\ *) printf '%s' "$text" | awk '{print $2}' ;;
    *) printf '%s' "$text" | awk '{print $1}' ;;
  esac
}

expected_rust="$(awk -F'"' '/^[[:space:]]*channel[[:space:]]*=/{print $2; exit}' rust-toolchain.toml 2>/dev/null)"
expected_rust="${expected_rust:-1.95.0}"

echo "Adze development environment doctor"
echo "Repository: $ROOT"
echo

info "Required tools"
if have rustc; then
  rustc_line="$(rustc --version 2>&1)"
  rustc_version="$(extract_rust_version "$rustc_line")"
  if version_ge "$rustc_version" "$expected_rust"; then
    ok "rustc $rustc_version (required >= $expected_rust)"
  else
    fail "rustc $rustc_version is older than required $expected_rust; rustup should honor rust-toolchain.toml"
  fi
else
  fail "rustc not found; install Rust/rustup before building Adze"
fi

if have cargo; then
  cargo_line="$(cargo --version 2>&1)"
  cargo_version="$(extract_rust_version "$cargo_line")"
  ok "cargo $cargo_version"
else
  fail "cargo not found; install Rust/rustup before building Adze"
fi

if have just; then
  ok "$(just --version 2>&1)"
else
  warn "just not found; install it to use repository shortcuts such as 'just ci-supported'"
fi

echo
info "Useful optional tools"
if have cargo-insta; then
  ok "cargo-insta available for snapshot review"
else
  warn "cargo-insta not found; install with 'cargo install cargo-insta' before reviewing snapshots"
fi

if have cargo-mutants; then
  ok "cargo-mutants available for mutation testing"
else
  warn "cargo-mutants not found; only needed for 'just mutate' / 'just mutate-all'"
fi

if have node; then
  ok "node $(node --version 2>&1) available for Tree-sitter CLI compatibility work"
else
  warn "node not found; only needed for legacy Tree-sitter CLI compatibility work"
fi

if have tree-sitter; then
  ok "$(tree-sitter --version 2>&1 | head -n1)"
else
  warn "tree-sitter CLI not found; only needed for legacy Tree-sitter CLI compatibility work"
fi

if have cc || have gcc || have clang; then
  ok "C compiler available for C compatibility crates"
else
  warn "no C compiler found on PATH; only needed for C compatibility crates"
fi

if have pkg-config && pkg-config --exists tree-sitter 2>/dev/null; then
  ok "libtree-sitter detected via pkg-config"
elif ldconfig -p 2>/dev/null | awk '{print $1}' | sort -u | awk '$0 == "libtree-sitter.so" { found = 1 } END { exit found ? 0 : 1 }'; then
  ok "libtree-sitter shared library detected"
else
  warn "libtree-sitter not detected; only needed for ts-bridge / C compatibility work"
fi

echo
info "Repository checks"
if [ -f Cargo.toml ]; then
  ok "workspace Cargo.toml present"
else
  fail "Cargo.toml not found at repository root"
fi

if [ -f justfile ]; then
  ok "justfile present"
else
  fail "justfile not found at repository root"
fi

if [ -x scripts/ci-supported.sh ]; then
  ok "scripts/ci-supported.sh is executable"
else
  fail "scripts/ci-supported.sh is missing or not executable"
fi

if have cargo; then
  if cargo locate-project --workspace >/dev/null 2>&1; then
    ok "cargo can locate the workspace"
  else
    fail "cargo cannot locate the workspace; run from a valid checkout"
  fi
fi

echo
info "Default concurrency caps"
printf '  RUST_TEST_THREADS=%s\n' "${RUST_TEST_THREADS:-2}"
printf '  RAYON_NUM_THREADS=%s\n' "${RAYON_NUM_THREADS:-4}"
printf '  CARGO_BUILD_JOBS=%s\n' "${CARGO_BUILD_JOBS:-4}"
printf '  TOKIO_WORKER_THREADS=%s\n' "${TOKIO_WORKER_THREADS:-2}"

echo
if [ "$status" -eq 0 ]; then
  if [ "$warns" -gt 0 ]; then
    echo "Doctor completed with $warns warning(s). Required checks passed."
  else
    echo "Doctor completed successfully."
  fi
else
  echo "Doctor found required setup problem(s)."
fi

exit "$status"

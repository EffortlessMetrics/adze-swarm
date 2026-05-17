#!/usr/bin/env bash
# Validate the local development environment and print the shortest path to a
# green supported-lane check.
set -u

status=0
warn_count=0

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$repo_root"

ok() { printf '✓ %s\n' "$*"; }
warn() { printf '⚠ %s\n' "$*"; warn_count=$((warn_count + 1)); }
fail() { printf '✗ %s\n' "$*"; status=1; }
info() { printf '• %s\n' "$*"; }

need_command() {
  local name="$1"
  local hint="$2"
  if command -v "$name" >/dev/null 2>&1; then
    ok "$name: $($name --version 2>/dev/null | head -n 1)"
  else
    fail "$name is missing. $hint"
  fi
}

optional_command() {
  local name="$1"
  local hint="$2"
  if command -v "$name" >/dev/null 2>&1; then
    ok "$name: $($name --version 2>/dev/null | head -n 1)"
  else
    warn "$name is not installed. $hint"
  fi
}

printf 'Adze development environment check\n'
printf '==================================\n\n'

info "Repository: $repo_root"

expected_rust="$(sed -n 's/^channel = "\(.*\)"/\1/p' rust-toolchain.toml | head -n 1)"
if [[ -n "$expected_rust" ]]; then
  info "Pinned Rust toolchain: $expected_rust"
fi

need_command rustc 'Install Rust with rustup; this repository selects its toolchain via rust-toolchain.toml.'
need_command cargo 'Install Rust with rustup; cargo is required for every build and test lane.'
need_command git 'Install git before contributing changes.'
optional_command just 'Install with `cargo install just` or your OS package manager to use the documented recipes.'
optional_command rg 'Install ripgrep for faster repository searches and policy scripts.'

if command -v rustc >/dev/null 2>&1 && [[ -n "$expected_rust" ]]; then
  actual_rust="$(rustc --version | awk '{print $2}')"
  if [[ "$actual_rust" == "$expected_rust" ]]; then
    ok "rustc matches rust-toolchain.toml ($expected_rust)"
  else
    warn "rustc reports $actual_rust; rust-toolchain.toml pins $expected_rust. Run through rustup-managed cargo/rustc if this is unexpected."
  fi
fi

if command -v rustfmt >/dev/null 2>&1; then
  ok "rustfmt: $(rustfmt --version 2>/dev/null | head -n 1)"
else
  fail 'rustfmt component is missing. Run `rustup component add rustfmt`.'
fi

if command -v cargo >/dev/null 2>&1 && cargo clippy --version >/dev/null 2>&1; then
  ok "clippy: $(cargo clippy --version 2>/dev/null | head -n 1)"
else
  fail 'clippy component is missing. Run `rustup component add clippy`.'
fi

if command -v pkg-config >/dev/null 2>&1 && pkg-config --exists tree-sitter 2>/dev/null; then
  ok 'tree-sitter development library is available for ts-bridge work'
else
  warn 'tree-sitter development library was not detected. This is only required for tools/ts-bridge.'
fi

printf '\nRecommended next commands\n'
printf '%s\n' '-------------------------'
if command -v just >/dev/null 2>&1; then
  printf '  just check-fast       # fastest compile feedback\n'
  printf '  just ci-supported     # required PR gate\n'
else
  printf '  cargo check -p adze -p adze-ir -p adze-glr-core --profile dev-fast\n'
  printf '  bash ./scripts/ci-supported.sh\n'
fi
printf '  cargo t2              # workspace tests capped at 2 threads\n'

printf '\nResult\n'
printf '%s\n' '------'
if [[ "$status" -eq 0 ]]; then
  if [[ "$warn_count" -gt 0 ]]; then
    printf 'Ready for supported-lane work with %d warning(s).\n' "$warn_count"
  else
    printf 'Ready for supported-lane work.\n'
  fi
else
  printf 'Missing required development dependency/dependencies.\n'
fi

exit "$status"

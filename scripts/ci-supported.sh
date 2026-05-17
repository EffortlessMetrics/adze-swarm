#!/usr/bin/env bash
set -euo pipefail

export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export RUST_TEST_THREADS="${RUST_TEST_THREADS:-2}"

supported_crate_paths=(
  runtime
  macro
  tool
  common
  ir
  glr-core
  tablegen
)

supported_crates=(
  -p adze
  -p adze-macro
  -p adze-tool
  -p adze-common
  -p adze-ir
  -p adze-glr-core
  -p adze-tablegen
)

./scripts/fmt-workspace.sh --check "${supported_crate_paths[@]}"

cargo clippy "${supported_crates[@]}" --all-targets -- -D warnings
cargo test "${supported_crates[@]}" --lib --tests --bins -- --test-threads="$RUST_TEST_THREADS"
cargo test -p adze-glr-core --features serialization --doc -- --test-threads="$RUST_TEST_THREADS"

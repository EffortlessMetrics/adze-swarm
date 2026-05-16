#!/usr/bin/env bash
set -euo pipefail

export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export RUST_TEST_THREADS="${RUST_TEST_THREADS:-2}"

supported_crate_names=(
  adze
  adze-macro
  adze-tool
  adze-common
  adze-ir
  adze-glr-core
  adze-tablegen
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

for crate in "${supported_crate_names[@]}"; do
  cargo fmt -p "$crate" -- --check
done

cargo clippy "${supported_crates[@]}" --all-targets -- -D warnings
cargo test "${supported_crates[@]}" --lib --tests --bins -- --test-threads="$RUST_TEST_THREADS"
cargo test -p adze-glr-core --features serialization --doc -- --test-threads="$RUST_TEST_THREADS"

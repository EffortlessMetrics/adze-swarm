#!/usr/bin/env bash
set -euo pipefail

export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export RUST_TEST_THREADS="${RUST_TEST_THREADS:-2}"
export CARGO_PROFILE_TEST_DEBUG="${CARGO_PROFILE_TEST_DEBUG:-0}"

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

test_args=(--test-threads="$RUST_TEST_THREADS" --quiet)

./scripts/fmt-workspace.sh --check "${supported_crate_paths[@]}"

cargo clippy "${supported_crates[@]}" --all-targets -- -D warnings

# Zero-test guard (#995 G1a): cargo test exits 0 even when every
# selected target reports "running 0 tests". These invocations span
# many targets (some legitimately test-free), so the guard requires at
# least one executed test plus an ok result instead of rejecting each
# individual zero-test target.
guard_test_output() {
  local output="$1"
  local label="$2"
  if ! grep -q "test result: ok" <<<"$output"; then
    printf '%s produced no "test result: ok"; refusing vacuous pass.\n' "$label" >&2
    return 1
  fi
  if ! grep -Eq "running [1-9][0-9]* tests?" <<<"$output"; then
    printf '%s executed 0 tests; refusing vacuous pass.\n' "$label" >&2
    return 1
  fi
}

set +e
supported_output="$(cargo test "${supported_crates[@]}" --lib --tests --bins -- "${test_args[@]}" 2>&1)"
supported_status=$?
set -e
printf '%s\n' "$supported_output"
if [[ $supported_status -ne 0 ]]; then
  exit "$supported_status"
fi
guard_test_output "$supported_output" "supported lib/tests/bins"

set +e
doctest_output="$(cargo test -p adze-glr-core --features serialization --doc -- "${test_args[@]}" 2>&1)"
doctest_status=$?
set -e
printf '%s\n' "$doctest_output"
if [[ $doctest_status -ne 0 ]]; then
  exit "$doctest_status"
fi
guard_test_output "$doctest_output" "supported doctests"

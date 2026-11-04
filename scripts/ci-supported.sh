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

# Keep executed-test accounting shared with the individual product canaries.
source "$(dirname "${BASH_SOURCE[0]}")/lib/required-test-output.sh"

# #1267: exercise the validator itself once in this existing integration job.
# Passing these regressions does not prove the ledger commands were executed.
python3 scripts/test_support_tier_qualification.py
cargo clippy --locked -p xtask --lib --test support_tier_proof_commands -- -D warnings
python3 scripts/run-required-test.py --label "support-tier validator regressions (static screen only)" --expected-tests 15 -- \
  cargo test --locked -p xtask --test support_tier_proof_commands -- --nocapture

set +e
supported_output="$(cargo test "${supported_crates[@]}" --lib --tests --bins -- "${test_args[@]}" 2>&1)"
supported_status=$?
set -e
printf '%s\n' "$supported_output"
if [[ $supported_status -ne 0 ]]; then
  exit "$supported_status"
fi
required_test_output "$supported_output" "supported lib/tests/bins"

set +e
doctest_output="$(cargo test -p adze-glr-core --features serialization --doc -- "${test_args[@]}" 2>&1)"
doctest_status=$?
set -e
printf '%s\n' "$doctest_output"
if [[ $doctest_status -ne 0 ]]; then
  exit "$doctest_status"
fi
required_test_output "$doctest_output" "supported doctests"

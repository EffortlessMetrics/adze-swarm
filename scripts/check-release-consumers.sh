#!/usr/bin/env bash
# check-release-consumers.sh — Verify release tooling reads the same ordered set
# as `cargo xtask check-release-graph`.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$ROOT_DIR"

mapfile -t graph_crates < <(cargo run -q -p xtask -- print-release-graph)
mapfile -t helper_crates < <("${SCRIPT_DIR}/release-graph-crates.sh")
mapfile -t surface_crates < <(RELEASE_SURFACE_MODE=fixed "${SCRIPT_DIR}/release-surface.sh")
mapfile -t txt_crates < <(awk 'NF && $1 !~ /^#/ {print $1}' "${SCRIPT_DIR}/release-crates.txt")

compare_sets() {
  local label="$1"
  shift
  local -n left="$1"
  local -n right="$2"

  if ((${#left[@]} != ${#right[@]})); then
    echo "FAIL: ${label} length mismatch (${#left[@]} vs ${#right[@]})" >&2
    return 1
  fi

  local idx
  for idx in "${!left[@]}"; do
    if [[ "${left[$idx]}" != "${right[$idx]}" ]]; then
      echo "FAIL: ${label} order mismatch at index ${idx}: ${left[$idx]} vs ${right[$idx]}" >&2
      return 1
    fi
  done
}

failures=0
compare_sets "release-graph-crates.sh vs print-release-graph" graph_crates helper_crates || failures=$((failures + 1))
compare_sets "release-surface.sh fixed vs print-release-graph" graph_crates surface_crates || failures=$((failures + 1))
compare_sets "release-crates.txt vs print-release-graph" graph_crates txt_crates || failures=$((failures + 1))

if (( failures > 0 )); then
  echo "Release consumer convergence check failed (${failures} mismatch(es))." >&2
  exit 1
fi

echo "Release consumer convergence check passed for ${#graph_crates[@]} crate(s)."

#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
RELEASE_GRAPH_ARTIFACT="${RELEASE_GRAPH_ARTIFACT:-${ROOT_DIR}/policy/release-graph.toml}"
RELEASE_CRATE_FILE="${RELEASE_CRATE_FILE:-${SCRIPT_DIR}/release-crates.txt}"
RELEASE_SURFACE_MODE="${RELEASE_SURFACE_MODE:-fixed}"
STRICT_PUBLISH_SURFACE="${STRICT_PUBLISH_SURFACE:-0}"
PACKAGE_BOUNDARY_RELEASE_GATE="${PACKAGE_BOUNDARY_RELEASE_GATE:-1}"

DRY_RUN=false
if [[ "${1:-}" == "--dry-run" ]]; then
  DRY_RUN=true
  shift
fi

release_version="${1:-$(cargo metadata --no-deps --format-version 1 2>/dev/null | jq -r '.packages[] | select(.name == "adze") | .version' | head -n 1)}"
if [[ -z "$release_version" ]]; then
  echo "Usage: $0 [--dry-run] <version>" >&2
  echo "Provide a version explicitly or run from workspace after bumping Cargo versions." >&2
  exit 1
fi

release_version="${release_version#v}"
if [[ -z "$release_version" ]]; then
  echo "Invalid release version." >&2
  exit 1
fi
tag="v${release_version}"
msg="Release ${tag}: Algorithmically correct GLR parser"

mapfile -t CRATES_TO_PUBLISH < <(RELEASE_SURFACE_MODE="$RELEASE_SURFACE_MODE" \
  RELEASE_CRATE_FILE="$RELEASE_CRATE_FILE" "${SCRIPT_DIR}/release-surface.sh")
if [[ ${#CRATES_TO_PUBLISH[@]} -eq 0 ]]; then
  echo "Error: release crate list is empty." >&2
  exit 1
fi

echo "=== Release helper ==="
echo "Tag: ${tag}"
echo "Dry run: ${DRY_RUN}"
echo "Release surface mode: ${RELEASE_SURFACE_MODE}"
echo "Release graph artifact: ${RELEASE_GRAPH_ARTIFACT}"
echo "Derived crate list: ${RELEASE_CRATE_FILE}"
echo "Strict publish surface: ${STRICT_PUBLISH_SURFACE}"
echo "Package-boundary release gate: ${PACKAGE_BOUNDARY_RELEASE_GATE}"
echo

# Run release surface validation before publishing
RELEASE_SURFACE_MODE="$RELEASE_SURFACE_MODE" \
RELEASE_CRATE_FILE="$RELEASE_CRATE_FILE" \
STRICT_PUBLISH_SURFACE="$STRICT_PUBLISH_SURFACE" \
PACKAGE_BOUNDARY_RELEASE_GATE="$PACKAGE_BOUNDARY_RELEASE_GATE" \
"${SCRIPT_DIR}/validate-release-surface.sh"
echo

# Check if tag already exists locally
if git rev-parse -q --verify "refs/tags/${tag}" >/dev/null; then
  echo "Tag ${tag} already exists locally. Aborting."
  exit 1
fi

# Check if tag already exists on origin
if git ls-remote --exit-code --tags origin "refs/tags/${tag}" >/dev/null 2>&1; then
  echo "Tag ${tag} already exists on origin. Aborting."
  exit 1
fi

# Allow SLEEP_SECS override
SLEEP_SECS="${SLEEP_SECS:-75}"

publish() {
  crate=$1
  if $DRY_RUN; then
    echo "DRY RUN: cargo publish -p ${crate}"
  else
    echo "Publishing ${crate} ..."
    cargo publish -p "${crate}"
    echo "Sleeping ${SLEEP_SECS}s to allow crates.io indexing..."
    sleep "${SLEEP_SECS}"
  fi
}

for crate in "${CRATES_TO_PUBLISH[@]}"; do
  publish "$crate"
done

if $DRY_RUN; then
  echo "DRY RUN: git tag -a ${tag} -m \"${msg}\""
  echo "DRY RUN: git push origin ${tag}"
else
  git tag -a "${tag}" -m "${msg}"
  git push origin "${tag}"
  echo "Tag created after successful publish: ${tag}"
fi

echo "Done. If not dry-run: double-check crates.io and run smoke tests."

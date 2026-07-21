#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
RELEASE_GRAPH_ARTIFACT="${RELEASE_GRAPH_ARTIFACT:-${ROOT_DIR}/policy/release-graph.toml}"
RELEASE_CRATE_FILE="${RELEASE_CRATE_FILE:-${SCRIPT_DIR}/release-crates.txt}"
RELEASE_SURFACE_MODE="${RELEASE_SURFACE_MODE:-fixed}"
RELEASE_CRATE_SYNC="${RELEASE_CRATE_SYNC:-0}"

usage() {
  cat <<'EOF'
Usage: release-surface.sh [--mode fixed|auto] [--sync]

Modes:
  fixed   Read crates from policy/release-graph.toml via xtask (default)
  auto    Recompute publishable crates from workspace metadata and topo-sort them

Options:
  --sync  Write the committed release graph order to RELEASE_CRATE_FILE
          (default: scripts/release-crates.txt).
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --mode)
      if [[ $# -lt 2 ]]; then
        echo "::error::--mode requires an argument (fixed|auto)." >&2
        exit 1
      fi
      RELEASE_SURFACE_MODE="$2"
      shift 2
      ;;
    --sync)
      RELEASE_CRATE_SYNC=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "::error::Unknown argument: $1" >&2
      usage
      exit 1
      ;;
  esac
done

case "${RELEASE_SURFACE_MODE,,}" in
  fixed)
    RELEASE_SURFACE_MODE="fixed"
    ;;
  auto)
    RELEASE_SURFACE_MODE="auto"
    ;;
  *)
    echo "::error::Invalid RELEASE_SURFACE_MODE '${RELEASE_SURFACE_MODE}'. Expected fixed|auto." >&2
    exit 1
    ;;
esac

emit_graph_crates() {
  if [[ ! -f "$RELEASE_GRAPH_ARTIFACT" ]]; then
    echo "::error::Missing release graph artifact: ${RELEASE_GRAPH_ARTIFACT}" >&2
    echo "::error::Run \`cargo xtask generate-release-graph\`." >&2
    exit 1
  fi

  mapfile -t crates < <("${SCRIPT_DIR}/release-graph-crates.sh")
  if [[ ${#crates[@]} -eq 0 ]]; then
    echo "::error::Release graph is empty: ${RELEASE_GRAPH_ARTIFACT}" >&2
    exit 1
  fi

  declare -A seen=()
  for crate in "${crates[@]}"; do
    if [[ -n "${seen[$crate]+x}" ]]; then
      echo "::error::Duplicate crate in release graph: ${crate}" >&2
      exit 1
    fi
    seen["$crate"]=1
    printf '%s\n' "$crate"
  done
}

emit_auto_crates() {
  local metadata
  local metadata_file
  metadata="$(cargo metadata --no-deps --format-version 1)"
  if ! jq -e '.packages | length > 0' <<<"$metadata" >/dev/null 2>&1; then
    echo "::error::Failed to read cargo metadata for auto surface calculation." >&2
    exit 1
  fi
  metadata_file="$(mktemp)"
  printf '%s' "$metadata" >"$metadata_file"
  python3 - "$metadata_file" <<'PY'
import json
import sys
from collections import defaultdict
from heapq import heapify, heappop, heappush

with open(sys.argv[1], "r", encoding="utf-8") as fh:
    metadata = json.load(fh)

publishable = []
workspace_crates = set()
for pkg in metadata["packages"]:
    workspace_crates.add(pkg["name"])
    publish = pkg.get("publish")
    if publish is None or publish is True or (isinstance(publish, list) and "crates.io" in publish):
        publishable.append(pkg["name"])

publishable = sorted(set(publishable))
if not publishable:
    print("::error::No publishable crates found in workspace metadata.", file=sys.stderr)
    sys.exit(1)

publishable_set = set(publishable)
dependents = defaultdict(set)
indegree = {crate: 0 for crate in publishable}

for pkg in metadata["packages"]:
    crate = pkg["name"]
    if crate not in publishable_set:
        continue

    for dep in pkg.get("dependencies", []):
        kind = dep.get("kind")
        kinds = [kind] if isinstance(kind, str) else (kind or ["normal"])
        if "dev" in kinds:
            continue
        if dep.get("optional", False):
            continue

        dep_name = dep["name"]
        if dep_name not in publishable_set or dep_name == crate:
            continue
        if crate not in dependents[dep_name]:
            dependents[dep_name].add(crate)
            indegree[crate] += 1

ready = [crate for crate in publishable if indegree[crate] == 0]
heapify(ready)
ordered = []

while ready:
    crate = heappop(ready)
    ordered.append(crate)
    for dependent in sorted(dependents.get(crate, ())):
        indegree[dependent] -= 1
        if indegree[dependent] == 0:
            heappush(ready, dependent)

if len(ordered) != len(publishable):
    unresolved = [crate for crate in publishable if indegree[crate] > 0]
    print("::error::Could not order publishable crates in a dependency-safe way.", file=sys.stderr)
    print(f"::error::Unordered crates: {' '.join(unresolved)}", file=sys.stderr)
    sys.exit(1)

sys.stdout.write("\n".join(ordered))
if ordered:
    sys.stdout.write("\n")
PY
  rm -f "$metadata_file"
}

if [[ "$RELEASE_SURFACE_MODE" == "fixed" ]]; then
  mapfile -t CRATES_TO_PUBLISH < <(emit_graph_crates)
else
  auto_surface_tmp="$(mktemp)"
  if ! emit_auto_crates >"$auto_surface_tmp"; then
    rm -f "$auto_surface_tmp"
    exit 1
  fi
  mapfile -t CRATES_TO_PUBLISH <"$auto_surface_tmp"
  rm -f "$auto_surface_tmp"
fi

if [[ "${RELEASE_CRATE_SYNC}" == "1" || "${RELEASE_CRATE_SYNC,,}" == "true" ]]; then
  {
    echo "# Generated from policy/release-graph.toml. Do not hand-edit."
    echo "# Regenerate: cargo xtask generate-release-graph"
    echo
    printf '%s\n' "${CRATES_TO_PUBLISH[@]}"
  } > "$RELEASE_CRATE_FILE"
fi

printf '%s\n' "${CRATES_TO_PUBLISH[@]}"

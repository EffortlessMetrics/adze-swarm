#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
STRICT_PUBLISH_SURFACE="${STRICT_PUBLISH_SURFACE:-0}"
RELEASE_GRAPH_ARTIFACT="${RELEASE_GRAPH_ARTIFACT:-${ROOT_DIR}/policy/release-graph.toml}"
RELEASE_CRATE_FILE="${RELEASE_CRATE_FILE:-${SCRIPT_DIR}/release-crates.txt}"
RELEASE_SURFACE_MODE="${RELEASE_SURFACE_MODE:-fixed}"
PACKAGE_BOUNDARY_RELEASE_GATE="${PACKAGE_BOUNDARY_RELEASE_GATE:-0}"

boundary_gate="${PACKAGE_BOUNDARY_RELEASE_GATE,,}"
if [[ "$boundary_gate" == "1" || "$boundary_gate" == "true" || "$boundary_gate" == "yes" || "$boundary_gate" == "on" ]]; then
  python - "${SCRIPT_DIR}/.." <<'PY'
from pathlib import Path
import sys
import tomllib

root = Path(sys.argv[1]).resolve()
policy_path = root / "policy" / "package-boundary.toml"
data = tomllib.loads(policy_path.read_text(encoding="utf-8"))
targets = [
    package.get("name", "<unknown>")
    for package in data.get("package", [])
    if package.get("category") == "owner-module-migration-target"
]
if targets:
    shown = ", ".join(targets[:12])
    if len(targets) > 12:
        shown = f"{shown}, and {len(targets) - 12} more"
    print(
        f"::error::Release blocked: {len(targets)} owner-module migration target(s) remain in policy/package-boundary.toml: {shown}",
        file=sys.stderr,
    )
    print(
        "::error::Move them into SRP owner submodules, remove them, or reclassify them with an accepted ADR before release.",
        file=sys.stderr,
    )
    sys.exit(1)

print("Package-boundary release gate passed: no owner-module migration targets remain.")
PY
fi

mapfile -t ALLOWED_CRATES < <(RELEASE_SURFACE_MODE="$RELEASE_SURFACE_MODE" \
  RELEASE_CRATE_FILE="$RELEASE_CRATE_FILE" "${SCRIPT_DIR}/release-surface.sh")
if [[ ${#ALLOWED_CRATES[@]} -eq 0 ]]; then
  echo "::error::Release surface is empty (mode: ${RELEASE_SURFACE_MODE})." >&2
  exit 1
fi
METADATA_JSON="$(cargo metadata --no-deps --format-version 1)"
if ! jq -e '.packages | length > 0' <<<"$METADATA_JSON" >/dev/null 2>&1; then
  echo "::error::Unable to load cargo metadata JSON for release-surface validation." >&2
  exit 1
fi

declare -A ALLOWLIST=()
declare -A ALLOWLIST_INDEX=()
has_failure=0
STRICT_PUBLISH_SURFACE="${STRICT_PUBLISH_SURFACE,,}"

for idx in "${!ALLOWED_CRATES[@]}"; do
  crate="${ALLOWED_CRATES[$idx]}"
  if [[ -n "${ALLOWLIST[$crate]+x}" ]]; then
    echo "::error::Duplicate crate in allowlist: ${crate}" >&2
    has_failure=1
    continue
  fi
  ALLOWLIST["$crate"]=1
  ALLOWLIST_INDEX["$crate"]="$idx"
done
declare -A SEEN_ALLOWED=()

extra_publishable=()

while IFS=$'\t' read -r crate manifest_path publishable; do
  if [[ "$publishable" == "false" ]]; then
    if [[ "$RELEASE_SURFACE_MODE" == "fixed" && -n "${ALLOWLIST[$crate]+x}" ]]; then
      echo "::error::Allowlisted crate '$crate' is not publishable (publish = false)." >&2
      has_failure=1
    fi
    continue
  fi

  if [[ -z "${ALLOWLIST[$crate]+x}" ]]; then
    if [[ "$RELEASE_SURFACE_MODE" == "fixed" && ("$STRICT_PUBLISH_SURFACE" == "1" || "$STRICT_PUBLISH_SURFACE" == "true" || "$STRICT_PUBLISH_SURFACE" == "yes" || "$STRICT_PUBLISH_SURFACE" == "on") ]]; then
      echo "::error::Unexpected publishable crate: $crate ($manifest_path)" >&2
      has_failure=1
      continue
    elif [[ "$RELEASE_SURFACE_MODE" == "fixed" ]]; then
      extra_publishable+=("$crate")
    else
      continue
    fi
  fi

  SEEN_ALLOWED["$crate"]=1
done < <(jq -r '
  .packages[] |
  .publish as $publish |
  (if $publish == null then true
   elif $publish == true then true
   elif (($publish | type) == "array" and (($publish | index("crates.io")) != null)) then true
   else false end) as $is_publishable |
  "\(.name)\t\(.manifest_path)\t\($is_publishable)"' <<<"$METADATA_JSON")

# Validate allowlist order against workspace dependency graph for normal/build dependencies.
if [[ "$RELEASE_SURFACE_MODE" == "fixed" ]]; then
  while IFS=$'\t' read -r crate dep; do
    [[ -z "${ALLOWLIST[$crate]+x}" ]] && continue
    [[ -z "${ALLOWLIST[$dep]+x}" ]] && continue
    [[ "$crate" == "$dep" ]] && continue

    crate_idx="${ALLOWLIST_INDEX[$crate]}"
    dep_idx="${ALLOWLIST_INDEX[$dep]}"
    if (( dep_idx >= crate_idx )); then
      echo "::error::Allowlist order violation: '$dep' must be published before '$crate'" >&2
      has_failure=1
    fi
  done < <(jq -r '.packages[] as $pkg | $pkg.name as $crate | $pkg.dependencies[]? | select((.kind // ["normal"]) | index("dev") | not) | select((.optional // false) | not) | "\($crate)\t\(.name)"' <<<"$METADATA_JSON")
fi

for crate in "${ALLOWED_CRATES[@]}"; do
  if [[ -z "${SEEN_ALLOWED[$crate]+x}" ]]; then
    echo "::error::Allowlisted crate '$crate' is not marked as publishable." >&2
    has_failure=1
  fi
done

if [[ "$RELEASE_SURFACE_MODE" == "fixed" && $has_failure == 0 ]] && (( ${#extra_publishable[@]} > 0 )); then
  extra_count="${#extra_publishable[@]}"
  if (( extra_count <= 12 )); then
    echo "::warning::Extra publishable crates are not in ${RELEASE_GRAPH_ARTIFACT}: ${extra_publishable[*]}" >&2
  else
    shown=("${extra_publishable[@]:0:12}")
    remaining=$(( extra_count - 12 ))
    echo "::warning::Extra publishable crates are not in ${RELEASE_GRAPH_ARTIFACT}: ${shown[*]}" >&2
    echo "::warning::... and ${remaining} more publishable crates" >&2
  fi
fi

if (( has_failure != 0 )); then
  echo "::error::Publish-surface validation failed." >&2
  exit 1
fi

echo "Publish-surface validation passed for mode=${RELEASE_SURFACE_MODE}:" \
  "${ALLOWED_CRATES[*]}"

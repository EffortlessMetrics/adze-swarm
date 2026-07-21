#!/usr/bin/env bash
# check-publish.sh — Verify that core release crates pass `cargo package --list`
# and have complete metadata for crates.io publishing.
#
# CANONICAL publishability check. This is the script wired into
# `just check-publishable` and referenced by docs/reference/PUBLISH_CHECKLIST.md.
# Crate order and membership come from policy/release-graph.toml via
# scripts/release-graph-crates.sh (see `cargo xtask check-release-graph`).
#
# Usage:
#   ./scripts/check-publish.sh          # Check all release-graph crates
#   ./scripts/check-publish.sh adze-ir  # Check a single crate
#
# Exit codes:
#   0  all checks pass
#   1  at least one check failed

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

mapfile -t CORE_CRATES < <("${SCRIPT_DIR}/release-graph-crates.sh")
if [[ ${#CORE_CRATES[@]} -eq 0 ]]; then
  echo "No release-graph crates found. Run \`cargo xtask generate-release-graph\`." >&2
  exit 1
fi

METADATA_JSON="$(cargo metadata --no-deps --format-version 1)"
declare -A CRATE_DIR=()
while IFS=$'\t' read -r crate manifest_path; do
  [[ -z "$crate" ]] && continue
  CRATE_DIR["$crate"]="$(dirname "$manifest_path")"
done < <(jq -r '.packages[] | "\(.name)\t\(.manifest_path)"' <<<"$METADATA_JSON")

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'

ERRORS=0

check_crate() {
  local crate="$1"
  local dir="${CRATE_DIR[$crate]:-}"
  local manifest

  echo ""
  echo "━━━ Checking $crate ━━━"

  if [[ -z "$dir" ]]; then
    echo -e "  ${RED}FAIL${NC} crate not found in workspace metadata"
    ((ERRORS++))
    return
  fi

  manifest="${dir}/Cargo.toml"
  echo "  path: ${dir#${ROOT_DIR}/}/"

  # 1. Cargo.toml exists
  if [[ ! -f "$manifest" ]]; then
    echo -e "  ${RED}FAIL${NC} Cargo.toml missing at $manifest"
    ((ERRORS++))
    return
  fi

  # 2. Required metadata fields
  local required_fields=(
    "^name"
    "^version"
    "^edition"
    "^description"
    "^license"
    "^repository"
    "^readme"
  )
  for field in "${required_fields[@]}"; do
    if ! grep -qP "$field" "$manifest"; then
      echo -e "  ${RED}FAIL${NC} missing field matching $field"
      ((ERRORS++))
    fi
  done

  # 3. publish = true (not inheriting workspace publish = false)
  if grep -qP '^publish\s*=\s*false' "$manifest"; then
    echo -e "  ${RED}FAIL${NC} publish = false"
    ((ERRORS++))
  elif ! grep -qP '^publish\s*=' "$manifest"; then
    echo -e "  ${YELLOW}WARN${NC} no explicit publish = true (workspace default is publish = false)"
    ((ERRORS++))
  fi

  # 4. README exists
  if [[ ! -f "$dir/README.md" ]]; then
    echo -e "  ${RED}FAIL${NC} README.md missing in $dir/"
    ((ERRORS++))
  fi

  # 5. LICENSE files exist
  if [[ ! -f "$dir/LICENSE-MIT" ]] && [[ ! -f "$dir/LICENSE" ]]; then
    echo -e "  ${RED}FAIL${NC} no LICENSE-MIT or LICENSE file in $dir/"
    ((ERRORS++))
  fi

  # 6. cargo package --list succeeds (metadata-only check, no registry)
  if cargo package --list --allow-dirty -p "$crate" >/dev/null 2>&1; then
    echo -e "  ${GREEN}OK${NC} cargo package --list"
  else
    echo -e "  ${RED}FAIL${NC} cargo package --list"
    cargo package --list --allow-dirty -p "$crate" 2>&1 | tail -5
    ((ERRORS++))
  fi

  echo -e "  ${GREEN}OK${NC} metadata check"
}

# If a crate name was passed, check only that one
if [[ $# -ge 1 ]]; then
  check_crate "$1"
else
  echo "=== Adze publish readiness check ==="
  echo "Authority: policy/release-graph.toml"
  echo ""
  echo "Publish order:"
  for i in "${!CORE_CRATES[@]}"; do
    echo "  $((i+1)). ${CORE_CRATES[$i]}"
  done

  for crate in "${CORE_CRATES[@]}"; do
    check_crate "$crate"
  done
fi

echo ""
if [[ $ERRORS -eq 0 ]]; then
  echo -e "${GREEN}All checks passed.${NC}"
  exit 0
else
  echo -e "${RED}$ERRORS check(s) failed.${NC}"
  exit 1
fi

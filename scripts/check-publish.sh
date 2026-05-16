#!/usr/bin/env bash
# check-publish.sh — Verify that core release crates pass `cargo package --list`
# and have complete metadata for crates.io publishing.
#
# Usage:
#   ./scripts/check-publish.sh          # Check all core crates
#   ./scripts/check-publish.sh adze-ir  # Check a single crate
#
# Exit codes:
#   0  all checks pass
#   1  at least one check failed

set -euo pipefail

# Publish order (dependency-first)
CORE_CRATES=(
  adze-common
  adze-ir
  adze-glr-core
  adze-tablegen
  adze-macro
  adze-tool
  adze-cli
  adze
)

# Map crate name -> directory
declare -A CRATE_DIR=(
  [adze-common]=common
  [adze-ir]=ir
  [adze-glr-core]=glr-core
  [adze-tablegen]=tablegen
  [adze-macro]=macro
  [adze-tool]=tool
  [adze-cli]=cli
  [adze]=runtime
)

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'

ERRORS=0

check_crate() {
  local crate="$1"
  local dir="${CRATE_DIR[$crate]}"
  local manifest="$dir/Cargo.toml"

  echo ""
  echo "━━━ Checking $crate ($dir/) ━━━"

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

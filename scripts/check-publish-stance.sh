#!/usr/bin/env bash
# check-publish-stance.sh — Verify every workspace package declares an explicit
# Cargo `publish` key in its own manifest (no omission/inheritance-only stance).
#
# This is the PR1 (#855) metadata-truth proof. It does not compare ledger
# category with publish values; that ledger/Cargo alignment check lands in PR2.
#
# Usage:
#   ./scripts/check-publish-stance.sh
#
# Exit codes:
#   0  every workspace member has an explicit publish key
#   1  at least one member omits publish

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

mapfile -t MANIFESTS < <(
  cargo metadata --format-version 1 --no-deps \
    | python -c "
import json, sys
data = json.load(sys.stdin)
members = set(data.get('workspace_members', []))
for pkg in data.get('packages', []):
    if pkg['id'] in members:
        print(pkg['manifest_path'])
"
)

if [[ ${#MANIFESTS[@]} -eq 0 ]]; then
  echo "check-publish-stance: no workspace members found" >&2
  exit 1
fi

ERRORS=0
for manifest in "${MANIFESTS[@]}"; do
  rel="${manifest#"$ROOT"/}"
  if ! grep -qE '^[[:space:]]*publish[[:space:]]*=' "$manifest"; then
    echo -e "${RED}FAIL${NC} $rel: missing explicit publish key"
    ((ERRORS++)) || true
  else
    echo -e "${GREEN}OK${NC} $rel"
  fi
done

echo ""
echo "checked ${#MANIFESTS[@]} workspace package(s)"

if [[ $ERRORS -gt 0 ]]; then
  echo -e "${RED}$ERRORS package(s) missing explicit publish metadata${NC}" >&2
  exit 1
fi

echo -e "${GREEN}All workspace packages declare an explicit publish stance.${NC}"

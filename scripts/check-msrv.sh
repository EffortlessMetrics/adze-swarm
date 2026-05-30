#!/usr/bin/env bash
set -euo pipefail

MSRV=$(grep '^channel' rust-toolchain.toml | sed 's/.*"\(.*\)"/\1/')
echo "MSRV from rust-toolchain.toml: $MSRV"

errors=0
while IFS= read -r file; do
  value=$(grep '^rust-version' "$file" | head -1 || true)
  [ -z "$value" ] && continue

  if echo "$value" | grep -q 'workspace = true'; then
    echo "  OK $file (inherits workspace)"
  elif echo "$value" | grep -q "\"$MSRV\""; then
    echo "  OK $file (explicit $MSRV)"
  else
    echo "  FAIL $file: $value (expected $MSRV)"
    errors=$((errors + 1))
  fi
done < <(find . -path './target' -prune -o -name Cargo.toml -type f -print | sort)

if [ "$errors" -gt 0 ]; then
  echo "FAIL: $errors Cargo.toml file(s) have mismatched rust-version"
  exit 1
fi

echo "OK: all rust-version fields match MSRV $MSRV"

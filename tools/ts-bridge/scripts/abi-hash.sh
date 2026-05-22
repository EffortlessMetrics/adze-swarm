#!/usr/bin/env bash
set -euo pipefail

# Check that vendored headers match pinned hashes
sha_ref_api=$(cat tools/ts-bridge/ci/pinned.api.h.sha)
sha_ref_parser=$(cat tools/ts-bridge/ci/pinned.parser.h.sha)
sha_api=$(sha256sum tools/ts-bridge/ci/vendor/tree_sitter/api.h | awk '{print $1}')
sha_parser=$(sha256sum tools/ts-bridge/ci/vendor/tree_sitter/parser.h | awk '{print $1}')

if [[ "$sha_api" != "$sha_ref_api" || "$sha_parser" != "$sha_ref_parser" ]]; then
  echo "Header hash mismatch (ABI drift)"
  echo "  api.h:    expected $sha_ref_api, got $sha_api"
  echo "  parser.h: expected $sha_ref_parser, got $sha_parser"
  exit 1
fi

echo "Header hashes match ✓"

# Runtime ABI (v15) check using shim. ts-bridge is intentionally outside the
# main workspace member set, so use its manifest path explicitly.
cargo run -q --manifest-path tools/ts-bridge/Cargo.toml --bin tsb-abi-check

echo "ABI verification complete ✓"

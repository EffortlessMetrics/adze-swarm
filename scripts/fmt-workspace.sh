#!/usr/bin/env bash
set -euo pipefail

# `cargo fmt --all` can exceed Windows command-line limits before rustfmt runs.
# Keep the declared workspace-member proof portable by formatting each member
# with a short manifest-path invocation.
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

args=("$@")
if [ "${#args[@]}" -eq 0 ]; then
  args=(--check)
fi

mapfile -t members < <(
  awk '
    /^[[:space:]]*members[[:space:]]*=/ { in_members = 1; next }
    in_members && /^[[:space:]]*\]/ { exit }
    in_members {
      line = $0
      sub(/#.*/, "", line)
      gsub(/[",]/, "", line)
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", line)
      if (line != "") print line
    }
  ' Cargo.toml
)

if [ "${#members[@]}" -eq 0 ]; then
  echo "No workspace members found in Cargo.toml" >&2
  exit 1
fi

for member in "${members[@]}"; do
  manifest="$member/Cargo.toml"
  if [ ! -f "$manifest" ]; then
    echo "Workspace member manifest missing: $manifest" >&2
    exit 1
  fi
  echo "fmt: $member"
  cargo fmt --manifest-path "$manifest" "${args[@]}"
done

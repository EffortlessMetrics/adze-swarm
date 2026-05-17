#!/usr/bin/env bash
set -euo pipefail

# `cargo fmt --all` can exceed Windows command-line limits before rustfmt runs.
# Keep the declared workspace-member proof portable by formatting each member
# with a short manifest-path invocation.
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

rustfmt_args=()
members=()

for arg in "$@"; do
  case "$arg" in
    -*)
      rustfmt_args+=("$arg")
      ;;
    *)
      members+=("$arg")
      ;;
  esac
done

if [ "${#rustfmt_args[@]}" -eq 0 ]; then
  rustfmt_args=(--check)
fi

if [ "${#members[@]}" -eq 0 ]; then
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
fi

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

  rust_files=()
  if [ -f "$member/build.rs" ]; then
    rust_files+=("$member/build.rs")
  fi

  for source_root in src tests examples benches; do
    source_dir="$member/$source_root"
    if [ ! -d "$source_dir" ]; then
      continue
    fi

    while IFS= read -r -d '' file; do
      rust_files+=("$file")
    done < <(find "$source_dir" -name '*.rs' -type f -print0)
  done

  if [ "${#rust_files[@]}" -eq 0 ]; then
    continue
  fi

  chunk=()
  for file in "${rust_files[@]}"; do
    chunk+=("$file")
    if [ "${#chunk[@]}" -ge 40 ]; then
      rustfmt "${rustfmt_args[@]}" "${chunk[@]}"
      chunk=()
    fi
  done

  if [ "${#chunk[@]}" -gt 0 ]; then
    rustfmt "${rustfmt_args[@]}" "${chunk[@]}"
  fi
done

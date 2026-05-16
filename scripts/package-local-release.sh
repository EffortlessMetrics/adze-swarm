#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/package-local-release.sh <crate> [cargo-package-args...]

Runs `cargo package -p <crate> --allow-dirty` with local crates.io patches for
Adze co-release crates that may not exist on crates.io yet. The target crate is
excluded from the patch set to avoid package self-collisions during verification.
EOF
}

if [[ $# -lt 1 ]]; then
  usage >&2
  exit 2
fi

crate="$1"
shift

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

patches=(
  "adze=runtime"
  "adze-cli=cli"
  "adze-common=common"
  "adze-glr-core=glr-core"
  "adze-ir=ir"
  "adze-linecol-core=crates/linecol-core"
  "adze-macro=macro"
  "adze-parsetable-metadata=crates/parsetable-metadata"
  "adze-tablegen=tablegen"
  "adze-tool=tool"
  "adze-bdd-governance-core=crates/bdd-governance-core"
)

config_args=()
for patch in "${patches[@]}"; do
  patch_crate="${patch%%=*}"
  patch_path="${patch#*=}"
  if [[ "$patch_crate" == "$crate" ]]; then
    continue
  fi
  config_args+=(--config "patch.crates-io.${patch_crate}.path=\"${patch_path}\"")
done

cargo "${config_args[@]}" package -p "$crate" --allow-dirty "$@"

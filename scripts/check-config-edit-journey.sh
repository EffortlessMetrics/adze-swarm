#!/usr/bin/env bash
# One committed-lock consumer proof on the admitted Linux merge group. The
# fixture is an independent workspace and is not in the root Cargo.lock.
set -euo pipefail
[[ ${BASE_SHA:-} =~ ^[0-9a-f]{40}$ && ${EXPECTED_HEAD:-} =~ ^[0-9a-f]{40}$ ]] ||
  { echo 'Missing or invalid merge-group base/head.' >&2; exit 1; }
[[ $(git rev-parse HEAD) == "$EXPECTED_HEAD" ]] ||
  { echo 'Refusing to qualify a different checkout.' >&2; exit 1; }
# The checkout normally already includes the base. Fetch just the event base
# if needed, without guessing HEAD^ for multi-PR groups.
if ! git cat-file -e "${BASE_SHA}^{commit}" 2>/dev/null; then
  git fetch --no-tags --depth=1 origin "$BASE_SHA"
fi
sample=samples/config-dsl
if git diff --quiet --no-renames "$BASE_SHA" "$EXPECTED_HEAD" -- \
  "$sample"/{Cargo.toml,Cargo.lock,build.rs,src,tests} \
  scripts/check-config-edit-journey.sh scripts/run-required-test.py \
  .github/workflows/em-ci-routed-rust.yml; then
  echo 'Unrelated merge-group paths; config-dsl edit journey not selected.'
  exit 0
else
  status=$?
  [[ $status == 1 ]] || exit "$status"
fi

manifest=$sample/Cargo.toml
# Require the tracked lock and preserve its exact bytes; no regenerated lock.
git ls-files --error-unmatch -- "$sample/Cargo.lock" >/dev/null
git diff --exit-code HEAD -- "$sample/Cargo.lock"
before=$(sha256sum "$sample/Cargo.lock")
printf 'Config edit base=%s head=%s; lock: %s\n' "$BASE_SHA" "$EXPECTED_HEAD" "$before"
log=$(mktemp)
trap 'rm -f -- "$log"' EXIT
cargo fetch --locked --target host-tuple --manifest-path "$manifest"
printf '%s\n' "$before" | sha256sum --check --status
# The outer workspace and the four nested phase commands now run offline.
CARGO_NET_OFFLINE=true python3 scripts/run-required-test.py --label "config-dsl grammar edit/break/repair" -- \
  cargo test --locked --offline --manifest-path "$manifest" --test edit_debug -- \
  --ignored --exact grammar_edit_break_repair --format pretty --test-threads=1 --color never | tee "$log"
# The common guard already checks complete, well-formed libtest summaries and
# forwards child output/status. This filtered binary may omit sibling tests,
# but only one result row and one 1/0/0/0 summary can qualify this journey.
awk '
  /^test result:/ {
    summaries++
    if ($0 ~ /^test result: ok[.] 1 passed; 0 failed; 0 ignored; 0 measured; /) exact_summary++
    next
  }
  /^test / {
    rows++
    if ($0 == "test grammar_edit_break_repair ... ok") exact_row++
  }
  END { exit !(summaries == 1 && exact_summary == 1 && rows == 1 && exact_row == 1) }
' "$log" || { echo 'Expected exactly one grammar_edit_break_repair pass and one 1/0/0/0 summary.' >&2; exit 1; }
printf '%s\n' "$before" | sha256sum --check --status

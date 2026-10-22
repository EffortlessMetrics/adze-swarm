#!/usr/bin/env bash
# Offline contract tests. The fake gh refuses every non-GET request.
set -euo pipefail
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
tmp=$(mktemp -d)
trap 'rm -rf -- "$tmp"' EXIT
mkdir "$tmp/bin"
cat > "$tmp/bin/gh" <<'FAKE'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> "$CALLS"
[[ $1 == api && $2 == --hostname && $3 == github.com && $4 == --method && $5 == GET ]] || exit 94
endpoint=$6
if [[ $endpoint == *per_page* ]]; then
  [[ ${7:-} == --paginate && ${8:-} == --slurp ]] || exit 95
fi
sha=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
other=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
case "$endpoint" in
  */branches/main)
    if [[ $CASE == moved-main ]]; then
      if [[ -f $STATE ]]; then sha=$other; else : > "$STATE"; fi
    fi
    printf '{"commit":{"sha":"%s"}}\n' "$sha";;
  */pulls\?*) printf '[[{"number":1032,"head":{"sha":"%s"},"base":{"sha":"%s"},"draft":false}]]\n' "$sha" "$sha";;
  */pulls/1032|*/pulls/1054|*/pulls/991)
    n=${endpoint##*/}; status=open; merged=false
    if [[ $n == 1054 ]]; then status=closed; merged=true; fi
    if [[ $n == 991 && $CASE == closed-unmerged ]]; then status=closed; fi
    if [[ $CASE == moved-head && $n == 1032 ]]; then
      if [[ -f $STATE ]]; then sha=$other; else : > "$STATE"; fi
    fi
    if [[ $CASE == wrong-pr && $n == 1032 ]]; then n=7; fi
    printf '{"number":%s,"state":"%s","draft":false,"merged":%s,"merge_commit_sha":null,"head":{"sha":"%s","repo":{"full_name":"EffortlessMetrics/adze-swarm"}},"base":{"sha":"%s","repo":{"full_name":"EffortlessMetrics/adze-swarm"}}}\n' "$n" "$status" "$merged" "$sha" "$other";;
  */check-runs\?*)
    [[ $endpoint == *filter=all* ]] || exit 96
    case "$CASE" in
      api-error) echo 'simulated unavailable'; exit 1;;
      malformed) echo '{';;
      missing-page) echo '[{"total_count":2,"check_runs":[{"id":1}]}]';;
      duplicate-check) echo '[{"total_count":2,"check_runs":[{"id":1},{"id":1}]}]';;
      *) printf '[{"total_count":2,"check_runs":[{"id":1,"name":"Rust Small Result","head_sha":"%s","status":"completed","conclusion":"success"}]},{"total_count":2,"check_runs":[{"id":2,"name":"Rust Small Result","head_sha":"%s","status":"in_progress","conclusion":null}]}]\n' "$sha" "$sha";;
    esac;;
  */statuses\?*|*/reviews\?*|*/comments\?*) echo '[[]]';;
  *) printf 'Unexpected request: %s\n' "$endpoint" >&2; exit 97;;
esac
FAKE
chmod +x "$tmp/bin/gh"
export PATH="$tmp/bin:$PATH"
tests=0
run_case() {
  export CASE=$1 CALLS="$tmp/$1.calls" STATE="$tmp/$1.state"
  local expected=$2 actual=0
  bash "$script_dir/capture-codex-handoff.sh" "$tmp/$1.output" 1032 1054 991 > "$tmp/$1.stdout" 2> "$tmp/$1.stderr" || actual=$?
  [[ $actual == "$expected" ]] || { cat "$tmp/$1.stderr" >&2; printf 'FAIL %s exit %s expected %s\n' "$1" "$actual" "$expected" >&2; exit 1; }
  jq -e --argjson expected "$expected" '.qualification == false and .admission == false and .collection_complete == ($expected == 0)' "$tmp/$1.output/summary.json" >/dev/null
  tests=$((tests + 1))
}
run_case complete 0
jq -e '[.[] .check_runs[]] | length == 2 and .[1].status == "in_progress" and .[1].conclusion == null' "$tmp/complete.output/pr-1032-H-checks.json" >/dev/null
[[ ! -f "$tmp/complete.output/INCOMPLETE" ]]
[[ ! -e "$tmp/complete.output/pr-1054-H-checks.json" ]]
run_case api-error 1
[[ -f "$tmp/api-error.output/INCOMPLETE" ]]
run_case malformed 1
run_case missing-page 1
run_case duplicate-check 1
run_case moved-main 1
run_case moved-head 1
run_case wrong-pr 1
run_case closed-unmerged 0
jq -e '.state == "closed" and .merged == false' "$tmp/closed-unmerged.output/pr-991-before.json" >/dev/null
# Refuse overwrite, symlink, duplicate IDs and argument-shaped PR strings before API access.
for scenario in overwrite symlink duplicate invalid; do
  export CASE=$scenario CALLS="$tmp/$scenario.calls" STATE="$tmp/$scenario.state"
  output="$tmp/$scenario.output"; args=(1032)
  case "$scenario" in
    overwrite) mkdir "$output"; printf 'retain\n' > "$output/sentinel";;
    symlink) ln -s "$tmp/overwrite.output" "$output";;
    duplicate) args=(1032 1032);;
    invalid) args=('1032;touch /tmp/not-allowed');;
  esac
  actual=0
  bash "$script_dir/capture-codex-handoff.sh" "$output" "${args[@]}" > "$tmp/$scenario.stdout" 2> "$tmp/$scenario.stderr" || actual=$?
  [[ $actual == 2 && ! -e $CALLS ]]
  tests=$((tests + 1))
done
[[ $(cat "$tmp/overwrite.output/sentinel") == retain ]]
printf '%s offline scenarios passed; gh transport is faked, no live GitHub call performed.\n' "$tests"

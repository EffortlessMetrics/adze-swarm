#!/usr/bin/env bash
# Libtest's "running N tests" includes ignored tests. Require an executed
# pass/fail count from a result summary, allowing auxiliary empty targets.
required_test_output() {
  local output="$1" label="$2" line
  local executed=0 saw_ok=false
  local summary='^test result: (ok|FAILED)\. ([0-9]+) passed; ([0-9]+) failed;'
  while IFS= read -r line; do
    if [[ "$line" =~ $summary ]]; then
      executed=$((executed + 10#${BASH_REMATCH[2]} + 10#${BASH_REMATCH[3]}))
      if [[ "${BASH_REMATCH[1]}" == ok ]]; then
        saw_ok=true
      fi
    fi
  done <<<"$output"
  if [[ "$saw_ok" != true ]] || (( executed == 0 )); then
    printf '%s executed no tests with a successful result summary; refusing vacuous pass.\n' "$label" >&2
    return 1
  fi
}

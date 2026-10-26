#!/usr/bin/env bash
# One read-only evidence pass for adze-swarm. No readiness/admission decision.
# Usage: bash scripts/capture-codex-handoff.sh NEW_OUTPUT_DIRECTORY PR_NUMBER...
set -euo pipefail
umask 077
repo=EffortlessMetrics/adze-swarm
if (( $# < 2 )); then
  printf 'Usage: %s NEW_OUTPUT_DIRECTORY PR_NUMBER...\n' "$0" >&2
  exit 2
fi
out=$1
shift
case "$out" in ''|-) printf 'A new output directory is required.\n' >&2; exit 2;; esac
seen=' '
for pr in "$@"; do
  if [[ ! $pr =~ ^[1-9][0-9]{0,8}$ ]] || [[ $seen == *" $pr "* ]]; then
    printf 'Invalid or duplicate PR number: %s\n' "$pr" >&2
    exit 2
  fi
  seen+="$pr "
done
for tool in gh jq; do
  command -v "$tool" >/dev/null || { printf 'Missing tool: %s\n' "$tool" >&2; exit 2; }
done
# Exclusive creation: never overwrite a prior receipt or follow an existing link.
mkdir -- "$out" || { printf 'Output directory must not already exist.\n' >&2; exit 2; }
export GH_PROMPT_DISABLED=1 GH_PAGER=cat
printf '%s\n' "$@" | jq -s '.' > "$out/selected-prs.json"
printf 'collection_in_progress\n' > "$out/INCOMPLETE"
errors=0
observations=0
printf '' > "$out/problems.jsonl"
problem() {
  errors=$((errors + 1))
  jq -nc --arg resource "$1" --arg reason "$2" \
    '{resource:$resource,reason:$reason}' >> "$out/problems.jsonl"
}
read_api() {
  local file=$1 endpoint=$2 paged=$3 predicate=$4
  local -a command=(gh api --hostname github.com --method GET "$endpoint")
  if [[ $paged == yes ]]; then command+=(--paginate --slurp); fi
  # stderr is local operational evidence; do not upload it without inspection.
  if ! "${command[@]}" > "$out/$file.json" 2> "$out/$file.stderr"; then
    problem "$file" api_failed
    return 1
  fi
  if ! jq -e "$predicate" "$out/$file.json" >/dev/null 2>&1; then
    problem "$file" invalid_or_incomplete_payload
    return 1
  fi
  observations=$((observations + 1))
}
branch_shape='type == "object" and (.commit.sha | type == "string" and test("^[0-9a-f]{40}$"))'
pages_shape='type == "array" and length > 0 and all(.[]; type == "array")'
check_shape='type == "array" and length > 0 and all(.[]; (.check_runs | type == "array") and (.total_count | type == "number")) and ([.[].total_count] | unique | length == 1) and ([.[].check_runs[]] | length) == .[0].total_count and ([.[].check_runs[].id] | unique | length) == .[0].total_count'
read_api main-before "repos/$repo/branches/main" no "$branch_shape" || true
read_api open-prs-before "repos/$repo/pulls?state=open&per_page=100" yes "$pages_shape" || true
for pr in "$@"; do
  pr_shape="type == \"object\" and .number == $pr and .base.repo.full_name == \"$repo\" and (.state == \"open\" or .state == \"closed\") and (.merged | type == \"boolean\") and (.head.sha | type == \"string\" and test(\"^[0-9a-f]{40}$\")) and (.base.sha | type == \"string\" and test(\"^[0-9a-f]{40}$\"))"
  if ! read_api "pr-$pr-before" "repos/$repo/pulls/$pr" no "$pr_shape"; then continue; fi
  state=$(jq -r '.state' "$out/pr-$pr-before.json")
  # Preserve merged and closed-unmerged distinctly; neither implies product acceptance.
  if [[ $state == open ]]; then
    head=$(jq -r '.head.sha' "$out/pr-$pr-before.json")
    head_repo=$(jq -r '.head.repo.full_name // ""' "$out/pr-$pr-before.json")
    if [[ $head_repo != "$repo" ]]; then
      problem "pr-$pr" fork_head_checks_not_collected
    else
      read_api "pr-$pr-H-checks" "repos/$repo/commits/$head/check-runs?filter=all&per_page=100" yes "$check_shape and all(.[].check_runs[]; .head_sha == \"$head\")" || true
      read_api "pr-$pr-H-statuses" "repos/$repo/commits/$head/statuses?per_page=100" yes "$pages_shape" || true
    fi
    test_merge=$(jq -r '.merge_commit_sha // ""' "$out/pr-$pr-before.json")
    # P is GitHub's observed test-merge ref, NOT an asserted merge-group G.
    if [[ $test_merge =~ ^[0-9a-f]{40}$ && $test_merge != "$head" ]]; then
      read_api "pr-$pr-P-checks" "repos/$repo/commits/$test_merge/check-runs?filter=all&per_page=100" yes "$check_shape and all(.[].check_runs[]; .head_sha == \"$test_merge\")" || true
    fi
    read_api "pr-$pr-reviews" "repos/$repo/pulls/$pr/reviews?per_page=100" yes "$pages_shape" || true
    read_api "pr-$pr-review-comments" "repos/$repo/pulls/$pr/comments?per_page=100" yes "$pages_shape" || true
    read_api "pr-$pr-discussion" "repos/$repo/issues/$pr/comments?per_page=100" yes "$pages_shape" || true
    if read_api "pr-$pr-after" "repos/$repo/pulls/$pr" no "$pr_shape"; then
      before=$(jq -c '[.head.sha,.base.sha,.state,.draft,.merged,.merge_commit_sha]' "$out/pr-$pr-before.json")
      after=$(jq -c '[.head.sha,.base.sha,.state,.draft,.merged,.merge_commit_sha]' "$out/pr-$pr-after.json")
      [[ $before == "$after" ]] || problem "pr-$pr" identity_or_state_changed_during_collection
    fi
  fi
done
read_api open-prs-after "repos/$repo/pulls?state=open&per_page=100" yes "$pages_shape" || true
read_api main-after "repos/$repo/branches/main" no "$branch_shape" || true
if jq -e "$branch_shape" "$out/main-before.json" >/dev/null 2>&1 && \
   jq -e "$branch_shape" "$out/main-after.json" >/dev/null 2>&1; then
  [[ $(jq -r '.commit.sha' "$out/main-before.json") == $(jq -r '.commit.sha' "$out/main-after.json") ]] || problem main main_moved_during_collection
fi
if jq -e "$pages_shape" "$out/open-prs-before.json" >/dev/null 2>&1 && \
   jq -e "$pages_shape" "$out/open-prs-after.json" >/dev/null 2>&1; then
  before=$(jq -c '[.[][] | [.number,.head.sha,.base.sha,.draft]] | sort' "$out/open-prs-before.json")
  after=$(jq -c '[.[][] | [.number,.head.sha,.base.sha,.draft]] | sort' "$out/open-prs-after.json")
  [[ $before == "$after" ]] || problem open_prs open_inventory_changed_during_collection
fi
jq -n --arg repo "$repo" --arg collected_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --argjson errors "$errors" --argjson observations "$observations" \
  --slurpfile selected "$out/selected-prs.json" --slurpfile problems "$out/problems.jsonl" \
  '{schema:"adze.handoff.observation.v1",repository:$repo,collected_at:$collected_at,
    selected_prs:$selected[0],successful_reads:$observations,problems:$problems,
    collection_complete:($errors == 0),qualification:false,admission:false,
    limits:["Sequential observation, not an atomic snapshot or freshness guarantee",
      "No local process, dirty worktree, native queue membership or inherited protection inventory",
      "REST review comments do not establish resolved-thread state",
      "No latest-success selection; all returned check attempts including pending remain raw",
      "Check endpoint history is subject to GitHub retention and suite limits",
      "Actual G membership/run/attempt and current authorization need separate live readback"]}' \
  > "$out/summary.json"
if (( errors > 0 )); then
  printf 'Incomplete observation: %s problem(s); inspect %s/summary.json\n' "$errors" "$out" >&2
  exit 1
fi
rm -- "$out/INCOMPLETE"
printf 'Collected read-only evidence in %s; this is NOT approval or qualification.\n' "$out"

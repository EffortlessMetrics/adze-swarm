# Parallel agent lanes

How multiple agents work this repo concurrently without colliding.
Every rule below is grounded in a verifiable instance in repo history.
Owner: repo governance. Linked: [AGENTS.md](../../AGENTS.md),
[adze-swarm-operating-model.md](adze-swarm-operating-model.md),
[SPEC_SYSTEM.md](SPEC_SYSTEM.md), [queue-admission.md](queue-admission.md).

## 1. Worktree-per-lane + file-seam ownership

Each writer gets an isolated worktree and explicit seam ownership.
The setup (operating model):

```bash
git fetch origin
git worktree add -b swarm/<task> ../adze-wt-<task> origin/main
```

Seams are declared per PR-local slice as owned / shared / forbidden
([SPEC_SYSTEM.md](SPEC_SYSTEM.md), "Before and during implementation").
Reuse an existing same-scope branch; never open a competing branch for the
same slice (AGENTS.md: "Reuse an existing same-scope PR").

On overlap: coordinate the shared interface, do not serialize the lanes.
The worked example in the operating model: after #1023's target-manifest
repair lands, #1073 still owns Cargo cwd/config and snapshot isolation,
while #1068 independently improves document source indexing — neither lane
needs closure of the other or of the #859 architecture epic to start.

## 2. Rebase-verify-push-queue loop with exact-head OIDs

Admit only the exact reviewed head through the protected path (AGENTS.md).
The H / G / M identities are distinct ([queue-admission.md](queue-admission.md)):

- **H** — exact PR source head (full 40-hex SHA) reviewed and locally validated.
- **G** — actual integration composition being qualified; stored separately.
- **M** — protected squash commit produced by landing, never by the record.

Nothing infers H from `github.sha`, a queue-ref suffix, commit dates, or
commit counts. Resolve exact heads with `git ls-remote`, never guessed
suffixes. Changed heads, new material findings, and revoked authorization
invalidate eligibility; repair, re-review, and re-emit (queue-admission
`revoke`: a prior clean review never suppresses a new finding).

Behind-only restacking onto a moved `main` without re-verifying H is
avoided for the same reason: the admission record binds the reviewed H,
and re-validation (`queue-admission validate`) fails closed on
`head_mismatch`. Rebase, then re-prove, then re-record.

## 3. Research-first, and the "already landed" check

Research answers a named question with a bounded output; it is not a
recurring permission request (AGENTS.md). Before implementing, refresh the
live queue and verify the work is not already done:

```bash
gh pr list --repo EffortlessMetrics/adze-swarm --state open
gh issue view 853 --repo EffortlessMetrics/adze-swarm
```

Two cases where work turned out already done:

- `docs/proposals/ADZE-PROP-0003-glr-toolkit-productization.md:172`
  records that its first work item "already landed in
  `EffortlessMetrics/adze-swarm#124`" — the proposal scope shrank on contact
  with the log.
- `docs/status/CORRECTNESS_PUSH.md` ("Post-Queue Issues"): #464 and #465
  were closed after behavior canaries landed, with "reopen or replace only
  for new behavior work" — and "do not resurrect old PR numbers from
  handoff notes unless `gh pr view <PR>` shows that the PR is still open."

## 4. Red-before proof and nonzero-selection evidence

Run focused failing/passing tests with the required feature profile and
verify nonzero selection. What counts as executed proof (AGENTS.md,
SPEC_SYSTEM.md "Evidence and claims"):

- Record source/build identity, toolchain/features, full command, exit
  status, and actual selected/executed/ignored counts.
- A test-name substring, helper function, cfg-disabled test, zero-test
  invocation, or missing failure text is **not** evidence the intended
  behavior ran.

What does **not** count: zero selected tests, an ignored-only run, absent
failure text, a bare count of `test result:` lines, a source review or
Python model check in place of a Rust execution receipt.

Enforced in history: `548368d8` "fail required checks on zero-test
selection (#995) (#1201)" added `scripts/lib/required-test-output.sh` so
empty selection fails the gate; `ca984b33` "fail required results when
selected proof fails or cannot run (#995) (#1126)"; `fcc5c5db` "reject
vacuous and unsupported proof inventory rows (#1058) (#1231)".
The negative example to avoid: the closed #501 JavaScript canary-ignore
PR — do not weaken strict canaries to make a dashboard green
(CORRECTNESS_PUSH.md).

## 5. Stop-and-report seam rules

A lane stops at a concrete missing authority, interface, tool, or evidence;
it names the blocker and preserves the next executable action while
independent lanes continue. Never cross into another lane's files to
"finish faster." Three real boundaries:

1. **Test-only harnesses are not production code.**
   `docs/reference/parser-engine-ownership.md` (entry-point inventory):
   production routes must not call test-only harnesses
   (`runtime-e2e` helpers, `glr-test-support`). The #897 retirement
   (`2b3c9463` "delete legacy parser engines", `f156b67c` ownership-map
   update) deleted `parser_v2/v3`, legacy `glr`, `incremental*`, and
   `parallel_parser` — a lane needing those reaches a blocker, not a
   resurrection permit.
2. **Release/promotion needs separate authority.**
   Promotion, tags, publishing, signing, and Cargo-token operations require
   the #325 process; ordinary swarm work stays in adze-swarm (operating
   model "Mission and repository boundary"). A lane that discovers it needs
   a publish step stops and reports instead of improvising one.
3. **Repo boundary: swarm vs public adze.**
   AGENTS.md: do not open swarm work against public `adze`/`adze-dev`
   unless explicitly asked for a public-repo promotion/sync. Cross-repo
   drift is a handoff item, not a silent fix.

Also: do not overwrite unrelated staged work or take another writer's
worktree (SPEC_SYSTEM.md); do not hand-edit generated status — run the
named generator (AGENTS.md).

## 6. Worktree hygiene

Clean owned worktrees/cache only after code and receipts are retained, and
never delete another worker's uncommitted work (AGENTS.md; operating model
"Run and worktree ownership"). Uncommitted work in a worktree is fragile —
COMMIT the lane branch and PUSH it early and often so a cleanup cannot
strand it. A pushed branch survives worktree removal; an unpushed one may
not.

Precedent: `docs/status/FRICTION_LOG.md` FR-019 — during PR #264 cleanup,
`/tmp/adze-local-improvements` had drifted from linked worktree into
standalone-repo form, `git worktree remove` failed, and cleanup required
manual deletion plus `git worktree prune`. The fix
(`scripts/cleanup-worktrees.sh`, `just worktree-list`,
`just worktree-prune-stale`, documented in `docs/DEVELOPER_GUIDE.md`)
distinguishes linked worktrees from standalone clones and refuses to remove
the main root. Check `git worktree list` / prune stale metadata at closeout.

## 7. GitHub API rate-limit discipline

Batch reads, space writes, prefer cheap local proof over polling the API.
The rule (CORRECTNESS_PUSH.md "Live-State Refresh"): "If GitHub API access
is rate-limited, do not guess mergeability. Continue with local rebases and
tests, but report that live PR count could not be refreshed."

Concretely: one `gh pr list` refresh per decision point, not per command;
never sweep-cancel runs by workflow display name (AGENTS.md — cancel only
explicitly owned run IDs after checking repository, workflow, event, and
ref); treat Ready/reopen as expensive, not a cheap control action (#1028
readback); offline validators (e.g. #1063's record validator) are not live
admission providers.

## 8. What the human (not agents) must do

- **Merge decisions and landing authorization.** Successful G fulfills the
  conditional landing decision, but the decision itself, any revocation,
  and the #1028 protected-cutover ownership sit with the delivery owner /
  Steven — agents record evidence and request admission, they do not
  self-authorize (queue-admission.md "Boundaries"; operating model).
- **Draft/undraft calls.** #1070 restores full non-draft qualification
  before queue qualification; Ready/reopen triggers expensive work, so
  flipping draft state is a human spending decision, not an agent shortcut.
- **Shared-seam arbitration.** Agents coordinate shared
  compiler/runtime/CLI interfaces (AGENTS.md); when lanes genuinely
  contend on one seam and cannot coordinate, the lane stops, names the
  owner/decision needed, and the delivery owner sequences the work — the
  bounded protected cutover is finished by its owner while other lanes
  continue (operating model).
- **Issue closure verdicts.** Agents report "what landed, its evidence,
  remaining acceptance, next action" (SPEC_SYSTEM.md "Required headers
  and closeout"); reconciling that against #853 acceptance and declaring
  a child/parent closed follows the receipts, never a finished batch,
  empty queue, or green subset (AGENTS.md).
- **Campaign stop.** Only selected product acceptance satisfied, Steven's
  explicit pause, or blockers covering all useful remaining work stops the
  campaign. A finished batch, empty queue, or green subset never does.

# Adze-swarm operating model

Status: active operating policy
Owner: repo governance
Created: 2026-05-29
Last reviewed: 2026-10-22
Linked issues: #853, #617, #1025–#1029, #915–#920
Linked source-of-truth system: [SPEC_SYSTEM.md](SPEC_SYSTEM.md)
Linked release authorization tracker: #325
Linked runner evidence tracker: #598
Support-tier impact: none
Policy impact: execution guidance; no workflow or protection mutation

## Mission and repository boundary

`EffortlessMetrics/adze-swarm` develops and proves release-promotable Adze states.
Public `EffortlessMetrics/adze` remains promotion, release and external intake.
Ordinary swarm code, tests, research, docs and CI work belongs in adze-swarm.
Public promotion, tags, publishing, signing, Cargo-token operations and real
crates.io-install claims require the separate #325 authority and receipts.

The selected product endpoint is the **0.10.0** boundary in #853, not 1.0 and not
the completion of a session's chosen batch. ADZE-SPEC-0012 defines three user
paths: typed values, native tooling documents, and a selected-tree compatibility
subset. Improvements must make those paths work, not merely raise a test count.

## Read current state, then select work

Start from [AGENTS.md](../../AGENTS.md), the current owning issue/PR and accepted
spec/ADR. #617 retains bounded research questions; it is not a global work lock.
Refresh live state before opening or taking over a PR:

```bash
gh pr list --repo EffortlessMetrics/adze-swarm --state open
gh issue view 853 --repo EffortlessMetrics/adze-swarm
```

Choose eligible issue-led work with a named behavior/oracle, known facts,
unknowns, dependencies, seam ownership and proof. Do not create speculative
replacement systems or competing same-scope PRs. Several independent lanes may
progress concurrently; every writer uses an isolated worktree. Coordinate shared
interfaces without making whole-parent closure a dependency when only one
concrete interface is needed.

For a new owned lane, a typical worktree setup is:

```bash
git fetch origin
git worktree add -b swarm/<task> ../adze-wt-<task> origin/main
```

Use an existing owning branch when one already exists. Do not move or clean
another worker's checkout. #915–#920 govern migration to requirements and PR-local
slices; legacy `.adze/goals` state does not govern scheduling. Still-enforced
legacy checks remain real qualification dependencies until their reviewed
replacement lands; do not bypass them.

## Proof before expensive integration

Implement and test the selected slice locally, then review, repair and simplify.
Prove useful expected values/tree facts as well as projection agreement. For
invalid input, prove the actual failure class/span and no fabricated success.
A previous clean review does not exempt the next diff or a newly found defect.

Retain source/build identity, feature profile, argv, exit status and actual test
selection/execution. Reuse evidence only where its source/dependency/configuration
scope is unchanged. Do not run the same full baseline in every worker or run
standalone broad lint/test aggregates immediately before another aggregate that
already performs them. Missing tools, zero-test selections and killed processes
must not be reported as successful proof.

## Requested lifecycle versus deployed controls

#1025–#1028 own this target:

```text
implementation + focused local proof + review + repair
 -> exact reviewed source head H + revocable conditional landing decision
 -> cheap trusted native-queue admission
 -> selected expensive proof once on actual integration composition G
 -> protected squash M
 -> cheap landed acceptance reconciliation
```

Keep source H, PR test-merge/check subjects, G and M distinct. Check actual group
membership; build/merge limits do not identify the source members. Changed heads,
new material findings and revoked authorization invalidate eligibility. Successful
G completes the conditional landing decision without a routine new approval pause.

At the October 22, 2026 readback, the native queue exists but #1070 restores full
non-draft PR qualification before G qualification. This is deployed reality, not
acceptance of expensive Ready CI as the final policy. #1028 holds live cutover
status. Recheck effective repository/inherited/classic protection and workflow
producers; a checked-in settings file or this document is not deployment proof.

The observed required contexts are `Rust Small Result` and `Product Proof Result`,
not individual routed runner jobs. Their absence, unexpected skip/cancellation or
missing selected proof must not become success. #995 owns result integrity;
#998/#1058 own selected/executed accounting. #1063's offline record validator is
not by itself a trusted live admission provider.

Do not delete old producers before protected replacement admission works, spoof
qualification success, silently introduce runner fallback, relax protection or
use direct unqualified merging. Ready/reopen can currently trigger expensive
work: it is not a cheap admission API. The delivery owner finishes the bounded
protected cutover; other product lanes continue local implementation/review.
A current infrastructure blocker is not a permanent reason to warehouse all PRs.

## Run and worktree ownership

Maintain explicit owned process/run IDs and their repository, workflow, event,
source/check subject and ref. Cancel only those validated owned runs when they
are superseded or otherwise selected for cancellation. Never cancel all recent
repository runs merely because their workflow display name is outside a list.
Unknown ownership means leave the run alone and resolve the owner.

When reading checks, retain the current pending/in-progress attempt; filtering
for completed conclusions first can let an older success hide current work.
Validate the exact expected head/subject/App/event and current authorization.
Do not synthesize a passing status to compensate for a deficient poller.

Clean owned worktrees/cache only after code and receipts are retained and current
state is checked. A cleanup helper is not authorization to delete another lane's
uncommitted changes.

## Worked checkpoint

For example, after #1023's target-manifest repair lands, reconcile its narrow
acceptance; #1073 still owns Cargo cwd/config and snapshot isolation. Select that
remaining first-hour slice rather than declare #874 complete. Independently,
#1068 can improve document source indexing while the CLI owner works. Neither
lane needs closure of the other or the entire #859 architecture epic to start.

After either lands, verify its actual accepted behavior and continue through
#853's next eligible gap. The example is a dependency pattern, not a frozen
current-status board: reread the issues/PRs before taking ownership.

## Product truth and claims

`AdzeDocument` is the canonical tooling product; `parse()` remains the typed-value
fast path. Typed CST/AST, diagnostics, ambiguity, compatibility, queries, JSON and
CLI must agree on covered source/tree/schema facts without inventing a separate
parse truth. A document root is not an internal forest root; a syntax-complete
parse can still have a semantic conversion failure.

Support tiers and current receipts govern public claims. No whole-query parity,
full GLR coverage, stable incremental reuse, package-install portability or public
performance promise follows from code existence. runtime2 is development-only;
stateful scanners remain Experimental under #931. Read the selected support and
package graph rather than treating all declared features as supported.

## Stopping and handoff

A lane stops at a concrete missing authority, interface, tool or evidence; name
it and preserve the next executable action. Continue independent useful work.
Stop the campaign for the selected product acceptance being satisfied, Steven's
explicit pause, or blockers covering all useful remaining work. A batch ending,
empty PR list, session percentage or green subset is not such a stop condition.

Release remains separate: develop/prove here, prepare an exact candidate, then
promote/publish from public adze only through the separately authorized process.

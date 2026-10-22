# Codex takeover: finish the Muse wave, then prove the Adze product

Status: execution handoff; observation below is dated, not live authority
Owner: integration lead under Steven
Created: 2026-10-22
Linked proposal: n/a; continuation of the selected campaign
Linked specs: ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
Linked ADRs: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Linked plan: productization-plan.md
Linked issues: #853 (product acceptance), #1029 (continuation), #1025–#1028 (delivery)
Linked PRs: #1032, #1074, #1075; #991 has separate maintainer ownership
Support-tier impact: none from this plan or its observation collector
Policy impact: preserves current protection; no new admission or scheduler authority

## Objective and two completion boundaries

Take over the existing branches, evidence and worktrees. Do not restart the
campaign, recreate merged PRs, or wait for Muse's allowance to reset.

1. **Finish Muse's interrupted wave:** repair and qualify the remaining #1032,
   reconcile the 25 tracked dispositions, run the integrated checkpoint, and
   safely hand off or clean only owned local state. This completes a wave.
2. **Finish the selected 0.10.0 product:** meet #853's user-shaped and architecture
   acceptance with exact candidate evidence. An empty PR list, 25/25 merges,
   or Muse's final 70% display does not meet this boundary. No 1.0 or public
   release is implied; #325 retains release authority.

## Evidence and current snapshot

Basis: the supplied `Pasted text(5).txt`, ending in Muse's quota-exhausted error,
plus live GitHub reads on 2026-10-22. The transcript reports 24/25 landed,
#1032 still qualifying, and no completed final main audit. Its earlier queued
orchestrator states are historical. Local process survival, uncommitted edits,
cache ownership, current native queue membership and latest check attempts must
be inspected on the execution host; this planning pass did not observe them.

Live main: `174f9dfb646cafb8a3df901e0629089c12cc32a5`, the #1054 merge.
Open inventory at this read: #991, #1032, #1074 and #1075.

| Item | Observed state | Next action |
| --- | --- | --- |
| #1032 | Open, non-draft, H=`2fe7011b86fb112039ac719aa0726981ca43e580` | Repair the current cardinality findings before qualification. Do not inherit “enqueue when green” as review approval. |
| #1074 | Draft, H=`d8e25fd5123f7141a535df474fd4970ad0a7b4cb` | Execute authored Rust tests, review, repair and land in place. This was not one of Muse's 25. |
| #1075 | Draft instruction-reconciliation candidate | Review and execute repo policy checks; this handoff and collector extend that existing PR, not a competing branch. |
| #991 | Open maintainer timeout/concurrency lane | Read and coordinate its scope. Do not silently rebase, close or subsume it. |

The 24 landed members of Muse's 25 are:
`#1056 #1023 #1024 #1033 #1034 #1035 #1036 #1039 #1041 #1042 #1043 #1044
#1045 #1046 #1048 #1049 #1050 #1051 #1052 #1053 #1055 #1063 #1047 #1054`.
#1057, #1070 and #1037 are additional landed prerequisite/spec changes, not
missing members of that denominator. Verify ancestry/dispositions against the
selected current main before recording the wave as reconciled.

Useful landing anchors:

| PR | Landed commit | Remaining parent contract |
| --- | --- | --- |
| #1045 | `554f459ae21ba854856136fe5f206ac84761e8a7` | #864 conversion repair; #1062 exact container values |
| #1047 | `28bb95671c06bb0c51ec4d0b2b1b76d19a9c02ff` | #981 alias/origin integration; #1061 consumers |
| #1054 | `174f9dfb646cafb8a3df901e0629089c12cc32a5` | #980 capacity/identity; #1059/#1060 actual callers |
| #1051 | `0b461257caa4c4d84776f1d475c2788cb8228c8a` | #973 integrated conformance receipt |
| #1063 | `ec214ab41502fbec35e84f1eada09a0f59f0acd0` | #1026 trusted live admission/revocation |

Sources: [main](https://github.com/EffortlessMetrics/adze-swarm/tree/174f9dfb646cafb8a3df901e0629089c12cc32a5),
[#1032](https://github.com/EffortlessMetrics/adze-swarm/pull/1032),
[#1074](https://github.com/EffortlessMetrics/adze-swarm/pull/1074),
[#1075](https://github.com/EffortlessMetrics/adze-swarm/pull/1075).
The supplied transcript is user-provided evidence, not a checked-in public log.

## Phase 0 — Take ownership without destroying state or creating a second writer

Use the existing host/repo when available. The transcript's checkout was
`H:\Code\Muse\adze-swarm`, but derive the actual path and remote; a path is
not evidence that the worktree is clean or abandoned.

1. Read the current instructions and #853/#1029, then inspect all relevant
   worktrees and processes. For a worktree with changes, retain branch/SHA,
   staged and unstaged diffs, untracked files and owner before moving anything.
   Do not run reset, clean, blanket stash, worktree prune or branch deletion.
2. Inspect the actual `orchestrate-queue*.ps1` processes/scripts and their logs.
   Muse's model stopping does not prove its child processes stopped. Coordinate
   or stop only the identified mutating orchestrator after recording exact PID,
   command, start time and ownership. Do not kill all PowerShell/gh/Cargo work.
   Leave legitimate in-flight GitHub qualification running unless a specific
   reviewed reason requires cancelling that owned run.
3. Refresh main, PRs, review threads, native queue and effective protection.
   A closed-unmerged PR is not a merged PR. Record H, observed PR test-merge P,
   actual queue G and landed M separately; do not infer G membership from a ref
   suffix or queue build/merge limits.
4. Assign one integration owner for queue mutations and one writer per worktree.
   Other workers may review, reproduce, implement and test disjoint work.

Read-only local inspection, run from the verified checkout:

```bash
git remote -v
git status --short --branch
git worktree list --porcelain
git log -8 --oneline
gh pr list --repo EffortlessMetrics/adze-swarm --state open --limit 100
```

PowerShell process inspection (read only):

```powershell
Get-CimInstance Win32_Process |
  Where-Object { $_.CommandLine -match 'orchestrate-queue|adze-wt|adze-swarm' } |
  Select-Object ProcessId, ParentProcessId, CreationDate, CommandLine
```

This is an inventory filter, not a kill list. Record tool versions and verify
`git config user.name` is `EffortlessSteven` and the configured GitHub noreply
email is `15812269+EffortlessSteven@users.noreply.github.com` before new commits.
Do not change global account settings as part of the handoff.

### Read-only evidence helper

The accompanying Bash script uses the existing `gh` and `jq` tools. It does
not implement an admission service or another scheduler. Run with Git Bash on
Windows or Bash on Linux; create the parent directory first and use a new output
name for each explicit observation.

```bash
mkdir -p target/handoff
bash scripts/test-capture-codex-handoff.sh
bash scripts/capture-codex-handoff.sh target/handoff/takeover-20261022-01 \
  1056 1023 1024 1032 1033 1034 1035 1036 1039 1041 1042 1043 1044 \
  1045 1046 1048 1049 1050 1051 1052 1053 1055 1063 1047 1054 1074 1075 991
```

All network calls are GETs against adze-swarm. The collector preserves raw pages,
all returned checks (including pending attempts), review submissions/comments,
current source heads, and observed test-merge check subjects. It checks selected
PR identity and before/after head/base/state/main changes. Output is exclusively
created; API failure, malformed/truncated check results or observed movement
leave `INCOMPLETE` and return nonzero. Other local failures/interruption also
leave `INCOMPLETE`. Inspect local stderr before sharing it.

`collection_complete` means the selected reads completed without a detected
inconsistency; **it is not qualification, approval, atomicity or guaranteed
freshness**. The script never selects an older successful check over a pending
attempt. It makes no green/red admission decision at all. It does not collect
local dirty state, native queue membership, review-thread resolution or inherited
rules. It has no independent network watchdog; interrupt a stuck collection and
retain its incomplete output rather than inferring success.

Native queue readback, separately and without mutation:

```bash
gh api graphql --paginate --slurp \
  -f query='query($endCursor:String){repository(owner:"EffortlessMetrics",name:"adze-swarm"){mergeQueue(branch:"main"){entries(first:100,after:$endCursor){nodes{state pullRequest{number headRefOid}} pageInfo{hasNextPage endCursor}}}}}'
```

Use actual run and attempt metadata for qualification; a URL's numeric run ID
alone does not order reruns of that same run. Review all unresolved threads at
the current head, including material findings on already-landed source. API
inaccessibility is an explicit blocker/non-observation, not an empty queue.

## Phase 1 — Finish the inherited candidates

### A. #1032: repair before landing

Work on `swarm/node-types-cardinality-863`, not a competing PR. At the inspected
head the source still ORs optionality across occurrences and reads only the
first member of a Sequence. Current reviews also identify epsilon/choice and
normalization/second-generator coverage questions. Verify each against source;
a suggested bot patch is not the acceptance oracle.

Required cases:

- Mandatory plus optional occurrences of one field in one alternative still
  guarantee presence. Combine occurrences within an alternative before
  combining alternatives; absence from an alternative remains meaningful.
- A sequence contributes all admitted children and target types, including a
  repeat in its tail and an optional prefix followed by a required member.
  Multiplicity is about possible child count, not merely `any(member.multiple)`.
- Epsilon contributes no real child. Resolve empty sequence/choice behavior
  against the admitted grammar contract and #1047's empty-field rejection;
  do not invent a required `empty` node to satisfy old snapshots.
- Execute an annotated grammar through the actual generation/normalization
  path. Identify which node-types producer supplies the artifact. Direct-IR
  tests alone cannot establish wrapper semantics survive normalization.

A bounded correct subset may land with explicit remaining limitations under
#1061/#863. Do not require all typed-CST/query migration to fit this PR, and do
not call the node-types-only slice the shared-schema completion.

If this exact defective head is already enqueued, the integration owner must
prevent that head from landing using the existing permitted queue controls;
record why. Do not pause unrelated qualified product candidates or weaken rules.

### B. #1074: execute the existing implementation

This branch has authored code/tests, not a Rust execution receipt. Include the
builder mechanism test as well as the five source-position unit tests; the
initial PR command list did not select that added builder test.

```bash
cargo test -p adze --lib document::source_position::tests -- --list
cargo test -p adze --lib document::tree_builder::tests -- --list
cargo test -p adze --lib document::source_position::tests -- --nocapture
cargo test -p adze --lib document::tree_builder::tests -- --nocapture
cargo test -p adze --features pure-rust --test document_source_positions -- --nocapture
cargo test -p adze --features "pure-rust,glr" --test document_source_positions -- --nocapture
cargo test -p adze --features "pure-rust,glr,serialization,ts-compat" --test projection_equivalence -- --nocapture
cargo clippy -p adze --lib -- -D warnings
```

Confirm actual compiled names and feature gates before relying on filters.
Expected authored additions: five source-position unit tests, one builder test,
two integration cases without GLR and three with GLR. Formatting and real tests
may expose defects; repair them, review, then qualify and land this existing PR.
#1068 stays open for flat construction/remaining indexing and #901 for storage.

### C. #1075: adopt consistent operating instructions

Review the six entrypoint changes plus this handoff/tooling addition. Execute
`bash scripts/test-capture-codex-handoff.sh`, whitespace checks and the current
Cargo-backed doc/file-policy commands after inspecting their help/selection.
Do not delete active-goal checkers to make this pass; route a genuine machinery
conflict to #920. The shell files are within the existing `scripts/**` tooling
registration and this plan is within `plans/**`; run the actual checker rather
than inferring acceptance solely from those patterns. Then qualify and land.

## Phase 2 — Repair inherited defects on main, not on merged PR branches

These are post-merge follow-ups. Credit the landed work and keep the remaining
acceptance explicit. Use focused new PRs under the existing issues after checking
for overlapping work; do not reopen old PRs or pretend their fixes never landed.

| Owner | Source-grounded priority at main `174f9dfb` | Required independent witness |
| --- | --- | --- |
| #980 / #1059 / #1060 | `try_normalize` eagerly uses `max.checked_add(1).expect(...)` for production slots, and increments again when allocating. Source-visible panic remains in a fallible API. | Primitive grammar with maximum ProductionId and no helper need; final available slot; genuine exhaustion with typed non-mutating failure; no regression of SymbolId allocation or field origins. |
| #864 | `WithTryLeaf::try_extract` maps invalid present-node text to `""` with `unwrap_or_default()` before invoking user conversion. | Invalid/inverted/out-of-bounds/non-UTF-8 present node and empty-accepting converter: structured failure, zero converter calls. Keep legitimate missing/epsilon semantics distinct. |
| #864 / #1072 | Review unit-item attribute detection and caller-resolved Result/String in generated fallible code after #1045. | Generated unit struct/variant with a rejecting transform; downstream name-shadowing compile pass; open custom Extract compatibility. |
| #1062 | Fallible conversion does not establish repetition traversal selects the right tokens. | Exact singleton/multiple/delimited/nested Option/Spanned values and ranges, empty/nonempty contracts and malformed delimiters. |
| #874 / #1073 / #997 | #1023/#1024 landed narrow manifest/scaffold repairs, not the full first-hour acceptance. | Target-local Cargo config from unrelated cwd; update isolation; repeat init and unowned staging/sentinel preservation at current source. |

Capacity and conversion findings above were verified in source, not reproduced
in Rust by this planning pass. Treat the new minimal Rust test as the next
execution obligation, not as already red/green evidence.

Compiler capacity must be fixed at its actual owning allocator/validation seam,
not by weakening high-ID tests or forcing every production slot to be unique.
Useful many-to-one production metadata remains valid. Reuse #1059/#1060 for
caller migration and shared-production parsing behavior.

## Phase 3 — Integrated proof and remaining product delivery

One integration owner runs a checkpoint on a clean, named main/candidate source.
Use existing actual-G proof where its source/configuration applies, rather than
repeating every full baseline in every worker. Run focused new tests while
repairing; widen once after the repairs converge. Preserve logs and process exits.

The current `scripts/ci-supported.sh` covers seven core crates at its selected
features plus GLR-core serialization doctests. It does **not** alone establish
CLI packaging, every GLR feature profile, conformance or installed behavior.
An exit-zero summary with zero intended tests also does not establish those.

Suggested checkpoint obligations, after checking target existence/features:

```bash
just ci-supported
cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors --test fallible_transforms --test generated_grammar_matrix --test document_parse_agreement -- --nocapture
cargo test -p adze --features "pure-rust,glr,serialization,ts-compat" --test projection_equivalence -- --nocapture
```

Add the exact #1032/#1062/#1073 regressions selected for this candidate. For
PowerShell native commands preserve `$LASTEXITCODE` before running another
command or interpreting a log; in Bash pipelines retain `pipefail`. The count
of `test result:` lines and a grep for no `FAILED` are not process results.

Conformance now has all four components on main. Run the existing #973–#977
harness with corpus, Python reference and candidate-built driver from the
selected clean tree. Require the 13-case denominator and its existing negative
controls; a skipped end-to-end test or historical sibling-checkout receipt does
not qualify the integrated candidate. Keep this advisory, not per-PR fanout.

Then continue these existing workstreams:

1. **Native parsing/errors:** #1030 + #896, then #897. Carry native GLR failure
   facts; stop substituting a full typed LR parse's diagnostics. Prove actual
   engine invocation, clean/failure/clean behavior and syntax versus conversion.
2. **Compiler/schema:** #1059/#1060/#981 -> #1061. Prove one compact generated
   repeated/optional/aliased fixture across IR, tables, typed values, document,
   node-types and query metadata before broadening the corpus.
3. **Document/tooling:** #899 interface + #1068 construction -> #860/#861/#900
   consumers -> #901 retirement; #1069 retains all capture occurrences and
   correct rollback/predicates. Do not make consumer migration wait on retirement.
4. **Installed first hour:** #864/#1073/#997 -> #874; #1064/#1065/#880 produce
   selected Linux/Windows archive-to-installed-binary receipts. Preserve separate
   original/transformed archives and actual command/resolution/binary identity.
5. **Release-boundary quality:** #867 scoped blocking debt control, #1071 bounded
   runtime reliability, #1072 external API/profile compatibility; #1066 then
   #1067 private protocol/cache; #869 performance, #873 claim reconciliation,
   #882 compiled recipes, #915–#920 governance parity, #881 read-only handoff.

Do not use cache work, broad governance migration, or advisory EBNF/reference
expansion as prerequisites for ordinary parser/first-hour repairs. Stateful
scanners remain Experimental under #931; runtime2/incremental/full-query parity
and LSP are not silently promoted into the current completion denominator.

## Parallel ownership and build economics

Use four initial streams plus the integration lead; scale only as compute and
nonoverlapping work permit. These are assignments, not Codex/Muse product limits.

| Stream | First assignment | Shared seam / exit |
| --- | --- | --- |
| Schema | #1032 repair and generated-path proof | Own node_types; coordinate #1061, then hand off exact supported subset. |
| Runtime | #864 post-merge safety, then #1062 | Own extraction/macro edits; serialize with native-error work in __private.rs. |
| Compiler | #980 capacity repair, then #1059/#1060 | One writer on ir/src/lib.rs; preserve landed #1044/#1047/#1054 logic. |
| Qualification/DevEx | Verify #1074/#1075; prepare #1073/package witnesses | Separate worktrees; coordinate cli/src/main.rs with starter/protocol work. |
| Integration lead | Local takeover, current review/evidence, queue lifecycle, final acceptance | Own queue mutations and ledger reconciliation, not all implementation. |

Review may happen in parallel with implementation elsewhere. Bound concurrent
Cargo builds by actual memory/disk/CPU; do not run four full seven-crate builds
against the same resources. Preserve reusable caches, but bind every claimed
result to source/features/toolchain/target and dirty state. Do not delete all
target directories merely because one PR landed.

## Delivery lane: finish the selected economics without bypassing protection

#1070 currently restores expensive Ready-PR qualification before G. That is
observed deployed behavior, not a new ruling accepting it. #1025–#1028 own the
remaining cheap exact-H admission/one-expensive-G cutover. #1063 is a landed
record/fixture validator; it is not yet the authoritative live provider.

Prove the existing phase-aware lifecycle-gate candidate under equivalent
protection before changing required contexts/producers. Keep Rust/Product proof
results honest, enforce current revocation and actual group membership, and
consume #995 fail-closed aggregation plus #1058 executed denominators. No fake
greens, dispatch-as-PR-proof assumptions or temporary check relaxation.

Do not bulk reopen/Ready PRs, rerun successful jobs, restack behind-only branches,
or blindly re-enqueue every ejected candidate. Diagnose the actual failure and
reuse only matching evidence. Fixing delivery is parallel enabling work, not a
reason to stop all local product progress or to strand candidates indefinitely.

The transcript's v2 orchestrator improved over v1: it retains pending entries
and reads queue membership. It still selects by a numeric run ID; that is not a
complete event/subject/App/run-attempt check. Only excerpts are supplied, so
inspect the full script before attributing unseen cancellation behavior to v2.
A queue-build limit of five is neither a membership proof nor a reason to change
concurrency without actual capacity evidence.

## Closeout and continuation

For every landed slice record H, actual G composition/run/attempt, M, selected
proof and remaining parent acceptance. Resolve review threads only after the
finding is fixed or explicitly refuted with evidence. A prior clean review does
not suppress a new finding, including one discovered after merge.

Wave closeout needs: all 25 dispositions reconciled; #1032 repaired or an explicit
blocked disposition; integrated checkpoint retained; surviving local work and
processes owned; cleanup limited to clean, pushed, no-longer-used owned state.
No issue is bulk-closed merely because a linked PR merged.

Product closeout needs #853's accepted 0.10.0 cells: useful exact valid values,
native trustworthy invalid-input behavior, consistent projections, installed
first-hour proof, and remaining architecture/reliability/API claims satisfied
or explicitly deferred by Steven with accurate non-claims. A blocked stream
reports its exact blocker while other streams continue. Public promotion and
publication require #325's separate authorization.

## Supporting-tool validation and rollback

`bash -n` plus the offline shell harness cover successful paginated collection,
pending preservation, API error, malformed JSON, missing/duplicate check rows,
main/head movement, wrong PR identity, closed-unmerged classification, and
refusal to overwrite files/links or accept duplicate/injected PR arguments.
The harness fakes gh transport: it does not prove real authentication, Git Bash
compatibility or live API behavior. Execute one real read-only capture on the
host before relying on its output. No hosted CI or parser execution is needed
to test the collector locally. Revert these three added files if defective;
retain previous receipts, current source work and protected delivery mode.

Platform references checked 2026-10-22:
[gh api pagination](https://cli.github.com/manual/gh_api),
[check-run listing and history limits](https://docs.github.com/en/enterprise-cloud@latest/rest/checks/runs#list-check-runs-for-a-git-reference).

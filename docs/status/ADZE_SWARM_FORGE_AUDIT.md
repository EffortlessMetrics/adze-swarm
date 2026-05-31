# Adze-swarm forge endpoint audit

Status: current audit
Owner: repo governance
Updated: 2026-05-30
Scope: `EffortlessMetrics/adze-swarm` development/proof forge endpoint
Completion: not complete

This audit maps the current forge endpoint requirements to concrete repo
evidence. It is not a release authorization, publish checklist, or crates.io
install receipt.

## Summary

`adze-swarm` is back in paused forge standby after completing the non-release
Adze Adoption Hardening lane. The near-term CI governance, repo-boundary,
proof-refresh, release-candidate bundle, and adoption-hardening tasks are
complete. Post-closeout audit, CI-routing, and active-goal verifier hygiene are
also complete, including the Windows `check-msrv` shell-path tooling fix and
the Windows `just build` PDB collision fix. A CX53 Rust Small stale-queue
receipt is now tracked as a blocked runner investigation, with route candidate
diagnostics, a temporary Rust Small route quarantine, label/group diagnostics,
route-runner self-count exclusion, and PR body guidance for avoiding accidental
issue auto-close.
Legacy PR Gate queued runs left behind
after #602 and #603 merged are tracked separately by issue #604 and mitigated
by PR-close cancellation in #606. PR #629 records the explicit hosted
control-plane exception for routing, path detection, capacity diagnostics, and
aggregate result reporting while keeping Rust/product execution self-hosted by
default. PR #630 moves serialized CI Policy source-of-truth checks
into the same hosted control-plane exception. PR #609 resolves issue #549 by
explicitly
keeping the paused forge standby state instead of selecting a new
implementation lane. The broader product/release endpoint is not complete
because release/publish authorization and a real crates.io `adze-cli` install
receipt are still absent.

## Prompt-to-artifact checklist

| Requirement | Evidence | Status |
| --- | --- | --- |
| Work starts from `adze-swarm/main`. | `AGENTS.md`; `docs/reference/adze-swarm-operating-model.md`; `active.toml` standby state. | Covered. |
| Public `adze` is not the implementation/productization/CI/docs-proof target. | `AGENTS.md`; `docs/reference/adze-swarm-operating-model.md`; `docs/reference/PUBLISH_CHECKLIST.md`; `active.toml` end state. | Covered. |
| One work item per branch and PR. | `docs/reference/SPEC_SYSTEM.md`; PR template queue/scope fields; `active.toml` standby handoff. | Covered by policy. |
| PRs link source-of-truth artifacts. | `docs/reference/SPEC_SYSTEM.md`; PR template source-of-truth fields for proposal, spec, ADR, plan item, active goal, support-tier row, and policy ledger; `docs/reference/adze-swarm-operating-model.md`. | Covered by policy. |
| PRs state claim boundary, proof, CI cost, and rollback. | `.github/PULL_REQUEST_TEMPLATE.md`. | Covered by policy. |
| Default CI is self-hosted; no silent hosted fallback. | `docs/reference/CODEX_CI_EFFICIENCY_COMPATIBILITY.md`; `docs/reference/adze-swarm-operating-model.md`; PRs #539, #577, #580, #586, #603, #606, #607, #608, #610, #611, #628, and #629; PRs #572-#597 check receipts where `Rust Small on GitHub Hosted` stayed skipped; issue #598 tracks a CX53 stale-queue runner investigation from #597; issue #604 tracks stale advisory PR Gate and aggregate queued runs after merged/superseded PRs. | Covered with an explicit hosted control-plane exception for routing, path detection, source-of-truth policy checks, capacity diagnostics, and aggregate result reporting. Rust/product execution remains self-hosted unless the recorded GitHub-hosted fallback path is selected; #598 remains a blocked CI follow-up and CX53 is quarantined from Rust Small selection. |
| `Rust Small Result` remains the normalized base gate. | `AGENTS.md`; `docs/reference/adze-swarm-operating-model.md`; live branch-protection API on 2026-05-30 required `Rust Small Result` and `Product Proof Result` with `strict = true`; PRs #579-#583 checks: `Route Rust Small`, selected self-hosted Rust Small lane, and `Rust Small Result` succeeded. | Covered. |
| Heavy/advisory/coverage/benchmark/product/full-matrix lanes are scoped. | `docs/reference/CODEX_CI_EFFICIENCY_COMPATIBILITY.md`; PRs #572-#586 check summaries show broad implementation lanes skipped or cancelled for docs/status/CI-routing/verifier changes while required source-of-truth, product-proof, and Rust Small result checks stayed green. | Covered for observed recent PRs. |
| Public `adze` receives promotion only intentionally. | `docs/reference/PUBLISH_CHECKLIST.md`; `docs/reference/adze-swarm-operating-model.md`; `active.toml` release blocker. | Covered by policy; no current promotion PR. |
| `AdzeDocument` is the canonical parse product. | `docs/reference/adze-swarm-operating-model.md`; `docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md`; support-tier/product docs. | Covered by architecture docs. |
| Public views are projections over the document. | `docs/reference/adze-swarm-operating-model.md`; `ADZE-ADR-0001`; projection docs and support-tier rows. | Covered by architecture docs; not re-proven in this audit. |
| Stable claims require README/support-tier/proof/CI/examples/limitations alignment. | `docs/status/SUPPORT_TIERS.md`; `docs/status/PRODUCT_OBJECTIVE_AUDIT.md`; `docs/reference/adze-swarm-operating-model.md`. | Covered by policy; not exhaustively re-verified here. |
| `cargo install adze-cli` claim has a real crates.io receipt. | Live `cargo info --registry crates-io adze-cli` on 2026-05-30 returned not found. | Not complete. |
| No unsupported parity/performance claims. | `docs/reference/adze-swarm-operating-model.md`; `SUPPORT_TIERS.md`; `PRODUCT_OBJECTIVE_AUDIT.md`. | Covered by policy; not exhaustively re-verified here. |
| Near-term CI-efficiency rules landed. | PR #538 merged. | Complete. |
| Self-hosted implementation routing landed and runner/tooling assumptions fixed. | PRs #539, #543, #545, #577, #580, #591, #595, #603, #607, #608, #610, #611, #628, and #629 merged; #577 isolated Rust Small Cargo homes, #580 aligned CPX42 route labels, #591 removed the Windows `cygpath` dependency from `just check-msrv`, #595 removed the Windows `just build` PDB collision warning, #603 quarantined CX53 from Rust Small route selection while preserving candidate diagnostics, #607 excludes the routed Rust Small router's current route runner from idle counts when relevant, #608 aligns contributor-facing CI docs with the current capacity-policy behavior, #610 records CX53 label/group and planned rust-large diagnostics, #611 adds a manual CX53 rust-large diagnostic workflow, #628 hardens stale advisory and aggregate cancellation guards, and #629 moves routing/result control-plane work off self-hosted capacity while keeping implementation lanes self-hosted by default. The CI Policy source-of-truth checks are also moved to hosted control-plane runners as the remaining #604 follow-up. Issue #598 tracks whether CX53 should regain Rust Small eligibility after a stale selected-lane queue in #597; issue #604 tracks the remaining post-merge CI Policy capacity/control-plane receipt. | Complete for current routing/tooling assumptions once the CI Policy hosted-control-plane follow-up lands; #598 and #604 are blocked on runner/control-plane evidence. |
| Duplicate same-scope PRs collapsed. | PR #542 merged; duplicate document SRP PRs were not all merged. | Complete for the observed queue. |
| Current active goal complete/paused/superseded before new goal. | `adze-adoption-hardening` is complete and archived; `active.toml` is restored to the paused `adze-swarm-forge-standby` manifest; PR #579 made issue-tracked blocked items verifier-clean; PRs #581, #582, #585-#587, #591, #593, and #595 refreshed the standby evidence after #576-#584; PR #609 resolves issue #549 by explicitly keeping the paused standby state until a future non-release lane is selected. | Complete. |
| Public `adze` remains clean unless promotion/release. | Live `gh pr list` checks for public `adze` returned no open PRs on 2026-05-30. | Covered at audit time. |

## Current evidence snapshot

Recent merged PRs:

- #538: `docs: add hard Codex CI-efficiency compatibility invariants`
- #539: `ci: route swarm workflows to self-hosted runners only`
- #542: `Refactor document lifecycle primitives into SRP submodules`
- #545: `ci: harden ripr advisory on minimal runners`
- #546: `docs: record adze-swarm operating model`
- #547: `docs(goal): pause adze-swarm forge standby`
- #551: `docs(goal): start toolkit proof refresh`
- #552: `docs(status): record toolkit proof refresh`
- #553: `docs(goal): restore forge standby`
- #554: `docs(goal): start release candidate bundle readiness`
- #555: `docs(release): record current candidate snapshot`
- #556: `docs(release): add candidate bundle checklist`
- #557: `docs(release): record non-publish preflight receipts`
- #558: `ci: add cx33 rust small routing`
- #559: `ci: add supported gate timeout headroom`
- #560: `plans: close release candidate bundle lane`
- #561: `docs(goal): restore forge standby after release bundle`
- #562: `docs(goal): start adoption hardening lane`
- #563: `test(starter): mirror generated layout in downstream fixture`
- #564: `docs(api): tie choice guide to starter proof`
- #567: `docs(glr): clarify ambiguity walkthrough`
- #568: `docs(diagnostics): clarify recovery walkthrough`
- #569: `docs(query): clarify cookbook subset`
- #570: `docs(goal): close query cookbook item`
- #571: `ci: run cx33 rust small natively`
- #572: `docs(ts-compat): harden selected-tree adoption guide`
- #573: `docs(perf): clarify benchmark receipt guidance`
- #574: `docs(release): clarify swarm promotion boundary`
- #575: `docs(goal): close adoption hardening lane`
- #576: `docs(status): refresh forge audit standby evidence`
- #577: `ci: isolate rust small cargo homes`
- #578: `docs(status): record post-ci audit evidence`
- #580: `ci: align cpx42 route labels`
- #579: `xtask: accept issue-tracked active-goal blockers`
- #581: `docs(status): refresh standby verifier evidence`
- #582: `docs(status): avoid self-staling forge audit snapshot`
- #583: `docs(status): record branch protection receipt`
- #584: `docs(pr): require support-tier source links`
- #585: `docs(policy): fix PR template path casing`
- #586: `ci(status): record casing receipt and harden router output path`
- #587: `docs(status): refresh forge audit receipt ranges`
- #588: `docs(cli): keep source build path local`
- #589: `docs(status): refresh product audit standby state`
- #591: `fix(tooling): make check-msrv Windows-compatible`
- #593: `docs(status): record check-msrv standby receipt`
- #595: `fix(tooling): avoid just build PDB collision`
- #597: `docs(status): record just build standby receipt`
- #599: `docs(status): record CX53 runner tracker`
- #600: `ci: log routed runner candidates`
- #602: `docs(pr): avoid accidental issue auto-close`
- #603: `ci: quarantine cx53 from rust-small route`
- #605: `docs(status): record stale PR Gate queue tracker`
- #606: `ci: cancel PR Gate on PR close`
- #607: `ci: exclude route runner from idle counts`
- #608: `docs(ci): clarify rust small capacity policy`
- #609: `docs(goal): keep forge standby paused`
- #610: `ci: enrich cx53 runner diagnostics`
- #611: `ci: add cx53 rust-large diagnostic`

Current active manifest:

```text
.adze/goals/active.toml
  id = "adze-swarm-forge-standby"
  status = "paused"
```

Current live blockers:

```text
release-publish-authorization: blocked, tracked by #325
cx53-rust-small-stale-queue-investigation: blocked, tracked by #598
```

Current live queue at audit time:

```text
EffortlessMetrics/adze-swarm open PRs: none
EffortlessMetrics/adze open PRs: none
```

Current branch-protection receipt on 2026-05-30:

```text
gh api repos/EffortlessMetrics/adze-swarm/branches/main/protection/required_status_checks
strict: true
required contexts: Rust Small Result, Product Proof Result
```

Current crates.io install receipt state on 2026-05-30:

```text
cargo info --registry crates-io adze-cli
error: could not find `adze-cli` in registry `https://github.com/rust-lang/crates.io-index`
```

## Current standby and routing receipt

On 2026-05-30, PR #575 closed the non-release Adze Adoption Hardening lane and
restored the paused forge standby manifest. PR #577 fixed the runner cache
permission assumption exposed by the next audit PR by moving routed Rust Small
Cargo homes to job-scoped scratch paths. PR #578 refreshed the audit after that
CI hardening. PR #580 fixed the CPX42 route-label assumption exposed by the
active-goal verifier PR. PR #579 then made issue-tracked blocked items
verifier-clean while preserving #325 and #549 as live blockers. PR #581
refreshed the paused standby evidence after those verifier and routing fixes.
PR #585 corrected PR template path casing in the CI policy and forge audit.
PR #586 recorded that casing cleanup in the paused standby ledger and hardened
the routed Rust Small target-selection job against a missing
`$GITHUB_OUTPUT` directory on self-hosted runners.
PR #587 refreshed the audit table receipt ranges through #586.
PR #588 kept the CLI reference's current source-build path scoped to the
checkout under test instead of implying public `adze` already carries the
current swarm proof state.
PR #589 refreshed the product objective audit front matter to the current
paused standby state and recorded the #588 stable-product receipt.
PR #591 removed the Windows `cygpath` dependency from `just check-msrv` by
moving the check body out of a just shebang recipe and into
`scripts/check-msrv.sh`.
PR #593 recorded the #591 standby receipt in the checked-in source-of-truth
files after the post-merge policy run passed.
PR #595 split `just build` into a workspace build excluding `adze-cli`,
followed by `cargo build -p adze-cli`, so Windows no longer emits the
`adze.pdb` output filename collision warning between the runtime lib target
and the CLI bin target.
PR #597 recorded the #593 and #595 standby receipts in the checked-in
source-of-truth files. During #597, the first routed Rust Small attempt
selected CX53 and stayed with no job steps until cancellation; the unchanged
rerun selected CPX42 and passed. Issue #598 tracks the runner scheduling or
configuration investigation without adding hosted fallback.
PR #599 records #598 in the paused standby manifest and this forge audit
without changing runner routing or resolving the blocked investigation.
PR #600 adds route-log diagnostics for matching runner candidates by class so
future stale selected-lane incidents include the runner name, online status,
busy state, and current-runner marker without changing route priority or
hosted fallback policy. PR #610 extends those diagnostics to record candidate
labels, runner-group ID/name when available, and the planned CX53 `rust-large`
candidate set so #598 can separate wrong `rust-small` eligibility from missing
`rust-large` prep.
PR #602 updates the PR template and friction log after #599 and #600 showed
that negative PR wording can still trigger GitHub issue auto-close keywords
when the keyword is placed next to an issue number.
PR #603 temporarily quarantines CX53 from required Rust Small route selection
while #598 remains blocked. The router still logs CX53 candidate state for
future evidence, but selected Rust Small capacity now remains CPX42, CX43,
CX33, or explicit fallback by recorded exception.
After #602 and #603 merged, their advisory PR Gate workflow runs remained
queued on `Supported Rust Gate` jobs after cancellation requests. Issue #604
tracks that stale PR Gate queued-run behavior separately from the CX53 routed
Rust Small investigation. Required checks for both PRs had passed before merge.
PR #605 records #604 in the paused standby manifest and this forge audit.
PR #606 changes PR Gate so `pull_request.closed` events share the same
PR-number concurrency group, cancel older same-PR PR Gate runs, and skip every
PR Gate job on the closed event. This preserves PR Gate as optional signal
without scheduling extra self-hosted work after a PR is merged or closed.
PR #627 reproduced #604 for manually cancelled advisory runs: the original
`PR Gate` run queued `Supported Rust Gate` after `PR Plan` was cancelled, and
the `ripr advisory` workflow queued its advisory job after its `PR Plan` was
cancelled. PR #628 keeps closed-event cancellation, adds
cancelled-upstream guards to those advisory jobs, gives `ripr advisory`
the same `pull_request.closed` cancellation path, and applies the same stale
cancelled-upstream guard to routed Rust Small's aggregate `Rust Small Result`
and Product Proof's aggregate `Product Proof Result` jobs so superseded
required-gate runs do not hold the PR concurrency group.
After #628 merged, post-merge `CI Policy` run `26703643323` completed
`Source of Truth` but left `CI Lane Whitelist` and `GLR Invariants` queued
with zero steps. The #629 follow-up mitigation serializes those advisory
policy jobs with cancellation-aware dependencies so future policy runs do not
fan out three self-hosted requests at once. It also moves required
control-plane work off self-hosted capacity: `Route Rust Small`, Product Proof
path detection, aggregate result reporting, and the optional
`Runner Capacity / Fallback Policy` diagnostic use hosted control-plane
runners. The required `Rust Small Result` aggregate no longer waits on the
optional capacity diagnostic after a no-idle/no-fallback route outcome; it
records that policy outcome directly, while Rust Small implementation work and
selected Product Proof canaries remain self-hosted unless explicit
GitHub-hosted fallback is allowed by label or manual dispatch. After #629
merged, post-merge `CI Policy` run `26704822871` still left the first
serialized `CI Lane Whitelist` job queued with zero steps, so the remaining
PR #630 moves the serialized CI Policy source-of-truth jobs to hosted
control-plane runners as well.
PR #607 excludes the routed Rust Small route job's own runner from idle counts.
Candidate diagnostics still mark that runner with `current=true`, but it no
longer contributes to the selected-lane count for CPX42, CX43, CX33, or the
quarantined CX53 diagnostic count.
PR #608 aligns the contributor-facing CI overview, lane map, and baseline docs
with that capacity policy. It also records that `Rust Small on GitHub Hosted`
runs only by explicit fallback, the non-required `Runner Capacity / Fallback
Policy` job is the no-idle/no-fallback signal, and branch protection requires
both `Rust Small Result` and `Product Proof Result`.
PR #611 adds the manual `CX53 Rust Large Diagnostic` workflow as a
non-required burn-in path for #598 by probing CX53 `rust-large` candidate
visibility before running a selected host-smoke job. It does not change Rust
Small branch protection or re-enable CX53 in the required base gate.

The exact current `adze-swarm/main` commit is intentionally not hardcoded here.
Every audit refresh changes that commit and would immediately stale this
document. Verify the live selected state with:

```text
git fetch origin main --prune
git rev-parse origin/main
gh pr list --repo EffortlessMetrics/adze-swarm --state open --json number,title,url
gh pr list --repo EffortlessMetrics/adze --state open --json number,title,url
```

The latest relevant check receipts kept the current routing and claim boundary
intact:

```text
PR #579:
  CI Lane Whitelist: success
  Source of Truth: success
  GLR Invariants: success
  Route Rust Small: success
  Rust Small on CX43: success
  Rust Small Result: success
  Product Proof Result: success
  Rust Small on GitHub Hosted: skipped

PR #581:
  CI Lane Whitelist: success
  Source of Truth: success
  GLR Invariants: success
  Route Rust Small: success
  Rust Small on CX33: success
  Rust Small Result: success
  Product Proof Result: success
  Rust Small on GitHub Hosted: skipped
  Broad optional PR Gate workflow: force-cancelled after required gates passed

PR #591:
  CI Lane Whitelist: success
  Source of Truth: success
  GLR Invariants: success
  Route Rust Small: success
  Rust Small on CPX42: success
  Rust Small Result: success
  Product Proof Result: success
  Supported Rust Gate: success
  PR Gate Success: success
  Rust Small on GitHub Hosted: skipped
  Post-merge CI Policy push run 26682375507: success

PR #593:
  CI Lane Whitelist: success
  Source of Truth: success
  GLR Invariants: success
  Docs Gate: success
  Rust Small Result: success
  Product Proof Result: success
  PR Gate Success: success
  Post-merge CI Policy push run 26682659621: success

PR #595:
  CI Lane Whitelist: success
  Source of Truth: success
  GLR Invariants: success
  Route Rust Small: success
  Rust Small on CX43: success
  Rust Small Result: success
  Product Proof Result: success
  Supported Rust Gate: success
  PR Gate Success: success
  Rust Small on GitHub Hosted: skipped
  Post-merge CI Policy push run 26683508220: success

PR #597:
  CI Lane Whitelist: success
  Source of Truth: success
  GLR Invariants: success
  Docs Gate: success
  Product Proof Result: success
  PR Gate Success: success
  Attempt 1 Route Rust Small: success, selected CX53 with cx53_idle
  Attempt 1 Rust Small on CX53: cancelled after no job steps
  Attempt 1 Rust Small Result: failure from cancelled selected lane
  Attempt 2 Route Rust Small: success, selected CPX42 with cpx42_idle
  Attempt 2 Rust Small on CPX42: success
  Attempt 2 Rust Small Result: success
  Rust Small on GitHub Hosted: skipped
  Post-merge CI Policy push run 26684617220: success

PR #580:
  CI Lane Whitelist: success
  Source of Truth: success
  GLR Invariants: success
  Route Rust Small: success
  Rust Small on CX33: success
  Rust Small Result: success
  Product Proof Result: success
  Rust Small on GitHub Hosted: skipped

PR #578:
  CI Lane Whitelist: success
  Source of Truth: success
  GLR Invariants: success
  Route Rust Small: success
  Product Proof Result: success
  Rust Small on CX43: success
  Rust Small Result: success
  Rust Small on GitHub Hosted: skipped

PR #577:
  CI Lane Whitelist: success
  Source of Truth: success
  GLR Invariants: success
  Rust Small on CX43: success
  Rust Small Result: success
  Product Proof Result: success
  Rust Small on GitHub Hosted: skipped
```

Claim boundary:

```text
This is a non-release standby restoration receipt.
It does not authorize a public promotion PR, release tag, cargo publish,
signing, Cargo-token work, or real crates.io install receipt.
```

## Current proof-refresh receipt

On 2026-05-29, the non-release `adze-toolkit-proof-refresh` lane refreshed the
current local proof receipts from `adze-swarm/main` after PR #551 selected the
lane:

```text
just ci-product-stable
result: passed

just ci-supported
result: passed on rerun with a longer local timeout after the first invocation
        timed out during cold-cache Windows supported-crate linking

just check-publishable
result: passed for adze-common, adze-ir, adze-glr-core, adze-tablegen,
        adze-macro, adze-tool, adze-cli, and adze
```

Claim boundary:

```text
These are non-publish proof receipts.
They do not authorize a release tag, cargo publish, signing, Cargo-token work,
or a crates.io install receipt.
The cargo install adze-cli claim remains incomplete until public release and a
real post-publish crates.io install verifier pass.
```

## Current release-candidate bundle receipt

On 2026-05-29, PR #560 closed the non-publish release-candidate bundle lane.
The selected `adze-swarm/main` state at standby restoration time is:

```text
135ae93c626d9af36a84d6f856c507b2ac931803
plans: close release candidate bundle lane (#560)
```

The public release surface remains intentionally separate:

```text
public adze/main: 6263c6a80046d13fb98e3ad319dfe726f32f1010
public drift: 386 files changed, 20612 insertions(+), 9816 deletions(-)
changed paths: 386
```

The release-candidate bundle lane recorded:

```text
just ci-supported
just ci-product-stable
just check-publishable
cargo run -q -p xtask -- verify-crates-io-install adze-cli --bin adze --version X.Y.Z --locked --dry-run
```

Claim boundary:

```text
These are non-publish release-candidate receipts.
They do not open a public promotion PR, tag a release, publish crates, change
signing or Cargo-token workflows, run a real crates.io install, or prove
cargo install adze-cli.
```

## Incomplete or weakly verified items

The overall endpoint must not be marked complete yet.

Remaining incomplete or blocked items:

- no explicit human release/publish authorization is recorded for tag, publish,
  signing, Cargo-token, or crates.io install-receipt work;
- `adze-cli` is not present in crates.io, so `cargo install adze-cli` must not
  be claimed;
- CX53 Rust Small stale-queue behavior remains a blocked runner investigation
  tracked by #598; the required Rust Small route no longer selects CX53 while
  that evidence is absent, and route selection no longer counts the route
  job's own runner as idle selected-lane capacity;
- this audit did not rerun the full product-surface proof matrix for typed CST,
  typed AST, diagnostics, ambiguity summaries, Tree-sitter-compatible output,
  query subset, JSON, CLI, and WASM projections;
- support-tier and README claim alignment are governed by existing ledgers, but
  this audit did not independently re-verify every row and claim.

## Decision

Do not mark the active thread goal complete.

The safe current state is:

```text
adze-swarm: paused forge standby after the completed adoption-hardening lane
public adze: release/public-intake surface only
release/publish/install: blocked pending explicit authorization and receipts
```

The next valid non-release action is to replace this paused standby state with
a focused active lane before opening implementation work. Public promotion or
release work remains blocked on explicit authorization tracked by #325.

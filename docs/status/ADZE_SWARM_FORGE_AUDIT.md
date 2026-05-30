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
also complete. The broader product/release endpoint is not complete because
release/publish authorization and a real crates.io `adze-cli` install receipt
are still absent.

## Prompt-to-artifact checklist

| Requirement | Evidence | Status |
| --- | --- | --- |
| Work starts from `adze-swarm/main`. | `AGENTS.md`; `docs/reference/adze-swarm-operating-model.md`; `active.toml` standby state. | Covered. |
| Public `adze` is not the implementation/productization/CI/docs-proof target. | `AGENTS.md`; `docs/reference/adze-swarm-operating-model.md`; `docs/reference/PUBLISH_CHECKLIST.md`; `active.toml` end state. | Covered. |
| One work item per branch and PR. | `docs/reference/SPEC_SYSTEM.md`; PR template queue/scope fields; `active.toml` standby handoff. | Covered by policy. |
| PRs link source-of-truth artifacts. | `docs/reference/SPEC_SYSTEM.md`; PR template source-of-truth fields; `docs/reference/adze-swarm-operating-model.md`. | Covered by policy. |
| PRs state claim boundary, proof, CI cost, and rollback. | `.github/pull_request_template.md`. | Covered by policy. |
| Default CI is self-hosted; no silent hosted fallback. | `docs/reference/CODEX_CI_EFFICIENCY_COMPATIBILITY.md`; `docs/reference/adze-swarm-operating-model.md`; PRs #539, #577, and #580; PRs #572-#580 check receipts where `Rust Small on GitHub Hosted` stayed skipped and routed self-hosted lanes passed. | Covered for current routing policy. |
| `Rust Small Result` remains the normalized base gate. | `AGENTS.md`; `docs/reference/adze-swarm-operating-model.md`; PR #579 and #580 checks: `Route Rust Small`, selected self-hosted Rust Small lane, and `Rust Small Result` succeeded. | Covered. |
| Heavy/advisory/coverage/benchmark/product/full-matrix lanes are scoped. | `docs/reference/CODEX_CI_EFFICIENCY_COMPATIBILITY.md`; PRs #572-#580 check summaries show broad implementation lanes skipped or cancelled for docs/status/CI-routing/verifier changes while required source-of-truth, product-proof, and Rust Small result checks stayed green. | Covered for observed recent PRs. |
| Public `adze` receives promotion only intentionally. | `docs/reference/PUBLISH_CHECKLIST.md`; `docs/reference/adze-swarm-operating-model.md`; `active.toml` release blocker. | Covered by policy; no current promotion PR. |
| `AdzeDocument` is the canonical parse product. | `docs/reference/adze-swarm-operating-model.md`; `docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md`; support-tier/product docs. | Covered by architecture docs. |
| Public views are projections over the document. | `docs/reference/adze-swarm-operating-model.md`; `ADZE-ADR-0001`; projection docs and support-tier rows. | Covered by architecture docs; not re-proven in this audit. |
| Stable claims require README/support-tier/proof/CI/examples/limitations alignment. | `docs/status/SUPPORT_TIERS.md`; `docs/status/PRODUCT_OBJECTIVE_AUDIT.md`; `docs/reference/adze-swarm-operating-model.md`. | Covered by policy; not exhaustively re-verified here. |
| `cargo install adze-cli` claim has a real crates.io receipt. | Live `cargo info --registry crates-io adze-cli` on 2026-05-30 returned not found. | Not complete. |
| No unsupported parity/performance claims. | `docs/reference/adze-swarm-operating-model.md`; `SUPPORT_TIERS.md`; `PRODUCT_OBJECTIVE_AUDIT.md`. | Covered by policy; not exhaustively re-verified here. |
| Near-term CI-efficiency rules landed. | PR #538 merged. | Complete. |
| Self-hosted-only routing landed and runner/tooling assumptions fixed. | PRs #539, #543, #545, #577, and #580 merged; #577 isolated Rust Small Cargo homes, #580 aligned CPX42 route labels, and hosted stayed skipped. | Complete for current routing. |
| Duplicate same-scope PRs collapsed. | PR #542 merged; duplicate document SRP PRs were not all merged. | Complete for the observed queue. |
| Current active goal complete/paused/superseded before new goal. | `adze-adoption-hardening` is complete and archived; `active.toml` is restored to the paused `adze-swarm-forge-standby` manifest; PR #579 made issue-tracked blocked items verifier-clean. | Complete. |
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

Current active manifest:

```text
.adze/goals/active.toml
  id = "adze-swarm-forge-standby"
  status = "paused"
```

Current live blockers:

```text
release-publish-authorization: blocked, tracked by #325
next-non-release-lane-selection: blocked, tracked by #549
```

Current live queue at audit time:

```text
EffortlessMetrics/adze-swarm open PRs: none
EffortlessMetrics/adze open PRs: none
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
verifier-clean while preserving #325 and #549 as live blockers.

The current selected `adze-swarm/main` state is:

```text
20aab0dc8d3e5f312fa899cbdefe1069ca3b3fa7
xtask: accept issue-tracked active-goal blockers (#579)
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
- no new active non-release lane has been selected after the completed and
  archived Adze Adoption Hardening lane;
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

The next valid non-release action is to select a focused lane from the paused
standby state tracked by #549. Public promotion or release work remains blocked
on explicit authorization tracked by #325.

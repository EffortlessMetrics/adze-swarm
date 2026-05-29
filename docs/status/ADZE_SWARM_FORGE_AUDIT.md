# Adze-swarm forge endpoint audit

Status: current audit
Owner: repo governance
Updated: 2026-05-29
Scope: `EffortlessMetrics/adze-swarm` development/proof forge endpoint
Completion: not complete

This audit maps the current forge endpoint requirements to concrete repo
evidence. It is not a release authorization, publish checklist, or crates.io
install receipt.

## Summary

`adze-swarm` is in a clean paused standby state for development and proof work.
The near-term CI governance and repo-boundary tasks are complete. The broader
product/release endpoint is not complete because release/publish authorization
and a real crates.io `adze-cli` install receipt are still absent.

## Prompt-to-artifact checklist

| Requirement | Evidence | Status |
| --- | --- | --- |
| Work starts from `adze-swarm/main`. | `AGENTS.md`; `docs/reference/adze-swarm-operating-model.md`; `active.toml` standby state. | Covered. |
| Public `adze` is not the implementation/productization/CI/docs-proof target. | `AGENTS.md`; `docs/reference/adze-swarm-operating-model.md`; `docs/reference/PUBLISH_CHECKLIST.md`; `active.toml` end state. | Covered. |
| One work item per branch and PR. | `docs/reference/SPEC_SYSTEM.md`; PR template queue/scope fields; `active.toml` standby handoff. | Covered by policy. |
| PRs link source-of-truth artifacts. | `docs/reference/SPEC_SYSTEM.md`; PR template source-of-truth fields; `docs/reference/adze-swarm-operating-model.md`. | Covered by policy. |
| PRs state claim boundary, proof, CI cost, and rollback. | `.github/pull_request_template.md`. | Covered by policy. |
| Default CI is self-hosted; no silent hosted fallback. | `docs/reference/CODEX_CI_EFFICIENCY_COMPATIBILITY.md`; `docs/reference/adze-swarm-operating-model.md`; PR #539; PR #547 rerun evidence where hosted stayed skipped and CX43 passed. | Covered for current routing policy. |
| `Rust Small Result` remains the normalized base gate. | `AGENTS.md`; `docs/reference/adze-swarm-operating-model.md`; PR #546 and #547 checks. | Covered. |
| Heavy/advisory/coverage/benchmark/product/full-matrix lanes are scoped. | `docs/reference/CODEX_CI_EFFICIENCY_COMPATIBILITY.md`; PR #546 and #547 check summaries show broad implementation lanes skipped for docs/status changes. | Covered for observed recent PRs. |
| Public `adze` receives promotion only intentionally. | `docs/reference/PUBLISH_CHECKLIST.md`; `docs/reference/adze-swarm-operating-model.md`; `active.toml` release blocker. | Covered by policy; no current promotion PR. |
| `AdzeDocument` is the canonical parse product. | `docs/reference/adze-swarm-operating-model.md`; `docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md`; support-tier/product docs. | Covered by architecture docs. |
| Public views are projections over the document. | `docs/reference/adze-swarm-operating-model.md`; `ADZE-ADR-0001`; projection docs and support-tier rows. | Covered by architecture docs; not re-proven in this audit. |
| Stable claims require README/support-tier/proof/CI/examples/limitations alignment. | `docs/status/SUPPORT_TIERS.md`; `docs/status/PRODUCT_OBJECTIVE_AUDIT.md`; `docs/reference/adze-swarm-operating-model.md`. | Covered by policy; not exhaustively re-verified here. |
| `cargo install adze-cli` claim has a real crates.io receipt. | Live `cargo info --registry crates-io adze-cli` on 2026-05-29 returned not found. | Not complete. |
| No unsupported parity/performance claims. | `docs/reference/adze-swarm-operating-model.md`; `SUPPORT_TIERS.md`; `PRODUCT_OBJECTIVE_AUDIT.md`. | Covered by policy; not exhaustively re-verified here. |
| Near-term CI-efficiency rules landed. | PR #538 merged. | Complete. |
| Self-hosted-only routing landed and runner/tooling assumptions fixed. | PRs #539, #543, #545 merged; PR #547 rerun selected CX43 and hosted skipped. | Complete for current routing. |
| Duplicate same-scope PRs collapsed. | PR #542 merged; duplicate document SRP PRs were not all merged. | Complete for the observed queue. |
| Current active goal complete/paused/superseded before new goal. | `active.toml` is now `status = "paused"` for `adze-swarm-forge-standby`. | Complete. |
| Public `adze` remains clean unless promotion/release. | Live `gh pr list` checks for public `adze` returned no open PRs on 2026-05-29. | Covered at audit time. |

## Current evidence snapshot

Recent merged PRs:

- #538: `docs: add hard Codex CI-efficiency compatibility invariants`
- #539: `ci: route swarm workflows to self-hosted runners only`
- #542: `Refactor document lifecycle primitives into SRP submodules`
- #545: `ci: harden ripr advisory on minimal runners`
- #546: `docs: record adze-swarm operating model`
- #547: `docs(goal): pause adze-swarm forge standby`

Current standby manifest:

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

Current crates.io install receipt state:

```text
cargo info --registry crates-io adze-cli
error: could not find `adze-cli`
```

## Incomplete or weakly verified items

The overall endpoint must not be marked complete yet.

Remaining incomplete or blocked items:

- no explicit human release/publish authorization is recorded for tag, publish,
  signing, Cargo-token, or crates.io install-receipt work;
- `adze-cli` is not present in crates.io, so `cargo install adze-cli` must not
  be claimed;
- this audit did not rerun the full product-surface proof matrix for typed CST,
  typed AST, diagnostics, ambiguity summaries, Tree-sitter-compatible output,
  query subset, JSON, CLI, and WASM projections;
- support-tier and README claim alignment are governed by existing ledgers, but
  this audit did not independently re-verify every row and claim.

## Decision

Do not mark the active thread goal complete.

The safe current state is:

```text
adze-swarm: paused development/proof forge standby
public adze: release/public-intake surface only
release/publish/install: blocked pending explicit authorization and receipts
```

The next valid action must be one of:

1. explicit human authorization for a public `adze` promotion/release path,
   tracked by #325; or
2. explicit selection of a new non-release `adze-swarm` active goal, tracked by
   #549.

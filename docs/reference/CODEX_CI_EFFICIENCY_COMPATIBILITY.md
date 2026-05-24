# Codex CI-Efficiency Compatibility Invariants

Status: active
Owner: CI policy
Created: 2026-05-24

This section is a **hard compatibility contract** for Codex-authored CI-efficiency
PRs in EffortlessMetrics repos.

> Do not optimize CI by blindly canceling active work or by routing metadata edits
> through Rust. Optimize by classifying changes correctly, keeping one active run,
> one pending replacement slot, and making default PR paths tiny.

## 1) Concurrency semantics (heavy/core workflows)

- Do **not** set `cancel-in-progress: true` for heavy/core PR workflows unless a
  repository explicitly documents that workflow as safe-to-cancel.
- Required queue model is **single active + single pending replacement slot**:
  - running job continues;
  - newest queued run replaces any older pending run;
  - active run is not terminated near completion.
- Canonical pattern:

```yaml
concurrency:
  group: ${{ github.workflow }}-${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: false
```

Rationale: canceling active compile/test runs wastes already-spent runner time,
hurts cache progression, and increases queue churn.

## 2) Change classification

Codex must not treat all changed files as Rust-input changes.

Default light/control-plane surfaces (unless mixed with real code changes):

- `docs/**`
- `*.md`
- `README*`, `CHANGELOG*`, `SECURITY*`, `CONTRIBUTING*`
- `policy/**`
- `plans/**`
- `badges/**`
- `AGENTS.md`
- `.github/CODEOWNERS`
- `.github/dependabot.yml`
- `.github/pull_request_template.md`
- `.github/PULL_REQUEST_TEMPLATE/**`
- `.codex/campaigns/**`
- `docs/tracking/**`
- `ci/hardware/**` receipts
- `.rails/**`
- `.uselesskey/**`

Special case:

- `.github/workflows/**` is **not** docs-light. Route workflow edits to minimal
  hosted workflow validation/safety, not full Rust CI by default.

## 3) Default PR routing policy

Classify first, then select the cheapest truthful lane:

- docs/control-plane-only -> no Rust compile.
- workflow-only -> hosted YAML/workflow validation, no full Rust.
- Rust/build/test touched -> routed Rust-small.
- hardware/GPU/receipt-only -> syntax/receipt validation only.
- unknown/mixed -> Rust-small (not full CI).
- full CI only by label/manual dispatch/main push/release/schedule/merge queue.

## 4) Hosted fallback policy

- Do not silently replace a self-hosted Rust-small lane with a full hosted Rust
  lane when runners are busy or unavailable.
- Fork PRs may use a tiny hosted safety lane.
- Runner token/readiness/idle issues must not auto-trigger long hosted fallback.
- Require explicit opt-in for expensive hosted fallback (labels/dispatch inputs),
  e.g. `full-ci`, `allow-github-hosted`, `ci-budget-ack`.

## 5) Artifact policy

- Do not upload receipts/JUnit/logs on default PR paths with `if: always()` unless
  branch protection explicitly requires it and artifacts are tiny.
- Prefer upload-on-failure with 3-7 day retention.
- Keep policy receipts minimal and avoid docs/control-plane artifact uploads.

## 6) Required tests for CI-only PRs

Every CI-efficiency PR must include evidence for:

- `git diff --check`
- YAML parse/validation for edited workflows
- classification dry-run or unit coverage for:
  - docs-only
  - `.rails/**`
  - `.uselesskey/**`
  - workflow-file-only
  - Rust-file change
  - mixed docs + Rust
- confirmation that heavy/core concurrency remains no-cancel semantics unless the
  PR intentionally and explicitly documents an exception.

## Reviewer gate (reject unless all true)

1. Heavy/core lanes preserve `cancel-in-progress: false`.
2. Metadata/control-plane-only edits avoid Rust CI.
3. Workflow edits are not routed through docs-light.
4. No silent expensive hosted fallback is introduced.
5. The change reduces actual billable work (not merely shifts cost).

## Explicit "do not" list

Do **not**:

- flip heavy/core Rust CI to `cancel-in-progress: true` as a generic optimization;
- classify `.rails/**`, `.uselesskey/**`, `.codex/campaigns/**`,
  `docs/tracking/**`, `policy/**`, or receipt-only edits as Rust source changes;
- treat workflow edits as docs-light;
- replace self-hosted Rust lanes with broad hosted equivalents on runner pressure;
- add broad hosted fallback under a `rust-small` name;
- add default-path always-upload artifacts unless required by merge policy;
- add matrix OS coverage to default PR CI;
- add deny/fuzz/mutants/docs/examples/release/BDD/GPU/hardware checks to default
  PR paths unless explicitly classified and budgeted.

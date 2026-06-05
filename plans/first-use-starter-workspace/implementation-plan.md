# First-Use Starter Workspace Hardening Plan

Status: active
Owner: cli/product
Created: 2026-06-05
Linked proposal: ../../docs/proposals/ADZE-PROP-0006-user-experience-hardening.md
Linked specs:
- ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
Linked ADRs:
- ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/first-use-starter-workspace-hardening.toml
Linked issues:
- EffortlessMetrics/adze-swarm#617
- EffortlessMetrics/adze-swarm#680
Support-tier impact: none
Policy impact: no workflow/router, hosted-fallback, release, publish, signing, Cargo-token, crates.io install, support-tier promotion, or public-repo implementation work

## Goal

Select #680 option A as the next bounded non-release implementation lane:
generated starter crates should include an empty `[workspace]` table so the
repo-built `adze init` first-use path remains buildable when the output is
nested under an existing Cargo workspace.

This lane addresses only the starter workspace robustness gap. The #680
diagnostic wording gap remains a separate later decision because it touches
parser diagnostic presentation rather than starter manifest generation.

## Operating Rules

- Work in `EffortlessMetrics/adze-swarm`.
- Do not open implementation, proof, docs-productization, CI, or CLI PRs in
  public `EffortlessMetrics/adze`.
- Do not tag, publish, sign, mutate Cargo-token surfaces, change release
  workflows, or claim crates.io install support in this lane.
- Do not edit workflow files, runner router logic, branch protection, or merge
  queue settings for this lane.
- Do not edit runtime/parser diagnostic presentation in this lane.
- Keep support-tier claims bounded by `docs/status/SUPPORT_TIERS.md`.
- Inspect open `adze-swarm` and public `adze` PR queues before opening
  duplicate work.

## Work Item: first-use-starter-source-of-truth

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0006-user-experience-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks:
- starter-workspace-escape
Blocked by: n/a

### Goal

Replace the paused post-visualization manifest with a focused non-release
first-use starter lane selected by #617 and #680.

### Production Delta

Docs and source-of-truth metadata only.

### Non-Goals

- No CLI implementation change.
- No runtime or parser behavior change.
- No diagnostic wording change.
- No workflow, runner routing, branch protection, or merge queue change.
- No support-tier promotion.
- No release, publish, signing, Cargo-token, crates.io install, or public
  `adze` work.

### Acceptance

- `.adze/goals/active.toml` names this campaign.
- `.adze/goals/first-use-starter-workspace-hardening.toml` exists.
- `policy/doc-artifacts.toml` registers the plan and named goal.
- #680 starter workspace robustness is the single ready implementation item.
- #680 diagnostic wording polish is deferred to a separate source-of-truth
  selection.
- #325 remains outside this lane as the release authorization blocker.

### Proof Commands

```bash
python -c "import tomllib; tomllib.load(open('.adze/goals/active.toml', 'rb')); tomllib.load(open('.adze/goals/first-use-starter-workspace-hardening.toml', 'rb')); tomllib.load(open('policy/doc-artifacts.toml', 'rb'))"
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
gh pr list --repo EffortlessMetrics/adze-swarm --state open --json number,title,isDraft,headRefName,mergeStateStatus,url
gh pr list --repo EffortlessMetrics/adze --state open --json number,title,isDraft,headRefName,mergeStateStatus,url
```

### Rollback

Revert the setup PR to restore the previous paused active manifest.

## Work Item: starter-workspace-escape

Status: ready
Linked proposal: ../../docs/proposals/ADZE-PROP-0006-user-experience-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
Linked ADR: n/a
Blocks: n/a
Blocked by:
- first-use-starter-source-of-truth

### Goal

Make generated `adze init` starter crates standalone Cargo workspace roots so
they build both outside a parent workspace and when generated beneath one.

### Production Delta

Expected implementation PR:

```text
cli/src/main.rs
cli/tests/cli_test.rs
```

The generated starter `Cargo.toml` should contain an empty `[workspace]` table.
The focused test should create a temporary parent Cargo workspace, generate a
starter beneath it, and prove the generated child passes `cargo test` or
`cargo check` when run from the child directory.

### Scope

Include:

```text
cli/src/main.rs
cli/tests/cli_test.rs
```

Exclude:

```text
runtime/*
tool/*
.github/workflows/*
runner router logic
release/publish/tag/signing/Cargo-token/crates.io work
public adze
support-tier promotion
diagnostic wording changes
```

### Acceptance

- Generated starter `Cargo.toml` includes an empty `[workspace]` table.
- Existing generated-starter tests still pass.
- A new focused test proves nested generation under a temp parent workspace
  builds from the generated child directory.
- The generated starter still parses the arithmetic example.
- The implementation PR links #617 and #680 and states claim boundary, proof
  commands, CI cost expectation, and rollback.
- No public `adze`, release, publish, signing, Cargo-token, or crates.io install
  claim is made.

### Proof Commands

```bash
cargo test -p adze-cli test_init_generated_cargo_toml_is_valid -- --exact --nocapture
cargo test -p adze-cli test_init_default_cwd_generates_buildable_project -- --exact --nocapture
cargo test -p adze-cli test_init_generates_buildable_project -- --exact --nocapture
cargo test -p adze-cli test_init_generated_project_under_parent_workspace_passes -- --exact --nocapture
cargo test -p adze-cli getting_started_quickstart_builds_parses_and_reports_diagnostics -- --exact --nocapture
git diff --check
```

### CI Cost Expectation

Small CLI-only implementation plus focused generated-starter tests. Expected
required PR gate remains `Rust Small Result`; no broad hosted fanout, workflow
change, coverage expansion, benchmark lane, or product-proof requirement is
selected by this lane.

### Rollback

Revert the focused implementation PR. The expected rollback surface is the
generated manifest stanza plus the one focused CLI test.

## Work Item: diagnostic-wording-polish

Status: blocked
Linked proposal: ../../docs/proposals/ADZE-PROP-0006-user-experience-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: n/a
Blocked by:
- separate source-of-truth selection after starter workspace robustness

### Goal

Keep the #680 bad-token wording gap visible without bundling parser diagnostic
presentation work into the starter manifest PR.

### Non-Goals

- No parser diagnostic change in this lane.
- No broad diagnostics-stability claim.
- No support-tier promotion.

### Candidate Proof Commands If Selected Later

```bash
cargo test -p adze --features pure-rust --test generated_parse_errors generated_typed_parser_bad_token_reports_source_span -- --exact --nocapture
cargo test -p adze --features pure-rust --test generated_parse_errors generated_typed_parser_error_contract_is_feature_stable -- --exact --nocapture
cargo test -p adze --features pure-rust --test typed_cst_generated_document generated_parse_document_diagnostics_byte_and_point_ranges_agree -- --exact --nocapture
cargo test -p adze-cli getting_started_quickstart_builds_parses_and_reports_diagnostics -- --exact --nocapture
```

### Rollback

Not applicable until a separate implementation lane is selected.

## Work Item: release-publish-authorization

Status: blocked
Linked proposal: ../../docs/proposals/ADZE-PROP-0005-release-promotion-readiness.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Blocked by: explicit human release authorization on EffortlessMetrics/adze-swarm#325

### Goal

Keep release, publish, signing, Cargo-token, public promotion, and crates.io
install-receipt work outside this lane.

### Proof Commands

```bash
cargo info --registry crates-io adze-cli
just check-publishable
cargo run -q -p xtask -- verify-crates-io-install adze-cli --bin adze --version <authorized-version> --locked
```

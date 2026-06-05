# First-Use Starter Workspace Hardening Plan

Status: paused
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

Close #680 diagnostic wording polish after the starter workspace robustness
slice completed through EffortlessMetrics/adze-swarm#682 and #683 and the
diagnostic wording slice completed through EffortlessMetrics/adze-swarm#686.

This lane addresses only the bad-token wording gap recorded on #680: for a
non-EOF invalid source byte such as `1 + @`, the diagnostic should not point at
the offending byte while rendering the found token as `end`. It must preserve
true EOF wording for `1 +`, expected-token text, byte/point spans, UTF-8 and
multiline behavior, and existing document diagnostic range agreement.

No non-release implementation item remains active or ready in this campaign.
Return to #617 for the next decision packet before opening more implementation
work.

## Operating Rules

- Work in `EffortlessMetrics/adze-swarm`.
- Do not open implementation, proof, docs-productization, CI, or CLI PRs in
  public `EffortlessMetrics/adze`.
- Do not tag, publish, sign, mutate Cargo-token surfaces, change release
  workflows, or claim crates.io install support in this lane.
- Do not edit workflow files, runner router logic, branch protection, or merge
  queue settings for this lane.
- Do not edit runtime/parser diagnostic presentation outside the selected
  non-EOF invalid-token wording rule.
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
- #680 starter workspace robustness is complete through #682 and #683.
- #680 diagnostic wording polish is the single ready implementation item.
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

Status: complete
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

Merged by EffortlessMetrics/adze-swarm#682 as one focused CLI implementation
diff:

```text
cli/src/main.rs
cli/tests/cli_test.rs
```

The generated starter `Cargo.toml` contains an empty `[workspace]` table. The
focused test creates a temporary parent Cargo workspace, generates a starter
beneath it, and proves the generated child passes `cargo test --quiet` when run
from the child directory.

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

### Completion Receipts

- Implementation PR: EffortlessMetrics/adze-swarm#682.
- Merge commit: `3899647e63e9362871987297d4b1733281ba92bb`.
- Required gates before merge: `Rust Small Result` and `Product Proof Result`
  passed.
- Focused local proof: targeted formatting, generated Cargo.toml shape test,
  nested parent-workspace generated-starter test, default-cwd build canary,
  generated-project test/parse canary, getting-started quickstart canary,
  active-goal check, and `git diff --check` passed.
- Broad `cargo fmt --all` was unavailable on Windows with `os error 206`, so
  formatting was applied through `cargo fmt -p adze-cli` and targeted
  `rustfmt`.

### Rollback

Revert the focused implementation PR. The expected rollback surface is the
generated manifest stanza plus the one focused CLI test.

## Work Item: diagnostic-wording-polish

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0006-user-experience-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: n/a
Blocked by: n/a

### Goal

Make generated-parser bad-token presentation less misleading for non-EOF
invalid source bytes without broadening the diagnostics support tier.

When bad input points at an offending byte such as `@`, public diagnostic
formatting should not render that byte-span diagnostic as `unexpected token
"end"`. True EOF diagnostics, such as `1 +`, should keep EOF/end wording and
zero-width EOF span behavior.

### Production Delta

Merged by EffortlessMetrics/adze-swarm#686 as one focused runtime/diagnostic
presentation diff:

```text
runtime/src/__private.rs
runtime/tests/generated_parse_errors.rs
runtime/tests/typed_cst_generated_document.rs
cli/tests/cli_test.rs
```

The pure-Rust parser diagnostic conversion now maps an EOF symbol reported
before the real end of input to the offending source scalar, or to an explicit
byte phrase for invalid UTF-8 boundaries. True EOF still uses the language
EOF/end symbol. Document diagnostics and CLI JSON projections remain
projections of the same parser diagnostic facts.

### Non-Goals

- No broad diagnostics-stability claim.
- No support-tier promotion.
- No change to expected-token text.
- No change to byte spans, point ranges, UTF-8/multiline behavior, or document
  diagnostic range agreement.
- No starter manifest, workflow/router, public `adze`, release, publish,
  signing, Cargo-token, or crates.io install work.

### Acceptance

- `1 + @` does not render the byte-span `4..5` diagnostic as `unexpected token
  "end"`.
- The non-EOF invalid byte is rendered as `@`, `invalid token`, or an
  equivalent offending-source-byte phrase.
- `1 +` keeps EOF/end wording and zero-width EOF span behavior.
- Existing expected-token text remains present.
- Existing byte/point span and document diagnostic range contracts remain
  covered by focused tests.
- The implementation PR links #617 and #680 and states claim boundary, proof
  commands, CI cost expectation, and rollback.

### Proof Commands

```bash
cargo test -p adze --features pure-rust --test generated_parse_errors generated_typed_parser_bad_token_reports_source_span -- --exact --nocapture
cargo test -p adze --features pure-rust --test generated_parse_errors generated_typed_parser_error_contract_is_feature_stable -- --exact --nocapture
cargo test -p adze --features pure-rust --test typed_cst_generated_document generated_parse_document_diagnostics_byte_and_point_ranges_agree -- --exact --nocapture
cargo test -p adze-cli getting_started_quickstart_builds_parses_and_reports_diagnostics -- --exact --nocapture
git diff --check
```

### CI Cost Expectation

Small runtime/diagnostic presentation change plus focused tests. Expected
required PR gate remains `Rust Small Result`; no broad hosted fanout, workflow
change, coverage expansion, benchmark lane, release lane, or support-tier
promotion is selected by this lane.

### Completion Receipts

- Implementation PR: EffortlessMetrics/adze-swarm#686.
- Merge commit: `d6aee95c9cad59dbbd593d6f53f0b8cd8c6648ee`.
- Required gates before merge: `Rust Small Result` and `Product Proof Result`
  passed.
- PR Gate before merge: `PR Plan / PR Plan`, `Supported Rust Gate`, and
  `PR Gate Success` passed on run `26997407560`.
- Additional source-relevant hosted receipts: `Pure Rust Implementation CI`
  passed, including `Test Pure Rust Implementation (self-hosted-linux, stable)`.
- Known non-source CI red: `Runner Capacity / Fallback Policy` remained the
  expected no-idle/no-default-fallback control-plane signal, and
  `Microcrate CI / Test Runtime Crates` was classified on #686 as runner/job
  state evidence rather than a diagnostic source failure.
- Focused local proof included the named #680 diagnostic commands, CLI JSON
  diagnostics projection, UTF-8 and multiline document diagnostic canaries, the
  #686-only bad-token document message canary, touched-file rustfmt,
  active-goal check, and `git diff --check`.
- Broad `cargo fmt --all` and `cargo fmt -p adze -p adze-cli` were unavailable
  on Windows with `os error 206`, so touched files were formatted directly with
  `rustfmt`.

### Rollback

Revert the focused implementation PR. The expected rollback surface is the
diagnostic presentation change plus its focused tests.

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

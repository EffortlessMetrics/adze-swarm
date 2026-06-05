# Visualization No-Panic Burn-Down Plan

Status: complete
Owner: tooling/policy
Created: 2026-06-05
Linked proposal: ../../docs/proposals/ADZE-PROP-0012-parser-runtime-maintainability-hardening.md
Linked policy:
- ../../docs/NO_PANIC_POLICY.md
Linked ADRs:
- ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/visualization-no-panic-burn-down.toml
Linked issues:
- EffortlessMetrics/adze-swarm#617
- EffortlessMetrics/adze-swarm#661
Support-tier impact: none
Policy impact: no no-panic allowlist edit, checker-mode promotion, CI routing change, release, publish, signing, Cargo-token, crates.io install, or public-repo implementation work

## Goal

Select #661 as the next bounded non-release implementation lane from the
research board. The lane removes `write!` / `writeln!` unwraps into an
in-memory `String` from `tool/src/visualization.rs` only while preserving the
existing visualization outputs.

## Operating Rules

- Work in `EffortlessMetrics/adze-swarm`.
- Do not open implementation, proof, docs-productization, CI, policy, or
  visualization PRs in public `EffortlessMetrics/adze`.
- Do not tag, publish, sign, mutate Cargo-token surfaces, change release
  workflows, or claim crates.io install support in this lane.
- Do not edit `policy/no-panic-allowlist.toml`, `policy/clippy-lints.toml`, or
  CI routing policy for this lane.
- Do not promote the no-panic checker from advisory to blocking.
- Keep support-tier claims bounded by `docs/status/SUPPORT_TIERS.md`.
- Inspect open `adze-swarm` and public `adze` PR queues before opening
  duplicate work.

## Work Item: visualization-no-panic-source-of-truth

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0012-parser-runtime-maintainability-hardening.md
Linked policy: ../../docs/NO_PANIC_POLICY.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks:
- visualization-string-write-unwraps
Blocked by: n/a

### Goal

Replace the paused forge-standby manifest with a focused non-release policy
burn-down lane selected by #617 and #661.

### Production Delta

Docs and source-of-truth metadata only.

### Non-Goals

- No runtime behavior change.
- No visualization implementation change.
- No no-panic allowlist or clippy policy edit.
- No checker-mode promotion.
- No support-tier promotion.
- No release, publish, signing, Cargo-token, crates.io install, or public
  `adze` work.

### Acceptance

- `.adze/goals/active.toml` names this campaign.
- `.adze/goals/visualization-no-panic-burn-down.toml` exists.
- `policy/doc-artifacts.toml` registers the plan and named goal.
- #661 is the single ready implementation item.
- #325 remains outside this lane as the release authorization blocker.

### Proof Commands

```bash
python -c "import tomllib; tomllib.load(open('.adze/goals/active.toml', 'rb')); tomllib.load(open('.adze/goals/visualization-no-panic-burn-down.toml', 'rb')); tomllib.load(open('policy/doc-artifacts.toml', 'rb'))"
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
gh pr list --repo EffortlessMetrics/adze-swarm --state open --json number,title,isDraft,headRefName,mergeStateStatus,url
gh pr list --repo EffortlessMetrics/adze --state open --json number,title,isDraft,headRefName,mergeStateStatus,url
```

### Rollback

Revert the setup PR to restore the previous paused forge-standby manifest.

## Work Item: visualization-string-write-unwraps

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0012-parser-runtime-maintainability-hardening.md
Linked policy: ../../docs/NO_PANIC_POLICY.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: n/a
Blocked by:
- visualization-no-panic-source-of-truth

### Goal

Remove the no-panic findings in `tool/src/visualization.rs` that come from
`write!` and `writeln!` calls into an in-memory `String`.

### Production Delta

Merged by EffortlessMetrics/adze-swarm#678 as one focused tool implementation
diff in `tool/src/visualization.rs` only. The change preserves DOT, SVG, text
summary, and dependency output behavior.

### Scope

Include:

```text
tool/src/visualization.rs only
```

Exclude:

```text
runtime/*
tool/src/grammar_js/*
tool/src/scanner_build.rs
policy/no-panic-allowlist.toml
policy/clippy-lints.toml
checker-mode promotion
public adze
release/publish/tag/signing/Cargo-token/crates.io work
CI routing changes
support-tier or benchmark claims
```

### Acceptance

- The `tool/src/visualization.rs` `write!` / `writeln!` unwrap findings were
  removed.
- Visualization output behavior was preserved.
- No policy exceptions were added.
- No checker-mode promotion happened.
- Public `adze` was untouched.
- The implementation PR linked #661 and #617 and stated claim boundary, proof
  commands, CI cost expectation, and rollback.

### Proof Commands

```bash
cargo test -p adze-tool --lib visualization -- --nocapture
cargo test -p adze-tool --test visualization_comprehensive -- --nocapture
cargo test -p adze-tool --test visualization_comprehensive_v2 -- --nocapture
cargo test -p adze-tool --test build_pipeline_comprehensive visualizer_to_dot_produces_digraph -- --exact --nocapture
cargo run -q -p xtask -- check-no-panic-family --mode advisory
git diff --check
```

### Completion Receipts

- Implementation PR: EffortlessMetrics/adze-swarm#678.
- Merge commit: `8da166eede0f6bccc86ea738aec355171f2ae6b1`.
- Required gate: PR Gate `Supported Rust Gate` and `PR Gate Success` passed.
- Focused local proof: visualization lib tests, comprehensive v1/v2 tests,
  build-pipeline DOT canary, advisory no-panic report, targeted fmt, targeted
  clippy, and active-goal check passed.
- The generated no-panic report had no `tool/src/visualization.rs` entries
  after the implementation.

### CI Cost Expectation

Small tool-only implementation plus advisory no-panic check. No broad CI
routing change. Expected required PR gate remains `Rust Small Result`; Product
Proof should not be required unless path routing selects it.

### Rollback

Revert the focused implementation PR. The expected rollback surface is
`tool/src/visualization.rs` only.

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

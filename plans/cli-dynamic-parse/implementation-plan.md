# CLI Dynamic Parse Boundary Hardening Plan

Status: active
Owner: cli/product
Created: 2026-05-21
Linked proposal: ../../docs/proposals/ADZE-PROP-0016-cli-dynamic-parse-boundary.md
Linked specs:
- ../../docs/specs/ADZE-SPEC-0008-json-cli-wasm-projections.md
- ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADRs:
- ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
- ../../docs/adr/ADZE-ADR-0004-schema-versioned-projections.md
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/cli-dynamic-parse-boundary.toml
Closeout: ./closeout.md
Support-tier impact: dynamic CLI parse output remains below Stabilizing until behavior receipts exist
Policy impact: no release, publish, signing, Cargo-token, branch-protection, or public-promotion change

## Goal

Make the feature-gated `adze parse --dynamic` surface safe and unsurprising by
proving its current boundary, correcting recipe-like documentation, and keeping
release/install/schema claims bounded.

## Operating Rules

- Work in `EffortlessMetrics/adze-swarm`.
- Do not open CLI implementation PRs in public `EffortlessMetrics/adze`.
- Do not tag, publish, sign, mutate Cargo-token surfaces, change release
  workflows, or claim crates.io install support in this lane.
- Keep support-tier claims bounded by `docs/status/SUPPORT_TIERS.md`.
- Do not claim dynamic parse output is implemented until a behavior PR proves
  it.
- Inspect open `adze-swarm` PRs before opening duplicate work.

## Work Item: cli-dynamic-parse-source-of-truth

Status: complete
Completed by: EffortlessMetrics/adze-swarm#471
Linked proposal: ../../docs/proposals/ADZE-PROP-0016-cli-dynamic-parse-boundary.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks:
- dynamic-cli-boundary-receipts
Blocked by: n/a

### Goal

Replace the completed CLI static JSON/DOT manifest with a focused non-release
lane for dynamic parse boundary hardening.

### Production Delta

Docs and source-of-truth metadata only.

### Acceptance

- `.adze/goals/active.toml` names the CLI dynamic parse boundary campaign.
- `.adze/goals/cli-dynamic-parse-boundary.toml` exists.
- `policy/doc-artifacts.toml` registers the proposal, plan, and goal.
- `plans/README.md` lists the lane.
- Release blocker tracker #325 remains the release/publish authorization
  checkpoint.
- Completed by #471.

### Proof Commands

```bash
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Revert the setup PR to restore the completed CLI static JSON/DOT active
manifest.

## Work Item: dynamic-cli-boundary-receipts

Status: complete
Completed by: EffortlessMetrics/adze-swarm#472
Linked proposal: ../../docs/proposals/ADZE-PROP-0016-cli-dynamic-parse-boundary.md
Linked spec: ../../docs/specs/ADZE-SPEC-0008-json-cli-wasm-projections.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks:
- cli-dynamic-parse-closeout
Blocked by:
- cli-dynamic-parse-source-of-truth

### Goal

Add executable receipts and docs/support-tier wording that prove dynamic parse
is experimental, feature-gated, and not currently a supported output path.

### Production Delta

CLI tests, boundary messages, dynamic-loading guide wording, and support-tier
wording only.

### Non-Goals

- No full dynamic parse output implementation.
- No stable CLI/WASM schema claim.
- No Tree-sitter dynamic parser parity claim.
- No release/install claim.
- No dependency on a system Tree-sitter grammar library.

### Acceptance

- Building `adze-cli` without `dynamic` and running `adze parse --dynamic`
  reports that the feature is required.
- Building `adze-cli` with `dynamic` and pointing to a missing grammar reports
  a bounded missing-library error.
- Dynamic parse output remains clearly unimplemented after a successful load
  boundary; tests cover helper behavior without requiring a system grammar
  library when possible.
- `book/src/guide/dynamic-loading.md` is visibly a design sketch and no longer
  reads like a supported workflow recipe.
- `docs/status/SUPPORT_TIERS.md` keeps dynamic parse output outside supported
  CLI claims.
- Completed by #472.

### Proof Commands

```bash
cargo test -p adze-cli test_parse_dynamic_without_feature_reports_feature_gate -- --exact --nocapture
cargo test -p adze-cli --features dynamic dynamic -- --nocapture
cargo fmt -p adze-cli -- --check
cargo clippy -p adze-cli --all-targets --features dynamic -- -D warnings
git diff --check
```

### Rollback

Revert the behavior PR. Dynamic parse remains feature-gated and unimplemented,
with the previous documentation boundary restored.

## Work Item: cli-dynamic-parse-closeout

Status: ready
Linked proposal: ../../docs/proposals/ADZE-PROP-0016-cli-dynamic-parse-boundary.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: n/a
Blocked by:
- dynamic-cli-boundary-receipts

### Goal

Close the lane after dynamic parse boundary receipts land and support-tier
language matches the proved surface.

### Production Delta

Source-of-truth closeout only when behavior receipts exist.

### Proof Commands

```bash
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Revert the closeout PR if it overstates behavior or support-tier status.

# Query and Tooling Expansion Plan

Status: active
Owner: runtime/tooling
Created: 2026-05-20
Linked proposal: ../../docs/proposals/ADZE-PROP-0008-query-tooling-expansion.md
Linked specs:
- ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
- ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
- ../../docs/specs/ADZE-SPEC-0013-query-compatibility.md
Linked ADRs:
- ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/query-tooling-expansion.toml
Support-tier impact: no promotion by campaign setup
Policy impact: no release, publish, signing, Cargo-token, or public-repo implementation work

## Goal

Continue non-release product work from a clean `adze-swarm` active goal by
making query and tooling behavior easier to exercise, understand, and prove.

## Operating Rules

- Work in `EffortlessMetrics/adze-swarm`.
- Do not open implementation, proof, docs-productization, or CI PRs in public
  `EffortlessMetrics/adze`.
- Do not tag, publish, sign, mutate Cargo-token surfaces, or change release
  workflows in this lane.
- Keep public `adze` as release/public-intake/publish surface.
- Keep query compatibility bounded by `ADZE-SPEC-0013`.
- Keep `AdzeDocument` as the one parse truth for tooling projections.
- Use `Rust Small Result` as the GitHub gate.
- Inspect open `adze-swarm` PRs before opening duplicate work.

## Work Item: query-tooling-source-of-truth

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0008-query-tooling-expansion.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks:
- query-example-cli-smoke
- query-gap-matrix-receipts
- support-tier-boundary-refresh
Blocked by: n/a

### Goal

Replace the paused no-active-lane manifest with a focused non-release query and
tooling expansion lane.

### Production Delta

Docs and source-of-truth metadata only.

### Non-Goals

- No runtime behavior change.
- No query parity claim.
- No release/publish authorization.
- No support-tier promotion.

### Acceptance

- `.adze/goals/active.toml` names this campaign.
- `.adze/goals/query-tooling-expansion.toml` exists.
- `policy/doc-artifacts.toml` registers the proposal, plan, and named goal.
- Release blocker tracker #325 remains outside this lane.

### Proof Commands

```bash
python -c "import tomllib; tomllib.load(open('.adze/goals/active.toml', 'rb')); tomllib.load(open('.adze/goals/query-tooling-expansion.toml', 'rb')); tomllib.load(open('policy/doc-artifacts.toml', 'rb'))"
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Revert the setup PR to restore the previous paused no-active-lane manifest.

## Work Item: query-example-cli-smoke

Status: ready
Linked proposal: ../../docs/proposals/ADZE-PROP-0008-query-tooling-expansion.md
Linked spec: ../../docs/specs/ADZE-SPEC-0013-query-compatibility.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: support-tier-boundary-refresh
Blocked by: query-tooling-source-of-truth

### Goal

Refresh runnable query examples and CLI/tooling smoke receipts for the currently
supported query subset.

### Production Delta

Expected future PRs may touch query examples, CLI smoke tests, or docs that
teach the supported query subset.

### Non-Goals

- No full query parity claim.
- No stable CLI schema claim.
- No crates.io install claim.

### Acceptance

- Runnable examples cover the supported subset they document.
- Source-aware and source-free behavior remains explicit.
- Any CLI smoke stays advisory unless support tiers promote it later.

### Proof Commands

```bash
cargo test -p adze --features query --lib query -- --nocapture
cargo run -p adze --features query --example query_highlighting
git diff --check
```

### Rollback

Revert the focused example or CLI smoke PR.

## Work Item: query-gap-matrix-receipts

Status: ready
Linked proposal: ../../docs/proposals/ADZE-PROP-0008-query-tooling-expansion.md
Linked spec: ../../docs/specs/ADZE-SPEC-0013-query-compatibility.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: support-tier-boundary-refresh
Blocked by: query-tooling-source-of-truth

### Goal

Make supported query behavior and known gaps easier to verify from fixtures.

### Production Delta

Expected future PRs may add focused query fixtures, differential receipts for
the supported subset, or known-gap documentation.

### Non-Goals

- No unsupported alternation/directive parity claim.
- No GLR forest query matching claim.

### Acceptance

- Supported features are backed by canaries or examples.
- Unsupported features are recorded as explicit gaps rather than hidden failures.

### Proof Commands

```bash
cargo test -p adze --features "pure-rust,ts-compat,query" --test query_differential -- --nocapture
cargo test -p adze --features query --lib query -- --nocapture
git diff --check
```

### Rollback

Revert the focused fixture or docs PR.

## Work Item: support-tier-boundary-refresh

Status: blocked
Linked proposal: ../../docs/proposals/ADZE-PROP-0008-query-tooling-expansion.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: n/a
Blocked by:
- query-example-cli-smoke
- query-gap-matrix-receipts

### Goal

Refresh support-tier or product-audit wording only after new query/tooling proof
exists.

### Production Delta

Expected future PRs may update `docs/status/SUPPORT_TIERS.md` or
`docs/status/PRODUCT_OBJECTIVE_AUDIT.md`.

### Non-Goals

- No support-tier promotion unless proof and limitations justify it.
- No README Stable claim change by setup.

### Acceptance

- New proof commands are named in the relevant support/audit rows.
- Any remaining query/tooling limitations are explicit.

### Proof Commands

```bash
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
cargo run -q -p xtask -- check-active-goal --mode blocking
git diff --check
```

### Rollback

Revert the status refresh PR.

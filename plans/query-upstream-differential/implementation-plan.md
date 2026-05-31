# Query Upstream Differential Plan

Status: active
Owner: runtime/query
Created: 2026-05-31
Linked proposal: ../../docs/proposals/ADZE-PROP-0008-query-tooling-expansion.md
Linked specs:
- ../../docs/specs/ADZE-SPEC-0013-query-compatibility.md
Linked ADRs:
- ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/query-upstream-differential.toml
Linked issues:
- EffortlessMetrics/adze-swarm#617
- EffortlessMetrics/adze-swarm#643
Support-tier impact: no promotion by setup
Policy impact: no release, publish, signing, Cargo-token, crates.io install, hosted fallback, or public-repo implementation work

## Goal

Add the first upstream Tree-sitter query differential canary for the documented
Adze query subset. The lane answers one research-board question from #643: can
one explicit upstream grammar/input/query slice compare upstream Tree-sitter
captures with Adze captures without broadening the query compatibility claim?

## Operating Rules

- Work in `EffortlessMetrics/adze-swarm`.
- Do not open implementation, proof, docs-productization, CI, or query PRs in
  public `EffortlessMetrics/adze`.
- Do not tag, publish, sign, mutate Cargo-token surfaces, or change release
  workflows in this lane.
- Keep query compatibility bounded by `ADZE-SPEC-0013`.
- Keep `AdzeDocument` as the native parse truth; Tree-sitter compatibility and
  query matching remain projections over selected-tree facts.
- Use `Rust Small Result` as the GitHub gate.
- Inspect open `adze-swarm` and public `adze` PR queues before opening work.

## Work Item: query-upstream-differential-source-of-truth

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0008-query-tooling-expansion.md
Linked spec: ../../docs/specs/ADZE-SPEC-0013-query-compatibility.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks:
- upstream-query-differential-canary
Blocked by: n/a

### Goal

Replace the paused forge-standby manifest with a focused non-release query
proof-spike lane selected by #617 and #643.

### Production Delta

Docs and source-of-truth metadata only.

### Non-Goals

- No runtime behavior change.
- No query parity claim.
- No support-tier promotion.
- No release, publish, signing, Cargo-token, crates.io install, or public `adze`
  work.

### Acceptance

- `.adze/goals/active.toml` names this campaign.
- `.adze/goals/query-upstream-differential.toml` exists.
- `policy/doc-artifacts.toml` registers the plan and named goal.
- #643 is the single ready implementation/proof item.
- #325 remains outside this lane as the release authorization blocker.

### Proof Commands

```bash
python -c "import tomllib; tomllib.load(open('.adze/goals/active.toml', 'rb')); tomllib.load(open('.adze/goals/query-upstream-differential.toml', 'rb')); tomllib.load(open('policy/doc-artifacts.toml', 'rb'))"
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Revert the setup PR to restore the previous paused forge-standby manifest.

## Work Item: upstream-query-differential-canary

Status: ready
Linked proposal: ../../docs/proposals/ADZE-PROP-0008-query-tooling-expansion.md
Linked spec: ../../docs/specs/ADZE-SPEC-0013-query-compatibility.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: n/a
Blocked by: query-upstream-differential-source-of-truth

### Goal

Add one advisory upstream differential canary for a supported-subset query
shape. Prefer an existing optional upstream grammar dependency such as
`tree-sitter-json` through the `with-grammars` feature.

### Production Delta

Expected future PR adds one focused test target or fixture and updates only the
minimal proof surface needed to name the receipt.

### Non-Goals

- No full Tree-sitter query parity claim.
- No broad query parser or matcher rewrite.
- No directive, highlighting, or injection semantics claim.
- No GLR-forest-wide query matching claim.
- No support-tier promotion unless a later PR separately aligns proof and
  limitations.

### Acceptance

- The canary names the upstream grammar and crate version, input, query pattern,
  expected upstream captures, and Adze captures.
- The query shape is already inside the supported subset in `ADZE-SPEC-0013`.
- The test records whether it is a normal test target, feature-gated
  `with-grammars`, or another explicit advisory lane.
- The PR states the final proof command and why it is scoped.

### Proof Commands

```bash
cargo test -p adze --features "pure-rust,ts-compat,query,with-grammars" --test upstream_query_differential -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat,query" --test query_differential -- --nocapture
git diff --check
```

### Rollback

Revert the focused canary or fixture PR. The previous local fixture-only query
compatibility receipt remains unchanged.

# Parser Runtime Maintainability Hardening Plan

Status: active
Owner: runtime/product
Created: 2026-05-21
Linked proposal: ../../docs/proposals/ADZE-PROP-0012-parser-runtime-maintainability-hardening.md
Linked specs:
- ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
- ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
Linked ADRs:
- ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/parser-runtime-maintainability-hardening.toml
Support-tier impact: no promotion by campaign setup
Policy impact: no release, publish, signing, Cargo-token, branch-protection, or public-promotion change

## Goal

Continue from a clean `adze-swarm` state with a non-release maintenance lane.
The lane owns small, proof-backed parser/runtime/tablegen hardening after the
product-proof closeout while release and crates.io install work remains blocked
on explicit authorization.

## Operating Rules

- Work in `EffortlessMetrics/adze-swarm`.
- Do not open implementation, CI, examples, docs-productization, or proof PRs
  in public `EffortlessMetrics/adze`.
- Do not tag, publish, sign, mutate Cargo-token surfaces, change release
  workflows, or claim crates.io install support in this lane.
- Keep public `adze` as release/public-intake/publish surface.
- Keep support-tier claims bounded by `docs/status/SUPPORT_TIERS.md`.
- Use `Rust Small Result` and `Product Proof Result` as GitHub gates.
- Inspect open `adze-swarm` PRs before opening duplicate work.

## Work Item: parser-runtime-maintainability-source-of-truth

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0012-parser-runtime-maintainability-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks:
- supported-surface-maintainability-audit
- focused-runtime-hardening-prs
- maintainability-closeout-and-release-boundary-refresh
Blocked by: n/a

### Goal

Replace the paused release-boundary active manifest with a non-release
maintainability goal so agents have a current lane without touching release
machinery.

### Production Delta

Docs and source-of-truth metadata only.

### Non-Goals

- No runtime behavior change.
- No release/publish authorization.
- No crates.io install claim.
- No support-tier promotion.
- No branch-protection change.

### Acceptance

- `.adze/goals/active.toml` names the parser/runtime maintainability campaign.
- `.adze/goals/parser-runtime-maintainability-hardening.toml` exists.
- `policy/doc-artifacts.toml` registers the proposal, plan, and named goal.
- Release blocker tracker #325 remains the release/publish authorization
  checkpoint.
- Completed by #443.

### Proof Commands

```bash
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Revert the setup PR to restore the previous paused release-boundary active
manifest.

## Work Item: supported-surface-maintainability-audit

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0012-parser-runtime-maintainability-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks:
- focused-runtime-hardening-prs
Blocked by:
- parser-runtime-maintainability-source-of-truth

### Goal

Collect a short maintainability audit of supported parser/runtime/tablegen
surfaces and choose only PR-sized follow-ups that have a proof or product
reason.

### Production Delta

Audit notes or targeted issue comments only; no runtime behavior change unless
a follow-up PR is deliberately split out.

### Non-Goals

- No broad rewrite queue.
- No source churn for style alone.
- No public `adze` work.

### Acceptance

- The audit names at most a small queue of focused follow-up candidates.
- Each candidate names the edited surface, reason, expected diff shape, and
  proof command.
- Release/publish tasks remain out of scope.
- Audit recorded in
  [`supported-surface-audit.md`](./supported-surface-audit.md).

### Proof Commands

```bash
just ci-product-stable
CARGO_PROFILE_TEST_DEBUG=0 just ci-supported
git diff --check
```

### Rollback

Close or supersede the audit note if the chosen candidates prove noisy or
irrelevant.

## Work Item: focused-runtime-hardening-prs

Status: active
Linked proposal: ../../docs/proposals/ADZE-PROP-0012-parser-runtime-maintainability-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks:
- maintainability-closeout-and-release-boundary-refresh
Blocked by:
- supported-surface-maintainability-audit

### Goal

Land small parser/runtime/tablegen hardening PRs selected from the audit.

### Production Delta

Focused code or test changes only when they improve maintainability of a
supported parser/runtime surface or make an existing proof more reliable.

### Non-Goals

- No claim promotion without support-tier updates.
- No unbounded SRP refactor wave.
- No release machinery work.

### Acceptance

- Each PR has one reason and one edited surface family.
- Each PR includes focused proof for the touched surface.
- Aggregate gates remain `Rust Small Result` and `Product Proof Result`.
- Initial focused tablegen hardening landed in #444, #445, and #446.

### Proof Commands

```bash
cargo test -p adze --features pure-rust
cargo test -p adze-tablegen --all-features
git diff --check
```

### Rollback

Revert the focused PR that introduced the regression; do not roll back unrelated
maintainability work.

## Work Item: maintainability-closeout-and-release-boundary-refresh

Status: blocked
Linked proposal: ../../docs/proposals/ADZE-PROP-0012-parser-runtime-maintainability-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: n/a
Blocked by:
- focused-runtime-hardening-prs

### Goal

Close or pause this lane only after the focused maintainability queue is empty
or superseded, then refresh release-boundary state if there is a material new
fact.

### Production Delta

Source-of-truth closeout only.

### Non-Goals

- No release/publish execution without explicit authorization.
- No crates.io install claim.

### Acceptance

- Completed follow-up PRs are listed in the active/named goal manifest.
- Any remaining work is explicitly superseded, paused, or moved into a new
  source-of-truth lane.
- Release-only work remains tracked on #325.

### Proof Commands

```bash
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Reopen or supersede the closeout if a needed maintainability item was closed
without proof.

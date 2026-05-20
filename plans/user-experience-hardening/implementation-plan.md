# User Experience Hardening Plan

Status: active
Owner: runtime/product
Created: 2026-05-20
Linked proposal: ../../docs/proposals/ADZE-PROP-0006-user-experience-hardening.md
Linked specs:
- ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
- ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
- ../../docs/specs/ADZE-SPEC-0013-query-compatibility.md
- ../../docs/specs/ADZE-SPEC-0014-performance-and-regression.md
Linked ADRs:
- ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
- ../../docs/adr/ADZE-ADR-0003-summary-first-glr-ambiguity.md
- ../../docs/adr/ADZE-ADR-0004-schema-versioned-projections.md
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/user-experience-hardening.toml
Support-tier impact: no promotion by campaign setup
Policy impact: no release, publish, signing, Cargo-token, or branch-protection change

## Goal

Continue non-release product work from a clean `adze-swarm` active goal. This
lane owns adoption polish, examples, local proof-loop ergonomics, and
documentation usability while the release/publish tracker remains blocked on
explicit authorization.

## Operating Rules

- Work in `EffortlessMetrics/adze-swarm`.
- Do not open implementation, CI, examples, docs-productization, or proof PRs
  in public `EffortlessMetrics/adze`.
- Do not tag, publish, sign, mutate Cargo-token surfaces, or change release
  workflows in this lane.
- Keep public `adze` as release/public-intake/publish surface.
- Keep support-tier claims bounded by `docs/status/SUPPORT_TIERS.md`.
- Use `Rust Small Result` as the GitHub gate.
- Inspect open `adze-swarm` PRs before opening duplicate work.

## Work Item: user-experience-hardening-source-of-truth

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0006-user-experience-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks:
- starter-example-polish
- api-navigation-polish
- diagnostics-query-ts-compat-walkthroughs
- performance-receipt-guidance
- local-proof-loop-friction
Blocked by: n/a

### Goal

Replace the paused release-boundary active manifest with a non-release
development goal so agents have a clear next lane without touching release
machinery.

### Receipt

Landed in PR #350.

### Production Delta

Docs and source-of-truth metadata only.

### Non-Goals

- No runtime behavior change.
- No release/publish authorization.
- No crates.io install claim.
- No support-tier promotion.

### Acceptance

- `.adze/goals/active.toml` names the user-experience hardening campaign.
- `.adze/goals/user-experience-hardening.toml` exists.
- `policy/doc-artifacts.toml` registers the proposal, plan, and named goal.
- Release blocker tracker #325 remains the release/publish authorization
  checkpoint.

### Proof Commands

```bash
python -c "import tomllib; tomllib.load(open('.adze/goals/active.toml', 'rb')); tomllib.load(open('.adze/goals/user-experience-hardening.toml', 'rb')); tomllib.load(open('policy/doc-artifacts.toml', 'rb'))"
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Revert the setup PR to restore the previous paused release-boundary active
manifest.

## Work Item: starter-example-polish

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0006-user-experience-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
Linked ADR: n/a
Blocks: n/a
Blocked by: user-experience-hardening-source-of-truth

### Goal

Polish starter and downstream examples only where focused tests or checked-in
fixtures prove the behavior.

### Receipt

Landed in PR #352.

### Production Delta

- Generated starter README teaches first-run commands, API choice, project
  layout, and runnable success/error examples.
- Checkout-built CLI scaffolds and parse runners use local sibling path
  dependencies when the workspace crates are present.
- Downstream starter README mirrors the generated starter product ladder.
- CLI canaries assert generated README guidance remains visible.

### Non-Goals

- No crates.io install claim.
- No release workflow change.

### Acceptance

- Edited examples build or have focused canary coverage.
- Docs link to current starter paths.

### Proof Commands

```bash
cargo test -p adze-cli test_init -- --nocapture
cargo test --manifest-path testing/downstream-starter/Cargo.toml
git diff --check
```

### Rollback

Revert the focused example/doc PR.

## Work Item: api-navigation-polish

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0006-user-experience-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: n/a
Blocked by: user-experience-hardening-source-of-truth

### Goal

Improve "which API should I use?" navigation without changing support-tier
claims.

### Receipt

Landed in PR #351.

### Production Delta

- README links directly to the API-choice guide from the install and
  documentation paths.
- The API-choice guide names the user-experience hardening lane and current
  `AdzeDocument` support boundary.
- The campaign plan state matches the active goal manifest.

### Non-Goals

- No new stable API claim.
- No Tree-sitter or query parity claim.

### Acceptance

- Beginner path still points at `grammar::parse`.
- Tooling path still points at `grammar::parse_document`.
- Advanced projections stay support-tier bounded.

### Proof Commands

```bash
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Revert the focused docs PR.

## Work Item: diagnostics-query-ts-compat-walkthroughs

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0006-user-experience-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: n/a
Blocked by: user-experience-hardening-source-of-truth

### Goal

Add or refresh walkthroughs for diagnostics, query, and selected-tree
compatibility only when examples or canaries back the behavior.

### Receipt

Landed in PR #353.

### Production Delta

- Added a diagnostics and recovery reference page that links typed parser
  errors, `parse_document()` diagnostics, GLR bad input, JSON projection
  boundaries, and proof commands.
- Linked the new diagnostics reference from the docs index.
- Routed the diagnostics reference through Product Proof path filters and the
  route canary.
- Cross-linked the diagnostics page with the existing query and Tree-sitter
  compatibility references.

### Non-Goals

- No full Tree-sitter compatibility claim.
- No full query parity claim.

### Acceptance

- Walkthroughs name supported subset and known gaps.
- Any runnable example has a proof command.

### Proof Commands

```bash
cargo test -p adze --features query --lib query -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_selected_tree -- --nocapture
git diff --check
```

### Rollback

Revert the focused walkthrough PR.

## Work Item: performance-receipt-guidance

Status: ready
Linked proposal: ../../docs/proposals/ADZE-PROP-0006-user-experience-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0014-performance-and-regression.md
Linked ADR: n/a
Blocks: n/a
Blocked by: user-experience-hardening-source-of-truth

### Goal

Improve performance receipt guidance without adding unreceipted speed claims.

### Production Delta

To be defined by the PR that picks this item.

### Non-Goals

- No benchmark gate promotion.
- No parser throughput claim without receipt.

### Acceptance

- Performance docs distinguish benchmark receipts from public claims.
- Any concrete performance number links to command, commit, fixture, and
  context.

### Proof Commands

```bash
cargo run -q -p xtask -- perf-receipt --profile product-smoke
git diff --check
```

### Rollback

Revert the focused docs or receipt PR.

## Work Item: local-proof-loop-friction

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0006-user-experience-hardening.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: n/a
Blocks: n/a
Blocked by: user-experience-hardening-source-of-truth

### Goal

Reduce local proof-loop friction when it blocks routine `adze-swarm`
development.

### Receipt

Landed in PR #329.

### Production Delta

- `scripts/ci-supported.sh` now defaults `CARGO_PROFILE_TEST_DEBUG=0`
  for the supported test profile, reducing Windows MSVC linker/PDB pressure
  without changing the supported crate/test surface.
- `docs/status/FRICTION_LOG.md` records FR-021 with the symptom, repro,
  mitigation, and remaining environmental boundary around disk pressure.

### Non-Goals

- No weakening of supported behavior proof.
- No branch-protection change.

### Acceptance

- The friction log records the symptom and fix.
- The smallest relevant local proof command passes or the remaining failure is
  clearly environmental.

### Proof Commands

```bash
just ci-supported
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Revert the focused tooling or docs PR.

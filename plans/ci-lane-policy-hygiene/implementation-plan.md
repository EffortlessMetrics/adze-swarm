# CI Lane Policy Hygiene

Status: complete
Owner: release/ci
Created: 2026-05-21
Linked proposal: `ADZE-PROP-0001`
Linked spec: `docs/specs/ADZE-SPEC-0002-ci-economics.md`
Linked ADRs: n/a
Active goal: `.adze/goals/active.toml`
Policy impact: `.github/CI_LANES.md`, `policy/ci-lane-whitelist.toml`

## Work Item: register-cx53-rust-small-lane

Status: complete
Linked proposal: `ADZE-PROP-0001`
Linked spec: `ADZE-SPEC-0002`
Linked ADR: n/a
Blocks: n/a
Blocked by: n/a
PR: `adze-swarm#393`

### Goal

Register the `rust-small-cx53` implementation job now that the routed Rust
Small workflow can select CX53 as overflow capacity.

### Production Delta

- Add `Rust Small on CX53` to the CI lane map.
- Add `em-rust-small-cx53` to the CI lane whitelist.
- Refresh runner-class docs so they no longer describe CX53 as future-only for
  Rust Small overflow.
- Keep branch protection on aggregate contexts only.

### Non-Goals

- No branch-protection context changes.
- No release, publish, signing, Cargo-token, or crates.io install work.
- No public `adze` work.
- No broad matrix or `rust-large` required-gate change.

### Acceptance

- `check-ci-lane-whitelist --mode blocking-strict` no longer reports
  `rust-small-cx53` as undeclared.
- `Rust Small Result` and `Product Proof Result` remain the required contexts.
- CX53 remains an implementation lane, not a required check.

### Proof Commands

```bash
cargo run -q -p xtask -- check-ci-lane-whitelist --mode blocking-strict
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Remove the `em-rust-small-cx53` lane entry and restore runner-class wording to
describe CX53 Rust Small routing as future-only.

## Work Item: prune-absent-swarm-workflow-lanes

Status: complete
Linked proposal: `ADZE-PROP-0001`
Linked spec: `ADZE-SPEC-0002`
Linked ADR: n/a
Blocks: n/a
Blocked by: n/a
PR: `adze-swarm#394`

### Goal

Keep the adze-swarm lane whitelist scoped to workflow jobs that exist in this
repo. Public/release, Droid, fuzz, benchmark, and performance workflows that
are absent from adze-swarm should not be registered as current swarm lanes.

### Production Delta

- Remove lane whitelist rows for workflow files absent from adze-swarm.
- Remove exceptions that only referred to those absent lanes.
- Update risk-pack deep-lane hints to point at existing product/performance
  evidence lanes.
- Clarify that staged fuzz/benchmark/performance lanes require deliberate
  workflow restoration before they re-enter the whitelist.

### Non-Goals

- No workflow deletion.
- No release, publish, signing, Cargo-token, or crates.io install work.
- No public `adze` work.
- No branch-protection, runner routing, or required-check changes.

### Acceptance

- `check-ci-lane-whitelist --mode blocking-strict` reports no missing workflow
  files.
- Public/release workflows remain outside adze-swarm unless explicitly
  reintroduced.
- Existing aggregate required checks remain unchanged.

### Proof Commands

```bash
cargo run -q -p xtask -- check-ci-lane-whitelist --mode blocking-strict
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Re-add the removed lane rows and exceptions if their workflow files are
deliberately restored in adze-swarm.

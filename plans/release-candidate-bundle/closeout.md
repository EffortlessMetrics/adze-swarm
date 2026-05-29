# Release Candidate Bundle Readiness Closeout

Status: complete
Owner: release/product
Closed: 2026-05-29
Active goal: `../../.adze/goals/active.toml`
Named goal: `../../.adze/goals/release-candidate-bundle.toml`
Plan: `./implementation-plan.md`
Proposal: `../../docs/proposals/ADZE-PROP-0017-release-candidate-bundle.md`
Release authorization tracker: https://github.com/EffortlessMetrics/adze-swarm/issues/325

## Outcome

Outcome: **complete as a non-publish release-candidate bundle; release remains
unauthorized**.

The lane now gives maintainers a reviewable bundle for a deliberate public
promotion/release decision without performing release work:

- selected swarm state and public drift are recorded;
- the bundle checklist is published in `docs/reference/RELEASE_CANDIDATE_BUNDLE.md`;
- non-publish preflight receipts are recorded;
- pre-publish evidence is separated from post-publish crates.io install proof;
- release, publish, signing, Cargo-token, public promotion, and real crates.io
  install work remain blocked on #325.

## Current State

Current `adze-swarm/main` at closeout:

```text
47bc362ee5b02b3efa6c7be77e662efe0aaa4974
docs(release): record non-publish preflight receipts (#557)
```

Open PR queues at closeout:

```text
EffortlessMetrics/adze-swarm: []
EffortlessMetrics/adze: []
```

Public `adze/main` at closeout:

```text
6263c6a80046d13fb98e3ad319dfe726f32f1010
docs(status): sync paused product trust handoff (#798)
```

Read-only public drift at closeout:

```text
git rev-list --left-right --count public/main...origin/main
10    519

git diff --shortstat public/main..origin/main
385 files changed, 20459 insertions(+), 9817 deletions(-)
```

Interpretation: public `adze` is still not the current swarm proof state. Any
release must start with an explicit public promotion/release decision and PR.

## Landed Work

| Work item | PRs | Result |
| --- | --- | --- |
| Source-of-truth setup | #554 | Added ADZE-PROP-0017, active/named goals, implementation plan, and artifact registrations. |
| Current candidate snapshot | #555 | Recorded point-in-time swarm/public state and public drift boundary. |
| Bundle checklist | #556 | Added the release-candidate bundle checklist and linked it from docs. |
| CI routing unblocker | #558 | Added CX33 as a self-hosted Rust Small backfill lane without default hosted fallback. |
| Supported-gate timeout headroom | #559 | Raised the supported-gate timeout so slow proof runs report real failures instead of timing out. |
| Non-publish preflight receipts | #557 | Recorded `just ci-supported`, `just ci-product-stable`, `just check-publishable`, and install-verifier dry-run receipts. |
| Closeout | #560 | Closed the non-publish bundle lane and kept release authorization blocked on #325. |

## Proof Receipts

Representative local proof recorded in this lane:

```bash
just ci-supported
just ci-product-stable
just check-publishable
cargo run -q -p xtask -- verify-crates-io-install adze-cli --bin adze --version X.Y.Z --locked --dry-run
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

CI receipts:

- PR #558 passed `Rust Small on CX33`, `Rust Small Result`, `Supported Rust
  Gate`, `Source of Truth`, and `CI Lane Whitelist`.
- PR #559 passed `Rust Small on CX43`, `Rust Small Result`, `Supported Rust
  Gate`, `PR Gate Success`, `Source of Truth`, and `CI Lane Whitelist`.
- PR #557 passed `Rust Small on CX53`, `Rust Small Result`, `Supported Rust
  Gate`, `PR Gate Success`, `Product Proof Result`, and `Source of Truth`.

## Claim Boundaries

This closeout does not claim or authorize:

- public `adze` promotion;
- release tags;
- crate publishing;
- signing workflow changes;
- Cargo-token work;
- real crates.io install verification;
- a public `cargo install adze-cli` claim;
- broader Tree-sitter parity, query parity, incremental performance, GLR
  generality, or benchmark performance claims.

The dry-run install verifier only proves the command plan. It does not contact
crates.io and does not prove that `cargo install adze-cli` works.

## Next Step

The next release action is a human decision on #325. If release is authorized,
perform public promotion/release work from public `EffortlessMetrics/adze`.

If release is not authorized, keep this bundle as the non-publish handoff and
start a new `adze-swarm` non-release goal only when a maintainer selects the
next development/proof lane.

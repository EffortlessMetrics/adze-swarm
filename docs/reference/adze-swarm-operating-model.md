# Adze-swarm operating model

Status: active operating policy
Owner: repo governance
Created: 2026-05-29
Linked policy: `docs/reference/CODEX_CI_EFFICIENCY_COMPATIBILITY.md`
Linked source-of-truth system: `docs/reference/SPEC_SYSTEM.md`
Linked release checklist: `docs/reference/PUBLISH_CHECKLIST.md`
Support-tier impact: none
Policy impact: defines repo-boundary and CI-boundary expectations

## Mission

`EffortlessMetrics/adze-swarm` is the development, proof, and
release-candidate factory for Adze.

Public `EffortlessMetrics/adze` is the release, public intake, promotion, tag,
publish, signing, Cargo-token, and crates.io receipt surface.

The steady-state rule is:

```text
adze-swarm produces proof-backed, release-promotable states.
public adze receives explicit promotion or release PRs from those states.
```

## Responsibilities

Use `adze-swarm` for:

- runtime, tablegen, GLR, query, diagnostics, recovery, examples, docs, and
  performance proof work;
- CI economics and self-hosted runner policy;
- source-of-truth updates for proposals, specs, ADRs, plans, active goals,
  support tiers, policy ledgers, and proof receipts;
- same-repo swarm PRs that start from `adze-swarm/main`;
- release-candidate preparation before a deliberate public promotion.

Use public `adze` for:

- public release-readable state after explicit promotion;
- public intake and external contributions;
- release branches, tags, publish workflows, signing, Cargo-token handling, and
  crates.io post-publish install receipts.

Do not open implementation, productization, CI-hardening, docs-proof, examples,
runtime, tablegen, query, diagnostics, or performance PRs in public `adze`
unless the task is explicitly a public promotion or release task.

## Normal swarm PR flow

For routine work:

```bash
git fetch origin --prune
git switch main
git pull --ff-only origin main
gh pr list --repo EffortlessMetrics/adze-swarm --state open

git switch -c swarm/<task>
```

Each PR must:

- cover one work item;
- link the relevant proposal, spec, ADR, plan, active goal, support-tier row, or
  policy ledger;
- state its claim boundary, proof commands, CI cost expectation, and rollback
  path;
- avoid duplicate same-scope work in the live PR queue;
- wait for `Rust Small Result` plus the relevant path-routed proof.

## CI boundary

`adze-swarm` defaults to self-hosted execution. GitHub Actions remains the
orchestrator, but default PR proof should run on project-controlled runners.

Required base gate:

```text
Rust Small Result
```

Do not require implementation lanes directly:

```text
Rust Small on CX43
Rust Small on CX53
Rust Small on CPX42
Rust Small on GitHub Hosted
```

Exactly one implementation lane may run while the others skip. A GitHub-hosted
fallback must not be introduced silently. Hosted execution in `adze-swarm`
requires an explicit recorded exception, such as a scoped label, manual dispatch
input, or release/public-safety note.

Heavy, advisory, coverage, benchmark, product-proof, and full-matrix lanes run
only when path-routed, labeled, scheduled, manually dispatched, or otherwise
explicitly budgeted.

## Product proof boundary

Adze product claims must become true in `adze-swarm` before they are promoted to
public `adze`.

The architectural invariant is:

```text
AdzeDocument is the canonical parse product.
Every public view is a projection over that document.
```

Typed CST, typed AST, diagnostics, ambiguity summaries, Tree-sitter-compatible
selected-tree output, query surfaces, JSON, CLI, WASM, and benchmark receipts
must not invent independent semantic state. Optimizations may add compact
storage or caches only when they are document-local, invalidation-aware, and
covered by equivalence proof.

A feature is not Stable merely because code exists. Stable public claims require
aligned README wording, support-tier rows, proof commands, CI lanes, examples,
and known limitations.

Do not claim any of the following without a current receipt:

- `cargo install adze-cli` works from crates.io;
- full Tree-sitter parity;
- full query parity;
- stable incremental reuse or performance;
- general GLR coverage beyond the proven grammar classes;
- benchmark throughput or memory numbers as public claims.

## Promotion boundary

Release flow is deliberate:

1. Develop and prove in `adze-swarm`.
2. Freeze a release-candidate snapshot.
3. Promote the selected state into public `adze` with an explicit promotion PR.
4. Run public release preflight from public `adze`.
5. Tag and publish from public `adze` only after human authorization.
6. Record crates.io install receipts after publish.

Do not move release, publish, signing, or Cargo-token workflows into
`adze-swarm`.

## Stop conditions

Stop and report instead of guessing when:

- the live `adze-swarm` queue already has same-scope work;
- the active goal is complete or paused and no new lane was explicitly selected;
- the work would touch public `adze` without a promotion or release instruction;
- a public claim lacks support-tier proof;
- a CI change introduces hosted fallback without a recorded exception;
- a release, tag, publish, signing, Cargo-token, or crates.io install-receipt
  action would be required.

## Endpoint

The endpoint is boring operation:

```text
A worker opens one adze-swarm PR.
The PR links source-of-truth artifacts.
Self-hosted scoped proof runs.
The claim boundary is explicit.
The PR merges after green proof.
Public adze changes only through intentional promotion or release.
```

That makes `adze-swarm` the workshop, lab, and ledger, while public `adze`
remains the storefront and release counter.

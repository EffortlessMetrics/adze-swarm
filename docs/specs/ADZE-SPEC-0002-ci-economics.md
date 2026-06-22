# ADZE-SPEC-0002: CI economics

Status: accepted
Owner: Adze maintainers
Created: 2026-05-12
Linked proposal: ../proposals/ADZE-PROP-0001-0.9-contract-convergence.md
Linked ADRs:
Linked plan: ../../plans/0.9.0/implementation-plan.md
Linked issues:
Linked PRs:

## Problem

Adze needs more verification than a broad default PR matrix can economically
provide. Parser correctness, GLR routing, tablegen ABI, typed extraction,
diagnostics, Tree-sitter compatibility, WASM, golden parity, benchmarks, and
release policy all matter, but not every PR should pay for every lane.

The current CI policy already defines LEM, risk routing, advisory lanes, and
branch-protection promotion criteria. This spec turns that policy into a
behavior contract for 0.9 work: every CI lane must have an owner, cost model,
trigger rule, support-tier purpose, and rollback path.

## Behavior

CI economics must preserve product proof while routing expensive checks by risk.

Adze CI lanes must be classified into these verification tiers:

| Tier | Purpose | Default behavior |
| --- | --- | --- |
| Frontdoor | Required proof for supported product claims and policy consistency. | Runs on every PR and blocks merge. |
| Advisory | Cheap or useful signal that should inform review but not block ordinary PRs. | Runs when economical; non-blocking. |
| Risk-routed | Deep checks selected by path, label, or declared risk pack. | Runs only when the change touches the matching surface or is explicitly requested. |
| Deep | Expensive validation for broad confidence. | Runs on `main`, scheduled workflows, release prep, or explicit labels. |
| Release | Publication and release-readiness proof. | Runs for tags, release branches, or manual release operations. |

The CI lane whitelist and risk-pack ledger own the concrete lane inventory:

- `../../policy/ci-lane-whitelist.toml`
- `../../policy/ci-risk-packs.toml`

This spec owns the behavior rules. It must not duplicate the full whitelist.

## LEM Bands

CI cost should be expressed in LEM as defined by
`../ci/cost-and-verification-policy.md`.

The current behavioral bands are:

| Band | LEM | Required behavior |
| --- | ---: | --- |
| Ordinary | 0-35 | Preferred default for normal PRs. |
| Elevated | 36-75 | Warning with explicit risk surface. |
| High | 76-125 | High warning with explicit label or acknowledgement. |
| Over ceiling | >125 | Fails unless `full-ci` or `ci-budget-override` is present. |

Learned estimates belong in `../ci/learned-estimates.md` and policy ledgers,
not in this spec. This spec requires the estimates to be traceable and
reviewable.

## Required Lane Metadata

Every CI lane in the whitelist must record:

- workflow and job name;
- tier;
- owner surface;
- trigger rule;
- blocking status;
- estimated LEM or cost band;
- support-tier purpose;
- risk packs or labels that activate the lane;
- rollback path;
- policy exception, if any.

Any workflow job that is not in the whitelist must either be added to the
ledger, explicitly exempted, or removed.

## Non-Goals

This spec does not:

- weaken `just ci-supported` or the hosted `CI / ci-supported` gate;
- make `ripr` or broad advisory lanes blocking;
- promote learned LEM budgets before enough actuals exist;
- change branch protection by itself;
- remove broad validation from `main`, nightly, label-triggered, or release
  paths;
- duplicate the CI lane whitelist, risk-pack ledger, or learned estimates.

## Required Evidence

A CI-economics implementation must provide:

- a machine-readable lane whitelist;
- a verifier that detects unregistered workflow jobs;
- explicit routing for risk-routed lanes;
- LEM estimates or learned actuals for ordinary PR planning;
- a branch-protection plan before required checks change;
- PR evidence that names workflows touched, default PR effect, rollback path,
  proof obligation, and branch-protection impact.

## Acceptance Examples

### Frontdoor lane

`CI / ci-supported` is frontdoor proof because it represents the supported core
pipeline. A PR that changes supported runtime, tablegen, macro, tool, common,
IR, or GLR core behavior must keep this gate green.

### Advisory lane

An advisory static-analysis or review lane may run on every PR when cheap, but
its result is not a substitute for supported product proof and should not block
ordinary docs or low-risk changes unless explicitly promoted.

### Risk-routed lane

Golden Tree-sitter parity, fuzz build, benchmark compile, and broad grammar
matrix jobs should run when a matching path, risk pack, or label requests them.
They should not be the default cost paid by unrelated docs or policy PRs.

### Over-ceiling PR

A PR plan that estimates more than 125 LEM must fail or require an explicit
override label. The override is a review signal, not a silent escape hatch.

## Test Mapping

The CI-economics verifier should include tests for:

- every workflow job is present in the lane whitelist or an explicit exception;
- each lane has exactly one tier;
- blocking lanes are a subset of frontdoor or explicitly promoted release gates;
- risk-routed lanes name a risk pack, path trigger, label trigger, or manual
  trigger;
- LEM bands are parsed and compared consistently;
- over-ceiling PR plans fail without override;
- docs-only PRs do not route parser-heavy risk packs by default;
- branch-protection promotion requires a documented stability window.

These tests may live in xtask, policy-check crates, or workflow-specific test
fixtures, but the implementation plan must name the command that runs them.

## Implementation Mapping

Expected implementation surfaces:

- `../../policy/ci-lane-whitelist.toml` for lane inventory;
- `../../policy/ci-risk-packs.toml` for risk routing;
- `../ci/cost-and-verification-policy.md` for CI doctrine;
- `../ci/adze-rollout-plan.md` for per-PR rollout history;
- `../ci/branch-protection.md` for required-check promotion;
- `../ci/learned-estimates.md` for actuals-based estimate updates;
- an xtask verifier such as `cargo run -q -p xtask -- check-ci-lane-whitelist`;
- PR Plan or equivalent output for LEM estimates and risk routing.

Specs should link to these files instead of copying their tables.

## CI Proof

The intended proof sequence for CI-economics changes is:

```bash
cargo run -q -p xtask -- check-ci-lane-whitelist
cargo run -q -p xtask -- ci plan
just ci-supported
```

If a PR changes branch protection, it must also include the branch-protection
promotion evidence required by `../ci/branch-protection.md`.

If a PR changes package boundaries, it must also run the package-boundary proof
from `ADZE-SPEC-0001-package-surface-boundary.md`.

## Metrics / Promotion Rule

The CI-economics contract is satisfied for 0.9 when:

- every workflow job is whitelisted or explicitly exempted;
- ordinary PRs have a visible LEM estimate;
- elevated and high-cost PRs produce explicit warnings;
- over-ceiling PRs require an override;
- risk-routed lanes have path, label, or manual triggers;
- frontdoor product proof remains green and required;
- branch-protection changes happen only after documented stability windows.

This spec can move from proposed to accepted after the whitelist verifier,
risk-pack routing, PR-cost signal, and branch-protection policy all have
passing proof commands and current documentation.

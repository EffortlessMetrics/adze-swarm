# ADZE-SPEC-0001: Package surface boundary

Status: accepted
Owner: Adze maintainers
Created: 2026-05-12
Linked proposal: ../proposals/ADZE-PROP-0001-0.9-contract-convergence.md
Linked ADRs: ADZE-ADR-0002 no durable unpublished production crates
Linked plan: ../../plans/0.9.0/microcrate-collapse.md
Linked issues:
Linked PRs:

## Problem

The Adze workspace has grown beyond the stable product surface. Some packages
are published user-facing crates, some are test or governance support, and some
are temporary seams created while parser, policy, CI, and product-proof work was
being split into small changes.

That distinction is useful only if it is explicit. A workspace package that is
neither publishable nor clearly dev-only becomes a hidden production surface:
it affects CI cost, MSRV and lint migration effort, release confidence, and
maintainer reasoning, but it has no stable user contract.

Adze needs a package-boundary contract before 0.9 work changes MSRV, lint
policy, CI routing, or public support claims.

## Behavior

Every workspace package must be classified as exactly one of these categories:

| Category | Meaning | Allowed durability |
| --- | --- | --- |
| Published crate | A public package intended for crates.io or an equivalent stable user surface. | Durable when support-tier and release metadata agree. |
| Dev-only crate | A package used only for tests, examples, benchmarks, fixtures, xtask support, or internal repo automation. | Durable only while it has a current dev/test/tooling owner and is excluded from production claims. |
| Owner-module migration target | A temporary package scheduled to move into the public crate, dev-only crate, or xtask module that owns it as an SRP submodule. | Temporary; must name its target owner and removal condition. |

There is no durable unpublished production crate category.

There is also no release-state owner-module migration category. A migration
target is a pre-release transition state. Before the next release, every
migration-target microcrate must be removed, inlined, moved into an SRP
submodule under its owner, or reclassified by an accepted ADR.

If a package is used by production code but is not intended to be published as a
public surface, it must either become an owner-module migration target or be
reclassified with an explicit ADR and support-tier impact.

## Required Package Metadata

The package-boundary ledger must record enough data for automation and review:

- package name;
- package path;
- category;
- owner surface or owning module;
- publish intent;
- support-tier impact;
- CI lane or risk-pack impact;
- migration target when category is owner-module migration target, expressed as
  the intended SRP owner module or xtask/tooling module;
- removal or promotion condition;
- date and PR that last changed the classification.

The ledger location is `../../policy/package-boundary.toml`. This spec is the
behavior contract and implementation plans must not invent conflicting category
names.

## Non-Goals

This spec does not:

- move, merge, publish, or delete any package;
- decide which current packages belong in each category;
- change Cargo metadata by itself;
- change branch protection or CI routing by itself;
- define support-tier promotion rules beyond requiring links to
  `../status/SUPPORT_TIERS.md`;
- require every dev-only crate to be collapsed immediately.

## Required Evidence

A package-boundary implementation must provide:

- a machine-readable package-boundary ledger;
- a verifier that checks every workspace package has exactly one category;
- a release-gate verifier mode that fails while any owner-module migration
  target remains unresolved;
- a verifier failure when a package is production-used, unpublished, and not a
  migration target;
- a verifier failure when an owner-module migration target has no owner or no
  exit condition;
- a docs update for any change that alters stable product claims;
- a CI or local proof command that can be run before package-collapse PRs land.

## Acceptance Examples

### Published crate

`adze` is a published crate because it is a user-facing runtime package and has
stable product claims in `../status/SUPPORT_TIERS.md`.

A published crate must have release metadata, support-tier mapping for stable
claims, and a supported proof path.

### Dev-only crate

A benchmark, fixture, or repo-policy helper package can be dev-only when it is
not part of the advertised runtime or generated-parser API and exists to support
tests, measurement, examples, or automation.

A dev-only crate must not be cited as a stable production surface unless it is
reclassified.

### Owner-module migration target

A single-use helper crate consumed by one public crate can be a migration target
when its desired end state is an SRP submodule under that owner.

The classification must name the target owner and the condition that closes the
migration. That closure must happen before the next release.

### Invalid durable category

A package cannot remain indefinitely classified as:

```text
unpublished production crate
```

That state hides a production dependency from release, support-tier, and CI
economics decisions. It must be converted to a published crate, dev-only crate,
or owner-module migration target.

## Test Mapping

The package-boundary verifier should include tests for:

- every workspace member appears in the ledger;
- unknown package names fail validation;
- duplicate package entries fail validation;
- each package has exactly one category;
- published packages have publish intent and release metadata checks;
- dev-only packages cannot be marked as stable product proof surfaces;
- migration targets have owner and exit-condition fields;
- production-used unpublished packages fail unless they are migration targets;
- policy changes produce stable diagnostics that identify the package and field.

These tests may live in xtask or a dedicated policy-check crate, but the command
must be documented in the implementation plan.

## Implementation Mapping

Expected implementation surfaces:

- `policy/package-boundary.toml` for package classification;
- `policy/release-graph.toml` for the generated ledger-published, dependency-ordered release graph;
- an xtask verifier such as `cargo run -q -p xtask -- check-package-boundary`;
- release-graph generator/checker commands such as `cargo run -q -p xtask -- generate-release-graph` and `cargo run -q -p xtask -- check-release-graph`;
- `Cargo.toml` workspace metadata only when a later implementation PR changes
  package membership or release metadata;
- `docs/status/SUPPORT_TIERS.md` when stable product claims change;
- `policy/ci-lane-whitelist.toml` and `policy/ci-risk-packs.toml` when package
  classification changes CI routing.

Specs and proposals must link to those ledgers instead of duplicating their
contents.

## CI Proof

The intended proof sequence for package-boundary changes is:

```bash
cargo metadata --format-version 1 --no-deps
cargo run -q -p xtask -- check-package-boundary
just ci-supported
```

Implementation PRs may add narrower tests for the verifier. Package-collapse PRs
must run the verifier and the supported gate after any workspace membership or
Cargo metadata change.

The release-candidate proof adds:

```bash
cargo run -q -p xtask -- check-package-boundary --release-gate
cargo run -q -p xtask -- check-release-graph
./scripts/check-release-consumers.sh
PACKAGE_BOUNDARY_RELEASE_GATE=1 ./scripts/validate-release-surface.sh
```

Those commands are expected to fail while the transition is still active.

## Metrics / Promotion Rule

This spec is satisfied for 0.9 when:

- every workspace member is classified in the package-boundary ledger;
- the verifier is part of the documented 0.9 proof sequence;
- no package is classified as a durable unpublished production crate;
- every migration target has an owner, SRP submodule target, and exit condition;
- the release checklist blocks release while any migration target remains
  unresolved;
- support-tier and CI policy docs are updated when package classification
  changes stable claims or CI routing.

The package-boundary policy can be promoted from proposal-level guidance to an
accepted repo contract after the ledger and verifier have both landed and
`just ci-supported` is green with the verifier in the 0.9 proof sequence.

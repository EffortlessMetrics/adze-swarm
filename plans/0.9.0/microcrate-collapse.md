# 0.9 Microcrate To SRP Submodule Transition Plan

Status: complete
Owner: release/package
Created: 2026-05-13
Linked proposal: ../../docs/proposals/ADZE-PROP-0001-0.9-contract-convergence.md
Linked spec: ../../docs/specs/ADZE-SPEC-0001-package-surface-boundary.md
Linked ADR: ../../docs/adr/ADZE-ADR-0002-no-durable-unpublished-production-crates.md
Active goal: ../../.adze/goals/active.toml
Policy ledger: ../../policy/package-boundary.toml

## Goal

Before the next release, transition migration-target microcrates into SRP
submodules under their owning public crate, dev-only crate, or xtask/tooling
module.

This is a release prerequisite, not a documentation preference. A package can
remain a standalone crate at release only when it is a published surface or a
durable dev-only/tooling surface with a current owner and proof rationale.

The release-blocking transition was completed by PR #758 and closed out by PR
#759. This plan remains as the closeout record and as the release-gate contract
for keeping migration targets out of the release surface.

## Current State

As of the post-collapse workspace after PR #104:

```text
workspace packages: 29
owner-module migration targets: 0
```

The package-boundary ledger is the source of truth for the exact package list.

## Target Shape

For each migration target, choose exactly one outcome:

1. **Delete** it when it has no live external consumer.
2. **Inline** it into the single crate that consumes it.
3. **Move** it into an SRP submodule under the owning crate or xtask module.
4. **Reclassify** it only with an accepted ADR explaining why it remains a
   standalone crate before release.

The expected end state before release:

```text
owner-module migration targets: 0
```

## Work Item: retire-zero-reverse-dependency-facades

Status: complete
Linked PRs: #696, #697, #741, #742, #743, #744, #745, #746, #747, #748, #749, #750, #751

### Goal

Remove facade crates that have no reverse dependencies and only re-export or
wrap another owner surface.

### Production Delta

Delete the crate directory, remove the workspace member, remove lockfile and
release-list entries, update policy ledgers, update microcrate CI routing, and
run package-boundary proof.

### Proof Commands

```bash
cargo metadata --format-version 1 --no-deps
cargo run -q -p xtask -- check-package-boundary
cargo run -q -p xtask -- check-ci-lane-whitelist
just ci-supported
```

## Work Item: parser-contracts-to-srp-submodules

Status: done
Owner: parser-contracts

### Goal

Move parser contract/support crates into SRP submodules under their actual
consumer, or remove them when they are only test scaffolding.

### Completed Packages

- `adze-feature-policy-core` moved into
  `adze-bdd-governance-core::feature_policy`.
- `adze-parsetable-metadata` reclassified as a durable published support crate
  by `ADZE-ADR-0005`.

### Proof Commands

```bash
cargo metadata --format-version 1 --no-deps
cargo run -q -p xtask -- check-package-boundary
cargo test -p adze-parsetable-metadata -- --test-threads=2
just ci-supported
```

## Work Item: governance-to-xtask-srp-submodules

Status: complete
Owner: governance/policy

### Goal

Move governance matrix/status/runtime reporting microcrates into xtask or the
single policy owner that consumes them.

### Candidate Packages

No standalone governance runtime package targets remain.

### Completed Packages

- `adze-governance-runtime-profile-core` moved into
  `adze-governance-runtime-core::profile`.
- `adze-governance-contract` facade removed; its only remaining dev consumer
  now imports `adze-bdd-governance-core` directly.
- `adze-governance-matrix-contract` facade removed; remaining consumers now
  import `adze-governance-matrix-core` directly.
- `adze-governance-matrix-core-impl` facade removed; `adze-governance-matrix-core`
  now imports `adze-bdd-governance-core` directly.
- `adze-governance-matrix-core` facade removed; runtime governance consumers now
  import `adze-bdd-governance-core` directly.
- `adze-governance-runtime-reporting` collapsed into
  `adze-governance-runtime-core`; runtime report formatting is now owned by the
  remaining runtime governance core.
- `adze-governance-metadata` moved into
  `adze-parsetable-metadata::governance`.
- `adze-governance-runtime-core` moved into
  `adze-bdd-governance-core::runtime`.

### Proof Commands

```bash
cargo metadata --format-version 1 --no-deps
cargo run -q -p xtask -- check-package-boundary
cargo test -p adze-bdd-governance-core -- --test-threads=2
just ci-supported
```

## Work Item: concurrency-to-srp-submodules

Status: complete
Owner: governance/concurrency
Linked PRs: #729, #730, #731, #732, #733, #734, #735, #736, #737

### Goal

Collapse concurrency policy/configuration helper crates into one owner module or
remove unused seams. No standalone concurrency microcrate targets remain.

### Candidate Packages

No standalone concurrency microcrate targets remain.
### Proof Commands

```bash
cargo metadata --format-version 1 --no-deps
cargo run -q -p xtask -- check-package-boundary
cargo test -p adze -- --test-threads=2
just ci-supported
```

## Work Item: bdd-to-srp-submodules

Status: complete
Owner: governance/bdd

### Goal

Collapse BDD/governance fixture and contract crates into one owner module or
test support module.

### Candidate Packages

No standalone BDD migration targets remain.

### Completed Packages

- `adze-bdd-governance-contract` facade removed; remaining consumers now
  import `adze-bdd-governance-core` directly.
- `adze-bdd-governance-fixtures` facade removed; scenario fixtures now own the
  current-profile helper functions and import BDD governance core directly.
- `adze-bdd-scenario-fixtures` facade removed; the GLR BDD test now imports
  grammar fixtures and BDD governance reporting helpers directly.
- `adze-bdd-grid-core` moved into `adze-bdd-governance-core::grid`.
- `adze-bdd-grammar-fixtures` moved into
  `glr-test-support::grammar_fixtures`.
- `adze-bdd-governance-core` reclassified as a durable published support crate
  by `ADZE-ADR-0005`.

### Proof Commands

```bash
cargo metadata --format-version 1 --no-deps
cargo run -q -p xtask -- check-package-boundary
cargo test -p adze-bdd-governance-core -p glr-test-support -- --test-threads=2
just ci-supported
```

## Work Item: feature-policy-to-srp-submodule

Status: done
Owner: governance/feature-policy

### Goal

Move feature/backend selection policy into the governance owner module that
actually consumes it, or reclassify it with an accepted ADR if it remains a
durable standalone crate.

### Completed Packages

- `adze-feature-policy-core` moved into
  `adze-bdd-governance-core::feature_policy`.

### Proof Commands

```bash
cargo metadata --format-version 1 --no-deps
cargo run -q -p xtask -- check-package-boundary
cargo test -p adze-bdd-governance-core -- --test-threads=2
just ci-supported
```

## Work Item: runtime-governance-to-srp-submodules

Status: complete
Owner: runtime-governance

### Goal

Move runtime governance support crates into the runtime, runtime2, or xtask
owner modules that consume them.

### Candidate Packages

- None remaining in this work item.

### Completed Packages

- `adze-runtime-governance` facade removed; runtime now imports
  the shared governance runtime core directly while preserving the public
  `adze::parser_selection::*` compatibility module.
- `adze-runtime-governance-matrix` removed; runtime now owns
  `adze::parser_selection::*` helper composition and runtime2 owns its
  runtime2-specific helper composition.
- `adze-runtime-governance-api` facade removed before the runtime facade;
  the public `adze::parser_selection::*` compatibility module remains owned by
  runtime.
- `adze-runtime2-governance` facade removed; runtime2 now imports
  the shared governance runtime core directly while preserving runtime2-specific
  public helper functions in the runtime2 owner module.
- `adze-governance-runtime-core` moved into
  `adze-bdd-governance-core::runtime`.

### Proof Commands

```bash
cargo metadata --format-version 1 --no-deps
cargo run -q -p xtask -- check-package-boundary
cargo test -p adze-bdd-governance-core runtime -- --test-threads=2
cargo test -p adze -p adze-runtime -- --test-threads=2
just ci-supported
```

## Work Item: source-location-and-formatting-srp-submodules

Status: complete
Owner: diagnostics/source-location and syntax-formatting

### Goal

Move source-location, stack-pool, syntax-formatting, and table-metadata helpers
into the owner crate where each API is actually used, unless a release-facing
reason keeps it standalone.

### Candidate Packages

No standalone source-location migration targets remain.

### Completed Packages

- `adze-linecol-core` reclassified as a durable published support crate by
  `ADZE-ADR-0005`.
- `adze-common-type-ops-core` is tracked as a durable published support crate
  in the package-boundary ledger for macro/tool type-shape helpers.

### Proof Commands

```bash
cargo metadata --format-version 1 --no-deps
cargo run -q -p xtask -- check-package-boundary
cargo test -p adze-linecol-core -p adze-common-type-ops-core -- --test-threads=2
just ci-supported
```

## Release Gate

Routine collapse or classification PRs should keep this transition check green:

```bash
cargo run -q -p xtask -- check-package-boundary
```

The stricter release gate now passes and must stay green before the next
release:

```bash
cargo run -q -p xtask -- check-package-boundary --release-gate
```

The release helper and release workflow pass:

```bash
PACKAGE_BOUNDARY_RELEASE_GATE=1 ./scripts/validate-release-surface.sh
```

Those release-gate checks fail if any
`owner-module-migration-target` entry remains in
`policy/package-boundary.toml`.

If any migration target remains, release is blocked unless a new accepted ADR
reclassifies that package as a durable standalone crate.

## Closeout

The transition removed or folded the temporary governance, BDD, runtime
governance, concurrency, feature-policy, parser-contract, and source-location
microcrate seams. The only remaining production support crates are durable
published support surfaces recorded by `ADZE-ADR-0005`:

- `adze-bdd-governance-core`
- `adze-common-type-ops-core`
- `adze-linecol-core`
- `adze-parsetable-metadata`

Release readiness is now checked by:

```bash
cargo run -q -p xtask -- check-package-boundary --release-gate
PACKAGE_BOUNDARY_RELEASE_GATE=1 ./scripts/validate-release-surface.sh
```

## Rollback

Each owner-sized transition PR should be independently revertible. Rollback must
restore workspace membership, lockfile entries, release lists, policy ledger
classification, CI routing, and docs references for the affected owner group.

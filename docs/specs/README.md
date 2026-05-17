# Specs

Specs are the behavior-contract layer. They define what must be true, what is
out of scope, how the behavior is accepted, and which proof commands establish
the contract.

Specs are not proposals, ADRs, or task plans. They should link to those layers
instead of copying their contents.

## Source Of Truth

Specs own:

- behavior requirements
- non-goals
- acceptance examples
- required evidence
- implementation ownership boundaries
- test and CI proof mapping
- support-tier promotion criteria

Other artifacts own:

- why the work exists: `docs/proposals/`
- durable architecture decisions: `docs/adr/`
- PR-sized sequencing: `plans/<milestone>/`
- active agent/operator state: `.adze/goals/active.toml`
- product claim proof mapping: `docs/status/SUPPORT_TIERS.md`
- exception ledgers: `policy/*.toml`

## Naming

Use stable IDs:

```text
ADZE-SPEC-0001-short-kebab-title.md
```

Examples:

```text
ADZE-SPEC-0001-package-surface-boundary.md
ADZE-SPEC-0002-ci-economics.md
ADZE-SPEC-0003-canonical-parse-document.md
ADZE-SPEC-0004-typed-cst-and-ast-projections.md
ADZE-SPEC-0005-diagnostics-and-recovery.md
ADZE-SPEC-0006-tree-sitter-compatibility-adapter.md
ADZE-SPEC-0007-glr-ambiguity-summary.md
ADZE-SPEC-0008-json-cli-wasm-projections.md
ADZE-SPEC-0009-incremental-document-lifecycle.md
ADZE-SPEC-0010-language-metadata-and-node-types.md
ADZE-SPEC-0011-product-proof-and-support-tiers.md
ADZE-SPEC-0012-glr-toolkit-product-contract.md
ADZE-SPEC-0013-query-compatibility.md
ADZE-SPEC-0014-performance-and-regression.md
```

## Header

Every spec should start with:

```md
Status: proposed | accepted | implemented | superseded
Owner:
Created:
Linked proposal:
Linked ADRs:
Linked plan:
Linked issues:
Linked PRs:
Support-tier impact:
Policy impact:
```

## Template

```md
# ADZE-SPEC-0001: Title

Status:
Owner:
Created:
Linked proposal:
Linked ADRs:
Linked plan:
Linked issues:
Linked PRs:
Support-tier impact:
Policy impact:

## Problem

What behavior or contract gap exists?

## Behavior

What must be true?

## Non-Goals

What is out of scope?

## Required Evidence

What proof is required?

## Acceptance Examples

Concrete examples of accepted and rejected behavior.

## Test Mapping

Which tests, fixtures, or snapshots cover this contract?

## Implementation Mapping

Which crates, modules, docs, or policy files own the implementation?

## CI Proof

Which commands and CI lanes prove the contract?

## Metrics And Promotion Rule

What moves this from experimental/advisory to stable?
```

## Duplication Rule

Specs may reference product claims in `docs/status/SUPPORT_TIERS.md`, but must
not copy the full feature-to-proof table. Specs may reference CI economics and
exceptions in `policy/*.toml`, but must not copy policy ledgers into prose.

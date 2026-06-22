# ADZE-PROP-0001: 0.9 contract convergence

Status: implemented
Owner: Adze maintainers
Created: 2026-05-12
Target milestone: 0.9.0
Linked specs: ADZE-SPEC-0001 package surface boundary; ADZE-SPEC-0002 CI economics; ADZE-SPEC-0003 canonical parse document; ADZE-SPEC-0011 product proof and support tiers
Linked ADRs: ADZE-ADR-0001 AdzeDocument one parse truth; ADZE-ADR-0002 no durable unpublished production crates; ADZE-ADR-0003 summary-first GLR ambiguity
Linked plan: ../../plans/0.9.0/implementation-plan.md
Linked issues:
Linked PRs:
Support-tier impact: ../status/SUPPORT_TIERS.md
Policy impact: ../../policy/package-boundary.toml; ../../policy/ci-lane-whitelist.toml; ../../policy/clippy-lints.toml; ../../policy/non-rust-allowlist.toml

## Problem

Adze 0.8 established a publishable Rust parser-generator baseline, but the repo
surface is now wider than the product contract. The workspace contains many
support crates and governance lanes, while the advertised product story depends
on a smaller set of stable promises:

- Rust types define grammar structure.
- Generated parsers return typed ASTs directly.
- The pure-Rust path is the ordinary supported path.
- GLR behavior is honest about conflicts and ambiguity.
- Parse errors are useful to downstream users.
- Stable README claims map to concrete proof commands.

The next release should not add more loosely connected surfaces before the
existing contract is easier to reason about. The 0.9 milestone should converge
the package surface, CI economics, lint policy, and product-proof map so
maintainers can tell which parts of Adze are stable, which are developing, and
which are intentionally advisory.

## Users And Surfaces

This proposal affects four groups:

- Rust users who want `grammar::parse(source)` to produce typed values without
  repo-specific setup.
- Grammar authors who need GLR conflicts, tablegen output, diagnostics, and
  typed extraction to fail clearly.
- Maintainers and agents who need a bounded workspace, readable CI signal, and
  machine-readable execution state.
- Tooling adopters who need Tree-sitter compatibility, WASM, CLI output,
  runtime2, and broader grammars to be labeled honestly until promoted.

The affected repo surfaces are the workspace package graph, `just ci-supported`,
CI policy ledgers, support-tier documentation, README feature claims, parser
proof tests, and the native document and Tree-sitter compatibility roadmap.

## Success Criteria

0.9 contract convergence is successful when:

- every workspace package is classified as a published crate, dev-only crate,
  or temporary owner-module migration target on its way to an SRP submodule;
- no durable unpublished production crate category remains;
- the supported CI lane stays green and remains the required proof for stable
  core claims;
- CI lane routing and LEM estimates reflect the post-collapse workspace shape;
- Rust/MSRV, lint, and allowlist policy changes are represented in policy
  ledgers instead of scattered prose;
- every stable README claim maps to a row and proof command in
  `../status/SUPPORT_TIERS.md`;
- experimental surfaces remain explicitly labeled until promotion proof lands;
- product-proof canaries cover the advertised typed AST quickstart, pure-Rust
  parsing path, diagnostics, tablegen, GLR conflict behavior, and native
  document projections at their claimed support tiers.

## Proposed Shape

Treat 0.9.0 as a contract-convergence release.

The release should make the supported product smaller, clearer, and easier to
prove before expanding the public surface again. The campaign has four parts:

1. Collapse or classify the workspace package surface.
2. Recalibrate CI economics and policy ledgers around that smaller surface.
3. Promote only product claims that have support-tier proof.
4. Keep native parser API design moving through specs and ADRs rather than
   incidental implementation drift.

This proposal does not make `AdzeDocument`, typed CST, Tree-sitter
compatibility, WASM, runtime2, CLI output, or broader grammar support stable by
declaration. Those surfaces can advance during the milestone, but each needs its
own spec, proof lane, and support-tier update before being marketed as stable.

## Alternatives Considered

### Feature-first 0.9

Adze could make 0.9 primarily about more parser features, more grammar
coverage, or broader compatibility claims.

Rejected because the current repo already has valuable developing surfaces, but
the main risk is claim drift: users and maintainers need clearer proof of what
is stable before more surfaces are promoted.

### CI-only 0.9

Adze could focus only on CI cost and workflow policy.

Rejected because CI economics are tied to package boundaries and support tiers.
Cheaper CI without a clarified product contract would hide the underlying
maintenance problem.

### Docs-only 0.9

Adze could document the current state without changing package boundaries,
policy, or proof lanes.

Rejected because the workspace surface and support claims need executable
policy, not just narrative cleanup.

### Big-bang cleanup

Adze could combine package collapse, MSRV, lint policy, CI economics, and
product proof in one large PR.

Rejected because the high-judgment parts need small reviewable artifacts and
proof commands. The campaign should be a linked sequence, not a single opaque
diff.

## Specs To Create Or Update

The milestone needs behavior specs for:

- `ADZE-SPEC-0001-package-surface-boundary.md`: workspace package categories,
  owner-module migration targets, the SRP submodule transition requirement, and
  the rule that there is no durable unpublished production crate category.
- `ADZE-SPEC-0002-ci-economics.md`: blocking versus advisory lanes, LEM bands,
  risk routing, and how policy ledgers own CI exceptions.
- `ADZE-SPEC-0003-canonical-parse-document.md`: the native parse document as
  the source of truth for generic CST, typed CST, typed AST, diagnostics,
  Tree-sitter-compatible projection, and GLR ambiguity summaries.
- `ADZE-SPEC-0011-product-proof-and-support-tiers.md`: the rule that stable
  README claims require proof commands and support-tier mapping.

Specs define behavior and acceptance evidence. They must link to
`../status/SUPPORT_TIERS.md` and `../../policy/*.toml` instead of copying those
ledgers.

## Architecture Decisions Needed

The milestone needs ADRs for durable choices that should outlive the 0.9 plan:

- `ADZE-ADR-0001-adze-document-one-parse-truth.md`: `AdzeDocument` is the
  canonical parse product; typed CST, typed AST, diagnostics, GLR summaries, and
  Tree-sitter compatibility are projections.
- `ADZE-ADR-0002-no-durable-unpublished-production-crates.md`: production
  workspace packages are either published public surfaces or temporary
  migration targets that must move into SRP owner submodules before release.
- `ADZE-ADR-0003-summary-first-glr-ambiguity.md`: native GLR output starts with
  user-facing ambiguity summaries and selection reasons before raw forest
  internals become a public contract.

ADRs record architecture decisions. They should not own PR sequencing or policy
exception lists.

## Implementation Campaign Shape

The campaign should proceed through small PRs:

1. Define the source-of-truth scaffolding for proposals, specs, ADRs, plans, and
   active goal manifests.
2. Add this 0.9 contract-convergence proposal.
3. Add the package surface boundary spec.
4. Add the CI economics spec and link it to existing CI policy docs.
5. Record the AdzeDocument one-parse-truth ADR.
6. Add the 0.9 implementation plan and active goal manifest.
7. Implement package-boundary audit tooling and policy.
8. Collapse or reclassify microcrates in owner-sized batches.
9. Apply Rust/MSRV and lint policy updates after the package graph is smaller.
10. Refresh CI whitelist, LEM estimates, support tiers, and stable claim proof.
11. Close the campaign with a handoff that records evidence and remaining work.

Each implementation PR should name its linked proposal, spec, ADR if any, proof
commands, policy impact, rollback path, and support-tier impact.

## Evidence Plan

Evidence for this proposal comes from linked specs, plans, policy ledgers, and
support-tier rows. The expected proof surface includes:

```bash
cargo metadata --format-version 1 --no-deps
cargo run -q -p xtask -- check-package-boundary
cargo run -q -p xtask -- check-ci-lane-whitelist
cargo run -q -p xtask -- check-lint-policy
just check-msrv
just ci-supported
```

Product proof should continue to use exact canaries listed in
`../status/SUPPORT_TIERS.md`, including typed extraction, pure-Rust parser,
structured parse errors, GLR conflict routing, tablegen ABI, `AdzeDocument`,
typed CST, and Tree-sitter compatibility rows at their current tiers.

CI economics proof should link to `../../policy/ci-lane-whitelist.toml`,
`../../policy/ci-risk-packs.toml`, `../ci/cost-and-verification-policy.md`, and
`../ci/learned-estimates.md` instead of duplicating those contents here.

## Risks

- Package collapse can accidentally remove useful isolation if the audit treats
  every small crate as inherently wasteful.
- CI economics work can be mistaken for weaker verification if support-tier
  proof is not kept explicit.
- MSRV and lint changes can churn too many files if they happen before the
  package graph is reduced.
- Product claims can drift if README updates are not tied to support-tier proof.
- Native API design can sprawl if `AdzeDocument`, typed CST, and GLR output are
  implemented without specs and ADRs.

## Non-Goals

This proposal does not:

- implement parser or runtime behavior;
- stabilize runtime2, WASM, Tree-sitter compatibility, CLI output, or broader
  grammar crates by declaration;
- define the full `AdzeDocument` API contract;
- define the full typed CST API;
- implement query support;
- replace support tiers, policy TOMLs, or CI docs as their own sources of truth;
- prescribe every PR in the campaign beyond the high-level sequence.

## Exit Criteria

This proposal is implemented when:

- the linked package-boundary, CI-economics, canonical-document, and product-proof
  specs exist or are explicitly superseded;
- required ADRs for one parse truth and package-boundary policy are accepted or
  explicitly superseded;
- `plans/0.9.0/implementation-plan.md` and `.adze/goals/active.toml` reflect the
  active campaign state;
- package-boundary and CI-policy checks have concrete commands or tracked
  follow-up items;
- `../status/SUPPORT_TIERS.md` remains the feature-claim proof map;
- a closeout or handoff records what landed, which proof commands passed, and
  what remains outside 0.9.

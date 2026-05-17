# ADZE-SPEC-0011: Product proof and support tiers

Status: accepted
Owner: release/product
Created: 2026-05-14
Linked proposal: ../proposals/ADZE-PROP-0001-0.9-contract-convergence.md
Linked ADRs:
Linked plan: ../../plans/0.9.0/implementation-plan.md
Linked issues:
Linked PRs:
Support-tier impact: ../status/SUPPORT_TIERS.md
Policy impact: ../../scripts/ci-product-stable.sh

## Problem

Adze's README is the public front door, but feature claims can drift from the
proof that actually protects users. The 0.9 release needs an explicit contract
for Stable claims so users, maintainers, and agents can tell which advertised
surfaces are proven, which are still developing, and which are only advisory.

## Behavior

### B1. `SUPPORT_TIERS.md` is the product claim proof map

`../status/SUPPORT_TIERS.md` is the source of truth for feature tiers, proof
commands, and CI lanes. README capability tables may summarize those rows, but
they must not become an independent proof map.

### B2. Stable README claims require proof

Every README row marked **Stable** must map to:

- a row in `../status/SUPPORT_TIERS.md`;
- at least one repeatable proof command;
- a lane that runs the proof or a documented reason it remains advisory.

### B3. Stable-product canaries are explicit

`../../scripts/ci-product-stable.sh` owns the bounded stable-product canary lane
for README Stable claims. That lane must include the README proof-alignment
canary:

```bash
cargo test -p adze-cli readme_stable_claims_are_in_stable_product_lane -- --exact --nocapture
```

The canary must fail when a README Stable proof command is missing from
`SUPPORT_TIERS.md` or from the stable-product lane.

### B4. Stable-product canaries do not replace the required gate

`just ci-supported` / `CI / ci-supported` remains the required supported gate.
The stable-product lane is advisory until branch protection explicitly promotes
it. It should still run on PRs that touch README Stable-claim surfaces, support
tier proof mapping, quickstart canaries, or the stable-product canary script.

### B5. Tier promotion is evidence-based

Surfaces outside the Stable rows must not be promoted by wording alone. GLR
conflict routing, structured parse errors, `AdzeDocument`, typed CST, CLI,
WASM, Tree-sitter compatibility, runtime2, grammars, golden tests, and
benchmarks remain at their documented tiers until their support-tier rows and
proof commands justify promotion.

## Non-Goals

- No promotion of Stabilizing, Experimental, Advisory, or intentionally
  excluded surfaces.
- No requirement that broad product-proof advisory lanes become branch
  protection gates.
- No duplication of the full support-tier table in this spec.
- No runtime behavior change.

## Required Evidence

- README Stable proof-alignment canary passes.
- Stable product lane passes.
- Required supported gate remains green.
- `SUPPORT_TIERS.md` and README agree about Stable rows.

## Acceptance Examples

Accepted:

```text
README marks Typed extraction Stable.
SUPPORT_TIERS.md has a Typed extraction row with proof commands.
scripts/ci-product-stable.sh runs the stable canaries that cover the claim.
```

Rejected:

```text
README marks a surface Stable, but the proof command only appears in prose or
in an optional broad workflow with no support-tier row.
```

Rejected:

```text
README promotes Tree-sitter compatibility from Advisory to Stable without a
support-tier promotion and parity proof commands.
```

## Test Mapping

```text
cli/tests/readme_quickstart.rs::readme_stable_claims_are_in_stable_product_lane
scripts/ci-product-stable.sh
```

## Implementation Mapping

```text
README.md
docs/status/SUPPORT_TIERS.md
docs/status/KNOWN_RED.md
scripts/ci-product-stable.sh
.github/workflows/product-proof.yml
```

## CI Proof

```bash
just ci-product-stable
just ci-supported
```

Focused proof:

```bash
cargo test -p adze-cli readme_stable_claims_are_in_stable_product_lane -- --exact --nocapture
```

## Metrics And Promotion Rule

The 0.9 product-proof release blocker is complete when README Stable claims are
covered by `SUPPORT_TIERS.md`, `readme_stable_claims_are_in_stable_product_lane`
passes, `just ci-product-stable` passes, and `CI / ci-supported` remains green
on the closeout PR.

Future promotion of `ci-product stable canaries` from advisory to required needs
a separate CI policy change and branch-protection update.

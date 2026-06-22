# ADZE-ADR-0006: User-Defined Pure-Rust Grammar Is the Stable Product

Status: accepted
Date: 2026-06-22
Owner: release/product
Linked proposal: ../proposals/ADZE-PROP-0003-glr-toolkit-productization.md
Linked specs: ../specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
Linked plan: ../../plans/glr-toolkit/productization-plan.md

## Context

Adze ships five grammar crates under `grammars/` (python, javascript, go,
python-simple, test-vec-wrapper). These are documented as "Advisory" in
`docs/status/SUPPORT_TIERS.md` and described as "reference implementations
and integration fixtures" in `docs/reference/language-support.md`.

The product promise (stated in `docs/proposals/ADZE-PROP-0003` and
`grammars/README.md`) is that Adze lets Rust developers define a grammar as
Rust types, generate a parser at build time, and parse into typed Rust values
through the `adze` runtime. The stable product contract is the generated
pure-Rust parser path for user-defined grammars — not bundled language packs.

However, this product direction was stated only in prose (SUPPORT_TIERS.md,
language-support.md, grammars/README.md) and not recorded as a durable
architecture decision. This ADR promotes the prose decision to an ADR so it
survives support-tier doc churn.

## Decision

The stable product contract for Adze is the **user-defined pure-Rust grammar
path**: a developer writes `#[adze::grammar]` Rust types, `adze-tool`
generates a parser at build time, and the `adze` runtime parses input into
typed Rust values. This is the surface that README claims, CI proves, and
users depend on.

The bundled grammar crates (`grammars/*`) are **advisory fixtures** —
valuable examples and integration surfaces, but not a stable published
language-pack guarantee. They will not be promoted to Stable without an
explicit support-tier row, proof commands, and a new ADR.

## Consequences

- `grammars/*` crates remain `dev-only` / `Advisory` per
  `policy/package-boundary.toml` and `SUPPORT_TIERS.md` until explicitly
  promoted.
- Grammar completeness work (fixing Python triple-quotes, Go declaration
  parsing, etc.) is **not** required for the 0.9 release. It is valuable for
  demonstrating the pipeline but does not block release.
- The `golden-tests` crate's role is advisory parity signal, not a required
  gate.
- If a future decision promotes a grammar to Stable (e.g. `adze-python`
  becomes a supported language pack), that requires: (a) a new ADR, (b) a
  SUPPORT_TIERS row with proof commands, (c) a `category = "published"`
  reclassification in `package-boundary.toml`.

## Non-Goals

- This ADR does not decide whether grammar crates should be **deleted**.
  They serve as advisory examples and integration fixtures; deletion is a
  separate scope decision.
- This ADR does not change the GLR toolkit product contract
  (`ADZE-SPEC-0012`) — that governs the generated-parser path, which this
  ADR confirms as the stable product.

# ADZE-PROP-0007: External Scanner Recovery Hardening

Status: accepted
Owner: runtime/diagnostics
Created: 2026-05-20
Target milestone: post-0.9 / product trust
Linked specs:
- docs/specs/ADZE-SPEC-0005-diagnostics-and-recovery.md
- docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
- docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
Linked ADRs:
- ADZE-ADR-0001-adze-document-one-parse-truth
Linked plan:
- ../../plans/external-scanner-recovery/implementation-plan.md
Linked issues:
- none
Linked PRs:
- none yet
Support-tier impact:
- Narrows the External scanners Experimental gap and Structured parse errors
  Stabilizing gap without promoting either surface by setup alone.
Policy impact:
- Keeps all implementation and proof work in `EffortlessMetrics/adze-swarm`.
- Does not authorize release, tag, publish, signing, Cargo-token, or crates.io
  install receipt work.

## Problem

The product objective audit still records a non-release product-trust gap:
broader parser-generated external-scanner recovery coverage remains future
work. Existing receipts prove focused parser-v4 external-scanner dispatch,
invalid-token rejection, and a generated external-token bad-input matrix, but
they do not yet justify support-tier promotion or a broad stable scanner API
claim.

This lane exists to burn down that gap with focused generated-parser and
document-diagnostic proof instead of treating release/publish blockers as
routine swarm work.

## Users And Surfaces

- Grammar authors need external-token grammars to fail with useful spans,
  expected-token names, and document error facts.
- Tooling authors need `parse_document()` diagnostics and selected-tree error
  facts to stay consistent with generated `parse()` errors.
- Maintainers need a bounded proof path before any support-tier promotion is
  considered.

Affected surfaces:

- generated external-token parser examples;
- parser-v4 external-scanner dispatch canaries;
- `parse()` / `parse_document()` diagnostic agreement;
- `docs/status/SUPPORT_TIERS.md`;
- `docs/status/PRODUCT_OBJECTIVE_AUDIT.md`.

## Success Criteria

- A fresh active goal names this non-release lane in `adze-swarm`.
- Generated external-token recovery tests cover additional EOF, line-boundary,
  and nested/body malformed-input shapes.
- Direct parser-v4 external-scanner diagnostic canaries assert useful document
  error facts, ordered byte spans, matching point ranges, and public expected
  names where available.
- Support-tier and product-audit wording is refreshed only after proof exists.
- External scanners remain Experimental unless a later support-tier review
  explicitly promotes a proven slice.

## Proposed Shape

Work in small PRs:

```text
source-of-truth setup
  -> generated external-token recovery matrix expansion
    -> parser-v4 external diagnostic-detail canaries
      -> support-tier and product-audit boundary refresh
```

The work should favor existing generated examples and focused parser-v4 tests
over broad runtime rewrites.

## Alternatives Considered

### Start Release Work

Rejected. Release, publish, signing, Cargo-token, and crates.io install receipt
work still requires explicit human authorization and belongs in public `adze`.

### Promote External Scanners Now

Rejected. The current proof is useful but still explicitly Experimental in
`SUPPORT_TIERS.md`.

### Add A Broad Scanner Rewrite

Rejected. The next product-trust gap is proof coverage, not a runtime rewrite.

## Specs To Create Or Update

No new behavior spec is required at campaign start. Existing diagnostics,
product proof, and toolkit product specs own the behavior contract.

Update specs only if the proof work exposes a new durable behavior contract.

## Architecture Decisions Needed

No new ADR is required at campaign start. The durable rule remains:
`AdzeDocument` is the one parse truth, and diagnostics are document facts.

## Implementation Campaign Shape

1. Start the external-scanner recovery active goal.
2. Expand the generated external-token malformed-input matrix.
3. Harden parser-v4 external-scanner document-diagnostic canaries.
4. Refresh support-tier and product-audit wording only after the proof commands
   exist and pass.

## Evidence Plan

Focused proof:

```bash
cargo test --manifest-path example/Cargo.toml external_word_example::tests::generated_external_grammar_bad_input_matrix_returns_diagnostic_document --features pure-rust -- --exact --nocapture
cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests::test_external_scanner_parse_document_bad_input_returns_diagnostic_document -- --exact --nocapture
cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests -- --nocapture
```

Source-of-truth proof:

```bash
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

## Risks

- Tests can imply a broader stable scanner API than support tiers allow.
- Generated example coverage can accidentally become a release claim.
- Parser-v4 and generated-parser surfaces can drift if proof commands are not
  kept in the support ledger.

## Non-Goals

- No support-tier promotion by setup PR.
- No full external-scanner stability claim.
- No raw scanner API redesign.
- No release, tag, publish, signing, Cargo-token, or crates.io install work.
- No public `adze` implementation PRs.

## Exit Criteria

The lane can close when the generated and parser-v4 external-scanner recovery
proof commands are current, the product objective audit no longer lists this
specific coverage gap as unaddressed, and support tiers either keep the surface
Experimental with a narrower gap or promote a proven slice with explicit proof.

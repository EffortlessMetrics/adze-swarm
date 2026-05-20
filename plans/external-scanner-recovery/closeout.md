# External Scanner Recovery Hardening Closeout

Status: complete
Owner: runtime/diagnostics
Closed: 2026-05-20
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/external-scanner-recovery-hardening.toml
Plan: ./implementation-plan.md
Proposal: ../../docs/proposals/ADZE-PROP-0007-external-scanner-recovery-hardening.md

## Outcome

Outcome: **complete; no support-tier promotion and no release authorization
implied**.

This campaign narrowed the external-scanner recovery proof gap while keeping
work in `EffortlessMetrics/adze-swarm`. Public `EffortlessMetrics/adze` remains
the release, public-intake, tag, signing, and publish surface.

## Landed Work

| Work item | PRs | Result |
| --- | --- | --- |
| Source-of-truth setup | #358 | Opened the focused non-release external-scanner recovery proposal, plan, active goal, and artifact registration. |
| Generated recovery matrix | #359 | Added empty-source, whitespace-only, missing-condition, multibyte body-token, CRLF boundary, and nested invalid-expression cases to the generated external-token recovery matrix. |
| Parser-v4 diagnostic detail | #360 | Added canaries for rejected-token input-position safety and rendered document diagnostics for direct parser-v4 external-scanner bad input. |
| Support-tier boundary refresh | final closeout PR | Refreshed support-tier and product-audit wording without promoting external scanners beyond Experimental. |

## Proof Receipts

Representative proof commands from the campaign:

```bash
cargo test --manifest-path example/Cargo.toml external_word_example::tests::generated_external_grammar_bad_input_matrix_returns_diagnostic_document --features pure-rust -- --exact --nocapture
cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests::test_external_scanner_parse_document_bad_input_returns_diagnostic_document -- --exact --nocapture
cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests::test_external_scanner_rejects_token_not_in_valid_symbols -- --exact --nocapture
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

GitHub receipts across the campaign included `Rust Small Result`, Source of
Truth, CI Lane Whitelist, GLR Invariants, PR Gate Success, Coverage Lite, and
path-routed runtime/product receipts where relevant.

## Claim Boundaries

This closeout does not claim:

- external scanners are Stable;
- full parser-generated external-scanner recovery is complete;
- every external-scanner grammar shape has recovery parity;
- release, tag, publish, signing, Cargo-token, or crates.io install work was
  authorized or performed;
- public `EffortlessMetrics/adze` is the swarm working repo.

## Next Step

No ready routine work remains in this campaign. Future non-release work should
open a fresh active goal in `adze-swarm`. Release/publish work remains blocked
until explicit human authorization and must execute from public
`EffortlessMetrics/adze`.

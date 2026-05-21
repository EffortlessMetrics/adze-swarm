# Parser Recovery Real-Grammar Coverage Closeout

Status: complete
Owner: runtime/diagnostics
Closed: 2026-05-20
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/parser-recovery-real-grammar.toml
Plan: ./implementation-plan.md
Proposal: ../../docs/proposals/ADZE-PROP-0009-parser-recovery-real-grammar-coverage.md

## Outcome

Outcome: **complete; no external-scanner promotion and no release
authorization implied**.

This campaign narrowed the remaining real-grammar external-scanner recovery
proof gap while keeping work in `EffortlessMetrics/adze-swarm`. Public
`EffortlessMetrics/adze` remains the release, public-intake, tag, signing, and
publish surface.

## Landed Work

| Work item | PRs | Result |
| --- | --- | --- |
| Source-of-truth setup | #376 | Opened the focused non-release parser recovery proposal, plan, active goal, and artifact registration. |
| Generated external grammar matrix | #377, #378 | Expanded the generated external-token recovery matrix and followed up with formatting-only hygiene. |
| Parser-v4 scanner smoke | #379 | Expanded direct parser-v4 scanner diagnostic-document smoke across invalid root, UTF-8, newline, extra newline, and CRLF shapes. |
| Support-tier boundary refresh | final closeout PR | Refreshed support-tier and product-audit wording without promoting external scanners beyond Experimental. |

## Proof Receipts

Representative proof commands from the campaign:

```bash
cargo test --manifest-path example/Cargo.toml external_word_example::tests::generated_external_grammar_bad_input_matrix_returns_diagnostic_document --features pure-rust -- --exact --nocapture
cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests::test_external_scanner_parse_document_bad_input_returns_diagnostic_document -- --exact --nocapture
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

GitHub receipts across the campaign included `Rust Small Result`, Source of
Truth, CI Lane Whitelist, GLR Invariants, PR Gate, Coverage Lite,
`ci-product stable canaries`, Golden-Master smoke, and path-routed
runtime/product receipts where relevant.

## Claim Boundaries

This closeout does not claim:

- external scanners are Stable;
- corpus-wide external-scanner recovery parity is complete;
- every real grammar or imported grammar shape has recovery parity;
- a stable public external-scanner API exists;
- release, tag, publish, signing, Cargo-token, or crates.io install work was
  authorized or performed;
- public `EffortlessMetrics/adze` is the swarm working repo.

## Next Step

No ready routine work remains in this campaign. Future non-release work should
open a fresh active goal in `adze-swarm`. Release/publish work remains blocked
until explicit human authorization and must execute from public
`EffortlessMetrics/adze`.

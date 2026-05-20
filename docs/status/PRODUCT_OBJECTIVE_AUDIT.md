# Product Objective Audit

**Last updated:** 2026-05-19
**Status:** incomplete; use this as an audit checklist, not as a support-tier
promotion. The active execution lane is
[`../../plans/product-gap-burn-down/implementation-plan.md`](../../plans/product-gap-burn-down/implementation-plan.md).
**Source of truth:** [`SUPPORT_TIERS.md`](./SUPPORT_TIERS.md) remains the
authoritative support-tier ledger.

This audit maps the current product objective to concrete repo evidence. It is
intentionally stricter than "the manifest is complete" or "tests passed": every
claim below needs a file, command, CI receipt, or named gap.

## Objective Restated As Deliverables

Adze should be release-readable as a Rust parser generator where:

1. Rust types define the grammar and generated parsers return typed ASTs
   directly.
2. The quickstart works in a clean downstream crate without repo archaeology.
3. The core pure-Rust pipeline is green, bounded, and boring.
4. Tablegen emits valid tables.
5. GLR handles real conflicts honestly.
6. Typed extraction is deterministic.
7. Parse errors are useful instead of incidental.
8. Every Stable README claim maps to concrete proof.
9. Experimental or developing surfaces remain labeled unless promoted with
   receipts.
10. The advertised product works under ordinary user pressure and fails clearly.

## Prompt-To-Artifact Checklist

| Requirement | Evidence inspected | Current result | Gap / next action |
| --- | --- | --- | --- |
| Rust types define grammar and generated parsers return typed ASTs. | README core example; `SUPPORT_TIERS.md` Stable `Typed extraction` row; `PRODUCT_PROOF_MAP.md` Stable typed extraction claim. | Covered for the supported generated-parser contract. | Keep Stable claim limited to proof rows; do not broaden to every grammar shape. |
| Quickstart works in a clean downstream crate. | `testing/downstream-starter/`; `docs/product/ACCEPTANCE_MATRIX.md`; `SUPPORT_TIERS.md` Pure-Rust parser row. | Covered for path-dependency downstream wiring, generated starter shape, and local `adze-cli` package verification. | Published `cargo install adze-cli` is not proven until `adze-cli` is published and an install receipt exists. |
| Core pure-Rust pipeline is green, bounded, and boring. | `just ci-supported`; `Rust Small Result`; `KNOWN_RED.md` supported-lane description; `adze-swarm` PRs #284 and #285. | Covered as the required swarm gate plus local supported proof. PR #284 bounds the broad Rust tail, and PR #285 scopes the default pure-rust PR test step to supported crates while keeping full workspace tests explicit through manual/full-ci. | `ci-product-stable` remains advisory until branch protection explicitly promotes it. |
| Tablegen emits valid tables. | `SUPPORT_TIERS.md` Tablegen `TSLanguage` ABI row; `PRODUCT_PROOF_MAP.md` tablegen ABI claim. | Stabilizing with compressed decode, field metadata, aliases, externals, lex modes, and conflict-cell proof. | Broader generated-language roundtrip and full Tree-sitter parity remain future work. |
| GLR handles real conflicts honestly. | `SUPPORT_TIERS.md` GLR conflict routing row; `docs/product/ACCEPTANCE_MATRIX.md` GLR ambiguity row. | Stabilizing with generated shift/reduce conflict preservation, generated reduce/reduce preservation and selected typed-AST extraction, dangling-else nearest-else selected typed AST proof, retained alternatives, deterministic selected output, ambiguity summaries, and no-panic bad-input guardrails. | Broader conflict-class coverage and any Stable GLR promotion still require support-tier proof review. |
| Typed extraction is deterministic. | `typed_ast_contract_left_associative_addition`; `typed_ast_contract_repeated_parse_is_deterministic`; `readme_arithmetic_quickstart_builds_and_runs`. | Covered for Stable typed extraction rows. | Keep determinism claims scoped to supported generated-parser shapes. |
| Parse errors are useful instead of incidental. | `SUPPORT_TIERS.md` Structured parse errors and External scanners rows; `PRODUCT_PROOF_MAP.md` parse-error claim; CLI recovery diagnostics proof. | Stabilizing with spans, excerpts, expected tokens, UTF-8, EOF, multiline, no-panic, generated-parser matrix canaries, and object-like `parse_document()`/JSON recovery proof for separator, UTF-8, multiline value, and multiline EOF cases. External-scanner dispatch now has focused parser-v4 proof for emitted-token byte spans/text, rejection of scanner tokens that are invalid in the parser state, direct parser-v4 `parse_document()` diagnostic-document behavior for bad input in an external-scanner grammar shape, and generated external-token grammar diagnostic-document behavior for malformed input. | Full parser-generated external-scanner recovery coverage remains future work; any Stable promotion still needs support-tier review. |
| Every Stable README claim maps to proof. | README capability table; `SUPPORT_TIERS.md`; `readme_stable_claims_are_in_stable_product_lane`; `scripts/ci-product-stable.sh`. | Covered by current proof map and stable-product canaries. | The stable-product lane is still advisory, not branch-protection required. |
| Experimental/developing surfaces are clearly labeled. | README capability table; `SUPPORT_TIERS.md`; `KNOWN_RED.md`; `PRODUCT_PROOF_MAP.md`. | Covered for runtime2, broader grammars, WASM, Tree-sitter interop, CLI, benchmarks, typed CST, incremental, and JSON. | Re-check after any README, support-tier, or release-facing wording change. |
| Product works under ordinary user pressure and fails clearly. | Downstream starter fixture; README/tutorial/book quickstart canaries; CLI document JSON recovery diagnostics. | Partially covered by local/downstream fixtures and CLI recovery smoke. | Published CLI install, public promotion, and any future crates.io release surface need fresh receipts. |

## Commands And Receipts

Current stable product receipts:

```bash
just ci-supported
just ci-product-stable
cargo test -p adze-cli cargo_install_adze_cli_claims_stay_release_surface_bounded -- --exact --nocapture
cargo test -p adze-cli readme_stable_claims_are_in_stable_product_lane -- --exact --nocapture
cargo test --manifest-path testing/downstream-starter/Cargo.toml
cargo run --manifest-path testing/downstream-starter/Cargo.toml --example parse
```

GitHub workflow dispatch
[`Product Proof` run 26104726428](https://github.com/EffortlessMetrics/adze-swarm/actions/runs/26104726428)
passed on 2026-05-19 from current `adze-swarm/main` after PR #281, commit
`0b79a36a`. The `ci-product stable canaries` job passed in 3m02s and the broad
advisory canaries skipped under the stable-only default. This is evidence for
the README Stable claim lane; it is not a branch-protection change.

Local receipt after residual product-trust PRs #295-#310: `just
ci-product-stable` passed on 2026-05-20 from `adze-swarm/main` at commit
`e965cba2`, and the refreshed public promotion PR later passed the hosted
`ci-product stable canaries` job. These receipts cover README stable proof
alignment, bounded published CLI install-claim wording, clean-room README/
Getting Started/book quickstarts, checked-in downstream demo, standalone
downstream starter fixture, typed AST determinism, operator precedence, and
core table serialization canaries. This remains advisory product proof, not a
branch-protection change.

Current CI-tail receipts:

- PR #284 bounded `pure-rust-ci` and `pr-gate` Rust tail steps so advisory
  Rust jobs fail clearly instead of hanging indefinitely.
- PR #285 scoped the default `pure-rust-ci` PR test step to the supported
  pipeline crates and kept full workspace tests available through
  `workflow_dispatch` / `full-ci`.
- On PR #285, `Rust Small Result` passed in 6s, `Supported Rust Gate` passed in
  22m56s, and `Test Pure Rust Implementation (ubuntu-latest, stable)` passed
  in 23m10s after running the scoped supported-crate test step.

Current structured parse-error receipts:

```bash
cargo test -p adze --features "pure-rust,glr,serialization,ts-compat" --test recovery_matrix generated_object_like_bad_input_matrix_preserves_document_diagnostics_and_json -- --exact --nocapture
cargo test -p adze --features "pure-rust,glr,serialization,ts-compat" --test recovery_matrix -- --nocapture
```

PR #293 added object-like `parse_document()` and document JSON recovery proof
for missing separators, multibyte invalid identifier continuations, multiline
invalid values, and multiline EOF. This narrows the invalid-span product gap;
it is not a support-tier promotion.

Current external-scanner dispatch receipts:

```bash
cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests::test_parser_with_external_scanner -- --exact --nocapture
cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests::test_external_scanner_rejects_token_not_in_valid_symbols -- --exact --nocapture
cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests::test_external_scanner_parse_document_bad_input_returns_diagnostic_document -- --exact --nocapture
cargo test --manifest-path example/Cargo.toml external_word_example::tests::generated_external_grammar_bad_input_returns_diagnostic_document --features pure-rust -- --exact --nocapture
cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests -- --nocapture
cargo test -p adze --features external_scanners
```

PR #298 fixed parser-v4 external scanner token spans so emitted tokens use the
pre-scan byte position and preserve token text. This closes the focused
dispatch/span gap for the parser-v4 scanner canaries. The follow-up parser-v4
document canary proves bad input in a direct external-scanner grammar shape
returns a diagnostic document with error facts. The generated external-token
example canary proves generated `parse_document()` returns a diagnostic document
with bounded byte spans and matching point ranges for malformed input. These
receipts do not close the full parser-generated external-scanner recovery gap,
promote external scanners out of Experimental, or create a stable public scanner
API claim.

Current release-surface readiness receipts:

```bash
just check-publishable
```

`just check-publishable` passed on 2026-05-19 from `adze-swarm/main` after
PR #253, again at commit `b613ebbb` after residual product-trust PRs
#295-#301, and again on 2026-05-20 at commit `e965cba2` after PRs #309-#310.
It verifies publish-order metadata and `cargo package --list` for
the core publish surface (`adze-common`, `adze-ir`, `adze-glr-core`,
`adze-tablegen`, `adze-macro`, `adze-tool`, `adze-cli`, and `adze`). This is
package metadata/file-list evidence only; it does not publish crates or prove
registry installation.

Current public promotion PR receipt:

Public `EffortlessMetrics/adze#795` supersedes closed/unmerged public PR #794.
It was opened from the explicit public promotion execution decision after the
promotion branch was refreshed to the current `adze-swarm/main` tree. Verify
the exact public head SHA live before review or merge because each accepted
`adze-swarm` refresh can advance the promotion source. The refreshed public
PR #795 check set passed on
2026-05-20, including `Rust Small Result`, `Supported Rust Gate`, `PR Gate
Success`, `Source of Truth`, `CI Lane Whitelist`, `GLR Invariants`, `Coverage
Lite`, `ci-product stable canaries`, `Test Core Crates`, `Test Runtime Crates`,
and `Test Pure Rust Implementation`.

This is a ready-for-review public promotion receipt, not a completed
promotion. PR #795 remains open, mergeable, not draft, and has squash
auto-merge enabled. The public merge state is blocked by the public `main`
branch requirement for one approving review, not by failed CI.

Current first-use / CLI boundary receipts:

```bash
cargo test -p adze-cli test_init -- --nocapture
cargo test -p adze-cli getting_started_quickstart_builds_parses_and_reports_diagnostics -- --exact --nocapture
cargo test -p adze-cli cargo_install_adze_cli_claims_stay_release_surface_bounded -- --exact --nocapture
just package-local adze-cli
cargo info adze-cli
cargo run -q -p xtask -- verify-crates-io-install adze-cli --bin adze --version X.Y.Z
```

`just package-local adze-cli` packages and verifies the CLI crate with local
patches for unpublished co-release crates. It passed on 2026-05-19 from
`adze-swarm`, producing and verifying `adze-cli v0.8.0-dev`. This is local
publish-readiness evidence, not a crates.io install receipt.

`cargo_install_adze_cli_claims_stay_release_surface_bounded` keeps live
beginner/status/spec docs from presenting `cargo install adze-cli` as a ready
quickstart until a crates.io receipt exists. It is a claim-boundary canary, not
registry installation proof.

The `cargo info adze-cli` command must be run outside the workspace when it is
used to verify registry publication. On 2026-05-19 it reported that `adze-cli`
could not be found in crates.io, so `cargo install adze-cli` remains a
release-surface target rather than current product proof.

`cargo run -q -p xtask -- verify-crates-io-install adze-cli --bin adze --version
X.Y.Z` is the post-publish receipt hook for the missing crates.io install proof.
It installs from crates.io into an isolated temporary root and runs
`adze --version`. The `--dry-run` mode is pre-publish command-shape evidence
only; it does not contact crates.io and does not close the install-receipt gap.

## Current Non-Completion Reasons

Do not mark the product objective complete while any of these are true:

- `cargo install adze-cli` has no crates.io install receipt.
- `ci-product-stable` is advisory and not a required branch-protection gate.
- Full parser-generated external-scanner recovery coverage remains future work.
- Public promotion has not happened. Public PR #795 is open, refreshed through
  the latest source-side example-crate proof fix, green, and auto-merge-enabled, but
  it still requires one public approving review before it can merge. Public
  `EffortlessMetrics/adze` remains release/public-intake until promotion is
  accepted.
- GLR conflict routing, structured parse errors, Tree-sitter compatibility,
  query compatibility, CLI document output, and `AdzeDocument` are not all
  Stable; their current tiers and limitations are recorded in
  `SUPPORT_TIERS.md`.

## Next Concrete Actions

1. Review and approve public `EffortlessMetrics/adze#795`, or leave it parked
   as the explicit green promotion PR until review is available. If
   `adze-swarm/main` advances before review, refresh or supersede #795 again
   before merge. If it merges, record a promotion closeout and refresh
   public/main before any tag, publish, or release-workflow work.
2. Run the crates.io install receipt after publish and before any doc claims
   `cargo install adze-cli` as the supported quickstart. The current local
   package receipt and verifier dry run are publish-readiness evidence only.
3. Consider promoting `ci-product-stable` only after advisory receipts are
   consistently green and branch-protection policy is updated deliberately.

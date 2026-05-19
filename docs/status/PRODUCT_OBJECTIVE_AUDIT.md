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
| Parse errors are useful instead of incidental. | `SUPPORT_TIERS.md` Structured parse errors row; `PRODUCT_PROOF_MAP.md` parse-error claim; CLI recovery diagnostics proof. | Stabilizing with spans, excerpts, expected tokens, UTF-8, EOF, multiline, no-panic, and generated-parser matrix canaries. | Broader invalid-span and external-scanner recovery coverage remain future work. |
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

Current CI-tail receipts:

- PR #284 bounded `pure-rust-ci` and `pr-gate` Rust tail steps so advisory
  Rust jobs fail clearly instead of hanging indefinitely.
- PR #285 scoped the default `pure-rust-ci` PR test step to the supported
  pipeline crates and kept full workspace tests available through
  `workflow_dispatch` / `full-ci`.
- On PR #285, `Rust Small Result` passed in 6s, `Supported Rust Gate` passed in
  22m56s, and `Test Pure Rust Implementation (ubuntu-latest, stable)` passed
  in 23m10s after running the scoped supported-crate test step.

Current release-surface readiness receipts:

```bash
just check-publishable
```

`just check-publishable` passed on 2026-05-19 from `adze-swarm/main` after
PR #253. It verifies publish-order metadata and `cargo package --list` for the
core publish surface (`adze-common`, `adze-ir`, `adze-glr-core`,
`adze-tablegen`, `adze-macro`, `adze-tool`, `adze-cli`, and `adze`). This is
package metadata/file-list evidence only; it does not publish crates or prove
registry installation.

Current public promotion PR receipt:

Public `EffortlessMetrics/adze#794` was opened from the explicit public
promotion execution decision and refreshed after source-side fixes landed in
`adze-swarm` #290 and #291. On 2026-05-19, public PR #794 at commit
`2550b21f30e49956e0d44ca56b6bbcdee79749fd` passed the refreshed public check
set, including `Rust Small Result`, `Supported Rust Gate`, `PR Gate Success`,
`Source of Truth`, `CI Lane Whitelist`, `GLR Invariants`, `Coverage Lite`,
`ci-product stable canaries`, `Test Core Crates`, `Test Runtime Crates`, and
`Test Pure Rust Implementation`.

This is a ready-for-manual-review public promotion receipt, not a completed
promotion. PR #794 remains open, mergeable, and not draft; auto-merge is not
enabled, and the public merge state is blocked by normal public review/merge
controls rather than failed CI.

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
- Public promotion has not happened. Public PR #794 is open and green, but it
  has not been merged, and public `EffortlessMetrics/adze` remains
  release/public-intake until promotion is accepted.
- GLR conflict routing, structured parse errors, Tree-sitter compatibility,
  query compatibility, CLI document output, and `AdzeDocument` are not all
  Stable; their current tiers and limitations are recorded in
  `SUPPORT_TIERS.md`.

## Next Concrete Actions

1. Review and either merge, close, or supersede public
   `EffortlessMetrics/adze#794`. If it merges, record a promotion closeout and
   refresh public/main before any tag, publish, or release-workflow work.
2. Run the crates.io install receipt after publish and before any doc claims
   `cargo install adze-cli` as the supported quickstart. The current local
   package receipt and verifier dry run are publish-readiness evidence only.
3. Consider promoting `ci-product-stable` only after advisory receipts are
   consistently green and branch-protection policy is updated deliberately.

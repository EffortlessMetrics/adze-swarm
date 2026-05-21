# Supported Surface Maintainability Audit

Status: complete
Owner: runtime/product
Date: 2026-05-21
Linked proposal: ../../docs/proposals/ADZE-PROP-0012-parser-runtime-maintainability-hardening.md
Linked plan: ./implementation-plan.md
Linked goal: ../../.adze/goals/active.toml

## Objective Restatement

Adze's release-readable product surface should be a trustworthy Rust parser
generator: Rust types define the grammar, generated parsers return typed ASTs
directly, the quickstart works in a clean downstream crate, core pure-Rust
parser/tablegen/GLR behavior is green and bounded, parse errors are useful, and
stable README claims map to concrete proof lanes. Developing surfaces must stay
clearly tiered unless promoted with receipts.

This audit is a non-release maintainability checkpoint. It does not authorize
tagging, publishing, signing, Cargo-token work, or a crates.io install claim.

## Prompt-To-Artifact Checklist

| Requirement | Evidence inspected | Result |
|---|---|---|
| Work happens in `adze-swarm`, not public `adze` | `git status --short --branch`; `gh pr list --repo EffortlessMetrics/adze-swarm --state open`; `gh pr list --repo EffortlessMetrics/adze --state open` | Local main is synced to `adze-swarm/main`; both live PR queues were empty after #449 merged. |
| Rust types define the grammar and generated parsers return typed ASTs | `just ci-product-stable`; README stable row for Typed extraction; `testing/downstream-starter/src/lib.rs`; `samples/downstream-demo` | Stable canaries passed, including typed AST exact value and repeated parse determinism. |
| Quickstart works in a clean downstream crate without repo archaeology | `just ci-product-stable`; `cargo test --manifest-path testing/downstream-starter/Cargo.toml`; `cargo run --manifest-path testing/downstream-starter/Cargo.toml --example parse` | Passed through the stable product lane. The fixture covers build dependency wiring, typed AST parse, bad-input diagnostics, and recovered document diagnostics. |
| Core pure-Rust pipeline is green, bounded, and boring | `CARGO_PROFILE_TEST_DEBUG=0 just ci-supported`; GitHub `Supported Rust Gate` on #449 | Passed locally on Windows and remotely on GitHub. |
| Tablegen emits valid tables and rejects invalid metadata cleanly | PRs #444, #445, #446; `cargo test -p adze-tablegen generate -- --nocapture`; `cargo test -p adze-tablegen validation -- --nocapture`; `just ci-supported` | Recent focused hardening landed NUL-name, null field-name, and zero-symbol metadata guards; supported gate remains green. |
| GLR handles real conflicts honestly | `docs/status/SUPPORT_TIERS.md`; `just ci-product-stable`; existing GLR conflict-routing row | Stabilizing, not Stable. Existing proof includes generated conflict canaries and operator-precedence stable proof. Broader GLR Stable promotion remains future review. |
| Typed extraction is deterministic | `just ci-product-stable` typed extraction exact value and repeated-parse determinism canaries | Passed. |
| Parse errors are useful, not incidental | `just ci-product-stable`; downstream starter bad-input diagnostics; `docs/status/SUPPORT_TIERS.md` structured parse errors row | Stable path proves user-facing bad-input spans and expected tokens for quickstart/downstream shapes. Broader structured parse errors remain Stabilizing. |
| Every stable README claim maps to concrete CI proof | `cargo test -p adze-cli readme_stable_claims_are_in_stable_product_lane -- --exact --nocapture`; `scripts/ci-product-stable.sh`; `docs/status/SUPPORT_TIERS.md` | Passed through `just ci-product-stable`. |
| Experimental surfaces are promoted with receipts or clearly labeled developing | `docs/status/SUPPORT_TIERS.md`; `README.md`; `cargo test -p adze-cli cargo_install_adze_cli_claims_stay_release_surface_bounded -- --exact --nocapture` | Passed claim-boundary canaries. runtime2, broader grammars, WASM, Tree-sitter interop, CLI, benchmarks, external scanners, and incremental parsing remain explicitly non-Stable where appropriate. |
| Release/publish boundary remains blocked without authorization | `docs/reference/PUBLISH_CHECKLIST.md`; `README.md`; `docs/status/SUPPORT_TIERS.md`; active goal handoff | No release, publish, signing, Cargo-token, or crates.io install work occurred. `cargo install adze-cli` remains bounded until a real crates.io receipt exists. |

## Proof Run

Commands run from `C:\Code\Rust2\adze-swarm` on 2026-05-21:

```bash
just ci-product-stable
CARGO_PROFILE_TEST_DEBUG=0 just ci-supported
```

Both commands passed. The stable product lane included README claim alignment,
claim-boundary canaries, typed extraction determinism, README/tutorial/book
clean-room quickstarts, checked-in downstream sample tests and binary run,
standalone downstream starter tests and binary run, operator precedence, and
core parse-table serialization proof.

The supported lane passed formatting, clippy, tests for the supported core
crates, and `adze-glr-core` serialization doctests with
`CARGO_PROFILE_TEST_DEBUG=0`.

## Maintainability Decisions

- Treat the supported product path as currently green.
- Do not reopen product-proof closeout work unless a material proof or claim
  changes.
- Keep focused hardening work limited to parser/runtime/tablegen surfaces with
  one edited surface family per PR.
- Keep release execution and crates.io install receipts blocked on explicit
  human authorization through tracker #325.

## Follow-Up Queue

The next work item is focused runtime hardening. Candidate PRs should be chosen
only when they improve one of these currently tiered surfaces:

1. tablegen/ABI validation guardrails discovered by real malformed metadata or
   decode edge cases;
2. generated parser diagnostics where a user-facing error span, expected-token
   set, or recovery document is weakly covered;
3. GLR conflict-routing canaries that close a named support-tier limitation;
4. document/projection consistency checks where one parse truth could drift.

Do not start broad SRP cleanup, release work, or support-tier promotion from
this audit alone.

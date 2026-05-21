# Parser Runtime Maintainability Hardening Closeout

Status: complete
Owner: runtime/product
Closed: 2026-05-21
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/parser-runtime-maintainability-hardening.toml
Plan: ./implementation-plan.md
Proposal: ../../docs/proposals/ADZE-PROP-0012-parser-runtime-maintainability-hardening.md

## Outcome

Outcome: **complete; non-release hardening closed with release/publish still
blocked on explicit authorization**.

This lane kept normal implementation and proof work in
`EffortlessMetrics/adze-swarm`. Public `EffortlessMetrics/adze` remains the
release, public-intake, promotion, tag, publish, signing, and Cargo-token
surface.

## Objective Restatement

The objective for this maintenance lane was to keep the already proven parser
generator product surface trustworthy: Rust types define the grammar, generated
parsers return typed ASTs directly, quickstarts build in clean downstream
crates, the supported pure-Rust pipeline stays green and bounded, tablegen/GLR
proofs do not drift, parse errors remain useful, and Stable README claims stay
mapped to concrete proof lanes while developing surfaces remain tiered.

## Prompt-To-Artifact Checklist

| Requirement | Evidence inspected | Result |
| --- | --- | --- |
| Work remains in `adze-swarm` | `git status --short --branch`; `gh pr list --repo EffortlessMetrics/adze-swarm --state open`; `gh pr list --repo EffortlessMetrics/adze --state open` | Local checkout is synced to `adze-swarm/main`; both checked queues were empty after #454 merged. |
| Rust types define grammars and generated parsers return typed ASTs | `scripts/ci-product-stable.sh`; README Stable proof table; `testing/downstream-starter` | Stable proof lane still names typed extraction, quickstart, downstream demo, and standalone downstream starter receipts. |
| Clean downstream quickstart works | `cargo test --manifest-path testing/downstream-starter/Cargo.toml`; `cargo run --manifest-path testing/downstream-starter/Cargo.toml --example parse`; `just ci-product-stable` from the maintainability audit | Covered by stable canaries and the checked-in downstream starter fixture; crates.io install remains a release-only claim. |
| Supported pure-Rust pipeline stays green and bounded | `CARGO_PROFILE_TEST_DEBUG=0 just ci-supported`; GitHub `Supported Rust Gate` on #454 | The supported gate passed locally in the audit and remotely on #454. |
| Tablegen emits valid tables and rejects invalid metadata cleanly | #444, #445, #446, #451, #452, #453, #454; `cargo test -p adze-tablegen -- --test-threads=2`; Microcrate `Test Core Crates (ir, glr-core, tablegen)` on #454 | Focused hardening closed NUL-name, null field-name, zero-symbol metadata, exact field-name count, generated ABI arrays, generated API reads, property/static fixtures, and validation fixtures. |
| GLR handling remains honestly tiered | `docs/status/SUPPORT_TIERS.md`; `docs/product/ACCEPTANCE_MATRIX.md`; stable operator-precedence canaries | GLR conflict routing remains tiered by support docs; this lane did not promote GLR claims. |
| Typed extraction is deterministic | `scripts/ci-product-stable.sh` typed extraction exact-value and repeated-parse determinism entries | Stable proof lane remains mapped to concrete canaries. |
| Parse errors are useful | `testing/downstream-starter/tests/parser.rs`; `docs/status/SUPPORT_TIERS.md`; `scripts/ci-product-stable.sh` quickstart/downstream entries | User-facing bad-input diagnostics stay covered by stable product and downstream starter proof. |
| Stable README claims map to CI proof | `cargo test -p adze-cli readme_stable_claims_are_in_stable_product_lane -- --exact --nocapture`; `scripts/ci-product-stable.sh`; `README.md` | Stable proof alignment remains explicit; no new Stable claim was added in this lane. |
| Developing surfaces remain clearly tiered | `docs/status/SUPPORT_TIERS.md`; `README.md`; `docs/reference/known-limitations.md` | runtime2, broader grammar crates, WASM, Tree-sitter interop, CLI, benchmarks, external scanners, and incremental parsing remain non-Stable unless proof rows say otherwise. |
| Release/publish boundary stays blocked | `.adze/goals/active.toml`; `docs/reference/PUBLISH_CHECKLIST.md`; tracker #325 references | No tag, publish, signing, Cargo-token, release workflow, public promotion, or crates.io install receipt work occurred. |

## Landed Work

| Work item | PRs | Result |
| --- | --- | --- |
| Source-of-truth setup | #443 | Replaced the paused release-boundary lane with a non-release parser/runtime maintainability lane. |
| Initial tablegen validation hardening | #444, #445, #446 | Added NUL-name, null field-name, and zero-symbol metadata guardrails. |
| CI linker/no-output friction mitigation | #447, #448 | Added supported-test heartbeat output and reduced pure-rust test linker pressure. |
| Rust Small routing capacity | #449 | Added CPX42-first Rust Small routing without changing the required gate name. |
| Maintainability audit | #450 | Recorded the supported-surface audit and follow-up queue. |
| Exact field-name ABI hardening | #451, #452, #453, #454 | Moved tablegen and tests to exact `field_names[0..field_count]` semantics and repaired stale integration/property fixtures found by Microcrate CI. |

## Proof Receipts

Representative proof commands from the lane:

```bash
just ci-product-stable
CARGO_PROFILE_TEST_DEBUG=0 just ci-supported
cargo test -p adze-tablegen -- --test-threads=2
cargo clippy -p adze-tablegen --all-targets -- -D warnings
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

Hosted receipts for #454 included `Rust Small Result`, `Supported Rust Gate`,
`Product Proof Result`, `Test Core Crates (ir, glr-core, tablegen)`, and
`Test Pure Rust Implementation (ubuntu-latest, stable)`.

## Claim Boundaries

This closeout does not claim:

- release, tag, publish, signing, Cargo-token, or crates.io install work was
  authorized or performed;
- `cargo install adze-cli` works from crates.io;
- Product Proof is branch-protection required;
- GLR, Tree-sitter compatibility, query, runtime2, WASM, benchmarks, external
  scanners, or incremental parsing are Stable beyond the current support-tier
  rows;
- public `EffortlessMetrics/adze` is the working repo for swarm development.

## Next Step

No ready routine work remains in this maintainability lane. Future non-release
work should open a fresh active goal in `adze-swarm` with one concrete product
or proof reason. Release/publish work remains blocked on explicit human
authorization tracked by #325 and must execute from public `EffortlessMetrics/adze`.

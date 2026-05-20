# Product Gap Burn-Down Plan

Status: active
Owner: runtime/product
Created: 2026-05-19
Linked proposal:
- ../../docs/proposals/ADZE-PROP-0003-glr-toolkit-productization.md
- ../../docs/proposals/ADZE-PROP-0004-toolkit-excellence.md
- ../../docs/proposals/ADZE-PROP-0005-release-promotion-readiness.md
Linked specs:
- ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
- ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
Linked ADRs:
- ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
- ../../docs/adr/ADZE-ADR-0003-summary-first-glr-ambiguity.md
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/product-gap-burn-down.toml
Support-tier map: ../../docs/status/SUPPORT_TIERS.md
Policy impact: no branch-protection, release, publish, or signing change in this plan

## Goal

Burn down the remaining blockers named in
`docs/status/PRODUCT_OBJECTIVE_AUDIT.md` without broadening public claims. This
plan owns the next execution queue after the completed toolkit excellence and
release-promotion readiness campaigns.

## Operating Rules

- Work in `EffortlessMetrics/adze-swarm`.
- Do not open public `EffortlessMetrics/adze` PRs from this lane.
- Keep public promotion separate and explicit.
- Do not claim `cargo install adze-cli` until a crates.io install receipt exists.
- Do not treat `ci-product-stable` as a required branch-protection gate until
  policy records that promotion.
- Do not promote GLR, Tree-sitter, query, CLI, or document API surfaces without
  support-tier rows and proof commands.

## Work Item: product-gap-burn-down-source-of-truth

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0004-toolkit-excellence.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks:
- stable-product-receipt-refresh
- dangling-else-selected-tree-gap
- generated-reduce-reduce-gap
Blocked by: n/a

### Goal

Replace the completed active manifest with a narrow active gap-burn-down queue
so agents can continue from current repo truth instead of stale completed
campaigns.

### Receipt

Landed in PR #263.

### Production Delta

Docs and source-of-truth metadata only.

### Non-Goals

- No runtime behavior change.
- No public promotion PR.
- No support-tier promotion.
- No branch-protection change.

### Acceptance

- `.adze/goals/active.toml` has `status = "active"`.
- The named goal exists at `.adze/goals/product-gap-burn-down.toml`.
- The plan names only currently known product blockers.
- The artifact ledger can parse and points to existing files.

### Proof Commands

```bash
python -c "import tomllib; tomllib.load(open('.adze/goals/active.toml', 'rb')); tomllib.load(open('.adze/goals/product-gap-burn-down.toml', 'rb')); tomllib.load(open('policy/doc-artifacts.toml', 'rb'))"
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

### Rollback

Revert the source-of-truth PR to restore the completed release-promotion
readiness active manifest.

## Work Item: stable-product-receipt-refresh

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0004-toolkit-excellence.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: n/a
Blocks:
- public-promotion-decision-refresh
Blocked by: product-gap-burn-down-source-of-truth

### Goal

Refresh the stable product receipts from current `adze-swarm/main` after the SRP
queue cleanup and record any meaningful drift in the product audit.

### Receipt

`just ci-product-stable` passed on 2026-05-19 from `adze-swarm/main` at commit
`e7a7862c`.

### Production Delta

Status docs only unless a proof command exposes a real product failure.

### Non-Goals

- No release or publish claim.
- No `cargo install adze-cli` claim.
- No support-tier promotion by receipt alone.

### Proof Commands

```bash
just ci-product-stable
cargo test --manifest-path testing/downstream-starter/Cargo.toml
cargo run --manifest-path testing/downstream-starter/Cargo.toml --example parse
git diff --check
```

### Rollback

Revert only the status receipt update. Do not revert code if the commands expose
a separate product failure; fix that failure in its own PR.

## Work Item: dangling-else-selected-tree-gap

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0003-glr-toolkit-productization.md
Linked spec: ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
Linked ADR: ../../docs/adr/ADZE-ADR-0003-summary-first-glr-ambiguity.md
Blocks: n/a
Blocked by: product-gap-burn-down-source-of-truth

### Goal

Fix the generated dangling-else selected-tree gap. The generated grammar should
preserve the shift/reduce conflict, select the nearest-else typed AST, and
record retained ambiguity alternatives on `AdzeDocument`.

### Receipt

The focused proof now passes with generated `[a-z]+` lexer support, generic
leaf enum extraction, tuple positional extraction, and a positive
dangling-else selected-AST plus ambiguity-summary canary.

### Production Delta

Focused runtime, macro, and tablegen fixes:

- Generated lexers recognize lowercase alpha regex tokens such as `[a-z]+`.
- Macro-generated leaf enum extraction applies to all single-field leaf
  variants instead of a special-case `Number` variant.
- Pure-Rust extraction handles tuple positional fields before named field
  matching and does not drop the cursor when extracting token children.
- The dangling-else generated parser now returns a typed selected AST and a
  document ambiguity summary.

### Non-Goals

- No raw GLR forest stability claim.
- No broad GLR Stable promotion.
- No Tree-sitter full parity claim.

### Proof Commands

```bash
cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test test_dangling_else_conflicts generated_dangling_else_selects_nearest_else_and_records_ambiguity -- --exact --nocapture
cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test test_dangling_else_conflicts -- --nocapture
cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test glr_conflict_matrix -- --nocapture
cargo test -p adze --features pure-rust --test typed_ast_contract -- --nocapture
cargo test -p adze-tablegen --test lexer_generation_comprehensive -- --nocapture
cargo test -p adze-macro -- --nocapture
git diff --check
```

### Rollback

Revert the focused fix or boundary update. Keep existing gap canaries unless a
replacement proof fully covers the selected-tree behavior.

## Work Item: generated-reduce-reduce-gap

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0003-glr-toolkit-productization.md
Linked spec: ../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
Linked ADR: ../../docs/adr/ADZE-ADR-0003-summary-first-glr-ambiguity.md
Blocks:
- public-promotion-decision-refresh
Blocked by: product-gap-burn-down-source-of-truth

### Goal

Fix generated reduce/reduce preservation and typed extraction. The generated
grammar should keep distinct wrapper reductions, the parse table should retain
the reduce/reduce cell, `parse()` should extract a deterministic selected typed
AST, and `parse_document()` should expose a native ambiguity summary.

### Receipt

The focused proof now passes with single-field non-leaf wrapper identity
preserved during grammar expansion, inferred conflict declarations for those
wrapper siblings, unresolved reduce/reduce ties preserved unless rule
precedence differentiates them, and enum extraction preserving explicit wrapper
nodes long enough to select the generated variant.

### Production Delta

Focused tablegen, GLR-core, macro, and runtime canary fixes:

- Grammar expansion no longer collapses single-field non-leaf enum wrappers
  such as `Choice::FromA(FromA)` and `Choice::FromB(FromB)` into identical
  terminal alternatives.
- Generated grammar JSON records wrapper sibling conflict declarations using
  the underlying nonterminal names so generated reduce/reduce fixtures remain
  valid ambiguity input.
- GLR automaton reduce/reduce precedence resolution preserves ties instead of
  silently picking one rule.
- Macro enum extraction unwraps `source_file` and hidden rules, while keeping
  explicit wrapper nodes visible for variant selection.
- `generated_reduce_reduce_gap` is now a positive product canary.

### Non-Goals

- No broad reduce/reduce stability claim from hand-built core tests alone.
- No raw GLR forest stability claim.
- No full GLR Stable promotion.

### Proof Commands

```bash
cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test generated_reduce_reduce_gap -- --nocapture
cargo test -p adze-tool --lib tests::single_field_non_leaf_variants_preserve_identity_for_reduce_reduce -- --exact --nocapture
cargo test -p adze-glr-core decide_reduce_reduce -- --nocapture
cargo test -p adze-glr-core --test advanced_conflict_proptest -- --nocapture
git diff --check
```

### Rollback

Revert the focused fix or boundary update. Preserve an explicit gap canary until
generated reduce/reduce behavior has deterministic product proof.

## Work Item: public-promotion-decision-refresh

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0005-release-promotion-readiness.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: n/a
Blocked by: n/a

### Goal

Refresh the release-promotion decision after the product gap burn-down receipts
are current.

### Receipt

The public promotion decision was refreshed from current `adze-swarm/main` after
the dangling-else and generated reduce/reduce product gaps were fixed. Both
public and swarm PR queues were empty. `ci-product-stable` and
`check-publishable` passed locally. PR #267 supplied the hosted supported-lane
receipt after the wrapper-preservation follow-up, including `Supported Rust
Gate`, `Test Runtime Crates`, JavaScript and Python golden tests, and `Rust
Small Result`.

Outcome: **proceed conditionally, but do not open a public PR by default**.
Public promotion still requires a fresh explicit execution goal using
`plans/release-promotion/public-promotion-pr-plan.md`.

### Production Delta

Release-promotion status only. No public promotion PR was opened.

### Non-Goals

- No public PR from this work item by default.
- No release tag, publish, signing, or workflow-token change.

### Proof Commands

```bash
gh pr list --repo EffortlessMetrics/adze --state open
gh pr list --repo EffortlessMetrics/adze-swarm --state open
gh pr checks 267 --repo EffortlessMetrics/adze-swarm
just ci-product-stable
just check-publishable
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
```

### Rollback

Revert status-only updates. If a public promotion PR is opened later, use the
rollback plan in `plans/release-promotion/public-promotion-pr-plan.md`.

## Work Item: external-scanner-recovery-proof

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0004-toolkit-excellence.md
Linked spec: ../../docs/specs/ADZE-SPEC-0005-diagnostics-and-recovery.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks:
- public-promotion-blocker-watch
Blocked by: n/a

### Goal

Keep the external-scanner product boundary honest by proving focused
parser-v4 dispatch behavior and a generated external-token diagnostic-document
canary while leaving full parser-generated external-scanner recovery as future
work until it has broader canaries.

### Receipt

PR #298 fixed parser-v4 external scanner token span reporting. The parser now
captures the pre-scan byte position, slices emitted token text from that range,
and rejects scanner-emitted tokens that are not valid in the current parser
state. PR #300 added a follow-up parser-v4 canary proving bad input in a direct
external-scanner grammar shape returns a diagnostic document with error facts.
PR #309 added a generated external-token grammar canary proving generated
`parse_document()` returns a diagnostic document with bounded byte spans and
matching point ranges for malformed input.

### Production Delta

- Parser-v4 external scanner dispatch is synchronized with the parser loop
  byte position before invoking the scanner.
- Emitted scanner tokens preserve `start`, `end`, and text from the pre-scan
  position instead of the scanner-advanced position.
- Direct parser-v4 `parse_document()` returns a diagnostic document for bad
  input in an external-scanner grammar shape.
- Generated external-token grammar `parse_document()` returns a diagnostic
  document for malformed input.
- `SUPPORT_TIERS.md` records focused external-scanner canaries while keeping
  the surface Experimental.

### Non-Goals

- No support-tier promotion.
- No stable external scanner API claim.
- No full parser-generated external-scanner recovery claim.
- No public promotion change.

### Proof Commands

```bash
cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests::test_parser_with_external_scanner -- --exact --nocapture
cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests::test_external_scanner_rejects_token_not_in_valid_symbols -- --exact --nocapture
cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests::test_external_scanner_parse_document_bad_input_returns_diagnostic_document -- --exact --nocapture
cargo test --manifest-path example/Cargo.toml external_word_example::tests::generated_external_grammar_bad_input_returns_diagnostic_document --features pure-rust -- --exact --nocapture
cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests -- --nocapture
cargo test -p adze --features external_scanners
git diff --check
```

### Rollback

Revert PR #300 to remove the direct parser-v4 diagnostic-document canary and
receipt updates. Revert PR #309 to remove the generated external-token
diagnostic-document canary and receipt updates. Revert PR #298 only if the
scanner span behavior itself needs to be rolled back. Keep external scanners
Experimental and keep full parser-generated recovery coverage listed as future
work.

## Work Item: product-objective-audit-refresh

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0005-release-promotion-readiness.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: n/a
Blocked by: n/a

### Goal

Refresh the objective audit and proof map after the parser-v4
external-scanner dispatch/span receipt lands, without implying that
parser-generated scanner recovery is complete.

### Production Delta

Status and source-of-truth docs only.

### Non-Goals

- No runtime behavior change.
- No support-tier promotion.
- No public promotion PR.

### Proof Commands

```bash
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
cargo run -q -p xtask -- check-active-goal --mode blocking
git diff --check
```

### Rollback

Revert the status-doc refresh. Do not revert PR #298 unless the parser-v4
external-scanner behavior itself needs to be rolled back.

## Work Item: release-publish-decision-preflight

Status: complete
Linked proposal: ../../docs/proposals/ADZE-PROP-0005-release-promotion-readiness.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: explicit-release-publish-workflow
Blocked by: n/a

### Goal

Prepare the explicit release/publish decision without publishing. This item
refreshes the registry boundary and local publish-readiness receipts so a human
release decision can happen from current facts.

### Production Delta

Source-of-truth status only unless a proof command exposes a real package or
claim-boundary failure.

### Receipt

On 2026-05-20 from `adze-swarm/main` at commit `390ab76f`:

- `cargo info adze-cli` reported that `adze-cli` is not present in crates.io.
- `just package-local adze-cli` packaged and verified `adze-cli v0.8.0-dev`
  with local co-release patches.
- `cargo run -q -p xtask -- verify-crates-io-install adze-cli --bin adze
  --version X.Y.Z --locked --dry-run` printed the post-publish install command
  plan and reiterated that dry-run is not registry installation proof.
- `just check-publishable` passed for `adze-common`, `adze-ir`,
  `adze-glr-core`, `adze-tablegen`, `adze-macro`, `adze-tool`, `adze-cli`, and
  `adze`.

This is a release decision preflight receipt, not a publish, tag, or install
receipt.

### Non-Goals

- No crate publish.
- No release tag.
- No signing, Cargo-token, or release-workflow mutation.
- No README claim that `cargo install adze-cli` is the supported quickstart.

### Proof Commands

```bash
cargo info adze-cli
just package-local adze-cli
cargo run -q -p xtask -- verify-crates-io-install adze-cli --bin adze --version X.Y.Z --locked --dry-run
just check-publishable
```

### Rollback

Revert only source-of-truth receipt updates. If proof commands expose a real
package or claim-boundary failure, fix that in a separate focused PR.

## Work Item: explicit-release-publish-workflow

Status: blocked
Linked proposal: ../../docs/proposals/ADZE-PROP-0005-release-promotion-readiness.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: crates-io-cli-install-receipt
Blocked by: explicit human release/publish authorization after the completed
preflight

### Goal

Publish the release surface only through an explicit release workflow. This is
outside routine swarm implementation and product-proof work.

### Non-Goals

- No automatic publish from product-proof receipts.
- No tag, signing, Cargo-token, or release-workflow mutation without explicit
  release authorization.
- No public install claim until the post-publish install receipt passes.

### Rollback

If a release or publish operation happens, do not rewrite public history. Use
the release incident process and publish a corrective release if needed.

## Work Item: crates-io-cli-install-receipt

Status: blocked
Linked proposal: ../../docs/proposals/ADZE-PROP-0005-release-promotion-readiness.md
Linked spec: ../../docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADR: ../../docs/adr/ADZE-ADR-0001-adze-document-one-parse-truth.md
Blocks: objective-completion
Blocked by: explicit-release-publish-workflow

### Goal

Close the remaining published first-use receipt gap only after the CLI release
surface exists on crates.io. Until then, the product proof remains limited to
repo-built CLI quickstarts, downstream fixtures, local package verification, and
claim-boundary checks.

### Current Receipt

On 2026-05-20, `cargo info adze-cli` was run outside the workspace and reported:

```text
error: could not find `adze-cli` in registry `https://github.com/rust-lang/crates.io-index`
```

This confirms the `cargo install adze-cli` path is still a release-surface
target, not current product proof.

### Production Delta

Source-of-truth status only until an explicit release/publish workflow runs.

### Non-Goals

- No crate publish from this work item.
- No release tag.
- No signing, Cargo-token, or release-workflow mutation.
- No README claim that `cargo install adze-cli` is the supported quickstart
  until the install receipt passes.

### Proof Commands

Pre-publish boundary:

```bash
cargo info adze-cli
just package-local adze-cli
cargo run -q -p xtask -- verify-crates-io-install adze-cli --bin adze --version X.Y.Z --locked --dry-run
```

Post-publish receipt:

```bash
cargo run -q -p xtask -- verify-crates-io-install adze-cli --bin adze --version X.Y.Z --locked
```

### Rollback

Revert only the source-of-truth receipt update. If a release or publish
operation has happened, use the release incident process instead of rewriting
public history.

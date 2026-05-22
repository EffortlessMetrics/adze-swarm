# ADZE-PROP-0016: CLI Dynamic Parse Boundary Hardening

Status: implemented
Owner: cli/product
Created: 2026-05-21
Target milestone: post-0.9 / non-release CLI hardening
Linked specs:
- docs/specs/ADZE-SPEC-0008-json-cli-wasm-projections.md
- docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
Linked ADRs:
- ADZE-ADR-0001-adze-document-one-parse-truth
- ADZE-ADR-0004-schema-versioned-projections
Linked plan:
- ../../plans/cli-dynamic-parse/implementation-plan.md
Linked issues:
- EffortlessMetrics/adze-swarm#325
Linked PRs:
- EffortlessMetrics/adze-swarm#471
- EffortlessMetrics/adze-swarm#472
- EffortlessMetrics/adze-swarm#473
Support-tier impact:
- Keeps dynamic CLI parse output below Stabilizing after boundary receipts
  proved the current feature-gated, unimplemented output surface.
Policy impact:
- Keeps CLI hardening in `EffortlessMetrics/adze-swarm`.
- Keeps release, tag, publish, signing, Cargo-token, and crates.io install
  receipt work in public `EffortlessMetrics/adze` after explicit
  authorization.

## Problem

The static `adze parse` surface now has document-backed `tree`, `sexp`, `json`,
`dot`, and explicit document projection receipts. The dynamic parse path is a
different surface: it is feature-gated, can attempt to load a shared library,
and still exits with `dynamic parse mode is currently unimplemented`.

That boundary is honest, but it needs product-trust hardening. Help text,
design sketches, support tiers, and tests must agree that dynamic loading is not
currently a supported parse-output path. Otherwise users and agents can mistake
the presence of `--dynamic` for an implemented runtime parsing contract.

## Users And Surfaces

- CLI users who try `adze parse --dynamic`.
- Maintainers reviewing release-readiness and support-tier claims.
- Agents selecting the next non-release CLI hardening task.
- Documentation readers using the dynamic-loading guide.

Affected surfaces:

- `cli/src/main.rs`
- `cli/tests/`
- `cli/README.md`
- `book/src/guide/dynamic-loading.md`
- `docs/status/SUPPORT_TIERS.md`
- source-of-truth artifacts for this lane

## Success Criteria

- `adze-swarm` remains the operating repo for this work.
- Dynamic parse output remains explicitly experimental and unimplemented unless
  a behavior PR proves otherwise.
- CLI errors and docs consistently distinguish:
  - CLI not built with the `dynamic` feature;
  - dynamic grammar path or symbol load failure;
  - successful dynamic symbol load but no parse-output implementation yet.
- The dynamic-loading guide no longer reads like a supported workflow recipe.
- Support-tier wording keeps dynamic parse output out of Stable and
  Stabilizing claims.
- No release, publish, signing, Cargo-token, public promotion, or crates.io
  install receipt work is performed.

## Proposed Shape

Treat this as a boundary-hardening lane first, not a full dynamic parser
implementation lane.

```text
source-of-truth setup
  -> dynamic parse boundary receipts and docs
  -> closeout / future implementation decision
```

If a later behavior PR implements dynamic parse output, it must either route
through a document-backed parse truth or explicitly document why the dynamic
surface is a separate compatibility layer. It must not silently create a second
parse truth.

## Alternatives Considered

### Implement full dynamic parse output immediately

Rejected for this lane. The current code can load a symbol, but full output
would need a clear runtime/parser contract and likely broader Tree-sitter
interop decisions. A small boundary-hardening lane lowers product risk without
pretending dynamic parse is ready.

### Remove `--dynamic`

Rejected. The feature-gated command shape is useful as an experimental design
surface and future compatibility target. Removing it would erase a tracked gap
rather than making it safer.

### Leave docs and tests as-is

Rejected. Release-boundary closeout depends on clear claims. A design sketch
that contains recipe-like examples for unimplemented output can mislead users.

## Specs To Create Or Update

No new spec is required for the boundary-hardening lane. The relevant contract
is already covered by:

- `ADZE-SPEC-0008` for CLI projection behavior and schema boundaries.
- `ADZE-SPEC-0011` for product proof and support-tier behavior.

## Architecture Decisions Needed

No new ADR is needed for this lane.

Existing constraints:

- `ADZE-ADR-0001`: `AdzeDocument` remains the one native parse truth.
- `ADZE-ADR-0004`: serialized CLI/WASM projections are schema-versioned.

## Implementation Campaign Shape

1. Open this focused non-release source-of-truth lane.
2. Add CLI tests and docs/support-tier wording that prove the dynamic parse
   boundary is clear and non-promoted.
3. Close out with remaining implementation options and proof requirements.

## Evidence Plan

- Source-of-truth proof:
  - `cargo run -q -p xtask -- check-active-goal --mode blocking`
  - `cargo run -q -p xtask -- check-doc-artifacts --mode blocking`
- CLI proof:
  - focused `adze-cli` tests for the feature-gated dynamic boundary
  - `cargo test -p adze-cli --features dynamic dynamic -- --nocapture`
- Hygiene:
  - `cargo fmt -p adze-cli -- --check`
  - `cargo clippy -p adze-cli --all-targets --features dynamic -- -D warnings`
  - `git diff --check`

## Risks

- The lane could become another documentation-only restatement unless the next
  PR adds executable receipts.
- The `dynamic` feature may expose platform-specific loader behavior; tests
  should avoid requiring a real system grammar library.
- Over-eager wording could imply Tree-sitter dynamic parsing is supported.

## Non-Goals

- No release tag, crate publish, signing, Cargo-token, or release workflow work.
- No `cargo install adze-cli` claim until a real crates.io install receipt
  exists.
- No public `adze` implementation PRs.
- No full dynamic parse output implementation in the setup PR.
- No stable CLI/WASM schema claim.
- No full Tree-sitter compatibility claim.

## Exit Criteria

- The active goal and plan identify exactly one ready behavior item.
- Dynamic parse docs, tests, and support tiers agree on the current boundary.
- Any remaining full implementation work is explicitly deferred with required
  proof commands.

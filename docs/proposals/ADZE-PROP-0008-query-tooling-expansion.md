# ADZE-PROP-0008: Query and Tooling Expansion

Status: accepted
Owner: runtime/tooling
Created: 2026-05-20
Target milestone: post-0.9 / non-release product polish
Linked specs:
- docs/specs/ADZE-SPEC-0011-product-proof-and-support-tiers.md
- docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
- docs/specs/ADZE-SPEC-0013-query-compatibility.md
Linked ADRs:
- ADZE-ADR-0001-adze-document-one-parse-truth
Linked plan:
- ../../plans/query-tooling-expansion/implementation-plan.md
Linked issues:
- EffortlessMetrics/adze-swarm#325
Linked PRs:
- none yet
Support-tier impact:
- Improves query/tooling proof and examples without promoting support tiers by setup.
Policy impact:
- Keeps development and proof work in `EffortlessMetrics/adze-swarm`.
- Does not authorize release, tag, publish, signing, Cargo-token, or crates.io
  install receipt work.

## Problem

The completed product and user-experience campaigns left Adze in a release-readable
state, but the active goal is paused and there is no selected routine
non-release lane. Meanwhile, `NOW_NEXT_LATER.md` still names query and tooling
expansion as the next non-release product area: broader query compatibility must
advance only through `ADZE-SPEC-0013`, and CLI/tooling polish should continue
without creating full Tree-sitter or query parity claims.

Without a fresh active goal, agents can either churn closed status lanes or drift
toward release/publish work that still requires human authorization.

## Users And Surfaces

- Tooling users need query examples and CLI/document workflows that are easy to
  exercise from generated parser outputs.
- Editor-integration users need supported query behavior and known gaps to stay
  visible.
- Maintainers need query/tooling work to remain tied to proof commands and
  support-tier boundaries.

Affected surfaces:

- `docs/specs/ADZE-SPEC-0013-query-compatibility.md`;
- `docs/reference/query-compatibility.md`;
- query examples and canaries;
- CLI/tooling smoke paths;
- `docs/status/SUPPORT_TIERS.md` and `docs/status/PRODUCT_OBJECTIVE_AUDIT.md`
  only after proof exists.

## Success Criteria

- A fresh active goal names this non-release lane in `adze-swarm`.
- Query examples and CLI/tooling receipts exercise supported subset behavior
  without implying full Tree-sitter query parity.
- Any new query/tooling claim maps to a proof command and support-tier row.
- Release/publish blockers remain tracked on issue #325 and are not treated as
  routine swarm work.

## Proposed Shape

Work in small PRs:

```text
source-of-truth setup
  -> query example and CLI smoke refresh
    -> query gap matrix and fixture receipts
      -> support-tier boundary refresh when proof exists
```

The lane should prefer runnable examples and focused canaries over broad runtime
rewrites.

## Alternatives Considered

### Continue The Paused External-Scanner Goal

Rejected. That goal is complete and explicitly says future routine non-release
work should open a fresh active goal.

### Start Release Work

Rejected. Release, publish, signing, Cargo-token, and crates.io install receipt
work still requires explicit human authorization and belongs in public `adze`.

### Claim Full Query Parity

Rejected. `ADZE-SPEC-0013` defines a documented subset and known gaps. This
lane expands proof and usability within that boundary.

## Specs To Create Or Update

No new behavior spec is required at campaign start. `ADZE-SPEC-0013` owns query
compatibility behavior. Update it only when the implementation changes the
supported subset or known-gap boundary.

## Architecture Decisions Needed

No new ADR is required at campaign start. The durable rule remains:
`AdzeDocument` is the one parse truth, and query/tooling surfaces are projections
over document-backed data.

## Implementation Campaign Shape

1. Start the query/tooling expansion active goal.
2. Refresh query examples and CLI/tooling smoke receipts for the supported
   subset.
3. Add or update gap-matrix fixtures only for behavior that is explicitly
   supported or intentionally marked as a gap.
4. Refresh support-tier and product-audit wording only after proof commands
   exist and pass.

## Evidence Plan

Focused proof:

```bash
cargo test -p adze --features query --lib query -- --nocapture
cargo run -p adze --features query --example query_highlighting
git diff --check
```

Source-of-truth proof:

```bash
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

## Risks

- Examples can accidentally read like a full query parity claim.
- CLI/tooling smoke can imply stable schema or installability claims not backed
  by release receipts.
- Agents can drift into public `adze` instead of `adze-swarm`.

## Non-Goals

- No release, tag, publish, signing, Cargo-token, or crates.io install work.
- No full Tree-sitter query parity claim.
- No support-tier promotion by setup PR.
- No broad query/runtime rewrite.
- No public `adze` implementation PRs.

## Exit Criteria

The lane can close when query/tooling examples and receipts are current for the
documented subset, known gaps remain explicit, and any support-tier or product
audit wording is refreshed without unsupported claims.

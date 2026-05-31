# Query Upstream Differential Closeout

Status: complete
Owner: runtime/query
Closed: 2026-05-31
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/query-upstream-differential.toml
Archived goal: ../../.adze/goals/archive/2026-05-31-query-upstream-differential.toml
Plan: ./implementation-plan.md
Issue: https://github.com/EffortlessMetrics/adze-swarm/issues/643
Board: https://github.com/EffortlessMetrics/adze-swarm/issues/617

## Outcome

Outcome: complete; no release or support-tier promotion implied.

This campaign answered the #643 research-board proof question with one upstream
Tree-sitter query differential canary. Work stayed in
`EffortlessMetrics/adze-swarm`; public `EffortlessMetrics/adze` remains the
release, public-intake, promotion, tag, publish, signing, Cargo-token, and
crates.io receipt surface.

## Landed Work

| Work item | PR | Result |
| --- | --- | --- |
| Source-of-truth setup | #644 | Selected the non-release query differential lane, added the named goal and implementation plan, and registered doc artifacts. |
| Upstream query differential canary | #645 | Added one `tree-sitter-json 0.24.8` capture differential receipt and Tree-sitter postfix capture syntax support. |
| Lane closeout | #646 | Marked the plan complete, archived the named goal, and returned the active manifest to paused forge standby. |

## Proof Receipts

Local proof commands recorded for the completed campaign:

```bash
python -c "import tomllib; tomllib.load(open('.adze/goals/active.toml', 'rb')); tomllib.load(open('.adze/goals/query-upstream-differential.toml', 'rb')); tomllib.load(open('policy/doc-artifacts.toml', 'rb'))"
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
cargo test -p adze --features query --lib query::compiler::tests::test_query_with_tree_sitter_postfix_capture -- --exact --nocapture
cargo test -p adze --features "pure-rust,ts-compat,query,with-grammars" --test upstream_query_differential -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat,query" --test query_differential -- --nocapture
git diff --check
```

GitHub proof on #645 included:

- `Rust Small Result`: pass.
- `Product Proof Result`: pass.
- `CI Lane Whitelist`: pass.
- `Source of Truth`: pass.
- `GLR Invariants`: pass.

Post-merge main CI Policy run `26710608995` passed on
`f45a366b3c9087d2f335ddcc79960bd8ea4b54e8` after #646.

## Claim Boundaries

This closeout does not claim or authorize:

- full Tree-sitter query parity;
- directive, highlighting, injection, or GLR-forest-wide query semantics;
- support-tier promotion;
- public `adze` implementation work;
- public promotion;
- release tags;
- crate publishing;
- signing workflow changes;
- Cargo-token work;
- real crates.io install verification;
- a public `cargo install adze-cli` claim.

`AdzeDocument` remains the canonical parse product. Tree-sitter compatibility
and query matching remain projections over selected-tree facts.

## Remaining Blockers

- #598 remains open for CX53 runner readiness or admin decision evidence.
- #325 remains open for release authorization and crates.io install receipt
  tracking.

## Next Step

No routine query differential work remains in this campaign. Keep the active
manifest paused until #617 or a successor board selects a new research question,
decision packet, or bounded proof spike. Any future query or Tree-sitter parity
work should open as a fresh issue with explicit scope, proof commands, and
claim boundaries.

# User Experience Hardening Closeout

Status: complete
Owner: droid-factory
Closed: 2026-05-20
Active goal: ../../.adze/goals/active.toml
Named goal: ../../.adze/goals/user-experience-hardening.toml
Plan: ./implementation-plan.md
Proposal: ../../docs/proposals/ADZE-PROP-0006-user-experience-hardening.md

## Outcome

Outcome: **complete; no release or publish authorization implied**.

This campaign made the already-proven Adze toolkit easier to adopt without
changing release claims. Work remained in `EffortlessMetrics/adze-swarm`; public
`EffortlessMetrics/adze` remains the release, public-intake, tag, signing, and
publish surface.

## Landed Work

| Work item | PRs | Result |
| --- | --- | --- |
| Source-of-truth setup | #350 | Opened the user-experience hardening proposal, plan, active goal, and artifact registration. |
| API navigation polish | #351 | Refreshed the API choice guide and README path to keep `grammar::parse()` as the beginner path and `parse_document()` as the tooling path. |
| Starter example polish | #352 | Improved generated starter README guidance and local path dependency behavior for checkout-built `adze init` flows. |
| Diagnostics/query/Tree-sitter walkthroughs | #353 | Added diagnostics and recovery reference guidance with proof commands and claim boundaries. |
| Local proof-loop friction | #329, #354 | Recorded and validated the Windows supported-gate PDB-pressure mitigation. |
| Performance receipt guidance | #231, #355 | Recorded the advisory `product-smoke` benchmark receipt and non-claim boundaries. |

## Proof Receipts

Representative proof commands from the campaign:

```bash
cargo test -p adze-cli test_init_default_cwd_generates_buildable_project -- --exact --nocapture
cargo test -p adze-cli test_init_cargo_toml_references_adze_dependency -- --exact --nocapture
cargo test -p adze-cli getting_started_quickstart_builds_parses_and_reports_diagnostics -- --exact --nocapture
cargo test --manifest-path testing/downstream-starter/Cargo.toml
cargo run --manifest-path testing/downstream-starter/Cargo.toml --example parse
cargo run -p adze --features "pure-rust,glr,serialization" --example diagnostics_recovery
cargo test -p adze --features query --lib query -- --nocapture
cargo test -p adze --features "pure-rust,ts-compat" --test ts_compat_selected_tree -- --nocapture
cargo run -q -p xtask -- perf-receipt --profile product-smoke
just ci-supported
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

GitHub receipts across the closeout PRs included `Rust Small Result`, Source of
Truth, CI Lane Whitelist, GLR Invariants, Docs Gate, PR Gate Success, and
Product Proof where relevant.

## Claim Boundaries

This closeout does not claim:

- a release tag exists;
- crates were published;
- `cargo install adze-cli` works from crates.io;
- Cargo-token, signing, publish, or release workflows changed;
- benchmarks are stable throughput or memory-use claims;
- `ci-product-stable` is branch-protection-required;
- full GLR, full Tree-sitter, full query, stable CLI/WASM schema, raw GLR forest,
  or incremental performance is Stable.

## Next Step

No routine UX-hardening work remains in this campaign. Future non-release work
should open a new active goal in `adze-swarm`. Release/publish work must remain
blocked until explicit human authorization, and actual release execution must use
public `EffortlessMetrics/adze`.
